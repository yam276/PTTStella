"""Collect and relocate explicit Mach-O runtime dependencies, never a whole prefix."""
import hashlib
import json
from pathlib import Path
import re
import shutil
import subprocess
import tarfile

SYSTEM = ('/System/Library/', '/usr/lib/')
LOADS = {'LC_LOAD_DYLIB', 'LC_LOAD_WEAK_DYLIB', 'LC_REEXPORT_DYLIB',
         'LC_LOAD_UPWARD_DYLIB', 'LC_LAZY_LOAD_DYLIB'}
# Full source exports include checked-out submodules (libjxl is about 123 MiB).
# Notice extraction has its own much smaller per-file and aggregate limits.
SOURCE_ARCHIVE_LIMIT = 128 * 1024 * 1024


def run(*command):
    return subprocess.check_output(command, text=True, stderr=subprocess.PIPE, timeout=60)


def digest(path):
    with path.open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def parse_commands(text):
    """Read names with spaces intact; do not confuse LC_ID_DYLIB with an import."""
    imports, rpaths, identity = [], [], None
    command = None
    for line in text.splitlines():
        line = line.strip()
        if line.startswith('cmd '):
            command = line[4:]
        elif command in LOADS | {'LC_ID_DYLIB', 'LC_RPATH'}:
            match = re.fullmatch(r'(?:name|path) (.+) \(offset \d+\)', line)
            if match:
                name = match[1]
                if command == 'LC_ID_DYLIB':
                    identity = name
                elif command == 'LC_RPATH':
                    rpaths.append(name)
                else:
                    imports.append(name)
    return {'imports': imports, 'rpaths': rpaths, 'id': identity}


def inspect(path):
    return parse_commands(run('otool', '-l', str(path)))


def expand_loader_path(value, owner, executable):
    for token, base in (('@loader_path', owner.parent),
                        ('@executable_path', executable.parent)):
        if value == token:
            return base
        if value.startswith(token + '/'):
            return base / value[len(token) + 1:]
    if value.startswith('/') or Path(value).is_absolute():
        return Path(value)
    raise ValueError(f'Unsupported loader path: {value}')


def resolve_import(name, owner, executable, rpaths, allowed):
    if name.startswith(SYSTEM):
        if '..' in Path(name).parts:
            raise ValueError(f'Parent traversal in system loader path: {name}')
        return None

    if name.startswith('@rpath/'):
        candidates = [expand_loader_path(path, owner, executable) / name[len('@rpath/'):]
                      for path in rpaths]
    else:
        candidates = [expand_loader_path(name, owner, executable)]
    for candidate in candidates:
        if candidate.is_file():
            resolved = candidate.resolve(strict=True)
            if not any(resolved.is_relative_to(prefix) for prefix in allowed):
                raise ValueError(f'Runtime outside explicit allowed prefixes: {resolved}')
            return resolved
    raise ValueError(f'Unresolved import in {owner}: {name}')


def closure(executable, allowed):
    executable = executable.resolve(strict=True)
    allowed = tuple(p.resolve(strict=True) for p in allowed)
    queue, records, names = [(executable, ())], {}, {}
    total = 0
    while queue:
        path, inherited_rpaths = queue.pop()
        if path in records:
            continue
        if len(records) >= 129:
            raise ValueError('Runtime exceeds 128 libraries')
        size = path.stat().st_size
        if size > 128 * 1024 * 1024:
            raise ValueError(f'Runtime file exceeds 128 MiB: {path}')
        if path != executable:
            total += size
            if total > 384 * 1024 * 1024:
                raise ValueError('Runtime exceeds 384 MiB')
            if path.name in names and names[path.name] != path:
                raise ValueError(f'Colliding library basename: {path.name}')
            names[path.name] = path
        info = inspect(path)
        # dyld searches the loading chain's run paths. Expand a parent's
        # @loader_path in that parent's context before passing it to a child.
        search = tuple(dict.fromkeys([
            *(str(expand_loader_path(value, path, executable)) for value in info['rpaths']),
            *inherited_rpaths,
        ]))
        edges = {}
        for name in info['imports']:
            resolved = resolve_import(name, path, executable, search, allowed)
            if resolved:
                edges[name] = resolved
                queue.append((resolved, search))
        records[path] = {**info, 'edges': edges, 'sha256': digest(path), 'bytes': size}
    return records


def source_notices(source, archive, destination):
    expected = next(c['checksumValue'] for c in source['checksums'] if c['algorithm'] == 'SHA256')
    if archive.stat().st_size > SOURCE_ARCHIVE_LIMIT or digest(archive) != expected:
        raise ValueError('Source archive digest/size mismatch')
    destination.mkdir()
    records = []
    total = 0
    with tarfile.open(archive) as tar:
        for member in tar:
            parts = Path(member.name).parts
            if not member.isfile() or not parts or '..' in parts or member.name.startswith('/'):
                continue
            if not (parts[-1].casefold().startswith(('license', 'licence', 'copying', 'copyright', 'notice'))
                    or parts[-1].casefold() in ('ftl.txt', 'gplv2.txt')
                    or any(p.casefold() in ('licenses', 'licences') for p in parts[:-1])):
                continue
            if member.size > 128 * 1024:
                raise ValueError('Unexpectedly large source notice')
            total += member.size
            if total > 8 * 1024 * 1024 or len(records) >= 256:
                raise ValueError('Too many source notices')
            out = destination / '__'.join(parts)
            if out.exists():
                raise ValueError('Colliding source notice names')
            stream = tar.extractfile(member)
            with stream:
                data = stream.read(128 * 1024 + 1)
            if len(data) != member.size:
                raise ValueError('Truncated source notice')
            out.write_bytes(data)
            records.append({'file': out.name, 'sha256': digest(out)})
    if not records:
        raise ValueError('No notices in source archive')
    (destination / 'provenance.json').write_text(json.dumps({'source': source, 'archive_sha256': expected,
                                                            'files': records}, indent=2) + '\n')
    return records


def external_sources(records, executable, manifest):
    """Admit source-built dylibs through bounded local hashes, never a fake keg."""
    if manifest is None:
        return [], set()
    manifest = manifest.resolve(strict=True)
    if not 0 < manifest.stat().st_size <= 2 * 1024 * 1024:
        raise ValueError('Invalid runtime source manifest size')
    root = manifest.parent
    data = json.loads(manifest.read_text(encoding='utf-8'))
    if not isinstance(data, dict):
        raise ValueError('Expected a runtime source manifest object')
    packages = data.get('packages')
    if data.get('schema') != 1 or not isinstance(packages, list) or not 0 < len(packages) <= 128:
        raise ValueError('Expected runtime source manifest schema 1 with bounded packages')

    def local_file(value):
        if not isinstance(value, str) or Path(value).is_absolute() or '..' in Path(value).parts:
            raise ValueError('Invalid runtime source input path')
        path = (root / value).resolve(strict=True)
        if not path.is_file() or not path.is_relative_to(root):
            raise ValueError('Runtime source input escapes its manifest directory')
        return path

    def checked(value, maximum):
        if (not isinstance(value, dict) or not isinstance(value.get('sha256'), str)
                or not re.fullmatch(r'[0-9a-f]{64}', value['sha256'])):
            raise ValueError('Expected a SHA256-bound runtime source input')
        path = local_file(value.get('file'))
        if not 0 < path.stat().st_size <= maximum:
            raise ValueError('Runtime source input exceeds size limit')
        if digest(path) != value['sha256']:
            raise ValueError('Runtime source input hash mismatch: ' + str(path))
        return path

    accepted, covered, labels = [], set(), set()
    for package in packages:
        if not isinstance(package, dict):
            raise ValueError('Expected a runtime source package object')
        label, source = package.get('package'), package.get('source')
        if not isinstance(label, str) or not re.fullmatch(r'[A-Za-z0-9][A-Za-z0-9_.+-]{0,127}', label):
            raise ValueError('Invalid runtime source package label')
        if label in labels:
            raise ValueError('Duplicate runtime source package')
        labels.add(label)
        if not isinstance(source, dict) or any(
                not isinstance(source.get(key), str) or not 0 < len(source[key]) <= 4096
                for key in ('name', 'versionInfo', 'downloadLocation')):
            raise ValueError('Expected explicit runtime source identity')
        checksums = source.get('checksums')
        if not isinstance(checksums, list) or len(checksums) != 1 or not isinstance(checksums[0], dict) or checksums[0].get('algorithm') != 'SHA256':
            raise ValueError('Expected one runtime source archive SHA256')
        archive = checked({'file': package.get('archive'), 'sha256': checksums[0].get('checksumValue')},
                          SOURCE_ARCHIVE_LIMIT)
        receipt = checked(package.get('build_record'), 2 * 1024 * 1024)
        build = json.loads(receipt.read_text(encoding='utf-8'))
        if not isinstance(build, dict) or build.get('status') != 'completed' or build.get('exit') != 0:
            raise ValueError('Runtime source build did not complete successfully')
        if build.get('source_archive_sha256') != checksums[0]['checksumValue']:
            raise ValueError('Runtime build record does not match its source archive')
        libraries = package.get('libraries')
        if not isinstance(libraries, list) or len(libraries) > 128:
            raise ValueError('Invalid runtime source library list')
        kind = package.get('kind', 'runtime')
        if kind not in ('runtime', 'build-input') or (kind == 'runtime') != bool(libraries):
            raise ValueError('Runtime owners and source-only build inputs must be explicit')
        owned = []
        for library in libraries:
            path = checked(library, 128 * 1024 * 1024)
            if path == executable or 'Cellar' in path.parts or path not in records:
                raise ValueError('Source manifest library is not an external reachable runtime')
            if path in covered:
                raise ValueError('Duplicate runtime source ownership')
            if records[path]['sha256'] != library['sha256']:
                raise ValueError('Source manifest does not match inspected runtime')
            if not isinstance(build.get('runtime_outputs'), dict) or build['runtime_outputs'].get(
                    library['file']) != library['sha256']:
                raise ValueError('Runtime build record does not match its output library')
            covered.add(path)
            owned.append(path)
        accepted.append({'package': label, 'source': source, 'archive': archive,
                         'build_record': receipt, 'build_record_sha256': package['build_record']['sha256'],
                         'libraries': owned, 'kind': kind})
    return accepted, covered


def copy_notices(records, destination, source_archives=None, *, executable=None,
                 source_manifest=None):
    """Keep the installed formula, source hashes, receipt, SBOM and actual notices.

    This inventory is not a declaration of distribution readiness or a substitute
    for corresponding-source delivery. The caller records that separate gate.
    """
    external, covered = external_sources(records, executable, source_manifest)
    unknown = [path for path in records if path != executable and 'Cellar' not in path.parts
               and path not in covered]
    if unknown:
        raise ValueError('Runtime needs explicit source/notice provenance: '
                         + ', '.join(str(path) for path in unknown))
    destination.mkdir()
    packages = {}
    for path in records:
        if 'Cellar' not in path.parts:
            continue
        at = path.parts.index('Cellar')
        keg = Path(*path.parts[:at + 3])
        if keg in packages:
            continue
        sbom = json.loads((keg / 'sbom.spdx.json').read_text())
        source = next(p for p in sbom['packages'] if p['SPDXID'].endswith('-src'))
        label = keg.parent.name + '-' + keg.name
        target = destination / label
        target.mkdir()
        selected = {p for p in keg.iterdir() if p.is_file()
                    and p.name.casefold().startswith(('license', 'licence', 'copying', 'copyright', 'notice'))}
        selected.update(p for folder in ('LICENSES', 'licenses')
                        for p in (keg / folder).rglob('*') if p.is_file())
        selected.update((keg / 'INSTALL_RECEIPT.json', keg / 'sbom.spdx.json'))
        selected.update((keg / '.brew').glob('*.rb'))
        notices = []
        for file in sorted(selected):
            relative = file.relative_to(keg)
            out = target / relative
            out.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(file, out)
            notices.append({'file': str(relative), 'sha256': digest(out)})
        found = any(f.name not in ('INSTALL_RECEIPT.json', 'sbom.spdx.json')
                    and f.suffix != '.rb' for f in selected)
        if not found:
            archives = list(source_archives.glob(label + '.tar.*')) if source_archives else []
            if len(archives) != 1:
                raise ValueError(f'{label} needs its exact SBOM source archive for missing notices')
            upstream = source_notices(source, archives[0], target / 'upstream')
            notices.extend({**entry, 'file': 'upstream/' + entry['file']} for entry in upstream)
        packages[keg] = {'package': label, 'source': source, 'files': notices,
                         'notice_files_found': True, 'notices_from_archive': not found}
    inventory = list(packages.values())
    for package in external:
        target = destination / ('source-built-' + package['package'])
        notices = source_notices(package['source'], package['archive'], target)
        shutil.copy2(package['build_record'], target / 'build-record.json')
        if digest(target / 'build-record.json') != package['build_record_sha256']:
            raise ValueError('Runtime build record changed during notice collection')
        inventory.append({'package': package['package'], 'source': package['source'],
                          'kind': package['kind'], 'libraries': [str(p) for p in package['libraries']],
                          'files': notices, 'notice_files_found': True, 'notices_from_archive': True,
                          'build_record_sha256': package['build_record_sha256']})
    (destination / 'inventory.json').write_text(json.dumps(inventory, indent=2) + '\n')
    return inventory


def install(executable, binary, frameworks, notices, allowed, source_archives=None, *,
            source_manifest=None):
    records = closure(executable, allowed)
    inventory = copy_notices(records, notices, source_archives,
                             executable=executable.resolve(strict=True), source_manifest=source_manifest)
    frameworks.mkdir()
    mapping = {}
    for original in records:
        out = binary if original == executable.resolve() else frameworks / original.name
        shutil.copy2(original, out)
        if digest(out) != records[original]['sha256']:
            raise ValueError('Runtime input changed during staging: ' + str(original))
        out.chmod(0o755)
        mapping[original] = out
    manifest = []
    for original, out in mapping.items():
        record = records[original]
        command = ['install_name_tool']
        if original != executable.resolve():
            command += ['-id', '@rpath/' + out.name]
        for name, child in record['edges'].items():
            command += ['-change', name, '@rpath/' + child.name]
        for path in dict.fromkeys(record['rpaths']):
            command += ['-delete_rpath', path]
        # The executable owns the run path; dependent dylibs inherit it. Adding
        # the same path to every dylib is unnecessary and may exceed a vendor
        # library's load-command header space.
        expected_rpaths = ['@executable_path/../Frameworks'] if out == binary else []
        for path in expected_rpaths:
            command += ['-add_rpath', path]
        command += [str(out)]
        run(*command)
        check = inspect(out)
        for name in check['imports']:
            if not name.startswith(SYSTEM) and not (
                    name.startswith('@rpath/') and (frameworks / name[len('@rpath/'):]).is_file()):
                raise ValueError(f'Unbundled relocated dependency: {name}')
        if check['rpaths'] != expected_rpaths:
            raise ValueError(f'Unexpected relocated rpaths: {out}')
        run('codesign', '--force', '--sign', '-', '--timestamp=none', str(out))
        manifest.append({'file': str(out.relative_to(binary.parent.parent)),
                         'original': str(original), 'original_sha256': record['sha256'],
                         'bundled_sha256': digest(out), 'bytes': out.stat().st_size,
                         'imports': check['imports'], 'rpaths': check['rpaths']})
    return {'files': manifest, 'packages': inventory,
            'source_delivery': 'not_ready; inventory has source URLs/hashes and build recipes only',
            'dlopen_dependencies': 'requires runtime trace of each supported codec/backend'}


def reuse_candidate(executable, binary, frameworks, notices, candidate):
    """Reuse byte-identical signed libraries and their recorded source notices.

    The package report binds the old final executable: its signature changes
    after the earlier link-stage runtime manifest was written. Libraries retain
    their manifest hashes. This is transport of an existing closure, not a build.
    """
    candidate = Path(candidate)
    if candidate.is_symlink() or not candidate.is_dir():
        raise ValueError('Expected a regular candidate App directory')
    candidate = candidate.resolve(strict=True)
    contents = candidate / 'Contents'
    resources = contents / 'Resources'
    if any(path.is_symlink() or not path.is_dir()
           for path in (contents, resources, contents / 'MacOS', contents / 'Frameworks')):
        raise ValueError('Candidate payload directories must be regular directories')
    payloads = [candidate.parent / 'package-report.json',
                resources / 'candidate.json', resources / 'runtime-manifest.json']
    records = []
    for path in payloads:
        if path.is_symlink() or not path.is_file() or not 0 < path.stat().st_size <= 8 * 1024 * 1024:
            raise ValueError('Missing or invalid candidate provenance')
        records.append(json.loads(path.read_text(encoding='utf-8')))
    report, descriptor, previous = records
    arch = run('lipo', '-archs', str(executable)).strip()
    target = {'arm64': 'aarch64-apple-darwin', 'x86_64': 'x86_64-apple-darwin'}.get(arch)
    if (target is None or any(record.get('target') != target
                             or record.get('features') != 'native-player-probe'
                             or record.get('status') != 'bundled'
                             for record in (report, descriptor))):
        raise ValueError('Candidate architecture/features differ')
    old_binary = contents / 'MacOS/native'
    if (old_binary.is_symlink() or not old_binary.is_file()
            or digest(old_binary) != report.get('executable_sha256')):
        raise ValueError('Candidate executable hash mismatch')
    run('codesign', '--verify', '--deep', '--strict', str(candidate))
    files = previous.get('files')
    if not isinstance(files, list) or not 2 <= len(files) <= 129:
        raise ValueError('Invalid candidate runtime inventory')
    old_frameworks = contents / 'Frameworks'
    entries = {}
    main = 0
    for entry in files:
        name = entry['file']
        if name == 'MacOS/native':
            main += 1
            continue
        if not re.fullmatch(r'Frameworks/[A-Za-z0-9_.+-]+\.dylib', name) or name in entries:
            raise ValueError('Invalid or duplicate candidate library')
        path = contents / name
        if (path.is_symlink() or not path.is_file() or not 0 < path.stat().st_size <= 128 * 1024 * 1024
                or digest(path) != entry.get('bundled_sha256')):
            raise ValueError('Candidate library hash mismatch')
        information = inspect(path)
        if information['imports'] != entry['imports'] or information['rpaths']:
            raise ValueError('Candidate library loader metadata mismatch')
        run('lipo', str(path), '-verify_arch', arch)
        entries[name] = entry
    if (main != 1 or sum((contents / name).stat().st_size for name in entries) > 384 * 1024 * 1024
            or old_frameworks.is_symlink()
            or {p.name for p in old_frameworks.iterdir()} != {Path(name).name for name in entries}):
        raise ValueError('Candidate runtime inventory differs from Frameworks')
    allowed_imports = {'@rpath/' + Path(name).name for name in entries}

    def check_imports(information):
        for name in information['imports']:
            if name.startswith(SYSTEM) and '..' not in Path(name).parts:
                continue
            if name not in allowed_imports:
                raise ValueError('Unbundled reused runtime import: ' + name)

    for name in entries:
        check_imports(inspect(contents / name))
    source_info = inspect(executable)
    relocations = {}
    source_libraries = {}
    for name in source_info['imports']:
        if name in allowed_imports or name.startswith(SYSTEM):
            continue
        path = Path(name)
        if not path.is_absolute() or '..' in path.parts:
            raise ValueError('Unbundled reused runtime import: ' + name)
        try:
            original = path.resolve(strict=True)
            if not original.is_file() or not 0 < original.stat().st_size <= 128 * 1024 * 1024:
                raise ValueError('Invalid reused source library: ' + name)
            checksum = digest(original)
        except OSError as error:
            raise ValueError('Missing reused source library: ' + name) from error
        matches = []
        for destination, entry in entries.items():
            link = entry.get('link_source', {'file': entry.get('original'),
                                              'sha256': entry.get('original_sha256')})
            if (isinstance(link, dict) and isinstance(link.get('file'), str)
                    and link['file'] == str(original) and link.get('sha256') == checksum):
                matches.append(destination)
        if len(matches) != 1:
            raise ValueError('Reused import differs from recorded source library: ' + name)
        relocations[name] = '@rpath/' + Path(matches[0]).name
        source_libraries[str(original)] = checksum
    check_imports({'imports': [relocations.get(name, name) for name in source_info['imports']]})
    old_notices = resources / 'NativeRuntimeNotices'
    if old_notices.is_symlink() or not old_notices.is_dir():
        raise ValueError('Missing candidate notices')
    inventory = old_notices / 'inventory.json'
    if (inventory.is_symlink() or not inventory.is_file()
            or not 0 < inventory.stat().st_size <= 8 * 1024 * 1024
            or not isinstance(previous.get('packages'), list) or not previous['packages']
            or json.loads(inventory.read_text(encoding='utf-8')) != previous['packages']):
        raise ValueError('Candidate notice inventory differs from provenance')
    notice_hashes = {}
    for path in old_notices.rglob('*'):
        if path.is_symlink() or not (path.is_file() or path.is_dir()):
            raise ValueError('Candidate notices must be regular files/directories')
        if path.is_file():
            # Source archives can contain empty generated LICENSE placeholders
            # alongside the actual notices. Preserve those exact signed bytes;
            # this transport does not reinterpret them as license coverage.
            if path.stat().st_size > 4 * 1024 * 1024 or len(notice_hashes) >= 8192:
                raise ValueError('Candidate notices exceed bounds')
            notice_hashes[str(path.relative_to(old_notices))] = digest(path)
    if not notice_hashes or sum(p.stat().st_size for p in old_notices.rglob('*') if p.is_file()) > 64 * 1024 * 1024:
        raise ValueError('Candidate notice inventory is empty or too large')
    before = {str(path): digest(path) for path in [*payloads, old_binary, executable]}
    before.update(source_libraries)
    frameworks.mkdir()
    shutil.copytree(old_notices, notices)
    result = []
    for name, entry in entries.items():
        source, destination = contents / name, frameworks / Path(name).name
        shutil.copy2(source, destination)
        if digest(destination) != entry['bundled_sha256'] or digest(source) != entry['bundled_sha256']:
            raise ValueError('Candidate library changed during reuse')
        reused = {**entry, 'original': str(source), 'original_sha256': digest(source)}
        # Retain the original link input across repeated package transports.
        # The immediately previous candidate contains relocated/signed bytes.
        link = entry.get('link_source', {'file': entry.get('original'),
                                          'sha256': entry.get('original_sha256')})
        if (isinstance(link, dict) and isinstance(link.get('file'), str)
                and Path(link['file']).is_absolute()
                and re.fullmatch(r'[0-9a-f]{64}', str(link.get('sha256')))):
            reused['link_source'] = dict(link)
        result.append(reused)
    shutil.copy2(executable, binary)
    command = ['install_name_tool']
    for original, destination in relocations.items():
        command += ['-change', original, destination]
    for path in dict.fromkeys(source_info['rpaths']):
        command += ['-delete_rpath', path]
    command += ['-add_rpath', '@executable_path/../Frameworks', str(binary)]
    run(*command)
    installed = inspect(binary)
    check_imports(installed)
    if installed['rpaths'] != ['@executable_path/../Frameworks']:
        raise ValueError('Unexpected reused executable rpaths')
    run('codesign', '--force', '--sign', '-', '--timestamp=none', str(binary))
    for name, checksum in notice_hashes.items():
        if digest(notices / name) != checksum or digest(old_notices / name) != checksum:
            raise ValueError('Candidate notices changed during reuse')
    if before != {path: digest(Path(path)) for path in before}:
        raise ValueError('Candidate provenance changed during reuse')
    provenance = notices / 'reused-runtime'
    provenance.mkdir(exist_ok=True)
    snapshot = provenance / (before[str(payloads[2])] + '.json')
    if snapshot.exists():
        if digest(snapshot) != before[str(payloads[2])]:
            raise ValueError('Conflicting reused runtime provenance')
    else:
        shutil.copy2(payloads[2], snapshot)
    result.insert(0, {'file': 'MacOS/native', 'original': str(executable),
                      'original_sha256': before[str(executable)], 'bundled_sha256': digest(binary),
                      'bytes': binary.stat().st_size, 'imports': installed['imports'],
                      'rpaths': installed['rpaths']})
    return {'files': result, 'packages': previous['packages'],
            'source_delivery': previous['source_delivery'],
            'dlopen_dependencies': previous['dlopen_dependencies'],
            'reused_candidate': {'executable_sha256': before[str(old_binary)],
                                 'report_sha256': before[str(payloads[0])],
                                 'runtime_manifest_sha256': before[str(payloads[2])],
                                 'source_import_relocations': relocations,
                                 'scope': 'Unmodified signed runtime closure and notices; not a new runtime build or native acceptance.'}}

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
    if archive.stat().st_size > 32 * 1024 * 1024 or digest(archive) != expected:
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


def copy_notices(records, destination, source_archives=None, *, executable=None):
    """Keep the installed formula, source hashes, receipt, SBOM and actual notices.

    This inventory is not a declaration of distribution readiness or a substitute
    for corresponding-source delivery. The caller records that separate gate.
    """
    unknown = [path for path in records if path != executable and 'Cellar' not in path.parts]
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
    (destination / 'inventory.json').write_text(json.dumps(inventory, indent=2) + '\n')
    return inventory


def install(executable, binary, frameworks, notices, allowed, source_archives=None):
    records = closure(executable, allowed)
    inventory = copy_notices(records, notices, source_archives,
                             executable=executable.resolve(strict=True))
    frameworks.mkdir()
    mapping = {}
    for original in records:
        out = binary if original == executable.resolve() else frameworks / original.name
        shutil.copy2(original, out)
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

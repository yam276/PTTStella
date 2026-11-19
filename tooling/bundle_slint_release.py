"""Slint release downloads and corresponding-source sets for the native runner.

Called by bundle_release.py. Builds the `pttstella` Slint entry with native
video, packages it under the product identity, and writes one source set per
platform. Never launches the GUI, signs with a real identity or uploads.
"""
import hashlib
import json
import os
from pathlib import Path
import plistlib
import re
import shutil
import subprocess
import tarfile
import tempfile
import time
import tomllib
import urllib.error
import urllib.parse
import urllib.request
import zipfile

import bundle_common

ROOT = Path(__file__).resolve().parent.parent
FEATURES = bundle_common.SLINT_FEATURES
NAME = 'PTTStella'
IDENTIFIER = tomllib.loads((ROOT/'Dioxus.toml').read_text(encoding='utf-8'))['bundle']['identifier']
GPL_NOTICE = 'PTTStella-GPL-3.0-or-later.txt'
# Official mpv LGPL build for Windows x64 and the signed source archive of the same tag.
MPV_WINDOWS = {
    'version': '0.42.0', 'license': 'LGPL-2.1-or-later',
    'archive_url': 'https://github.com/mpv-player/mpv/releases/download/v0.42.0/'
                   'libmpv-v0.42.0-x86_64-w64-mingw32-lgpl.zip',
    'archive_sha256': '8e5f77430efc53ef175804ba6618527e3af0448a0dcabf04d3cc72659d1a6fce',
    'source_url': 'https://github.com/mpv-player/mpv/archive/refs/tags/v0.42.0.tar.gz',
    'source_sha256': 'bcfebbfa2b65a6d88dc97ea18117831072373e38b5785a2afb874ed4df5070dd',
    'source_signature_url': 'https://github.com/mpv-player/mpv/releases/download/v0.42.0/mpv-0.42.0.tar.gz.asc',
}
MAX_DOWNLOAD = 128*1024*1024
MAX_SOURCES = 1536*1024*1024


def digest(path):
    with path.open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def run(*args, cwd=ROOT, env=None, timeout=600):
    return subprocess.run(args, cwd=cwd, env=env, check=True, capture_output=True,
                          text=True, encoding='utf-8', timeout=timeout).stdout.strip()


def download(url, path, expected=None, limit=MAX_DOWNLOAD):
    """Bounded HTTPS download; a mismatching hash leaves no file behind."""
    if urllib.parse.urlparse(url).scheme != 'https':
        raise ValueError('Only HTTPS sources are downloaded: '+url)
    for attempt in range(3):
        try:
            request = urllib.request.Request(url, headers={'User-Agent': 'PTTStella-release-packager'})
            with urllib.request.urlopen(request, timeout=120) as response, path.open('wb') as stream:
                if not response.geturl().startswith('https://'):
                    raise ValueError('Source redirected away from HTTPS: '+url)
                size = 0
                while chunk := response.read(1024*1024):
                    size += len(chunk)
                    if size > limit:
                        raise ValueError('Source exceeds the download limit: '+url)
                    stream.write(chunk)
            break
        except (OSError, urllib.error.URLError):
            path.unlink(missing_ok=True)
            if attempt == 2:
                raise
            time.sleep(5*(attempt+1))
    if expected is not None and digest(path) != expected:
        path.unlink()
        raise ValueError('Downloaded source hash mismatch: '+url)
    return path


def build(env=None):
    """Release build of the Slint entry; returns the executable and Skia's built license."""
    outputs = {}
    executable = bundle_common.build(env=env, build_outputs=outputs)
    licenses = [d/'skia/LICENSE_SKIA' for d in outputs.get('skia-bindings', []) if (d/'skia/LICENSE_SKIA').is_file()]
    if len(set(map(digest, licenses))) != 1:
        raise RuntimeError('Expected one built Skia license from skia-bindings')
    return executable, licenses[0]


def rust_sources():
    """Directions to every locked crate; crates.io archives are immutable once published."""
    lock = tomllib.loads((ROOT/'Cargo.lock').read_text(encoding='utf-8'))
    crates = []
    for package in lock['package']:
        source = package.get('source', '')
        if source.startswith('registry+https://github.com/rust-lang/crates.io-index'):
            name, version = package['name'], package['version']
            crates.append({'name': name, 'version': version, 'sha256': package['checksum'],
                           'url': f'https://static.crates.io/crates/{name}/{name}-{version}.crate'})
        elif source:
            raise ValueError('Unexpected non-crates.io dependency source: '+package['name'])
    metadata = json.loads(run('cargo', 'metadata', '--locked', '--format-version=1', '--features', FEATURES))
    skia = next(p for p in metadata['packages'] if p['name'] == 'skia-bindings')
    tag = (skia.get('metadata') or {}).get('skia')
    if not tag:
        raise ValueError('skia-bindings does not declare its Skia source tag')
    return {'crates': crates, 'path_crates': 'vendor/ in the PTTStella source at the same commit',
            'skia': {'bindings': skia['version'], 'source_tag': tag,
                     'source_url': f'https://github.com/rust-skia/skia/archive/refs/tags/{tag}.tar.gz',
                     'build': 'skia-bindings release binaries; rebuild from the tag with '
                              'SKIA_BUILD_FROM_SOURCE / the skia-bindings build script'}}


def source_set(staging, stem, commit, native, notes):
    """One tar per platform: the source manifest plus any native runtime archives."""
    root = staging/(stem+'-sources')
    root.mkdir()
    manifest = {'format': 'PTTStella.CorrespondingSource', 'version': 1,
                'pttstella': {'commit': commit, 'license': 'GPL-3.0-or-later',
                              'url': f'https://github.com/yam276/PTTStella/archive/{commit}.tar.gz'},
                'rust': rust_sources(), 'native': [], 'notes': notes}
    shutil.copy2(ROOT/'LICENSE', root/GPL_NOTICE)
    shutil.copy2(ROOT/'Cargo.lock', root/'Cargo.lock')
    total = 0
    for entry in native:
        record = {key: value for key, value in entry.items() if key != 'path'}
        if entry.get('path') is not None:
            target = root/'native'/entry['file']
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(entry['path'], target)
            total += target.stat().st_size
            if total > MAX_SOURCES or digest(target) != entry['sha256']:
                raise ValueError('Native source set exceeds its limit or changed: '+entry['file'])
        manifest['native'].append(record)
    missing = [e for e in manifest['native'] if e.get('status') != 'included']
    manifest['status'] = 'complete' if not missing else 'partial'
    (root/'SOURCES.json').write_text(json.dumps(manifest, indent=2, ensure_ascii=False)+'\n', encoding='utf-8')
    archive = staging/(stem+'-sources.tar')
    with tarfile.open(archive, 'w', format=tarfile.PAX_FORMAT) as stream:
        stream.add(root, arcname=root.name)
    return archive, {'status': manifest['status'], 'native_files': len(manifest['native']),
                     'incomplete': [e['package'] for e in missing]}


# --- macOS --------------------------------------------------------------------------------------

def git_revision(formula, url):
    """The pinned commit of a git-sourced formula: `url "<url>", revision: "<sha>"`."""
    text = formula.read_text(encoding='utf-8')
    match = re.search(r'url\s+"'+re.escape(url)+r'",\s*(?:\w+:\s*"[^"]*",\s*)*revision:\s*"([0-9a-f]{40})"', text)
    return match.group(1) if match else None


def git_archive(url, revision, label, archives):
    """Export exactly the pinned commit; the revision is the integrity reference."""
    with tempfile.TemporaryDirectory(dir=archives) as temporary:
        clone = Path(temporary)/'repository'
        run('git', 'init', '-q', str(clone))
        run('git', '-C', str(clone), 'fetch', '-q', '--depth', '1', url, revision, timeout=900)
        if run('git', '-C', str(clone), 'rev-parse', 'FETCH_HEAD') != revision:
            raise ValueError('Fetched git source does not match the formula revision: '+label)
        archive = archives/(label+'.tar.gz')
        run('git', '-C', str(clone), 'archive', '--format=tar.gz', '--prefix='+label+'/',
            '-o', str(archive), revision)
    if archive.stat().st_size > MAX_DOWNLOAD:
        raise ValueError('Git source exceeds the download limit: '+label)
    return archive


def homebrew_sources(executable, prefix, archives):
    """Download each keg's SBOM source archive; formulae and receipts record patches and options."""
    import native_runtime_macos as runtime
    native = []
    kegs = {}
    for path in runtime.closure(executable, [prefix]):
        if 'Cellar' in path.parts:
            at = path.parts.index('Cellar')
            kegs.setdefault(Path(*path.parts[:at+3]), None)
    for keg in kegs:
        label = keg.parent.name+'-'+keg.name
        source = runtime.keg_source(keg)
        url = source.get('downloadLocation', '')
        expected = next((c['checksumValue'] for c in source.get('checksums', [])
                         if c.get('algorithm') == 'SHA256'), None)
        formula = next(iter(sorted((keg/'.brew').glob('*.rb'))), None)
        entry = {'package': label, 'source_url': url, 'license': source.get('licenseDeclared'),
                 'formula': f'formulae/{label}.rb' if formula else None}
        if formula:
            native.append({'package': label+' formula', 'file': f'formulae/{label}.rb', 'path': formula,
                           'sha256': digest(formula), 'status': 'included'})
            receipt = keg/'INSTALL_RECEIPT.json'
            native.append({'package': label+' receipt', 'file': f'formulae/{label}.INSTALL_RECEIPT.json',
                           'path': receipt, 'sha256': digest(receipt), 'status': 'included'})
        revision = git_revision(formula, url) if formula and url.endswith('.git') else None
        if url.startswith('https://') and revision:
            archive = git_archive(url, revision, label, archives)
            native.append({**entry, 'file': archive.name, 'path': archive, 'sha256': digest(archive),
                           'revision': revision, 'status': 'included'})
            continue
        match = re.search(r'(\.tar\.(?:gz|xz|bz2|zst|lz)|\.tgz|\.zip)$', urllib.parse.urlparse(url).path)
        if not url.startswith('https://') or not expected or match is None:
            native.append({**entry, 'file': None, 'status': 'directions_only',
                           'reason': 'source is not a checksummed HTTPS archive'})
            continue
        archive = download(url, archives/(label+match.group(1)), expected)
        native.append({**entry, 'file': archive.name, 'path': archive, 'sha256': expected, 'status': 'included'})
    return native


def macos(staging, stem, target, commit):
    import bundle_macos
    import bundle_release
    import bundle_slint_macos
    prefix = Path(run('brew', '--prefix'))
    executable, skia_license = build()
    archives = staging/'source-archives'
    archives.mkdir()
    native = homebrew_sources(executable, prefix, archives)
    app = staging/'app'/(NAME+'.app')
    contents = app/'Contents'
    contents.mkdir(parents=True)
    # Kegs without notice files take them from the same verified source archives.
    binary, arch, manifest = bundle_slint_macos.stage_runtime(executable, contents, skia_license,
                                                              [prefix], archives,
                                                              executable_name=executable.name)
    expected = 'arm64' if target == 'macos-arm64' else 'x86_64'
    if arch != expected:
        raise RuntimeError('macOS bundle architecture does not match the download name')
    resources = contents/'Resources'
    icon = bundle_macos.make_icon(ROOT/'assets/icons/pttstella.png', resources, staging)
    version = tomllib.loads((ROOT/'Cargo.toml').read_text(encoding='utf-8'))['package']['version']
    numeric = version.split('-', 1)[0]
    (contents/'Info.plist').write_bytes(plistlib.dumps({
        'CFBundleExecutable': binary.name, 'CFBundleIdentifier': IDENTIFIER,
        'CFBundleName': NAME, 'CFBundleDisplayName': NAME, 'CFBundlePackageType': 'APPL',
        'CFBundleInfoDictionaryVersion': '6.0', 'CFBundleVersion': numeric,
        'CFBundleShortVersionString': numeric, 'CFBundleIconFile': icon,
        'NSHighResolutionCapable': True, 'NSPrincipalClass': 'NSApplication',
        'CFBundleDevelopmentRegion': 'en', 'LSMinimumSystemVersion': '11.0',
        'LSApplicationCategoryType': 'public.app-category.social-networking'}))
    shutil.copy2(ROOT/'docs/macos-release.txt', resources/'README.txt')
    shutil.copy2(ROOT/'LICENSE', resources/GPL_NOTICE)
    run('plutil', '-lint', str(contents/'Info.plist'))
    run('codesign', '--force', '--sign', '-', '--timestamp=none', str(app))
    run('codesign', '--verify', '--deep', '--strict', str(app))
    archive = staging/(stem+'.zip')
    run('ditto', '-c', '-k', '--sequesterRsrc', '--keepParent', str(app), str(archive))
    image = staging/(stem+'.dmg')
    bundle_release.macos_disk_image(app, image)
    sources, source_details = source_set(staging, stem, commit, native, [
        'Native runtime: Homebrew bottles from the closure of libmpv; each keg formula and '
        'INSTALL_RECEIPT records its patches, resources and build options.'])
    return [archive, image, sources], {
        'signing': 'ad_hoc', 'notarized': False, 'runtime': 'macOS 11+ on '+expected,
        'native_runtime': {'files': len(manifest['files'])-1, 'homebrew_prefix': str(prefix),
                           'packages': [p['package'] for p in manifest['packages']]},
        'sources': source_details}


# --- Windows ------------------------------------------------------------------------------------

def windows_mpv(directory):
    """Fetch the pinned official LGPL libmpv; MSVC links its GNU import archive as mpv.lib."""
    directory.mkdir(parents=True, exist_ok=True)
    archive = download(MPV_WINDOWS['archive_url'], directory/'libmpv.zip', MPV_WINDOWS['archive_sha256'])
    with zipfile.ZipFile(archive) as stream:
        for member, name in (('libmpv-2.dll', 'libmpv-2.dll'), ('libmpv.dll.a', 'mpv.lib'),
                             ('LICENSE.LGPL', 'LICENSE.LGPL')):
            (directory/name).write_bytes(stream.read(member))
    return directory


def visual_cpp_runtime():
    """App-local CRT from the runner's Visual Studio redistributable directory."""
    vswhere = Path(os.environ.get('ProgramFiles(x86)', r'C:\Program Files (x86)'))/'Microsoft Visual Studio/Installer/vswhere.exe'
    install = Path(run(str(vswhere), '-latest', '-products', '*', '-property', 'installationPath'))
    folders = sorted((install/'VC/Redist/MSVC').glob('*/x64/Microsoft.VC14*.CRT'))
    if not folders:
        raise RuntimeError('Visual C++ x64 redistributable DLLs were not found')
    return folders[-1]


def windows(staging, stem, version, commit):
    import bundle_slint_windows
    import windows_installer
    mpv = windows_mpv(ROOT/'target/release-mpv')
    env = os.environ.copy()
    env['MPV_LIB_DIR'] = str(mpv)
    executable, skia_license = build(env)
    package = staging/stem
    package.mkdir()
    shutil.copy2(executable, package/'pttstella.exe')
    shutil.copy2(mpv/'libmpv-2.dll', package/'libmpv-2.dll')
    # CRT DLLs import each other (msvcp140 needs vcruntime140_1); bundle the closure.
    crt, crt_folder = {}, None
    for _ in range(8):
        runtime = [package/'libmpv-2.dll', *(package/name for name in crt)]
        _, required = bundle_slint_windows.inspect_payload(package/'pttstella.exe', runtime, main='pttstella.exe')
        missing = sorted(set(required) - set(crt))
        if not missing:
            break
        crt_folder = crt_folder or visual_cpp_runtime()
        for name in missing:
            shutil.copy2(crt_folder/name, package/name)
            crt[name] = digest(package/name)
    else:
        raise RuntimeError('Visual C++ runtime closure did not converge')
    shutil.copy2(ROOT/'assets/icons/pttstella.ico', package/'PTTStella.ico')
    licenses = package/'licenses'
    bundle_common.copy_licenses(licenses, 'x86_64-pc-windows-msvc', feature=FEATURES)
    shutil.copy2(skia_license, licenses/'LICENSE_SKIA')
    shutil.copy2(mpv/'LICENSE.LGPL', licenses/'LICENSE_MPV_LGPL')
    shutil.copy2(ROOT/'LICENSE', licenses/GPL_NOTICE)
    (licenses/'libmpv-provenance.json').write_text(json.dumps(
        {**MPV_WINDOWS, 'files': {'libmpv-2.dll': digest(package/'libmpv-2.dll')}}, indent=2)+'\n')
    if crt:
        (licenses/'vc-crt-provenance.json').write_text(json.dumps(
            {'source': str(crt_folder), 'license': 'Microsoft Visual C++ Redistributable', 'files': crt},
            indent=2)+'\n')
    setup, setup_details = windows_installer.build_slint(package, staging, version)
    portable = staging/(stem+'-portable.zip')
    # crates.io notices carry 1970 timestamps; ZIP clamps them to 1980.
    with zipfile.ZipFile(portable, 'w', zipfile.ZIP_DEFLATED, compresslevel=9,
                         strict_timestamps=False) as stream:
        for path in sorted(package.rglob('*')):
            if path.is_file():
                stream.write(path, (stem+'/'+path.relative_to(package).as_posix()))
    source = download(MPV_WINDOWS['source_url'], staging/'mpv-0.42.0.tar.gz', MPV_WINDOWS['source_sha256'])
    signature = download(MPV_WINDOWS['source_signature_url'], staging/'mpv-0.42.0.tar.gz.asc', limit=64*1024)
    native = [{'package': 'mpv-0.42.0', 'file': source.name, 'path': source, 'sha256': digest(source),
               'source_url': MPV_WINDOWS['source_url'], 'status': 'included'},
              {'package': 'mpv-0.42.0 signature', 'file': signature.name, 'path': signature,
               'sha256': digest(signature), 'status': 'included'},
              {'package': 'libmpv-2.dll static dependencies', 'file': None, 'status': 'directions_only',
               'source_url': 'https://github.com/mpv-player/mpv/releases/tag/v0.42.0',
               'reason': 'The official LGPL build links FFmpeg and other libraries statically; '
                         'their exact versions come from the mpv v0.42.0 build CI.'}]
    sources, source_details = source_set(staging, stem, commit, native, [
        'libmpv-2.dll is the unmodified official mpv v0.42.0 LGPL x86_64 build.'])
    return [setup, portable, sources], {
        'authenticode_signed': False, 'runtime': 'Windows 10/11 x64; no WebView2',
        'installer': setup_details, 'libmpv': MPV_WINDOWS['archive_sha256'],
        'app_local_crt': sorted(crt), 'sources': source_details}


# --- Linux --------------------------------------------------------------------------------------

def linux(staging, stem, version, commit):
    import bundle_release
    import bundle_slint_linux
    executable, skia_license = build()
    files, details = bundle_release.linux_package(
        staging, stem, version, executable, program='pttstella', display_name=NAME,
        desktop_id=IDENTIFIER, feature=FEATURES,
        resources={'licenses/LICENSE_SKIA': skia_license, 'licenses/'+GPL_NOTICE: ROOT/'LICENSE'},
        forbidden_libraries=('webkit', 'javascriptcore', 'webview', 'libwry'),
        extra_depends=bundle_slint_linux.WINIT_RUNTIME)
    if 'libmpv' not in details['depends']:
        raise RuntimeError('Linux package must depend on the system libmpv')
    details['late_loaded_dependencies'] = list(bundle_slint_linux.WINIT_RUNTIME)
    sources, source_details = source_set(staging, stem, commit, [], [
        'libmpv and its dependencies are Ubuntu system packages (libmpv2) and are not redistributed.'])
    return [*files, sources], {**details, 'sources': source_details}


if __name__ == '__main__':
    import argparse
    import sys
    parser = argparse.ArgumentParser(description='Prepare the pinned Windows libmpv for building and tests')
    parser.add_argument('command', choices=['windows-mpv'])
    parser.add_argument('directory', type=Path)
    args = parser.parse_args()
    try:
        print(windows_mpv(args.directory.resolve()))
    except (OSError, ValueError, KeyError, zipfile.BadZipFile) as error:
        print(str(error), file=sys.stderr)
        raise SystemExit(1)

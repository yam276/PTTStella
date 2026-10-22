#!/usr/bin/env python3
"""Create native Release downloads, checksums and a source manifest. Never uploads."""
import argparse
import hashlib
import json
from pathlib import Path
import platform
import shutil
import struct
import subprocess
import sys
import tarfile
import tempfile
import tomllib

import bundle_common
import bundle_macos
import bundle_windows
from bundle_common import publish_files

ROOT = Path(__file__).resolve().parent.parent
TARGETS = {
    'windows-x64': 'x86_64-pc-windows-msvc',
    'macos-arm64': 'aarch64-apple-darwin',
    'macos-x64': 'x86_64-apple-darwin',
    'linux-x64': 'x86_64-unknown-linux-gnu',
}


def run(*args, cwd=ROOT):
    return subprocess.run(args, cwd=cwd, check=True, capture_output=True,
                          text=True, encoding='utf-8', timeout=120).stdout.strip()


def digest(path):
    with path.open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def native_platform():
    system, machine = sys.platform, platform.machine().lower()
    if system == 'win32' and machine in ('amd64', 'x86_64'):
        return 'windows-x64'
    if system == 'darwin' and machine in ('arm64', 'aarch64', 'x86_64'):
        return 'macos-arm64' if machine != 'x86_64' else 'macos-x64'
    if system == 'linux' and machine == 'x86_64':
        return 'linux-x64'
    raise RuntimeError(f'Unsupported native Release platform: {system}/{machine}')


def inspect_elf(executable):
    with executable.open('rb') as stream:
        header = stream.read(64)
    if (len(header) != 64 or header[:6] != b'\x7fELF\x02\x01'
            or struct.unpack_from('<H', header, 18)[0] != 62):
        raise ValueError('Expected a little-endian x86-64 ELF executable')
    libraries = run('ldd', str(executable))
    if 'not found' in libraries:
        raise RuntimeError('Linux package has unresolved shared libraries: '+libraries)
    for line in libraries.splitlines():
        if '=>' in line:
            path = line.split('=>', 1)[1].strip().split(' ', 1)[0]
            if not path.startswith(('/lib/', '/lib64/', '/usr/lib/')):
                raise RuntimeError('Linux package uses a non-system library: '+path)
    return libraries


def linux_package(staging, stem, version):
    executable = bundle_common.build()
    libraries = inspect_elf(executable)
    package = staging/stem
    package.mkdir()
    shutil.copy2(executable, package/'pttstella')
    (package/'pttstella').chmod(0o755)
    bundle_common.copy_licenses(package/'licenses', TARGETS['linux-x64'])
    shutil.copy2(ROOT/'docs/linux-package.txt', package/'README.txt')
    # Calculate dependency versions from the actual ELF and installed dpkg data.
    # A missing symbol/library mapping fails instead of guessing a dependency.
    debian = staging/'debian'
    debian.mkdir()
    control = ('Source: pttstella\nSection: net\nPriority: optional\n'
               'Maintainer: yam276 <59956724+yam276@users.noreply.github.com>\n\n'
               'Package: pttstella\nArchitecture: amd64\nDescription: PTT desktop client\n')
    (debian/'control').write_text(control, encoding='utf-8')
    dependencies = run('dpkg-shlibdeps', '-O', '-e'+str(executable), cwd=staging)
    fields = [line.removeprefix('shlibs:Depends=') for line in dependencies.splitlines()
              if line.startswith('shlibs:Depends=')]
    if len(fields) != 1 or not fields[0]:
        raise RuntimeError('dpkg-shlibdeps did not report runtime dependencies')
    depends = fields[0]+', ca-certificates, xdg-desktop-portal, xdg-desktop-portal-gtk'
    debroot = staging/'debroot'
    binary = debroot/'usr/bin/pttstella'
    binary.parent.mkdir(parents=True)
    shutil.copy2(package/'pttstella', binary)
    doc = debroot/'usr/share/doc/pttstella'
    doc.parent.mkdir(parents=True)
    shutil.copytree(package/'licenses', doc/'licenses')
    shutil.copy2(package/'README.txt', doc/'README.txt')
    desktop = debroot/'usr/share/applications/io.github.yam276.pttstella.desktop'
    desktop.parent.mkdir(parents=True)
    desktop.write_text('[Desktop Entry]\nType=Application\nName=PTTStella\n'
                       'Comment=Terminal-style BBS client\nExec=pttstella\n'
                       'Icon=pttstella\nTerminal=false\nCategories=Network;\n', encoding='utf-8')
    icon = debroot/'usr/share/icons/hicolor/1024x1024/apps/pttstella.png'
    icon.parent.mkdir(parents=True)
    shutil.copy2(ROOT/'assets/icons/pttstella.png', icon)
    metadata = debroot/'DEBIAN'
    metadata.mkdir()
    deb_version = version.replace('-', '~', 1)
    size = sum(p.stat().st_size for p in debroot.rglob('*') if p.is_file())//1024+1
    (metadata/'control').write_text(
        f'Package: pttstella\nVersion: {deb_version}\nArchitecture: amd64\n'
        'Maintainer: yam276 <59956724+yam276@users.noreply.github.com>\n'
        f'Depends: {depends}\nInstalled-Size: {size}\nSection: net\nPriority: optional\n'
        'Description: Terminal-style BBS desktop client\n', encoding='utf-8')
    deb = staging/(stem+'.deb')
    run('dpkg-deb', '--build', '--root-owner-group', str(debroot), str(deb))
    if run('dpkg-deb', '--field', str(deb), 'Architecture') != 'amd64':
        raise RuntimeError('Debian archive architecture check failed')
    archive = staging/(stem+'.tar.gz')
    with tarfile.open(archive, 'w:gz', compresslevel=9) as stream:
        stream.add(package, arcname=stem)
    return [archive, deb], {'runtime': 'Ubuntu 24.04 x64 or compatible; see package dependencies',
                            'depends': depends, 'libraries': libraries, 'signed': False}


def macos_package(staging, stem, target):
    result = bundle_macos.bundle(staging/'app')
    expected = 'arm64' if target == 'macos-arm64' else 'x86_64'
    if result['architecture'] != expected:
        raise RuntimeError('macOS bundle architecture does not match the download name')
    app = Path(result['app'])
    bundle_common.copy_licenses(app/'Contents/Resources/licenses', TARGETS[target])
    shutil.copy2(ROOT/'docs/macos-release.txt', app/'Contents/Resources/README.txt')
    # Resources are part of the signature, so sign after adding license inventory.
    run('codesign', '--force', '--sign', '-', '--timestamp=none', str(app))
    run('codesign', '--verify', '--deep', '--strict', str(app))
    archive = staging/(stem+'.zip')
    run('ditto', '-c', '-k', '--sequesterRsrc', '--keepParent', str(app), str(archive))
    return [archive], {'signing': 'ad_hoc', 'notarized': False,
                        'runtime': 'macOS on '+expected}


def bundle(output):
    target = native_platform()
    version = tomllib.loads((ROOT/'Cargo.toml').read_text(encoding='utf-8'))['package']['version']
    # Release policy also checks ordering; local packaging accepts development versions.
    from release import parse_version
    parse_version(version)
    stem = f'PTTStella-{version}-{target}'
    output = output.resolve()
    output.mkdir(parents=True, exist_ok=True)
    if any(any(output.glob(stem+suffix)) for suffix in ('.*', '-setup.*', '-portable.*')):
        raise FileExistsError('Existing Release files will not be overwritten; choose another --out-dir')
    revision = run('git', 'rev-parse', 'HEAD')
    dirty = bool(run('git', 'status', '--porcelain', '--untracked-files=normal'))
    with tempfile.TemporaryDirectory(prefix='.release-', dir=output) as temporary:
        staging = Path(temporary)
        if target == 'windows-x64':
            result = bundle_windows.bundle(staging/'windows', with_installer=True, with_portable=True)
            # ZIP remains a local verification artifact; users choose Setup or
            # the standalone App EXE, both from the same static-CRT build.
            assets = []
            for source in (Path(result['installer']), Path(result['portable'])):
                archive = staging/source.name
                shutil.copy2(source, archive)
                assets.append(archive)
            details = {'authenticode_signed': False,
                       'runtime': 'Microsoft Edge WebView2 Evergreen Runtime',
                       'installer': result['installer_details'],
                       'portable': result['portable_details']}
        elif target.startswith('macos-'):
            assets, details = macos_package(staging, stem, target)
        else:
            assets, details = linux_package(staging, stem, version)
        manifest = {'version': version, 'platform': target, 'target': TARGETS[target],
                    'profile': 'release', 'source_commit': revision, 'source_dirty': dirty,
                    'gui_acceptance': 'not_run', **details,
                    'files': {p.name: digest(p) for p in assets}}
        report = staging/(stem+'.json')
        report.write_text(json.dumps(manifest, indent=2)+'\n', encoding='utf-8')
        files = assets+[report]
        for path in files.copy():
            checksum = staging/(path.name+'.sha256')
            checksum.write_text(digest(path)+'  '+path.name+'\n', encoding='ascii')
            files.append(checksum)
        delivered = publish_files(files, output)
    return {'status': 'bundled', 'platform': target, 'files': [str(p) for p in delivered],
            'source_commit': revision, 'source_dirty': dirty}


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--out-dir', type=Path, default=ROOT/'dist/release')
    args = parser.parse_args()
    try:
        print(json.dumps(bundle(args.out_dir)))
    except (OSError, ValueError, KeyError, RuntimeError, subprocess.SubprocessError) as error:
        print(str(error), file=sys.stderr)
        raise SystemExit(1)

#!/usr/bin/env python3
"""Build a native, ad-hoc-signed macOS app. Does not launch or publish it."""

import argparse
import json
import os
from pathlib import Path
import plistlib
import re
import shutil
import subprocess
import sys
import tempfile
import tomllib

ROOT = Path(__file__).resolve().parent.parent


def run(*command):
    return subprocess.run(command, cwd=ROOT, check=True, capture_output=True,
                          text=True, timeout=60).stdout


def build():
    executable = None
    command = ['cargo', 'build', '--locked', '--release', '--features', 'desktop',
               '--bin', 'pttstella', '--message-format=json-render-diagnostics']
    # Cargo reports the actual executable path, including a guard's managed target.
    # Keep stderr attached so long builds show progress and compiler diagnostics.
    with subprocess.Popen(command, cwd=ROOT, stdout=subprocess.PIPE, text=True) as child:
        try:
            for line in child.stdout:
                message = json.loads(line)
                if (message.get('reason') == 'compiler-artifact'
                        and message.get('target', {}).get('name') == 'pttstella'
                        and 'bin' in message.get('target', {}).get('kind', [])
                        and message.get('executable')):
                    executable = Path(message['executable'])
            if child.wait() != 0:
                raise RuntimeError('Cargo build failed; no app was produced')
        except BaseException:
            if child.poll() is None:
                child.terminate()
            raise
    if executable is None or not executable.is_file():
        raise RuntimeError('Cargo did not report the pttstella executable')
    return executable


def check_dependencies(executable):
    # This package embeds JS/CSS/fonts and has no binary sidecars. Fail
    # rather than ship an app that depends on a developer's Homebrew/Rust paths.
    libraries = [line.strip().split(' (compatibility version', 1)[0]
                 for line in run('otool', '-L', str(executable)).splitlines()[1:]]
    external = [library for library in libraries
                if not library.startswith(('/System/Library/', '/usr/lib/'))]
    if external:
        raise RuntimeError('Unbundled non-system libraries: ' + ', '.join(external))


def bundle(output):
    if sys.platform != 'darwin':
        raise RuntimeError('Build this app on macOS with Xcode Command Line Tools installed')
    for tool in ('cargo', 'xcrun', 'otool', 'codesign', 'plutil', 'lipo'):
        if shutil.which(tool) is None:
            raise RuntimeError(f'Missing required tool: {tool}')
    run('xcrun', '--find', 'clang')
    config = tomllib.loads((ROOT / 'Dioxus.toml').read_text(encoding='utf-8'))
    package = tomllib.loads((ROOT / 'Cargo.toml').read_text(encoding='utf-8'))['package']
    name = config['application']['name']
    if not re.fullmatch(r'[A-Za-z0-9_-]+', name):
        raise RuntimeError('App name must be a simple file name')
    destination = output / f'{name}.app'
    # Never delete or replace a retained app. A new output directory can be used
    # to keep two builds side by side, including when a previous build is open.
    if os.path.lexists(destination):
        raise RuntimeError(f'Output already exists: {destination}; choose another --out-dir')
    executable = build()
    check_dependencies(executable)
    version = package['version'].split('-', 1)[0].split('+', 1)[0]
    if not re.fullmatch(r'\d+\.\d+\.\d+', version):
        raise RuntimeError('Cargo version must have three numeric components')
    info = {
        'CFBundleDevelopmentRegion': 'en',
        'CFBundleDisplayName': name,
        'CFBundleExecutable': package['name'],
        'CFBundleIdentifier': config['bundle']['identifier'],
        'CFBundleInfoDictionaryVersion': '6.0',
        'CFBundleName': name,
        'CFBundlePackageType': 'APPL',
        'CFBundleShortVersionString': version,
        'CFBundleVersion': version,
        'NSHighResolutionCapable': True,
        'NSPrincipalClass': 'NSApplication',
        'LSApplicationCategoryType': 'public.app-category.social-networking',
    }
    output.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix='.pttstella-bundle-', dir=output) as staging:
        app = Path(staging) / destination.name
        binary = app / 'Contents' / 'MacOS' / package['name']
        binary.parent.mkdir(parents=True)
        resources = app / 'Contents' / 'Resources'
        resources.mkdir()
        notices = resources / 'FontLicenses'
        notices.mkdir()
        for name in ('NOTICE.txt', 'Cascadia-OFL.txt', 'NotoSansCJK-OFL.txt', 'sources.json'):
            shutil.copy2(ROOT / 'assets' / 'fonts' / name, notices / name)
        shutil.copy2(executable, binary)
        binary.chmod(0o755)
        plist = app / 'Contents' / 'Info.plist'
        plist.write_bytes(plistlib.dumps(info))
        run('plutil', '-lint', str(plist))
        run('codesign', '--force', '--sign', '-', '--timestamp=none', str(app))
        run('codesign', '--verify', '--deep', '--strict', str(app))
        architecture = run('lipo', '-archs', str(binary)).strip()
        # The finished app leaves the managed Cargo target before the guard exits.
        app.rename(destination)
    return {'status': 'bundled', 'app': str(destination), 'version': package['version'],
            'architecture': architecture, 'signing': 'ad_hoc',
            'notarized': False, 'gui_acceptance': 'not_run'}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--out-dir', type=Path, default=ROOT / 'dist' / 'macos',
                        help='Output directory; an existing PTTStella.app is preserved')
    args = parser.parse_args()
    try:
        report = bundle(args.out_dir.expanduser().resolve())
    except (OSError, ValueError, KeyError, RuntimeError, subprocess.SubprocessError) as error:
        print(f'macOS bundle failed: {error}', file=sys.stderr)
        return 1
    print(json.dumps(report))
    return 0


if __name__ == '__main__':
    raise SystemExit(main())

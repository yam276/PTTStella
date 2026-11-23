"""Resolve both Slint feature configurations on the four candidate targets.

This checks selected Cargo normal/build dependencies. It does not compile,
inspect native imports, launch an app, or establish runtime WebView absence.
No compilation profile or executable payload is produced. Cargo can update its
small target metadata files while resolving the trees.
"""
import argparse
import hashlib
import json
from pathlib import Path
import re
import subprocess

ROOT = Path(__file__).resolve().parent.parent
TARGETS = ('x86_64-pc-windows-msvc', 'aarch64-apple-darwin',
           'x86_64-apple-darwin', 'x86_64-unknown-linux-gnu')
FEATURES = ('slint-research', 'native-player-probe')
FORBIDDEN_PREFIXES = ('dioxus', 'wry', 'tao', 'webview2', 'webkit2gtk',
                      'javascriptcore-rs', 'gtk-webkit')
REQUIRED = {'pttstella', 'slint', 'i-slint-backend-winit', 'i-slint-renderer-skia'}
PACKAGE = re.compile(r'^([A-Za-z0-9_-]+) v([0-9][^\s]*)(?: .*)?$')


def inspect_tree(output, feature):
    if feature not in FEATURES:
        raise ValueError('Unknown candidate feature')
    packages = {}
    for line in output.splitlines():
        match = PACKAGE.fullmatch(line.strip())
        if not match:
            raise ValueError('Unexpected Cargo tree output: ' + line[:200])
        packages.setdefault(match[1], set()).add(match[2])
    required = REQUIRED | ({'libmpv2-sys'} if feature == 'native-player-probe' else set())
    missing = sorted(required - packages.keys())
    forbidden = sorted(name for name in packages if name.startswith(FORBIDDEN_PREFIXES))
    if missing or forbidden:
        raise ValueError(f'Invalid Slint dependencies: missing={missing}, forbidden={forbidden}')
    return {name: sorted(versions) for name, versions in sorted(packages.items())}


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def verify(output):
    output.mkdir(parents=True, exist_ok=False)
    sources = {name: digest(ROOT / name) for name in ('Cargo.toml', 'Cargo.lock')}
    report = {'status': 'running', 'scope': __doc__.strip(), 'cases': {},
              'source_ref': subprocess.check_output(['git', 'rev-parse', 'HEAD'],
                                                     cwd=ROOT, text=True).strip(),
              'manifest_hashes': sources}

    def record():
        (output / 'report.json').write_text(json.dumps(report, indent=2) + '\n', encoding='utf-8')

    try:
        for feature in FEATURES:
            for target in TARGETS:
                key = feature + '-' + target
                command = ['cargo', 'tree', '--locked', '--no-default-features',
                           '--features', feature, '--target', target, '--edges', 'normal,build',
                           '--prefix', 'none', '--format', '{p}', '--color', 'never']
                process = subprocess.run(command, cwd=ROOT, capture_output=True,
                                         text=True, timeout=120)
                (output / (key + '.tree.txt')).write_text(process.stdout, encoding='utf-8')
                (output / (key + '.stderr.txt')).write_text(process.stderr, encoding='utf-8')
                case = {'status': 'failed', 'command': command, 'exit': process.returncode}
                report['cases'][key] = case
                if process.returncode:
                    raise RuntimeError(f'{key}: Cargo tree failed with exit {process.returncode}')
                packages = inspect_tree(process.stdout, feature)
                case.update(status='passed', packages=packages,
                            unique_packages=len(packages),
                            tree_sha256=hashlib.sha256(process.stdout.encode()).hexdigest())
                record()
        if len(report['cases']) != len(TARGETS) * len(FEATURES):
            raise RuntimeError('Missing required dependency matrix entries')
        if sources != {name: digest(ROOT / name) for name in sources}:
            raise RuntimeError('Cargo manifest or lock changed during the audit')
        report['status'] = 'passed'
    except BaseException as error:
        report.update(status='failed', error=str(error))
        raise
    finally:
        record()
    return report


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=Path, required=True,
                        help='New directory for the eight trees and report; never replaces prior output')
    args = parser.parse_args()
    report = verify(args.output.expanduser().resolve())
    print(json.dumps({'status': report['status'], 'cases': len(report['cases']),
                      'output': str(args.output)}))


if __name__ == '__main__':
    main()

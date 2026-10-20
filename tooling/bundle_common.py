"""Shared Cargo build and license inventory for native desktop packages."""
import json
import os
from pathlib import Path
import shutil
import subprocess

ROOT = Path(__file__).resolve().parent.parent


def publish_files(paths, output):
    """Publish complete files without replacement; roll back only this attempt."""
    created = []
    try:
        for path in paths:
            destination = output/path.name
            os.link(path, destination)
            created.append(destination)
    except BaseException:
        for path in created:
            path.unlink()
        raise
    return created


def build(target=None):
    command = ['cargo', 'build', '--locked', '--release', '--features', 'desktop',
               '--bin', 'pttstella', '--message-format=json-render-diagnostics']
    if target:
        command += ['--target', target]
    executable = None
    with subprocess.Popen(command, cwd=ROOT, stdout=subprocess.PIPE,
                          text=True, encoding='utf-8') as child:
        try:
            for line in child.stdout:
                message = json.loads(line)
                if (message.get('reason') == 'compiler-artifact'
                        and message.get('target', {}).get('name') == 'pttstella'
                        and 'bin' in message.get('target', {}).get('kind', [])
                        and message.get('executable')):
                    executable = Path(message['executable'])
            if child.wait() != 0:
                raise RuntimeError('Cargo release build failed; no package was produced')
        except BaseException:
            if child.poll() is None:
                child.terminate()
                child.wait()
            raise
    if executable is None or not executable.is_file():
        raise RuntimeError('Cargo did not report the pttstella executable')
    return executable


def copy_licenses(destination, target):
    destination.mkdir()
    for name in ('NOTICE.txt', 'Cascadia-OFL.txt', 'NotoSansCJK-OFL.txt'):
        shutil.copy2(ROOT/'assets/fonts'/name, destination/name)
    shutil.copy2(ROOT/'assets/vendor/fluent/LICENSE', destination/'fluent-Apache-2.0.txt')
    result = subprocess.run(['cargo', 'metadata', '--locked', '--format-version=1',
                             '--filter-platform', target, '--features', 'desktop'],
                            cwd=ROOT, capture_output=True, check=True, encoding='utf-8')
    inventory = []
    for package in json.loads(result.stdout)['packages']:
        if not package.get('source'):
            continue
        source = Path(package['manifest_path']).parent
        label = package['name']+'-'+package['version']
        files = {p for pattern in ('LICENSE*', 'LICENCE*', 'COPYING*', 'NOTICE*')
                 for p in source.glob(pattern) if p.is_file()}
        if package.get('license_file'):
            files.add(source/package['license_file'])
        for file in sorted(files):
            (destination/label).mkdir(exist_ok=True)
            shutil.copy2(file, destination/label/file.name)
        inventory.append({key: package.get(key) for key in
                          ('name', 'version', 'license', 'repository', 'source')})
    (destination/'rust-dependencies.json').write_text(
        json.dumps(inventory, indent=2)+'\n', encoding='utf-8')

"""Build a standalone App EXE; fail if any non-system DLL remains required."""
import os
from pathlib import Path
import shutil
import subprocess

import bundle_common

TARGET = 'x86_64-pc-windows-msvc'


def build(notices):
    env = os.environ.copy()
    # Cargo's encoded form takes precedence; retain existing flags and append the
    # target's CRT choice. An explicit --target keeps these off host proc macros.
    flags = (env['CARGO_ENCODED_RUSTFLAGS'].split('\x1f')
             if env.get('CARGO_ENCODED_RUSTFLAGS') else env.get('RUSTFLAGS', '').split())
    env['CARGO_ENCODED_RUSTFLAGS'] = '\x1f'.join(flags+['-C', 'target-feature=+crt-static'])
    env['PTTSTELLA_NOTICES_FILE'] = str(notices.resolve())
    return bundle_common.build(TARGET, env=env)


def write_notices(licenses, destination):
    parts = ['PTTStella — third-party notices\n'
             'Exported by --export-licenses OUTPUT_FILE; no installation or connection is started.\n']
    for path in sorted(licenses.rglob('*')):
        if path.is_file():
            parts += ['\n===== '+path.relative_to(licenses).as_posix()+' =====\n',
                      path.read_text(encoding='utf-8')+'\n']
    text = ''.join(parts)
    if not (licenses/'rust-dependencies.json').is_file() or len(text.encode('utf-8')) > 32*1024*1024:
        raise RuntimeError('Missing or oversized portable license inventory')
    destination.write_text(text, encoding='utf-8')


def verify_dependencies(pe):
    from bundle_windows import SYSTEM_DLLS
    for dll in pe['imports']:
        if (not dll.startswith(('api-ms-win-', 'ext-ms-win-'))
                and Path(dll).stem not in SYSTEM_DLLS):
            raise RuntimeError('Portable EXE requires an external DLL: '+dll)


def verify_export(executable, notices, staging):
    """Exercise the shipped binary from an empty folder, without UI or network."""
    directory = staging/'portable-check'
    directory.mkdir()
    isolated = directory/'pttstella.exe'
    shutil.copy2(executable, isolated)
    exported = directory/'licenses.txt'
    subprocess.run([str(isolated), '--export-licenses', str(exported)], cwd=directory,
                   check=True, capture_output=True, timeout=60)
    if exported.read_bytes() != notices.read_bytes():
        raise RuntimeError('Portable EXE did not export the compiled license inventory')
    if {p.name for p in directory.iterdir()} != {'pttstella.exe', 'licenses.txt'}:
        raise RuntimeError('Portable license export created unexpected files')

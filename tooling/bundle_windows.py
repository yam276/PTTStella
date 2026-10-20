#!/usr/bin/env python3
"""Build Windows packages and standalone EXE; never opens App UI or connects to BBS."""
import argparse
import ctypes
import hashlib
import json
import os
from pathlib import Path
import shutil
import struct
import subprocess
import sys
import tempfile
import tomllib
import zipfile

import bundle_common

ROOT = Path(__file__).resolve().parent.parent
TARGET = 'x86_64-pc-windows-msvc'
SYSTEM_DLLS = frozenset('advapi32 bcrypt bcryptprimitives combase comctl32 comdlg32 crypt32 '
                       'dwmapi gdi32 imm32 kernel32 ntdll ole32 oleaut32 secur32 shell32 '
                       'shlwapi user32 userenv uxtheme version winmm winspool ws2_32 ucrtbase'.split())


def digest(path):
    with path.open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def inspect_pe(path):
    """Check architecture/subsystem and read actual imported DLL names (PE32+)."""
    data = path.read_bytes()
    pe = struct.unpack_from('<I', data, 0x3c)[0]
    if data[:2] != b'MZ' or data[pe:pe+4] != b'PE\0\0':
        raise ValueError('Not a PE executable')
    machine, count = struct.unpack_from('<HH', data, pe+4)
    optional_size = struct.unpack_from('<H', data, pe+20)[0]
    opt = pe+24
    if machine != 0x8664 or struct.unpack_from('<H', data, opt)[0] != 0x20b:
        raise ValueError('Expected an x64 PE32+ executable')
    if struct.unpack_from('<H', data, opt+68)[0] != 2:
        raise ValueError('Expected Windows GUI subsystem (release build)')
    sections = [struct.unpack_from('<IIII', data, opt+optional_size+i*40+8) for i in range(count)]

    def offset(rva):
        for virtual_size, address, raw_size, raw in sections:
            if address <= rva < address+max(virtual_size, raw_size):
                return raw+rva-address
        raise ValueError('PE RVA has no section')

    rva, size = struct.unpack_from('<II', data, opt+120)
    imports = []
    if rva:
        start = offset(rva)
        for i in range(min(size//20, 4096)):
            entry = struct.unpack_from('<IIIII', data, start+i*20)
            if not any(entry):
                break
            name = offset(entry[3])
            imports.append(data[name:data.index(b'\0', name, name+256)].decode('ascii').lower())
        else:
            raise ValueError('Unterminated import directory')
    # Never silently omit a delayed sidecar dependency.
    if any(struct.unpack_from('<II', data, opt+112+13*8)):
        raise ValueError('Delay imports require explicit dependency handling')
    return {'machine': 'AMD64', 'subsystem': 'Windows GUI', 'imports': sorted(set(imports))}


def verify_icon(executable):
    kernel = ctypes.WinDLL('kernel32', use_last_error=True)
    kernel.LoadLibraryExW.argtypes = [ctypes.c_wchar_p, ctypes.c_void_p, ctypes.c_uint]
    kernel.LoadLibraryExW.restype = ctypes.c_void_p
    kernel.FindResourceW.argtypes = [ctypes.c_void_p, ctypes.c_void_p, ctypes.c_void_p]
    kernel.FindResourceW.restype = ctypes.c_void_p
    kernel.SizeofResource.argtypes = [ctypes.c_void_p, ctypes.c_void_p]
    kernel.SizeofResource.restype = ctypes.c_uint
    kernel.LoadResource.argtypes = [ctypes.c_void_p, ctypes.c_void_p]
    kernel.LoadResource.restype = ctypes.c_void_p
    kernel.LockResource.argtypes = [ctypes.c_void_p]
    kernel.LockResource.restype = ctypes.c_void_p
    kernel.FreeLibrary.argtypes = [ctypes.c_void_p]
    handle = kernel.LoadLibraryExW(str(executable), None, 0x60)  # resources only; no code execution
    if not handle:
        raise OSError(ctypes.get_last_error(), 'Cannot inspect executable resources')
    try:
        def resource(ordinal, kind):
            found = kernel.FindResourceW(handle, ordinal, kind)
            if not found:
                raise ValueError('Executable is missing its application icon')
            pointer = kernel.LockResource(kernel.LoadResource(handle, found))
            if not pointer:
                raise ValueError('Cannot read embedded icon')
            return ctypes.string_at(pointer, kernel.SizeofResource(handle, found))

        group = resource(1, 14)  # group icon 1, also used by Tao
        original = (ROOT/'assets/icons/pttstella.ico').read_bytes()
        count = struct.unpack_from('<H', original, 4)[0]
        if group[:6] != original[:6] or count != 7:
            raise ValueError('Embedded icon group does not match the seven-size ICO')
        for index in range(count):
            source = original[6+index*16:6+(index+1)*16]
            entry = group[6+index*14:6+(index+1)*14]
            size, offset = struct.unpack_from('<II', source, 8)
            ordinal = struct.unpack_from('<H', entry, 12)[0]
            # rc.exe normalizes PNG ICO planes from 0 to 1 in the group header.
            if (entry[:4] != source[:4] or entry[6:12] != source[6:12]
                    or struct.unpack_from('<H', entry, 4)[0] != 1
                    or resource(ordinal, 3) != original[offset:offset+size]):
                raise ValueError('Embedded icon pixels differ from the source ICO')
    finally:
        kernel.FreeLibrary(handle)


def build():
    return bundle_common.build(TARGET)


def find_crt(names, explicit=None):
    candidates = [Path(explicit)] if explicit else [
        p for key in ('ProgramFiles', 'ProgramFiles(x86)')
        for p in Path(os.environ.get(key, 'C:/Program Files')).glob(
            'Microsoft Visual Studio/*/*/VC/Redist/MSVC/*/x64/Microsoft.VC*.CRT')]
    candidates.sort(key=lambda p: tuple(int(v) for v in p.parents[1].name.split('.')), reverse=True)
    for directory in candidates:
        if all((directory/name).is_file() for name in names):
            return directory
    raise RuntimeError('Matching Microsoft redistributable CRT not found; use --crt-dir')


def copy_licenses(destination):
    bundle_common.copy_licenses(destination, TARGET)


def make_zip(staging, name):
    archive = staging/'package.zip'
    with zipfile.ZipFile(archive, 'w', compression=zipfile.ZIP_DEFLATED, compresslevel=9,
                         strict_timestamps=False) as zip_out:
        for path in sorted((staging/name).rglob('*')):
            if path.is_file():
                zip_out.write(path, path.relative_to(staging))
    with zipfile.ZipFile(archive) as zip_in:
        if zip_in.testzip() is not None:
            raise RuntimeError('ZIP integrity check failed')
    return archive


def inspect_application(executable, portable=False):
    pe = inspect_pe(executable)
    verify_icon(executable)
    if portable:
        from windows_portable import verify_dependencies
        verify_dependencies(pe)
    crt = []
    for dll in pe['imports']:
        if dll.startswith(('vcruntime140', 'msvcp140')):
            crt.append(dll)
        elif not dll.startswith(('api-ms-win-', 'ext-ms-win-')) and Path(dll).stem not in SYSTEM_DLLS:
            raise RuntimeError('Unbundled non-system import: '+dll)
    return pe, crt


def bundle(output, crt_dir=None, with_installer=False, with_portable=False):
    if sys.platform != 'win32':
        raise RuntimeError('Run this bundler on Windows with the MSVC toolchain')
    version = tomllib.loads((ROOT/'Cargo.toml').read_text(encoding='utf-8'))['package']['version']
    name = f'PTTStella-{version}-windows-x64'
    output = Path(output).resolve()
    output.mkdir(parents=True, exist_ok=True)
    names = [name+'.zip', name+'.zip.sha256']
    if with_installer:
        names += [name+'-setup.exe', name+'-setup.exe.sha256']
    if with_portable:
        names += [name+'-portable.exe', name+'-portable.exe.sha256']
    if any(os.path.lexists(output/filename) for filename in names):
        raise FileExistsError('Existing downloads will not be overwritten; choose another --out-dir')
    if not with_portable:
        executable = build()
        pe, crt = inspect_application(executable)
    with tempfile.TemporaryDirectory(prefix='.pttstella-', dir=output) as temp:
        staging = Path(temp)
        package = staging/name
        package.mkdir()
        copy_licenses(package/'licenses')
        portable_report = None
        if with_portable:
            import windows_portable
            notices = staging/'third-party-notices.txt'
            windows_portable.write_notices(package/'licenses', notices)
            executable = windows_portable.build(notices)
            pe, crt = inspect_application(executable, portable=True)
        crt_source = find_crt(crt, crt_dir) if crt else None
        shutil.copy2(executable, package/'pttstella.exe')
        shutil.copy2(ROOT/'assets/icons/pttstella.ico', package/'PTTStella.ico')
        for dll in crt:
            shutil.copy2(crt_source/dll, package/dll)
        shutil.copy2(ROOT/'docs/windows-test-package.txt', package/'README.txt')
        revision = subprocess.run(['git', 'rev-parse', 'HEAD'], cwd=ROOT, capture_output=True, text=True, check=True).stdout.strip()
        dirty = bool(subprocess.run(['git', 'status', '--porcelain'], cwd=ROOT, capture_output=True, text=True, check=True).stdout.strip())
        manifest = {'version': version, 'target': TARGET, 'profile': 'release',
                    'source_commit': revision, 'source_dirty': dirty, 'pe': pe,
                    'embedded_icon': True, 'crt_source_version': crt_source.parents[1].name if crt_source else None,
                    'prerequisite': 'Microsoft Edge WebView2 Evergreen Runtime',
                    'authenticode_signed': False,
                    'files': {str(p.relative_to(package)).replace('\\', '/'): digest(p)
                              for p in sorted(package.rglob('*')) if p.is_file()}}
        if with_portable:
            manifest.update(crt_linkage='static', embedded_notices_sha256=digest(notices))
        (package/'manifest.json').write_text(json.dumps(manifest, indent=2)+'\n', encoding='utf-8')
        setup_report = None
        downloads = []
        if with_portable:
            portable = staging/(name+'-portable.exe')
            shutil.copy2(executable, portable)
            windows_portable.verify_export(portable, notices, staging)
            downloads.append(portable)
            portable_report = {'crt_linkage': 'static', 'pe': pe,
                               'embedded_notices_sha256': digest(notices),
                               'license_export': 'passed',
                               'prerequisite': 'Microsoft Edge WebView2 Evergreen Runtime',
                               'runtime_installation': 'none', 'gui_acceptance': 'not_run'}
        if with_installer:
            import windows_installer
            setup, setup_report = windows_installer.build(package, staging, version)
            downloads.append(setup)
        archive = make_zip(staging, name)
        # Name the final ZIP before atomic publication, keeping all files staged
        # until the optional installer and every checksum are complete.
        final_zip = staging/(name+'.zip')
        archive.rename(final_zip)
        downloads.insert(0, final_zip)
        files = downloads.copy()
        for path in downloads:
            checksum_file = staging/(path.name+'.sha256')
            checksum_file.write_text(digest(path)+'  '+path.name+'\n', encoding='ascii')
            files.append(checksum_file)
        bundle_common.publish_files(files, output)
    archive = output/(name+'.zip')
    checksum = digest(archive)
    report = {'status': 'bundled', 'zip': str(archive), 'sha256': checksum,
            'bytes': archive.stat().st_size, 'pe': pe, 'bundled_crt': crt,
            'scope': 'packaging only; desktop and live-login verification reported separately'}
    if with_installer:
        report.update(installer=str(output/(name+'-setup.exe')), installer_details=setup_report)
    if with_portable:
        report.update(portable=str(output/(name+'-portable.exe')), portable_details=portable_report)
    return report


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--out-dir', type=Path, default=ROOT/'dist/windows')
    parser.add_argument('--crt-dir', type=Path)
    parser.add_argument('--installer', action='store_true', help='Also build Setup with automatic WebView2 provisioning')
    parser.add_argument('--portable', action='store_true', help='Also build a standalone EXE with static CRT and embedded notices')
    arguments = parser.parse_args()
    try:
        print(json.dumps(bundle(arguments.out_dir, arguments.crt_dir, arguments.installer, arguments.portable)))
    except (OSError, ValueError, RuntimeError, subprocess.SubprocessError) as error:
        print(str(error), file=sys.stderr)
        raise SystemExit(1)

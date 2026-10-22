"""Build Setup from an already-staged Windows package; never runs the product."""
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import time
import urllib.request

ROOT = Path(__file__).resolve().parent.parent
BOOTSTRAPPER_URL = 'https://go.microsoft.com/fwlink/p/?LinkId=2124703'
MAX_BOOTSTRAPPER_BYTES = 16*1024*1024  # Small online bootstrapper, not the offline Runtime.


def compiler():
    configured = os.environ.get('INNO_SETUP_COMPILER')
    candidate = configured or shutil.which('ISCC.exe')
    if candidate and Path(candidate).is_file():
        return Path(candidate)
    for key in ('ProgramFiles', 'ProgramFiles(x86)'):
        root = os.environ.get(key)
        if root:
            path = Path(root)/'Inno Setup 7/ISCC.exe'
            if path.is_file():
                return path
    raise RuntimeError('Inno Setup 7.1.0 is required; set INNO_SETUP_COMPILER to ISCC.exe')


def run(*args):
    result = subprocess.run(args, cwd=ROOT, check=False, capture_output=True,
                            text=True, encoding='utf-8', errors='replace', timeout=300)
    if result.returncode:
        raise RuntimeError('Installer tool failed: '+(result.stdout+result.stderr)[-6000:])
    return result.stdout


def download_bootstrapper(path):
    """Use the official Evergreen endpoint, with bounded bytes and HTTPS redirects."""
    created = False
    deadline = time.monotonic()+120
    try:
        with urllib.request.urlopen(BOOTSTRAPPER_URL, timeout=60) as response:
            if not response.geturl().startswith('https://'):
                raise RuntimeError('WebView2 bootstrapper redirected away from HTTPS')
            size = 0
            with path.open('xb') as stream:
                created = True
                while chunk := response.read(64*1024):
                    if time.monotonic() > deadline:
                        raise RuntimeError('WebView2 bootstrapper download timed out')
                    size += len(chunk)
                    if size > MAX_BOOTSTRAPPER_BYTES:
                        raise RuntimeError('WebView2 bootstrapper exceeds the size limit')
                    stream.write(chunk)
        if path.read_bytes()[:2] != b'MZ':
            raise RuntimeError('WebView2 download is not a Windows executable')
    except BaseException:
        if created:
            path.unlink(missing_ok=True)
        raise


def build(package, staging, version):
    from release import parse_version
    parse_version(version)
    iscc = compiler()
    config = json.loads((ROOT/'tooling/windows/installer-tools.json').read_text(encoding='utf-8'))
    if run(str(iscc), '--version').strip() != config['version']:
        raise RuntimeError('Use the pinned Inno Setup '+config['version']+' compiler')
    bootstrapper = staging/'MicrosoftEdgeWebview2Setup.exe'
    download_bootstrapper(bootstrapper)
    signature = json.loads(run('powershell.exe', '-NoProfile', '-NonInteractive',
                              '-ExecutionPolicy', 'Bypass', '-File',
                              str(ROOT/'tooling/windows/verify_bootstrapper.ps1'), '-Path', str(bootstrapper)))
    actual = hashlib.sha256(bootstrapper.read_bytes()).hexdigest()
    if (signature.get('status') != 'Valid' or signature.get('publisher') != 'Microsoft Corporation'
            or signature.get('sha256') != actual):
        raise RuntimeError('WebView2 bootstrapper signature/hash verification failed')
    name = f'PTTStella-{version}-windows-x64-setup'
    output = staging/'setup'
    output.mkdir()
    run(str(iscc), '/Qp', '/DAppVersion='+version,
        '/DNumericVersion='+version.split('-', 1)[0], '/DPackageDir='+str(package),
        '/DOutputDir='+str(output), '/DOutputName='+name,
        '/DBootstrapperPath='+str(bootstrapper), '/DBootstrapperSHA256='+actual,
        str(ROOT/'tooling/windows/pttstella.iss'))
    setup = output/(name+'.exe')
    if not setup.is_file():
        raise RuntimeError('Inno Setup did not produce the installer')
    # Inno Setup is explicitly configured for x64 GUI output.
    from bundle_windows import inspect_pe
    inspect_pe(setup)
    return setup, {'compiler': 'Inno Setup '+config['version'],
                   'webview2': {'mode': 'evergreen_bootstrapper', 'url': BOOTSTRAPPER_URL,
                               'bootstrapper': signature, 'install_only_if_missing': True,
                               'internet_required_when_missing': True},
                   'scope': 'current_user', 'runtime_installation_acceptance': 'not_run'}

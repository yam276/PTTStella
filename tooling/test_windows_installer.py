"""Installer packaging never trusts unsigned payloads or delivers partial output."""
import contextlib
import hashlib
import io
import json
import os
from pathlib import Path
import tempfile
import tomllib
import unittest
from unittest.mock import patch
import zipfile

import bundle_windows
import windows_installer as installer


class InstallerTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory(dir=os.environ.get('RUST_GUARD_ISOLATED_ROOT'))
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)

    def response(self, content, url='https://download.microsoft.com/bootstrapper.exe'):
        response = io.BytesIO(content)
        response.geturl = lambda: url
        return contextlib.nullcontext(response)

    def test_bootstrapper_download_is_bounded_and_cleans_only_its_own_file(self):
        path = self.root/'bootstrapper.exe'
        with patch.object(installer.urllib.request, 'urlopen', return_value=self.response(b'MZsynthetic')):
            installer.download_bootstrapper(path)
        self.assertEqual(path.read_bytes(), b'MZsynthetic')
        with patch.object(installer.urllib.request, 'urlopen', return_value=self.response(b'MZreplacement')):
            with self.assertRaises(FileExistsError):
                installer.download_bootstrapper(path)
        self.assertEqual(path.read_bytes(), b'MZsynthetic')
        path.unlink()
        for content, url in [(b'<html>not executable</html>', 'https://example.invalid/file'),
                             (b'MZinsecure', 'http://example.invalid/file'),
                             (b'MZ'+b'x'*100, 'https://example.invalid/file')]:
            with patch.object(installer, 'MAX_BOOTSTRAPPER_BYTES', 64), \
                    patch.object(installer.urllib.request, 'urlopen', return_value=self.response(content, url)):
                with self.assertRaises(RuntimeError):
                    installer.download_bootstrapper(path)
            self.assertFalse(path.exists())

    def test_download_deadline_is_enforced(self):
        path = self.root/'bootstrapper.exe'
        with patch.object(installer.urllib.request, 'urlopen', return_value=self.response(b'MZsynthetic')), \
                patch.object(installer.time, 'monotonic', side_effect=[0, 121]):
            with self.assertRaisesRegex(RuntimeError, 'timed out'):
                installer.download_bootstrapper(path)
        self.assertFalse(path.exists())

    def test_untrusted_publisher_or_hash_cannot_reach_installer_compilation(self):
        for signature in [{'status': 'NotSigned'},
                          {'status': 'Valid', 'publisher': 'Other Publisher', 'sha256': 'a'*64},
                          {'status': 'Valid', 'publisher': 'Microsoft Corporation', 'sha256': 'a'*64}]:
            staging = self.root/str(len(list(self.root.iterdir())))
            staging.mkdir()
            def download(path):
                path.write_bytes(b'MZsynthetic')
            with patch.object(installer, 'compiler', return_value=Path('ISCC.exe')), \
                    patch.object(installer, 'download_bootstrapper', side_effect=download), \
                    patch.object(installer, 'run', side_effect=['Inno Setup 7.1.0', json.dumps(signature)]) as run:
                with self.assertRaisesRegex(RuntimeError, 'signature/hash'):
                    installer.build(staging, staging, '0.1.0')
                self.assertEqual(run.call_count, 2)
            self.assertFalse((staging/'setup').exists())

    def test_verified_payload_hash_is_bound_into_the_compiled_setup(self):
        calls = []
        expected = hashlib.sha256(b'MZsynthetic').hexdigest()
        def run(*args):
            calls.append(args)
            if args[-1] == '/?':
                return 'Inno Setup 7.1.0'
            if args[0] == 'powershell.exe':
                return json.dumps({'status': 'Valid', 'publisher': 'Microsoft Corporation', 'sha256': expected})
            (self.root/'setup/PTTStella-0.1.0-windows-x64-setup.exe').write_bytes(b'MZsynthetic installer')
            return ''
        with patch.object(installer, 'compiler', return_value=Path('ISCC.exe')), \
                patch.object(installer, 'download_bootstrapper', side_effect=lambda p: p.write_bytes(b'MZsynthetic')), \
                patch.object(installer, 'run', side_effect=run), \
                patch.object(bundle_windows, 'inspect_pe', return_value={'machine': 'AMD64'}):
            setup, report = installer.build(self.root, self.root, '0.1.0')
        self.assertTrue(setup.is_file())
        self.assertIn('/DBootstrapperSHA256='+expected, calls[-1])
        self.assertTrue(report['webview2']['install_only_if_missing'])
        self.assertEqual(report['runtime_installation_acceptance'], 'not_run')

    def test_wrong_compiler_fails_before_downloading_or_building_an_installer(self):
        with patch.object(installer, 'compiler', return_value=Path('ISCC.exe')), \
                patch.object(installer, 'run', return_value='Inno Setup 6.6.1'), \
                patch.object(installer, 'download_bootstrapper') as download:
            with self.assertRaisesRegex(RuntimeError, 'pinned Inno Setup'):
                installer.build(self.root, self.root, '0.1.0')
        download.assert_not_called()


class WindowsPublicationTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory(dir=os.environ.get('RUST_GUARD_ISOLATED_ROOT'))
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        self.output = self.root/'output'
        self.executable = self.root/'pttstella.exe'
        self.executable.write_bytes(b'synthetic app; never executed')
        self.version = tomllib.loads((bundle_windows.ROOT/'Cargo.toml').read_text(encoding='utf-8'))['package']['version']
        self.name = f'PTTStella-{self.version}-windows-x64'
        self.stack = contextlib.ExitStack()
        self.addCleanup(self.stack.close)
        self.stack.enter_context(patch.object(bundle_windows.sys, 'platform', 'win32'))
        self.stack.enter_context(patch.object(bundle_windows, 'build', return_value=self.executable))
        self.stack.enter_context(patch.object(bundle_windows, 'inspect_pe', return_value={'imports': []}))
        self.stack.enter_context(patch.object(bundle_windows, 'verify_icon'))
        self.stack.enter_context(patch.object(bundle_windows, 'copy_licenses', side_effect=lambda p: p.mkdir()))

    def setup_file(self, package, staging, version):
        path = staging/(self.name+'-setup.exe')
        path.write_bytes(b'synthetic installer; never executed')
        return path, {'webview2': {'install_only_if_missing': True}}

    def test_installer_failure_delivers_neither_installer_nor_zip(self):
        with patch.object(installer, 'build', side_effect=RuntimeError('compiler failed')):
            with self.assertRaisesRegex(RuntimeError, 'compiler failed'):
                bundle_windows.bundle(self.output, with_installer=True)
        self.assertEqual(list(self.output.iterdir()), [])

    def test_zip_and_setup_share_one_staged_app_and_bootstrapper_is_not_in_zip(self):
        with patch.object(installer, 'build', side_effect=self.setup_file):
            report = bundle_windows.bundle(self.output, with_installer=True)
        self.assertEqual({p.name for p in self.output.iterdir()},
                         {self.name+suffix for suffix in ('.zip', '.zip.sha256', '-setup.exe', '-setup.exe.sha256')})
        with zipfile.ZipFile(report['zip']) as archive:
            self.assertEqual(archive.read(self.name+'/pttstella.exe'), self.executable.read_bytes())
            self.assertFalse(any('Webview2Setup' in name for name in archive.namelist()))
        for path in self.output.glob('*.sha256'):
            data = self.output/path.name.removesuffix('.sha256')
            self.assertEqual(path.read_text(), hashlib.sha256(data.read_bytes()).hexdigest()+'  '+data.name+'\n')

    def test_setup_receives_app_local_crt_icon_and_license_payload_with_hashes(self):
        crt = self.root/'redist/14.0/x64/Microsoft.VC.CRT'
        crt.mkdir(parents=True)
        payload = {'pttstella.exe': self.executable.read_bytes(),
                   'vcruntime140.dll': b'synthetic runtime',
                   'vcruntime140_1.dll': b'synthetic exception runtime',
                   'licenses/NOTICE.txt': b'synthetic license notice'}
        for name in ('vcruntime140.dll', 'vcruntime140_1.dll'):
            (crt/name).write_bytes(payload[name])
        def licenses(path):
            path.mkdir()
            (path/'NOTICE.txt').write_bytes(payload['licenses/NOTICE.txt'])
        def setup(package, staging, version):
            manifest = json.loads((package/'manifest.json').read_text())
            for name, expected in payload.items():
                self.assertEqual((package/name).read_bytes(), expected)
                self.assertEqual(manifest['files'][name], hashlib.sha256(expected).hexdigest())
            self.assertEqual((package/'PTTStella.ico').read_bytes(),
                             (bundle_windows.ROOT/'assets/icons/pttstella.ico').read_bytes())
            self.assertTrue((package/'README.txt').is_file())
            return self.setup_file(package, staging, version)
        with patch.object(bundle_windows, 'inspect_pe', return_value={
                'imports': ['kernel32.dll', 'vcruntime140.dll', 'vcruntime140_1.dll']}), \
                patch.object(bundle_windows, 'copy_licenses', side_effect=licenses), \
                patch.object(installer, 'build', side_effect=setup):
            report = bundle_windows.bundle(self.output, crt_dir=crt, with_installer=True)
        self.assertEqual(report['bundled_crt'], ['vcruntime140.dll', 'vcruntime140_1.dll'])
        with zipfile.ZipFile(report['zip']) as archive:
            for name, expected in payload.items():
                self.assertEqual(archive.read(self.name+'/'+name), expected)

    def test_installer_publication_race_preserves_other_delivery_and_rolls_back_new_zip(self):
        def raced(package, staging, version):
            result = self.setup_file(package, staging, version)
            (self.output/(self.name+'-setup.exe')).write_bytes(b'existing delivery')
            return result
        with patch.object(installer, 'build', side_effect=raced):
            with self.assertRaises(FileExistsError):
                bundle_windows.bundle(self.output, with_installer=True)
        self.assertEqual([p.name for p in self.output.iterdir()], [self.name+'-setup.exe'])
        self.assertEqual((self.output/(self.name+'-setup.exe')).read_bytes(), b'existing delivery')


if __name__ == '__main__':
    unittest.main()

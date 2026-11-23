"""Package/import failure cases use independent PE bytes, never a Windows process."""
import hashlib
import json
from pathlib import Path
import shutil
import struct
import subprocess
import tempfile
import unittest
from unittest.mock import patch
import zipfile

import bundle_slint_windows as bundle
import test_bundle_windows as windows_fixtures


class CandidateTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory(prefix='pttstella-windows-bundle-')
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        self.exe = self.root / 'app.exe'
        self.exe.write_bytes(windows_fixtures.DependencyTests().image(delayed='libmpv-2.dll'))
        self.mpv = self.root / 'libmpv-2.dll'
        self.dll(self.mpv, 'MSVCP140.dll')
        self.license = self.root / 'license.txt'
        self.license.write_text('self-authored synthetic license fixture')
        self.provenance = self.root / 'origin.json'
        self.provenance.write_text(json.dumps({'files': {'libmpv-2.dll': bundle.digest(self.mpv)}}))
        self.record = self.root / 'build.json'
        self.record.write_text(json.dumps({'source_ref': 'a' * 40, 'source_dirty': False,
            'target': bundle.TARGET, 'features': 'native-player-probe', 'build_exit': 0,
            'version': '0.5.0-dev.3', 'exe_sha256': bundle.digest(self.exe)}))
        self.fixtures = self.root / 'fixtures'
        self.fixtures.mkdir()
        files = []
        for name in ['quadrants.mp4', 'quadrants.webm', 'quadrants.ogv']:
            path = self.fixtures / name
            path.write_bytes(('synthetic packaging fixture: ' + name).encode())
            files.append({'file': name, 'sha256': bundle.digest(path)})
        (self.fixtures / 'manifest.json').write_text(json.dumps({'license': 'CC0-1.0', 'files': files}))
        self.output = self.root / 'output'
        self.licenses = patch.object(bundle.bundle_common, 'copy_licenses', side_effect=self.notices)
        self.licenses.start()
        self.addCleanup(self.licenses.stop)

    @staticmethod
    def dll(path, dependency):
        data = windows_fixtures.DependencyTests().image(delayed=dependency)
        struct.pack_into('<H', data, 0xdc, 3)
        path.write_bytes(data)

    @staticmethod
    def notices(destination, target, *, feature):
        assert target == bundle.TARGET and feature == 'native-player-probe'
        destination.mkdir()
        (destination / 'rust-dependencies.json').write_text('[]')

    def package(self, *, with_installer=False):
        return bundle.bundle(self.exe, [self.mpv], self.license, self.license,
                             self.provenance, self.record, self.fixtures, self.output,
                             with_installer=with_installer)

    def test_offline_installer_requires_closed_crt_imports(self):
        with self.assertRaisesRegex(ValueError, 'self-contained CRT closure: msvcp140.dll'):
            self.package(with_installer=True)
        self.assertFalse(self.output.exists())

    def test_app_local_crt_must_match_the_recorded_dll_hash(self):
        crt = self.root / 'msvcp140.dll'
        self.dll(crt, 'kernel32.dll')
        manifest = self.root / 'crt-origin.json'
        for files in ({'msvcp140.dll': '0' * 64}, {'msvcp140.dll': bundle.digest(crt)}):
            manifest.write_text(json.dumps({'files': files}))
            args = (self.exe, [self.mpv, crt], self.license, self.license,
                    self.provenance, self.record, self.fixtures, self.output)
            if files['msvcp140.dll'] != bundle.digest(crt):
                with self.assertRaisesRegex(ValueError, 'CRT provenance'):
                    bundle.bundle(*args, crt_provenance=manifest)
                self.assertFalse(self.output.exists())
            else:
                report = bundle.bundle(*args, crt_provenance=manifest)
                self.assertEqual(report['required_vc_redist_dlls'], [])
                self.assertIn('licenses/vc-crt-provenance.json', report['payload_sha256'])

    def test_installer_uses_the_same_complete_payload_as_the_zip(self):
        self.dll(self.mpv, 'kernel32.dll')
        self.provenance.write_text(json.dumps({'files': {'libmpv-2.dll': bundle.digest(self.mpv)}}))
        staged = {}

        def setup(package, stage, version):
            self.assertEqual(version, '0.5.0-dev.3')
            staged.update((path.relative_to(package).as_posix(), path.read_bytes())
                          for path in package.rglob('*') if path.is_file())
            result = stage / 'candidate-setup.exe'
            result.write_bytes(b'test installer, never executed')
            return result, {'runtime_installation_acceptance': 'not_run'}

        with patch('windows_installer.build_slint_candidate', side_effect=setup):
            report = self.package(with_installer=True)
        self.assertEqual(report['required_vc_redist_dlls'], [])
        installed = self.output / report['installer']['file']
        self.assertEqual(report['installer']['sha256'], bundle.digest(installed))
        self.assertEqual((self.output / (installed.name + '.sha256')).read_text(),
                         f'{bundle.digest(installed)}  {installed.name}\n')
        archive, = self.output.glob('*.zip')
        with zipfile.ZipFile(archive) as zipped:
            self.assertEqual({name.split('/', 1)[1]: zipped.read(name) for name in zipped.namelist()},
                             staged)

    def test_compiler_failure_leaves_no_partial_candidate_download(self):
        self.dll(self.mpv, 'kernel32.dll')
        self.provenance.write_text(json.dumps({'files': {'libmpv-2.dll': bundle.digest(self.mpv)}}))
        with patch('windows_installer.build_slint_candidate', side_effect=RuntimeError('compiler failed')):
            with self.assertRaisesRegex(RuntimeError, 'compiler failed'):
                self.package(with_installer=True)
        self.assertEqual(list(self.output.iterdir()), [])

    def test_full_payload_checksums_runners_and_pending_native_scope(self):
        result = self.package()
        self.assertEqual(result['required_vc_redist_dlls'], ['msvcp140.dll'])
        self.assertEqual(result['native_acceptance'], 'not_run')
        self.assertEqual(result['source_delivery'], 'not_ready')
        archive, = self.output.glob('*.zip')
        self.assertEqual(bundle.digest(archive), result['zip_sha256'])
        with zipfile.ZipFile(archive) as zipped:
            prefix = zipped.namelist()[0].split('/')[0] + '/'
            for name, checksum in result['payload_sha256'].items():
                self.assertEqual(hashlib.sha256(zipped.read(prefix + name)).hexdigest(), checksum)
            offline = zipped.read(prefix + 'Run-Offline.cmd').decode('ascii')
            self.assertIn('--probe-benchmark "%run%"', offline)
            self.assertIn('set PTTSTELLA_', offline)
            self.assertNotIn('--connect', offline)
            software = zipped.read(prefix + 'Run-Video-Software.cmd').decode('ascii')
            self.assertIn('--software-renderer --probe-media', software)
            self.assertIn('PTTSTELLA_VIDEO_FIXTURE=%CD%', software)
            self.assertNotIn(str(self.root), software)
            drafts = zipped.read(prefix + 'Run-Drafts.cmd').decode('ascii')
            self.assertIn('PTTSTELLA_SLINT_DRAFTS_PROBE_DIR=%CD%\\evidence\\drafts', drafts)
            draft_ime = zipped.read(prefix + 'Run-Drafts-IME.cmd').decode('ascii')
            self.assertIn('PTTSTELLA_SLINT_DRAFTS_PROBE_DIR=%CD%\\evidence\\draft-ime', draft_ime)
            self.assertIn('if not exist "%draft%" copy /b /-y', draft_ime)
            self.assertIn('PTTSTELLA_SLINT_EDITOR_TRACE=1', draft_ime)
            self.assertIn('--probe-drafts', draft_ime)
            self.assertNotIn('--connect', draft_ime)
            seed = json.loads(zipped.read(prefix + 'fixtures/draft-ime.json'))
            self.assertEqual(seed['entries'][0]['scope']['account'], 'StellaTest')
            self.assertEqual(seed['entries'][0]['body'], '合成稿第一行\n\x1b[31m保留這行紅字\x1b[m\n')
            motion = zipped.read(prefix + 'Check-Motion-Software.cmd').decode('ascii')
            self.assertIn('--software-renderer --verify-motion "%run%"', motion)
            checks = zipped.read(prefix + 'Check-All.cmd').decode('ascii')
            self.assertIn('powershell.exe -NoLogo -NoProfile -File', checks)
            self.assertNotIn('ExecutionPolicy', checks)
            self.assertEqual(zipped.read(prefix + 'Check-Native.ps1'), bundle.ACCEPTANCE.read_bytes())

    def test_missing_player_or_console_main_is_rejected(self):
        with self.assertRaisesRegex(ValueError, 'requires libmpv'):
            bundle.inspect_payload(self.exe, [])
        console = bytearray(self.exe.read_bytes())
        struct.pack_into('<H', console, 0xdc, 3)
        self.exe.write_bytes(console)
        with self.assertRaisesRegex(ValueError, 'Windows GUI'):
            bundle.inspect_payload(self.exe, [self.mpv])

    def test_unreachable_cycle_is_not_a_runtime_closure(self):
        left, right = self.root / 'left.dll', self.root / 'right.dll'
        self.dll(left, 'right.dll')
        self.dll(right, 'left.dll')
        with self.assertRaisesRegex(ValueError, 'Unused runtime DLLs'):
            bundle.inspect_payload(self.exe, [self.mpv, left, right])

    def test_unknown_and_webview_delay_imports_are_rejected(self):
        for dependency, error in [('private.dll', 'Unbundled non-system'),
                                  ('WebView2Loader.dll', 'WebView dependency'),
                                  ('../kernel32.dll', 'Invalid import name')]:
            with self.subTest(dependency=dependency):
                self.dll(self.mpv, dependency)
                with self.assertRaisesRegex(ValueError, error):
                    bundle.inspect_payload(self.exe, [self.mpv])

    def test_existing_output_is_preserved(self):
        self.output.mkdir()
        sentinel = self.output / 'old.zip'
        sentinel.write_bytes(b'other delivery')
        with self.assertRaises(FileExistsError):
            self.package()
        self.assertEqual(sentinel.read_bytes(), b'other delivery')

    def test_source_license_failure_delivers_no_payload(self):
        with patch.object(bundle.bundle_common, 'copy_licenses', side_effect=RuntimeError('notices failed')):
            with self.assertRaisesRegex(RuntimeError, 'notices failed'):
                self.package()
        self.assertEqual(list(self.output.iterdir()), [])

    def test_changed_source_during_packaging_delivers_nothing(self):
        def change(destination, target, *, feature):
            self.notices(destination, target, feature=feature)
            self.exe.write_bytes(b'changed source')
        with patch.object(bundle.bundle_common, 'copy_licenses', side_effect=change):
            with self.assertRaisesRegex(ValueError, 'input changed'):
                self.package()
        self.assertEqual(list(self.output.iterdir()), [])

    def test_wrong_build_or_runtime_provenance_is_rejected(self):
        self.exe.write_bytes(self.exe.read_bytes() + b'changed')
        with self.assertRaisesRegex(ValueError, 'build record'):
            self.package()
        record = json.loads(self.record.read_text())
        record['exe_sha256'] = bundle.digest(self.exe)
        self.record.write_text(json.dumps(record))
        self.mpv.write_bytes(self.mpv.read_bytes() + b'changed')
        with self.assertRaisesRegex(ValueError, 'provenance'):
            self.package()
        self.assertFalse(self.output.exists())

    def test_changed_fixture_is_not_published(self):
        (self.fixtures / 'quadrants.mp4').write_bytes(b'wrong')
        with self.assertRaisesRegex(ValueError, 'checksum'):
            self.package()
        self.assertFalse(self.output.exists())

    @unittest.skipUnless(shutil.which('pwsh') or shutil.which('powershell.exe'),
                         'PowerShell receipt-helper runner unavailable; not native Windows acceptance')
    def test_powershell_receipt_helpers_reject_false_positive_results(self):
        executable = shutil.which('pwsh') or shutil.which('powershell.exe')
        result = subprocess.run([executable, '-NoLogo', '-NoProfile', '-NonInteractive', '-File',
                                 str(Path(__file__).with_name('test_slint_acceptance.ps1'))],
                                capture_output=True, text=True, timeout=30)
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertIn('Receipt helper fixtures passed', result.stdout)


if __name__ == '__main__':
    unittest.main()

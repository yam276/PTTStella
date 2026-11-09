"""Exercise archive layout, isolation and launchers without a Linux GUI or package install."""
import json
import os
from pathlib import Path
import struct
import subprocess
import tarfile
import tempfile
import unittest
from unittest.mock import patch

import bundle_slint_linux as bundle


class LinuxCandidateTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory(prefix='pttstella-linux-bundle-')
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        data = bytearray(64)
        data[:6] = b'\x7fELF\x02\x01'
        struct.pack_into('<H', data, 18, 62)
        self.exe = self.root / 'synthetic-elf'
        self.exe.write_bytes(data)
        self.license = self.root / 'license'
        self.license.write_text('synthetic license')
        self.record = self.root / 'build.json'
        self.record.write_text(json.dumps({'target': bundle.TARGET, 'features': 'native-player-probe',
            'source_ref': 'a' * 40, 'version': '0.5.0-dev.3', 'exe_sha256': bundle.digest(self.exe),
            'build_exit': 0, 'source_dirty': False}))
        self.fixtures = self.root / 'fixtures'
        self.fixtures.mkdir()
        files = []
        for name in ['quadrants.mp4', 'quadrants.webm', 'quadrants.ogv']:
            p = self.fixtures / name
            p.write_bytes(b'synthetic media fixture')
            files.append({'file': name, 'sha256': bundle.digest(p)})
        (self.fixtures / 'manifest.json').write_text(json.dumps({'license': 'CC0-1.0', 'files': files}))
        self.output = self.root / 'output'
        self.calls = []
        self.layouts = []
        for mocked in [patch.object(bundle.sys, 'platform', 'linux'),
                       patch.object(bundle.bundle_release, 'run', side_effect=self.run_tool),
                       patch.object(bundle.bundle_common, 'copy_licenses', side_effect=self.notices)]:
            mocked.start()
            self.addCleanup(mocked.stop)

    @staticmethod
    def notices(destination, target, *, feature='desktop'):
        destination.mkdir()
        (destination / 'rust-dependencies.json').write_text(json.dumps({'target': target, 'feature': feature}))

    def run_tool(self, *command, cwd=None):
        self.calls.append(command)
        if command[0] == 'ldd' or '--list' in command:
            return 'libmpv.so.2 => /lib/x86_64-linux-gnu/libmpv.so.2 (0x1)'
        if command[0] == 'dpkg-shlibdeps':
            return 'shlibs:Depends=libc6 (>= 2.36), libmpv2 (>= 0.35)'
        if command[0] == 'dpkg-deb' and '--build' in command:
            root = Path(command[-2])
            self.layouts.append({p.relative_to(root).as_posix(): p.read_bytes()
                                 for p in root.rglob('*') if p.is_file()})
            Path(command[-1]).write_bytes(b'synthetic deb tool output')
            return ''
        if command[0] == 'dpkg-deb' and '--field' in command:
            return 'amd64'
        raise AssertionError(command)

    def package(self, **kwargs):
        return bundle.bundle(self.exe, self.license, self.record, self.fixtures, self.output, **kwargs)

    def test_candidate_tar_and_deb_keep_product_identity_separate(self):
        with patch.object(bundle.bundle_common, 'build', side_effect=AssertionError('must not build')):
            report = self.package(loader=Path('/lib64/ld-linux-x86-64.so.2'))
        self.assertEqual(report['native_acceptance'], 'not_run')
        self.assertEqual(report['source_delivery'], 'not_ready')
        self.assertEqual(self.calls[0], ('/lib64/ld-linux-x86-64.so.2', '--list', str(self.exe)))
        for name, checksum in report['downloads'].items():
            self.assertEqual(bundle.digest(self.output / name), checksum)
        archive, = self.output.glob('*.tar.gz')
        with tarfile.open(archive) as tar:
            names = tar.getnames()
            prefix = names[0] + '/'
            self.assertEqual(tar.extractfile(prefix + bundle.PROGRAM).read(), self.exe.read_bytes())
            self.assertEqual(tar.getmember(prefix + bundle.PROGRAM).mode & 0o777, 0o755)
            self.assertIn(prefix + 'fixtures/quadrants.mp4', names)
            self.assertEqual(tar.getmember(prefix + 'Run-IME.sh').mode & 0o777, 0o755)
        layout = self.layouts[0]
        self.assertIn('usr/bin/' + bundle.PROGRAM, layout)
        self.assertNotIn('usr/bin/pttstella', layout)
        desktop = layout['usr/share/applications/io.github.yam276.pttstella.slint.candidate.desktop']
        self.assertIn(b'Exec=pttstella-slint-candidate\n', desktop)
        self.assertIn(b'Package: pttstella-slint-candidate\n', layout['DEBIAN/control'])
        self.assertIn('usr/share/doc/' + bundle.PROGRAM + '/licenses/LICENSE_SKIA', layout)
        self.assertIn(b'libxkbcommon-x11-0', layout['DEBIAN/control'])
        self.assertIn(b'libwayland-client0', layout['DEBIAN/control'])
        self.assertNotIn(b'-dev', layout['DEBIAN/control'].split(b'Depends: ', 1)[1].splitlines()[0])
        self.assertEqual(report['runtime']['late_loaded_dependencies'], list(bundle.WINIT_RUNTIME))

    def test_established_package_defaults_still_use_original_identity(self):
        stage = self.root / 'stage'
        stage.mkdir()
        with patch.object(bundle.bundle_common, 'build', return_value=self.exe) as build:
            bundle.bundle_release.linux_package(stage, 'original', '0.5.0-dev.3')
        build.assert_called_once_with()
        self.assertIn('usr/bin/pttstella', self.layouts[0])
        self.assertIn('usr/share/applications/io.github.yam276.pttstella.desktop', self.layouts[0])
        self.assertNotIn('usr/bin/' + bundle.PROGRAM, self.layouts[0])

    def test_unresolved_or_webview_dependency_does_not_publish(self):
        for output in ['libmpv.so.2 => not found',
                       'libWebKit2GTK.so => /usr/lib/libWebKit2GTK.so (0x1)']:
            with self.subTest(output=output), patch.object(bundle.bundle_release, 'run', return_value=output):
                with self.assertRaises(RuntimeError):
                    self.package()
                self.assertEqual(list(self.output.iterdir()), [])
                self.output.rmdir()

    def test_existing_output_and_changed_source_are_preserved(self):
        self.output.mkdir()
        (self.output / 'old.tar.gz').write_bytes(b'previous delivery')
        with self.assertRaises(FileExistsError):
            self.package()
        self.assertEqual((self.output / 'old.tar.gz').read_bytes(), b'previous delivery')

    def test_input_change_during_staging_publishes_nothing(self):
        def change(destination, target, *, feature):
            self.notices(destination, target, feature=feature)
            self.exe.write_bytes(b'changed')
        with patch.object(bundle.bundle_common, 'copy_licenses', side_effect=change):
            with self.assertRaisesRegex(ValueError, 'input changed'):
                self.package()
        self.assertEqual(list(self.output.iterdir()), [])

    def test_conflicting_resource_cannot_overwrite_binary(self):
        stage = self.root / 'stage'
        stage.mkdir()
        with self.assertRaisesRegex(ValueError, 'resource path'):
            bundle.bundle_release.linux_package(stage, 'bad', '0.5.0-dev.3', self.exe,
                                                resources={'pttstella': self.license})

    @unittest.skipIf(os.name == 'nt', 'POSIX launcher execution requires a POSIX shell')
    def test_runner_clears_diagnostics_keeps_ime_environment_and_exit_code(self):
        base = self.root / 'path with spaces'
        base.mkdir()
        program = base / bundle.PROGRAM
        program.write_text('''#!/bin/sh
test -z "${PTTSTELLA_VIDEO_FIXTURE+x}" || exit 11
test -z "${SLINT_BACKEND+x}" || exit 12
test "$XMODIFIERS" = '@im=fcitx' || exit 13
test "$1" = '--verify-shell' || exit 14
test ! -e "$2" || exit 15
printf 'synthetic target called\\n'
exit 7
''')
        program.chmod(0o755)
        script = base / 'run.sh'
        script.write_text(bundle.runner('--verify-shell "$run/result"'))
        environment = dict(os.environ, PTTSTELLA_VIDEO_FIXTURE='inherited', SLINT_BACKEND='bad',
                           XMODIFIERS='@im=fcitx')
        result = subprocess.run(['sh', str(script)], capture_output=True, text=True, env=environment)
        self.assertEqual(result.returncode, 7, result.stdout + result.stderr)
        self.assertIn('synthetic target called', result.stdout)
        self.assertEqual(len(list((base / 'evidence').glob('run.*'))), 1)


if __name__ == '__main__':
    unittest.main()

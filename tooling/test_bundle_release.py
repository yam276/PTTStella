"""Native Release packages preserve old downloads and reject incomplete input."""
import os
import json
from pathlib import Path
import struct
import tempfile
import tomllib
import unittest
from unittest.mock import patch

import bundle_release as bundle


class PackageTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory(dir=os.environ.get('RUST_GUARD_ISOLATED_ROOT'))
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)

    def test_publication_rolls_back_own_files_on_a_race_and_preserves_existing_output(self):
        staging, output = self.root/'staging', self.root/'output'
        staging.mkdir()
        output.mkdir()
        for name in ('new.zip', 'existing.zip'):
            (staging/name).write_bytes(b'candidate')
        (output/'existing.zip').write_bytes(b'retained')
        with self.assertRaises(FileExistsError):
            bundle.publish_files([staging/'new.zip', staging/'existing.zip'], output)
        self.assertEqual([p.name for p in output.iterdir()], ['existing.zip'])
        self.assertEqual((output/'existing.zip').read_bytes(), b'retained')

    def test_linux_elf_machine_and_unresolved_libraries_are_rejected(self):
        executable = self.root/'pttstella'
        header = bytearray(64)
        header[:6] = b'\x7fELF\x02\x01'
        struct.pack_into('<H', header, 18, 62)
        executable.write_bytes(header)
        with patch.object(bundle, 'run', return_value='libwebkit.so => not found'):
            with self.assertRaisesRegex(RuntimeError, 'unresolved'):
                bundle.inspect_elf(executable)
        with patch.object(bundle, 'run', return_value='custom.so => /home/developer/lib/custom.so (0x123)'):
            with self.assertRaisesRegex(RuntimeError, 'non-system'):
                bundle.inspect_elf(executable)
        header[18] = 183
        executable.write_bytes(header)
        with self.assertRaisesRegex(ValueError, 'x86-64 ELF'):
            bundle.inspect_elf(executable)

    def test_macos_wrong_architecture_cannot_be_mislabeled_or_archived(self):
        with patch.object(bundle.bundle_macos, 'bundle', return_value={'architecture': 'arm64'}), \
                patch.object(bundle, 'run') as runner:
            with self.assertRaisesRegex(RuntimeError, 'architecture'):
                bundle.macos_package(self.root, 'candidate', 'macos-x64')
        runner.assert_not_called()

    def test_native_build_failure_leaves_no_release_downloads(self):
        with patch.object(bundle, 'native_platform', return_value='linux-x64'), \
                patch.object(bundle, 'run', return_value='a'*40), \
                patch.object(bundle.bundle_common, 'build', side_effect=RuntimeError('compiler failed')):
            with self.assertRaisesRegex(RuntimeError, 'compiler failed'):
                bundle.bundle(self.root)
        self.assertEqual(list(self.root.iterdir()), [])

    def test_windows_release_delivers_only_setup_without_portable_or_loose_payloads(self):
        version = tomllib.loads((bundle.ROOT/'Cargo.toml').read_text(encoding='utf-8'))['package']['version']
        stem = f'PTTStella-{version}-windows-x64'
        def windows(output, with_installer=False):
            self.assertTrue(with_installer)
            output.mkdir()
            archive, setup = output/(stem+'.zip'), output/(stem+'-setup.exe')
            archive.write_bytes(b'synthetic portable download')
            setup.write_bytes(b'synthetic setup download')
            (output/'vcruntime140.dll').write_bytes(b'synthetic CRT')
            (output/'assets').mkdir()
            (output/'assets/font.ttf').write_bytes(b'synthetic embedded asset')
            return {'zip': str(archive), 'installer': str(setup),
                    'installer_details': {'webview2': {'install_only_if_missing': True}}}
        output = self.root/'release'
        with patch.object(bundle, 'native_platform', return_value='windows-x64'), \
                patch.object(bundle, 'run', side_effect=['a'*40, '']), \
                patch.object(bundle.bundle_windows, 'bundle', side_effect=windows) as packaged:
            bundle.bundle(output)
        self.assertEqual(packaged.call_count, 1)
        report = json.loads((output/(stem+'.json')).read_text())
        self.assertEqual(set(report['files']), {stem+'-setup.exe'})
        self.assertEqual({p.name for p in output.iterdir()},
                         {stem+suffix for suffix in ('-setup.exe', '-setup.exe.sha256',
                                                     '.json', '.json.sha256')})
        self.assertEqual((output/(stem+'-setup.exe')).read_bytes(), b'synthetic setup download')
        self.assertFalse(report['source_dirty'])
        self.assertTrue(report['installer']['webview2']['install_only_if_missing'])


if __name__ == '__main__':
    unittest.main()

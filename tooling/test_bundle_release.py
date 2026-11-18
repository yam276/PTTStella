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

    def test_macos_disk_image_rejects_missing_shortcut_and_detaches(self):
        app = self.root/'app'/'PTTStella.app'
        app.mkdir(parents=True)
        image = self.root/'candidate.dmg'
        with patch.object(Path, 'symlink_to'), patch.object(bundle, 'run') as runner:
            with self.assertRaisesRegex(RuntimeError, 'missing the /Applications shortcut'):
                bundle.macos_disk_image(app, image)
        runner.assert_called_with('hdiutil', 'detach', str(self.root/'mounted-dmg'))

    def test_macos_dmg_failure_preserves_atomic_zip_delivery(self):
        import bundle_slint_release

        def macos(staging, stem, target, commit):
            (staging/(stem+'.zip')).write_bytes(b'synthetic ZIP')
            raise RuntimeError('image failed')

        with patch.object(bundle, 'native_platform', return_value='macos-arm64'), \
                patch.object(bundle, 'run', side_effect=['a'*40, '']), \
                patch.object(bundle_slint_release, 'macos', side_effect=macos):
            with self.assertRaisesRegex(RuntimeError, 'image failed'):
                bundle.bundle(self.root)
        self.assertEqual(list(self.root.iterdir()), [])

    def test_native_build_failure_leaves_no_release_downloads(self):
        with patch.object(bundle, 'native_platform', return_value='linux-x64'), \
                patch.object(bundle, 'run', return_value='a'*40), \
                patch.object(bundle.bundle_common, 'build', side_effect=RuntimeError('compiler failed')):
            with self.assertRaisesRegex(RuntimeError, 'compiler failed'):
                bundle.bundle(self.root)
        self.assertEqual(list(self.root.iterdir()), [])

    def test_existing_portable_zip_is_preserved_before_any_build(self):
        version = tomllib.loads((bundle.ROOT/'Cargo.toml').read_text())['package']['version']
        path = self.root/f'PTTStella-{version}-windows-x64-portable.zip'
        path.write_bytes(b'retained portable delivery')
        import bundle_slint_release
        with patch.object(bundle, 'native_platform', return_value='windows-x64'), \
                patch.object(bundle_slint_release, 'windows') as build:
            with self.assertRaises(FileExistsError):
                bundle.bundle(self.root)
        build.assert_not_called()
        self.assertEqual(path.read_bytes(), b'retained portable delivery')

    def test_windows_release_delivers_slint_setup_portable_zip_and_sources(self):
        import bundle_slint_release
        version = tomllib.loads((bundle.ROOT/'Cargo.toml').read_text(encoding='utf-8'))['package']['version']
        stem = f'PTTStella-{version}-windows-x64'
        def windows(staging, name, package_version, commit):
            self.assertEqual((name, package_version, commit), (stem, version, 'a'*40))
            files = []
            for suffix, data in (('-setup.exe', b'synthetic setup'), ('-portable.zip', b'synthetic portable'),
                                 ('-sources.tar', b'synthetic sources')):
                files.append(staging/(stem+suffix))
                files[-1].write_bytes(data)
            # Loose payloads in staging are never delivered.
            (staging/'libmpv-2.dll').write_bytes(b'synthetic runtime')
            return files, {'installer': {'webview2': 'not_required'}, 'sources': {'status': 'partial'}}
        output = self.root/'release'
        with patch.object(bundle, 'native_platform', return_value='windows-x64'), \
                patch.object(bundle, 'run', side_effect=['a'*40, '']), \
                patch.object(bundle_slint_release, 'windows', side_effect=windows) as packaged:
            bundle.bundle(output)
        self.assertEqual(packaged.call_count, 1)
        report = json.loads((output/(stem+'.json')).read_text())
        names = {stem+'-setup.exe', stem+'-portable.zip', stem+'-sources.tar'}
        self.assertEqual(set(report['files']), names)
        self.assertEqual(report['frontend'], 'slint')
        self.assertEqual({p.name for p in output.iterdir()},
                         names | {n+'.sha256' for n in names} | {stem+'.json', stem+'.json.sha256'})
        self.assertFalse(report['source_dirty'])
        self.assertEqual(report['installer']['webview2'], 'not_required')


if __name__ == '__main__':
    unittest.main()

"""Publication must fail closed and preserve an existing Windows test package."""
import tempfile
import os
from pathlib import Path
import unittest
from unittest.mock import patch
import zipfile

import bundle_windows as bundle


class PublicationTests(unittest.TestCase):
    def test_existing_package_is_not_replaced(self):
        with tempfile.TemporaryDirectory() as temp:
            output = Path(temp)
            old = output/'PTTStella-0.1.0-dev.1-windows-x64.zip'
            old.write_bytes(b'previous verified package')
            with patch.object(bundle.sys, 'platform', 'win32'), patch.object(bundle, 'build') as build:
                with self.assertRaises(FileExistsError):
                    bundle.bundle(output)
                build.assert_not_called()
            self.assertEqual(old.read_bytes(), b'previous verified package')

    def test_build_failure_does_not_deliver_a_zip(self):
        with tempfile.TemporaryDirectory() as temp:
            with patch.object(bundle.sys, 'platform', 'win32'), patch.object(bundle, 'build', side_effect=RuntimeError('compiler failed')):
                with self.assertRaisesRegex(RuntimeError, 'compiler failed'):
                    bundle.bundle(Path(temp))
            self.assertEqual(list(Path(temp).iterdir()), [])

    def test_missing_embedded_icon_does_not_deliver_a_zip(self):
        with tempfile.TemporaryDirectory() as temp:
            with patch.object(bundle.sys, 'platform', 'win32'), patch.object(bundle, 'build', return_value=Path('unused.exe')), \
                 patch.object(bundle, 'inspect_pe', return_value={'imports': []}), \
                 patch.object(bundle, 'verify_icon', side_effect=ValueError('missing icon')):
                with self.assertRaisesRegex(ValueError, 'missing icon'):
                    bundle.bundle(Path(temp))
            self.assertEqual(list(Path(temp).iterdir()), [])

    def test_unknown_dll_is_not_silently_omitted(self):
        with tempfile.TemporaryDirectory() as temp:
            with patch.object(bundle.sys, 'platform', 'win32'), patch.object(bundle, 'build', return_value=Path('unused.exe')), \
                 patch.object(bundle, 'inspect_pe', return_value={'imports': ['private-development.dll']}), \
                 patch.object(bundle, 'verify_icon'):
                with self.assertRaisesRegex(RuntimeError, 'Unbundled non-system import'):
                    bundle.bundle(Path(temp))
            self.assertEqual(list(Path(temp).iterdir()), [])

    def test_atomic_publication_contains_only_staged_package_and_refuses_race(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            output = root/'output'
            output.mkdir()
            staging = root/'staging'
            package = staging/'PTTStella-test'
            package.mkdir(parents=True)
            (package/'README.txt').write_text('test package')
            (staging/'private-settings.json').write_text('must not ship')
            result = bundle.publish_zip(staging, output, package.name)
            with zipfile.ZipFile(result) as archive:
                self.assertEqual(archive.namelist(), ['PTTStella-test/README.txt'])
                self.assertIsNone(archive.testzip())
            original = result.read_bytes()
            (package/'README.txt').write_text('new candidate')
            with self.assertRaises(FileExistsError):
                bundle.publish_zip(staging, output, package.name)
            self.assertEqual(result.read_bytes(), original)

    def test_registry_license_timestamp_before_1980_is_packaged(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            staging, output = root/'staging', root/'output'
            package = staging/'PTTStella-test'
            package.mkdir(parents=True)
            output.mkdir()
            license_file = package/'LICENSE'
            license_file.write_text('synthetic license')
            os.utime(license_file, (0, 0))
            archive = bundle.publish_zip(staging, output, package.name)
            with zipfile.ZipFile(archive) as zipped:
                self.assertEqual(zipped.read('PTTStella-test/LICENSE'), b'synthetic license')
                self.assertEqual(zipped.getinfo('PTTStella-test/LICENSE').date_time[:3], (1980, 1, 1))


if __name__ == '__main__':
    unittest.main()

"""Publication must fail closed and preserve an existing Windows test package."""
import tempfile
import os
from pathlib import Path
import unittest
import tomllib
from unittest.mock import patch
import zipfile
import struct

import bundle_windows as bundle


class DependencyTests(unittest.TestCase):
    def image(self, delayed='USER32.dll', attributes=1, image_base=0x140000000):
        # Independent minimal PE32+: header at 0x80, one file-backed section
        # (RVA 0x1000 -> file 0x200), normal and delay import tables.
        data = bytearray(0x600)
        data[:2] = b'MZ'
        struct.pack_into('<I', data, 0x3c, 0x80)
        data[0x80:0x84] = b'PE\0\0'
        struct.pack_into('<HH', data, 0x84, 0x8664, 1)
        struct.pack_into('<H', data, 0x94, 240)
        struct.pack_into('<H', data, 0x98, 0x20b)
        struct.pack_into('<Q', data, 0xb0, image_base)
        struct.pack_into('<H', data, 0xdc, 2)
        struct.pack_into('<IIII', data, 0x190, 0x400, 0x1000, 0x400, 0x200)
        struct.pack_into('<II', data, 0x110, 0x1000, 40)
        struct.pack_into('<IIIII', data, 0x200, 0, 0, 0, 0x1100, 0)
        data[0x300:0x30d] = b'KERNEL32.dll\0'
        struct.pack_into('<II', data, 0x170, 0x1040, 64)
        name = 0x1120+(image_base if attributes == 0 else 0)
        struct.pack_into('<IIIIIIII', data, 0x240, attributes, name, 0, 0, 0, 0, 0, 0)
        text = delayed.encode('ascii')+b'\0'
        data[0x320:0x320+len(text)] = text
        return data

    def inspect(self, data):
        with tempfile.TemporaryDirectory() as temp:
            path = Path(temp)/'synthetic.exe'
            path.write_bytes(data)
            return bundle.inspect_pe(path)

    def test_regular_and_delayed_imports_share_the_dependency_inventory(self):
        from windows_portable import verify_dependencies
        result = self.inspect(self.image())
        self.assertEqual(result, {'machine': 'AMD64', 'subsystem': 'Windows GUI',
                                'imports': ['kernel32.dll', 'user32.dll'],
                                'delay_imports': ['user32.dll']})
        verify_dependencies(result)
        legacy = self.inspect(self.image(attributes=0, image_base=0x400000))
        self.assertEqual(legacy, result)

    def test_delayed_private_dll_is_still_rejected_by_both_application_policies(self):
        from windows_portable import verify_dependencies
        result = self.inspect(self.image(delayed='private-development.dll'))
        with self.assertRaisesRegex(RuntimeError, 'external DLL: private-development.dll'):
            verify_dependencies(result)
        with patch.object(bundle, 'inspect_pe', return_value=result), patch.object(bundle, 'verify_icon'):
            with self.assertRaisesRegex(RuntimeError, 'Unbundled non-system import: private-development.dll'):
                bundle.inspect_application(Path('unused.exe'))

    def test_malformed_delay_tables_fail_closed(self):
        for field, value, error in [(0x240, 2, 'attributes'),
                                    (0x244, 0x1800, 'file-backed'),
                                    (0x174, 32, 'Unterminated'),
                                    (0x170, 0, 'missing its RVA')]:
            with self.subTest(field=field):
                data = self.image()
                struct.pack_into('<I', data, field, value)
                with self.assertRaisesRegex(ValueError, error):
                    self.inspect(data)
        data = self.image()
        data[0x320:0x420] = b'x'*256
        with self.assertRaisesRegex(ValueError, 'DLL name'):
            self.inspect(data)
        data = self.image()
        struct.pack_into('<I', data, 0x244, 0x13ff)
        data[-1] = ord('x')
        with self.assertRaisesRegex(ValueError, 'file-backed'):
            self.inspect(data)


class PublicationTests(unittest.TestCase):
    def test_existing_package_is_not_replaced(self):
        with tempfile.TemporaryDirectory() as temp:
            output = Path(temp)
            version = tomllib.loads((bundle.ROOT/'Cargo.toml').read_text(encoding='utf-8'))['package']['version']
            old = output/f'PTTStella-{version}-windows-x64.zip'
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

    def test_zip_contains_only_staged_package(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            output = root/'output'
            output.mkdir()
            staging = root/'staging'
            package = staging/'PTTStella-test'
            package.mkdir(parents=True)
            (package/'README.txt').write_text('test package')
            (staging/'private-settings.json').write_text('must not ship')
            result = bundle.make_zip(staging, package.name)
            with zipfile.ZipFile(result) as archive:
                self.assertEqual(archive.namelist(), ['PTTStella-test/README.txt'])
                self.assertIsNone(archive.testzip())

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
            archive = bundle.make_zip(staging, package.name)
            with zipfile.ZipFile(archive) as zipped:
                self.assertEqual(zipped.read('PTTStella-test/LICENSE'), b'synthetic license')
                self.assertEqual(zipped.getinfo('PTTStella-test/LICENSE').date_time[:3], (1980, 1, 1))


if __name__ == '__main__':
    unittest.main()

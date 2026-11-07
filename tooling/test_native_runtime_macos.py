"""Independent loader/fixture rejection cases; actual Mach-O acceptance is separate."""
import hashlib
import io
from pathlib import Path
import tempfile
import tarfile
import unittest

from native_runtime_macos import parse_commands, resolve_import, source_notices


class RuntimeTests(unittest.TestCase):
    def test_load_identity_spaces_weak_and_rpaths_are_distinct(self):
        value = parse_commands('''Load command 0
          cmd LC_ID_DYLIB
      cmdsize 48
         name /brew/lib/self.dylib (offset 24)
Load command 1
          cmd LC_LOAD_WEAK_DYLIB
         name /brew/with spaces/other.dylib (offset 24)
Load command 2
          cmd LC_RPATH
         path @loader_path/../lib (offset 12)
Load command 3
          cmd LC_LOAD_DYLIB
         name /usr/lib/libSystem.B.dylib (offset 24)
''')
        self.assertEqual(value, {'id': '/brew/lib/self.dylib',
                                 'imports': ['/brew/with spaces/other.dylib', '/usr/lib/libSystem.B.dylib'],
                                 'rpaths': ['@loader_path/../lib']})

    def test_missing_traversing_and_symlink_escape_cannot_copy_user_files(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            allowed = root / 'allowed'
            allowed.mkdir()
            outside = root / 'outside.dylib'
            outside.write_bytes(b'owned test')
            (allowed / 'escape.dylib').symlink_to(outside)
            exe = allowed / 'native'
            for name in ('@loader_path/escape.dylib', '@loader_path/../outside.dylib', '@rpath/missing.dylib'):
                with self.assertRaises(ValueError):
                    resolve_import(name, exe, exe, ['@loader_path/lib'], (allowed,))
            self.assertIsNone(resolve_import('/usr/lib/libSystem.B.dylib', exe, exe, [], (allowed,)))
            with self.assertRaises(ValueError):
                resolve_import('/usr/lib/../../private/file', exe, exe, [], (allowed,))

    def test_source_notices_verify_archive_and_never_extract_links_or_traversal(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            archive = root / 'owned.tar'
            with tarfile.open(archive, 'w') as tar:
                for name in ('owned/LICENSES/LGPL-2.1.txt', '../COPYING', '/COPYRIGHT'):
                    data = b'owned test notice'
                    member = tarfile.TarInfo(name)
                    member.size = len(data)
                    tar.addfile(member, io.BytesIO(data))
                link = tarfile.TarInfo('owned/LICENSES/link')
                link.type = tarfile.SYMTYPE
                link.linkname = '/private/file'
                tar.addfile(link)
            source = {'checksums': [{'algorithm': 'SHA256', 'checksumValue': '0' * 64}]}
            with self.assertRaises(ValueError):
                source_notices(source, archive, root / 'bad')
            self.assertFalse((root / 'bad').exists())
            source['checksums'][0]['checksumValue'] = hashlib.sha256(archive.read_bytes()).hexdigest()
            records = source_notices(source, archive, root / 'notices')
            self.assertEqual([r['file'] for r in records], ['owned__LICENSES__LGPL-2.1.txt'])
            self.assertEqual((root / 'notices' / records[0]['file']).read_bytes(), b'owned test notice')
            self.assertEqual(len(list((root / 'notices').iterdir())), 2)  # One notice and provenance.


if __name__ == '__main__':
    unittest.main()

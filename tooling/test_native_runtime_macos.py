"""Loader/notice regressions, with an actual dyld relocation fixture on macOS."""
import hashlib
import io
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import tarfile
import unittest
from unittest.mock import patch

import native_runtime_macos as runtime
from native_runtime_macos import parse_commands, resolve_import, source_notices


class RuntimeTests(unittest.TestCase):
    def test_inherited_run_paths_keep_each_loader_context(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            executable = root / 'app/MacOS/native'
            library = root / 'app/Frameworks/liba.dylib'
            child = root / 'app/Frameworks/nested/libb.dylib'
            sibling = root / 'app/Frameworks/libc.dylib'
            for file in (executable, library, child, sibling):
                file.parent.mkdir(parents=True, exist_ok=True)
                file.write_bytes(b'synthetic Mach-O stand-in')
            information = {
                executable: {'id': None, 'imports': ['@rpath/liba.dylib'],
                             'rpaths': ['@loader_path/../Frameworks']},
                library: {'id': '@rpath/liba.dylib', 'imports': ['@rpath/libb.dylib'],
                          'rpaths': ['@loader_path/nested']},
                child: {'id': '@rpath/libb.dylib', 'imports': ['@rpath/libc.dylib'],
                        'rpaths': []},
                sibling: {'id': '@rpath/libc.dylib', 'imports': ['/usr/lib/libSystem.B.dylib'],
                        'rpaths': []},
            }
            with patch.object(runtime, 'inspect', side_effect=lambda p: information[p]):
                records = runtime.closure(executable, (root / 'app/Frameworks',))
            self.assertEqual(records[executable]['edges'], {'@rpath/liba.dylib': library})
            self.assertEqual(records[library]['edges'], {'@rpath/libb.dylib': child})
            self.assertEqual(records[child]['edges'], {'@rpath/libc.dylib': sibling})

    def test_inherited_run_path_does_not_bypass_allowed_prefix(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            executable, foreign = root / 'native', root / 'outside/libb.dylib'
            executable.write_bytes(b'owned')
            foreign.parent.mkdir()
            foreign.write_bytes(b'not allowed')
            allowed = root / 'allowed'
            allowed.mkdir()
            library = allowed / 'liba.dylib'
            library.write_bytes(b'owned')
            information = {
                executable: {'id': None, 'imports': ['@rpath/liba.dylib'],
                             'rpaths': ['@loader_path/allowed', '@loader_path/outside']},
                library: {'id': '@rpath/liba.dylib', 'imports': ['@rpath/libb.dylib'], 'rpaths': []},
            }
            with patch.object(runtime, 'inspect', side_effect=lambda p: information[p]):
                with self.assertRaisesRegex(ValueError, 'outside explicit allowed'):
                    runtime.closure(executable, (allowed,))

    def test_non_cellar_dependency_cannot_silently_skip_notices(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            executable, foreign = root / 'native', root / 'libmpv.dylib'
            destination = root / 'notices'
            with self.assertRaisesRegex(ValueError, 'source/notice provenance'):
                runtime.copy_notices({executable: {}, foreign: {}}, destination,
                                     executable=executable)
            self.assertFalse(destination.exists())

    @unittest.skipUnless(sys.platform == 'darwin', 'Requires actual Mach-O tools and dyld')
    def test_actual_transitive_dyld_load_after_relocation(self):
        with tempfile.TemporaryDirectory(prefix='pttstella-dyld-') as temporary:
            root = Path(temporary)
            keg = root / 'Cellar/owned/1'
            lib = keg / 'lib'
            lib.mkdir(parents=True)
            (keg / 'LICENSE').write_text('Synthetic test library; no third-party code.')
            (keg / 'INSTALL_RECEIPT.json').write_text('{}')
            (keg / 'sbom.spdx.json').write_text(json.dumps({'packages': [
                {'SPDXID': 'SPDXRef-owned-src', 'name': 'synthetic fixture'}]}))
            for name, text in {'b': 'int value_b(void){return 41;}',
                               'a': 'int value_b(void); int value_a(void){return value_b()+1;}',
                               'main': 'int value_a(void); int main(void){return value_a()==42?0:1;}'}.items():
                (root / (name + '.c')).write_text(text)
            for name, extra in [('b', []), ('a', ['-L' + str(lib), '-lb'])]:
                subprocess.run(['clang', '-dynamiclib', str(root / (name + '.c')),
                                '-Wl,-install_name,@rpath/lib' + name + '.dylib',
                                *extra, '-o', str(lib / ('lib' + name + '.dylib'))],
                               check=True, capture_output=True, text=True)
            source = root / 'native'
            subprocess.run(['clang', str(root / 'main.c'), '-L' + str(lib), '-la',
                            '-Wl,-rpath,' + str(lib), '-o', str(source)],
                           check=True, capture_output=True, text=True)
            environment = {key: value for key, value in os.environ.items()
                           if not key.startswith(('DYLD_', 'LD_'))}
            subprocess.run([str(source)], check=True, capture_output=True, env=environment)
            contents = root / 'bundle/Contents'
            binary = contents / 'MacOS/native'
            binary.parent.mkdir(parents=True)
            (contents / 'Resources').mkdir()
            manifest = runtime.install(source, binary, contents / 'Frameworks',
                                       contents / 'Resources/notices', (keg,))
            self.assertEqual(len(manifest['files']), 3)
            self.assertEqual(runtime.inspect(binary)['rpaths'], ['@executable_path/../Frameworks'])
            for file in (contents / 'Frameworks').iterdir():
                self.assertEqual(runtime.inspect(file)['rpaths'], [])
            subprocess.run([str(binary)], check=True, capture_output=True, env=environment)

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

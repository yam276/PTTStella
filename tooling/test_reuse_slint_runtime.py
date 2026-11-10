"""Reusing an existing candidate must preserve the audited closure and notices."""
import json
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch

import native_runtime_macos as runtime


class ReuseTests(unittest.TestCase):
    def fixture(self, root):
        old = root / 'previous package/Candidate.app'
        contents = old / 'Contents'
        resources = contents / 'Resources'
        notices = resources / 'NativeRuntimeNotices'
        for path in (contents / 'MacOS', contents / 'Frameworks', notices):
            path.mkdir(parents=True)
        old_binary = contents / 'MacOS/native'
        library = contents / 'Frameworks/libmpv.2.dylib'
        old_binary.write_bytes(b'old executable fixture')
        library.write_bytes(b'unchanged library fixture')
        packages = [{'package': 'self-authored-CC0', 'notice_files_found': True}]
        (notices / 'LICENSE').write_text('Self-authored fixture, CC0.')
        (notices / 'generated-empty-LICENSE').write_bytes(b'')
        (notices / 'inventory.json').write_text(json.dumps(packages))
        descriptor = {'target': 'x86_64-apple-darwin', 'features': 'native-player-probe',
                      'status': 'bundled'}
        (resources / 'candidate.json').write_text(json.dumps(descriptor))
        (old.parent / 'package-report.json').write_text(json.dumps({
            **descriptor, 'executable_sha256': runtime.digest(old_binary)}))
        manifest = {'files': [{'file': 'MacOS/native', 'bundled_sha256': 'link-stage hash'},
                              {'file': 'Frameworks/libmpv.2.dylib',
                               'bundled_sha256': runtime.digest(library),
                               'imports': ['/usr/lib/libSystem.B.dylib'], 'rpaths': []}],
                    'packages': packages, 'source_delivery': 'not_ready',
                    'dlopen_dependencies': 'requires codec/backend traces'}
        (resources / 'runtime-manifest.json').write_text(json.dumps(manifest))
        source = root / 'new-executable'
        source.write_bytes(b'new executable fixture')
        output = root / 'stage/Contents'
        (output / 'MacOS').mkdir(parents=True)
        (output / 'Resources').mkdir()
        return old, source, output

    def information(self, path):
        if path.name.endswith('.dylib'):
            return {'imports': ['/usr/lib/libSystem.B.dylib'], 'rpaths': [],
                    'id': '@rpath/' + path.name}
        return {'imports': ['@rpath/libmpv.2.dylib', '/usr/lib/libSystem.B.dylib'],
                'rpaths': ['/old/build/path'] if path.name == 'new-executable'
                else ['@executable_path/../Frameworks'], 'id': None}

    def tools(self, *command):
        return 'x86_64\n' if command[:2] == ('lipo', '-archs') else ''

    def reuse(self, old, source, output):
        return runtime.reuse_candidate(source, output / 'MacOS/native', output / 'Frameworks',
                                       output / 'Resources/NativeRuntimeNotices', old)

    def test_copies_exact_libraries_and_notices_without_claiming_a_runtime_build(self):
        with tempfile.TemporaryDirectory() as temporary:
            old, source, output = self.fixture(Path(temporary))
            before = runtime.digest(source)
            with patch.object(runtime, 'inspect', side_effect=self.information), \
                    patch.object(runtime, 'run', side_effect=self.tools) as commands:
                report = self.reuse(old, source, output)
            self.assertEqual(runtime.digest(source), before)
            self.assertEqual((output / 'Frameworks/libmpv.2.dylib').read_bytes(),
                             (old / 'Contents/Frameworks/libmpv.2.dylib').read_bytes())
            self.assertEqual(report['source_delivery'], 'not_ready')
            self.assertEqual(report['reused_candidate']['executable_sha256'],
                             runtime.digest(old / 'Contents/MacOS/native'))
            self.assertEqual(report['files'][0]['rpaths'], ['@executable_path/../Frameworks'])
            self.assertEqual((output / 'Resources/NativeRuntimeNotices/generated-empty-LICENSE').read_bytes(), b'')
            self.assertEqual(len(list((output / 'Resources/NativeRuntimeNotices/reused-runtime').glob('*.json'))), 1)
            self.assertIn(('codesign', '--verify', '--deep', '--strict', str(old.resolve())),
                          [call.args for call in commands.call_args_list])

    def test_rejects_mutated_missing_duplicate_extra_or_symlinked_inputs(self):
        for bad in ('binary', 'library', 'notice', 'extra', 'duplicate', 'architecture', 'report'):
            with self.subTest(bad=bad), tempfile.TemporaryDirectory() as temporary:
                old, source, output = self.fixture(Path(temporary))
                resources = old / 'Contents/Resources'
                manifest_path = resources / 'runtime-manifest.json'
                if bad == 'binary':
                    (old / 'Contents/MacOS/native').write_bytes(b'mutated')
                elif bad == 'library':
                    (old / 'Contents/Frameworks/libmpv.2.dylib').write_bytes(b'mutated')
                elif bad == 'notice':
                    (resources / 'NativeRuntimeNotices/link').symlink_to(source)
                elif bad == 'extra':
                    (old / 'Contents/Frameworks/extra.dylib').write_bytes(b'extra')
                elif bad == 'duplicate':
                    manifest = json.loads(manifest_path.read_text())
                    manifest['files'].append(manifest['files'][1])
                    manifest_path.write_text(json.dumps(manifest))
                elif bad == 'architecture':
                    descriptor_path = resources / 'candidate.json'
                    descriptor = json.loads(descriptor_path.read_text())
                    descriptor['target'] = 'aarch64-apple-darwin'
                    descriptor_path.write_text(json.dumps(descriptor))
                else:
                    (old.parent / 'package-report.json').unlink()
                with patch.object(runtime, 'inspect', side_effect=self.information), \
                        patch.object(runtime, 'run', side_effect=self.tools):
                    with self.assertRaises(ValueError):
                        self.reuse(old, source, output)
                self.assertFalse((output / 'Frameworks').exists())

    def test_rejects_unbundled_imports_and_signature_failure(self):
        for bad in ('import', 'signature'):
            with self.subTest(bad=bad), tempfile.TemporaryDirectory() as temporary:
                old, source, output = self.fixture(Path(temporary))

                def information(path):
                    value = self.information(path)
                    if bad == 'import' and path == source:
                        value['imports'].append('@rpath/foreign.dylib')
                    return value

                def tools(*command):
                    if bad == 'signature' and command[:2] == ('codesign', '--verify'):
                        raise subprocess.CalledProcessError(1, command)
                    return self.tools(*command)

                with patch.object(runtime, 'inspect', side_effect=information), \
                        patch.object(runtime, 'run', side_effect=tools):
                    with self.assertRaises((ValueError, subprocess.CalledProcessError)):
                        self.reuse(old, source, output)
                self.assertFalse((output / 'Frameworks').exists())

"""Standalone delivery rejects DLL sidecars and proves its embedded notices."""
import contextlib
import hashlib
import json
import os
from pathlib import Path
import tempfile
import tomllib
import unittest
from unittest.mock import patch

import bundle_windows
import windows_installer
import windows_portable as portable


class PortableTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory(dir=os.environ.get('RUST_GUARD_ISOLATED_ROOT'))
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)

    def test_static_build_preserves_host_environment_and_existing_compiler_flags(self):
        for supplied, expected in [({'RUSTFLAGS': '-C debuginfo=0'}, ['-C', 'debuginfo=0']),
                                   ({'CARGO_ENCODED_RUSTFLAGS': '--cfg\x1fsynthetic="a b"',
                                     'RUSTFLAGS': 'ignored'}, ['--cfg', 'synthetic="a b"'])]:
            with patch.dict(os.environ, supplied, clear=True), \
                    patch.object(portable.bundle_common, 'build') as build:
                os.environ['PATH'] = 'retained compiler wrapper'
                before = os.environ.copy()
                portable.build(self.root/'licenses.txt')
                self.assertEqual(dict(os.environ), before)
                env = build.call_args.kwargs['env']
                self.assertEqual(env['PATH'], before['PATH'])
                self.assertEqual(env['CARGO_ENCODED_RUSTFLAGS'].split('\x1f'),
                                 expected+['-C', 'target-feature=+crt-static'])
                self.assertEqual(env['PTTSTELLA_NOTICES_FILE'], str((self.root/'licenses.txt').resolve()))
                self.assertEqual(build.call_args.args, ('x86_64-pc-windows-msvc',))

    def test_only_system_dlls_are_allowed_in_standalone_exe(self):
        portable.verify_dependencies({'imports': ['kernel32.dll', 'user32.dll', 'api-ms-win-core-synch-l1-2-0.dll']})
        for dll in ('vcruntime140.dll', 'vcruntime140_1.dll', 'msvcp140.dll',
                    'ucrtbased.dll', 'WebView2Loader.dll', 'private-development.dll'):
            with self.subTest(dll=dll), self.assertRaisesRegex(RuntimeError, 'external DLL'):
                portable.verify_dependencies({'imports': ['kernel32.dll', dll]})

    def test_license_collection_retains_nested_text_and_requires_inventory(self):
        licenses = self.root/'licenses'
        (licenses/'synthetic-1.0').mkdir(parents=True)
        (licenses/'synthetic-1.0/LICENSE').write_text('Synthetic license\n合成授權\n', encoding='utf-8')
        output = self.root/'notices.txt'
        with self.assertRaisesRegex(RuntimeError, 'inventory'):
            portable.write_notices(licenses, output)
        self.assertFalse(output.exists())
        (licenses/'rust-dependencies.json').write_text('[]', encoding='utf-8')
        portable.write_notices(licenses, output)
        text = output.read_text(encoding='utf-8')
        self.assertIn('===== synthetic-1.0/LICENSE =====\nSynthetic license\n合成授權\n', text)
        self.assertIn('rust-dependencies.json', text)

    def test_export_runs_shipped_exe_in_an_empty_folder_and_rejects_changed_content(self):
        executable, notices = self.root/'app.exe', self.root/'notices.txt'
        executable.write_bytes(b'synthetic; never executed')
        notices.write_bytes(b'synthetic compiled notices')
        for correct in (True, False):
            staging = self.root/str(correct)
            staging.mkdir()
            def run(command, **kwargs):
                isolated, flag, output = command
                self.assertEqual(Path(isolated).read_bytes(), executable.read_bytes())
                self.assertEqual(flag, '--export-licenses')
                self.assertEqual(kwargs['cwd'], Path(isolated).parent)
                self.assertEqual({p.name for p in kwargs['cwd'].iterdir()}, {'pttstella.exe'})
                Path(output).write_bytes(notices.read_bytes() if correct else b'incomplete')
            with patch.object(portable.subprocess, 'run', side_effect=run):
                if correct:
                    portable.verify_export(executable, notices, staging)
                else:
                    with self.assertRaisesRegex(RuntimeError, 'compiled license inventory'):
                        portable.verify_export(executable, notices, staging)


class PortablePublicationTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory(dir=os.environ.get('RUST_GUARD_ISOLATED_ROOT'))
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        self.output = self.root/'output'
        self.executable = self.root/'app.exe'
        self.executable.write_bytes(b'synthetic standalone App with embedded assets')
        version = tomllib.loads((bundle_windows.ROOT/'Cargo.toml').read_text())['package']['version']
        self.stem = f'PTTStella-{version}-windows-x64'
        stack = contextlib.ExitStack()
        self.addCleanup(stack.close)
        stack.enter_context(patch.object(bundle_windows.sys, 'platform', 'win32'))
        stack.enter_context(patch.object(bundle_windows, 'verify_icon'))
        stack.enter_context(patch.object(bundle_windows, 'copy_licenses', side_effect=self.licenses))
        self.compiler = stack.enter_context(patch.object(portable, 'build', return_value=self.executable))
        self.inspector = stack.enter_context(patch.object(bundle_windows, 'inspect_pe', return_value={
            'imports': ['kernel32.dll', 'user32.dll']}))
        self.exporter = stack.enter_context(patch.object(portable, 'verify_export'))
        self.installer = stack.enter_context(patch.object(windows_installer, 'build', side_effect=self.setup))

    def licenses(self, path):
        path.mkdir()
        (path/'rust-dependencies.json').write_text('[]')
        (path/'NOTICE.txt').write_text('synthetic notice')

    def setup(self, package, staging, version):
        self.assertEqual((package/'pttstella.exe').read_bytes(), self.executable.read_bytes())
        self.assertFalse(list(package.glob('*.dll')))
        manifest = json.loads((package/'manifest.json').read_text())
        self.assertEqual(manifest['crt_linkage'], 'static')
        self.assertEqual(manifest['embedded_notices_sha256'],
                         hashlib.sha256((staging/'third-party-notices.txt').read_bytes()).hexdigest())
        path = staging/(self.stem+'-setup.exe')
        path.write_bytes(b'synthetic setup wrapping standalone App')
        return path, {'runtime_installation_acceptance': 'not_run'}

    def test_setup_and_portable_share_one_static_build_with_atomic_checksums(self):
        result = bundle_windows.bundle(self.output, with_installer=True, with_portable=True)
        self.compiler.assert_called_once()
        self.installer.assert_called_once()
        self.exporter.assert_called_once()
        self.assertEqual(Path(result['portable']).read_bytes(), self.executable.read_bytes())
        self.assertEqual(result['portable_details']['crt_linkage'], 'static')
        self.assertEqual(result['portable_details']['gui_acceptance'], 'not_run')
        self.assertEqual(result['bundled_crt'], [])
        self.assertEqual({p.name for p in self.output.iterdir()}, {self.stem+suffix for suffix in (
            '.zip', '.zip.sha256', '-setup.exe', '-setup.exe.sha256', '-portable.exe', '-portable.exe.sha256')})
        for path in self.output.glob('*.sha256'):
            payload = self.output/path.name.removesuffix('.sha256')
            self.assertEqual(path.read_text(), hashlib.sha256(payload.read_bytes()).hexdigest()+'  '+payload.name+'\n')

    def test_dll_dependency_or_license_export_failure_delivers_nothing(self):
        for failure in ('dynamic CRT', 'license export'):
            self.inspector.return_value = {'imports': ['vcruntime140.dll'] if failure == 'dynamic CRT' else []}
            self.exporter.side_effect = RuntimeError('license export failed') if failure == 'license export' else None
            with self.subTest(failure=failure), self.assertRaises(RuntimeError):
                bundle_windows.bundle(self.output, with_installer=True, with_portable=True)
            self.assertEqual(list(self.output.iterdir()), [])
        self.installer.assert_not_called()

    def test_portable_publication_race_rolls_back_only_this_attempt(self):
        def race(*args):
            (self.output/(self.stem+'-portable.exe')).write_bytes(b'retained other delivery')
        self.exporter.side_effect = race
        with self.assertRaises(FileExistsError):
            bundle_windows.bundle(self.output, with_installer=True, with_portable=True)
        self.assertEqual({p.name for p in self.output.iterdir()}, {self.stem+'-portable.exe'})
        self.assertEqual((self.output/(self.stem+'-portable.exe')).read_bytes(), b'retained other delivery')

    def test_installer_failure_after_portable_validation_still_delivers_nothing(self):
        self.installer.side_effect = RuntimeError('synthetic installer failure')
        with self.assertRaisesRegex(RuntimeError, 'installer failure'):
            bundle_windows.bundle(self.output, with_installer=True, with_portable=True)
        self.exporter.assert_called_once()
        self.assertEqual(list(self.output.iterdir()), [])


if __name__ == '__main__':
    unittest.main()

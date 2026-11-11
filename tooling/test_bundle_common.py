"""Cargo output parsing cannot deliver a library or an incomplete executable."""
import io
import json
import os
from pathlib import Path
import tempfile
import unittest
from unittest.mock import MagicMock, patch

import bundle_common as bundle


class CargoBuildTests(unittest.TestCase):
    def test_uses_cargo_reported_binary_and_optional_target(self):
        with tempfile.TemporaryDirectory(dir=os.environ.get('RUST_GUARD_ISOLATED_ROOT')) as temporary:
            binary = Path(temporary)/'pttstella'
            binary.write_bytes(b'synthetic; never executed')
            messages = [
                {'reason': 'compiler-artifact', 'target': {'name': 'pttstella', 'kind': ['lib']}, 'executable': 'wrong'},
                {'reason': 'compiler-artifact', 'target': {'name': 'pttstella', 'kind': ['bin']}, 'executable': str(binary)},
                {'reason': 'build-finished', 'success': True},
            ]
            child = MagicMock()
            child.stdout = io.StringIO('\n'.join(json.dumps(m) for m in messages))
            child.wait.return_value = 0
            process = MagicMock()
            process.__enter__.return_value = child
            with patch.object(bundle.subprocess, 'Popen', return_value=process) as popen:
                self.assertEqual(bundle.build('synthetic-target'), binary)
            self.assertEqual(popen.call_args.args[0][-2:], ['--target', 'synthetic-target'])

    def test_failed_or_empty_cargo_output_cannot_produce_a_package(self):
        for code in (0, 1):
            child = MagicMock()
            child.stdout = io.StringIO('')
            child.wait.return_value = code
            process = MagicMock()
            process.__enter__.return_value = child
            with patch.object(bundle.subprocess, 'Popen', return_value=process), self.assertRaises(RuntimeError):
                bundle.build()


class LicenseTests(unittest.TestCase):
    def test_local_dependency_keeps_licenses_and_patch_attribution(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            fonts = root/'assets/fonts'
            fonts.mkdir(parents=True)
            for name in ('NOTICE.txt', 'Cascadia-OFL.txt', 'NotoSansCJK-OFL.txt'):
                (fonts/name).write_text('synthetic font notice')
            icons = root/'assets/icons/lucide'
            icons.mkdir(parents=True)
            (icons/'LICENSE.txt').write_text('synthetic icon notice')
            local = root/'vendor/renderer'
            (local/'LICENSES').mkdir(parents=True)
            (local/'LICENSES/upstream.txt').write_text('synthetic upstream license')
            for name in ('PTTSTELLA-UPSTREAM.json', 'PTTSTELLA-PATCH.md'):
                (local/name).write_text('synthetic patch attribution')
            (local/'implementation.rs').write_text('not a notice')
            registry = root/'registry/dependency'
            registry.mkdir(parents=True)
            (registry/'LICENSE').write_text('synthetic registry license')
            packages = [
                {'name': 'pttstella', 'version': '1.0.0', 'source': None,
                 'manifest_path': str(root/'Cargo.toml')},
                {'name': 'renderer', 'version': '1.2.3', 'source': None,
                 'manifest_path': str(local/'Cargo.toml'), 'license': 'MIT'},
                {'name': 'dependency', 'version': '2.0.0', 'source': 'registry+synthetic',
                 'manifest_path': str(registry/'Cargo.toml'), 'license': 'Apache-2.0'},
            ]
            result = MagicMock(stdout=json.dumps({'packages': packages}))
            destination = root/'notices'
            with patch.object(bundle, 'ROOT', root), patch.object(bundle.subprocess, 'run', return_value=result):
                bundle.copy_licenses(destination, 'synthetic-target', feature='native-player-probe')
            inventory = json.loads((destination/'rust-dependencies.json').read_text())
            self.assertEqual([p['name'] for p in inventory], ['renderer', 'dependency'])
            self.assertEqual(inventory[0]['local_path'], 'vendor/renderer')
            self.assertIsNone(inventory[0]['source'])
            self.assertNotIn(str(root), json.dumps(inventory))
            self.assertEqual((destination/'Lucide-Feather.txt').read_text(),
                             'synthetic icon notice')
            self.assertEqual((destination/'renderer-1.2.3/LICENSES/upstream.txt').read_text(),
                             'synthetic upstream license')
            self.assertEqual((destination/'dependency-2.0.0/LICENSE').read_text(),
                             'synthetic registry license')
            for name in ('PTTSTELLA-UPSTREAM.json', 'PTTSTELLA-PATCH.md'):
                self.assertEqual((destination/'renderer-1.2.3'/name).read_text(),
                                 'synthetic patch attribution')
            self.assertFalse((destination/'renderer-1.2.3/implementation.rs').exists())


if __name__ == '__main__':
    unittest.main()

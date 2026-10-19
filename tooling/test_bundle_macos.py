"""Bundle publication failures must preserve the retained destination."""
import os
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch

import bundle_macos


class BundlePublicationTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory(dir=os.environ.get('RUST_GUARD_ISOLATED_ROOT'))
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)

    def test_icon_conversion_failure_cannot_publish_or_sign_an_incomplete_app(self):
        output = self.root / 'delivery'
        executable = self.root / 'pttstella'
        executable.write_bytes(b'synthetic executable; never launched')

        def failed_converter(*command):
            if command[0] == 'iconutil':
                raise subprocess.CalledProcessError(1, command)
            return ''

        with patch.object(bundle_macos.sys, 'platform', 'darwin'), \
                patch.object(bundle_macos.shutil, 'which', return_value='/synthetic/tool'), \
                patch.object(bundle_macos, 'build', return_value=executable), \
                patch.object(bundle_macos, 'check_dependencies'), \
                patch.object(bundle_macos, 'run', side_effect=failed_converter) as runner:
            with self.assertRaises(subprocess.CalledProcessError):
                bundle_macos.bundle(output)
        self.assertEqual(list(output.iterdir()), [])
        self.assertFalse(any(call.args[0] == 'codesign' for call in runner.call_args_list))

    def test_existing_delivery_is_preserved_before_any_build_or_conversion(self):
        output = self.root / 'delivery'
        app = output / 'PTTStella.app'
        app.mkdir(parents=True)
        marker = app / 'retained'
        marker.write_bytes(b'previous delivery')
        with patch.object(bundle_macos.sys, 'platform', 'darwin'), \
                patch.object(bundle_macos.shutil, 'which', return_value='/synthetic/tool'), \
                patch.object(bundle_macos, 'run'), \
                patch.object(bundle_macos, 'build') as build:
            with self.assertRaisesRegex(RuntimeError, 'Output already exists'):
                bundle_macos.bundle(output)
            build.assert_not_called()
        self.assertEqual(marker.read_bytes(), b'previous delivery')


if __name__ == '__main__':
    unittest.main()

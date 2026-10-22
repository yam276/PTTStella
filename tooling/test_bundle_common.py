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


if __name__ == '__main__':
    unittest.main()

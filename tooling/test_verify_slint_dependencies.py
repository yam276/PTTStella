import json
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest import mock

import verify_slint_dependencies as gate


TREE = '\n'.join(name + ' v1.0.0' for name in sorted(gate.REQUIRED | {'libmpv2-sys'})) + '\n'


class SlintDependencyTests(unittest.TestCase):
    def test_webview_and_dioxus_dependencies_fail_even_with_native_backend_present(self):
        for name in ('dioxus', 'dioxus-desktop', 'wry', 'tao', 'webview2-com-sys',
                     'webkit2gtk-sys', 'javascriptcore-rs-sys'):
            with self.subTest(package=name), self.assertRaisesRegex(ValueError, 'forbidden'):
                gate.inspect_tree(TREE + name + ' v1.0.0\n', 'native-player-probe')

    def test_empty_or_wrong_backend_cannot_pass(self):
        for value in ('', 'pttstella v1.0.0\n', TREE.replace('i-slint-renderer-skia v1.0.0\n', '')):
            with self.subTest(tree=value), self.assertRaisesRegex(ValueError, 'missing'):
                gate.inspect_tree(value, 'slint-research')
        with self.assertRaisesRegex(ValueError, 'libmpv2-sys'):
            gate.inspect_tree(TREE.replace('libmpv2-sys v1.0.0\n', ''), 'native-player-probe')

    def test_unrecognized_or_partial_output_is_not_a_valid_tree(self):
        with self.assertRaisesRegex(ValueError, 'Unexpected Cargo'):
            gate.inspect_tree(TREE + 'error: unresolved graph\n', 'slint-research')

    def test_prior_evidence_is_not_replaced(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            sentinel = root / 'report.json'
            sentinel.write_bytes(b'previous evidence')
            with self.assertRaises(FileExistsError):
                gate.verify(root)
            self.assertEqual(sentinel.read_bytes(), b'previous evidence')

    def test_successful_partial_tree_keeps_failed_report(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary) / 'new'
            partial = subprocess.CompletedProcess([], 0, stdout='pttstella v1.0.0\n', stderr='')
            with mock.patch.object(gate.subprocess, 'check_output', return_value='synthetic-ref\n'), \
                    mock.patch.object(gate.subprocess, 'run', return_value=partial):
                with self.assertRaisesRegex(ValueError, 'missing'):
                    gate.verify(root)
            report = json.loads((root / 'report.json').read_text())
            self.assertEqual(report['status'], 'failed')
            self.assertEqual(len(report['cases']), 1)
            self.assertEqual(next(iter(report['cases'].values()))['status'], 'failed')


if __name__ == '__main__':
    unittest.main()

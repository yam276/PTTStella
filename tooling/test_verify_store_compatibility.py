import copy
import json
from pathlib import Path
import tempfile
import unittest
from unittest import mock

import verify_store_compatibility as compatibility


class CompatibilityRunnerTests(unittest.TestCase):
    def runner(self, root):
        runner = compatibility.Runner.__new__(compatibility.Runner)
        runner.output = root
        runner.drivers = {'candidate': None}
        runner.jobs = []
        runner.report = {'status': 'running', 'cases': {}, 'calls': []}
        return runner

    def test_existing_output_cannot_overwrite_prior_evidence(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            sentinel = root / 'original.txt'
            sentinel.write_bytes(b'prior evidence')
            with self.assertRaises(FileExistsError):
                compatibility.Runner({}, root)
            self.assertEqual(sentinel.read_bytes(), b'prior evidence')
            self.assertEqual(list(root.iterdir()), [sentinel])

    def test_successful_process_with_lost_color_or_trailing_lines_is_rejected(self):
        with tempfile.TemporaryDirectory() as temporary:
            runner = self.runner(Path(temporary))
            root, expected = runner.seed('fixture')
            wrong = copy.deepcopy(expected)
            wrong['bookmarks']['entries'][0]['archive']['rows'][1][0][4] = 1
            wrong['drafts']['entries'][1]['body'] = wrong['drafts']['entries'][1]['body'].rstrip()
            with mock.patch.object(runner, 'invoke', return_value=wrong):
                with self.assertRaisesRegex(AssertionError, 'Independent expectation mismatch'):
                    runner.inspect('candidate', root, expected, 'semantic')
            failure = json.loads((runner.output / 'failure-semantic.json').read_text())
            self.assertEqual(failure['expected'], expected)
            self.assertNotEqual(failure['actual'], expected)

    def test_rejected_corrupt_read_still_fails_if_a_driver_rewrites_the_file(self):
        with tempfile.TemporaryDirectory() as temporary:
            runner = self.runner(Path(temporary))

            def rewrite(_actor, _command, root, **_kwargs):
                (root / 'settings.json').write_bytes(b'{"version":6}')

            with mock.patch.object(runner, 'invoke', side_effect=rewrite):
                with self.assertRaisesRegex(AssertionError, 'Independent expectation mismatch'):
                    runner.invalid_reads()
            self.assertEqual(runner.report['cases'], {})

    def test_failed_run_joins_only_its_still_running_children(self):
        with tempfile.TemporaryDirectory() as temporary:
            runner = self.runner(Path(temporary))
            running, finished = mock.Mock(), mock.Mock()
            running.poll.side_effect = [None, -9]
            finished.poll.return_value = 0
            runner.jobs = [running, finished]
            with mock.patch.object(runner, 'roundtrips', side_effect=RuntimeError('fixture failure')):
                with self.assertRaisesRegex(RuntimeError, 'fixture failure'):
                    runner.run()
            running.kill.assert_called_once_with()
            running.wait.assert_called_once_with(timeout=15)
            finished.kill.assert_not_called()
            finished.wait.assert_not_called()
            self.assertTrue(runner.report['owned_children_joined'])
            self.assertEqual(runner.report['status'], 'failed')

    def test_empty_or_skipped_work_cannot_be_reported_as_passed(self):
        with tempfile.TemporaryDirectory() as temporary:
            runner = self.runner(Path(temporary))
            with mock.patch.object(runner, 'roundtrips'), mock.patch.object(runner, 'invalid_reads'), \
                    mock.patch.object(runner, 'capability_boundary'), mock.patch.object(runner, 'concurrent'):
                with self.assertRaisesRegex(AssertionError, 'Missing required compatibility cases'):
                    runner.run()
            self.assertEqual(runner.report['status'], 'failed')


if __name__ == '__main__':
    unittest.main()

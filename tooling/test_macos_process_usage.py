import os
import json
from pathlib import Path
import sys
import tempfile
import time
import unittest
from unittest import mock

from macos_process_usage import (
    Identity, MacProcesses, NotReady, OwnedProcess, collect_steady_phase, main,
    select_family, summarize_steady_phase,
)


class FamilyAttributionTests(unittest.TestCase):
    def test_group_and_all_process_pid_values_cannot_be_owned_identities(self):
        for pid, start in [(0, 1), (-1, 1), (2 ** 31, 1), (10, 0), (10, -1), (True, 1)]:
            with self.assertRaises(ValueError):
                Identity(pid, start)

    def test_launchd_xpc_belongs_to_responsible_app_but_same_named_decoys_do_not(self):
        rows = {
            10: {'ppid': 900, 'responsible': 10, 'name': 'app'},
            11: {'ppid': 1, 'responsible': 10, 'name': 'WebKit'},
            12: {'ppid': 11, 'responsible': 12, 'name': 'helper'},
            20: {'ppid': 1, 'responsible': 20, 'name': 'WebKit'},
            21: {'ppid': 900, 'responsible': 900, 'name': 'sampler'},
        }
        self.assertEqual(select_family(rows, 10), {10, 11, 12})

    def test_shared_desktop_responsibility_is_rejected_instead_of_charging_other_chats(self):
        with self.assertRaisesRegex(NotReady, 'isolated responsibility'):
            select_family({10: {'ppid': 900, 'responsible': 900}}, 10)
        with self.assertRaises(NotReady):
            select_family({}, 10)

    def test_unrelated_parent_cycle_never_becomes_a_family(self):
        self.assertEqual(select_family({
            10: {'ppid': 1, 'responsible': 10},
            20: {'ppid': 21, 'responsible': 20},
            21: {'ppid': 20, 'responsible': 21},
        }, 10), {10})

    def test_reused_root_is_rejected_before_any_inventory_or_foreign_sampling(self):
        api = object.__new__(MacProcesses)
        api.usage = lambda _pid: {'start': 200, 'exit': 0}
        api.inventory = mock.Mock(side_effect=AssertionError('must not scan a reused root'))
        with self.assertRaisesRegex(NotReady, 'PID was reused'):
            api.sample(Identity(10, 100))
        api.inventory.assert_not_called()

    def test_cleanup_never_signals_a_pid_with_a_different_identity(self):
        api = mock.Mock()
        api.usage.return_value = {'start': 200}
        process = OwnedProcess(api, Identity(10, 100))
        with mock.patch('macos_process_usage.os.waitpid', return_value=(0, 0)), \
                mock.patch('macos_process_usage.os.kill') as kill:
            with self.assertRaisesRegex(NotReady, 'reused benchmark PID'):
                process.terminate()
            kill.assert_not_called()


class PhaseAccountingTests(unittest.TestCase):
    @staticmethod
    def sample(at, cpu, start=100):
        return {'monotonic_ns': at, 'footprint_bytes': 2000 + cpu, 'rss_bytes': 4000,
                'members': [{'pid': 10, 'start': start, 'user_ns': cpu, 'system_ns': 0}]}

    def test_known_work_and_peak_are_computed_from_a_stable_phase(self):
        result = summarize_steady_phase([self.sample(1000, 100), self.sample(2000, 600),
                                        self.sample(3000, 1100)])
        self.assertEqual(result['wall_ns'], 2000)
        self.assertEqual(result['cpu_ns'], 1000)
        self.assertEqual(result['mean_cpu_cores'], .5)
        self.assertEqual(result['sampled_peak_footprint_bytes'], 3100)

    def test_exits_new_members_and_pid_reuse_cannot_be_treated_as_cpu_improvement(self):
        for last in [dict(self.sample(2000, 200), members=[]), self.sample(2000, 200, start=101)]:
            with self.assertRaises(NotReady):
                summarize_steady_phase([self.sample(1000, 100), last])

    def test_empty_trace_clock_regression_and_counter_reset_are_not_ready(self):
        for samples in [[], [self.sample(1000, 100)],
                        [self.sample(1000, 100), self.sample(999, 200)],
                        [self.sample(1000, 100), self.sample(2000, 99)]]:
            with self.assertRaises(NotReady):
                summarize_steady_phase(samples)

    def test_sampler_observes_the_whole_requested_interval_and_keeps_raw_samples(self):
        now = [0.]
        api = mock.Mock()
        api.sample.side_effect = lambda _root: self.sample(int(now[0] * 1e9), int(now[0] * 2e8))
        samples = []
        result = collect_steady_phase(api, Identity(10, 100), .25, .1, samples.append,
                                      clock=lambda: now[0], sleep=lambda value: now.__setitem__(0, now[0] + value))
        self.assertEqual(result['wall_ns'], 250_000_000)
        self.assertEqual(result['cpu_ns'], 50_000_000)
        self.assertEqual(len(samples), 4)

    def test_invalid_or_unbounded_sampling_never_observes_a_process(self):
        api = mock.Mock()
        for duration, interval in [(0, .1), (61, .1), (.1, 0), (.1, .2), (10, .001),
                                   (float('nan'), .1), (1, float('inf'))]:
            with self.assertRaises(ValueError):
                collect_steady_phase(api, Identity(10, 100), duration, interval, mock.Mock())
        api.sample.assert_not_called()

    def test_cli_retains_partial_trace_and_not_ready_when_root_disappears(self):
        with tempfile.TemporaryDirectory() as temporary:
            output = Path(temporary) / 'phase'
            api = mock.Mock()
            api.sample.side_effect = [self.sample(1000, 100), NotReady('owned member exited')]
            with mock.patch('macos_process_usage.MacProcesses', return_value=api):
                self.assertEqual(main(['--pid', '10', '--start', '100', '--duration', '.1',
                                       '--interval', '.1', '--output', str(output)]), 2)
            result = json.loads((output / 'phase.json').read_text())
            self.assertEqual(result['status'], 'not_ready')
            self.assertNotIn('phase', result)
            self.assertEqual(len((output / 'samples.jsonl').read_text().splitlines()), 1)
            self.assertEqual((output / 'samples.jsonl').stat().st_mode & 0o777, 0o600)
            with self.assertRaises(FileExistsError):
                main(['--pid', '10', '--start', '100', '--output', str(output)])


@unittest.skipUnless(sys.platform == 'darwin', 'native macOS diagnostic APIs required')
class NativeAccountingTests(unittest.TestCase):
    def test_real_cli_collects_owned_phase_without_stopping_the_target(self):
        api = MacProcesses()
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            process = api.spawn(['/bin/sleep', '1'], dict(os.environ), directory / 'sleep.log', cwd=directory)
            output = directory / 'phase'
            try:
                code = main(['--pid', str(process.pid), '--start', str(process.identity.start),
                             '--duration', '.2', '--interval', '.1', '--output', str(output)])
                report = json.loads((output / 'phase.json').read_text())
                self.assertEqual(code, 0, report)
                self.assertEqual(report['status'], 'passed')
                self.assertEqual(report['phase']['samples'], 3)
                self.assertGreaterEqual(report['phase']['wall_ns'], 180_000_000)
                self.assertGreater(report['phase']['sampled_peak_footprint_bytes'], 0)
                samples = [json.loads(line) for line in (output / 'samples.jsonl').read_text().splitlines()]
                self.assertEqual({p['pid'] for s in samples for p in s['members']}, {process.pid})
                self.assertIsNone(process.poll())
                self.assertEqual(process.wait(3), 0)
            finally:
                process.terminate()
                process.wait(3)

    def test_real_disclaimed_root_forked_child_and_decoy_are_attributed_and_reaped(self):
        api = MacProcesses()
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            child_file = directory / 'child.txt'
            script = (
                'import os,time,pathlib; pid=os.fork(); '
                'time.sleep(1.5) if pid==0 else pathlib.Path(' + repr(str(child_file)) + ').write_text(str(pid)); '
                'os._exit(0) if pid==0 else os.waitpid(pid,0)'
            )
            parent = api.spawn([sys.executable, '-c', script], dict(os.environ), directory / 'parent.log', cwd=directory)
            decoy = api.spawn(['/bin/sleep', '1.5'], dict(os.environ), directory / 'decoy.log', cwd=directory)
            try:
                deadline = time.monotonic() + 1
                while not child_file.exists() and time.monotonic() < deadline:
                    time.sleep(.01)
                self.assertTrue(child_file.exists(), (directory / 'parent.log').read_text())
                child = int(child_file.read_text())
                sample = api.sample(parent.identity)
                self.assertEqual({p['pid'] for p in sample['members']}, {parent.pid, child})
                self.assertNotIn(decoy.pid, {p['pid'] for p in sample['members']})
                self.assertGreater(sample['footprint_bytes'], 0)
                self.assertGreater(sample['rss_bytes'], 0)
                self.assertEqual(parent.wait(5), 0)
                self.assertEqual(decoy.wait(5), 0)
            finally:
                for process in (parent, decoy):
                    process.terminate()
                    process.wait(5)

    def test_cpu_counter_units_match_process_cpu_clock_and_identity_is_nonzero(self):
        api = MacProcesses()
        first = api.usage(os.getpid())
        started = time.process_time_ns()
        while time.process_time_ns() - started < 80_000_000:
            pass
        elapsed = time.process_time_ns() - started
        last = api.usage(os.getpid())
        cpu = last['user_ns'] + last['system_ns'] - first['user_ns'] - first['system_ns']
        self.assertLess(abs(cpu - elapsed), 20_000_000)
        self.assertEqual(first['start'], last['start'])
        self.assertGreater(first['start'], 0)

    def test_existing_log_and_invalid_spawn_inputs_never_launch_a_child(self):
        api = MacProcesses()
        with tempfile.TemporaryDirectory() as temporary:
            log = Path(temporary) / 'output.log'
            log.write_text('retain me')
            with self.assertRaises(FileExistsError):
                api.spawn(['/bin/false'], {}, log)
            self.assertEqual(log.read_text(), 'retain me')
            for command, environment in [(['relative'], {}), (['/bin/false'], {'x=y': 'z'}),
                                          (['/bin/false\0'], {}), (['/bin/false'], {'': 'z'})]:
                with self.assertRaises(ValueError):
                    api.spawn(command, environment, Path(temporary) / 'new.log')
                self.assertFalse((Path(temporary) / 'new.log').exists())


if __name__ == '__main__':
    unittest.main()

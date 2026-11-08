"""Verify native draft persistence with own loopback child processes.

Requires a built pttstella-slint executable and a NEW output directory.
Only Python's standard library is needed. Widget gestures are synthetic;
this does not verify OS input/IME or arbitrary power-loss boundaries.
The ANSI text and synthetic StellaTest scope below are self-authored CC0.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import subprocess
import time

BODY = '第一行中文\n\x1b[1;31m亮紅中文\x1b[m\n第三行'
MAX_LOG = 2 * 1024 * 1024
MAX_MARKER = 64 * 1024


def digest(path):
    value = hashlib.sha256()
    with path.open('rb') as source:
        for chunk in iter(lambda: source.read(1024 * 1024), b''):
            value.update(chunk)
    return value.hexdigest()


def kill_marked_child(process, marker):
    """A fresh marker must identify this still-owned direct child exactly."""
    if process.poll() is not None or marker.get('pid') != process.pid:
        raise ValueError('Crash marker does not identify the running child')
    process.kill()  # SIGKILL on POSIX; TerminateProcess on Windows.
    return process.wait(timeout=15)


class Runner:
    def __init__(self, executable, output):
        self.executable = executable.resolve(strict=True)
        if not self.executable.is_file():
            raise ValueError('Expected a built native executable')
        self.output = output.resolve()
        self.output.mkdir(parents=True, exist_ok=False)
        self.jobs = []
        self.report = {
            'status': 'running', 'platform': platform.platform(),
            'binary_sha256': digest(self.executable),
            'scope': 'Native main-window owners with synthetic widgets and loopback peers; '
                     'no OS input, IME, arbitrary mid-write crash or power-loss claim',
            'fixture_license': 'CC0-1.0', 'cases': {},
        }
        self.record()

    def record(self):
        (self.output / 'report.json').write_text(
            json.dumps(self.report, ensure_ascii=False, indent=2) + '\n', encoding='utf-8')

    def start(self, case, profile):
        out = self.output / case
        if out.exists():
            raise ValueError('A previous case output is preserved')
        profile.mkdir(exist_ok=True)
        env = {k: v for k, v in os.environ.items() if not k.startswith('PTTSTELLA_')}
        selected = {'PTTSTELLA_DRAFTS_STORAGE_CASE': case,
                    'PTTSTELLA_SLINT_DRAFTS_PROBE_DIR': str(profile)}
        env.update(selected)
        command = [str(self.executable), '--probe-drafts', '--verify-drafts', str(out)]
        log_path = self.output / (case + '.log')
        log = log_path.open('wb')
        try:
            process = subprocess.Popen(command, env=env, stdout=log, stderr=subprocess.STDOUT)
        except BaseException:
            log.close()
            raise
        job = (process, log, out, log_path, command, selected)
        self.jobs.append(job)
        return job

    @staticmethod
    def check_log(job):
        if job[3].stat().st_size > MAX_LOG:
            raise ValueError('Fixture diagnostic exceeded its 2 MiB limit')

    def marker(self, job, name):
        limit = time.monotonic() + 40
        while time.monotonic() < limit:
            self.check_log(job)
            path = job[2] / name
            if path.is_symlink():
                raise ValueError('A fixture marker cannot be a symlink')
            if path.exists():
                with path.open('rb') as source:
                    data = source.read(MAX_MARKER + 1)
                if len(data) > MAX_MARKER:
                    raise ValueError('Fixture marker exceeded its size limit')
                try:
                    return json.loads(data)
                except json.JSONDecodeError:
                    pass  # The owned child may still be writing this marker.
            if job[0].poll() is not None:
                raise RuntimeError('Child exited before its fixture marker')
            time.sleep(0.02)
        raise TimeoutError('Child did not reach the expected fixture boundary')

    def finish(self, case, job):
        limit = time.monotonic() + 60
        while job[0].poll() is None and time.monotonic() < limit:
            self.check_log(job)
            time.sleep(0.02)
        self.check_log(job)
        code = job[0].wait(timeout=1)
        job[1].close()
        raw = job[3].read_text(encoding='utf-8', errors='replace')
        if code != 0 or '"close":"acknowledged"' not in raw:
            raise RuntimeError(f'{case} failed or did not acknowledge Close: {raw[-2000:]}')
        data = json.loads((job[2] / 'drafts.json').read_text(encoding='utf-8'))
        if data['status'] != 'passed':
            raise ValueError('Child did not report completed verification')
        self.report['cases'][case] = {
            'command': job[4], 'env': job[5], 'pid': job[0].pid,
            'exit_code': code, 'close': 'acknowledged', 'report': data,
        }
        self.record()
        print(case + ': passed; exact child and strict peer', flush=True)

    @staticmethod
    def seed(profile, ids):
        profile.mkdir(exist_ok=False)
        entries = [dict(id=value, scope=dict(
            site=dict(endpoint='wss://ws.ptt.cc/bbs', encoding='uao'),
            account='StellaTest', board='Stella'), revision=1, title='本機草稿測試',
            body=BODY, saved_at=1, conflict=False) for value in ids]
        (profile / 'probe-drafts.json').write_text(
            json.dumps(dict(version=1, entries=entries), ensure_ascii=False) + '\n',
            encoding='utf-8')

    @staticmethod
    def entries(profile):
        return json.loads((profile / 'probe-drafts.json').read_text(encoding='utf-8'))['entries']

    def cases(self):
        for case in ['backup', 'corrupt']:
            profile = self.output / (case + '-store')
            self.finish(case, self.start(case, profile))
            entries = self.entries(profile)
            if len(entries) != 1 or entries[0]['body'] != BODY + '\n失敗後仍保留\n修復後保存':
                raise ValueError('Repaired durable source differs')
        profile = self.output / 'crash-store'
        job = self.start('crash-stage', profile)
        ready = self.marker(job, 'crash-ready.json')
        before = (profile / 'probe-drafts.json').read_bytes()
        if json.loads(before)['entries'][0]['body'] != BODY:
            raise ValueError('The pre-crash acknowledged source differs')
        code = kill_marked_child(job[0], ready)
        job[1].close()
        if code == 0 or (profile / 'probe-drafts.json').read_bytes() != before:
            raise ValueError('Termination did not preserve the selected save boundary')
        self.report['cases']['crash-stage'] = {
            'command': job[4], 'env': job[5], 'pid': job[0].pid, 'exit_code': code,
            'close': 'not expected after forced termination', 'saved_bytes_unchanged': True,
            'ready': ready, 'termination': 'kill of verified owned direct child',
            'scope': 'After save acknowledgment and before pending debounce flush only',
        }
        self.record()
        self.finish('crash-recover', self.start('crash-recover', profile))
        if self.entries(profile)[0]['body'] != BODY + '\n恢復後修改':
            raise ValueError('Recovered source could not be modified and saved')
        for mode, ids in [('same', ['one']), ('disjoint', ['one', 'two'])]:
            profile = self.output / (mode + '-store')
            self.seed(profile, ids)
            a, b = self.start(mode + '-a', profile), self.start(mode + '-b', profile)
            ready_a, ready_b = self.marker(a, 'ready.json'), self.marker(b, 'ready.json')
            if ready_a['pid'] != a[0].pid or ready_b['pid'] != b[0].pid or a[0].pid == b[0].pid:
                raise ValueError('Concurrent markers do not identify two owned processes')
            (a[2] / 'release').write_text('go\n', encoding='ascii')
            if mode == 'same':
                self.finish(mode + '-a', a)
            (b[2] / 'release').write_text('go\n', encoding='ascii')
            if mode != 'same':
                self.finish(mode + '-a', a)
            self.finish(mode + '-b', b)
            entries = self.entries(profile)
            if len(entries) != 2:
                raise ValueError('Concurrent saves did not retain exactly two entries')
            first = next(e for e in entries if e['id'] == 'one')
            second = next(e for e in entries if e['id'] != 'one')
            if first['body'] != '中文 A\n\x1b[31m紅\x1b[m' or first['conflict']:
                raise ValueError('First concurrent source changed')
            expected = '中文 B\n\x1b[32m綠\x1b[m' + ('\n副本後續修改' if mode == 'same' else '')
            if (second['body'] != expected or second['conflict'] != (mode == 'same')
                    or second['revision'] != 2 or (second['id'] == 'two') != (mode == 'disjoint')):
                raise ValueError('Conflict/merge source, identity or revision differs')
            self.report['cases'][mode + '-durable'] = {
                'status': 'passed', 'entry_count': 2, 'independent_processes': True,
                'conflict_count': sum(e['conflict'] for e in entries),
                'body_a': first['body'], 'body_b': second['body'],
            }
            self.record()

    def close(self):
        errors = []
        for process, log, *_ in self.jobs:
            try:
                if process.poll() is None:
                    process.kill()
                process.wait(timeout=15)
            except Exception as error:
                errors.append(str(error))
            finally:
                log.close()
        if errors:
            raise RuntimeError('Owned child cleanup failed: ' + '; '.join(errors))


def verify(executable, output):
    runner = Runner(executable, output)
    failure = None
    try:
        runner.cases()
    except BaseException as error:
        failure = error
    try:
        runner.close()
    except Exception as error:
        failure = RuntimeError(f'{failure}; cleanup: {error}' if failure else str(error))
    if failure is not None:
        runner.report.update(status='failed', error=str(failure))
        runner.record()
        raise failure
    runner.report['status'] = 'passed'
    runner.record()
    return runner.report


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--executable', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True, help='New isolated evidence directory')
    arguments = parser.parse_args()
    verify(arguments.executable, arguments.output)
    print(json.dumps({'status': 'passed', 'report': str(arguments.output / 'report.json')}))

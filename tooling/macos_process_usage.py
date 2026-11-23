"""Read-only macOS process-family accounting for owned desktop benchmarks.

The diagnostic launcher isolates responsibility using the same spawn SPI call
used in Chromium's base/process/launch_mac.cc. It is not a product dependency.
Missing SPI, a shared responsible root, or unreadable owned members is not_ready.
No process names, launch order, or system-wide memory deltas establish ownership.

ABI sources: the installed SDK's libproc.h, sys/proc_info.h, sys/resource.h;
https://github.com/chromium/chromium/blob/main/base/process/launch_mac.cc
https://github.com/apple-oss-distributions/xnu/blob/main/doc/observability/recount.md
"""
import ctypes as C
from dataclasses import dataclass
import errno
import argparse
import json
import math
import os
from pathlib import Path
import signal
import sys
import time


class NotReady(RuntimeError):
    pass


class _BSD(C.Structure):
    _fields_ = [(name, C.c_uint32) for name in ('pid', 'ppid', 'pgid', 'status')]
    _fields_ += [('comm', C.c_char * 16)]
    _fields_ += [(name, C.c_uint32) for name in
                 ('flags', 'uid', 'gid', 'ruid', 'rgid', 'svuid', 'svgid', 'reserved')]


class _Usage(C.Structure):
    _fields_ = [('uuid', C.c_uint8 * 16)]
    _fields_ += [(name, C.c_uint64) for name in
                 ('user_ticks', 'system_ticks', 'idle_wakes', 'interrupt_wakes', 'pageins',
                  'wired_bytes', 'rss_bytes', 'footprint_bytes', 'start', 'exit')]


class _Timebase(C.Structure):
    _fields_ = [('numer', C.c_uint32), ('denom', C.c_uint32)]


@dataclass(frozen=True)
class Identity:
    pid: int
    start: int

    def __post_init__(self):
        if (type(self.pid) is not int or not 0 < self.pid < 2 ** 31
                or type(self.start) is not int or not 0 < self.start < 2 ** 64):
            raise ValueError('process identity requires a positive PID and start counter')


def select_family(rows, root):
    """Responsibility plus the transitive PPID closure, never the caller's root."""
    if root not in rows or rows[root]['responsible'] != root:
        raise NotReady('benchmark root does not own an isolated responsibility scope')
    family = {root}
    family.update(pid for pid, row in rows.items() if row['responsible'] == root)
    while True:
        added = {pid for pid, row in rows.items() if row['ppid'] in family} - family
        if not added:
            return family
        family.update(added)


def summarize_steady_phase(samples):
    """A sampled stable phase, not CPU accounting for short-lived helpers.

    Refuse membership changes and counter resets rather than silently dropping
    exited CPU or clamping a negative delta to zero. Keep the original trace.
    """
    if len(samples) < 2:
        raise NotReady('a steady phase needs at least two samples')
    first = samples[0]
    previous = None
    identities = None
    for sample in samples:
        current = {(p['pid'], p['start']): p['user_ns'] + p['system_ns']
                   for p in sample['members']}
        if not current or len(current) != len(sample['members']):
            raise NotReady('empty or duplicate process identities')
        if identities is None:
            identities = set(current)
        if set(current) != identities:
            raise NotReady('process membership changed; stable CPU delta is unavailable')
        if previous is not None:
            if sample['monotonic_ns'] <= previous[0]:
                raise NotReady('sample clock did not advance')
            if any(current[key] < previous[1][key] for key in identities):
                raise NotReady('a process CPU counter reset')
        previous = sample['monotonic_ns'], current
    duration = samples[-1]['monotonic_ns'] - first['monotonic_ns']
    initial = sum(p['user_ns'] + p['system_ns'] for p in first['members'])
    cpu = sum(previous[1].values()) - initial
    return {'wall_ns': duration, 'cpu_ns': cpu, 'mean_cpu_cores': cpu / duration,
            'sampled_peak_footprint_bytes': max(s['footprint_bytes'] for s in samples),
            'sampled_peak_rss_bytes': max(s['rss_bytes'] for s in samples),
            'samples': len(samples),
            'scope': 'stable observed live family only; no unsampled exited-helper CPU or display-latency claim'}


def validate_schedule(duration, interval):
    if (not math.isfinite(duration) or not math.isfinite(interval)
            or not .1 <= duration <= 60 or not .05 <= interval <= 1
            or duration < interval or math.ceil(duration / interval) > 1200):
        raise ValueError('duration must be .1–60s, interval .05–1s, with at most 1200 intervals')


def collect_steady_phase(api, root, duration, interval, record, *, clock=time.monotonic,
                         sleep=time.sleep):
    """Bounded read-only sampling; an incomplete interval never becomes passed."""
    validate_schedule(duration, interval)
    samples = []
    began = clock()
    deadline = began + duration
    # An inclusive last observation measures the complete requested interval.
    for index in range(math.ceil(duration / interval) + 1):
        due = min(began + index * interval, deadline)
        remaining = due - clock()
        if remaining > 0:
            sleep(remaining)
        sample = api.sample(root)
        record(sample)
        samples.append(sample)
    return summarize_steady_phase(samples)


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--pid', type=int, required=True)
    parser.add_argument('--start', type=int, required=True,
                        help='opaque start counter from the diagnostic launcher, not a wall-clock timestamp')
    parser.add_argument('--duration', type=float, default=10)
    parser.add_argument('--interval', type=float, default=.1)
    parser.add_argument('--output', type=Path, required=True, help='new evidence directory')
    args = parser.parse_args(argv)
    try:
        root = Identity(args.pid, args.start)
        validate_schedule(args.duration, args.interval)
    except ValueError as error:
        parser.error(str(error))
    args.output.mkdir(mode=0o700, exist_ok=False)
    # This observer never launches, activates or signals the supplied process.
    report = {'status': 'not_ready', 'identity': {'pid': root.pid, 'start': root.start},
              'duration_seconds': args.duration, 'interval_seconds': args.interval,
              'scope': 'observed stable process family only; no workload, foreground, startup or display-latency validation'}
    code = 2
    try:
        descriptor = os.open(args.output / 'samples.jsonl', os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
        with os.fdopen(descriptor, 'w') as trace:
            def record(sample):
                trace.write(json.dumps(sample, allow_nan=False) + '\n')
                trace.flush()
            report['phase'] = collect_steady_phase(MacProcesses(), root, args.duration, args.interval, record)
        report['status'] = 'passed'
        code = 0
    except (NotReady, OSError, ValueError) as error:
        report['reason'] = str(error)
    finally:
        descriptor = os.open(args.output / 'phase.json', os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
        with os.fdopen(descriptor, 'w') as result:
            json.dump(report, result, ensure_ascii=False, indent=2, allow_nan=False)
            result.write('\n')
    return code


class MacProcesses:
    def __init__(self):
        if sys.platform != 'darwin':
            raise NotReady('macOS process accounting requires a macOS runner')
        if C.sizeof(_BSD) != 64 or C.sizeof(_Usage) != 96:
            raise NotReady('unexpected macOS SDK ABI layout')
        self.lib = C.CDLL('/usr/lib/libSystem.B.dylib', use_errno=True)
        signatures = {
            'proc_listallpids': ([C.c_void_p, C.c_int], C.c_int),
            'proc_pidinfo': ([C.c_int, C.c_int, C.c_uint64, C.c_void_p, C.c_int], C.c_int),
            'proc_pid_rusage': ([C.c_int, C.c_int, C.c_void_p], C.c_int),
            'proc_pidpath': ([C.c_int, C.c_void_p, C.c_uint32], C.c_int),
            'mach_timebase_info': ([C.POINTER(_Timebase)], C.c_int),
            'responsibility_get_pid_responsible_for_pid': ([C.c_int], C.c_int),
            'responsibility_spawnattrs_setdisclaim': ([C.POINTER(C.c_void_p), C.c_int], C.c_int),
            'posix_spawnattr_init': ([C.POINTER(C.c_void_p)], C.c_int),
            'posix_spawnattr_destroy': ([C.POINTER(C.c_void_p)], C.c_int),
            'posix_spawn_file_actions_init': ([C.POINTER(C.c_void_p)], C.c_int),
            'posix_spawn_file_actions_destroy': ([C.POINTER(C.c_void_p)], C.c_int),
            'posix_spawn_file_actions_addopen':
                ([C.POINTER(C.c_void_p), C.c_int, C.c_char_p, C.c_int, C.c_uint16], C.c_int),
            'posix_spawn_file_actions_adddup2':
                ([C.POINTER(C.c_void_p), C.c_int, C.c_int], C.c_int),
            'posix_spawn_file_actions_addchdir_np':
                ([C.POINTER(C.c_void_p), C.c_char_p], C.c_int),
            'posix_spawn': ([C.POINTER(C.c_int), C.c_char_p, C.POINTER(C.c_void_p),
                             C.POINTER(C.c_void_p), C.POINTER(C.c_char_p), C.POINTER(C.c_char_p)], C.c_int),
        }
        for name, (args, result) in signatures.items():
            try:
                function = getattr(self.lib, name)
            except AttributeError as error:
                raise NotReady(f'missing macOS diagnostic interface: {name}') from error
            function.argtypes, function.restype = args, result
        base = _Timebase()
        if self.lib.mach_timebase_info(C.byref(base)) or not base.numer or not base.denom:
            raise NotReady('mach timebase unavailable')
        self.timebase = {'numer': int(base.numer), 'denom': int(base.denom)}

    def usage(self, pid):
        value = _Usage()
        if self.lib.proc_pid_rusage(pid, 0, C.byref(value)) != 0:
            code = C.get_errno()
            raise OSError(code, os.strerror(code), pid)
        if not value.start:
            raise NotReady('process identity has no start counter')
        result = {name: int(getattr(value, name)) for name, _ in _Usage._fields_ if name != 'uuid'}
        for kind in ('user', 'system'):
            result[kind + '_ns'] = result[kind + '_ticks'] * self.timebase['numer'] // self.timebase['denom']
        return result

    def bsd(self, pid):
        value = _BSD()
        if self.lib.proc_pidinfo(pid, 13, 0, C.byref(value), C.sizeof(value)) != C.sizeof(value):
            return None
        if value.pid != pid:
            raise NotReady('process-info identity mismatch')
        return {'pid': pid, 'ppid': value.ppid, 'uid': value.uid,
                'name': value.comm.decode('utf-8', errors='replace'),
                'responsible': self.lib.responsibility_get_pid_responsible_for_pid(pid)}

    def inventory(self):
        capacity = self.lib.proc_listallpids(None, 0)
        if not 0 < capacity < 131072:
            raise NotReady('invalid process inventory size')
        slots = min(131072, capacity + 1024)
        array = (C.c_int * slots)()
        count = self.lib.proc_listallpids(array, C.sizeof(array))
        if count <= 0 or count >= slots:
            raise NotReady('process inventory unavailable or truncated')
        rows = {}
        unreadable = 0
        for pid in array[:count]:
            row = self.bsd(pid) if pid > 0 else None
            if pid > 0 and row is None:
                unreadable += 1
            if row is not None and row['uid'] == os.getuid():
                rows[pid] = row
        self.inventory_coverage = {'listed': count, 'readable_same_uid': len(rows),
                                   'unreadable_bsd_info': unreadable}
        return rows

    def sample(self, root):
        started = time.monotonic_ns()
        initial = self.usage(root.pid)
        if initial['start'] != root.start:
            raise NotReady('benchmark root PID was reused')
        if initial['exit']:
            raise NotReady('benchmark root has already exited')
        rows = self.inventory()
        family = select_family(rows, root.pid)
        members = []
        for pid in sorted(family):
            try:
                usage = self.usage(pid)
            except OSError as error:
                raise NotReady('owned member exited or could not be read during sample') from error
            if usage['exit']:
                raise NotReady('owned member is no longer live')
            if usage['start'] < root.start:
                raise NotReady('responsible member predates benchmark root; scope may be shared or PID-reused')
            # Detect exit/reuse between membership discovery and rusage.
            checked = self.bsd(pid)
            if checked != rows[pid]:
                raise NotReady('owned membership changed during sample')
            path = C.create_string_buffer(4096)
            length = self.lib.proc_pidpath(pid, path, C.sizeof(path))
            members.append({**rows[pid], **usage,
                            'path': os.fsdecode(path.value) if length > 0 else None})
        final = self.usage(root.pid)
        if final['start'] != root.start:
            raise NotReady('benchmark root changed during sample')
        if final['exit']:
            raise NotReady('benchmark root exited during sample')
        uncertain_webkit = sum(row['name'].startswith('com.apple.WebKi') and row['responsible'] <= 0
                               for row in rows.values())
        if uncertain_webkit:
            raise NotReady('a same-user WebKit responsibility could not be resolved')
        return {'monotonic_ns': started, 'sample_duration_ns': time.monotonic_ns() - started,
                'cpu_timebase': self.timebase,
                'inventory_coverage': self.inventory_coverage,
                'members': members, 'footprint_bytes': sum(p['footprint_bytes'] for p in members),
                'rss_bytes': sum(p['rss_bytes'] for p in members),
                'live_cpu_counter_ns': sum(p['user_ns'] + p['system_ns'] for p in members),
                'scope': 'live responsibility/PPID family; CPU total excludes unsampled dead processes; RSS sum can double-count shared pages'}

    def spawn(self, command, environment, log, cwd=None):
        """Create only this benchmark child; stdout/stderr use an exclusive log."""
        command = [os.fspath(value) for value in command]
        if not command or not Path(command[0]).is_absolute():
            raise ValueError('benchmark executable must be an absolute path')
        if any(not key or '=' in key for key in environment):
            raise ValueError('invalid environment key')
        if cwd is not None and (not Path(cwd).is_absolute() or not Path(cwd).is_dir()):
            raise ValueError('benchmark working directory must be an existing absolute directory')
        strings = [*command, *(key + '=' + value for key, value in environment.items())]
        if any('\0' in value for value in strings) or sum(len(os.fsencode(v)) for v in strings) > 1024 * 1024:
            raise ValueError('invalid or oversized spawn arguments/environment')
        argv = (C.c_char_p * (len(command) + 1))(*(os.fsencode(v) for v in command), None)
        env = (C.c_char_p * (len(environment) + 1))(
            *(os.fsencode(k + '=' + v) for k, v in environment.items()), None)
        attr, actions = C.c_void_p(), C.c_void_p()
        initialized_attr = initialized_actions = False
        descriptor = os.open(log, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)

        def checked(code):
            if code:
                actual = code if code > 0 else C.get_errno() or errno.EIO
                raise OSError(actual, os.strerror(actual))

        try:
            checked(self.lib.posix_spawnattr_init(C.byref(attr)))
            initialized_attr = True
            checked(self.lib.responsibility_spawnattrs_setdisclaim(C.byref(attr), 1))
            checked(self.lib.posix_spawn_file_actions_init(C.byref(actions)))
            initialized_actions = True
            checked(self.lib.posix_spawn_file_actions_addopen(C.byref(actions), 0, b'/dev/null', os.O_RDONLY, 0))
            for destination in (1, 2):
                checked(self.lib.posix_spawn_file_actions_adddup2(C.byref(actions), descriptor, destination))
            if cwd is not None:
                checked(self.lib.posix_spawn_file_actions_addchdir_np(C.byref(actions), os.fsencode(cwd)))
            pid = C.c_int()
            checked(self.lib.posix_spawn(C.byref(pid), os.fsencode(command[0]), C.byref(actions),
                                        C.byref(attr), argv, env))
            if pid.value <= 0:
                raise NotReady('spawn did not return a positive owned PID')
            # Capture identity before the caller can signal this owned child.
            try:
                identity = Identity(pid.value, self.usage(pid.value)['start'])
            except BaseException:
                # Still our direct, unreaped child. No other PID can replace it.
                os.kill(pid.value, signal.SIGTERM)
                os.waitpid(pid.value, 0)
                raise
            return OwnedProcess(self, identity)
        finally:
            if initialized_actions:
                self.lib.posix_spawn_file_actions_destroy(C.byref(actions))
            if initialized_attr:
                self.lib.posix_spawnattr_destroy(C.byref(attr))
            os.close(descriptor)


class OwnedProcess:
    def __init__(self, api, identity):
        self.api, self.identity, self.returncode = api, identity, None

    @property
    def pid(self):
        return self.identity.pid

    def poll(self):
        if self.returncode is None:
            pid, status = os.waitpid(self.pid, os.WNOHANG)
            if pid:
                self.returncode = os.waitstatus_to_exitcode(status)
        return self.returncode

    def wait(self, timeout):
        deadline = time.monotonic() + timeout
        while self.poll() is None:
            if time.monotonic() >= deadline:
                raise TimeoutError('owned benchmark process is still running')
            time.sleep(.02)
        return self.returncode

    def terminate(self):
        if self.poll() is not None:
            return
        usage = self.api.usage(self.pid)
        if usage['start'] != self.identity.start:
            raise NotReady('refusing to signal a reused benchmark PID')
        os.kill(self.pid, signal.SIGTERM)


if __name__ == '__main__':
    raise SystemExit(main())

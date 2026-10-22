#!/usr/bin/env python3
"""Compile and execute the same prerequisite policy with synthetic OS adapters."""
import argparse
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile

import windows_installer


def check(output):
    if sys.platform != 'win32':
        return {'status': 'not_ready', 'reason': 'Native Inno Setup policy probe requires Windows'}, 2
    try:
        compiler = windows_installer.compiler()
    except RuntimeError as error:
        return {'status': 'not_ready', 'reason': str(error)}, 2
    output.mkdir(parents=True, exist_ok=False)
    try:
        with tempfile.TemporaryDirectory(dir=os.environ.get('RUST_GUARD_ISOLATED_ROOT')) as temporary:
            staging = Path(temporary)
            compile_log = windows_installer.run(str(compiler), '/Qp', '/DOutputDir='+str(staging),
                str(windows_installer.ROOT/'tooling/windows/installer-policy-probe.iss'))
            (output/'compile.log').write_text(compile_log, encoding='utf-8')
            report = output/'policy.json'
            result = subprocess.run([str(staging/'installer-policy-probe.exe'), '/VERYSILENT',
                                     '/SUPPRESSMSGBOXES', '/NORESTART', '/SP-',
                                     '/LOG='+str(output/'probe.log'), '/Report='+str(report)],
                                    check=True, capture_output=True, timeout=60)
            data = json.loads(report.read_text(encoding='utf-8-sig'))
            if (data.get('status') != 'passed' or data.get('assertions') != 11
                    or data.get('real_registry_access') is not False
                    or data.get('real_bootstrapper_executed') is not False):
                raise ValueError('Installer policy probe report is incomplete')
            return {**data, 'scope': 'native prerequisite decision policy; synthetic registry/process adapters',
                    'real_runtime_installation': 'not_run', 'exit_code': result.returncode}, 0
    except (OSError, ValueError, RuntimeError, subprocess.SubprocessError) as error:
        return {'status': 'failed', 'reason': str(error)}, 1


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    report, code = check(args.output.resolve())
    if args.output.is_dir():
        (args.output/'summary.json').write_text(json.dumps(report, indent=2)+'\n', encoding='utf-8')
    print(json.dumps(report))
    raise SystemExit(code)

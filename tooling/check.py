#!/usr/bin/env python3
"""Repository-local offline product gates; never contacts PTT."""

import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import platform
import subprocess
import shutil
import sys
import socket
import tempfile
import tomllib
from urllib.parse import unquote, urlsplit

ROOT = Path(__file__).resolve().parent.parent
REQUIRED = (
    'README.md', 'AGENTS.md', 'CONTRIBUTING.md',
    'docs/architecture.md', 'docs/roadmap.md',
    'docs/dual-color.md', 'docs/keepalive.md', 'docs/development-harness.md',
    'docs/active/minimal-browser.md',
    'tooling/check.py', 'tooling/test_check.py', 'tooling/desktop_input.py', 'tooling/desktop_capture.py', 'tooling/inspect_png.ps1',
)
RESEARCH_PATHS = (
    'research', 'reverse-engineering', 'evidence', 'tools', '.local/evidence',
    'docs/analysis.md', 'docs/native-static.md', 'docs/local-formats.md',
    'docs/legacy-rendering-notes.md', 'docs/rea-ghidra-confirmation.md',
    'docs/reproduce.md', 'docs/analysis-tools-windows.md',
)
SOURCE_ARTIFACT_NAMES = frozenset((
    'legacy-client.exe', 'legacy-disassembly.txt', 'previous-history.bundle',
))


class CheckFailure(Exception):
    pass


def require_suite(stdout):
    match = re.search(r'test result: ok\. ([1-9][0-9]*) passed; 0 failed; 0 ignored; 0 measured; 0 filtered out;', stdout)
    if not match:
        raise CheckFailure('product suite is empty, skipped, filtered, incomplete or unsuccessful')
    return int(match[1])


def provenance(root):
    facts = {'platform': platform.platform(), 'commit': None, 'dirty': None,
             'fixture_sha256': {}}
    if shutil.which('git'):
        try:
            revision = subprocess.run(['git', 'rev-parse', 'HEAD'], cwd=root, capture_output=True, text=True, timeout=10)
            status = subprocess.run(['git', 'status', '--porcelain'], cwd=root, capture_output=True, text=True, timeout=10)
            if revision.returncode == 0:
                facts['commit'] = revision.stdout.strip()
            if status.returncode == 0:
                facts['dirty'] = bool(status.stdout.strip())
        except (OSError, subprocess.TimeoutExpired):
            facts['git_metadata'] = 'unavailable'
    for name in ('dual-color.json', 'redraw.json', 'navigation.json'):
        path = root / 'tests/fixtures' / name
        if path.is_file():
            facts['fixture_sha256'][name] = hashlib.sha256(path.read_bytes()).hexdigest()
    return facts


def read_text(path):
    try:
        text = path.read_text(encoding='utf-8-sig')
    except (OSError, UnicodeError) as error:
        raise CheckFailure(f'cannot read UTF-8 file: {path.name}: {error}') from error
    if '\ufffd' in text:
        raise CheckFailure(f'replacement character in text: {path.name}')
    return text


def prose(text):
    """Ignore fenced code/comments; validate inline links and reference definitions."""
    text = re.sub(r'<!--.*?-->', '', text, flags=re.S)
    lines = []
    fence = None
    for line in text.splitlines():
        marker = re.match(r'^ {0,3}(`{3,}|~{3,})(.*)$', line)
        if fence is None and marker:
            fence = marker[1]
        elif fence is not None:
            if marker and marker[1][0] == fence[0] and len(marker[1]) >= len(fence) and not marker[2].strip():
                fence = None
        else:
            lines.append(line)
    return '\n'.join(lines)


def local_link_targets(text):
    text = prose(text)
    inline = re.findall(r'!?\[[^\]\n]*\]\(\s*(<[^>\n]+>|[^\s)]+)(?:\s+"[^"\n]*")?\s*\)', text)
    references = re.findall(r'^ {0,3}\[[^\]\n]+\]:\s*(<[^>\n]+>|[^\s]+)', text, re.M)
    for target in inline + references:
        try:
            parsed = urlsplit(target.strip('<>'))
        except ValueError as error:
            raise CheckFailure(f'malformed link target: {target}') from error
        if not parsed.scheme and not parsed.netloc and parsed.path:
            yield unquote(parsed.path)


def unique_object(pairs):
    result = {}
    for key, value in pairs:
        if key in result:
            raise CheckFailure(f'duplicate JSON key: {key}')
        result[key] = value
    return result


def reject_constant(value):
    raise CheckFailure(f'non-finite JSON constant: {value}')


def check_isolation(root):
    product_json = []
    for name in RESEARCH_PATHS:
        if (root / name).exists() or (root / name).is_symlink():
            raise CheckFailure(f'research must be outside the product repository: {name}')
    for directory, children, files in os.walk(root):
        children[:] = [name for name in children if name not in (
            '.git', 'target', 'node_modules', 'dist', '__pycache__')]
        for name in files:
            if name.casefold() in SOURCE_ARTIFACT_NAMES or Path(name).suffix.casefold() in ('.exe', '.dll', '.pdb', '.bin', '.idb', '.i64', '.gpr', '.gzf', '.bundle'):
                path = (Path(directory) / name).relative_to(root)
                raise CheckFailure(f'original/research artifact in product repository: {path}')
            path = Path(directory) / name
            if path.suffix == '.json' and path.relative_to(root).parts[0] != '.local':
                product_json.append(path)
    return sorted(product_json)


def reject_research_records(value):
    pending = [value]
    while pending:
        item = pending.pop()
        if isinstance(item, dict):
            if {'evidence_id', 'subject', 'provider', 'operation', 'raw_result'} <= item.keys():
                raise CheckFailure('REA research record is not a product fixture')
            pending.extend(item.values())
        elif isinstance(item, list):
            pending.extend(item)


def check_repository(root):
    root = root.resolve()
    artifacts = check_isolation(root)
    for name in REQUIRED:
        path = root / name
        if not path.is_file() or not path.resolve().is_relative_to(root):
            raise CheckFailure(f'required repository file missing or outside root: {name}')
    documents = [root / name for name in ('README.md', 'AGENTS.md', 'CONTRIBUTING.md')]
    documents += sorted((root / 'docs').rglob('*.md'))
    links = 0
    for document in documents:
        if not document.resolve().is_relative_to(root):
            raise CheckFailure(f'document outside root: {document}')
        for link in local_link_targets(read_text(document)):
            target = (document.parent / link).resolve()
            if not target.is_relative_to(root) or not target.exists():
                raise CheckFailure(f'invalid local link in {document.relative_to(root)}: {link}')
            links += 1
    for artifact in artifacts:
        if not artifact.resolve().is_relative_to(root):
            raise CheckFailure(f'product JSON outside root: {artifact}')
        try:
            value = json.loads(read_text(artifact), object_pairs_hook=unique_object, parse_constant=reject_constant)
            reject_research_records(value)
        except (ValueError, CheckFailure) as error:
            raise CheckFailure(f'invalid product JSON: {artifact.relative_to(root)}: {error}') from error
    return {'documents': len(documents), 'local_file_links': links, 'fixture_json_files': len(artifacts),
            'research_isolation': 'known paths, source artifact names and REA record schema checked',
            'coverage': 'working-tree integrity only; excludes Git history, heading anchors, remote URLs and product behavior'}


def run_docs(root):
    try:
        coverage = check_repository(root)
        tests = subprocess.run(
            [sys.executable, str(root / 'tooling/test_check.py')], cwd=root,
            capture_output=True, text=True, encoding='utf-8', errors='replace', timeout=60,
        )
        if tests.returncode:
            raise CheckFailure('gate regression tests failed:\n' + tests.stdout + tests.stderr)
        try:
            report = json.loads(tests.stdout)
        except ValueError as error:
            raise CheckFailure('gate regression runner did not return a JSON result') from error
        if (not isinstance(report, dict) or type(report.get('tests_run')) is not int
                or report['tests_run'] < 1 or report.get('status') != 'passed'
                or any(report.get(key) != 0 for key in ('failures', 'errors', 'skipped'))):
            raise CheckFailure('gate regression result is empty, skipped, incomplete or unsuccessful')
        return {'profile': 'docs', 'status': 'passed', **coverage,
                'gate_regressions': 'passed', 'regression_tests_run': report['tests_run']}
    except (CheckFailure, OSError, subprocess.TimeoutExpired) as error:
        return {'profile': 'docs', 'status': 'failed', 'reason': str(error)}


def unavailable(profile):
    reasons = {
        'core': 'Rust product and production-core replay runner are not implemented; no Cargo checks ran.',
        'desktop': 'Dioxus mounted/physical probe is not implemented; no GUI or pixel checks ran.',
    }
    return {'profile': profile, 'status': 'not_ready', 'reason': reasons[profile]}


def run_core(root):
    needed = ('Cargo.toml', 'Cargo.lock', 'rust-toolchain.toml', 'tests/screen.rs',
              'tests/interaction.rs', 'tests/navigation.rs', 'tests/fixtures/dual-color.json', 'tests/fixtures/redraw.json', 'tests/fixtures/navigation.json')
    if any(not (root / path).is_file() for path in needed) or not shutil.which('cargo'):
        return {'profile': 'core', 'status': 'not_ready', 'reason': 'Missing pinned Rust product, corpus or cargo.'}
    try:
        toolchain = tomllib.loads((root / 'rust-toolchain.toml').read_text(encoding='utf-8'))['toolchain']['channel']
        cargo = ['cargo', '+' + toolchain]
        commands = [cargo + ['fmt', '--all', '--', '--check'],
                    cargo + ['clippy', '--no-default-features', '--all-targets', '--locked', '--', '-D', 'warnings']]
        commands += [cargo + ['test', '--no-default-features', '--test', suite, '--locked']
                     for suite in ('screen', 'interaction', 'navigation')]
        commands += [cargo + ['run', '--quiet', '--locked', '--no-default-features', '--bin', 'replay', '--',
                              'tests/fixtures/' + name] for name in ('dual-color.json', 'redraw.json')]
        for command in commands:
            result = subprocess.run(command, cwd=root, capture_output=True, text=True, encoding='utf-8',
                                    errors='replace', timeout=300)
            if result.returncode:
                raise CheckFailure(' '.join(command) + '\n' + (result.stdout + result.stderr)[-6000:])
            if command[len(cargo)] == 'test':
                require_suite(result.stdout)
            if command[len(cargo)] == 'run':
                replay = json.loads(result.stdout)
                if replay.get('status') != 'passed' or replay.get('glyph_checks', 0) < 1:
                    raise CheckFailure('replay returned no successful glyph checks')
        return {'profile': 'core', 'status': 'passed', 'commands': len(commands),
                'replay_cases': 2, 'scope': 'PTT screen, selection and Telnet framing; no shell, PTY, GUI or network'}
    except (OSError, ValueError, KeyError, CheckFailure, subprocess.TimeoutExpired) as error:
        return {'profile': 'core', 'status': 'failed', 'reason': str(error)}


def run_desktop(root):
    if any(not (root / path).is_file() for path in ('src/main.rs', 'tooling/desktop_input.py', 'tooling/desktop_capture.py', 'tooling/desktop_navigation.py', 'tooling/inspect_png.ps1')) or not shutil.which('cargo'):
        return unavailable('desktop')
    if sys.platform != 'win32':
        return {'profile': 'desktop', 'status': 'not_ready', 'reason': 'Physical probe has only been verified on Windows.'}
    try:
        channel = tomllib.loads((root / 'rust-toolchain.toml').read_text(encoding='utf-8'))['toolchain']['channel']
        for arguments in (['fmt', '--all', '--', '--check'],
                          ['clippy', '--features', 'desktop', '--all-targets', '--locked', '--', '-D', 'warnings'],
                          ['test', '--locked', '--features', 'desktop', '--lib'],
                          ['build', '--locked', '--features', 'desktop', '--bin', 'pttstella']):
            result = subprocess.run(['cargo', '+' + channel, *arguments], cwd=root, capture_output=True,
                                    text=True, encoding='utf-8', errors='replace', timeout=300)
            if result.returncode:
                raise CheckFailure((result.stdout + result.stderr)[-6000:])
            if arguments[0] == 'test':
                require_suite(result.stdout)
        output = root / 'target/verification/desktop'
        output.mkdir(parents=True, exist_ok=True)
        report_path = output / 'probe.json'
        report_path.unlink(missing_ok=True)
        # Only the isolated probe's synthetic settings are reset; never user config.
        for name in ('probe-settings.json', 'probe-settings.json.bak', 'reload.json', 'reloaded-screen.png', 'settings-after.png'):
            (output / name).unlink(missing_ok=True)
        import desktop_input
        with socket.socket() as listener:
            listener.bind(('127.0.0.1', 0))
            port = listener.getsockname()[1]
        with tempfile.TemporaryFile() as log:
            process = subprocess.Popen([str(root / 'target/debug/pttstella.exe'), '--probe', str(output), str(port)],
                                       cwd=root, stdout=log, stderr=log,
                                       creationflags=getattr(subprocess, 'CREATE_NO_WINDOW', 0))
            try:
                input_report = desktop_input.run(port, process, output)
                if input_report.get('png_geometry_and_pixels') is not True:
                    raise CheckFailure('independent PNG inspection did not run')
                if input_report.get('selected_rows_png', {}).get('status') != 'passed':
                    raise CheckFailure('selected rows PNG inspection did not run')
                if input_report.get('font_preview_save_cancel_validation') is not True:
                    raise CheckFailure('native settings interaction did not run')
                if input_report.get('mouse_navigation',{}).get('status') != 'passed':
                    raise CheckFailure('native navigation interaction did not run')
                if input_report.get('mouse_navigation',{}).get('keepalive_default_switch_and_ui_pause') is not True:
                    raise CheckFailure('native keepalive switch and UI pause interaction did not run')
                process.wait(timeout=15)
            finally:
                if process.poll() is None:
                    process.kill()
                    process.wait(timeout=5)
        if process.returncode or not report_path.is_file():
            raise CheckFailure('desktop probe did not produce a successful report')
        report = json.loads(report_path.read_text(encoding='utf-8'))
        required = ('arrow_input', 'enter_input', 'text_input', 'trusted_webview_input', 'selection_copy', 'ime_composition', 'url_open', 'png_export', 'rows_png_export', 'settings_saved', 'mouse_navigation', 'paste_single', 'paste_confirm', 'paste_cancel', 'paste_rejected', 'loopback_input_roundtrip', 'closed_after_roundtrip', 'listeners_released', 'connection_worker_joined',
                    'fills_body', 'ansi_colours', 'art_geometry', 'hover_controls', 'proportional_zoom')
        if report.get('status') != 'passed' or any(report.get(key) is not True for key in required):
            raise CheckFailure('desktop rendering/input/cleanup probe failed: ' + json.dumps(report))
        # A second application process uses the first one's persisted preferences.
        with socket.socket() as listener:
            listener.bind(('127.0.0.1', 0))
            port = listener.getsockname()[1]
        with tempfile.TemporaryFile() as log:
            process = subprocess.Popen([str(root / 'target/debug/pttstella.exe'), '--probe', str(output), str(port), '--reload-settings'],
                                       cwd=root, stdout=log, stderr=log,
                                       creationflags=getattr(subprocess, 'CREATE_NO_WINDOW', 0))
            try:
                process.wait(timeout=25)
            finally:
                if process.poll() is None:
                    process.kill()
                    process.wait(timeout=5)
        if process.returncode or not (output/'reload.json').is_file():
            raise CheckFailure('second application did not complete its settings reload probe')
        reloaded = json.loads((output/'reload.json').read_text(encoding='utf-8'))
        saved = json.loads((output/'probe-settings.json').read_text(encoding='utf-8'))
        if reloaded.get('status') != 'passed' or reloaded.get('settings_loaded') != saved:
            raise CheckFailure('second application did not load persisted settings')
        if (output/'settings-after.png').read_bytes() != (output/'reloaded-screen.png').read_bytes():
            raise CheckFailure('restarted application pixels differed from the saved display settings')
        return {'profile': 'desktop', 'status': 'passed', 'probe': report, 'input_driver': input_report,
                'settings_restart_pixels': True,
                'artifacts': 'target/verification/desktop', 'scope': 'Windows WebView2; synthetic data, loopback only; no PTT'}
    except (OSError, ValueError, KeyError, CheckFailure, subprocess.TimeoutExpired) as error:
        return {'profile': 'desktop', 'status': 'failed', 'reason': str(error)}


def exit_code(results):
    if any(result['status'] == 'failed' for result in results):
        return 1
    if any(result['status'] != 'passed' for result in results):
        return 2
    return 0


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('profile', choices=('docs', 'core', 'desktop', 'all'))
    profile = parser.parse_args().profile
    profiles = ('docs', 'core', 'desktop') if profile == 'all' else (profile,)
    results = [run_docs(ROOT) if selected == 'docs' else run_core(ROOT) if selected == 'core'
               else run_desktop(ROOT) for selected in profiles]
    code = exit_code(results)
    print(json.dumps({'repository': str(ROOT), 'profile': profile, 'provenance': provenance(ROOT),
                      'status': ('passed', 'failed', 'not_ready')[code], 'checks': results}, ensure_ascii=True))
    return code


if __name__ == '__main__':
    raise SystemExit(main())

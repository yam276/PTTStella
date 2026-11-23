"""Cross-version production-store acceptance, using three built example drivers.

Compile examples/store_compatibility.rs with each actual revision (desktop for
the Dioxus refs, desktop-services for the candidate). No production profiles,
vaults or networks are used. Fixtures and independent expected values below
are self-authored CC0. This is file/schema/transaction acceptance, not UI or
platform credential-store acceptance.
"""
import argparse
import copy
import hashlib
import json
import os
from pathlib import Path
import platform
import subprocess
import time

FILES = ('settings.json', 'sites.json', 'filters.json', 'bookmarks.json',
         'drafts.json', 'recent-boards.json', 'display-profile.json')
STORES = dict(zip(('settings', 'sites', 'filters', 'bookmarks', 'drafts', 'recent', 'profile'), FILES))
MARKER = {'format': 'PTTStella.StoreCompatibility', 'version': 1, 'synthetic': True}
STAGES = {'candidate': (1, 'FromSlint', 'C_Chat'),
          'rollback': (2, 'BackDioxus', 'Gossiping'),
          'final': (3, 'FinalSlint', 'Tech_Job')}


def encoded(value):
    return json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(',', ':')).encode()


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def glyphs(text, left=7, right=None):
    """Our fixture uses only ASCII and explicitly two-column Chinese glyphs."""
    row, col = [], 0
    for character in text:
        width = 1 if ord(character) < 128 else 2
        row.append([col, width, character, left, left if right is None else right, False])
        col += width
    if col > 80:
        raise ValueError('Authored fixture exceeds 80 columns')
    row.extend([i, 1, ' ', left, left, False] for i in range(col, 80))
    return row


def fixture(version, auto_expand=False):
    display = {'fonts': {'kind': 'custom', 'ascii_font': 'Cascadia Mono',
                         'cjk_font': 'Noto Sans CJK TC'},
               'font_size': 21.5, 'baseline': 19.0, 'font_width': 102.0}
    app = {'version': 6, 'display': display, 'hover': 'follow_cursor',
           'language': 'en-US' if version >= 3 else 'zh-TW',
           'tools_opacity': 61 if version >= 4 else 100,
           'tools_access': {'right_click_menu': version < 5, 'show_floating_button': True},
           'article_mode': 'terminal' if version >= 6 else 'web',
           'article_boundary_navigation': version >= 6}
    if version >= 6 and auto_expand:
        app['auto_expand_images'] = True
    disk = copy.deepcopy(app)
    if version == 1:
        disk = {'version': 1, 'ascii_font': 'Cascadia Mono', 'cjk_font': 'Noto Sans CJK TC',
                'font_size': 21.5, 'baseline': 19.0, 'font_width': 102.0}
        app['hover'] = 'top_right'  # v1 carried display fields only.
    else:
        disk['version'] = version
        for first, field in [(3, 'language'), (4, 'tools_opacity'), (5, 'tools_access'),
                             (6, 'article_mode'), (6, 'article_boundary_navigation')]:
            if version < first:
                disk.pop(field)
    site = {'endpoint': 'wss://ws.ptt.cc/bbs', 'encoding': 'uao'}
    archive_rows = [glyphs('作者 Alice (local) 看板 Stella'), glyphs('中 ANSI 原始末字'),
                    glyphs('推 Alice: original source'), glyphs('最後一行 trailing  ')]
    archive_rows[1][0][3:5] = [1, 4]  # Independent left/right colors of 中.
    archive = {'version': 1, 'saved_at': 111, 'rows': archive_rows,
               'footer': glyphs('瀏覽 第 1/1 頁 (100%) 目前顯示: 第 1~4 行 (←)離開(h)說明', 0x700),
               'links': [['https://example.com/original', [[1, 2, 6]]]]}
    scope = {'site': site, 'account': 'StellaTest', 'board': 'Stella'}
    drafts = []
    for identity, body in [('edited', '第一行中文\n\x1b[1;31m亮紅中文\x1b[m\n尾行\n'),
                           ('untouched', '不改的尾空行  \n\n'),
                           ('partial', '未完成 ANSI\x1b[1;')]:
        drafts.append({'id': identity, 'scope': copy.deepcopy(scope), 'revision': 3,
                       'title': '原始標題末字', 'body': body, 'saved_at': 200, 'conflict': False})
    result = {'settings': app,
              'sites': {'version': 1, 'default': site, 'favourites': [
                  {'name': '原始 PTT', 'target': site},
                  {'name': '同端點 UAO', 'target': {'endpoint': 'telnet://legacy.example:2323', 'encoding': 'uao'}},
                  {'name': '同端點 GBK', 'target': {'endpoint': 'telnet://legacy.example:2323', 'encoding': 'gbk'}}]},
              'filters': {'version': 4, 'allow_color': 13, 'entries': [
                  {'id': 'Alice', 'note': '原始整行遮罩末字', 'policy': 'mask_row'},
                  {'id': 'Bob', 'note': '原始白名單末字', 'policy': 'allow', 'color': 5},
                  {'id': 'Carol', 'note': '原始內容遮罩🦀', 'policy': 'mask'}]},
              'bookmarks': {'version': 2, 'entries': [
                  {'board': 'Stella', 'aid': '00000101', 'label': '原始收藏末字🦀',
                   'created_at': 100, 'archive': archive}]},
              'drafts': {'version': 1, 'entries': drafts},
              'recent': {'version': 1, 'boards': ['Stella', 'Test']},
              'profile': {'format': 'PTTStella.DisplayProfile', 'version': 1,
                          'name': '合成可攜設定末字', 'display': copy.deepcopy(display)}}
    return result, disk


def changed(expected, stage):
    result = copy.deepcopy(expected)
    number, account, board = STAGES[stage]
    result['settings']['tools_opacity'] = 21 + 22 * number
    result['sites']['favourites'].append({'name': '新增末字 ' + stage,
                                         'target': {'endpoint': f'telnet://{stage}.example:2323', 'encoding': 'gbk'}})
    result['filters']['entries'].append({'id': account, 'note': '名單末字 ' + stage, 'policy': 'mask'})
    result['bookmarks']['entries'].insert(0, {'board': 'Stella', 'aid': f'0000010{number + 1}',
                                             'label': '收藏末字 ' + stage, 'created_at': 100 + number})
    entry = result['drafts']['entries'][0]
    entry['body'] += '\n修改末字:' + stage
    entry['revision'] += 1
    entry['saved_at'] = 200 + number
    result['recent']['boards'].insert(0, board)
    result['profile']['name'] += ' · ' + stage
    return result


class Runner:
    def __init__(self, drivers, output):
        self.drivers = {name: path.resolve(strict=True) for name, path in drivers.items()}
        self.output = output.resolve()
        self.output.mkdir(parents=True, exist_ok=False)
        self.environment = {k: v for k, v in os.environ.items() if not k.startswith('PTTSTELLA_')}
        self.report = {'status': 'running', 'platform': platform.platform(),
                       'scope': 'Actual production stores in separately compiled revisions; no GUI, live network, vault, or production-profile access',
                       'fixture_license': 'CC0-1.0', 'drivers': {}, 'calls': [], 'cases': {}}
        self.jobs = []
        for name, path in self.drivers.items():
            self.report['drivers'][name] = {'path': str(path), 'sha256': digest(path),
                                           'description': self.invoke(name, '--describe')}
        self.record()

    def record(self):
        (self.output / 'report.json').write_text(json.dumps(self.report, ensure_ascii=False, indent=2) + '\n', encoding='utf-8')

    def invoke(self, name, *args, code=0):
        command = [str(self.drivers[name]), *map(str, args)]
        start = time.monotonic()
        result = subprocess.run(command, env=self.environment, capture_output=True, timeout=30)
        payload = result.stdout if result.returncode == 0 else result.stderr
        try:
            message = json.loads(payload)
        except (ValueError, UnicodeDecodeError):
            raise AssertionError(f'{name} did not return structured JSON: {payload[:1000]!r}')
        self.report['calls'].append({'actor': name, 'args': list(map(str, args)), 'exit': result.returncode,
                                     'elapsed': time.monotonic() - start, 'output_sha256': hashlib.sha256(payload).hexdigest(),
                                     'status': message['status']})
        if result.returncode != code or message['status'] != ('ok' if code == 0 else 'error'):
            self.record()
            raise AssertionError(f'{name} expected exit {code}: {message}')
        return message.get('value', message)

    def same(self, actual, expected, name):
        if actual != expected:
            (self.output / ('failure-' + name + '.json')).write_text(json.dumps(
                {'actual': actual, 'expected': expected}, ensure_ascii=False, indent=2) + '\n', encoding='utf-8')
            raise AssertionError('Independent expectation mismatch: ' + name)

    def seed(self, name, version=6, auto_expand=False):
        root = self.output / name
        root.mkdir()
        (root / '.store-compatibility.json').write_bytes(encoded(MARKER))
        expected, settings_disk = fixture(version, auto_expand)
        for key, filename in STORES.items():
            (root / filename).write_bytes(encoded(settings_disk if key == 'settings' else expected[key]))
        return root, expected

    @staticmethod
    def hashes(root):
        return {p.name: digest(p) for p in sorted(root.iterdir())
                if p.is_file() and (p.name.endswith('.json') or p.name.endswith('.json.bak'))}

    def inspect(self, actor, root, expected, label):
        before = self.hashes(root)
        self.same(self.invoke(actor, 'inspect', root), expected, label)
        self.same(self.hashes(root), before, label + '-read-only')

    def roundtrips(self):
        for old in ('baseline', 'release'):
            for version in range(1, 7):
                name = f'{old}-settings-v{version}'
                root, expected = self.seed(name, version, auto_expand=old == 'baseline')
                self.inspect(old, root, expected, name + '-old-read')
                self.inspect('candidate', root, expected, name + '-candidate-read')
                for actor, stage, reader in [('candidate', 'candidate', old),
                                              (old, 'rollback', 'candidate'),
                                              ('candidate', 'final', old)]:
                    before = {filename: (root / filename).read_bytes() for filename in FILES}
                    expected = changed(expected, stage)
                    self.same(self.invoke(actor, 'edit', root, stage), expected, name + '-' + stage)
                    for filename in FILES[:-1]:
                        self.same((root / (filename + '.bak')).read_bytes(), before[filename], name + '-backup-' + filename)
                    self.inspect(reader, root, expected, name + '-' + stage + '-reopen')
                self.report['cases'][name] = {'status': 'passed', 'settings_input_version': version,
                    'stores': list(STORES), 'final_snapshot_sha256': hashlib.sha256(encoded(expected)).hexdigest(),
                    'original_archive_sha256': hashlib.sha256(encoded(expected['bookmarks']['entries'][-1]['archive'])).hexdigest(),
                    'three_writes_and_cross_version_reopens': True, 'backups_preserve_previous_bytes': True}
                self.record()

    def invalid_reads(self):
        for filename in FILES:
            name = 'corrupt-' + filename.removesuffix('.json')
            root, _ = self.seed(name)
            (root / filename).write_bytes(b'{ broken synthetic JSON')
            before = self.hashes(root)
            for actor in self.drivers:
                self.invoke(actor, 'inspect', root, code=2)
                self.same(self.hashes(root), before, name + '-' + actor)
            self.report['cases'][name] = {'status': 'passed', 'all_revisions_rejected_without_rewrite': True}
        # A future settings version is never silently reset to defaults.
        root, _ = self.seed('future-version')
        (root / 'settings.json').write_bytes(b'{"version":999}')
        before = self.hashes(root)
        for actor in self.drivers:
            self.invoke(actor, 'inspect', root, code=2)
            self.same(self.hashes(root), before, 'future-version-' + actor)
        self.report['cases']['future-version'] = {'status': 'passed', 'unchanged': True}
        self.record()

    def capability_boundary(self):
        descriptions = {name: item['description'] for name, item in self.report['drivers'].items()}
        self.same({name: item['accepts_auto_expand_images'] for name, item in descriptions.items()},
                  {'candidate': True, 'baseline': True, 'release': False}, 'version-capabilities')
        root, expected = self.seed('release-new-field-guard', auto_expand=True)
        self.inspect('candidate', root, expected, 'new-field-candidate')
        self.inspect('baseline', root, expected, 'new-field-baseline')
        before = self.hashes(root)
        self.invoke('release', 'inspect', root, code=2)
        self.same(self.hashes(root), before, 'new-field-rejected-read-only')
        self.report['cases']['release-new-field-guard'] = {
            'status': 'passed', 'operation': 'Reject unsupported schema without rewriting',
            'new_field_rollback_to_public_release': 'not_supported',
            'compatible_rollback_ref': descriptions['baseline']['source_ref'],
            'detail': 'v0.4.1 predates auto_expand_images; full newer settings must roll back to the supported Dioxus baseline.'}
        root = self.output / 'unmarked-root'
        root.mkdir()
        for actor in self.drivers:
            self.invoke(actor, 'inspect', root, code=2)
        self.same(list(root.iterdir()), [], 'unmarked-root-no-store-side-effects')
        self.report['cases']['unmarked-root'] = {'status': 'passed', 'explicit_fixture_guard': True}
        self.report['rollback_targets'] = {
            'baseline': 'Supported full seeded schema, including enabled auto images',
            'release': 'Legacy settings v1-v6 without the later auto-image field; not a rollback target for the newer field'}
        self.record()

    def concurrent(self, old, kind):
        name = f'{old}-concurrent-{kind}'
        root, expected = self.seed(name)
        barrier = root / 'barrier'
        barrier.mkdir()
        fields = [('opacity', '44'), ('hover', 'off')] if kind == 'disjoint' else (
            [('opacity', '44'), ('opacity', '65')] if kind == 'conflict' else [('draft', 'unused')] * 2)
        jobs = []
        for actor, implementation, (field, argument) in zip(('a', 'b'), (old, 'candidate'), fields):
            command = [str(self.drivers[implementation]), 'concurrent', str(root), actor, field, argument]
            process = subprocess.Popen(command, env=self.environment, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
            self.jobs.append(process)
            jobs.append((process, implementation, actor, command))
        deadline = time.monotonic() + 15
        while not all((barrier / ('ready-' + actor)).is_file() for _, _, actor, _ in jobs):
            if any(p.poll() is not None for p, *_ in jobs) or time.monotonic() >= deadline:
                raise AssertionError('Owned concurrent driver did not reach the fresh fixture barrier')
            time.sleep(0.01)
        (barrier / 'continue').write_bytes(b'go')
        results = []
        for process, implementation, actor, command in jobs:
            stdout, stderr = process.communicate(timeout=30)
            message = json.loads(stdout if process.returncode == 0 else stderr)
            results.append((process.returncode, message))
            self.report['calls'].append({'actor': implementation, 'args': command[1:], 'exit': process.returncode,
                                         'status': message['status'], 'fixture_actor': actor})
        if kind == 'disjoint':
            self.same([code for code, _ in results], [0, 0], name + '-codes')
            expected['settings'].update(tools_opacity=44, hover='off')
            self.inspect(old, root, expected, name + '-merged-old')
            self.inspect('candidate', root, expected, name + '-merged-candidate')
        elif kind == 'conflict':
            self.same(sorted(code for code, _ in results), [0, 2], name + '-codes')
            expected['settings']['tools_opacity'] = 44 if results[0][0] == 0 else 65
            self.inspect(old, root, expected, name + '-winner-old')
            self.inspect('candidate', root, expected, name + '-winner-candidate')
        else:
            self.same([code for code, _ in results], [0, 0], name + '-codes')
            self.same(sorted(m['value']['conflicted'] for _, m in results), [False, True], name + '-copies')
            actual = self.invoke('candidate', 'inspect', root)
            entries = actual['drafts']['entries']
            self.same(len(entries), 4, name + '-count')
            original = next(e for e in entries if e['id'] == 'edited')
            conflict = next(e for e in entries if e['conflict'])
            body = expected['drafts']['entries'][0]['body']
            self.same(sorted([original['body'], conflict['body']]),
                      sorted([body + '\n並行末字:a', body + '\n並行末字:b']), name + '-bodies')
            self.same([original['revision'], conflict['revision']], [4, 1], name + '-revisions')
            winner = 'b' if results[0][1]['value']['conflicted'] else 'a'
            loser = 'a' if winner == 'b' else 'b'
            base = copy.deepcopy(expected['drafts']['entries'][0])
            expected['drafts']['entries'][0]['body'] += '\n並行末字:' + winner
            expected['drafts']['entries'][0]['revision'] = 4
            base.update(id='conflict-' + loser, body=base['body'] + '\n並行末字:' + loser,
                        revision=1, conflict=True)
            expected['drafts']['entries'].append(base)
            self.same(actual, expected, name + '-independent-draft-result')
            self.inspect(old, root, expected, name + '-read-old')
            self.inspect('candidate', root, expected, name + '-read-candidate')
        self.report['cases'][name] = {'status': 'passed', 'two_owned_processes_joined': True,
                                     'native_store_lock': True, 'test_barrier_scope': 'Only these two fixture actors'}
        self.record()

    def run(self):
        try:
            self.roundtrips()
            self.invalid_reads()
            self.capability_boundary()
            for old in ('baseline', 'release'):
                for kind in ('disjoint', 'conflict', 'draft'):
                    self.concurrent(old, kind)
            if len(self.report['cases']) != 28:
                raise AssertionError('Missing required compatibility cases')
            self.report['status'] = 'passed'
        except BaseException as error:
            self.report.update(status='failed', error=str(error))
            raise
        finally:
            for process in self.jobs:
                if process.poll() is None:
                    process.kill()
                    process.wait(timeout=15)
            self.report['owned_children_joined'] = all(p.poll() is not None for p in self.jobs)
            self.record()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ('candidate', 'baseline', 'release'):
        parser.add_argument('--' + name, type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    runner = Runner({name: getattr(args, name) for name in ('candidate', 'baseline', 'release')}, args.output)
    runner.run()
    print(json.dumps({'status': runner.report['status'], 'cases': len(runner.report['cases']),
                      'output': str(runner.output)}, ensure_ascii=False))


if __name__ == '__main__':
    main()

"""Release admission and publication must fail closed, without touching GitHub."""
import copy
import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch

import release

REPOSITORY = 'example/stella'
COMMIT = 'a'*40


def pull_request():
    return {'title': 'release: v0.1.0', 'draft': False,
            'head': {'ref': 'develop', 'sha': COMMIT, 'repo': {'full_name': REPOSITORY}},
            'base': {'ref': 'main', 'sha': 'b'*40, 'repo': {'full_name': REPOSITORY}},
            'merged_at': '2026-01-01T00:00:00Z', 'merge_commit_sha': COMMIT}


class FakeGitHub:
    repository = REPOSITORY

    def __init__(self, pr=None):
        self.prs = [pr or pull_request()]
        self.tag = None
        self.release = None
        self.calls = []

    def tag_commit(self, tag):
        return self.tag

    def release_for_tag(self, tag, missing_ok=False):
        return self.api('releases/tags/'+tag, missing_ok=missing_ok)

    def api(self, endpoint, payload=None, missing_ok=False):
        self.calls.append((endpoint, payload))
        if endpoint.startswith('commits/'):
            return self.prs
        if endpoint.startswith('releases/tags/'):
            return self.release
        if endpoint == 'git/refs':
            self.tag = payload['sha']
            return {}
        raise AssertionError('Unexpected API call: '+endpoint)


class ReleaseLookupTests(unittest.TestCase):
    def test_exact_graphql_identity_resolves_draft_and_published_releases(self):
        github = release.GitHub(REPOSITORY)
        response = {'data': {'repository': {'release': {'databaseId': 123}}}}
        for draft in (True, False):
            expected = {'tag_name': 'v0.1.0', 'draft': draft, 'assets': []}
            with patch.object(release, 'command', return_value=json.dumps(response)) as command, \
                    patch.object(github, 'api', return_value=expected) as api:
                self.assertEqual(github.release_for_tag('v0.1.0'), expected)
                api.assert_called_once_with('releases/123')
                self.assertEqual(command.call_args.args[:3], ('gh', 'api', 'graphql'))
                for field in ('owner=example', 'name=stella', 'tag=v0.1.0'):
                    self.assertIn(field, command.call_args.args)

    def test_missing_release_is_distinct_from_inaccessible_repository_or_graphql_error(self):
        github = release.GitHub(REPOSITORY)
        response = {'data': {'repository': {'release': None}}}
        with patch.object(release, 'command', return_value=json.dumps(response)), \
                patch.object(github, 'api') as api:
            self.assertIsNone(github.release_for_tag('v0.1.0', missing_ok=True))
            with self.assertRaisesRegex(RuntimeError, 'does not exist'):
                github.release_for_tag('v0.1.0')
            api.assert_not_called()
        for response in ({'data': {'repository': None}}, {'data': {'repository': {}}},
                         {'errors': [{'message': 'forbidden'}], 'data': {'repository': {'release': None}}}):
            with patch.object(release, 'command', return_value=json.dumps(response)), \
                    self.assertRaises(RuntimeError):
                github.release_for_tag('v0.1.0', missing_ok=True)

    def test_invalid_release_identity_and_wrong_tag_fail_closed(self):
        github = release.GitHub(REPOSITORY)
        for identifier in (None, True, 0, -1, '123'):
            response = {'data': {'repository': {'release': {'databaseId': identifier}}}}
            with patch.object(release, 'command', return_value=json.dumps(response)), \
                    patch.object(github, 'api') as api, self.assertRaises(RuntimeError):
                github.release_for_tag('v0.1.0', missing_ok=True)
            api.assert_not_called()
        response = {'data': {'repository': {'release': {'databaseId': 123}}}}
        with patch.object(release, 'command', return_value=json.dumps(response)), \
                patch.object(github, 'api', return_value={'tag_name': 'v0.9.0'}), \
                self.assertRaisesRegex(RuntimeError, 'does not match'):
            github.release_for_tag('v0.1.0')


class VersionTests(unittest.TestCase):
    def test_semver_precedence_uses_numeric_identifiers_and_stable_after_prerelease(self):
        ordered = ['0.1.0-dev.1', '0.1.0-dev.2', '0.1.0-dev.10', '0.1.0-rc.1',
                   '0.1.0', '0.1.1', '0.2.0', '1.0.0', '2.0.0', '10.0.0']
        self.assertEqual(sorted(reversed(ordered), key=release.parse_version), ordered)
        self.assertEqual(release.parse_version('1.0.0'), release.parse_version('1.0.0'))

    def test_rejects_noncanonical_or_injectable_versions(self):
        for value in ['v1.0.0', '01.0.0', '1.0', '1.0.0-', '1.0.0-01',
                      '1.0.0+build', '1.0.0\ntag=bad', '1.0.0;echo bad', '１.0.0']:
            with self.subTest(value=value), self.assertRaises(ValueError):
                release.parse_version(value)


class AdmissionTests(unittest.TestCase):
    def test_explicit_replacement_is_limited_to_same_version_and_exact_prior_base(self):
        pr = pull_request()
        pr['head']['ref'] = 'codex/reissue-v0.1.0'
        record = {'version': '0.1.0', 'previous_commit': pr['base']['sha'],
                  'reason': 'Owner requested replacement after an editor regression.'}
        release.validate_pr(pr, REPOSITORY, '0.1.0', '0.1.0', record)
        for branch in ('develop', 'feature', 'codex/reissue-v0.2.0'):
            bad = copy.deepcopy(pr)
            bad['head']['ref'] = branch
            with self.assertRaises(ValueError):
                release.validate_pr(bad, REPOSITORY, '0.1.0', '0.1.0', record)
        for previous, replacement in [('0.0.9', record), ('0.1.0', {**record, 'previous_commit': 'c'*40})]:
            with self.assertRaises(ValueError):
                release.validate_pr(pr, REPOSITORY, '0.1.0', previous, replacement)
        with self.assertRaises(ValueError):
            release.validate_pr(pr, REPOSITORY, '0.1.0', '0.1.0')

    def test_replacement_record_is_read_from_committed_source_and_rejects_wrong_anchor(self):
        record = {'version': '0.1.0', 'previous_commit': 'b'*40, 'reason': 'Explicit replacement.'}
        for change in ({}, {'version': '0.2.0'}, {'previous_commit': 'c'*40}, {'reason': ''}):
            with patch.object(release, 'command', side_effect=['docs/release-replacements/v0.1.0.json', json.dumps({**record, **change})]):
                if change:
                    with self.assertRaises(ValueError):
                        release.replacement_at(COMMIT, 'b'*40, '0.1.0')
                else:
                    self.assertEqual(release.replacement_at(COMMIT, 'b'*40, '0.1.0'), record)
        with patch.object(release, 'command', return_value=''):
            self.assertIsNone(release.replacement_at(COMMIT, 'b'*40, '0.1.0'))

    def test_replacement_still_requires_merged_pr_and_absent_old_publication(self):
        pr = pull_request()
        pr['head']['ref'] = 'codex/reissue-v0.1.0'
        record = {'version': '0.1.0', 'previous_commit': 'b'*40, 'reason': 'Explicit replacement.'}
        def command(*args):
            if args[:2] == ('git', 'status'):
                return ''
            return COMMIT if args[-1] == 'HEAD' else 'b'*40
        with patch.object(release, 'command', side_effect=command), \
                patch.object(release, 'version_at', return_value='0.1.0'), \
                patch.object(release, 'replacement_at', return_value=record):
            github = FakeGitHub(pr)
            self.assertEqual(release.prepare(REPOSITORY, COMMIT, 'refs/heads/main', github)['version'], '0.1.0')
            github.tag = 'b'*40
            with self.assertRaisesRegex(ValueError, 'another commit'):
                release.prepare(REPOSITORY, COMMIT, 'refs/heads/main', github)
            github.tag = None
            github.release = {'draft': False}
            with self.assertRaisesRegex(ValueError, 'already published'):
                release.prepare(REPOSITORY, COMMIT, 'refs/heads/main', github)
            github.prs = []
            with self.assertRaisesRegex(ValueError, 'merge commit'):
                release.prepare(REPOSITORY, COMMIT, 'refs/heads/main', github)

    def test_release_pr_requires_local_develop_and_an_increased_version(self):
        good = pull_request()
        release.validate_pr(good, REPOSITORY, '0.1.0', '0.1.0-dev.1')
        for field, value in [('title', 'feature'), ('draft', True),
                             ('head.ref', 'feature'), ('head.repo.full_name', 'fork/stella'),
                             ('base.ref', 'develop')]:
            bad = copy.deepcopy(good)
            target = bad
            parts = field.split('.')
            for part in parts[:-1]:
                target = target[part]
            target[parts[-1]] = value
            with self.subTest(field=field), self.assertRaises(ValueError):
                release.validate_pr(bad, REPOSITORY, '0.1.0', '0.1.0-dev.1')
        for previous in ['0.1.0', '0.2.0']:
            with self.assertRaisesRegex(ValueError, 'must increase'):
                release.validate_pr(good, REPOSITORY, '0.1.0', previous)

    def test_prepare_rejects_direct_push_wrong_branch_tag_retarget_and_published_version(self):
        def command(*args):
            if args[:2] == ('git', 'status'):
                return ''
            return COMMIT if args[-1] == 'HEAD' else 'b'*40

        with patch.object(release, 'command', side_effect=command), \
                patch.object(release, 'version_at', side_effect=lambda sha: '0.1.0' if sha == COMMIT else '0.1.0-dev.1'):
            github = FakeGitHub()
            result = release.prepare(REPOSITORY, COMMIT, 'refs/heads/main', github)
            self.assertEqual(result['tag'], 'v0.1.0')
            self.assertFalse(result['prerelease'])
            with self.assertRaisesRegex(ValueError, 'only run on main'):
                release.prepare(REPOSITORY, COMMIT, 'refs/heads/develop', github)
            github.prs = []
            with self.assertRaisesRegex(ValueError, 'merge commit'):
                release.prepare(REPOSITORY, COMMIT, 'refs/heads/main', github)
            github.prs = [pull_request()]
            github.tag = 'c'*40
            with self.assertRaisesRegex(ValueError, 'another commit'):
                release.prepare(REPOSITORY, COMMIT, 'refs/heads/main', github)
            github.tag = COMMIT
            github.release = {'draft': False}
            with self.assertRaisesRegex(ValueError, 'already published'):
                release.prepare(REPOSITORY, COMMIT, 'refs/heads/main', github)

    def test_versions_are_read_from_exact_git_commits_and_lockfile_must_match(self):
        with tempfile.TemporaryDirectory(dir=os.environ.get('RUST_GUARD_ISOLATED_ROOT')) as temporary:
            root = Path(temporary)
            def git(*args):
                return subprocess.run(['git', *args], cwd=root, check=True, capture_output=True,
                                      text=True).stdout.strip()
            git('init', '-b', 'main')
            git('config', 'user.name', 'Synthetic Test')
            git('config', 'user.email', 'test@example.invalid')
            (root/'Cargo.toml').write_text('[package]\nname="pttstella"\nversion="0.1.0"\n')
            (root/'Cargo.lock').write_text('version=4\n[[package]]\nname="pttstella"\nversion="0.1.0"\n')
            git('add', '.')
            git('commit', '-m', 'synthetic version')
            first = git('rev-parse', 'HEAD')
            (root/'Cargo.toml').write_text('[package]\nname="pttstella"\nversion="0.2.0"\n')
            git('commit', '-am', 'synthetic mismatched lock')
            with patch.object(release, 'ROOT', root):
                self.assertEqual(release.version_at(first), '0.1.0')
                with self.assertRaisesRegex(ValueError, 'same version'):
                    release.version_at(git('rev-parse', 'HEAD'))


class PublicationTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory(dir=os.environ.get('RUST_GUARD_ISOLATED_ROOT'))
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        self.context = {'version': '0.1.0', 'commit': COMMIT, 'tag': 'v0.1.0', 'prerelease': False}
        self.github = FakeGitHub()
        for platform, suffixes in release.PLATFORMS.items():
            stem = 'PTTStella-0.1.0-'+platform
            files = {}
            for suffix in suffixes:
                path = self.root/(stem+suffix)
                path.write_bytes(b'synthetic native archive '+platform.encode()+suffix.encode())
                files[path.name] = release.digest(path)
            report = self.root/(stem+'.json')
            report.write_text(json.dumps({'version': '0.1.0', 'platform': platform,
                                         'source_commit': COMMIT, 'source_dirty': False,
                                         'profile': 'release', 'files': files}))
        for path in list(self.root.iterdir()):
            self.checksum(path)

    def checksum(self, path):
        (self.root/(path.name+'.sha256')).write_text(release.digest(path)+'  '+path.name+'\n')

    def test_complete_four_architecture_set_and_hashes_are_required(self):
        files = release.validate_assets(self.root, '0.1.0', COMMIT)
        self.assertEqual(len(files), 6)
        self.assertFalse(any(path.suffix == '.json' for path in files))
        (self.root/'private-settings.json').write_text('{}')
        with self.assertRaisesRegex(ValueError, 'unexpected files'):
            release.validate_assets(self.root, '0.1.0', COMMIT)
        (self.root/'private-settings.json').unlink()
        files[0].write_bytes(b'corrupted')
        with self.assertRaises(ValueError):
            release.validate_assets(self.root, '0.1.0', COMMIT)

    def test_wrong_commit_dirty_source_missing_platform_or_changed_manifest_cannot_publish(self):
        report = self.root/'PTTStella-0.1.0-linux-x64.json'
        original = json.loads(report.read_text())
        for key, value in [('source_commit', 'd'*40), ('source_dirty', True),
                           ('platform', 'macos-arm64'), ('files', {'../other': '0'*64})]:
            data = {**original, key: value}
            report.write_text(json.dumps(data))
            self.checksum(report)
            with self.subTest(key=key), self.assertRaises(ValueError):
                release.publish(self.context, self.root, self.github)
            self.assertEqual(self.github.calls, [])
        report.unlink()
        with self.assertRaises(FileNotFoundError):
            release.publish(self.context, self.root, self.github)

    def test_missing_windows_installer_prevents_publication_even_with_a_valid_zip(self):
        archive = self.root/'PTTStella-0.1.0-windows-x64.zip'
        archive.write_bytes(b'synthetic portable package')
        self.checksum(archive)
        (self.root/'PTTStella-0.1.0-windows-x64-setup.exe').unlink()
        with self.assertRaises(FileNotFoundError):
            release.publish(self.context, self.root, self.github)
        self.assertEqual(self.github.calls, [])

    def test_extra_windows_zip_or_loose_runtime_cannot_be_published(self):
        for name in ('PTTStella-0.1.0-windows-x64.zip', 'vcruntime140.dll', 'assets.zip'):
            path = self.root/name
            path.write_bytes(b'synthetic extra payload')
            self.checksum(path)
            with self.subTest(name=name), self.assertRaisesRegex(ValueError, 'unexpected files'):
                release.publish(self.context, self.root, self.github)
            self.assertEqual(self.github.calls, [])
            path.unlink()
            path.with_name(name+'.sha256').unlink()

    def test_missing_portable_exe_prevents_publication_even_when_setup_exists(self):
        (self.root/'PTTStella-0.1.0-windows-x64-portable.exe').unlink()
        with self.assertRaises(FileNotFoundError):
            release.publish(self.context, self.root, self.github)
        self.assertEqual(self.github.calls, [])

    def test_upload_failure_leaves_draft_and_never_publishes_partial_release(self):
        calls = []
        def command(*args):
            calls.append(args)
            if args[:3] == ('gh', 'release', 'create'):
                self.github.release = {'draft': True, 'assets': []}
            if args[:3] == ('gh', 'release', 'upload'):
                raise RuntimeError('synthetic upload failure')
            return ''
        with patch.object(release, 'command', side_effect=command):
            with self.assertRaisesRegex(RuntimeError, 'upload failure'):
                release.publish(self.context, self.root, self.github)
        self.assertTrue(self.github.release['draft'])
        self.assertFalse(any(args[:3] == ('gh', 'release', 'edit') for args in calls))

    def test_complete_upload_publishes_only_after_all_files_and_sums(self):
        calls = []
        uploaded = []
        def command(*args):
            calls.append(args)
            if args[:3] == ('gh', 'release', 'create'):
                self.github.release = {'draft': True, 'assets': []}
            if args[:3] == ('gh', 'release', 'upload'):
                path = Path(args[4])
                uploaded.append(path.name)
                if path.name == 'SHA256SUMS':
                    lines = path.read_text().splitlines()
                    self.assertEqual(len(lines), 6)
                    self.assertFalse(any('.json' in line for line in lines))
            return ''
        with patch.object(release, 'command', side_effect=command):
            result = release.publish(self.context, self.root, self.github)
        self.assertEqual(result['assets'], 7)
        self.assertEqual(self.github.tag, COMMIT)
        self.assertEqual(len(uploaded), 7)
        self.assertFalse(any(name.endswith('.json') for name in uploaded))
        self.assertEqual(calls[-1][:3], ('gh', 'release', 'edit'))
        self.assertIn('--latest=true', calls[-1])

    def test_versioned_chinese_notes_are_published_verbatim_without_generated_changelog(self):
        source = self.root/'source'
        notes = source/'docs/release-notes/v0.1.0.md'
        notes.parent.mkdir(parents=True)
        text = '首個正式版。\n\nFirst stable release.\n'
        notes.write_text(text, encoding='utf-8')
        observed = []
        def command(*args):
            if args[:3] == ('gh', 'release', 'create'):
                observed.append((Path(args[args.index('--notes-file') + 1]).read_text(encoding='utf-8'), args))
                self.github.release = {'draft': True, 'assets': []}
            return ''
        # The notes source is separate from the exact native artifact whitelist.
        assets = self.root/'assets'
        assets.mkdir()
        for path in list(self.root.iterdir()):
            if path.is_file():
                path.rename(assets/path.name)
        with patch.object(release, 'ROOT', source), patch.object(release, 'command', side_effect=command):
            result = release.publish(self.context, assets, self.github)
        self.assertEqual(result['assets'], 7)
        self.assertEqual(observed[0][0], text)
        self.assertNotIn('--generate-notes', observed[0][1])

    def test_changed_tag_or_existing_public_release_is_never_replaced(self):
        self.github.tag = 'b'*40
        with self.assertRaisesRegex(ValueError, 'tag changed'):
            release.publish(self.context, self.root, self.github)
        self.github.tag = COMMIT
        self.github.release = {'draft': False}
        with patch.object(release, 'command') as command, self.assertRaisesRegex(ValueError, 'cannot be modified'):
            release.publish(self.context, self.root, self.github)
        command.assert_not_called()


if __name__ == '__main__':
    unittest.main()

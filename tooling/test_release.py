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
        self.assertEqual(len(files), 9)  # five downloads plus four source manifests
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
        def command(*args):
            calls.append(args)
            if args[:3] == ('gh', 'release', 'create'):
                self.github.release = {'draft': True, 'assets': []}
            return ''
        with patch.object(release, 'command', side_effect=command):
            result = release.publish(self.context, self.root, self.github)
        self.assertEqual(result['assets'], 10)
        self.assertEqual(self.github.tag, COMMIT)
        self.assertEqual(sum(args[:3] == ('gh', 'release', 'upload') for args in calls), 10)
        self.assertEqual(calls[-1][:3], ('gh', 'release', 'edit'))
        self.assertIn('--latest=true', calls[-1])

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

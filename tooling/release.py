#!/usr/bin/env python3
"""Validate release PRs and publish verified, complete native Release assets."""
import argparse
from functools import total_ordering
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import tempfile
import tomllib

ROOT = Path(__file__).resolve().parent.parent
VERSION = re.compile(r'(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)(?:-([0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*))?')
SHA = re.compile(r'[0-9a-f]{40}')
PLATFORMS = {'windows-x64': ('-setup.exe', '-portable.exe'), 'macos-arm64': ('.zip',),
             'macos-x64': ('.zip',), 'linux-x64': ('.deb', '.tar.gz')}


@total_ordering
class Version:
    def __init__(self, value):
        match = VERSION.fullmatch(value)
        if not match:
            raise ValueError('Use SemVer major.minor.patch[-prerelease], without build metadata')
        self.core = tuple(int(part) for part in match.groups()[:3])
        self.pre = tuple(match[4].split('.')) if match[4] else ()
        if any(part.isdigit() and len(part) > 1 and part.startswith('0') for part in self.pre):
            raise ValueError('Numeric prerelease identifiers cannot have leading zeroes')

    def __eq__(self, other):
        return (self.core, self.pre) == (other.core, other.pre)

    def __lt__(self, other):
        if self.core != other.core:
            return self.core < other.core
        if not self.pre or not other.pre:
            return bool(self.pre) and not other.pre
        for left, right in zip(self.pre, other.pre):
            if left == right:
                continue
            if left.isdigit() and right.isdigit():
                return int(left) < int(right)
            if left.isdigit() != right.isdigit():
                return left.isdigit()
            return left < right
        return len(self.pre) < len(other.pre)


def parse_version(value):
    return Version(value)


def command(*args, cwd=None):
    return subprocess.run(args, cwd=cwd or ROOT, check=True, capture_output=True,
                          text=True, encoding='utf-8', timeout=120).stdout.strip()


def check_sha(value):
    if not isinstance(value, str) or not SHA.fullmatch(value):
        raise ValueError('Expected a full commit SHA')
    return value


def version_at(sha):
    check_sha(sha)
    package = tomllib.loads(command('git', 'show', sha+':Cargo.toml'))['package']
    parse_version(package['version'])
    lock = tomllib.loads(command('git', 'show', sha+':Cargo.lock'))
    roots = [p for p in lock['package'] if p['name'] == package['name'] and 'source' not in p]
    if len(roots) != 1 or roots[0]['version'] != package['version']:
        raise ValueError('Cargo.toml and the root package in Cargo.lock must have the same version')
    return package['version']


def validate_pr(pr, repository, version, previous):
    if (pr['base']['ref'] != 'main' or pr['head']['ref'] != 'develop'
            or pr['base']['repo']['full_name'] != repository
            or (pr['head'].get('repo') or {}).get('full_name') != repository):
        raise ValueError('Only a develop → main release PR from this repository is allowed')
    if pr.get('draft'):
        raise ValueError('A draft PR is not ready for release')
    if pr['title'] != 'release: v'+version:
        raise ValueError('Release PR title must be exactly: release: v'+version)
    if parse_version(version) <= parse_version(previous):
        raise ValueError(f'Release version must increase: {previous} → {version}')


def check_pr(event, repository):
    pr = event['pull_request']
    version = version_at(pr['head']['sha'])
    previous = version_at(pr['base']['sha'])
    validate_pr(pr, repository, version, previous)
    return {'version': version, 'previous_version': previous, 'tag': 'v'+version}


class GitHub:
    def __init__(self, repository):
        if not re.fullmatch(r'[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+', repository):
            raise ValueError('Invalid repository name')
        self.repository = repository

    def api(self, endpoint, payload=None, missing_ok=False):
        args = ['gh', 'api', 'repos/'+self.repository+'/'+endpoint]
        if payload is not None:
            args += ['--method', 'POST', '--input', '-']
        result = subprocess.run(args, input=json.dumps(payload) if payload is not None else None,
                                check=False, capture_output=True, text=True,
                                encoding='utf-8', timeout=120)
        try:
            data = json.loads(result.stdout)
        except ValueError:
            raise RuntimeError('GitHub API returned a non-JSON response: '+result.stderr) from None
        if result.returncode:
            if missing_ok and isinstance(data, dict) and str(data.get('status')) == '404':
                return None
            raise RuntimeError('GitHub API failed: '+result.stderr)
        return data

    def tag_commit(self, tag):
        ref = self.api('git/ref/tags/'+tag, missing_ok=True)
        if ref is None:
            return None
        obj = ref['object']
        for _ in range(8):
            if obj['type'] == 'commit':
                return check_sha(obj['sha'])
            if obj['type'] != 'tag':
                break
            obj = self.api('git/tags/'+check_sha(obj['sha']))['object']
        raise ValueError('Release tag does not resolve to a commit')

    def release_for_tag(self, tag, missing_ok=False):
        # REST lookup by tag can return 404 for an existing draft. Resolve its
        # exact identity through GraphQL, then read the same REST release by ID.
        owner, name = self.repository.split('/')
        query = ('query($owner:String!,$name:String!,$tag:String!){'
                 'repository(owner:$owner,name:$name){release(tagName:$tag){databaseId}}}')
        data = json.loads(command('gh', 'api', 'graphql', '-f', 'query='+query,
                                  '-F', 'owner='+owner, '-F', 'name='+name, '-F', 'tag='+tag))
        repository = (data.get('data') or {}).get('repository')
        if data.get('errors') or not isinstance(repository, dict) or 'release' not in repository:
            raise RuntimeError('GitHub could not resolve the Release repository')
        record = repository['release']
        if record is None:
            if missing_ok:
                return None
            raise RuntimeError('Release does not exist: '+tag)
        identifier = record.get('databaseId') if isinstance(record, dict) else None
        if type(identifier) is not int or identifier <= 0:
            raise RuntimeError('Release has no valid database identity')
        result = self.api('releases/'+str(identifier))
        if not isinstance(result, dict) or result.get('tag_name') != tag:
            raise RuntimeError('Resolved Release does not match the requested tag')
        return result


def prepare(repository, sha, ref, github):
    if ref != 'refs/heads/main':
        raise ValueError('Release jobs may only run on main')
    check_sha(sha)
    if command('git', 'rev-parse', 'HEAD') != sha:
        raise ValueError('Checkout does not match the triggering commit')
    if command('git', 'status', '--porcelain'):
        raise ValueError('Release checkout must be clean')
    version = version_at(sha)
    parent = command('git', 'rev-parse', sha+'^1')
    previous = version_at(parent)
    # This second admission check protects publishing even when repository-plan
    # limits make server-side branch protection unavailable.
    prs = [pr for pr in github.api('commits/'+sha+'/pulls?per_page=100')
           if pr.get('merged_at') and pr.get('merge_commit_sha') == sha]
    if len(prs) != 1:
        raise ValueError('Release commit must be the merge commit of one release PR')
    validate_pr(prs[0], repository, version, previous)
    tag = 'v'+version
    tagged = github.tag_commit(tag)
    if tagged is not None and tagged != sha:
        raise ValueError('Existing release tag points to another commit; tags are never moved')
    release = github.release_for_tag(tag, missing_ok=True)
    if release is not None and not release['draft']:
        raise ValueError('This version is already published; published assets are never replaced')
    return {'version': version, 'previous_version': previous, 'tag': tag, 'commit': sha,
            'prerelease': bool(parse_version(version).pre)}


def inspect(sha, ref):
    """Explicit manual build verification has no publishing authority."""
    check_sha(sha)
    if ref not in ('refs/heads/develop', 'refs/heads/main'):
        raise ValueError('Manual package verification is limited to develop or main')
    if command('git', 'rev-parse', 'HEAD') != sha or command('git', 'status', '--porcelain'):
        raise ValueError('Verification requires a clean checkout of the triggering commit')
    return {'version': version_at(sha), 'commit': sha}


def digest(path):
    with path.open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def validate_assets(directory, version, sha):
    expected, downloads = set(), []
    for target, suffixes in PLATFORMS.items():
        stem = f'PTTStella-{version}-{target}'
        report = directory/(stem+'.json')
        data = json.loads(report.read_text(encoding='utf-8'))
        names = {stem+suffix for suffix in suffixes}
        if (data['version'] != version or data['platform'] != target
                or data['source_commit'] != sha or data['source_dirty'] is not False
                or data['profile'] != 'release' or set(data['files']) != names):
            raise ValueError('Release manifest does not match its version, commit or platform: '+report.name)
        for name in sorted(names):
            path = directory/name
            if digest(path) != data['files'][name]:
                raise ValueError('Release archive hash mismatch: '+name)
            downloads.append(path)
        # Manifests remain required build evidence in Actions artifacts. Only
        # executable downloads are public Release assets and listed in sums.
        expected |= names | {report.name}
    for name in expected.copy():
        path = directory/name
        checksum = directory/(name+'.sha256')
        if checksum.read_text(encoding='ascii') != digest(path)+'  '+name+'\n':
            raise ValueError('Release checksum mismatch: '+name)
        expected.add(checksum.name)
    actual = {p.name for p in directory.iterdir()}
    if actual != expected or any(not p.is_file() or p.is_symlink() for p in directory.iterdir()):
        raise ValueError('Release directory contains missing or unexpected files')
    return sorted(downloads)


def publish(context, directory, github):
    files = validate_assets(directory, context['version'], context['commit'])
    checksums = '\n'.join(digest(p)+'  '+p.name for p in files)+'\n'
    # Keep generated release metadata outside the immutable artifact set.
    with tempfile.TemporaryDirectory(dir=os.environ.get('RUST_GUARD_ISOLATED_ROOT')) as temporary:
        temporary = Path(temporary)
        sums = temporary/'SHA256SUMS'
        sums.write_text(checksums, encoding='ascii')
        files.append(sums)
        tag, repository = context['tag'], github.repository
        tagged = github.tag_commit(tag)
        if tagged is None:
            github.api('git/refs', {'ref': 'refs/tags/'+tag, 'sha': context['commit']})
        elif tagged != context['commit']:
            raise ValueError('Release tag changed during the build')
        release = github.release_for_tag(tag, missing_ok=True)
        if release is None:
            notes = temporary/'notes.md'
            notes.write_text(
                'Windows x64 Setup / portable EXE · macOS Apple Silicon / Intel ZIP · Linux x64 DEB / tar.gz\n\n'
                'SHA-256 checksums: `SHA256SUMS`. Source: `'+context['commit']+'`.\n\n'
                'macOS builds are ad-hoc signed and not notarized; Windows builds are not '
                'Authenticode signed. Windows Setup installs Evergreen WebView2 only if missing '
                '(internet required). The portable EXE requires an existing WebView2 Runtime and '
                'does not install it. Both include static CRT, embedded assets and third-party notices; '
                'export notices with `--export-licenses OUTPUT_FILE`. '
                'Linux packages require a compatible '
                'Ubuntu 24.04 graphical environment. Packaging and automated tests do not replace '
                'platform GUI or live-login acceptance.\n', encoding='utf-8')
            custom_notes = ROOT/'docs'/'release-notes'/(tag+'.md')
            if custom_notes.is_file():
                notes.write_text(custom_notes.read_text(encoding='utf-8'), encoding='utf-8')
            args = ['gh', 'release', 'create', tag, '--repo', repository, '--verify-tag',
                    '--draft', '--title', 'PTTStella '+tag, '--notes-file', str(notes)]
            if not custom_notes.is_file():
                args.append('--generate-notes')
            if context['prerelease']:
                args.append('--prerelease')
            command(*args)
            release = github.release_for_tag(tag)
        if not release['draft']:
            raise ValueError('Published Releases cannot be modified by this workflow')
        existing = {asset['name'] for asset in release['assets']}
        if existing - {p.name for p in files}:
            raise ValueError('Draft Release contains unexpected assets; inspect it before retrying')
        for path in files:
            if path.name in existing:
                download = temporary/'existing'
                download.mkdir(exist_ok=True)
                command('gh', 'release', 'download', tag, '--repo', repository,
                        '--pattern', path.name, '--dir', str(download))
                if digest(download/path.name) != digest(path):
                    raise ValueError('Existing draft asset differs; it will not be overwritten: '+path.name)
            else:
                command('gh', 'release', 'upload', tag, str(path), '--repo', repository)
        # Publish only after the entire set has uploaded. Failures leave a draft.
        command('gh', 'release', 'edit', tag, '--repo', repository, '--draft=false',
                '--prerelease='+str(context['prerelease']).lower(),
                '--latest='+str(not context['prerelease']).lower())
    return {'status': 'published', 'tag': tag, 'commit': context['commit'], 'assets': len(files)}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('operation', choices=('check-pr', 'inspect', 'prepare', 'publish'))
    parser.add_argument('--assets', type=Path)
    args = parser.parse_args()
    repository = os.environ['GITHUB_REPOSITORY']
    if args.operation == 'check-pr':
        event = json.loads(Path(os.environ['GITHUB_EVENT_PATH']).read_text(encoding='utf-8'))
        result = check_pr(event, repository)
    elif args.operation == 'inspect':
        result = inspect(os.environ['GITHUB_SHA'], os.environ['GITHUB_REF'])
    else:
        github = GitHub(repository)
        result = prepare(repository, os.environ['GITHUB_SHA'], os.environ['GITHUB_REF'], github)
        if args.operation == 'publish':
            if args.assets is None:
                parser.error('publish requires --assets')
            result = publish(result, args.assets, github)
    if args.operation != 'publish' and os.environ.get('GITHUB_OUTPUT'):
        with Path(os.environ['GITHUB_OUTPUT']).open('a', encoding='utf-8') as stream:
            for key, value in result.items():
                stream.write(key+'='+str(value).lower()+'\n' if isinstance(value, bool)
                             else key+'='+value+'\n')
    print(json.dumps(result))


if __name__ == '__main__':
    try:
        main()
    except (OSError, ValueError, KeyError, RuntimeError, subprocess.SubprocessError) as error:
        print(str(error), file=sys.stderr)
        raise SystemExit(1)

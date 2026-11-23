"""Reconstruct candidate product sources from a local, hash-checked Git bundle.

This reconstructs product files, not third-party source closure or a binary.
Only a new destination is accepted. No network, build or existing checkout is used.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import re
import subprocess


def digest(path):
    with path.open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def relative_name(value):
    if (not isinstance(value, str) or not value or len(value) > 512
            or '\\' in value or '\0' in value or ':' in value):
        raise ValueError('invalid source path')
    path = PurePosixPath(value)
    if (path.is_absolute() or str(path) != value
            or any(part in ('.', '..', '.git') for part in path.parts)):
        raise ValueError('source path must be normalized and stay outside .git')
    return value


def checked_hash(value, size=64):
    if not isinstance(value, str) or re.fullmatch('[0-9a-f]{' + str(size) + '}', value) is None:
        raise ValueError('invalid source digest')
    return value


def regular_input(root, record, limit):
    name = relative_name(record['file'])
    path = root / name
    if (path.is_symlink() or not path.is_file() or path.resolve().parent != root
            or not 0 < path.stat().st_size <= limit):
        raise ValueError('source payload must be a bounded regular file in the manifest directory')
    if digest(path) != checked_hash(record['sha256']):
        raise ValueError('source payload hash mismatch: ' + name)
    return path


def reconstruct(manifest_path, candidate, output):
    manifest_path = Path(manifest_path)
    if manifest_path.is_symlink() or not 0 < manifest_path.stat().st_size <= 16 * 1024 * 1024:
        raise ValueError('invalid source manifest')
    root = manifest_path.resolve().parent
    manifest = json.loads(manifest_path.read_text(encoding='utf-8'))
    if (manifest.get('format') != 'PTTStella.CandidateProductSource'
            or type(manifest.get('version')) is not int or manifest['version'] != 1):
        raise ValueError('unsupported source manifest')
    record = manifest['candidates'][candidate]
    commit = checked_hash(record['base_commit'], 40)
    tree = checked_hash(record['source_tree'], 40)
    files = record['files']
    if (not isinstance(files, dict) or not 1 <= len(files) <= 8192
            or not {'Cargo.toml', 'Cargo.lock', 'build.rs'} <= files.keys()
            or not any(name.startswith('src/') for name in files)):
        raise ValueError('source file inventory is incomplete or too large')
    for name, checksum in files.items():
        relative_name(name)
        checked_hash(checksum)
    bundle = regular_input(root, manifest['bundle'], 256 * 1024 * 1024)
    patch = regular_input(root, record['patch'], 8 * 1024 * 1024) if record.get('patch') else None
    # Resolve parents, but never traverse an existing destination or replace it.
    output = Path(output)
    if os.path.lexists(output):
        raise ValueError('source destination already exists')
    output = output.parent.resolve(strict=True) / output.name
    if output.name in ('', '.', '..'):
        raise ValueError('invalid source destination')
    output.mkdir(mode=0o700)
    environment = {key: value for key, value in os.environ.items() if not key.startswith('GIT_')}
    environment.update(GIT_CONFIG_NOSYSTEM='1', GIT_CONFIG_GLOBAL=os.devnull)

    def git(*arguments):
        return subprocess.run(
            ['git', '-c', 'core.autocrlf=false', '-c',
             'core.hooksPath=' + str(output / '.git/disabled-hooks'), *arguments],
            cwd=output, env=environment, check=True, capture_output=True, text=True,
            timeout=120).stdout.strip()

    git('init', '--quiet', '--object-format=sha1')
    git('fetch', '--quiet', '--no-tags', '--no-write-fetch-head', str(bundle), 'HEAD')
    git('checkout', '--quiet', '--detach', commit)
    if patch:
        git('apply', '--check', '--index', str(patch))
        git('apply', '--index', str(patch))
    if git('write-tree') != tree:
        raise ValueError('reconstructed Git tree differs from the recorded build source')
    for name, checksum in files.items():
        path = output / name
        if (path.is_symlink() or not path.is_file()
                or not path.resolve().is_relative_to(output) or digest(path) != checksum):
            raise ValueError('reconstructed source mismatch: ' + name)
    tracked = set(git('ls-files', '-z').rstrip('\0').split('\0'))
    if tracked != set(files):
        raise ValueError('reconstructed source inventory differs')
    return {'status': 'product_source_reconstructed', 'candidate': candidate,
            'base_commit': commit, 'source_tree': tree, 'files_verified': len(files),
            'output': str(output), 'bundle_sha256': digest(bundle),
            'scope': 'Product files only; no third-party closure, compilation, binary reproduction or native acceptance claim.'}


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--manifest', type=Path, required=True)
    parser.add_argument('--candidate', required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args(argv)
    result = reconstruct(args.manifest, args.candidate, args.output)
    print(json.dumps(result, ensure_ascii=False, indent=2))


if __name__ == '__main__':
    main()

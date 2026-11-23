"""Shared provenance and fixture admission for internal Slint candidate packages."""
import json
import re

from bundle_windows import digest


def inspect_build_record(path, executable, target):
    record = json.loads(path.read_text(encoding='utf-8'))
    if not (record.get('target') == target and record.get('features') == 'native-player-probe'
            and record.get('build_exit') == 0 and record.get('exe_sha256') == digest(executable)
            and re.fullmatch(r'[0-9a-f]{40}', record.get('source_ref', ''))
            and re.fullmatch(r'\d+\.\d+\.\d+-dev\.\d+', record.get('version', ''))):
        raise ValueError('Expected a matching full-candidate build record')
    return record


def media_inputs(directory):
    manifest_path = directory / 'manifest.json'
    manifest = json.loads(manifest_path.read_text(encoding='utf-8'))
    if manifest.get('license') != 'CC0-1.0':
        raise ValueError('Expected self-authored CC0 media fixtures')
    media = {}
    for entry in manifest.get('files', []):
        name = entry.get('file')
        if name not in ('quadrants.mp4', 'quadrants.webm', 'quadrants.ogv') or name in media:
            raise ValueError('Unexpected fixture member')
        path = directory / name
        if not 0 < path.stat().st_size <= 24 * 1024 * 1024 or digest(path) != entry.get('sha256'):
            raise ValueError('Fixture checksum mismatch')
        media[name] = path
    if len(media) != 3:
        raise ValueError('All three media formats are required')
    return manifest_path, media

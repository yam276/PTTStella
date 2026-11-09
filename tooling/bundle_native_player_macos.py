"""Bundle the explicit loopback player experiment; no launch, publish or replacement."""
import argparse
import json
import os
from pathlib import Path
import plistlib
import shutil
import sys
import tempfile

import bundle_slint_macos
import native_runtime_macos as runtime

ROOT = Path(__file__).resolve().parent.parent
NAME = 'Native-Player-Bundled.app'


def fixtures(source, destination):
    manifest = json.loads((source / 'manifest.json').read_text())
    expected = {'quadrants.mp4', 'quadrants.webm', 'quadrants.ogv'}
    if {entry['file'] for entry in manifest['files']} != expected or len(manifest['files']) != 3:
        raise ValueError('Expected the three owned native-player fixtures')
    if manifest.get('license') != 'CC0-1.0':
        raise ValueError('Fixture provenance is missing')
    destination.mkdir()
    for entry in manifest['files']:
        path = source / entry['file']
        if path.is_symlink() or not 0 < path.stat().st_size <= 16 * 1024 * 1024:
            raise ValueError('Fixture must be a regular bounded file')
        if path.stat().st_size != entry['bytes'] or runtime.digest(path) != entry['sha256']:
            raise ValueError('Fixture digest mismatch')
        shutil.copy2(path, destination / path.name)
    shutil.copy2(source / 'manifest.json', destination / 'manifest.json')


def bundle(executable, media, skia_license, output, allowed, source_archives=None):
    if sys.platform != 'darwin':
        raise ValueError('Native macOS tooling is required')
    output.mkdir(parents=True, exist_ok=True)
    final = output / NAME
    if os.path.lexists(final):
        raise ValueError('An existing deliverable is preserved; choose a new output directory')
    with tempfile.TemporaryDirectory(prefix='.native-player-', dir=output) as stage:
        app = Path(stage) / NAME
        contents = app / 'Contents'
        binary, arch, manifest = bundle_slint_macos.stage_runtime(
            executable, contents, skia_license, allowed, source_archives)
        resources = contents / 'Resources'
        fixtures(media, resources / 'media')
        (resources / 'READ-ME-zh-TW.txt').write_text(
            'PTTStella 原生播放器本機實驗包，並非完整產品或正式發佈。\n'
            '啟動後只連自帶合成影片的 loopback 服務，按播放才載入。\n'
            '不讀取 PTTStella 設定、密碼庫或站端資料。\n'
            '預設 MP4。命令列可執行 Contents/MacOS/native --fixture FILE NEW_OUTPUT。\n'
            '關閉後報告在 macOS 暫存目錄 PTTStella-Native-Player-*；log 在同名 .log。\n'
            '動態庫、來源 URL／hash、Homebrew recipe 及授權文字見 NativeRuntimeNotices。\n'
            '對應原始碼交付和乾淨機器／其他架構驗收仍待完成；勿作正式套件發佈。\n', encoding='utf-8')
        launch = binary.parent / 'launch'
        launch.write_text('''#!/bin/sh
set -eu
out="${TMPDIR:-/tmp/}PTTStella-Native-Player-$(/usr/bin/uuidgen)"
exec "$(dirname "$0")/native" --fixture "$(dirname "$0")/../Resources/media/quadrants.mp4" "$out" > "$out.log" 2>&1
''')
        launch.chmod(0o755)
        (contents / 'Info.plist').write_bytes(plistlib.dumps({
            'CFBundleExecutable': 'launch',
            'CFBundleIdentifier': 'io.github.yam276.pttstella.slint.player.bundled',
            'CFBundleName': 'Native Player Bundled', 'CFBundlePackageType': 'APPL',
            'CFBundleVersion': '1', 'NSHighResolutionCapable': True,
        }))
        runtime.run('codesign', '--force', '--sign', '-', '--timestamp=none', str(app))
        runtime.run('codesign', '--verify', '--deep', '--strict', str(app))
        app.rename(final)
    return {'app': str(final), 'architecture': arch, 'signing': 'ad_hoc',
            'notarized': False, 'runtime_files': len(manifest['files']) - 1,
            'runtime_bytes': sum(f['bytes'] for f in manifest['files'] if f['file'].startswith('Frameworks/')),
            'source_delivery': manifest['source_delivery'], 'native_acceptance': 'not_run'}


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--executable', type=Path, required=True)
    parser.add_argument('--media', type=Path, required=True)
    parser.add_argument('--skia-license', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--allow-prefix', type=Path, action='append', required=True)
    parser.add_argument('--source-archives', type=Path, help='Exact SBOM archives for absent installed notices')
    args = parser.parse_args()
    print(json.dumps(bundle(args.executable.resolve(strict=True), args.media.resolve(strict=True),
                            args.skia_license.resolve(strict=True), args.output.resolve(),
                            args.allow_prefix, args.source_archives), indent=2))

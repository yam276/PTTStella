"""Package the full Slint main app with libmpv for internal macOS acceptance.

Accepts an already built native-player-probe / pttstella-slint executable.
Never builds, launches a GUI, contacts a BBS, changes the product identity,
or publishes a release. Corresponding-source and native acceptance remain
separate requirements; this package is explicitly a candidate.
"""
import argparse
import json
import os
from pathlib import Path
import plistlib
import re
import shutil
import subprocess
import sys
import tempfile
import tomllib

import bundle_common
import bundle_macos
import bundle_release
import native_runtime_macos as runtime
from slint_candidate_common import inspect_build_record

ROOT = Path(__file__).resolve().parent.parent
NAME = 'PTTStella Slint Candidate'
IDENTIFIER = 'io.github.yam276.pttstella.slint.candidate'
TARGETS = {'arm64': 'aarch64-apple-darwin', 'x86_64': 'x86_64-apple-darwin'}


def stage_runtime(executable, contents, skia_license, allowed, source_archives=None, *,
                  source_manifest=None):
    """Shared native Slint/libmpv payload for the main app and player probe."""
    if not skia_license.is_file() or not 0 < skia_license.stat().st_size <= 4 * 1024 * 1024:
        raise ValueError('The built Skia license is required and must be bounded')
    arch = runtime.run('lipo', '-archs', str(executable)).strip()
    if arch not in TARGETS:
        raise ValueError('Expected one supported macOS executable architecture')
    binary = contents / 'MacOS' / 'native'
    binary.parent.mkdir(parents=True)
    resources = contents / 'Resources'
    resources.mkdir(exist_ok=True)
    options = {'source_manifest': source_manifest} if source_manifest is not None else {}
    manifest = runtime.install(executable, binary, contents / 'Frameworks',
                               resources / 'NativeRuntimeNotices', allowed, source_archives, **options)
    if not any(Path(entry['file']).name.startswith('libmpv.') for entry in manifest['files']):
        raise ValueError('Full native-video candidate requires a bundled libmpv')
    for entry in manifest['files']:
        runtime.run('lipo', str(contents / entry['file']), '-verify_arch', arch)
    bundle_common.copy_licenses(resources / 'RustNotices', TARGETS[arch], feature='native-player-probe')
    shutil.copy2(skia_license, resources / 'RustNotices' / 'LICENSE_SKIA')
    (resources / 'runtime-manifest.json').write_text(json.dumps(manifest, indent=2) + '\n', encoding='utf-8')
    return binary, arch, manifest


def inspect_profile(binary):
    # The production --show-profile branch only describes the research paths.
    # Do not let developer loader overrides hide an unbundled dependency.
    environment = {key: value for key, value in os.environ.items()
                   if not key.startswith(('DYLD_', 'LD_', 'PTTSTELLA_'))}
    process = subprocess.run([str(binary), '--show-profile'], env=environment,
                             capture_output=True, text=True, check=True, timeout=30)
    profile = json.loads(process.stdout)
    if not (profile.get('application') == 'PTTStellaSlintResearch'
            and profile.get('credential_scope') == 'SlintResearch'
            and profile.get('probe_credential_scope') == 'SlintSyntheticProbe'
            and profile.get('stores_read') is False and profile.get('vault_opened') is False):
        raise ValueError('Expected the isolated Slint main-app profile descriptor')
    return profile


def bundle(executable, skia_license, output, allowed, source_archives=None, *, source_manifest=None,
           build_record=None):
    if sys.platform != 'darwin':
        raise ValueError('Native macOS packaging tools are required')
    # Each candidate gets a new owned directory. Existing deliveries, evidence,
    # and even empty caller directories are preserved before any tool is run.
    output.mkdir(parents=True, exist_ok=False)
    version = tomllib.loads((ROOT / 'Cargo.toml').read_text(encoding='utf-8'))['package']['version']
    build = None
    if build_record is not None:
        build_arch = runtime.run('lipo', '-archs', str(executable)).strip()
        if build_arch not in TARGETS:
            raise ValueError('Expected one supported macOS build architecture')
        build = inspect_build_record(build_record, executable, TARGETS[build_arch])
        if build['version'] != version:
            raise ValueError('Candidate build and package version differ')
    numeric_version = version.split('-', 1)[0].split('+', 1)[0]
    if not re.fullmatch(r'\d+\.\d+\.\d+', numeric_version):
        raise ValueError('Cargo version must have three numeric components')
    with tempfile.TemporaryDirectory(prefix='.slint-main-', dir=output) as temporary:
        stage = Path(temporary)
        app = stage / 'app' / (NAME + '.app')
        contents = app / 'Contents'
        source_hash = runtime.digest(executable)
        options = {'source_manifest': source_manifest} if source_manifest is not None else {}
        binary, arch, manifest = stage_runtime(executable, contents, skia_license, allowed,
                                              source_archives, **options)
        if (runtime.digest(executable) != source_hash
                or next(entry['original_sha256'] for entry in manifest['files']
                        if entry['file'] == 'MacOS/native') != source_hash):
            raise ValueError('Source executable changed during runtime staging')
        resources = contents / 'Resources'
        icon = bundle_macos.make_icon(ROOT / 'assets/icons/pttstella.png', resources, stage)
        info = {'CFBundleExecutable': 'native', 'CFBundleIdentifier': IDENTIFIER,
                'CFBundleName': NAME, 'CFBundleDisplayName': NAME,
                'CFBundlePackageType': 'APPL', 'CFBundleInfoDictionaryVersion': '6.0',
                'CFBundleVersion': numeric_version, 'CFBundleShortVersionString': numeric_version,
                'CFBundleIconFile': icon, 'NSHighResolutionCapable': True,
                'NSPrincipalClass': 'NSApplication', 'CFBundleDevelopmentRegion': 'en',
                'LSApplicationCategoryType': 'public.app-category.social-networking'}
        (contents / 'Info.plist').write_bytes(plistlib.dumps(info))
        (resources / 'READ-ME-zh-TW.txt').write_text(
            'PTTStella Slint 主程式候選；供內部驗收，尚非正式發佈。\n'
            '正常啟動會連至獨立研究設定的預設站，初始為 PTT WSS；不自動登入。\n'
            '設定與資料使用 PTTStellaSlintResearch，密碼庫使用 SlintResearch。\n'
            '不自動讀取、搬移或覆寫原 PTTStella 資料；舊資料只以副本驗收。\n'
            '無需 Homebrew 或外部 WebView runtime；libmpv 閉包包含於 Frameworks。\n'
            '離線格線可執行 Contents/MacOS/native --probe-grid；輸入法用 --probe-ime。\n'
            '軟體繪製可加 --software-renderer；影片沿用原生CPU路徑，輸出最高1280×720。\n'
            '命令 --show-profile 只列路徑，不開啟資料或密碼庫。\n'
            '第三方文字與來源清單在 RustNotices／NativeRuntimeNotices。\n'
            '完整對應原始碼交付、乾淨機器／其他架構／功能矩陣及效能仍待驗收。\n'
            '此包使用 ad-hoc 簽名，未公證；安裝時勿覆蓋正式 PTTStella。\n', encoding='utf-8')
        profile = inspect_profile(binary)
        packaging_ref = runtime.run('git', '-C', str(ROOT), 'rev-parse', 'HEAD').strip()
        packaging_dirty = bool(runtime.run('git', '-C', str(ROOT), 'status', '--porcelain').strip())
        report = {'status': 'bundled', 'version': version, 'architecture': arch,
                  'target': TARGETS[arch], 'features': 'native-player-probe',
                  'source_ref': build['source_ref'] if build is not None else packaging_ref,
                  'source_dirty': build.get('source_dirty', True) if build is not None else packaging_dirty,
                  'packaging_ref': packaging_ref, 'packaging_dirty': packaging_dirty,
                  'build': build,
                  'source_executable_sha256': source_hash,
                  'profile': profile, 'signing': 'ad_hoc', 'notarized': False,
                  'native_acceptance': 'not_run', 'source_delivery': manifest['source_delivery'],
                  'runtime_files': len(manifest['files']) - 1,
                  'runtime_bytes': sum(f['bytes'] for f in manifest['files'] if f['file'].startswith('Frameworks/'))}
        (resources / 'candidate.json').write_text(json.dumps(report, indent=2) + '\n', encoding='utf-8')
        runtime.run('plutil', '-lint', str(contents / 'Info.plist'))
        runtime.run('codesign', '--force', '--sign', '-', '--timestamp=none', str(app))
        runtime.run('codesign', '--verify', '--deep', '--strict', str(app))
        # The main app starts directly, without absolute developer paths,
        # synthetic flags, seed data or a loopback-only shell launcher.
        stem = f'PTTStella-Slint-Candidate-{version}-macos-{arch}'
        archive, image = stage / (stem + '.zip'), stage / (stem + '.dmg')
        runtime.run('ditto', '-c', '-k', '--sequesterRsrc', '--keepParent', str(app), str(archive))
        bundle_release.macos_disk_image(app, image, volume_name=NAME)
        checksums = stage / (stem + '.sha256')
        checksums.write_text(''.join(f'{runtime.digest(p)}  {p.name}\n' for p in (archive, image)), encoding='ascii')
        report.update(app=str(output / app.name), executable_sha256=runtime.digest(binary),
                      downloads={p.name: runtime.digest(p) for p in (archive, image)})
        if runtime.digest(executable) != source_hash:
            raise ValueError('Source executable changed before publication')
        # All artifacts are complete and signature/image checks have passed.
        app.rename(output / app.name)
        bundle_common.publish_files((archive, image, checksums), output)
        (output / 'package-report.json').write_text(json.dumps(report, indent=2) + '\n', encoding='utf-8')
    return report


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--executable', type=Path, required=True)
    parser.add_argument('--skia-license', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--allow-prefix', type=Path, action='append', required=True)
    parser.add_argument('--source-archives', type=Path)
    parser.add_argument('--source-manifest', type=Path)
    parser.add_argument('--build-record', type=Path)
    args = parser.parse_args()
    print(json.dumps(bundle(args.executable.resolve(strict=True), args.skia_license.resolve(strict=True),
                            args.output.resolve(), args.allow_prefix, args.source_archives,
                            source_manifest=args.source_manifest, build_record=args.build_record), indent=2))


if __name__ == '__main__':
    main()

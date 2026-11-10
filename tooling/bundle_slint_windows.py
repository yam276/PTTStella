"""Package an already built full Slint/libmpv Windows candidate; never launch or build it."""
import argparse
import json
from pathlib import Path
import re
import shutil
import tempfile
import zipfile

import bundle_common
from bundle_windows import SYSTEM_DLLS, TARGET, digest, inspect_pe
from slint_candidate_common import inspect_build_record, media_inputs

# Additional Windows components imported by Slint/Skia and the native player.
SYSTEM = SYSTEM_DLLS | frozenset('avicap32 avrt cfgmgr32 d2d1 d3d11 d3d12 d3dcompiler_47 '
                                'dwrite dxgi iphlpapi ncrypt normaliz opengl32 rpcrt4 setupapi '
                                'shcore uiautomationcore usp10 wsock32'.split())
CRT = frozenset('msvcp140.dll msvcp140_1.dll msvcp140_2.dll vcruntime140.dll '
                'vcruntime140_1.dll concrt140.dll'.split())
MPV = 'libmpv-2.dll'
ACCEPTANCE = Path(__file__).parent / 'windows' / 'slint_acceptance.ps1'


def inspect_payload(executable, runtime_dlls):
    """Use the same normal/delay import reader as the existing Windows packager."""
    runtime = {}
    for path in runtime_dlls:
        name = path.name.lower()
        if not re.fullmatch(r'[a-z0-9_.+-]+\.dll', name) or name in runtime:
            raise ValueError('Invalid or duplicate runtime DLL: ' + name)
        runtime[name] = path
    if MPV not in runtime:
        raise ValueError('The full candidate requires libmpv-2.dll')
    images = {'pttstella-slint.exe': inspect_pe(executable)}
    images.update((name, inspect_pe(path, require_gui=False)) for name, path in runtime.items())
    if MPV not in images['pttstella-slint.exe']['imports']:
        raise ValueError('Expected a main executable linked to libmpv-2.dll')
    used, required_crt, visited = set(), set(), set()
    pending = ['pttstella-slint.exe']
    while pending:
        image_name = pending.pop()
        if image_name in visited:
            continue
        visited.add(image_name)
        image = images[image_name]
        for name in image['imports']:
            if not re.fullmatch(r'[a-z0-9_.+-]+\.dll', name):
                raise ValueError('Invalid import name: ' + name)
            if any(forbidden in name for forbidden in ('webview', 'webkit', 'wry')):
                raise ValueError('WebView dependency is forbidden: ' + name)
            if name in runtime:
                used.add(name)
                pending.append(name)
            elif name in CRT:
                required_crt.add(name)
            elif not name.startswith(('api-ms-win-', 'ext-ms-win-')) and Path(name).stem not in SYSTEM:
                raise ValueError('Unbundled non-system dependency: ' + name)
    if used != runtime.keys():
        raise ValueError('Unused runtime DLLs: ' + ', '.join(sorted(runtime.keys() - used)))
    return images, sorted(required_crt)


def runner(arguments, *, software=False, video=False, drafts=False):
    # Everything runs under setlocal; inherited diagnostic switches cannot turn
    # an offline acceptance entry into a different probe or external endpoint.
    return ('@echo off\nsetlocal\ncd /d "%~dp0"\n'
            'for /f "tokens=1 delims==" %%V in (\'set PTTSTELLA_ 2^>nul\') do set "%%V="\n'
            'for /f "tokens=1 delims==" %%V in (\'set SLINT_ 2^>nul\') do set "%%V="\n'
            'if not exist evidence mkdir evidence\n'
            'set "run=evidence\\run-%RANDOM%-%RANDOM%"\n'
            + ('set "PTTSTELLA_VIDEO_FIXTURE=%CD%\\fixtures\\quadrants.mp4"\n' if video else '')
            + ('if not exist evidence\\drafts mkdir evidence\\drafts\n'
               'set "PTTSTELLA_SLINT_DRAFTS_PROBE_DIR=%CD%\\evidence\\drafts"\n' if drafts else '')
            + 'start "" /b /wait "%CD%\\pttstella-slint.exe" '
            + ('--software-renderer ' if software else '') + arguments
            + ' >"%run%.log" 2>&1\n'
            'set "probe_exit=%ERRORLEVEL%"\ntype "%run%.log"\n'
            'echo Exit: %probe_exit%\necho Log: %run%.log\npause\nexit /b %probe_exit%\n')


def checks_runner():
    return ('@echo off\nsetlocal\ncd /d "%~dp0"\n'
            'if not exist evidence mkdir evidence\n'
            'set "log=evidence\\checks-%RANDOM%-%RANDOM%.log"\n'
            'powershell.exe -NoLogo -NoProfile -File "%CD%\\Check-Native.ps1" >"%log%" 2>&1\n'
            'set "probe_exit=%ERRORLEVEL%"\ntype "%log%"\n'
            'echo Exit: %probe_exit%\necho Log: %log%\npause\nexit /b %probe_exit%\n')


README = '''PTTStella Slint Windows x64 完整主程式候選（內部驗收）

先執行 Check-All.cmd：13案只用合成資料與本機服務，逐案核對結果、退出清理及Close ACK。
每案最多90秒；evidence/native-.../native-results.json保存通過、失敗或not_ready及各案log。
Exit 0只代表component案例通過，原生IME／DPI／可聽音訊等人工項目仍未驗收。
本入口使用Windows內建PowerShell，不變更系統設定或執行原則；若無法執行可保留log並用下列入口。

手動操作可執行 Run-Offline.cmd：只連本機合成服務，可試浮球、縮放、設定、關於與關窗。
正常雙擊 pttstella-slint.exe 會連至研究設定的預設站，初始為 PTT WSS，不自動登入。
資料使用 PTTStellaSlintResearch、密碼庫使用 SlintResearch；不自動搬移正式版資料。

Run-Grid.cmd／Run-Grid-Software.cmd：80×24、雙色字、框線、縮放、末行及全螢幕。
Run-IME.cmd：切 Microsoft 注音，在第22行輸入、選字、Esc取消、再Enter提交「中」。
候選窗應靠近游標，只有一次提交／2 bytes。這項須由實際鍵盤操作驗收。
Run-Reading.cmd：合成文章，可試兩種閱讀模式、尋找及選取。
Run-Drafts.cmd：合成草稿；請另試注音、末字、儲存、重開與正常退出。
Run-Video.cmd／Run-Video-Software.cmd：點開合成四色影片，試播放、暫停、拖曳、關閉。
Check-Shell.cmd／Check-Shell-Software.cmd：自動工具包案例，正常會自行關窗。
Check-Motion.cmd／Check-Motion-Software.cmd：讀取真實OS動畫偏好並檢查通知與退出，不改系統設定。
動畫變更事件需要Windows 10 version 2004／build19041以上；不支援時會明確標示not_ready。
Show-Profile.cmd：只列資料路徑，不開啟資料或密碼庫。

evidence保存各次log與探針結果；輸出目錄不覆寫。探針使用合成資料，不需帳密。
請回報Windows版本、顯示倍率、GPU、執行哪個入口及錯誤，附合成截圖和log即可。
也請在沒有WebView2的環境啟動；不需移除日常使用的WebView2。
若缺少MSVCP140／VCRUNTIME140，需Microsoft Visual C++ x64 Redistributable：
https://learn.microsoft.com/en-us/cpp/windows/latest-supported-vc-redist/
本包未簽章；Windows原生功能、乾淨安裝、資料回退、對應來源交付及效能尚未驗收。
libmpv與Rust／Skia授權在licenses；自製影片為CC0，詳fixtures/manifest.json。
'''


def bundle(executable, runtime_dlls, skia_license, mpv_license, provenance, build_record,
           fixture_directory, output):
    record = inspect_build_record(build_record, executable, TARGET)
    images, required_crt = inspect_payload(executable, runtime_dlls)
    origin = json.loads(provenance.read_text(encoding='utf-8'))
    mpv = next(path for path in runtime_dlls if path.name.lower() == MPV)
    if origin.get('files', {}).get(MPV) != digest(mpv):
        raise ValueError('libmpv provenance does not match the DLL')
    inputs = [executable, *runtime_dlls, skia_license, mpv_license, provenance, build_record, ACCEPTANCE]
    for license in [skia_license, mpv_license]:
        if not 0 < license.stat().st_size <= 4 * 1024 * 1024:
            raise ValueError('A bounded nonempty runtime license is required')
    manifest_path, media = media_inputs(fixture_directory)
    inputs += [manifest_path, *media.values()]
    original = {str(path): digest(path) for path in inputs}
    output.mkdir(parents=True, exist_ok=False)
    with tempfile.TemporaryDirectory(prefix='.slint-candidate-', dir=output) as temp:
        stage = Path(temp)
        stem = f'PTTStella-Slint-Candidate-{record["version"]}-windows-x64'
        app = stage / stem
        app.mkdir()
        shutil.copy2(executable, app / 'pttstella-slint.exe')
        for path in runtime_dlls:
            shutil.copy2(path, app / path.name.lower())
        licenses = app / 'licenses'
        bundle_common.copy_licenses(licenses, TARGET, feature='native-player-probe')
        shutil.copy2(skia_license, licenses / 'LICENSE_SKIA')
        shutil.copy2(mpv_license, licenses / 'LICENSE_MPV_LGPL')
        shutil.copy2(provenance, licenses / 'libmpv-provenance.json')
        fixtures = app / 'fixtures'
        fixtures.mkdir()
        shutil.copy2(manifest_path, fixtures / 'manifest.json')
        for path in media.values():
            shutil.copy2(path, fixtures / path.name)
        shutil.copy2(ACCEPTANCE, app / 'Check-Native.ps1')
        (app / 'Check-All.cmd').write_text(checks_runner(), encoding='ascii', newline='\r\n')
        for name, args in {
            'Run-Offline': '--probe-benchmark "%run%"', 'Run-Grid': '--probe-grid',
            'Run-IME': '--probe-ime', 'Run-Reading': '--probe-article',
            'Run-Drafts': '--probe-drafts', 'Run-Video': '--probe-media',
            'Check-Shell': '--verify-shell "%run%"', 'Check-Motion': '--verify-motion "%run%"',
            'Show-Profile': '--show-profile',
        }.items():
            configurations = [False, True] if name in ('Run-Grid', 'Run-Video', 'Check-Shell', 'Check-Motion') else [False]
            for software in configurations:
                filename = name + ('-Software' if software else '') + '.cmd'
                (app / filename).write_text(runner(args, software=software, video=name == 'Run-Video',
                                                  drafts=name == 'Run-Drafts'),
                                            encoding='ascii', newline='\r\n')
        (app / 'READ-ME-zh-TW.txt').write_text(README, encoding='utf-8-sig', newline='\r\n')
        report = {'status': 'bundled', 'target': TARGET, 'features': 'native-player-probe',
                  'build': record, 'images': images, 'required_vc_redist_dlls': required_crt,
                  'profile_acceptance': 'not_run', 'native_acceptance': 'not_run',
                  'source_delivery': 'not_ready', 'signing': 'unsigned',
                  'dependency_scope': 'PE normal/delay imports; dynamic loading and native runtime untested',
                  'runtime_closure': 'requires_vc_redist' if required_crt else 'bundled',
                  'payload_sha256': {p.relative_to(app).as_posix(): digest(p)
                                     for p in app.rglob('*') if p.is_file()}}
        (app / 'candidate.json').write_text(json.dumps(report, indent=2) + '\n', encoding='utf-8')
        archive = stage / (stem + '.zip')
        with zipfile.ZipFile(archive, 'x', zipfile.ZIP_DEFLATED, strict_timestamps=False) as zipped:
            for path in sorted(app.rglob('*')):
                if path.is_file():
                    zipped.write(path, path.relative_to(stage).as_posix())
        checksum = stage / (archive.name + '.sha256')
        checksum.write_text(f'{digest(archive)}  {archive.name}\n', encoding='ascii')
        report['zip_sha256'] = digest(archive)
        external = stage / 'package-report.json'
        external.write_text(json.dumps(report, indent=2) + '\n', encoding='utf-8')
        if original != {str(path): digest(path) for path in inputs}:
            raise ValueError('Candidate input changed during packaging')
        bundle_common.publish_files((archive, checksum, external), output)
    return report


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ('exe', 'skia-license', 'mpv-license', 'provenance', 'build-record', 'fixtures', 'output'):
        parser.add_argument('--' + name, type=Path, required=True)
    parser.add_argument('--runtime-dll', type=Path, action='append', required=True)
    args = parser.parse_args()
    report = bundle(args.exe, args.runtime_dll, args.skia_license, args.mpv_license,
                    args.provenance, args.build_record, args.fixtures, args.output)
    print(json.dumps({'status': report['status'], 'zip_sha256': report['zip_sha256'],
                      'native_acceptance': report['native_acceptance']}))


if __name__ == '__main__':
    main()

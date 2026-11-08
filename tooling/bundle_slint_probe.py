"""Package the offline Slint grid and IME probes for Windows x64."""
import argparse
import hashlib
from pathlib import Path
import shutil
import struct
import tempfile
from zipfile import ZIP_DEFLATED, ZipFile

from bundle_common import copy_licenses, publish_files

TARGET = 'x86_64-pc-windows-msvc'
STEM = 'PTTStella-Slint-S1-Windows-x64'

README = """PTTStella Slint 離線原生驗收
============================

解壓 ZIP 後直接執行下列批次檔。格線探針只連本機 WebSocket，
輸入法探針全程離線。測試使用合成內容，無需帳號或密碼。

1. Run-Grid.cmd
   確認 80×24、雙色「中」的兩半、16 色、框線及第 24 行可見。
   試 125% 和 150%，拖到末行。再切全螢幕，以 Esc 恢復。
   關窗後批次檔應顯示 received_bytes 513 與 close acknowledged。
2. Run-IME.cmd
   切到 Microsoft 注音。點第 22 行淺藍底線，輸入 ㄓㄨㄥ。
   先按 Esc 取消一次，再重新輸入，使用空白或方向鍵選字，
   以 Enter 確認「中」。候選窗應靠近終端游標，底部應顯示
   「組字提交 1 次 · 2 bytes」。關窗後應只列一筆 ime commit。
3. Run-Grid-Software.cmd
   用軟體 renderer 重跑第 1 項，確認字形和雙色仍正確。
4. Capture-Grid.cmd（選用）
   自動擷取原生視窗與固定 960×576 格線 PNG，並關窗。
   產物寫進解壓目錄下的 grid-evidence。重跑前先移走舊輸出。

回報 Windows 版本、顯示倍率、哪個批次檔成功或失敗。可以附合成
視窗的截圖、候選窗及 grid-run.log／ime-run.log／software-run.log。
若環境本來沒有 WebView2，可在該環境執行相同測試。

此包是 x64 離線探針，含交叉編譯的本機驗收執行檔。
若 Windows 回報缺少 MSVCP140.dll 或 VCRUNTIME140.dll，
請使用 Microsoft 的 Visual C++ x64 Redistributable
https://learn.microsoft.com/en-us/cpp/windows/latest-supported-vc-redist/
關於按鈕顯示 Slint 署名，第三方授權位於 licenses。
"""

RUNNERS = {
    'Run-Grid.cmd': ('winit-skia', '--probe-grid', 'grid-run.log'),
    'Run-IME.cmd': ('winit-skia', '--probe-ime', 'ime-run.log'),
    'Run-Grid-Software.cmd': ('winit-skia-software', '--probe-grid', 'software-run.log'),
    'Capture-Grid.cmd': (
        'winit-skia',
        '--probe-grid --capture grid-evidence\\window.png --source-png grid-evidence\\grid.png',
        'capture-run.log',
    ),
}


def verify_x64(path):
    with path.open('rb') as binary:
        header = binary.read(64)
        if len(header) != 64 or header[:2] != b'MZ':
            raise ValueError('expected a Windows executable')
        offset, = struct.unpack_from('<I', header, 60)
        binary.seek(offset)
        if binary.read(6) != b'PE\0\0\x64\x86':
            raise ValueError('expected a Windows x64 executable')


def bundle(executable, skia_license, output):
    verify_x64(executable)
    output.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix='.slint-probe-', dir=output) as temporary:
        staging = Path(temporary)
        contents = staging / STEM
        contents.mkdir()
        shutil.copy2(executable, contents / 'pttstella-slint.exe')
        copy_licenses(contents / 'licenses', TARGET, feature='slint-research')
        shutil.copy2(skia_license, contents / 'licenses' / 'LICENSE_SKIA')
        (contents / 'READ-ME-zh-TW.txt').write_text(README, encoding='utf-8-sig', newline='\r\n')
        for name, (backend, arguments, log) in RUNNERS.items():
            runner = f'''@echo off
setlocal
cd /d "%~dp0"
set "SLINT_BACKEND={backend}"
'''
            if name == 'Capture-Grid.cmd':
                runner += 'if not exist grid-evidence mkdir grid-evidence\n'
            runner += f'''pttstella-slint.exe {arguments} >"{log}" 2>&1
set "probe_exit=%ERRORLEVEL%"
type "{log}"
pause
exit /b %probe_exit%
'''
            (contents / name).write_text(runner, encoding='ascii', newline='\r\n')
        archive = staging / (STEM + '.zip')
        with ZipFile(archive, 'x', ZIP_DEFLATED, strict_timestamps=False) as file:
            for path in sorted(contents.rglob('*')):
                if path.is_file():
                    file.write(path, path.relative_to(staging).as_posix())
        with archive.open('rb') as file:
            checksum = hashlib.file_digest(file, 'sha256').hexdigest()
        digest = staging / (archive.name + '.sha256')
        digest.write_text(f'{checksum}  {archive.name}\n', encoding='ascii')
        return publish_files((archive, digest), output)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--exe', type=Path, required=True)
    parser.add_argument('--skia-license', type=Path, required=True)
    parser.add_argument('--out-dir', type=Path, required=True)
    args = parser.parse_args()
    for path in bundle(args.exe, args.skia_license, args.out_dir):
        print(path.resolve())


if __name__ == '__main__':
    main()

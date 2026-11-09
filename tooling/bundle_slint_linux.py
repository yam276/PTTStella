"""Package an existing full Slint ELF as an isolated Linux candidate; never build or launch UI."""
import argparse
import json
from pathlib import Path
import sys
import tempfile

import bundle_common
import bundle_release
from bundle_windows import digest
from slint_candidate_common import inspect_build_record, media_inputs

TARGET = 'x86_64-unknown-linux-gnu'
PROGRAM = 'pttstella-slint-candidate'
# Winit/xkbcommon/Wayland use dlopen, so ELF DT_NEEDED and dpkg-shlibdeps
# cannot discover these. Keep desktop runtime packages separate from SDK -dev packages.
WINIT_RUNTIME = ('libx11-6', 'libx11-xcb1', 'libxcursor1', 'libxi6',
                 'libxkbcommon0', 'libxkbcommon-x11-0', 'libwayland-client0',
                 'libwayland-cursor0', 'libwayland-egl1', 'libegl1', 'libgl1')


def runner(arguments, *, software=False, video=False, drafts=False):
    return (r'''#!/bin/sh
set -eu
base=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
for name in $(env | sed -n 's/^\([A-Za-z_][A-Za-z0-9_]*\)=.*/\1/p'); do
    case "$name" in PTTSTELLA_*|SLINT_*) unset "$name";; esac
done
mkdir -p "$base/evidence"
run=$(mktemp -d "$base/evidence/run.XXXXXX")
'''
            + ('export PTTSTELLA_VIDEO_FIXTURE="$base/fixtures/quadrants.mp4"\n' if video else '')
            + ('mkdir -p "$base/evidence/drafts"\n'
               'export PTTSTELLA_SLINT_DRAFTS_PROBE_DIR="$base/evidence/drafts"\n' if drafts else '')
            + 'set +e\n"$base/' + PROGRAM + '" '
            + ('--software-renderer ' if software else '') + arguments
            + ' >"$run/run.log" 2>&1\nstatus=$?\nset -e\ncat "$run/run.log"\n'
            'printf "Exit: %s\\nLog: %s\\n" "$status" "$run/run.log"\nexit "$status"\n')


README = '''PTTStella Slint Linux x64 完整主程式候選（內部驗收）

此包不替換正式PTTStella；資料使用PTTStellaSlintResearch、密碼庫SlintResearch。
一般執行檔／桌面入口會連研究設定的預設站，初始PTT WSS，不自動登入。
先用可寫目錄解壓tar.gz；其中Run-Offline.sh只連本機合成服務，可試浮球與設定。
Run-Grid.sh／Run-Grid-Software.sh驗格線、雙色字、末行、縮放與全螢幕。
Run-IME.sh須實際使用繁中輸入法，核對候選窗、取消、選字及一次「中」提交。
Run-Reading.sh提供合成文章；Run-Drafts.sh將合成草稿保存在evidence/drafts供重開。
Run-Video.sh／Run-Video-Software.sh點開四色影片，試播放、暫停、拖曳與關閉。
Check-Shell.sh／Check-Shell-Software.sh是會自行關窗的工具包案例，不證明原生輸入。
Show-Profile.sh只列路徑，不讀寫資料或密碼庫。各次log與輸出保存在evidence，不覆寫。

.deb使用獨立名稱pttstella-slint-candidate，安裝時由套件管理器處理Depends。
tar.gz不內附系統函式庫；需安裝package-report.json的Depends所列套件，包含libmpv2
及X11／Wayland runtime（例如libxkbcommon-x11-0），不需要安裝完整開發SDK。
第三方授權在licenses；自製三格式影片為CC0，詳fixtures/manifest.json。
原生X11／Wayland、IME、DPI、聲音、乾淨安裝、資料回退、來源與效能仍待完整驗收。
'''


def bundle(executable, skia_license, build_record, fixtures, output, *, loader=None):
    if sys.platform != 'linux':
        raise ValueError('Linux loader and dpkg packaging tools are required')
    record = inspect_build_record(build_record, executable, TARGET)
    manifest, media = media_inputs(fixtures)
    if not 0 < skia_license.stat().st_size <= 4 * 1024 * 1024:
        raise ValueError('A bounded nonempty Skia license is required')
    inputs = [executable, skia_license, build_record, manifest, *media.values()]
    original = {str(path): digest(path) for path in inputs}
    output.mkdir(parents=True, exist_ok=False)
    with tempfile.TemporaryDirectory(prefix='.slint-linux-', dir=output) as temporary:
        stage = Path(temporary)
        readme = stage / 'candidate-readme.txt'
        readme.write_text(README, encoding='utf-8')
        resources = {'licenses/LICENSE_SKIA': skia_license,
                     'licenses/build-record.json': build_record, 'fixtures/manifest.json': manifest}
        resources.update(('fixtures/' + name, path) for name, path in media.items())
        for name, args in {
            'Run-Offline': '--probe-benchmark "$run/result"', 'Run-Grid': '--probe-grid',
            'Run-IME': '--probe-ime', 'Run-Reading': '--probe-article',
            'Run-Drafts': '--probe-drafts', 'Run-Video': '--probe-media',
            'Check-Shell': '--verify-shell "$run/result"', 'Show-Profile': '--show-profile',
        }.items():
            for software in ([False, True] if name in ('Run-Grid', 'Run-Video', 'Check-Shell') else [False]):
                filename = name + ('-Software' if software else '') + '.sh'
                path = stage / filename
                path.write_text(runner(args, software=software, video=name == 'Run-Video',
                                       drafts=name == 'Run-Drafts'), encoding='utf-8')
                path.chmod(0o755)
                resources[filename] = path
        stem = f'PTTStella-Slint-Candidate-{record["version"]}-linux-x64'
        files, runtime = bundle_release.linux_package(
            stage, stem, record['version'], executable, program=PROGRAM,
            display_name='PTTStella Slint Candidate',
            desktop_id='io.github.yam276.pttstella.slint.candidate', feature='native-player-probe',
            readme=readme, resources=resources, loader=loader,
            forbidden_libraries=('webkit', 'javascriptcore', 'webview', 'libwry'),
            extra_depends=WINIT_RUNTIME)
        runtime['runtime'] = 'Linux x64; see actual dpkg Depends and recorded build environment'
        runtime['late_loaded_dependencies'] = list(WINIT_RUNTIME)
        report = {'status': 'bundled', 'target': TARGET, 'features': 'native-player-probe',
                  'build': record, 'runtime': runtime, 'native_acceptance': 'not_run',
                  'source_delivery': 'not_ready', 'signing': 'unsigned',
                  'dependency_scope': 'ELF loader resolution and dpkg mappings; GUI/IME/runtime not proven',
                  'downloads': {file.name: digest(file) for file in files}}
        report_file = stage / 'package-report.json'
        report_file.write_text(json.dumps(report, indent=2) + '\n', encoding='utf-8')
        checksums = stage / (stem + '.sha256')
        checksums.write_text(''.join(f'{digest(file)}  {file.name}\n' for file in files), encoding='ascii')
        if original != {str(path): digest(path) for path in inputs}:
            raise ValueError('Candidate input changed during packaging')
        bundle_common.publish_files([*files, checksums, report_file], output)
    return report


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ('exe', 'skia-license', 'build-record', 'fixtures', 'output'):
        parser.add_argument('--' + name, type=Path, required=True)
    parser.add_argument('--loader', type=Path)
    args = parser.parse_args()
    report = bundle(args.exe, args.skia_license, args.build_record, args.fixtures,
                    args.output, loader=args.loader)
    print(json.dumps({'status': report['status'], 'downloads': report['downloads']}))


if __name__ == '__main__':
    main()

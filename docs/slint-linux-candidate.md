# Slint Linux x64 候選包

2026-11-10 最新候選已改用 `.local/verification/slint-merged-id-spaces/linux-package/`，包含重複推文 ID 空格修正（產品來源 `26d9947`）。檔案、雜湊及本輪範圍見[最新四架構候選](slint-evaluation-plan.md#2026-11-10-最新候選)。以下原生／模擬與 SHA 紀錄屬先前候選，不能套用成新二進位的實機結果；舊包的必要 metadata／notice 另封存在本批 `retired-package-metadata/`。

此為 S8 內部驗收候選，尚未取代正式 Dioxus 入口。目標為 Ubuntu 24.04 x64，提供獨立名稱的 deb 與 portable tar.gz，包含 `native-player-probe` 主程式。尚未通過 Linux 原生桌面、繁中 IME、DPI、音訊、硬體 GPU、資料回退與效能驗收。

## 目前包與來源

檔案位於 Slint worktree 的 `.local/verification/slint-linux-candidate/package-current/`：

| 檔案 | SHA-256 |
| --- | --- |
| `PTTStella-Slint-Candidate-0.5.0-dev.3-linux-x64.tar.gz` | `73d88928b3d83d48e68f2f3ac76ea9c3b488b7e459407a926ffdfbf57ffc8fa5` |
| `PTTStella-Slint-Candidate-0.5.0-dev.3-linux-x64.deb` | `8e34ac7547eaee9a9ed55547ae7bf2df783a6c1517fa7435219c23d07920e334` |

執行檔 SHA-256 為 `9b68eb81ed5348f4dd44f3e05ab5a1a8284a89b884a7240f6c6df22bd7c6d5f7`，91,524,232 bytes。Rust 來源為 `6d70196` 加同批 `slint-find-pause/fixed-source.patch`，僅補診斷及修正搜尋探針先決條件，生產搜尋／失焦保護未改。建置來源、環境及 binary hash 在 `slint-find-pause/fixed-build.json`；其他打包來源狀態另記在證據。來源 commit 不是完整第三方來源交付，`source_delivery` 仍為 `not_ready`。

使用 macOS arm64 的 Rust 1.97.0／Clang／Rust LLD 交叉編譯 Linux GNU x64，連結經逐檔 hash 核對的 Debian x64 sysroot；保持既有 release 最佳化，不使用主機 libmpv。ELF、`libmpv.so.2`、target loader 的實際相依解析及 release all-target Clippy 已核對。先前嘗試在 2GiB ARM VM 內以原生 ARM rustc 交叉編譯，最後的主程式 rustc 被 OOM kill；x64 rustc 的 QEMU 試跑亦未成功。這些不是 native Linux x64 編譯通過證據。本輪確切命令、來源、套件版本及 Clippy 見 `slint-find-pause/fixed-build.json`、`clippy-exit.json` 與 `sysroot-inventory.json`；先前建置失敗仍保留於 `slint-linux-candidate/oom-inspection.json` 等紀錄。

## 在 Linux 驗收

先在交付目錄核對校驗檔：

```sh
sha256sum -c PTTStella-Slint-Candidate-0.5.0-dev.3-linux-x64.sha256
sudo apt install ./PTTStella-Slint-Candidate-0.5.0-dev.3-linux-x64.deb
pttstella-slint-candidate --show-profile
tar -xzf PTTStella-Slint-Candidate-0.5.0-dev.3-linux-x64.tar.gz
```

deb 使用獨立套件、執行檔及桌面 ID；`--show-profile` 只顯示研究用設定／資料與密碼庫 scope，不讀寫 store。一般執行檔或桌面入口會連研究設定的預設站，初始為 PTT WSS，不自動登入。要先離線試操作，進入解壓後的可寫目錄執行 `./Run-Offline.sh`，只連本機合成服務。

| Portable 入口 | 驗收內容 |
| --- | --- |
| `Run-Grid.sh`、`Run-Grid-Software.sh` | 80×24、雙色字、末行、縮放與全螢幕 |
| `Run-IME.sh` | 實際繁中輸入法候選窗、取消／選字及一次提交；需要人工操作 |
| `Run-Reading.sh` | 合成文章閱讀、尋找與捲動 |
| `Run-Drafts.sh` | 草稿輸入與重開；合成草稿明示保存於 `evidence/drafts` |
| `Run-Video.sh`、`Run-Video-Software.sh` | 自製四色 MP4、播放／暫停／拖曳與關閉 |
| `Check-Shell.sh`、`Check-Shell-Software.sh` | 自動執行 toolkit shell 案例後關窗，不證明原生輸入 |
| `Show-Profile.sh` | 顯示隔離路徑 |

每次 launcher 建立新的 `evidence/run.XXXXXX` 並保留 log；診斷環境變數會清除，保留正常的 IME 環境。診斷 launcher／影片 fixture 在 tar.gz；deb 只安裝主程式、桌面入口、說明及授權。tar.gz 不內附系統函式庫，需有 `package-report.json` 的 `runtime.depends` 所列套件。Winit／xkbcommon 使用 dlopen，不能只靠 ELF `DT_NEEDED` 推導；因此明列 X11／Wayland／GL runtime，包含 `libxkbcommon-x11-0`，不要求安裝開發 SDK。

## 已驗證範圍與待辦

第一版包僅由 `dpkg-shlibdeps` 推導相依，在 fresh Ubuntu 24.04 容器安裝後缺少 late-loaded xkbcommon-x11，GUI 失敗。已修正 Depends，以同一 ELF 重新打包；第一版完整包刪除，保留 hash／manifest 與失敗證據。

修正包實際安裝於 fresh Ubuntu 24.04 amd64 容器，以 QEMU／Xvfb software 執行；安裝 shell、含空格路徑的 portable shell、MP4／WebM／OGV 及草稿共六案 exit0，嚴格本機 peer 均收到 Close ACK。三格式影片核對像素及 session／render owner 清理。安裝清單沒有 WebKit／JavaScriptCore。Debian 的 software shell 與三格式影片另有分立結果。這些是套件安裝與模擬環境的 component 證據，不證明實體 Linux 視窗、X11／Wayland 原生操作、IME、聲音、GPU 或效能。

先前文章搜尋報 `background ran a pending query`。新增分階段診斷後確認：Ubuntu文章案例在送出測試編輯前已 `foreground=false`、`find_blocked=true`、輸入欄失焦；編輯被正確拒絕，draft仍無、timer未啟動、版本4及三筆舊命中完全不變。這份結果不能證明產品在背景重新搜尋，先前的產品失敗解讀撤回；獨特反例與來源／binary hash仍保留。

探針現先建立可接受編輯的synthetic toolkit foreground／field focus，立即確認已保留draft並清空舊命中，再停用，確認blocked／零命中／draft保留，最後恢復並完成搜尋。生產 `find_edit`、debounce與失焦保護完全未改。Debian與Ubuntu文章／終端模式共六案（含Ubuntu重跑）通過，逐案驗證receipt與Close ACK；新包重新安裝到原本的Ubuntu runtime root，Depends逐字不變，shell／relocated launcher／三格式影片／兩種尋找／草稿八案通過。這一輪是reinstall回歸，沒有冒充另一個fresh clean install。命令、診斷前後值、source patch、各binary hash及receipt見 `slint-find-pause/` 的 `ubuntu-diagnostic-results.json`、`ubuntu-fixed-results.json`、`package-acceptance.json` 與 `package-evidence/ubuntu-execution.json`；最初的失敗仍在舊批證據中。

以上搜尋只使用toolkit text／synthetic activation；未宣稱OS焦點、繁中IME或原生背景操作完成。八案中的草稿也未驗證新程序讀回，影片未證明聲音可聽；各receipt保留實際scope。

封裝以 `tooling/bundle_slint_linux.py` 消費已建 ELF、匹配的 build record、Skia 授權及三格式 CC0 fixture，不自行編譯或啟動 GUI。沿用 `tooling/bundle_release.py` 的 Linux layout／相依處理，既有 Dioxus 預設名稱及流程保留。Windows／Linux 共用 build-record 與 fixture admission。全150個Python測試（包含7個Linux打包案例）、文件、fmt、actionlint及diff gate通過；本輪Linux target release/all-target Clippy亦通過。完整 S8 仍依[總計畫](slint-evaluation-plan.md)驗收。

候選包的 `native_acceptance: not_run`、`source_delivery: not_ready` 與 unsigned 標示保留。建置 target、暫存 sysroot／磁碟映像、解包副本與驗收容器按批次清理；只保留最新包與最小來源／結果／獨特失敗證據，清理不等於產品驗收完成。

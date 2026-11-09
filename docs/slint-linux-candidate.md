# Slint Linux x64 候選包

此為 S8 內部驗收候選，尚未取代正式 Dioxus 入口。目標為 Ubuntu 24.04 x64，提供獨立名稱的 deb 與 portable tar.gz，包含 `native-player-probe` 主程式。尚未通過 Linux 原生桌面、繁中 IME、DPI、音訊、硬體 GPU、資料回退與效能驗收。

## 目前包與來源

檔案位於 Slint worktree 的 `.local/verification/slint-linux-candidate/package-current/`：

| 檔案 | SHA-256 |
| --- | --- |
| `PTTStella-Slint-Candidate-0.5.0-dev.3-linux-x64.tar.gz` | `e006df23f7c0c10d600b1808d7ac282538628fbd64eae23a1cc746610e31c0e2` |
| `PTTStella-Slint-Candidate-0.5.0-dev.3-linux-x64.deb` | `e944977208807a83afd383aeb83f6f0eb892bc7d2ef2cec23fe43764bd14b1c0` |

執行檔 SHA-256 為 `a7c00a9dc78abf3ea7598f91034e3b2b077be57729e6a09e3f439a051f3a888f`，91,524,800 bytes。Rust 來源為 `5046f2f6bfd220a3ee89309c550851f02c78045f`，沒有 Rust source patch；建置時打包 helper 的未提交變更與來源狀態另記在建置證據。來源 commit 不是完整第三方來源交付，`source_delivery` 仍為 `not_ready`。

使用 macOS arm64 的 Rust 1.97.0／Clang／Rust LLD 交叉編譯 Linux GNU x64，連結經逐檔 hash 核對的 Debian x64 sysroot；保持既有 release 最佳化，不使用主機 libmpv。ELF、`libmpv.so.2`、target loader 的實際相依解析及 release all-target Clippy 已核對。先前嘗試在 2GiB ARM VM 內以原生 ARM rustc 交叉編譯，最後的主程式 rustc 被 OOM kill；x64 rustc 的 QEMU 試跑亦未成功。這些不是 native Linux x64 編譯通過證據。確切命令、來源、套件版本、失敗與清理結果見同批 `mac-build-final.json`、`cross-clippy-exit.json`、`sysroot-inventory.json` 及 `oom-inspection.json`。

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

文章搜尋在背景暫停檢查失敗（`background ran a pending query`），沒有 Close ACK，保留結果，仍待追查。首輪測試把 DBus 放在 Xvfb 外，portal 因沒有 DISPLAY 產生警告；改成 `xvfb-run ... dbus-run-session -- ...`，以同一 ELF 重跑後，文章模式兩次仍在相同檢查失敗，終端模式通過獨立命中範圍、背景暫停、來源／PNG不變及關閉釋放，收到 Close ACK。因此不能把失敗歸因於 DBus 順序，也尚未證明是「背景重新執行」或「編輯未被接受而保留舊命中」；需在編輯接受與暫停邊界補診斷。原始七案結果位於 `slint-linux-candidate/ubuntu-final-evidence/ubuntu-execution.json`，三案受控重跑的命令／個別 exit／receipt 位於 `slint-linux-runtime-recheck/recheck.json`；重跑 wrapper exit0只表示完成紀錄，不能解讀為三案通過。

封裝以 `tooling/bundle_slint_linux.py` 消費已建 ELF、匹配的 build record、Skia 授權及三格式 CC0 fixture，不自行編譯或啟動 GUI。沿用 `tooling/bundle_release.py` 的 Linux layout／相依處理，既有 Dioxus 預設名稱及流程保留。Windows／Linux 共用 build-record 與 fixture admission。全150個Python測試（包含新增7個Linux打包案例）、文件、fmt、actionlint及diff gate通過；Linux target release/all-target Clippy先前已通過，本次沒有Rust變更或重建。完整 S8 仍依[總計畫](slint-evaluation-plan.md)驗收。

候選包的 `native_acceptance: not_run`、`source_delivery: not_ready` 與 unsigned 標示保留。建置 target、暫存 sysroot／磁碟映像、解包副本與驗收容器按批次清理；只保留最新包與最小來源／結果／獨特失敗證據，清理不等於產品驗收完成。

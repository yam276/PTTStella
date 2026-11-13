# Slint Linux x64 候選包

## 2026-11-13 2ef65ce 候選

Linux deb／tar.gz 在 `.local/verification/slint-sites-panel-candidate/linux/`。已核對 AMD64 套件、共用檔案及兩包主程式，deb 共 1,111 檔，tar.gz 共 1,124 檔。deb SHA-256 為 `33f36d0562188f10fd266deb12fa11d0ebdfc0e440cd8e3a2ee4e83bccfec1c4`，tar.gz 為 `7f0bd6909b5b1b0cae9fb1779e97b67dfaf72cd9dbd7e3558046279f36fcb215`。

用便攜包內 ELF 在 QEMU／X11 下以 XTest 操作 1,000×690 視窗，選取 GBK 與 Big5-UAO 並各點擊一次「連線切換」。同一 TCP 位址的三次連線均依序關閉，站端收到零輸入 bytes，程序 exit 0。收據在 `native-x11-result.json`。Linux x64 實機待驗。

同包在 QEMU／Xvfb 完整重跑 14 項軟體繪圖元件測試，每案各一次 Close ACK。涵蓋草稿保存與重開、主視窗、文章及終端搜尋、長文與取消續跑、文章輸出、設定搜尋、推文更新、收藏及 MP4／WebM／OGV。首輪在 11 案後遇到 Podman EOF，保留 `linux-components-first.log`，重跑收據在 `linux-components/summary.json`。兩輪客體暫存已清理，容器及虛擬機已停止。

以同一便攜包執行檔在隔離 X11 用滑鼠右鍵開啟文章選單，點「更新推文」啟用，完成兩輪追加後顯示新增 6 則。重開選單可見勾選，保持開啟 5.4 秒沒有下一輪站端請求。再點同一列取消，勾選消失。按 Esc 回到文章，後續 5.4 秒沒有再送請求。站端核對全部 19 個取頁及刷新步驟並收到唯一 Close ACK，程序 exit 0。

首輪校準只開啟選單，關閉時站端仍等待啟用指令。另一次在首次刷新進行中嘗試重開選單，未取得有效的已勾選畫面，該輪停止於站端第 8 步。保留校準和中斷日誌。有效結果、前後截圖及客體暫存清理位於 `.local/verification/slint-x11-interactions/`。本輪使用 QEMU／Xvfb／Openbox 與軟體繪圖，Linux 實機與 Wayland 待驗。

### 浮球與設定的 X11 操作

同一便攜包以 XTest 點開右側浮球，看到五個分類，再用 End／Enter 開啟主視窗內的設定。輸入 `privacy` 並點搜尋結果可開啟帳號與隱私。圖片與媒體的文章模式下拉選單可用 Esc 先收合，重新展開後選取「終端機」。關閉再開啟設定，畫面讀回「終端機」，關窗收據也讀回存檔中的 `article_mode: terminal`。

把浮球拖到左側後，扇形改向視窗內展開。進入導覽後用左方向鍵回扇形，再按 Esc 收合。視窗縮成 840×560 後，浮球保持左側停靠，仍可展開及收合。站端只收到 Close 並回覆，程式正常退出，設定儲存沒有留下待處理工作或子視窗。畫面、日誌與清理收據在 `.local/verification/slint-x11-tools-settings/`。

校準輪在導覽子選單按 Esc 時已收合整個浮球，再按一次 Esc 後合成站回報收到額外輸入。當輪未記錄該筆內容。正式重跑以左方向鍵返回扇形，Esc 關閉。QEMU 下的畫面和鍵鼠結果保留為 X11 證據，Linux 實機、Wayland、無障礙焦點與動畫時序待驗。

## 2026-11-13 d047e55 候選

Linux deb／tar.gz 在 `.local/verification/slint-windows-refresh-rate/linux/`。已核對 AMD64 套件、共用檔案及兩包主程式，deb 共 1,111 檔，tar.gz 共 1,124 檔。deb SHA-256 為 `0ca717e1a31791f2b7bdf7084a6c71184f07b994b2a6adfb2691f3c0bdc83251`，tar.gz 為 `40398732e4622784a7b480bab76406a30a347211ba85d3af78c43ea7cbc305c4`。Linux 實機待驗。

便攜包的 ELF 在 ARM Podman／QEMU x64／Xvfb 跑完 14 項軟體繪圖元件案例，全數正常退出且各收到一次 Close ACK。涵蓋草稿保存與跨程序重開、主視窗、兩種搜尋、長文與取消續跑、複製／PNG、設定 48 個定位、推文更新、收藏及三種影片格式。收據位於同一驗收目錄的 `linux-components/summary.json`。

## 2026-11-13 96e5159 候選

Linux deb／tar.gz 在 `.local/verification/slint-settings-focus-mount/linux/`，與其他三架構同取乾淨 `96e5159`。核對 AMD64 套件資料、主程式、共用內容及 1,111／1,124 檔。deb SHA-256 為 `58fa4c83a242e3c3ef539424b75c80294b91739dcf4b03f2fc4924dea45705af`，tar.gz 為 `2c8c0a17cfbf83853c637d4a587234c6dd20fd72e208256796c13437ed66abd2`。同一修正程式在交付前的 QEMU／Xvfb 設定搜尋回歸通過六輪。此包 Linux 實機待驗。

使用交付便攜包的實際 ELF（SHA-256 `bc92000501a8ec04ed2c767cfb18c9bcfa850f7d52684435f6b12f525020fe6f`）在 ARM Podman／QEMU x64／Xvfb 軟體繪圖跑完 14 案，全部 exit 0 且收到唯一 Close ACK。設定搜尋一輪完成 48 個中英文目標。長文 837 行／37 筆、取消續跑 441 行／19 筆、草稿跨程序重開及帶回、推文追加、收藏、文章搜尋與輸出、主視窗和三格式影片均通過。收據及暫存清理見 `.local/verification/slint-settings-focus-mount/linux-components/` 與 `linux-components-cleanup.json`。實機安裝、Wayland、IME、音訊裝置及效能待驗。

## 2026-11-13 57e000c 候選

Linux deb／tar.gz 位於 `.local/verification/slint-passive-window-candidate/linux/`，與其他三架構同取乾淨 `57e000c`。沿用 Debian bookworm amd64 SDK，核對 3,419 份 SDK 檔案及連結。已核對 AMD64 套件資料、兩包主程式、共用內容與 1,111／1,124 個檔案。deb SHA-256 為 `f8a4764500de5f97b27430b68cf78923928c8757367409978d477052a3bdc045`，tar.gz 為 `e36ff35e186c1f5a6a18eb4158aab3d9d83c039575bf0df1fe0a22d8192010ab`。Linux 實機待驗。

在 ARM Podman 的 QEMU x64、Xvfb 與軟體繪圖執行原便攜包，ELF SHA-256 為 `0baa77bbfcf59d05c4a42267a459faeb66f636244a3059008d714107cf95d86b`。草稿保存／跨程序重開、主視窗、兩種閱讀模式搜尋、長文／取消續跑、文章輸出、推文追加、收藏及三格式影片共 13 項首輪成立。收藏的初步彙整誤要求通用 `status` 欄位，後續依實際格式核對 45 列、雙色樣式、儲存／恢復、關閉與 worker join。設定搜尋首輪未能聚焦繁中防閒置欄位，重跑同一 ELF 後通過中英文 48 項。曾有一次 Podman 斷線中止重測，未取得該輪結果，已清理部分提取檔。14 項有效結果皆有唯一 Close ACK。首輪資料、重測及彙整在 `.local/verification/slint-passive-window-candidate/` 下的 `linux-components/`、`linux-settings-followup/` 與 `linux-components-validated.json`。原生 X11／Wayland、IME、音訊裝置及效能待實機驗收。

## 2026-11-13 草稿工具列候選

保留 `.local/verification/slint-draft-history/linux/` 的 deb 作版本對照。與其餘架構同取乾淨 `b57eae8`。當時通過 AMD64 套件資料、主程式、共用內容與 1,110／1,123 個檔案核對。deb SHA-256 為 `5f637bfdc7c92d7c86d3a446818f36e95a89526c84fe27ce9337d200918cee53`，後續候選備妥後移除本版 tar.gz。

## 2026-11-13 草稿與文章被動驗收候選

Linux deb／tar.gz 位於 `.local/verification/slint-passive-drafts-reading/linux/`。從僅比 `675b150` 多三份文件變更的乾淨 `4f95094` 交叉建置，沿用既有 Debian bookworm amd64 SDK 並核對 3,419 份檔案與連結。核對 AMD64 套件資料、deb 的 1,110 份檔案、tar.gz 的 1,123 份檔案，以及兩包的主程式與共用內容。deb SHA-256 為 `e5d398e97c590957ea5b00847ab971d7bfa81836b252b1f0bcd60db20ef2eb2a`，tar.gz 為 `05027cff204c0a090d0f880c51e54faf92a9684db792fd30ecc9e6f6d770f168`。Linux 實機驗收待補。

## 2026-11-13 文章逐行對齊候選

Linux x64 deb／tar.gz 在 `.local/verification/slint-article-arrow-alignment/linux/`。從僅有文件差異的 `04191e1` 交叉建置，產品程式對應 `9376627`。AMD64 deb 的 1,110 份檔案及 tar.gz 的 1,123 份檔案已核對，兩包主程式、共用授權與說明一致。deb SHA-256 為 `eea7c26230cc73654daff43cb717dd0fd66e6273efebb7ae72227323ed4e4314`，tar.gz 為 `59bfad2fdacebfeb5c780a17a7ccbfd7dac3fb150ec52a3ac8a24f9ab4aec47b`。此版原生驗收待補。

## 2026-11-13 收藏載入焦點修正候選

Linux x64 deb／tar.gz 在 `.local/verification/slint-cross-bookmark-load/linux/`。與 `4d7d174` 只差三份文件的乾淨 `ebc1035` 工作樹用固定 Cargo target 及既有 Debian bookworm amd64 SDK 建置，逐一核對 3,419 個 SDK 檔案與連結。AMD64 deb 的 1,110 個檔案與便攜包的 1,123 個檔案已核對。兩包主程式、共用授權與說明一致。

| 交付 | SHA-256 |
| --- | --- |
| Linux x64 deb | `bb267c421c3b86ad3d0792a20cbf9ec9a581efbff44436b6fccab315ff8d5f25` |
| Linux x64 tar.gz | `32a905a7ee823abd0ea497be51038803c8f2e5484722469eb823822a9e13e315` |

QEMU／Xvfb 軟體繪圖執行草稿保存、草稿重開、主視窗、收藏和 MP4／WebM／OGV，共七案通過及收到嚴格 Close。收藏的首次呼叫在參數檢查退出，改用既有媒體探針入口後驗證保存與 worker 退出。這輪收據與清理路徑在 `slint-cross-bookmark-load/`。實機安裝、Wayland、原生 IME、聲音和效能待驗。

## 2026-11-12 草稿組字修正候選

Linux x64 deb／tar.gz 位於 `.local/verification/slint-cross-ime-edge/linux/`。乾淨 `06487df` 與 Apple Silicon／Windows x64 候選 `4e28e82` 之間只有文件變更。沿用既有 Cargo target、Apple Clang／ELF LLD 及 Debian bookworm amd64 SDK，重新核對 SDK 的 3,419 個檔案與連結。

| 交付 | SHA-256 |
| --- | --- |
| Linux x64 deb | `35f4fcb134981c0cae8dab5d68ae4c0b855fc82585a1f8f5328d324a45134a81` |
| Linux x64 tar.gz | `e998093b491565c511ab4da667f69662dc3b63ee0ad249bbb2953e85f2209fa3` |

AMD64 deb 的 1,110 個檔案及 portable 的 1,123 個檔案已核對，共用主程式 SHA-256 為 `e098b75611f635335c2065bc76c34c41446fbfcc83c0d66541bd201017c55ec0`。從 portable 在 QEMU／Xvfb 的軟體繪圖環境完成草稿、重開並帶回站端、主視窗及 MP4／WebM／OGV 六案，全部正常退出並收到嚴格 Close。影片核對音量、靜音回覆和資源退場。這輪涵蓋 deb metadata 及 portable 執行，實機安裝、Wayland、原生 IME、聲音和效能待驗。

## 2026-11-12 含原生影片控制的候選

這輪 Linux x64 deb／tar.gz 在 `.local/verification/slint-cross-audio-current/linux/`。以乾淨 `88b52af42b895db85d2848b70c0b516a67823f24` 在 macOS arm64 上交叉編譯，使用 Rust 1.97.0、Apple Clang、Rust 工具鏈的 ELF LLD 與既有 Debian bookworm amd64 sysroot。SDK 的 3,419 個檔案及連結均核對。ELF SHA-256 為 `e801538bb86ebc4fd3211ba196877e26707ea02e49ab906377a25029a9654fe8`。

| 交付 | SHA-256 |
| --- | --- |
| `PTTStella-Slint-Candidate-0.5.0-dev.3-linux-x64.deb` | `88a765e3b3fe96d8e2042c340747587076a29122fed6d13ce8fb4c5bbcd22e15` |
| `PTTStella-Slint-Candidate-0.5.0-dev.3-linux-x64.tar.gz` | `bd5960a8b00fe6a879502df6308d3c9719eab74e41c9baa1df519a5f10e68c07` |

在既有 ARM Podman 容器以 x64 loader 解析依賴，`dpkg-shlibdeps` 取得 Depends。未發現 WebView／WebKit 的載入依賴。逐檔核對 deb 1,110 項及 tar 1,123 項，兩包共用的執行檔、授權和 README 一致。以 QEMU／Xvfb 的軟體繪圖執行 portable 包的 MP4、WebM、OGV，三案均 exit 0，取得音量／靜音回覆、播放器退場與嚴格 Close。這次驗收涵蓋 deb metadata 和 portable 執行。Linux x64 實機、Wayland、原生 IME、實際聲音、乾淨安裝及效能待驗。

## 2026-11-11 候選

2026-11-11 候選位於 `.local/verification/slint-ui-cjk-fallback/linux-package/`。ELF `68601365…` 以乾淨 `ff0ae84` 交叉編譯，含文章焦點與固定狀態列探針修正。release、all-target Clippy、loader／dpkg 及 tar 1,122／deb 1,109 個 payload 均已核對。deb 重裝與免安裝包的軟體繪圖完整視窗檢查通過，預設繪圖在 QEMU 下仍遇到通知時序失敗。安裝執行檔也通過 X11 背景零取頁與取得焦點後 37 次取頁的案例。[本輪封裝驗收](slint-evaluation-plan.md#2026-11-11-固定狀態列探針的-windows-與-linux-封裝)

較早的 `af476061` ELF 通過 20 項合成案例，包含兩項針對性重測。歷史結果見[文章焦點同步](slint-evaluation-plan.md#2026-11-11-文章焦點修正的-windows-與-linux-候選同步)。QEMU 結果的執行環境為 Xvfb／Openbox，實機、Wayland、原生 IME、音訊和效能待驗。[來源還原](slint-product-source.md)選 `shell-layout-cross-current`。

此為 S8 內部驗收候選，尚未取代正式 Dioxus 入口。目標為 Ubuntu 24.04 x64，提供獨立名稱的 deb 與 portable tar.gz，包含 `native-player-probe` 主程式。尚未通過 Linux 原生桌面、繁中 IME、DPI、音訊、硬體 GPU、資料回退與效能驗收。

## 先前候選與來源

當時檔案位於 Slint worktree 的 `.local/verification/slint-linux-candidate/package-current/`：

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
| `Run-Video.sh`、`Run-Video-Software.sh` | 自製四色 MP4、播放／暫停、音量／靜音、拖曳與關閉 |
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

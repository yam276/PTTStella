# Slint macOS x64 建置與待封裝輸入

完整 `native-player-probe`／`pttstella-slint` 主程式已交叉編譯為 x64 Mach-O，並在這台 Apple Silicon Mac 的 Rosetta 下通過分立 component 測試。尚未產出可移交的 App／ZIP／DMG，Intel 實機、OS 鍵鼠／IME、可聽音訊及效能仍未驗收；此紀錄不替代[總計畫](slint-evaluation-plan.md)的 S8 門檻。

## 目前輸入與來源

保留位置為 Slint worktree 的 `.local/verification/slint-macos-x64-candidate/`：

- `pending-current/pttstella-slint`：唯一待封裝 CLI，SHA-256 `b1e810584025eb0d4fc65992045c7eb99ae5b25bdd0e4e9bbed6831453731e64`，69,874,676 bytes。
- `runtime-input/`：該 CLI 必要的44個x64 dylib，59,601,056 bytes，另有指向同目錄的 `libmpv.dylib` 捷徑。
- `build.json`：Rust來源 `d37190d`，Rust source patch為空，macOS arm64的Rust1.97.0／Clang交叉編譯；完整命令與環境在此。
- `main-inspection.json`、`component-results.json`、`component-acceptance.json`：架構、隔離profile、逐案命令、exit、Close ACK及個別receipt。
- `pending-current/status.json`、`packaging-readiness.json`：封裝仍為 `not_ready`，完整來源與notice provenance未完成。

此 CLI 仍有指向同批 `runtime-input` 的本機開發用 RPATH，不是可攜交付，不應直接移給別台機器。正常無參數啟動會連研究設定的預設站、不自動登入；離線可用 `--verify-shell NEW_DIR` 或 `--probe-grid`，只連本機合成服務。`--show-profile`只列研究路徑，不讀寫store／vault。封裝應重用保留的CLI，Rust來源不變時不重建。

建置明示 `CARGO_INCREMENTAL=0`、dev／test debug=0，保持release最佳化；target linker為Apple Clang，`MPV_LIB_DIR`明示x64輸入，不使用主機arm64 libmpv。`MACOSX_DEPLOYMENT_TARGET=11.0`是此輪編譯請求，不代表已實測支援macOS11；實際執行環境為macOS15.7.7 arm64／Rosetta。

## Runtime 選擇與尚缺資料

目前Homebrew mpv0.42.0 API沒有macOS Intel bottle。[官方公式資料](https://formulae.brew.sh/api/formula/mpv.json) 已保存於同批證據，不能將arm64 bottle拿來連結x64。[mpv官方0.42.0 Intel ZIP](https://github.com/mpv-player/mpv/releases/tag/v0.42.0) 經GitHub asset digest核對後，實際只有單一CLI，沒有libmpv或可重用dylib；完整ZIP已移除，保留雜湊及成員表。

這輪改查[IINA官方v1.5.1](https://github.com/iina/iina/releases/tag/v1.5.1)簽章套件，其DMG SHA-256為 `f414507ec60bcfeda3a9f539f74470253e31b619d031e7a9e9a71519da1598cd`。核對GitHub asset digest、唯讀掛載與完整App簽章後，只收集libmpv實際可達的函式庫，取x64 slice並ad-hoc簽名；沒有啟動或複製IINA App。原始／thin／prepared hash及相依圖在 `runtime-input.json`；完整DMG與掛載已清除。

實際C API初始化回報mpv0.41.0、FFmpeg9.0.1、client API131077；FFmpeg的 `avcodec_license()`／`avutil_license()` 都回報GPL version 2 or later。這些是該二進位的觀測值，不以IINA Credits中的通用LGPL文字代替完整授權判定。[IINA dependency recipes](https://github.com/iina/deps-buildscripts)的版本／checksum及CoreAudio補丁已保存，但截至IINA釋出日的recipe寫FFmpeg9.0.2／little-cms2 2.19，與實際函式庫資訊不一致，不能宣稱該recipe就是此binary的精確來源。此provider與arm64候選的Homebrew mpv0.42.0不同；來源及完整notice清單需繼續核對，不以Rosetta通過就宣稱四架構runtime等價完成。

共用collector現在明確拒絕沒有來源／notice owner的非Cellar相依，已用這份實際runtime驗證拒絕且不產出空清單。下一步需完成外部runtime的明確來源／notice admission及收集，或改用能完整交付對應來源的runtime；不偽造Homebrew receipt、不跳過notice檢查。之後才能將這份CLI與相依搬進App、移除開發RPATH，再產出ZIP／DMG和封裝後回歸。

## Loader 修正與證據

原collector只查每個dylib自己的RPATH，會拒絕實際可由dyld父層run-path chain載入的 `libmpv → libass`。同一個x64 C probe已實際執行成功；舊collector仍報unresolved，新collector取得45個Mach-O（CLI加44個dylib），red／green在 `loader-red-green.json`。現在先以各owner的上下文展開 `@loader_path`，再傳給子相依，保留allowed-prefix及symlink escape檢查。

搬移後只由主執行檔擁有 `@executable_path/../Frameworks`，dylib繼承該路徑；不重複塞入每個dylib。原本把長絕對路徑寫進IINA的LuaJIT會超過Mach-O load-command空間，反例在 `input-relocation-failure.json`；目前保留其原始 `@rpath` identity。新增macOS實際Clang／dyld fixture驗證兩層dylib搬移後仍能載入，另有可在其他host跑的三層路徑上下文、繼承時越界拒絕及unknown notice拒絕案例。這些是loader／封裝測試，不是Intel硬體或產品OS輸入驗收。

## 此輪測試範圍

軟體與OpenGL各驗shell、文章／終端尋找及MP4／WebM／OGV，另有軟體草稿，合計13案全部exit0、Close ACK，逐一核對receipt。影片有非空四色像素樣本、render退休及session join；尋找有接受Pending、停用保留draft、恢復、來源／PNG不變及索引釋放。草稿未測新程序還原，所有案例都只用toolkit事件與synthetic foreground；未驗OS IME、原生鍵鼠、可聽音訊或效能。

另以DYLD載入追蹤執行shell：觀測到六筆System WebKit／JavaScriptCore相關映射，並同時有WritingTools／WritingToolsUI。不能據此宣稱執行期零WebKit，也未由這份trace證明其呼叫成因、WebView實例或完整IME流程；產品內容與互動的路徑仍須依S8證據分辨。確切映射在 `dyld-audit.json`，原始trace保留。這組Rosetta／不同runtime的OpenGL通過，不結案先前arm64 MP4偶發錯色。

清掉Cargo target、卸載並移除IINA DMG後，用保留CLI與自有相依重跑MP4 OpenGL，像素／退出／Close ACK通過，證明此案例不依賴已移除的編譯或掛載路徑。結果在 `post-clean-acceptance.json`。x64 release/all-target Clippy、154個Python測試及相關gate依實際紀錄列入總計畫；不把此輪component結果換算成整體完成度。

## 產物清理

本輪Cargo原生完整clean移除8281檔／2.5GiB，target du2634744KiB；完整IINA DMG／掛載、已完成C probe執行檔與自有pycache已移除。17張byte-identical generated PNG改以保留路徑與SHA映射，原始碼、獨特失敗、必要資料及目前各平台候選保留。10497份受保護檔案hash未變，精確盤點與映射見 `cleanup/ledger.json`。

只保留一份待封裝x64 CLI與必要函式庫，不留編譯profile；本批約130928KiB，Slint worktree約1020184KiB。filesystem available 218310406144→221069762560 bytes是並行期間觀測值，不全部歸因於本批刪除。Rust x64 target為已安裝工具，registry／既有SDK與不明的共用系統cache保留。清理後未重建Rust產品；Python gate的獨立C／rustc fixture使用TemporaryDirectory自動清理，沒有重建Cargo target。

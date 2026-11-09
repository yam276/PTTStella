# PTTStella Slint 平替實作與驗收計畫

從既有 Rust 終端核心和 macOS Slint 原型出發，做出能讀寫現有 PTTStella 資料、保留目前操作並可安裝在 Windows、macOS、Linux 的原生桌面候選版。以實測決定是否將 Slint 改為正式入口，再移除 Dioxus 與 WebView 發佈相依。

Dioxus 初始基準為 `5ea9ad6a498331d0cbebc7d9803e0a51df46e5c3`，已包含分段接續長文、大型 GIF 與選用的自動開圖。Slint 起點為獨立 worktree 內的 `slint-research`／`pttstella-slint` 原型。原型已在 macOS arm64 實測雙色字、80×24、放大拖曳、全螢幕及獨立輸入欄的威注音。實體 Retina、終端內組字、跨平台原生輸入和與 Dioxus 的同負載效能比較仍待驗。實驗數據及已知差異見 [離線原型紀錄](slint-prototype.md)。

## 目前進度（2026-11-10）

**尚未達到可平替 Dioxus 的驗收條件。** 現有 Slint 來源及計畫已提交，正常影片退出以 `1b2bb96` 修正。下表區分已接線與尚須完成的驗收；歷次結果及確切 scope 見 [原型紀錄](slint-prototype.md)。

| 階段 | 目前已有 | 尚須完成 |
| --- | --- | --- |
| S0 | 共用 Rust 服務、獨立 Slint 入口及可編譯提交序列；四 target 的一般／播放器兩配置 Cargo 依賴 gate | 完整功能案例對照、各平台連結／runtime 依賴與最終基準重跑 |
| S1 | 格線／雙色字／縮放、macOS 原生輸入的分立證據 | 候選窗與跨螢幕 DPI、可及性、Windows／Linux 原生 |
| S2 | 共用 Connection、登入／保活、站點表單、嚴格切站／雙編碼 bytes | 同 endpoint 編碼切換、預設站重啟／不登入、完整信任／vault 原生矩陣 |
| S3 | 選取、輸出、浮球／選單及原生互動的分立證據 | 全部快捷鍵／pointer／modal／可及性操作矩陣 |
| S4 | 雙閱讀模式、長文取頁、尋找、名單／右鍵／揭露／底色、書籤／離線、快速前往與最近看板、目前／指定列表文章收藏、共用推文索引／原生定位與側標、推文更新／未讀／保留與復原；macOS基本按鈕、列表右鍵指定收藏與新程序斷線讀回補驗 | 跨平台推文控制／定位與完整鍵盤／焦點矩陣、其他平台列表右鍵及新程序離線操作、混合 Live 等效與全功能回歸 |
| S5 | 圖片／GIF／文內展圖、主視窗影片；正常關窗／Quit 的 GL 清理；原生時間軸修正；失敗像素／播放狀態與Close audit、三格式錯色負向對照 | MP4 GPU 像素偶發失敗仍未重現／解決、多圖峰值、GL 恢復、音訊、四架構可交付 runtime |
| S6 | 原文編輯／色票／站端上色、取消／部分失敗、草稿故障與多開復原 | 原生 IME 候選／完整輸入邊界、跨平台操作；舊末字異常原因仍未定 |
| S7 | 原生設定八分類、語言／站點／名單、24項搜尋定位、具名設定檔與正常退出；[共用版本檢查／下載入口](slint-release-check.md)、toolkit空白鍵及退出HTTP／listener join；三版本store往返28案 | 通知成功淡出／錯誤優先、其他管理入口、完整Dock／modal／OS I/O與可及性矩陣、實際前端／平台路徑及vault往返 |
| S8 | S1 Windows 探針；macOS arm64 主程式候選 App／ZIP／DMG及無Homebrew sandbox；Windows x64含libmpv的GUI候選與ZIP核對；[Linux x64完整deb／tar.gz](slint-linux-candidate.md)、Ubuntu安裝／software八案；[macOS x64 App／ZIP／DMG](slint-macos-x64.md)、自建47項runtime閉包／32份來源、兩開發prefix不可用的Rosetta13案 | Intel／Windows與Linux原生執行、IME／DPI／音訊、VC runtime／最低OS／完整乾淨安裝、完整產品來源交付與重播、資料回退及同負載效能；尚未通過 |

接續完整鍵盤／焦點與其他平台、跨平台列表／重開、混合功能／退出／資料往返案例及完整候選打包。使用者已確認可在 Windows 執行驗收包。缺少 runner 的項目保持 `not_ready`，不將 macOS 或 toolkit 結果換算成跨平台完成度。S0–S8 goal 持續；S9 正式入口與發佈尚未執行。

## 執行方式與完成條件

- 實作在指定 Slint worktree 內連續推進。依下列 S0–S8 的相依順序完成程式、回歸和證據。缺 runner 或硬體時登記確切缺項，繼續可獨立驗證的階段。若原生輸入、雙色格線或原生播放器證明不可行，先修正或換成可驗收的技術路徑，再擴大相依功能。階段證據整理完即可接續，日常實作與重跑沿用本計畫。
- 每階段做成可獨立驗收的變更，遵守 [AGENTS.md](../AGENTS.md) 的分支、提交與清理規則。需要新分支、推送、實站操作、切換正式入口或發佈時，依本 repo 規則和使用者當時授權執行。測試與研究 App 使用獨立 App ID、設定／資料目錄、診斷及密碼庫服務，合成探針只連本機。
- 候選版須通過所有既有功能、四個目標架構安裝、舊資料讀寫與回退，以及下文效能門檻。S8 交付可審閱的候選包、可逆切換方案與結論。使用者接受後才進入 S9。若 `develop` 推進，切換前以待整合的實際 commit 重跑變更相關項目和整體效能，新增功能納入驗收。

## 產物保留規則（2026-11-09 使用者要求）

已測完且被取代的產物包含過去累積與之後新增，均於每個可獨立驗收批次完成時清理，沿用 [task-owned 清理流程](development-harness.md#產物清理與結案)。不再為每階段各留 App，也不因完整 goal active 就長期保留 target/debug／release；只在同一批工作中重用。一般 dev／test 命令明示 `CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0`，release 設定不變。已確認無使用者的編譯快取用 Cargo 原生 clean，必要候選包與最小來源／驗收／失敗證據移出 target，保留原因及 exact removed paths 另記。不啟動背景刪除服務。

2026-11-10 使用者再次要求即時清理。本次沒有啟動新建置或宣稱新功能驗收：先將已完成建置、尚未驗收的通知焦點探針移出 target，只留 `slint-notice-focus/pending-current/` 的單一 CLI、來源 patch 與 hash，再以 Cargo 原生 clean 清掉 `slint-notice-focus/target/release`（5679檔，Cargo宣告2.0GiB；target的du由2128852KiB降至8KiB）。另移除735張 byte-identical 的歷次 generated PNG 副本（logical 65,577,981 bytes／allocated 67,096,576 bytes），每張原圖的保留位置與SHA記在 `slint-artifact-cleanup-20261110/removed-duplicate-pngs.json`，歷次報告的已刪圖片路徑可由此追溯；所有獨特影像保留。已停止的 Dioxus 對照App專用系統暫存也移除。最新macOS候選改以 `slint-shell-chrome-final/package-current/` 為準，Windows S1包、Dioxus對照App、原始媒體／必要授權與測試資料均保留；1562份受保護檔案hash未變。所有已知Slint target只剩8–16KiB metadata，worktree的du為778964KiB。filesystem available 231056789504→233583538176 bytes僅為觀測，不能全歸因於本次刪除。exact removed paths、owner／resolved path／symlink／lsof、Cargo與du紀錄見 `.local/verification/slint-artifact-cleanup-20261110/ledger.json`；canonical與其他聊天namespace因歸屬不同或不明保留。未整合worktree與待驗來源修改未動。

2026-11-10 原生 runtime 批次中途清理：確認沒有編譯程序或選定目錄 open files，重新驗證30份原始套件與3份bootstrap工具來源archive的SHA，另將libjxl／libplacebo含submodule的來源逐檔封存核對；保存131份必要config／build診斷及mpv Swift MetalLayer失敗。移除36個精確目錄：32份解壓／Git來源副本、3份工具解壓副本與本批recipes/build，中間產物du合計1658992KiB；exact paths、owner／symlink、程序與保留雜湊在 `.local/verification/slint-artifact-cleanup-20261110-source/ledger.json`。12975份保留檔案hash不變。filesystem available 219806351360→221484134400 bytes是並行觀測；來源封存本身的新增空間與du分開記錄，不能全歸因於刪檔。保留31項已完成的runtime依賴install prefix供僅重試mpv、約22MiB臨時工具、最新各配置交付與最小證據；工具／install待本批用完後清理。兩個Cargo target沒有build profile，未重建Rust；共享SDK／registry／VM與其他聊天產物未動。系統ThemeWidgetControlViewService仍持有已刪舊探針exe，未為清理停止系統程序。首次保留清單遇到APFS來源目錄大小寫別名，已依刪前inventory逐筆核對並另記，不宣稱遺失來源或提前通過runtime驗收。 文件gate另抓出臨時venv附帶的Windows launcher；已刪除用完的pip bootstrap，未放寬gate，重跑通過。另記5個exact移除路徑，最終12086份保留檔案hash一致，Slint worktree du1435912KiB。

後續每批開建置前先檢查上一批target與待驗CLI，來源不變時優先重用保留的單一執行檔。中途收到清理要求也立即清理已停止的profile；批次完成後移除被取代的CLI／App／ZIP／DMG及重複正向證據，保留最新必要交付和獨特失敗。只為清理或更新本紀錄不重建Rust，不把待驗當passed，也不因goal持續而累積歷代binary。

2026-11-10 Linux 批次中途再次執行即時清理：兩個專屬測試容器 `pttstella-slint-linux-native-base-20261110`／`pttstella-slint-linux-ubuntu-final-20261110` 已匯出結果，核對 ID／task label、無 bind mount 及僅有自有測試 helper 後正常 stop／rm；移除其約3.66GiB可寫層與無使用者的臨時 Ubuntu image。刪除缺少 late-loaded runtime 相依的第一版 Linux deb／tar.gz（du69512KiB），保留小型 manifest／hash／失敗紀錄；修正相依的同一執行檔候選由 `package-runtime-fixed` 移至 `slint-linux-candidate/package-current`，內容hash不變。先前該批 guest Cargo clean 為6622檔／2.4GiB，host Cargo clean為9790檔／3.1GiB，host target、暫存case-sensitive SDK映像、link wrapper及smoke binary均已清除；本次沒有重建Rust。另刪除已完成 settingsbaseline 對照的兩個專用 WebKit cache。exact paths、owner／resolved path／symlink、容器／程序盤點與保留例外見 `.local/verification/slint-artifact-cleanup-20261110-immediate/ledger.json`，前段Cargo證據在 `slint-linux-candidate/compile-cleanup/ledger.json`。

共用 VM 不關機、不 compact、不刪磁碟；確認 discard 支援後執行線上 TRIM，稀疏磁碟 allocated bytes 21678956544→11832786944，host filesystem available 212732891136→222590758912 bytes。TRIM 回報99.5GiB是discard範圍，並非釋放量；可用空間約增加9.18GiB包含先前guest已釋放的區塊及並行活動，不能全歸因於本次刪檔。Slint所有target只剩8–16KiB metadata。最新必要候選、Dioxus對照、來源／資產／使用者資料／獨特失敗保留；既有Windows SDK／工具約1.11GiB、shared Skia registry source約966MiB及其他聊天／來源不明cache不刪，未整合worktree與尚未提交的Linux打包實作保留。

清理不代表Linux驗收通過：修正runtime的候選在fresh Ubuntu24.04 x64／QEMU／Xvfb software下，安裝shell、含空格路徑的portable launcher、三格式影片及草稿共6案exit0且收到Close ACK；文章搜尋因「background ran a pending query」失敗、沒有Close ACK，證據保留於 `slint-linux-candidate/ubuntu-final-evidence`，尚待診斷。所有原生硬體／IME／GPU／音訊及效能結論仍待驗，候選報告維持not_run／not_ready；後續可由保留包重現，無須保留完整測試環境。

2026-11-09 已回收 main Slint target 的 dev／release、獨立 Windows 交叉編譯 target，以及34份過時 macOS 測試 App。當前只保留 Slint 一般候選、影片配置候選及唯一 Windows S1 ZIP；兩個最小 reopen bundle metadata 目錄沒有 native executable。被刪 App 的非執行紀錄／授權／測試資源壓成約3.2MB ZIP，完整 binaries 不存進 archive。精確清單、來源雜湊、lsof、Cargo 輸出與空間前後在 `.local/verification/slint-artifact-cleanup-20261109/`。Slint target 12KiB、verification 約416MiB；原始碼、未提交的推文導覽工作及必要交付內容不變。canonical checkout 的其他聊天產物不屬於本 Slint goal 所有權，保留而未跨案刪除。

後續三版本 store 往返批次驗收後，另清掉 `.local/verification/slint-store-roundtrip/target/release`、`drivers/candidate`、`drivers/baseline`、`drivers/release`，並正常移除 `checkouts/baseline` 與 `checkouts/release` 兩個臨時 worktree。Cargo 宣告6932檔／1.8GiB；filesystem free 由238467133440增至240648839168 bytes，這是並行工作期間的觀測值，不全歸因於本次清理。28案例結果、唯一失敗資料、來源 patch、各 ref／driver hash 均保留，原始碼與必要交付 hash 不變。主 target 與這批獨立 target 各12KiB，整個 Slint worktree 約497MiB（`.local` 約441MiB、Windows ZIP 約28MiB）。目前保留的一般候選為 `slint-native-list/Native-Bookmarks.app` 及其同檔重開所需 `native-state/bookmarks.json`，影片候選仍為 `slint-app-exit/Native-Video-Close.app`，Windows S1 ZIP 未更動。精確清單／du／Cargo輸出／空間前後記在 `.local/verification/slint-store-roundtrip/cleanup/ledger.json`；版本對照臨時 checkout 與 headless driver 納入往後每批清理，不保留可重建執行檔副本。

主程式打包批次後，最新 OpenGL／影片配置改保留 `.local/verification/slint-main-bundle/package-current/` 的 App／ZIP／DMG（約271MiB），取代並移除舊 `Native-Video-Close.app`。一般軟體繪製配置的 `Native-Bookmarks.app`、其合成收藏檔與唯一 Windows S1 ZIP 仍有必要，繼續保留。另清掉本批2.6GiB release profile及三個已失敗／被取代的 package目錄，沒有留舊執行檔或完整ZIP／DMG副本；精確路徑、source／交付hash、du及free前後在 `.local/verification/slint-main-bundle/cleanup/ledger.json`。本批target剩8KiB metadata，不因 MP4 重複性缺項而繼續保留編譯快取；最新候選與唯一失敗證據保留供接續查驗。

## 共用 Rust 邊界

| 擁有者 | 實作與介面 |
| --- | --- |
| 終端與連線 | 保留 [Screen](../src/screen.rs)、[Connection](../src/connection.rs) 與 Session worker。Screen 是 bytes、cell、ANSI、游標及 revision 的唯一來源。Session 唯一序列化輸入、站點切換、登入、保活、文章取頁與關閉。Slint 只發有型別的意圖及消費最新 `View`。 |
| 桌面服務 | 將現有 `desktop` 拆出共用桌面服務 feature。由依賴範圍決定最小移動，把既有站點／檔案／設定／字型／剪貼簿／圖片下載／密碼庫與持久化 owner 接給兩邊。Dioxus 的既有 `desktop` 呼叫維持有效，Slint 的候選 feature 單獨可建置。Headless core 繼續可測。 |
| 互動與呈現 | 從 [surface](../src/ui/surface.rs)、[SurfaceState](../src/ui/surface_state.rs)、[presentation](../src/ui/presentation.rs)、[protocol](../src/ui/protocol.rs) 逐項提取 WebView 無關的准入、投影、lease 與 reducer。Rust 共用資料可直接傳值或共享不可變快照。Dioxus adapter 仍負責既有 JSON wire、Eval、DOM ACK 和 WebView 信任檢查。Slint adapter 只接原生事件和本機模型。 |
| 畫面交接 | 保留一筆在途呈現與一筆合併後的最新快照。以「內容與對應幾何已安裝到同一可互動畫面」確認 presentation，顯示器呈現延遲另量。輸入、hit test、選取、媒體及輸出依既有各自需要的 generation／revision／input epoch／規則版本／presentation 核對。新來源 pending 直接撤銷舊輸出租約，即使 UI 尚未收到新畫面。過期確認只釋放原在途工作。 |
| 執行緒與關閉 | Slint 主執行緒擁有視窗、焦點、原生事件及 UI model。Connection／下載走既有非同步工作，磁碟、密碼庫、解碼與 PNG 走有界 worker；選檔器沿用平台要求在 UI thread 執行。只排程必要的 UI 喚醒。關窗先撤銷操作、計時器與回呼，再取消可取消工作，等待已開始的 OS 寫入、連線和 workers，顯示未完成或失敗，最後關視窗。 |

### WebView 模組遷移清單

| 現有程式 | Slint 對應 |
| --- | --- |
| `surface.js`、`surface-lifetime.js`、`surface-packets.js`、`surface-diagnostics.js` | 分離原生輸入、視窗事件與診斷。Slint 事件轉 typed intent，畫面由同一 Rust controller 派發。Dioxus 封包及 JS listeners 留在其 adapter 至切換。 |
| `surface-renderer.js`、`surface-fonts.js` | 原型 Skia painter 變成共用的 Slint 格線 painter。終端、可見文章列、字型預覽、ANSI 草稿投影與 PNG 消費同一份幾何與字形規則。 |
| `surface-navigation.js`、`surface-mouse.js`、`surface-context-menu.js`、`surface-tools.js`、`surface-toast.js` | Rust 管操作策略、命中、互動作用域和焦點復原。Slint 提供浮球、選單、狀態列、工具與通知的版面和原生事件。 |
| `article-find.js`、`article-updates.js`、`reply-navigation.js`、`surface-bookmarks.js`、`surface-filters.js` | Rust 共用文章快取、搜尋／推文索引、過濾投影與儲存交易。Slint 提供虛擬捲動、尋找、書籤和名單介面。 |
| `media.js`、`inline-images.js`、`gif-playback.js` | Rust 共用 URL 分類、下載、資源 owner 和文章錨點。原生 decoder／player 供 Slint 顯示，GIF 播放選擇和取消由單一 owner 管理。 |
| `surface-drafts.js`、`surface-editor-color.js`、`vendor/composer` | Rust 原始文件、編輯交易、ANSI 投影與索引。Slint 編輯介面、原生 IME、選取、色票及虛擬可視列取代 CodeMirror。 |
| `surface-settings*.js`、`settings-catalog.js`、`surface-login.js`、`surface-sites.js`、`i18n.js`、`vendor/fluent`、`app.css` | 共用 Rust 設定／站點／登入／Fluent 模型與字串。Slint 實作表單、分類搜尋、語言切換、版面、原生焦點及鍵盤可及性。JS 與 CSS 留作舊版對照。 |

## S0 固定基線與測試入口

1. 記錄兩個來源樹、工具鏈、OS／CPU／GPU／DPI、字型與 backend。對照 README、設定、快捷鍵、產品契約、`tooling/test_*.mjs`、原生探針和 release assets。每個既有互動指定一個案例 ID、基準證據、Slint 對應測試與所需平台。Dioxus 已有缺陷或未驗邊界單獨標示。
2. 做一份兩版共用的合成資料與 loopback runner。輸入為相同 byte chunks、分段時序、固定字型條件與本機 HTTP 媒體。peer 以獨立 expected 驗證出站 bytes、請求次數、取消、Close 和禁止操作。從既有 `tests/fixtures/dual-color.json`、`redraw.json` 和 `src/ui/probe.rs` 的閱讀案例著手。UI 驗證用真實 Slint／Dioxus host 產物，保存原生擷圖及可觀察狀態。診斷介面保留在 probe 範圍。
3. 建立共用服務 feature 和原生候選入口。用各 target 的 Cargo tree、link/import 清單及原生程序檢查驗證 Slint 可單獨建置。開發期以 `cargo build --locked --release --features slint-research --bin pttstella-slint` 和既有 `desktop` 建置作雙入口檢查，若候選 feature 改名則同步更新命令。檢查 Slint 授權和 AboutSlint 署名，列出 Skia、字型、編解碼器的授權及 notices 生成來源。預先登記 S8 的量測與門檻。

**通過條件**　兩版可重跑同一 replay 及基線用例，既有 core／desktop gates 通過。四個目標可各自解析獨立依賴圖。保留 Dioxus 對照產物及證據。

## S1 終端 renderer、原生輸入與高風險驗證

1. 把原型 renderer 接到真正的 `Frame` 和 `DisplayProfile`。採實體像素共享整數格線，背景先畫，雙色 glyph 只算一次原點、字型及 baseline，再做左右裁切。沿用 16 色／亮色／反白、未映射標記、ANSI 方塊／三角與框線，實作 ASCII／CJK 兩套字型、bundled fallback、字高、基線及 50%–150% 文字寬度。以 ink bounds 修正括號下緣，非整數 DPI 以邏輯字格計算字形縮放。固定 80×24 的寬高比、符合視窗、等比例 zoom、可捲動、全螢幕與最後一列。
2. 把 resize／DPI／frame 更新合併為事件驅動的 raster；同一畫面重用 pixel buffer 或 texture。先量全頁成本，再把有變動的列與游標／overlay 分離。使用有界字型、glyph 與可視列快取，逐項記錄 invalidation、配置、拷貝及上傳量。對齊畫面與 PNG 的畫筆，互動浮層放在獨立層。
3. 以原生文字元件做 caret 與 IME 入口，再驗證可觀察的原生 preedit／commit／cancel。組字文字就地顯示，候選窗錨點跟目前格線游標與 DPI。提交才由既有 `input` 編碼一次。前景、焦點、modal、選取及 input epoch 使用同一 gate。Slint 元件若無法完整提供 lifecycle，用鎖定版本的 Winit `Ime` 事件接最小 adapter，實測事件順序；原生文字事件與 raw key 不雙重送出。保留 PTT 編輯器 Ctrl+V 站端指令、Cmd+V／Ctrl+Shift+V 本機貼上、含 ANSI／多行先預覽、再次核對 scope，並沿用安全 SGR／輸入大小規則。 [Slint TextInput](https://docs.slint.dev/latest/docs/slint/reference/keyboard-input/textinput/) · [Winit Ime](https://docs.rs/winit/0.30.13/winit/event/enum.Ime.html) · [顯示契約](dual-color.md) · [輸入契約](feature-research.md)
4. 先做兩個小型原生驗證。編輯器用中文、混合 ANSI、跨行選取及一次 undo 證明可行。媒體用兩張 GIF、MP4／WebM／OGV 合成串流，證明有界播放、音訊、取消、釋放和乾淨環境可打包。先從 Windows 的 Winit＋Skia 主路徑及軟體路徑驗證，macOS 和 Linux 接著用同一 fixture。選後端時用實際文字、DPI、焦點、輸入與效能結果。

**通過條件**　Windows、macOS、Linux 都有真實 native window 的畫面／注音／貼上證據。驗證 100%／125%／150%／實體 Retina、跨螢幕 DPI、fallback、雙色裁切、括號、最末列、縮放拖曳與全螢幕。組字開始、候選鍵、空白、Enter、取消、失焦及換站的 peer bytes 可核對；同一字只提交一次，未提交部分為零 bytes。編輯器與影片的小型驗證各有可繼續實作的路徑。

## S2 連線、站點、登入與保活

1. Slint window 接既有 Connection watch 和命令 API。共用 WSS 系統 TLS、PTT SSH 公開帳號與 host-key pinning，保留 Big5-UAO／Big5／GBK、輸入預檢、舊明文端點管理與預設站 fallback。開窗只連已保存預設站一次，切站先取消舊傳輸再改 generation，畫面顯示連線、等待、錯誤和重試。
2. 接上原生登入表單、單次登入 reducer、保存／忘記及系統密碼庫。手動登入始終可用。按鈕登入只在精確 PTT 目標與允許畫面生效，密碼、2FA、重複連線處置與異常提示遵照現有政策，使用者明確授權才保存或送出。非 PTT 站可原始操作並使用共同擷取、複製與媒體。
3. 保活沿用 Connection 的唯一計時器，預設開啟、依 session 和已確認前景 UI 的 gate，只在辨識列表／閱讀作用。組字、貼上、選取、編輯、工具、失焦／睡眠及斷線期間暫停。重連與恢復後從新週期開始。

**通過條件**　本機 WSS／SSH peer 驗證 TLS／金鑰錯誤、原始 bytes、80×24、手動與按鈕登入、取消等待、分段提示、晚到結果、切站、重連及 Close。舊 session 零重播，未知／非 PTT 畫面零 PTT 自動命令。保活在合成時鐘與原生失焦測試核對。三種 OS vault 用合成帳密通過保存、移除、不可用與兩次啟動。本機測試用合成 CA／SSH host key 驗證傳輸，正式信任策略沿用原值。實站登入與帳密交互另由授權人工案例驗證。 [站點](sites.md) · [登入](login.md) · [保活](keepalive.md)

## S3 鍵盤、滑鼠、選單與擷取

1. 把既有按鍵優先序移到 typed 路由。modal／選單、IME、搜尋或編輯元件、本機選取與導覽、Vim、站端 raw 各用明確狀態，保留 repeat、按下／放開、Vim Normal／輸入／原生一次、計數及進板繼續。保留原始 PTT 控制鍵，從同一既有 writer 發送。
2. Rust 用已確認的 geometry 判定 cell／行、連結、圖片、捲軸、浮層及四周留白。使用按下／放開同一 lease、原生 pointer capture、拖曳閾值與撤銷規則。列表可點整行開文，左緣返回，前後翻頁，首尾與狀態列操作，既有藍金提示保持 ANSI 對比並支援減少動態效果。站端滑鼠模式維持 SGR press／release／wheel 的既有能力、回壓與取消，Shift 保留本機選單。
3. 以既有 `selection` 產生純文字／ANSI（含 SGR 66）及完整 URL／OSC 8 操作。右鍵依目前已確認的命中分流圖片、連結、ID、選取、編輯器、閱讀背景及浮球；選單外點消費同一手勢。建立原生鍵盤可操作的右鍵選單、浮球扇形分類、快捷鍵與對話框焦點復原。
4. PNG 從同一 Rust renderer 在 worker 產生。整頁固定 960×576、選定連續行為 960px 寬且最多 24 行，可存檔或寫剪貼簿，排除游標、反白、圖片與 UI。選取／匯出／圖片的 lease 在產生、選檔回覆與 OS 寫入前重驗，只有明確操作才開啟 picker 或存取剪貼簿。

**通過條件**　真實原生按鍵、全螢幕外緣、可信滑鼠拖曳、修飾鍵／站端滑鼠、右鍵優先序、浮層焦點、剪貼簿讀回與 PNG 獨立像素驗證通過。失焦、來源更新、resize、切篇或關閉時撤銷舊操作，peer 不收到第二次手勢或被攔截的操作。 [導航](navigation.md) · [站端滑鼠](station-mouse.md) · [Vim](vim-browsing.md) · [右鍵](context-menu.md) · [連結](links.md)

## S4 文章、搜尋、推文與收藏

1. 從既有 Rust `navigation_job`、`ArticleCache`、文章更新及書籤 owner 接入快速切板、最近看板、AID／URL 快速前往和文章列表。站端任務需逐頁核對回覆，切站／切篇及手動操作撤銷舊任務。收藏保留從目前文章及列表建立、管理、前往、完整快取可離線開啟。
2. 終端機模式保留原站端逐頁。網頁模式是 Slint 原生長文 viewport，使用 Rust 現有 cache 合併、折行重疊及尾頁確認。虛擬化可視列與上下緩衝、固定頁尾及媒體間隙，保持文字列＋列內偏移的閱讀錨點。沿用 4,096 列／32 MiB 等既有上限、分段接續、取消／繼續和快取完整性。取頁沿用單命令與既有 deadline，接頁與繪製解耦。PageUp／PageDown 一行銜接，保留 j／k、Home／End、空白、滾輪、未知邊緣補頁、首尾方向鍵切篇的開關與防 repeat 規則。
3. 對已確認投影建立有界搜尋索引，只在有查詢時建立並於查詢時合併重算，捲動沿用索引。保留 Cmd/Ctrl+F、命中高亮、上一筆／下一筆與 IME 優先，站端 `/` 繼續送站端。Rust 過濾和推文投影保留黑白名單、兩種遮罩、文字名單匯入、作者欄、連推合併、同 ID 高亮／只看／恢復、原 PO／自己側標及快速定位。查詢、高亮和遮罩不讓原文或被遮內容從剪貼簿、媒體、PNG 或 hit test 旁路出現。
4. 推文更新預設關閉，限明確啟用的目前文章。沿用既有 5 秒週期、尾段比對、未讀提示及跳底，保留本機閱讀錨點、媒體及失敗後「顯示站端畫面」復原。暫停、取消與晚到回覆維持原 scope。

**通過條件**　重跑 `full_load`、`long_load`、`terminal_pages`、跨段上限、晚到／停滯、切篇、`inline_pages` 的無圖基準、收藏重開與推文更新。獨立核對每段首尾、折行、顏色、兩版 peer bytes、無意外翻頁、末列可達、搜尋焦點、推文更新失敗的復原路徑及隔離後的擷取。 [文章閱讀](article-reading.md) · [快速切板](quick-boards.md) · [指定文章與收藏](article-navigation.md) · [內容過濾](content-filters.md)

## S5 圖片、GIF 與原生影片

1. 沿用 Rust URL 分類、HTTP 下載、redirect／TLS 驗證、大小／尺寸、cache、request／generation、展圖 batch 與取消。解碼放在可 join 的 worker，持有一份經驗證原始 bytes，首幀、畫布與解碼中的中間資料分開計算上限。先用已鎖定的 `image` 解碼支援的格式，為未覆蓋的動畫格式依本機 fixture 驗證 decoder 與 disposal／blend。候選只能支援既有產品接受且有明確錯誤回退的範圍。
2. 實作 hover 跟隨／右上／關閉、升級完整預覽免重抓、等比例 fit、游標錨點滾輪 1%–800%、拖曳與 capture 撤銷。圖片選單保留原尺寸複製、另存原始格式、複製網址與外開。HTTP 404／超限／逾時／不支援要可見且可依既有政策重試。YouTube、GIFV 和非直連頁沿原規則外開。
3. 網頁模式用原有的 link index／已確認投影和文章作用域，維持網址末端逐張箭頭、短 Enter 依可視順序開圖／等寬、長按 500 ms 全開、去重及讀取錨點。自動開圖預設關閉，明確啟用時按文章與推文順序加入同一批次。終端機模式維持 hover／預覽。每段維持最多 32 URL、32 MiB payload、16M 首幀像素等既有預算。
4. 做一個共用動圖播放 owner，依來源 frame delay 排程、保留首幀及少量有界畫布，避免預先解出全部幀。文章內至多兩張中央可視 GIF，完整預覽至多一張並暫停文章動畫；保留手動停播、回首幀、可視／焦點暫停、重開、資源上限及取消語義。以實際多圖峰值再決定快取與背壓。
5. MP4／WebM／OGV 需要原生內嵌播放、音訊、播放／暫停／進度控制與外開 fallback。S1 先驗 `libmpv` Render API 作候選，以可取消的串流及有界 video buffer 接 Slint，控制與解碼在各自 owner，Slint 主執行緒只消費目前畫面。以實際包大小、runtime 依賴、授權、HTTP range／跳轉政策及各平台可用 codec 作選型；若不合格，再驗平台原生播放 adapter。以 Dioxus 在各平台實際可播的格式為驗收下限。開啟預覽先呈現可播放狀態，沿用不自動播放及播放失敗可立即外開的契約。 [現有媒體契約](media.md) · [libmpv Render API](https://github.com/mpv-player/mpv/blob/master/include/mpv/render.h)

**通過條件**　本機 HTTP 與原生畫面核對自動開圖開／關、正文＋推文、單張／全開、等寬、跨頁歷史內容、hover 零重抓、原圖／原始檔、GIF 首幀和可視暫停、各平台影片及取消。驗證切篇／關窗後網路、解碼、播放、GPU 影像與 timers 都釋放，舊工作無法復活。

## S6 本機撰寫、草稿及站端上色

1. 沿用 PTT Ctrl+P、分類及標題流程。由既有 `editor_source`、Connection 和 scope 完整讀取站端範本、ANSI、畫面外文字及末尾空行。進入前依站點／帳號／板名確認身分與可恢復草稿。列表入口維持管理已存草稿。
2. Rust 持有單一原始文件、undo／redo 交易與文字偏移對應。維護原始 UTF-8、完整 SGR token、可見文字及 cell 的映射，以既有安全 grammar 和 Screen 投影產生彩色／隱藏色碼／原文三種可編輯視圖。原生輸入元件處理 caret、IME 候選及提交。虛擬化 79×23 可視格線與長文捲動，輸入更新受影響的列和仍受 ANSI 樣式延續影響的後續列。標題、剪貼簿、移動、選取與編輯歷史共用同一交易狀態。
3. 保留字元／跨行、整行和 ANSI 矩形選取，完整雙欄字不切半。浮動 16 前景／8 背景色票對當前 revision 一次套用並可 undo。切換顯示不變更原文、尾空行、未知或半成品色碼。站端編輯器框選上色走既有 `editor_color`，驗證來源和站端模式後只插入受控 SGR。
4. 沿用草稿 store 的 2 秒 debounce、連續保存最短 5 秒、IME 不保存半成品、失焦／退出／明確保存 flush，以及一筆在途和一份合併後快照。同稿衝突保留獨立副本，不同稿不互蓋。帶回需再次明確確認，依既有 worker 重讀並比較原範本、核對空白文件、一次送正文、核對末行／欄與模式。保留本機稿，由使用者在站端檢查與發表。

**通過條件**　原生 IME 和真實選取／拖曳，中文、ANSI、一般與矩形上色、undo／redo、三種模式原文等同。跑 `drafts`／`drafts_restart`／`editor_color` 等效探針，合成 peer 核對 79 欄／byte 限制、範本變動拒絕、分段等待、取消與部分寫入錯誤、重開恢復和零發表命令。兩程序同稿／不同稿、磁碟失敗及強制終止後恢復分開驗證。 [草稿契約](drafts.md) · [站端上色](context-menu.md)

## S7 設定、資料相容與系統整合

1. 接上現有設定分類、中文／英文搜尋與 Fluent。完整移植站點、字型清單／官方來源、傳統字型組合、預覽和重設、可攜 profile、語言、文章模式與首尾、hover／自動開圖、浮球透明度／可見／右鍵工具、保活／Vim／站端滑鼠對應入口。依各設定現有保存範圍處理。保留保存成功後生效、錯誤保留草稿、最新草稿在關窗時 drain。通知沿用成功淡出、錯誤及斷線保持可見。
2. 沿用現有 store 的型別、路徑決策、版本、備份、原子寫入和 OS 鎖。研究版使用明確的隔離 namespace。正式候選用複本測 `settings.json`／`sites.json`／`filters.json`、`bookmarks.json` 含離線資料、`recent-boards.json`、`drafts.json` 和原有 display profile；設定涵蓋 v1–v6，驗收現行公開版及最後一個受支援 Dioxus 基準的資料。讀舊資料先在記憶體解析，驗證重開和一次 Slint 寫入後再由指定回退版 Dioxus 讀寫。文件毀損、鎖逾時、容量、不同欄位合併、同欄拒絕、清空屏障及草稿衝突副本皆有回歸。

   已驗的完整新欄位回退基準為 `5ea9ad6`。公開版 v0.4.1（`e304be2`）尚無 `auto_expand_images`，會拒讀含該欄位的設定；其支援的舊格式可以往返，但不能把含新欄位的資料直接交回此公開版。保留拒讀且不改檔反例，不靠刪除欄位宣稱相容。headless 生產 store 的28案不替代實際前端、產品資料路徑、vault 及平台安裝回退；重跑方式及範圍見 [三版本資料往返](slint-prototype.md#s7-三版本生產-store-往返與回退界線2026-11-09)。
3. 合成帳密用隔離 vault 測 macOS Keychain、Windows Credential Manager、Linux Secret Service 的舊格式、重啟、刪除與拒絕。確認候選切換產品 ID、正式路徑、既有 service 與帳密讀取後，簽名變化是否需要重新授權。診斷沿用有界 JSONL、每次啟動隔離與資料遮蔽。保留更新檢查、錯誤、版本比對、下載外開及第三方授權介面。
4. 核對 native menu、IME 焦點、clipboard、檔案 picker、外開、縮放／全螢幕、Tab 操作與可及性。原型 AX 未找到輸入欄，須用平台 accessibility inspector 或 screen reader 重新驗證必要控制項、語意標籤與焦點復原。

**通過條件**　合成舊資料、副本與雙版本往返測試可重現，舊版在讀取後仍能做正常修改。研究 App 零 production store／vault 存取。兩前端並行的合併測試只使用遵守同一鎖契約的基準版本。 [設定](preferences-and-profiles.md) · [多開](multi-instance.md) · [語言](i18n.md) · [診斷](diagnostics.md) · [更新與發佈](releases.md)

## S8 候選打包、平台驗收與效能

### 平台與 WebView 驗收

| 目標 | 發佈格式與原生驗收 |
| --- | --- |
| Windows 10／11 x64 | 安裝版與可攜版。既有 Dioxus→Slint 升級、移除、離線安裝、原生 CredMan、注音、細明體、多螢幕 DPI、低階 GPU 及軟體 renderer。乾淨 VM 缺 WebView2 runtime 時仍能安裝和完成終端／媒體案例；記錄模組／子程序及 installer 實際行為，Slint 包不帶 bootstrapper 或 WebView2 prerequisite。 |
| macOS arm64／x64 | 各自 DMG＋ZIP、原生安裝／啟動、簽名與 Keychain、Retina、威注音／系統快捷鍵和全螢幕。核對 Apple Silicon、Intel 的實際證據，Rosetta 只算補充。以 dyld trace 區分產品連結、OS 間接載入、WebView 實例及 browser child。原型已見 WritingToolsUI 載入 WebKit，當前「執行期零 WebKit 映射」未達成。候選必須能證明內容與互動走原生路徑，若 OS 仍載入框架，由使用者決定此差異是否符合平替要求。 |
| Linux x64 | Ubuntu 24.04 的 `.deb`＋`.tar.gz`，如果待整合基準有新增格式就一併驗。乾淨環境移除 WebKitGTK 依賴後跑安裝、Wayland／X11、Fcitx5 或 IBus、字型 fallback、clipboard owner 與 Secret Service。核對 apt package metadata、動態連結、缺服務及無 GPU 的失敗或替代路徑。 |

用獨立驗證包或既有 workflow 的 verify 路徑產出四架構附件。保留原產品識別對照、SHA-256、依賴與授權清單、安裝及升級／降版操作紀錄。平台矩陣分列建置、安裝、實際視窗、原生輸入與實站。受影響平台須有自己的證據。正式候選功能門檻包括已驗證的原生播放器與編輯器。經授權的實站案例單列 WSS／SSH 登入、進板閱讀、斷線重連及草稿帶回後停在編輯器；帳密由使用者操作，結果不混入 loopback gate。 [Windows 安裝](windows-installer.md) · [macOS 打包](macos-release.txt) · [Linux 打包](linux-package.txt) · [發佈流程](releases.md)

### 同負載效能門檻

- **固定條件**　同一台機器、相同 release 設定與等效功能、視窗大小、DPI、字型、焦點狀態、loopback replay／HTTP 資料和 driver；記錄 Slint renderer 與基準 WebView 版本。先在低階 Windows 登記機型，再測其餘目標。冷／暖分開，各至少 10 次啟動、每組至少 100 次輸入或翻頁；交錯兩版順序，記錄冷啟動定義、原始數據、誤差和離群原因。
- **時間與正確性**　啟動到第一個可見且可輸入 frame、原生輸入到畫面呈現、快取翻頁／長文捲動的 p50／p95 及掉幀。站端取頁和圖片下載時間分列。對兩版採相同觀測點，不能拿 Rust 交付 bitmap 的時間對比另一版螢幕呈現。相同劇本核對畫面、實際 bytes、HTTP 和資源清理結果。
- **資源**　比較閒置、長文快速捲動、兩張 GIF、影片、草稿編輯、多開與反覆切篇／開關後的程序族 CPU、峰值及穩定記憶體。Dioxus 納入 WebView 子程序，Slint 納入播放器或任何子程序。各平台固定一種主記憶體指標，另列 RSS／GPU。取樣至少覆蓋穩定閒置與回收後狀態，核對無殘留連線、decoder、timer 或累積配置。
- **建議判定**　低階 Windows 的暖機操作 p95 或閒置記憶體至少一項改善 20%。其餘主要指標在各平台無超過已登記波動且可重複的 10% 以上退步。正確性、功能完整、資料和清理為獨立必要條件。量測差異落在波動內則重測；若只達成 WebView2 移除，將該收益及效能差異分列，由使用者決定是否接受。

**通過條件**　完整矩陣、原始資料、平台包、具體 renderer 選擇、所有仍待人工項目與可重跑命令齊備。每項標 `passed`、`failed` 或 `not_ready`，附 commit／dirty、platform／arch、fixture digest、實際輸入、預期／結果和證據。跨平台建置、合成事件或計數各只證明自己的範圍。只要必要項未通過，維持候選狀態，保留該項的最小重現。

## S9 取得核可後切換與回退

1. 以 S8 的結果請使用者決定。提供候選 App、差異、效能、macOS OS 框架載入與資料回退演練。確認採用後，以當時 `develop` 作整合基準，將正式 `pttstella` 指向 Slint，讓候選套用既有產品 ID、資料目錄、密碼庫與升級路徑。切換提交與刪除舊前端的提交分開，便於先回退入口。
2. 做一次完整原地升級與回退演練。關閉寫入中的舊程序，備份各 store 主檔／備份及版本證據，保存前一個可安裝 Dioxus 產物。啟動 Slint、讀舊資料、做代表性保存與密碼庫存取，再換回 Dioxus，核對資料、連線和可攜設定。格式若確實需擴充，先設計雙版可讀方案並驗證，避免靠丟棄升級後的新資料回退。
3. 切換驗收通過後，再從正式入口與包移除 Dioxus／Wry／WebView 相關依賴、舊 JS／CSS／CodeMirror 資產、WebView2 bootstrapper 和 Linux WebKitGTK package requirement。已移植行為的舊 JS gate 由對應 Rust／Slint／原生 gate 接手。保留基準 Git ref、安裝包、fixture 和對照證據；回退走已驗證的前一版或可逆入口提交。
4. 依 repo 流程更新 README、安裝說明、測試及 release workflow，重跑 core／native／安裝與回退。正式版號、release PR、tag 與發佈依使用者的該版授權。

## 驗證與交接紀錄

每階段使用 [CONTRIBUTING](../CONTRIBUTING.md) 的對應 gate，再加 Slint 專屬 unit／Clippy、共用 loopback、真實視窗和各平台 package gate。舊前端仍用既有 JavaScript、Rust、desktop probes 作對照。原生 IME、裝置、vault 或實站缺項各自登記。證據保存在 worktree 的 `.local/verification/`，保留候選包、雜湊、最小重現及原始資料。交接記錄完成階段、下一個入口、缺 runner／授權、實際命令和證據位置。結案先確認 resolved path、程序與所有權，再清理自己被取代的產物並記錄空間前後。 [開發與清理流程](development-harness.md)


2026-11-07：文章尋找已接入共用 Rust 索引、Slint 原生搜尋列與 Dioxus typed adapter。macOS arm64 兩種閱讀模式的合成完整 App 及原生 OS 注音／貼上／焦點已取得分立證據；詳細範圍、負向紀錄與仍待驗項目見 [原型紀錄](slint-prototype.md#s4-文章尋找共用-rust-索引與原生搜尋列)。S4 其他移植和 S5–S8 繼續，完整 goal 保持 active。

2026-11-08：S5 主視窗已有原生圖片／GIF 預覽、原尺寸複製／原始檔另存及圖片右鍵選單；影片 runtime 封裝仍是獨立播放器實驗，尚未整合主入口。macOS arm64 的實際輸出、HTTP 計數、焦點與 owner 清理分開記在 [原型紀錄](slint-prototype.md)。hover、文內展圖、動畫語義、編輯器、跨平台原生、回退及效能缺項繼續，沒有將局部成果當成 S1／S5 或完整候選版通過。

2026-11-08：原生 hover 的三種偏好、350ms 停留與同資源升級已接入，關閉模式的一般連結提示及設定衝突亦有回歸。macOS 原生操作與 toolkit 的被動焦點／四角定位證據範圍分列於 [原型紀錄](slint-prototype.md)。文內展圖、媒體／編輯器整合及其餘 S0–S8 缺項持續；目前沒有跨平台或效能結案。

2026-11-08：既有展圖 batch 已移到共用 Rust，Slint 長文 viewport 已接入有界圖片間隙座標及錨點；實際文內圖片與操作仍待接線。完整預覽的增量播放邏輯已抽為後續共用的原生 owner。合成長文的 500ms＋150ms 逐頁延遲保留作負向驗證，另有立即回覆診斷及共用有界時鐘；初步數據不代表 S8 效能通過。詳細證據與範圍見 [原型紀錄](slint-prototype.md#s5-展圖批次圖片間隙與播放排程2026-11-08)。完整 S0–S8 goal 維持 active，S9 未切換或發佈。

2026-11-08：Slint 文內單張圖片、原圖預覽升級與可視 GIF 已接進既有 Rust batch／同一媒體 actor。完整元件及 macOS 原生操作分立驗證，保留版面通知、雙 GIF 恢復及已開箭頭穿透的負向案例和修正。短／長 Enter、等寬、自動展圖設定、圖片選單及其餘 S0–S8 項目繼續；多圖峰值和跨平台效能未通過，S9 未切換／發佈。詳細範圍見 [原型紀錄](slint-prototype.md#s5-原生文內圖片與可視-gif2026-11-08)。

2026-11-08：短／長 Enter、可見圖片順序與等寬已接入同一原生圖片 owner；超長等寬圖先以來源像素裁切再交 Slint。完整元件 short／long／pointer 及 macOS 原生長按、放開、短按等寬分開取得證據；細節、負向案例和仍待原生／資源峰值的範圍見 [原型紀錄](slint-prototype.md#enter-展圖與等寬裁切2026-11-08)。自動展圖設定、圖片選單及其餘 S0–S8 繼續，整體 goal 維持 active，S9 未切換／發佈。

2026-11-08：自動展圖已接上共用設定保存、失敗重試及同一 actor 的動態取消；共用 `223b51c` 加入自動 batch 來源標記與成功解碼後的既有 LRU 准入。完整元件、真實 stalled HTTP、macOS 原生勾選及明確 All 保留分開驗證，來源／快取政策和重啟未驗範圍見 [原型紀錄](slint-prototype.md#自動展圖設定與原圖快取2026-11-08)。原型工具列不代表 S7 設定中心完成；圖片選單及其餘 S0–S8 缺項繼續，沒有切換入口／發佈。

2026-11-08：文內圖片四項選單及背景選單 All 已接入共用 policy／輸出 owner，並補上原生 Shift 狀態、輸出後鍵盤恢復及可視 model 的逐列更新；先前黑圖判定因工具預覽與原始像素不一致而撤回；同版六個原生程序、原始 OS 擷取及獨立解碼器已複核文內圖片與輸出。十一組完整元件及分 hash 的 macOS 原生輸出、選檔和從零全開證據見 [原型紀錄](slint-prototype.md#文內圖片選單共用輸出及原生驗證2026-11-08)。影片／編輯器、原型來源提交序列、S7 完整操作、四架構／回退及效能仍待完成，完整 goal 維持 active，S9 未切換／發佈。

2026-11-08：影片 control／GL ABI 已抽為主視窗與獨立 probe 共用 session；有界 Rust relay 共用圖片 TLS／redirect 政策，主視窗接入 GPU FBO 影片區、明確播放、暫停／進度與退場。三格式、停滯、播放清單拒絕、macOS 原生輸入及各輪 binary 範圍見 [原型紀錄](slint-prototype.md#s5-共用影片串流與主視窗播放2026-11-08)。這是 S5 的本機整合子項；音訊人工、完整 runtime／其他平台、GL 恢復及 S6–S8 等缺項繼續，沒有平替結案或 S9 切換／發佈。

2026-11-08：S6 已有保留完整原文的 Rust 文件／歷史、逐列 rendition 快取及兩前端共用保存 owner；SGR 66 及既有 Canvas 直接消費者修正已分開提交，實際 Dioxus 的保存／新程序恢復／合成帶回取得分立證據。詳細範圍見 [原型紀錄](slint-prototype.md#s6-原文文件與共用草稿2026-11-08)。Slint 編輯 UI、原生 IME、排程與帶回接線仍待完成，沒有將核心或舊 host 驗證當成 S6 原生通過；S0–S8 goal 保持 active。

2026-11-08：Slint 主視窗已接上 79×23 原文編輯區、同一 Document／歷史、原生 IME／實體快捷鍵、可見列繪圖、保存 actor 與既有範本／帶回流程。嚴格 loopback 的 fresh／restart、macOS 實際注音取消／單次提交、貼上、標題、拖曳上色及關窗保存分開取得證據；四項負向案例與修正、候選清單未觀測及仍待驗項目見 [原型紀錄](slint-prototype.md#s6-原生編輯主視窗初版2026-11-08)。浮動色票／選取入口、取消／失敗、完整站端上色、S7–S8、跨平台／回退／效能及原型提交序列繼續；未將初版整合當成 S6 完成，完整 goal 保持 active。

2026-11-08：本機編輯器已新增浮動 16／8 色票、文字／整行／矩形選取與可見取消；修正真實上色後的焦點及長文組字定位。五組完整元件、macOS 原生操作、獨立像素與保存原文的範圍及負向案例見 [原型紀錄](slint-prototype.md#s6-色票明確選取與可見取消2026-11-08)。矩形選取歷史、站端上色、部分失敗及其餘 S6–S8 繼續；合成長文刻意延遲保留，完整 goal 維持 active，沒有切換入口或發佈。

2026-11-08：選區形狀已納入同一 Document 歷史，修正矩形 undo 後再輸入會刪到未選文字的缺陷。共用核心 `93657ea`、完整元件與 macOS 真實拖曳／undo／redo／上色／貼上／保存的範圍見 [原型紀錄](slint-prototype.md#s6-選區與正文一起還原2026-11-08)。站端上色、部分失敗及其他 S6–S8 缺項繼續，完整 goal 維持 active，沒有切換入口或發佈。

2026-11-08：原生站端框選上色已接上同一 Output 選區與 Connection Flow，提供預覽、明確套用、可見取消及部分寫入警示。四組完整元件、三個 macOS 原生程序、共用草稿回歸與嚴格 bytes／模式／像素的範圍見 [原型紀錄](slint-prototype.md#s6-原生站端框選上色2026-11-08)。負向 peer 以 `8f0d7ad` 提交；本機草稿部分錯誤、原生邊界、S7–S8 及來源提交序列繼續，完整 goal 維持 active，沒有切換入口或發佈。

2026-11-08：本機草稿新增範本變動、改標題／清空／正文部分執行、TCP 中斷及讀取／帶回中關窗的嚴格獨立 peer，以 `a63f5ff` 提交。七組完整元件、五組既有草稿回歸及三個 macOS 原生程序核對本機稿保留、不重送、關窗與離線保存；詳細範圍見 [原型紀錄](slint-prototype.md#s6-草稿部分失敗斷線與工作中關窗2026-11-08)。這些是指定步驟的故障注入，不代表站端已接收 bytes 可以回滾或每個網路切分都已驗證。磁碟失敗／強制終止／多程序、原生 IME 邊界、S7–S8 及來源提交序列繼續，完整 goal 維持 active，沒有切換入口或發佈。

2026-11-08：Slint 完整主視窗新增備份寫入失敗／損壞資料保留與重試、確認保存後強制終止／新程序恢復，以及兩程序同稿衝突副本／不同稿合併驗證。嚴格只讀範本站台入口以 `d0a3f67` 提交。macOS 第二輪原生錯誤提示、保留正文與重試逐 byte 通過，但第一輪曾觀察到末字消失，尚無原因，不列為已修復；證據與界線見 [原型紀錄](slint-prototype.md#s6-草稿儲存故障程序終止與多開2026-11-08)。原生輸入異常、IME／可及性、S7–S8 及來源提交序列繼續，完整 goal 維持 active，沒有切換入口或發佈。

2026-11-08：共用有效偏好／編輯 baseline／載入警告已由 Dioxus adapter 抽至 Rust，以 `19afa3f` 提交；Slint 媒體設定接入同一型別，啟動時移交原 Store，保留明確恢復損壞檔所需的讀取觀測。兩版 release／Clippy、382 組 JS、三組 Slint 完整元件及 macOS 實際 Dioxus 保存／失敗／重試／重開／PNG／Close 的範圍見 [原型紀錄](slint-prototype.md#s7-共用偏好狀態與啟動-store-交接2026-11-08)。Slint 完整設定中心、原生 IME 邊界及 S8 繼續；沒有將狀態共用當成 S7 或平替完成。使用者確認末字異常期間有其他操作，但不足以確認原因；本輪 IME 嘗試未收到 preedit，明列 not_ready。


2026-11-08：Slint 原生設定視窗初版已接入單一偏好／檔案 owner、300ms最新有效修改、成功後生效、重試與關窗交接；四個完整元件及 macOS 原生設定檔匯出／匯入／重開通過。快捷鍵、picker取消及合成 seed 的負向案例與修正分 hash 保留；OS Quit 清理、完整管理／語言與各 consumer 原生同步等仍待完成。範圍見 [原型紀錄](slint-prototype.md#s7-原生設定視窗與保存交接2026-11-08)。完整 S0–S8 goal 維持 active，沒有將設定視窗初版當成 S7 或平替完成，S9 未切換／發佈。

2026-11-08：macOS原生App選單已接入正常Rust退出，指定的設定待存修改／草稿貼入後⌘Q逐檔與嚴格Close通過；啟動文章首尾偏好亦接入第一份投影。Documents路徑的另一個合成草稿讀取仍停在OS open，保留原程序及sample，沒有列為通過。Dock／modal退出、原生輸入邊界、完整S7與S8繼續；精確hash、負向及範圍見[原型紀錄](slint-prototype.md#s7-原生-app-選單與正常退出2026-11-08)。完整S0–S8 goal保持active，S9未切換／發佈。

2026-11-08：主視窗、編輯器、搜尋與既有登入／媒體／色票標籤接入同一語言refresh，共用呈現helper以`4578b4f`提交。四個完整App及macOS原生下拉中英往返分立驗證內容／查詢／輸出選項保留；前輪讀取停滯程序以probe timeout結束，仍屬失敗。範圍與負向見[原型紀錄](slint-prototype.md#s7-介面語言同步2026-11-08)。完整S0–S8繼續，完整管理／搜尋、原生邊界與S8未結案，S9未切換／發佈。

2026-11-08：共用站點編輯狀態以`3e09e7f`提交，Slint已接上原生站點／編碼／最愛／獨立預設、成功後生效及寫入交接。指定完整元件與macOS保存／重開／主視窗輸入隔離分hash驗證；有效切站與其餘S2／S7仍待完整App驗收。範圍見[原型紀錄](slint-prototype.md#s2s7-原生站點管理2026-11-08)。完整S0–S8目標持續，S9未切換／發佈。

2026-11-08：以`e00088f`新增共用獨立站點peer，三個完整App驗證連續切站、斷線後切站與取消未完成握手；macOS實際按鍵／貼上另核對精確GBK／UAO bytes及三站Close。範圍、hash、負向及剩餘S2項目見[原型紀錄](slint-prototype.md#s2-本機切站與雙編碼出站2026-11-08)。完整S0–S8目標持續，S9未切換／發佈。

2026-11-08：共用desktop-services、輸入交易、媒體HTTP／relay／可選libmpv及第一個Slint終端／文章前端已分成八筆可獨立編譯提交。原生最新完整WIP保留，沒有降版或切換正式入口；確切tree、各筆gate及後續提交範圍見[原型紀錄](slint-prototype.md#s0-可獨立編譯的提交序列2026-11-08)。完整S0–S8持續，S9未切換／發佈。

2026-11-08：文章操作／尋找、圖片／hover、文內展圖與主視窗影片UI已接續分成四筆可編譯提交；影片探針補上每個錯誤出口的GL清理與原訊息保留，並分列失焦與Agent探針插入錯誤的負向。各筆確切來源、GPU／HTTP／Close及剩餘退出邊界見[原型紀錄](slint-prototype.md#s4s5-前端提交與影片失敗清理2026-11-08)。完整S0–S8繼續，S9未切換／發佈。


2026-11-08：現有完整 Slint 前端、原文編輯／色票／失敗儲存、設定／語言／站點、獨立播放器與探針打包工具已接續提交。公開草稿驗收 driver 在 macOS arm64 跑完8個子程序、7個正常 Close、1個指定保存邊界 kill 和2組獨立磁碟核對；不代表原生 IME、任意斷電或其他平台通過。各提交及重跑方式見[原型紀錄](slint-prototype.md#s6s7-完整前端提交與可重現草稿驗收2026-11-08)。產品 active-video 退出、管理／完整搜尋及四架構／回退／效能等仍未完成；S0–S8 goal active，S9 未切換／發佈。


2026-11-08：`1b2bb96` 將正常 App Quit／根視窗 Close 集中到單一退出 owner，釋放根視窗／GL 後保留事件迴圈等待影片工作結束。四個完整程序涵蓋未播放、播放中、停滯與最小化；macOS 實際 ⌘Q／關窗另驗，並重跑設定待存、文章取消、草稿工作中關閉與8程序儲存案例。負向、各 hash、清理和仍待驗範圍見[原型紀錄](slint-prototype.md#s5s7-正常影片退出與根視窗所有權2026-11-08)。完整 S0–S8 持續，S9 未切換／發佈。


2026-11-08：`bae1852` 共用24項固定搜尋目錄與有界 NFKC 索引，`8195c7c` 對齊 UTF-16 邊界，`b2af0a4` 接上23個目前可用的原生目標、可取消焦點排程、暫時操作開關與具名設定檔。中英文46次完整元件定位、三組設定／退出回歸，以及 macOS 兩版實際搜尋／聚焦分 hash 驗證；Slint另驗原生 picker 匯出／讀回名稱，Dioxus另驗PNG與Close。名單管理尚未移植，因此該搜尋目標仍待接線；其餘 S0–S8 未結案。詳細範圍、負向與清理見[原型紀錄](slint-prototype.md#s7-共用搜尋目錄原生定位與具名設定檔2026-11-08)。

2026-11-08：追加 `a1dee9a`，用先失敗、後通過的完整視窗案例修正一般搜尋欄 focus 被舊定位 timer 拉走；既有明確搜尋導覽取消仍保留。最新普通 CLI／gate 與 OS 檢查點的不同 hash 分列於原型紀錄，未外推至跨平台或整體 S8。

2026-11-08：黑白名單原生表單已接入共用規則、匯入預覽及原子保存；設定補齊八分類與24項搜尋定位。終端／網頁的合成完整元件、取消與晚到錯誤、備份失敗、壞檔唯讀及退出時在途保存已有分立證據；macOS arm64 實際視窗的搜尋、繁中貼上、系統選檔與保存亦已驗證。這不是 OS 注音或跨平台驗收，也未完成帳號右鍵、揭露及書籤管理。詳細範圍見 [原型紀錄](slint-prototype.md)。S0–S8 持續，S9 未切換／發佈。

2026-11-08：帳號右鍵的三策略／移除、列表作者欄、暫時揭露與推文底色標示已由 Slint 接到既有 Rust reducer及名單保存。兩種閱讀模式的完整合成 App 驗證遮罩輸出、舊選單／租約拒絕、保存失敗、連點取消及 Shift選字；網頁模式另驗已解碼圖片／GIF／等寬與閱讀位置保留。此為 toolkit與本機 peer證據，可信 OS手勢／IME、混合 Live與跨平台仍待驗；詳細範圍見 [原型紀錄](slint-prototype.md)。S0–S8持續，S9未執行。


2026-11-09：`e704fc1` 將書籤狀態、離線投影及收藏身分取得移至共用 Rust；`577f927` 接入 Slint 原生書籤管理、23 行離線 viewport、成功後提交、唯讀壞檔／重試和退出時在途保存。完整元件及新程序斷線閱讀核對原副本、當前遮罩、雙色字與零額外按鍵／HTTP；macOS 實際浮球、繁中貼上保存、翻頁、返回與設定交接另列證據。浮球分類的舊 allowlist 及新增狀態／讀取提示已修正。這一輪尚未接上目前／列表文章收藏、前往或最近看板；完整四架構、資料回退及效能也仍待完成。詳細 hash、負向與清理見 [原型紀錄](slint-prototype.md)。S0–S8 goal active，S9 未執行。


2026-11-09：`576f40e` 共用最近看板狀態、revision、待寫合併及清除屏障；`42e86a9` 接入 Slint 快速前往／最近看板、⌘K／Ctrl+K 與書籤前往草稿交接。導覽共用原生單一 receipt 欄位及 Session 工作，切板、AID／URL、立即到達、錯誤／取消及晚到回覆已有完整元件證據；退出另驗一份待收集工作及一份待寫資料均完成。macOS 實際快捷鍵、兩次切板與同板到達、最近清單、雙檔清除及等待中 ⌘Q 分列驗證；沒有將按住 Return 當成 IME 或實際 repeat 事件計數通過。原生目前／列表文章收藏、推文導覽／更新及其餘 S0–S8 缺項繼續，S9 未執行。詳細 hash、範圍與清理見 [原型紀錄](slint-prototype.md)。

2026-11-09：`502cdb7` 接上目前／指定列表文章收藏，沿用共用 Acquisition／FromList、同一 Session 核對及原子封存交易。最終五組 strict peer 核對目前、無標頭、指定列、取消與失焦；macOS 實際浮球收藏、離線讀至第45行及 Quit 有分立證據。新程序已解碼／保存相同副本，但原生重開操作未完成；列表實體右鍵、完整焦點／IME 與其他平台仍待驗，不推定同時操作就是失敗原因。詳細來源、負向紀錄、回歸及清理見 [原型紀錄](slint-prototype.md#s4-目前與列表文章收藏2026-11-09)。S0–S8 goal 持續，S9 尚未切換／整合／發佈。

2026-11-09：Slint 已接入共用 Rust 原PO／確認登入的自己／指定帳號推文索引、原生群組與循環定位、可見側標、背景右鍵入口。完整元件測試核對縮放／捲動、搜尋交接、遮罩／還原、模式與 scope 清理、重繪 ACK 保護和零站端輸入。這批 native bin79項、全 target Clippy、native-player-probe 編譯與全文／輸出／收藏回歸通過；列表收藏一次取消、同 binary 獨立重跑通過，負向紀錄保留。macOS 原生視窗可顯示側標，但 CUA 點擊未取得前景焦點，實體按鈕／鍵盤保持 not_ready；兩次獨立原生重試 CmdQ 的 Close ACK／零額外輸入／HTTP joined 有記錄，不把正常退出替代操作驗收。細節見 [原型紀錄](slint-prototype.md#s4-原生推文定位與身分側標2026-11-09)。S4、S8 及完整 S0–S8 尚未完成，S9 未執行。

推文導覽批次 `40e4749` 驗收後再清理 target/release（Cargo2.7GiB）、上一個收藏 App與冗餘正向影像；target回到12KiB。必要一般配置最新包改為 `.local/verification/slint-reply-navigation/Native-Replies.app`，影片配置與Windows S1 ZIP保留；來源265檔及必要包hash不變。第二份 exact清單與free前後在 `.local/verification/slint-reply-navigation/cleanup.json`，仍依每批規則執行。

2026-11-09：Slint 推文自動更新已接入同一 SurfaceState、5秒 deadline與單一 receipt owner；預設關閉、顯式開啟、未讀提示／跳底、操作暫停、取消／失焦保留原文與圖片及站端復原皆有完整 App證據。與 Dioxus 共用的既有 peer 驗兩次追加、一次未變尾段核對與晚到 Q；三種圖片案例各只下載一次，10個最終 native CLI案例皆通過。macOS 實體長按 Right／Enter、復原按鈕、150%無輔助縮放的站端游標可見及正常關窗另通過；完整更新控制／混合 Live、IME與四架構仍待驗。已補同來源幾何 ACK 的游標捲動，沒有新增更新 timer／sender task。詳細來源／負向／擷圖限制見 [原型紀錄](slint-prototype.md#s4-原生推文更新與站端復原2026-11-09)。S0–S8仍 active；S9未執行。

推文更新批次 `384c899` 驗收後再清理自己的release profile、上一個推文定位App與3個冗餘正向PNG，target回到12KiB；必要一般配置最新包改為 `.local/verification/slint-reply-updates/Native-Updates.app`。清理前後、保留包hash及source267檔不變在同批 `cleanup.json`。接續混合Live時仍須核對保留／復原中的推文側標、底色、尋找與媒體交互及完整可信OS控制；本批三個Live fixture不涵蓋所有角色標記組合。

2026-11-09 清理追補：使用者再次要求先處理磁碟產物，已在確認沒有程序／open files 使用後，以 Cargo 原生 clean 移除 Slint worktree 的 `target/release`（5658 檔、Cargo 宣告2.0GiB；du由2122336KiB降至12KiB）。本次 filesystem available由245599801344增至247608643584 bytes，僅記觀測差額，不全部歸因於本次刪除。混合閱讀探針尚未通過，保留該版完整未提交patch、來源基準、binary hash及失敗報告，沒有把失敗標為驗收。485份來源／資產／文件hash與3份必要交付hash均不變。exact path、lsof及前後記錄在 `.local/verification/slint-artifact-cleanup-20261109-followup/cleanup.json`；其餘聊天產物和未整合worktree保留。後續仍逐批清理，不因goal持續而累積歷代target或App；本次清理後不為驗證清理而重建Rust。

2026-11-09：混合推文角色、指定帳號底色、PNG／GIF與wide／搜尋交互已補完整元件證據，修正取消後側標／本機定位消失、部分編碼頁清掉焦點及暫時viewport夾限覆寫anchor。最終同binary六案及九項回歸通過；Rust共用／兩前端578項、JS383項與Clippy／native-player-probe編譯通過。兩輪更新失焦取消保留原log，同binary獨立重跑通過。macOS實際群組／下一則有部分證據。影像追補：重新讀取原始JPEG／PNG後撤回「正文缺字後恢復」推論；後續座標定位／復原及無自動代按的更新控制另驗。完整OS矩陣仍未完成。完整來源、hash、負向與剩餘範圍見 [原型紀錄](slint-prototype.md#s4-混合推文角色尋找與媒體保留2026-11-09)。S0–S8持續，S9未執行。

混合閱讀修正 `c817538` 驗收後依逐批規則清掉本Slint `target/release`（Cargo宣告12936檔、4.9GiB；target的du由5251928KiB降至12KiB），並移除已被同功能回歸涵蓋的舊 `slint-reply-updates/Native-Updates.app`（du65708KiB）。該App只留2574bytes非執行metadata ZIP；最新一般配置改為 `slint-mixed-reading/Native-Mixed.app`，影片配置與Windows S1 ZIP仍保留。267份src及3份必要交付hash不變，未重建Rust；精確resolved paths、owner、symlink／lsof、保留例外及free前後在 `.local/verification/slint-mixed-reading/cleanup.json`。filesystem available 237879652352→242873319424 bytes只記觀測差額，不全歸因於清理；原生正反畫面、未整合source／worktree及其他聊天產物保留。

2026-11-09：重讀原始JPEG、確認兩份曾被判成不同呈現的PNG逐byte相同並解碼文字像素後，撤回先前「cache／snapshot漏字」推論，沒有為內嵌輸出改產品繪圖。既有binary補驗macOS座標推文定位／150%站端復原，以及完全不代按或合成前景的手動更新／51行／6未讀／查看／暫停51.57秒零新命令／Quit。舊AX與焦點負向保留，完整鍵盤／IME／其他平台不由基本按鈕路徑替代。原始證據與範圍見 [原型紀錄](slint-prototype.md#s4-原始影像複核與手動原生更新2026-11-09)。沒有重建Rust或新增App副本，target仍12KiB；S0–S8持續，S9未執行。

2026-11-09：新增不代按的native-list loopback入口及僅探針使用的持久資料檔選項，macOS實際右鍵第一篇（站端游標起初在第二篇）完成13步指定收藏；新程序直接讀同一檔，實際斷線後開啟副本、翻至45行並在離線視窗Quit。原始45行／80欄／ANSI青帶及全檔hash、零HTTP／零站端閱讀命令、兩程序close與worker回收分開驗證。最終binary11案、Rust457項、Clippy／native-player-probe編譯與fmt通過。原先預建verification目錄被安全拒絕的負向保留。完整scope與原始證據見 [原型紀錄](slint-prototype.md#s4-列表指定收藏與新程序斷線讀回2026-11-09)；其他平台、完整資料回退及候選打包仍未結案，S9未執行。

列表原生驗收批次 `c3c75e6` 完成後，以Cargo原生clean移除本Slint target/release（8385檔、2.7GiB；du原2848280KiB），並清掉上一個 `slint-mixed-reading/Native-Mixed.app`（du65736KiB）及11個被最終同案取代的首輪正向generated目錄。舊App僅留2571bytes非binary metadata ZIP；來源／hash／log、唯一啟動失敗及原始OS擷圖保留。target回到12KiB，最新一般配置測試包改為 `slint-native-list/Native-Bookmarks.app`，同一收藏檔、影片配置與Windows S1 ZIP保留且hash不變，267份src不變。精確刪除路徑、逐檔hash、owner／symlink／lsof及保留例外見 `.local/verification/slint-native-list/cleanup.json`。filesystem available 238569623552→241283104768 bytes只記觀測差額；不全歸因於本批，不重建Rust，也不刪其他聊天產物或未整合worktree。

2026-11-09 即時產物清理：依使用者再次要求，盤點本 Slint worktree 所有 target、歷代測試產物與必要交付，確認軟體影片批次已無程序／open files 使用後，以 Cargo 原生 clean 移除 `.local/verification/slint-video-software/target/release`（5717檔，Cargo宣告2.1GiB；target的du由2242236KiB降至8KiB）。worktree的du由3060352KiB降至818132KiB，filesystem available由234602717184增至236684951552bytes，差額僅為觀測、不全部歸因於本次刪除。496份來源／文件與1192份必要交付／資料hash不變；未提交的CPU影片來源及失敗紀錄保留，只移出一份當前執行檔與Skia授權至同批 `pending-validation/` 供後續驗證，沒有保留整包依賴快取或另外建立App副本。已結束的Rust測試及18組影片案例有結果，但CPU原生呈現／閱讀回歸／Clippy與跨平台仍待驗，不能因清理而稱該功能完成。最新完整影片候選包、已驗收的一般software配置App及唯一Windows S1 ZIP仍有各自用途；canonical Dioxus產物歸屬未確認、未整合Slint來源worktree亦保留。exact paths、逐檔盤點、idle檢查、保留理由及前後空間在 `.local/verification/slint-artifact-cleanup-20261109-now/ledger.json`。往後每批及中途清理依[開發harness](development-harness.md#產物清理與結案)執行，本次未重建Rust。S0–S8 goal持續，S9未執行。

2026-11-09：`4a7db4c`完成明確software compositor與原生CPU影片worker，輸出最高1280×720、單一latest frame／有界wake、沿用原Session及退出所有權；macOS實際四色／seek／Play／Pause／Quit與資源清理有分立證據。接續修正候選無參數仍進離線原型的缺口，網路禁止環境的原生啟動／失敗提示／Quit及資料不變已驗。最終候選18組影片與2組混合案例通過，但閱讀9案中的收藏目前文章出現間歇失敗，CPU及GL重複實驗都保留反例；整體回歸明列failed。下一步追查cold store讀取前後的source／stamp／ACK，不以重跑或更改expected結案。精確binary、參數、驗證範圍、原始失敗與剩餘缺項見[原型紀錄](slint-prototype.md#s2s8-候選無參數啟動與收藏反例2026-11-09)。S0–S8持續，S9未執行。

啟動修正以`6c6a8e1`提交；舊完整GL候選另重現同一收藏失敗。批次結束已清掉自有checks-target/release（Cargo8385檔／2.7GiB）及兩代被取代App／ZIP／DMG與舊待驗執行檔（payload du621580KiB），worktree du770120KiB、所有自有profile已清空；496份來源與1192份必要交付／資料hash不變。保留最新 `slint-startup/package-current`、不同feature的舊software收藏對照與唯一Windows S1 ZIP，沒有因failed回歸留下整包編譯快取。exact paths、保留例外與free觀測233476833280→236733288448bytes見同批 `cleanup/ledger.json`；清理後不重建Rust，完整S0–S8仍未達成。

2026-11-09：已重現並修正cold書籤檔讀回後收藏停在Loading：結果回呼仍在output-busy gate內，原先只靠可被合併的changed邊緣恢復；現在I/O owner完成join／回呼並清掉gate後明確通知，Loading沿用既有30秒capture deadline，不改來源／ACK保護。CPU／GL各10次目前收藏加列表／無標頭／取消／失焦共24案，以及最終候選18影片＋9閱讀＋2混合共29案皆通過。macOS原生浮球收藏、保存後本機讀至45行、⌘Q及獨立JSON／strict peer／零HTTP／owner清理另驗，Rust486／兩配置Clippy／fmt通過。此單項問題結案；既有GPU偶發錯色、其他平台／IME、對應來源交付、完整回退及效能未由本批代替。來源、hash及scope見[原型紀錄](slint-prototype.md#s4-本機檔案完成後恢復收藏2026-11-09)。S0–S8 goal持續，S9未執行。

收藏修正`3cf91c2`驗收後已清理自有release profile（Cargo8417檔／2.7GiB）、上一代完整候選App／ZIP／DMG及已完成對照用途的舊Native-Bookmarks.app（payload du343544KiB）。現在只留 `slint-capture-ack/package-current` 最新完整候選及唯一Windows S1 ZIP，兩份native合成收藏資料、原始失敗與source保留；worktree du719176KiB、所有自有profile已清空。496份來源與1185份必要交付／資料hash不變，free觀測233442390016→236464750592bytes不全部歸因於清理。exact paths與檢查在本批 `cleanup/ledger.json`；清理後不重建Rust，S0–S8仍active。

2026-11-10 即時產物清理：依使用者要求先處理磁碟，確認無程序／open files 使用後，以 Cargo 原生 clean 清掉 `slint-perf-macos/target/release`（4257檔、Cargo宣告1.2GiB；du1297828→12KiB），另刪兩個已保存最小證據的系統暫存目錄，以及此次獨有 `io.github.yam276.pttstella.slint.perf-dioxus` 的 Library/WebKit／Caches（合計du984KiB）。worktree du2062432→764624KiB；free233114390528→234357141504bytes只是並行期間觀測，不能全歸因於清理。498份來源／文件與1203份保留檔案hash不變，未重建Rust。最新Slint完整包、唯一Windows S1 ZIP、約39MiB的一份同樹Dioxus對照App、未提交量測工具、使用者資料與獨特證據保留；canonical及其他聊天產物未動。所有自有target均僅餘8–16KiB紀錄。exact五筆刪除路徑、逐檔owner／resolved path／symlink、idle檢查、保留理由與空間在 `.local/verification/slint-perf-macos/cleanup/ledger.json`。效能校準不等於同負載效能通過，S0–S8仍active；後續每批及中途清理繼續依harness執行，包含target外的專用WebKit cache與暫存。

2026-11-10：macOS程序族採樣及穩定區間CLI接入開發tooling，PID＋start、獨立responsibility／PPID、Mach CPU單位校準、physical footprint與RSS分列，拒絕共用scope／重用／membership變動／counter reset，保留partial trace。完整Python126項（含4個原生校準）通過；先前兩個實際host smoke只驗程序歸屬與各自格線／Close，不符合共同視窗／焦點／次數，沒有比較勝負。確切來源、命令、限制與正式比較前置條件見[macOS量測入口](slint-macos-measurement.md)。自有profile已清，後續沿用保留程式；完整S8效能／四架構與其他S0–S8仍待驗，S9未執行。

2026-11-10：新增完整服務的顯式 `--probe-benchmark NEW_DIRECTORY`，沿用Live的服務掛載判斷、隔離store／synthetic vault及嚴格本機peer，避免把只畫Frame的舊grid探針當完整host比較。16項實際服務／watch檢查、release格線／幾何安裝、macOS CUA畫面及⌘Q／exit0／Close ACK已分立核對；Rust466項＋6既有明確ignored、全target Clippy通過。原型常駐列使1040×740視窗的格線僅736×442，CUA前景未證明，因此仍不發布效能勝負。下一步依產品浮球契約收斂正常原生chrome，再做共同viewport／前景與完整負載。首輪sandbox／打包／識別及SIGTERM負向保留，細節見[macOS量測入口](slint-macos-measurement.md)。S0–S8仍active，S9未執行。

完整服務入口 `ccd1b8c` 驗收後，先前dev profile已以Cargo原生clean移除7604檔／3.6GiB及失敗strip copy；最後再清掉release profile8232檔／2.4GiB、診斷App、六個本批系統暫存目錄、既有輸出負向資料及本批產生的單一採樣器pycache。release target的du2611368→8KiB，worktree3520888→768704KiB，free231442927616→234030350336bytes只記觀測、不全歸因本批。513份來源及1203份保留檔案hash不變；新入口接續的viewport／chrome工作必須改來源，因此不另存本次machine binary，保留最終原生PNG／設定／準備報告、source／hash及唯一失敗。最新Slint完整候選、唯一Windows S1 ZIP及一份Dioxus對照保留；兩份小型舊bundle helper pycache歸屬不明保留，未動canonical／其他聊天或未整合worktree。exact九筆主刪除路徑與額外pycache清單、逐檔gzip盤點、idle／du／Cargo／free及保留理由見 `.local/verification/slint-benchmark-session/cleanup/`；清理後未重建Rust。

2026-11-10：Live與完整benchmark共用產品shell，將原型常駐列收至既有浮球／設定，補上草稿及Slint授權入口，保留閱讀情境控制；無文章控制列時viewport1040×740、格線1040×624，固定PNG逐byte不變。連線phase的持續提示與本機通知分離、通知不占格線；純連線狀態不顯示無效X。最終候選CPU／GL shell、兩浮球、18影片／9閱讀／2混合共33案，以及macOS實際授權／Escape焦點／斷線／Quit／exit0／Close ACK通過；前版native125%／100%及240秒監督逾時的負向分hash保留。Rust466項＋6明確ignored、兩配置Clippy、Python126及fmt通過；actionlint缺工具not_ready。細節及必要待驗見[macOS量測入口](slint-macos-measurement.md)，最新完整候選為 `slint-shell-chrome-final/package-current`。成功淡出／通知焦點、有效scope草稿新入口、完整原生鍵盤／IME及各工作負載viewport、四架構／回退／效能仍待驗，S0–S8持續，S9未執行。

產品shell來源以 `67738f9` 提交後，確認無程序／open files使用，再清掉同批release profile（Cargo8410檔／2.7GiB）、上一代完整候選與首版App／ZIP／DMG、34個被最終同案取代的generated目錄及兩個系統暫存目錄；exact共43筆見 `slint-shell-chrome-final/cleanup/ledger.json`。target du2910680→8KiB、worktree4248212→776760KiB，free230704709632→233953509376bytes僅記並行／APFS觀測，不全部歸因本次清理。514份來源及1203份保留檔案hash不變；唯一新候選改為 `slint-shell-chrome-final/package-current`，保留Windows S1、Dioxus對照、原始資料、首次無效X／逾時反例、來源patch／hash與最小metadata，不存完整舊binary archive。gzip逐檔盤點、lsof／owner／resolved path、du／Cargo及保留理由皆在同批cleanup；canonical與其他聊天及未整合worktree未動，清理後未重建Rust。

2026-11-10 通知關閉回歸：`shell_probe.rs` 改為以實際standard Button的toolkit pointer事件確認關閉錯誤通知、畫面大小不變、焦點仍在終端且沒有站端輸入。重用清理時保留的CLI，software與OpenGL兩配置均通過並收到嚴格peer Close ACK，未重建Rust。未重現pointer關閉遺失焦點，因此沒有改生產callback；keyboard Tab／搜尋欄／OS pointer與IME仍待驗。來源patch、binary hash及兩案結果見 `.local/verification/slint-notice-focus/acceptance.json`。批次完成後刪除該待驗CLI（約65MiB），未替換最新已驗收App。

2026-11-10 S0／S8跨平台影片建置邊界：原 `build.rs` 在交叉編譯且未設 `MPV_LIB_DIR` 時會執行主機pkg-config，合成Windows目標已重現它輸出主機library search path。現改為比對Cargo HOST／TARGET；交叉編譯必須明示目標libmpv目錄，缺少Cargo identity也報錯，不嘗試主機pkg-config。原生編譯仍可使用pkg-config，明示有效目錄的原生／交叉編譯沿用原路徑；目錄存在不代表library架構／ABI／runtime已驗收。`tooling/test_mpv_build.py` 直接編譯並執行同一production build.rs，以本機可執行的pkg-config替身驗證7案（包含前述red→green、明示路徑、錯誤、無影片配置）；不編譯Slint相依，也不冒充跨平台link或原生執行。全133個Python測試及core gate 14命令／2replay通過；來源、修前／修後結果及清理ledger在 `.local/verification/slint-mpv-cross-discovery/`。Windows／Linux／macOS x64的實際libmpv來源交付、連結、打包與原生驗收仍待完成。

上述跨平台建置檢查完成後，以Cargo原生clean移除 `slint-mpv-cross-discovery/target/debug`（1561檔／268.3MiB），target的du由279660KiB降至8KiB；測試的獨立rustc執行檔由TemporaryDirectory清理，無殘留。1546份受保護來源／資產／交付hash不變。filesystem free 230992998400→231269281792 bytes只記觀測差額；清理後不重建Rust。exact paths、du、Cargo與open-file檢查在同批 `cleanup.json`。

2026-11-10 Windows x64完整主程式候選：重用既有cargo-xwin0.23.1／Windows SDK，以Rust1.97在macOS arm64交叉編譯 `native-player-probe`／`pttstella-slint`，包含桌面服務與播放器；不只S1探針。首輪實際PE顯示CUI子系統，已加上與Dioxus相同的release Windows GUI屬性，重建後由共用PE parser及獨立LLVM工具確認x64／GUI與libmpv匯入。取用mpv官方v0.42.0 LGPL x64 ZIP（SHA256 `8e5f77430efc53ef175804ba6618527e3af0448a0dcabf04d3cc72659d1a6fce`），原GNU COFF import archive僅改副檔名為mpv.lib，MSVC／LLD小型連結與完整主程式均通過；沒有把主機mpv用於目標連結。[mpv官方版本](https://github.com/mpv-player/mpv/releases/tag/v0.42.0)

新增 `tooling/bundle_slint_windows.py`，只消費已建執行檔、匹配的來源／binary hash、runtime來源紀錄及自製CC0三格式影片；沿用既有Windows PE普通／延遲匯入parser，遞迴核對主程式可達DLL、拒絕未知／WebView依賴及未用DLL，明列尚需安裝的MSVCP140／VCRUNTIME140。共用parser預設仍嚴格要求GUI，只有顯式runtime檢查允許CUI。ZIP含原生主程式、libmpv、Rust／Skia／libmpv授權、三格式fixture及離線／格線／IME／閱讀／持久合成草稿／影片／show-profile入口。離線入口只連合成loopback；一般exe仍會連研究設定的預設站、不自動登入。所有驗收cmd入口清掉繼承的診斷變數、各次log與探針輸出不覆寫；草稿明示重用包內evidence/drafts供重開驗收。

最新完整Windows候選為 `.local/verification/slint-windows-candidate/package-current/PTTStella-Slint-Candidate-0.5.0-dev.3-windows-x64.zip`，SHA256 `d733f7f5f8508c7b4c67c7de8749a141f70530318463d9b3741b14aa96fd3b59`。Windows target release/all-target Clippy、全143個Python測試、四target／兩配置8案依賴gate、fmt、docs、actionlint與diff檢查通過；獨立ZIP驗證核對CRC、912個payload hash與LLVM匯入表。首輪驗證器曾因同一DLL不同大小寫的重複LLVM條目錯比set，改成先lower再去重，沒有修改產品／ZIP來配合expected。執行環境仍是macOS，未執行Windows exe：原生啟動、注音／鍵盤、播放器圖像／聲音、profile實際路徑、無WebView2啟動、乾淨安裝及完整來源交付仍not_run／not_ready，不宣稱S1／S5／S8完成；尚需Visual C++ x64 Runtime，未內附CRT。原S1無libmpv的ZIP暫留，作為獨立無影片配置的原生比較，不再為新功能保留額外Windows執行檔副本。命令、SDK路徑、source ref／patch、首輪CUI與最終GUI binary hash、兩個runtime的LLVM報告、封裝／依賴／測試結果在 `.local/verification/slint-windows-candidate/`。

Windows候選批次清理：保存最終ZIP及必要授權／來源／唯一失敗紀錄後，先用Cargo profile clean清掉969.3MiB，但交叉target仍有1985388KiB，因此未宣稱清完；再對同一個已確認無使用者、專屬本批的target執行Cargo原生完整clean（5687檔／1.9GiB），target目錄已不存在。另刪首輪被取代的ZIP、下載的重複runtime ZIP、已封裝的DLL副本、重複import archive與小型smoke執行檔／PDB及自己的pycache。保留一份小型mpv.lib與必要license供下一批、既有cargo-xwin／SDK不刪。1644份受保護來源／資產／交付hash不變，Windows批次約98MiB，Slint整個worktree約794MiB；後續清理free 221845131264→223943602176 bytes僅為觀測。首輪cleanup assertion中斷前的完整du／free-before未持久保存，沒有補造數字；兩段Cargo輸出、後續盤點、exact paths與已取代包的前後hash見 `.local/verification/slint-windows-candidate/cleanup/ledger.json`。首次docs gate指出smoke.exe尚在，完成清理後重跑已通過，沒有放寬gate。清理後不重建Rust。

2026-11-10 Linux x64完整主程式候選：以macOS原生Rust／Clang／LLD及經hash核對的target sysroot完成release連結與all-target Clippy，沿用Windows的完整 `native-player-probe` 功能，沒有改release最佳化或Rust來源。新增Linux打包入口，重用既有Linux deb／tar layout，Windows／Linux共用build-record及CC0 fixture admission；既有Dioxus預設名稱不變。fresh Ubuntu24.04實際安裝抓出ELF相依之外的dlopen runtime缺漏，已明列X11／Wayland／GL套件，以同一ELF重封裝，隔離執行檔／套件／桌面ID。最新deb／tar、確切hash、來源／建置方法、入口及限制見[Linux候選說明](slint-linux-candidate.md)。全150個Python測試、docs、fmt、actionlint與diff通過；既有Windows ZIP hash不變，未重建Rust。

此批component證據為Ubuntu QEMU／Xvfb software的shell／relocated launcher／三格式影片／草稿六案通過，原生OS／IME／聲音／GPU／效能仍未驗收。搜尋以正確 `xvfb-run ... dbus-run-session` 順序重用同一ELF：文章兩案仍失敗、終端一案通過且Close ACK，排除DBus順序作為唯一解釋；不能把wrapper exit0寫成三案passed，也尚未區分「背景套用」與「編輯被拒而保留舊命中」。逐案命令、receipt及反例在 `.local/verification/slint-linux-runtime-recheck/recheck.json`，下一步須在接受編輯／Pending／deactivation邊界補診斷。

上述受控重跑沒有新Rust建置；匯出最小證據後，核對task label／container ID與無mounts，再正常stop／rm `pttstella-slint-linux-recheck-20261110`，並移除已無使用者的臨時Ubuntu image。共用VM只做線上TRIM，不關機、不compact／prune。10135份受保護檔案hash不變；filesystem available 221296967680→222357221376 bytes僅記觀測。exact container／guest paths、程序、du／可寫層大小、來源／候選保留理由見 `slint-linux-runtime-recheck/cleanup/ledger.json`。未留下第二套Linux執行檔或測試環境；最新必要交付、來源及獨特失敗保留，S0–S8繼續，S9未執行。


2026-11-10 Linux搜尋暫停反例完成定位：診斷版在Ubuntu文章案例送出編輯前已foreground=false／find_blocked=true且欄位失焦，編輯後draft仍無、timer未啟動，版本與三筆舊命中不變。先前「背景執行待查詢」解讀撤回；生產拒絕背景編輯的保護正確，未修改find_edit／debounce／失焦邊界。探針先確認可接受的synthetic foreground／field focus，接著確認Pending已保留draft／清空命中，再驗證停用仍blocked、零命中與draft保留，最後恢復查詢。Debian與Ubuntu文章／終端六案（含Ubuntu重跑）均通過且Close ACK；分階段診斷與原失敗保留於 `slint-find-pause/`。此為toolkit證據，原生OS焦點與IME仍待驗。

新版Linux候選使用同一完整配置，ELF SHA256為 `9b68eb81ed5348f4dd44f3e05ab5a1a8284a89b884a7240f6c6df22bd7c6d5f7`，取代原a7c00a9包；路徑仍為 `slint-linux-candidate/package-current`。Depends逐字相同，新包在原Ubuntu runtime root重新安裝，shell／含空格路徑portable launcher／三格式影片／草稿／兩種尋找八案均通過、逐案receipt及Close ACK已核對，不冒充另一個fresh clean install。影片另核對非空四色像素樣本、render退休與session join；草稿未測新程序讀回，不擴大scope。Linux release/all-target Clippy、150個Python測試、fmt／actionlint／diff通過，詳[Linux候選說明](slint-linux-candidate.md)；native_acceptance與完整來源交付仍not_run／not_ready。

本批驗證後以Cargo原生完整clean移除 `slint-find-pause/target`（9706檔／3.1GiB，du3247480KiB），卸載並刪除case-sensitive SDK映像、wrapper／外部LLD捷徑、smoke及重複baseline CLI，外部Rust工具不動；兩個自有Debian／Ubuntu容器與無使用者的臨時Ubuntu image已正常移除。舊完整包刪除，只留最小manifest／hash；42張byte-identical generated PNG以保留位置／SHA映射取代（allocated2854912bytes）。10304份受保護檔案hash不變；worktree du889336KiB，filesystem available 214002139136→221830262784bytes只記觀測，不全歸因於本批刪檔。exact paths、owner／resolved path／symlink、lsof／程序、Cargo／du、保留例外及共享VM線上TRIM紀錄在 `slint-find-pause/cleanup/ledger.json`。清理後不重建Rust，S0–S8持續，S9未執行。


2026-11-10 macOS x64完整主程式已建置：Rust1.97.0／Apple Clang從arm64交叉編譯完整native-player-probe，Rust來源仍為d37190d、Rust patch為空；CLI SHA256 `b1e810584025eb0d4fc65992045c7eb99ae5b25bdd0e4e9bbed6831453731e64`。實際x64 libmpv已在Rosetta初始化，完整CLI的軟體／OpenGL shell、兩種尋找、三格式影片及軟體草稿共13案exit0／Close ACK，逐一檢查非空像素樣本、render退休、session join及搜尋Pending／背景保留／恢復；這是Apple Silicon Rosetta component證據，不算Intel實機／OS輸入／IME／可聽音訊／效能。x64 release/all-target Clippy及154個Python測試通過；新增Mac原生Clang／dyld fixture與可跨host執行的loader／notice負向測試，沒有把skip或wrapper exit0當案例通過。細節與尚缺欄位見[macOS x64待封裝輸入](slint-macos-x64.md)。

x64 runtime暫取自經GitHub digest及完整簽章核對的IINA1.5.1，僅保留44個可達x64 dylib，實際回報mpv0.41.0／FFmpeg9.0.1，與arm64的Homebrew0.42不同。釋出日recipe版本及Credits不能直接代替此binary的精確來源／完整notices，封裝gate仍not_ready；尚未產出x64 App／ZIP／DMG。共用collector已修正dyld父層RPATH繼承，保留各owner上下文與allowed-prefix拒絕；只由主執行檔擁有Frameworks RPATH，避免在沒有headerpad的vendor dylib重複加路徑。非Cellar相依缺來源／notice owner時明確拒絕，不能默默跳過並輸出空清單。這份CLI留本機developer RPATH，不能當可攜包移交；後續來源不變時重用它做Python封裝，不重建Rust。

同輪DYLD shell追蹤仍有六筆System WebKit／JavaScriptCore相關映射，同時載入WritingTools／WritingToolsUI；沒有以source無直接WebView依賴換算成執行期零映射，也未宣稱由此證明WebView實例或完整IME的成因。此差異持續依S8處理；新Rosetta／不同runtime的影片通過不結案原arm64 MP4偶發問題。

本批以Cargo原生clean移除8281檔／2.5GiB（target du2634744KiB），刪除完整IINA DMG／掛載、完成的C fixture執行檔、自有pycache及17張重複generated PNG。10497份受保護檔案hash未變；待封裝CLI及必要44個dylib、來源／簽章／版本／獨特失敗與最小圖像證據保留。清理後以同一CLI及自有runtime重跑MP4 OpenGL，像素／清理／Close ACK通過，證明該案例不依賴已刪target或掛載。batch du130928KiB、Slint worktree1020184KiB，filesystem available 218310406144→221069762560 bytes只記並行觀測。exact paths、owner／resolved path／symlink、lsof、Cargo／du及保留例外在 `slint-macos-x64-candidate/cleanup/ledger.json`；新增Rust x64 target屬已安裝工具保留，未動共用VM或其他聊天。清理後沒有Rust產品重建，Python gate的獨立C／rustc fixture在TemporaryDirectory自動清理；S0–S8仍active，S9未執行。

2026-11-10 macOS x64候選補齊App／ZIP／DMG：重用d37190d完整Rust CLI，從已核對來源建置mpv0.42.0、FFmpeg9.0.2、libplacebo7.387與完整32項依賴，取47個reachable dylib。31項完成後mpv的Swift MetalLayer引用失敗，按實際Vulkan條件加guard，只重編mpv約20秒通過；mujs的build-relative install ID另由producer recipe與既有install修正，沒有放寬collector。來源manifest綁定archive、成功build receipt及實際輸出SHA，提取116份notice，無owner／失敗build／錯來源或輸出／越界與過大輸入會拒絕。實際C API為mpv v0.42.0、client API2.6、FFmpeg9.0.2，FFmpeg license回報GPLv3 or later；完整產品來源交付與乾淨重播仍not_ready，不能由notice gate替代。

最新x64候選在 `slint-macos-x64-candidate/package-current/`，App主程式SHA `3908f73e6f12f9b2ca54ff0b24530a557a8df56bbc21fffef9b2a39d976b60df`，ZIP／DMG及精確scope見[候選說明](slint-macos-x64.md)。1159份payload在App、ZIP、唯讀DMG逐檔一致；48個Mach-O皆x64，ad-hoc簽章deep／strict與掛載後隔離profile通過。將App搬至含空格路徑，兩套開發runtime prefix同時不可用且無DYLD／LD搜尋覆寫，CPU／OpenGL shell、兩種find、三格式影片及CPU草稿13案各有receipt、非空影片四色像素／render退休／session join及Close ACK。另一次封裝shell載入trace有六筆System WebKit／JSC相關映射及WritingTools／UI；不能宣稱零runtime WebKit或推定WebView實例成因。這是macOS15.7.7 arm64／Rosetta component證據，不算Intel硬體／OS輸入／IME／可聽音訊／效能，也不結案arm64 MP4偶發錯色。S0–S8持續，S9未執行。

本批165個Python測試（含真實macOS loader fixture）、fmt、actionlint及diff通過。驗收後移除11個自有obsolete輸入／工具／native build／臨時App路徑，du499156KiB；舊IINA runtime／待封裝CLI由完整候選取代，原始來源與49份recipe／patch輸入、失敗診斷及最小證據保留，沒有Rust重建。24張byte-identical PNG改為SHA／保留路徑映射，16張獨特PNG保留；最終11358份保護檔案及32份來源archive雜湊一致。filesystem available 219832209408→220345688064bytes僅為並行觀測，worktree du1495272KiB，來源交付292272KiB、x64完整候選297812KiB，Cargo target16KiB。清理後同一最終App重跑OpenGL MP4，像素／render退休／session join及Close ACK通過。exact paths、owner／resolved path／symlink、lsof／程序、du／free及保留例外在 `slint-macos-x64-source/cleanup/ledger.json`；shared工具／registry／VM、其他聊天及canonical未動。來源worktree尚未整合，保留全部待整合提交；S9未執行。

2026-11-10 S7版本檢查已接線：Live啟動一次、手動查詢與下載／repository外開沿用共用Checker及固定Rust來源；正常HTTP失敗靜默且不假稱最新。既有Tokio runtime新增一個有界command slot／最新status owner，無新thread／idle timer；關窗先撤銷排隊意圖，再取消並join worker及Slint listener。一般probe／benchmark零公開請求，明確releases案例只用IPv4 loopback。兩個新fixture曾因缺少每client的rustls provider卡住，保留明確panic／失敗後修正，未安裝全域provider；等待與close audit均有deadline。

完整更新案例已過操作階段卻退出45秒逾時，取樣CPU98.1%與Video::step／close堆疊證實root消失後閒置watcher自我Notify；改成一次closed／撤銷，原生命週期排空。CPU／OpenGL更新、播放中CPU Quit／GL Close四案在約1–1.6秒內exit0／Close ACK；後續更新案例另由按鈕焦點／空白鍵觸發手動重試，peer獨立要求三次HTTP及停滯body的EOF／reset。離頁／關閉拒絕、惡意JSON URL忽略、設定bytes不變、worker與listener實際join均核對。這是macOS arm64 toolkit證據，不算OS鍵盤／IME／browser／可及性或其他平台完成；原arm64 MP4偶發錯色仍未結案。詳細來源／反例／命令見[版本檢查與退出回歸](slint-release-check.md)。

本批library377 passed／4 ignored、binary83 passed／1 ignored（影片修正後另重跑binary）、all-target Clippy及165 Python通過，未將ignored算passed。新arm64 App／ZIP／DMG取代原c9413d3，仍在slint-shell-chrome-final/package-current；主程式SHA bbfde96110519ab1d1149b807dc57532bfe6a45df219795a88b473eb1c119ba4，詳候選紀錄。1176份payload跨App／ZIP／唯讀DMG一致、51個Mach-O皆arm64；禁止讀取Homebrew的sandbox下更新CPU／GL、shell及MP4四案有receipt／Close ACK。完整OS／平台來源交付仍not_run／not_ready，其他架構包須同步這批Rust變更。

本批清理Cargo原生8264檔／2.5GiB（du2666732KiB），舊arm64完整包（du277948KiB）、兩個中斷probe的自有OS設定暫存與23張重複generated PNG移除；保留13張獨特圖片、最小來源／失敗／metadata及最新必要交付。最終10289份保護檔案與12份Rust／Slint來源hash一致，worktree du1500908KiB。filesystem available 217673043968→220457906176bytes只記觀測。精確路徑、owner／resolved path／symlink、open files、Cargo／du／free與保留例外在slint-release-check/cleanup/ledger.json；清理後同一候選於無Homebrew sandbox重跑更新／空白鍵／取消／Close ACK通過，沒有重建Rust。未動canonical、其他聊天、共用SDK／registry／VM，未整合來源worktree保留，S0–S8持續，S9未執行。

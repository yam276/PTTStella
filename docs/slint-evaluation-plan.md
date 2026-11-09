# PTTStella Slint 平替實作與驗收計畫

從既有 Rust 終端核心和 macOS Slint 原型出發，做出能讀寫現有 PTTStella 資料、保留目前操作並可安裝在 Windows、macOS、Linux 的原生桌面候選版。以實測決定是否將 Slint 改為正式入口，再移除 Dioxus 與 WebView 發佈相依。

Dioxus 初始基準為 `5ea9ad6a498331d0cbebc7d9803e0a51df46e5c3`，已包含分段接續長文、大型 GIF 與選用的自動開圖。Slint 起點為獨立 worktree 內的 `slint-research`／`pttstella-slint` 原型。原型已在 macOS arm64 實測雙色字、80×24、放大拖曳、全螢幕及獨立輸入欄的威注音。實體 Retina、終端內組字、跨平台原生輸入和與 Dioxus 的同負載效能比較仍待驗。實驗數據及已知差異見 [離線原型紀錄](slint-prototype.md)。

## 目前進度（2026-11-09）

**尚未達到可平替 Dioxus 的驗收條件。** 現有 Slint 來源及計畫已提交，正常影片退出以 `1b2bb96` 修正。下表區分已接線與尚須完成的驗收；歷次結果及確切 scope 見 [原型紀錄](slint-prototype.md)。

| 階段 | 目前已有 | 尚須完成 |
| --- | --- | --- |
| S0 | 共用 Rust 服務、獨立 Slint 入口及可編譯提交序列 | 完整功能案例對照、四 target 依賴與最終基準重跑 |
| S1 | 格線／雙色字／縮放、macOS 原生輸入的分立證據 | 候選窗與跨螢幕 DPI、可及性、Windows／Linux 原生 |
| S2 | 共用 Connection、登入／保活、站點表單、嚴格切站／雙編碼 bytes | 同 endpoint 編碼切換、預設站重啟／不登入、完整信任／vault 原生矩陣 |
| S3 | 選取、輸出、浮球／選單及原生互動的分立證據 | 全部快捷鍵／pointer／modal／可及性操作矩陣 |
| S4 | 雙閱讀模式、長文取頁、尋找、名單／右鍵／揭露／底色、書籤／離線、快速前往與最近看板、目前／指定列表文章收藏、共用推文索引／原生定位與側標、推文更新／未讀／保留與復原 | 推文控制完整實體操作、推文定位實體操作、列表實體右鍵及新程序離線操作補驗、混合 Live 等效與全功能回歸 |
| S5 | 圖片／GIF／文內展圖、主視窗影片；正常關窗／Quit 的 GL 清理 | 多圖峰值、GL 恢復、音訊、四架構可交付 runtime |
| S6 | 原文編輯／色票／站端上色、取消／部分失敗、草稿故障與多開復原 | 原生 IME 候選／完整輸入邊界、跨平台操作；舊末字異常原因仍未定 |
| S7 | 原生設定八分類、語言／站點／名單、24項搜尋定位、具名設定檔及正常退出 owner | 其他管理入口、完整 Dock／modal／OS I/O 退出矩陣、雙版本資料往返 |
| S8 | S1 Windows 探針及 macOS 本機實驗工具 | 完整四架構候選包、使用者 Windows 原生驗收、回退及同負載效能；尚未通過 |

接續推文控制／定位實體操作、列表實體右鍵與新程序離線操作、混合功能／退出／資料往返案例及完整候選打包。使用者已確認可在 Windows 執行驗收包。缺少 runner 的項目保持 `not_ready`，不將 macOS 或 toolkit 結果換算成跨平台完成度。S0–S8 goal 持續；S9 正式入口與發佈尚未執行。

## 執行方式與完成條件

- 實作在指定 Slint worktree 內連續推進。依下列 S0–S8 的相依順序完成程式、回歸和證據。缺 runner 或硬體時登記確切缺項，繼續可獨立驗證的階段。若原生輸入、雙色格線或原生播放器證明不可行，先修正或換成可驗收的技術路徑，再擴大相依功能。階段證據整理完即可接續，日常實作與重跑沿用本計畫。
- 每階段做成可獨立驗收的變更，遵守 [AGENTS.md](../AGENTS.md) 的分支、提交與清理規則。需要新分支、推送、實站操作、切換正式入口或發佈時，依本 repo 規則和使用者當時授權執行。測試與研究 App 使用獨立 App ID、設定／資料目錄、診斷及密碼庫服務，合成探針只連本機。
- 候選版須通過所有既有功能、四個目標架構安裝、舊資料讀寫與回退，以及下文效能門檻。S8 交付可審閱的候選包、可逆切換方案與結論。使用者接受後才進入 S9。若 `develop` 推進，切換前以待整合的實際 commit 重跑變更相關項目和整體效能，新增功能納入驗收。

## 產物保留規則（2026-11-09 使用者要求）

已測完且被取代的產物包含過去累積與之後新增，均於每個可獨立驗收批次完成時清理，沿用 [task-owned 清理流程](development-harness.md#產物清理與結案)。不再為每階段各留 App，也不因完整 goal active 就長期保留 target/debug／release；只在同一批工作中重用。一般 dev／test 命令明示 `CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0`，release 設定不變。已確認無使用者的編譯快取用 Cargo 原生 clean，必要候選包與最小來源／驗收／失敗證據移出 target，保留原因及 exact removed paths 另記。不啟動背景刪除服務。

2026-11-09 已回收 main Slint target 的 dev／release、獨立 Windows 交叉編譯 target，以及34份過時 macOS 測試 App。當前只保留 Slint 一般候選、影片配置候選及唯一 Windows S1 ZIP；兩個最小 reopen bundle metadata 目錄沒有 native executable。被刪 App 的非執行紀錄／授權／測試資源壓成約3.2MB ZIP，完整 binaries 不存進 archive。精確清單、來源雜湊、lsof、Cargo 輸出與空間前後在 `.local/verification/slint-artifact-cleanup-20261109/`。Slint target 12KiB、verification 約416MiB；原始碼、未提交的推文導覽工作及必要交付內容不變。canonical checkout 的其他聊天產物不屬於本 Slint goal 所有權，保留而未跨案刪除。

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

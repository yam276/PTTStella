# PTTStella 最簡 terminal 計畫

狀態：**畫面核心、Windows 視窗與 WSS 登入前連線已驗證；手動登入驗收待使用者實測**。第一個驗收平台為 Windows；Rust／Dioxus 維持可攜，macOS／Linux 後續各自驗證。

## 目標

開啟視窗、自動連上 PTT、手動輸入帳號密碼、看見登入後畫面，最後可正常斷線。文字、固定排版與雙色字由核心與 renderer 實作。

使用者已明確補充：只有外觀像 terminal，並非真實系統終端。本產品不開 shell／PTY，不追求通用 terminal 相容。

初期 terminal surface 已擴充本機操作功能、依 [導覽契約](../navigation.md) 的最小滑鼠導覽，以及目前畫面連續行 PNG 截圖。本階段依 [媒體 MVP 規格](../media.md) 加入按需媒體預覽。沒有獨立文章 UI、板名導航按鈕、登入腳本、自動輸入／記住密碼或背景追蹤。

## 本階段：按需媒體預覽

使用者已授權寫規格後實作。範圍為圖片／動圖、直接影片網址、YouTube 縮圖與外開；一次預覽一個媒體，不提前載入整篇文章。

| 工作 | 完成條件 | 狀態 |
| --- | --- | --- |
| M1 URL 與載入 owner | Rust 分類、取消、錯誤、有界記憶體快取，本機 HTTP 正反例 | 已完成；2 個 URL 核心案例、4 個 loader 案例，包含取消的 TCP EOF 與 body 總期限 |
| M2 圖片／動圖預覽 | 暗底 dialog、等比縮放、播放／停止回首幀、原網址／外開、焦點與清理 | 已完成；Windows 真實 PNG／JPEG／WebP 像素、GIF 紅／藍幀及停止、快取、解碼失敗重抓、關閉焦點通過 |
| M3 影片與 YouTube | 直接影片手動播放，縮圖不擋外開，失敗可恢復 | 已完成；Windows WebM trusted 播放及首幀、Esc 關閉通過；YouTube 用本機縮圖及外開 sink 驗證，未連網站 |
| M4 驗證與交付 | 核心／HTTP／真實 Dioxus gate、現況文件、功能 commit 與 push | 本機完整 gate 已通過；以本階段功能提交交付，既有輸入／PNG／設定重啟／清理亦通過 |

本階段不含第三方網站內嵌播放器、圖床 HTML 擷取、GIFV 猜測轉換、progressive 圖片解碼或磁碟快取。影片 codec、實際網站及其他 OS 的結果另列驗證邊界。

2026-10-16 媒體驗證：Windows WebView2、Rust 1.97.0、Dioxus 0.7.10；`python tooling/check.py all` 及增量 desktop gate 通過，沒有實站媒體請求。媒體預覽與原 BBS canvas／input epoch 隔離；關閉取消 HTTP、撤銷 Blob URL、停止 video，錯誤不改變 BBS 連線狀態。GIF 實際畫面以獨立 System.Drawing 解碼，WebM 以 presented-frame callback／播放時間／尺寸確認；不以 DOM 元素存在當成播放成功。

本機 64×32 PNG 的一次量測：冷開約 44ms、記憶體命中約 33ms、關閉與 HTTP 取消約 12ms，包含 CDP 輸入／輪詢成本。這是新功能自身的合成工作量，沒有等價舊功能基線；不是實站速度或提升倍數。CPU／process-family peak memory 未量測。MP4／OGV、動畫 WebP／PNG、實際 YouTube／圖床、OS 外開、macOS／Linux 尚未驗收。

## 本次修正：中文組字與 Enter

2026-10-16 macOS 中文輸入回報：使用者觀察到獨立組字框，確認後多送 Enter。修正前輸入橋接只檢查 `isComposing`，未處理 legacy WebKit 在 `compositionend` 後送出 `keyCode=229` 的確認鍵；[WebKit 事件順序紀錄](https://bugs.webkit.org/show_bug.cgi?id=165004) 及 [macOS Web App 回報](https://bugs.webkit.org/show_bug.cgi?id=324380) 支持此相容性假設，但尚未擷取使用者的實際事件。

修正於同一輸入邊界排除 IME 按鍵、讓 keydown 唯一管理 Enter，忽略 textarea 的換行編輯；不使用組字結束後的時間窗或吃掉下一次普通 Enter。組字宿主改成游標位置、沿用當格 ANSI 配色的無框單行，寬度依內容與剩餘行寬調整；保留原生選字，確認前的內容不送站端。

Windows 真實 WebView 與模擬舊 WebKit 順序的增量 gate 已通過，包含確認後立即按普通 Enter 的精確 bytes、取消、相同字接續輸入、無框宿主及既有互動回歸；macOS 原生注音／拼音、取消、連續中文與下一次普通 Enter 仍需實機驗收。

## Vim 瀏覽模式

2026-10-16 實作增量：Rust 單一鍵盤策略接管模式、前綴／計數、原生一次與編輯歸屬，Connection 仍序列化送鍵。Windows WebView＋本機精確 bytes 驗證主要瀏覽、原生一次按鍵／Ctrl+d／中文、i/Esc；既有 IME、貼上、選取、PNG、媒體及設定重啟 gate 通過。核心計數／repeat／未知／邊界案例通過。手動 pending 加入回應期限與未確認訊息，空操作不送 bytes。實站及 macOS／Linux 尚待驗收。

以下為最初研究紀錄：

2026-10-16：已依使用者選定的瀏覽範圍，完成 [按鍵衝突與實作方案](../vim-browsing.md)。內容包含模式切換、hjkl、gg/G、計數、Ctrl+d/u/f/b/e/y，以及後續補充的「原生一次」按鈕、f 快捷入口與持續原生輸入。公開站端來源可確認 h／g／G／Ctrl+d/u 等衝突；閱讀 j/k 與方向鍵的邊界行為不同。採本機 Rust 指令解析與既有 Connection owner，分 V1 模式、V2 捲動及 V3 跨平台驗證；本輪只交付研究文件，未改產品按鍵行為。來源副本與 provenance 保存在 repo 外的功能研究目錄，文件 gate（含 19 個 harness 回歸案例）及 diff 檢查已通過；實站與原生 Vim 模式均未驗收。

## 後續建議研究：hover、名單與設定檔

2026-10-16：本輪完成來源／架構研究及待實作規格；沒有改產品互動或套用私人設定。

| 建議 | 決定與入口 | 現況 |
| --- | --- | --- |
| 圖片小預覽 | [hover 方案](../media.md#圖片-hover-小預覽)：跟隨游標／右上固定可切換，單一 request、首幀、點擊升級、不搶焦點 | 已實作；Windows pointer／HTTP、取消、定位與提升驗證通過；其他平台待驗收 |
| 本機黑白名單 | [名單方案](../content-filters.md)：完整 ID、標題字面條件、備註、例外與可揭露遮罩；共享辨識及輸出，保留固定格線 | repo 外原生靜態 Evidence 已取得；未執行目標或讀出私人名單。舊格式完整遷移／所有效果尚未驗證 |
| 預設組合與自訂檔 | [設定方案](../preferences-and-profiles.md)：任何平台均可選 Windows／Mac／Linux 組合，自訂另存、載入與匯出／匯入，保留明確選擇 | 已實作選擇意圖、具名可攜檔與 v1 遷移；Windows 保存／重啟像素通過 |

使用者已指定名單分一般遮黑與移除推文兩級，本輪不實作名單；詳見名單方案。先完成設定意圖與保存邊界，再實作 hover 呈現；名單須先補 Rust 的作者／推文範圍與共享呈現結果。Vim、設定檔及 hover 的實作與驗收分列如下；名單本輪保持設計階段。

## 可攜顯示設定檔實作

2026-10-16：已接入 Fonts 選擇意圖、version 2 AppSettings、具名 .pttstella.json、目前生效設定匯出與匯入草稿。任何平台均可選固定 Windows／macOS／Linux，手動字型改動才成為 Custom，切換組合保留字高／基線。舊 v1 不自動覆寫，明確保存時備份原始 bytes。

核心 3 個契約案例、Store 遷移／備份／失敗與 Windows 真實表單／檔案往返通過，包含兩個具名組合、固定 preset 識別、匯入預覽／取消／套用、損壞／超量／未知版本保留原檔，並由第二個 app 程序確認設定與逐像素重啟一致。既有輸入／媒體／PNG／Vim 回歸通過；probe 僅替代原生選檔路徑，原生 picker、macOS／Linux 及逐字 fallback 未實測。

## 圖片 hover 小預覽實作

2026-10-16：已實作 350ms dwell、右上／跟隨游標／關閉及本機保存；單一 Preview、HTTP future、decoder ACK 與 LRU 共用，hover 到完整預覽只提升呈現、不重開下載。hover 不取得焦點、攔截滑鼠、播放影片或更動 ANSI 格線；圖片首幀等比縮小，邊緣轉向並限制在 viewport。

Windows 真實 pointer＋loopback HTTP 已驗證快掠零請求、同連結一次、A→B 的 TCP EOF、已完成及下載中提升不重抓、兩種位置／右下碰邊、關閉偏好、設定重啟及 Canvas／PTT bytes 不變。Rust lease 案例驗證相同來源跨 revision 可續用，新輸入／session／範圍變動／未完成畫面均失效。現有全部離線 gate 通過；真實網站、macOS／Linux、原生 picker 與人工登入仍待驗收。

## 技術邊界

### 實站截圖版面對應修正

2026-10-16：使用者提供滿頁看板／最愛與文章列表截圖，均顯示「未辨識的畫面格式」。確認三個不相容條件：看板前進提示為「閱讀」而非僅「進入」；返回為「回上層」而非僅「回到上層／離開」；文章人氣與日期直接緊貼，原先 whitespace tokenization 會將 `+1610/16` 誤當日期。前一版合成列在這些欄位插入了多餘空白，因此漏掉實站格式。

文章解析已移除 token 日期搜尋，按 1／6／4／6／13 的固定 cell 欄位取編號、狀態人氣、日期、作者及標題。正常、爆文／標記、置底、單位數月日及六位數文章編號共用同一生產解析；legacy 圓形指標蓋住編號首字時，只在指標列接受數字尾段，包含 100000 的 00000。看板仍依自己的標頭及固定板名欄辨識，前進接受進入／閱讀，新增回上層文字。不把「閱讀」一詞當成文章列表判斷，h/l 仍經 Intent／Connection 序列化。

先以兩個自製滿頁案例在舊 reducer 重現失敗，再實作修正；fixture 使用合成作者／標題／板名，不保存私人 session。更新舊合成列到同一欄寬，新增完整 20 列看板與 15 一般＋5 置底的文章頁，原生 probe 也改消費這兩頁。22 個 navigation、27 個 desktop lib、全 targets desktop Clippy、fmt、core 六套 suite／兩個 replay、docs 19 個 gate 回歸通過；非法日期／提示覆蓋／缺前進命令仍暫停。

macOS arm64／WKWebView 的隔離 App 使用內建跨平台字型，在主選單→滿頁看板→最愛→滿頁文章執行原生 j/k/gg/G/l 或 h；兩種問題格式實際觀察均為 NORMAL，peer 比對 20 筆精確 bytes、零額外輸入，⌘W 後 listener 釋放與 worker join 為 passed。這是按截圖格式製作的本機往返，不代表已操作使用者帳號或完成實站重試。完整 desktop gate 的 Mac backend 仍為 not_ready，未取得 Windows runner 結果。

App 重新打包至 `dist/macos-navigation-layout/PTTStella.app`，包含本輪辨識修正與前輪內建字型；保留舊包，需退出舊 App 後開啟新版。沒有 push／公開發佈。

### 隨 App 附帶與載入字型

2026-10-16：使用者希望缺少的字型能隨設定直接套用。加入 `bundled`「內建跨平台」組合：Cascadia Mono 2407.024＋Noto Sans CJK TC Regular 2.004，兩份未修改的官方字型共約 16.4 MiB，OFL 1.1、來源／SHA-256／著作權隨產品保存。Windows 組合也可使用附帶的 Cascadia；Microsoft JhengHei 沒有隨 App 散布，不會靜默替換成 Noto。已保存的設定與平台初始值保留，需明確選擇新組合。

字型只編入 desktop executable，經 Dioxus 同 origin 的固定資產路由提供；沒有字型下載、外部 URL、任意檔案路徑或系統安裝。選取時 FontFace 載入 App 專用 family，預覽／主畫面／組字宿主／PNG 使用同一對應；最多兩個內建載入 promise，失敗可重試並顯示來源，其他字型仍核對 local face。來源與兩份授權可在設定視窗展開閱讀，也附於 macOS Resources／FontLicenses。設定視窗提高可用高度，實際 Mac 畫面確認載入訊息及儲存按鈕可見。

驗證：27 個 desktop lib、1 個 bin 字型資產路由案例、3 個可攜設定案例、全 targets desktop Clippy、fmt、core gate（六套 suite／兩個 replay）、docs 19 個 gate 回歸、JS／Python 語法通過。固定路由核對 MIME／原始 TTF／OTF 標頭、HEAD，以及未知路徑／方法拒絕；下載字型的來源 blob／SHA-256 與 byte 數核對，App notices 與來源相同，ad-hoc strict 驗章通過。

macOS arm64／WKWebView 隔離流程實際確認：未安裝 Cascadia 的 Mac 選 Windows 後只剩正黑體缺失訊息；選內建跨平台兩 face 載入成功。取消預覽的整頁 PNG 與原圖完全相同；保存後 960×576 中英字形均改變（ASCII 標題區 768 像素、中文選單區 5,163 像素），第二個程序讀取 `bundled` 後 PNG bytes 完全一致。兩次流程各跑 20 筆跨 profile 原生 Vim 事件，peer 沒有額外輸入，分別 ⌘W／⌘Q 退出，listener／worker 清理報告 passed。

最終 App 約 27.6 MiB，位於 `dist/macos-fonts/PTTStella.app`；舊包及驗證產物保留。完整 desktop gate 的本機 backend 仍是 not_ready，Windows 選單 gate 的選擇索引改由 option 位置取得以容納新增組合，但本輪沒有 Windows runner／Linux／跨 DPI／所有 glyph 或原生 IME 的實測結果。未輸入實站帳密或操作使用者站端 session。

### 主選單／看板列表的 Vim 支援增量

2026-10-16：使用者回報原生 Command 操作正常，但主畫面、看板列表／我的最愛及文章列表仍出現暫停。確認前兩類根本沒有 profile；文章列表原先僅在底列尋找完整返回／閱讀命令，未涵蓋上下操作列分配。新增 MainMenu／BoardList（含分類入口）與自製 fixture；命令可分布於上方操作列及底列，底列仍須有正常標記／樣式。提示覆蓋、未完成 ANSI、非法指標與操作區未映射資料仍拒絕，按鍵提示現在保留暫停原因。

主選單、看板／最愛與分類均沿同一 Keyboard → Intent → navigation::plan → Connection；j/k、計數、半頁使用方向鍵，h/l 返回／進入，gg/G 使用 Home／End。半頁依內容高度計算。主選單廣告及分類裝飾區的未映射字不再使操作暫停，標題／狀態／選項仍驗證。新增 profile 不提供文章命中區、不啟用滑鼠／防閒置；Connection 以明確 allows_keepalive 限定文章列表／閱讀，測試強行給新 profile UI permission 也無刷新輸入。

驗證：18 個 navigation、8 個 Vim、27 個 desktop lib（包含 20 筆跨 profile 精確 bytes 及新增防閒置拒絕）通過；全 targets desktop Clippy、core gate 的六套 suite／兩個 replay、fmt 通過。macOS arm64／WKWebView 的隔離 App 使用真正原生事件，依序在主選單、看板列表、最愛、上方閱讀命令的文章列表執行 j/k/gg/G 與 l 或 h；本機 peer 核對 20 筆 bytes，全部維持 NORMAL，⌘W 後 listener 釋放與 worker join，window-probe.json 為 passed。分類入口只有核心合成案例，沒有宣稱 Mac 視窗覆蓋該入口或實站登入後已修復。沒有操作使用者帳號的站端按鍵，也沒有錄製私人流量。

App 保留於 `dist/macos-navigation/PTTStella.app`，ad-hoc 簽署／arm64 打包完成，舊 App 保留。完整 Windows desktop gate 在本機 Mac 維持 not_ready；本輪沒有 Windows runner 或實站登入後結果。使用者須退出舊 App 後開啟這份新版再重試。

### macOS 操作修正與兩層輸入對應

2026-10-16：修正一般列表的空白補齊日期（如 `1/ 9`）、投票／鎖文標記及作者 `-` 的刪文列辨識。新增正常與錯誤日期／提示正反例，仍要求標頭、操作列、合法游標及完整格線。暫停會顯示具體原因；未辨識畫面仍不猜測站端動作。這些合成格式已驗證，使用者原先實站看板仍須重試，沒有據此宣稱實站問題完全結案。

依使用者要求維持兩層對應：`Keyboard` 將按鍵轉為瀏覽 Intent，`navigation::plan` 根據畫面產生 PTT bytes；移除 JS 重複的 key-to-byte 表。Mac 注音輸入來源的實測先重現 j 啟動組字；修正為 Normal／暫停使用 readonly 編輯宿主，Rust 只在瀏覽模式解析 Process／非 ASCII key 的實體碼與 Shift。i／f 原生輸入保留組字 owner；核心案例確認原生組字仍可送出，原生候選窗的完整流程本輪沒有重驗。

macOS 15.7.7／arm64／WKWebView 真實原生事件已確認：包含上述三種特殊文章列的看板維持 VIM NORMAL，j／k／gg／G 由同一 Connection 往返，loopback 逐筆只接受 Down／Up／Home／End。原生選單提供 ⌘W、⌘Q、⌃⌘F 及 Zoom；兩次成功視窗流程分別用 ⌘W 與 ⌘Q 關閉，`window-probe.json` 均確認 listener 移除、worker join 與零額外站端輸入。全螢幕進出及 Zoom 有實際視窗觀察。首次全螢幕測試曾被 UI 自動化的焦點／視窗變動打斷且服務端拒絕額外輸入；該次不算通過，後續獨立流程才通過。

字型視窗新增指定 local face 的載入失敗提示；本機選 Windows 時實際確認 Cascadia Mono／Microsoft JhengHei 無法載入，先前外觀相同是系統替代字型。新增 macOS 宋體組合（Menlo／Songti TC），保留原 macOS 黑體（Menlo／PingFang TC）與明確選擇意圖。真實表單切換、預覽、保存與主畫面 PNG 已驗證：960×576、英數首列全部像素相同，中文區域有 8,051 個像素不同，保存值為 `macos_serif`。只證明這台 Mac 的這兩個 face 與案例，不宣稱逐字 fallback 或跨 DPI 完整驗收。

驗證：桌面 feature 的全 targets Clippy、27 個非空 lib 測試、core gate（fmt／Clippy／六套 suite／兩個 replay）、JS 語法及 docs gate；完整 `desktop` gate 在 Mac 仍為 not_ready，沒有 Windows runner 結果。Mac 的既有 HTTP 取消案例原先因 RST 不等於 EOF 失敗；測試現在共用有限期的 FIN／ConnectionReset 關閉斷言，其他錯誤／額外資料仍失敗，下一筆圖片成功下載也必須通過。所有一般測試只使用合成資料與 loopback，沒有執行實站登入。

本次本機資源 guard 曾以全機磁碟預留超限拒絕預設 16 GiB 的 core invocation，該次未執行、不算通過。檢查本專案 namespace 約 0.8 GiB 後，以 4 GiB ceiling 重新執行，保留全機 60 GiB／free-space 100 GiB 規則，沒有刪除其他 owner 的資料或變更 harness。更新 App 保留於 `dist/macos-fixes/PTTStella.app`，舊產物保留；沒有 push／公開發佈。

### macOS 本機 App 打包增量

2026-10-16：新增 [macOS App 打包入口](../macos-app.md) 與 `Dioxus.toml` metadata。同一 Rust 1.97.0／Dioxus 0.7.10 package 在 macOS 15.7.7 Apple Silicon 完成原生 arm64 release 編譯，產出約 11 MiB 的 `PTTStella.app`；plist、僅系統動態依賴、ad-hoc 簽署及 strict 驗章通過。腳本從 Cargo JSON 取得 executable，不假設 target 路徑，並在 guard 結束前將 app 保留於 repo 的 dist。

以 bundle 內 executable 的 `--probe` 確認 macOS WKWebView 視窗能顯示自製中英文、雙色字、調色盤及方塊；本機接收 owner 已啟動。這是啟動觀察，沒有執行完整 Windows CDP 互動序列，提前關閉時因未完成 probe 序列回報 `unexpected loopback input at step 0`，不能算 desktop gate／完整清理驗收。另正常啟動曾顯示已連線，沒有輸入帳密；觀察後關閉並確認程序已退出。未做人工登入、macOS 像素判定、IME／剪貼簿／媒體與其他 OS 版本驗收。

本機 `dx bundle --macos --release --package-types macos --features desktop --bin pttstella --locked --out-dir dist/macos` 曾因 CLI 0.7.9 額外呼叫 nightly 的 unit-graph 探針，被 guard 以 `toolchain changed` 拒絕；該次不是通過。交付腳本改以受同一 guard 管理的固定工具鏈 Cargo build 建立標準 app，沒有改 guard 或切換工具鏈。產物為 ad-hoc 本機測試版，沒有 Developer ID、公證、自訂圖示、DMG 或 Universal Binary；既有 `desktop` gate 的 macOS backend 維持 not_ready。

### 既有核心與傳輸邊界

- 一個 Cargo package：純 Rust 終端核心與 desktop 依賴分離；同一核心供產品及離線 replay 使用。
- 核心管理 ANSI、byte/cell、cursor、局部覆寫／捲動與左右樣式；顯示依 [文字與排版契約](../dual-color.md)。
- Dioxus 開視窗與輸入介面；優先驗證 Canvas 薄橋接，收到的文字不拼成程式碼。實際畫面探針決定是否可採用。
- 原型邏輯格線可先用 80×24，這是初始值而非已驗證站端要求；視窗改變先縮放／留邊，不重新排字。傳輸探針確認需不需要協商其他尺寸。
- 只實作一條經驗證的加密傳輸，優先探針 WSS；確認端點、TLS、握手、payload 與可能的 framing 後才定案。需要改走 SSH 時另驗證 host key，不同時堆多條路線。
- 接收 bytes 完整進核心；送鍵由單一 session 擁有者序列化。generation 隔離舊連線、舊 frame 與晚到 callback，重連不重送帳密。
- 已鎖定 Rust 1.97.0、Dioxus 0.7.10 及 Cargo.lock；這是實測工具鏈，不宣稱 MSRV。
- mapping 來源與授權明確，不使用原版表格；Big5／UAO 差異與未映射狀態需有自己的案例。

## 首次登入的三個階段

| 階段 | 工作 | 完成條件 | 現況 |
| --- | --- | --- | --- |
| T1 核心 | Cargo 骨架、ANSI／cell 狀態、mapping 與離線 replay | 同一核心的非空合成案例通過，包含雙色字與任意 byte 邊界切分 | 基線完成：12 個核心測試、2 個 replay；未知控制碼保留計數，完整相容性未宣稱 |
| T2 視窗 | 真正 Dioxus terminal、固定排版、左右裁切、焦點與必要送鍵 | 看得對、送得對、斷開離線 sink／關閉視窗時 owner 可清理 | Windows WebView2 合成顯示探針通過：紅／綠字形、藍／紫背景、輸入與 listener 清理；IME 實機未驗證 |
| T3 登入 | 一種傳輸、接收／送鍵、手動登入與斷線 | 實際看見登入前與登入後畫面；斷線／錯誤可用且不污染新 session | 傳輸及鍵盤已接上；7 個本機傳輸／TLS 設定案例通過，實站登入前提示已確認，人工登入後畫面待驗證 |

T1 先完成 ASCII 格線與 snapshot 的最小路徑，再依必要案例加入中文／顏色／控制碼。T1／T2 共用 fixture 收斂顯示；T3 的無登入傳輸探針可提前獨立驗證。不要先還原整個參考程式，也不要先建立高階瀏覽器 UI。

T1 驗證 fmt／Clippy、核心測試及生產 replay；T2 需要真實 backend 的字型／frame 完成訊號與畫面判定；T3 的一般測試只用離線 adapter，實站登入是獨立人工流程。必要案例缺少就保持未就緒，不能僅有畫面或編譯成功就說完成。

## 最小驗收

| 案例 | 必須觀察 |
| --- | --- |
| 固定格線、中英文、ANSI 色彩 | 行列、空白、cursor 與字寬正確 |
| 一個中文字左右兩色／背景 | 同一完整字形與左右獨立樣式 |
| 控制碼／雙 byte 中間分割 | 最終狀態與不分割一致，無不相關位置配對 |
| 局部清除、覆寫、捲動 | 受影響畫面與字形配對正確 |
| 框線、方塊、縮放 | 固定排版與接縫正確，不隨視窗重排 |
| 必要按鍵與文字輸入 | 輸出 bytes 正確、焦點可用，不重複送出；無法編碼時明確報錯 |
| 取消、斷線、重連與晚到資料 | UI 仍可操作、清理 owner、舊 generation 不污染新 session |
| 實站手動登入 | 使用者看見站端登入後畫面，沒有自動猜提示／輸入／保存密碼 |

完整 IME／選取複製、所有 ANSI 指令與所有字型不列首次登入原型的完整支援宣稱。若所需登入流程或必要文字輸入用到某能力，就必須實作並驗證，不能以不支援規避。

## 完成與後續

使用者已啟動 [操作功能研究](../feature-research.md) 的實作 goal。各項在保留固定格線／雙色字的前提下獨立交付；登入卡住的實站重試另待使用者回報。

| 操作功能 | 狀態與驗證 |
| --- | --- |
| 選取與純文字複製 | 已實作 Rust 字形選取、滑鼠拖曳、雙擊整行、整頁選取；Windows 真實 WebView Ctrl+C 與系統剪貼簿讀回確認雙色中文字只複製一次。5 個互動核心測試；macOS／Linux 尚未實測 |
| IME 宿主與組字狀態 | 已實作游標位置的無框組字宿主、IME 確認鍵隔離、依編輯事件去重、失焦／session 變更清理；Windows WebView CDP 組字取消、確認與接續相同中文字的往返通過。原生輸入法候選窗及其他平台仍待實機驗收 |
| 單行貼上與多行預覽 | 已實作共用輸入 bytes 上限、單行直接排入、換行正規化、多行原生 dialog 預覽／取消、session 檢查與明確拒絕。8 個互動核心案例及 Windows 原生 Ctrl+V／剪貼簿往返通過；取消、不可編碼、超量、控制碼都未送入測試服務端 |
| 網址開啟與複製 | 已實作單列 HTTP／HTTPS 偵測、字形欄位映射、點擊開啟、右鍵選取後複製。拖曳不開啟，行末可能截斷或含未映射 cell 的候選不開啟。Windows WebView 點擊／拖曳／系統剪貼簿測試通過；開啟 adapter 在 probe 使用確認 sink，不啟動外部瀏覽器，實際 OS 瀏覽器啟動仍待人工驗收 |
| 整頁 PNG | 已以同一 renderer 匯出 960×576、不含選取／游標的完整頁面，限制 PNG 3 MiB／base64 4 MiB，原子替換保存。Windows 真實匯出按鈕、檔案獨立解碼與像素／畫布位置驗證通過；另存原生視窗的人工操作尚待驗收 |
| 選取行 PNG | 已加入目前畫面連續行匯出，完整 80 欄、每行 24 像素，保留空白行與雙色字；Rust 核對選取／畫面識別與對應圖片尺寸。Windows trusted 滑鼠／按鈕測試涵蓋單行、反向多行、首尾與整頁，以獨立解碼後的全部像素與整頁對應區域比對通過；縮放、可見畫布與捲動不受影響。未實作跨頁／不連續多段合併；原生另存與其他平台仍待人工驗收 |
| 字型設定與保存 | 已實作英數／中文字型、預設組合、字高／基線、同一 renderer 的獨立預覽與恢復預設。Rust 驗證、使用者設定目錄、備份與原子替換；損壞／超量／保存失敗保留原檔。Windows 真實表單編輯、取消、保存、拒絕無效值，以及第二個應用程序重新載入的逐像素比較通過；逐字 fallback、跨 DPI 與其他平台仍待實測 |
| 最小滑鼠導覽 | 已實作保守畫面辨識、文章編號點擊、閱讀翻頁／返回及列表滾輪移動。12 個核心正反例、2 個 production Connection 競態案例與 Windows WebView 的 trusted pointer／wheel 往返通過；服務端比對精確 bytes，取消、雙擊延續、組字／dialog／未知畫面及放大捲動均未夾帶額外輸入。hover 僅透明細底線，Canvas 像素不變，閱讀按鈕使用既有 hover 工具列。實站文章、實體 IME、跨 DPI 與 macOS／Linux 尚未驗證 |
| 預設開啟防閒置 | 已實作 [保活契約](../keepalive.md)：Connection 單調時鐘、60 秒閒置刷新、UI／畫面／session 許可、暫停開關、無回應釋放與錯誤。2 個時鐘案例、4 個 production Connection loopback 與 Windows trusted 開關／UI 暫停案例通過；實站長時間效果、實機睡眠與其他平台待驗收 |

當前 terminal 交付完成須 T1–T3 的必要項通過，且明列實測平台、字型、backend 與控制碼／mapping 範圍。Windows 成功只稱 Windows 原型；其他平台不當成已通過。後續是否加看板／文章導覽、現代介面或其他功能，再另行決定。

開工順序為 **Cargo 骨架 → ASCII grid／snapshot／replay → 中文與雙色字 → Dioxus 顯示 → 連線與手動登入**。每個可獨立驗收的子目標完成後提交，狀態只在本計畫更新。

## 目前驗證邊界

2026-10-15：Rust 1.97.0／Dioxus 0.7.10、Windows WebView2。

2026-10-16 增量：最小滑鼠導覽的核心、Connection 及 Windows 桌面合成往返通過；13 個 desktop lib 測試包含 2 個新增導覽競態案例。既有選取、IME、貼上、網址、PNG、設定保存／重啟像素與清理 gate 一併通過。未執行人工 PTT 登入後導覽；原先登入提示卡住仍待使用者實站重試。

同日符號比例修正：移除一般文字逐字橫向拉滿的幾何，改為自然比例置中、超寬等比縮小；連接框線／陰影及幾何方塊保留格線行為。Windows 預設字型的 `●○◎⊙` 筆畫 bounds 由 20×15、20×16、18×9、16×13 改為 16×15、16×16、10×9、14×13；獨立 PNG 解碼、雙色字、16 色、方塊接縫、截圖、桌面互動與設定重啟像素 gate 通過。實測紅／綠字形像素現在為 22／41；下列 32／43 為先前基線。其他字型／DPI／平台尚未全面驗證。Connection 當時只有站端 Ping 的 Pong 回應；其後新增預設開啟的 BBS 保活，見上述獨立驗證範圍。沒有 OS 防休眠計時器。

- 核心：fmt／Clippy、12 個測試與 2 份 replay 的獨立 expected／全部單一 byte 切分；強度可獨立保存在中文字左右兩格。
- 桌面：viewport 填滿視窗，工具列 hover 浮現且不推擠畫面；預設符合視窗並維持比例，提供等比例放大與捲動。Windows DPR 1 的合成探針畫布為 1040×624；100／125／150% 的連續方塊非預期像素均為 0（舊路徑為 1,712），半方塊與四向三角形抽樣通過。紅／綠字形像素為 32／43，16 色及 SGR 1／22／0 通過。listener／resize observer／縮放工具清理與 worker join 通過。字型 stack 為 Cascadia Mono／Microsoft JhengHei／monospace；逐字 fallback 未量測。合成結果不代表已重播使用者截圖中的實站文章，也不宣稱所有 ANSI Art 皆相容。
- 傳輸：本機 production WebSocket 案例涵蓋分割 bytes、協商、無自動登入、送鍵順序／escaping、重連清畫面、拒絕舊輸入、取消握手、站端關閉、異常 framing／容量及離線輸入；另驗證明確 TLS provider 可在 desktop 依賴組合下建立。
- 輸入修正：實際 WebView 點擊畫面可重現焦點離開 textarea；原因為可捲動容器的預設聚焦覆蓋 pointerdown 內設定的焦點。改由整個 viewport 管理點擊並取消內容區的預設聚焦，保留捲軸操作。實際瀏覽器點擊畫面／黑邊／縮放按鈕後 activeElement 均為 keyboard；ASCII、中文字編碼、Enter、方向鍵由 trusted input 到達 Rust。已移除會繞過焦點的合成輸入通過路徑。尚未以實體鍵盤或實站登入驗證。
- 確認提示調查：擴充 mounted probe，讓 ASCII／中文、方向鍵、Enter 經同一生產 Connection 送到本機 WebSocket，收到合成確認提示後再輸入 `n`、Enter，必須收到並繪製完成畫面；完整往返已通過。本機服務端隨後以代碼 1008 與合成原因關閉，瀏覽器確認原因文字且移開滑鼠後仍可見。斷線／錯誤工具列持續顯示，保留站端關閉代碼與原因（未提供時明示），斷線後輸入不覆蓋該原因。hover class 與 Rust 狀態 attribute 分別由各自擁有者管理。
- 實站卡住尚未重現：使用者回報重複登入提示輸入無反應、之後斷線；沒有捕獲該次站端關閉原因，不能宣稱已確認根因或已修復登入。[公開站端程式](https://github.com/ptt/pttbbs/blob/master/mbbsd/mbbsd.c#L384) 的 `multi_user_check` 在選 `n` 且仍達重複登入上限時會主動結束連線；這是來源推論，沒有證明實站部署版本或使用者當時連線數。
- 實站：獨立 `--probe-live` 成功連上 WSS，收到 4,397 bytes 並在同一生產 Screen 確認登入前提示，未送帳密，之後關閉並 join worker。報告不保存原始流量；這不是人工登入或登入後操作證據。

使用者下一步：啟動正常視窗 → 自動連上 PTT → 人工登入 → 確認站端登入後畫面 → 斷線。使用者截圖可觀察到文章閱讀畫面與 ANSI Art 接縫；本次修正在自製合成案例驗證，未錄製私人 session。實機 IME、原生關窗操作及 macOS／Linux 尚未人工驗證，跨 DPI 視覺相容性尚無完整判定。沒有宣稱效能改善，CPU／process-family peak memory 與完整任務 token 成本未量測。[依賴來源](../dependencies.md) 為公開套件。

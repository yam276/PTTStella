# PTTStella

Rust／Dioxus 的 BBS 桌面介面，以 PTT 為預設站，外觀像 terminal；不嵌入系統終端、shell 或 PTY。啟動連至設定的預設站，手動登入，呈現固定格線、ANSI 色彩與雙色字。

Rust 核心與 renderer 實作固定格線、ANSI 色彩、雙色字與 ASCII Art，搭配公開映射依賴及合成驗收案例。

第一個實測目標為 Windows；macOS 已提供本機 arm64 App 打包與啟動觀察，完整功能及 Linux 仍待各自驗證。畫面核心、離線 replay、Dioxus／WebView2 顯示及 WSS 登入前連線探針已通過；macOS 專用帳號的手動／按鈕登入、Vim 切板／閱讀及關閉斷線已實測，案例及待驗收範圍見 active plan。

## 目前範圍

- 一個 terminal 畫面及連線／斷線狀態。
- 介面支援繁體中文與英文；「字型與顯示」可保存語言偏好，下次啟動套用。PTT 站端內容不翻譯。翻譯資源與契約見 [介面語言](docs/i18n.md)，驗證進度見 active plan。
- 浮球「連線 → 站點」可輸入位址、選擇 Big5-UAO／Big5／GBK，保存站點最愛、切換連線或設定下次啟動的預設站；內附 PTT、PTT2、巴哈與水木入口。支援 WSS／WS／Telnet；WS／Telnet 不加密，連線前會明示。其他站使用原始輸入，PTT 專用登入／導覽／保活／名單僅用於預設 PTT 目標。尚不支援 SSH、UTF-8 或 GB18030 四位元組字元，詳見 [站點與編碼](docs/sites.md)。
- 固定欄位、中文字寬、ANSI 顏色、游標、局部重繪與雙色字。
- 操作元件採深藍／金色主題，浮球以平滑三聯星標示，主要確認／儲存按鈕以金色填色；PTT 內容保留 ANSI 原色。
- 設定頁可保存浮球不透明度（20%–100%），滑鼠移入或鍵盤聚焦時恢復清晰；功能選單與斷線／錯誤提示保持不透明。取消不保存，不影響 PNG。
- 畫面維持比例並符合視窗，工具以可拖曳浮球開啟，六個 Lucide 分類按鈕扇形展開，空間不足時改為可捲動格狀排列。選取分類後，星座浮球改為返回箭頭，點擊回到分類選單並恢復星座，再點一次收合；明暗色依站端指令。
- 工具列可開啟 Vim 瀏覽，支援主選單、分類／看板／我的最愛列表、文章列表與閱讀：hjkl、gg/G、計數、Ctrl+d/u/f/b/e/y；f 或「原生一次」只放行下一個完整按鍵／組字，i 持續原始輸入。標準進板圖可按空白鍵、Enter 或 l 繼續並保持 Vim；未知畫面暫停 Vim，登入預設原始輸入；範圍與限制見 [Vim 瀏覽](docs/vim-browsing.md)。
- 新命令說明頁可用 Vim 移動、翻頁與返回；執行選中命令須按 f 後用原始確認鍵。已辨識的空看板／文章列表可按 h 返回。DEC2026 同步輸出期間保留上一張完整圖並暫停畫面導覽，結束或一秒恢復期限後重新辨識。
- 工具列「快速前往」、⌘K／Ctrl+K 可輸入板名、#AID@板名或支援的 PTT 文章網址，按「前往」才啟動。支援已辨識主選單、看板／文章列表及閱讀頁，逐步等待站端畫面；進板圖或其他確認停下來交還手動，提供「停止導覽」。指定文章會先核對 AID 與選取列再開啟，範圍見 [指定文章](docs/article-navigation.md)。最近看板最多 20 筆，只記錄確認到達的正式名稱，可重新載入與清除；選清單只填入草稿。原生與實站範圍見 [快速切板](docs/quick-boards.md)。
- 「文章書籤」可手動輸入 AID／文章網址，保存選填名稱、新增或移除，最多 500 筆；本機保存並在重啟時還原，不下載文章或猜目前 AID。按「前往…」先填入快速前往，確認後才開啟。壞檔保留並停用保存，契約與平台範圍見 [本機書籤](docs/article-navigation.md#本機書籤資料)。
- macOS 工具列「登入」可選擇將一個預設帳號與密碼存入系統鑰匙圈；按登入才送出，密碼不寫入設定檔或回填畫面，提供移除已存帳密。啟動／重連不自動登入；2FA 提示會交還原始輸入，錯碼由使用者重試，取消或本機引導逾時不重送帳密。其他後續確認由使用者操作。見 [登入與保存](docs/login.md)。
- 關窗／⌘Q 會有界關閉目前傳輸；WebSocket 等待 Close 回覆，Telnet 明確標示本機傳輸已關閉，不當成站端確認。
- 一般黑名單可新增／修改／移除完整 ID 與備註，明確儲存於本機；遮黑已辨識的標準推文內容並保留原行列、作者與時間。可暫時顯示目前畫面；複製、PNG 及連結使用相同過濾結果。截斷、無作者續行與非標準格式保留原樣，範圍見 [本機過濾](docs/content-filters.md)。
- 必要鍵盤輸入；斷線或連線失敗時介面仍可操作。
- 滑鼠拖曳選取、雙擊整行、工具列整頁選取；Ctrl+C／Cmd+C 複製純文字，雙色中文字保留完整字形。選取保留空白，新畫面會清除舊選取；Esc 清除選取，沒有選取時 Ctrl+C 仍送到 PTT。
- 組字文字以無框樣式跟隨 PTT 游標，確認後送出文字，取消不送出；確認候選字的 Enter 不當作 PTT 的 Enter，接著再按 Enter 才送站端。已驗證 Windows WebView 組字路徑及模擬的 WebKit 事件順序，macOS 原生 IME 候選窗仍待實機驗收。
- Ctrl+V／Cmd+V 貼上單行，不附加 Enter；多行先預覽確認。無法編碼、超限或包含不支援控制碼的文字會整段拒絕。
- 站端提供 OSC 8 時，跨行各片段對應同一完整 HTTP／HTTPS 網址；沒有 metadata 時維持完整單列偵測，不猜接換行。直接圖片／影片與 YouTube 開媒體預覽，其他網址開系統瀏覽器。右鍵選取網址後可用 Ctrl+C／Cmd+C 複製完整目的地，一般拖曳仍複製畫面文字。契約與驗收邊界見 [完整網址](docs/links.md)。
- 圖片連結停留 350ms 會顯示首幀小預覽，預設右上固定，可在工具列改為跟隨游標或關閉並保存。移開、組字、送鍵或切頁取消；不搶焦點，點擊沿用已下載或下載中的圖片開完整預覽。
- 圖片預覽提供等比例縮放、符合／原尺寸、重試及外開；GIF／WebP 動圖預設停在首幀，可播放／停止回首幀。關閉會取消下載並釋放瀏覽器資源；成功解碼的圖片才進入有界記憶體快取，不寫磁碟。
- MP4／WebM／OGV 直接網址提供手動播放器，格式支援依 OS／WebView；YouTube 提供背景縮圖與立即外開，不等待標題查詢。支援範圍、容量及驗收見 [媒體規格](docs/media.md)。
- 工具列可匯出整頁 960×576 PNG，保留雙色字與方塊，不包含工具列、選取或本機游標；不受目前縮放與捲動影響。
- 拖曳選取目前畫面的連續行後，工具列「選取行 PNG」可匯出涵蓋的完整行；雙擊文字可選單行。固定 960 像素寬、每行 24 像素高，保留空白行與 ANSI 原色，預設檔名包含起訖行號。尚未提供跨頁拼接或不連續多段合併。
- 字型視窗可分別從本機清單選擇英數／中文字型，或手動輸入名稱。預設涵蓋跨平台、Windows／macOS 傳統 BBS、黑體／宋體及 Linux；明確選擇不隨 OS 改寫。內建 Cascadia Mono／Noto Sans CJK TC 無須安裝；細明體與正黑體須另有合法安裝，缺少時提供官方取得／授權入口。macOS 傳統 BBS 以 Songti TC 同時顯示英數與中文，不宣稱等同 Windows 細明體。具名 .pttstella.json 僅匯出顯示選擇與排版，載入先預覽，套用後才保存，取消保留原設定。
- 工具列「字型」可分別指定英數／中文字型、字高與基線，提供預設組合與雙色字預覽。設定頁採 Lucide 圖示搭配短標籤，保留基線、來源及載入／匯出範圍說明。按「套用並儲存」後才改主畫面；固定格線與 ANSI 色彩不變。
- 已辨識的文章列表可點編號開啟，hover 只加細底線；標題保留拖曳與雙擊選字。閱讀時浮球「導覽」提供「返回／上頁／下頁」，普通滾輪翻頁、列表滾輪移動一項；放大時優先捲動本機畫面，Ctrl＋滾輪仍縮放。
- 站端協商 1000／1002／1003＋1006 時可回報左／中／右鍵按下與釋放、垂直滾輪，並依模式回報拖曳或游標移動，避免同時觸發本機導覽；Shift 保留本機選字，Ctrl＋滾輪縮放。工具列可關閉回報並恢復本機點編號與滾輪；未支援的模式／編碼顯示原因。交付 App 的限定 macOS 點擊案例已驗證；新增移動／滾輪、原生 Shift 拖曳／Ctrl＋滾輪及其他平台邊界見 [站端滑鼠](docs/station-mouse.md)。

滑鼠導覽的 Windows WebView／本機服務端往返已通過；僅支援保守辨識的列表及一般閱讀狀態列，未知畫面保留手動鍵盤。支援範圍及限制見 [導覽契約](docs/navigation.md)。有界快速切板、本機最近清單、指定文章入口與手動書籤已實作，平台與實站驗證狀態維護於 active plan。先不做獨立文章清單 UI、搜尋介面、發文編輯器或啟動時自動登入。

## 開發入口

- [工作規則](AGENTS.md)
- [驗證命令](CONTRIBUTING.md)
- [最簡 terminal 計畫](docs/active/minimal-browser.md)
- [文字與排版契約](docs/dual-color.md)
- [架構](docs/architecture.md)
- [站點與編碼](docs/sites.md)
- [開發 harness](docs/development-harness.md)
- [後續操作功能研究](docs/feature-research.md)
- [Vim 瀏覽模式](docs/vim-browsing.md)
- [字型組合與可攜設定檔](docs/preferences-and-profiles.md)
- [本機名單與內容過濾](docs/content-filters.md)
- [媒體預覽規格](docs/media.md)
- [macOS App 打包](docs/macos-app.md)

```text
python tooling/check.py docs
```

`python tooling/check.py core` 執行 fmt、Clippy、畫面／互動測試與 2 個 replay 案例。`desktop` 包含本機傳輸／TLS 設定測試，並啟動實際視窗驗證雙色字像素、16 色與強度切換、填滿本體、輸入、剪貼簿與清理；`all` 跑全部離線 gate，不連 PTT。依賴來源見 [依賴說明](docs/dependencies.md)。

啟動：`cargo run --locked --features desktop --bin pttstella`。初始預設為 `wss://ws.ptt.cc/bbs`；明確「設為預設」後，下次啟動改連該站，仍須手動登入。方向鍵、Enter、Backspace、Tab、Esc、Home／End、PageUp／Down、Delete 與 Ctrl＋字母會送到目前站點（複製／貼上／快速切板的本機快捷鍵另行處理）。點擊浮球開啟導覽／顯示／擷取／輸入／連線／過濾六類工具；拖曳後會依速度滑行並吸附側邊，視窗上緣保留安全距離，減少動態效果時直接吸邊。Escape 或點外部收合，外部點擊只收合工具，不送入站端。斷線或錯誤時保持狀態提示可見，列出站端提供的關閉代碼與原因；沒有原因時明確標示。斷線後由你決定是否重連，不重送舊輸入，不錄製完整流量；macOS 可明確選擇將 PTT 帳密保存於鑰匙圈，按登入才取用，其他站不取用該帳密。

「＋／－」、Ctrl＋加減鍵或 Ctrl＋滾輪以相同比例縮放；「符合視窗」或 Ctrl＋0 還原。放大後可用捲軸查看超出區域。固定 80×24 不重新排字；不同視窗長寬比會留黑邊。方塊／三角形按格線繪製，背景以整數像素共用邊界，避免 ANSI Art 接縫。

一般文字保留字型比例並置中於固定欄位，圓形等窄符號不橫向拉滿。字型設定可預覽符號、雙色字與框線。

防閒置預設開啟：在已辨識的文章列表／閱讀頁閒置 60 秒後刷新，不移動閱讀位置；登入、未知畫面、組字、貼上、選字或 dialog 期間暫停。浮球「連線」中的「防閒置：開／關」可暫停本次啟動的保活，重啟預設開啟。未收到刷新回應時會暫停並顯示原因，不持續重送或卡住手動導覽。這與 WebSocket Ping／Pong 分開，實站長時間效果尚待驗收，詳見 [保活契約](docs/keepalive.md)。

顯示偏好保存於 OS 使用者設定目錄（Windows 為 `%APPDATA%\PTTStella\config\settings.json`），覆寫前保存 `settings.json.bak`。本機設定使用 version 4，保存介面語言與浮球不透明度；舊 v2／v3 預設完全不透明，舊 v2 預設繁體中文，舊 v1 明確字型名稱遷移成自訂，明確套用後才寫入新版；載入失敗時使用預設並顯示原因，不自動覆寫原檔。字型名稱由系統解析，缺字可能使用替代字型；沒有宣稱已驗證每個字形的 fallback。

獨立實站檢查：`cargo run --locked --features desktop --bin pttstella -- --probe-live target/verification/live`。這會連至 `wss://ws.ptt.cc/bbs`，只確認登入前提示並斷線，不送帳密；不包含在任何一般 gate。

## Windows ZIP 測試包

在 Windows x64 安裝鎖定 Rust 工具鏈、Python 3.11+、Visual Studio C++ Build Tools 與 Windows SDK；執行環境需要 Microsoft Edge WebView2 Evergreen Runtime。

```text
python tooling/check.py core
python tooling/check.py desktop
python tooling/bundle_windows.py
```

產物為 `dist/windows/PTTStella-0.1.0-dev.1-windows-x64.zip`，附 `.zip.sha256`。解壓整個資料夾後執行 `pttstella.exe`；不需要 Rust／Python／Node。圖示由同一 PNG 母檔衍生七種尺寸 ICO，嵌入 EXE 並用於視窗／工作列；包內另附 ICO、必要的 app-local VC++ runtime DLL、README、授權資料及檔案雜湊 manifest。WebView2 Runtime 須另外安裝。本機 ZIP 未作 Authenticode 簽署，不包含私人設定、帳密或流量。

腳本不覆寫既有 ZIP，改用 `--out-dir dist/windows-next` 建立另一份；若找不到 Microsoft redistributable CRT，使用 `--crt-dir` 指向 Visual Studio `VC/Redist/MSVC/<version>/x64/Microsoft.VC*.CRT`。打包本身不代表 GUI 或實站登入通過；平台與驗證結果見 active plan，Windows 仍使用 terminal 手動登入，未提供 macOS Keychain 功能。解壓後可用 `python tooling/check.py desktop --desktop-executable <EXE絕對路徑> --desktop-output <獨立驗證目錄>` 驗證實際交付的 EXE，不重新編譯；此模式只跑 GUI，建置檢查須另行完成。

## macOS App

在 Mac 安裝鎖定的 Rust 工具鏈、Python 3.11+ 與 Xcode Command Line Tools 後，執行：

```text
python3 tooling/bundle_macos.py
open dist/macos/PTTStella.app
```

有本機資源 harness 的環境，須依工作規則將打包腳本放在 build wrapper 內執行。產物使用本機架構及 ad-hoc 簽章，可移至 Applications；開啟後會自動連上預設站，仍須手動登入。既有 `.app` 不覆寫，可用 `--out-dir dist/macos-next` 保留另一份。這不是 Developer ID 簽署／Apple 公證的發佈版本，也不代表 macOS 完整功能驗收。細節見 [macOS App 打包](docs/macos-app.md)。

macOS 原生選單支援 ⌘W 關窗、⌘Q 結束、⌃⌘F 全螢幕及 Window → Zoom；關窗／結束會先清理 renderer 與連線。Vim Normal 將按鍵轉成瀏覽意圖後再轉成 PTT 指令，並避免中文輸入法將瀏覽鍵啟動為組字；i／原生一次仍提供中文輸入。實測範圍與未驗收項目見 active plan。

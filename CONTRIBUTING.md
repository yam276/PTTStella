# 開發與驗證

目前產品為 [terminal 外觀的 BBS 介面](docs/active/minimal-browser.md)，以 PTT 為預設站，並有按需的 [媒體預覽](docs/media.md)。核心、Dioxus 視窗、WebSocket／Telnet 及 [站點與編碼](docs/sites.md) 已實作；手動登入的實測狀態見 active plan。沒有 shell／PTY 或一般系統終端功能。

## 當前 gate

日常功能以 `develop` 為 base；`main` 只接受遞增版本的 Release PR。工作流程、人工驗收邊界與目前 GitHub 方案的分支保護限制見 [自動發佈](docs/releases.md)。發佈 tooling 變更須跑 `python -m unittest discover -s tooling -p 'test_*.py'`、docs、`actionlint .github/workflows/*.yml`，並手動執行 workflow 的 verify 模式，核對各平台實際建置；不能以一台機器的測試代替其他 runner。

需要 Python 3.11+；Git 用於提交與歷史檢查。RTK 是個人 shell 包裝，產品與 gate 不依賴它。

```text
python tooling/check.py docs
git diff --check
```

`docs` 檢查 UTF-8、必要文件、相對檔案連結、fixture／verification JSON、repo 內容及 gate 回歸測試。不檢查 heading anchors、遠端 URL，也不代替產品測試。搬移 checkout、沒有 Git metadata 或 sibling repo 時仍可檢查產品文件。

```text
python tooling/check.py core
python tooling/check.py desktop
python tooling/check.py all
```

core 執行 Rust 1.97.0 的 fmt、Clippy、畫面與互動測試 suite 及 2 個獨立 expected replay；desktop 包含本機傳輸／TLS 設定測試，再於 Windows 開真正的 Dioxus／WebView2 視窗，檢查雙色字像素、獨立背景、16 色與強度切換、填滿本體、送鍵橋接、監聽器／resize observer 清理與 connection worker join，產物位於 target/verification/desktop。all 跑離線 gate，不連 PTT。退出碼 0＝指定 scope 全部通過、1＝失敗、2＝必要能力未就緒；其他 OS 的 desktop 尚為 not_ready。腳本輸出 JSON，附 commit／dirty、fixture digest 與平台；root 從腳本位置決定。

`cargo run --locked --features desktop --bin pttstella -- --probe-live target/verification/live` 是另外的實站動作，只接收登入前畫面與回覆協定協商，確認後即斷線；報告只存結果與接收數量，不存畫面、原始流量或帳密。一般 gate 不執行它。真正登入需在正常視窗人工輸入，看到站端登入後畫面才可宣稱登入通過。

`--probe-site OUTPUT_DIRECTORY ADDRESS uao|big5|gbk` 使用同一 production Connection 與選定編碼，最多等待 20 秒取得可見登入前文字，再正常關閉。只有協商 bytes 不能通過；不送鍵或帳密、不讀鑰匙圈，也不保存站端畫面。WebSocket 未收到 Close 回覆仍記失敗；Telnet 僅記 TransportClosed。這也是明確的網路診斷，不包含在一般離線 gate，不能當作實站登入驗收。

站點變更另跑 `cargo test --locked --no-default-features --test sites`、desktop lib／bin、`node --test tooling/test_surface_sites.mjs tooling/test_surface_dialogs.mjs tooling/test_surface_navigation.mjs` 及 i18n／Clippy。`sites` 原生探針需要獨立本機 Telnet peers：以 GBK／UAO 自製中文字驗證雙色格線、精確貼上 bytes 與 EOF；介面需完成無效網址拒絕、最愛保存與移除、預設保存、GBK→UAO→GBK 三次切站、PNG 及全部 owner join。`sites_restart` 原樣複製 probe-sites.json，啟動即連相同 loopback 預設站，驗證 GBK 往返及設定未自動改寫。合成站點只准連 127.0.0.1；僅這兩種 Telnet 案例接受 TransportClosed，不放寬既有 WebSocket Close 或滑鼠往返的條件。

## 實作後的驗證

短文章末頁接頁另用 `--probe-surface OUTPUT_DIRECTORY short_overlap --media-origin LOOPBACK_ORIGIN`：自製兩頁回報邏輯 1～22／9～31，實際末頁從第 10 個畫面列開始，合併後必須完整保留 32 列。首尾頁尾分別使用藍底／青底，須核對 Home／End／Home 隨來源頁換色及雙色樣式；來源 PNG 保留站端末頁配色。包含相同黑背景但空白前景色不同的重疊、簽名檔、末尾兩列與歷史一般網址；peer 只接受一次 PageDown 和 Close。原生須確認末尾可見、Home 回看並左鍵點不在站端當頁的一般網址（probe 擷取到 sink，不啟動外站）、Enter 展圖、來源 PNG 與完整清理；報告須有完整 article_reading 1～32/end/complete、link request 及既有 batch 計數。真正系統瀏覽器啟動另外記實站或正常 App 的結果，不用 sink 當作外部啟動證明。

全文載入另跑 core、desktop lib／bin、Clippy、fmt、完整 JS 與 docs。`--probe-surface OUTPUT_DIRECTORY full_load --media-origin LOOPBACK_ORIGIN` 提供 20 頁、441 行自製文章，peer 逐筆核對 19 個 PageDown（延遲與分片回覆），再僅接受 Close。需取消並確認不再前進、明確繼續至 100%，Home／End／PageUp／PageDown 本機回讀不得加 bytes；完成前 HTTP 零請求，之後兩次長按 Enter 一個 batch，三個圖片網址（含 404）各一次 GET。核對完整來源 PNG 960×576 不含展圖、Close acknowledged、全部 owner／HTTP server join 與程序退出。`terminal_pages` 使用兩頁 peer，停留第一頁不自動取頁，一次手動 PageDown 才進第二頁，圖片 HTTP 零請求，匯出 PNG 後完整清理。這些本機案例不代替實站或正文滑鼠人工驗收。

原生 `window_probe` 與 `filters_probe` 明確採終端機文章模式，保留各自原本的逐筆站端鍵盤／刷新契約；網頁全文另由上述案例驗證。單頁的 inline／context menu／名單、文章定位及 pointer-row 合成畫面使用完整的 1/1、100% 頁尾，不以測試專用開關關閉網頁模式的載入。

短文章格線／網址變更另跑 article cache 的完整格線及連結回歸，涵蓋 1／3／22／23 行、一般網址、OSC 8 圖片、選取及 ID 字串。`--probe-surface OUTPUT_DIRECTORY short_links --media-origin LOOPBACK_ORIGIN` 提供三行文章、一般及 OSC 8 圖片各一張；owned HTTP server 提供 `/colours.png`、`/colours-two.png`。實際 WebView 需核對兩條網址底線、兩次長按 Enter 只展開一個 batch、兩圖解碼且各一次 GET、完整 960×576 PNG、Close acknowledged 與全部 owner join。peer 拒絕所有站端輸入；此鍵盤展圖案例不替代原生滑鼠點圖／hover 驗收。

滑鼠瀏覽增量另跑 core、desktop lib／bin、Clippy、i18n 與全套 JS。`--probe-surface OUTPUT_DIRECTORY pointer_rows` 現在以本機合成主選單開始：點個人設定、原始 Left 返回、點我的最愛及目錄、滾輪下／上頁、點文章整行及左緣返回；peer 依序獨立核對 Down＋CR、Left、CR、Up＋CR、PageDown、PageUp、Down＋CR、Left，共七筆導覽與一次原始鍵，不接受多送。最後必須匯出 PNG、Close acknowledged、所有 owner join。原生右鍵另核對無反白開扇形、有反白／Shift 保留情境、checkbox 自動保存／重開／重啟、隱藏浮球仍可返回收合；hover 目視亮度與金邊，Canvas／匯出不可混入本機裝飾。DOM doubles 與 loopback 不能代替原生 UI，缺平台明記 not_ready。

Windows ZIP 打包變更另跑 `python -m unittest discover -s tooling -p test_bundle_windows.py`、docs、core 與 desktop gate。`python tooling/bundle_windows.py` 由 Cargo JSON 取得 x64 release executable，檢查 GUI subsystem、實際 PE imports、七份嵌入圖示與來源 ICO payload，打包 Microsoft redistributable CRT 與授權，不覆寫既有 ZIP。publication 回歸涵蓋建置／圖示／未知 DLL 失敗零交付，以及既有 ZIP 和競爭建立不被覆寫。

Windows Setup 變更須執行全部 Python tooling 測試、docs、core 及 workflow actionlint，並在 Windows 以 Inno Setup 7.1.0 執行 `python tooling/check_windows_installer.py --output <新的獨立目錄>` 與實際 Setup 編譯；該原生政策 probe 不操作真實 registry／Runtime。已有／缺少 Runtime、離線、安裝失敗、升級與解除安裝保留資料另需測試環境驗收；缺 Windows runner 必須記 `not_ready`，不能把合成 adapter 當作真實補裝已通過，詳見 [安裝包契約](docs/windows-installer.md)。

Windows 免安裝 EXE 變更另跑 desktop lib／bin 測試與 Clippy。`--installer --portable`／Release 打包須用同一次靜態 CRT 編譯，核對 PE 無非系統 DLL 依賴、嵌入圖示及空目錄 `--export-licenses` 與編入授權逐 byte 相同；任一失敗不能交付 Setup 或免安裝 EXE。原生授權匯出不啟動 WebView／網路，不等於 GUI 驗收；仍須在乾淨 Windows 10／11、只有 EXE 的資料夾啟動並確認內嵌資產、輸入、正常斷線及缺少 WebView2 的行為。其他 OS 的 CLI 結果不能代替 Windows 靜態連結成功。

必須解壓 ZIP 至獨立目錄，核對 manifest 所列 SHA-256，並對解壓的 EXE 執行 `python tooling/check.py desktop --desktop-executable <EXE絕對路徑> --desktop-output <獨立驗證目錄>`。此模式不重新編譯，報告保留 executable 雜湊與 `build_checks_run=false`；兩個程序均從 EXE 所在目錄啟動，使用同一合成／loopback gate，不能將這份 GUI 結果當作編譯或 Clippy 已執行。指定 EXE 必須提供獨立 output，缺檔不能退回 debug binary。未在乾淨 Windows／不同版本／DPI 驗證的範圍仍須分列，實站登入另由使用者人工確認。

Windows CDP runner 以 trusted 點擊浮球與分類揭露功能，操作後經同一浮球返回並收合，避免外部點擊只收合工具而被誤當作 BBS 操作；設定在同一主 WebView 的 .settings-host #font-dialog 操作，不建立或探索第二個 debug target。Vim 採 120ms 按鍵節奏，讓無站端重繪的 no-op 等待同一 command owner 完成；不宣稱最高速連發通過。貼上要求 typed rejected 狀態與目前多站版的原因文字，服務端仍獨立比對精確 bytes。這些是實際 WebView 的自動化操作，不代表實體 IME 候選窗或人工審閱。

macOS 打包變更另跑 `python3 -m unittest discover -s tooling -p test_bundle_macos.py`，確認圖示轉換失敗不簽署／交付半成品、既有 App 不被覆寫。原生打包後需核對 ICNS 各尺寸、透明角落、plist 指向與 strict 簽章；這些不代替桌面功能 gate。

站端滑鼠另跑 `cargo test --locked --features desktop --lib mouse`：同一 Screen／CSI parser 的每個分片、固定模式 trace／epoch、RIS／session reset、同步期間模式、取消與超長拒絕，以及純 encoder 的獨立 SGR bytes／修飾鍵／格線邊界；交易 owner 與 production Connection loopback 核對更新後釋放、舊租約／取消、滿佇列控制、速率、模式／重連及保活。UI 另跑 desktop bin 與 `node --test tooling/test_surface_mouse.mjs tooling/test_surface_links.mjs tooling/test_surface_dialogs.mjs`，涵蓋顯示 ACK、封包型別、pointer 交易與分流；不以單元或現有本機滑鼠 gate 代替新的站端原生互動驗收，需求見 [站端滑鼠](docs/station-mouse.md)。

`--probe-surface OUTPUT_DIRECTORY mouse` 使用固定自製文章列表及真正 production Connection；peer 精確要求零基 col 4／row 4 的左鍵 press／release，切成未支援 1001 後拒絕站端滑鼠與滾輪，只有明確停用後點同一列右側文章區（如 col 40）可送一次 CR。重新啟用後要求同座標右鍵一對，再要求中鍵按下；此時 peer 送同步 off／on 撤銷交易，只接受 Close，不能補送中鍵釋放。以原生 Shift 雙擊選行、Cmd-C／草稿 Cmd-V 讀回系統剪貼簿，草稿不保存；另外核對開關、PNG、一次本機導覽、單筆取消、Close acknowledged 與全部 owner join。中鍵使用可保持 1,200ms 的真正按下，以便模式更新先於釋放。不能只看程序退出碼，缺報告或 cleanup false 必須失敗。原生 Shift 拖曳與 Ctrl＋滾輪、所有 DPI／其他平台仍須另驗，不能用這個限定案例冒充通過。

`--probe-surface OUTPUT_DIRECTORY mouse_boundary` 共用同一格線與模式，但 peer 只接受 col 4／row 4 的一次左鍵按下及 Close。先點格線外的黑邊，再從該格拖曳到黑邊放開；黑邊點擊不得送出任何 bytes，離開格線不得合成座標或釋放／本機導覽。報告必須只有一筆按下、至少一次交易取消、沒有本機導覽／連結／開關操作、沒有殘留交易，另需匯出 PNG、Close acknowledged 及全部 owner 清理。這個案例只驗同一原生尺寸的黑邊與格線外放開，不代替離窗、失焦、所有 DPR 或實體修飾鍵手勢。

`mouse_drag` 使用同一自製場景與 1002＋1006，要求零基 col 4／row 4 左鍵按下，沿同列移至 col 10 後釋放。peer 只接受獨立手寫的該水平區間 SGR motion，最多 16 筆，必須抵達終格並收到精確釋放；達到筆數上限仍缺釋放時必須失敗。`mouse_motion` 先在 col 4／row 4 點擊、釋放，再由 peer 開啟 1003；之後只接受同一水平區間的無按鈕 motion，終格 col 10 必須到達。原生工具可用指向終格的水平滾動移動指標；水平輪本身應零站端 bytes，不能改用合成 DOM 事件通過。

`mouse_round_trip` 使用 1002＋1006 與可見的大色塊：零基 A 為 col 8–23／row 7–11，B 為 col 48–63／row 7–11；允許在 col 4–67／row 5–13 的點狀走廊內移動，不要求停在單格或收到每一個中間 cell。按下可在走廊內，但只有按下或按住移入 A 才顯示綠色 READY；起點周邊不等於已到 A。之後必須保持按住進入 B；peer 重繪第二列 `HOLD: return to A, then release` 後，必須收到返回 A 的 held motion，最後在 A 放開。UI 額外確認返回事件發生時提示 frame 已由 renderer ACK；釋放不能代替返回 motion。等待人工開始的總期限為 15 分鐘；正式持有交易仍維持原本 30 秒期限。失敗立即重繪紅色 FAILED，不把斷線後的本機反白算作站端操作。peer 獨立解析 canonical SGR bytes，拒絕修飾鍵、其他按鈕、重按、越界、順序缺漏及額外輸入；上限 256 motion 加一對 edge，不改正式輸入佇列／速率／交易期限。完成後還原 fixture、切到 1001，另需 PNG、Close acknowledged 與全部 owner join。測試包含合併取樣、不同區域落點及各個缺步負例；原生人工操作前不宣稱通過。探針的非預期 binary 診斷只顯示長度或單獨 Escape 的分類，不輸出任意輸入內容。

probe 已收集的互動報告在 cleanup／peer 失敗時仍保存，強制 `status=failed`、`cleanup_completed=false`，未知 disconnect／join 欄位為 null；不能因可讀到報告或程序退出碼 0 而判通過。成功 cleanup 仍須 Close acknowledged，並保留原先互動判定。若 run 本身在建立報告前失敗，`cleanup.json` 仍獨立記錄，不虛構缺少的互動。

`--probe-surface OUTPUT_DIRECTORY tools` 是浮球與本機選字的自由試用，沒有啟用站端滑鼠模式或 A/B 往返驗收；一般拖曳不會觸發該驗收的 FAILED。peer 仍拒絕所有站端輸入。報告的 passed 僅代表明確匯出 PNG 與正常 Close／owner 清理；浮球放手／滑行是有界觀察，不以這個 smoke 結果代替實體手感或 `mouse_round_trip` 的嚴格條件。

mouse report 的 `edges` 保留至多 16 筆已准入事件的格線座標、按鈕、動作與修飾鍵；原 `reports` 的按鈕／動作配對從同一份有界記錄衍生，沒有第二份事件 owner。這些是 UI 觀察，peer 的獨立精確 bytes 判定仍為必要證據；只出現 UI press 不能證明 socket 已接受，亦不包含原始鍵入文字。

`mouse_wheel` 從 1000＋1006 開始，要求 col 4／row 4 的向下 tick；畫面顯示 `WHEEL: now scroll UP` 並切到 1002，再要求同格向上 tick，允許有限筆同方向原生重複。完成後還原原場景。三個案例完成時都切到未支援 1001 停止回報，接著匯出 PNG、正常關窗；UI report 需非空且不超量、正確按鈕／motion／方向、零本機導覽／連結／偏好切換、沒有 held、Close acknowledged 與所有 owner 清理。每次原生執行的成功／失敗各自保留，raw input 單元僅驗證探針本身，不算原生操作。

內建字型／桌面資產路由變更另跑 `cargo test --locked --features desktop --bin pttstella`，檢查固定路由的 MIME、原始字型標頭及未知路徑／方法拒絕；主畫面、預覽、保存及 PNG 仍須實際 WebView 驗證。新字型來源、SHA-256、著作權及授權需記錄於 assets/fonts。

2FA 引導另用 `--probe-surface OUTPUT_DIRECTORY login_2fa`，開啟 Vim 後按鈕登入合成帳密；原始輸入六位錯碼、八位救援碼，進入主選單後明確 Esc 回 Normal，再正常關窗。peer 精確核對一次帳密與兩次手動碼，額外輸入失敗；報告需包含提示／錯碼／瀏覽三個狀態、Close acknowledged 與 owner join。不代替實站 2FA 或原生 IME 驗收。

SGR 66 顯示變更另使用 `--probe-surface OUTPUT_DIRECTORY sgr66_legacy` 與 `--probe-surface OUTPUT_DIRECTORY sgr66` 啟動兩個獨立程序，沿用相同 probe-settings.json 與字型；在原生視窗點「匯出 PNG」後正常關閉。兩者使用等價自製串流、本機 peer 拒絕站端 bytes，各自留下 export.png、surface-probe.json 與 cleanup.json。需獨立解碼兩張 960×576 PNG 核對全部像素及已知左右背景／明暗反相，不能只檢查 PNG 標頭或兩份 Core 相等；CPU 核心另測每個分片、單寬字、控制碼取消、覆寫／捲動與 session reset。這不代表已驗證 UTF-8 或其他 OS。

Vim gate 由真正 WebView 編輯／按鍵事件送至 Rust policy，再由本機服務逐筆比對原生一次 h／Ctrl+d／中文、計數、半頁、gg/G、h/l 與模式返回；前綴取消不能夾帶原始字母。macOS 原生 window probe 另以 46 筆精確輸入覆蓋主選單／看板／最愛／文章、無更新時的列表移動，以及三次進板提示的 l／Enter／空白鍵繼續與後續六位數文章列表操作；最後六筆覆蓋命令說明頁的移動／首尾／返回及空列表返回，l／Enter 執行由負面案例拒絕。每次換頁加入 DEC2026、未知 ESC intermediate 與超長 CSI，畫面分兩段、間隔 120ms；runner 應觀察更新後再輸入，不能把本例宣稱為極速連發或逐像素防撕裂證據。同步持有舊圖、拒絕舊導覽、允許原始輸入／Ping、缺結束恢復及批次中關閉另由 Connection loopback 與核心測試核對。進板辨識、兩種鍵盤模式的一般按鍵、長按跨畫面、重複／過期 Continue 另由核心與 Connection 的負面案例核對。暫停提示變更另跑 `node --test tooling/test_continue_keyboard.mjs tooling/test_article_pointer.mjs`，檢查橋接保留 IME／本機快捷鍵、整格線單次點擊、網址優先權覆寫及拖曳／Shift／雙擊／忙碌／過期／幾何變更拒絕；DOM doubles 不算原生點擊驗收。可攜顯示設定驗證兩份具名檔、固定 preset、載入預覽／取消／套用與錯誤保留，重啟程序另外比對保存結果和全部畫素。probe 只替代原生 picker 的路徑，沒有宣稱操作原生選檔視窗。

圖片 hover gate 以 trusted pointer 與 owned HTTP server 核對快掠零請求、同連結一次、取消 TCP EOF、已完成／下載中提升不重抓、跟隨與固定位置、右下碰邊及停用偏好；Canvas、焦點與站端輸入保持一致。GIF 小預覽檢查靜止首幀，組字開始取消；Rust lease 測試則覆蓋 frame revision 續用與新輸入／session／來源範圍失效。以上實機 backend 為 Windows WebView2；其他 OS、原生輸入法候選窗與實站仍需各自驗證。

媒體尺寸變更另跑 `node --test tooling/test_media_i18n.mjs`。以 `python3 -m http.server 60271 --bind 127.0.0.1` 在 repo 根目錄提供靜態頁，於瀏覽器開啟 `http://127.0.0.1:60271/tooling/media_layout_probe.html`；六個案例均須回報 passed。此頁使用生產 CSS／媒體控制器及自製圖片，核對大／小／橫／直圖、窄視窗、右上與碰邊位置、預覽提升重用圖片、原尺寸／符合及清理。它只驗證實際瀏覽器排版與解碼，程式呼叫的控制項不能代替原生 trusted pointer、Rust 准入或 HTTP gate；結束後關閉本機 server。


選取測試以真正拖曳選中合成雙色中文字的右半格，透過瀏覽器 Ctrl+C 觸發 copy 事件，另一個程序只比較系統剪貼簿與預期完整中文字，不輸出剪貼簿內容。另檢查整頁選取有 24 列，清除選取後 Ctrl+C 恢復送出 0x03。此測試會將系統剪貼簿設為合成資料。

IME gate 使用 CDP 的 imeSetComposition，驗證取消不送出、確認送出一次、接著輸入相同字不被去重，以及組字宿主位於合成游標座標。CDP 完成組字時的 compositionend 可能不是 trusted；此時只接受由 trusted compositionstart 與最後 trusted input 確認、且文字完全相同的組字結果。這不代表已操作原生輸入法候選窗。

另在真正組字確認後，派送負面事件模擬 legacy WebKit 的 `compositionend → keydown(Enter, keyCode=229, isComposing=false)`，以及 textarea 的 `insertLineBreak`／`insertParagraph`。這些事件不得送出 bytes，IME 按鍵也不能 preventDefault；隨後立即由瀏覽器送出一次普通 Enter，loopback 逐筆比對確認只有該次 Enter。合成負面事件只驗證防漏送規則，不算 macOS 原生輸入法通過。組字宿主另驗證無框、內容寬度、行高及游標座標，保留 `ime-inline.png`。

貼上 gate 以背景 PowerShell 設定合成剪貼簿，再由真正 Ctrl+V 觸發 paste 事件。檢查單行不附加 Enter、多行預覽內容、取消與送出，以及不可編碼、超量、ESC 三種拒絕；本機服務端比對完整輸入序列，拒絕／取消不能混入 bytes。原生 dialog 關閉後另驗證正常字母按鍵，不能只檢查 DOM activeElement，避免 native 編輯焦點未恢復的假通過。

網址 gate 以實際滑鼠拖曳、點擊與右鍵選取合成網址，驗證拖曳不發起開啟、點擊恰好發起一次，以及系統剪貼簿的網址文字。一般 probe 的 opener 是確認 sink，不啟動外部瀏覽器；實際 OS 預設瀏覽器另由人工驗收。純 Rust 案例涵蓋字寬映射、標點／括號、大小寫、port、IPv6、行末截斷、未映射字格及非 HTTP scheme。

媒體 gate 由 `tooling/desktop_media.py` 的 owned loopback HTTP server 提供自製 PNG／GIF 與實際 WebView 產生的 JPEG／WebP／WebM。trusted 點擊經生產網址分類與 HTTP loader，檢查圖片像素、等比縮放、真實 GIF 紅／藍幀、停止回首幀、WebM `requestVideoFrameCallback` 與播放時間、取消的 TCP EOF、快取命中、解碼失敗後重新抓取、外開 sink、焦點及原 BBS canvas／input epoch 不變。GIF 幀使用螢幕 crop，再由 System.Drawing 獨立解碼；[Canvas 的動畫圖來源只取預設／首幀](https://html.spec.whatwg.org/multipage/canvas.html#image-sources-for-2d-rendering-contexts)，不可用 drawImage 假裝驗證實際動畫。YouTube 縮圖在 probe 換為明確本機來源，沒有接觸網站。HTTP lib 案例另檢查未知長度超限、redirect、no-store／Age、尺寸與動畫上限、總期限涵蓋 body，以及 drop 後的連線關閉。實際網站、MP4／OGV codec、動畫 WebP、外部瀏覽器及其他 OS 尚待實機驗收。

PNG gate 點擊真正匯出按鈕，以相同 renderer 畫出離屏快照，probe 使用固定產物路徑代替原生另存視窗。`tooling/inspect_png.ps1` 使用 Windows System.Drawing 獨立解碼檔案，確認尺寸、左右背景、方塊及游標排除；另確認匯出不改變可見畫布與捲動。PNG 標頭檢查只負責橋接尺寸／格式及容量，沒有當作通用 PNG 解碼器。檔案替換另測成功與失敗保留原檔／清理暫存；原生另存、取消及 OS 視窗互動仍需人工驗收。

選取行 PNG 由 Rust 選取結果提供完整行範圍，Rust 核對畫面／選取 request 與 PNG 尺寸後才保存。`tooling/desktop_capture.py` 使用 trusted 滑鼠輸入及實際按鈕，驗證雙色字單行、反向多行、首尾／空白行與整頁；獨立 System.Drawing 解碼後，以對應整頁區域的全部像素雜湊比對，不能只看標頭。另驗證縮放不改輸出、畫布／捲動不變，以及清除選取後停用按鈕。輸出範例位於 `target/verification/desktop/capture-lines-*.png`，只含自製資料。

`navigation` 核心 suite 使用自製列表與閱讀 fixture 驗證多條件辨識、編號 cell 命中、精確送鍵計畫與負面案例。只有 footer 文字相似、沒有指標、列表缺行、彈窗、未映射 byte 或未完成控制碼均不算可導覽畫面。

production Connection 的導覽案例以 loopback WebSocket 比對實際 bytes：鍵盤先入列後的舊點擊、等待更新時的重複點擊、翻頁邊界、過期 frame／session、未完成控制碼及已取消的排隊請求均不得混入額外輸入。

`tooling/desktop_navigation.py` 接續同一個真實 Dioxus probe，以 trusted pointer／wheel 驗證編號點擊、標題雙擊／拖曳、鍵盤與畫面更新時取消 held click、原生 lostpointercapture、翻頁按鈕、列表移動及閱讀邊界。本機服務端逐筆比對 bytes，組字／dialog／未知畫面、雙擊延續與放大後本機捲動不能夾帶額外輸入。hover 的獨立細底線另外驗證樣式與 Canvas 逐像素不變，保留列表及閱讀工具列合成截圖。此流程不代表實站文章或實體滑鼠／IME 已驗證。

字型 gate 以實際表單輸入驗證預覽／取消不改主畫面或檔案、保存後修改字形像素、無效值拒絕，以及恢復預設僅預覽。第二個應用程序讀取第一個程序寫出的設定，畫面逐像素相同才通過；probe 設定只在 target/verification/desktop，測試開始只清除該路徑下的合成設定。Rust 案例另涵蓋損壞／超量檔案、備份、無效值與備份寫入失敗。系統字型 fallback、跨 DPI 與 macOS／Linux 的實機結果不包含在此驗證。

防閒置使用 [保活契約](docs/keepalive.md)。時鐘單元案例用明確時間驗證 60 秒與計時缺口；4 個 production Connection loopback 案例縮短同一 timer 的期限，檢查精確 Ctrl+L bytes、UI／畫面／session admission、等待與關閉不重複、無回應後釋放自動等待，以及不能清除較新手動輸入。Windows trusted 操作另檢查預設開啟、切換與選字／組字／dialog／未知畫面對實際 owner 的暫停狀態。短期限測試與來源推論不能冒充實站長時間保活或實機睡眠驗收。

ANSI 調色盤以合成完整方塊的實際 RGB 檢查，與獨立 expected 比對；不以文字反鋸齒的最常見顏色推定色值。中文字形仍另測左右紅／綠筆畫與藍／紫背景。

符號比例以自製 `●○◎⊙` 合成列，經真正 renderer／PNG 匯出後，由 System.Drawing 獨立解碼量測筆畫 bounds；Windows 預設字型四個樣本的寬高差不得超過兩像素且必須有可見筆畫。這是指定 backend／字型的回歸案例，不宣稱所有符號、字型或 DPI 皆已驗證。字型設定預覽包含相同符號，方便人工對照。

桌面探針另驗證 hover 工具列、實際縮放按鈕、100／125／150% 畫面比例、連續方塊像素及半方塊／三角形幾何。`--probe` 明確停用正常啟動的自動連線，不輸入私人帳密或存取實站文章。

輸入 gate 使用獨立 WebView2 profile 與僅 probe 啟用的 loopback DevTools port，由 `tooling/desktop_input.py` 發送瀏覽器滑鼠點擊與文字／按鍵輸入，驗證畫面、黑邊、縮放控制後的焦點。ASCII／中文編碼、Enter 與方向鍵經同一生產 Connection 送往本機 WebSocket；合成確認提示顯示後，再以 `n`、Enter 完成往返，必須收到完成畫面並確認繪製。服務端接著關閉連線，檢查代碼／原因文字以及移開滑鼠後仍可見的狀態。只接受瀏覽器 trusted input；直接 dispatchEvent 不算通過。這不是實體鍵盤／IME 或實站登入證據。正常執行不開 debug port；runner 持有並等待自己啟動的程序，失敗時終止該 probe。

| 變更 | 證據 |
| --- | --- |
| 文件／規則／gate | docs、diff；gate 變更包含負面與搬移測試 |
| 核心／編碼 | 鎖定工具鏈、Cargo.lock、fmt／Clippy、核心測試及同一生產核心的非空 replay |
| renderer／輸入 | 核心加真實 Dioxus 視窗、顯示與送鍵測試；不能只編譯或擷取非空畫面 |
| 傳輸／登入 | 離線 framing／錯誤／取消測試，以及獨立、有界、手動實站登入流程 |

第一個 Rust 交付選定實際驗證的版本，啟用對應 gate 並讓 CI 使用同一入口；目前沒有宣稱 MSRV 或 CI 成功。文件修正不必啟動 GUI，產品完成不能只跑 docs。

macOS 本機 `.app` 的建立入口為 `python3 tooling/bundle_macos.py`，細節見 [打包說明](docs/macos-app.md)。打包變更須跑 docs、diff 檢查並在 Mac 實際建立 release app，確認 plist、系統依賴與 ad-hoc 簽章；本機資源政策仍包住整個腳本。`bundled` 只代表產物完成，不能當作 desktop gate 或人工登入通過。

登入變更須跑 core login suite、desktop lib 的精確收送／取消／期限／重連案例，並在 macOS 以 `--probe-login OUTPUT_DIRECTORY` 操作自製帳密，驗證鑰匙圈保存、第二程序取用與移除；probe 只使用獨立的 synthetic service，不得放入私人帳密。關閉後檢查 `login-probe.json` 及 worker／listener 清理。實站登入由使用者直接在 App 輸入，另行驗收，不能以本機往返結案。

斷線變更另以 production Connection 驗證正常 Close 碼、延遲回覆／TCP FIN、未回覆即結束連線與無回覆時的期限／未確認狀態。原生視窗分別用 ⌘W、⌘Q 檢查停止輸入、關閉握手及 renderer／worker 清理；不能只以程序消失或 Close 已排入佇列當成站端回覆。

網址標示與 OSC 8 變更另跑 `node --test tooling/test_surface_links.mjs`；renderer double 另核對無 hover 的常駐底線、跨列範圍、50%／100%／125%／200% 比例、hover 加亮、連結消失與 PNG 不含提示。這些不代替原生像素驗收。它以事件／狀態 doubles 直接執行生產 handler，核對任一片段選取索引、組字／非連結拒絕及一次 released hit 的版本檢查，不冒充原生拖曳。`--probe-surface OUTPUT_DIRECTORY osc8 --media-origin http://127.0.0.1:PORT` 使用 owned loopback 與自製 HTTP 圖像。原生依序點第 9／10 列各一次、右鍵並複製各一次、跨兩列拖曳、開啟第 15／16 列圖片並分別關閉、匯出 PNG，再正常關窗。嚴格報告要求兩列開啟／複製、跨列 range、兩次圖片解碼及 PNG；缺輸入 backend 為 not_ready，PNG 或正常退出不能代替互動通過。

macOS CUA 若一般視窗座標不可用，可在獨立 probe 直接用 ⌃⌘F 進全螢幕，再按「符合視窗」並取得新 screenshot。截圖可能帶原生視窗的垂直偏移；先以右鍵選取的「選取行 PNG」第 9／10 行提示確認實際命中，再執行嚴格順序，不固定沿用其他視窗座標。以 DOM dialog 的關閉按鈕離開預覽／草稿可保留全螢幕；Escape 可能同時離開全螢幕。不要在原生視窗選單中透過 App 的按鍵介面按 Escape 當作關單，應用當前 AX 暴露的 Cancel 動作，避免送入 BBS。完整圖片判讀使用完整 screenshot；差分圖不能當作缺像素。

原生剪貼簿可在各次右鍵／Cmd-C 後，開啟本機書籤草稿並用 Cmd-V 貼回，核對 `https://example.com/pttstella?complete=one-two&tail=both`，再關閉、不保存。兩次必須相同完整 URL；嚴格資料仍需 open／copy rows `[8,9]`、dragged、兩次 decoded、PNG、HTTP `/colours.png` 一次及所有清理。此流程只替代 OS 外開為 probe sink，不連第三方網站；與先前失敗報告分開保留。

UI 准入與生命週期變更另跑 `cargo test --locked --features desktop --bin pttstella` 及 `node --test tooling/test_surface_lifetime.mjs`（Node 18+）。前者檢查單筆命令等待／取消、立即入列與 busy 不執行 factory、事件來源與 session、組字例外及超時保留清理；desktop lib 另用實際 WebSocket 比對 browse 在原始 Enter 之前入列，尚未 poll 回覆也不能改變順序。後者以 fake channel 驗證生產 JS stop owner 的正常接收、資料通道失敗、停止後不新增 receive，以及兩萬個封包不累積 unresolved stop Promise 的 reaction，沒有操作瀏覽器。

快速切板 UI 另跑 `node --test tooling/test_surface_navigation.mjs`，檢查本機快捷鍵、IME Enter／229、取消／晚到回覆、session 改變、草稿及 dispose。`--probe-surface OUTPUT_DIRECTORY boards` 由 owned peer 核對 Stella／Other 兩次完整流程，再等待 Beta 的返回並延遲 3 秒；應在同一次 UI 自動化呼叫內觀察最新 AX「停止導覽」並立即點擊，以免人與工具回合超過延遲。另需取消未確認草稿、輸入超長名稱確認完整保留並被拒絕，最後等晚到列表、正常關窗。嚴格結果須為兩個正式到達、至少一次拒絕、有效取消及無額外 bytes；若超過測試間隔而續送，保留失敗，不調整 expected 放行。

UI owner／封包變更再跑 `node --test tooling/test_surface_dialogs.mjs`。使用明確 DOM／clock doubles 核對晚到字型結果、關閉／revoke、舊預覽、登入草稿清除、名單 busy／失敗恢復及 router 拒絕未知／legacy envelope；這些不是原生 trusted 操作證據。[ui-packets](tests/fixtures/ui-packets.json) 是獨立編寫的 synthetic wire expected，Rust bin 比對生產 Packet 的 serde 結果，JS 測試核對分派，沒有用序列化往返當 expected。Paint 與內嵌預覽另檢查結構，完整呈現仍由原生 PNG／畫面驗收。

AID 核心另核對 article_navigation 的自製 article-open fixture 與 navigation_job typed progress。production Connection 的 article_tests 需精確比對切板、# 提示、八碼／CR、Q 資訊、空白返回及方向右；錯誤、權限提示、過期輸入、所有取消階段與晚到正確資訊不得續送。同步資訊框保持零空白且可 Pong，整體／每步期限不被碎片延長，重連零重播。BoardVisit 另證明導覽仍待輸入時已發布原 session 的到達事實，重連不得重貼舊 generation。既有 boards／window 原生結果只算共同 consumer 回歸，不能代替待接 AID 入口的原生驗收。

最近清單另核對 lib 的 recent_boards model／store 與 bin 的 Files writer／recent state 測試：正式到達、大小寫去重、20 筆上限、損壞／超量保留、備份、清除失敗、慢寫入合併、picker 期間保存與關閉 drain。`boards` 原生結果須保留 `[Other, Stella]` 並排除取消的 Beta；將該合成資料複製至新程序的 probe-recent-boards.json，用 `--probe-surface OUTPUT_DIRECTORY recent` 開啟切板視窗、確認順序、清除、等待完成後關閉，再以清除後檔案啟動第三個程序確認仍空。獨立讀取主檔與 .bak 均須為空；peer 拒絕所有站端 bytes。`boards_close` 另在第二次到達後開始 Beta，同一次 UI 呼叫確認「停止導覽」可見後立即 ⌘Q，檢查等待中的工作停止、Close acknowledged、各 owner join 與只保留兩個正式到達。兩個新 probe 不取用正式帳號／本機真實清單。

檔案工作／畫面狀態變更另核對 bin 的 Files 與 SurfaceState 案例：慢 blocking worker 不阻塞事件輪詢、忙碌不啟動第二筆、關閉時保留已開始工作且不吞失敗，以及單筆繪製／ACK、規則／來源／選取的失效。延期 picker 使用同一 production PNG 管線，八種狀態變更後舊 PNG 不寫，新請求仍須確實保存。JS 測試執行實際 clearSelection，以 [selection-cleared](tests/fixtures/selection-cleared.json) 核對直接撤銷通知；DOM doubles 不冒充原生操作。

`--probe-surface OUTPUT_DIRECTORY files` 使用合成主畫面與拒絕站端輸入的本機 peer，但 PNG／設定檔會走真正的原生 picker。確認原生 Save／Open、讀回 PNG／設定檔、取消及關閉清理；固定 probe 路徑不能替代 picker 驗收。`files_slow` 在 blocking worker 等待 runner 建立 OUTPUT_DIRECTORY/file-release（最多 120 秒），先寫 file-started.json，完成寫 file-finished.json。等待中可開啟／取消設定；第二筆保存應拒絕並恢復控制，⌘Q 後須保留 owner、顯示超時等待，release 後才完成清理及 Close。runner 必須收回自己啟動的程序與釋放訊號，不能留下背景工作。

`--probe-surface OUTPUT_DIRECTORY dialogs --media-origin http://127.0.0.1:PORT` 提供明確本機 UI／媒體 fixture，peer 拒絕輸入 bytes；操作設定預覽／自動保存／關閉 flush、具名設定檔匯出／匯入、整頁及行 PNG、本機圖片與外開 sink 後才檢查報告。媒體 server 必須由 runner 擁有並停止；不能將 sink 宣稱為 OS 瀏覽器實測。對話框拆分另以閱讀畫面逐一開／關名單、字型及登入，確認不用額外點擊或 frame 才恢復瀏覽控制項，再完成原 filters 回歸。

`--probe-surface OUTPUT_DIRECTORY CASE` 是不開 DevTools、不連實站的原生 WebView 探針；CASE 為 `decode`、`painted`、`send`、`renderer_error`、`untrusted`、`confirm` 或 `slow`。前四項分別造成真實事件解碼、版本、Eval 封包序列化及 JS 例外錯誤，應留下 `run_completed=false`、`cleanup_completed=true` 與 acknowledged 的 `cleanup.json`；錯誤畫面之後仍須能 ⌘W／⌘Q 結束。`untrusted` 驗證 JS 合成 paste／按鈕不改狀態，以及 Rust 拒絕三個不可信貼上 wire 事件；`confirm` 由原生貼上 `A\nB` 建立真實預覽，再拒絕合成確認且保留預覽。`slow` 持有 60 秒的合成結果 future，期間以原生按鈕開啟／取消字型設定並關窗，確認命令仍 pending 時就完成清理；它不宣稱模擬了真實網路 backpressure。三者檢查 `surface-probe.json`。所有案例的本機 peer 拒絕任何輸入 bytes，只接受 Close。負面合成事件不當成 trusted 正向輸入證據。

黑白名單顏色與推文辨識變更另用 `--probe-surface OUTPUT_DIRECTORY filter_colors`：自製完整推文以等價 ANSI 亮色編碼，分隔空白保留非預期前景色；不能因不可見前景色跳過推文。原生表單至少四次明確保存（共用色與三筆 ID），最後共用亮綠色、Alice 遮內容並保留亮紅 ID、Alice2 沿用共用色、Bob 個別亮紫色。匯出 PNG 後獨立解碼核對黑色遮罩與 ID RGB，檢查 schema v4、個別覆寫／共用值、Close acknowledged 與完整清理；peer 拒絕所有站端 bytes。合成 PNG 不代替實站格式與原生正文滑鼠驗收。

一般黑名單變更須跑 core 的 filters suite、desktop lib store 與 bin presentation admission 案例。`--probe-filters OUTPUT_DIRECTORY` 提供自製推文、獨立名單檔與本機 peer，不開 DevTools port、不連 PTT。原生視窗驗證新增／同 ID 更新／移除、拒絕與保存失敗、遮黑與暫時顯示、系統剪貼簿讀回、整頁／選取行 PNG；獨立解碼確認變更只在命中 cell 範圍、選取行與整頁對應裁切完全相同。peer 僅接受 PageDown／PageUp 往返及 Close，管理名單不能送其他 PTT bytes。檢查 `filters-probe.json`，並另以新程序確認規則保存；不能只以畫面有黑色或 DOM 元素存在當作通過。

## 資產與提交

資產與依賴須有來源、授權與版本；合成 fixture 保存獨立 expected。提交前檢查實際 diff，維持獨立 checkout 可建置及驗證。

後續以功能或可獨立驗收的修正為單位建立 Conventional Commit，body 說明結果、設計與實際檢查。只有使用者明確要求才壓縮或改寫歷史。工作完成更新 active plan；目前不實作板名導覽、文章模型或其他擴充功能。公開 repository、PR、套件出版與桌面發佈各自是另外的動作。

作者與提交者使用 `yam276 <59956724+yam276@users.noreply.github.com>`；提交前檢查 repo-local Git identity。依使用者要求將初期開發歷史整理為一個提交，回復備份保存在 repo 外。

介面翻譯變更另跑 `cargo test --locked --lib i18n::tests` 與 `node --test tooling/test_i18n.mjs tooling/test_media_i18n.mjs tooling/test_surface_dialogs.mjs tooling/test_surface_lifetime.mjs`。比較兩語的 key 與參數，核對完整訊息與複數、不可信內容仍為文字，並操作兩語原生視窗驗證保存／取消／重啟。媒體控制器的 DOM／network doubles 檢查中英文回饋、晚到結果與資源清理，不能當成 WebView 圖片解碼或 codec 驗收。語言偏好失敗不改生效設定，不能重連或重送輸入；原生選檔視窗的系統按鈕由 OS 決定語言。

字型清單／來源變更另跑 `node --test tooling/test_surface_fonts.mjs tooling/test_surface_dialogs.mjs`、desktop bin／lib 測試與全 targets Clippy。`--probe-surface OUTPUT_DIRECTORY fonts` 使用同一合成中英／符號／雙色字畫面、拒絕所有站端 bytes、固定 PNG 路徑及官方外開 sink；原生選字、取消／保存／重啟、重新讀取與來源按鈕需留下畫面及 PNG 比對。外開 sink 只證明請求目的地，不冒充真實 OS 瀏覽器啟動或字型下載／安裝。

指定文章入口另跑 `node --test tooling/test_surface_navigation.mjs`。`--probe-surface OUTPUT_DIRECTORY articles` 使用 Vim Normal：先取消未確認草稿，再以 `https://www.ptt.cc/bbs/Stella/M.1.A.001.html` 與 `#00000102@Stella` 兩次開啟；送 `#00000103@Stella` 後觀察缺文停止，明確按空白恢復。接著拒絕 `https://www.ptt.cc.evil.example/bbs/Stella/M.1.A.001.html` 及 `Stella`＋251 個空白＋`x` 的長草稿；長字串原生輸入可能尚未完成，應先確認 AX 中的完整 257-unit 值再按 Enter，不能把中途快照當成最終值。最後送 `#00000104@Stella`，在同一 UI 呼叫讀最新「停止導覽」並點擊，等晚到資訊框後正常關窗。`articles_close` 重跑相同步驟，最後改為觀察工作仍 active 後立即 ⌘Q。peer 精確比對兩次成功、一次缺文及明確手動恢復、最後延遲查詢；拒絕／草稿取消／晚到查詢不得多送任何 bytes。兩個 Reached、缺文觀察、至少兩次本機拒絕、取消或 active-close、Vim Normal、Close acknowledged 與所有 owner join 都必須成立。這只用自製 fixtures／本機服務，不連 PTT。

書籤另跑 `node --test tooling/test_surface_bookmarks.mjs tooling/test_surface_navigation.mjs`、desktop lib／bin、core、Clippy 與 Fluent。原生 `--probe-surface OUTPUT_DIRECTORY bookmarks` 從空合成檔開始：保存 `#00000101@Stella` 及 `#00000102@Other`，選第一筆改名為「自製更新」，移除第二筆，拒絕只有 Stella 的草稿並取消；重開確認清單，選第一筆按「前往…」，核對共用快速前往草稿後才確認。peer 精確核對一次完整文章流程，其餘修改／取消不得送站端 bytes；至少四次保存、一次拒絕、唯一正確目標與正式 Reached 必須成立。可匯出文章 PNG 並獨立解碼。

將該程序的 probe-bookmarks.json 原樣複製至新目錄，以英文 probe-settings.json 啟動 `bookmarks_restart`，檢查中文名稱與識別不變，再明確移除最後一筆；複製新空檔到第三目錄啟動 `bookmarks_empty`，確認仍空。第四目錄將 probe-bookmarks.json 設成自製 `broken`，啟動 `bookmarks_corrupt`，確認警告、保存／移除停用、輸入合法目標後 Enter 零修改。後三個 peer 拒絕所有站端 bytes，各程序需 Close acknowledged／所有 owner join／退出碼 0。獨立核對更名保留建立時間、移除後備份為前版 bytes、重啟 seed 相同、空檔及壞檔未自動改寫。只用獨立 probe 路徑，不取用正式書籤；原生貼上中文不代表 IME 候選窗通過。

浮球工具列變更另跑 `node --test tooling/test_surface_tools.mjs`：檢查點擊／拖曳分流、六類功能與直接設定、七入口及 4K 邏輯尺寸的非重疊幾何、最近 120ms／最多 16 筆軌跡的放手速度、末端小抖動、合併至 pointerup 的位移、停止／反向、慣性與吸邊、有界座標、外來 pointer、實體按鈕遺失、取消／失焦／隱藏／清理、減少動態效果與小視窗。正式功能仍沿用原本的按鈕 ID、Rust 准入及 owner，不新增第二條操作管線。合成 probe 關閉時附帶有型別、定長的浮球觀察（按下／移動／放開計數、最後放手速度、滑行幀數與減少動態效果）；正常模式不保存這些資料。原生驗證需確認扇形／各分類、拖曳後不誤開啟、選取後仍可匯出、沒有額外站端 bytes，以及正常 Close／全部 owner join；單元的模擬事件不算原生驗收。

Enter 文章內圖片變更另跑 desktop lib／bin、`node --test tooling/test_inline_images.mjs tooling/test_surface_links.mjs tooling/test_surface_dialogs.mjs`、i18n 及 Clippy。原生 inline probe 的 loopback 圖片 peer 應提供 colours.png 和明確 404 的 missing.png，核對兩次長按 Enter 僅各一個 GET、文章內插圖與錯誤、PNG 不含插圖、零站端 bytes 及 Close／owner join；瀏覽器排版測試不能取代此准入驗收。

自訂右鍵另跑 `node --test tooling/test_surface_context_menu.mjs tooling/test_surface_links.mjs tooling/test_surface_mouse.mjs tooling/test_inline_images.mjs`、desktop lib／bin、Fluent、core 與 Clippy。原生 `context_menu` 探針、剪貼簿讀回、選單不穿透／按鍵不送站端、PNG 與正常關閉的要求見 [右鍵操作契約](docs/context-menu.md)。剪貼簿單元使用注入 writer，不操作開發者的系統剪貼簿；原生探針才在明確測試操作時寫入自製內容。


黑白名單／匯入變更另跑 filters 核心與 store、filter_import owner、Files、SurfaceState、所有 JS／Fluent 及 Clippy。自製格式案例涵蓋 UTF-8 BOM、完整 ID、備註分隔、白名單衝突、未支援項目、容量及取消／舊 request。原生 `account_lists` 使用固定測試路徑，確認先預覽、取消零保存、合併後白名單只標示 ID、兩種黑名單分別遮內容／整行、選取複製與 PNG 一致、正常斷線及清理。`account_articles` 使用自製文章列表補驗相同三策略，核對遮內容的作者保留、整行 80 欄全黑、白名單只有 ID 改色、揭露與保存後重啟。兩種模式的原生 PNG 以獨立解碼核對範圍，複製讀回不得含被遮內容；站端零輸入且完整 Close／owner 清理。此探針不代替原生選檔視窗驗收。整行／左緣導覽另核對 `test_surface_links.mjs` 及 `desktop_navigation.py` 的真實 pointer、Shift 選取、光棒範圍與 Canvas 不變；缺平台 runner 仍記 not_ready。

文章快速名單與暫時篩選使用 `--probe-surface OUTPUT_DIRECTORY account_actions`：保持標準閱讀頁與頁尾游標，在 Alice 字串右鍵分別執行 mask／mask_row／allow，核對三次保存、備註與實際 PNG；以右鍵推文選單（或另行人工左鍵）啟用／取消只看此 ID，核對其他推文全黑、恢復像素相同、永久規則未改。探針須至少三次 account action／保存、兩次 focus action、PNG、零額外站端 bytes、Close acknowledged 與全部 owner 清理。單次動作計數不能代替像素、檔案或操作者；CUA 無視窗／逾時須明記 not_ready，不降低條件。關閉後勿再次要求原生工具讀取已退出 App，以免工具重啟正常副本；用 runner 退出結果核對。

連續推文合併另以 `--probe-surface OUTPUT_DIRECTORY merged_replies` 驗證自製標準推文：第一筆 ID 保留、下一筆相同 ID 只塗黑 ID，符號／冒號／正文／時間不變。明確匯出完整 PNG、原生情境選單複製，Cmd-V 預覽確認遮罩後取消，正常關閉。peer 拒絕所有站端輸入；報告須包含 PNG、context copy、Close acknowledged 及全部 owner 清理。原生像素／剪貼簿讀回另核對，不能只依報告 passed；跨頁快取與政策交互由 Rust 獨立 expected 覆蓋。


統一設定視窗變更另跑完整 JS、desktop lib／bin、Clippy、i18n 與 Python tooling 測試。Windows driver 核對中英文搜尋、實際偏好檔與畫面像素、無效中間值不寫入、分類恢復預設、匯入自動保存及重開；設定 driver 必須回傳主 target 本身，重複開啟不探索或建立輔助 CDP target。缺少 Windows runner 仍為 not_ready。macOS 原生另核對 ⌘,、品牌與 Lucide、連續修改自動保存、同一主視窗內的對話框關閉／重開、設定開啟時 ⌘Q、PNG 與 Close／全部 owner join，還要確認程序退出；只有 cleanup JSON 不能證明沒有隱藏視窗殘留。

`--probe-surface OUTPUT_DIRECTORY inline_pages --media-origin http://127.0.0.1:PORT` 使用兩頁自製閱讀畫面。同一 URL 在兩頁的 link index／行位置不同，第二頁分段重繪；peer 只接受一次 PageDown 及 Close，往回讀必須走本機快取，任何 PageUp／額外輸入均失敗，圖片 HTTP 請求不得重複。仍須兩次長按 Enter 一個 batch、兩頁可見圖片、歷史文字複製、目前站端 PNG 與完整清理。此案例不取代原本零站端 bytes 的 inline 探針。

短文章尺寸另用 `--probe-surface OUTPUT_DIRECTORY short_reading`：自製三行內容、80×24 來源格線與底列頁尾。以相同字型／視窗切換網頁與終端機模式，擷取原生畫面比較字格、空白區與頁尾位置，匯出 960×576 PNG 後正常關閉；peer 拒絕所有站端輸入，報告必須包含 PNG、Close acknowledged 與完整 owner 清理。這個 smoke 不代表原生滑鼠或滾輪已驗收。滾輪分流另由生產 JS handler 測試覆蓋列表 overflow、拒絕情境與網頁文章本機捲動。

即時語言變更另核對獨立 Localizer 往返、具名參數無隔離字元、invalid Localization 保留舊 bundles、目前搜尋／快取字型選項／草稿、macOS 原生選單及第一層設定入口；同一合成 session 切換前後 PNG 須逐像素相同，peer 拒絕全部非預期 bytes。返回區變更須分別驗日期／作者邊界、標頭不攔截、完整列光棒與只裝飾一欄的返回提示；新版最愛、完整看板與第 11 欄分類游標另由 Connection 核對列差＋CR 及過期拒絕。

短文展圖的捲動末端可用 `short_links` 搭配兩張 100×60 本機圖片驗證：展圖後內容仍在原 24 列內，Home／End 畫面及頁尾位置應相同，重複 End／PageDown 不得多送站端 bytes。較長圖片仍須可捲至完整末端；JS 另以誇大的 DOM scrollHeight 核對 renderer 高度邊界，不能以 DOM double 代替原生滾輪驗收。

長文 ANSI 接頁使用 `--probe-surface OUTPUT_DIRECTORY long_load --media-origin LOOPBACK_ORIGIN`：38 頁共 837 列，peer 只接受 37 筆精確 PageDown 及 Close；第 31 頁首列僅重設前景色。確認載入完成、End 看見第 837 列、Home 回首頁、兩次長按 Enter 一個 batch，圖片各一次 HTTP、來源 PNG 與完整 owner 清理。不能以只接受 20 頁的原 `full_load` 案例宣稱覆蓋這個邊界；原案例的取消／繼續驗收維持。

內嵌管理分類另核對同一設定對話框內的站點／黑白名單／帳號與隱私／授權：分類直接顯示表單與授權文字，不再經 settings_launch 交接；工具入口與搜尋共用同一 owner。離開帳號頁、開始搜尋或關閉設定須清除密碼草稿，搜尋連續輸入不失焦；保存、晚到回覆與正常關閉需保留原有持久化／取消規則。文章列表作者欄右鍵另以 Rust 獨立 expected 驗證完整欄位、標頭／標題拒絕、整行遮罩不洩露 ID、過期畫面及本機路由；原生座標右鍵失敗須記 not_ready，不能用表單保存代替右鍵驗收。

單張／等寬展圖另核對 Rust 單張索引只下載指定網址、同批次新增另一張、單張後 Paint 不自動擴張、全開升級重用資源、非法索引與終端機拒絕。JS 檢查 500ms held timer、repeat／keyup 去重、失焦／撤銷／換篇取消、可見中央目標、第二次等寬與捲動重設、解碼前的寬度意圖，以及滿行／後方有文字時箭頭不遮原文。`inline` 原生先用箭頭單開一張，核對 HTTP 只有指定圖片，短按 Enter 等寬後長按全開；另用新程序以明確 50ms 短按、方向鍵捲動及再短按核對 index 切換，500ms 以上才是長按。來源 PNG 仍不含控制項與展圖，正常 Close／全部 owner join，並核對 `inline_targets` 與下載計數。原生工具的預設按鍵持續時間不當作短按證明。

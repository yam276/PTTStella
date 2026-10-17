# 開發與驗證

目前產品為 [terminal 外觀的 PTT 介面](docs/active/minimal-browser.md)，並有按需的 [媒體預覽](docs/media.md)。核心、Dioxus 視窗及單一 WSS 路徑已實作；手動登入的實測狀態見 active plan。沒有 shell／PTY 或一般系統終端功能。

## 當前 gate

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

## 實作後的驗證

站端滑鼠第一批另跑 `cargo test --locked --lib mouse::tests`：同一 Screen／CSI parser 的每個分片、固定模式 trace／epoch、RIS／session reset、同步期間模式、取消與超長拒絕，以及純 encoder 的獨立 SGR bytes／修飾鍵／格線邊界。這批尚無送出 owner 或 UI pointer，只能宣稱解析與編碼，不以現有本機滑鼠 gate 代替新的站端互動驗收；接線及原生需求見 [站端滑鼠](docs/station-mouse.md)。

內建字型／桌面資產路由變更另跑 `cargo test --locked --features desktop --bin pttstella`，檢查固定路由的 MIME、原始字型標頭及未知路徑／方法拒絕；主畫面、預覽、保存及 PNG 仍須實際 WebView 驗證。新字型來源、SHA-256、著作權及授權需記錄於 assets/fonts。

2FA 引導另用 `--probe-surface OUTPUT_DIRECTORY login_2fa`，開啟 Vim 後按鈕登入合成帳密；原始輸入六位錯碼、八位救援碼，進入主選單後明確 Esc 回 Normal，再正常關窗。peer 精確核對一次帳密與兩次手動碼，額外輸入失敗；報告需包含提示／錯碼／瀏覽三個狀態、Close acknowledged 與 owner join。不代替實站 2FA 或原生 IME 驗收。

SGR 66 顯示變更另使用 `--probe-surface OUTPUT_DIRECTORY sgr66_legacy` 與 `--probe-surface OUTPUT_DIRECTORY sgr66` 啟動兩個獨立程序，沿用相同 probe-settings.json 與字型；在原生視窗點「匯出 PNG」後正常關閉。兩者使用等價自製串流、本機 peer 拒絕站端 bytes，各自留下 export.png、surface-probe.json 與 cleanup.json。需獨立解碼兩張 960×576 PNG 核對全部像素及已知左右背景／明暗反相，不能只檢查 PNG 標頭或兩份 Core 相等；CPU 核心另測每個分片、單寬字、控制碼取消、覆寫／捲動與 session reset。這不代表已驗證 UTF-8 或其他 OS。

Vim gate 由真正 WebView 編輯／按鍵事件送至 Rust policy，再由本機服務逐筆比對原生一次 h／Ctrl+d／中文、計數、半頁、gg/G、h/l 與模式返回；前綴取消不能夾帶原始字母。macOS 原生 window probe 另以 46 筆精確輸入覆蓋主選單／看板／最愛／文章、無更新時的列表移動，以及三次進板提示的 l／Enter／空白鍵繼續與後續六位數文章列表操作；最後六筆覆蓋命令說明頁的移動／首尾／返回及空列表返回，l／Enter 執行由負面案例拒絕。每次換頁加入 DEC2026、未知 ESC intermediate 與超長 CSI，畫面分兩段、間隔 120ms；runner 應觀察更新後再輸入，不能把本例宣稱為極速連發或逐像素防撕裂證據。同步持有舊圖、拒絕舊導覽、允許原始輸入／Ping、缺結束恢復及批次中關閉另由 Connection loopback 與核心測試核對。進板辨識、長按跨畫面、重複／過期 Continue 另由核心與 Connection 的負面案例核對。可攜顯示設定驗證兩份具名檔、固定 preset、載入預覽／取消／套用與錯誤保留，重啟程序另外比對保存結果和全部畫素。probe 只替代原生 picker 的路徑，沒有宣稱操作原生選檔視窗。

圖片 hover gate 以 trusted pointer 與 owned HTTP server 核對快掠零請求、同連結一次、取消 TCP EOF、已完成／下載中提升不重抓、跟隨與固定位置、右下碰邊及停用偏好；Canvas、焦點與站端輸入保持一致。GIF 小預覽檢查靜止首幀，組字開始取消；Rust lease 測試則覆蓋 frame revision 續用與新輸入／session／來源範圍失效。以上實機 backend 為 Windows WebView2；其他 OS、原生輸入法候選窗與實站仍需各自驗證。

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

OSC 8 變更另跑 `node --test tooling/test_surface_links.mjs`；它以事件／狀態 doubles 直接執行生產 handler，核對任一片段選取索引、組字／非連結拒絕及一次 released hit 的版本檢查，不冒充原生拖曳。`--probe-surface OUTPUT_DIRECTORY osc8 --media-origin http://127.0.0.1:PORT` 使用 owned loopback 與自製 HTTP 圖像。原生依序點第 9／10 列各一次、右鍵並複製各一次、跨兩列拖曳、開啟第 15／16 列圖片並分別關閉、匯出 PNG，再正常關窗。嚴格報告要求兩列開啟／複製、跨列 range、兩次圖片解碼及 PNG；缺輸入 backend 為 not_ready，PNG 或正常退出不能代替互動通過。

UI 准入與生命週期變更另跑 `cargo test --locked --features desktop --bin pttstella` 及 `node --test tooling/test_surface_lifetime.mjs`（Node 18+）。前者檢查單筆命令等待／取消、立即入列與 busy 不執行 factory、事件來源與 session、組字例外及超時保留清理；desktop lib 另用實際 WebSocket 比對 browse 在原始 Enter 之前入列，尚未 poll 回覆也不能改變順序。後者以 fake channel 驗證生產 JS stop owner 的正常接收、資料通道失敗、停止後不新增 receive，以及兩萬個封包不累積 unresolved stop Promise 的 reaction，沒有操作瀏覽器。

快速切板 UI 另跑 `node --test tooling/test_surface_navigation.mjs`，檢查本機快捷鍵、IME Enter／229、取消／晚到回覆、session 改變、草稿及 dispose。`--probe-surface OUTPUT_DIRECTORY boards` 由 owned peer 核對 Stella／Other 兩次完整流程，再等待 Beta 的返回並延遲 3 秒；應在同一次 UI 自動化呼叫內觀察最新 AX「停止導覽」並立即點擊，以免人與工具回合超過延遲。另需取消未確認草稿、輸入超長名稱確認完整保留並被拒絕，最後等晚到列表、正常關窗。嚴格結果須為兩個正式到達、至少一次拒絕、有效取消及無額外 bytes；若超過測試間隔而續送，保留失敗，不調整 expected 放行。

UI owner／封包變更再跑 `node --test tooling/test_surface_dialogs.mjs`。使用明確 DOM／clock doubles 核對晚到字型結果、關閉／revoke、舊預覽、登入草稿清除、名單 busy／失敗恢復及 router 拒絕未知／legacy envelope；這些不是原生 trusted 操作證據。[ui-packets](tests/fixtures/ui-packets.json) 是獨立編寫的 synthetic wire expected，Rust bin 比對生產 Packet 的 serde 結果，JS 測試核對分派，沒有用序列化往返當 expected。Paint 與內嵌預覽另檢查結構，完整呈現仍由原生 PNG／畫面驗收。

AID 核心另核對 article_navigation 的自製 article-open fixture 與 navigation_job typed progress。production Connection 的 article_tests 需精確比對切板、# 提示、八碼／CR、Q 資訊、空白返回及方向右；錯誤、權限提示、過期輸入、所有取消階段與晚到正確資訊不得續送。同步資訊框保持零空白且可 Pong，整體／每步期限不被碎片延長，重連零重播。BoardVisit 另證明導覽仍待輸入時已發布原 session 的到達事實，重連不得重貼舊 generation。既有 boards／window 原生結果只算共同 consumer 回歸，不能代替待接 AID 入口的原生驗收。

最近清單另核對 lib 的 recent_boards model／store 與 bin 的 Files writer／recent state 測試：正式到達、大小寫去重、20 筆上限、損壞／超量保留、備份、清除失敗、慢寫入合併、picker 期間保存與關閉 drain。`boards` 原生結果須保留 `[Other, Stella]` 並排除取消的 Beta；將該合成資料複製至新程序的 probe-recent-boards.json，用 `--probe-surface OUTPUT_DIRECTORY recent` 開啟切板視窗、確認順序、清除、等待完成後關閉，再以清除後檔案啟動第三個程序確認仍空。獨立讀取主檔與 .bak 均須為空；peer 拒絕所有站端 bytes。`boards_close` 另在第二次到達後開始 Beta，同一次 UI 呼叫確認「停止導覽」可見後立即 ⌘Q，檢查等待中的工作停止、Close acknowledged、各 owner join 與只保留兩個正式到達。兩個新 probe 不取用正式帳號／本機真實清單。

檔案工作／畫面狀態變更另核對 bin 的 Files 與 SurfaceState 案例：慢 blocking worker 不阻塞事件輪詢、忙碌不啟動第二筆、關閉時保留已開始工作且不吞失敗，以及單筆繪製／ACK、規則／來源／選取的失效。延期 picker 使用同一 production PNG 管線，八種狀態變更後舊 PNG 不寫，新請求仍須確實保存。JS 測試執行實際 clearSelection，以 [selection-cleared](tests/fixtures/selection-cleared.json) 核對直接撤銷通知；DOM doubles 不冒充原生操作。

`--probe-surface OUTPUT_DIRECTORY files` 使用合成主畫面與拒絕站端輸入的本機 peer，但 PNG／設定檔會走真正的原生 picker。確認原生 Save／Open、讀回 PNG／設定檔、取消及關閉清理；固定 probe 路徑不能替代 picker 驗收。`files_slow` 在 blocking worker 等待 runner 建立 OUTPUT_DIRECTORY/file-release（最多 120 秒），先寫 file-started.json，完成寫 file-finished.json。等待中可開啟／取消設定；第二筆保存應拒絕並恢復控制，⌘Q 後須保留 owner、顯示超時等待，release 後才完成清理及 Close。runner 必須收回自己啟動的程序與釋放訊號，不能留下背景工作。

`--probe-surface OUTPUT_DIRECTORY dialogs --media-origin http://127.0.0.1:PORT` 提供明確本機 UI／媒體 fixture，peer 拒絕輸入 bytes；操作設定預覽／取消／保存、具名設定檔匯出／匯入、整頁及行 PNG、本機圖片與外開 sink 後才檢查報告。媒體 server 必須由 runner 擁有並停止；不能將 sink 宣稱為 OS 瀏覽器實測。對話框拆分另以閱讀畫面逐一開／關名單、字型及登入，確認不用額外點擊或 frame 才恢復瀏覽控制項，再完成原 filters 回歸。

`--probe-surface OUTPUT_DIRECTORY CASE` 是不開 DevTools、不連實站的原生 WebView 探針；CASE 為 `decode`、`painted`、`send`、`renderer_error`、`untrusted`、`confirm` 或 `slow`。前四項分別造成真實事件解碼、版本、Eval 封包序列化及 JS 例外錯誤，應留下 `run_completed=false`、`cleanup_completed=true` 與 acknowledged 的 `cleanup.json`；錯誤畫面之後仍須能 ⌘W／⌘Q 結束。`untrusted` 驗證 JS 合成 paste／按鈕不改狀態，以及 Rust 拒絕三個不可信貼上 wire 事件；`confirm` 由原生貼上 `A\nB` 建立真實預覽，再拒絕合成確認且保留預覽。`slow` 持有 60 秒的合成結果 future，期間以原生按鈕開啟／取消字型設定並關窗，確認命令仍 pending 時就完成清理；它不宣稱模擬了真實網路 backpressure。三者檢查 `surface-probe.json`。所有案例的本機 peer 拒絕任何輸入 bytes，只接受 Close。負面合成事件不當成 trusted 正向輸入證據。

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

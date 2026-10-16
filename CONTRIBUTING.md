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

內建字型／桌面資產路由變更另跑 `cargo test --locked --features desktop --bin pttstella`，檢查固定路由的 MIME、原始字型標頭及未知路徑／方法拒絕；主畫面、預覽、保存及 PNG 仍須實際 WebView 驗證。新字型來源、SHA-256、著作權及授權需記錄於 assets/fonts。

Vim gate 由真正 WebView 編輯／按鍵事件送至 Rust policy，再由本機服務逐筆比對原生一次 h／Ctrl+d／中文、計數、半頁、gg/G、h/l 與模式返回；前綴取消不能夾帶原始字母。macOS 原生 window probe 另以 40 筆精確輸入覆蓋主選單／看板／最愛／文章、無更新時的列表移動，以及三次進板提示的 l／Enter／空白鍵繼續與後續六位數文章列表操作。進板辨識、長按跨畫面、重複／過期 Continue 另由核心與 Connection 的負面案例核對。可攜顯示設定驗證兩份具名檔、固定 preset、載入預覽／取消／套用與錯誤保留，重啟程序另外比對保存結果和全部畫素。probe 只替代原生 picker 的路徑，沒有宣稱操作原生選檔視窗。

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

UI 准入與生命週期變更另跑 `cargo test --locked --features desktop --bin pttstella` 及 `node --test tooling/test_surface_lifetime.mjs`（Node 18+）。前者檢查單筆命令等待／取消、立即入列與 busy 不執行 factory、事件來源與 session、組字例外及超時保留清理；desktop lib 另用實際 WebSocket 比對 browse 在原始 Enter 之前入列，尚未 poll 回覆也不能改變順序。後者以 fake channel 驗證生產 JS stop owner 的正常接收、資料通道失敗、停止後不新增 receive，以及兩萬個封包不累積 unresolved stop Promise 的 reaction，沒有操作瀏覽器。

`--probe-surface OUTPUT_DIRECTORY CASE` 是不開 DevTools、不連實站的原生 WebView 探針；CASE 為 `decode`、`painted`、`send`、`renderer_error`、`untrusted`、`confirm` 或 `slow`。前四項分別造成真實事件解碼、版本、Eval 封包序列化及 JS 例外錯誤，應留下 `run_completed=false`、`cleanup_completed=true` 與 acknowledged 的 `cleanup.json`；錯誤畫面之後仍須能 ⌘W／⌘Q 結束。`untrusted` 驗證 JS 合成 paste／按鈕不改狀態，以及 Rust 拒絕三個不可信貼上 wire 事件；`confirm` 由原生貼上 `A\nB` 建立真實預覽，再拒絕合成確認且保留預覽。`slow` 持有 60 秒的合成結果 future，期間以原生按鈕開啟／取消字型設定並關窗，確認命令仍 pending 時就完成清理；它不宣稱模擬了真實網路 backpressure。三者檢查 `surface-probe.json`。所有案例的本機 peer 拒絕任何輸入 bytes，只接受 Close。負面合成事件不當成 trusted 正向輸入證據。

一般黑名單變更須跑 core 的 filters suite、desktop lib store 與 bin presentation admission 案例。`--probe-filters OUTPUT_DIRECTORY` 提供自製推文、獨立名單檔與本機 peer，不開 DevTools port、不連 PTT。原生視窗驗證新增／同 ID 更新／移除、拒絕與保存失敗、遮黑與暫時顯示、系統剪貼簿讀回、整頁／選取行 PNG；獨立解碼確認變更只在命中 cell 範圍、選取行與整頁對應裁切完全相同。peer 僅接受 PageDown／PageUp 往返及 Close，管理名單不能送其他 PTT bytes。檢查 `filters-probe.json`，並另以新程序確認規則保存；不能只以畫面有黑色或 DOM 元素存在當作通過。

## 資產與提交

資產與依賴須有來源、授權與版本；合成 fixture 保存獨立 expected。提交前檢查實際 diff，維持獨立 checkout 可建置及驗證。

後續以功能或可獨立驗收的修正為單位建立 Conventional Commit，body 說明結果、設計與實際檢查。只有使用者明確要求才壓縮或改寫歷史。工作完成更新 active plan；目前不實作板名導覽、文章模型或其他擴充功能。公開 repository、PR、套件出版與桌面發佈各自是另外的動作。

作者與提交者使用 `yam276 <59956724+yam276@users.noreply.github.com>`；提交前檢查 repo-local Git identity。依使用者要求將初期開發歷史整理為一個提交，回復備份保存在 repo 外。

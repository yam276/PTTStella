# 媒體 MVP 規格

本階段在 terminal 外觀的 PTT 畫面上加入按需媒體預覽；格線、ANSI 顏色、文字選取及 PNG 匯出保持原有責任。唯一進度與驗證狀態維護於 [active plan](active/minimal-browser.md)。

## 入口及互動

- 點完整單列 HTTP／HTTPS 連結：直接 JPEG／PNG／GIF／WebP 開圖片預覽，MP4／WebM／OGV 開播放器；YouTube 的 watch／youtu.be／shorts 先開預覽卡。其他網址維持系統瀏覽器。右鍵選取與拖曳仍只選文字。
- 暗底 dialog 一次一個媒體，保留原網址、外開、關閉、錯誤與重試；不自動掃描／下載文章所有連結，不改變 BBS 排版。
- 完整預覽四邊各留 16 CSS px，使用其餘視窗空間，不再限制為 1100×800。圖片預設符合可用區域且不放大超過原尺寸，提供等比例放大／縮小、原尺寸／符合；大圖在預覽內捲動，不能把圓形或文字橫向拉長。
- GIF／WebP 保留原始格式，預設顯示靜止首幀，可播放／停止並回首幀；重新播放從頭開始，沒有任意幀暫停／逐幀功能。關閉後停止解碼。GIFV 與無直接來源的圖床頁維持外開，不靠刪除網址字尾猜來源。
- 直接影片用 WebView 的標準 controls，`preload=none`、不自動播放；可用格式依 OS／WebView codec。播放失敗或停滯顯示原因並可立即外開，不下載完整影片進 Rust 記憶體。
- YouTube 顯示原網址與背景取得的縮圖，外開按鈕從一開始就可用，不查標題／作者。這階段不內嵌第三方網站播放器。
- Esc 只關閉預覽，不送到 PTT。開啟期間暫停本機導覽／送鍵及閒置刷新；關閉恢復輸入焦點。關窗、session generation 改變時釋放預覽；一般刷新同一 session 不重載預覽。

## 載入與所有權

Rust 唯一分類 URL 與圖片型態；JS 消費結構化媒體描述並交 WebView 解碼，不重解 BBS 串流或拼接 HTML。來源文字以 textContent／DOM property 處理。

- 一個預覽 owner，一個可取消圖片 HTTP future；重試／切換／關閉即 drop 舊 future，request id 防止晚到結果覆蓋新預覽。沒有 detached decoder 子程序或跨圖片解碼鎖。
- 圖片 HTTP 總期限 20 秒、連線 8 秒、最多 5 次 redirect；HTTPS 不降級 HTTP，拒絕媒體 URL 的嵌入帳密。只在點擊、Enter 明確展圖或啟用的小預覽停留後發出請求，以 `PTTStella/版本` 識別下載程式，無 cookies、文章 referrer 或磁碟 cache。
- 單圖 payload 上限 8 MiB；確認 PNG／JPEG／GIF／WebP magic 及尺寸後才送 WebView，單邊最多 8192、總像素最多 16,777,216。GIF／WebP／PNG 動畫紀錄最多 256 幀，畫布像素乘幀數最多 67,108,864；超限提供外開。這是解碼工作量限制，不是實測 decoder 記憶體上限；動畫由 WebView 管理，關閉撤銷 Blob URL。
- 第一版圖片須完整接收並通過型態／尺寸檢查再交 decoder；明確顯示載入狀態，先保證可取消及輸入回應，未宣稱 progressive decode。
- WebView 確認解碼成功後才存入記憶體 LRU，最多 8 張／24 MiB，最長 5 分鐘，遵守 no-store／no-cache、max-age／Age（上限 5 分鐘）；以精確請求 URL（移除 fragment）為 key，保留 query。關閉 app 清空，沒有背景更新或私人 URL 日誌。
- 傳輸容量限制以串流累計檢查，不只相信 Content-Length；拒絕超限、損壞、HTML／SVG 回應。載入失敗留在預覽內，不能把媒體錯誤當成 BBS 斷線。

## 圖片 hover 小預覽

使用者要求小預覽可選「跟隨滑鼠」或「右上固定」。預設為右上固定，另提供關閉；位置與啟用偏好由 app settings 保存，既有字型組合與可攜檔的範圍見 [設定方案](preferences-and-profiles.md)。已接入同一個 Preview／Loader owner，完整預覽仍由點擊開啟。

小預覽請求由停留圖片連結 350ms 觸發；工具列設定清楚表示滑過會載入。仍不預載整篇文章，關閉 hover 後保留點擊與 Enter 明確展圖。

- 同一圖片網址停留 350ms 才載入；此值是初始 UX 選擇，需以實測調整。在同一連結內移動只改位置，不重設倒數或發第二個請求；快速掠過不連網。
- 跟隨位置與游標保留約 16 CSS px 距離，碰邊轉向並限制於內容 viewport；右上固定距內容 viewport 上緣及右緣各 12 CSS px，不以全視窗浮球容器的底緣定位。整張預覽卡（包含狀態、邊框及內距）上限取 560 CSS px 寬、viewport 寬 55%／高 60%，並保留邊界空間；圖片始終等比例、不放大超過原尺寸。
- 小預覽不攔 pointer、不取得焦點、不清除選取，也不切換 IME 或 Vim 模式；PTT 鍵盤持續可用。它只顯示圖片首幀；影片與 YouTube 保留點擊才載入／播放。
- 換連結、離開內容區、開始拖曳／組字／貼上、開 dialog、視窗失焦或送鍵，取消倒數及 hover request。切頁或舊 stamp 失效立即取消；任何晚到結果不能復活。失敗不自動重試或外開，點擊後才提供完整錯誤與重試。
- 仍使用單一 Preview／Loader／cache owner。點擊同一圖片時把 hover 升級為完整預覽，沿用下載中或已解碼的資源；open 對同一個 hover request 改變呈現狀態，保留 future 與已解碼資源。
- modal 才執行既有 prepare、showModal、聚焦及關閉後還焦點；hover 的開／關只處理浮層，沿用同一資源 dispose。浮層定位有一個 RAF／resize owner，dwell timer 由 surface 管理，destroy 全部取消。
- Rust 從當前 displayed links 分類可預覽種類，hover 請求帶 generation／revision／input epoch 與 link identity；不接受 JS 任意 URL 或由非媒體 hover 落入外開 fallback。名單遮罩區不發媒體請求，見 [內容過濾方案](content-filters.md)。

完整預覽在 session generation 變化時重置；hover 使用較短的 lease 有效期。新 frame 到來時，由同一來源重新核對指標命中、URL、範圍與內容；可重新授權仍有效的同一 hover，避免單純 keepalive／不相關刷新造成閃爍重抓，不能只看 URL 相同就放行舊 stamp。UI 不重解 footer、ANSI 或 Big5。

已沿用既有 8 MiB payload、尺寸／動畫預算、HTTP 總期限、decoder ACK、no-store 與 LRU；hover 請求同樣可被取消。一般 gate 仍只用本機 HTTP；真實 pointer 測快掠零請求、同連結一次、A→B 取消、切頁晚到、兩種位置與邊界、點擊升級不重抓，以及焦點／PTT bytes／canvas 不變。Windows 設定重啟已驗證，macOS／Linux 留待實機驗收；跨頁失效目前有 Rust lease 契約案例，沒有額外宣稱實站往返。

## 驗收

1. 核心：URL host／scheme／query／YouTube ID 正反例，不誤判相似網域，不改寫未知網址。
2. 生產 HTTP loader＋本機服務：圖片型態／尺寸、未知長度超限、redirect／錯誤／逾時、取消與 cache hit／失效／容量。
3. 真實 Windows Dioxus：trusted 點擊開圖、獨立預期圖像、等比例縮放、動圖播放／停止、影片首幀／播放狀態、錯誤／重試／外開 sink、關閉焦點與載入取消；原畫布像素及 PTT bytes 不受媒體操作影響。
4. 一般 gate 只連本機服務，不接觸 PTT、YouTube 或圖床。外開 adapter 在 probe 使用 sink；實際網站／OS 外開、macOS／Linux 分別留待實機驗證。未量測的效能不宣稱改善倍數。

## 閱讀頁 Enter 展圖

在已辨識的 PTT 文章閱讀頁，Enter 將目前畫面中的直接圖片網址展開於對應文字行下方；同一完整網址只展開一次，跨列連結放在最後一段之後。再次 Enter 保持展開，不收合、不重新下載，也不送站端按鍵。沒有圖片時保留原 Enter 行為；Vim 的原生一次、i 輸入、計數／前綴、IME 確認與站端滑鼠模式不攔截。列表、進板提示與其他站不套用此快捷鍵。

- 只按需處理目前已繪製、已套用名單規則的畫面，不抓取整篇文章或自動翻頁。同篇閱讀的 Page Up／Down 保留展圖狀態，新頁依目前已繪製的圖片網址自動定位；翻回前頁重用已解碼圖片。離開文章、送出非閱讀移動、切站、過濾規則改變或斷線時取消並釋放。展開期間暫停閒置刷新，避免刷新撤銷圖片。
- 每頁最多 8 個不同圖片網址，同篇文章最多保留 32 個；只在目前頁面取得新的網址，依序下載。整篇合計 payload 上限 24 MiB、解碼畫布合計 16,777,216 像素。單圖仍走同一 Loader 的格式、尺寸、動畫、期限與 HTTP 規則。圖片顯示首幀，不自動播放；影片與 YouTube 保留點擊預覽。
- 圖片符合文字區寬度，單張顯示高度最多 560 CSS px，不放大超過原尺寸；使用本機捲軸閱讀。原網址仍可點擊開完整預覽、縮放或重試。失敗顯示於該圖片位置，其他文字與圖片仍可操作；不自動重試。
- Rust 擁有請求及取消，JS 只將已驗證 payload 解碼成靜態 canvas，成功或失敗均釋放 Blob URL，解碼等待最長 10 秒。同篇換頁只卸下舊頁排版並依 Rust 的 URL 資源識別重掛；不以舊 link index 或舊行列處理新頁點擊。離開文章／關閉取消與等待工作完成，舊 batch 不得復活。inline 不建立第二份編碼圖片快取。
- 原始格線及 renderer 不變；展開時以相同 renderer 的行切片呈現文字，額外文字畫布總像素不超過原畫布的 16M 限制。圖片間隙沒有 terminal 格座標，選取反白只涵蓋原文字行；完整網址、純文字複製與 PNG 仍消費 Rust 的原始行列。PNG 保持 960×576／選取完整行，不含插入圖片。

回歸包含 Rust 的已繪製／過濾版本／輸入 epoch 准入、原生輸入例外、重複展開、去重與數量限制；JS 驗證插入空間的逆座標、原列高度及 Enter 路由。`tooling/inline_images_probe.html` 提供獨立的真實 DOM／decoder 檢查頁，是否執行以 active plan 為準。`--probe-surface OUTPUT inline --media-origin http://127.0.0.1:PORT` 使用自製閱讀頁，要求兩次 Enter 只開一個 batch 與 PNG，peer 拒絕全部站端 bytes；圖片顯示、HTTP 請求次數及清理另核對原生畫面及服務端紀錄，不以封包計數冒充視覺驗收。

`inline_pages` 原生探針另外要求 PageDown → PageUp 的精確 bytes，保留圖片解碼結果並在不同列重新定位。HTTP 計數、圖片仍可見的兩頁畫面、翻回結果、PNG、Close acknowledged 與所有 owner join 必須共同核對；不同文章或新 connection 不沿用該 batch。

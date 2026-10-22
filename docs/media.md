# 媒體 MVP 規格

本階段在 terminal 外觀的 PTT 畫面上加入按需媒體預覽；格線、ANSI 顏色、文字選取及 PNG 匯出保持原有責任。唯一進度與驗證狀態維護於 [active plan](active/minimal-browser.md)。

## 入口及互動

- 點完整單列 HTTP／HTTPS 連結：直接 JPEG／PNG／GIF／WebP 開圖片預覽，MP4／WebM／OGV 開播放器；YouTube 的 watch／youtu.be／shorts／live 與其他網頁直接開系統瀏覽器，不先顯示縮圖。右鍵選取與拖曳仍只選文字。
- 暗底 dialog 一次一個媒體，保留原網址、外開、關閉、錯誤與重試；不自動掃描／下載文章所有連結，不改變 BBS 排版。
- 完整預覽四邊各留 16 CSS px，使用其餘視窗空間，不再限制為 1100×800。圖片預設符合可用區域且不放大超過原尺寸，提供等比例放大／縮小、原尺寸／符合；大圖在預覽內捲動，不能把圓形或文字橫向拉長。
- GIF／WebP 保留原始格式，預設顯示靜止首幀，可播放／停止並回首幀；重新播放從頭開始，沒有任意幀暫停／逐幀功能。關閉後停止解碼。GIFV 與無直接來源的圖床頁維持外開，不靠刪除網址字尾猜來源。
- 直接影片用 WebView 的標準 controls，`preload=none`、不自動播放；可用格式依 OS／WebView codec。播放失敗或停滯顯示原因並可立即外開，不下載完整影片進 Rust 記憶體。
- YouTube 沿同一版本有效的 OpenLink 外開路徑，保留原網址、query／時間參數與 fragment；App 不下載 YouTube 縮圖或建立預覽 owner。這階段不內嵌第三方網站播放器。
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
- 小預覽不攔 pointer、不取得焦點、不清除選取，也不切換 IME 或 Vim 模式；PTT 鍵盤持續可用。它只顯示圖片首幀；直接影片保留點擊才載入／播放，YouTube 點擊後交由外部瀏覽器。
- 換連結、離開內容區、開始拖曳／組字／貼上、開 dialog、視窗失焦或送鍵，取消倒數及 hover request。切頁或舊 stamp 失效立即取消；任何晚到結果不能復活。失敗不自動重試或外開，點擊後才提供完整錯誤與重試。
- 仍使用單一 Preview／Loader／cache owner。點擊同一圖片時把 hover 升級為完整預覽，沿用下載中或已解碼的資源；open 對同一個 hover request 改變呈現狀態，保留 future 與已解碼資源。
- modal 才執行既有 prepare、showModal、聚焦及關閉後還焦點；hover 的開／關只處理浮層，沿用同一資源 dispose。浮層定位有一個 RAF／resize owner，dwell timer 由 surface 管理，destroy 全部取消。
- Rust 從當前 displayed links 分類可預覽種類，hover 請求帶 generation／revision／input epoch 與 link identity；不接受 JS 任意 URL 或由非媒體 hover 落入外開 fallback。名單遮罩區不發媒體請求，見 [內容過濾方案](content-filters.md)。

完整預覽在 session generation 變化時重置；hover 使用較短的 lease 有效期。新 frame 到來時，由同一來源重新核對指標命中、URL、範圍與內容；可重新授權仍有效的同一 hover，避免單純 keepalive／不相關刷新造成閃爍重抓，不能只看 URL 相同就放行舊 stamp。UI 不重解 footer、ANSI 或 Big5。

已沿用既有 8 MiB payload、尺寸／動畫預算、HTTP 總期限、decoder ACK、no-store 與 LRU；hover 請求同樣可被取消。一般 gate 仍只用本機 HTTP；真實 pointer 測快掠零請求、同連結一次、A→B 取消、切頁晚到、兩種位置與邊界、點擊升級不重抓，以及焦點／PTT bytes／canvas 不變。Windows 設定重啟已驗證，macOS／Linux 留待實機驗收；跨頁失效目前有 Rust lease 契約案例，沒有額外宣稱實站往返。

## 驗收

1. 核心：URL scheme／query 與媒體副檔名正反例；YouTube／其他網頁不分類為圖片或直接影片，不改寫外開網址。
2. 生產 HTTP loader＋本機服務：圖片型態／尺寸、未知長度超限、redirect／錯誤／逾時、取消與 cache hit／失效／容量。
3. 真實 Windows Dioxus：trusted 點擊開圖、獨立預期圖像、等比例縮放、動圖播放／停止、影片首幀／播放狀態、錯誤／重試／外開 sink、關閉焦點與載入取消；原畫布像素及 PTT bytes 不受媒體操作影響。
4. 一般 gate 只連本機服務，不接觸 PTT、YouTube 或圖床。外開 adapter 在 probe 使用 sink；實際網站／OS 外開、macOS／Linux 分別留待實機驗證。未量測的效能不宣稱改善倍數。

## 文章快取與 Enter 展圖

已辨識的 PTT 閱讀畫面使用 [文章快取](article-reading.md)：Rust 將已取得、已過濾的完整頁面按文章行號合併，本機捲動與站端翻頁分離。文章快取不寫磁碟，全文載入不會自動下載圖片。一般 hover 仍是暫時預覽；要保留圖片可按 Enter 展開。

正文及推文的圖片網址末端內顯示半透明方形向下箭頭，直接以 Rust 連結索引開啟單一圖片。方框靠齊網址右端且垂直置中，寬度不超出連結範圍；只疊在網址末端，不延伸到後方文字／時間欄，也不另插控制項空白行。相同完整網址沿用原本去重規則，只展開一次；相鄰跨列片段放在最後一段末端，較晚再次出現的同一網址不把已展開的第一張圖片搬走。控制項、圖片與新增間隙不進來源 PNG，也不取得 terminal cell 座標。

短按 Enter 在放開時，按文章行／欄順序由上到下，選擇可見範圍內尚未展開的第一張圖片。只有可見圖片全部開啟後，才依相同順序將尚未等寬且未失敗的圖片等比例放大至 BBS 文字區完整寬度，不再限制原尺寸或 560px 高。尚未收到開啟確認時重按仍請求相同索引，由 Rust batch 去重，不跳過圖片或預先放大。已開啟但未解碼的圖片可記錄等寬狀態，解碼後套用；失敗圖片不做無效放大。捲動後重新依可見範圍選擇，不保留第二次 Enter 目標。

長按 Enter 500ms 只觸發一次全開，放開及按鍵 repeat 不再額外開圖或傳送 PTT 按鍵。全開沿用原有批次及後續已取得內容的擴展；單張模式的後續 Paint 只更新已明確開啟的圖片，不順帶載入其他網址。全開也可由既有情境選單操作。沒有圖片時保留原 Enter 行為；Vim 的原生一次、i 輸入、計數／前綴、IME 確認與站端滑鼠模式不攔截。列表、進板提示及其他站不套用。

推文作者高亮只是同篇文章的本機背景投影更新，沿用原展圖 batch／resource keys；新 Paint 及 ACK 後的 InlineOpen 都保留圖片間隙與讀取錨點，已開／等寬／GIF／失敗狀態不重建。不降低舊輸出租約撤銷、切篇或永久名單更新的隔離要求。

- 每次投影最多新增 8 個不同圖片，同一閱讀作用域最多 32 個。全開後新取得的快取內容沿用展圖狀態，依序載入新增網址；單張請求只加入明確指定的網址；本文合計 payload 上限 24 MiB、解碼畫布合計 16,777,216 像素。單圖沿用 Loader 的格式、尺寸、動畫、期限及 HTTP 規則，GIF 依下節的可見範圍與共同動畫預算播放；其他動態格式仍顯示首幀；直接影片點擊開播放器，YouTube 點擊開系統瀏覽器。
- 一般展開圖片符合文字區寬度，最多 560 CSS px 高，不放大超過原尺寸；明確 Enter 等寬操作才放大且解除該顯示高度限制。載入成功後以文章文字行錨點補償捲動位置；失敗保留原位訊息，不自動重試。網址仍可開完整預覽、縮放或重試。
- 即時 frame／revision 改變不拆掉快取排版。Rust 的 batch／URL key 擁有下載與取消；JS 只解碼已驗證 payload，靜態解碼完成或失敗釋放 Blob URL；GIF 僅在本篇作用域保留原有 Blob，清理時撤銷，解碼等待最長 10 秒。不保留第二份編碼圖片快取，不以舊 link index 操作新投影。
- 文字以同一 renderer 逐行繪製，只為目前可見行與上下兩行建立畫布；不建立整篇長 Canvas。圖片間隙沒有 terminal 格座標，選取只涵蓋 Rust 投影的文字行。完整網址與複製仍消費已 ACK、版本有效的 Rust 格線與 links。
- 完整頁 PNG 仍是目前站端 24 行畫面（960×576），不含歷史快取／插圖／游標／反白。快取中的選取行可獨立輸出，一次最多 24 行、完整 960px 行寬；超過時停用並說明，不悄悄裁切。
- 離開文章、原始輸入、切站、名單變更或斷線會取消並釋放；晚到 batch 不得復活。關閉會等待解碼工作與其餘 owner 清理。

回歸包含重疊行去重、連續範圍、雙色字與歷史完整網址、來源版本／ACK／名單准入、原始輸入、數量及容量限制、局部捲動、邊界取頁、非進展停止與解碼錨點。`inline` 原生探針須核對單張下載隔離、短按／長按／等寬、共用一個 batch、PNG、零站端輸入及 Close／owner join；探針另以有界 target index 記錄區分單張與全開，不記網址。

`inline_pages` 使用本機兩頁及分段重繪，peer 只接受一次 PageDown，之後只准 Close；往回讀不得送 PageUp，HTTP 不得重抓。必須共同核對快取兩頁的可見文字／圖片、歷史內容複製、PNG、HTTP 計數、Close acknowledged 與全部 owner join。離線及原生結果分別記於 active plan，不以計數冒充視覺驗收。

文章內展圖僅供網頁文章模式；終端機模式只保留 hover 或右上預覽，不插入圖片佔位列。切換與保存規則見 [文章閱讀](article-reading.md)。

## GIF 播放與資源限制

完整圖片預覽的 GIF 自動播放，既有播放／停止按鈕仍可手動停止並回首幀；hover／右上小預覽維持首幀，升級完整預覽後才播放。其他動態圖片格式維持原本手動播放規則。

文章內 GIF 保留首幀 Canvas 及一個原生 img，共用已驗證的 Blob，不另抓網址或建第二份 base64 快取。只選可視範圍內最接近中央的兩張，隱藏分頁、失焦、開啟對話框或離開可視區時移除 img src 並顯示首幀；恢復時從首幀播放。使用者明確暫停完整預覽後，重新聚焦不會擅自播放。

兩種呈現共用同一 playback owner 實作；沒有逐格 Canvas／requestAnimationFrame 動畫迴圈。Rust 已驗證的尺寸 × 幀數作為成本，可播放集合合計最多 67,108,864 個幀像素；缺 metadata 時保守視為一整份預算。文章內至多兩張，完整預覽至多一張；完整預覽開啟時文章動畫暫停。原有單圖 8 MiB／256 幀／8192 邊長、本文 24 MiB payload／16M 首幀像素預算不變。此為准入上限，不宣稱等於 WebView 實際 RSS；實際解碼與快取由 WebView 管理。

換篇、換模式、名單／session 失效及退出會先停動畫，移除來源，再撤銷 Blob／中止未完成解碼與移除監聽器。來源及行 PNG 保留原本純 BBS 圖片，不包含動畫。

等寬圖片的比例可能產生小數 CSS 像素高度；超出原格線的文章高度只在整篇末端向外取整，不到一個 CSS 像素，避免 WebView 的整數捲動上限使完整行裁切隱藏最後一行。短文仍維持原格線，圖片尺寸與文字行不改變，不新增空白文字列。

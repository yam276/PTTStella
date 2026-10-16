# 媒體 MVP 規格

本階段在 terminal 外觀的 PTT 畫面上加入按需媒體預覽；格線、ANSI 顏色、文字選取及 PNG 匯出保持原有責任。唯一進度與驗證狀態維護於 [active plan](active/minimal-browser.md)。

## 入口及互動

- 點完整單列 HTTP／HTTPS 連結：直接 JPEG／PNG／GIF／WebP 開圖片預覽，MP4／WebM／OGV 開播放器；YouTube 的 watch／youtu.be／shorts 先開預覽卡。其他網址維持系統瀏覽器。右鍵選取與拖曳仍只選文字。
- 暗底 dialog 一次一個媒體，保留原網址、外開、關閉、錯誤與重試；不自動掃描／下載文章所有連結，不改變 BBS 排版。
- 圖片預設符合視窗，提供等比例放大／縮小、原尺寸／符合；大圖在預覽內捲動，不能把圓形或文字橫向拉長。
- GIF／WebP 保留原始格式，預設顯示靜止首幀，可播放／停止並回首幀；重新播放從頭開始，沒有任意幀暫停／逐幀功能。關閉後停止解碼。GIFV 與無直接來源的圖床頁維持外開，不靠刪除網址字尾猜來源。
- 直接影片用 WebView 的標準 controls，`preload=none`、不自動播放；可用格式依 OS／WebView codec。播放失敗或停滯顯示原因並可立即外開，不下載完整影片進 Rust 記憶體。
- YouTube 顯示原網址與背景取得的縮圖，外開按鈕從一開始就可用，不查標題／作者。這階段不內嵌第三方網站播放器。
- Esc 只關閉預覽，不送到 PTT。開啟期間暫停本機導覽／送鍵及閒置刷新；關閉恢復輸入焦點。關窗、session generation 改變時釋放預覽；一般刷新同一 session 不重載預覽。

## 載入與所有權

Rust 唯一分類 URL 與圖片型態；JS 消費結構化媒體描述並交 WebView 解碼，不重解 BBS 串流或拼接 HTML。來源文字以 textContent／DOM property 處理。

- 一個預覽 owner，一個可取消圖片 HTTP future；重試／切換／關閉即 drop 舊 future，request id 防止晚到結果覆蓋新預覽。沒有 detached decoder 子程序或跨圖片解碼鎖。
- 圖片 HTTP 總期限 20 秒、連線 8 秒、最多 5 次 redirect；HTTPS 不降級 HTTP，拒絕媒體 URL 的嵌入帳密。只在點擊後發出請求，無 cookies、文章 referrer 或磁碟 cache。
- 單圖 payload 上限 8 MiB；確認 PNG／JPEG／GIF／WebP magic 及尺寸後才送 WebView，單邊最多 8192、總像素最多 16,777,216。GIF／WebP／PNG 動畫紀錄最多 256 幀，畫布像素乘幀數最多 67,108,864；超限提供外開。這是解碼工作量限制，不是實測 decoder 記憶體上限；動畫由 WebView 管理，關閉撤銷 Blob URL。
- 第一版圖片須完整接收並通過型態／尺寸檢查再交 decoder；明確顯示載入狀態，先保證可取消及輸入回應，未宣稱 progressive decode。
- WebView 確認解碼成功後才存入記憶體 LRU，最多 8 張／24 MiB，最長 5 分鐘，遵守 no-store／no-cache、max-age／Age（上限 5 分鐘）；以精確請求 URL（移除 fragment）為 key，保留 query。關閉 app 清空，沒有背景更新或私人 URL 日誌。
- 傳輸容量限制以串流累計檢查，不只相信 Content-Length；拒絕超限、損壞、HTML／SVG 回應。載入失敗留在預覽內，不能把媒體錯誤當成 BBS 斷線。

## 驗收

1. 核心：URL host／scheme／query／YouTube ID 正反例，不誤判相似網域，不改寫未知網址。
2. 生產 HTTP loader＋本機服務：圖片型態／尺寸、未知長度超限、redirect／錯誤／逾時、取消與 cache hit／失效／容量。
3. 真實 Windows Dioxus：trusted 點擊開圖、獨立預期圖像、等比例縮放、動圖播放／停止、影片首幀／播放狀態、錯誤／重試／外開 sink、關閉焦點與載入取消；原畫布像素及 PTT bytes 不受媒體操作影響。
4. 一般 gate 只連本機服務，不接觸 PTT、YouTube 或圖床。外開 adapter 在 probe 使用 sink；實際網站／OS 外開、macOS／Linux 分別留待實機驗證。未量測的效能不宣稱改善倍數。

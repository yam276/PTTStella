# PTTStella

Rust／Dioxus 的 PTT 桌面介面，外觀像 terminal；不嵌入系統終端、shell 或 PTY。啟動即連上 PTT，手動登入，呈現固定格線、ANSI 色彩與雙色字。

Rust 核心與 renderer 實作固定格線、ANSI 色彩、雙色字與 ASCII Art，搭配公開映射依賴及合成驗收案例。

第一個實測目標為 Windows；macOS／Linux 保留為後續平台目標。畫面核心、離線 replay、Dioxus／WebView2 顯示及 WSS 登入前連線探針已通過；手動登入後的操作仍待使用者實測。

## 目前範圍

- 一個 terminal 畫面及連線／斷線狀態。
- 一種經驗證的加密傳輸，使用者自行輸入帳號密碼。
- 固定欄位、中文字寬、ANSI 顏色、游標、局部重繪與雙色字。
- 畫面維持比例並符合視窗，工具列浮在上緣、hover 才顯示；明暗色依站端指令。
- 工具列可開啟 Vim 瀏覽：hjkl、gg/G、計數、Ctrl+d/u/f/b/e/y；f 或「原生一次」只放行下一個完整按鍵／組字，i 持續原始輸入。未知畫面暫停 Vim，登入預設原始輸入；範圍與限制見 [Vim 瀏覽](docs/vim-browsing.md)。
- 必要鍵盤輸入；斷線或連線失敗時介面仍可操作。
- 滑鼠拖曳選取、雙擊整行、工具列整頁選取；Ctrl+C／Cmd+C 複製純文字，雙色中文字保留完整字形。選取保留空白，新畫面會清除舊選取；Esc 清除選取，沒有選取時 Ctrl+C 仍送到 PTT。
- 組字文字以無框樣式跟隨 PTT 游標，確認後送出文字，取消不送出；確認候選字的 Enter 不當作 PTT 的 Enter，接著再按 Enter 才送站端。已驗證 Windows WebView 組字路徑及模擬的 WebKit 事件順序，macOS 原生 IME 候選窗仍待實機驗收。
- Ctrl+V／Cmd+V 貼上單行，不附加 Enter；多行先預覽確認。無法編碼、超限或包含不支援控制碼的文字會整段拒絕。
- 完整單列 HTTP／HTTPS 網址可點擊；直接圖片／影片與 YouTube 開媒體預覽，其他網址開系統瀏覽器。右鍵選取網址後可用 Ctrl+C／Cmd+C 複製，拖曳選字不會開啟連結。
- 圖片連結停留 350ms 會顯示首幀小預覽，預設右上固定，可在工具列改為跟隨游標或關閉並保存。移開、組字、送鍵或切頁取消；不搶焦點，點擊沿用已下載或下載中的圖片開完整預覽。
- 圖片預覽提供等比例縮放、符合／原尺寸、重試及外開；GIF／WebP 動圖預設停在首幀，可播放／停止回首幀。關閉會取消下載並釋放瀏覽器資源；成功解碼的圖片才進入有界記憶體快取，不寫磁碟。
- MP4／WebM／OGV 直接網址提供手動播放器，格式支援依 OS／WebView；YouTube 提供背景縮圖與立即外開，不等待標題查詢。支援範圍、容量及驗收見 [媒體規格](docs/media.md)。
- 工具列可匯出整頁 960×576 PNG，保留雙色字與方塊，不包含工具列、選取或本機游標；不受目前縮放與捲動影響。
- 拖曳選取目前畫面的連續行後，工具列「選取行 PNG」可匯出涵蓋的完整行；雙擊文字可選單行。固定 960 像素寬、每行 24 像素高，保留空白行與 ANSI 原色，預設檔名包含起訖行號。尚未提供跨頁拼接或不連續多段合併。
- 字型視窗可選依平台／Windows／macOS／Linux／自訂，任一平台均保留明確選擇。具名 .pttstella.json 可另存多份；匯出目前生效的顯示設定，載入先預覽，套用後才保存，取消保留原設定。檔案只包含名稱、字型選擇與排版，不包含字型檔。
- 工具列「字型」可分別指定英數／中文字型、字高與基線，提供預設組合與雙色字預覽。按「套用並儲存」後才改主畫面；固定格線與 ANSI 色彩不變。
- 已辨識的文章列表可點編號開啟，hover 只加細底線；標題保留拖曳與雙擊選字。閱讀時上緣提供「返回／上頁／下頁」，普通滾輪翻頁、列表滾輪移動一項；放大時優先捲動本機畫面，Ctrl＋滾輪仍縮放。

滑鼠導覽的 Windows WebView／本機服務端往返已通過；僅支援保守辨識的列表及一般閱讀狀態列，未知畫面保留手動鍵盤。支援範圍及限制見 [導覽契約](docs/navigation.md)。先不做獨立文章清單 UI、板名導航按鈕、搜尋、書籤、發文介面、自動登入或記住密碼。

## 開發入口

- [工作規則](AGENTS.md)
- [驗證命令](CONTRIBUTING.md)
- [最簡 terminal 計畫](docs/active/minimal-browser.md)
- [文字與排版契約](docs/dual-color.md)
- [架構](docs/architecture.md)
- [開發 harness](docs/development-harness.md)
- [後續操作功能研究](docs/feature-research.md)
- [Vim 瀏覽模式](docs/vim-browsing.md)
- [字型組合與可攜設定檔](docs/preferences-and-profiles.md)
- [本機名單與內容過濾方案（尚未實作）](docs/content-filters.md)
- [媒體預覽規格](docs/media.md)

```text
python tooling/check.py docs
```

`python tooling/check.py core` 執行 fmt、Clippy、畫面／互動測試與 2 個 replay 案例。`desktop` 包含本機傳輸／TLS 設定測試，並啟動實際視窗驗證雙色字像素、16 色與強度切換、填滿本體、輸入、剪貼簿與清理；`all` 跑全部離線 gate，不連 PTT。依賴來源見 [依賴說明](docs/dependencies.md)。

啟動：`cargo run --locked --features desktop --bin pttstella`。預設直接開啟 `wss://ws.ptt.cc/bbs` 的 PTT 連線，手動輸入帳號密碼；方向鍵、Enter、Backspace、Tab、Esc、Home／End、PageUp／Down、Delete 與 Ctrl＋字母會送到 PTT（複製／貼上的本機快捷鍵另行處理）。滑鼠移至視窗內容上緣即可顯示連線／斷線及縮放工具，移開即隱藏；鍵盤焦點停在工具列時保持顯示。斷線或錯誤時工具列持續顯示，列出站端提供的關閉代碼與原因；沒有原因時明確標示。斷線後由你決定是否重連，不重送舊輸入，不保存帳密或完整流量。

「＋／－」、Ctrl＋加減鍵或 Ctrl＋滾輪以相同比例縮放；「符合視窗」或 Ctrl＋0 還原。放大後可用捲軸查看超出區域。固定 80×24 不重新排字；不同視窗長寬比會留黑邊。方塊／三角形按格線繪製，背景以整數像素共用邊界，避免 ANSI Art 接縫。

一般文字保留字型比例並置中於固定欄位，圓形等窄符號不橫向拉滿。字型設定可預覽符號、雙色字與框線。

防閒置預設開啟：在已辨識的文章列表／閱讀頁閒置 60 秒後刷新，不移動閱讀位置；登入、未知畫面、組字、貼上、選字或 dialog 期間暫停。上緣「防閒置：開／關」可暫停本次啟動的保活，重啟預設開啟。未收到刷新回應時會暫停並顯示原因，不持續重送或卡住手動導覽。這與 WebSocket Ping／Pong 分開，實站長時間效果尚待驗收，詳見 [保活契約](docs/keepalive.md)。

顯示偏好保存於 OS 使用者設定目錄（Windows 為 `%APPDATA%\PTTStella\config\settings.json`），覆寫前保存 `settings.json.bak`。本機設定使用 version 2，舊 v1 明確字型名稱遷移成自訂，明確套用後才寫入新版；載入失敗時使用預設並顯示原因，不自動覆寫原檔。字型名稱由系統解析，缺字可能使用替代字型；沒有宣稱已驗證每個字形的 fallback。

獨立實站檢查：`cargo run --locked --features desktop --bin pttstella -- --probe-live target/verification/live`。這會連至 `wss://ws.ptt.cc/bbs`，只確認登入前提示並斷線，不送帳密；不包含在任何一般 gate。

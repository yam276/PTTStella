# 最簡 terminal 架構

Rust／Dioxus。先做單一桌面視窗與一條傳輸；第一個實測平台為 Windows，核心保持無 GUI／網路即可執行。工作順序見 [active plan](active/minimal-browser.md)。

```text
connection framing → raw terminal bytes → Rust terminal state
                                               ↓
                                  glyph mapping／render frame
                                               ↓
                                     Dioxus terminal surface

manual keyboard input → session-owned send queue → connection
```

初期一個 Cargo package；終端狀態、encoding、render、transport 與 UI 依需要分模組，不提前建立空框架。desktop 依賴不污染 headless 核心測試。

| 擁有者 | 責任 |
| --- | --- |
| terminal | byte/cell、ANSI、cursor、捲動／覆寫、樣式及 revision |
| encoding | 明確來源的映射與反向輸入編碼；保留未映射／非法狀態 |
| input | 共用編碼後 bytes 上限；貼上換行、控制字元與完整預檢；預覽保留正規化文字及相同送出 bytes |
| text／navigation | 共用完整字形到 UTF-8／cell 的投影；保守辨識列表與閱讀畫面，產生 typed 瀏覽意圖及有限的送鍵計畫 |
| render | 完整字形、固定幾何、左右樣式及 frame ID，不重解 raw stream |
| transport／session | framing、收送、取消、斷線與 generation；不辨識文章／登入流程 |
| UI | 連線控制、terminal surface、必要焦點／按鍵、顯示及可見錯誤 |

優先驗證 Dioxus Desktop 與 Canvas 的薄繪製橋接；實際呈現路線以同一 [顯示契約](dual-color.md) 的 probe 決定。若採 JavaScript，只接收結構化 frame，不持有第二份終端狀態。

UI 正常掛載後只發起一次預設 PTT WSS 連線；合成 probe 只連本機 WebSocket。renderer 唯一管理符合視窗／等比例 zoom、hover 工具列及繪製排程；接收的 Screen frame 不因縮放改寫。hover 的 CSS class 由 renderer 管理，Rust 透過獨立 data attribute 要求斷線／錯誤狀態持續可見。尺寸變更合併為一個 animation callback，背景共用整數像素格線，圖形幾何與文字都使用同一格線與左右 clip。所有輸入／縮放／hover listener 由同一 bindings 清單註冊與移除。

不丟原始 bytes；UI 可合併最新 frame，帶 generation／revision 防止晚到更新。手動送鍵序列化，連線關閉時停止對應擁有者，重連是新 session。

目前單一路徑為 WSS → 有界 binary frame → Telnet framing → Screen。每則 WebSocket 訊息上限 64 KiB；不把文字 frame 猜成 Big5。輸入上限 4 KiB、佇列 64 項；無法編碼或佇列滿會顯示錯誤。連線、送出與關閉都有逾時，送出失敗不重試；連線 owner 管理並 join 握手／收送工作。UI 只允許一個尚未完成的繪製 frame，其餘保留最新狀態，不堆積 JS 繪製佇列。關閉視窗時先停止 listener 與連線工作，再關閉 host。

滑鼠導覽只建立目前格線畫面的辨識與命中，不建立獨立文章模型、板名導航按鈕、登入腳本或背景追蹤。文字、排版與雙色字由核心與 renderer 實作；操作及視覺契約見 [滑鼠導覽](navigation.md)。

`navigation` 唯一產生畫面種類、文章編號 hit region 與 typed intent 的按鍵計畫；UI 管理一筆 pointer 交易，網址優先，拖曳／取消／失焦／畫面或輸入版本變更即取消。UI 帶上 generation、revision、input epoch，Connection 在序列化送出處核對並從目前 Screen 重新規劃。鍵盤輸入會遞增 epoch；新終端 bytes 到達前暫停滑鼠導覽，避免用舊游標連續計算，但不阻擋手動送鍵。這是本機防重入機制，沒有把新 bytes 當成站端完成動作的保證。JS 不解析 BBS 文字，只消費 Rust 的結構化辨識與命中資料。

選取核心同時產生純文字、字形 spans 與完整行範圍；PNG 消費同一行範圍，不從 DOM 選取框反推。匯出先用同一 renderer 畫固定 960×576 的無游標快照，再按完整行裁切；獨立於顯示縮放／捲動，亦不包含本機提示層。Rust 保留選取 request 與行範圍，保存前核對畫面及選取識別、行數對應高度與共用容量限制。每次只允許一筆匯出，停止時等待編碼工作並移除按鈕 listener。

Connection 同時擁有防閒置的單調時鐘、預設開啟狀態、UI watch 許可及自動刷新等待。UI 僅回報本機操作是否允許，不建立自己的 timer；owner 核對當前 Screen、session／輸入版本及等待狀態後才送一次刷新。手動輸入與自動刷新以不同 PendingInput 變體管理，只有自動等待會因無回應而逾時釋放並停用保活。規則與公開站端來源推論見 [保活契約](keepalive.md)。

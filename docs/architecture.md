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
| render | 完整字形、固定幾何、左右樣式及 frame ID，不重解 raw stream |
| transport／session | framing、收送、取消、斷線與 generation；不辨識文章／登入流程 |
| UI | 連線控制、terminal surface、必要焦點／按鍵、顯示及可見錯誤 |

優先驗證 Dioxus Desktop 與 Canvas 的薄繪製橋接；實際呈現路線以同一 [顯示契約](dual-color.md) 的 probe 決定。若採 JavaScript，只接收結構化 frame，不持有第二份終端狀態。

UI 正常掛載後只發起一次預設 PTT WSS 連線；合成 probe 只連本機 WebSocket。renderer 唯一管理符合視窗／等比例 zoom、hover 工具列及繪製排程；接收的 Screen frame 不因縮放改寫。hover 的 CSS class 由 renderer 管理，Rust 透過獨立 data attribute 要求斷線／錯誤狀態持續可見。尺寸變更合併為一個 animation callback，背景共用整數像素格線，圖形幾何與文字都使用同一格線與左右 clip。所有輸入／縮放／hover listener 由同一 bindings 清單註冊與移除。

不丟原始 bytes；UI 可合併最新 frame，帶 generation／revision 防止晚到更新。手動送鍵序列化，連線關閉時停止對應擁有者，重連是新 session。

目前單一路徑為 WSS → 有界 binary frame → Telnet framing → Screen。每則 WebSocket 訊息上限 64 KiB；不把文字 frame 猜成 Big5。輸入上限 4 KiB、佇列 64 項；無法編碼或佇列滿會顯示錯誤。連線、送出與關閉都有逾時，送出失敗不重試；連線 owner 管理並 join 握手／收送工作。UI 只允許一個尚未完成的繪製 frame，其餘保留最新狀態，不堆積 JS 繪製佇列。關閉視窗時先停止 listener 與連線工作，再關閉 host。

沒有文章模型、畫面語意辨識、板名導航按鈕、登入腳本或背景追蹤。文字、排版與雙色字由核心與 renderer 實作。

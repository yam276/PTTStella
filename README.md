# PTTStella

Rust／Dioxus 的 PTT 桌面介面，外觀像 terminal；不嵌入系統終端、shell 或 PTY。啟動即連上 PTT，手動登入，呈現固定格線、ANSI 色彩與雙色字。

Rust 核心與 renderer 實作固定格線、ANSI 色彩、雙色字與 ASCII Art，搭配公開映射依賴及合成驗收案例。

第一個實測目標為 Windows；macOS／Linux 保留為後續平台目標。畫面核心、離線 replay、Dioxus／WebView2 顯示及 WSS 登入前連線探針已通過；手動登入後的操作仍待使用者實測。

## 目前範圍

- 一個 terminal 畫面及連線／斷線狀態。
- 一種經驗證的加密傳輸，使用者自行輸入帳號密碼。
- 固定欄位、中文字寬、ANSI 顏色、游標、局部重繪與雙色字。
- 畫面維持比例並符合視窗，工具列浮在上緣、hover 才顯示；明暗色依站端指令。
- 必要鍵盤輸入；斷線或連線失敗時介面仍可操作。
- 滑鼠拖曳選取、雙擊整行、工具列整頁選取；Ctrl+C／Cmd+C 複製純文字，雙色中文字保留完整字形。選取保留空白，新畫面會清除舊選取；Esc 清除選取，沒有選取時 Ctrl+C 仍送到 PTT。

先不做文章清單 UI、畫面語意辨識、板名導航按鈕、圖片、搜尋、書籤、發文介面、自動登入或記住密碼。

## 開發入口

- [工作規則](AGENTS.md)
- [驗證命令](CONTRIBUTING.md)
- [最簡 terminal 計畫](docs/active/minimal-browser.md)
- [文字與排版契約](docs/dual-color.md)
- [架構](docs/architecture.md)
- [開發 harness](docs/development-harness.md)
- [後續操作功能研究](docs/feature-research.md)

```text
python tooling/check.py docs
```

`python tooling/check.py core` 執行 fmt、Clippy、12 個核心測試與 2 個 replay 案例。`desktop` 包含 7 個本機傳輸／TLS 設定測試，並啟動實際視窗驗證雙色字像素、16 色與強度切換、填滿本體、輸入橋接與清理；`all` 跑全部離線 gate，不連 PTT。依賴來源見 [依賴說明](docs/dependencies.md)。

啟動：`cargo run --locked --features desktop --bin pttstella`。預設直接開啟 `wss://ws.ptt.cc/bbs` 的 PTT 連線，手動輸入帳號密碼；方向鍵、Enter、Backspace、Tab、Esc、Home／End、PageUp／Down、Delete 與 Ctrl＋字母會送到 PTT。滑鼠移至視窗內容上緣即可顯示連線／斷線及縮放工具，移開即隱藏；鍵盤焦點停在工具列時保持顯示。斷線或錯誤時工具列持續顯示，列出站端提供的關閉代碼與原因；沒有原因時明確標示。斷線後由你決定是否重連，不重送舊輸入，不保存帳密或完整流量。目前停用貼上。

「＋／－」、Ctrl＋加減鍵或 Ctrl＋滾輪以相同比例縮放；「符合視窗」或 Ctrl＋0 還原。放大後可用捲軸查看超出區域。固定 80×24 不重新排字；不同視窗長寬比會留黑邊。方塊／三角形按格線繪製，背景以整數像素共用邊界，避免 ANSI Art 接縫。

獨立實站檢查：`cargo run --locked --features desktop --bin pttstella -- --probe-live target/verification/live`。這會連至 `wss://ws.ptt.cc/bbs`，只確認登入前提示並斷線，不送帳密；不包含在任何一般 gate。

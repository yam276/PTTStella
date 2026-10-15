# 最簡 terminal 開發 harness

保留先前參考 Diolama 整理的核心原則：單一狀態擁有者、可重放輸入、獨立 expected、生命週期可追蹤，以及語意／實際畫面／實站結果分開。這裡不建立生成式 Agent、文章語意層或通用操作框架。

## 三層驗證

| 層級 | 檢查 | 不能代替 |
| --- | --- | --- |
| semantic replay | 合成 bytes／chunk 經同一 Rust 核心後的格線、cursor、樣式與配對 | 真實字型、GUI 或登入 |
| mounted desktop | Dioxus 視窗、bridge、文字排版、焦點、必要送鍵及 owner 清理 | 實站連線／登入或其他平台 |
| live login | 指定環境的連線 → 手動輸入帳密 → 看見站端登入後畫面 → 斷線 | 完整 PTT 功能或三平台相容 |

Fixture 使用自己生成或明確可散布的資料，帶 chunk、grid、來源／版本及獨立 expected。replay runner 呼叫生產核心，不另外實作測試用 terminal。

必要案例見 [顯示契約](dual-color.md)。另外檢查 framing、分割／合併、cancel、peer close、寫入失敗、重連 generation 及晚到 callback；一般 gate 使用本機測試 adapter，不登入 PTT。

輸入驗證必須經過真實 WebView 的滑鼠預設行為與編輯事件，確認 activeElement 為 keyboard，再由瀏覽器輸入到 Rust，經同一 Connection 送到本機 WebSocket，最後收到並繪製回覆。合成確認提示需實際輸入 `n` 與 Enter 才完成，不能只確認 Rust 收到事件。只派送合成 KeyboardEvent／InputEvent 會繞過焦點與原生編輯，不能證明使用者可打字。probe 使用隔離 profile 與 loopback CDP；正常應用不啟用此入口。

## 畫面與資源

等待字型與實際 frame 完成後再檢查；只有非空截圖或固定 sleep 不算排版正確。記錄 OS／backend、字型／fallback、DPI、grid／viewport 與縮放。重複掛載／關閉時查實際 listener、timer、task、channel、callback 持有者，不只清空計數。

輸入 bytes 不丟棄，UI 允許有界合併 frame。送鍵由同一 session 擁有者序列化；不重送帳密，不把 write 成功當成站端登入成功，不聲稱遠端操作可回滾。

## Gate 與驗證報告

當前 [gate](../CONTRIBUTING.md) 分 docs／core／desktop。docs 檢查產品文件、fixture／verification JSON、repo 內容與 gate 自身回歸；core 驗證同一 Rust reducer；desktop 跑本機 WebSocket 生命週期案例及真實 Windows 視窗探針。必要 suite 必須非空且沒有 ignored／filtered 案例。這些 gate 都不連 PTT；實站登入另外驗證。

每次產品驗證報告記錄 commit／dirty、fixture digest、實際命令、平台、覆蓋／未執行範圍及結果。產物可用 verification 目錄；私人執行診斷保持本機私有。

passed＝指定必要項確實通過；failed＝反例／工具失敗；not_ready＝必要 runner／工具／backend 尚缺；not_run＝未執行。空 trace、零案例、全部 skip 或部分成功不可讓整體變綠。

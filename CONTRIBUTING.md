# 開發與驗證

目前產品範圍只有 [terminal 外觀的 PTT 介面](docs/active/minimal-browser.md)。核心、Dioxus 視窗及單一 WSS 路徑已實作；手動登入的實測狀態見 active plan。沒有 shell／PTY 或一般系統終端功能。

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

core 執行 Rust 1.97.0 的 fmt、Clippy、12 個核心測試及 2 個獨立 expected replay；desktop 包含 7 個本機傳輸／TLS 設定測試，再於 Windows 開真正的 Dioxus／WebView2 視窗，檢查雙色字像素、獨立背景、16 色與強度切換、填滿本體、送鍵橋接、監聽器／resize observer 清理與 connection worker join，產物位於 target/verification/desktop。all 跑離線 gate，不連 PTT。退出碼 0＝指定 scope 全部通過、1＝失敗、2＝必要能力未就緒；其他 OS 的 desktop 尚為 not_ready。腳本輸出 JSON，附 commit／dirty、fixture digest 與平台；root 從腳本位置決定。

`cargo run --locked --features desktop --bin pttstella -- --probe-live target/verification/live` 是另外的實站動作，只接收登入前畫面與回覆協定協商，確認後即斷線；報告只存結果與接收數量，不存畫面、原始流量或帳密。一般 gate 不執行它。真正登入需在正常視窗人工輸入，看到站端登入後畫面才可宣稱登入通過。

## 實作後的驗證

桌面探針另驗證 hover 工具列、實際縮放按鈕、100／125／150% 畫面比例、連續方塊像素及半方塊／三角形幾何。`--probe` 明確停用正常啟動的自動連線，不輸入私人帳密或存取實站文章。

輸入 gate 使用獨立 WebView2 profile 與僅 probe 啟用的 loopback DevTools port，由 `tooling/desktop_input.py` 發送瀏覽器滑鼠點擊與文字／按鍵輸入，驗證畫面、黑邊、縮放控制後的焦點。ASCII／中文編碼、Enter 與方向鍵經同一生產 Connection 送往本機 WebSocket；合成確認提示顯示後，再以 `n`、Enter 完成往返，必須收到完成畫面並確認繪製。服務端接著關閉連線，檢查代碼／原因文字以及移開滑鼠後仍可見的狀態。只接受瀏覽器 trusted input；直接 dispatchEvent 不算通過。這不是實體鍵盤／IME 或實站登入證據。正常執行不開 debug port；runner 持有並等待自己啟動的程序，失敗時終止該 probe。

| 變更 | 證據 |
| --- | --- |
| 文件／規則／gate | docs、diff；gate 變更包含負面與搬移測試 |
| 核心／編碼 | 鎖定工具鏈、Cargo.lock、fmt／Clippy、核心測試及同一生產核心的非空 replay |
| renderer／輸入 | 核心加真實 Dioxus 視窗、顯示與送鍵測試；不能只編譯或擷取非空畫面 |
| 傳輸／登入 | 離線 framing／錯誤／取消測試，以及獨立、有界、手動實站登入流程 |

第一個 Rust 交付選定實際驗證的版本，啟用對應 gate 並讓 CI 使用同一入口；目前沒有宣稱 MSRV 或 CI 成功。文件修正不必啟動 GUI，產品完成不能只跑 docs。

## 資產與提交

資產與依賴須有來源、授權與版本；合成 fixture 保存獨立 expected。提交前檢查實際 diff，維持獨立 checkout 可建置及驗證。

後續以功能或可獨立驗收的修正為單位建立 Conventional Commit，body 說明結果、設計與實際檢查。只有使用者明確要求才壓縮或改寫歷史。工作完成更新 active plan；目前不實作板名導覽、文章模型或其他擴充功能。公開 repository、PR、套件出版與桌面發佈各自是另外的動作。

作者與提交者使用 `yam276 <59956724+yam276@users.noreply.github.com>`；提交前檢查 repo-local Git identity。依使用者要求將初期開發歷史整理為一個提交，回復備份保存在 repo 外。

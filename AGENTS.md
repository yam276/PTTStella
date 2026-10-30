# PTTStella 工作規則

## 目前目標

- 第一個交付是一個 Rust／Dioxus 的 PTT 專用圖形介面，外觀像 terminal：連線、手動登入 PTT、正確顯示、必要鍵盤及斷線。Windows 先驗證，架構維持可攜；其他平台後續各自實測。
- [操作功能研究](docs/feature-research.md) 的選取／複製、IME／貼上、網址操作、PNG 匯出與字型設定已逐項實作及提交；實測範圍及待人工驗收項目維護於同一 active plan。登入卡住的實站重試待使用者回報，不以合成測試結案。
- 先讀 [README](README.md)、[交付流程](CONTRIBUTING.md)、[唯一 active plan](docs/active/minimal-browser.md) 與相關程式。字型／欄位／雙色字要求見 [顯示契約](docs/dual-color.md)。
- 最小滑鼠導覽支援保守辨識列表／閱讀畫面、點文章整行開啟、左緣返回與翻頁。採低透明度藍金呼吸光棒，保留 ANSI 文字對比及原始格線；減少動態效果時停用動畫，視覺提示不進 PNG。研究資料維持在 repo 外，產品契約見 [滑鼠導覽](docs/navigation.md)。
- 截圖提供整頁與目前畫面連續行的 PNG，消費 Rust 選取結果的行範圍與同一 renderer；完整行寬、不因縮放改變尺寸，不包含選取、游標或工具列。只在使用者明確點擊時匯出，不自動跨頁或錄製 session。
- 防閒置依使用者要求預設開啟，僅在已辨識的列表／閱讀畫面、UI 與 connection 的版本及操作狀態均允許時刷新；登入／編輯／組字／貼上／選取等期間暫停。唯一 timer 由 connection 擁有，不補送睡眠或斷線期間操作；開關與驗證邊界見 [保活契約](docs/keepalive.md)。
- 媒體依 [MVP 規格](docs/media.md) 按需預覽，Rust 擁有 URL／下載／容量與快取，WebView 負責顯示及 codec；背景工作不得阻塞 BBS 或污染新預覽。先不實作獨立文章模型、板名導覽、啟動時自動登入或追蹤。使用者後續授權的按鈕登入與鑰匙圈保存依 [登入契約](docs/login.md)，只有明確選擇才保存、按登入才送出。一般 terminal 的手動鍵盤操作不等於整個 session 唯讀。
- 當前已有 Rust 畫面核心、replay、Dioxus 畫面及 WebSocket／Telnet 傳輸；依使用者要求擴充 [站點與編碼](docs/sites.md)，其他站先使用原始輸入，PTT 專用命令僅用於精確 PTT 目標。不是系統終端或通用模擬器，不實作 shell／PTY。驗證狀態以 active plan 為準，規劃、編譯、重放、實際畫面及實站登入結果分開報告。
- 僅診斷編譯與合成測試視窗置頂，啟動時不搶焦點；正常發佈版使用一般視窗。置頂不等於取得鍵盤焦點，不為測試放寬失焦／隱藏時暫停全文的保護；原生操作工具無法啟用視窗時如實記錄限制。
- 正常啟動連至使用者明確保存的預設站，初始為 PTT WSS；只自動連線，不自動登入或重送輸入。一般 gate 的合成 probe 只連本機服務。工具列由可拖曳浮球開啟並按分類扇形展開，避免上緣 hover 與系統列衝突；斷線／錯誤保持狀態提示可見。畫面縮放維持比例，方塊圖形不依賴字型留白。

## 依賴與測試資料

- 建置與一般測試應可從獨立 checkout 執行，只依賴本產品與公開套件。
- 資產、映射依賴與 fixture 記錄來源及授權；測試使用自製、可散布且有獨立 expected 的資料。
- 私人帳密與流量不進 repo；預設不錄製完整 session，診斷不記密碼；明確選擇的密碼只存 OS credential store。一般測試只用合成資料與本機服務，不連 PTT。

## 實作與所有權

- 初期一個 Cargo package，按實際責任分模組。先解決當前目標，不建通用插件、Agent 或多後端框架。
- Rust 終端核心唯一管理 byte／cell、ANSI、游標、樣式與 revision；控制碼先作用於格線，再衍生完整字形及左右樣式。UI／繪製橋接不得重解原始串流。
- 傳輸只處理連線、framing、收送與取消。session generation 隔離舊連線與晚到結果；一個擁有者序列化送鍵，重連不重送帳密或舊操作。
- 不丟輸入 bytes；可合併顯示 frame，舊 frame 不覆蓋新 session。完整字形與左右 clip／背景分離，不能以每字一種顏色代替雙色字。
- listener、timer、task、channel 與 callback 有清楚擁有者及停止條件。清理未完成要保留追蹤，不只清空計數。
- 輸入、編碼、傳輸及清理用明確 Result／enum 表達。保留未知／未映射狀態，不吞使用者在意的失敗；生產程式避免未證明的 unwrap／expect、unsafe 與可變全域。
- 輸入、channel、快取、診斷及重試須有界。限制應對應實際格式或資源；bytes 送出不可假裝回滾，write 成功也不等於站端登入成功。
- 畫面文字不進 dangerous_inner_html；若用 Canvas JavaScript 橋接，只傳結構化繪製資料，不拼接文章或帳密到程式碼。

## 變更與驗證

- 本專案不屬於 Diolama 系列，不套用 Diolama 的資源 harness；建置與測試直接使用本 repo 的 Cargo／tooling 流程，不占用或清除其他專案的共用 target。

- 本 repo 的產物清理採 task-owned 結案規則，見 [開發 harness](docs/development-harness.md#產物清理與結案)。每個工作結案前盤點自己建立的 App／編譯快取／fixture output，檢查 resolved path、symlink、內容與仍在使用的程序，移除已完成且被取代的產物；保留最新必要交付、source／assets、使用者資料與獨特失敗證據。不得把 `.local`、`dist` 整包刪除，或因 target 名稱像別的專案就推定歸屬。共用／活動／不明外部 target 保留並記原因；不為結案要求其他 Agent 釋放或等待。使用 Cargo 原生 clean 清理本 repo 已無使用者的編譯 profile，不繞過 native lock；只改清理文件時不重建 Rust。每次記 exact removed paths、保留例外與 filesystem free before／after；du、Cargo 宣告清除量和實際空間變化分開，不把其他工作並行釋放的空間全歸因於自己。
- 發佈授權：除非使用者明確要求發佈該版本，日常開發、修正、Critic、測試、提交與一般推送都只在 `develop` 執行，工作分支保持 `develop`；不自行切換或推進 `main`／`master`、建立其他工作／發佈分支、改成正式版號，或建立／合併 Release PR、Tag、GitHub Release。測試通過、功能完成及先前版本的發佈授權都不等於本次發佈授權；使用者另有明確分支指示時依其指示。
- 分支版號：`main`／`master` 的當前產品版號只准使用正式版號（如 `0.2.2`），不得使用 `-dev`、`-alpha`、`-beta`、`-rc` 等預發佈後綴或 `+build` metadata；只有 `develop` 使用 dev 版號（如 `0.3.0-dev.2`）。本 repo 的正式分支名稱為 `main`；若存在 `master`，適用相同規則，不因此另建或改名分支。Cargo.toml／Cargo.lock、README 當前版本與版本徽章須一致。
- 使用者明確要求發佈後，才在 `develop` 更新 Cargo.toml／Cargo.lock、README、版本徽章與該版 Release notes，統一為目標正式版號；完成驗證後，以標題 `release: v<package version>` 的 develop → main PR 推進，正式版號須高於當前 `main`。合併與發佈後，把 `main` 整合回 `develop`，再讓 `develop` 恢復下一個 dev 版號；不得把恢復 dev 版號的提交同步回 `main`／`master`。不要因修改 CI 直接提高版本或發佈產品，流程與實際分支保護狀態見 [自動發佈](docs/releases.md)。
- 把目標寫成可觀察的輸入、結果及驗收。可逆且已授權的工作持續完成；只為實質缺少資訊或授權詢問，不為例行選擇反覆確認。
- 保持方法與狀態責任清楚；第二份非平凡近似實作出現時找共同擁有者。修 bug 檢查產生邊界及直接消費者，移除被取代路徑，不混入無關重構。
- 執行 [對應 gate](CONTRIBUTING.md)。缺 runner／案例／backend 回報 not_ready，未執行不算 passed；空 trace、全 skip 或只改 expected 不能結案。
- 核心測試使用同一生產 reducer 與獨立 expected；顯示問題另用實際 Dioxus host；登入以實站人工流程證明。單平台結果不能冒充跨平台完成。
- 效能以同一工作量量測自己的基線；區分 cold／warm、wall time、CPU 與 process-family memory，不搬用其他專案的門檻。
- 後續以功能或可獨立驗收的修正為單位提交，完成驗證後使用 Conventional Commit 加短 body；不延續初期整批壓成單一 commit 的做法，只有使用者明確要求才壓縮或改寫歷史。沒有要求不建立 remote／PR 或公開發佈；不得代替使用者宣稱人工審閱完成。
- 本 repo 的 Git author 與 committer 固定使用 `yam276 <59956724+yam276@users.noreply.github.com>`，由 repo-local Git 設定保存；每次提交前核對身分。歷史整理遵從使用者明確要求，改寫前在 repo 外保存回復備份。
- 協作遠端以 `origin`（`https://github.com/yam276/PTTStella.git`）為準；提交前確認遠端狀態，完成使用者要求的 push 後核對遠端 commit。

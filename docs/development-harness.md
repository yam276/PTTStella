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

媒體測試由同一 desktop gate 啟動並清理本機 HTTP server，沒有外網依賴；PNG／GIF 來源及 CC0 聲明在 [fixture 說明](../tests/fixtures/media/README.md)。JPEG／WebP／WebM 由 probe WebView 的 Canvas／MediaRecorder 產生；真實呈現、codec 與生命週期需各自驗證，不能只看 URL 或 media element 存在。報告保存單次小圖的冷載入／記憶體 cache 及取消 wall time，包含 CDP 輸入／輪詢成本；不把它稱為實站速度或改善倍數，CPU／process-family peak memory 尚未量測。

passed＝指定必要項確實通過；failed＝反例／工具失敗；not_ready＝必要 runner／工具／backend 尚缺；not_run＝未執行。空 trace、零案例、全部 skip 或部分成功不可讓整體變綠。


## 產物清理與結案

PTTStella 沒有全域資源／產物清理 harness，也不套用 Diolama 的 allocator、預留、排隊或 all-owner release。以下是現有的局部清理入口；程序／listener 的正確退出不等於磁碟產物已清掉。

| 入口 | 正常完成／例外／中斷 | 刻意保留或未涵蓋 |
| --- | --- | --- |
| Rust `tempfile::tempdir()` 測試（例如 `tests/multi_instance.rs`、`src/ui/files_tests.rs`） | RAII scope drop 在正常返回、Result 錯誤及 unwinding 時刪除自己的暫存；異常終止／SIGKILL 不保證 drop | Cargo target、呼叫者指定的 probe 輸出不在 RAII 暫存內 |
| Python `tooling/test_*.py` 的 `TemporaryDirectory`／`addCleanup` | context exit／unittest tearDown 清理自己建立的目錄；正常例外及可展開的 KeyboardInterrupt 會執行，SIGKILL 不保證 | 少數舊測試／release helper 仍讀 `RUST_GUARD_ISOLATED_ROOT` 作可選暫存父路徑，僅是路徑，不啟動任何 guard／lease；未知外部父目錄不會整包清理 |
| `tooling/bundle_macos.py` 的 `.pttstella-bundle-*`、`bundle_windows.py` 的 `.pttstella-*`、`bundle_release.py` 的 `.release-*` | `TemporaryDirectory` 清掉尚未發布的 staging；`bundle_common.publish_files()` 在發布失敗時只移除本次建立的檔案；`bundle_common.build()` 例外會停止並 wait 自己的 Cargo child | 成功 App／Release 移到呼叫者指定的 output 後保留；編譯快取也保留，不會自動刪舊 App |
| `tooling/check.py run_desktop()`／`desktop_input.py`／`desktop_media.FixtureServer` | finally 關閉自己的 DevTools、HTTP server 並 join；自己的子程序逾時仍存活時 kill＋wait | 指定的 verification 目錄刻意保留 JSON、截圖與合成偏好；不是通用目錄回收器 |
| `src/ui/surface.rs` → `src/ui/lifecycle.rs Resources::finish()` | 正常及有處理的錯誤均 join renderer／connection／files／credentials，寫出 cleanup 結果；未完成或強制終止不宣稱成功 | `cleanup.json` 是程序資源驗收，不會移除指定 OUTPUT_DIRECTORY、App 包或 Cargo cache |

先前的缺口是：成功發布的測試 App、手動複製的驗收 App 及 Cargo profile 未經結案盤點，會跨工作累積。現在每個工作必須執行以下結案步驟；這是本 repo 的清理流程，不是背景自動刪除服務。

1. 在本次 `.local/verification/<task>/` 保留清理清單，逐筆記錄 exact path、owner／用途、resolved path、symlink 與內容、狀態及必要的 hash。列出 latest deliverable／source／assets／使用者資料／獨特失敗證據的保留項目；不從名稱或副檔名猜歸屬。
2. 檢查自己與 kernel 可見的程序／open files。仍有使用者、共用或歸屬不明的項目保留並說明；不停止別人的程序、不建立鎖或等待所有 owner。已驗收且被取代的測試 App 可刪，不能把相鄰的 screenshot、trace、失敗報告一併刪掉。
3. 已確認本 repo 的 `target` 沒有活動使用者且必要 App／報告已在 target 外保存時，才在 repo root 用下列原生命令清掉選定 profile；保留 native lock 行為。`--target-dir` 必須是盤點後的絕對路徑，不能直接複製範例或把未知環境變數／歷史 shared cache 當成已授權範圍。

```text
cargo clean --profile dev --target-dir <已核對的本 repo target 絕對路徑>
cargo clean --release --target-dir <已核對的本 repo target 絕對路徑>
```

4. 用 `df` 或 filesystem API 在操作前後記錄實際 available bytes；另外記 `du` 的 allocated size 與 Cargo 顯示的清除量。APFS clone／snapshot、hardlink 及其他工作可使三者不同，不把 logical size 當成實際回收量。確認最新交付仍存在且 hash 不變，記下 exact removed paths 與保留例外後才結案。

此流程不在每次 gate 後立刻刪快取，避免同一工作反覆編譯；結案清理也不為證明清理而重跑 Rust build。一般 docs、Python helper 單元測試可在清完後執行，不重建 target。若工作在硬中斷後恢復，只重新盤點自己的殘留，不啟動 legacy guard 或猜測清掉外部共享路徑。

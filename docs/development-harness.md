# 最簡 terminal 開發 harness

PTTStella 的驗證原則是單一狀態擁有者、可重放輸入、獨立 expected、生命週期可追蹤，以及語意／實際畫面／實站結果分開。這裡不建立生成式 Agent、文章語意層或通用操作框架。

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

PTTStella 使用本 repo 的 Cargo／tooling 流程與原生鎖，不使用全域資源預留或排隊機制。產物由建立它的工作負責盤點與清理。以下是現有的局部清理入口；程序／listener 的正確退出不等於磁碟產物已清掉。

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

使用者於2026-11-09明確要求清理已測完的 artifacts，包含過去累積與之後新增的產物。Slint 評估因此以「每個可獨立驗收的批次」為清理單位，不等整個 S0–S8 goal 結束：

- 同一批仍在編譯／驗證時可以重用自己的 profile；該批驗收完且必要交付已移出 target，確認無活動使用者後，就用上列 Cargo 原生 clean 清掉已用完的 profile。長期 goal 仍 active 不構成持續保留 debug／release 快取的理由。
- 使用者要求即時清理或批次中途交接時，也先盤點已停止使用的 profile；不能因驗收尚未完成就持續保留整包編譯快取。若後續確實需要同一 binary，只移出一份當前執行檔、必要授權與來源／hash／結果，明列待驗範圍，再清掉 profile；優先重用該執行檔，只有來源變更或缺少必要檢查才重建。中途清理不等於驗收通過，不提前刪掉尚未被完整取代的唯一交付。
- 只保留每個仍有必要的平台／執行配置的最新候選包，不為每個功能階段各保留一份 App。新候選涵蓋舊功能後，清掉已測舊包；尚缺平台驗收的唯一交付可以保留並說明。不得以測試包副本充當唯一來源備份。
- 歷次證據保留來源 commit／patch、binary hash、必要結果、最小重現資料與獨特失敗紀錄。舊 bundle 的 launcher／plist／授權／資源可以壓成小型紀錄；不再把完整可重建 executable／dylib 一起存進每階段的證據目錄。來源、原始 assets、使用者資料、已安裝工具與 registry cache 仍保護。
- 獨立 `--target-dir`、headless 驗收 driver 與版本對照用臨時 checkout 也屬於該批產物，不能因不在主 target 內而漏清。保存來源 ref／patch、driver hash 與結果後，確認沒有使用者，清掉已用完的 Cargo profile／driver；臨時 worktree 須確認沒有未保存修改，再用 `git worktree remove` 正常移除，不強制刪除。尚未整合的 Slint 來源 worktree 保留。
- 交叉編譯／打包的 task-owned 容器、容器內 target、暫存 sysroot／磁碟映像與解包副本也納入同批清理；先匯出最小結果／失敗證據，核對容器 ID、task label、mounts 與程序後，正常停止及移除自己的容器。只移除已無使用者的自有臨時 image，不執行全域 prune，不停止或刪除共用 VM／磁碟。若稀疏 VM 已刪檔但主機空間尚未回收，可在確認 discard 支援後執行線上 filesystem TRIM；分別記錄 guest free、主機 available 與 sparse disk allocated bytes，TRIM 回報範圍不等於實際回收量。已安裝的 SDK／工具與共用 registry source cache 另列保留理由。
- macOS 對照 App 的專用 `Library/WebKit/<task bundle id>`、`Library/Caches/<task bundle id>` 及系統暫存目錄也要列入盤點；probe 輸出目錄隔離不代表 WebKit cache 都在其中。只清 exact task namespace，先保存最小必要證據並核對 owner、resolved path、symlink、程序與 open files；共用 WebKit、產品資料和其他聊天 namespace 不刪。後續量測須記錄這些 cache 的 cold／warm 狀態。
- 後續 Slint goal 的一般 dev／test 建置明示 `CARGO_INCREMENTAL=0`、`CARGO_PROFILE_DEV_DEBUG=0`、`CARGO_PROFILE_TEST_DEBUG=0`，減少多代 incremental 與完整 debug symbols。這是該命令的 Cargo 標準選項，不改全機設定或 release profile；需要符號診斷時只在該批明示調整，完成後照樣清理。
- 原生 runtime 的 C／C++／Swift 編譯目錄、解壓來源副本與臨時 bootstrap 工具同樣按批清理。中途清理先保存已完成且後續可直接重用的 install prefix、原始來源 archive／SHA、recipe／patch 及必要 config／失敗紀錄，再移除無活動使用者的 objects／build trees。只有 Git checkout 的來源先連同必要 submodule 匯出並逐檔核對，不能只記 commit 就刪掉唯一修改。後續只重建缺少的元件；當批用完的臨時工具及已被候選包涵蓋的 install 副本也要清理，不將其當成永久 SDK。
- 每批記錄 exact removed paths、保留例外、du、Cargo 宣告量與 filesystem free 前後。共用／活動／不明項目保留原因；不啟動 daemon、定時刪除或 owner-release 流程，不跨案刪別人的產物。

結案清理不為證明清理而重跑 Rust build。一般 docs、Python helper 單元測試可在清完後執行，使用 `PYTHONDONTWRITEBYTECODE=1` 避免再次產生 pycache；不重建 target。若工作在硬中斷後恢復，只重新盤點自己的殘留，不啟動 legacy guard 或猜測清掉外部共享路徑。

# Slint／Dioxus macOS 效能量測入口

這份文件記錄 S0／S8 的量測工具及目前證據，不是效能通過報告。完整負載、次數與平台門檻仍以[平替計畫](slint-evaluation-plan.md#同負載效能門檻)為準。

## 2026-11-10 macOS 預設軟體合成與影片尺寸

本輪來源將 macOS 的無旗標路徑改用 Skia 軟體合成及既有原生 CPU 影片 worker。Windows／Linux 的 Auto 選擇保持原邏輯；`SLINT_BACKEND` 仍可覆寫，另可明確指定互斥的 `--software-renderer`、`--opengl-renderer`、macOS `--metal-renderer`。Metal UI 必須在 mount 前選定 CPU 影片：首輪三格式都停在 Ready／offer 尚未被接手，未發 HTTP；修正後三案通過，沒有放寬就緒或像素條件。預設切換僅發生在 Slint 候選，不切換正式 Dioxus 入口。

同時移除 CPU 影片既有720p上限，與 GPU 共用8192邊／16,777,216像素預算，超出才等比縮小。沿用單一 latest frame、有界通知及 worker 退出；依實際 view 配置，閒置不預配最大 buffer。新增獨立 expected 檢查 Full HD／4K 保持尺寸、超大正方形縮為4096×4096、越界拒絕，以及3840×2160四角 RGB／alpha255和完整 frame 長度。這是實際生產 buffer／轉換測試，尚非4K影片播放速度、4K顯示器或原生音訊驗收。

證據在 `.local/verification/slint-renderer-api-current/`。R4 raw為 `d1fd308f…`，使用既有50庫閉包的診斷 native為 `c8eeab19…`；確切hash、來源8f40e47加patch／351份fingerprint、各revision及命令分開保存。前一輪的同機／字型條件延用，M3 Ultra／256GiB／macOS15.7.7。無 renderer 旗標的完整16-owner視窗、viewport1040×740／grid1040×624，在原生 CUA terminal focus首尾可見下完成10秒／101筆穩定採樣：footprint峰值104,027,712 bytes（約99MiB）、CPU0.207124876秒，RSS峰值260,440,064 bytes。先前 OpenGL 842–845MiB與明確Metal R1校準約574MiB分列，不宣稱已達S8的兩版改善門檻或證明所有機型相同比例。

同一R4實際視窗以原生 `⌃⌘F` 進1920×1080全螢幕，再回1040×768（含標題列），terminal focus保留；兩張 screenshot 在CUA紀錄，沒有另存副本。返回後 family footprint74,061,888 bytes；vmmap主程序57.1M、圖形IOAccelerator resident16K。正常關窗exit0／唯一嚴格Close ACK，最後觀測的PID＋start全數消失。沒有用本輪推定實體IME或F11矩陣完成。

R4無旗標的MP4／WebM／OGV三案、837行／37次取頁長文、PNG／單GIF、文章輸入與貼上共七案皆exit0／嚴格Close，影片核對CPU render_path、四色、暫停／seek／失焦、render退休及session join；HTTP workers全部joined。固定960×576的source PNG與三API控制逐byte相同，仍為 `294df2bb…`。雙GIF峰值、100次操作／cold與warm重複、OS顯示延遲及Dioxus原生前景條件仍須補齊；單GIF案例沒有擴大標成雙GIF。

另以沒有Slint／Skia／媒體或視窗的自製CGL程式定位成本：context current約5.9MB，第一次清除1040×624 FBO後主程序峰值457,721,408 bytes，vmmap圖形392.2M；destroy context後約7.2MB。紅色pixel `[255,0,0,255]` 獨立核對、exit0，原始碼／trace／vmmap在 `slint-gl-allocation-audit/`，臨時exe已清理。此反例證明部分圖形成本不需要Slint快取才會出現，並未定位或宣稱修復driver洩漏。

本輪Rust native library390 passed／4 ignored、兩種Slint配置各97 passed／1 ignored，兩配置all-target release Clippy、fmt、Python177項通過。首輪Metal三案failed、fixture App因外部Frameworks symlink而deep-strict失敗及後續未啟動的runner錯誤均保留；standalone native strict通過，正式封裝仍要求完整閉包及原strict gate。R4最新平台包與清理結果在完成封裝後另記；此處不把診斷wrapper當可分發App，也不把測試通過當S0–S8完成。

## 2026-11-10 閒置格線校準與圖形記憶體

新證據在 `.local/verification/slint-perf-equivalence-current/`，機器為 Apple M3 Ultra／256GiB RAM／macOS15.7.7，不能外推其他機型。沿用已封裝的 arm64 Slint native `6841e17e…`，沒有修改或重建 Rust；以獨立 host、responsibility root 和自有 marked-store 執行。兩版設定 bytes 相同：內建 Cascadia Mono／Noto Sans CJK TC、字高23、基線20、字寬100、en-US、hover off，完整 owner 保留。CUA 實見相同格線內容與顯示尺寸；Slint prepared report 為 viewport1040×740、grid1040×624、scale1／zoom1及16個服務存在。Dioxus 的相同顯示幾何由 screenshot 目視，沒有冒充機器讀取的 viewport receipt。

每輪在畫面就緒後進行10秒／101筆穩定程序族採樣，之後正常關窗。下表是校準原始觀測，**不構成兩版效能勝負**：

| 程序／renderer | 取樣 footprint 峰值 bytes | 取樣 RSS 峰值 bytes | CPU seconds／約10秒 wall |
| --- | --- | --- | --- |
| Slint 預設 Skia OpenGL，首輪 | 886,086,528 | 220,872,704 | 0.041751084 |
| 同 native `--software-renderer` | 103,224,960 | 268,075,008 | 0.225581208 |
| Slint 預設 Skia OpenGL，新程序重測 | 883,185,344 | 221,724,672 | 0.073074500 |
| Dioxus 格線對照版，原生前景未確認 | 238,693,376 | 453,165,056 | 0.015280126 |

Slint 預設路徑兩次約842–845MiB footprint，software 這一輪約84–98MiB。取樣後對仍在運行的自有 PID 執行 `vmmap -summary`：GPU 主程序 physical footprint829.5M，其中 IOAccelerator (graphics) resident／dirty666.1M，另有64.0M IOAccelerator。這證明圖形配置是主要占用類別，尚未定位是哪個 texture／driver／cache 分配，不能宣稱是洩漏。software 的 CPU 與 RSS 這次較高，須補長文、雙GIF及影片同劇本的取捨，才決定預設 renderer；本輪沒有改產品預設。

原 Dioxus `5ea9ad6` 拒絕 `parity_grid`，該次 exit2，沒有視窗或採樣。真正完成採樣的是既有 `2846dc4` Dioxus 對照 native `948d2a9e…`，含共用核心抽取，不能標成初始基準。它的 titlebar 非作用中，AX 能操作 PNG 匯出及關窗，座標操作仍回報 noWindowsAvailable；修改 diagnostic CFBundleExecutable 為 native 並未解決。該次 codesign 因 unsigned RunFixture subcomponent 失敗，native bytes 未變；只以明確合成 argv 啟動，沒有宣稱新 host 通過簽名。Slint 三輪首尾 AX terminal focus 與作用中 titlebar 可見，仍未連續記錄整段 OS foreground。前景不等、次數不足、未控制 cold／warm，故 S8 比較維持 not_ready，不能用上述數字計算達標比例。

四段採樣皆通過原穩定 membership／counter 規則，family 含相同 responsible PID 的主程式、WebKit／MTL／ThemeWidget helper，沒有按程序名稱拉入別的聊天。正常退出後，最後觀測到的全數 PID＋start 身分已消失；Slint 三輪都有唯一嚴格 Close ACK，Dioxus PNG 匯出／surface／owner audit passed。原始 samples、run／sampling／acceptance、vmmap、硬體與來源 hash 保留。首次 Documents evidence 寫入 EPERM、錯誤 fixture 的語言／未知欄位導致 fallback、原基準不支援案例、前景未確認及簽名失敗，各自保留；後續成功不改寫它們。改用自有 `/private/tmp` 後可啟動，沒有變更 TCC 或系統權限。

本批兩份 host App、已用完的自有暫存資料根與五個專用 OS cache／WebKit／暫存目錄共110,762,516 regular-file bytes已清理；僅保留小型合成資料／影像／原始 trace，沒有保留環境副本。3,016份候選／來源／兩個必要原生對照 hash 不變；Frameworks symlink 不跟隨刪除。exact八個路徑、resolved／UID／symlink／lsof、du及filesystem free247333158912→247453286400 bytes在 `cleanup/ledger.json`，free僅為並行觀測；另4份 byte-identical generated PNG 的SHA與保留位置見 `cleanup/duplicate-pngs.json`。最新四包、必要來源、共享工具與使用者資料未變。接續優先處理 renderer 圖形占用與正式前景觀測，再補完整重複次數／負載／平台矩陣。

## 程序族與指標

[macos_process_usage.py](../tooling/macos_process_usage.py) 是開發用 Python 工具，沒有新增產品依賴。它讀取本機 SDK 的 `proc_bsdshortinfo`／`rusage_info_v0`，以 PID 加啟動 counter 識別程序；不能用舊 PID 直接延續採樣或發 signal。主要記憶體指標為 live family 的 physical footprint，另列 RSS；RSS 加總可能重複計入共用頁面。這不是 GPU 專用記憶體或系統可用記憶體的差額。

診斷 launcher 使用 macOS 的 responsibility spawn SPI，讓自己的 benchmark child 成為獨立 responsible root。程序族包含相同 responsible PID 的程序及遞迴 PPID 子程序。只有父子 PID 會漏掉 launchd 啟動的 WebKit XPC；反之，名稱相同、啟動時間相近或同一使用者都不足以建立歸屬。若目標沿用 Codex 的共用 responsibility，工具拒絕採樣，不把其他聊天的瀏覽器算給 PTTStella。這個 SPI 僅用於診斷，不變更 OS 設定或授權；介面不存在時為 `not_ready`。實際 spawn 呼叫對照 [Chromium launch_mac.cc](https://github.com/chromium/chromium/blob/main/base/process/launch_mac.cc)，CPU 介面背景見 [Apple XNU recount](https://github.com/apple-oss-distributions/xnu/blob/main/doc/observability/recount.md)。

本機 CPU 原始 counter 必須先經 `mach_timebase_info` 換算。本次 arm64 校準為 numer=125、denom=3，80ms CPU 工作與獨立的 Python process CPU clock 核對；直接假設原始值為 ns 的初次失敗保留，沒有用錯誤單位產生結論。每個 sample 保留原始 ticks、換算 ns、timebase 及讀取耗時；其他 OS／架構須重新校準。

## 穩定區間採樣

在同一診斷 launcher 已取得目標 `Identity(pid, start)`，並獨立證明工作負載、畫面及焦點條件後執行：

```text
python3 tooling/macos_process_usage.py --pid <PID> --start <START_COUNTER> --duration 10 --interval 0.1 --output <NEW_DIRECTORY>
```

輸出 `samples.jsonl` 與 `phase.json`。目錄必須不存在，trace 權限為0600，工具不啟動、啟用或停止目標。duration 0.1–60秒、interval 0.05–1秒，最多1200個間隔；在完整指定區間的首尾均讀取 counter。CLI exit0只代表這段採樣完成，exit2及 `not_ready` 表示不能計算完整區間。部分 trace 保留，沒有用它冒充完整結果。

`summarize_steady_phase` 只在每筆 sample 的 PID／start 集合完全相同、時鐘遞增且每個 CPU counter 未倒退時計算 CPU delta、平均使用核心數及**取樣到的**記憶體峰值。任何程序加入、退出、PID 重用、讀取失敗或 counter reset 都拒絕穩定區間彙總，不能把消失程序的 CPU 當成改善。它仍可能漏掉兩次採樣間完整出生及退出的 helper，也不捕捉瞬間記憶體峰值；啟動／反覆切換的完整 CPU、GPU、掉幀及顯示延遲需要額外觀測，不能由本工具證明。

單元與 native 校準入口：

```text
PYTHONDONTWRITEBYTECODE=1 python3 -m unittest discover -s tooling -p test_macos_process_usage.py
```

macOS 原生案例驗證獨立 root、fork child、同名 decoy 排除、CPU clock 單位，以及 CLI 採樣後目標仍運行並正常退出。其他平台跳過 native 案例，不算該平台驗收。負向案例核對共用 responsibility、PID 重用／群組 PID、程序消失、時鐘／counter 反例、有界採樣及保留 partial trace。

## 已取得的校準證據

證據在 `.local/verification/slint-perf-macos/`：

- `dioxus-counterpart.json`：同來源樹 `2846dc4` 的 release Dioxus 對照 App，獨立 bundle ID，launcher 僅允許顯式 surface probe。它不是最初 `5ea9ad6` 的基準；初始基準及其證據另保留。Slint 採用已保留的 `slint-capture-ack/package-current` 候選。
- `accounting-calibration.json`、兩份 `*-family.jsonl`：實際 Dioxus 原生視窗共444筆採樣，觀察到主程序、GPU／WebContent／Networking及操作工具後的 AppKit helper；四個 helper 的 PPID 為1，responsibility 仍指向同一主程序。Slint 自動格線擷取有5筆、當時只觀察到主程序，不能外推永遠沒有 helper。
- `dioxus-calibration/`：實際浮球／PNG 操作、960×576輸出、probe passed及Close／owner cleanup；`dioxus-family-after-close.json` 確認五個觀測身分均消失。`slint-calibration/` 保存960×576格線與1000×690 toolkit擷取，Close ACK由原始log核對。這些不是相同 OS 顯示延遲觀測點。
- `accounting-smoke-source.py`／JSON 凍結上述兩次 smoke 當時工具；後加的 PID／coverage 保護與穩定區間 CLI 由最終單元及原生測試另驗，不把新功能倒填進舊證據。

兩次工作時間、操作、視窗及焦點條件不同，不能據此比較 CPU 或記憶體。舊資料中 Dioxus與Slint的字型／視窗及5次對1次啟動也不符合共同條件；不拿來計算勝負或改善比例。

## 接續正式比較的必要條件

2026-11-10 接續修正：正常Live及完整服務benchmark現在使用同一個產品shell，隱藏常駐的原型標題、媒體開關、站點selector、輸出、Vim／滑鼠及footer列；沒有關閉服務。這個無文章控制列的終端viewport已到1040×740、格線1040×624。閱讀中的載入／更新／推文等情境控制仍保留，其他工作負載要各自核對實際viewport。縮放、擷取、連線與設定沿用浮球；補上收藏分類的草稿入口及設定分類的Slint授權面板。有效站端scope的草稿新入口仍待完整操作矩陣，不能只由掛接宣稱已驗。

連線狀態另有來自Connection phase的attention欄位，不由翻譯字串推測；正常連線訊息不常駐遮住格線，斷線／錯誤／警告浮層保持可見。操作通知可明確關閉，純連線狀態不顯示無效關閉鈕；同一連線訊息的後續frame不再覆蓋本機操作結果。浮層不參與終端版面，長訊息可捲動，沒有改PNG、原始格線或輸入准入。完整成功淡出、通知焦點、原生IME及全功能操作矩陣仍在S3／S7範圍內待驗。

新增 `--verify-shell NEW_DIRECTORY`，以同一16項owner及嚴格peer驗證完整viewport、授權面板／Escape／焦點、浮球縮放選擇、拒絕未選行的實際匯出錯誤、通知及明確斷線；使用toolkit回呼／synthetic foreground，不能當OS輸入。最終完整候選native SHA-256 `c9413d395939e76dfd76cc31d16cc7b8559232ee7d0e9addb4a75c7c1301c0d3` 的CPU／GL兩案、兩種浮球及18影片＋9閱讀＋2混合共33案通過。固定960×576的source PNG仍逐byte等於上一輪，SHA-256 `294df2bbe92bff771a2fc36480935f3c10904547f9c89850032e6cfcf49e4e37`。

macOS arm64最終封裝另以CUA實見浮球→設定→Slint授權、Escape後AX焦點回到終端、明確斷線後持續狀態且無無效X，以及⌘Q／exit0／Close ACK；來源為 `.local/verification/slint-shell-chrome-final/native-accepted.json`。前一份native hash `7d9c9ba1…` 的實際125%／100%、面板捲動及第24行有證據，但監督程序在240秒逾時終止該實例，exit -15，未列為正常退出。原生觀察因此促成移除純連線狀態的無效X；舊負向保留。最終退出的OS IMK mach-port訊息仍保留，未用這次結果宣稱IME正常或異常已修。

本批release Rust373＋83＋10通過、6個既有明確ignored，兩配置全target Clippy、Python126及fmt通過。actionlint未安裝、workflow來源未修改，該工具檢查為not_ready；其他架構／乾淨機器／對應來源與完整效能仍未由此包證明。最終App／ZIP／DMG位於同批 `package-current/`；先前包與本批編譯／暫存依逐批清理，來源與唯一反例保留。

2026-11-10 已新增 `pttstella-slint --probe-benchmark NEW_DIRECTORY`。舊 `--probe-grid` 在 UI 外持有 loopback Connection，視窗只接靜態 Frame，不能與完整 Dioxus host 比資源。新入口用與 Live 共用的完整服務判斷，掛載文章、輸入、輸出、媒體／設定、草稿、站點、名單、收藏、前往、登入、保活、滑鼠／選單及 connection watch；16項實際 owner／mount 檢查完成後才輸出 `prepared.json`、固定960×576的 `source.png` 與 toolkit `snapshot.png`。唯一20ms準備 timer 在完成或5秒失敗時停止；成功不自動輸入、合成前景或關窗。正式資料路徑、vault 及正常啟動行為不變；新入口的所有 store 在指定新目錄，憑證仍走 synthetic scope，peer 拒絕任何站端輸入。

`.local/verification/slint-benchmark-session/accepted-result.json` 記錄最終 release native hash。macOS arm64 在禁止讀 Homebrew、禁止讀寫產品／研究版一般資料、對外連線僅允許 localhost 的 sandbox 裡，16項服務及格線／幾何已安裝。CUA 實見雙色「中」、16色與第24行，⌘Q後程序exit0、嚴格peer Close ACK及既有各owner finish完成。Raise與標題點擊沒有證明前景啟用，工具仍disabled；不宣稱可輸入或完整焦點驗收。先前兩個被SIGTERM結束的實例及sandbox／staging失敗各自保留，不能合併成正常退出通過。未最佳化及strip後未最佳化執行檔都超出打包器128MiB上限，未放寬限制；改用release後成功。

這次同樣1040×740外視窗內的 Slint viewport 是1016×442、格線736×442；原型常駐工具列占用了畫面，外視窗相同並不等於共同工作量。因此先前及本次閒置採樣都不產生與Dioxus的改善比例。下一步應依產品浮球工具列契約移除正常視窗的原型常駐列，保留全部操作入口與持續可見的斷線／錯誤提示，再驗證兩個host的實際viewport及前景。不能為跑分關掉服務或只在benchmark隱藏產品仍有的工作。

本批Rust為native-player-probe的lib373、bin83、多程序10項通過，另有6個既有明確ignored；全target Clippy通過。release打包closure的50個dylib與上一個候選的原始hash相同；加上主程式manifest共51筆，並非新增了另一個runtime依賴。診斷App的CFBundleExecutable先指向script，CUA無法綁定；改成與實際native主程式一致並重簽後可綁定，產品packager原本即採native。這是診斷staging修正，沒有改產品識別。

先讓兩個完整 host 的顯式 probe 接受相同 viewport／DPI／字型、工作負載和開始／結束觀測點，核對輸入 bytes、畫面及 Close。區分 native OS 前景與合成 toolkit foreground，不能為跑分放寬生產失焦保護。首個可見且可輸入 frame、輸入到顯示器呈現、下載／取頁及 bitmap 計算分開記錄。

cold／warm 要記錄定義；macOS WebKit 會在 `Library/WebKit/<task bundle id>` 和 `Library/Caches/<task bundle id>` 留下資料，僅換 probe 目錄不足以稱 cold。只能清自己 namespace，不能清系統 cache 或其他 App 資料。兩版順序交錯，各至少10次啟動及每組100次操作；閒置、長文、雙GIF、影片、草稿、多開、反覆切換／回收後各自觀測。穩定區間工具可重用，membership churn 的階段另補完整 lifetime 記帳。

本批 release profile、系統暫存與專用 WebKit cache 已依[清理規則](development-harness.md#產物清理與結案)清理，exact五筆路徑及保留理由在同批 `cleanup/ledger.json`；不為工具測試重建 Rust。下一批沿用各一份 Slint候選與Dioxus對照程式，只有來源變更或必要 gate 才重建。低階 Windows門檻、四架構、完整回退及S0–S8功能驗收仍未完成，S9未執行。

# Slint／Dioxus macOS 效能量測入口

這份文件記錄 S0／S8 的量測工具及目前證據，不是效能通過報告。完整負載、次數與平台門檻仍以[平替計畫](slint-evaluation-plan.md#同負載效能門檻)為準。

## 2026-11-11 雙 GIF 三組交錯比較

在 Mac15,14、macOS 15.7.7 arm64 上，以當時 Slint arm64 封裝 native `f50ffb46`（乾淨 `7466c31`）對照既有 Dioxus native `4a3035b6`（實際 `3c667eb` 加 R3 patch，產品來源核對 `f48e7e4`）。已核對共用閱讀場景與素材產生器在這兩個來源間相同。六輪採 Slint／Dioxus、Dioxus／Slint、Slint／Dioxus 順序，使用相同設定、23 行文章和兩個獨立的 loopback GIF URL。兩圖都是同一份 512×160、32 幀、100ms 幀延遲的合成 GIF。每輪用新程序，重用兩個測試 App 身分。

實際介面展開兩張全尺寸圖片，外窗擷取為 1040×768。採樣前後核對文章輸入焦點和紅藍畫面。Dioxus 的兩張 GIF 在背景可展開，進出全螢幕再點文章空白處後，恢復原窗尺寸和播放。Slint 用 Raise 及文章點擊，首輪另捲回文章頂端。Slint 文章 viewport 實際回報 1040×676，原彙整誤用了歷史 1040×740。Dioxus 圖片置中，Slint 圖片靠左，兩版頁尾與內部版面有差異。首段 Slint 0.287 秒缺少結束時的文章焦點證據，保留原始 trace。重新取得焦點後採用獨立的 `playing-focused` 區間，並觀察到兩張圖都切換紅藍畫面。

| 配對 | Slint CPU 秒 | Dioxus CPU 秒 | Slint footprint MiB | Dioxus footprint MiB |
| --- | ---: | ---: | ---: | ---: |
| 1 | 0.296 | 0.180 | 127.4 | 700.4 |
| 2 | 0.287 | 0.279 | 119.8 | 674.5 |
| 3 | 0.388 | 0.171 | 125.9 | 686.7 |
| 中位數 | 0.296 | 0.180 | 125.9 | 686.7 |

每段 10 秒、101 筆，程序族成員穩定。相對這次 Dioxus 對照，Slint 取樣 footprint 峰值中位數低 81.7%，RSS 峰值中位數低 42.1%。CPU 中位數高 64.6%，三組配對差距為 64.6%、2.9%、127.5%，幅度有波動。六輪都各取得兩次 GIF GET、正常 exit0 和嚴格 Close，HTTP owner 均 join。Dioxus 另核對兩個 inline 請求／一個 batch、完整 23 行及 960×576 的原始頁 PNG。22 個觀測到的 PID／start 身分均已消失。

第 5 輪在計時採樣之外執行五秒程序堆疊取樣。Skia render 的 131 筆包含 95 筆 softbuffer present、87 筆 `CGContextDrawImage`，其下可見 vImage 像素轉換。實際使用的 softbuffer 0.4.8 macOS backend 在 `present_with_damage` 直接提交整個 surface。後續效能工作可針對這段提交成本，再核對色彩、縮放、兩圖播放及閒置資源。

原始資料、固定設定、執行檔 SHA、堆疊與可重跑控制器在 `.local/verification/slint-dual-gif-comparison/`。將 `compare.py` 複製到同層新目錄後執行 `setup`，逐輪執行 `run N`，以實際 App 操作展開及取得焦點，再從另一個終端執行 `sample N playing`。採樣後核對畫面並 Quit。Dioxus Quit 前匯出原始頁 PNG。兩個 controller 以 exit／Close、HTTP 次數與原始採樣結果保存驗收。清理移除六個自有根路徑共 110,849,788 regular-file bytes，七份 trace 無損壓縮後省 1,471,893 bytes。filesystem free 240004988928→240117846016 bytes 為觀測值。精確路徑、lsof、du、雜湊與壓縮映射見 `cleanup.json`。

這組比較採相同外窗、可見雙 GIF 與端點焦點。連續前景、顯示延遲、冷暖啟動、100 次操作、GPU 記憶體及正式 S8 門檻仍待驗。

## 2026-11-11 雙 GIF 在設定開關前後的資源

重用同一 macOS arm64 機器、Slint `f50ffb46…`、Dioxus `4a3035b6…`、設定及兩個獨立 URL 的 512×160 GIF。按 Slint／Dioxus／Dioxus／Slint 順序，用四個新程序。每輪都在 1040×768 外窗展開兩張圖並取得文章輸入焦點，以原生 App 選單開設定的「一般」頁，點 Close，再取得文章焦點。各階段量 10 秒／101 筆，兩端確認畫面與焦點。設定在 Slint 是獨立視窗，在 Dioxus 是同一 WebView 的 modal；開設定本身也佔資源。

| 輪次 | 程序 | CPU 秒，播放／設定／恢復 | 程序族 footprint 峰值 MiB，播放／設定／恢復 |
| --- | --- | --- | --- |
| 1 | Slint | 0.268／0.041／0.292 | 104.2／124.6／119.4 |
| 2 | Dioxus | 0.200／0.097／0.182 | 683.5／678.6／677.7 |
| 3 | Dioxus | 0.589／0.126／0.671 | 684.6／686.7／691.0 |
| 4 | Slint | 0.570／0.131／0.911 | 120.0／133.7／138.4 |

開設定後，相對各自開啟前，Slint 兩輪 CPU 分別少 84.6%／77.1%，Dioxus 少 51.7%／78.7%。四輪恢復後均看到兩圖紅藍換幀。每輪兩個 URL 各 GET 一次、正常 exit 0、嚴格 Close、HTTP join，所有觀測到的 PID／start 身分均消失。Dioxus 兩輪亦各有整頁 PNG 和既有 owner 清理收據。

可見播放的 CPU 差距沒有固定方向。第二輪 Dioxus 相對第一輪多 0.389 秒，其中 WebKit GPU 多 0.137、WebContent 多 0.150、主程序多 0.103 秒。Slint 的 CPU 波動集中在主程序，第二輪關閉設定後的 footprint 峰值仍高於開啟前 18.4 MiB。這些差異待以固定暖機時間、連續前景觀測和多次開關追查，單次開關不足以判斷記憶體是否累積。

最初以最小化為情境，但桌面工具仍能取得視窗畫面，缺少可靠的最小化觀測，未為該狀態取樣。Slint 第二張圖初次 AX 點擊未穩定展開，改用視窗中的可見箭頭並在開始前確認兩圖。Dioxus 兩輪以原生全螢幕往返後恢復文章座標操作，第二輪另遇截圖／座標工具錯誤，確認外窗與焦點才採樣。此批未量最小化、隱藏圖像的逐幀狀態、decoder 退休、顯示延遲或連續前景，仍屬 S8 的局部證據。

原始紀錄、重跑控制器、每個程序的 CPU delta 和取樣資料在 `.local/verification/slint-gif-minimized/`。複製 `compare.py` 到同層新證據目錄，執行 `setup`，逐輪 `run N`，依上述原生操作在各狀態執行 `sample N playing`、`sample N settings`、`sample N restored`。Dioxus Quit 前匯出整頁 PNG。清理移除六個自有根路徑共 110,624,900 regular-file bytes，12 份 trace 無損壓縮省 2,572,895 bytes。精確路徑、保留雜湊及 filesystem free 觀測見 `cleanup.json`。本輪沿用現有執行檔與固定 Cargo target。

## 2026-11-11 設定重複開關與按需繪製預覽

同一台 macOS arm64、相同 1040×768 文章外窗及兩張 512×160 GIF。兩個現有 Slint 候選程序 `f50ffb46…` 各做 12 次原生「一般」設定開關，然後在同一暫存 App 換上修正後 `a243d3bf…` 的新程序重跑。每次確實進入 Settings 再回文章，採樣點確認雙圖及輸入焦點。每點取樣 5 秒／51 筆，下表使用末 31 筆的程序族中位數。原始峰值、RSS、CPU 與程序身分也保留。

| 版本 | 記憶體指標 | 播放基準 | 首次開啟 | 第 1 次關閉 | 第 4 次 | 第 8 次 | 第 12 次 |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: |
| 原候選 1 | footprint MiB | 92.3 | 110.3 | 114.3 | 114.6 | 116.2 | 117.9 |
| 原候選 2 | footprint MiB | 90.5 | 109.3 | 111.8 | 114.7 | 117.8 | 120.6 |
| 修正後 | footprint MiB | 98.0 | 98.1 | 99.5 | 103.6 | 110.0 | 106.9 |
| 原候選 1 | RSS MiB | 333.1 | 369.2 | 388.0 | 406.5 | 408.4 | 425.7 |
| 原候選 2 | RSS MiB | 344.1 | 374.9 | 386.3 | 411.2 | 419.6 | 427.7 |
| 修正後 | RSS MiB | 348.5 | 361.0 | 372.8 | 385.2 | 391.8 | 399.5 |

原候選的主要 footprint 跳升發生在首次開關。從第 1 到第 12 次，兩輪 footprint 再增 3.6／8.8 MiB，RSS 增 37.7／41.4 MiB，RSS 變動主要出在主程序。程式原來會在一般頁及搜尋時準備字型預覽。現在只有設定已開、外觀頁可見才繪製，關閉會釋放 preview renderer 和圖片，保留設定視窗本身供重用。修正版第 12 次的 footprint 低於兩輪原候選 11.0／13.7 MiB，但 RSS 仍隨開關增加。三輪依序執行且暖機基線有差異，現有結果不足以歸因全部差異或確認長期記憶體回收。

首版用 Slint 的 property change callback 觸發預覽，設定與釋放案通過，但搜尋前往外觀的 Zoom Fit 焦點失敗，原候選同案通過。改成由分類切換、搜尋清空和結果導覽在排定焦點前明確更新。最終設定／搜尋／關閉待存三個完整探針都通過，搜尋包含兩種語言共 48 個定位。設定探針另核對隱藏預覽零配置、外觀預覽尺寸 576、關閉後無 renderer 或圖片、重開後重新繪製。實際視窗也在 12 次取樣後額外驗證外觀頁預覽與關閉重開，看到雙色字、字形與圖形，回文章後雙 GIF 仍切換紅藍。三輪均正常 exit 0、嚴格 Close、HTTP join，兩張圖各只下載一次。[Slint callback 文件](https://docs.slint.dev/latest/docs/slint/reference/language/callbacks/)

原始證據在 `.local/verification/slint-settings-retention/`。用 `compare.py` 在同層新目錄 setup，執行 `run 1`、`run 2`，每輪照順序 `sample N baseline`、`open-01`、`closed-01`、`closed-04`、`closed-08`、`closed-12`。用固定 Cargo target release build，`stage.py` 在同一診斷 App 暫存改動後的 binary，再執行第 3 輪。首次焦點失敗、來源補丁、最終三個探針、Rust bin 106 passed／1 ignored 及 Clippy 各有紀錄。暫存 App 與專用系統 cache 已清理 68,892,153 regular-file bytes，18 份 trace 無損壓縮及 8 份相同 PNG 去重另有清單。兩個 macOS 候選包已納入此修正並通過各 23 項封裝後回歸。上述量測使用原先記錄的執行檔，舊 Apple Silicon 對照以 `arm64-f50ffb46-native.gz` 保留於 `slint-renderer-api-current/cross-current/settings-package-current/`。[候選同步紀錄](slint-evaluation-plan.md#2026-11-11-設定預覽修正的-macos-候選同步)

## 2026-11-11 快照損耗待重畫狀態的修補

在同一App重跑保留的 `4a4da212…` 失敗native，確認尚未展圖時頁尾、下方控制列已缺失，之後GIF換幀仍未補回；探針保存的 `snapshot.png` 卻完整。原 `ad01a6d5…` 對照則正常。重現前另發現上一輪把write_bytes產生的0644備份copy2回App，導致診斷副本無執行權限；本輪修回0755、核對access及簽章，實際原版啟動／退出也通過。這是診斷副本還原錯誤，四平台包未被修改；不能只核對binary hash而忽略mode。原始啟動失敗log另存，不算原生渲染案。

根因已由反例驗證：在同一renderer上更新footer位置、於視窗repaint前呼叫 `take_snapshot()`，原發布版Skia使full／retained畫面不一致。Slint快照重用item trackers及dirty history，已把更新當成畫過，但獨立window buffer尚是舊畫面。先前118組測試沒有這個呼叫順序，因此不能重現。新增此順序後原依賴確實失敗；修補快照後標記完整window dirty並request redraw後，一般Slint／native-player兩配置的118組像素比較再次通過，後續100次動畫更新仍要求真正partial damage。這次沒有刪除快照或把準備／原生驗收降級。

專案以Cargo path patch保存發布的Skia1.18.1來源及完整原授權；25份原始檔與發布crate逐檔核對，僅修改 `lib.rs`、`software_surface.rs`，新增 `software_shadow.rs`。前者維護snapshot／window正確性；後兩者只在macOS software保留一張私有BGRA前幀，成功後複製到CoreGraphics獨立target，尺寸／stride／錯誤失效、縮小釋放原配置。不修改shared registry，不改圖片大小／幀數／delay、不共用可變CGImage記憶體。來源SHA／VCS、改動及退役方式在 `vendor/i-slint-renderer-skia/PTTSTELLA-UPSTREAM.json`、`PTTSTELLA-PATCH.md`。這取代下節未採用的shadow-only實驗，舊拒用／失敗紀錄仍保留。

最終native raw `7a78fcba…`／診斷 `43a2b2d9…`（aff5ac8 dirty、native-player-probe）以原launcher關閉診斷trace後驗證：CUA首次文章顯示已含頁尾／控制列，兩張512×160動畫、1920×1080全螢幕及1040×768還原、原生Settings開關及恢復紅藍相位通過；全螢幕往返後由實際Raise／focus恢復操作，沒有合成foreground許可。嚴格peer Close唯一一次passed，觀測PID＋start消失。HTTP兩URL各一次且正常join；第一個失敗版加原版對照另用同一server，各URL合計2次，皆Close通過。LaunchServices orphan仍無數值exit code。影像是原生CUA觀察，toolkit snapshot不冒充OS證據。

同一最終native兩段各10秒／101筆CPU為1.103324417／1.099903833秒，physical footprint峰值104,666,944／107,583,296 bytes、RSS355,024,896／357,941,248 bytes。期間無CUA操作且沒有trace開銷；只算單輪校準，尚無完整交錯cold／warm、100次原生操作、OS顯示延遲或連續foreground，不以相較舊1.32–1.36秒計算正式改善比例。core392 passed／4 ignored、shadow2案、兩配置BIN各103／1、兩配置像素案、含vendor的native／一般all-target release Clippy及Python178案通過。額外實際notice collector已確認本地Skia未被跳過、三份上游license及兩份來源／修補說明逐byte一致；已修正所有無registry source套件被略過的錯誤，只排除本產品root，local path記相對位置。所有新建置沿用固定Cargo cache。紀錄位於 `.local/verification/slint-dual-gif-current/render-audit/native-repeat/`；四平台下載與S8完整判定尚未更新，S9未執行。

依賴解析四target×兩配置8案、fmt／diff及docs21項回歸也通過；actionlint不在PATH，復用canonical checkout既有1.7.12工具通過，沒有重新安裝。GitHub Actions verify尚未針對未公開的本機變更執行。關閉／join全部本輪程序後，移除已被反例與修補驗證取代的 `render-audit/shadow/failure-native.gz`（34,055,533 bytes）及重複的 `native-repeat/fixed/rust-notices-check`（5,600,472 bytes）；先保留578筆notice inventory、5份notice的hash、失敗來源archive／logs與唯一原版ad01控制gzip。兩處resolved／UID／無symlink／lsof均核對，40份來源及交付保護檔hash不變。free264477679616→264520744960 bytes為並行觀測，logical／du分列於 `fixed/cleanup.json`。永久vendor來源、固定Cargo中間檔、當前診斷pair、四平台包及共享工具／SDK／使用者資料保留，沒有Cargo clean。下輪須重新配置loopback與create_new輸出，不能直接重啟過期診斷launcher；下一步更新真正候選包及來源交付，再做完整S8對照。

## 2026-11-11 整窗重畫原因與未採用的 shadow 實驗

後續已加入 `tests/slint_rendering.rs`，由正常Cargo整合測試直接使用本產品生成的 `TerminalWindow`／`ArticleViewport` 與Slint公開的Skia software surface API。兩個獨立window／renderer各有自己的緩衝：reference每幀age0全畫，另一個保留前幀並回報age1；沒有在同一renderer上另取snapshot而改變其dirty history。固定合成時鐘，逐像素比較BGRA；另用獨立指定的灰色footer及紅／藍圖片防止兩邊都空白仍誤過。一般核心／Dioxus測試不啟用此診斷；兩項直接optional依賴仍是原已選用的Slint1.18.1，只有明確開 `slint-renderer-audit` 才使用，未patch任何依賴或增加新repo。

本機一般Slint及native-player兩配置各118組畫面完全一致，包含terminal→article、focus、載入框→圖片尺寸、重建／退休文章及圖片repeater、頁尾位置和下方控制列變動、100次雙圖換幀、1920×1080放大／還原，以及physical buffer不變時1.25／1.5／1.0倍率往返。每次暖機換幀另要求age1且damage小於整窗，確保不是拿兩個full repaint互比。這100次是合成model更新，不能算S8的100次原生輸入／翻頁或OS掉幀證據。兩配置每案執行約0.44／0.45秒，這是診斷測試wall time，不能算產品效能。

上述對照**尚未重現原生App首次展圖的頁尾缺失**，因此不推翻下面的拒用結論，也未重新採用shadow或替換四平台包。它只縮小了已檢查的離屏情境；Rust owner在實際事件迴圈中的投影順序、window/presentation及瞬間輸入狀態仍需核對。初版Rc弱參照型別推導錯誤、早期成功但有unused DrawOutcome warning的log，以及後續嚴格成功版皆分開保存在 `render-audit/offscreen/`；沒有略過失敗。固定Cargo cache內重用同一套依賴及兩份必要測試執行檔，不建立額外App或測試repo。

原兩種候選feature×四target的依賴選用8案、collector的5項Python測試、含native-player的audit all-target release Clippy、fmt／diff及docs21項gate回歸通過；依賴解析不算跨平台編譯。最終同來源 `cargo test … --no-run` 暖檢查wall0.401秒／Cargo0.34秒，沒有Compiling，測試exe前後SHA同為 `b563d689…`。兩份約31MB測試exe及固定cache（此時盤點6,225,580KiB）保留供後續重用；沒有新增過期App／解壓副本可清，不做Cargo clean。四份保護檔hash不變；本輪removed為空、free前後及cache／exe精確身分在 `offscreen/cleanup.json`，原始資料及範圍在 `offscreen/acceptance.json`。候選App仍為已驗 `ad01a6d5…`；Cargo整合測試的cache頂層CLI另列hash及執行過的Cargo配置，不能當已驗發佈包。

沿用同一App pair、固定Cargo cache與雙GIF素材。先核對實際選用的softbuffer0.4.8 CoreGraphics、Slint Skia1.18.1 software surface及renderer來源，三個檔案均與本機保存的發布crate逐byte相符。CoreGraphics每次取得buffer都配置全新的零值像素、age固定0，`present_with_damage`忽略damage；Skia雖已開partial rendering，age0仍使其把整個視窗加入dirty region。這是目前依賴的實際路徑，不是缺少一個啟用partial rendering的環境開關；上游亦有[CoreGraphics提交研究](https://github.com/rust-windowing/softbuffer/issues/83)。

以原已驗native `ad01a6d5…` 啟用上游 `SLINT_SKIA_PARTIAL_RENDERING=log`，CUA實際展開兩圖；192筆repaint紀錄、含最近40筆，均為100%。未重編原程式，沒有把log數當OS呈現幀數或CPU比例。唯一嚴格Close ACK通過、root PID消失、HTTP兩URL各一次且owner正常join。退出時一次CUA複合呼叫timeout，後續核對App inventory、peer receipt及PID確認終止，未因此重啟。

接著在同一worktree用小型Cargo path patch試作macOS專用private BGRA shadow：保留最後成功畫面，尺寸／stride改變或render error時age0，其餘age1；完成後複製到softbuffer的獨立target，不改CoreGraphics持有的像素。非macOS及GPU路徑不變，額外保留一張視窗大小buffer。25份原始crate檔及授權以SHA核對後才實驗；兩個實際shadow測試和core392 passed／4 ignored，固定cache下release主程式建置54.94秒。首次選錯Cargo package feature參數被拒絕，改為同時選root與依賴後通過，沒有新建測試repo。離線update曾重新選部分依賴，立即還原，只留當時實驗所需的Skia path來源差異；最終Cargo.toml／lock已全部回復HEAD。

實驗raw `1c01de96…`／staged `4a4da212…` 在雙圖暖機時通常只重畫21.29%，但**首次展開的實際CUA畫面曾缺頁尾／底列，左緣另有青色短線**。全螢幕往返後及開關原生設定後可恢復，不足以證明初始顯示正確；原因仍未定位，故整個patch未採用。另取恢復後10秒／101筆CPU1.149632709秒、footprint峰值126,048,576、RSS543,506,432 bytes；此輪開repaint log且先經fullscreen／Settings，不能和舊輪直接算改善。較早一段0.845429127秒區間與後續工具操作的交界未登記，排除比較。實驗生命週期的唯一嚴格Close及HTTP join通過，不等於畫面驗收通過。

已還原原 `ad01a6d5…` native及原launcher並重新簽章，四架構交付包完全未替換。保留最小失敗native gzip34,055,533 bytes、完整實驗來源與授權tar97,391 bytes及小型設定／packet，供後續釐清dirty invalidation；不留下第二套App。原始紀錄與拒用理由在 `.local/verification/slint-dual-gif-current/render-audit/`。移除 `/Users/ladesine/Documents/Github/PTTStella-slint/vendor/i-slint-renderer-skia` 與該證據根的 `baseline-native` 重複副本前，核對resolved／UID／無symlink／lsof、逐檔archive byte equality及6份保護檔SHA。free265846763520→265915838464 bytes僅為並行觀測，exact inventory見 `cleanup.json`。固定target及可重用中間檔保留，未Cargo clean；cache頂層raw仍是拒用實驗，須經正常Cargo依目前來源重建才可另作驗收，不可把檔名當來源身分。下一步不重做相同age1實驗；先定位實際像素失效，或評估能明確保證dirty／presentation語義的上游方案。S8仍not_ready，S9未執行。

## 2026-11-10 GIF 循環重用與剩餘繪製成本

接續同一對診斷App、固定Cargo target、marked store及 `47c975f3…` 雙GIF素材，沒有另建repo或整套環境。先用原R3 native `69c32791…` 重測，10秒／101筆CPU3.664067167秒，footprint峰值97,277,696 bytes。其後另用系統 `sample PID 5 1 -file OUTPUT` 取堆疊，沒有開TextEdit；profiler區間不計入前述資源數據。樣本同時出現media Playback→GIF／weezl解碼，以及主執行緒Skia software／softbuffer CoreGraphics present／vImage色彩轉換。這是執行緒狀態抽樣，不把含sleep的stack次數當成CPU百分比。

已在共用Playback加入[有界循環快取](media.md#slint-候選的-gif-循環快取)。首次完整逐幀解碼、合成與驗證EOF後才跳過重複LZW；每個播放者至多16MiB唯一RGBA，逐幀保留delay，像素相同可共用buffer，hash碰撞仍完整比對。超額退回原增量路徑；暫停、自然結束或停止釋放不再需要的循環。沒有變更來源512×160、32幀、100ms與兩圖可見條件。

修正後的raw `f359050a…`／診斷 `bb2322aa…`，真實建置為f48e7e4加保存patch。相同設定／runtime／原生焦點／游標及完整16服務下，取得兩段連續10秒／101筆，然後另外做5秒stack取樣：

| 修正後狀態 | CPU seconds | footprint峰值 bytes | RSS峰值 bytes |
| --- | --- | --- | --- |
| 播放 | 1.930350334 | 100,341,696 | 377,225,216 |
| 接續暖機 | 1.867000249 | 103,421,888 | 380,305,408 |
| 另一新程序：開關設定後恢復 | 1.705098042 | 109,680,192 | 405,078,016 |

暖機stack樣本未再取到GIF／weezl解碼，仍有Skia render與CoreGraphics present／vImage。這支持先前的重複解碼成本已降低；剩餘UI提交仍需處理，CPU仍高於前輪Dioxus0.745–1.15秒，沒有宣稱S8通過。單輪與不同程序的波動、literal URL port、版面／foreground及OS呈現限制沿用前節，不能以此計算正式平替改善比例或OS掉幀。

最終Clippy將同一整除判定改用is_multiple_of後，raw hash為 `9524b6d6…`／診斷 `ad01a6d5…`，另以這份最終native重跑，兩段10秒／101筆CPU分別1.319630750／1.355970584秒，footprint峰值107,157,376／106,207,104 bytes，RSS417,873,920／416,923,648 bytes。兩圖、設定開關、恢復紅藍相位、文章焦點、嚴格Close及全數觀測身分退出皆核對；HTTP兩URL各一次，恢復未重抓。與中途1.87秒的波動尚未定位，不能歸因於Clippy修整或拿最佳單輪計算正式改善。精確binary、設定、時間與來源核對在 `profile-after/accepted/`。

實際重跑也發現被動準備誤用article_reading_ready（包含原生焦點權限）。現在只核對原始frame、已ack且完整的閱讀packet與widget啟用；準備報告實見foreground=false、ready=false仍能prepared，未合成前景。後續由CUA原生Raise／文章點擊取得焦點及展開兩圖。正式輸入、組字、失焦與原始bytes准入未由此放寬。

第一個完整播放／取樣程序因手動檢視超過既有Surface peer300秒期限而斷線，log為loopback probe timed out，不能列為正常退出通過；其資源取樣與stack在期限內，失敗與畫面保留。另以同一binary／store快速重跑：兩GIF展開、開關設定、恢復紅藍相位與文章焦點、⌘Q，唯一嚴格Close ACK passed，全部觀測PID＋start消失。共兩個真正載圖程序對兩URL各2次GET，恢復未重抓，HTTP owner正常join。numeric exit仍未由LaunchServices orphan觀測。證據分列於 `slint-dual-gif-current/profile-before/`、`profile-after/final/`、`profile-after/lifecycle/`，不合併失敗與成功生命週期。

最終兩種Slint配置各103 passed／1 ignored，包含六項循環快取案例；兩配置all-target release Clippy通過。初次Image clone編譯錯誤與is_multiple_of lint失敗另存，沒有放寬檢查。更新文件與fmt不另重建Rust。已移除 `/private/tmp/pttstella-dual-gif-profile-kpsokzuu` 及 `slint-dual-gif-current/profile-before/baseline` 的舊68,604,528-byte native（Frameworks symlink不跟隨刪除），臨時根另94,665 bytes；兩者均先核對UID／resolved／lsof與inventory。3,244份保護檔hash不變，free266199617536→266268336128 bytes僅為並行觀測；exact paths／du／保留理由在 `profile-after/cleanup/ledger.json`。固定Cargo cache、最新診斷pair／素材、必要舊基準與唯一未解證據、四平台下載、來源及共享工具／使用者資料保留，未執行Cargo clean。接續重跑須先重設過期的loopback origin／runtime與新evidence；S0–S8仍持續，S9未執行。

## 2026-11-10 共用雙 GIF 負載與 CPU 反例

新增兩版共用的 [閱讀頁面](../src/replay/media_benchmark.rs)：23行、完整1/1頁、兩個獨立loopback GIF網址，由同一Rust格線reducer處理。來源只接受明確 `http://127.0.0.1:PORT` origin，拒絕遠端、帳密、path／query／fragment和控制字元；正常Live不使用此來源。Slint以marked-store的 `--store-scene gifs` 掛載與Live相同16項服務；準備檢查核對原始站端frame、已ack的完整閱讀packet及實際article widget，不再錯用web模式下不存在的terminal bitmap presentation。準備timer完成即停止，不合成focus、展圖、輸入或自動退出。

素材由既有CC0標準函式庫generator產出：512×160、32幀、每幀100ms、紅藍交替／無限循環，encoded1,974,503 bytes／SHA `47c975f33f5bfeac5d9cc7b11cc0eb8d90b5d240ba8d00e14b1d361b9dc094f4`。每圖2,621,440 frame pixels，兩圖5,242,880；不是最大GIF成本或瞬間峰值測試。舊generator四種入口產物逐byte不變。原先直接與tracked PNG比較失敗，舊generator本來就有不同encoded PNG；未修改原fixture或expected，改核對old／new generator輸出，失敗另保留。

重跑素材與HTTP owner（輸出目的地須不存在）：

```text
python3 tooling/generate_media_fixtures.py --dual-gif /ABS/owned.gif
python3 tooling/desktop_media.py --dual-gif /ABS/owned.gif --output /ABS/new-http
```

從 `new-http/ready.json` 取origin。結束時建立該目錄的 `stop` 檔或Ctrl+C，`result.json` 保存逐路徑count與join；20分鐘未結束為timeout，不算通過。獨立CLI已實際驗兩個URL的完整SHA／各一次GET與正常join，未使用UI driver。兩版原生參數如下，採相同marked synthetic settings：version6、bundled字型23／20／100、en-US、web閱讀、auto-expand off；`STORE` 含明確 `.store-compatibility.json` marker。完整實際設定bytes、marker與SHA在本輪證據，不能以使用者一般store代替。

```text
pttstella-slint --probe-store NEW_EVIDENCE STORE --store-scene gifs --media-origin ORIGIN
pttstella --probe-surface NEW_DIOXUS_STORE dual_gifs --media-origin ORIGIN
```

兩版在同一worktree／固定Cargo target建置，沒有另建測試repo；診斷App沿用同PID LaunchServices launcher。Dioxus為目前共用核心的對照，並非最初5ea9ad6。R3 raw Slint `79bdaa59…`／診斷 `69c32791…`，Dioxus raw `2c903bab…`／診斷 `4a3035b6…`；真實來源3c667eb加保存patch，沒有倒填為後續乾淨commit。Cargo驗證後target頂層Dioxus binary hash改變，原因未獨立定位；已測診斷App的兩份native hash均不變，記於 `post-gate-binaries.json`，未用新binary替換採樣身分。

CUA實際點兩個箭頭，兩張512×160同時可見，分別觀測到紅／藍相位；首尾焦點回文章輸入，播放／恢復採樣的游標在標題列(520,13)。外視窗均1040×768；Slint左靠排圖並保留原footer，Dioxus置中且投影可視行，未宣稱viewport／OS raster完全相同。開設定、關設定後確認兩圖恢復，再⌘Q正常關窗。各狀態10秒／101筆穩定程序族結果如下；paused列為開啟設定的應用整體，兩版設定面板尺寸／實作不同，只供各自前後對照。

| 前端／狀態 | footprint峰值 bytes | RSS峰值 bytes | CPU seconds |
| --- | --- | --- | --- |
| Slint software／兩圖播放 | 98,276,864 | 368,459,776 | 3.603962167 |
| Dioxus／兩圖播放 | 733,376,192 | 554,745,856 | 1.149564664 |
| Slint／設定開啟 | 120,722,944 | 409,092,096 | 0.209669166 |
| Dioxus／設定開啟 | 751,234,816 | 594,083,840 | 0.462977875 |
| Slint／恢復兩圖 | 123,180,608 | 430,800,896 | 3.504497416 |
| Dioxus／恢復兩圖 | 753,299,264 | 595,001,344 | 0.745315333 |

**此雙GIF負載下已觀測到Slint CPU成本較高，不能只以低記憶體判為可平替。** 尚未定位是解碼、UI重繪或其他成本；下一步對同一native與素材取樣hot stacks再最佳化，不先降低幀數、可見圖數或解析度。這只是單輪校準：沒有連續OS foreground、完整cold／warm／交錯重複、100次操作或OS顯示延遲，不能計算S8達標比例。開設定時主圖不可見，這些GUI觀察亦沒有獨立數出decoder退休；worker准入／釋放由既有生產owner測試另證，反覆開關後回收與瞬間下載／decode峰值仍待驗。

同一HTTP owner共三個實際程序取圖（Dioxus首輪、Slint R3、Dioxus R3），兩路徑各3次GET；設定開關／恢復沒有重抓，server正常join。Dioxus R3核對23行完整閱讀、兩次顯式請求／一個batch、原始PNG、surface／owner audit及Close；Slint R3準備報告核對16服務／閱讀ack，最後唯一嚴格slint_full_dual_gifs Close ACK passed。最後觀測的全部PID＋start消失。LaunchServices orphan主程序的numeric exit未觀測，不記exit0。原始log／trace／設定／來源與失敗在 `.local/verification/slint-dual-gif-current/`；raw缺rpath的dyld失敗、web準備條件失敗、首輪Dioxus漏計新case的failed audit與編譯錯誤均保留，沒有放寬原expected。

已核對無open files後移除 `/private/tmp/pttstella-dual-gif-q2lzq45q`（regular-file logical214,712 bytes／du256KiB），小型結果先逐檔核SHA保存在 `native-evidence/`；3,058份保護檔hash未變。free266962845696→266963103744 bytes僅為並行觀測，exact path／resolved／UID／lsof／inventory見 `cleanup/ledger.json`。兩份當前診斷App、同一2MB素材及其精確OS namespace保留供尚未完成的CPU profile重用，不另留各階段App；舊一次性store及probe輸出已移除，重跑前須重新配置明確loopback origin與新evidence，不能直接重啟過期launcher。固定Cargo cache未clean，本次cleanup時du4668044KiB；後續core gate仍沿用同一target且開啟dev／test增量。最新四平台下載、來源／最小基準、共享工具及使用者資料保留，Slint worktree尚未整合。

本批core gate14命令／2個replay passed；native-player的lib392 passed／4 ignored、Slint bin97／1；desktop的lib379／4、bin121／1、多程序10／1；一般Slint bin97／1。native-player／一般Slint／desktop三配置all-target release Clippy、fmt、JS383與Python177通過。獨立server CLI另實際核對兩次完整payload SHA與正常join。文件gate通過；這些驗證不等於動畫CPU問題已修或S0–S8完成。

## 2026-11-10 同程序啟動與原生焦點校準

本輪不重編Rust，重用目前arm64候選與既有2846dc4 Dioxus對照。診斷App改由LaunchServices啟動RunFixture，再以 `os.execve` 於同一PID執行native，不再由Python另spawn主程式。兩者都觀測到PPID1／responsible等於自己的PID；Dioxus的WebKit GPU／WebContent／Networking及ThemeWidget依responsibility歸入，Slint則含native與ThemeWidget。原生CUA現在可在Dioxus點擊標題列、座標點浮球、按Escape關閉選單並返回終端焦點，未再出現前輪noWindowsAvailable。此結果支持使用同PID啟動繼續量測，尚未單獨證明前輪故障的唯一原因。

證據在 `.local/verification/slint-foreground-current/`。兩版設定SHA相同，bundled Cascadia Mono／Noto Sans CJK TC、23／20／100、en-US、hover off；相同80×24內容及1040×768含標題列外視窗，游標固定標題列(520,13)，首尾CUA終端焦點／Escape有原生證據。Slint prepared另證明16個服務、1040×740 viewport／1040×624 grid、scale1／zoom1；Dioxus幾何由實際screenshot核對，沒有機器viewport receipt。兩版OS字形／baseline並非pixel-identical，未把相同名義尺寸當成所有光柵細節相同。

| 程序／預設renderer | 10秒／101筆footprint峰值 bytes | RSS峰值 bytes | CPU seconds |
| --- | --- | --- | --- |
| Dioxus同PID對照，原生輸入可用 | 642,592,384（約613MiB） | 466,550,784 | 0.370350541 |
| Slint無renderer旗標，macOS software | 93,230,720（約89MiB） | 283,148,288 | 0.192464708 |

此為單輪閒置校準，沒有連續記錄整段OS foreground，未控制／重複cold與warm；不能計算S8達標比例。首輪Dioxus游標仍在浮球上，另有101筆／0.536860208秒CPU，完整保留但不混入表列條件。Slint首個座標點浮球未展開，改用AX點擊後原生Escape成功，原始未變狀態也保留為待查操作觀察，未宣稱所有pointer矩陣完成。

Dioxus產品來源native `948d2a9e…`，重新ad-hoc簽名後 `fd52aa8d…`，仍不是最初5ea9ad6基準；Slint封裝來源 `08cb87df…`，診斷重新簽名後 `dd3a0d4e…`，產品來源核對09a13da，實際原建置8f40e47加R4 patch不變。Dioxus診斷App deep-strict通過；Slint診斷App因外部Frameworks symlink而deep-strict失敗，standalone native strict通過，未當成可分發App。產品四包未改。

CUA明確匯出Dioxus全頁PNG，960×576／SHA `1e25a773…` 逐byte等於前輪原生輸出；Slint source PNG亦為960×576／既有 `294df2bb…`，兩種renderer輸出各自核對，未宣稱彼此bytes相同。Dioxus surface／owner audit passed、connection／credential／file worker join、cleanup完成；Slint唯一嚴格slint_full_parity_grid Close ACK passed。兩版均用CUA⌘Q結束，全部觀測PID＋start消失；LaunchServices啟動的orphan主程序無可wait的parent，numeric exit code未觀測，沒有記成exit0。IMK mach-port訊息仍保留，不推定IME結果。

已清理兩份診斷App、單一暫存資料根及五個精確自有OS cache／WebKit／暫存namespace，合計110,801,856 regular-file logical bytes。exact八條resolved路徑、UID／symlink／lsof、inventory／du及free270185943040→270296879104 bytes在 `cleanup/ledger.json`；free僅為並行觀測。首輪preflight將系統 `/var`→`/private/var` ancestor alias當未知差異而停止，未刪檔；明確核對後用resolved exact path接續，失敗另存。3,037份保護檔hash不變；只留小型來源／啟動器metadata、結果與trace。固定Cargo target／worktree、最新候選、原始對照、來源交付與使用者資料保留，未執行Cargo clean。下一步補雙GIF／混合負載、相同顯示觀測點及完整次數，S8仍not_ready。

## 2026-11-10 macOS 預設軟體合成與影片尺寸

本輪來源將 macOS 的無旗標路徑改用 Skia 軟體合成及既有原生 CPU 影片 worker。Windows／Linux 的 Auto 選擇保持原邏輯；`SLINT_BACKEND` 仍可覆寫，另可明確指定互斥的 `--software-renderer`、`--opengl-renderer`、macOS `--metal-renderer`。Metal UI 必須在 mount 前選定 CPU 影片：首輪三格式都停在 Ready／offer 尚未被接手，未發 HTTP；修正後三案通過，沒有放寬就緒或像素條件。預設切換僅發生在 Slint 候選，不切換正式 Dioxus 入口。

同時移除 CPU 影片既有720p上限，與 GPU 共用8192邊／16,777,216像素預算，超出才等比縮小。沿用單一 latest frame、有界通知及 worker 退出；依實際 view 配置，閒置不預配最大 buffer。新增獨立 expected 檢查 Full HD／4K 保持尺寸、超大正方形縮為4096×4096、越界拒絕，以及3840×2160四角 RGB／alpha255和完整 frame 長度。這是實際生產 buffer／轉換測試，尚非4K影片播放速度、4K顯示器或原生音訊驗收。

證據在 `.local/verification/slint-renderer-api-current/`。R4 raw為 `d1fd308f…`，使用既有50庫閉包的診斷 native為 `c8eeab19…`；確切hash、來源8f40e47加patch／351份fingerprint、各revision及命令分開保存。前一輪的同機／字型條件延用，M3 Ultra／256GiB／macOS15.7.7。無 renderer 旗標的完整16-owner視窗、viewport1040×740／grid1040×624，在原生 CUA terminal focus首尾可見下完成10秒／101筆穩定採樣：footprint峰值104,027,712 bytes（約99MiB）、CPU0.207124876秒，RSS峰值260,440,064 bytes。先前 OpenGL 842–845MiB與明確Metal R1校準約574MiB分列，不宣稱已達S8的兩版改善門檻或證明所有機型相同比例。

同一R4實際視窗以原生 `⌃⌘F` 進1920×1080全螢幕，再回1040×768（含標題列），terminal focus保留；兩張 screenshot 在CUA紀錄，沒有另存副本。返回後 family footprint74,061,888 bytes；vmmap主程序57.1M、圖形IOAccelerator resident16K。正常關窗exit0／唯一嚴格Close ACK，最後觀測的PID＋start全數消失。沒有用本輪推定實體IME或F11矩陣完成。

R4無旗標的MP4／WebM／OGV三案、837行／37次取頁長文、PNG／單GIF、文章輸入與貼上共七案皆exit0／嚴格Close，影片核對CPU render_path、四色、暫停／seek／失焦、render退休及session join；HTTP workers全部joined。固定960×576的source PNG與三API控制逐byte相同，仍為 `294df2bb…`。雙GIF峰值、100次操作／cold與warm重複、OS顯示延遲及Dioxus原生前景條件仍須補齊；單GIF案例沒有擴大標成雙GIF。

另以沒有Slint／Skia／媒體或視窗的自製CGL程式定位成本：context current約5.9MB，第一次清除1040×624 FBO後主程序峰值457,721,408 bytes，vmmap圖形392.2M；destroy context後約7.2MB。紅色pixel `[255,0,0,255]` 獨立核對、exit0，原始碼／trace／vmmap在 `slint-gl-allocation-audit/`，臨時exe已清理。此反例證明部分圖形成本不需要Slint快取才會出現，並未定位或宣稱修復driver洩漏。

本輪Rust native library390 passed／4 ignored、兩種Slint配置各97 passed／1 ignored，兩配置all-target release Clippy、fmt、Python177項通過。首輪Metal三案failed、fixture App因外部Frameworks symlink而deep-strict失敗及後續未啟動的runner錯誤均保留；standalone native strict通過，正式封裝仍要求完整閉包及原strict gate。R4正式arm64 native為 `08cb87df…`，封裝與清理結果如下；此處不把診斷wrapper當可分發App，也不把測試通過當S0–S8完成。

正式arm64 App／ZIP逐檔1234個payload一致，50庫與202份既有notice未變，另保存一份舊runtime manifest receipt；App deep-strict及DMG驗證通過。封裝native在拒讀Homebrew的sandbox重跑上述七案，加明確software／OpenGL／Metal格線，共十案通過；自動toolkit操作沒有擴大標成OS鍵鼠或S8效能。最初notice collector錯把合法receipt當額外檔案的失敗保留，修正為核對舊manifest原bytes，未修改封裝內容。

依使用者最新指示，保留同一worktree及既有3298516KiB Cargo target；忽略於Git的 `.local/slint-build.toml` 固定指向此cache，dev／test增量開啟、debug0，release不變。同來源warm release wall0.668157292秒／Cargo0.61秒，沒有Compiling、raw SHA完全相同；證據為 `cache-reuse.json`。不再每批full clean；只有來源還原等必要驗證另建隔離checkout。

本輪清理五項：`slint-renderer-api-current/Stella Renderer Lab.app`、`/private/tmp/pttstella-renderer-api-4jmita6y`、同批 `restores/`、舊 `slint-ui-cjk-fallback/arm64-package`（改symlink至新包），及 `/private/var/folders/nd/qb7hn9s118qd524df0l6j1rm0000gn/C/io.github.yam276.pttstella.slint.storefixture.rendererapi`。先核對UID／resolved／symlink／lsof，暫存的非fixture結果複製後逐檔核SHA；共3,025份保護檔hash及cache exe不變。selected regular-file logical bytes合計515,133,931，du分列於 `cleanup/ledger.json`；filesystem available269875634176→270397325312 bytes僅為並行觀測，不全歸因本批。Cargo clean未執行；最新四包、來源／最小基準／唯一失敗證據、使用者資料及共享SDK／registry／VM保留，Slint worktree尚未整合，S0–S8持續。

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

## 2026-11-11 macOS 局部呈現實驗

鎖定的 softbuffer 0.4.8 在 macOS 將 `present_with_damage` 轉成整窗提交。實驗以 256×128 像素圖塊提交受影響的區域，首次繪製、超過範圍限制或非支援倍率時走整窗。目標是降低軟體 renderer 在多張 GIF 播放時的 CPU 開銷。[上游 macOS 呈現討論](https://github.com/rust-windowing/softbuffer/issues/83)

同一個 arm64 診斷執行檔 `1db8a1d6…`，按整窗、圖塊、圖塊、整窗順序播放兩張 512×160 的合成 GIF。1040×768 外視窗，每次取樣 10 秒，共 101 筆，觀察取樣前後的文章焦點及紅藍換幀。四次各下載兩個不同 URL 一次，正常退出、嚴格 Close ACK 及 HTTP join 均成立。第三次在取樣後另外確認原生全螢幕 1920×1080 與回復原尺寸的畫面。

| 模式 | 10 秒 CPU | 取樣 footprint 峰值 |
| --- | ---: | ---: |
| 整窗 1 | 0.326 s | 117.2 MiB |
| 圖塊 2 | 0.269 s | 100.0 MiB |
| 圖塊 3 | 0.271 s | 110.5 MiB |
| 整窗 4 | 0.314 s | 104.0 MiB |

兩組配對分別少 17.5% 和 13.7% CPU。前一組同視窗的生命週期計數平均每次提交 769,600／352,842 像素，少 54.2%。這是同一 Slint 診斷程式的 A/B；先前 Dioxus 的雙 GIF CPU 中位數 0.180 秒仍低於圖塊組的 0.270 秒。畫面只經實際視窗觀察，尚缺連續焦點、呈現延遲、高 DPI、多螢幕與其他場景驗證。

第一次 staging 缺 LC_RPATH，修正後以既有候選的 runtime 啟動。首次圖塊畫面上下位置顛倒，改用 Core Animation 的垂直原點後重跑全組。Cargo 初次離線重解誤動其他依賴，還原後兩次 locked build 均成功。上述原始紀錄均保留，原生工具在 Quit 後的觀察逾時由實際 exit 0、Close 和程序消失獨立核對。

原始資料、兩份實驗補丁、原始 crate SHA-256 及編譯命令在 `.local/verification/slint-macos-damage-current/`。重跑時以該目錄的 `final-source.json` 所列 commit 為基準，從 `softbuffer-source.json` 核對發布的 0.4.8 crate 並按其檔案清單放入 `vendor/softbuffer`，套用 `final-tracked.patch` 和 `softbuffer-0.4.8-macos-tiles.patch`，沿用 `final-build.json` 命令及 `trial.py` 的模式與合成資料。實驗依賴已退出產品來源，保留四架構候選的現有呈現方式。唯一暫存 App、其專用空 cache 和暫用 vendor 副本已清理，固定 Cargo target 保留。確切路徑、檔案大小和磁碟觀測見 `cleanup.json`。

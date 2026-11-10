# PTTStella Slint 平替實作與驗收計畫

從既有 Rust 終端核心和 macOS Slint 原型出發，做出能讀寫現有 PTTStella 資料、保留目前操作並可安裝在 Windows、macOS、Linux 的原生桌面候選版。以實測決定是否將 Slint 改為正式入口，再移除 Dioxus 與 WebView 發佈相依。

Dioxus 初始基準為 `5ea9ad6a498331d0cbebc7d9803e0a51df46e5c3`，已包含分段接續長文、大型 GIF 與選用的自動開圖。Slint 起點為獨立 worktree 內的 `slint-research`／`pttstella-slint` 原型。原型已在 macOS arm64 實測雙色字、80×24、放大拖曳、全螢幕及獨立輸入欄的威注音。實體 Retina、終端內組字、跨平台原生輸入和與 Dioxus 的同負載效能比較仍待驗。實驗數據及已知差異見 [離線原型紀錄](slint-prototype.md)。

## 目前進度（2026-11-11）

**尚未達到可平替 Dioxus 的驗收條件。** 現有 Slint 來源及計畫已提交，正常影片退出以 `1b2bb96` 修正。下表區分已接線與尚須完成的驗收；歷次結果及確切 scope 見 [原型紀錄](slint-prototype.md)。

文章直接輸入抖動與 macOS `⌃⌘F` 修正已在四架構候選保留。兩Mac仍為6ad8e39，各15項封裝後合成案例通過；最新Linux已核對fbcfe2e的閱讀狀態列及探針修正，ELF150c1a0d完成14案與四案針對性重測，全部exit0／嚴格Close。GL案例使用QEMU／Xvfb＋Mesa softpipe，原LLVM中止與一次GL逾時的失敗證據仍保留，不能當原生硬體或S8效能通過。另備Windowsac788a45仍為61d1c3f，尚未包含這批新修正；使用者待驗Windows69542cb7入口不變。Intel／Linux實機依使用者指示暫緩，其餘原生／資料／效能矩陣未完成；candidate index分列來源與包SHA。

macOS預設Skia軟體合成及原生CPU影片已降低閒置圖形記憶體；GIF循環重用與保留raster後，同一雙GIF校準的10秒CPU約1.10秒，較早期約3.60秒下降。這是不同修正階段的校準，未完成正式冷暖次數、100次操作、連續前景／延遲、峰值及所有平台矩陣，不能據此宣布勝過Dioxus或S8通過。CPU影片與GPU共用尺寸／像素預算，保留Full HD／4K及明確GL／Metal選項；MP4 GPU偶發錯色仍未解。詳 [效能與原始限制](slint-macos-measurement.md)。

| 階段 | 目前已有 | 尚須完成 |
| --- | --- | --- |
| S0 | 共用 Rust 服務、獨立 Slint 入口及可編譯提交序列；四 target 的一般／播放器兩配置 Cargo 依賴 gate | 完整功能案例對照、各平台連結／runtime 依賴與最終基準重跑 |
| S1 | 格線／雙色字／縮放、無系統CJK字型的控制項fallback及macOS 原生輸入的分立證據；Windows Microsoft注音候選位置／Esc取消／單次提交的使用者人工回報 | 完整候選窗／組字／失焦矩陣、跨螢幕 DPI、可及性與其他平台原生 |
| S2 | 共用 Connection、登入／保活、站點表單、嚴格切站／雙編碼 bytes | 同 endpoint 編碼切換、預設站重啟／不登入、完整信任／vault 原生矩陣 |
| S3 | 選取、輸出、浮球／選單及原生互動的分立證據 | 全部快捷鍵／pointer／modal／可及性操作矩陣 |
| S4 | 雙閱讀模式、長文取頁、尋找、名單／右鍵／揭露／底色、書籤／離線、快速前往與最近看板、目前／指定列表文章收藏、共用推文索引／原生定位與側標、推文更新／未讀／保留與復原；macOS基本按鈕、列表右鍵指定收藏與新程序斷線讀回補驗 | 跨平台推文控制／定位與完整鍵盤／焦點矩陣、其他平台列表右鍵及新程序離線操作、混合 Live 等效與全功能回歸 |
| S5 | 圖片／GIF／文內展圖、主視窗影片；正常關窗／Quit 的 GL 清理；原生時間軸修正；失敗像素／播放狀態與Close audit、三格式錯色負向對照 | MP4 GPU 像素偶發失敗仍未重現／解決、多圖峰值、GL 恢復、音訊、四架構可交付 runtime |
| S6 | 原文編輯／色票／站端上色、取消／部分失敗、草稿故障與多開復原 | 原生 IME 候選／完整輸入邊界、跨平台操作；舊末字異常原因仍未定 |
| S7 | 原生設定八分類、語言／站點／名單、24項搜尋定位、具名設定檔與正常退出；[共用版本檢查／下載入口](slint-release-check.md)、toolkit空白鍵及退出HTTP／listener join；[共用通知政策／原生時序](slint-notifications.md)、hover／Tab焦點暫停及timer退出；[三平台系統動畫偏好來源](slint-system-motion.md)、macOS初讀／本機通知回呼與observer／UI listener退出；三版本store往返28案；[實際前端往返](slint-store-frontends.md)補 macOS v1／v6 設定與站點六輪、名單三輪、可攜設定三輪操作（原儲存位置仍待查）、收藏名稱／離線副本與最近看板清除三輪、同一資料草稿編輯保存三輪 | 實際OS動畫偏好切換／還原及其他平台原生、原生通知／其他管理入口、完整Dock／modal／OS I/O與可及性矩陣、實際前端／平台路徑及vault往返 |
| S8 | [32套件x64 runtime乾淨重建](slint-native-source-replay.md)、47庫閉包核對（首輪MP4逾時未定因）；[三組產品來源完整還原](slint-product-source.md)、新還原目錄的macOS arm64一般Slint配置97測試通過；S1 Windows 探針；macOS arm64 主程式候選 App／ZIP／DMG（含bc380f9導覽修正）及無Homebrew sandbox；[Windows x64完整GUI候選](slint-windows-candidate.md)，目前包與已測舊exe分列，Windows10實機13案回報經修正helper重驗（原版10過／3個格式誤判），嚴格ACK／像素／join；[Linux x64完整deb／tar.gz](slint-linux-candidate.md)、Ubuntu安裝／software八案；[macOS x64 App／ZIP／DMG](slint-macos-x64.md)、自建47項runtime閉包／32份來源、兩開發prefix不可用的Rosetta13案 | Intel與Linux原生、Windows完整人工操作、IME／DPI／音訊、VC runtime／最低OS／完整乾淨安裝、完整第三方來源交付與播放器重播、資料回退及同負載效能；尚未通過 |

接續完整鍵盤／焦點與其他平台、跨平台列表／重開、混合功能／退出／資料往返案例及完整候選打包。使用者已確認可在 Windows 執行驗收包。缺少 runner 的項目保持 `not_ready`，不將 macOS 或 toolkit 結果換算成跨平台完成度。S0–S8 goal 持續；S9 正式入口與發佈尚未執行。

## 執行方式與完成條件

- 實作在指定 Slint worktree 內連續推進。依下列 S0–S8 的相依順序完成程式、回歸和證據。缺 runner 或硬體時登記確切缺項，繼續可獨立驗證的階段。若原生輸入、雙色格線或原生播放器證明不可行，先修正或換成可驗收的技術路徑，再擴大相依功能。階段證據整理完即可接續，日常實作與重跑沿用本計畫。
- 每階段做成可獨立驗收的變更，遵守 [AGENTS.md](../AGENTS.md) 的分支、提交與清理規則。需要新分支、推送、實站操作、切換正式入口或發佈時，依本 repo 規則和使用者當時授權執行。測試與研究 App 使用獨立 App ID、設定／資料目錄、診斷及密碼庫服務，合成探針只連本機。
- 候選版須通過所有既有功能、四個目標架構安裝、舊資料讀寫與回退，以及下文效能門檻。S8 交付可審閱的候選包、可逆切換方案與結論。使用者接受後才進入 S9。若 `develop` 推進，切換前以待整合的實際 commit 重跑變更相關項目和整體效能，新增功能納入驗收。

## 產物保留規則（2026-11-09 使用者要求）

目前最新四架構候選均在 `.local/verification/slint-merged-id-spaces/`，格式、來源、執行檔雜湊與驗證範圍見[最新候選](#2026-11-10-最新候選)。先前 `slint-windows-current/package-current/`、`slint-system-motion/package-current/`、Intel與Linux前份完整包已被取代並清理；保留必要metadata、notices與獨特證據，不要求歷史段落的舊latest路徑繼續存在。

Windows 10 build19045／PowerShell5.1的13案receipt及Microsoft注音三項人工回報屬舊04249364…執行檔；目前新69542cb7…尚待使用者晚點實測。已封裝Mac的shell／通知及網頁account失敗紀錄保留；最新來源修正後的五個Rosetta帳號／推文案例已通過，下載包尚未替換，詳文末。不能將Rosetta稱為Intel硬體驗收。Intel／Linux原生依使用者「先包起來」指示不等待，目前仍未驗。

2026-11-10 使用者進一步要求復用編譯中間檔，取代先前「每批清空target」的做法。日常實作固定使用同一Slint worktree與既有 `.local/verification/slint-renderer-api-current/target`；這是持續復用的單一Cargo快取，不隨驗收批次另建target或複製測試repo。本機已建立忽略於Git的 `.local/slint-build.toml`，一般Cargo命令使用 `cargo --config .local/slint-build.toml …`；dev／test保留增量編譯、debug symbols為0，release profile不變。已驗同來源release暖編譯wall0.668秒／Cargo0.61秒，沒有Compiling且exe SHA不變；cache盤點3298516KiB，證據在 slint-renderer-api-current/cache-reuse.json。

已測且被取代的App／ZIP／解壓副本、一次性測試資料仍依 [task-owned流程](development-harness.md#產物清理與結案) 清理；可復用編譯快取保留並盤點，不因某一批驗收完成就cargo clean。一般測試直接用目前worktree；完整來源還原、乾淨安裝或必要負例才用獨立checkout／隔離資料，不能以復用污染驗收。cache失效、已退役或不再需要時再做針對性原生clean，保護必要來源／交付／獨特證據，不啟動背景刪除服務。

2026-11-10 使用者再次要求即時處理磁碟增長：先中斷功能擴充，完成系統動畫偏好批次的中途清理。`slint-system-motion/target` 已無 open files，必要的新包及唯一待驗 CLI 已移出；Cargo 原生 clean 移除20,505檔、回報5.7GiB，清理前 du為6032644KiB。285份產品來源與3584份保護交付檔案hash不變；這次清理不代表動畫偏好功能驗收完成，前一份已驗候選仍保留到新包覆蓋回歸。另將96份歷史大型patch／log／盤點JSON無損gzip封存，逐份解壓核對SHA，169483387 bytes縮為33385092 bytes；刪除7份逐byte相同的合成PNG，各留一份及對照。清除歷次只剩metadata的11個target目錄，metadata另封存；沒有殘留自有Slint容器。exact paths、owner／resolved path／symlink／lsof、原生clean結果、保留例外與filesystem觀測見 `.local/verification/slint-artifact-cleanup-20261110-motion/ledger.json`；被壓縮的歷史證據改讀原路徑加`.gz`，映射與SHA見同目錄`historical-plan.json`。本次不重建Rust，不動其他聊天、共用SDK／registry／VM或尚未整合的來源worktree。

每次開始先重用固定Cargo target與符合來源的既有執行檔；驗收結束或交接時盤點cache與交付，清理被取代的包及一次性暫存。保留可復用cache符合上述最新指示；早期逐批full clean紀錄是歷史行為，不再作為之後必須清空cache的依據。大型文字證據可無損封存，不累積每階段App或重複正向截圖。

2026-11-10 使用者再次要求即時清理。本次沒有啟動新建置或宣稱新功能驗收：先將已完成建置、尚未驗收的通知焦點探針移出 target，只留 `slint-notice-focus/pending-current/` 的單一 CLI、來源 patch 與 hash，再以 Cargo 原生 clean 清掉 `slint-notice-focus/target/release`（5679檔，Cargo宣告2.0GiB；target的du由2128852KiB降至8KiB）。另移除735張 byte-identical 的歷次 generated PNG 副本（logical 65,577,981 bytes／allocated 67,096,576 bytes），每張原圖的保留位置與SHA記在 `slint-artifact-cleanup-20261110/removed-duplicate-pngs.json`，歷次報告的已刪圖片路徑可由此追溯；所有獨特影像保留。已停止的 Dioxus 對照App專用系統暫存也移除。最新macOS候選改以 `slint-shell-chrome-final/package-current/` 為準，Windows S1包、Dioxus對照App、原始媒體／必要授權與測試資料均保留；1562份受保護檔案hash未變。所有已知Slint target只剩8–16KiB metadata，worktree的du為778964KiB。filesystem available 231056789504→233583538176 bytes僅為觀測，不能全歸因於本次刪除。exact removed paths、owner／resolved path／symlink／lsof、Cargo與du紀錄見 `.local/verification/slint-artifact-cleanup-20261110/ledger.json`；canonical與其他聊天namespace因歸屬不同或不明保留。未整合worktree與待驗來源修改未動。

2026-11-10 原生 runtime 批次中途清理：確認沒有編譯程序或選定目錄 open files，重新驗證30份原始套件與3份bootstrap工具來源archive的SHA，另將libjxl／libplacebo含submodule的來源逐檔封存核對；保存131份必要config／build診斷及mpv Swift MetalLayer失敗。移除36個精確目錄：32份解壓／Git來源副本、3份工具解壓副本與本批recipes/build，中間產物du合計1658992KiB；exact paths、owner／symlink、程序與保留雜湊在 `.local/verification/slint-artifact-cleanup-20261110-source/ledger.json`。12975份保留檔案hash不變。filesystem available 219806351360→221484134400 bytes是並行觀測；來源封存本身的新增空間與du分開記錄，不能全歸因於刪檔。保留31項已完成的runtime依賴install prefix供僅重試mpv、約22MiB臨時工具、最新各配置交付與最小證據；工具／install待本批用完後清理。兩個Cargo target沒有build profile，未重建Rust；共享SDK／registry／VM與其他聊天產物未動。系統ThemeWidgetControlViewService仍持有已刪舊探針exe，未為清理停止系統程序。首次保留清單遇到APFS來源目錄大小寫別名，已依刪前inventory逐筆核對並另記，不宣稱遺失來源或提前通過runtime驗收。 文件gate另抓出臨時venv附帶的Windows launcher；已刪除用完的pip bootstrap，未放寬gate，重跑通過。另記5個exact移除路徑，最終12086份保留檔案hash一致，Slint worktree du1435912KiB。

後續每批開建置前先檢查上一批target與待驗CLI，來源不變時優先重用保留的單一執行檔。中途收到清理要求也依最新復用規則盤點，只清已退役且不再需要的profile，不因暫無程序就清空快取；批次完成後移除被取代的CLI／App／ZIP／DMG及重複正向證據，保留最新必要交付和獨特失敗。只為清理或更新本紀錄不重建Rust，不把待驗當passed，也不因goal持續而累積歷代binary。

2026-11-10 通知批次結案：兩版共用copy／success時長與錯誤優先政策，Slint單一timer完成hover／Tab焦點與淡出暫停、舊期限撤銷、未知提示持續及退出；設定自動保存成功留在管理視窗，草稿帶回PTT的確認提示持續保留。最終arm64封裝software／OpenGL通知、版本檢查、設定搜尋與文章收藏五案皆exit0／Close ACK，1176份App／ZIP／唯讀DMG payload一致；最新包仍位於 `slint-shell-chrome-final/package-current`，exe SHA為 `b5e82341b7c7797fae98a911e696d96374aa969bb33a6732d36d336f4fb891ce`。來源、各gate、原生與未驗範圍見[通知紀錄](slint-notifications.md)。首個搜尋privacy焦點失敗有保留，同版重跑／舊版對照／最終版通過不等於原因已釐清；系統動畫偏好與其他架構同步仍待驗。

本批清理使用Cargo原生clean移除15683檔、宣告4.5GiB，另刪被取代的第一個通知包與先前arm64包、32張byte-identical PNG及自有pycache；source／必要來源／其他平台包保留，10976份保留檔與1176份新payload雜湊一致。Cargo自行刪除target根目錄後，輔助腳本重複rmtree同目錄而中止；已依target不存在的權威狀態接續，沒有重建或重跑清理。該階段尚未序列化的pre-clean free／du值不可追回，ledger明列measurement gap，不用後來數字冒充；後續舊包清理free220312371200→220600111104bytes為並行觀測。exact paths、逐檔inventory、lsof、Cargo宣告量、空間缺口與保留例外在 `slint-notification/cleanup/ledger.json`。Slint worktree du1504488KiB，通知批次證據5112KiB；未整合來源worktree、global工具／registry、共享VM及其他聊天產物保留，S0–S8仍active，S9未執行。

2026-11-10 Linux 批次中途再次執行即時清理：兩個專屬測試容器 `pttstella-slint-linux-native-base-20261110`／`pttstella-slint-linux-ubuntu-final-20261110` 已匯出結果，核對 ID／task label、無 bind mount 及僅有自有測試 helper 後正常 stop／rm；移除其約3.66GiB可寫層與無使用者的臨時 Ubuntu image。刪除缺少 late-loaded runtime 相依的第一版 Linux deb／tar.gz（du69512KiB），保留小型 manifest／hash／失敗紀錄；修正相依的同一執行檔候選由 `package-runtime-fixed` 移至 `slint-linux-candidate/package-current`，內容hash不變。先前該批 guest Cargo clean 為6622檔／2.4GiB，host Cargo clean為9790檔／3.1GiB，host target、暫存case-sensitive SDK映像、link wrapper及smoke binary均已清除；本次沒有重建Rust。另刪除已完成 settingsbaseline 對照的兩個專用 WebKit cache。exact paths、owner／resolved path／symlink、容器／程序盤點與保留例外見 `.local/verification/slint-artifact-cleanup-20261110-immediate/ledger.json`，前段Cargo證據在 `slint-linux-candidate/compile-cleanup/ledger.json`。

共用 VM 不關機、不 compact、不刪磁碟；確認 discard 支援後執行線上 TRIM，稀疏磁碟 allocated bytes 21678956544→11832786944，host filesystem available 212732891136→222590758912 bytes。TRIM 回報99.5GiB是discard範圍，並非釋放量；可用空間約增加9.18GiB包含先前guest已釋放的區塊及並行活動，不能全歸因於本次刪檔。Slint所有target只剩8–16KiB metadata。最新必要候選、Dioxus對照、來源／資產／使用者資料／獨特失敗保留；既有Windows SDK／工具約1.11GiB、shared Skia registry source約966MiB及其他聊天／來源不明cache不刪，未整合worktree與尚未提交的Linux打包實作保留。

清理不代表Linux驗收通過：修正runtime的候選在fresh Ubuntu24.04 x64／QEMU／Xvfb software下，安裝shell、含空格路徑的portable launcher、三格式影片及草稿共6案exit0且收到Close ACK；文章搜尋因「background ran a pending query」失敗、沒有Close ACK，證據保留於 `slint-linux-candidate/ubuntu-final-evidence`，尚待診斷。所有原生硬體／IME／GPU／音訊及效能結論仍待驗，候選報告維持not_run／not_ready；後續可由保留包重現，無須保留完整測試環境。

2026-11-09 已回收 main Slint target 的 dev／release、獨立 Windows 交叉編譯 target，以及34份過時 macOS 測試 App。當前只保留 Slint 一般候選、影片配置候選及唯一 Windows S1 ZIP；兩個最小 reopen bundle metadata 目錄沒有 native executable。被刪 App 的非執行紀錄／授權／測試資源壓成約3.2MB ZIP，完整 binaries 不存進 archive。精確清單、來源雜湊、lsof、Cargo 輸出與空間前後在 `.local/verification/slint-artifact-cleanup-20261109/`。Slint target 12KiB、verification 約416MiB；原始碼、未提交的推文導覽工作及必要交付內容不變。canonical checkout 的其他聊天產物不屬於本 Slint goal 所有權，保留而未跨案刪除。

後續三版本 store 往返批次驗收後，另清掉 `.local/verification/slint-store-roundtrip/target/release`、`drivers/candidate`、`drivers/baseline`、`drivers/release`，並正常移除 `checkouts/baseline` 與 `checkouts/release` 兩個臨時 worktree。Cargo 宣告6932檔／1.8GiB；filesystem free 由238467133440增至240648839168 bytes，這是並行工作期間的觀測值，不全歸因於本次清理。28案例結果、唯一失敗資料、來源 patch、各 ref／driver hash 均保留，原始碼與必要交付 hash 不變。主 target 與這批獨立 target 各12KiB，整個 Slint worktree 約497MiB（`.local` 約441MiB、Windows ZIP 約28MiB）。目前保留的一般候選為 `slint-native-list/Native-Bookmarks.app` 及其同檔重開所需 `native-state/bookmarks.json`，影片候選仍為 `slint-app-exit/Native-Video-Close.app`，Windows S1 ZIP 未更動。精確清單／du／Cargo輸出／空間前後記在 `.local/verification/slint-store-roundtrip/cleanup/ledger.json`；版本對照臨時 checkout 與 headless driver 納入往後每批清理，不保留可重建執行檔副本。

主程式打包批次後，最新 OpenGL／影片配置改保留 `.local/verification/slint-main-bundle/package-current/` 的 App／ZIP／DMG（約271MiB），取代並移除舊 `Native-Video-Close.app`。一般軟體繪製配置的 `Native-Bookmarks.app`、其合成收藏檔與唯一 Windows S1 ZIP 仍有必要，繼續保留。另清掉本批2.6GiB release profile及三個已失敗／被取代的 package目錄，沒有留舊執行檔或完整ZIP／DMG副本；精確路徑、source／交付hash、du及free前後在 `.local/verification/slint-main-bundle/cleanup/ledger.json`。本批target剩8KiB metadata，不因 MP4 重複性缺項而繼續保留編譯快取；最新候選與唯一失敗證據保留供接續查驗。

## 共用 Rust 邊界

| 擁有者 | 實作與介面 |
| --- | --- |
| 終端與連線 | 保留 [Screen](../src/screen.rs)、[Connection](../src/connection.rs) 與 Session worker。Screen 是 bytes、cell、ANSI、游標及 revision 的唯一來源。Session 唯一序列化輸入、站點切換、登入、保活、文章取頁與關閉。Slint 只發有型別的意圖及消費最新 `View`。 |
| 桌面服務 | 將現有 `desktop` 拆出共用桌面服務 feature。由依賴範圍決定最小移動，把既有站點／檔案／設定／字型／剪貼簿／圖片下載／密碼庫與持久化 owner 接給兩邊。Dioxus 的既有 `desktop` 呼叫維持有效，Slint 的候選 feature 單獨可建置。Headless core 繼續可測。 |
| 互動與呈現 | 從 [surface](../src/ui/surface.rs)、[SurfaceState](../src/ui/surface_state.rs)、[presentation](../src/ui/presentation.rs)、[protocol](../src/ui/protocol.rs) 逐項提取 WebView 無關的准入、投影、lease 與 reducer。Rust 共用資料可直接傳值或共享不可變快照。Dioxus adapter 仍負責既有 JSON wire、Eval、DOM ACK 和 WebView 信任檢查。Slint adapter 只接原生事件和本機模型。 |
| 畫面交接 | 保留一筆在途呈現與一筆合併後的最新快照。以「內容與對應幾何已安裝到同一可互動畫面」確認 presentation，顯示器呈現延遲另量。輸入、hit test、選取、媒體及輸出依既有各自需要的 generation／revision／input epoch／規則版本／presentation 核對。新來源 pending 直接撤銷舊輸出租約，即使 UI 尚未收到新畫面。過期確認只釋放原在途工作。 |
| 執行緒與關閉 | Slint 主執行緒擁有視窗、焦點、原生事件及 UI model。Connection／下載走既有非同步工作，磁碟、密碼庫、解碼與 PNG 走有界 worker；選檔器沿用平台要求在 UI thread 執行。只排程必要的 UI 喚醒。關窗先撤銷操作、計時器與回呼，再取消可取消工作，等待已開始的 OS 寫入、連線和 workers，顯示未完成或失敗，最後關視窗。 |

### WebView 模組遷移清單

| 現有程式 | Slint 對應 |
| --- | --- |
| `surface.js`、`surface-lifetime.js`、`surface-packets.js`、`surface-diagnostics.js` | 分離原生輸入、視窗事件與診斷。Slint 事件轉 typed intent，畫面由同一 Rust controller 派發。Dioxus 封包及 JS listeners 留在其 adapter 至切換。 |
| `surface-renderer.js`、`surface-fonts.js` | 原型 Skia painter 變成共用的 Slint 格線 painter。終端、可見文章列、字型預覽、ANSI 草稿投影與 PNG 消費同一份幾何與字形規則。 |
| `surface-navigation.js`、`surface-mouse.js`、`surface-context-menu.js`、`surface-tools.js`、`surface-toast.js` | Rust 管操作策略、命中、互動作用域和焦點復原。Slint 提供浮球、選單、狀態列、工具與通知的版面和原生事件。 |
| `article-find.js`、`article-updates.js`、`reply-navigation.js`、`surface-bookmarks.js`、`surface-filters.js` | Rust 共用文章快取、搜尋／推文索引、過濾投影與儲存交易。Slint 提供虛擬捲動、尋找、書籤和名單介面。 |
| `media.js`、`inline-images.js`、`gif-playback.js` | Rust 共用 URL 分類、下載、資源 owner 和文章錨點。原生 decoder／player 供 Slint 顯示，GIF 播放選擇和取消由單一 owner 管理。 |
| `surface-drafts.js`、`surface-editor-color.js`、`vendor/composer` | Rust 原始文件、編輯交易、ANSI 投影與索引。Slint 編輯介面、原生 IME、選取、色票及虛擬可視列取代 CodeMirror。 |
| `surface-settings*.js`、`settings-catalog.js`、`surface-login.js`、`surface-sites.js`、`i18n.js`、`vendor/fluent`、`app.css` | 共用 Rust 設定／站點／登入／Fluent 模型與字串。Slint 實作表單、分類搜尋、語言切換、版面、原生焦點及鍵盤可及性。JS 與 CSS 留作舊版對照。 |

## S0 固定基線與測試入口

1. 記錄兩個來源樹、工具鏈、OS／CPU／GPU／DPI、字型與 backend。對照 README、設定、快捷鍵、產品契約、`tooling/test_*.mjs`、原生探針和 release assets。每個既有互動指定一個案例 ID、基準證據、Slint 對應測試與所需平台。Dioxus 已有缺陷或未驗邊界單獨標示。
2. 做一份兩版共用的合成資料與 loopback runner。輸入為相同 byte chunks、分段時序、固定字型條件與本機 HTTP 媒體。peer 以獨立 expected 驗證出站 bytes、請求次數、取消、Close 和禁止操作。從既有 `tests/fixtures/dual-color.json`、`redraw.json` 和 `src/ui/probe.rs` 的閱讀案例著手。UI 驗證用真實 Slint／Dioxus host 產物，保存原生擷圖及可觀察狀態。診斷介面保留在 probe 範圍。
3. 建立共用服務 feature 和原生候選入口。用各 target 的 Cargo tree、link/import 清單及原生程序檢查驗證 Slint 可單獨建置。開發期以 `cargo build --locked --release --features slint-research --bin pttstella-slint` 和既有 `desktop` 建置作雙入口檢查，若候選 feature 改名則同步更新命令。檢查 Slint 授權和 AboutSlint 署名，列出 Skia、字型、編解碼器的授權及 notices 生成來源。預先登記 S8 的量測與門檻。

**通過條件**　兩版可重跑同一 replay 及基線用例，既有 core／desktop gates 通過。四個目標可各自解析獨立依賴圖。保留 Dioxus 對照產物及證據。

## S1 終端 renderer、原生輸入與高風險驗證

1. 把原型 renderer 接到真正的 `Frame` 和 `DisplayProfile`。採實體像素共享整數格線，背景先畫，雙色 glyph 只算一次原點、字型及 baseline，再做左右裁切。沿用 16 色／亮色／反白、未映射標記、ANSI 方塊／三角與框線，實作 ASCII／CJK 兩套字型、bundled fallback、字高、基線及 50%–150% 文字寬度。以 ink bounds 修正括號下緣，非整數 DPI 以邏輯字格計算字形縮放。固定 80×24 的寬高比、符合視窗、等比例 zoom、可捲動、全螢幕與最後一列。
2. 把 resize／DPI／frame 更新合併為事件驅動的 raster；同一畫面重用 pixel buffer 或 texture。先量全頁成本，再把有變動的列與游標／overlay 分離。使用有界字型、glyph 與可視列快取，逐項記錄 invalidation、配置、拷貝及上傳量。對齊畫面與 PNG 的畫筆，互動浮層放在獨立層。
3. 以原生文字元件做 caret 與 IME 入口，再驗證可觀察的原生 preedit／commit／cancel。組字文字就地顯示，候選窗錨點跟目前格線游標與 DPI。提交才由既有 `input` 編碼一次。前景、焦點、modal、選取及 input epoch 使用同一 gate。Slint 元件若無法完整提供 lifecycle，用鎖定版本的 Winit `Ime` 事件接最小 adapter，實測事件順序；原生文字事件與 raw key 不雙重送出。保留 PTT 編輯器 Ctrl+V 站端指令、Cmd+V／Ctrl+Shift+V 本機貼上、含 ANSI／多行先預覽、再次核對 scope，並沿用安全 SGR／輸入大小規則。 [Slint TextInput](https://docs.slint.dev/latest/docs/slint/reference/keyboard-input/textinput/) · [Winit Ime](https://docs.rs/winit/0.30.13/winit/event/enum.Ime.html) · [顯示契約](dual-color.md) · [輸入契約](feature-research.md)
4. 先做兩個小型原生驗證。編輯器用中文、混合 ANSI、跨行選取及一次 undo 證明可行。媒體用兩張 GIF、MP4／WebM／OGV 合成串流，證明有界播放、音訊、取消、釋放和乾淨環境可打包。先從 Windows 的 Winit＋Skia 主路徑及軟體路徑驗證，macOS 和 Linux 接著用同一 fixture。選後端時用實際文字、DPI、焦點、輸入與效能結果。

**通過條件**　Windows、macOS、Linux 都有真實 native window 的畫面／注音／貼上證據。驗證 100%／125%／150%／實體 Retina、跨螢幕 DPI、fallback、雙色裁切、括號、最末列、縮放拖曳與全螢幕。組字開始、候選鍵、空白、Enter、取消、失焦及換站的 peer bytes 可核對；同一字只提交一次，未提交部分為零 bytes。編輯器與影片的小型驗證各有可繼續實作的路徑。

## S2 連線、站點、登入與保活

1. Slint window 接既有 Connection watch 和命令 API。共用 WSS 系統 TLS、PTT SSH 公開帳號與 host-key pinning，保留 Big5-UAO／Big5／GBK、輸入預檢、舊明文端點管理與預設站 fallback。開窗只連已保存預設站一次，切站先取消舊傳輸再改 generation，畫面顯示連線、等待、錯誤和重試。
2. 接上原生登入表單、單次登入 reducer、保存／忘記及系統密碼庫。手動登入始終可用。按鈕登入只在精確 PTT 目標與允許畫面生效，密碼、2FA、重複連線處置與異常提示遵照現有政策，使用者明確授權才保存或送出。非 PTT 站可原始操作並使用共同擷取、複製與媒體。
3. 保活沿用 Connection 的唯一計時器，預設開啟、依 session 和已確認前景 UI 的 gate，只在辨識列表／閱讀作用。組字、貼上、選取、編輯、工具、失焦／睡眠及斷線期間暫停。重連與恢復後從新週期開始。

**通過條件**　本機 WSS／SSH peer 驗證 TLS／金鑰錯誤、原始 bytes、80×24、手動與按鈕登入、取消等待、分段提示、晚到結果、切站、重連及 Close。舊 session 零重播，未知／非 PTT 畫面零 PTT 自動命令。保活在合成時鐘與原生失焦測試核對。三種 OS vault 用合成帳密通過保存、移除、不可用與兩次啟動。本機測試用合成 CA／SSH host key 驗證傳輸，正式信任策略沿用原值。實站登入與帳密交互另由授權人工案例驗證。 [站點](sites.md) · [登入](login.md) · [保活](keepalive.md)

## S3 鍵盤、滑鼠、選單與擷取

1. 把既有按鍵優先序移到 typed 路由。modal／選單、IME、搜尋或編輯元件、本機選取與導覽、Vim、站端 raw 各用明確狀態，保留 repeat、按下／放開、Vim Normal／輸入／原生一次、計數及進板繼續。保留原始 PTT 控制鍵，從同一既有 writer 發送。
2. Rust 用已確認的 geometry 判定 cell／行、連結、圖片、捲軸、浮層及四周留白。使用按下／放開同一 lease、原生 pointer capture、拖曳閾值與撤銷規則。列表可點整行開文，左緣返回，前後翻頁，首尾與狀態列操作，既有藍金提示保持 ANSI 對比並支援減少動態效果。站端滑鼠模式維持 SGR press／release／wheel 的既有能力、回壓與取消，Shift 保留本機選單。
3. 以既有 `selection` 產生純文字／ANSI（含 SGR 66）及完整 URL／OSC 8 操作。右鍵依目前已確認的命中分流圖片、連結、ID、選取、編輯器、閱讀背景及浮球；選單外點消費同一手勢。建立原生鍵盤可操作的右鍵選單、浮球扇形分類、快捷鍵與對話框焦點復原。
4. PNG 從同一 Rust renderer 在 worker 產生。整頁固定 960×576、選定連續行為 960px 寬且最多 24 行，可存檔或寫剪貼簿，排除游標、反白、圖片與 UI。選取／匯出／圖片的 lease 在產生、選檔回覆與 OS 寫入前重驗，只有明確操作才開啟 picker 或存取剪貼簿。

**通過條件**　真實原生按鍵、全螢幕外緣、可信滑鼠拖曳、修飾鍵／站端滑鼠、右鍵優先序、浮層焦點、剪貼簿讀回與 PNG 獨立像素驗證通過。失焦、來源更新、resize、切篇或關閉時撤銷舊操作，peer 不收到第二次手勢或被攔截的操作。 [導航](navigation.md) · [站端滑鼠](station-mouse.md) · [Vim](vim-browsing.md) · [右鍵](context-menu.md) · [連結](links.md)

## S4 文章、搜尋、推文與收藏

1. 從既有 Rust `navigation_job`、`ArticleCache`、文章更新及書籤 owner 接入快速切板、最近看板、AID／URL 快速前往和文章列表。站端任務需逐頁核對回覆，切站／切篇及手動操作撤銷舊任務。收藏保留從目前文章及列表建立、管理、前往、完整快取可離線開啟。
2. 終端機模式保留原站端逐頁。網頁模式是 Slint 原生長文 viewport，使用 Rust 現有 cache 合併、折行重疊及尾頁確認。虛擬化可視列與上下緩衝、固定頁尾及媒體間隙，保持文字列＋列內偏移的閱讀錨點。沿用 4,096 列／32 MiB 等既有上限、分段接續、取消／繼續和快取完整性。取頁沿用單命令與既有 deadline，接頁與繪製解耦。PageUp／PageDown 一行銜接，保留 j／k、Home／End、空白、滾輪、未知邊緣補頁、首尾方向鍵切篇的開關與防 repeat 規則。
3. 對已確認投影建立有界搜尋索引，只在有查詢時建立並於查詢時合併重算，捲動沿用索引。保留 Cmd/Ctrl+F、命中高亮、上一筆／下一筆與 IME 優先，站端 `/` 繼續送站端。Rust 過濾和推文投影保留黑白名單、兩種遮罩、文字名單匯入、作者欄、連推合併、同 ID 高亮／只看／恢復、原 PO／自己側標及快速定位。查詢、高亮和遮罩不讓原文或被遮內容從剪貼簿、媒體、PNG 或 hit test 旁路出現。
4. 推文更新預設關閉，限明確啟用的目前文章。沿用既有 5 秒週期、尾段比對、未讀提示及跳底，保留本機閱讀錨點、媒體及失敗後「顯示站端畫面」復原。暫停、取消與晚到回覆維持原 scope。

**通過條件**　重跑 `full_load`、`long_load`、`terminal_pages`、跨段上限、晚到／停滯、切篇、`inline_pages` 的無圖基準、收藏重開與推文更新。獨立核對每段首尾、折行、顏色、兩版 peer bytes、無意外翻頁、末列可達、搜尋焦點、推文更新失敗的復原路徑及隔離後的擷取。 [文章閱讀](article-reading.md) · [快速切板](quick-boards.md) · [指定文章與收藏](article-navigation.md) · [內容過濾](content-filters.md)

## S5 圖片、GIF 與原生影片

1. 沿用 Rust URL 分類、HTTP 下載、redirect／TLS 驗證、大小／尺寸、cache、request／generation、展圖 batch 與取消。解碼放在可 join 的 worker，持有一份經驗證原始 bytes，首幀、畫布與解碼中的中間資料分開計算上限。先用已鎖定的 `image` 解碼支援的格式，為未覆蓋的動畫格式依本機 fixture 驗證 decoder 與 disposal／blend。候選只能支援既有產品接受且有明確錯誤回退的範圍。
2. 實作 hover 跟隨／右上／關閉、升級完整預覽免重抓、等比例 fit、游標錨點滾輪 1%–800%、拖曳與 capture 撤銷。圖片選單保留原尺寸複製、另存原始格式、複製網址與外開。HTTP 404／超限／逾時／不支援要可見且可依既有政策重試。YouTube、GIFV 和非直連頁沿原規則外開。
3. 網頁模式用原有的 link index／已確認投影和文章作用域，維持網址末端逐張箭頭、短 Enter 依可視順序開圖／等寬、長按 500 ms 全開、去重及讀取錨點。自動開圖預設關閉，明確啟用時按文章與推文順序加入同一批次。終端機模式維持 hover／預覽。每段維持最多 32 URL、32 MiB payload、16M 首幀像素等既有預算。
4. 做一個共用動圖播放 owner，依來源 frame delay 排程、保留首幀及少量有界畫布，避免預先解出全部幀。文章內至多兩張中央可視 GIF，完整預覽至多一張並暫停文章動畫；保留手動停播、回首幀、可視／焦點暫停、重開、資源上限及取消語義。以實際多圖峰值再決定快取與背壓。
5. MP4／WebM／OGV 需要原生內嵌播放、音訊、播放／暫停／進度控制與外開 fallback。S1 先驗 `libmpv` Render API 作候選，以可取消的串流及有界 video buffer 接 Slint，控制與解碼在各自 owner，Slint 主執行緒只消費目前畫面。以實際包大小、runtime 依賴、授權、HTTP range／跳轉政策及各平台可用 codec 作選型；若不合格，再驗平台原生播放 adapter。以 Dioxus 在各平台實際可播的格式為驗收下限。開啟預覽先呈現可播放狀態，沿用不自動播放及播放失敗可立即外開的契約。 [現有媒體契約](media.md) · [libmpv Render API](https://github.com/mpv-player/mpv/blob/master/include/mpv/render.h)

**通過條件**　本機 HTTP 與原生畫面核對自動開圖開／關、正文＋推文、單張／全開、等寬、跨頁歷史內容、hover 零重抓、原圖／原始檔、GIF 首幀和可視暫停、各平台影片及取消。驗證切篇／關窗後網路、解碼、播放、GPU 影像與 timers 都釋放，舊工作無法復活。

## S6 本機撰寫、草稿及站端上色

1. 沿用 PTT Ctrl+P、分類及標題流程。由既有 `editor_source`、Connection 和 scope 完整讀取站端範本、ANSI、畫面外文字及末尾空行。進入前依站點／帳號／板名確認身分與可恢復草稿。列表入口維持管理已存草稿。
2. Rust 持有單一原始文件、undo／redo 交易與文字偏移對應。維護原始 UTF-8、完整 SGR token、可見文字及 cell 的映射，以既有安全 grammar 和 Screen 投影產生彩色／隱藏色碼／原文三種可編輯視圖。原生輸入元件處理 caret、IME 候選及提交。虛擬化 79×23 可視格線與長文捲動，輸入更新受影響的列和仍受 ANSI 樣式延續影響的後續列。標題、剪貼簿、移動、選取與編輯歷史共用同一交易狀態。
3. 保留字元／跨行、整行和 ANSI 矩形選取，完整雙欄字不切半。浮動 16 前景／8 背景色票對當前 revision 一次套用並可 undo。切換顯示不變更原文、尾空行、未知或半成品色碼。站端編輯器框選上色走既有 `editor_color`，驗證來源和站端模式後只插入受控 SGR。
4. 沿用草稿 store 的 2 秒 debounce、連續保存最短 5 秒、IME 不保存半成品、失焦／退出／明確保存 flush，以及一筆在途和一份合併後快照。同稿衝突保留獨立副本，不同稿不互蓋。帶回需再次明確確認，依既有 worker 重讀並比較原範本、核對空白文件、一次送正文、核對末行／欄與模式。保留本機稿，由使用者在站端檢查與發表。

**通過條件**　原生 IME 和真實選取／拖曳，中文、ANSI、一般與矩形上色、undo／redo、三種模式原文等同。跑 `drafts`／`drafts_restart`／`editor_color` 等效探針，合成 peer 核對 79 欄／byte 限制、範本變動拒絕、分段等待、取消與部分寫入錯誤、重開恢復和零發表命令。兩程序同稿／不同稿、磁碟失敗及強制終止後恢復分開驗證。 [草稿契約](drafts.md) · [站端上色](context-menu.md)

## S7 設定、資料相容與系統整合

1. 接上現有設定分類、中文／英文搜尋與 Fluent。完整移植站點、字型清單／官方來源、傳統字型組合、預覽和重設、可攜 profile、語言、文章模式與首尾、hover／自動開圖、浮球透明度／可見／右鍵工具、保活／Vim／站端滑鼠對應入口。依各設定現有保存範圍處理。保留保存成功後生效、錯誤保留草稿、最新草稿在關窗時 drain。通知沿用成功淡出、錯誤及斷線保持可見。
2. 沿用現有 store 的型別、路徑決策、版本、備份、原子寫入和 OS 鎖。研究版使用明確的隔離 namespace。正式候選用複本測 `settings.json`／`sites.json`／`filters.json`、`bookmarks.json` 含離線資料、`recent-boards.json`、`drafts.json` 和原有 display profile；設定涵蓋 v1–v6，驗收現行公開版及最後一個受支援 Dioxus 基準的資料。讀舊資料先在記憶體解析，驗證重開和一次 Slint 寫入後再由指定回退版 Dioxus 讀寫。文件毀損、鎖逾時、容量、不同欄位合併、同欄拒絕、清空屏障及草稿衝突副本皆有回歸。

   已驗的完整新欄位回退基準為 `5ea9ad6`。公開版 v0.4.1（`e304be2`）尚無 `auto_expand_images`，會拒讀含該欄位的設定；其支援的舊格式可以往返，但不能把含新欄位的資料直接交回此公開版。保留拒讀且不改檔反例，不靠刪除欄位宣稱相容。headless 生產 store 的28案不替代實際前端、產品資料路徑、vault 及平台安裝回退；重跑方式及範圍見 [三版本資料往返](slint-prototype.md#s7-三版本生產-store-往返與回退界線2026-11-09)。
3. 合成帳密用隔離 vault 測 macOS Keychain、Windows Credential Manager、Linux Secret Service 的舊格式、重啟、刪除與拒絕。確認候選切換產品 ID、正式路徑、既有 service 與帳密讀取後，簽名變化是否需要重新授權。診斷沿用有界 JSONL、每次啟動隔離與資料遮蔽。保留更新檢查、錯誤、版本比對、下載外開及第三方授權介面。
4. 核對 native menu、IME 焦點、clipboard、檔案 picker、外開、縮放／全螢幕、Tab 操作與可及性。原型 AX 未找到輸入欄，須用平台 accessibility inspector 或 screen reader 重新驗證必要控制項、語意標籤與焦點復原。

**通過條件**　合成舊資料、副本與雙版本往返測試可重現，舊版在讀取後仍能做正常修改。研究 App 零 production store／vault 存取。兩前端並行的合併測試只使用遵守同一鎖契約的基準版本。 [設定](preferences-and-profiles.md) · [多開](multi-instance.md) · [語言](i18n.md) · [診斷](diagnostics.md) · [更新與發佈](releases.md)

## S8 候選打包、平台驗收與效能

### 平台與 WebView 驗收

| 目標 | 發佈格式與原生驗收 |
| --- | --- |
| Windows 10／11 x64 | 安裝版與可攜版。既有 Dioxus→Slint 升級、移除、離線安裝、原生 CredMan、注音、細明體、多螢幕 DPI、低階 GPU 及軟體 renderer。乾淨 VM 缺 WebView2 runtime 時仍能安裝和完成終端／媒體案例；記錄模組／子程序及 installer 實際行為，Slint 包不帶 bootstrapper 或 WebView2 prerequisite。 |
| macOS arm64／x64 | 各自 DMG＋ZIP、原生安裝／啟動、簽名與 Keychain、Retina、威注音／系統快捷鍵和全螢幕。核對 Apple Silicon、Intel 的實際證據，Rosetta 只算補充。以 dyld trace 區分產品連結、OS 間接載入、WebView 實例及 browser child。原型已見 WritingToolsUI 載入 WebKit，當前「執行期零 WebKit 映射」未達成。候選必須能證明內容與互動走原生路徑，若 OS 仍載入框架，由使用者決定此差異是否符合平替要求。 |
| Linux x64 | Ubuntu 24.04 的 `.deb`＋`.tar.gz`，如果待整合基準有新增格式就一併驗。乾淨環境移除 WebKitGTK 依賴後跑安裝、Wayland／X11、Fcitx5 或 IBus、字型 fallback、clipboard owner 與 Secret Service。核對 apt package metadata、動態連結、缺服務及無 GPU 的失敗或替代路徑。 |

用獨立驗證包或既有 workflow 的 verify 路徑產出四架構附件。保留原產品識別對照、SHA-256、依賴與授權清單、安裝及升級／降版操作紀錄。平台矩陣分列建置、安裝、實際視窗、原生輸入與實站。受影響平台須有自己的證據。正式候選功能門檻包括已驗證的原生播放器與編輯器。經授權的實站案例單列 WSS／SSH 登入、進板閱讀、斷線重連及草稿帶回後停在編輯器；帳密由使用者操作，結果不混入 loopback gate。 [Windows 安裝](windows-installer.md) · [macOS 打包](macos-release.txt) · [Linux 打包](linux-package.txt) · [發佈流程](releases.md)

### 同負載效能門檻

- **固定條件**　同一台機器、相同 release 設定與等效功能、視窗大小、DPI、字型、焦點狀態、loopback replay／HTTP 資料和 driver；記錄 Slint renderer 與基準 WebView 版本。先在低階 Windows 登記機型，再測其餘目標。冷／暖分開，各至少 10 次啟動、每組至少 100 次輸入或翻頁；交錯兩版順序，記錄冷啟動定義、原始數據、誤差和離群原因。
- **時間與正確性**　啟動到第一個可見且可輸入 frame、原生輸入到畫面呈現、快取翻頁／長文捲動的 p50／p95 及掉幀。站端取頁和圖片下載時間分列。對兩版採相同觀測點，不能拿 Rust 交付 bitmap 的時間對比另一版螢幕呈現。相同劇本核對畫面、實際 bytes、HTTP 和資源清理結果。
- **資源**　比較閒置、長文快速捲動、兩張 GIF、影片、草稿編輯、多開與反覆切篇／開關後的程序族 CPU、峰值及穩定記憶體。Dioxus 納入 WebView 子程序，Slint 納入播放器或任何子程序。各平台固定一種主記憶體指標，另列 RSS／GPU。取樣至少覆蓋穩定閒置與回收後狀態，核對無殘留連線、decoder、timer 或累積配置。
- **建議判定**　低階 Windows 的暖機操作 p95 或閒置記憶體至少一項改善 20%。其餘主要指標在各平台無超過已登記波動且可重複的 10% 以上退步。正確性、功能完整、資料和清理為獨立必要條件。量測差異落在波動內則重測；若只達成 WebView2 移除，將該收益及效能差異分列，由使用者決定是否接受。

**通過條件**　完整矩陣、原始資料、平台包、具體 renderer 選擇、所有仍待人工項目與可重跑命令齊備。每項標 `passed`、`failed` 或 `not_ready`，附 commit／dirty、platform／arch、fixture digest、實際輸入、預期／結果和證據。跨平台建置、合成事件或計數各只證明自己的範圍。只要必要項未通過，維持候選狀態，保留該項的最小重現。

## S9 取得核可後切換與回退

1. 以 S8 的結果請使用者決定。提供候選 App、差異、效能、macOS OS 框架載入與資料回退演練。確認採用後，以當時 `develop` 作整合基準，將正式 `pttstella` 指向 Slint，讓候選套用既有產品 ID、資料目錄、密碼庫與升級路徑。切換提交與刪除舊前端的提交分開，便於先回退入口。
2. 做一次完整原地升級與回退演練。關閉寫入中的舊程序，備份各 store 主檔／備份及版本證據，保存前一個可安裝 Dioxus 產物。啟動 Slint、讀舊資料、做代表性保存與密碼庫存取，再換回 Dioxus，核對資料、連線和可攜設定。格式若確實需擴充，先設計雙版可讀方案並驗證，避免靠丟棄升級後的新資料回退。
3. 切換驗收通過後，再從正式入口與包移除 Dioxus／Wry／WebView 相關依賴、舊 JS／CSS／CodeMirror 資產、WebView2 bootstrapper 和 Linux WebKitGTK package requirement。已移植行為的舊 JS gate 由對應 Rust／Slint／原生 gate 接手。保留基準 Git ref、安裝包、fixture 和對照證據；回退走已驗證的前一版或可逆入口提交。
4. 依 repo 流程更新 README、安裝說明、測試及 release workflow，重跑 core／native／安裝與回退。正式版號、release PR、tag 與發佈依使用者的該版授權。

## 驗證與交接紀錄

2026-11-11：已定位並修復前筆shadow漏畫：原生探針於window repaint前的toolkit snapshot消耗共用dirty history；補入相同順序的像素反例後，原Skia確實失敗。專案保存可重現的Skia1.18.1 path patch（原25檔／授權核對、只改兩檔及新增shadow helper），快照後保留window全畫，平時macOS software沿用前幀做partial repaint。最終原生首次頁尾、雙GIF、全螢幕、Settings及Close通過，兩段十秒CPU各約1.10秒，僅為校準。兩配置118組像素、BIN103／1、core392／4、shadow2案、兩配置all-target Clippy及Python178通過；本地依賴授權collector亦已補齊原license與patch來源。詳見[快照修補紀錄](slint-macos-measurement.md#2026-11-11-快照損耗待重畫狀態的修補)。沿用同一App／Cargo cache；舊shadow-only拒用紀錄不抹除。診斷還原mode已補回0755並實際核對原版啟退；四平台下載尚未替換，完整S8及S9狀態不變。

2026-11-11：新增同repo的 `slint-renderer-audit` 明確診斷feature及正常Cargo整合測試，兩個獨立Skia renderer消費同一生產TerminalWindow／ArticleViewport，每種配置118組full／retained畫面逐像素一致。涵蓋展圖載入／metadata與repeater退休、100次實際partial換幀、頁尾／控制列、focus、尺寸及同physical buffer的倍率往返，並有獨立指定像素色塊防空測試。一般Slint／native-player皆通過，但尚未重現上一筆原生漏畫；不以此採用shadow或宣稱OS呈現／100次原生輸入／S8通過。方法及命令見[量測紀錄](slint-macos-measurement.md#2026-11-11-整窗重畫原因與未採用的-shadow-實驗)與[交付流程](../CONTRIBUTING.md)。沿用固定Cargo cache、無新repo／App。

2026-11-11：同一App及固定Cargo cache確認macOS software整窗重畫根因：softbuffer0.4.8每次新buffer／age0，使Skia1.18.1實際192筆repaint均100%。試作private shadow後暖機通常21.29%，但首次展圖有頁尾像素缺失，故拒用並完整還原Cargo來源／lock、已驗native及launcher；沒有改四平台包。精確原／實驗binary、來源archive、正常Close／HTTP、失敗畫面觀察及清理在[macOS量測紀錄](slint-macos-measurement.md#2026-11-11-整窗重畫原因與未採用的-shadow-實驗)。保留固定中間檔，移除未採用來源副本和重複baseline，不再逐批full clean。缺失原因與完整S8仍待驗，不能以重畫面積縮小宣稱效能／正確性通過；S9未執行。

每階段使用 [CONTRIBUTING](../CONTRIBUTING.md) 的對應 gate，再加 Slint 專屬 unit／Clippy、共用 loopback、真實視窗和各平台 package gate。舊前端仍用既有 JavaScript、Rust、desktop probes 作對照。原生 IME、裝置、vault 或實站缺項各自登記。證據保存在 worktree 的 `.local/verification/`，保留候選包、雜湊、最小重現及原始資料。交接記錄完成階段、下一個入口、缺 runner／授權、實際命令和證據位置。結案先確認 resolved path、程序與所有權，再清理自己被取代的產物並記錄空間前後。 [開發與清理流程](development-harness.md)


2026-11-07：文章尋找已接入共用 Rust 索引、Slint 原生搜尋列與 Dioxus typed adapter。macOS arm64 兩種閱讀模式的合成完整 App 及原生 OS 注音／貼上／焦點已取得分立證據；詳細範圍、負向紀錄與仍待驗項目見 [原型紀錄](slint-prototype.md#s4-文章尋找共用-rust-索引與原生搜尋列)。S4 其他移植和 S5–S8 繼續，完整 goal 保持 active。

2026-11-08：S5 主視窗已有原生圖片／GIF 預覽、原尺寸複製／原始檔另存及圖片右鍵選單；影片 runtime 封裝仍是獨立播放器實驗，尚未整合主入口。macOS arm64 的實際輸出、HTTP 計數、焦點與 owner 清理分開記在 [原型紀錄](slint-prototype.md)。hover、文內展圖、動畫語義、編輯器、跨平台原生、回退及效能缺項繼續，沒有將局部成果當成 S1／S5 或完整候選版通過。

2026-11-08：原生 hover 的三種偏好、350ms 停留與同資源升級已接入，關閉模式的一般連結提示及設定衝突亦有回歸。macOS 原生操作與 toolkit 的被動焦點／四角定位證據範圍分列於 [原型紀錄](slint-prototype.md)。文內展圖、媒體／編輯器整合及其餘 S0–S8 缺項持續；目前沒有跨平台或效能結案。

2026-11-08：既有展圖 batch 已移到共用 Rust，Slint 長文 viewport 已接入有界圖片間隙座標及錨點；實際文內圖片與操作仍待接線。完整預覽的增量播放邏輯已抽為後續共用的原生 owner。合成長文的 500ms＋150ms 逐頁延遲保留作負向驗證，另有立即回覆診斷及共用有界時鐘；初步數據不代表 S8 效能通過。詳細證據與範圍見 [原型紀錄](slint-prototype.md#s5-展圖批次圖片間隙與播放排程2026-11-08)。完整 S0–S8 goal 維持 active，S9 未切換或發佈。

2026-11-08：Slint 文內單張圖片、原圖預覽升級與可視 GIF 已接進既有 Rust batch／同一媒體 actor。完整元件及 macOS 原生操作分立驗證，保留版面通知、雙 GIF 恢復及已開箭頭穿透的負向案例和修正。短／長 Enter、等寬、自動展圖設定、圖片選單及其餘 S0–S8 項目繼續；多圖峰值和跨平台效能未通過，S9 未切換／發佈。詳細範圍見 [原型紀錄](slint-prototype.md#s5-原生文內圖片與可視-gif2026-11-08)。

2026-11-08：短／長 Enter、可見圖片順序與等寬已接入同一原生圖片 owner；超長等寬圖先以來源像素裁切再交 Slint。完整元件 short／long／pointer 及 macOS 原生長按、放開、短按等寬分開取得證據；細節、負向案例和仍待原生／資源峰值的範圍見 [原型紀錄](slint-prototype.md#enter-展圖與等寬裁切2026-11-08)。自動展圖設定、圖片選單及其餘 S0–S8 繼續，整體 goal 維持 active，S9 未切換／發佈。

2026-11-08：自動展圖已接上共用設定保存、失敗重試及同一 actor 的動態取消；共用 `223b51c` 加入自動 batch 來源標記與成功解碼後的既有 LRU 准入。完整元件、真實 stalled HTTP、macOS 原生勾選及明確 All 保留分開驗證，來源／快取政策和重啟未驗範圍見 [原型紀錄](slint-prototype.md#自動展圖設定與原圖快取2026-11-08)。原型工具列不代表 S7 設定中心完成；圖片選單及其餘 S0–S8 缺項繼續，沒有切換入口／發佈。

2026-11-08：文內圖片四項選單及背景選單 All 已接入共用 policy／輸出 owner，並補上原生 Shift 狀態、輸出後鍵盤恢復及可視 model 的逐列更新；先前黑圖判定因工具預覽與原始像素不一致而撤回；同版六個原生程序、原始 OS 擷取及獨立解碼器已複核文內圖片與輸出。十一組完整元件及分 hash 的 macOS 原生輸出、選檔和從零全開證據見 [原型紀錄](slint-prototype.md#文內圖片選單共用輸出及原生驗證2026-11-08)。影片／編輯器、原型來源提交序列、S7 完整操作、四架構／回退及效能仍待完成，完整 goal 維持 active，S9 未切換／發佈。

2026-11-08：影片 control／GL ABI 已抽為主視窗與獨立 probe 共用 session；有界 Rust relay 共用圖片 TLS／redirect 政策，主視窗接入 GPU FBO 影片區、明確播放、暫停／進度與退場。三格式、停滯、播放清單拒絕、macOS 原生輸入及各輪 binary 範圍見 [原型紀錄](slint-prototype.md#s5-共用影片串流與主視窗播放2026-11-08)。這是 S5 的本機整合子項；音訊人工、完整 runtime／其他平台、GL 恢復及 S6–S8 等缺項繼續，沒有平替結案或 S9 切換／發佈。

2026-11-08：S6 已有保留完整原文的 Rust 文件／歷史、逐列 rendition 快取及兩前端共用保存 owner；SGR 66 及既有 Canvas 直接消費者修正已分開提交，實際 Dioxus 的保存／新程序恢復／合成帶回取得分立證據。詳細範圍見 [原型紀錄](slint-prototype.md#s6-原文文件與共用草稿2026-11-08)。Slint 編輯 UI、原生 IME、排程與帶回接線仍待完成，沒有將核心或舊 host 驗證當成 S6 原生通過；S0–S8 goal 保持 active。

2026-11-08：Slint 主視窗已接上 79×23 原文編輯區、同一 Document／歷史、原生 IME／實體快捷鍵、可見列繪圖、保存 actor 與既有範本／帶回流程。嚴格 loopback 的 fresh／restart、macOS 實際注音取消／單次提交、貼上、標題、拖曳上色及關窗保存分開取得證據；四項負向案例與修正、候選清單未觀測及仍待驗項目見 [原型紀錄](slint-prototype.md#s6-原生編輯主視窗初版2026-11-08)。浮動色票／選取入口、取消／失敗、完整站端上色、S7–S8、跨平台／回退／效能及原型提交序列繼續；未將初版整合當成 S6 完成，完整 goal 保持 active。

2026-11-08：本機編輯器已新增浮動 16／8 色票、文字／整行／矩形選取與可見取消；修正真實上色後的焦點及長文組字定位。五組完整元件、macOS 原生操作、獨立像素與保存原文的範圍及負向案例見 [原型紀錄](slint-prototype.md#s6-色票明確選取與可見取消2026-11-08)。矩形選取歷史、站端上色、部分失敗及其餘 S6–S8 繼續；合成長文刻意延遲保留，完整 goal 維持 active，沒有切換入口或發佈。

2026-11-08：選區形狀已納入同一 Document 歷史，修正矩形 undo 後再輸入會刪到未選文字的缺陷。共用核心 `93657ea`、完整元件與 macOS 真實拖曳／undo／redo／上色／貼上／保存的範圍見 [原型紀錄](slint-prototype.md#s6-選區與正文一起還原2026-11-08)。站端上色、部分失敗及其他 S6–S8 缺項繼續，完整 goal 維持 active，沒有切換入口或發佈。

2026-11-08：原生站端框選上色已接上同一 Output 選區與 Connection Flow，提供預覽、明確套用、可見取消及部分寫入警示。四組完整元件、三個 macOS 原生程序、共用草稿回歸與嚴格 bytes／模式／像素的範圍見 [原型紀錄](slint-prototype.md#s6-原生站端框選上色2026-11-08)。負向 peer 以 `8f0d7ad` 提交；本機草稿部分錯誤、原生邊界、S7–S8 及來源提交序列繼續，完整 goal 維持 active，沒有切換入口或發佈。

2026-11-08：本機草稿新增範本變動、改標題／清空／正文部分執行、TCP 中斷及讀取／帶回中關窗的嚴格獨立 peer，以 `a63f5ff` 提交。七組完整元件、五組既有草稿回歸及三個 macOS 原生程序核對本機稿保留、不重送、關窗與離線保存；詳細範圍見 [原型紀錄](slint-prototype.md#s6-草稿部分失敗斷線與工作中關窗2026-11-08)。這些是指定步驟的故障注入，不代表站端已接收 bytes 可以回滾或每個網路切分都已驗證。磁碟失敗／強制終止／多程序、原生 IME 邊界、S7–S8 及來源提交序列繼續，完整 goal 維持 active，沒有切換入口或發佈。

2026-11-08：Slint 完整主視窗新增備份寫入失敗／損壞資料保留與重試、確認保存後強制終止／新程序恢復，以及兩程序同稿衝突副本／不同稿合併驗證。嚴格只讀範本站台入口以 `d0a3f67` 提交。macOS 第二輪原生錯誤提示、保留正文與重試逐 byte 通過，但第一輪曾觀察到末字消失，尚無原因，不列為已修復；證據與界線見 [原型紀錄](slint-prototype.md#s6-草稿儲存故障程序終止與多開2026-11-08)。原生輸入異常、IME／可及性、S7–S8 及來源提交序列繼續，完整 goal 維持 active，沒有切換入口或發佈。

2026-11-08：共用有效偏好／編輯 baseline／載入警告已由 Dioxus adapter 抽至 Rust，以 `19afa3f` 提交；Slint 媒體設定接入同一型別，啟動時移交原 Store，保留明確恢復損壞檔所需的讀取觀測。兩版 release／Clippy、382 組 JS、三組 Slint 完整元件及 macOS 實際 Dioxus 保存／失敗／重試／重開／PNG／Close 的範圍見 [原型紀錄](slint-prototype.md#s7-共用偏好狀態與啟動-store-交接2026-11-08)。Slint 完整設定中心、原生 IME 邊界及 S8 繼續；沒有將狀態共用當成 S7 或平替完成。使用者確認末字異常期間有其他操作，但不足以確認原因；本輪 IME 嘗試未收到 preedit，明列 not_ready。


2026-11-08：Slint 原生設定視窗初版已接入單一偏好／檔案 owner、300ms最新有效修改、成功後生效、重試與關窗交接；四個完整元件及 macOS 原生設定檔匯出／匯入／重開通過。快捷鍵、picker取消及合成 seed 的負向案例與修正分 hash 保留；OS Quit 清理、完整管理／語言與各 consumer 原生同步等仍待完成。範圍見 [原型紀錄](slint-prototype.md#s7-原生設定視窗與保存交接2026-11-08)。完整 S0–S8 goal 維持 active，沒有將設定視窗初版當成 S7 或平替完成，S9 未切換／發佈。

2026-11-08：macOS原生App選單已接入正常Rust退出，指定的設定待存修改／草稿貼入後⌘Q逐檔與嚴格Close通過；啟動文章首尾偏好亦接入第一份投影。Documents路徑的另一個合成草稿讀取仍停在OS open，保留原程序及sample，沒有列為通過。Dock／modal退出、原生輸入邊界、完整S7與S8繼續；精確hash、負向及範圍見[原型紀錄](slint-prototype.md#s7-原生-app-選單與正常退出2026-11-08)。完整S0–S8 goal保持active，S9未切換／發佈。

2026-11-08：主視窗、編輯器、搜尋與既有登入／媒體／色票標籤接入同一語言refresh，共用呈現helper以`4578b4f`提交。四個完整App及macOS原生下拉中英往返分立驗證內容／查詢／輸出選項保留；前輪讀取停滯程序以probe timeout結束，仍屬失敗。範圍與負向見[原型紀錄](slint-prototype.md#s7-介面語言同步2026-11-08)。完整S0–S8繼續，完整管理／搜尋、原生邊界與S8未結案，S9未切換／發佈。

2026-11-08：共用站點編輯狀態以`3e09e7f`提交，Slint已接上原生站點／編碼／最愛／獨立預設、成功後生效及寫入交接。指定完整元件與macOS保存／重開／主視窗輸入隔離分hash驗證；有效切站與其餘S2／S7仍待完整App驗收。範圍見[原型紀錄](slint-prototype.md#s2s7-原生站點管理2026-11-08)。完整S0–S8目標持續，S9未切換／發佈。

2026-11-08：以`e00088f`新增共用獨立站點peer，三個完整App驗證連續切站、斷線後切站與取消未完成握手；macOS實際按鍵／貼上另核對精確GBK／UAO bytes及三站Close。範圍、hash、負向及剩餘S2項目見[原型紀錄](slint-prototype.md#s2-本機切站與雙編碼出站2026-11-08)。完整S0–S8目標持續，S9未切換／發佈。

2026-11-08：共用desktop-services、輸入交易、媒體HTTP／relay／可選libmpv及第一個Slint終端／文章前端已分成八筆可獨立編譯提交。原生最新完整WIP保留，沒有降版或切換正式入口；確切tree、各筆gate及後續提交範圍見[原型紀錄](slint-prototype.md#s0-可獨立編譯的提交序列2026-11-08)。完整S0–S8持續，S9未切換／發佈。

2026-11-08：文章操作／尋找、圖片／hover、文內展圖與主視窗影片UI已接續分成四筆可編譯提交；影片探針補上每個錯誤出口的GL清理與原訊息保留，並分列失焦與Agent探針插入錯誤的負向。各筆確切來源、GPU／HTTP／Close及剩餘退出邊界見[原型紀錄](slint-prototype.md#s4s5-前端提交與影片失敗清理2026-11-08)。完整S0–S8繼續，S9未切換／發佈。


2026-11-08：現有完整 Slint 前端、原文編輯／色票／失敗儲存、設定／語言／站點、獨立播放器與探針打包工具已接續提交。公開草稿驗收 driver 在 macOS arm64 跑完8個子程序、7個正常 Close、1個指定保存邊界 kill 和2組獨立磁碟核對；不代表原生 IME、任意斷電或其他平台通過。各提交及重跑方式見[原型紀錄](slint-prototype.md#s6s7-完整前端提交與可重現草稿驗收2026-11-08)。產品 active-video 退出、管理／完整搜尋及四架構／回退／效能等仍未完成；S0–S8 goal active，S9 未切換／發佈。


2026-11-08：`1b2bb96` 將正常 App Quit／根視窗 Close 集中到單一退出 owner，釋放根視窗／GL 後保留事件迴圈等待影片工作結束。四個完整程序涵蓋未播放、播放中、停滯與最小化；macOS 實際 ⌘Q／關窗另驗，並重跑設定待存、文章取消、草稿工作中關閉與8程序儲存案例。負向、各 hash、清理和仍待驗範圍見[原型紀錄](slint-prototype.md#s5s7-正常影片退出與根視窗所有權2026-11-08)。完整 S0–S8 持續，S9 未切換／發佈。


2026-11-08：`bae1852` 共用24項固定搜尋目錄與有界 NFKC 索引，`8195c7c` 對齊 UTF-16 邊界，`b2af0a4` 接上23個目前可用的原生目標、可取消焦點排程、暫時操作開關與具名設定檔。中英文46次完整元件定位、三組設定／退出回歸，以及 macOS 兩版實際搜尋／聚焦分 hash 驗證；Slint另驗原生 picker 匯出／讀回名稱，Dioxus另驗PNG與Close。名單管理尚未移植，因此該搜尋目標仍待接線；其餘 S0–S8 未結案。詳細範圍、負向與清理見[原型紀錄](slint-prototype.md#s7-共用搜尋目錄原生定位與具名設定檔2026-11-08)。

2026-11-08：追加 `a1dee9a`，用先失敗、後通過的完整視窗案例修正一般搜尋欄 focus 被舊定位 timer 拉走；既有明確搜尋導覽取消仍保留。最新普通 CLI／gate 與 OS 檢查點的不同 hash 分列於原型紀錄，未外推至跨平台或整體 S8。

2026-11-08：黑白名單原生表單已接入共用規則、匯入預覽及原子保存；設定補齊八分類與24項搜尋定位。終端／網頁的合成完整元件、取消與晚到錯誤、備份失敗、壞檔唯讀及退出時在途保存已有分立證據；macOS arm64 實際視窗的搜尋、繁中貼上、系統選檔與保存亦已驗證。這不是 OS 注音或跨平台驗收，也未完成帳號右鍵、揭露及書籤管理。詳細範圍見 [原型紀錄](slint-prototype.md)。S0–S8 持續，S9 未切換／發佈。

2026-11-08：帳號右鍵的三策略／移除、列表作者欄、暫時揭露與推文底色標示已由 Slint 接到既有 Rust reducer及名單保存。兩種閱讀模式的完整合成 App 驗證遮罩輸出、舊選單／租約拒絕、保存失敗、連點取消及 Shift選字；網頁模式另驗已解碼圖片／GIF／等寬與閱讀位置保留。此為 toolkit與本機 peer證據，可信 OS手勢／IME、混合 Live與跨平台仍待驗；詳細範圍見 [原型紀錄](slint-prototype.md)。S0–S8持續，S9未執行。


2026-11-09：`e704fc1` 將書籤狀態、離線投影及收藏身分取得移至共用 Rust；`577f927` 接入 Slint 原生書籤管理、23 行離線 viewport、成功後提交、唯讀壞檔／重試和退出時在途保存。完整元件及新程序斷線閱讀核對原副本、當前遮罩、雙色字與零額外按鍵／HTTP；macOS 實際浮球、繁中貼上保存、翻頁、返回與設定交接另列證據。浮球分類的舊 allowlist 及新增狀態／讀取提示已修正。這一輪尚未接上目前／列表文章收藏、前往或最近看板；完整四架構、資料回退及效能也仍待完成。詳細 hash、負向與清理見 [原型紀錄](slint-prototype.md)。S0–S8 goal active，S9 未執行。


2026-11-09：`576f40e` 共用最近看板狀態、revision、待寫合併及清除屏障；`42e86a9` 接入 Slint 快速前往／最近看板、⌘K／Ctrl+K 與書籤前往草稿交接。導覽共用原生單一 receipt 欄位及 Session 工作，切板、AID／URL、立即到達、錯誤／取消及晚到回覆已有完整元件證據；退出另驗一份待收集工作及一份待寫資料均完成。macOS 實際快捷鍵、兩次切板與同板到達、最近清單、雙檔清除及等待中 ⌘Q 分列驗證；沒有將按住 Return 當成 IME 或實際 repeat 事件計數通過。原生目前／列表文章收藏、推文導覽／更新及其餘 S0–S8 缺項繼續，S9 未執行。詳細 hash、範圍與清理見 [原型紀錄](slint-prototype.md)。

2026-11-09：`502cdb7` 接上目前／指定列表文章收藏，沿用共用 Acquisition／FromList、同一 Session 核對及原子封存交易。最終五組 strict peer 核對目前、無標頭、指定列、取消與失焦；macOS 實際浮球收藏、離線讀至第45行及 Quit 有分立證據。新程序已解碼／保存相同副本，但原生重開操作未完成；列表實體右鍵、完整焦點／IME 與其他平台仍待驗，不推定同時操作就是失敗原因。詳細來源、負向紀錄、回歸及清理見 [原型紀錄](slint-prototype.md#s4-目前與列表文章收藏2026-11-09)。S0–S8 goal 持續，S9 尚未切換／整合／發佈。

2026-11-09：Slint 已接入共用 Rust 原PO／確認登入的自己／指定帳號推文索引、原生群組與循環定位、可見側標、背景右鍵入口。完整元件測試核對縮放／捲動、搜尋交接、遮罩／還原、模式與 scope 清理、重繪 ACK 保護和零站端輸入。這批 native bin79項、全 target Clippy、native-player-probe 編譯與全文／輸出／收藏回歸通過；列表收藏一次取消、同 binary 獨立重跑通過，負向紀錄保留。macOS 原生視窗可顯示側標，但 CUA 點擊未取得前景焦點，實體按鈕／鍵盤保持 not_ready；兩次獨立原生重試 CmdQ 的 Close ACK／零額外輸入／HTTP joined 有記錄，不把正常退出替代操作驗收。細節見 [原型紀錄](slint-prototype.md#s4-原生推文定位與身分側標2026-11-09)。S4、S8 及完整 S0–S8 尚未完成，S9 未執行。

推文導覽批次 `40e4749` 驗收後再清理 target/release（Cargo2.7GiB）、上一個收藏 App與冗餘正向影像；target回到12KiB。必要一般配置最新包改為 `.local/verification/slint-reply-navigation/Native-Replies.app`，影片配置與Windows S1 ZIP保留；來源265檔及必要包hash不變。第二份 exact清單與free前後在 `.local/verification/slint-reply-navigation/cleanup.json`，仍依每批規則執行。

2026-11-09：Slint 推文自動更新已接入同一 SurfaceState、5秒 deadline與單一 receipt owner；預設關閉、顯式開啟、未讀提示／跳底、操作暫停、取消／失焦保留原文與圖片及站端復原皆有完整 App證據。與 Dioxus 共用的既有 peer 驗兩次追加、一次未變尾段核對與晚到 Q；三種圖片案例各只下載一次，10個最終 native CLI案例皆通過。macOS 實體長按 Right／Enter、復原按鈕、150%無輔助縮放的站端游標可見及正常關窗另通過；完整更新控制／混合 Live、IME與四架構仍待驗。已補同來源幾何 ACK 的游標捲動，沒有新增更新 timer／sender task。詳細來源／負向／擷圖限制見 [原型紀錄](slint-prototype.md#s4-原生推文更新與站端復原2026-11-09)。S0–S8仍 active；S9未執行。

推文更新批次 `384c899` 驗收後再清理自己的release profile、上一個推文定位App與3個冗餘正向PNG，target回到12KiB；必要一般配置最新包改為 `.local/verification/slint-reply-updates/Native-Updates.app`。清理前後、保留包hash及source267檔不變在同批 `cleanup.json`。接續混合Live時仍須核對保留／復原中的推文側標、底色、尋找與媒體交互及完整可信OS控制；本批三個Live fixture不涵蓋所有角色標記組合。

2026-11-09 清理追補：使用者再次要求先處理磁碟產物，已在確認沒有程序／open files 使用後，以 Cargo 原生 clean 移除 Slint worktree 的 `target/release`（5658 檔、Cargo 宣告2.0GiB；du由2122336KiB降至12KiB）。本次 filesystem available由245599801344增至247608643584 bytes，僅記觀測差額，不全部歸因於本次刪除。混合閱讀探針尚未通過，保留該版完整未提交patch、來源基準、binary hash及失敗報告，沒有把失敗標為驗收。485份來源／資產／文件hash與3份必要交付hash均不變。exact path、lsof及前後記錄在 `.local/verification/slint-artifact-cleanup-20261109-followup/cleanup.json`；其餘聊天產物和未整合worktree保留。後續仍逐批清理，不因goal持續而累積歷代target或App；本次清理後不為驗證清理而重建Rust。

2026-11-09：混合推文角色、指定帳號底色、PNG／GIF與wide／搜尋交互已補完整元件證據，修正取消後側標／本機定位消失、部分編碼頁清掉焦點及暫時viewport夾限覆寫anchor。最終同binary六案及九項回歸通過；Rust共用／兩前端578項、JS383項與Clippy／native-player-probe編譯通過。兩輪更新失焦取消保留原log，同binary獨立重跑通過。macOS實際群組／下一則有部分證據。影像追補：重新讀取原始JPEG／PNG後撤回「正文缺字後恢復」推論；後續座標定位／復原及無自動代按的更新控制另驗。完整OS矩陣仍未完成。完整來源、hash、負向與剩餘範圍見 [原型紀錄](slint-prototype.md#s4-混合推文角色尋找與媒體保留2026-11-09)。S0–S8持續，S9未執行。

混合閱讀修正 `c817538` 驗收後依逐批規則清掉本Slint `target/release`（Cargo宣告12936檔、4.9GiB；target的du由5251928KiB降至12KiB），並移除已被同功能回歸涵蓋的舊 `slint-reply-updates/Native-Updates.app`（du65708KiB）。該App只留2574bytes非執行metadata ZIP；最新一般配置改為 `slint-mixed-reading/Native-Mixed.app`，影片配置與Windows S1 ZIP仍保留。267份src及3份必要交付hash不變，未重建Rust；精確resolved paths、owner、symlink／lsof、保留例外及free前後在 `.local/verification/slint-mixed-reading/cleanup.json`。filesystem available 237879652352→242873319424 bytes只記觀測差額，不全歸因於清理；原生正反畫面、未整合source／worktree及其他聊天產物保留。

2026-11-09：重讀原始JPEG、確認兩份曾被判成不同呈現的PNG逐byte相同並解碼文字像素後，撤回先前「cache／snapshot漏字」推論，沒有為內嵌輸出改產品繪圖。既有binary補驗macOS座標推文定位／150%站端復原，以及完全不代按或合成前景的手動更新／51行／6未讀／查看／暫停51.57秒零新命令／Quit。舊AX與焦點負向保留，完整鍵盤／IME／其他平台不由基本按鈕路徑替代。原始證據與範圍見 [原型紀錄](slint-prototype.md#s4-原始影像複核與手動原生更新2026-11-09)。沒有重建Rust或新增App副本，target仍12KiB；S0–S8持續，S9未執行。

2026-11-09：新增不代按的native-list loopback入口及僅探針使用的持久資料檔選項，macOS實際右鍵第一篇（站端游標起初在第二篇）完成13步指定收藏；新程序直接讀同一檔，實際斷線後開啟副本、翻至45行並在離線視窗Quit。原始45行／80欄／ANSI青帶及全檔hash、零HTTP／零站端閱讀命令、兩程序close與worker回收分開驗證。最終binary11案、Rust457項、Clippy／native-player-probe編譯與fmt通過。原先預建verification目錄被安全拒絕的負向保留。完整scope與原始證據見 [原型紀錄](slint-prototype.md#s4-列表指定收藏與新程序斷線讀回2026-11-09)；其他平台、完整資料回退及候選打包仍未結案，S9未執行。

列表原生驗收批次 `c3c75e6` 完成後，以Cargo原生clean移除本Slint target/release（8385檔、2.7GiB；du原2848280KiB），並清掉上一個 `slint-mixed-reading/Native-Mixed.app`（du65736KiB）及11個被最終同案取代的首輪正向generated目錄。舊App僅留2571bytes非binary metadata ZIP；來源／hash／log、唯一啟動失敗及原始OS擷圖保留。target回到12KiB，最新一般配置測試包改為 `slint-native-list/Native-Bookmarks.app`，同一收藏檔、影片配置與Windows S1 ZIP保留且hash不變，267份src不變。精確刪除路徑、逐檔hash、owner／symlink／lsof及保留例外見 `.local/verification/slint-native-list/cleanup.json`。filesystem available 238569623552→241283104768 bytes只記觀測差額；不全歸因於本批，不重建Rust，也不刪其他聊天產物或未整合worktree。

2026-11-09 即時產物清理：依使用者再次要求，盤點本 Slint worktree 所有 target、歷代測試產物與必要交付，確認軟體影片批次已無程序／open files 使用後，以 Cargo 原生 clean 移除 `.local/verification/slint-video-software/target/release`（5717檔，Cargo宣告2.1GiB；target的du由2242236KiB降至8KiB）。worktree的du由3060352KiB降至818132KiB，filesystem available由234602717184增至236684951552bytes，差額僅為觀測、不全部歸因於本次刪除。496份來源／文件與1192份必要交付／資料hash不變；未提交的CPU影片來源及失敗紀錄保留，只移出一份當前執行檔與Skia授權至同批 `pending-validation/` 供後續驗證，沒有保留整包依賴快取或另外建立App副本。已結束的Rust測試及18組影片案例有結果，但CPU原生呈現／閱讀回歸／Clippy與跨平台仍待驗，不能因清理而稱該功能完成。最新完整影片候選包、已驗收的一般software配置App及唯一Windows S1 ZIP仍有各自用途；canonical Dioxus產物歸屬未確認、未整合Slint來源worktree亦保留。exact paths、逐檔盤點、idle檢查、保留理由及前後空間在 `.local/verification/slint-artifact-cleanup-20261109-now/ledger.json`。往後每批及中途清理依[開發harness](development-harness.md#產物清理與結案)執行，本次未重建Rust。S0–S8 goal持續，S9未執行。

2026-11-09：`4a7db4c`完成明確software compositor與原生CPU影片worker，輸出最高1280×720、單一latest frame／有界wake、沿用原Session及退出所有權；macOS實際四色／seek／Play／Pause／Quit與資源清理有分立證據。接續修正候選無參數仍進離線原型的缺口，網路禁止環境的原生啟動／失敗提示／Quit及資料不變已驗。最終候選18組影片與2組混合案例通過，但閱讀9案中的收藏目前文章出現間歇失敗，CPU及GL重複實驗都保留反例；整體回歸明列failed。下一步追查cold store讀取前後的source／stamp／ACK，不以重跑或更改expected結案。精確binary、參數、驗證範圍、原始失敗與剩餘缺項見[原型紀錄](slint-prototype.md#s2s8-候選無參數啟動與收藏反例2026-11-09)。S0–S8持續，S9未執行。

啟動修正以`6c6a8e1`提交；舊完整GL候選另重現同一收藏失敗。批次結束已清掉自有checks-target/release（Cargo8385檔／2.7GiB）及兩代被取代App／ZIP／DMG與舊待驗執行檔（payload du621580KiB），worktree du770120KiB、所有自有profile已清空；496份來源與1192份必要交付／資料hash不變。保留最新 `slint-startup/package-current`、不同feature的舊software收藏對照與唯一Windows S1 ZIP，沒有因failed回歸留下整包編譯快取。exact paths、保留例外與free觀測233476833280→236733288448bytes見同批 `cleanup/ledger.json`；清理後不重建Rust，完整S0–S8仍未達成。

2026-11-09：已重現並修正cold書籤檔讀回後收藏停在Loading：結果回呼仍在output-busy gate內，原先只靠可被合併的changed邊緣恢復；現在I/O owner完成join／回呼並清掉gate後明確通知，Loading沿用既有30秒capture deadline，不改來源／ACK保護。CPU／GL各10次目前收藏加列表／無標頭／取消／失焦共24案，以及最終候選18影片＋9閱讀＋2混合共29案皆通過。macOS原生浮球收藏、保存後本機讀至45行、⌘Q及獨立JSON／strict peer／零HTTP／owner清理另驗，Rust486／兩配置Clippy／fmt通過。此單項問題結案；既有GPU偶發錯色、其他平台／IME、對應來源交付、完整回退及效能未由本批代替。來源、hash及scope見[原型紀錄](slint-prototype.md#s4-本機檔案完成後恢復收藏2026-11-09)。S0–S8 goal持續，S9未執行。

收藏修正`3cf91c2`驗收後已清理自有release profile（Cargo8417檔／2.7GiB）、上一代完整候選App／ZIP／DMG及已完成對照用途的舊Native-Bookmarks.app（payload du343544KiB）。現在只留 `slint-capture-ack/package-current` 最新完整候選及唯一Windows S1 ZIP，兩份native合成收藏資料、原始失敗與source保留；worktree du719176KiB、所有自有profile已清空。496份來源與1185份必要交付／資料hash不變，free觀測233442390016→236464750592bytes不全部歸因於清理。exact paths與檢查在本批 `cleanup/ledger.json`；清理後不重建Rust，S0–S8仍active。

2026-11-10 即時產物清理：依使用者要求先處理磁碟，確認無程序／open files 使用後，以 Cargo 原生 clean 清掉 `slint-perf-macos/target/release`（4257檔、Cargo宣告1.2GiB；du1297828→12KiB），另刪兩個已保存最小證據的系統暫存目錄，以及此次獨有 `io.github.yam276.pttstella.slint.perf-dioxus` 的 Library/WebKit／Caches（合計du984KiB）。worktree du2062432→764624KiB；free233114390528→234357141504bytes只是並行期間觀測，不能全歸因於清理。498份來源／文件與1203份保留檔案hash不變，未重建Rust。最新Slint完整包、唯一Windows S1 ZIP、約39MiB的一份同樹Dioxus對照App、未提交量測工具、使用者資料與獨特證據保留；canonical及其他聊天產物未動。所有自有target均僅餘8–16KiB紀錄。exact五筆刪除路徑、逐檔owner／resolved path／symlink、idle檢查、保留理由與空間在 `.local/verification/slint-perf-macos/cleanup/ledger.json`。效能校準不等於同負載效能通過，S0–S8仍active；後續每批及中途清理繼續依harness執行，包含target外的專用WebKit cache與暫存。

2026-11-10：macOS程序族採樣及穩定區間CLI接入開發tooling，PID＋start、獨立responsibility／PPID、Mach CPU單位校準、physical footprint與RSS分列，拒絕共用scope／重用／membership變動／counter reset，保留partial trace。完整Python126項（含4個原生校準）通過；先前兩個實際host smoke只驗程序歸屬與各自格線／Close，不符合共同視窗／焦點／次數，沒有比較勝負。確切來源、命令、限制與正式比較前置條件見[macOS量測入口](slint-macos-measurement.md)。自有profile已清，後續沿用保留程式；完整S8效能／四架構與其他S0–S8仍待驗，S9未執行。

2026-11-10：新增完整服務的顯式 `--probe-benchmark NEW_DIRECTORY`，沿用Live的服務掛載判斷、隔離store／synthetic vault及嚴格本機peer，避免把只畫Frame的舊grid探針當完整host比較。16項實際服務／watch檢查、release格線／幾何安裝、macOS CUA畫面及⌘Q／exit0／Close ACK已分立核對；Rust466項＋6既有明確ignored、全target Clippy通過。原型常駐列使1040×740視窗的格線僅736×442，CUA前景未證明，因此仍不發布效能勝負。下一步依產品浮球契約收斂正常原生chrome，再做共同viewport／前景與完整負載。首輪sandbox／打包／識別及SIGTERM負向保留，細節見[macOS量測入口](slint-macos-measurement.md)。S0–S8仍active，S9未執行。

完整服務入口 `ccd1b8c` 驗收後，先前dev profile已以Cargo原生clean移除7604檔／3.6GiB及失敗strip copy；最後再清掉release profile8232檔／2.4GiB、診斷App、六個本批系統暫存目錄、既有輸出負向資料及本批產生的單一採樣器pycache。release target的du2611368→8KiB，worktree3520888→768704KiB，free231442927616→234030350336bytes只記觀測、不全歸因本批。513份來源及1203份保留檔案hash不變；新入口接續的viewport／chrome工作必須改來源，因此不另存本次machine binary，保留最終原生PNG／設定／準備報告、source／hash及唯一失敗。最新Slint完整候選、唯一Windows S1 ZIP及一份Dioxus對照保留；兩份小型舊bundle helper pycache歸屬不明保留，未動canonical／其他聊天或未整合worktree。exact九筆主刪除路徑與額外pycache清單、逐檔gzip盤點、idle／du／Cargo／free及保留理由見 `.local/verification/slint-benchmark-session/cleanup/`；清理後未重建Rust。

2026-11-10：Live與完整benchmark共用產品shell，將原型常駐列收至既有浮球／設定，補上草稿及Slint授權入口，保留閱讀情境控制；無文章控制列時viewport1040×740、格線1040×624，固定PNG逐byte不變。連線phase的持續提示與本機通知分離、通知不占格線；純連線狀態不顯示無效X。最終候選CPU／GL shell、兩浮球、18影片／9閱讀／2混合共33案，以及macOS實際授權／Escape焦點／斷線／Quit／exit0／Close ACK通過；前版native125%／100%及240秒監督逾時的負向分hash保留。Rust466項＋6明確ignored、兩配置Clippy、Python126及fmt通過；actionlint缺工具not_ready。細節及必要待驗見[macOS量測入口](slint-macos-measurement.md)，最新完整候選為 `slint-shell-chrome-final/package-current`。成功淡出／通知焦點、有效scope草稿新入口、完整原生鍵盤／IME及各工作負載viewport、四架構／回退／效能仍待驗，S0–S8持續，S9未執行。

產品shell來源以 `67738f9` 提交後，確認無程序／open files使用，再清掉同批release profile（Cargo8410檔／2.7GiB）、上一代完整候選與首版App／ZIP／DMG、34個被最終同案取代的generated目錄及兩個系統暫存目錄；exact共43筆見 `slint-shell-chrome-final/cleanup/ledger.json`。target du2910680→8KiB、worktree4248212→776760KiB，free230704709632→233953509376bytes僅記並行／APFS觀測，不全部歸因本次清理。514份來源及1203份保留檔案hash不變；唯一新候選改為 `slint-shell-chrome-final/package-current`，保留Windows S1、Dioxus對照、原始資料、首次無效X／逾時反例、來源patch／hash與最小metadata，不存完整舊binary archive。gzip逐檔盤點、lsof／owner／resolved path、du／Cargo及保留理由皆在同批cleanup；canonical與其他聊天及未整合worktree未動，清理後未重建Rust。

2026-11-10 通知關閉回歸：`shell_probe.rs` 改為以實際standard Button的toolkit pointer事件確認關閉錯誤通知、畫面大小不變、焦點仍在終端且沒有站端輸入。重用清理時保留的CLI，software與OpenGL兩配置均通過並收到嚴格peer Close ACK，未重建Rust。未重現pointer關閉遺失焦點，因此沒有改生產callback；keyboard Tab／搜尋欄／OS pointer與IME仍待驗。來源patch、binary hash及兩案結果見 `.local/verification/slint-notice-focus/acceptance.json`。批次完成後刪除該待驗CLI（約65MiB），未替換最新已驗收App。

2026-11-10 S0／S8跨平台影片建置邊界：原 `build.rs` 在交叉編譯且未設 `MPV_LIB_DIR` 時會執行主機pkg-config，合成Windows目標已重現它輸出主機library search path。現改為比對Cargo HOST／TARGET；交叉編譯必須明示目標libmpv目錄，缺少Cargo identity也報錯，不嘗試主機pkg-config。原生編譯仍可使用pkg-config，明示有效目錄的原生／交叉編譯沿用原路徑；目錄存在不代表library架構／ABI／runtime已驗收。`tooling/test_mpv_build.py` 直接編譯並執行同一production build.rs，以本機可執行的pkg-config替身驗證7案（包含前述red→green、明示路徑、錯誤、無影片配置）；不編譯Slint相依，也不冒充跨平台link或原生執行。全133個Python測試及core gate 14命令／2replay通過；來源、修前／修後結果及清理ledger在 `.local/verification/slint-mpv-cross-discovery/`。Windows／Linux／macOS x64的實際libmpv來源交付、連結、打包與原生驗收仍待完成。

上述跨平台建置檢查完成後，以Cargo原生clean移除 `slint-mpv-cross-discovery/target/debug`（1561檔／268.3MiB），target的du由279660KiB降至8KiB；測試的獨立rustc執行檔由TemporaryDirectory清理，無殘留。1546份受保護來源／資產／交付hash不變。filesystem free 230992998400→231269281792 bytes只記觀測差額；清理後不重建Rust。exact paths、du、Cargo與open-file檢查在同批 `cleanup.json`。

2026-11-10 Windows x64完整主程式候選：重用既有cargo-xwin0.23.1／Windows SDK，以Rust1.97在macOS arm64交叉編譯 `native-player-probe`／`pttstella-slint`，包含桌面服務與播放器；不只S1探針。首輪實際PE顯示CUI子系統，已加上與Dioxus相同的release Windows GUI屬性，重建後由共用PE parser及獨立LLVM工具確認x64／GUI與libmpv匯入。取用mpv官方v0.42.0 LGPL x64 ZIP（SHA256 `8e5f77430efc53ef175804ba6618527e3af0448a0dcabf04d3cc72659d1a6fce`），原GNU COFF import archive僅改副檔名為mpv.lib，MSVC／LLD小型連結與完整主程式均通過；沒有把主機mpv用於目標連結。[mpv官方版本](https://github.com/mpv-player/mpv/releases/tag/v0.42.0)

新增 `tooling/bundle_slint_windows.py`，只消費已建執行檔、匹配的來源／binary hash、runtime來源紀錄及自製CC0三格式影片；沿用既有Windows PE普通／延遲匯入parser，遞迴核對主程式可達DLL、拒絕未知／WebView依賴及未用DLL，明列尚需安裝的MSVCP140／VCRUNTIME140。共用parser預設仍嚴格要求GUI，只有顯式runtime檢查允許CUI。ZIP含原生主程式、libmpv、Rust／Skia／libmpv授權、三格式fixture及離線／格線／IME／閱讀／持久合成草稿／影片／show-profile入口。離線入口只連合成loopback；一般exe仍會連研究設定的預設站、不自動登入。所有驗收cmd入口清掉繼承的診斷變數、各次log與探針輸出不覆寫；草稿明示重用包內evidence/drafts供重開驗收。

最新完整Windows候選為 `.local/verification/slint-windows-candidate/package-current/PTTStella-Slint-Candidate-0.5.0-dev.3-windows-x64.zip`，SHA256 `d733f7f5f8508c7b4c67c7de8749a141f70530318463d9b3741b14aa96fd3b59`。Windows target release/all-target Clippy、全143個Python測試、四target／兩配置8案依賴gate、fmt、docs、actionlint與diff檢查通過；獨立ZIP驗證核對CRC、912個payload hash與LLVM匯入表。首輪驗證器曾因同一DLL不同大小寫的重複LLVM條目錯比set，改成先lower再去重，沒有修改產品／ZIP來配合expected。執行環境仍是macOS，未執行Windows exe：原生啟動、注音／鍵盤、播放器圖像／聲音、profile實際路徑、無WebView2啟動、乾淨安裝及完整來源交付仍not_run／not_ready，不宣稱S1／S5／S8完成；尚需Visual C++ x64 Runtime，未內附CRT。原S1無libmpv的ZIP暫留，作為獨立無影片配置的原生比較，不再為新功能保留額外Windows執行檔副本。命令、SDK路徑、source ref／patch、首輪CUI與最終GUI binary hash、兩個runtime的LLVM報告、封裝／依賴／測試結果在 `.local/verification/slint-windows-candidate/`。

Windows候選批次清理：保存最終ZIP及必要授權／來源／唯一失敗紀錄後，先用Cargo profile clean清掉969.3MiB，但交叉target仍有1985388KiB，因此未宣稱清完；再對同一個已確認無使用者、專屬本批的target執行Cargo原生完整clean（5687檔／1.9GiB），target目錄已不存在。另刪首輪被取代的ZIP、下載的重複runtime ZIP、已封裝的DLL副本、重複import archive與小型smoke執行檔／PDB及自己的pycache。保留一份小型mpv.lib與必要license供下一批、既有cargo-xwin／SDK不刪。1644份受保護來源／資產／交付hash不變，Windows批次約98MiB，Slint整個worktree約794MiB；後續清理free 221845131264→223943602176 bytes僅為觀測。首輪cleanup assertion中斷前的完整du／free-before未持久保存，沒有補造數字；兩段Cargo輸出、後續盤點、exact paths與已取代包的前後hash見 `.local/verification/slint-windows-candidate/cleanup/ledger.json`。首次docs gate指出smoke.exe尚在，完成清理後重跑已通過，沒有放寬gate。清理後不重建Rust。

2026-11-10 Linux x64完整主程式候選：以macOS原生Rust／Clang／LLD及經hash核對的target sysroot完成release連結與all-target Clippy，沿用Windows的完整 `native-player-probe` 功能，沒有改release最佳化或Rust來源。新增Linux打包入口，重用既有Linux deb／tar layout，Windows／Linux共用build-record及CC0 fixture admission；既有Dioxus預設名稱不變。fresh Ubuntu24.04實際安裝抓出ELF相依之外的dlopen runtime缺漏，已明列X11／Wayland／GL套件，以同一ELF重封裝，隔離執行檔／套件／桌面ID。最新deb／tar、確切hash、來源／建置方法、入口及限制見[Linux候選說明](slint-linux-candidate.md)。全150個Python測試、docs、fmt、actionlint與diff通過；既有Windows ZIP hash不變，未重建Rust。

此批component證據為Ubuntu QEMU／Xvfb software的shell／relocated launcher／三格式影片／草稿六案通過，原生OS／IME／聲音／GPU／效能仍未驗收。搜尋以正確 `xvfb-run ... dbus-run-session` 順序重用同一ELF：文章兩案仍失敗、終端一案通過且Close ACK，排除DBus順序作為唯一解釋；不能把wrapper exit0寫成三案passed，也尚未區分「背景套用」與「編輯被拒而保留舊命中」。逐案命令、receipt及反例在 `.local/verification/slint-linux-runtime-recheck/recheck.json`，下一步須在接受編輯／Pending／deactivation邊界補診斷。

上述受控重跑沒有新Rust建置；匯出最小證據後，核對task label／container ID與無mounts，再正常stop／rm `pttstella-slint-linux-recheck-20261110`，並移除已無使用者的臨時Ubuntu image。共用VM只做線上TRIM，不關機、不compact／prune。10135份受保護檔案hash不變；filesystem available 221296967680→222357221376 bytes僅記觀測。exact container／guest paths、程序、du／可寫層大小、來源／候選保留理由見 `slint-linux-runtime-recheck/cleanup/ledger.json`。未留下第二套Linux執行檔或測試環境；最新必要交付、來源及獨特失敗保留，S0–S8繼續，S9未執行。


2026-11-10 Linux搜尋暫停反例完成定位：診斷版在Ubuntu文章案例送出編輯前已foreground=false／find_blocked=true且欄位失焦，編輯後draft仍無、timer未啟動，版本與三筆舊命中不變。先前「背景執行待查詢」解讀撤回；生產拒絕背景編輯的保護正確，未修改find_edit／debounce／失焦邊界。探針先確認可接受的synthetic foreground／field focus，接著確認Pending已保留draft／清空命中，再驗證停用仍blocked、零命中與draft保留，最後恢復查詢。Debian與Ubuntu文章／終端六案（含Ubuntu重跑）均通過且Close ACK；分階段診斷與原失敗保留於 `slint-find-pause/`。此為toolkit證據，原生OS焦點與IME仍待驗。

新版Linux候選使用同一完整配置，ELF SHA256為 `9b68eb81ed5348f4dd44f3e05ab5a1a8284a89b884a7240f6c6df22bd7c6d5f7`，取代原a7c00a9包；路徑仍為 `slint-linux-candidate/package-current`。Depends逐字相同，新包在原Ubuntu runtime root重新安裝，shell／含空格路徑portable launcher／三格式影片／草稿／兩種尋找八案均通過、逐案receipt及Close ACK已核對，不冒充另一個fresh clean install。影片另核對非空四色像素樣本、render退休與session join；草稿未測新程序讀回，不擴大scope。Linux release/all-target Clippy、150個Python測試、fmt／actionlint／diff通過，詳[Linux候選說明](slint-linux-candidate.md)；native_acceptance與完整來源交付仍not_run／not_ready。

本批驗證後以Cargo原生完整clean移除 `slint-find-pause/target`（9706檔／3.1GiB，du3247480KiB），卸載並刪除case-sensitive SDK映像、wrapper／外部LLD捷徑、smoke及重複baseline CLI，外部Rust工具不動；兩個自有Debian／Ubuntu容器與無使用者的臨時Ubuntu image已正常移除。舊完整包刪除，只留最小manifest／hash；42張byte-identical generated PNG以保留位置／SHA映射取代（allocated2854912bytes）。10304份受保護檔案hash不變；worktree du889336KiB，filesystem available 214002139136→221830262784bytes只記觀測，不全歸因於本批刪檔。exact paths、owner／resolved path／symlink、lsof／程序、Cargo／du、保留例外及共享VM線上TRIM紀錄在 `slint-find-pause/cleanup/ledger.json`。清理後不重建Rust，S0–S8持續，S9未執行。


2026-11-10 macOS x64完整主程式已建置：Rust1.97.0／Apple Clang從arm64交叉編譯完整native-player-probe，Rust來源仍為d37190d、Rust patch為空；CLI SHA256 `b1e810584025eb0d4fc65992045c7eb99ae5b25bdd0e4e9bbed6831453731e64`。實際x64 libmpv已在Rosetta初始化，完整CLI的軟體／OpenGL shell、兩種尋找、三格式影片及軟體草稿共13案exit0／Close ACK，逐一檢查非空像素樣本、render退休、session join及搜尋Pending／背景保留／恢復；這是Apple Silicon Rosetta component證據，不算Intel實機／OS輸入／IME／可聽音訊／效能。x64 release/all-target Clippy及154個Python測試通過；新增Mac原生Clang／dyld fixture與可跨host執行的loader／notice負向測試，沒有把skip或wrapper exit0當案例通過。細節與尚缺欄位見[macOS x64待封裝輸入](slint-macos-x64.md)。

x64 runtime暫取自經GitHub digest及完整簽章核對的IINA1.5.1，僅保留44個可達x64 dylib，實際回報mpv0.41.0／FFmpeg9.0.1，與arm64的Homebrew0.42不同。釋出日recipe版本及Credits不能直接代替此binary的精確來源／完整notices，封裝gate仍not_ready；尚未產出x64 App／ZIP／DMG。共用collector已修正dyld父層RPATH繼承，保留各owner上下文與allowed-prefix拒絕；只由主執行檔擁有Frameworks RPATH，避免在沒有headerpad的vendor dylib重複加路徑。非Cellar相依缺來源／notice owner時明確拒絕，不能默默跳過並輸出空清單。這份CLI留本機developer RPATH，不能當可攜包移交；後續來源不變時重用它做Python封裝，不重建Rust。

同輪DYLD shell追蹤仍有六筆System WebKit／JavaScriptCore相關映射，同時載入WritingTools／WritingToolsUI；沒有以source無直接WebView依賴換算成執行期零映射，也未宣稱由此證明WebView實例或完整IME的成因。此差異持續依S8處理；新Rosetta／不同runtime的影片通過不結案原arm64 MP4偶發問題。

本批以Cargo原生clean移除8281檔／2.5GiB（target du2634744KiB），刪除完整IINA DMG／掛載、完成的C fixture執行檔、自有pycache及17張重複generated PNG。10497份受保護檔案hash未變；待封裝CLI及必要44個dylib、來源／簽章／版本／獨特失敗與最小圖像證據保留。清理後以同一CLI及自有runtime重跑MP4 OpenGL，像素／清理／Close ACK通過，證明該案例不依賴已刪target或掛載。batch du130928KiB、Slint worktree1020184KiB，filesystem available 218310406144→221069762560 bytes只記並行觀測。exact paths、owner／resolved path／symlink、lsof、Cargo／du及保留例外在 `slint-macos-x64-candidate/cleanup/ledger.json`；新增Rust x64 target屬已安裝工具保留，未動共用VM或其他聊天。清理後沒有Rust產品重建，Python gate的獨立C／rustc fixture在TemporaryDirectory自動清理；S0–S8仍active，S9未執行。

2026-11-10 macOS x64候選補齊App／ZIP／DMG：重用d37190d完整Rust CLI，從已核對來源建置mpv0.42.0、FFmpeg9.0.2、libplacebo7.387與完整32項依賴，取47個reachable dylib。31項完成後mpv的Swift MetalLayer引用失敗，按實際Vulkan條件加guard，只重編mpv約20秒通過；mujs的build-relative install ID另由producer recipe與既有install修正，沒有放寬collector。來源manifest綁定archive、成功build receipt及實際輸出SHA，提取116份notice，無owner／失敗build／錯來源或輸出／越界與過大輸入會拒絕。實際C API為mpv v0.42.0、client API2.6、FFmpeg9.0.2，FFmpeg license回報GPLv3 or later；完整產品來源交付與乾淨重播仍not_ready，不能由notice gate替代。

最新x64候選在 `slint-macos-x64-candidate/package-current/`，App主程式SHA `3908f73e6f12f9b2ca54ff0b24530a557a8df56bbc21fffef9b2a39d976b60df`，ZIP／DMG及精確scope見[候選說明](slint-macos-x64.md)。1159份payload在App、ZIP、唯讀DMG逐檔一致；48個Mach-O皆x64，ad-hoc簽章deep／strict與掛載後隔離profile通過。將App搬至含空格路徑，兩套開發runtime prefix同時不可用且無DYLD／LD搜尋覆寫，CPU／OpenGL shell、兩種find、三格式影片及CPU草稿13案各有receipt、非空影片四色像素／render退休／session join及Close ACK。另一次封裝shell載入trace有六筆System WebKit／JSC相關映射及WritingTools／UI；不能宣稱零runtime WebKit或推定WebView實例成因。這是macOS15.7.7 arm64／Rosetta component證據，不算Intel硬體／OS輸入／IME／可聽音訊／效能，也不結案arm64 MP4偶發錯色。S0–S8持續，S9未執行。

本批165個Python測試（含真實macOS loader fixture）、fmt、actionlint及diff通過。驗收後移除11個自有obsolete輸入／工具／native build／臨時App路徑，du499156KiB；舊IINA runtime／待封裝CLI由完整候選取代，原始來源與49份recipe／patch輸入、失敗診斷及最小證據保留，沒有Rust重建。24張byte-identical PNG改為SHA／保留路徑映射，16張獨特PNG保留；最終11358份保護檔案及32份來源archive雜湊一致。filesystem available 219832209408→220345688064bytes僅為並行觀測，worktree du1495272KiB，來源交付292272KiB、x64完整候選297812KiB，Cargo target16KiB。清理後同一最終App重跑OpenGL MP4，像素／render退休／session join及Close ACK通過。exact paths、owner／resolved path／symlink、lsof／程序、du／free及保留例外在 `slint-macos-x64-source/cleanup/ledger.json`；shared工具／registry／VM、其他聊天及canonical未動。來源worktree尚未整合，保留全部待整合提交；S9未執行。

2026-11-10 S7版本檢查已接線：Live啟動一次、手動查詢與下載／repository外開沿用共用Checker及固定Rust來源；正常HTTP失敗靜默且不假稱最新。既有Tokio runtime新增一個有界command slot／最新status owner，無新thread／idle timer；關窗先撤銷排隊意圖，再取消並join worker及Slint listener。一般probe／benchmark零公開請求，明確releases案例只用IPv4 loopback。兩個新fixture曾因缺少每client的rustls provider卡住，保留明確panic／失敗後修正，未安裝全域provider；等待與close audit均有deadline。

完整更新案例已過操作階段卻退出45秒逾時，取樣CPU98.1%與Video::step／close堆疊證實root消失後閒置watcher自我Notify；改成一次closed／撤銷，原生命週期排空。CPU／OpenGL更新、播放中CPU Quit／GL Close四案在約1–1.6秒內exit0／Close ACK；後續更新案例另由按鈕焦點／空白鍵觸發手動重試，peer獨立要求三次HTTP及停滯body的EOF／reset。離頁／關閉拒絕、惡意JSON URL忽略、設定bytes不變、worker與listener實際join均核對。這是macOS arm64 toolkit證據，不算OS鍵盤／IME／browser／可及性或其他平台完成；原arm64 MP4偶發錯色仍未結案。詳細來源／反例／命令見[版本檢查與退出回歸](slint-release-check.md)。

本批library377 passed／4 ignored、binary83 passed／1 ignored（影片修正後另重跑binary）、all-target Clippy及165 Python通過，未將ignored算passed。新arm64 App／ZIP／DMG取代原c9413d3，仍在slint-shell-chrome-final/package-current；主程式SHA bbfde96110519ab1d1149b807dc57532bfe6a45df219795a88b473eb1c119ba4，詳候選紀錄。1176份payload跨App／ZIP／唯讀DMG一致、51個Mach-O皆arm64；禁止讀取Homebrew的sandbox下更新CPU／GL、shell及MP4四案有receipt／Close ACK。完整OS／平台來源交付仍not_run／not_ready，其他架構包須同步這批Rust變更。

本批清理Cargo原生8264檔／2.5GiB（du2666732KiB），舊arm64完整包（du277948KiB）、兩個中斷probe的自有OS設定暫存與23張重複generated PNG移除；保留13張獨特圖片、最小來源／失敗／metadata及最新必要交付。最終10289份保護檔案與12份Rust／Slint來源hash一致，worktree du1500908KiB。filesystem available 217673043968→220457906176bytes只記觀測。精確路徑、owner／resolved path／symlink、open files、Cargo／du／free與保留例外在slint-release-check/cleanup/ledger.json；清理後同一候選於無Homebrew sandbox重跑更新／空白鍵／取消／Close ACK通過，沒有重建Rust。未動canonical、其他聊天、共用SDK／registry／VM，未整合來源worktree保留，S0–S8持續，S9未執行。


2026-11-10 Windows 原生證據回收：使用者的 101 份原始 evidence 保持 hash 不變，確認 Win10 x64 build19045／PowerShell5.1、交付 exe 相符、13 案 exit0／嚴格 Close ACK。原 helper 的三個 OpenGL 影片格式誤判已修正，分別要求 software frame serial／size 或 OpenGL rect，獨立像素與退出檢查保留；原報告 failed 不改寫，13 案既有回報重驗通過。新版包僅改 helper／candidate manifest，其餘915個ZIP項目逐byte一致，未重建Rust。166個Python測試、receipt正反例、docs及diff通過；修正版Windows5.1重跑及人工IME／DPI／音訊仍未驗。

同輪即時清理被取代的Windows完整包與暫時提取的exe／DLL，latest從package-fixed正常rename回package-current；保留原錯誤helper、manifest、hash及使用者原始證據。另確認5個frontend往返測試App沒有open files／活動程序後移除，保留兩種必要raw CLI及獨特成功／失敗證據。Slint設定／站點原生保存有exit0／Close ACK，原Dioxus讀回已觀察，但回退操作輪次逾時，不能將完整資料往返算通過。frontend target與臨時baseline worktree先前已用Cargo／Git原生流程移除。本次exact paths、UID／resolved／symlink／內容／lsof、du及filesystem free前後分別在 `slint-windows-current/runner-fix/cleanup.json`、`slint-frontend-roundtrip/host-cleanup/ledger.json`；空間變化僅記觀測，未刪來源、其他聊天、共用工具或必要交付。S0–S8持續，S9未執行。


2026-11-10 實際前端資料往返：新增只收精確 marker／普通檔案的 `--probe-store NEW_DIR MARKED_STORE_ROOT`，用同一生產 Preferences／Store 與完整 Benchmark owner 讀既有副本，不先覆寫預設值。macOS arm64 以原生選單／欄位完成 v1、v6 的 Slint → 原5ea9ad6 Dioxus → Slint 共六輪，皆正常退出及嚴格連線關閉；原 Dioxus 另完整匯出 PNG。設定／站點的完整 expected 與指定輪次的前版備份核對通過，其他四 store 與可攜設定保持 seed hash，不冒充完整 GUI 操作。原 Dioxus 小數字高 step=1 限制、先前超時／工具啟動副作用與 v1 首寫備份證據缺口保留；完整跨平台資料回退仍未通過。CLI、重現 fixture、各輪結果、來源／建置與未驗界線見 [前端往返紀錄](slint-store-frontends.md)。


本批即時結案清理另移除5份已退出的frontend測試App，以及3個原Dioxus探針身份的9個WebKit／cache／系統暫存目錄；逐筆確認owner、resolved path、symlink、內容與lsof，保護來源／兩份必要raw CLI／實際保存資料hash一致。filesystem available 261046784000→261337927680 bytes僅為並行觀測，與先前其他聊天釋放空間不混算；du逐筆記錄。重複generated PNG另有保留位置／SHA映射。確切路徑與保留例外見 `slint-frontend-roundtrip/final-cleanup/ledger.json`；沒有Rust重建，沒有清共享runtime／registry或其他聊天產物。


2026-11-10 接續實際前端驗收：以32d94ac相同Rust來源／保留CLI完成v4名單三輪 Slint → 原5ea9ad6 Dioxus → Slint，繁中／emoji備註、mask_row→mask、白名單個別與共用底色讀回正確，完整expected／三次各自的前版bytes備份及無關資料不變均通過。可攜設定先由Slint原生選檔器讀原fixture的小數字高，再於自有暫存位置完成Slint原生匯出 → 原Dioxus固定probe路徑的UI匯入／匯出 → Slint原生讀回；採22字高，三輪皆正常退出、Close／owner audit通過。原Documents位置Save停用的首輪明確取消並保留partial，沒有推測原因或更改系統權限；其他路徑成功不結案原位置。完整scope與報告見 [前端往返](slint-store-frontends.md)。本批只新增驗收／文件，沒有Rust重建；完整資料回退、跨平台原生及效能仍未通過，S0–S8持續。


名單／可攜設定驗收後已清掉本輪7份App、兩個原Dioxus身份的6個cache／WebKit／系統暫存目錄及單一自有portable測試目錄；最終合成檔與原始成功／partial證據均先保存。來源285份fingerprint、兩份必要raw CLI及其他本批保護檔hash不變，未新增target。filesystem available 305768148992→306195558400 bytes僅記並行觀測，du逐筆另列；原儲存位置的partial不因新位置成功而改寫。exact paths、owner／resolved／symlink／lsof、保留例外在 `slint-frontend-roundtrip/filters-portable-cleanup/ledger.json`，重複generated PNG的SHA與保留位置另在同目錄 `duplicate-pngs.json`。沒有變更系統權限、清共用工具或其他聊天輸出。


同日Windows人工回報：使用者依Run-IME.cmd題目明確回覆「三項都正常」，Microsoft注音候選位置、Esc取消、選字後單次提交記human_reported_passed。尚未另檢視這輪IME bytes log／截圖／顯示倍率，不推定2 bytes或完整IME／DPI矩陣。原文、scope與預期候選exe hash在 `slint-windows-current/user-ime-20261110.json`；不改寫原13案自動報告，也不把人工回覆擴成整體平台完成。

可攜設定原Documents位置的新程序補驗：即使明確重選原資料夾，Save仍停用，正常取消／退出並有Close ACK，保留not_ready且原因未定。最後1份App及4個已退出探針身份的OS偏好plist另清理，小型plist先留作獨特診斷；source／必要raw CLI不變，補驗重複PNG有保留映射。exact paths與filesystem free 305506803712→305575333888 bytes（僅觀測）、du與保留診斷見 `slint-frontend-roundtrip/filters-portable-cleanup/followup/ledger.json`。本輪未動產品程式或重建Rust，完整平替仍待後續驗收。


2026-11-10 收藏／最近看板實際前端補驗：同一v6副本完成Slint → 原5ea9ad6 Dioxus → Slint三輪收藏改名與四行離線閱讀，原created_at、完整archive／footer／link／ANSI左右樣式及尾空白不變，三次收藏前版備份逐byte正確；三個原生畫面在CUA screenshot目視，沒有宣稱精確離線PNG像素稽核。最近看板先讀Stella／Test再清除，兩版主檔與備份皆空，原版重讀／再清除及Slint新程序空清單通過；未按Go，不推定最近新增／排序或站端導覽。三輪exit0／Close與原版owner清理通過；其他資料byte不變，完整scope見 [前端往返](slint-store-frontends.md)。本輪3份App與專用cache已清理，exact paths、free／du與保留例外見 `slint-frontend-roundtrip/bookmarks-cleanup/ledger.json`；沒有Rust修改／重建，完整平替仍未完成。


2026-11-10 草稿資料場景已接到同一 marked-store 入口與完整 owner，`--store-scene drafts` 不重播種資料，仍走嚴格本機 peer。修正原 parity-only 準備檢查後，草稿／預設格線兩個 macOS 原生程序皆完成場景特定 frame／16 owner 檢查、exit0 及嚴格 Close ACK；未修改正常產品入口。操作階段工具因前景保護停用，原生座標啟用不可用，沒有進站端編輯或保存，因此草稿完整 UI 往返仍 not_ready。最近看板正常記入合成 Stella 訪問，其他五 store／可攜檔 byte 不變；首輪診斷失敗與測試漏 connect 的失敗保留。範圍、來源、389／91 個測試與清理見 [前端往返](slint-store-frontends.md#草稿場景準備與格線回歸)。清掉3份臨時App、舊raw與本批Cargo target，最新診斷CLI只留一份；exact paths、du／free及保留例外在 slint-store-scenes/cleanup/ledger.json。S0–S8持續，S9未執行。


## 2026-11-10 文章輸入穩定與原生全螢幕

使用者確認抖動發生在文章畫面直接輸入，底下出現「返回網頁閱讀」。原本每次原始輸入都會建立文章交接，即使沒有快取文章；接著非閱讀畫面又撤銷，讓放在VerticalLayout的返回按鈕反覆改變viewport。現在只有快取文章首次交接才切換，普通終端輸入仍取消在途命令但不建立返回列；Unknown組字對話／局部重繪保留原始輸入狀態，直到已知站端畫面或使用者明確返回。返回操作改為浮層，不再佔用終端高度。新增明確Unknown／MainMenu／EmptyList邊界測試，實際Slint host輸入probe另核對主選單提交「中」時按鈕不出現、viewport精確不變。

macOS原生選單新增「檢視 → 全螢幕」及系統`⌃⌘F`。原生鍵盤處理在組字與站端送鍵前辨識physical KeyF，避免注音logical文字影響，消費按下／放開並忽略repeat；一般尋找快捷鍵仍分開。Slint root保留離線視窗fallback與F11。最終raw6652d9ca…用唯一App ID的原生主視窗，在CUA實際按`⌃⌘F`進入1920×1080全螢幕，再按回到1040×768視窗，終端焦點保留；正常關窗exit0，嚴格loopback peer確認零站端輸入與Close ACK。這是最小原生wrapper使用同一raw，不將它冒充正式App完整OS操作。r2僅觀察F11進入、之後timeout；最終F11未觀察到即時變化，完整F11／IME矩陣未驗收。早期失敗／timeout保留，沒有回填為passed。

最終來源為b9dbb4b加已保存patch，288份Cargo／build.rs／src／locale fingerprint一致。Slint播放器配置bin93過／1個明確helper ignored、all-target Clippy、release build及fmt通過。封裝App的native27291aa2…，ZIP內1232份App payload逐檔一致，另將1864份AppleDouble metadata獨立核對magic；初版collector誤把metadata當App payload的分類失敗另記，沒有修改套件。50個runtime檔與前包200份native notice逐byte相同，codesign strict／DMG verify通過。打包工具亦已執行映像唯讀掛載／payload／簽章核對。

最終App在禁止讀Homebrew的sandbox下，software／default的文章輸入及貼上四案均exit0、唯一嚴格Close ACK通過。額外shell兩案及同binary兩次控制重驗仍failed：software為「shell floating tools did not open」，default為「hover consumed readable time or kept a polling timer」；前包060598ec…software對照也failed，為「paused fade expired」。這些不是同一失敗證據，不能由舊版也失敗就判定新版本沒有退步；原因未定，原始failed保留，不放寬前景／pointer保護或改expected。最新候選僅完成上述局部驗證，完整shell／通知回歸仍待解，前包因此暫留作對照，尚未通過全功能平替。

證據在 `.local/verification/slint-input-stability/`，其中final保存封裝、各程序結果、native-observation、package-integrity及source patch。其他三架構尚未重建本修正，實站打字仍待使用者回報；合成通過不能證明私人站端每種輸入狀態。S0–S8持續，S9未執行。

本輪已用Cargo原生clean移除8851檔、回報2.9GiB（target刪前du3062312KiB），target只剩8KiB metadata；另移除10個精確路徑，包括5份raw、3個唯一ID原生wrapper及2份中間App／ZIP／DMG。刪前逐檔盤點、UID／resolved／symlink／lsof檢查完成，必要非binary metadata與hash已壓縮保存。4134份受保護產品來源、最新候選、前包對照、其他平台交付及Windows原始證據hash不變。filesystem available287426174976→291306532864 bytes為並行觀測，不能全歸因本次刪除；exact paths、Cargo與du、保留例外見 `slint-input-stability/cleanup/ledger.json`。共享工具／SDK／registry／VM、使用者資料、其他聊天與未整合來源worktree保留。

## 2026-11-10 工具列與通知失敗狀態追查

沿用475897f後，僅在明確conformance流程補診斷：每個工具操作記錄當時阻擋旗標，通知流程最多保留128筆變化（要求20ms取樣，實際排程可能延遲），不記通知內容、不替換生產callback／timer，也不改assertion或自動重試。原生／組字／前景保護維持原樣。診斷raw使用前述最新包的50個runtime，以明示DYLD_LIBRARY_PATH啟動；這不是無Homebrew或乾淨安裝驗收，不替換最新下載。

兩個診斷流程皆exit1，原始failed保留：software在恢復fade前已foreground=false，最後「automatic expiry changed terminal focus」；default在初始通知約878ms觀察foreground從true變false，hover／Tab／fade／錯誤優先等通知步驟其後通過，但最後斷線工具的blocked=true、foreground=false，所有modal／組字／closing旗標均false。由生產tools-source-blocked可確認此次最後工具失敗是前景准入拒絕，不能宣稱工具toggle壞掉或繞過保護。未取得OS事件級trace，不判定哪個程式或操作撤銷前景；也沒有據此替先前hover失敗定因或回填通過。

唯一ID的原生wrapper另以CUA補驗：Raise並點終端時曾觀察浮球enabled，下一次操作又disabled，未能維持至工具展開；原生完整shell記not_ready。`⌃⌘F`進出與正常關窗仍可操作，loopback零輸入、Close ACK及程序exit0。ScreenCaptureKit另回報-3812，沒有取得截圖，不將AX當像素證據。未操作使用者的正式PTTStella或其他聊天視窗。

診斷來源為475897f加兩個probe檔patch，288份產品fingerprint保存；Slint bin93過／1個helper ignored、all-target Clippy與release build通過。結果在 `.local/verification/slint-shell-state-audit/` 的results、notice-state-trace與native-observation。保留單一診斷raw供後續在可維持原生焦點的環境追查；最新arm64下載與另外三平台包均未變。這次沒有新增產品修正或宣稱shell／通知回歸解決，S0–S8持續。

本批Cargo原生clean回報8851檔／2.9GiB，target刪前du3061008KiB、刪後8KiB metadata；正常關閉且join後移除唯一ID原生wrapper，保留其最小metadata／hash。3987份來源、唯一診斷raw與保護交付檔案hash不變。filesystem available288864776192→291833364480 bytes僅為並行觀測，與du及Cargo宣告量分列；exact paths、UID／resolved／symlink／lsof、保留例外在 `slint-shell-state-audit/cleanup/ledger.json`。未重建或修改任何正式／跨平台下載，也未刪其他聊天、共享SDK／registry／VM或未整合來源worktree。

## 2026-11-10 輸入修正同步四架構與Linux補驗

其他三架構以乾淨475897f的臨時detached checkout建置，288份Cargo／build.rs／src／locale與已交付arm64逐份一致；9df42ba只增加conformance診斷，未混入本輪下載。三個release build、all-target Clippy及171項Python打包測試通過。Windows沿用既有cargo-xwin／MSVC SDK；Linux重用固定91729675…SDK archive，3419檔／symlink逐份核對，未重新bootstrap SDK。未推送、整合canonical develop、切換正式入口或發佈。

| 架構 | 下載 SHA-256 |
| --- | --- |
| macOS x64 ZIP | `a4ac138d9320cf8fbf0f9b39338edc4b4140d600ad4c3bb10e3e7cd880818676` |
| macOS x64 DMG | `8fa8c6951d20908f3a4393106920ed4cf3d221f606da73fd71700fe50a34b885` |
| Windows x64 ZIP | `60f7c551e8d92c90bbe042df0edd879a228e3bc18ba2bfe62d435a80fb0b56a6` |
| Linux x64 tar.gz | `00e020d80841a9ddbd95e78e348b4fde91b1f27fbc95923a2f24f0e34f0cdad2` |
| Linux x64 deb | `8c56b0e5ddc02ffd4d9237f6ac3b7ccb5089d12e5730be4eca37d9fb674bab21` |

三份最新資料夾在 `.local/verification/slint-input-cross/`，slint-merged-id-spaces入口已改為零複本symlink。current-candidates仍位於slint-navigation-cross，但內容已更新至本輪。Mac的1216個ZIP payload與App一致、47個runtime／183份前包notice不變，簽章與打包器的唯讀DMG核對通過。Windows的915個payload與前包相同，只有exe／candidate.json改變；normal／delay imports未變。Linux在自有ARM容器中使用實際x64 loader與dpkg封裝，tar／deb各有一個93,507,824-byte ELF，hash皆為0312b460…；這不是原生x64硬體驗收。Windows collector初版誤用macOS的downloads欄位、Linux初版把doc目錄算成第二個ELF，均保留分類失敗，按實際schema／regular-file修正；套件未改動。

Rosetta下software／default輸入與貼上四案、software終端account及兩個renderer推文導覽共七案exit0且唯一嚴格ACK；兩個網頁account案均failed於「focus image placement pending」，三個HTTP資源請求皆零，peer因未收到後續PageDown而cleanup failed。沒有將它歸因於前景、或以其他案例通過覆蓋此失敗；Intel硬體與原生IME／DPI仍未驗。

Linux同一封裝ELF在Xvfb-only最小環境的輸入／貼上兩案先失敗，另有缺session bus的訊息；加上隔離Openbox與session D-Bus後，輸入案通過（exact bytes、viewport不變、exit0與嚴格ACK），貼上仍在「fresh confirmed paste preview missing」failed、peer沒有收到預期貼上。這是改進runner的對照，不證明原失敗已定因，也未改產品或放寬assertion。最早helper因Debian Python不支援tar filter而在啟動app前失敗，後以明確member路徑／型別檢查修正；原log保留。Xvfb／Openbox與子程序均join。實際raw-station PNG另見Slint控制項中文顯示方框，終端內建Noto文字正常；目前未向Slint註冊內建字型，須補齊無系統CJK字型的控制項fallback，不能把輸入probe通過當完整GUI通過。

使用者再次回報IME正常並指定Downloads/evidence。CLI被macOS拒絕讀取，改以Finder Quick Look核對native-results，仍是舊04249364…／Windows10 build19045／PowerShell5.1，未修改或複製原結果。使用者確認新版晚點再測，所以新69542cb7…的原生驗收維持not_run；IME回報沒有可核對的新exe身分。原始101份Windows資料及既有helper修正重驗保持原樣。

證據在slint-input-cross，包括source／各平台build及integrity、x64-checks、Linux兩輪GUI結果與windows-user-evidence-observation。下一步處理Linux控制項CJK缺字與貼上預覽，並接續資料回退、來源重建及同負載效能；S0–S8仍active，S9未執行。

本輪先執行各target的release clean，回報成功但實際仍留下約5.6GiB；沒有把命令成功當整目錄已清空。最後對已確認無open files的task-owned target執行完整Cargo clean，回報18502檔／5.5GiB（當時du5800312KiB），target已不存在；先前各profile的2593／2550／2792檔及宣告量分列於cleanup，不混算實際free。另正常移除乾淨臨時worktree、自有容器d6c081e7…（SizeRw1,229,680,842 bytes）、SDK image／mount、link wrappers、運送／解包／raw副本及四份舊完整包。只保留兩個最小舊Mac native（共用且綁定新版runtime hash）與gzip舊Linux ELF，供未解失敗對照，不為每階段保留App。第一個flat Mac對照因rpath解析失敗，改回原MacOS／Frameworks相對結構後，兩架構皆在禁止Homebrew的sandbox通過profile-only，不讀store／vault；未冒充原App原生驗收。

舊arm64刪除時Finder新建.DS_Store使rmtree在目錄末尾中止；重查只剩該6148-byte檔、無open files後精確移除並接續，未停止Finder或使用force。初段2856份保護hash曾計算但未序列化，不能宣稱有完整初段前後對照；復原階段另保存2856份baseline，四平台下載再按先前report核對，後續hash全數一致。filesystem available初段292240674816、復原前292323188736、最終293019557888 bytes為並行觀測，不全歸因於刪除。exact removed paths、逐檔inventory、最小metadata、各階段free／du／Cargo及缺口見 `slint-input-cross/cleanup/ledger.json`。三個既有Slint target只餘8KiB metadata；新target無profile。來源／assets／使用者資料、最新必要交付、共享SDK／registry／VM與其他聊天保留。

## 2026-11-10 最新候選

依使用者要求，先交付四架構候選包，不等待目前沒有的 Intel Mac／Linux x64 實機。連續推文的重複 ID 逐欄改為空格，保留第一筆 ID、冒號、內文、時間及複製對齊；既有黑名單整行遮罩仍優先保留。共用 Rust 產品修正為 `26d9947`，未切換正式入口、整合 canonical develop 或發佈。

以下路徑皆在 `.local/verification/slint-merged-id-spaces/`，取代較早各批的完整候選包：

| 架構 | 最新資料夾 | 下載格式 |
| --- | --- | --- |
| macOS arm64 | `arm64-package/`（symlink經相容入口指向slint-renderer-api-current最新包） | App、ZIP、DMG、SHA-256 |
| macOS x64 | `x64-package/` | App、ZIP、DMG、SHA-256 |
| Windows x64 | `windows-package/` | ZIP、SHA-256 |
| Linux x64 | `linux-package/` | tar.gz、deb、SHA-256 |

| 執行檔 | 封裝內 SHA-256 |
| --- | --- |
| macOS arm64 | `08cb87df1868408cd077a6c24b66afa26909e6e0d9f7804907ebfe928712673a` |
| macOS x64 | `48fe1ce9776a7e2e3f9c5ce37d624f7df51d8951806d4dfd84de54ac3801758d` |
| Windows x64 | `69542cb7c78f5f7bf8b83fb4c70ff759f240d0f734bbae4b153037509d5aca34` |
| Linux x64 | `4e474a0afc2bd0349ba67ff40e996d9685d952c7c4b821100320f8f47db7909d` |

arm64 最新實際目錄為 `.local/verification/slint-renderer-api-current/arm64-package/`，舊 `slint-ui-cjk-fallback/arm64-package` 改為零複本 symlink。351份產品來源核對09a13da；原始建置仍記8f40e47加R4 patch，不倒填為乾淨09a13da。此包包含既有修正、完整shell及macOS預設軟體合成／高解析CPU影片；ZIP SHA-256為 `0d2ceb95189120d4023cd029c717fe6cb1f00f85af87a6a5738755442a21e210`，DMG為 `78fadc95cb38d63a255e51f0bec37c944d85f1c911f8d46bd59e00134d675635`。

Linux 實際目錄仍為 `.local/verification/slint-ui-cjk-fallback/linux-package/`，289份產品來源對應84e98a3；原始建置c67efdb加已保存patch，包含空格、推文浮層、輸入／全螢幕、CJK及貼上修正。tar SHA為 `c3b31a693cf4e785ae9e16e3b2728ac3090ab0508c23c25a2c9e0b2318318968`，deb為 `051970c97af29d07bbaac829fe4a5f030490ee740ce58b79116d437b5e8152b0`。

Windows／Intel 維持 `.local/verification/slint-input-cross/` 的固定475897f來源288份產品檔案，並未納入這次字型／貼上修正；各自下載 SHA 沿用[前輪同步](#2026-11-10-輸入修正同步四架構與linux補驗)。完整 index 在 slint-navigation-cross/current-candidates.json，已改為分平台 source_groups，不能再聲稱四包來源相同。新arm64 App／ZIP有1234個payload逐檔一致，50個runtime library及202份既有notice bytes未變，另依packager契約保存一份舊runtime manifest receipt；App deep-strict／DMG檢查通過。在禁止Homebrew讀取的sandbox通過預設MP4／WebM／OGV、837行長文、PNG／單GIF、文章輸入、貼上七案及三API格線共十案；HTTP／render／session退出及唯一嚴格Close ACK均核對。同產品來源的獨立診斷native c8eeab19…另有CUA原生全螢幕及約99MiB閒置校準，不能轉稱封裝exe的完整OS／效能驗收。詳[量測紀錄](slint-macos-measurement.md#2026-11-10-macos-預設軟體合成與影片尺寸)。

Linux的grid／輸入／貼上三案為隔離QEMU／Xvfb／Openbox／session bus，非原生硬體／IME。Windows13案及注音回報只屬各自舊exe，未轉填新包；69542cb7…仍等使用者稍後測試。shell／通知、Rosetta網頁account等既有失敗與完整原生／資料／效能缺項均保留；三組來源與完整tracked inventory已分別還原，詳[產品來源](slint-product-source.md)。

以下為首次四架構封裝紀錄；其中舊arm64已由上述包取代。所有下載雜湊、ZIP／App 逐檔對照及 Linux tar／deb 的實際 ELF 核對在 `package-integrity.json`。兩份 Mac ZIP 分別有1230／1214個payload與App一致；Windows917個ZIP項目只有exe與candidate.json改變，其餘915個逐byte等於前包，修正過的PowerShell helper、DLL與fixture未變。Mac／Linux的正式打包工具各自檢查簽章／映像或ELF loader／dpkg Depends；這些不等於安裝、原生輸入或實機驗收。

arm64／Windows在851aabd加本批patch建置，提交後286份Cargo／build.rs／src fingerprint與26d9947核對一致；x64／Linux產品來源為26d9947。各架構release build與all-target Clippy通過，建置環境、實際命令、dirty狀態及連結／簽章轉換各自留在build record與package report。沒有因診斷／封裝檔的後續提交而倒填產品建置來源。

macOS x64重用前份已簽章候選的47個dylib與notice，沒有重建runtime或改動library bytes。新增明確的 `--reuse-runtime-candidate`，檢查舊最終exe的report hash、簽章、target／feature、完整Frameworks inventory／每檔hash／架構／imports／rpaths與notice inventory，再只改新exe的rpath及簽章；輸入前後hash不變，原runtime manifest另封存。缺report、錯exe／library、額外或重複library、symlink、錯架構、外部import及簽章失敗均拒絕。舊來源封存中本就為空的zstd build/LICENSE副本逐byte保留，未把空檔解讀為授權覆蓋。Python169項通過；此前空檔拒絕的失敗保留，沒有刪去原notice。

Linux使用本批自有ARM容器的已核對x64 sysroot，由Mac的clang／LLD交叉編譯；3419個檔案／連結逐一驗證。Linux大小寫相異的headers先被盤點拒絕放到不分大小寫磁碟，改用自有暫時case-sensitive APFS image。封裝在同一自有容器使用實際x64 loader與dpkg資料；Cargo notices在host產出並逐檔hash綁定運送，沒有在VM假裝完成Rust建置。首次運送撞上仍在寫的tar而失敗；重試先join producer、核對雙端SHA，原失敗保留。

封裝後，兩份Mac App在禁止讀Homebrew的sandbox各跑software／default shell，共四案：16個owner、1040×740 viewport、shell／notice／退出receipt、exit0與唯一嚴格Close ACK均通過。x64是在Rosetta，不能冒充Intel硬體；toolkit synthetic focus／操作也不是OS鍵鼠／IME。Windows新exe沒有在Windows重跑，之前101份原始證據及Microsoft注音人工回報屬舊04249364…exe，保持原樣。Intel／Linux原生矩陣依使用者要求先不等待，仍記not_run／not_ready。

核心／Slint／desktop／filters、JS及打包測試範圍見[空格修正](content-filters.md#2026-11-10-重複-id-改為空格)。額外的網頁account流程有選單／viewport檢查失敗，舊候選對照亦有選單失敗，原因未定，原始failed及清理紀錄保留；終端完整account案例通過。沒有從這次打包推定完整平替、效能改善、影片／音訊／全IME或來源交付已通過。S0–S8仍持續，S9未執行。

本批已用Cargo原生clean移除31,991檔，Cargo回報10.4GiB、target刪前du11108024KiB；另移除自有SDK image／mount／transfer、自有停止容器（SizeRw1456120849 bytes）、重複raw／封裝input與四份被取代候選的17個頂層產物。舊manifest／notice／逐檔hash封存在 `retired-package-metadata/`，原始成功／失敗及必要source保留；一份當前raw CLI作未解網頁流程的最小重現，frontend相容入口改成指向它的零複本symlink。最新四份交付、Windows原始證據及286份產品來源共2846份保護檔案hash未變。exact paths、owner／resolved／symlink／lsof、Cargo／du／filesystem free及保留例外在 `cleanup/ledger.json`；共享SDK／registry／VM、候選共用OS偏好／cache及使用者資料均保留，不操作其他聊天。重複PNG及歷史文字gzip的SHA／保留位置另在同目錄mapping；free變化只記並行觀測，不全歸因本批。

本批一般gate及缺工具狀態另在 `final-gates.json`：Python169、docs、fmt、diff已通過；目前actionlint缺runner，workflow來源未修改，GitHub Actions未執行，這兩項未計為passed。清理後filesystem available觀測為288026492928 → 299136573440 bytes，與Cargo宣告／du分列；另刪15份byte-identical PNG、保留34份獨特PNG，兩份大型歷史文字gzip逐byte解壓核對。


2026-11-10 網頁推文聚焦診斷：目前交付 arm64 App 的 account 合成案例仍在 viewport 檢查失敗；新增只在明確 conformance checkpoint 取得的有界快照，沒有修改正常捲動、前景保護或通過條件。診斷版顯示點作者前後 Anchor 完全同為 row=1、within=1.7876068506485376；可用高度由564變524、行高23變21、wide圖片gap468變428，因此相同錨點對應的offset由414.600006變379.095734。尚未確定是哪個控制列引起40px版面差異，不能宣稱整個網頁流程通過或實際閱讀位置已跳動。原failed、嚴格peer因提前中止而未收到PageDown的cleanup failed，均保留。

診斷來源3d0c874加兩檔patch、raw exe fb35e2ee17d51ffa5f4e0721aa5e70c6224a6d3747cf323c1fc8f2588d265e12；Slint bin91項通過／1項helper ignored、all-target Clippy及release build通過。這是toolkit合成事件／loopback，不是OS輸入或四平台驗收；沒有用診斷版覆蓋最新四份候選包。建置／快照／failed原文在 slint-web-focus-audit/。本批自有target按Cargo原生clean移除，最新最小診斷CLI保留一份，替代前輪raw及frontend零複本相容入口；exact paths、hash／lsof／du／free及保留例外見該目錄cleanup/ledger.json。S0–S8持續，S9未執行。

本輪清理實測：Cargo clean回報8853檔／2.9GiB，刪前target du3062108KiB；另移除被診斷版取代的476dcf97…raw（68,481,664 bytes），frontend symlink已改指向fb35e2ee…單份診斷CLI。2561份當前候選／原Windows證據／必要CLI的hash前後一致，filesystem available 289505865728→292473794560 bytes僅為觀測，未歸因全數回收量。兩筆exact removed paths及保留理由在cleanup/ledger.json。


## 2026-11-10 推文導覽不再擠壓閱讀畫面

確認先前40px變動來自推文導覽入口：合成文章沒有原PO或自己的回覆，聚焦Alice後focused_rows首次非空，原本在VerticalLayout內的按鈕出現並縮小文章。導覽入口與控制列改放同一視窗的浮層，沿用原有callback、讀取lease與前景／組字保護，不改原始串流或閱讀錨點演算法，也未放寬原本精確offset驗收。

macOS arm64實際Slint host的software／default完整網頁account、software終端account與software／default推文導覽共五案，皆exit0、唯一嚴格Close ACK與HTTP worker join。兩種renderer在入口不可用→首次可用→開啟→關閉四階段，viewport、精確offset、Anchor、Layout及圖片gaps全部一致；三張解碼圖片／wide狀態沿用，GIF恢復，後續選單／Shift選取／過期操作拒絕與唯一PageDown亦通過。舊package與舊診斷版failed保留，不將這次通過回填為舊版完成，也不認定之前獨立選單失敗的原因已完全釐清。兩張實際元件PNG已目視核對浮層、推文側標與格線；這些仍是toolkit合成事件，不是OS鍵鼠或其他架構驗收。

來源d08bf5f加本批patch，raw c15fa37d09c45c22e7a83c2467b9a323e6c4574547a6afa403923e60ade28ccd；一般／播放器兩配置Slint bin各91過／1個helper ignored，播放器all-target Clippy與release build通過。初版臨時collector錯用result.json／status解讀無status欄位的reply-navigation.json，原始程序均exit0且app verifier／ACK通過；另按獨立明確row／boolean expected重驗，保留首輪分類與修正紀錄，不重跑或改寫原始報告。證據在slint-reply-overlay/。

最新四架構下載仍為26d9947的空格修正版，本次導覽修正尚未替換下載；只留一份最新raw供下一批封裝／原生驗收，跨平台、最小視窗／語言、原生輸入、完整資料回退與同負載效能仍待驗。自有target與被取代的前輪raw於本批清理，exact paths、UID／resolved／symlink／lsof、du／Cargo／filesystem free與保護雜湊在cleanup/ledger.json。S0–S8持續，S9未執行。

本批即時清理完成：Cargo原生clean回報8881檔／3.2GiB，刪前target du3380808KiB；前輪fb35e2ee…診斷raw已移除，frontend相容symlink改指向本輪單份c15fa37d…。2742份最新交付／原Windows證據／保留檔案hash不變；filesystem available 288987926528→292184289280 bytes只記並行觀測，與du及Cargo宣告量分列。不清共享SDK／registry／VM，不移除尚未整合來源worktree。


## 2026-11-10 arm64 封裝與雙語導覽補驗

bc380f9的同一產品來源（286份fingerprint一致）已封裝為最新arm64 App／ZIP／DMG；未重建Rust或播放器。原打包器重用路徑只接受新exe的@rpath相依，對arm64原本的 `/opt/homebrew/opt/mpv/lib/libmpv.2.dylib`拒絕，首輪failed保留。現在只對「解析後完整來源路徑與原manifest記錄相同、原始library SHA相同、只有一個對應bundle entry」的絕對import加入明確install_name_tool relocation；不按basename猜測、不複製未知library，也不變更舊dylib。link_source保留最初連結輸入供連續重用，安裝後仍檢查imports／rpath、簽章和輸入hash不變。兩次連續transport正例與不同路徑、變動／缺檔、錯hash／重複來源、未搬移import等負例通過；完整Python171項通過，workflow未變且actionlint缺工具仍not_ready。

新exe060598ec…、ZIP20d1628c…、DMG0a16bbe8…；50個runtime library逐byte相同、199份既有notice相同，新增一份前包runtime manifest來源快照；ZIP／App1231個payload一致，deep strict ad-hoc與唯讀DMG檢查通過，未公證。actual build是d08bf5f加patch（raw c15fa37d…），產品來源核對bc380f9；封裝工具為bc380f9加本批Python patch，dirty與provenance如實保留，不倒填成clean build。

封裝App在禁止讀Homebrew的sandbox完成software／default網頁account、software終端account、software／default推文導覽，加兩種renderer的shell／通知退出，共七案exit0、唯一精確Close ACK；HTTP owner全joined，shell16 owner／1040×740，notice退出timer停止。這是實際host的toolkit合成流程；其他架構、影音完整矩陣與OS輸入不由此推論。

另以同一簽章App的隔離reply fixture做macOS實際CUA操作：導覽控制在最小寬度（拖曳至580附近，截圖含標題列約581×390；沒有聲稱精確backing pixels）繁中／英文皆完整可見。繁中原PO聚焦後Down由0／4變1／4；Esc關閉並回文章輸入。經設定的一般頁選English，顯示保存成功及立即翻譯；英文Next reply由1／4變2／4，Esc再次恢復文章輸入。fixture PID46328正常OS close、exit0、嚴格零站端輸入peer ACK及HTTP join通過。CUA的原始圖／AX觀測在對話工具結果，沒有匯出成PNG，因此不冒充獨立像素驗收或完整鍵盤／可及性／DPI矩陣。

關窗後再讀bound app AX曾出現正常非probe入口，已立即關閉，最後inventory不再列Candidate；該次正常啟動未追蹤網路／store副作用，明確排除於fixture隔離與驗收範圍，不把它宣稱為測試程序零外連。之後關閉只查inventory／已擁有process handle，不在已退出App上讀AX。原PTTStella與其他聊天視窗未操作。

驗證完成後，清理前份完整arm64包及已被新包涵蓋的c15fa37d…raw；首次失敗的空staging移除，成功r2正常rename為arm64-package。保留舊manifest／notices／完整file SHA的小型封存，frontend相容symlink改指向新App的同一native，原四平台根目錄下arm64-package亦為相容symlink；沒有再留App／raw副本。exact paths、UID／resolved／symlink／lsof、du／filesystem free與不變hash見slint-reply-overlay/package-cleanup/ledger.json。其他三平台、原Windows101份證據、source與共享SDK／registry／VM／候選共用OS cache均保留。本批無Cargo建置／target，goal S0–S8持續，S9未執行。

本批清理前舊arm64包du279076KiB、冗餘raw68,495,776 bytes；filesystem available 291290202112→291640791040 bytes只記並行觀測，不能將差額全歸因於本批。1326份其他三平台／原Windows證據與286份產品來源hash不變，舊包1180份metadata／notice逐檔SHA封存核對。保留最新交付及獨特成功／失敗證據，不動共用OS cache。


## 2026-11-10 四架構同步候選

Windows／macOS x64／Linux的release build與all-target Clippy六個命令皆通過，均為乾淨de6245e；286份產品fingerprint與arm64候選完全相同。新包在slint-navigation-cross/{windows,x64,linux}-package，arm64維持slint-reply-overlay/arm64-package；原slint-merged-id-spaces下四個平台入口均為零複本symlink。完整下載路徑、七個下載SHA、各自真實build record及驗收界線在 `slint-navigation-cross/current-candidates.json`，沒有將不同編譯／封裝來源倒填為同一commit。

Windows917個ZIP payload只有exe與candidate.json不同，其餘915項（含修正過的PowerShell helper／DLL／fixture／授權）逐byte不變，PE匯入及VC runtime需求相同。Mac x64重用47個簽章dylib及182份既有notice，ZIP／App1215個payload一致、deep strict ad-hoc與DMG檢查通過。Linux同一ELF在tar／deb內的SHA均為97ff4f91…，非ELF檔只有build-record.json變動；實際loader／dpkg核對於本批有標記的ARM容器執行，不冒充Linux桌面或原生x64編譯。

Rosetta的初輪七個合成案例為六過／一失敗：software網頁account在第一次展圖前逾時，三個HTTP皆零請求、驗收提早中止使嚴格peer未收到預期PageDown。相同簽章exe的單案新程序重查完整通過，原failed與ACK／join均保留，整體仍記intermittent_not_ready，尚未確定原因，不推定CPU負載、前景或renderer缺陷。default網頁account、終端account、兩種renderer推文導覽及shell／notice退出各自通過。這些是toolkit合成事件／Rosetta；新Windows尚未執行、Intel／Linux原生依使用者要求先不等待打包，舊Windows13案與注音回報不延伸到新6b1de7df…exe。

本輪SDK逐份驗證3419檔，解壓在自有case-sensitive APFS image；使用Apple clang／Rust LLD及精確sysroot。APFSX參數的首輪建立exit1，stderr未保留；查本機help改用Case-sensitive APFS後完成，未宣稱首輪失敗原因已確定。為後續重播／交叉建置只保留一份已安裝的固定SDK工具 `/Users/ladesine/Library/Caches/PTTStella-Slint/linux-cross/bookworm-x64-sdk-91729675/`（archive112,842,618 bytes，另含逐檔inventory／套件版本／export recipe／SHA），不保留Cargo profile、解壓環境或整個容器。不建立背景服務／全域鎖，不清共享SDK／registry／base image／VM或歸屬不明的舊容器。

結案使用Cargo原生clean、本輪image正常卸載及本輪停止容器的非force移除；清理三份舊完整包、三個raw、編譯與封裝input／transfer／linker暫存。新四包、SDK工具、原Windows101份證據及產品來源受hash保護；舊metadata、完整file SHA與最小recipe封存，Rosetta失敗保留在新候選即可重現。exact paths、UID／resolved／symlink／lsof、du／Cargo／filesystem free及保留例外見slint-navigation-cross/cleanup/ledger.json。S0–S8仍需完整原生／資料回退／來源交付／同負載效能驗收，S9未執行。

本輪Cargo原生clean回報21,687檔／7.0GiB，刪前target du7446036KiB；本輪停止容器SizeRw 1455690398 bytes已非force移除，source286份及2568份保護檔hash不變。正式ledger的filesystem available 288739176448→297013121024 bytes是並行觀測，與Cargo／du分列；初輪preflight在掛載helper仍持有本輪image時停止，先正常卸載後重做盤點，前次未序列化的free值沒有補造。舊三包已移除內容並換成指向最新包的symlink；本輪target／image／unpacked sysroot／transfer／raw／link-input均不存在，唯一保留SDK工具如上。


## 2026-11-10 原生控制項使用內建中文字型

Slint 原生控制項現在在平台初始化後、第一個視窗顯示前，向共享 Fontique collection 註冊既有 Noto Sans CJK TC，補到漢字、注音、假名及韓文字體 fallback 的末端。先保存平台已解析的 fallback 順序，保留系統或使用者的優先選擇。終端與控制項共用同一份 static 字型 bytes，沒有新增字型資產或 WebView 依賴；採用 Slint 1.18.1 對應的 unstable-fontique-011 API，未升級依賴。

無系統中文字型的隔離 Linux runtime 中，實際 x64 ELF 透過 QEMU、ARM Xvfb／Openbox／session bus 執行，格線擷取及完整文章輸入皆 exit 0，輸入有唯一嚴格 ACK。已目視核對 r4/grid.png 的中文按鈕及提示可讀；終端文字仍使用 Rust renderer。這不是 Linux x64 硬體、OS 輸入法、其他平台或效能驗收。R4 ELF 0f1a554c…，93,591,968 bytes，完整 Noto bytes 僅出現一次；這個數字不代表 heap／RSS。播放器與一般 Slint 配置各 95 項單元測試通過、1 項 helper ignored，Linux release build／all-target Clippy 通過；兩個新增測試分別驗證既有 collection clone 能取得 fallback，以及已設定字體保留第一順位。依賴檢查八組 target／配置及五個驗證器測試通過。證據與來源 patch 在 slint-ui-cjk-fallback/。

貼上仍是獨立未解項目：舊 ELF、目前交付及字型原型皆在 fresh confirmed paste preview missing 失敗。R4 邊界紀錄證明讀取前來源、前景與欄位焦點有效，讀取已受理，但稍後出現 paste-stale；尚未確定瞬間失效原因，沒有放寬保護或改 expected。最新四份交付仍是 475897f 的固定來源，尚未納入本次字型修正；新版 Windows 69542cb7… 等使用者稍後驗收。S0–S8 持續，S9 未執行。


## 2026-11-10 貼上預覽跨幾何重繪

R5 同一流程的邊界紀錄進一步顯示：讀取完成時來源、前景與焦點全部有效，預覽開啟後 frame-paint-pending 變 true，使原先把來源暫不可操作視為來源已過期的維護 callback 撤銷預覽。修正只調整已受理預覽的存續條件：沿用 Rust presentation 的 generation／revision／policy、input epoch、連線及 UI 狀態，幾何重繪不撤銷同來源的預覽；新增讀取、完成讀取及確認送出仍要求已 ACK 的幾何。切換來源、變更規則及失焦等既有撤銷路徑保留。臨時產品邊界輸出已移除，原始診斷 patch／失敗 log 保留。

完整 article-paste 探針新增兩項獨立行為檢查：同來源 frame-paint-pending 期間預覽必須保持；此時按確認必須拒絕且站端零 bytes。之後重新開啟、取消、規則失效、組字拒絕讀取、舊 worker／錯誤、單行 raw handoff、返回閱讀及確認多行 UAO、關窗 join 的原驗收條件不變。R6 在無系統中文的 Linux 隔離 runner 通過 grid／article-input／article-paste 三案，輸入／貼上均 exit 0 與唯一嚴格 ACK；同來源 macOS arm64 raw host 的 software／default 貼上兩案亦通過。這些使用 toolkit 合成前景／輸入與自有剪貼簿來源，不是 OS clipboard／IME 或 Linux 硬體。

來源 c67efdb 加 R6 patch，Linux ELF 4e474a0a…；兩配置各 95 測試通過、1 helper ignored，Linux 與 host release build／all-target Clippy 通過。來源 289 份 fingerprint 前後一致；資料在 slint-ui-cjk-fallback/r6，之前 R3–R5 的 failed 未覆寫。已刪除被取代的 R2／R3／R5 raw 及已消費傳輸 tar 共 418,334,718 bytes，保留小型來源／診斷與獨特結果，exact paths／UID／symlink／lsof／filesystem free 見 obsolete-revisions-cleanup.json；free 變化只記並行觀測。


## 2026-11-10 字型與貼上候選封裝、清理

已使用本批既有 release ELF／Mach-O 完成 Linux 與 arm64 封裝，未再重建 Rust。arm64 ZIP／App 1,233 個 payload 逐檔一致，50 個 runtime library 及201份前包 native notice 不變，strict ad-hoc signature 與唯讀 DMG 檢查通過；禁止 Homebrew 的 sandbox 中 software／default 輸入／貼上四案 exit 0 且唯一嚴格 ACK。Linux tar／deb 的同一93,598,360-byte ELF SHA 均為4e474a0a…，實際 loader／dpkg 檢查通過；1,120 個 tar payload 只有 exe／build-record 改變，其餘1,118份逐byte相同。R6格線 PNG 與已目視核對的R3／R4逐byte相同，雜湊 ef9d7df8…；沒有以此宣稱其他 DPI、OS輸入或效能通過。完整 Python171測試通過，workflow未改，本批未執行actionlint／Actions。

本批清理已完成：Cargo 原生 full clean 回報18,844檔／5.8GiB，刪前 target du6,142,788KiB，實際確認 target 不存在；自有容器18c0ece5…正常 stop／rm（SizeRw 1,714,648,165 bytes），SDK image 正常卸載後移除。被替換的兩份舊完整包、R2／R3／R4／R5／R6 raw、已消費傳輸與封裝 input、linker wrapper、pycache及暫存 mount 均已移除；舊包／相容入口為指向最新包的零複本連結。12份逐byte重複PNG改為相對連結，22份獨特PNG保留，原報告路徑仍有效。2,758份最新交付／來源／最小對照保護 hash 前後一致，Mac舊baseline的Frameworks連結亦有效。

exact removed paths、resolved／UID／symlink／lsof、每檔SHA、Cargo宣告／du／SizeRw及filesystem free各自列在 slint-ui-cjk-fallback/cleanup/ledger.json；filesystem free只記並行觀測，不將其他工作的釋放或新增歸因本批。共享SDK／registry／toolchains／VM／base image、未知容器、canonical及使用者資料保留；尚未整合的Slint worktree和必要最小舊對照／shell診斷保留，未刪未整合來源。最新Windows69542cb7…未變，仍等使用者稍後實測；S0–S8持續，S9未執行。


## 2026-11-10 候選產品來源獨立還原

新增 repo 內的來源還原工具及六個自製 Git fixture 測試，完成先前 deferred 的產品來源還原。單一23,061,829-byte Git bundle 加 manifest／獨立工具／README，於新目錄還原兩組候選：84e98a3完整546檔、475897f完整545檔；full tree／逐檔SHA／完整tracked清單及289／288份原建置輸入皆核對。manifest綁定四個目前封裝exe SHA，各包原始build ref／dirty／patch不改寫。來源包包含必要產品assets與Git ancestry，不含worktree的.local、target或未追蹤使用者資料。

兩份來源的offline cargo metadata通過，從還原出的84e98a3一般Slint配置於macOS arm64完成release測試95過／1個helper ignored；使用公開Cargo cache與既有工具鏈，不把播放器／跨平台編譯、二進位逐byte重現或完整第三方閉包計為通過。完整Python177項通過；新六案涵蓋原checkout刪除後還原、patch／新檔與hash／tree／路徑／清單／目的地拒絕。首輪測試fixture遺漏移除ls-files結尾NUL造成六個setup error，已修正fixture並保留說明；不是生產還原失敗。

來源包與命令見 [產品來源還原](slint-product-source.md)，證據在 slint-source-reconstruction/；目前candidate index新增獨立來源交付指標，沒有重打包或修改四份二進位。完整第三方來源、播放器重播、原生／資料／效能必要缺項仍未達成，source_delivery維持not_ready，S0–S8繼續、S9未執行。

本批結案已用Cargo原生full clean刪除自有target，並逐檔核對後移除三份臨時還原checkout；來源交付只保留repo外的單一bundle及manifest／工具／README。既有docs gate拒絕repo內.bundle的首輪結果保留，沒有放寬gate。2,764份候選／原產品輸入／工具／來源交付保護hash未變；exact paths、resolved／UID／symlink／lsof、du／Cargo宣告與filesystem free分列在slint-source-reconstruction/cleanup/ledger.json。source worktree、必要候選與共享工具／使用者資料保留，沒有重建Rust來證明清理。


## 2026-11-10 x64 runtime 乾淨來源重建

以32份既有已核對來源與配方、兩個精確source-export transport override，在新prefix完成32／32套件，主建置程序禁network，未複用舊install；腳本回報10分26秒。47個library對應原source owner／相同basename且全為x86_64，非系統loader邊界只在新prefix，實際API版本mpv v0.42.0／131078與FFmpeg9.0.2核對。原歷史output hash和新hash分列，沒有bit-identical或Intel實機主張。

自製獨立client／render thread的C元件fixture，首輪MP4在25秒deadline後被終止，無JSON；WebM／OGV像素與刻意錯色負例符合expected。加有界checkpoint後四案符合expected，原MP4同一exe另三個程序及清理後的最小重現亦通過，但原失敗phase未知，重複性仍not_ready，沒有覆寫failed或稱為修正既有GPU異常。版本collector首版誤漏顯示前綴v，原程式exit0／JSON保持，再按獨立來源version映射核對，與解碼失敗分列。完整來源交付、Rust／Skia／其他架構、原生／資料／效能缺項繼續，四份既有候選bytes未變。

來源／工具／配方及最小未解閉包保留在repo外，路徑、命令、scope見 [來源重建](slint-native-source-replay.md)，candidate index新增分立native_source_replay證據。新展開來源、build／install、工具／venv與冗餘input／fixture／成功binary共11根、64,223 regular files已清理；只保留原失敗CLI及47 raw libraries共64,770,576 bytes，load-path compatibility links核對有效。2,902份原來源與候選保護hash不變，3,151,611-byte build log無損gzip成169,418 bytes。exact paths／UID／resolved／symlink／lsof、du與filesystem free分列在slint-runtime-source-replay/cleanup/ledger.json；free只記並行觀測，不全歸因本批，無Cargo target、未動共享工具／其他工作／使用者資料。S0–S8持續，S9未執行。


## 2026-11-10 草稿前端保存往返

目前 arm64 候選6841e17e…與原乾淨5ea9ad6 Dioxus在同一 marked-store 副本完成 Slint → Dioxus → Slint 新程序三輪草稿編輯保存。原版實際讀到 Slint 中文／emoji／亮紅ANSI，再經模板capture、上色及固定正文保存；Slint新程序讀回三行及標題，原生貼上新中文／emoji尾行並保存。原版既有gate為passed，capture／color／save為1／1／4且fixture_matches，三輪exit0／嚴格Close；原版各worker／listener完成退出。一代備份各等於上一主檔，原版revision6–10及回程11有觀察紀錄，其他兩筆草稿、其他五store與可攜檔不變。詳 [草稿原生往返](slint-store-frontends.md#草稿原生編輯與保存往返)。

首輪typeText只留下ASCII、240秒controller逾時/-15及無Close仍保留failed，原因未確定；後續以同一資料修正重複前綴並使用原生paste成功，不覆寫失敗或推定實體IME正常。含emoji的UAO上色拒絕與之後可編碼正文成功也分別記錄。新建／刪除／多開衝突／帶回站端、其他平台、正式路徑／vault與同負載效能仍待驗。

本輪沒有Rust變更／重建；兩份自有host App及五個專用OS cache／WebKit／暫存目錄已確認退出、lsof無open files後清理，共110,884,103 regular-file bytes。3,026份來源／候選／最終資料hash不變，Frameworks symlink不跟隨刪除；exact paths、inventory、du與filesystem free前後在 slint-draft-roundtrip-current/cleanup/ledger.json，free只記並行觀測。最新四包、原baseline raw與唯一失敗／合成資料證據保留；Windows69542cb7…不變，等使用者稍後實測。S0–S8持續，S9未執行。


## 2026-11-10 閒置校準與 renderer 圖形占用

目前arm64 native6841e17e…完成預設OpenGL兩輪及software一輪，各10秒／101筆穩定程序族採樣、完整16 owner與1040×740 viewport／1040×624 grid、scale1／zoom1，原生首尾terminal focus可見；三輪exit0／唯一Close ACK。程序族footprint預設兩次約842–845MiB，software單次約84–98MiB，但後者CPU／RSS較高。預設重測後的vmmap顯示主程序829.5M footprint，IOAccelerator (graphics) resident／dirty666.1M，圖形配置為主要占用類別；未定位個別分配／洩漏，不先更改預設。接續以長文／GIF／影片同劇本核對取捨並處理前景觀測。

既有2846dc4 Dioxus對照948d2a9e…以相同bundled字型／設定bytes及目視相同格線完成101筆採樣，PNG／surface／owner退出passed，最後觀測的所有程序身分消失。其原生前景仍未確認，且不是最初5ea9ad6；後者不接受parity_grid，exit2另保留。Documents EPERM、首份錯誤settings fallback、未確認前景與host簽名失敗均分案保存，沒有放寬權限／保護。完整原始數據與未驗範圍見 [閒置校準](slint-macos-measurement.md#2026-11-10-閒置格線校準與圖形記憶體)。S8沒有兩版改善比例、cold／warm、啟動／操作p95或完整次數／負載結果，仍not_ready。

本輪沒有Rust變更／重建；兩份App、自有temporary根及五個專用OS暫存目錄共110,762,516 regular-file bytes已清理，保留小型合成資料與唯一raw trace／vmmap／失敗。3,016份來源／候選／必要對照hash不變，另移除4份byte-identical PNG；exact paths、UID／resolved／symlink／lsof、du、filesystem free與重複圖映射在 slint-perf-equivalence-current/cleanup/，free僅記並行觀測。Windows等待稍後驗收的包不變，S0–S8繼續，S9未執行。

## 2026-11-10 同程序啟動與閒置補驗

重用既有binary，診斷App從LaunchServices以同PID exec進入native，兩版都具有獨立responsibility root。Dioxus已可用原生座標點擊／Escape並回終端焦點；相同設定、80×24名義格線、外視窗與游標條件下，各完成101筆／10秒採樣：Slint無旗標software footprint峰值93,230,720 bytes／CPU0.192464708秒，2846dc4 Dioxus對照642,592,384 bytes／0.370350541秒。兩版CUA⌘Q後全部觀測PID＋start消失，Dioxus surface／cleanup及Slint唯一Close ACK核對通過；LaunchServices orphan的numeric exit未觀測，未填exit0。

這不是初始5ea9ad6基準、完整冷暖重複或OS顯示延遲／S8通過；連續前景、雙GIF與混合功能、100次操作、原生平台矩陣仍缺。首次Dioxus浮球hover採樣與Slint座標點浮球未展開的觀察各自保留。詳細來源／簽名變化、幾何範圍、原始trace及清理見[同程序校準](slint-macos-measurement.md#2026-11-10-同程序啟動與原生焦點校準)。兩份App、自有暫存根及五個精確OS namespace已清理，固定Cargo cache與worktree保留；最新四包／原始來源未改。S0–S8持續、S9未執行。

## 2026-11-10 雙 GIF 原生負載與 CPU 待修

已新增兩版共用loopback閱讀頁面、固定512×160／32幀／100ms素材與可獨立停止／join的HTTP入口；Slint保留完整16服務，正常Live不受影響。兩版CUA實際展開兩張、開關設定／恢復、回文章焦點與⌘Q；Dioxus R3完整閱讀／圖片batch／原始PNG及owner audit passed，Slint R3已ack閱讀packet與唯一嚴格Close passed，全數觀測PID＋start消失。HTTP兩URL各3次（三程序），恢復未重抓。

單輪10秒／101筆的播放CPU為Slint3.603962167秒、Dioxus1.149564664秒；恢復後3.504497416／0.745315333秒。Slint footprint較低但動畫CPU退步，不能宣稱平替或S8通過。先對相同負載profile，再改善decoder／重繪，不降低解析度／幀數／可視圖數取巧。首輪準備與audit失敗、不同版面／footer、單輪／前景／瞬間峰值限制及完整原始證據見[雙GIF負載](slint-macos-measurement.md#2026-11-10-共用雙-gif-負載與-cpu-反例)。固定Cargo cache與當前診斷pair供接續profile復用；四平台下載維持原hash，Windows仍等使用者稍後驗收，S0–S8持續、S9未執行。

## 2026-11-10 GIF 快取修正與軟體提交待修

同一R3 native取樣確認GIF／weezl解碼與Skia software／CoreGraphics提交各有成本。已加入單播放者16MiB／最多512幀的有界循環重用，保留幀延遲、透明／disposal、有限循環與增量fallback。修正後同負載10秒CPU1.93／1.87秒，另一程序恢復後1.71秒；最終Clippy版本另測1.32／1.36秒，較修正前3.66秒低，但仍高於Dioxus。下一步處理剩餘軟體繪製／提交成本；不降低尺寸／幀數／可視圖數，S8仍未通過。

另修正被動準備與焦點權限混用；已取得背景prepared且原生操作後才可輸入的分立證據。超過300秒peer期限的失敗保留，另一快速完整生命週期取得嚴格Close／全部觀測身分退出及HTTP join。完整scope與來源見[循環重用](slint-macos-measurement.md#2026-11-10-gif-循環重用與剩餘繪製成本)；固定worktree／cache及診斷pair繼續復用，四平台下載未替換，S0–S8持續、S9未執行。

## 2026-11-11 保留 raster 與候選封裝

`9a06515` 修正 macOS software buffer 每次清空及 snapshot 消耗視窗 dirty history 的邊界；只保留 buffer 不足以修復頁尾漏畫。獨立完整／局部 Skia 測試在原版 renderer 重現失敗，修補後一般及 native-player 兩配置各 118 對逐像素相同，其中 100 次確實局部更新；另有原生全螢幕／恢復、頁尾與兩張 GIF、設定開關及退出證據。兩次 10 秒校準 CPU 約 1.10 秒，不能代替完整 Dioxus 同負載效能矩陣。來源、原反例及限制見 [macOS 量測](slint-macos-measurement.md)。

新 arm64 App／ZIP／DMG 已更新到原固定 `slint-renderer-api-current/arm64-package/`，native SHA-256 `f211c341c2b7404e44af0ff8fe3b83ffc53e1eb22531b6e089352ac4ebbfbafb`。乾淨9a執行 Cargo 0.32秒命中既有快取，實際原編譯仍列 aff5ac8 dirty，不改寫來源紀錄。App／ZIP 全部1,237份payload一致，strict signature／DMG驗證通過；50份runtime逐byte沿用，203份舊native notice全部保留並加一份前manifest receipt。Rust notice collector現在也收錄local dependency，含renderer三份原授權及兩份修補／來源文件。

同一封裝執行檔在拒讀Homebrew的sandbox通過預設三codec、長文、inline、文章input／paste及三API格線共10案，全部exit0／嚴格Close；影片control／renderer／HTTP均有退出receipt。首兩次MP4逾時的stack停在合成fixture的File::open；inline兩次逾時則停在CoreText建立字型時由getcwd進入open。三個合成影片複製到同一owned暫存目錄並核對SHA後影片通過；inline只改同一暫存CWD後通過。這定位了測試環境存取邊界，但未證明OS權限根因或產品權限修復，原失敗／stack保留，不以重跑抹去。這些是合成toolkit／loopback驗收，不是原生IME／OS輸入延遲或S8完整通過。

新的 [產品來源包](slint-product-source.md) 已完整還原580／546／545檔與tree；ARM一般Slint從還原來源建置103過／1 helper ignored，原checkout來源被sandbox拒讀、固定compiler cache復用。完整第三方來源閉包仍not_ready。三份臨時還原checkout驗後移除，沒有新target。更新candidate index後移除舊09a的完整App／ZIP／DMG，只留小型report／hash；50份library相容連結仍指向同一固定新包，其他三平台下載SHA未變。filesystem available264153116672→264440426496 bytes僅記並行觀測；exact paths、UID／resolved／lsof、du及保留例外在 `slint-renderer-api-current/raster-package-current/cleanup.json`。

本批來源核對、建置、封裝、原失敗與接受結果集中於 `slint-renderer-api-current/raster-package-current/`。Windows69542cb7維持原包，等使用者稍後實測；Intel／Linux實機、其餘原生／資料／效能矩陣及最新修正跨平台同步仍缺。S0–S8持續，S9未執行。

## 2026-11-11 三架構同步與未解帳號案例

乾淨 `9db9eda` 在同一worktree／固定Cargo target完成 Windows x64、macOS x64、Linux x64的release建置及各自all-target Clippy。三者與ARM9a的380份建置輸入逐byte相同，僅兩份tracked文件不同。Windows／Intel／Linux中間檔現在都保留在同一target供後續復用；沒有新建checkout或每架構另開target。

| 候選 | 執行檔 SHA-256 前綴 | 本批驗證與限制 |
| --- | --- | --- |
| Windows 新包 | `487c428b` | ZIP919檔，舊包914檔不變；改exe／candidate／Rust inventory，另加renderer兩份來源文件，三份原授權仍在。PE／閉包與hash核對，原生未執行。69542cb7舊ZIP及原下載路徑不變，仍等使用者稍後測試；新包放在index的prepared_updates。 |
| Intel Mac | `e232fd01` | rawbb3185a3；App／ZIP1,219檔相同，47份runtime與舊native notice不變，簽章／DMG通過。Rosetta十個基礎案例通過，另五個帳號／推文案例三過兩敗；不是Intel實機或IME驗收。 |
| Linux | `8fa36bcd` | tar1,122／deb1,109份regular files，共同exe／授權／README相符，tar另含明列的15份合成fixture／helper。實際loader／dpkg依賴不含WebKit；QEMU／Xvfb下grid／input／paste／long／inline五案通過，自有Xvfb／Openbox均join。無系統中文字型的grid另目視可讀；不是實機、Wayland、IME或原生安裝驗收。 |

Intel兩個web account案例仍失敗：一例停在inline圖片配置，另一例右鍵選單未開；都尚未送出peer期待的PageDown，HTTP零請求。終端帳號及兩個推文導覽案例通過，同一ARM封裝的software web account另一次通過。這些只證明差異仍可重現，未證明是產品或probe／排程的哪個邊界；原失敗、cleanup失敗及原始log保留，不以十項基礎通過替代此缺項。下一步追查手勢／已確認幾何／inline入口。

來源包新增cross-current，從共用immutable bundle＋10,009-byte文件patch還原9db完整580檔／tree，與新三包SHA綁定；三組歷史來源保留。一次還原目錄驗證後移除，沒有為各平台反覆開還原repo。[來源包與限制](slint-product-source.md)。Python178項及actionlint通過；首次actionlint誤填不存在build.yml後改用實際workflow glob，沒有workflow錯誤；GitHub Actions verify未執行。

Intel／Linux固定下載入口已更新，舊App／ZIP／DMG／tar／deb與消費完的封裝input共7根、462,475,264 logical bytes已移除，原hash與小型report保留；Windows兩份包因使用者待驗而保留。容器裡重複transport／output另核對hash後清除，六份自產pyc核對code object等同來源才刪；自有runner已停止，保留安裝好的工具／runtime供下次復用。SDK正常卸載但保留image與固定mount路徑，重掛helper不重解壓；四架構Cargo cache約12.4GiB繼續保留，不再每批clean。共用／未知的舊Created容器、共享VM／base image／registry／SDK工具、必要對照、原始失敗與使用者資料未動。exact paths、UID／resolved／lsof、du／logical及filesystem free分列於 `slint-renderer-api-current/cross-current/cleanup.json`、`container-cleanup.json`、`container-close.json`、`cache-retention.json`；free只記並行觀測。

全部build／來源／封裝／實際probe／失敗證據在 `slint-renderer-api-current/cross-current/`。完整第三方來源、帳號反例、原生／資料／效能矩陣仍未完成；S0–S8保持active，S9未執行。

快取補驗另發現一次主程式重編：先前build／Clippy及其他架構工作完成後，Windows補驗依賴有復用，但root重編超過60秒觀察deadline；外層xwin被終止，原Cargo／rustc後續均確認消失，沒有重啟仍活動的編譯。其後同一快取Windows再次build exit0、wall0.39秒且無Compiling。第一輪逾時保留，觸發重編原因仍待fingerprint證據；不是三架構暖編譯都通過，也不屬於S8應用程式效能結果。SDK重掛／完整3,419項核對／卸載另已通過，沒有重解壓。

補驗cache raw變為9b992ba8，與已交付487c428b共有12個byte不同；解析PE後確認差異僅在COFF／debug timestamp及CodeView GUID欄位，排除此明列metadata後其餘66,985,984-byte檔案內容完全相同。未聲稱原始binary逐byte重現，也未證明GUID生成或主程式重編的原因。固定ZIP／candidate／原建置SHA保持原樣，不拿後來的cache raw改寫已驗收紀錄。

## 2026-11-11 遮黑提示幾何修正與快取環境

已定位上批web account的版面邊界：遮黑數從0變1時，在文章上方新增提示列；Rust已確認的座標仍為origin y118，但Slint下一輪layout移到y158、viewport由612縮為572，原定inline點擊因而落入提示列，沒有HTTP請求。右鍵亦可能遇到過期來源幾何。生產UI改用固定32px底部狀態列容納連線／遮黑提示、揭露及Vim標示；載入列固定高度，返回閱讀按鈕避開狀態列。原有來源lease、焦點、輸入及背景保護不變；正常畫面會固定使用32px，後續效能比較須重核實際viewport。

真實生產Slint UI的獨立完整／局部Skia回歸新增遮黑數0→1→17→0及載入按鈕切換的幾何不變斷言。原UI先失敗（origin／height由0／740變32／708），修正後兩個feature配置通過，並保留逐像素、footer及100次局部GIF更新檢查。固定x64 target產出raw `64a2caf1`，輕量診斷App為 `a472985c`；五個web／terminal帳號與推文案例全部exit0／嚴格Close，HTTP join與遮黑copy／PNG／圖片等原斷言保留。首案控制器曾在staging完成前啟動舊binary，按實際SHA排除並單獨重跑；沒有把該failed改成passed。這些是Rosetta下toolkit／loopback，並非Intel硬體或OS輸入驗收。

ARM同來源raw `59c1c58f`已建置。新輕量原生診斷App雖有本機程序與session frame，CUA無法綁定其視窗；未執行可信OS鍵鼠，結果保持not_ready。逐次自身程序均依PID／start／command確認後結束，最終身分已消失，但沒有正常Close／owner join證據。保留一份最小App和原始log／stack／metadata；Unix IPC開放後仍無法綁定，不能宣稱其為根因。後續改用既有可控制診斷host補驗，不把工具限制當產品通過。這輪完整下載包及來源交付尚未更新。

一般及播放器bin各103過／1 ignored；兩配置繪圖audit、兩組all-target Clippy、fmt通過。診斷僅在既有account probe及明確INLINE_TRACE旗標下輸出幾何／狀態，不記文章或帳密。全部before／after、實際binary綁定及原生限制見 `slint-renderer-api-current/cross-current/account-diagnostic/` 的 `geometry-before.json`、`geometry-after-r4.json`、`fixed-account-acceptance.json`、`gates.json`、`native-os/final-attempt.json`。

Cargo fingerprint另證實host依賴曾因 `MACOSX_DEPLOYMENT_TARGET` 未設定與11.0交替而失效。忽略於Git的固定build config及本機Windows／Linux配方已統一11.0，沿用原target；後續x64只重編實際變動主程式。尚未完成整輪跨target暖建置，不宣稱全部零重編。`cache-environment.json`及兩份x64 build log保留原因與scope，沒有新checkout、target或Cargo clean。

結案核對resolved path／UID／symlink／lsof及SHA後，刪除兩份已取代診斷exe的gzip備份，另清六個已有逐byte證據副本的自有temp目錄，共70,453,084 regular-file bytes；exact八根及保留位置見同目錄 `cleanup.json`。filesystem available252814196736→252885217280 bytes僅記並行觀測。保留固定Cargo cache、目前各平台包、兩個新raw、唯一原始失敗與一份未解CUA最小App；未知系統暫存與共享來源不動。S0–S8持續，S9未執行。

## 2026-11-11 遮黑修正的 Mac 候選同步

兩個Mac固定下載入口已替換為6ad8e39修正的封裝：ARM native `0b2f5786`、Intel native `ae54f0bd`。直接復用上一輪raw59c1／64a2及原runtime，未重編Rust、未新建target。實際原建置a63c303 dirty與frozen三檔patch保留，並確認patch正好等於6ad8e39的產品／測試修改；不將原建置改稱乾淨6ad重編。

封裝後兩個實際exe各通過15案：預設MP4／WebM／OGV、長文、inline、文章input／paste、software／OpenGL／Metal格線、三個account及兩個replies案例，全部exit0／唯一嚴格Close，拒讀Homebrew。格線peer使用parity_grid／received_bytes513的獨立receipt，沒有expected_close欄位；首輪collector誤套其他peer欄位而停下，原exit0／原始receipt與false結果保留，修正schema後核對既有輸出PNG並接續其餘案例，沒有重跑已終結成功的程序或放寬產品斷言。ARM1,238／Intel1,220份App／ZIP payload逐byte一致；50／47個runtime與204／185份原native notice不變，各新增一份runtime重用receipt，簽章及DMG通過。這是合成toolkit／loopback，Intel使用Rosetta，不是原生鍵鼠／IME或S8效能。

既有Dual GIF診斷host亦補試：第一輪launcher停在外部JSON讀取；保存stack後改為內嵌相同合成設定、使用既有owned暫存CWD／log，原生程式確實開始並收到文章。但CUA仍回報running application not found，同時listApps顯示running，沒有可信OS輸入，保持not_ready。兩個自身身分均核對後結束，numeric exit／正常Close仍不成立。此重現取代上一個新ID最小App；原始log／設定／stack保存，過時App已清理。未更改OS權限或操作其他PTTStella視窗。

產品來源交付新增 `filter-current`，同一immutable bundle以hardlink復用，加一份產品patch還原6ad8e39完整580檔／tree，綁定兩個Mac新exe；既有Linux／Windows及歷史mapping保留。沒有再做獨立來源編譯，完整第三方閉包仍not_ready。工具Python178項與實際workflow glob的actionlint通過；首輪指定不存在ci.yml的呼叫失敗另記為命令選擇錯誤，並非workflow驗證通過。

候選index、固定下載alias及來源包已更新。清理兩個過時Mac包的未用部分、一次還原checkout、過時診斷App／exe備份、重複temp證據及39份逐code object核對來源的自產pyc，合計47個exact roots、583,606,841 regular-file bytes。舊ARM App由系統tccd持有native，完整保留於 `filter-package-current/retired-arm64/PTTStella Slint Candidate.app`，不停止系統服務；舊ZIP／DMG已移除。固定Cargo cache、新兩包、Windows待驗包、Linux及來源／對照hash不變。resolved／UID／symlink／lsof／SHA／du及filesystem available251401015296→251989946368 bytes在 `slint-renderer-api-current/cross-current/filter-package-current/cleanup.json`，free只記並行觀測。

本批實際封裝、15案、schema原始結果、來源還原及promotion集中於同目錄；已結束的原生工具限制在 `account-diagnostic/native-known-host/`。下一步同步Windows／Linux幾何修正並處理其餘原生／資料／效能缺項；S0–S8保持active，S9未執行。

## 2026-11-11 Windows／Linux 同步與 PNG 反例

同一worktree／固定Cargo target，以乾淨61d1c3f完成Windows及Linux release與各自all-target Clippy；host deployment target保持11.0，未重建整套依賴。Linux SDK只重掛及核對既有3,419項，原有具名容器／已安裝runtime與工具復用，不新增repo或target。Windows新備妥包native `ac788a45`取代原487c428b，919份ZIP payload中917份逐byte不變，只改exe／candidate.json，libmpv相同；原生未執行。使用者待驗69542cb7包與入口保持不變，沒有轉填舊Windows／IME結果。

Linux固定入口更新native `4fa0a14e`，tar1,122／deb1,109份regular files，共同payload／loader／dpkg核對通過，Depends不含WebKit。QEMU／Xvfb十案中格線、input、paste、long、inline、terminal account及software replies七案exit0／唯一嚴格Close；web software account在第一張PNG匯出前後失敗，default web account／replies兩案在LLVM／QEMU中止，不能以七過代替全部通過。無系統CJK字型的格線另目視確認中文控制項與末行可讀。GUI案例的自有Xvfb／Openbox均join；這不是x64硬體、Wayland、IME、音訊或效能驗收。

首次PNG尚未執行遮黑，也沒有送出PageDown或HTTP。以有界trace確認：同一1100×960視窗、origin y118，匯出前viewport為1076×670，output worker繪製期間改為1076×590；幾何回呼撤銷lease，第二次check拒絕寫檔。舊8fa36bcd封裝的同案亦重現相同PNG錯誤，不能歸因本次遮黑修正。僅snapshot／等待ready及另加一次native event-loop turn都未解決；這些未證實的準備更動與trace已保存完整patch／raw hash，tracked source恢復61d，未放寬產品保護或改expected。最新未解診斷raw ff966063保留於固定target，作為後續追查控制列／版面回呼的單一CLI，不當作已封裝產品。

另外依 [Mesa環境變數文件](https://docs.mesa3d.org/envvars.html#gallium-environment-variables)在自有runner選softpipe、DRAW_USE_LLVM=0重測：未改Slint renderer選擇或關閉raster，準備步驟實驗raw2b906b1e的default replies通過，web account仍因PNG失敗。這是替代Mesa路徑的補充證據，不把原LLVM134改成passed，也不轉填到4fa包。原系統driver／硬體GPU仍未驗。

產品來源在既有6ad來源交付新增 `filter-cross-current`，以同一immutable bundle及新patch還原61d完整580檔／tree，綁定Linux4fa及備妥Windowsac；Mac6ad mapping保留。完整tree比較確認578檔相同、只有兩份文件不同，380份產品選集只是補充；本輪未做獨立來源編譯，第三方完整來源仍not_ready。打包器及workflow相對前輪未改，沿用前輪178項Python／actionlint結果，新增實際artifact與上述原始執行證據，不為文件或清理重建Rust。

原始建置、封裝、十案／所有負例、Mesa補測、舊包baseline、來源還原與promotion在 `slint-renderer-api-current/cross-current/filter-package-current/`。原始失敗與獨特PNG／log保留；不再需要的完整舊Windows／Linux包、包裝input及一次還原目錄已逐檔核對後清理；唯一舊8fa exe壓縮保留作反例control。容器內14個已消費的傳輸／output／runner目錄共552,538,706 regular-file bytes另核對清理，自有runner已停止、runtime／工具保留。host exact paths、UID／resolved／symlink／lsof／du、保護hash與free觀測見 `cross-cleanup.json`；容器分列 `container-cleanup.json`，不將VM logical bytes當作host釋放量。SDK正常卸載、image及Cargo cache保留。S0–S8保持active，S9未執行。

host本輪exact五根共444,095,902 regular-file bytes；filesystem available246859952128→247308451840 bytes只記並行觀測，不全歸因本批。舊ARM App仍被tccd持有而保留，未停系統服務。固定Cargo cache的du為13,007,048KiB（約12.4GiB），沒有full clean；八份目前下載及來源manifest／380份產品輸入的最終hash均核對，來源完整580檔還原證據另列，保留單一未解raw與必要舊control。容器內初始sleep與已終結session bus的namespace在正常stop後退出，沒有其他專案或共享VM被停止。


## 2026-11-11 閱讀狀態列與 Linux 元件回歸

`94bc250`將全文載入與推文更新合併成一個32px閱讀狀態列，文章啟用時即保留空間；控制項晚到、未讀／復原／output busy改變不再額外擠壓已確認viewport。未改Rust來源lease、輸出／輸入權限或取頁時序。新增獨立Skia完整／局部renderer幾何案例，修改前確實因晚到推文控制改變viewport而失敗，修改後一般／播放器兩配置均通過；本輪不再重跑與先前逐byte相同的UI／audit輸入。

`46d7046`修正合成探針的來源准入：account首個匯出等待真實load／updates能力，每張圖片操作前重新等inline來源就緒，並核對指定index的實際圖片已placement；paste退出案例在snapshot後重取可准入來源，仍須證明實際read worker存在及正常join。沒有放寬生產lease、沒有增加假ready或移除Close斷言。`fbcfe2e`將雙色字PNG背景檢查由兩個字型可能上墨的角落，改成每半格嚴格多數的原始不透明藍／紫像素；獨立負例拒絕只剩兩個正確角落、錯色與非法bounds，未改期望色值或產品renderer。

一般／播放器bin各106過、1項明確原生clipboard helper ignored；兩配置all-target Clippy、Linux release／all-target Clippy與fmt通過。最終ELF `150c1a0dd10ade2e76c1686352c8cbefd7cc930cf6945e04251f2eb6987347c7`是f40e793加七檔patch的實際編譯，完整patch逐byte等同f40e793到fbcfe2e。無系統CJK字型的QEMU／Xvfb十四案（grid、input、paste、long、inline、三種account、兩種replies、output、updates append／cancel／blur）全部exit0／唯一嚴格Close；四個曾失敗的paste／web software／web default／output重測亦全過，兩輪WM／Xvfb均join。GL子集使用Mesa softpipe，不是硬體GPU、OS鍵鼠／IME／Wayland或S8效能。原始LLVM中止、GL逾時及未採用的幾何實驗都留存，重測未證明原GL逾時根因已解。

Linux固定下載入口已更新到同一150c ELF，實際loader／dpkg、tar1,122／deb1,109份payload及所有shared檔案核對通過；對舊4fa tar逐檔比較僅exe／build-record兩份改變，原1,103份Rust notices與fixture逐byte復用，Skia／實際建置receipt另外供應。打包器／Cargo.toml／lock未變。來源交付沿用同一immutable bundle與來源包目錄，增加reading-current還原完整580檔／tree；repo外同一暫存checkout在拒讀原worktree來源的sandbox通過一般Slint106項／1 helper ignored，復用固定compiler cache。第三方完整來源閉包仍not_ready。

核對resolved／UID／symlink／lsof及SHA後，清理舊Linux完整包、已消費的最小包裝input、還原checkout與一份自產pyc；另移除53份逐byte重複PNG，保留映射及全部獨特影像。host合計232,285,435 regular-file bytes，filesystem available244075118592→244295110656 bytes只記並行觀測。容器17根共322,715,968 logical bytes另核對host副本後清除，不當作host釋放量；原安裝工具／runtime保留，具名自有runner已停止，SDK正常卸載但image／固定路徑保留。Cargo固定target為13,012,196KiB，按使用者要求保留中間檔，不clean或新建target／repo。舊ARM App仍由tccd598持有，保留且不停止系統服務；四平台必要交付、Windows待驗包、最小舊native controls及原始失敗保留。

全部實際patch、command、原失敗／14＋4結果、archive payload對照、source restore／build、promotion與exact清理路徑在 `slint-renderer-api-current/cross-current/filter-package-current/chrome-current/refined/`；重複PNG舊路徑由cleanup.json追溯保留副本。其他三架構尚未納入此批新修正，Windows69542cb7待使用者稍後驗收的入口不變；Intel／Linux實機依指示暫緩，完整資料／效能及其餘原生矩陣仍未完成。S0–S8保持active，S9未執行。

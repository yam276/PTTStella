# Slint 與原 Dioxus 的資料往返

2026-11-11 已用同一對 macOS arm64 前端完成設定 v1–v6 的 Slint → 原 Dioxus → Slint 往返。各版本以獨立合成資料驗證兩次實際儲存、前一代備份與重開讀取。黑白名單、可攜設定、收藏名稱／離線讀取、最近看板清除及草稿編輯保存的先前證據見後節。完整新增／編輯／刪除、最近看板新增與排序、macOS 一般入口的自動開圖設定往返也已通過，完整平台資料回退、密碼庫與產品切換仍待驗。

## 2026-11-11 同一對前端的 v1–v6 設定往返

Slint 使用以 `7466c31` 建置的最新 arm64 候選，封裝 native SHA-256 為 `f50ffb4658b03f8a7ff855d0ead6b1c035d02d8e7c2d4bedf4f763e150c88a32`。原 Dioxus 仍取乾淨 `5ea9ad6` 的 baseline native，SHA-256 為 `35c3e27506280f430b364b82355e31f7b101a254fcadccc289ac7063ed6a44e3`。兩個測試 App 都放在同一個自有系統暫存目錄，重用各自的執行檔。Slint 透過已核對的 Frameworks 連結載入候選 runtime。每一階段由明確參數的本機程序啟動，透過 App 路徑綁定可及性操作，使用標記過的合成資料與本機服務。

各起始版本的字高都是 21.5。Slint 在外觀與字型改存 22，第一次主檔為 v6。原 Dioxus 讀到 22 後改存 23，Slint 再以新程序讀到 23。六組首次儲存的備份逐位元等於各自的起始主檔，Dioxus 的備份逐位元等於 Slint 剛存的主檔。最後一次讀取後，主檔與備份各自保有讀取前的雜湊。v1 的首次遷移備份因此有獨立證據。

六個版本各自完整比對 JSON。v4 起保留透明度 61，v5 起保留關閉右鍵扇形選單。v6 的 Terminal、自動開圖、文章首尾導覽及英文設定都保留，最後一次 Slint 也在介面確認 Terminal 和自動開圖。站點、名單、收藏、草稿、最近看板與可攜設定的原檔雜湊在各階段相同。v2 的 Dioxus 與 Slint 都實際打開同一篇四行收藏，畫面可見雙色「中」與末行，Dioxus 另有一次 `archive_reads` 收據。草稿正文及 ANSI 由完整檔案比對確認。

18 個程序皆正常 exit0，包含 12 份 Slint 的唯一嚴格 Close，以及 6 份 Dioxus 的 Close、listener／worker 退出及工具畫面 PNG。PNG 逐個檢查 CRC 與 zlib 解壓。Slint Quit 後的可及性觀察逾時，退出判定採程序與 Close 收據。v4 第一次讀取存檔提示時仍在儲存中，同一程序後續顯示已儲存，檔案與備份核對通過。

證據位於 `.local/verification/slint-store-v2-current/`。讀取 `results.json`、`snapshot-inventory.json`、各階段的 expected、實際主檔／備份、程序 log 與 Slint probe 收據。v2 的原生操作單獨保存在目錄根部，其餘版本在 `v1/`、`v3/` 到 `v6/`。用 `cleanup.json` 找重複 PNG 保留下來的副本。

程序全部退出後清理自有暫存根目錄、五個測試身份的 OS 產物路徑及十張重複 PNG，共 112,424,076 個 regular-file bytes。清理前後的 filesystem free 是 239500881920 與 239613779968 bytes，另列為觀測值。精確路徑、UID、symlink、lsof、du、雜湊及兩個來源執行檔的保護核對均記在 `cleanup.json`。

重驗時使用上述兩個執行檔，在新的自有系統暫存目錄建立兩個獨立身份的 App。各 App 將 native 放在 `Contents/MacOS/native`，使用可執行的 `RunFixture` 作為 `CFBundleExecutable`，Slint 的 Frameworks 指向相同候選 runtime。將 `roundtrip_fixture.py` 複製到 repo 內新的 `.local/verification/NEW_RUN/`，在同目錄準備 `launch.json` 和 `baseline-launch.json`。前者記錄 `root`、`app` 和 `host_native_sha256`，後者記錄 Dioxus 的 `app` 和 `host_native_sha256`。控制器會寫入固定本機參數並簽署測試 App，核對 native hash 後啟動程序。

從 repo 根目錄對每個版本依序執行以下三階段。每次啟動後以 App 介面完成前述操作並 Quit，等待該命令通過才開始下一階段。原 Dioxus 的工具模式要再匯出一張整頁 PNG。第三階段只讀字高。控制器自動播種第一階段、保留後續同一組檔案、核對各階段的實際內容及備份。

```sh
rtk proxy python3 .local/verification/NEW_RUN/roundtrip_fixture.py 1 slint
rtk proxy python3 .local/verification/NEW_RUN/roundtrip_fixture.py 1 baseline
rtk proxy python3 .local/verification/NEW_RUN/roundtrip_fixture.py 1 return
```

小數字高的 Dioxus 寫入、其餘資料的完整操作、正式資料路徑、系統密碼庫及其他平台仍待驗。

## 2026-11-11 小數顯示設定回退

原 Dioxus `5ea9ad6`（native `35c3e275…`）可顯示合成資料的字高 21.5、基線 19.125、字寬 102.75，但修改字高為 23.5 後，WebView 以 `step=1` 拒絕自動存檔。核對主檔逐位元相同、未建立備份。原版 Quit 先回報欄位無效，第二次退出時程序為 exit 0，但整頁 PNG 和探針清理未通過，原始失敗報告保留。

Slint worktree 的 Dioxus 入口改用 HTML 數字欄位 `step=any`，對齊 Rust 原有的有限浮點數驗證。三個欄位沿用原上下限，並以 `required` 拒絕空白。Rust schema、鎖和備份不變。Windows WebView2 的既有設定測試已加入三個小數值的實際存檔與重開檢查，該平台仍待執行。[HTML 數字欄位規格](https://html.spec.whatwg.org/dev/input.html)

同一組資料在修正版 Dioxus（native `52ac086b…`）經實際設定 UI 將字高存為 23.5。基線 19.125、字寬 102.75、閱讀模式與自動開圖等欄位都保留。主檔完整值符合獨立 expected，一代備份逐位元等於原始主檔，其他六份資料雜湊不變。實際 PNG 通過 CRC／解壓，Dioxus 正常 exit 0、嚴格 Close 及所有既有 worker／listener 清理收據均成立。接著用現有 Slint arm64 候選 `f50ffb46…` 重開，介面讀回三個精確小數值，主檔及備份雜湊不變，exit 0 且有唯一嚴格 Close。

驗證用的是同一 worktree 的修正版 Dioxus 與既有 Slint。固定的 `5ea9ad6` 原執行檔仍有上述限制。原版／修正版／Slint 的命令、來源補丁、原始收據及清理清單在 `.local/verification/slint-fractional-rollback/`。重跑使用固定 Cargo target 的 `cargo --config .local/slint-build.toml build --locked --offline --release --features desktop --bin pttstella`，依 `native.py` 建立合成資料並啟動三階段。測試 Cargo 命令曾更新 target 頂層執行檔，實測 SHA 以每階段啟動前核對的 native 副本為準。已移除測試 App 根目錄及五個專用系統產物，共 110,444,977 regular-file bytes，保留固定編譯快取與原候選。

## 2026-11-11 macOS 一般啟動與系統資料目錄

沿用已驗的 Slint `f50ffb46…` 與 Dioxus `35c3e275…`，不帶產品探針參數啟動。只覆寫測試程序的 HOME，把合成副本放在 `Library/Application Support/PTTStellaSlintResearch` 與 `Library/Application Support/PTTStella`。Sandbox 拒讀 Homebrew 及實際使用者的兩個產品資料目錄，另拒讀寫其 Keychains 路徑，IP 出站僅准許 loopback。兩個本機測試站各用不同 port，憑證只經該程序的 `SSL_CERT_FILE` 載入，未修改系統信任設定。

兩個一般入口各從自己的 `sites.json` 取到正確端點，以 TLS 1.3 向各自的測試站要求 `GET /bbs`。測試站送出合成畫面，一秒內沒有收到任何客戶端資料，主動送 Close 後兩版均回覆。兩個目錄的七份合成資料雜湊在啟動與連線後保持一致。Slint 的 `--show-profile` 另核對實際設定路徑及查詢時未開 vault。這補上 macOS 系統資料路徑的**預設站讀取**，寫入、回退讀取、vault 操作及一般 App 正常退出仍待驗。

三次實際視窗嘗試都收到本機 frame，但桌面工具未取得可操作的視窗。已保留未註冊、放行 Unix socket 並取消 CFFIXED_USER_HOME 後仍無法綁定，以及經 LaunchServices 啟動後有 App 名稱但無法操作的結果。測試控制器在確認身分後終止自有程序，這些 exit 及最後兩次路徑檢查的 exit 都不是正常 Quit。上一節已有的 `probe-*.json` 寫入往返仍有獨立證據。

證據在 `.local/verification/slint-normal-profile/`，含 `startup-results.json`、兩個實際 sites 副本、前後雜湊及原始 log。重驗時把 `roundtrip.py` 和 `startup-check.py` 放在新的同層證據目錄，先執行 `roundtrip.py setup`，再執行 `startup-check.py`。已清理兩個暫存 App 所在根目錄、唯一測試 TLS 私鑰及專用系統產物，共 110,753,519 regular-file bytes。精確六筆路徑、du、雜湊、程序與 filesystem free 觀測見 `cleanup.json`。全程重用現有兩個執行檔。

## 2026-11-11 macOS 一般入口的設定回退

使用最新 Apple Silicon 候選 `99949409…` 和同 worktree 的修正版 Dioxus `52ac086b…`。兩版均不帶產品探針參數，從隔離 HOME 下各自的一般 `Library/Application Support` 資料目錄讀寫。沙箱封鎖實際產品目錄和 Keychains 路徑，網路只連合成 WSS peer。各段正常退出後手動複製七份合成資料到下一個目錄，不轉換格式。Dioxus release 建置沿用固定 Cargo target；Slint 沿用既有包的執行檔和播放器函式庫。

| 階段 | 實際視窗操作與結果 |
| --- | --- |
| Slint | 從「圖片與媒體」關閉自動開圖。正常保存，v6 主檔按現有 serde 契約省略 false 欄位 |
| 修正版 Dioxus | 實際讀到未勾選，開啟自動開圖後正常保存為 true |
| Slint 新程序 | 實際看到設定已勾選，直接正常退出，主檔及備份雜湊相同 |

兩次保存的一代備份都逐位元等於寫入前主檔，字高 21.5 與其餘設定值保留，另六份資料逐位元相同。三段各 exit 0、TLS 1.3、只送正常 Close frame 且 peer 回覆並結束。保留前後畫面、實際儲存 JSON、SHA-256、正常退出和原始 log。

原 Dioxus `5ea9ad6` 能讀回自動開圖關閉，但既有字高 21.5 使 WebView 拒絕保存這個布林設定。主檔逐位元不變、沒有備份。此反例與修正版分列。第一輪桌面控制未綁定而逾時，第二輪改用 macOS 輔助使用 API 可開設定，但合成鍵盤未完成字高編輯，啟動又超出 peer 監聽期限。後續把啟動納入 runner，使用原生選單及實際滑鼠操作。Slint 首次成功保存的收集器原誤要求主檔明寫 false，依現行 schema 另驗原始主檔、備份及其餘雜湊，保留原始收據。

證據位於 `.local/verification/slint-normal-profile-current/`。以新同層證據目錄放置 `roundtrip.py`、`operate.py`、`native-ui.js`，先執行 `roundtrip.py setup --dioxus-binary` 並指定固定 target 的修正版 `release/pttstella`。依序執行 `roundtrip.py upgrade --launch-services`、`rollback --launch-services`、`return --launch-services`，每段從設定的圖片與媒體操作並使用 App 選單正常 Quit。實際操作與資料核對可在 `results.json` 和各階段收據重查。

本次驗證一般入口與 macOS 資料路徑上的合成設定寫入和回讀。實際安裝升級、正式產品身份及 Keychain 授權、完整資料操作、IME 與其他平台仍待驗。退出後清除六個本批專用 App／測試資料／系統暫存位置，共 110,346,431 regular-file bytes，另清三張相同 PNG 共 258,764 bytes。保留候選、固定 target、合成前後資料及獨特失敗證據。精確路徑、du 與並行可用空間觀測見 `cleanup.json`。

## 2026-11-11 從 macOS 交付包安裝與回退

在 Apple Silicon 從 [v0.4.1 正式 ZIP](https://github.com/yam276/PTTStella/releases/tag/v0.4.1) 解出 Dioxus App，再將候選 App 從唯讀 Slint DMG 複製到同一隔離 Applications 目錄。ZIP SHA-256 為 `118414cb8e0884e8886279520002c2ec062dff18ba8b91f2d90ba13006f2e274`，DMG SHA-256 為 `4792b9860eb57e249a35cce43cb932d1c743e56433419560623e0bb59b25ff16`。兩個 App 的深層嚴格簽章檢查均通過，Slint 的磁碟映像、安裝副本及封裝 App 逐檔相同。實際啟動原封裝執行檔 Dioxus `d22d5f0a` 和 Slint `793ecb91`，沿用各自的 bundle ID。沙箱限制出站至本機，封鎖實際產品 store 和 Keychains 路徑。合成資料在隔離 HOME 下各自的一般資料目錄，由控制器依序複製原檔。

| 階段 | 原生視窗操作 | 退出後核對 |
| --- | --- | --- |
| v0.4.1 基準 | 啟動並顯示合成站畫面，關閉主視窗 | 起始設定未變 |
| Slint 升級 | 關閉文章首尾方向鍵切篇 | 只改 `article_boundary_navigation`，備份等於寫入前的主檔 |
| v0.4.1 回退 | 開啟「右鍵開啟工具」 | 只改右鍵偏好，保留 Slint 的文章設定，備份等於 Slint 主檔 |
| Slint 再次啟動 | 讀到「右鍵開啟工具」已勾選 | 主檔與一代備份逐位元相同 |

另外六份站點、名單、離線收藏、草稿、最近看板和可攜外觀檔在每一步都與起始副本逐位元相同。共八個程序正常 exit 0，合成 WSS 各使用 TLS 1.3，只收到正常 Close，peer 都結束。第一次在背景嘗試從 v0.4.1 選單開設定未開出面板。基準從主視窗正常退出，設定操作在後續前景程序完成。

用已安裝的 Slint 開啟「自動開圖」後，將含 `auto_expand_images: true` 的實際主檔逐位元交給 v0.4.1。舊版顯示未知欄位並改用預設顯示。此時只切換右鍵選項，介面回報已儲存，但主檔的介面語言、字型與字高、浮球透明度及閱讀模式一起變成預設值，自動開圖欄位也消失。第一次儲存仍有完整一代備份，第二次儲存把該備份輪替成預設值版本。另六份檔案都保留。從第一輪獨立保留的備份恢復後，Slint 實際顯示自動開圖仍勾選，主檔逐位元回到原值，SHA-256 為 `612a855747e3e7bb7b414f3b84501cbc54624877534af229a50fb9d485c81f55`。

切換前須選定能讀寫新欄位的 Dioxus 回退包，並另存一份不受 App 備份輪替影響的完整資料。v0.4.1 可讀寫共通設定，但不能直接作為含新欄位資料的回退版。實際產品身份原地替換、Keychain 授權及 Finder 首開仍待驗。

證據位於 `.local/verification/slint-macos-install-rollback/`，包含八階段實際主檔及備份、正負案例畫面、正常退出與 WSS 收據。重跑時保留同層目錄結構，放入校驗過的正式 ZIP 並執行 `install.py`。依序用 `phase.py baseline dioxus`、`upgrade slint --handoff`、`rollback dioxus --handoff`、`return slint --handoff` 啟動並完成表中操作。接著依序執行 `enable-auto slint`、`public-unknown dioxus --handoff`、`public-second-save dioxus`，開啟自動開圖後在舊版各切一次右鍵選項。最後執行 `restore-fixture.py`、`phase.py recovered slint` 及 `verify.py`。啟動控制器與操作使用同一個隔離資料根。各階段須正常退出才開始下一階段。

核對簽章檔案、程序終結、DMG 卸載、resolved path、symlink、所有權和 open files 後，移除本輪隔離安裝位置及下載副本，共 220,889,069 regular-file bytes。filesystem free 觀測由 236,037,439,488 增至 236,264,677,376 bytes。精確路徑、du、SHA-256 與保留清單在 `cleanup.json`。

## 2026-11-11 相容的 Dioxus 回退包與獨立備份

以乾淨 `d66a069` 和既有 macOS 打包流程建立 Apple Silicon Dioxus 回退候選，沿用固定 Cargo target，包含小數外觀設定修正與自動開圖欄位。保留 DMG `PTTStella-0.5.0-dev.3-macos-arm64.dmg`，SHA-256 為 `9e740c0c5b86ce88751593f345babf353f7c7dfac2be10278c581bd848432041`。安裝執行檔為 `14a5ab7f`，Slint 安裝執行檔為 `793ecb91`。從兩個唯讀 DMG 複製簽章 App 到隔離 Applications 目錄後正常卸載，驗證深層簽章與逐檔雜湊。Dioxus 的 ZIP CRC 和所有普通 App 檔也與 DMG 安裝內容一致。這份 DMG 是 S8 的本機回退候選。

一般入口在隔離 HOME 及本機 WSS 依序完成四段操作。

| 階段 | 實際結果 |
| --- | --- |
| Slint 保存 | 關閉文章首尾切篇，保存含自動開圖、字高 23.5、基線 19.125 和字寬 102.75 的 v6 設定 |
| Dioxus 回退 | 讀到原設定，開啟「右鍵開啟工具」並正常儲存。自動開圖、三個小數及其他欄位全部保留 |
| Slint 重開 | 視窗顯示右鍵偏好已勾選，主檔及一代備份逐位元不變 |
| Slint 從獨立備份重開 | 在另一個空白 HOME 讀回 Dioxus 儲存前的設定，介面顯示自動開圖已勾選、右鍵偏好未勾選 |

每次正常退出後，站點、名單、離線收藏、草稿、最近看板和可攜外觀六份檔案都與起始副本逐位元相同。兩次儲存的一代備份各等於該次寫入前的主檔。四個程序皆 exit 0，合成 WSS 使用 TLS 1.3，只收到 Close 並完成回覆。

在 Slint 第一次保存並退出後，將七個主檔及現有的 `settings.json.bak` 複製到兩個 App 資料目錄之外，記錄八個檔的長度與 SHA-256。Dioxus 儲存和輪替其一代備份後，從獨立備份恢復至全新隔離 HOME，八份檔案在恢復前後及 Slint 重開退出後逐位元一致。另在備份測試副本中只修改有效 JSON 的一個布林值，恢復在寫入前因雜湊不符而中止，恢復目的地仍不存在。這條路徑保留現有資料版本，並驗證了上節 v0.4.1 一代備份會輪替時可用的獨立副本。

可重跑材料在 `.local/verification/slint-macos-compatible-rollback/`，包含 DMG、package manifest、實際主檔與備份、程式收據及畫面。以 `install.py` 安裝兩包。執行 `phase.py slint-save slint`，關閉文章首尾切篇並正常退出。執行 `backup-fixture.py save`。執行 `phase.py dioxus-rollback dioxus --handoff`，開啟右鍵偏好並退出。接著 `phase.py slint-return slint --handoff` 核對勾選狀態後退出。最後依序執行 `backup-fixture.py restore`、`phase.py restored slint --recovery-home` 和 `verify.py`。備份腳本只操作這輪合成位置。正式原地替換、Finder 首開、Keychain 與其他架構的回退仍待驗。

Python 打包與工具測試 178 項、actionlint 及 docs 檢查通過。驗後依簽章檔案、程序終結、resolved path、所有權和 open files 清掉隔離 App 與重複 ZIP 及其校驗檔，共 221,536,247 regular-file bytes。filesystem free 觀測由 235,929,083,904 增至 236,157,456,384 bytes。保留已驗的 DMG、獨立備份及證據。

## 同一份資料的入口

新增診斷入口：

```text
pttstella-slint --software-renderer --probe-store NEW_EVIDENCE_DIR MARKED_STORE_ROOT
```

此入口使用既有 Benchmark 本機服務與完整管理 owner，讀取既有 Preferences，而不先寫入預設設定。沿用生產 Store 的 schema、鎖、備份、合併與錯誤；UI 不實作第二份格式。`Profile::system()` 的一般設定／資料目錄與 SlintResearch 密碼庫身份保持原狀。

資料根目錄必須已存在，且 `.store-compatibility.json` 為不超過 1024 bytes 的普通檔案，內容精確為：

```json
{"format":"PTTStella.StoreCompatibility","version":1,"synthetic":true}
```

同一目錄使用原 Dioxus 探針的檔名：`probe-settings.json`、`probe-sites.json`、`probe-filters.json`、`probe-bookmarks.json`、`probe-drafts.json`、`probe-recent-boards.json`。根目錄、marker 及既有 store／一代備份／lock 不接受 symlink 或非普通檔案。缺少 store 時沿用既有預設讀取；壞檔與未來版本保留原 bytes／警告，不偷偷重設。

這是明確標記的合成副本入口，不會自動搬移正式資料，也不代表可以直接把使用中的產品目錄拿來驗收。原 Dioxus 使用其既有 `pttstella --probe-surface MARKED_STORE_ROOT tools`，兩個程序依序退出後才啟動下一個。兩邊讀寫相同檔案，不在階段間轉換 schema 或重新播種。

需要站端草稿情境時，可在相同入口加 `--store-scene drafts`；省略或明確指定 `--store-scene grid` 都保持原雙色格線。場景旗標不能獨立使用或混入其他 probe／verifier。草稿場景沿用嚴格本機 draft peer、完整 owner 與同一組 `probe-*.json`，不使用一般 profile，也不自動進編輯器或保存草稿。

## 可重現資料與操作

fixture 直接重用 `tooling/verify_store_compatibility.py` 的自製 CC0 資料。從 repo 根目錄，選一個不存在的新路徑建立：

```python
from pathlib import Path
import sys
sys.path.insert(0, "tooling")
from verify_store_compatibility import fixture, encoded, MARKER

root = Path("NEW_SYNTHETIC_STORE_ROOT")
root.mkdir()  # Existing paths fail; never overwrite an old profile.
expected, settings_disk = fixture(6, auto_expand=True)  # Repeat with v1 separately.
(root / ".store-compatibility.json").write_bytes(encoded(MARKER))
names = {"settings": "settings", "sites": "sites", "filters": "filters",
         "bookmarks": "bookmarks", "drafts": "drafts", "recent": "recent-boards"}
for key, name in names.items():
    value = settings_disk if key == "settings" else expected[key]
    (root / ("probe-" + name + ".json")).write_bytes(encoded(value))
(root / "display.pttstella.json").write_bytes(encoded(expected["profile"]))
```

使用原生 App 選單開設定，先核對讀入值，再以原生欄位操作保存字高、選原有 WSS 最愛並保存繁中站名；不按「連線切換」。原 Dioxus tools 案例另需明確匯出整頁 PNG，才符合其原有煙霧測試。正常 Quit 後核對程序 exit、連線 Close／worker 清理與完整 JSON，下一個前端才能重開。保存前後各留獨立快照，備份應逐 byte 等於上一份主檔；不能只看到 UI 成功提示。

## 實測結果

證據在 `.local/verification/slint-frontend-roundtrip/frontend-results.json`，各階段有 `run.json`、原始 log、獨立 expected 與 acceptance。Slint 來源為 `842264e` 加本批確切 source patch／285 份 fingerprint；raw CLI SHA-256 為 `8bd1b0333f3c0634169990b96d7013c746a9e28ec49b901d651999dabbdd49ae`。原 Dioxus 從乾淨 `5ea9ad6a498331d0cbebc7d9803e0a51df46e5c3` 建置，無生產來源 patch；raw CLI SHA-256 為 `35c3e27506280f430b364b82355e31f7b101a254fcadccc289ac7063ed6a44e3`。

| 起始資料 | Slint 首次保存 | 原 Dioxus 讀回／保存 | Slint 新程序讀回 |
| --- | --- | --- | --- |
| settings v6，英文、Terminal、自動開圖 | 字高 21.5 → 22.5，站名 `Slint PTT 末字` | 讀到 22.5／完整站名；保存字高 23、站名 `Dioxus PTT 末字` | 讀到 23／完整站名；再保存 20、`Slint 回程末字` |
| settings v1，繁中、Web 預設 | 讀到 21.5／v1 預設；保存為 v6、字高 22、站名 `Slint v1 末字` | 讀到 22／完整站名；保存 23、`Dioxus v1 末字` | 讀到 23／完整站名、Web／繁中；只讀且 JSON／備份 hash 不變 |

六輪皆正常 exit0。四輪 Slint 各有恰好一筆 `slint_full_parity_grid` Close acknowledged；兩輪原 Dioxus 的 surface report 為 passed、disconnect acknowledged，listeners、file／connection／credential worker 皆清理完成，PNG 為 true。兩張 PNG 另核對 chunk CRC 與 zlib 解壓，不以檔名代替圖片證據。

每次保存後完整六個 store 的值符合獨立 expected；同端點 UAO／GBK 最愛、預設站、名單、收藏 archive、草稿 ANSI／尾空白、最近看板與可攜設定未受無關修改。後五份檔案各自與 seed 的 SHA-256 一致，這只證明 byte-preservation，不代表它們已完成 GUI 操作。v6 三次保存，以及 v1 原 Dioxus 保存的一代備份，已逐 byte 核對前一份主檔；v1 首次 Slint 保存的備份未在後續覆寫前另存，不補造該項驗收。

## 黑白名單前端往返

同一已標記 v6 profile 的 `probe-filters.json` 為 v4。以相同的兩份 raw CLI，再完成三個 macOS 原生 UI 輪次，沒有 Rust 重建：

| 前端 | 實際讀取與保存 |
| --- | --- |
| Slint | 讀到 Alice 整行遮罩、Carol 保留 ID 遮罩、Bob 個別 Magenta（5）與共用 Bright magenta（13）；保存 Alice 備註 `Slint 名單末字🦀`，保留策略 |
| 原 Dioxus | 讀到完整備註及 mask_row；保存為 `Dioxus 名單末字🦀` 並改為 mask；核對 Bob 個別／共用底色仍正確 |
| Slint 新程序 | 讀到原 Dioxus 的 mask 與完整備註，再保存 `Slint 回程名單末字🦀`，保留 mask |

三次完整 store 都符合獨立 expected，一代備份各自等於緊接修改前的主檔 bytes，其他五個 store 與可攜設定 byte 不變。三案皆 exit0／嚴格 Close；原 Dioxus 另有 passed surface report、PNG 與 listener／worker 清理。結果在本批 `filters-results.json`，逐階段快照及原始 log 保留。這不代替名單匯入、完整 CRUD、實體 IME、其他平台與正式資料路徑驗收。

## 可攜設定前端往返

Slint 使用原生 NSOpenPanel 載入原 fixture 的 `display.pttstella.json`，正確讀到名稱 `合成可攜設定末字`、字高21.5、基線19、寬102與原字型；存成目前設定後再將字高調22。原位置 NSSavePanel 的 Save／New Folder 停用，因此明確取消，原可攜檔 bytes 不變，正常 Quit／Close。該首輪記 partial，沒有將取消視為匯出成功。

另一輪開始前，將七份合成 JSON 與 marker 放進單一自有暫存目錄，所有前端在這一輪讀寫同一份檔案。未變更權限、系統設定或實作。原生 Save 選到這個位置後啟用，Slint 實際確認覆寫自己的合成可攜檔，匯出名稱 `Slint 可攜末字🦀`、字高22／基線19／寬102及原字型。原 Dioxus 以 Load 實際讀到上述名稱和值，再以 `Dioxus 可攜末字🦀` Export；Slint 新程序透過 NSOpenPanel 讀回完整名稱與相同顯示值。

三個後續輪次皆 exit0／嚴格 Close，原 Dioxus 的 PNG／surface／各 owner 清理也通過，六個 store 的值不受可攜名稱變更影響。Slint 是實際 OS 選檔器；原 Dioxus tools 探針則沿用既有固定 `display.pttstella.json` 路徑，不能算作原 Dioxus 原生選檔器驗收。結果在 `portable-results.json`，最終合成 JSON 另存 `portable-final/`，測試暫存目錄在清理後不保留。

原 Documents 位置在另一新程序明確重選原資料夾後，Save 仍停用；已再次取消並正常退出，`slint-v6-portable-documents` 記 not_ready。原因未確定，不宣稱是 TCC、檔案權限、產品錯誤或輸入工具造成；另一路徑成功也不把原位置改判 passed。小數字高在原 Dioxus 的完整保存限制仍待驗；本次互相匯出採22，只證明該顯示值路徑，沒有據此宣稱所有可攜設定／平台檔案操作完成。

## 收藏副本與最近看板清除

同一份已標記資料再完成三輪 macOS UI 操作，未重建 Rust：

| 前端 | 實際操作 |
| --- | --- |
| Slint | 從浮球進收藏，讀到原名稱及本機副本；改名 `Slint 收藏末字🦀` 並保存，開啟四行離線文章；由收藏交接到 Go to… 審閱，讀到 Stella／Test，清除最近看板 |
| 原 Dioxus | 讀到 Slint 名稱與副本；改名 `Dioxus 收藏末字🦀` 並保存，讀取相同四行；Go to… 顯示空最近清單，再清除一次 |
| Slint 新程序 | 讀到原版名稱；保存 `Slint 回程收藏末字🦀`，再次開啟副本；Go to… 讀到空清單 |

三次完整 bookmark JSON 均符合獨立 expected，原 `created_at`、完整 archive rows／footer／links／ANSI 左右樣式與尾空白不變，archive canonical SHA 一致；一代收藏備份各等於前一份主檔 bytes。最近看板清除是隱私操作：主檔與備份都為 `version:1, boards:[]`，不要求備份保留被清除的歷史；原 Dioxus 讀回及再次清除後，Slint 重開仍為空。其他四個 store 與可攜檔 byte 不變。

三個實際離線視窗另以 CUA screenshot 目視確認四行、雙色「中」及末行仍存在；影像留在工具對話紀錄，未另存一份本機 screenshot，也沒有把目視當成精確 PNG 像素稽核。原版正常完成的整頁 PNG 另有 CRC／zlib 驗證，但那是原 tools 場景匯出，不冒充離線副本 PNG。

三輪皆 exit0／嚴格 Close；原 Dioxus surface report passed、archive_reads／recent_clears 有實際計數，listeners／各 worker 退出。三輪皆只審閱目的地，未按 Go。結果與前後快照在本批 `bookmarks-results.json`。完整新增／刪除／多頁副本／列表收藏、實際站端前往、最近看板新增／排序與其他平台仍另驗，不能由這三輪推定完成。

本輪3份App與原Dioxus專用WebKit／cache／系統暫存已確認退出／無open files後逐筆清除；來源與實際保存資料、兩份必要raw CLI保持hash不變。exact paths、UID／resolved／symlink、du、filesystem free前後及保留例外在 `bookmarks-cleanup/ledger.json`，重複PNG的保留位置／SHA在同目錄 `duplicate-pngs.json`。沒有修改產品程式、重建Rust、切換正式入口或發佈。

## 草稿原生編輯與保存往返

2026-11-10 在 `.local/verification/slint-draft-roundtrip-current/` 以同一 marked-store 副本完成三輪操作，階段間不重播種。Slint 使用目前 arm64 候選的 native（SHA-256 `6841e17e395878d9a90431028410f8e68a3e9d34f5a00f5b181c6f6f76c1d1c1`）及相同 Frameworks 的 symlink；原 Dioxus 使用乾淨 `5ea9ad6` native（`35c3e27506280f430b364b82355e31f7b101a254fcadccc289ac7063ed6a44e3`）。兩份獨立身份的 host 固定診斷參數，不能回退啟動一般產品或讀使用者 profile。

資料沿用 v6／auto-expand fixture，僅在首輪前把三個草稿標題改為唯一名稱，以免選錯。目標 `edited` 的初始正文為三行，含亮紅 ANSI 及末尾換行；另兩筆分別保留尾空白／兩個換行和未完成 `ESC[1;`。預期結果於各階段啟動前保存，所有輸入走 CUA 原生欄位／滑鼠／貼上，沒有直接修改 UI property 或磁碟正文。

| 階段 | 原生操作與結果 |
| --- | --- |
| Slint `slint-corrected` | 讀取既有草稿、修正前輪輸入試探留下的重複 ASCII 前綴，保存完整 `Slint 往返末字🦀`；原亮紅第二行不變。revision 5→6，正常退出／嚴格 Close ACK |
| 原版 `baseline-first` | Ctrl+P 進入本機 peer 編輯器、捕捉模板、恢復目標草稿，目視讀到 Slint 的全文／顏色；改標題為 `本機草稿測試`，實際完成上色，再以 ANSI source 寫入原版 gate 預先固定的三行正文。revision 6→7→8→9→10，exit0／surface passed |
| Slint 新程序 `slint-return` | 讀回原版標題、`第一行中文`／亮紅 `亮紅中文`／`第三行`；末端原生貼上換行及 `Slint 再讀回末字🦀`，保存。revision 10→11，正常退出／嚴格 Close ACK |

原版 gate 原有的 capture／color／save 計數及 `fixture_matches` 未放寬，實際為 1／1／4／true；連線、credential、file worker 和 listeners 均完成清理。原版標準正文的 ANSI bytes 是 `第一行中文\n\x1b[1;31m亮紅中文\x1b[m\n第三行`，不是可見的反斜線文字。每輪一代備份逐 byte 等於上一份主檔；有界 observer 每 50ms 記錄改變的 JSON，原版主檔 revision 6–10 全數捕捉，最終備份正是 revision10。其餘兩筆草稿的全部欄位、其他五個 store 及可攜設定皆 byte／value 核對不變。

首輪 `slint-first` 保留 failed：CUA `typeText` 對中文／emoji 只留下 ASCII；後續原生 paste 能保存完整 Unicode，但沒有據此確定首輪原因。該輪也超過 controller 240 秒，被終止為 -15、無 Close ACK；後續成功不改寫失敗。修正輪在同一份資料上移除重複前綴，沒有重新播種。原版對含 emoji 的全選上色先回報 UAO 無法編碼、沒有送站；換成預宣告可編碼正文後上色成功。這些觀察不等同實體 IME 驗收，也未釐清先前末字異常。

`results.json`、各 phase 的 `acceptance.json`／`run.json`／`native.log`／`store-history.json`、預宣告 expected 及前後合成資料保留。CUA 工具紀錄留有兩版讀回和最終四行的 screenshot，未額外保存重複影像。重跑時從 fixture 在新根目錄建立資料，以保存的 host metadata／`launch-current.py` 重建唯一身份 host，依上述操作序列逐個正常退出；`current-phase.json` 指定不存在的新 phase，不能重用既有 evidence 目錄。原生草稿新建／刪除／衝突、多開、帶回站端、實體 IME、其他平台與正式路徑仍待驗，這一項只結案既有草稿的編輯保存往返。

本輪未修改或重建 Rust。退出及 lsof 確認後移除兩份臨時 App 和五個專用 OS cache／WebKit／暫存目錄，共 110,884,103 regular-file bytes；Frameworks symlink 的既有候選內容保留。3,026 份候選／來源／最終合成資料 hash 不變；filesystem free 254630199296→254741151744 bytes 是並行觀測，與 du 分列，不全歸因本批。exact paths、UID／resolved／symlink、逐檔 SHA 與 du 在 `slint-draft-roundtrip-current/cleanup/ledger.json` 及 inventories；最新四包、原 baseline raw、唯一失敗證據、共享工具及使用者資料保留。

## 保留的失敗與待驗

### 草稿場景準備與格線回歸

後續診斷 CLI 補上上述場景選擇，來源為 `052eb3e` 加 `.local/verification/slint-store-scenes/revision-3/source.patch` 與逐檔 fingerprint，SHA-256 `2301617d9ab3e6570b5bd71fcfb68e7278bb447687745c28aac905f75b87b5c9`。它取代本批先前保留的 Slint raw CLI；原 `5ea9ad6` Dioxus raw CLI 仍保留，平台候選包沒有因此更新。

首輪草稿場景被原本只接受雙色格線的準備檢查拒絕，exit1／嚴格 Close acknowledged；失敗原樣保留。現在準備檢查依場景核對：grid 仍要求原 parity；drafts 要求已安裝的80×24、完整控制碼／可映射字形、同一 generation／revision，以及共用 `drafts::scope` 確認連線就緒且帳號 StellaTest、看板 Stella。原本5秒期限、幾何／繪製交接和16個完整 owner 檢查保留，沒有放寬前景或輸入保護。

修正後兩個 macOS arm64 原生程序各正常 exit0，分別有 `slint_drafts`、`slint_full_parity_grid` 的唯一嚴格 Close ACK，兩份 `prepared.json` 的16個 owner 皆存在。CUA screenshot 可見草稿用合成看板及原雙色格線。然而準備時的前景狀態不代表之後一直有焦點；操作時工具停用，Raise 未改變狀態，座標啟用回報 `noWindowsAvailable`。Ctrl+P 只留下本機欄位文字，隨後清空並正常關窗；沒有進入站端編輯器或保存。該歷史輪次的草稿 UI 往返記為 not_ready，不以兩輪正常啟閉代替；後續編輯保存往返的獨立證據見上節。

原六份 store 中，草稿等五份及可攜設定與場景前快照逐 byte 不變；最近看板則由完整 owner 正常記入已確認的合成 Stella 看板，包括第一輪失敗啟動。這是本情境的真實寫入，不宣稱零副作用或重新播種。結果、快照及失敗在 `slint-store-scenes/scene-results.json`、`before-drafts-v6/`、`after-scenes-v6/` 與各 phase。新測試使用真實 peer／連線，涵蓋 scene 互斥、錯帳號、pending、過期 frame 與正常 shutdown；初次測試漏呼叫 connect 的失敗另保留。最終 library389 passed／4 ignored、bin91 passed／1 ignored、release all-target Clippy／build 通過。

這一輪3份臨時 App、被取代的 raw CLI 與建置 target 已清理；最新診斷 CLI 只留在 `slint-frontend-roundtrip/pending-current/`。exact paths、owner／resolved／symlink／lsof、Cargo 回報、du 與 filesystem free 前後見 `slint-store-scenes/cleanup/ledger.json`。來源與唯一失敗／成功紀錄保留，未整合或發佈。

- 原 Dioxus HTML 字高欄位 `step=1`：能顯示 Slint 的 22.5，但試存 23.5 會顯示欄位不完整／超界且不寫檔。以 23 保存成功，只驗到整數路徑；小數字高的完整 GUI 回退仍未通過，不能從 store 接受浮點數推定 UI 相容。
- 首輪有一般入口被操作工具額外啟動，該輪不宣稱全程零外網／vault。後續固定參數 host 只執行明確合成探針，重複 LaunchServices 啟動不回退到一般 Live。
- 長輪次超過既有 peer 的 300 秒，以及兩個 240 秒 controller deadline，皆保留 failed／終止紀錄。後續短輪次的成功不改寫先前失敗。
- 原生貼上與 App 選單不是實體 IME、完整鍵鼠、主視窗 foreground／pointer、可及性或跨平台證據。名單完整CRUD／匯入、可攜設定所有顯示值／路徑，以及收藏、草稿、最近看板的完整 UI 往返與正式平台路徑／vault 仍須另驗。

Rust library 389 passed／4 ignored、Slint bin 89 passed／1 ignored，release all-target Clippy 與 CLI build 通過；ignored 項不計成已執行。新增測試涵蓋有界精確 marker、舊檔不重播種、壞檔／未來版保留、非普通檔案／symlink 拒絕及 CLI 互斥。這些 gate 與上列原生證據分開計算。

本批建置 target 與臨時 baseline worktree 已用 Cargo／Git 原生流程移除，必要 raw CLI 各留一份供後續 UI 與封裝；臨時 App 在確認無程序／open files 後逐個清理。exact paths、resolved／UID／symlink、du、filesystem free 前後與保留例外在本批 `compile-cleanup/ledger.json`、`host-cleanup/ledger.json` 及 `final-cleanup/ledger.json`。最後清除5份臨時App與9個本批專用WebKit／cache／系統暫存目錄；重複generated PNG的保留位置與SHA另記 `final-cleanup/duplicate-pngs.json`。既有各平台最新候選未被這份未封裝診斷 CLI 取代；沒有整合正式入口或發佈。


名單／可攜設定這一輪的臨時 App、原 Dioxus 專用 cache／WebKit／系統暫存、自有可攜測試目錄及重複 generated PNG，在保存最小資料與證據後逐筆清理；exact paths、owner／resolved／symlink／lsof、du、filesystem free 前後與保留例外記在 `filters-portable-cleanup/ledger.json`。來源與兩份必要 raw CLI 保持 hash 不變，不重建 Rust，也不替換既有各平台候選。


原位置補驗的最後1份App亦已清除，並將4個本批探針獨立身份的小型 OS 偏好 plist 封存後移除原檔；全為已退出且沒有 open files 的測試身份，不動共用或正式偏好。這段 exact paths、保留診斷與 free／du 在 `filters-portable-cleanup/followup/ledger.json`，補驗重複 PNG 映射在其 `duplicate-pngs.json`。原位 not_ready 不受後續文件提交影響。

# 分支與自動發佈

`develop` 是日常開發與一般 PR 的預設分支；`main`／`master` 只保留正式版號。此 repo 的正式分支是 `main`；若存在 `master`，適用相同規則，不需另建或改名。不要直接把功能分支合併或推送到 `main`，也不要在 GitHub 手動建立缺少下載檔的 Release。

## 2026-11-18 Slint 入口切換與發佈封鎖

`develop` 自 `ca09cd7` 起包含 Slint 版，`pttstella` 入口改為 Slint，Dioxus 版改名 `pttstella-dioxus`；切換前最後的 Dioxus 開發線保存在 `dioxus` 分支（`5ea9ad6`），正式版 `0.4.1` 由 `v0.4.1` tag 保存。`main` 維持 `0.4.1`。

PTTStella 改以 GPL-3.0-or-later 授權，讓 macOS 版可內附 Homebrew 的 GPL 版 mpv／FFmpeg。`Desktop Release` 已改為建置並打包 Slint：

- 各 runner 以 `native-player-probe` 跑 Clippy、Slint／lib／multi-instance 測試及原生密碼庫案例，再由 `bundle_slint_release` 以正式名稱、App ID 與安裝路徑打包。
- macOS 以 Homebrew 安裝 libmpv，收集實際 Mach-O 閉包放入 App；Windows 使用固定 SHA-256 的 mpv v0.42.0 官方 LGPL x64 DLL 與 app-local Visual C++ runtime，Setup 沿用原 AppId 原地升級、不再安裝 WebView2；Linux 依賴系統 `libmpv2`，不另行散布。
- 每個平台另發佈 `-sources.tar`：`SOURCES.json` 列出 PTTStella commit、所有 crates.io 套件的下載網址與校驗碼、Skia 來源標籤，以及該平台內附 runtime 的原始碼。macOS 內含每個 Homebrew 套件依 SBOM（舊 bottle 沒有 SBOM 時改用已安裝 formula 固定的網址與 SHA-256）核對的原始碼壓縮檔、formula 與安裝紀錄；Windows 內含 mpv v0.42.0 原始碼及其簽章。官方 Windows DLL 靜態連結的 FFmpeg 等相依、Homebrew formula 以網址引用的補丁仍只有指引，`SOURCES.json` 標為 `partial`。

2026-11-18 [verify run 44310874774](https://github.com/yam276/PTTStella/actions/runs/44310874774) 從乾淨 `c0851b1` 在四個 runner 通過工具／文件／核心檢查、`native-player-probe` 的 Clippy、Slint／lib／multi-instance 測試與 Windows／Linux 原生密碼庫案例，並各產出 3 個下載檔與 manifest：Windows Setup／portable ZIP／sources、兩個 macOS 的 DMG／ZIP／sources、Linux DEB／tar.gz／sources（artifact 約 159／356／361／72 MiB）。前四輪依序修正了 Windows 上的工具測試可攜性、媒體測試伺服器的自連喚醒、Visual C++ runtime 閉包、Intel runner 上沒有 SBOM 的舊 bottle，以及 ZIP 的 1980 年前時間戳。

2026-11-19 macOS 15.7.7 arm64 實機下載同一 run 的 `release-macos-arm64`：四個 SHA256 相符、DMG checksum 有效，App 為 `io.github.yam276.pttstella` `0.5.0`、ad-hoc 簽章且 `codesign --deep --strict` 通過，`spctl` 拒絕（與 `0.4.1` 相同，依說明以「強制打開」放行）。主程式與 50 個 Frameworks 只連系統路徑或 App 內；從 DMG 複製到空的 `/Applications` 後，`--show-profile` 指向正式資料目錄與 Product 密碼範圍且未讀資料。已安裝 App 的隔離探針通過 product shell、終端翻頁、全文載入、草稿、內嵌圖片，以及以自製 320×180 四色 MP4 的 `--verify-media`（軟體渲染四點相符，`DYLD_PRINT_LIBRARIES` 顯示 libmpv／libavcodec 來自 App 內、零 Homebrew 路徑）。一般媒體探針第一次因視窗未在前景失敗、第二次通過，與先前的間歇結果相同。前後正式資料檔 SHA-256 不變。該 artifact 的可執行檔名仍是 `native`（`0.4.1` 為 `pttstella`），只影響程序／當機報告名稱；`4a2f1f6` 改由正式流程使用 Cargo bin 名稱，研究候選維持 `native`。同機從乾淨 `4a2f1f6` 以 `bundle_release.py` 打包的 DMG 內為 `Contents/MacOS/pttstella`，Info.plist、runtime manifest 首項與程序名一致，簽章、零外部連結、`--show-profile`、product shell 與四色影片探針通過，正式資料仍不變。[verify run 44450621456](https://github.com/yam276/PTTStella/actions/runs/44450621456) 從乾淨 `81ec655` 四平台通過；其 macOS arm64 artifact 的 SHA256 相符、sources `complete`，DMG 內為 `Contents/MacOS/pttstella`，簽章、`--show-profile` 與零外部連結通過。實際以正式資料開啟、從 `0.4.1` 升級後的設定與密碼庫存取、回退及 Gatekeeper 隔離下載流程尚待人工操作。

發佈前仍需：各平台安裝／升級／回退與實際啟動（含 Windows 在沒有 WebView2 的環境）、README 與安裝說明更新，以及使用者指定版本。後兩項已於 2026-11-23 的 0.5.0 準備完成，其餘狀態見文末〈0.5.0 發佈準備〉。

## 何時執行發佈

只有使用者明確要求發佈該版本，才進入下列發佈流程。平常的開發、修正、Critic、測試、提交與一般推送都保持在 `develop`；不自行切換或推進 `main`／`master`、建立其他工作／發佈分支、改用正式版號，或建立／合併 Release PR、Tag、GitHub Release。使用者另有明確分支指示時依其指示。

「發佈」包含：在 `develop` 更新正式版號及 README／版本徽章／Release notes，完成驗證，再依既有 Release PR 流程將 `develop` 推進正式分支，建立該版 Tag 與完整下載。測試通過、功能完成或曾授權發佈其他版本，都不會自動啟動這些動作。

## 分支版號規則

- `main`／`master` 的當前產品版號只能是正式 SemVer，例如 `0.2.2`；不得包含 `-dev`、`-alpha`、`-beta`、`-rc` 等預發佈後綴或 `+build` metadata。
- 只有 `develop` 使用 dev 版號，例如 `0.3.0-dev.2`。使用者明確要求發佈後，準備 Release PR 時才暫時改為目標正式版號，讓合併到 `main` 的檔案已是正式版本。
- 正式發佈後將 `main` 整合回 `develop`，再更新 `develop` 的下一個 dev 版號。恢復 dev 版號的提交只留在 `develop`，不得同步回 `main`／`master`。
- `Cargo.toml`、`Cargo.lock`、README 當前版本與版本徽章必須一致；歷史紀錄中的舊版號維持原樣。

## 發佈一個版本

1. 確認使用者已明確要求發佈該版本。在 `develop` 完成變更與驗收，將 `Cargo.toml` 的 package version 更新為目標正式版號，並以 Cargo 更新 `Cargo.lock` 的本產品版本，同步 README、版本徽章與該版 Release notes；當前版本資訊必須一致。使用正式 SemVer，例如 `0.1.0`；不將 dev 或其他預發佈版號合併到 `main`，也不接受 `+build` metadata。
2. 將版本變更提交並推送到 `develop`，建立 **develop → main** 的 PR，標題必須為 `release: v0.1.0`（對應實際版本）。一般版本須高於當前 `main`。已取消且未公開的版本依下方恢復規則辦理。`Release PR policy` 會檢查分支、來源 repo、版本、lockfile、打包回歸與文件；草稿不通過。
   若有 `docs/release-notes/v<version>.md`，發佈時會原樣使用該說明，不另附自動變更清單；沒有版本專用說明時沿用自動摘要。
3. 檢查變更與驗收結果後，合併該 PR。一般 merge commit 或 squash merge 均可；不要使用 rebase merge，發佈需要 GitHub 記錄的 PR merge commit 與觸發 commit 完全一致。
4. `main` 的 `Cargo.toml` 更新觸發 `Desktop Release`。先核對剛合併的 PR 與遞增版本，再平行測試／打包四個架構，全部成功後才建立 `v0.1.0` tag 與 GitHub Release、附上下載檔。依目前分支規則只發佈正式版本，標示 Latest。
5. 將 `main` 合併回 `develop`，再把 `develop` 更新為下一個 dev 版號並同步 README／徽章；此提交不得合併回 `main`。保留兩條長期分支，不要在發佈後刪除 `develop`。

目前沒有自動提交版本、啟動 App、登入站點或發送站內訊息。日常 push 到 `develop` 不會自動執行這套建置。初次安裝 workflow 留在 `develop`，第一個正式 Release PR 會一併帶到 `main`；安裝本身不提高產品版本、不建立 Release。

## 下載內容與驗證邊界

| 平台 | Runner | 下載 |
| --- | --- | --- |
| Windows x64 | `windows-2025` | Setup `.exe` 與免安裝 `-portable.zip`；內含 libmpv、Visual C++ runtime 與授權，不需 WebView2 |
| macOS Apple Silicon | `macos-15` | 拖曳安裝 DMG、包含 `.app` 的 ZIP；App 內含 libmpv 閉包 |
| macOS Intel | `macos-15-intel` | 拖曳安裝 DMG、包含 `.app` 的 ZIP；App 內含 libmpv 閉包 |
| Linux x64 | `ubuntu-24.04` | `.deb` 與 `.tar.gz`；影片使用系統 `libmpv2`（`.deb` 列為 Recommends，執行時才載入；未安裝時程式照常啟動，只停用影片並提示） |

每個平台另有 `-sources.tar` 對應原始碼集，見上方 2026-11-18 說明。以下段落中 WebView2、static CRT 等描述屬 Dioxus 版。

每個架構有來源 manifest，列出 commit、profile、相依環境及下載檔 SHA-256；公開 Release 的 `SHA256SUMS` 涵蓋八個平台下載。Manifest 及其逐檔校驗保留在 Actions artifacts。下載採完整白名單核對，任何缺檔、混入額外檔案、來源不符、dirty checkout 或雜湊不符均禁止發佈。授權資料從同一 locked Cargo dependency graph 收集。Linux `.deb` 以 `dpkg-shlibdeps` 讀取實際 ELF 依賴，使用方式見 [Linux 套件說明](linux-package.txt)。

macOS DMG 和 ZIP 使用同一個已簽章的 App。打包器會驗證磁碟映像，唯讀掛載後確認「應用程式」捷徑指向 `/Applications` 並驗證 App 簽章，卸載成功後才交付兩種檔案。安裝步驟見 [macOS 發佈說明](macos-release.txt)。

所有 runner 執行 Python 打包／政策測試、docs、JS、core、desktop lib／bin、多程序保存整合測試與 desktop Clippy，再以 release profile 編譯。Windows／macOS 沿用原生打包驗證，Linux 核對 ELF／系統 shared libraries／Debian metadata。CI 不啟動產品連線，不把無 GUI 的單元與打包測試當成原生輸入、像素、IME 或實站登入驗收。

Windows job 額外安裝固定版 Inno Setup，執行原生合成前置政策 gate，以靜態 CRT 與內嵌授權的同一次 App 編譯產生 Setup 和免安裝 EXE；本機 ZIP 僅供開發／驗證，不上傳 Release。打包器拒絕非系統 DLL 依賴，將免安裝 EXE 複製至空目錄執行授權匯出，比對編入內容後才交付。這不建立 WebView、不連線，也不當作 GUI 通過。來源 manifest 必須列出兩個 Windows EXE，缺任一個或混入額外 ZIP／裸 DLL 均不能發佈。公開 Release 保留下載檔與 SHA256SUMS，來源 manifest 仍在 Actions artifacts 驗證；使用者擇一下載。兩者的 WebView2 需求及實測邊界見 [Windows 安裝包](windows-installer.md)。

Windows 暫無 Authenticode，macOS 為 ad-hoc 簽章，未做 Developer ID／Apple 公證，因此下載後可能出現系統安全提示。macOS 說明附於 App，見 [macOS 發佈說明](macos-release.txt)。本流程無需另設私人 token 或帳密 secrets，只有發佈 job 取得 `contents: write`，其他 job 為讀取權限；action 以完整 commit 固定版本。

## App 版本檢查

正常啟動及「關於與授權 → 檢查新版本」以無認證 GET 查詢本專案 `/releases/latest`。只接受比實際 package SemVer 更新的正式版本；新 dev 版不會被提示降回舊正式版。按下載才開啟固定 GitHub repo 的該 tag 頁，不自動下載或安裝。

private repo 的 404、權限／限流、網路、逾時或無效回應保持靜默；只有成功比較後才顯示沒有較新正式版本。每次最多一個請求、8 秒 HTTP 總期限、256 KiB 回應上限，拒絕 redirect；surface 結束直接取消所持 future，無背景輪詢／重試、無 token／鑰匙圈存取。合成 probe 不對 GitHub 發出請求。此介面對應 [GitHub Latest Release API](https://docs.github.com/en/rest/releases/releases#get-the-latest-release)，不更動發佈 workflow。

## 手動驗證與失敗重試

Actions → Desktop Release → Run workflow：

- **verify**（預設）：選 `develop` 或 `main`，執行相同四架構測試／打包，產物保留於該 run 的 artifacts 14 天。完全不建立 tag 或 Release，適合修改建置流程後驗證。
- **publish**：只准選已合併 Release PR 的 `main`，重跑未完成的發佈。不是繞過版本與分支檢查的入口。

2026-10-20 初次 verify run 在分配 runner 前因帳戶付款／spending limit 被 GitHub 拒絕，沒有執行建置步驟。2026-10-22 使用者曾將 repo 改為 Public，四架構 verify 全部通過，Release PR 也已合併 main；之後因使用期間偶發卡頓，使用者改回 Private 並優先要求診斷版。未完成的正式發佈 run 已取消，尚無 v0.1.0 tag 或 Release；不將 verify 產物當成正式公開發佈。後續恢復發佈與原生 UI 驗收邊界見 [active plan](active/minimal-browser.md)。

建置失敗不會建立 Release。上傳時先建 draft，全部檔案上傳成功才公開；上傳中斷會保留 draft。優先使用失敗 run 的 **Re-run failed jobs**，沿用成功 build 的 artifacts。重試可跳過 SHA-256 完全相同的 draft asset，不覆寫不同內容。若重新建置導致相同檔名內容不同，需人工檢查並刪除未發佈的 draft 後重試，或使用新版本；既有 tag 只准仍指向相同 commit，不能移動。已公開的 Release 不由 workflow 修改。

## 0.2.0 發佈狀態

2026-10-28 使用者將 repo 改為 Public 後，Release PR #2 的政策與 verify run 40654284739 四架構全部通過，PR 已合併為 main `2ad07554b6440b03b55f60c9d02d6b8b21d29539`。正式 run 40656739145 的四架構測試／打包亦全部成功；最後 publish 因 REST 依 tag 查詢既有草稿回傳 404 而停止，沒有把該 run 記為完整成功。

接手時從同一次正式 run 取回原始 artifacts，使用未修改的 `prepare`、`validate_assets` 與 `publish`；只以已確認的草稿 ID 426430986 解析該次 404，保留合併 PR、乾淨來源、版本、tag 不移動、完整白名單、逐檔 SHA-256 及最後才公開的全部檢查。[v0.2.0](https://github.com/yam276/PTTStella/releases/tag/v0.2.0) 已公開為 Latest，六個下載檔、四份 manifest 與 SHA256SUMS 共 11 個附件全部上傳完成，另核對 GitHub 的上傳 digest 與 manifest／校驗碼一致。

後續 publisher 在 develop 改以 GraphQL 精確 tag 查詢取得 Release ID，再讀取 REST 數字 ID，兼容草稿與正式版；查詢錯誤、無權限、無效 ID 或 tag 不符仍停止。0.2.0 的 tag、main 發佈提交與下載內容保持原樣，不因工具修正改寫已發佈版本。

## GitHub 分支保護

必要規則已準備於 [.github/rulesets/main-release.json](../.github/rulesets/main-release.json)：`main` 必須經 PR、要求 `Release PR policy`、禁止 force-push／刪除、沒有 bypass。該 JSON 是可匯入的設定，放入 repo **不會自動啟用**。

2026-10-20 私人 repo 的 GitHub API 曾對 rulesets 與 branch protection 回覆 403，要求 GitHub Pro 或公開 repo。2026-10-22 暫時 Public 期間查得 rulesets 為空、main 的 protected 為 false，未啟用伺服器端分支保護；使用者稍後已改回 Private，本輪未改動保護規則。PR workflow 能顯示失敗，發佈程式也會拒絕非 Release PR 的 commit，但**目前不能阻止具寫入權限者直接推送或合併到 main**。

若要啟用，在 Settings → Rules → Rulesets 匯入上述 JSON；也可由具有管理權限的使用者執行：

```text
gh api --method POST repos/yam276/PTTStella/rulesets --input .github/rulesets/main-release.json
```

`Release PR policy` 必須維持同名並由 GitHub Actions 提供。啟用後核對規則 active、沒有 bypass，並以不符合條件的測試 PR 確認不能合併，才可宣稱伺服器端限制已完成。

## 0.2.1 原始發佈狀態（已由下列重發取代）

Release PR #3 合併至 main `4f194551a02821e7b80ae78591a125774e74487e`。四架構 verify 40664831408 與正式 run 40667567525 全數成功，publisher 已正常完成。[v0.2.1](https://github.com/yam276/PTTStella/releases/tag/v0.2.1) 為 Latest stable，附六個平台下載套件、四份 manifest 與 SHA256SUMS；獨立核對精確附件清單、版本／來源／release profile、GitHub 上傳 digest、校驗碼與版本專用說明通過。這次仍使用既有 11 附件流程，下列清單精簡留待下一版。main 已整合回 develop，後續文件提交留 develop。

## 公開下載清單精簡

使用者後續指定這次重新發佈 0.2.1 就套用：建置資訊 JSON 保留於 GitHub Actions artifacts，繼續核對版本、平台、來源 commit、簽章狀態及執行檔 SHA-256；公開 Release 只放六個平台下載套件與 `SHA256SUMS`。校驗清單只列公開下載項。完整 manifest／逐檔校驗及建置目錄白名單仍全部驗證，缺少 JSON 或遭竄改也禁止發佈。

## 明確授權的同版本替換

一般流程維持 develop → main 與版本遞增。只有使用者明確要求替換既有版本時，才可從該發佈提交建立 `codex/reissue-v<version>` → main 的 Release PR，標題仍為 `release: v<version>`。必須提交 `docs/release-replacements/v<version>.json`，只含 `version`、`previous_commit` 與非空 `reason`；版本與父提交必須精確符合被替換的 main，不能套用到其他版本或後來的提交。來源仍須同 repo、乾淨 checkout、唯一已合併 PR 及精確 merge commit。這份紀錄不會自動刪除任何 Release 或移動 tag。

先備份舊 Release metadata、tag、Git bundle 與可下載的舊附件並核對 digest，再回移已驗證修復、通過 Release PR policy，合併後手動在 main 跑四架構 verify。全部實際建置通過後，才依明確授權刪除舊 Release 與 tag；以相同 main 提交及同次 verify 的原始 artifacts，重新執行 production `prepare`、`validate_assets`、`publish`。這個手動出版步驟由使用者的本次替換授權觸發；verify workflow 本身仍不具出版權限。一般 publisher 依然拒絕既有公開 Release 或指向別處的 tag，所有下載上傳完才公開。重發後核對遠端來源、六個套件及 SHA256SUMS，再把 main 整合回 develop，保留 develop 的較新開發版本。

## 恢復已取消且未公開的版本

使用者可要求把後續驗證完成的功能納入已合併但尚未發佈的同一版號。沿用 develop → main 的 Release PR，並在該版的 `docs/release-replacements/v<version>.json` 加入 `cancelled_run`。PR 檢查核對原正式分支提交、被取消的 Desktop Release push、同一正式版號，以及 GitHub 上沒有該版標籤或 Release。正式流程再次核對取消紀錄與合併提交。重試只接受指向該合併提交的標籤及未公開草稿，仍須通過原本的完整產物驗證。

## 0.2.1 重新發佈結果（2026-10-28）

Release PR #4 通過政策並合併至 main `1ac078b5a5f2b883f21be657505c823a08145a0c`，四架構 verify [40681843325](https://github.com/yam276/PTTStella/actions/runs/40681843325) 全部成功。先核對該次原始 artifacts 的四份 manifest、全部下載與逐檔 SHA-256，再按本次使用者明確授權，備份並移除原始 Release 426507886 與指向 `4f19455` 的 tag；使用相同乾淨 main 的 production prepare／validate_assets／publish 完成重新上傳及最後公開。verify 未取得或執行出版權限，這次出版由授權的本機 publisher 執行。

[v0.2.1](https://github.com/yam276/PTTStella/releases/tag/v0.2.1) 現為 Release 426610699，Latest stable，tag 指向 `1ac078b`。公開附件只有六個平台套件與 SHA256SUMS，共七個；四份 JSON 與其校驗仍保留於 Actions artifacts。公開校驗清單的六筆 hash、七個 GitHub asset digest／大小及雙語重發說明均獨立核對通過。舊來源與下載備份留 repo 外，細節見 active plan；main 已整合回 develop，後者保留 0.3.0-dev.1。因同版本替換，舊 0.2.1 使用者須手動重新下載，更新檢查不提示相同版號。


## 0.2.2 發佈結果（2026-10-29）

SubAgent Critic 提出的書籤合併超量與設定關窗保存基準問題修正後，Release PR #5 政策及 develop verify [40820260479](https://github.com/yam276/PTTStella/actions/runs/40820260479) 四架構全部通過，合併為 main `029b318d55bcad5d0cac3b37de05c89c18c3f73c`。正式 [40821773621](https://github.com/yam276/PTTStella/actions/runs/40821773621) 四架構測試／打包與 publisher 成功，[v0.2.2](https://github.com/yam276/PTTStella/releases/tag/v0.2.2) 為 Latest stable。公開六個平台下載與 SHA256SUMS，共七個附件，無 JSON；取回同次原始 artifacts，逐項核對四份 manifest、全部校驗、來源／版本、公開 digest／大小與說明通過。多程序整合測試已納入所有架構，CI 不取代 Windows／Linux GUI、IME 或實站登入驗收。main 已整合回 develop，恢復 0.3.0-dev.2；正式 tag 與下載保持不變。


## 0.3.0 發佈結果（2026-11-02）

使用者明確授權合併發佈後，Release PR [#6](https://github.com/yam276/PTTStella/pull/6) 以merge commit `ad81691a40f1173be6215a894d6b4423743e12bc` 推進main；正式 [41381604505](https://github.com/yam276/PTTStella/actions/runs/41381604505) 的來源檢查、Windows x64、macOS arm64／x64、Linux x64測試／原生打包及publisher全部成功。[v0.3.0](https://github.com/yam276/PTTStella/releases/tag/v0.3.0) 已公開為Latest stable，tag精確指向該合併提交。

取回同次正式run的原始artifacts，用未修改的production `validate_assets` 核對乾淨來源、版本／release profile、四份manifest、六個套件、完整白名單及全部逐檔SHA-256；另獨立核對公開七個附件的GitHub digest／大小、六筆SHA256SUMS、版本專用中英說明及Latest狀態，全部通過。公開只含六個下載與SHA256SUMS，沒有JSON；建置manifest仍保留Actions artifacts。下載及獨特校驗證據保留repo外與 `.local/verification/release-030-publish-20261102/`。本機校驗曾在Windows artifact尚未下載完成時因缺manifest停止，待兩個下載程序正常結束後完整重跑通過，未略過或放寬校驗。

main已整合回develop，develop同步Cargo／lock、README與徽章為 `0.4.0-dev.1`，main保持正式 `0.3.0`。本輪只做發佈與版本／文件更新，未新增產品功能，也未把CI當成Windows／Linux GUI或低階實機驗收。沒有建立臨時分支、worktree或本機App／Cargo編譯產物；保留同次正式下載及唯一發佈證據，未刪除其他工作的產物。

## 0.4.0 發佈結果（2026-11-04）

Release PR [#7](https://github.com/yam276/PTTStella/pull/7)政策與四架構verify [41820264264](https://github.com/yam276/PTTStella/actions/runs/41820264264)全部成功後，合併為main `96ff7ff2f74819f219a60294c5142e85f89cf635`。正式 [41822690206](https://github.com/yam276/PTTStella/actions/runs/41822690206) 的來源檢查、Windows x64、macOS arm64／x64、Linux x64測試／打包及publisher全部成功，[v0.4.0](https://github.com/yam276/PTTStella/releases/tag/v0.4.0)已公開為Latest stable，tag精確指向該合併提交。

分別取回verify與正式run的原始artifacts，使用未修改的production `validate_assets`核對版本、乾淨來源、release profile、四份manifest、六個下載與全部逐檔SHA-256。另獨立核對公開七個附件的GitHub digest／大小、六筆SHA256SUMS、版本專用中英說明及Latest狀態，全部通過。公開不附JSON，manifest仍保留Actions artifacts。證據在 `.local/verification/release-040-20261104/`，正式下載保留repo外，已清除本輪被取代的verify下載副本；未建立臨時分支／worktree或本機編譯產物。

main保持正式 `0.4.0`，develop整合main後同步版本、README與徽章為 `0.5.0-dev.1`，預設分支仍為main。CI不代替Windows／Linux原生GUI、IME或新的低階實機驗收。

## 暫停 0.4.1（2026-11-05）

Release PR #8 及四架構驗證 [41978240650](https://github.com/yam276/PTTStella/actions/runs/41978240650) 通過。Windows 認證管理員和 Linux Secret Service 原生測試各自成功，六個驗證下載的來源與 SHA-256 均已核對。PR 合併後收到暫停要求，正式 [41980332119](https://github.com/yam276/PTTStella/actions/runs/41980332119) 已取消。main 指向 `4866476843c45dfedb08067381eb42f52c654633`，未建立 0.4.1 的 tag 或 Release，公開最新版本是 0.4.0。SSH／WSS 開發留在 develop。

## 0.4.1 發佈結果（2026-11-05）

使用者明確要求恢復 0.4.1 並納入 SSH／WSS 與系統密碼庫。前一版候選已合併但未公開，使用原 main 提交與已取消工作 41980332119 的紀錄執行同版號恢復。PR 檢查確認沒有既有標籤或 Release，正式工作再次核對取消紀錄。新增兩項政策測試，包含錯誤來源、錯誤執行狀態、既有發佈與同一草稿重試。Python 84 項、actionlint 1.7.12、fmt 及 docs 通過。

Release PR [#9](https://github.com/yam276/PTTStella/pull/9) 與四架構驗證 [41987004705](https://github.com/yam276/PTTStella/actions/runs/41987004705) 通過。合併提交 `e304be283a4ad12772145c22da4cfc9eca4277b5` 與候選的 Git tree 一致。正式 [41989683683](https://github.com/yam276/PTTStella/actions/runs/41989683683) 的 SSH、原生密碼庫、各架構測試與打包、publisher 均成功。公開 [v0.4.1](https://github.com/yam276/PTTStella/releases/tag/v0.4.1) 是 Latest 正式版，tag 指向該合併提交。

驗證與正式工作各自使用原始產物核對版本、來源、四份 manifest 和逐檔 SHA-256。公開六個套件與 `SHA256SUMS` 的檔案大小、GitHub digest 和校驗清單與正式產物一致，公開說明使用該版中英文件。Manifest 留在 Actions。驗證下載副本已刪除，記錄位於 `.local/verification/release-041-resumed-20261105/verify-artifacts-audit.json`。正式下載和唯一證據保留。main 保持 0.4.1，develop 整合 main 後恢復 0.5.0-dev.3。

## 0.4.2 與 main 狀態（2026-11-23）

0.4.2 由 `dioxus` 分支的 Release PR #11 發佈：合併提交 `f5d0e81` 即 `v0.4.2` tag，正式 [45108430904](https://github.com/yam276/PTTStella/actions/runs/45108430904) 成功，內容為 Dioxus 版。隨後 `d316f7f` 把 `develop`（`bca7bc8`，Slint）直接合併進 `main` 並推送，`main` 的程式碼因此變成標記為 `0.4.2` 的 Slint 版。這次推送改到 `Cargo.toml`，觸發的 [45111110574](https://github.com/yam276/PTTStella/actions/runs/45111110574) 在來源驗證以「Release commit must be the merge commit of one release PR」停止，沒有打包或發佈任何檔案，`v0.4.2` 的 tag 與 Release 不受影響。使用者決定保留 `main`，由 0.5.0 的 Release PR 接續。

## 0.5.0 發佈準備（2026-11-23）

使用者要求準備 0.5.0。先把 `main` 合併回 `develop`（`53f499d`），帶入 v0.4.2 release notes，以及 `release.py` 允許 `dioxus` 分支開 Release PR 與手動驗證的修改；`develop` 保留 `0.5.0-dev.3`，避免 Release PR 把版號帶回 `0.4.2`。接著把 `Cargo.toml`／`Cargo.lock`、README、版本與狀態徽章改為正式版 `0.5.0`，README 的 Windows 列改為免裝 WebView2 的 Setup／portable ZIP，Linux 列註明影片需要 `libmpv2`，[Windows 安裝說明](windows-installer.md) 改以 0.5.0 為主、0.4.2 及更早的 Dioxus 說明移到獨立段落，並新增 [v0.5.0 release notes](release-notes/v0.5.0.md)。

截至準備時仍未人工驗收：Windows 實機覆蓋 0.4.2 Setup 安裝、沒有 WebView2 時啟動、輸入法與認證管理員密碼沿用；macOS 以正式資料從 0.4.x 升級、退回 0.4.2 與 Gatekeeper 隔離下載；Intel Mac 與 Linux 實機啟動及影片。Release PR 合併前由使用者決定是否先補這些驗收。

## 0.5.0 發佈結果（2026-11-23）

使用者在上述實機驗收完成前明確要求合併。Release PR [#12](https://github.com/yam276/PTTStella/pull/12) 的 Release PR policy 與 [verify run 45166309720](https://github.com/yam276/PTTStella/actions/runs/45166309720)（`751ad47`，四平台）通過後，以一般 merge commit `df6f0835fdff2de058d3c91ef9f549d70edce83e` 合併，其 Git tree 與 `751ad47` 相同。正式 [45172233180](https://github.com/yam276/PTTStella/actions/runs/45172233180) 通過來源驗證、四平台打包與發佈，建立指向該合併提交的 `v0.5.0` tag 及 Latest Release，公開 Windows Setup／portable ZIP、兩個 macOS 的 DMG／ZIP、Linux DEB／tar.gz、各平台 `-sources.tar` 與 `SHA256SUMS`；`SHA256SUMS` 與 GitHub 記錄的 12 個 asset digest 一致。

發佈後依使用者要求在說明補上效能比較：先更新 [v0.5.0 release notes](release-notes/v0.5.0.md)，再以 `gh release edit` 把同一份文字寫入已公開的 Release，tag 與下載檔不變。效能表沿用 Slint 評估期間在 Apple Silicon Mac 的同負載量測，Windows、Intel Mac、Linux 與實站仍未量測。

`main` 隨後快轉合併回 `develop`，`develop` 改為 `0.6.0-dev.1`，此提交不合併回 `main`。上一節列出的 Windows、macOS 升級／回退、Intel Mac 與 Linux 實機驗收仍未執行。

## 0.5.1 發佈準備（2026-11-26）

使用者要求發佈 0.5.1。`develop` 自 0.5.0 起只有修正與文件：網頁閱讀與終端共用 BBS 框（`36336d5`）、閱讀首尾改為最上一列與頁尾狀態列（`62a4a64`）、按住時的導覽游標（`9841542`）、展開圖片兩側可導覽（`4dcb213`）、移除底部網址列（`35ed3b0`）、長文載入中網頁閱讀仍接受點擊且返回一次即離開、PTT SSH 等待回覆時略過視窗通知（`58f2528`），以及 Windows 安裝說明、AGENTS.md 與 CONTRIBUTING 的文件更新。`Cargo.toml`／`Cargo.lock`、README、版本與狀態徽章改為正式版 `0.5.1`，新增 [v0.5.1 release notes](release-notes/v0.5.1.md)。

macOS arm64 實機已安裝並使用過同一批修正的本機測試版（`ec9136d`，0.6.0-dev.1）。Windows、Intel Mac 與 Linux 的實機驗收仍未執行，與 0.5.0 相同。

2026-11-27 使用者要求將 PTT SSH 修正（`58f2528`）併入 0.5.1。這項修正只以本機 OpenSSH 9.9p2 重現及合成測試驗證，PTT 實站尚未測試；上述 `ec9136d` 測試版也不包含這項修正。

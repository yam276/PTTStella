# 分支與自動發佈

`develop` 是日常開發與一般 PR 的預設分支；`main` 保留發佈版本。不要直接把功能分支合併或推送到 `main`，也不要在 GitHub 手動建立缺少下載檔的 Release。

## 發佈一個版本

1. 在 `develop` 完成變更與驗收，將 `Cargo.toml` 的 package version 提高，並以 Cargo 更新 `Cargo.lock` 的本產品版本；兩者必須一致。使用 SemVer，例如 `0.1.0` 或 `0.1.0-rc.1`，不接受 `+build` metadata。
2. 將版本變更提交並推送到 `develop`，建立 **develop → main** 的 PR，標題必須為 `release: v0.1.0`（對應實際版本）。版本必須高於當前 `main`。`Release PR policy` 會檢查分支、來源 repo、版本、lockfile、打包回歸與文件；草稿不通過。
3. 檢查變更與驗收結果後，合併該 PR。一般 merge commit 或 squash merge 均可；不要使用 rebase merge，發佈需要 GitHub 記錄的 PR merge commit 與觸發 commit 完全一致。
4. `main` 的 `Cargo.toml` 更新觸發 `Desktop Release`。先核對剛合併的 PR 與遞增版本，再平行測試／打包四個架構，全部成功後才建立 `v0.1.0` tag 與 GitHub Release、附上下載檔。預發佈版本自動標示 Pre-release；正式版標示 Latest。
5. 將 `main` 合併回 `develop`，保留兩條長期分支。不要在發佈後刪除 `develop`。

目前沒有自動提交版本、啟動 App、登入站點或發送站內訊息。日常 push 到 `develop` 不會自動執行這套建置。初次安裝 workflow 留在 `develop`，第一個正式 Release PR 會一併帶到 `main`；安裝本身不提高產品版本、不建立 Release。

## 下載內容與驗證邊界

| 平台 | Runner | 下載 |
| --- | --- | --- |
| Windows x64 | `windows-2025` | 單一 Setup `.exe`，包含 App／資產、app-local CRT 與授權；自動補裝缺少的 WebView2 |
| macOS Apple Silicon | `macos-15` | 包含 `.app` 的 ZIP |
| macOS Intel | `macos-15-intel` | 包含 `.app` 的 ZIP |
| Linux x64 | `ubuntu-24.04` | `.deb` 與 `.tar.gz` |

每個架構有來源 manifest，列出 commit、profile、相依環境及下載檔 SHA-256；Release 的 `SHA256SUMS` 涵蓋所有下載與 manifest。下載採完整白名單核對，任何缺檔、混入額外檔案、來源不符、dirty checkout 或雜湊不符均禁止發佈。授權資料從同一 locked Cargo dependency graph 收集。Linux `.deb` 以 `dpkg-shlibdeps` 讀取實際 ELF 依賴；不是通用靜態 Linux 程式，使用方式見 [Linux 套件說明](linux-package.txt)。

所有 runner 執行 Python 打包／政策測試、docs、JS、core、desktop lib／bin 與 desktop Clippy，再以 release profile 編譯。Windows／macOS 沿用原生打包驗證，Linux 核對 ELF／系統 shared libraries／Debian metadata。CI 不啟動產品連線，不把無 GUI 的單元與打包測試當成原生輸入、像素、IME 或實站登入驗收。

Windows job 額外安裝固定版 Inno Setup，執行原生合成前置政策 gate，再將完整 staged App 打成 Setup；本機打包器的 ZIP 僅供開發／驗證，不上傳 Release。來源 manifest 只列一個 Windows Setup 下載，缺 Setup 或混入額外 ZIP／裸 DLL 均不能發佈。校驗碼與來源 manifest 仍附於 Release，使用者安裝只需下載 Setup。Setup 的補裝、網路需求、解除安裝與實測邊界見 [Windows 安裝包](windows-installer.md)。

Windows 暫無 Authenticode，macOS 為 ad-hoc 簽章，未做 Developer ID／Apple 公證，因此下載後可能出現系統安全提示。macOS 說明附於 App，見 [macOS 發佈說明](macos-release.txt)。本流程無需另設私人 token 或帳密 secrets，只有發佈 job 取得 `contents: write`，其他 job 為讀取權限；action 以完整 commit 固定版本。

## 手動驗證與失敗重試

Actions → Desktop Release → Run workflow：

- **verify**（預設）：選 `develop` 或 `main`，執行相同四架構測試／打包，產物保留於該 run 的 artifacts 14 天。完全不建立 tag 或 Release，適合修改建置流程後驗證。
- **publish**：只准選已合併 Release PR 的 `main`，重跑未完成的發佈。不是繞過版本與分支檢查的入口。

2026-10-20 初次 verify run 在分配 runner 前因帳戶付款／spending limit 被 GitHub 拒絕，沒有執行建置步驟。使用者選擇稍後處理；恢復 Actions 額度後需重新執行 verify，不能將本機 macOS 打包結果當成 GitHub 四架構建置已通過。詳細驗證狀態見 [active plan](active/minimal-browser.md)。

建置失敗不會建立 Release。上傳時先建 draft，全部檔案上傳成功才公開；上傳中斷會保留 draft。優先使用失敗 run 的 **Re-run failed jobs**，沿用成功 build 的 artifacts。重試可跳過 SHA-256 完全相同的 draft asset，不覆寫不同內容。若重新建置導致相同檔名內容不同，需人工檢查並刪除未發佈的 draft 後重試，或使用新版本；既有 tag 只准仍指向相同 commit，不能移動。已公開的 Release 不由 workflow 修改。

## GitHub 分支保護

必要規則已準備於 [.github/rulesets/main-release.json](../.github/rulesets/main-release.json)：`main` 必須經 PR、要求 `Release PR policy`、禁止 force-push／刪除、沒有 bypass。該 JSON 是可匯入的設定，放入 repo **不會自動啟用**。

2026-10-20 此私人 repo 的 GitHub API 對 rulesets 與 branch protection 均回覆 403，要求 GitHub Pro 或公開 repo。因此目前 PR workflow 能顯示失敗，發佈程式也會拒絕非 Release PR 的 commit，但**不能阻止具寫入權限者直接推送或合併到 main**。這個限制不以 workflow 檔案冒充已解除，也不更改 repo 可見性或購買方案。

方案支援後，在 Settings → Rules → Rulesets 匯入上述 JSON 並啟用；也可由具有管理權限的使用者執行：

```text
gh api --method POST repos/yam276/PTTStella/rulesets --input .github/rulesets/main-release.json
```

`Release PR policy` 必須維持同名並由 GitHub Actions 提供。啟用後核對規則 active、沒有 bypass，並以不符合條件的測試 PR 確認不能合併，才可宣稱伺服器端限制已完成。

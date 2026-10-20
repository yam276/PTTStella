# Windows 安裝版、免安裝版與 WebView2

Windows x64 Release 提供兩種產品下載，擇一使用：

- `PTTStella-<version>-windows-x64-setup.exe`：安裝、捷徑與解除安裝，偵測／補裝缺少的 Microsoft Edge WebView2 Evergreen Runtime。
- `PTTStella-<version>-windows-x64-portable.exe`：免安裝直接開啟，無需外帶 CRT DLL 或 Assets 目錄；電腦須已有 WebView2，不會自行安裝 Runtime。

另附校驗碼與來源 manifest 供驗證。Windows 10／11 x64 為目標，ARM 與其他 Windows 版本未納入此交付。

兩者使用同一次 release build，透過 Rust `crt-static` 靜態連結 CRT，並以實際 PE imports 拒絕非系統 DLL；仍由 Windows 提供系統 DLL。MSVC 編譯器不隨產品交付，使用者不需安裝 Visual Studio。JS／CSS、介面翻譯、浮球圖示與內建字型由 Rust 內嵌；使用者額外指定的本機字型仍須自行合法安裝。Setup 安裝後仍有圖示、授權與 manifest 等檔案。

免安裝表示不建立安裝／解除安裝登錄；設定、最愛與 WebView 快取仍使用目前 Windows 帳號的資料目錄，並非將所有使用者資料帶在 EXE 旁邊。需要完全離線使用時，電腦應事先具備 WebView2。

免安裝 EXE 內含同一 locked dependency graph 的授權 inventory、字型及 Fluent 授權，可在 PowerShell 匯出至尚不存在的檔案；此命令不開視窗、不連線、不需建立 WebView：

```text
& .\PTTStella-<version>-windows-x64-portable.exe --export-licenses .\THIRD-PARTY-NOTICES.txt
```

macOS 交付 ZIP 內的一個 `.app`，Finder 視為單一 App，內部仍為標準目錄套件。Linux 提供 `.deb`／`.tar.gz`，程式同樣內嵌上述資產，但 GTK／WebKitGTK 等系統相依套件由發行版提供，並非跨發行版單檔免安裝程式。

## 使用者流程

1. Setup 提供繁體中文／英文，預設安裝到 `%LOCALAPPDATA%\Programs\PTTStella`，採目前使用者範圍。
2. 按「安裝」後，讀取 Microsoft 公開的 Runtime `pv` registry 值，同時檢查 HKLM／HKCU 的 32／64-bit view。空白、無效或零版本不當作已安裝。
3. 已有有效版本就跳過 bootstrapper。缺少時，準備安裝頁會顯示需要網路下載 Runtime，再執行內附的 Microsoft 官方 bootstrapper `/silent /install`。這是使用者啟動 Setup 後的必要依賴安裝，不在正常 App 啟動時偷偷下載或安裝。
4. 執行後再次檢查 Runtime。不能只憑退出碼 0 宣稱補裝完成；啟動失敗、網路／政策造成 Runtime 仍缺少、完整性錯誤等情況停在安裝前，顯示錯誤供使用者重試。需要重新啟動而 Runtime 尚不可用時停止並提示重啟後再安裝。不自動重試、重新啟動 Windows 或啟動 PTTStella。
5. 依賴可用後才複製 App、圖示、授權與 manifest，建立開始選單捷徑與解除安裝入口。解除安裝不移除共用 WebView2、不刪除使用者設定或登入資料。

目前使用者安裝不要求管理員權限；實際 WebView2 安裝範圍與允許的操作仍由 Microsoft bootstrapper、既有 Edge Updater 與 Windows 管理政策決定，失敗時不繞過政策。Evergreen 的後續更新由 Microsoft 管理，無需將完整 browser runtime 放進本產品 ZIP。

偵測位置與命令依 [Microsoft 部署文件](https://learn.microsoft.com/en-us/microsoft-edge/webview2/concepts/distribution)。Setup 的前置檢查使用 Inno Setup 的 [PrepareToInstall](https://jrsoftware.org/ishelp/topic_scriptevents.htm)，發生錯誤時不進入檔案安裝階段。

## 建置與來源

Windows 安裝 Inno Setup **7.1.0** 後執行：

```text
python tooling/bundle_windows.py --installer --portable --out-dir dist/windows-setup
```

可用 `INNO_SETUP_COMPILER` 指定 `ISCC.exe`。`--portable` 收集授權並編入 EXE，保留既有編譯旗標，最後追加 `-C target-feature=+crt-static`；明確指定 Windows target，避免將此旗標套到 host proc macros。打包器不信任旗標本身，另核對實際 PE imports、GUI subsystem 與圖示。免安裝 EXE 還須在空目錄成功匯出完全相同的授權文字，否則禁止交付。此模式的 Setup、免安裝 EXE 與本機 ZIP 共用同一 App，僅編譯一次。

既有輸出不得覆寫；缺少 compiler、下載／驗證失敗、非系統 DLL 依賴、授權匯出失敗或 Setup 編譯失敗，均不交付本次產物。省略 `--portable` 的舊本機 ZIP 模式仍可打包 app-local CRT，僅供開發／驗證。

GitHub workflow 以 [installer-tools.json](../tooling/windows/installer-tools.json) 的官方固定下載網址及 SHA-256 安裝編譯工具，不假設 runner 預載 Inno Setup。該雜湊對應上游 GitHub immutable release 的 asset digest，更新工具時一併核對上游來源。語言檔使用編譯工具內附官方翻譯，本產品自訂訊息另外存於 `tooling/windows/installer.*.isl`。

WebView2 bootstrapper 取自 Microsoft Evergreen 官方端點 `https://go.microsoft.com/fwlink/p/?LinkId=2124703`。建置時有 HTTPS、16 MiB 上限、socket／整體下載期限，以及 Windows Authenticode 有效性與 `Microsoft Corporation` 發行者檢查。將已驗證 SHA-256 編入 Setup，安裝時解出同一檔案再次核對。bootstrapper 本身嵌於 Setup、只在缺少 Runtime 時執行；不放入 portable ZIP，不放入使用者的 App 安裝目錄。Windows Release manifest 記錄 bootstrapper 版本、簽章結果與 SHA-256。

本機打包器在選定的 EXE、ZIP 與校驗碼全部完成才原子交付；競爭建立檔案時保留已有產物，清除本次新建的輸出。ZIP 保留作為開發／解壓驗證產物；Release 只收錄 Setup 與免安裝 EXE，不發佈 ZIP、裸 DLL 或資產資料夾。缺任一個 EXE 即禁止發佈。App 與 Setup 目前未作產品 Authenticode 簽署；Microsoft bootstrapper 的簽章驗證不能當作產品已簽署。

## 驗證

```text
python -m unittest discover -s tooling -p 'test_*.py'
python tooling/check_windows_installer.py --output target/verification/windows-installer
```

一般 Python 測試使用自製 bytes 與注入的工具結果，不接觸 Microsoft 下載、registry 或真實 Runtime。涵蓋 bootstrapper 下載／簽章邊界、失敗零交付、來源目錄／manifest、靜態 CRT 旗標與環境保留、拒絕 CRT／WebView2Loader／其他非系統 DLL、授權 inventory、空目錄匯出及不符內容拒絕、同一編譯、競爭交付、Release 兩個 EXE 必要與額外裸檔拒絕。Rust 測試核對授權匯出與不覆寫。合成工具結果不能代替真正 Windows 編譯及執行。

Windows 原生打包時會真正執行免安裝 EXE 的授權匯出；仍須另在乾淨的 Windows 10／11、只有 EXE 的資料夾確認 App 啟動、字型、Vim 與正常斷線，以及 WebView2 缺少時的實際啟動行為。不能把無 UI 的匯出測試當作 GUI 通過。

第二個命令僅在 Windows／Inno Setup 可用時執行，否則回報 `not_ready` 並退出 2。它以真實 Inno compiler 編譯並執行同一個 `webview2.iss` 決策函式，僅替換 registry／bootstrapper 系統操作為合成 adapter；11 項 assertion 核對四個 view 跳過、無效值、成功後再檢查與重入、失敗／取消、退出 0 卻仍缺少 Runtime、重啟及例外。probe 不安裝產品、不讀寫真實 registry、不執行 Microsoft bootstrapper；報告明示這個邊界。workflow 的 Windows job 將該原生政策 gate 視為必要，失敗不發佈。

原生政策測試與 Setup 編譯都不能代替完整安裝驗收。仍需在可拋棄的 Windows 10／11 測試環境確認：已有 Runtime 跳過；缺少時成功補裝後啟動 App；無網路／政策拒絕時清楚失敗；按安裝前取消不補裝；升級與解除安裝保留設定及共用 Runtime。若共享 Runtime 已補裝，即使後續取消 App 安裝也不移除它。不要為測試移除日常電腦上其他 App 共用的 WebView2。當前實際結果見 [active plan](active/minimal-browser.md)。

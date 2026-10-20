# Windows 安裝包與 WebView2

Windows x64 Release 同時提供 `PTTStella-<version>-windows-x64-setup.exe` 與原有 ZIP。Setup 偵測並補裝 Microsoft Edge WebView2 Evergreen Runtime；ZIP 保持可攜、不會自行安裝 Runtime。Windows 10／11 x64 為目標，ARM 與其他 Windows 版本未納入此安裝包。

## 使用者流程

1. Setup 提供繁體中文／英文，預設安裝到 `%LOCALAPPDATA%\Programs\PTTStella`，採目前使用者範圍。
2. 按「安裝」後，讀取 Microsoft 公開的 Runtime `pv` registry 值，同時檢查 HKLM／HKCU 的 32／64-bit view。空白、無效或零版本不當作已安裝。
3. 已有有效版本就跳過 bootstrapper。缺少時，準備安裝頁會顯示需要網路下載 Runtime，再執行內附的 Microsoft 官方 bootstrapper `/silent /install`。這是使用者啟動 Setup 後的必要依賴安裝，不在正常 App 啟動時偷偷下載或安裝。
4. 執行後再次檢查 Runtime。不能只憑退出碼 0 宣稱補裝完成；啟動失敗、網路／政策造成 Runtime 仍缺少、完整性錯誤等情況停在安裝前，顯示錯誤供使用者重試。需要重新啟動而 Runtime 尚不可用時停止並提示重啟後再安裝。不自動重試、重新啟動 Windows 或啟動 PTTStella。
5. 依賴可用後才複製 App、CRT、圖示、授權與 manifest，建立開始選單捷徑與解除安裝入口。解除安裝不移除共用 WebView2、不刪除使用者設定或登入資料。

目前使用者安裝不要求管理員權限；實際 WebView2 安裝範圍與允許的操作仍由 Microsoft bootstrapper、既有 Edge Updater 與 Windows 管理政策決定，失敗時不繞過政策。Evergreen 的後續更新由 Microsoft 管理，無需將完整 browser runtime 放進本產品 ZIP。

偵測位置與命令依 [Microsoft 部署文件](https://learn.microsoft.com/en-us/microsoft-edge/webview2/concepts/distribution)。Setup 的前置檢查使用 Inno Setup 的 [PrepareToInstall](https://jrsoftware.org/ishelp/topic_scriptevents.htm)，發生錯誤時不進入檔案安裝階段。

## 建置與來源

Windows 安裝 Inno Setup **7.1.0** 後執行：

```text
python tooling/bundle_windows.py --installer --out-dir dist/windows-setup
```

可用 `INNO_SETUP_COMPILER` 指定 `ISCC.exe`。既有輸出不得覆寫；沒有 Inno Setup、官方 bootstrapper 無法下載／驗證，或 Setup 編譯失敗，均不交付本次的 ZIP／Setup。兩者使用同一次 release build 的 App 與相同 staged package，不會再次編譯出不同內容的 App。

GitHub workflow 以 [installer-tools.json](../tooling/windows/installer-tools.json) 的官方固定下載網址及 SHA-256 安裝編譯工具，不假設 runner 預載 Inno Setup。該雜湊對應上游 GitHub immutable release 的 asset digest，更新工具時一併核對上游來源。語言檔使用編譯工具內附官方翻譯，本產品自訂訊息另外存於 `tooling/windows/installer.*.isl`。

WebView2 bootstrapper 取自 Microsoft Evergreen 官方端點 `https://go.microsoft.com/fwlink/p/?LinkId=2124703`。建置時有 HTTPS、16 MiB 上限、socket／整體下載期限，以及 Windows Authenticode 有效性與 `Microsoft Corporation` 發行者檢查。將已驗證 SHA-256 編入 Setup，安裝時解出同一檔案再次核對。bootstrapper 本身嵌於 Setup、只在缺少 Runtime 時執行；不放入 portable ZIP，不放入使用者的 App 安裝目錄。Windows Release manifest 記錄 bootstrapper 版本、簽章結果與 SHA-256。

Setup、ZIP 與校驗碼全部完成才原子交付；競爭建立檔案時保留已有產物，清除本次新建的輸出。Release 須同時具備 ZIP 與 Setup，缺少其中之一不能發佈。App 與 Setup 目前未作產品 Authenticode 簽署；Microsoft bootstrapper 的簽章驗證不能當作產品已簽署。

## 驗證

```text
python -m unittest discover -s tooling -p 'test_*.py'
python tooling/check_windows_installer.py --output target/verification/windows-installer
```

一般 Python 測試使用自製 bytes 與注入的工具結果，不接觸 Microsoft 下載、registry 或真實 Runtime。涵蓋下載超限／期限／HTTP／非 EXE、保留既有檔案、拒絕未知簽章或不符雜湊、Setup 失敗零交付、同源 ZIP／Setup、競爭交付與 Release 缺 Setup 拒絕。

第二個命令僅在 Windows／Inno Setup 可用時執行，否則回報 `not_ready` 並退出 2。它以真實 Inno compiler 編譯並執行同一個 `webview2.iss` 決策函式，僅替換 registry／bootstrapper 系統操作為合成 adapter；11 項 assertion 核對四個 view 跳過、無效值、成功後再檢查與重入、失敗／取消、退出 0 卻仍缺少 Runtime、重啟及例外。probe 不安裝產品、不讀寫真實 registry、不執行 Microsoft bootstrapper；報告明示這個邊界。workflow 的 Windows job 將該原生政策 gate 視為必要，失敗不發佈。

原生政策測試與 Setup 編譯都不能代替完整安裝驗收。仍需在可拋棄的 Windows 10／11 測試環境確認：已有 Runtime 跳過；缺少時成功補裝後啟動 App；無網路／政策拒絕時清楚失敗；按安裝前取消不補裝；升級與解除安裝保留設定及共用 Runtime。若共享 Runtime 已補裝，即使後續取消 App 安裝也不移除它。不要為測試移除日常電腦上其他 App 共用的 WebView2。當前實際結果見 [active plan](active/minimal-browser.md)。

# macOS App 打包

`tooling/bundle_macos.py` 在 Mac 上以同一 Cargo package 的 `desktop` feature 編譯 release，再建立標準 `PTTStella.app`。JS／CSS 已由 `include_str!` 編入程式，不需要 Node、外部 server 或另一套 renderer。

## 建立與開啟

需要 Python 3.11+、`rust-toolchain.toml` 指定的 Rust，以及 Xcode Command Line Tools。若尚未安裝 Apple 工具，可先執行 `xcode-select --install`。目前只建立本機架構；Apple Silicon 產生 arm64，Intel 產生 x86_64，不是 Universal Binary。

在 repo 根目錄執行：

```text
python3 tooling/bundle_macos.py
open dist/macos/PTTStella.app
```

採用本機 Rust 資源 harness 的環境，必須依工作規則用 build wrapper 包住整個 `python3 tooling/bundle_macos.py`，而不是直接執行。腳本尊重 wrapper 的 Cargo／target 設定，從 Cargo JSON 取得實際 executable；完成的 app 保存於 `dist/macos`，不留在可能被回收的 target。其他獨立 checkout 不依賴個人 harness。

預設產物為 `dist/macos/PTTStella.app`，可以用 Finder 開啟或移到 `/Applications`。正常開啟會自動連線 PTT，由使用者手動登入；打包本身不啟動程式、不連 PTT、不讀私人帳密。

為保留既有產物，輸出路徑已有 `.app` 時會在編譯前拒絕。可另指定目錄：

```text
python3 tooling/bundle_macos.py --out-dir dist/macos-next
```

## 打包內容與檢查

macOS 保留原生 PTTStella／Edit／Window 選單：`⌘W` 關窗、`⌘Q` 結束、`⌃⌘F` 切換全螢幕，並提供 Zoom／Minimize。關窗與結束共用原本的 renderer、媒體及 Connection 清理流程，Command 快捷鍵不傳至 PTT。

- `Dioxus.toml` 保存顯示名稱、bundle identifier 與 publisher；Cargo.toml 是產品版本的來源。
- `Contents/MacOS/pttstella` 是實際 release executable；`Contents/Info.plist` 保存 bundle metadata，`Resources` 目前為空，使用系統預設圖示。
- plist 版本使用 Cargo 版本的三段數字，例如 `0.1.0-dev.1` 對應 `0.1.0`；這不是發佈 build number 管理。
- `otool -L` 必須只列出系統函式庫／framework。若未來加入非系統動態函式庫，腳本會拒絕，須先實作 bundle 內依賴與路徑管理。
- 暫存 app 通過 `plutil -lint`、ad-hoc `codesign`、`codesign --verify --deep --strict` 及 `lipo -archs` 後才移至最終路徑。失敗不會留下被誤認為成功的最終 app。

腳本直接使用 Cargo，避免 Dioxus CLI 0.7.9 的額外 `cargo +nightly rustc --unit-graph` 進度估算；該探針會觸發本機 guard 的工具鏈變更拒絕。這不改變產品的 Dioxus／WebView 架構，也不需要修改資源 harness。

## 散布與驗收邊界

目前 ad-hoc 簽章供本機測試，不是 Developer ID 簽章，也沒有 Apple 公證。若要提供給其他人的 Mac，另需 Developer ID Application 簽署、hardened runtime、Apple notarization 與實際下載安裝驗證；`.dmg`、自訂圖示、Intel／Universal Binary、App Store 與自動更新不包含在本次打包。

標準 bundle 結構見 [Apple Bundle Programming Guide](https://developer.apple.com/library/archive/documentation/CoreFoundation/Conceptual/CFBundles/BundleTypes/BundleTypes.html)，正式散布要求見 [Apple notarization 文件](https://developer.apple.com/documentation/security/notarizing-macos-software-before-distribution)。

產生 `.app`、簽章有效、視窗啟動及完整功能驗收是不同證據。既有 `tooling/check.py desktop` 仍只有 Windows backend；macOS 尚不能靠打包成功宣稱 IME、媒體 codec、剪貼簿、像素或實站登入通過。實際狀態只維護在 [active plan](active/minimal-browser.md)。

`--probe-window OUTPUT_DIRECTORY` 是另外的原生視窗回歸入口，不開 DevTools port，不連 PTT，設定與 PNG 僅存指定目錄。真實視窗內開啟 Vim，依序 j、k、gg、G；本機 WebSocket 逐筆核對 Down／Up／Home／End。其後以原生關窗／結束快捷鍵退出；有額外站端輸入、序列未完成或 owner 清理失敗就不產生通過報告。`window-probe.json` 只表示這個有限流程的 listener／worker 與輸入檢查通過，並非完整 desktop gate。

關閉視窗／⌘Q 會先送 WebSocket 正常關閉通知，最多等候兩秒讓站端完成關閉握手，再清理視窗。工具列「斷線」也使用同一流程，並區分站端已回覆 Close、未回覆即結束連線、逾時／錯誤未確認；不以 BBS 選單腳本或登出按鍵代替協定關閉。

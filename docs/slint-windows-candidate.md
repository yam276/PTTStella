# Slint Windows x64 完整候選

這是 S8 內部驗收包，含現有 Rust 核心、原生 Slint 主程式與 libmpv，尚未切換正式 Dioxus 入口。Windows 原生啟動、IME、DPI、可聽音訊、硬體 GPU、乾淨安裝、資料回退與效能仍未驗，`native_acceptance: not_run` 保留。

## 本次交付與編譯方法

最新包在 Slint worktree 的 `.local/verification/slint-windows-current/package-current/`：

| 檔案 | SHA-256 |
| --- | --- |
| `PTTStella-Slint-Candidate-0.5.0-dev.3-windows-x64.zip` | `c6dac792bdfabf96e7384af9dea2cf7426e37d929a48f2873d6ce4a3fcc8f735` |
| 包內 `pttstella-slint.exe` | `04249364f01389873704de7166fd1ffc30f29bc83174aa12d3e7326f6c7caa91` |
| 包內 `libmpv-2.dll` | `94fb87247732c59915cd59808bc582e843a71c22214b7b6c1e5563aa8149cef4` |

Rust／Cargo 來源是乾淨提交 `c40f7a5`，已包含通知、正常影片退出、設定搜尋先決條件與系統動畫偏好接線。285份來源 fingerprint 在建置前後一致；主程式為66,856,448 bytes。打包器及批次檢查 helper 是本輪另行變更，`package-final-command.json` 保留當時的 dirty provenance 與兩個輸入檔hash，提交後再核對；不將後來的 commit 冒充建置時狀態。

這份 PE 是在 macOS arm64 用 Rust、cargo-xwin、Windows SDK 與 LLD **交叉編譯**，沒有 Windows runner 執行。沿用已安裝的專用 xwin SDK，將官方 libmpv 的原始 COFF import archive 以 `mpv.lib` 名稱交給 MSVC target 的 LLD；沒有把 macOS dylib 放進 Windows 包。完整命令、工具／環境與實際exit在本批 `cross-build.json`／`clippy.json`。精簡重現入口為：

```text
MPV_LIB_DIR=<已核對的 import archive 目錄> CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 cargo xwin build --release --locked --no-default-features --features native-player-probe --bin pttstella-slint --target x86_64-pc-windows-msvc --target-dir <本批自有絕對路徑>
```

libmpv 沿用已核對的 [mpv v0.42.0 官方 LGPL Windows x64 資產](https://github.com/mpv-player/mpv/releases/tag/v0.42.0)，DLL bytes 與前包相同；來源URL、archive／DLL hash及授權在 `licenses/libmpv-provenance.json`。這不是精確第三方對應來源的完整交付／重播，`source_delivery: not_ready` 保留。

PE 一般／延遲匯入已檢查：主程式是 AMD64 Windows GUI，直接連到 `libmpv-2.dll`，沒有 WebView／WebKit／Wry 匯入；仍需系統 `MSVCP140.dll`／`VCRUNTIME140.dll`，見 [Microsoft VC++ Redistributable 說明](https://learn.microsoft.com/en-us/cpp/windows/latest-supported-vc-redist/)。這只證明靜態及延遲匯入閉包，不能代替原生啟動、動態載入或無 WebView2 主機驗收。最低 OS 與缺服務／缺 GPU fallback 也仍需實測。

## 在 Windows 執行

核對 ZIP SHA-256，解壓到新的可寫資料夾，不覆蓋舊包。先執行 `Check-All.cmd`，無須帳密，僅啟動合成畫面、本機 BBS／HTTP fixture，不登入 PTT、不改 OS 動畫偏好。每次建立新的 `evidence/native-<UTC>-<GUID>/`，即時保存 `native-results.json` 與逐案log／receipt。不要將 loopback 的 toolkit 操作記成真正 OS 鍵鼠或 IME。

| 批次案例 | 必要結果 |
| --- | --- |
| software／default shell，2案 | shell與通知動作、hover／Tab／時序，以及 timer／deadline 退出 |
| software／default motion，2案 | 真實 Windows UISettings 初值、通知政策、observer移除與UI listener join；不切換 OS 值 |
| 版本檢查，1案 | 本機三次HTTP、重試／舊offer、停滯請求取消、worker／listener join |
| 設定搜尋，1案 | 兩語言48個不同目的欄位、草稿／隱私與視窗關閉 |
| 目前文章收藏，1案 | 真正保存結果、擷取／file worker／window 退出 |
| MP4／WebM／OGV各兩種renderer，6案 | 已進入播放、非空四色像素、renderer退休／session join；software走Quit，default走關窗 |

每案獨立等待退出，90秒逾時就記失敗；終止只針對自己啟動的PID，無法確認停止時保留PID並停止新增案例。stdout／stderr同步排空避免pipe卡死，退出後還有有界讀取等待。判定必須有exit0、恰好一筆對應fixture的嚴格Close ACK、非空且符合型別的receipt；影片另外依固定四色／30級容差重新核對像素，不能只相信`passed`欄位。

motion的來源確實不可用且正常清理時，單列 `not_ready`，不當成功。Windows事件要求Windows10 version2004／build19041以上。[Microsoft UISettings 事件要求](https://learn.microsoft.com/en-us/uwp/api/windows.ui.viewmanagement.uisettings.animationsenabledchanged?view=winrt-26100) 其他錯誤仍列failed。整組結果只有全部13案通過才是 `component_cases_passed_manual_pending`／exit0；有failed回1，有not_ready回2，不以全skip或部分成功冒充整體完成。

批次入口使用系統 `powershell.exe`，不修改執行原則。若本機不允許執行`.ps1`，保留 `evidence/checks-*.log`，可改用 `Check-Shell.cmd`／`Check-Shell-Software.cmd`、`Check-Motion.cmd`／`Check-Motion-Software.cmd`，各自保存結果，不必為驗收更改系統政策。這份script採PowerShell5.1可用語法／Process API；目前只在主機PowerShell7.6.9驗證parser與自製receipt fixtures，Windows PowerShell5.1執行本身仍未驗。

手動原生驗收使用下列入口，結果另列，不能由批次推定：

| 入口 | 人工操作 |
| --- | --- |
| `Run-Offline.cmd` | 浮球／設定／關於、正常關窗、縮放、失焦／恢復 |
| `Run-Grid.cmd`／`Run-Grid-Software.cmd` | 80×24、雙色字、末列、100／125／150%、跨螢幕DPI、全螢幕 |
| `Run-IME.cmd` | Microsoft注音組字、候選位置、取消／選字／一次提交；檢查合成bytes |
| `Run-Reading.cmd` | 文章兩模式、捲動、尋找、選取／複製及快捷鍵 |
| `Run-Drafts.cmd` | 繁中與末字、保存、關窗、新程序讀回；合成稿在`evidence/drafts` |
| `Run-Video.cmd`／`Run-Video-Software.cmd` | 明確播放、可聽音訊、暫停／拖曳與關閉 |
| `Show-Profile.cmd` | 只列隔離資料／vault scope，不讀寫store |

正常雙擊exe會連研究設定的預設站（初始PTT WSS），不自動登入；上列合成入口不需要實站帳密。請回報Windows版本、顯示倍率、執行入口及`native-results.json`的status／失敗或not_ready案名。無WebView2的啟動需在原本沒有該runtime的環境另驗，不要求卸載日常使用的WebView2。密碼庫、舊資料回退、實站與同負載效能依 [完整計畫](slint-evaluation-plan.md) 另驗。

## 本機證據與清理

Windows release build／all-target Clippy通過；Python166項包含PowerShell helper的正向及空trace、重複／錯fixture的Close、假布林、假OS來源、未join、空／錯像素與重複搜尋目的地反例。這些是host fixture／打包證據，沒有執行Windows主程式。最終ZIP916份payload逐檔hash一致，helper／launcher bytes與原始碼一致，校驗檔、build record與實際PE相符。

較早的docs gate與尚在產生的打包staging並行，正確攔到暫存DLL；保留失敗，待producer完成及清理後重跑，不放寬隔離規則。第一次actionlint只有PATH缺少工具，改用canonical checkout已安裝的精確工具路徑；不將未執行算passed。

已移除本批target（Cargo原生clean8237檔／2.8GiB，du2992204KiB）、重複DLL輸入、本批第一版包與被取代的舊Windows完整包；最終包從`package-final`正常rename為`package-current`。兩版必要metadata／授權封存1822項，沒有保留舊完整exe／DLL；2878份來源、asset、fixture、各平台最新包及必要來源hash不變。獨立無影片S1配置與自有mpv import archive仍有用途，另列保留。沒有新Rust重建，所有target都已移除；worktree du1377536KiB。filesystem available 211087757312→214248906752 bytes僅為並行觀測。exact paths、owner／resolved path／symlink／lsof、Cargo／du與保留理由見 `.local/verification/slint-windows-current/cleanup/ledger.json`。

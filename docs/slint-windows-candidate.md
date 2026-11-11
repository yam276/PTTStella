# Slint Windows x64 完整候選

使用者待驗包保持 `.local/verification/slint-input-cross/windows-package/`，產品來源475897f（288份建置輸入），exe `69542cb7…`、ZIP `60f7c551…`；使用者會稍後驗收，原生仍not_run。既有13案與注音回報只屬舊04249364…exe，未轉填。此待驗包尚未包含後續字型／貼上與保留raster修正。

2026-11-11 另備新包於 `.local/verification/slint-renderer-api-current/cross-current/windows-package/`。乾淨 `35b43e4` 交叉 build 及 all-target Clippy 通過；PE `b436d99c…` 含文章焦點修正。與最新 Linux 的 580 份 tracked 檔案一致；與最新兩個 macOS 候選有 380 份相同產品輸入，完整 tree 只差四份文件。919 份 ZIP payload 有 917 份與前一另備候選相同，僅 exe／candidate.json 改變。已核對一般／延遲 PE 匯入，仍需 VC runtime；原生未執行。另備包內完整視窗探針仍預期舊版面，下一次封裝須納入[固定狀態列的探針修正](slint-evaluation-plan.md#2026-11-11-固定狀態列的完整視窗驗收)。`69542cb7` 為使用者的待驗入口。完整 SHA 在[主計畫](slint-evaluation-plan.md#2026-11-11-文章焦點修正的-windows-與-linux-候選同步)所列索引；[來源還原](slint-product-source.md)選 `article-focus-cross-current` 或 `windows-intel-475-baseline`。完整第三方來源仍待驗。

這是 S8 內部驗收包，含現有 Rust 核心、原生 Slint 主程式與 libmpv，尚未切換正式 Dioxus 入口。已收到 Windows 10 x64 的原生元件證據；13 案經修正腳本後重驗既有回報通過。完整 IME 矩陣、DPI、可聽音訊、硬體解碼、乾淨安裝、資料回退與效能仍待驗（Microsoft 注音三項已有使用者人工回報），尚不能平替。包內 `native_acceptance: not_run` 是打包時狀態，後續實機結果見下節。

## 先前交付與編譯方法

當時交付包在 Slint worktree 的 `.local/verification/slint-windows-current/package-current/`：

| 檔案 | SHA-256 |
| --- | --- |
| `PTTStella-Slint-Candidate-0.5.0-dev.3-windows-x64.zip` | `c33737bc730cd6c3a176eef37a753a2e6b3bf24c230c8de4f923de1a5f275a7a` |
| 包內 `pttstella-slint.exe` | `04249364f01389873704de7166fd1ffc30f29bc83174aa12d3e7326f6c7caa91` |
| 包內 `libmpv-2.dll` | `94fb87247732c59915cd59808bc582e843a71c22214b7b6c1e5563aa8149cef4` |

Rust／Cargo 來源是乾淨提交 `c40f7a5`，已包含通知、正常影片退出、設定搜尋先決條件與系統動畫偏好接線。285份來源 fingerprint 在建置前後一致；主程式為66,856,448 bytes。打包器及批次檢查 helper 是本輪另行變更，`package-final-command.json` 保留當時的 dirty provenance 與兩個輸入檔hash，提交後再核對；不將後來的 commit 冒充建置時狀態。

這份 PE 是在 macOS arm64 用 Rust、cargo-xwin、Windows SDK 與 LLD **交叉編譯**，建置時沒有 Windows runner 執行；後續使用者提供的原生證據另列。沿用已安裝的專用 xwin SDK，將官方 libmpv 的原始 COFF import archive 以 `mpv.lib` 名稱交給 MSVC target 的 LLD；沒有把 macOS dylib 放進 Windows 包。完整命令、工具／環境與實際exit在本批 `cross-build.json`／`clippy.json`。精簡重現入口為：

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

批次入口使用系統 `powershell.exe`，不修改執行原則。若本機不允許執行`.ps1`，保留 `evidence/checks-*.log`，可改用 `Check-Shell.cmd`／`Check-Shell-Software.cmd`、`Check-Motion.cmd`／`Check-Motion-Software.cmd`，各自保存結果，不必為驗收更改系統政策。這份script採PowerShell5.1可用語法／Process API；目前只在主機PowerShell7.6.9驗證parser與自製receipt fixtures，原版已在Windows PowerShell5.1實際執行；本次helper修正僅在主機PowerShell7.6.9重驗既有原生回報，尚無修正版在5.1的重跑。

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

## 2026-11-10 Windows 實機回報與腳本修正

使用者提供 Downloads/evidence，透過 Finder 複製到 `.local/verification/slint-windows-current/evidence/`，原始 101 份檔案保持不變。OS 為 Windows 10 x64 build 19045，PowerShell 5.1.19041.6456；顯示倍率未提供。執行檔 hash 與交付包相符。

原始 `native-results.json` 為 failed：10 案 passed，三種格式的 default 影片都在驗收 helper 讀取不存在的 `frame_serial` 時失敗。這是 helper 將軟體 frame 的格式套到 OpenGL framebuffer 回報；三案原生程序皆 exit0、未逾時、已退出，四色像素、資源退休及 Close ACK 皆存在。原始 failed 不覆寫。

修正為依 `render_path` 驗證：software 要求正整數 frame serial／size；OpenGL 要求非負座標與正整數寬高的 rect。兩路都保留非空四色、固定 expected、30 級容差、正常退出／join 和嚴格 Close ACK。新增缺 serial／rect、零尺寸、負座標及未知路徑的反例。以修正版對全部 13 案原始回報與 log 離線重驗通過，記為 `existing_native_receipts_passed_manual_pending`；這不是重新執行 Windows 主程式，也不是人工操作通過。完整回報與原始檔 hash 在 `receipt-recheck.json`。

最新 ZIP 只更新 `Check-Native.ps1` 與相應 `candidate.json` hash，其餘 915 份 ZIP 項目保持逐 byte 一致，未重建 Rust。原包 ZIP hash `c6dac792bdfabf96e7384af9dea2cf7426e37d929a48f2873d6ce4a3fcc8f735`、錯誤 helper、manifest 與逐檔 hash 保存於 `runner-fix/`。舊 ZIP 與打包時暫時提取的 exe／DLL 已清除，僅留最新必要交付；exact paths、du、open-file 檢查及 filesystem free 前後在 `runner-fix/cleanup.json`。

這批證明實機啟動、shell、OS 動畫初值與 listener 清理、版本檢查、設定搜尋、收藏與三格式 software／OpenGL 影片元件。它不證明實體鍵鼠、IME、DPI、可聽音訊、硬體解碼、OS 動畫值變更、無 WebView2 主機、乾淨安裝、資料回退或效能；修正版 helper 的 Windows 5.1 重跑也另列待驗。

## 2026-11-10 Microsoft 注音人工回報

使用者依同一 Windows 包的 `Run-IME.cmd` 提示，回覆「三項都正常」：Microsoft 注音候選窗位置、Esc 取消與選字後單次提交。這三項列為 `human_reported_passed`，紀錄在 `.local/verification/slint-windows-current/user-ime-20261110.json`。

這是使用者的原生操作回報，並未另收到或讀取這一輪的 IME bytes log、截圖或新的執行檔 hash；不把它擴成已核對2 bytes、完整組字／快捷鍵／失焦／DPI矩陣，其他輸入法及音訊仍待驗。先前自動元件證據的 Windows 10 build19045／PowerShell5.1 與這份人工回覆分開保存，顯示倍率仍未提供。

## 本機證據與清理

Windows release build／all-target Clippy通過；Python166項包含PowerShell helper的正向及空trace、重複／錯fixture的Close、假布林、假OS來源、未join、空／錯像素與重複搜尋目的地反例。這些是host fixture／打包證據，沒有執行Windows主程式。最終ZIP916份payload逐檔hash一致，helper／launcher bytes與原始碼一致，校驗檔、build record與實際PE相符。

較早的docs gate與尚在產生的打包staging並行，正確攔到暫存DLL；保留失敗，待producer完成及清理後重跑，不放寬隔離規則。第一次actionlint只有PATH缺少工具，改用canonical checkout已安裝的精確工具路徑；不將未執行算passed。

已移除本批target（Cargo原生clean8237檔／2.8GiB，du2992204KiB）、重複DLL輸入、本批第一版包與被取代的舊Windows完整包；最終包從`package-final`正常rename為`package-current`。兩版必要metadata／授權封存1822項，沒有保留舊完整exe／DLL；2878份來源、asset、fixture、各平台最新包及必要來源hash不變。獨立無影片S1配置與自有mpv import archive仍有用途，另列保留。沒有新Rust重建，所有target都已移除；worktree du1377536KiB。filesystem available 211087757312→214248906752 bytes僅為並行觀測。exact paths、owner／resolved path／symlink／lsof、Cargo／du與保留理由見 `.local/verification/slint-windows-current/cleanup/ledger.json`。

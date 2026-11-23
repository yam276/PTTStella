# Slint 系統動畫偏好

2026-11-10 最新候選已改用 `.local/verification/slint-merged-id-spaces/arm64-package/`，包含重複推文 ID 空格修正（產品來源 `26d9947`）。檔案、雜湊及本輪範圍見[最新四架構候選](slint-evaluation-plan.md#2026-11-10-最新候選)。以下原生／模擬與 SHA 紀錄屬先前候選，不能套用成新二進位的實機結果；舊包的必要 metadata／notice 另封存在本批 `retired-package-metadata/`。

Slint 正常入口現在唯讀訂閱作業系統的動畫偏好，將初始值及後續事件送進既有浮球慣性與通知政策；不新增定時查詢、獨立 runtime 或常駐 thread。macOS arm64 已驗實際初始值、本機通知回呼、software／OpenGL 通知及退出。實際切換 OS 偏好、其他三架構的原生事件仍未驗，不將接線或交叉 Clippy 當作 S7／跨平台驗收完成。

## 來源與降級

`motion_preferences` 不依賴 Slint，以單一 watch channel 只保存最新讀值；UI 的 `system_motion::Owner` 是唯一訂閱擁有者。`Pending`、已知值及 `Unavailable { stage, detail }` 分開保存；未取得值或服務失敗時保守停用動畫，沒有假造「OS 已開啟減少動態效果」。錯誤文字有 UTF-8 容量上限並保留明確截斷標記。

| 平台 | 唯讀來源 | 訂閱及邊界 |
| --- | --- | --- |
| macOS | `NSWorkspace.accessibilityDisplayShouldReduceMotion` | 用 workspace 自己的 notification center、主 operation queue 訂閱 accessibility display 變更；先訂閱再讀初值，回呼重新讀當下值。block 只持有可跨執行緒的 channel／atomic，不持有 UI。 |
| Windows | `UISettings.AnimationsEnabled` 的反值 | 保留 `AnimationsEnabledChanged` token；初讀與回呼的讀取／發布共用 mutex，避免舊初值覆蓋新事件。handler 使用事件 sender，不形成持有 UISettings 的循環。 |
| Linux | Settings portal 的 `org.freedesktop.appearance/reduced-motion` | 標準 u32 的 1 為減少動態；0 及未知值依規格代表無偏好，未知 raw u32 仍保留。只有缺 key／method 才嘗試 GNOME `enable-animations` bool 反值；格式錯誤明確報 unavailable。 |

macOS API 與通知來源見 [Apple 屬性](https://developer.apple.com/documentation/appkit/nsworkspace/accessibilitydisplayshouldreducemotion)及 [Apple 通知](https://developer.apple.com/documentation/appkit/nsworkspace/accessibilitydisplayoptionsdidchangenotification)。Windows 事件要求 Windows 10 version 2004／build 19041；舊 OS 或不可用介面保守降級，仍需原生環境驗證。[Microsoft 事件要求](https://learn.microsoft.com/en-us/uwp/api/windows.ui.viewmanagement.uisettings.animationsenabledchanged?view=winrt-26100)

Linux 優先用 `ReadOne`，只有舊介面回 UnknownMethod 才改用 `Read`，明確解開後者兩層 variant。[XDG Settings 規格](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.Settings.html) GNOME fallback 是相容路徑，並非跨桌面標準；其 backend 來源見 [xdg-desktop-portal-gtk settings.c](https://browse.dgit.debian.org/xdg-desktop-portal-gtk.git/tree/src/settings.c?id=48dad15432dbeb92bfea6f5857697342ecef0b6e)。

Linux 服務只在既有 Tokio runtime 內擁有一個 task 和一條自有 session-bus connection。兩個精確 key 的訊號串流先建好，再讀初值；訊號只代表失效，收到後重讀當下值，避免採用排隊的舊 payload。連線／訂閱／讀取／關閉各有 deadline，queue 為 8、body 上限 4096 bytes，沒有 idle polling。服務失去連線後保留 unavailable；自動恢復及實際桌面重啟仍待驗。

正常連線入口、獨立 IME 視窗及明確的 motion 探針使用實際來源。其他合成探針固定減少動態效果，以免主機設定改變原有 expected。這個固定來源會明確標為 `controlled_synthetic_probe`，motion 驗收拒絕採用它。OS 變成減少動態效果時，通知沿用已讀時長規則且停止淡出，浮球停止進行中的慣性／gesture；不改位置、不送 BBS 輸入。

## 退出所有權

`Exit` 在其他 owner drain 前先設 closing、停止來源發布，之後移除訂閱並等待實際 worker／UI listener 結束，再釋放 root／renderer、清理媒體及結束 event loop。macOS／Windows 沒有自建 worker，所以 audit 的 `worker_joined` 是 null；Linux 在真實 JoinHandle await 完成後才回傳 true。

UI 的 shutdown deadline 使用一個 Slint timer／oneshot，獨立 IME 視窗沒有 Tokio runtime 也能正常退出。逾時／失敗保留未完成 handle，fallback abort 明確回報 join 未證明，不把清空欄位當成退出成功。motion 出錯仍執行媒體清理，合併回報錯誤。`motion-closed.json` 必須在正常退出後取得；僅有準備或驗證步驟的 pending receipt 不算 passed。

## 重現與已驗範圍

建置配置為 `native-player-probe`。與其他 batch 一樣，使用獨立、已盤點的 target，dev／test 明示停用 incremental 及 debug symbols；已清理後優先用保留候選，不為重跑以下探針重建。

```text
pttstella-slint --software-renderer --verify-motion <新證據目錄>
pttstella-slint --verify-motion <另一個新證據目錄>
```

預設只讀 OS 初值並驗證通知政策。macOS 另發出 process-local 的 workspace 通知以證明 production callback 仍連接；它不改系統值，也不能代替真正的偏好切換。兩案都有初值／結果、observer 移除與 UI listener join receipts，須連同 exit 0、嚴格 BBS Close ACK 核對。

`PTTSTELLA_SLINT_MOTION_EXPECT_CHANGE=1` 才等待實際 OS 值改變，驗通知後寫 `motion-change-observed.json`，再等原值還原並重驗。人工操作系統偏好前需取得原生操作工具要求的明確同意；本批尚未取得答覆，所以這項是 `not_run`。不可用暫時指定 UI bool 或 process-local notification 代替。

本批 `.local/verification/slint-system-motion/acceptance.json` 記錄9個已核對案例：封裝 CPU／GL motion、CPU／GL shell 通知、版本檢查、CPU 播放中 Quit、GL 播放中關窗、兩語言設定搜尋及文章收藏。均拒讀 `/opt/homebrew`，exit 0、各自非空結果／退出 receipt 及嚴格 Close ACK 相符。另以支援的 `--probe-ime` 獨立視窗原生關閉並得到 exit 0，證明無 Tokio runtime 的退出路徑，沒有宣稱繁中輸入或候選窗已驗。原生操作工具選取 App 時額外啟動的正常入口視窗也已關閉，沒有把該視窗計入合成案例。

Linux 四個 Unix socket fixtures 在 Mac 上執行同一個 production D-Bus consumer：標準優先／舊訊號重讀與無關大訊號過濾、legacy 雙 variant／GNOME fallback、錯誤型別不掩蓋並從後續訊號恢復、卡住 read 期間取消並確認實際 task join／peer EOF。加上 primitive 型別／raw 值與 unknown／錯誤容量測試，共新增7項；這是協定證據，不是 Linux 桌面事件驗收。

完整 Slint Rust 測試為389 library＋83 binary passed，另列4＋1 ignored。Dioxus 配置377＋121＋10 passed，另列4＋1＋1 ignored。兩配置 all-target Clippy、core gate 14命令／2 replay、165 Python 測試與四 target／兩配置依賴 gate 通過。獨立 `system-motion` library 的四 target Clippy 只證明服務配置可檢查，不代表完整 UI 連結、打包或原生事件已驗。

首次額外 driver 錯把 `--probe-ime` 與刻意禁止的 `--capture` 組合，程序在 parser 退出；改用支援的獨立視窗關閉驗證，沒有修改產品限制。封裝 verifier 首次錯將 ZIP 的 `__MACOSX` AppleDouble metadata 當成 App payload；修正 verifier 僅排除此 namespace 後，全部1230個 App payload 項目與 ZIP／唯讀 DMG 一致，51個 Mach-O 都是 arm64、load commands 無開發機路徑，ad-hoc 簽章有效但未公證。兩個 driver 問題與初始 D-Bus fixture 的重複 close 失敗有保留，未當成產品修正或放寬預期值。

最新候選在 `.local/verification/slint-system-motion/package-current/`：封裝 executable SHA256 `487905a9050da2c29e42fb5ac0ec3b3dec6fffbfb053120d58b3becc22d81857`，ZIP `2b81ee8ca04bf396d0bcc59d3381eb9ef3b3227c8c2c3fd7177b42228a9d9a99`，DMG `6de4ec8e6324b71a14a7d9e860961beec32b15b252c744245458c72d9ff0c730`。來源基準為 `3fbfb44` 加 `source-final.patch`／逐檔hash，保留原本的 precommit dirty provenance；提交後另核對同一產品 bytes。S8 對應來源交付仍為 not_ready。

實作提交為 `91d12e4`，`source-integration.json` 已核對13份 Cargo／產品檔案與建置前hash一致。中途清理已移除整個本批target；驗證後再移除被取代的 `slint-shell-chrome-final/package-current`、重複的 `slint-system-motion/pending-current` 原始CLI及23份byte相同的正向PNG。舊包1077項必要文字metadata／授權封存，完整舊payload只留hash清單；2876份受保護來源、資產、fixture、各配置最新包與必要來源hash不變。最後沒有target目錄，worktree du為1362756KiB；filesystem available 215716057088→216066596864 bytes是觀測，不全歸因於刪檔。exact paths、owner／resolved path／symlink／lsof與保留理由見本批`cleanup/ledger.json`，重複圖片的保留位置同記於該檔；不重建Rust證明清理。

實際 OS 改值／還原、Intel／Windows／Linux 原生、原生通知鍵鼠／可及性、完整前端資料回退和同負載效能門檻仍依 [完整計畫](slint-evaluation-plan.md)。已測快取、被取代包、待驗 CLI 及重複正向影像依 [清理規則](development-harness.md#產物清理與結案) 立即清理，不因整體 goal 持續而保留。

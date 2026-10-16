# 字型組合與可攜設定檔

2026-10-16。狀態：**具名檔案、跨平台選擇意圖與匯入／匯出已接入；驗收見 active plan**。使用者要求可在任何平台選擇自己喜歡的預設組合，也能保存自己的設定。

## 現況

既有 [DisplaySettings](../src/settings.rs) 提供英數／中文字型、字高與基線，三平台的內建組合已可在字型視窗手動選擇：

| 組合 | 英數字型 | 中文字型 |
| --- | --- | --- |
| Windows | Cascadia Mono | Microsoft JhengHei |
| macOS | Menlo | PingFang TC |
| Linux | DejaVu Sans Mono | Noto Sans Mono CJK TC |

三組皆為字高 20、基線 20。沒有既存有效設定時，依編譯平台選初始值；已保存的有效設定直接載入。平台名稱描述字型組合，**不限制使用平台**。

本機保存已有 version 1 JSON、8 KiB 讀取上限、欄位驗證、原始 bytes 備份 `settings.json.bak` 與原子替換。載入失敗使用平台初始值並顯示原因，不自動覆寫壞檔。保存／取消及重啟像素已在 Windows gate 驗證；新增的可攜檔透過明確選檔／另存入口使用。

設定目錄由 [Store](../src/settings/store.rs) 與鎖定的 directories 6.0.0 決定：Windows `%APPDATA%\PTTStella\config`、macOS `~/Library/Application Support/PTTStella`、Linux `$XDG_CONFIG_HOME/pttstella` 或 `~/.config/pttstella`。macOS／Linux 路徑為庫來源推導，沒有在這台 Windows 主機執行。

## 使用者操作

在既有顯示設定介面提供「依平台初始值／Windows／macOS／Linux／自訂」及「另存設定檔／載入設定檔」。使用者可將自己的組合另存成有名稱的 `*.pttstella.json`，保留多份檔案並自由搬移；第一版以明確開檔／另存入口使用，不建立雲端同步或字型下載系統。

Mac 上明確選擇 Windows 組合，重啟與再次匯入仍保留該選擇；不得因執行平台而換回 Menlo。編輯其中一個字型後成為自訂組合，其他欄位保留；按「恢復依平台初始值」才重新依 OS 解析。

匯入先填入同一個預覽草稿，展示雙色中文、英數、圓形符號與框線。可以保持匯入的組合、改選另一個內建組合，或手動調整。只有「套用並儲存」成功才改主畫面與本機檔；取消不改設定、備份或來源檔。提供「另存草稿」時應明確標示，避免把未套用的草稿與目前生效設定混淆；第一版預設匯出目前生效的設定。

設定檔保存字型名稱與選擇意圖，不包含字型檔。跨平台不保證有相同字型；畫面顯示要求的 family，並允許預覽及手動改選。既有 WebView 的 fonts.load／ready 只能等待載入，不能證明指定字型存在或每個字都使用它。沒有可靠能力時顯示「實際替代字型未確認」，不能假裝已檢查；不能靜默改寫使用者指定名稱。

## 格式、遷移與所有權

由同一個 Rust 設定 owner 管理儲存意圖及 renderer 所需的解析結果。建議字型選擇使用明確 enum：

- `PlatformDefault`：明確選擇依平台；解析為當前平台的內建字型。
- `Preset(Windows | MacOS | Linux)`：固定組合識別；任何平台都照該組合解析。
- `Custom { ascii_font, cjk_font }`：保留使用者名稱，不依 OS 改寫。

字高與基線獨立於字型選擇，切換組合可保留調整值；「恢復整組預設」另外明確還原數值。不要同時保存可互相矛盾的 preset 及另一份有效字型名稱，或由 JS 再猜應使用哪一份。

產品可攜檔以固定 `format`、`version`、顯示名稱與 display payload 識別；檔案內容是資料，不允許 CSS、HTML、腳本、外部字型 URL 或任意開檔指令。延用目前字型／數值驗證與有界解析；副檔名只供選檔提示。未知版本、未知欄位、重複 key、無效值與超限整份拒絕並顯示原因，不局部套用。

本機原有 version 1 的明確 family 遷移成 Custom，保留值；不能只因它恰好等於目前平台預設，就推論使用者要跟著 OS 改變。新設定結構須提供一次解析、向前版本辨識、備份與往返案例，移除 superseded 的平行儲存方式。

圖片 hover 的開關與位置也需要保存，見 [媒體方案](media.md#後續圖片-hover-預覽方案尚未實作)。它們屬 app preferences，由同一個設定 Store 管理；顯示 profile 首版只匯出字型與排版。若之後提供「匯出所有偏好」，須明確區別兩種檔案格式及範圍，不能偷偷帶出私人名單。帳密、session、原始流量、快取與暫時防閒置開關均不包含在顯示 profile。

## 實作順序與驗收

先做跨平台可選組合與自訂設定檔的儲存意圖，再接開檔／另存與既有預覽；媒體位置偏好沿用同一 app settings owner。可攜檔與本機設定不是兩個各自解釋字型的系統。

必要案例：

1. 選 Windows 組合後保存、重啟、匯出／匯入，組合識別與數值不變；在 macOS runner 同樣保留 Windows 選擇，不要求該字型已存在才允許保存。
2. 自訂任一字型、字高、基線後另存兩份具名檔，分別載入預覽、取消、套用；不互相覆寫或改來源檔。
3. 既有 v1 遷移、無設定初始值、損壞／超限／重複 key／未知版本／非法 family 及數值的正反例；備份或替換失敗保留原檔。
4. 缺字型時保留要求值並可改選，不能以 fonts.load 成功當作安裝證明；固定格線、雙色字、自然比例與 PNG 使用同一 renderer。
5. 真實原生開檔／另存取消與錯誤、返回 PTT 焦點、IME、選字及不送站端 bytes；Windows、macOS、Linux 分別記錄實測，不以單平台代替。

實作採 version 2 AppSettings、DisplayProfile 與 Fonts enum；renderer 只消費一次解析後的 DisplaySettings。舊 v1 讀入不立即寫檔；明確套用後才備份原始 bytes 並改存新格式。匯出只包含目前生效的 display 與使用者名稱，不含 app hover 偏好。核心案例驗證固定組合、自訂值、v1 遷移及重複／未知／無效欄位；原生選檔／另存及其他 OS 尚待人工驗收。

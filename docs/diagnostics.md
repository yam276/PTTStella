# 卡頓與輸入診斷版

診斷版供重現偶發卡頓、滑鼠／鍵盤無反應與操作後才恢復的問題。一般建置預設不開啟；本機以 `PTTSTELLA_DIAGNOSTIC_BUILD=1` 建置的 App，直接開啟就會記錄，視窗標題標示「診斷版」。不自動登入、不自動重送操作，也不改變原有操作准入規則。

macOS 紀錄目錄為 `~/Library/Application Support/PTTStella/diagnostics`；其他平台使用同一 ProjectDirs 本機資料目錄。可明確設定 `PTTSTELLA_DIAGNOSTICS_DIR` 指定診斷目錄；合成 probe 的診斷版預設寫在該 probe 輸出下，不混入正常使用紀錄。

遇到問題後，可按 macOS `Cmd+Shift+L`（其他平台 `Ctrl+Shift+L`）標記時間，再正常離開 App，把整個 diagnostics 目錄壓縮提供分析。標記只供診斷，不送至 PTT；如果當下無法操作，恢復後再按即可。每個 JSONL 行都有 session 的 Unix 毫秒時間、相對毫秒與程序 PID，可以比對大約發生時間。重現不需要另外開終端機。

## 記錄內容與容量

- Rust 記錄橋接的收／送種類、Paint／Painted 版本、送出成功與拒絕，以及每秒的 connection phase、接收 byte 數、input epoch、input／output／paint pending、command／file busy、設定／登入／貼上／組字與全文載入狀態。
- WebView 記錄按鍵、文字編輯、點擊、滾輪、焦點與組字事件有沒有到達、捕捉／處理後階段、固定分類的目標、准入狀態旗標及頁面捲動位置。只記事件種類，不記實際鍵名、字母或文字。
- WebView 每秒脈衝量測事件迴圈延遲，Rust 每秒狀態表示 UI loop 是否仍有進展。獨立寫入執行緒持續記錄 UI／WebView 最近回報距今多久，即使兩者不再回報，仍留下 watchdog 證據。睡眠、失焦與隱藏會影響時間，必須連同旗標解讀。
- 不記錄原始輸入 bytes、帳號／密碼／驗證碼、文章內容、站點網址、圖片網址、黑白名單、剪貼簿文字或錯誤訊息／stack 中可能夾帶的私人內容。不自動截圖，不上傳。
- 四個 `events-0.jsonl` 至 `events-3.jsonl`，每個最多 2 MiB，總計最多約 8 MiB；0 為最新，跨啟動保留並輪替。只輪替這四個自有名稱，不刪其他檔案。同一目錄使用 OS file lock，拒絕第二個 writer。
- UI 使用 256 筆有界非阻塞佇列，滿載只丟診斷事件並累計 dropped，不等待寫檔。WebView 每秒最多採樣 32 筆操作事件；耗時至少 16ms 的封包另外有每秒 8 筆額度，避免背景封包擠掉鍵盤與滑鼠證據。另保留脈衝／手動標記，不記高頻 pointermove。寫入失敗會提示，正常關閉保留 finish 與 cleanup 結果並 join writer。

## JSONL 旗標

WebView trace 的 flags 以 bit 表示：0 connected、1 input_pending、2 output_pending、3 navigationLocked、4 navigationRequest、5 composing、6 pastePending、7 dialog、8 selection、9 dragging、10 settings、11 document focus、12 hidden、13 keyboard sink focused、14 station mouse 非 local、15 tools open、16 web article、17 Vim enabled、18 keyboard pending、19 context menu。bits 20～24 為 navigation profile：0 unknown、1 main_menu、2 system_menu、3 document_list、4 article_list、5 article_reading、6 board_list、7 empty_list、8 command_help、9 continue_prompt。bits 25／26／27 分別是全文 loading／changed／limit。

phase 的低兩個 bit（phase & 3）中，0／1 表示捕捉階段尚未／已 preventDefault，2／3 表示一般處理後尚未／已 preventDefault；packet、reading_move 與 navigate 的 phase 0 是本機測點。key 事件另以 bit 2／3／4／5 分別記錄 Control／Meta／Alt／Shift，bit 6 表示組字或 Process／229，bit 7 表示 key 是單一 ASCII 控制字元；只記分類，不保存鍵名、字母、實體 code 或控制字元值。這能區分原生 Control、系統快捷鍵與事件形狀，不能重建鍵入內容。target 的低四個 bit（target & 15）中，0 為其他、1 為鍵盤接收區、2 為閱讀區、3 為對話框、4 為工具列。key 事件的 target bit 4～6 是按鍵形狀分類：0 無值、1 ASCII 字母、2 ASCII 控制字元、3 Process／Dead／Unidentified、4 其他具名鍵、5 單一非 ASCII 字元、6 其他單一 ASCII 字元；bit 7 表示實體 code 是字母鍵，仍不保存哪個字母。數字均為有界結構，不保存 DOM 文字或 ID。Web trace 的 pulse elapsed_ms 是距上一個脈衝的時間；packet elapsed_ms 是處理該封包的 wall time。

此版提供定位證據，尚未宣稱已找出偶發卡頓原因。合成 gate 不代替使用者日常實站重現。

導覽橋接的數字關聯：`event=bridge`／`direction=recv`／`kind=navigate` 的 presentation 欄保存 request ID，generation／revision 仍為來源版本；`event=bridge`／`direction=send`／`kind=navigation_result` 的 revision 欄保存相同 request ID、generation 為該請求的 session。這些只用於比對等待的建立與收尾，不包含按鍵或文章內容。

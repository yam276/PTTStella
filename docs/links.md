# 完整網址與 OSC 8

Rust Screen 是唯一控制碼解析者。OSC 8 的完整目的地附著於之後寫入的 byte-cell；換行、ANSI 樣式與 SGR 66 不會取消連結，後續 OSC 8 直接替換，空 URI 關閉。BEL 與 7-bit ST 均可終止，任意封包分片不改結果。格式依 [OSC 8 提案](https://gist.github.com/egmontkob/eb114294efbcd5adb1944c9f3cb5feda) 與 [站端跨行輸出](https://github.com/ptt/pttbbs/commit/5e82d56deacd815c53eb756619c087870c6b2730)；公開原始碼不代表實站部署證明。

連結屬性與 byte、樣式同屬一個 Cell，覆寫、插入／刪除字格、擦除、捲動／區域捲動及 session reset 同時更新；擦除出的空白不繼承連結。雙寬字的兩個 cell 必須屬於同一目的地才可點，未映射或混合屬性不猜接。畫面本身不加入 HTML 或自動外開。

## 上限與投影

- 單一 URI 最多 4096 bytes、參數最多 256 bytes；參數只作有界消費，不使用 id 合併不同目的地。metadata 採可列印 ASCII，非 ASCII URI 須先百分比編碼。
- 最多 128 個仍被 cell 引用的目標，目標字串合計 64 KiB；回收前檢查所有 cell，不讓新目標改寫舊 cell 的含義。格線最多 8000 cells，最多對應相同數目的屬性／範圍。
- OSC 暫存最多 4355 bytes；超限仍消費到終止，不將尾端畫成正文。無效或超限的新 OSC 8 不沿用前一個目標，也不對相同範圍改猜畫面文字。
- `links::validate` 是唯一 HTTP／HTTPS 與 host 檢查，拒絕控制字元、空白及超長正規化結果；metadata 與既有單列網址共用。UI 每張投影最多 512 個列範圍、重複目的地字串合計 256 KiB；同一目標的範圍一起接受或一起拒絕。

Frame 保留 Rust 專用 metadata，不序列化到 WebView；UI 只收到通過驗證且已過濾的連結範圍。沒有 metadata 時保留單列 HTTP／HTTPS 偵測；不從相鄰行拼接，也不讓 plain-text 偵測覆蓋同一 cell 的明確 metadata。未完成控制碼或同步更新期間沒有可點連結。

本機遮黑先投影 glyph 與 metadata，再衍生連結、複製及 PNG。接觸遮黑內容的範圍移除目的地並保留不可猜接標記；其他可見範圍才可保留其目的地。揭露目前畫面使用相同來源，畫面／規則／session 改變仍撤銷舊選取、hover 與匯出 lease。

## 使用與驗證

連結 hover 顯示完整實際目的地；點擊沿用 Rust 的媒體分類或明確外開。一般拖曳複製畫面文字；右鍵選單可直接複製完整目的地，或先選「選取完整網址」再用系統複製快捷鍵，任一跨行片段都對應同一網址。選取與點擊仍核對同一 presentation、generation、revision；過期索引不作用於新畫面。選取行 PNG 仍匯出畫面行，不把完整目的地另畫進圖片。

核心在 interaction／filters suite 使用自製串流與獨立 expected，覆蓋逐切點、逐 byte、BEL／ST、跨行雙寬字、替換／關閉、混合屬性、惡意 scheme／控制字元、覆寫／擦除／插入／刪除／捲動、容量與 slot 回收、遮黑及 reset。原生互動、平台與實站驗收狀態見 [active plan](active/minimal-browser.md)，核心通過不代表各平台點擊與剪貼簿已驗收。

2026-10-17 macOS 補驗：在已交付書籤 App 的獨立 probe 程序，以全螢幕／符合視窗取得可用原生座標事件。嚴格報告核對兩段依序各開啟／複製一次、跨行拖曳無額外外開、兩個 PNG 預覽真正解碼且第二次命中快取，HTTP 只下載一次。兩次 Cmd-C 後以 Cmd-V 貼回本機草稿，均觀察到同一完整合成 URL；未保存書籤或送站端 bytes。整頁 PNG 與先前相同場景基準逐像素相同，⌘Q Close acknowledged、所有 owner join、退出碼 0。

先前 `noWindowsAvailable` 與兩次校對試驗的失敗仍保留；本次只解除這個指定 OSC 8 原生案例的 not_ready。沒有宣稱一般視窗的所有座標、任意 DPI、其他 OS、影片／GIF、真正 OS 外開或實站全部驗收。重跑方式見 CONTRIBUTING，報告與基準保留 `.local/verification/osc8-pointer-recovery`。

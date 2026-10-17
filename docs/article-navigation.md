# AID 與指定文章

本契約對應 active plan 的 C3。已實作文章識別、網址驗證與 Session 導覽核心；使用者入口亦已實作；書籤與其他平台／實站驗收仍待接續，不把解析成功當成已開啟文章。

## 識別與輸入

`ArticleTarget` 由具驗證的 BoardName 與 Aid 組成，不持有按鍵字串。板名沿用 [快速切板](quick-boards.md)；大小寫保持輸入，由站端確認正式名稱。AID 保留大小寫，可在開頭加一個 #，正規化後固定八碼。

依公開 [AID 轉換](https://github.com/ptt/pttbbs/blob/68b88d8753d69000033037a0c84a4ee3596b937a/common/bbs/aids.c#L330) 與 [raw identity 欄位](https://github.com/ptt/pttbbs/blob/68b88d8753d69000033037a0c84a4ee3596b937a/include/pttstruct.h#L162)，使用 `0-9 A-Z a-z - _` 的六位元表示法，保留 M／G 類型、正數 time4_t 與 12-bit suffix；拒絕保留位元、零／超出正數範圍的時間及內部索引提示。這是本次明確支援的識別格式，不宣稱支援未來 AID 版本。

支援分開輸入板名與 AID、`#AID@Board`（# 可省略），以及 HTTP／HTTPS 的下列網址：

- `www.ptt.cc/bbs/Board/M.timestamp.A.HEX.html` 或 G 類型，HEX 為三碼十六進位；舊版沒有 suffix 的 `.A.html` 以零處理。
- `www.ptt.cc/bbs/Board/AID` 的短格式。

這些格式依公開 [文章網址產生](https://github.com/ptt/pttbbs/blob/68b88d8753d69000033037a0c84a4ee3596b937a/mbbsd/bbs.c#L455)；不是去任意頁面取尾段。輸入上限 256 bytes，板名及 AID 欄各自也有界；外圍非控制空白可清除，但不接受控制字元。網址只接受明確的 www.ptt.cc authority，拒絕帳密、顯式 port、其他主機、百分比編碼、反斜線、空白、dot path、query、fragment 與多餘路徑。不連網、跟隨轉址、取得文章或送出站端 bytes。

## 單一有界導覽的確認點

公開 [列表的 AID 搜尋](https://github.com/ptt/pttbbs/blob/68b88d8753d69000033037a0c84a4ee3596b937a/mbbsd/read.c#L502) 只選中文章，不能直接視為已閱讀。導覽已沿用 Session 唯一有界工作及 C2 切板流程：等目標板文章列表、送 #、核對空白搜尋提示、送 AID 與 CR 一次，再確認站端選取結果。不存在、刪除、無權限或未知格式均停止，保留手動操作，不自動確認錯誤提示。

站端的 [文章資訊查詢](https://github.com/ptt/pttbbs/blob/68b88d8753d69000033037a0c84a4ee3596b937a/mbbsd/bbs.c#L3584) 可回傳 AID 與板名；以已辨識列表來源、選取列與完整資訊框核對身分，不能只在文章內文找到相同文字就放行。只有已確認的資訊框可繼續，需等返回相同選取列後才開啟，最後確認閱讀畫面。只有資訊框身分匹配才送空白關閉；核對回到原選取列後才送方向右，最後需看到一般閱讀起頁、作者行與正確板名。文章不存在、刪除、權限提示、錯碼／錯板及非預期說明會停止；未知片段不延長期限。每步 5 秒、整體 30 秒，包含前面的切板步驟。取消、原始輸入、有效手動導覽、session 改變或關閉停止後續，不重試。M／G 身分使用同一核對流程，仍只准入已辨識列表與閱讀格式，不宣稱所有文摘版面已驗收。

## 驗證

純 Rust 使用獨立手算向量核對 timestamp、HEX、M／G 與舊檔名，不用自己的 encode／decode round trip 作唯一證據。另測大小寫、輸入上限、保留位元、時間範圍、格式錯誤、混淆主機／路徑與控制字元；中英錯誤走 Fluent。一般測試不連 PTT，不使用帳號或私人文章。

`navigation_job::Destination` 明列 Board／Article，對應一個 Session NavigationJob 與同一 write_input、input epoch、保活閘及清理；UI 只等啟動回覆，View.destination 的 Reached 才是完成。原有 C2 UI 消費相同 typed destination，沒有第二條發鍵管線或 timer。

Session 另發布帶 generation／serial 的最後一筆已確認 BoardVisit，供最近清單消費；這個事實可跨後續搜尋／錯誤畫面與 watch 合併保留，即使新操作仍在等待也不漏記。新連線先清除舊事實，UI 不把新 generation 套在舊紀錄上；清單清除不重置已消費序號。

自製 article-open fixture 與 production Connection 本機 peer 核對每步 bytes、同步資訊框零繼續與 Pong、五個等待階段的取消／原始輸入、過期輸入不取消現行工作、錯誤後晚到資訊框零操作、真實五秒逾時、重連零重播及關閉 join。macOS 的使用者入口與合成原生操作範圍見下節；實站及其他平台另行驗收。

## 快速前往入口

工具列「快速前往」、⌘K／Ctrl+K 開啟同一個本機草稿，可輸入板名、#AID@板名或支援的完整文章網址。Rust Destination::parse 選擇明確目的地，再交給既有 Connection；JS 不轉譯按鍵串、不依標題猜 AID。按「前往」或輸入欄的普通 Enter 才送出；IME／legacy 229、清單按鈕的 Enter、取消、舊 request 回覆及 session 改變維持同一准入與生命週期。最近項目只填入草稿。

Rust 在 trim 之前檢查 256-byte 上限與控制字元；HTML maxlength 由同一常數加一產生。即使超長貼上先被 WebView 截為 257 UTF-16 units，仍會超過 Rust 的 byte 上限而拒絕，不會將合法板名加大量空白／非法尾段裁成另一個可送出的目的地。錯誤保留草稿；正在執行時重開會顯示原板名或 #AID@板名，不能把 typed target 物件顯示成字串。

原生合成 probe 驗證網址與明確 AID 的兩次開啟、缺文停止與手動空白恢復、混淆主機／過長草稿拒絕、取消後晚到資訊框零續送，以及查詢等待中 ⌘Q。peer 核對所有 bytes，Close acknowledged、各 owner join 與正常退出必須同時成立；不是原生 IME 候選窗或實站登入驗收。

## 本機書籤資料

資料核心已實作，使用者入口與 Files 接線另批接續。目前不自動收藏閱讀頁，也不把先前導覽的 Reached 當成目前文章身分；只接受使用者明確指定的 ArticleTarget。資料只包含板名、八碼 AID、可選顯示名稱與 Unix 建立秒數，不保存內文或帳號。

`Bookmarks` 最多 500 筆，板名不分大小寫、AID 區分大小寫去重。新增放在最前；更新同一識別的名稱時保留原位置與建立時間。顯示名稱在 trim 前限制 128 個 Unicode 字元、512 bytes，拒絕控制字元；空名稱保存為 null。時間限正數至西元 9999 年末，不用當前時鐘拒絕跨裝置或時區資料。

獨立 `bookmarks.json` 位於產品本機資料目錄，document version 1、最大 512 KiB。載入需整份通過版本、容量、識別、標籤、時間與去重檢查，不略過錯誤項目或改寫原檔。保存先確認既有主檔有效，再將前一份原始 bytes 原子替換至 `.json.bak`，最後原子替換主檔；備份失敗不更動主檔，任一步失敗均不算保存成功。兩個檔案分別替換，並非跨檔案交易。壞檔與超量資料保留原樣；目前需使用者修復／還原後才能修改，不以空清單覆蓋。

書籤與最近看板共用 storage 的有界讀取及逐檔原子替換機制，各自維持資料驗證與恢復政策；最近清單的明確清除仍同時覆寫空備份及主檔。自製 [書籤 fixture](../tests/fixtures/bookmarks.json) 是獨立手寫 expected，核對固定身分、名稱、時間與 wire，不依賴外部文章。

# AID 與指定文章

本契約對應 active plan 的 C3。已實作文章識別、網址驗證、Session 導覽、快速前往與目前文章收藏／本機副本。macOS 已完成本機合成原生往返；其他平台／實站另行驗收，不把解析成功當成已開啟文章。

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

「文章書籤」已接 Files 與本機保存，不自動收藏閱讀頁，也不把先前導覽的 Reached 當成目前文章身分。手動新增仍只保存明確 ArticleTarget、名稱與建立時間；使用者按「收藏目前文章」才核對目前 AID 並保存完整文章副本，包含作者／推文 ID，但不包含登入、信件、密碼或完整 session。

`Bookmarks` 最多 500 筆，板名不分大小寫、AID 區分大小寫去重。新增放在最前；更新同一識別的名稱時保留原位置與建立時間。顯示名稱在 trim 前限制 128 個 Unicode 字元、512 bytes，拒絕控制字元；空名稱保存為 null。時間限正數至西元 9999 年末，不用當前時鐘拒絕跨裝置或時區資料。

獨立 `bookmarks.json` 位於產品本機資料目錄，document version 2、最大 32 MiB，仍可讀 version 1 並於下次明確保存升級。每筆可含一份 version 1 封存：4096 行／8 MiB、保存秒數、80欄連續原始 glyph tuples、雙色樣式、原頁尾與有界 OSC 8 spans。欄位、映射、row/col、style、來源頁尾、文字與容量均須驗證；row/glyph/entry 反序列化也限制數量。載入需整份通過版本、容量、識別、標籤、時間與去重檢查，不略過錯誤項目或改寫原檔。保存先確認既有主檔有效，再將前一份原始 bytes 原子替換至 `.json.bak`，最後原子替換主檔；備份失敗不更動主檔，任一步失敗均不算保存成功。兩個檔案分別替換，並非跨檔案交易。壞檔與超量資料保留原樣；目前需使用者修復／還原後才能修改，不以空清單覆蓋。

書籤與最近看板共用 storage 的有界讀取及逐檔原子替換機制，各自維持資料驗證與恢復政策；最近清單的明確清除仍同時覆寫空備份及主檔。自製 [書籤 fixture](../tests/fixtures/bookmarks.json) 是獨立手寫 expected，核對固定身分、名稱、時間與 wire，不依賴外部文章。

介面用清單選取已存項目，也可按「新增書籤」輸入 #AID@板名／文章網址及選填名稱；普通 Enter／保存才發出一筆修改，IME／229 不兼任保存。移除按鈕使用選取的已存識別，即使輸入欄已有其他草稿也不會誤刪。按「前往…」只交給既有快速前往 owner 填草稿；另按「前往」才啟動同一 Session 工作，不另建按鍵路徑。

ui/bookmarks 管理資料與保存後 commit；Files 的單一互動工作槽在 blocking pool 讀寫，忙碌明確拒絕、不建立第二個 queue。保存成功才替換已存清單，失敗保留生效資料與草稿。關閉清掉未送出草稿、撤銷讀取 request；已明確保存且開始的工作仍追蹤至完成，關窗時 join。晚到結果不重開視窗、不搶焦點或覆蓋下一份草稿。壞檔顯示警告並停用保存，Enter 也不能送出；可以手動修復／還原檔案後重啟。

DOM／event doubles 核對 request 身分、晚到讀取／保存、取消／重開、IME、readonly、不可信事件、選取移除、沿用快速前往及清理；Rust 另驗證不保存直到成功、錯誤保留、Files 撤銷未開始工作及 request 回傳。macOS 四個獨立程序驗證繁中新增／更名／移除、拒絕無 AID 草稿、書籤開啟文章、英文重啟／刪空、再重啟仍空及壞檔警告／Enter 零修改；正常關窗均需 Close acknowledged 與 owner join。中文使用原生貼上並核對欄位值，不冒充 IME 候選窗驗收；一般測試不連 PTT。


## 收藏目前文章與離線閱讀

只接受已 ACK、Web 模式且首尾皆完整的原文章，PTT／local mouse route／無待輸入與寫出／無其他命令／檔案工作／登入／設定。先 pin 同一 ArticleCache 的 Arc<Row> 與來源頁尾，原始列只在與名單投影不同時額外保存，計入既有32 MiB閱讀容量。一般無過濾差異頁只做一次有界比較，不額外配置全文。

沿用同一 Session 的 Refresh 流程（Q資訊、AID、重新開文、核對前三列並還原站端尾頁），不是純Q查詢，也不猜AID；使用者瀏覽位置和展圖由既有 held projection 保留。Acquisition 綁 request／operation Stamp，只在核對到同工作 Article Reached 後交 Files；取消、原始輸入、失焦、斷線、切篇、未知提示或期限停止，不存未核對／不完整內容。pin 在核對前完成，保存的是按鈕當下已完整讀取的版本，不冒稱永遠與站端最新一致。

Files 背景建立／校驗 archive、建立 draft，經共同鎖與三方 merge 把 metadata 與副本一次提交主檔；Arc 讓 UI 基線與草稿不深複製全文。不同收藏合併，同項衝突拒絕；容量不足不自動淘汰。保存開始前可取消核對，已開始的檔案工作保留至完成。移除收藏移除主檔副本；一代備份仍可能保留，非安全抹除。

開啟「閱讀本機副本」不連站，Files 依目前名單將原文切成完整24列 pmore projection（末段補空白）後套相同 Rust filter，再只傳過濾後本機 frame。此封包不帶 Connected、Stamp、送鍵、keepalive 或推文 timer。JS 使用同一 renderer／固定80×23 canvas，row index只畫可見23列，PageUp/Down重疊一列，Home/End及上下/滾輪均只移動本機viewport。圖片只保存網址，不發HTTP；雙色字、原始格線及文字不交由第二個ANSI parser。關閉釋放frame／row index，晚到讀取不能重開或蓋下一份選取。

原生新增 `bookmark_archive`（45列完整載入→核對一次AID→保存→讀本機）、`bookmark_archive_cancel`（Q延遲6秒、取消後零續送／零保存），以及 `bookmarks_archive_restart`（新程序只讀保存的副本、peer拒絕站端輸入）。各案均需Close acknowledged／owner join；讀取末行、返回、關閉與canvas未遮住按鈕另由真實Dioxus畫面確認。驗證狀態見active plan，不能以核心或DOM測試代替原生驗收。

無作者／標題／時間標頭的文章也可收藏。Refresh 的已知看板可為空，只從本工作尾頁 Q 的完整資訊框取得板名／AID；再核對返回同板列表、#定位、第二次Q身分及原選取列。僅此已核對重開路徑可用原文前三列比對取代一般 Open 的作者／板名標頭要求；一般快速前往維持原條件。缺原標頭不把圖畫／內文當成標題；收藏顯示可回退至 #AID@看板。身分或前三列不一致、取消、逾時與舊 operation 仍拒絕。

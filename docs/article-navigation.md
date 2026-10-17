# AID 與指定文章

本契約對應 active plan 的 C3。已實作文章識別與網址驗證；Session 導覽、使用者入口、書籤與原生驗收仍待接續，不把解析成功當成已開啟文章。

## 識別與輸入

`ArticleTarget` 由具驗證的 BoardName 與 Aid 組成，不持有按鍵字串。板名沿用 [快速切板](quick-boards.md)；大小寫保持輸入，由站端確認正式名稱。AID 保留大小寫，可在開頭加一個 #，正規化後固定八碼。

依公開 [AID 轉換](https://github.com/ptt/pttbbs/blob/68b88d8753d69000033037a0c84a4ee3596b937a/common/bbs/aids.c#L330) 與 [raw identity 欄位](https://github.com/ptt/pttbbs/blob/68b88d8753d69000033037a0c84a4ee3596b937a/include/pttstruct.h#L162)，使用 `0-9 A-Z a-z - _` 的六位元表示法，保留 M／G 類型、正數 time4_t 與 12-bit suffix；拒絕保留位元、零／超出正數範圍的時間及內部索引提示。這是本次明確支援的識別格式，不宣稱支援未來 AID 版本。

支援分開輸入板名與 AID、`#AID@Board`（# 可省略），以及 HTTP／HTTPS 的下列網址：

- `www.ptt.cc/bbs/Board/M.timestamp.A.HEX.html` 或 G 類型，HEX 為三碼十六進位；舊版沒有 suffix 的 `.A.html` 以零處理。
- `www.ptt.cc/bbs/Board/AID` 的短格式。

這些格式依公開 [文章網址產生](https://github.com/ptt/pttbbs/blob/68b88d8753d69000033037a0c84a4ee3596b937a/mbbsd/bbs.c#L455)；不是去任意頁面取尾段。輸入上限 256 bytes，板名及 AID 欄各自也有界；外圍非控制空白可清除，但不接受控制字元。網址只接受明確的 www.ptt.cc authority，拒絕帳密、顯式 port、其他主機、百分比編碼、反斜線、空白、dot path、query、fragment 與多餘路徑。不連網、跟隨轉址、取得文章或送出站端 bytes。

## 接續導覽的確認點

公開 [列表的 AID 搜尋](https://github.com/ptt/pttbbs/blob/68b88d8753d69000033037a0c84a4ee3596b937a/mbbsd/read.c#L502) 只選中文章，不能直接視為已閱讀。接續實作需沿用 Session 唯一有界工作及 C2 切板流程：等目標板文章列表、送 #、核對空白搜尋提示、送 AID 與 CR 一次，再確認站端選取結果。不存在、刪除、無權限或未知格式均停止，保留手動操作，不自動確認錯誤提示。

站端的 [文章資訊查詢](https://github.com/ptt/pttbbs/blob/68b88d8753d69000033037a0c84a4ee3596b937a/mbbsd/bbs.c#L3584) 可回傳 AID 與板名；後續以已辨識列表來源、選取列與完整資訊框核對身分，不能只在文章內文找到相同文字就放行。只有已確認的資訊框可繼續，需等返回相同選取列後才開啟，最後確認閱讀畫面。這些確認／取消／期限與原生測試尚未實作完成。

## 驗證

純 Rust 使用獨立手算向量核對 timestamp、HEX、M／G 與舊檔名，不用自己的 encode／decode round trip 作唯一證據。另測大小寫、輸入上限、保留位元、時間範圍、格式錯誤、混淆主機／路徑與控制字元；中英錯誤走 Fluent。一般測試不連 PTT，不使用帳號或私人文章。

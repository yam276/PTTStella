# 站點與編碼

使用者可輸入 BBS 位址、選擇編碼，並保存站點最愛及預設站。預設仍為 PTT；啟動只連線、不自動登入。這是既有 BBS 格線的擴充，不載入 shell／PTY。

入口為浮球「連線 → 站點」。選最愛只將位址、站名與編碼填入草稿；「連線切換」才關閉舊站並連至新站，不偷偷保存。「儲存最愛」及「設為預設」各自明確保存；設定預設不立即切站，移除最愛也不改變預設。切換或保存結果未回覆前停用表單，失敗保留草稿與原因。其他站的 PTT 登入按鈕及專用操作停用，Rust 仍在取用鑰匙圈或送出命令前再次拒絕。

支援 `wss://`、`ws://` 與 `telnet://`，省略協定的主機名稱使用 Telnet；Telnet 預設 port 23。一般網頁網址不是 BBS 串流，只有三個官方終端入口可以明確轉換。位址限 512 bytes，不保存內嵌帳密、query 或 fragment；無效位址留在草稿中顯示原因。WSS 使用系統信任根；ws／Telnet 不加密，介面應在連線前明示。先不支援 SSH、UTF-8 或 GB18030 的四位元組字元。

Big5-UAO 保持既有映射；標準 Big5 與 GBK 使用 encoding_rs 的 WHATWG 映射。編碼屬於站點／session，接收 byte 先作用於同一 ANSI 格線，再解成完整中文字與左右樣式。輸入及貼上使用該 session 的同一編碼；無法映射或超量時整段拒絕，不刪字或插入問號。切換編碼需要重新連線，不用新編碼重解舊格線。

同一 connection owner 先完成舊傳輸的有界關閉／dial 取消，再重設格線、framing、pending input、generation 及新站點。舊按鍵、貼上、畫面、mouse lease、登入及導覽不能套到新 session。Telnet 關閉只代表本機 transport 已 shutdown，不能報成 WebSocket 的站端 Close acknowledged。

PTT 專用的按鈕登入／鑰匙圈、自動導覽、最近看板、內容名單與保活只在精確 PTT WSS＋Big5-UAO 的 session 使用。PTT2、巴哈及其他自訂站先使用原始輸入；選字、複製、PNG、一般網址及媒體沿用共同畫面。其他站即使畫面類似 PTT，也不能觸發 PTT 自動命令或套用 PTT 名單。

站點設定獨立保存於使用者設定目錄的 sites.json（version 1），最多 32 個最愛、64 KiB；名稱最多 128 bytes。目標以正規化位址與編碼共同識別，同一地址可有不同編碼。預設站為獨立目標，移除最愛不偷偷改變預設。保存由既有檔案 owner 在 blocking pool 完成，成功後才更改有效資料；保留原始備份、壞檔不覆寫、失敗不假裝保存。顯示 profile 不包含站點、帳號或密碼。

初始選項：PTT、PTT2、Bahamut BBS（UAO）及 SMTH（GBK），不在載入設定時探測或連線全部站點。[PTT2 站方公告](https://www.ptt.cc/bbs/AppsForBBS/M.1622539491.A.7F1.html) 列出 WSS 與終端入口；[巴哈官方終端](https://term.gamer.com.tw/) 的部署資源指定 term.gamer.com.tw/bbs；[水木站內入口](https://www.newsmth.net/nForum/) 列出 telnet://bbs.newsmth.net。公開入口及歷史公告不等於本機已完成站端登入，當前可用性另以明確實站檢查記錄。

驗證使用自製 Big5／GBK 位元組及獨立預期，包含每個分片、左右換色、無效字元、貼上及 session reset。本機 WS→Telnet 案例須核對 Close 順序、舊輸入零重播、精確 GBK bytes、PTT 命令拒絕及 TCP EOF；儲存需核對重新載入、原始備份、損壞與失敗。原生管理介面與各站登入結果依 active plan 分開記錄。

# 站點與編碼

使用者可輸入 BBS 位址、選擇編碼，並保存站點最愛及預設站。預設仍為 PTT；啟動只連線、不自動登入。這是既有 BBS 格線的擴充，不載入 shell／PTY。

從浮球「連線 → 站點」選擇「PTT（WSS）」或「PTT（SSH）」，再按「連線切換」。兩個按鈕先填入站點與 Big5-UAO，可用「設為預設」保存下次啟動的選擇。切換或保存回覆前停用表單，失敗保留草稿與原因。移除最愛與儲存預設是獨立操作。

一般連線接受 `wss://` 與 PTT 官方 `ssh://bbs@ptt.cc`。WSS 使用系統信任根驗證憑證與主機名稱。SSH 使用固定的 `bbs` 公開服務帳號與 port 22，協商 RSA-SHA2，核對站方公布的 RSA SHA-256 指紋後才開啟 80×24 的 BBS 工作階段。SSH 的 Big5-UAO 資料直接進格線。PTT 帳密仍在 BBS 帳號提示後輸入。金鑰不符時終止連線。入口與指紋見 [PTT 官方首頁](https://www.ptt.cc/index.html?lang=zh) 與 [站方金鑰公告](https://www.ptt.cc/bbs/SYSOP/M.1728132668.A.BB7.html)。

Telnet、明文 WebSocket 與省略協定的舊端點可從原設定讀取及移除。一般連線會拒絕這些端點，也不能新增為最愛或預設站。舊預設使用停用的傳輸時，本次啟動連至 PTT WSS 並提示重新選擇，保留原設定供管理。明文傳輸限顯式開啟的本機合成測試。位址上限 512 bytes，禁止密碼、query 與 fragment。三個官方網頁終端入口可轉成 WSS，SSH 端點限上述 PTT 入口。

Big5-UAO 保持既有映射；標準 Big5 與 GBK 使用 encoding_rs 的 WHATWG 映射。編碼屬於站點／session，接收 byte 先作用於同一 ANSI 格線，再解成完整中文字與左右樣式。輸入及貼上使用該 session 的同一編碼；無法映射或超量時整段拒絕，不刪字或插入問號。切換編碼需要重新連線，不用新編碼重解舊格線。

同一 connection owner 先完成舊傳輸的有界關閉／dial 取消，再重設格線、framing、pending input、generation 及新站點。舊按鍵、貼上、畫面、mouse lease、登入及導覽不能套到新 session。SSH 與合成 Telnet 關閉回報本機傳輸結束。WebSocket 另等待站端回覆 Close。

PTT 專用的按鈕登入／鑰匙圈、自動導覽、最近看板、內容名單與保活只在精確 PTT WSS／SSH＋Big5-UAO 的 session 使用。PTT2、巴哈及其他自訂站先使用原始輸入；選字、複製、PNG、一般網址及媒體沿用共同畫面。其他站即使畫面類似 PTT，也不能觸發 PTT 自動命令或套用 PTT 名單。

站點設定獨立保存於使用者設定目錄的 sites.json（version 1），最多 32 個最愛、64 KiB；名稱最多 128 bytes。目標以正規化位址與編碼共同識別，同一地址可有不同編碼。預設站為獨立目標，移除最愛不偷偷改變預設。保存由既有檔案 owner 在 blocking pool 完成，成功後才更改有效資料；保留原始備份、壞檔不覆寫、失敗不假裝保存。顯示 profile 不包含站點、帳號或密碼。

初始最愛包含 PTT WSS、PTT SSH、PTT2 和 Bahamut BBS。PTT2 與巴哈的 WSS 入口見 [PTT2 站方公告](https://www.ptt.cc/bbs/AppsForBBS/M.1622539491.A.7F1.html) 與 [巴哈官方終端](https://term.gamer.com.tw/)。

驗證使用自製 Big5／GBK 位元組及獨立預期，包含每個分片、左右換色、無效字元、貼上及 session reset。本機 WS→Telnet 案例須核對 Close 順序、舊輸入零重播、精確 GBK bytes、PTT 命令拒絕及 TCP EOF；儲存需核對重新載入、原始備份、損壞與失敗。原生管理介面與各站登入結果依 active plan 分開記錄。

SSH 離線測試使用本機合成主機金鑰，核對金鑰不符時零驗證、80×24 終端請求、原始雙向資料、正常關閉與中途取消。實站探針只到登入提示並關閉。

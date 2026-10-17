# 站端滑鼠

對應 active plan 的 D，與既有 [本機滑鼠導覽](navigation.md) 分開。第一批只完成 Screen 模式追蹤與純 SGR 按鈕編碼；尚未接 Session 送出、設定或 UI pointer，不宣稱滑鼠已可用。

## 協定與範圍

公開站端 [term.c](https://github.com/ptt/pttbbs/blob/68b88d8753d69000033037a0c84a4ee3596b937a/mbbsd/term.c#L4) 使用 DECSET／DECRST 1000、1002、1003 與 1006，受站端功能及帳號滑鼠設定影響；啟動也可能要求 1003，不能假定只有點擊。公開 [接收器](https://github.com/ptt/pttbbs/blob/68b88d8753d69000033037a0c84a4ee3596b937a/common/sys/vtkbd.c#L303) 接受 SGR 的按下／釋放與零基格線座標。這是來源觀察，未宣稱實站部署或帳號設定已驗收。

依 [XTerm Mouse Tracking](https://invisible-island.net/xterm/ctlseqs/ctlseqs.html#h2-Mouse-Tracking)，追蹤模式與座標編碼分成兩組互斥狀態；[模式設定／重設](https://github.com/ThomasDickey/xterm-snapshots/blob/5c75ead9a75d4b2e666ca06fa94503b5766ed93f/charproc.c#L7071) 明列 tracking 重設關閉回報，encoding 重設只在目前模式相符時生效。產品只實作這個明確協定子集，不嵌入該實作。

Screen 由相同有界 CSI parser 管理 Modes，只有完整 private h／l 才更新。追蹤記錄 off、9、1000、1001、1002、1003；編碼記錄 legacy、1005、1006、1015、1016。追蹤任一 DECRST 關閉 tracking；編碼採最後啟用，重設其他編碼不改變目前值。知道控制碼格式不代表支援其互動：第一版只有 **1000 click + 1006 SGR** 可以編碼三個按鈕的 press／release；9／1001／1002／1003、其他編碼、motion／wheel 不輸出。

每次有效模式變化增加 epoch，同一批 off／on 即使最後狀態相同也可撤銷舊 pointer。重複設定同一模式不增加；RIS 清模式並保留遞增 epoch，新 session 則從 default／epoch 0 開始，由 connection generation 區隔。模式獨立於 retained Frame，所以 DEC2026 期間的變更可以立即觀察；但格式支援本身不是送出許可，後續 Session 還必須拒絕同步／未完成控制碼、舊世代與未擁有的釋放。

Report 使用零基 row／col、具名 button／action 與 Shift／Alt／Ctrl；Alt 對應協定的 Meta bit，macOS Command 不在此格式。唯一 encoder 將座標加一、修飾碼加入按鈕，產生 `CSI < button ; col ; row M/m`；不加舊 X10 的 32，也不將原始 JS 字串當封包。座標與面積遵從 Screen 同一上限，非法格線／溢位／越界整筆回傳錯誤，沒有部分 bytes。

## 接續准入與原生驗收

Session 必須持有一筆 pointer 交易、generation／mode epoch、使用者開關與容量／速率限制，並沿用單一 write_input。模式改變、重連、失焦、lost capture 或人工輸入介入須撤銷後續；不能因按下已送就假稱回滾。按下觸發新畫面時的釋放需有同一交易的規則，不能盲用舊 frame 或把新 session 套在舊事件。

UI 一次操作只走站端回報或本機導覽之一。未協商／使用者停用可沿用本機；未支援的站端模式需明示，不能把 1003 假稱成已支援 1000。Shift 保留選字覆寫，原生複製、Ctrl＋滾輪縮放與 dialog 仍可用；座標從 renderer 同一格線換算，排除黑邊並驗證縮放／DPR。這些為下一批契約，尚未當成實作或通過。

自製 [模式 trace](../tests/fixtures/mouse-modes.json) 是獨立手寫狀態 expected，逐 byte／所有兩段切分核對；另測無私有標記、錯格式、取消、字串與超長 CSI 不改模式，C0 中斷、重設、同步批次、非支援格式拒絕、格線邊界及獨立按鈕 bytes。一般測試只用合成資料，不連 PTT。接線後還需 production Connection 往返、真正 trusted pointer／選字／縮放與清理，才能宣稱站端滑鼠可操作。

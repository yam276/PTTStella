# 站端滑鼠

對應 active plan 的 D，與既有 [本機滑鼠導覽](navigation.md) 分開。Screen 模式追蹤、SGR 編碼、Session 交易准入與 UI pointer 已接線；1000＋1006 的限定 macOS 原生點擊案例通過。新增 1002／1003 移動與垂直 wheel 接線的核心／本機傳輸／JS 案例通過，原生視窗接續；Shift 拖曳／Ctrl＋滾輪、實站及其他平台仍待驗收，不宣稱所有滑鼠模式已完成。

## 協定與範圍

公開站端 [term.c](https://github.com/ptt/pttbbs/blob/68b88d8753d69000033037a0c84a4ee3596b937a/mbbsd/term.c#L4) 使用 DECSET／DECRST 1000、1002、1003 與 1006，受站端功能及帳號滑鼠設定影響；啟動也可能要求 1003，不能假定只有點擊。公開 [接收器](https://github.com/ptt/pttbbs/blob/68b88d8753d69000033037a0c84a4ee3596b937a/common/sys/vtkbd.c#L303) 接受 SGR 的按下／釋放與零基格線座標。這是來源觀察，未宣稱實站部署或帳號設定已驗收。

依 [XTerm Mouse Tracking](https://invisible-island.net/xterm/ctlseqs/ctlseqs.html#h2-Mouse-Tracking)，追蹤模式與座標編碼分成兩組互斥狀態；[模式設定／重設](https://github.com/ThomasDickey/xterm-snapshots/blob/5c75ead9a75d4b2e666ca06fa94503b5766ed93f/charproc.c#L7071) 明列 tracking 重設關閉回報，encoding 重設只在目前模式相符時生效。產品只實作這個明確協定子集，不嵌入該實作。

Screen 由相同有界 CSI parser 管理 Modes，只有完整 private h／l 才更新。追蹤記錄 off、9、1000、1001、1002、1003；編碼記錄 legacy、1005、1006、1015、1016。追蹤任一 DECRST 關閉 tracking；編碼採最後啟用，重設其他編碼不改變目前值。知道控制碼格式不代表支援其互動：目前支援 **1000／1002／1003 + 1006 SGR** 的三個按鈕 press／release，以及垂直 wheel；1002 加上持有按鈕的 motion，1003 加上無按鈕 motion。9／1001 與其他編碼仍拒絕，沒有新增水平 wheel／像素座標支援。

每次有效模式變化增加 epoch，同一批 off／on 即使最後狀態相同也可撤銷舊 pointer。重複設定同一模式不增加；RIS 清模式並保留遞增 epoch，新 session 則從 default／epoch 0 開始，由 connection generation 區隔。模式獨立於 retained Frame，所以 DEC2026 期間的變更可以立即觀察；但格式支援本身不是送出許可，後續 Session 還必須拒絕同步／未完成控制碼、舊世代與未擁有的釋放。

Report 使用零基 row／col、具名 button／action 與 Shift／Alt／Ctrl；Alt 對應協定的 Meta bit，macOS Command 不在此格式。唯一 encoder 將座標加一、修飾碼加入按鈕，產生 `CSI < button ; col ; row M/m`；不加舊 X10 的 32，也不將原始 JS 字串當封包。座標與面積遵從 Screen 同一上限，非法格線／溢位／越界整筆回傳錯誤，沒有部分 bytes。

D2 的純格式層另新增 MotionReport／WheelReport，與原按鈕共用同一 SGR 座標／修飾碼／容量驗證。1002 只接受有按鈕的 motion，1003 可接受無按鈕 motion；wheel 接受 1000／1002／1003，只有上／下方向，不存在 wheel release。其他 tracking／encoding 仍拒絕。Session／UI 接線沿用相同有界命令佇列及送出 owner。每筆 sample 有獨立遞增 sequence、原畫面／模式／policy lease；持有按鈕時核對原交易及按鈕，無按鈕則要求完整當前 stamp 與新的 transaction。Rust 對同格 motion 去重，motion／wheel 合計每秒最多 30 筆，不消耗按鈕釋放額度；切換偏好不重置速率，也不延長持有期限。被合併的 sequence 仍被消費，不能事後重送。

## Session 交易與容量

Session 持有一筆最長 30 秒的按鈕交易，沿用單一 write_input。按下需要當前 generation／revision／input epoch、mode epoch、policy epoch 與遞增交易編號；釋放核對原交易及按鈕，允許同一交易按下後的畫面／input epoch 更新。座標仍以當前格線驗證。重複、舊世代、舊模式、未擁有的釋放、同步／未完成控制碼、登入或導覽工作期間拒絕，不補送。

使用者偏好預設開啟且跨重連保留；模式、單筆交易及編號依 generation 隔離。View 的 Snapshot 由 Rust 推導 Local／Station／Blocked：未協商或使用者停用走 Local，支援的 1000＋1006 且准入有效才走 Station，其餘已協商模式為 Blocked。這是後續 UI 的分流來源，不由 JavaScript 解析控制碼。

同一 64 筆輸入佇列在剩餘容量不超過 8 筆時拒絕新按下，保留釋放空間；每秒最多 20 次按下，配對釋放不另吃速率額度。人工輸入仍可能用滿佇列，因此取消／停用另由有界 watch snapshot 傳遞，保留最新偏好、全域撤銷及單筆取消三項；worker 每次取命令前先套用，未開始的已取消按下也不能稍後執行。控制 serial 與 policy nonce 分開，人工空操作撤銷舊 lease 後不會吞掉下一次開關。這沒有新增送出 worker 或無界佇列。

人工按鍵、成功接受的導覽／登入會撤銷 pointer lease；模式改變、重連、斷線或關窗清除持有交易，不合成 release 或重送。按下可等待畫面回應，逾時安靜釋放等待；釋放不新增或延長等待，因公開站端 [io.c](https://github.com/ptt/pttbbs/blob/68b88d8753d69000033037a0c84a4ee3596b937a/mbbsd/io.c#L312) 會丟棄 KEY_MOUSE_RELEASE，不能要求它必須重繪。持有交易暫停防閒置，沿用既有 connection timer 到期清除。

## UI 分流與原生驗收

導覽命令已用封閉 Source 區分 Pointer／Button／Keyboard。Surface 的 Navigate 必須明確提供 pointer 或 button，不能透過這個封包冒充 Vim；Vim 仍由 Rust key policy 發出 Keyboard。Session 在任何導覽副作用前，拒絕非 Local route 的 Pointer，涵蓋尚未支援的站端模式；明確工具列與鍵盤命令保留，只有 Keyboard 可沿用無重繪的移動許可。使用者停用站端滑鼠後 Pointer 可回到本機導覽。

Paint 傳遞同一 Rust Snapshot，獨立 pointer owner 只消費 route，不重解協定。工具列開關只在 trusted click 改變本次啟動的偏好，顯示目前回報／本機／不可用狀態。新按下必須 trusted、primary、在 renderer 同一格線內，使用單調交易編號及 pointer capture；畫面已繪製且 mode／policy 相符才由 Surface 接受按下，Session 再核對當前狀態。釋放保留原 lease 並取當前座標，可以跨正常重繪；同步／不完整畫面、模式／policy／世代改變、失焦、lost capture、人工編輯或 modal 會取消，不重試、不合成釋放。舊 capture-loss 若同一 pointer 已重新 capture，不撤銷新交易。

站端接管時不執行本機點編號、網址開啟、hover 圖片或普通滾輪送鍵；Rust 同時拒絕非 Local 的 pointer 導覽與連結開啟。Shift **在按下時** 選擇本機選字路徑，之後放開 Shift 仍能結束同一選取；Shift 右鍵可選取完整網址以複製，但不外開。Ctrl＋滾輪維持本機縮放；放大時仍可捲動本機 viewport，沒有偽裝成站端 wheel／motion。未協商／使用者停用沿用本機，1001 等未支援模式顯示不可用與停用入口。正常按下引起的完整重繪不只因 revision／input epoch 改變就取消；同步或不完整輸出則由 Rust 的 Blocked 撤銷。

UI 不等待 press 的命令回覆才處理 release；只透過同一有界命令佇列送結構化事件。入列失敗會發有界單筆取消並回覆精確 lease，晚到拒絕不能取消新交易。Rust 共同清理先撤銷 mouse controls，再並行停止 renderer／connection；JS destroy 清 capture、移除既有共同 bindings，沒有新增 timer 或背景送出工作。座標／可信事件／選字／縮放的真正原生驗收仍待獨立程序完成，不以這些單元測試當作 macOS／Windows 或所有 DPR 已通過。

移動與 wheel 使用一筆 owned animation-frame callback，每 34ms 最多送一個最新 sample，wheel 優先；按鈕釋放前可沖出原交易最後一筆 sample。UI 只保留一筆 motion、一筆 wheel 與不足 40px 的垂直餘量；line 按 40px、page 按畫面行數換算，一次 callback 最多一個垂直 tick，超量／慣性沒有延後回放的佇列。Shift 選字、Ctrl 縮放、已選文字、modal、viewport 已放大可本機捲動時不產生 station wheel。模式／policy／來源改變、黑邊、取消及 destroy 會清 callback 與舊 sample；不新增 timer 或送出 worker。

可信的同一 primary 指標移動另核對 [Pointer Events 的 buttons 狀態](https://www.w3.org/TR/2026/REC-pointerevents3-20260630/#the-buttons-property)：持有按鈕必須仍有對應位元（左 1／中 4／右 2）。位元消失或狀態缺失即取消原交易、清 pending，不合成 release；此檢查先於黑邊及修飾鍵忽略，漏掉 up 後也能結束持有。外來 pointer ID 不影響原交易或其已排定移動；未持有時只接受 buttons 為 0 的無按鈕 motion，按在格線外再移入不冒充 hover。這是 UI 對原始 pointer 事件的驗證，Rust 仍核對 transaction／button／模式與版本。

所有 bytes 仍共用同一序列化 send／Telnet escape 路徑。被動 motion 不增加命令 input epoch，也不建立／延長畫面回應等待，避免游標剛移過就把後續有效點擊判為過期；畫面／模式／policy 的失效檢查仍保留。點擊與 wheel 照常增加命令 epoch，wheel 只在尚無等待時建立有界等待，不建立 held／release 交易。新 sample 在共同佇列剩餘容量不超過 8 時拒絕，保留 explicit input／release／Close 的餘量；取消控制仍由既有 watch 優先套用。

自製 [模式 trace](../tests/fixtures/mouse-modes.json) 是獨立手寫狀態 expected，逐 byte／所有兩段切分核對；另測無私有標記、錯格式、取消、字串與超長 CSI 不改模式，C0 中斷、重設、同步批次、非支援格式拒絕、格線邊界及獨立按鈕 bytes。八個交易 owner 與一個 control snapshot 案例核對租約、速率、逾時及撤銷；八個 production Connection loopback 案例獨立核對精確 SGR bytes、更新後釋放、重複零送出、佇列滿時控制、同步模式變化、重連隔離、保活暫停及導覽來源。Surface 測試核對顯示 ACK／模式／policy、可信准入、封閉型別及獨立 wire expected；八個 JS pointer／handler 案例核對原 lease、Shift 選字、滾輪分流、取消／capture／重放及清理。一般測試只用合成資料，不連 PTT。這些單元結果不能代替下述限定原生案例或尚未驗收的邊界。

macOS `mouse` probe 已以真正視窗完成 col 4／row 4 的左／右鍵精確按下釋放、未支援 1003 的點擊／滾輪零輸出、Shift 雙擊選行及 OS 剪貼簿讀回、停用後一次本機文章編號 CR、重新啟用及中鍵按住時的模式撤銷；peer 沒有收到中鍵釋放或額外本機 bytes。最後 mode epoch 6、無 held lease，正常 Close acknowledged 並 join renderer、connection、credential 及 file owners。前後 PNG 獨立解碼 960×576、全部像素相同；鍵盤縮放 100→125→100% 通過。

此案例不是 Shift 拖曳或 Ctrl＋滾輪的原生證據：工具拒絕單獨保持修飾鍵，並行按鍵／滾輪沒有形成縮放，因此後者保留 not_ready，只能引用 handler 單元結果。原生失焦／lost capture／重連、跨 DPR、實站與其他 OS 也沒有在本案例冒算通過。第一次原生 probe 誤將既有本機點編號的 CR 契約寫成方向右，cleanup false／缺 surface 報告，保留為失敗；修正測試服務後從頭重跑，沒有改產品送鍵去遷就 probe。證據及候選 App 身分在 active plan。

D2 另以兩個 macOS 原生程序通過指定增量：`mouse_motion` 的 anchor 左鍵點擊／釋放後開啟 1003，原生水平滾動將無按鈕指標移至 col 10／row 4，只收到 `ESC[<35;11;5M`，水平輪沒有送出站端輸入；`mouse_wheel` 在 col 4／row 4 收到向下、向上各一筆，沒有按鈕邊緣或本機導覽。兩者完成後 mode 為 1001、無 held，PNG 與前版相同合成場景全像素相同，Close acknowledged 與全部 owner join。這是兩個限定原生案例，不包括 held drag 或兩項實體修飾手勢。

`mouse_drag` 第一次在 anchor 後收到額外 raw Esc 而失敗，未確認該 Esc 的來源；第二次座標工具在全螢幕／一般視窗都回 noWindowsAvailable，只有 AX／截圖可讀，沒有完成 anchor。兩者 cleanup false／缺 surface report，退出 0 沒有算通過。保留嚴格 expected 與所有失敗紀錄，原生 held drag 繼續待驗。探針另以 raw synthetic 單元驗證三種正例與「到上限仍缺釋放必須失敗」，不將這些單元混稱 trusted GUI 驗收。

# PTTStella 最簡 terminal 計畫

狀態：**畫面核心、Windows 視窗與 WSS 登入前連線已驗證；macOS 專用帳號已觀察手動登入與部分實站瀏覽，平台與案例範圍分列如下**。第一個驗收平台為 Windows；Rust／Dioxus 維持可攜，macOS／Linux 後續各自驗證。

## 目標

目前增量：A／B 的明確子集與 C1／C2／C3 已交付並依下文區分平台與實站邊界；C1 指定 macOS 原生互動已補驗。D 點擊路徑及限定 macOS 原生案例通過；D2 移動／wheel 已接線，無按鈕移動與垂直 wheel 在最新測試 package 再次通過。使用者已確認實體 Shift 拖曳／複製及 Control＋滾輪縮放的可見結果，同次 peer 的零多送仍未證明；held drag 與其餘原生邊界接續。交付 App 仍為 `dist/macos-mouse-final/PTTStella.app`，尚未以 D2 測試 package 取代；保留前版 `dist/macos-bookmarks/PTTStella.app`。D2 最新測試 package 已重跑 46 筆 Vim 與 C1 完整網址；其餘平台／手勢邊界分開記錄，不混稱完整 desktop gate 通過。

開啟視窗、自動連上 PTT、手動輸入帳號密碼、看見登入後畫面，最後可正常斷線。文字、固定排版與雙色字由核心與 renderer 實作。

使用者已明確補充：只有外觀像 terminal，並非真實系統終端。本產品不開 shell／PTY，不追求通用 terminal 相容。

初期 terminal surface 已擴充本機操作功能、依 [導覽契約](../navigation.md) 的最小滑鼠導覽，以及目前畫面連續行 PNG 截圖。本階段依 [媒體 MVP 規格](../media.md) 加入按需媒體預覽。沒有獨立文章 UI、啟動時自動登入或背景追蹤；後續依使用者明確授權加入 [按鈕登入與鑰匙圈保存](../login.md)，以及下列有界快速切板計畫。

## 下一階段：站端相容性與桌面操作

2026-10-17 研究與排序；下表記錄研究時的缺口，執行結果見本節進度。使用者要求將近期站方功能一併納入桌面功能規劃。依公開公告與官方原始碼確認協定，再用自己的核心、互動意圖與合成案例實作；原始碼合併日期不代表 PTT1／PTT2 已部署。下列風險來自程式檢查，尚未證明是過去實站 Vim 暫停的原因。

| 近期變更與證據 | 目前狀態 | 可觀察的改善／驗收 |
| --- | --- | --- |
| 9/20 [ECMA-48 公告](https://www.ptt.cc/bbs/PttCurrent/M.1789835163.A.633.html)、[DEC2026 公告](https://www.ptt.cc/bbs/PttCurrent/M.1789835193.A.E46.html)，後者明列 PTT1 9/20、PTT2 9/13 上線 | Screen 能吞掉一般未知 private CSI，但未實作同步更新邊界；CSI 超長時會離開解析狀態，仍需檢查尾端文字洩漏 | 跨任意封包邊界都完整消費 CSI；同步批次內繼續處理 bytes，完成後才發布完整畫面／分析導覽；缺結束碼須有界恢復，斷線與新 session 清除同步狀態 |
| 9/23 原始碼加入 [SGR 66 雙色字](https://github.com/ptt/pttbbs/commit/5bec38cc8a18f08a660c01611e31bda11078c212)；UTF-8 或特定 DBCS 輸出選項使用新格式，一般 Big5 仍可走舊格式 | Screen 只支援原有分開兩個 byte 上色，SGR 66 尚未實作 | 用等價新舊串流核對完整字形、左右前景／背景／明暗、游標與後續樣式；實際 renderer 與 PNG 對照；不把「可吞控制碼」當成雙色正確 |
| 10/9 原始碼加入 [OSC 8 跨行完整網址](https://github.com/ptt/pttbbs/commit/5e82d56deacd815c53eb756619c087870c6b2730) | Screen 忽略 OSC 內容；links 只辨識同一列完整 HTTP／HTTPS，不接回折行 | 可點／複製跨行長網址，並沿用媒體分類；以站端完整 URL metadata 綁 cell，不從相鄰文字猜接。覆寫／捲動／清除／過濾後不可殘留舊連結；外開仍須明確點擊、Rust 驗證 scheme，懸停可查看實際目的地 |
| 9/16 原始碼加入 [滑鼠回報模式](https://github.com/ptt/pttbbs/commit/ce97cf7cf91a)，10/15 加入 [動態操作列／說明頁](https://github.com/ptt/pttbbs/commit/a11843a1bde982237d8e209711b85a50c2db2787)、[快速切換選單點擊](https://github.com/ptt/pttbbs/commit/e5e0975826bd)及[新信件標頭熱區](https://github.com/ptt/pttbbs/commit/c2bddaa1074eb5922b0cd526536bedd23e74d8bb) | 既有滑鼠功能由本機導覽轉成按鍵，尚未追蹤站端 mouse mode；畫面辨識仍要求若干固定 caption／指標條件 | 先補新舊操作列的 Vim 正反例，再分批支援協商後的站端滑鼠。一次事件只走一條送出路徑；保留文字選取／複製及本機縮放，測試拖曳、滾輪、座標、重連與模式切換。cmd_t 是站內資料結構，不當成客戶端可直接取得的命令清單 |
| 9/16 原始碼已有 [2FA 登入](https://github.com/ptt/pttbbs/commit/7665e5cd3f69fbe8e467c26b42ca05399a6b1020)與[每次／新 IP 驗證選項](https://github.com/ptt/pttbbs/commit/5bb97f4e90b3faeb030acc57750babea9b789c82)；不以 commit 日期推定首次上線日 | 帳密登入只辨識帳號／密碼提示，密碼已接受最多 72 個可列印 ASCII bytes；2FA 尚無專用引導，原始輸入可手動處理但未實站驗收 | 帳密送出後遇到驗證提示，明確交還使用者輸入六位驗證碼或八位救援碼；支援不需驗證、驗證錯誤、取消與逾時。不重送帳密、不保存碼或 TOTP secret、不把 write 成功視為登入成功 |
| 9/23 [UTF-8 偵測](https://github.com/ptt/pttbbs/commit/69f904fd6fed7f975271c71e32d1abdcd3f56766)、10 月原始碼持續加入 UTF-8 系統內部支援 | 本產品核心與輸入仍以 Big5／UAO 契約為準，沒有確認目前 WSS 需要改編碼 | 先確認連線模式與協商契約，維持既有 Big5；若實作 CPR 回覆必須報告真實格線游標，不為了宣告 UTF-8 回假值。UTF-8 decoder／字寬與 SGR 66 的完整交互另立可驗收增量，不以內部儲存修改推定線上輸出已全面切換 |

實作順序與每批邊界：

1. **相容性基礎**：先做 ECMA-48 邊界／超長丟棄與 DEC2026，再做 SGR 66，各自提交。Rust 核心唯一解析 bytes；Connection 擁有發布時機與恢復期限。同步中舊畫面的點擊／Vim／保活不可誤操作新內容，原始輸入、取消與關窗仍可用。測任意分片、同封包多批次、缺終止、重複開始、新 session 及有界狀態，再以實際 Dioxus host 核對完整畫面與清理。
2. **新操作列辨識**：獨立核對主選單、看板／最愛、文章列表、閱讀及說明頁；保留舊 fixture，新增自製且有獨立 expected 的新格式案例。未知或輸入提示仍保守拒絕導覽，不以刪除 caption 條件全面放寬。實站格式／部署另由人工觀察確認。
3. **完整網址**：先實作有界 OSC 8 metadata 與 cell 生命週期，再接同一套網址點擊、複製及媒體入口。測 BEL／ST 結束、超長／惡意 scheme、重繪與遮黑，並在實際 WebView 核對跨行命中、目的地及取消。
4. **登入相容性**：針對 2FA 增加明確等待／手動接續提示及合成往返；一般 gate 不使用真實帳密、驗證碼或連到 PTT。六／八位碼、無驗證路徑及實站登入各自報告，不以單元測試宣稱帳號已通過 2FA。
5. **桌面導覽**：⌘K／Ctrl+K 快速切板與有界最近看板清單，接著 AID／文章網址開啟及本機書籤。只在已辨識且版本有效的可切換畫面執行，過程可取消、失敗不盲重送；本機最近清單不冒充站端未讀／BRC 同步。站端滑鼠先做模式／選取衝突的設計與合成驗證，再啟用對應操作。
6. **後續候選**：推文草稿、手動開啟的即時推文與搜尋介面；更新必須只在使用者觀看且已辨識的閱讀狀態執行，有間隔、取消與無回應停止條件。多分頁、背景追蹤與發文編輯器需另處理文章／session 所有權，暫不併入上述批次。

各批沿用中英文資源、單一輸入准入與送出 owner，以及對應的 core／desktop gate；平台 not_ready、合成測試、實際視窗與實站結果分別記錄。macOS 原生 IME、睡眠喚醒與既有清理回歸也納入交付檢查，不因新增功能略過。

### 執行進度

使用者已授權以上述計畫為 goal 逐批實作。

**A1 完成核心修正與離線驗證**：Screen 分開處理 CSI 的 parameter／intermediate／final、超長／非法順序的丟棄狀態，以及 ESC intermediate；不把被丟棄序列的尾端寫入格線。C0 在 CSI／ESC 中執行而不結束指令，DEL 忽略，CAN／SUB 撤銷，ESC 可重啟；OSC 接受 BEL／ST，其他字串指令接受 ST，沒有新增回覆或 OS 副作用。Big5／UAO 高位 bytes 不解作 8-bit C1。格式參考 [DEC parser](https://vt100.net/emu/dec_ansi_parser) 與 [Xterm 控制序列](https://www.xfree86.org/current/ctlseqs.html)，只實作產品所需語意，未知命令完整消費。

新增四個核心案例以獨立文字／游標／樣式 expected 覆蓋每個切點、逐 byte、固定亂數多分片、64／65 bytes 邊界、4 KiB 超長、非法順序、C0／DEL、取消／重啟、字串終止與新 session；既有 expected 未修改。core gate（fmt／Clippy、八套 suite、兩份 replay）、desktop lib、desktop 全 targets Clippy、docs 及 diff 檢查通過，紀錄保留 `.local/verification/protocol-a1`。本批沒有修改繪製橋接或打包 App，尚未進行新的原生視窗／實站驗收；A2 同步輸出接續執行。

**A2 完成同步更新與 macOS 原生回歸**：Screen 持有 DEC2026 首次開始 revision，支援同指令的多個 private modes，繼續處理 bytes；Session 使用既有發布時鐘及一秒恢復上限，重複 begin 不延長同批等待，end／reset／斷線清除。View.output_pending 獨立於保留的完整 Frame；收到新終端 bytes 後先更新 watch 撤銷許可，再等待任何網路 write。presentation、SurfaceState、ExportLease 與 JS 一致拒絕過期導覽／選取／複製／連結／hover／PNG；結束後發布新完整圖，原始輸入及關閉保持可用。一秒為本產品選擇；[同步更新規格](https://github.com/contour-terminal/vt-extensions/blob/master/synchronized-output.md) 沒有統一逾時值。

core 八套 suite／兩份 replay、48 個 desktop lib、28 個 bin 及 desktop 全 targets Clippy 通過。新增核心驗證首個 begin、重複 begin、同封包多批次、多 mode、分片、reset；期限測試使用注入的 Instant，不 sleep 等待。production Connection 的 loopback 確認保留批次前完整圖、拒絕舊 Vim／滑鼠、無保活、原始 Enter 與 Ping／Pong 正常、end 後完整閱讀畫面一致、缺 end 恢復，以及新批次中 Close acknowledged／worker join。SurfaceState 案例確認 UI 尚未消費更新時，匯出 lease 已依來源 watch 失效，舊選取／hover 撤銷而新圖可恢復操作。

macOS WKWebView 的最終候選完成 40 筆原生 Vim 操作，每次換頁均含兩段相隔 120ms 的同步更新、未知 ESC intermediate 與超長 CSI；五次切板、無重繪 gg／4j、三次進板圖及六位數指標列表通過，長按 l 一秒只通過一次，最終 NORMAL。⌘Q 退出碼 0、Close acknowledged，listener／Connection／credential／file owner 清理完成。這是可操作性回歸，未宣稱逐像素防撕裂、極速連發、完整原生 IME、實站或其他 OS 通過；完整 Mac desktop backend 仍 not_ready。App 為 `dist/macos-protocol-a2/PTTStella.app`（arm64／ad-hoc strict 驗章），與測試 App 移除各自簽章後的 executable 雜湊相同。報告保留 `.local/verification/protocol-a2`；一次新增測試引用不存在的 fixture 名稱，修正後全套通過，原失敗 log 保留。A3 畫面辨識接續執行。

**A3 完成新操作列回歸與獨立說明／空列表 profile**：以自製 `navigation-actions.json` 確認新主選單、看板／最愛、文章與閱讀操作列沿用既有辨識條件即可通過，沒有放寬一般列表的欄位／指標／色帶要求。新增 CommandHelp 要求 Help 標頭、欄位、連續命令與指標，僅開放移動／翻頁／返回；l／Enter／Open 回報 RequiresNativeInput，具中英文原生一次提示。EmptyList 只在精確空目錄／沒有文章、底列 caption、原欄位、無其他內容及右下游標時允許返回；不開放其他操作、滑鼠或保活。未知、未映射、半張畫面、假提示、游標不符與有輸入提示的負例仍拒絕。

core 八套 suite／兩份 replay（新增三個 navigation 與一個 Vim 案例）、48 個 desktop lib、28 個 bin、desktop 全 targets Clippy 通過；Connection 的 window probe 擴充至 46 筆，額外拒絕 Help Forward／Open，peer 仍只接受指定方向鍵序列。macOS WKWebView 重驗同一 46 筆，包含全部同步分片、進板圖長按、Help j/k／gg/G／h 及空列表返回；另在普通與「刪除」Help 項目按 l／Enter 共四次，在空列表按 j／Enter 兩次，均沒有站端輸入，畫面與原生一次提示正確，最後 NORMAL。⌘W 退出碼 0、Close acknowledged，各 owner join；沒有實站、Windows／Linux、原生 IME 或逐像素防撕裂的完成宣稱。

目前 A 階段 App 為 `dist/macos-protocol-a3/PTTStella.app`；arm64／ad-hoc strict 驗章及與實測 App 去簽章後相同 executable 的核對保留 `.local/verification/protocol-a3`，中間測試 App 可清理而保留報告。B1 SGR 66 接續執行，2FA／完整網址／快速切板等仍依後續批次驗收。

**B1 完成 Big5／UAO 的 SGR 66 與原生像素驗證**：Screen 新增單一 next-cell style，66 保存當下樣式、後續參數改一般樣式，下一個可列印 byte-cell 消耗暫存；ESC／C0／DEL 與 reset 撤銷。沒有修改 renderer、字形配對或 UTF-8 decoder。新增兩個核心案例以獨立預期覆蓋左右前景／背景、強度／反相、同一 SGR reset、單寬字、重複 66、控制碼中止、分片、行末、覆寫、捲動及新 session；bin 另核對兩組等價自製畫面的已知 cell。core 八套 suite／兩份 replay、48 個 desktop lib、29 個 bin、desktop 全 targets Clippy 通過。

兩個 macOS WKWebView 程序使用相同 bundled 設定，分別讀舊式雙色與 SGR 66，實際點擊匯出完整 PNG；Pillow 獨立解碼確認 960×576 的全部像素差零，左右底色為藍／紫，亮色反相左底為亮紅、右底紫，皆符合獨立預期。兩者 ⌘W／⌘Q 退出碼 0、Close acknowledged，listener／Connection／credential／file owner join。一次背景視窗擷取暫缺文字，Raise 後恢復；匯出 PNG 一直正確，未以此宣稱背景合成器時序或防撕裂已驗證。新增覆寫測試起初誤用合法 Big5 尾碼 X 期待未映射，已改用無效尾碼空白並通過；原失敗 log 保留，既有 fixture／expected 未改。

最新候選為 `dist/macos-protocol-b1/PTTStella.app`，報告與必要 PNG 基準保留 `.local/verification/protocol-b1`；本輪未連 PTT，未宣稱實站、UTF-8、Windows／Linux 或完整 Mac desktop backend 通過。B2 2FA 手動接續與 C／D 階段尚待執行。

**B2 完成手動 2FA 引導與原生往返**：密碼送出後由 Session 保留單一無秘密的 FollowUp，期限 120 秒、不隨分片或錯碼延長。完整固定列精確辨識六位限時數字／八位救援碼提示，以及 logind／mbbsd 的錯碼格式；只在本次明確登入後交還原始輸入，不保存驗證碼、TOTP secret 或再送帳密。手動確認後等待站端結果；已辨識瀏覽畫面、返回登入提示、取消、逾時及斷線都結束引導。原生輸入交接保留 Vim 開關，完成後由使用者按 Esc 返回 Normal，不自動執行瀏覽命令。詳細期限與驗收契約見 [登入](../login.md)。

core 八套 suite／兩份 replay（login 新增逐切點、錯碼、假提示、非空欄位／未完成控制碼及可控制時鐘期限；Vim 新增原始交接案例）、50 個 desktop lib、29 個 bin、全 targets desktop Clippy 通過。兩個新增 Connection 往返核對六位錯碼／八位救援碼、過期輸入拒絕、取消、晚到提示與重連零重播；既有無 2FA 路徑維持通過。macOS WKWebView 實際開啟 Vim、按鈕送合成帳密、原始輸入六位錯碼與八位救援碼、Esc 返回 Normal；peer 僅接受一次帳密與兩次手動碼，之後只接受 Close。候選 ⌘W 與最終包 ⌘Q 皆退出碼 0、acknowledged，listener／Connection／credential／file owner join。

最新候選為 `dist/macos-protocol-b2-delivery/PTTStella.app`，strict ad-hoc 驗章與實測 executable 去簽章 SHA 核對一致；報告保留 `.local/verification/protocol-b2`。未連 PTT、未操作真實帳密或 2FA 設定；實站驗證碼、原生 IME、Windows／Linux 與完整 Mac desktop backend 尚未驗收。C1 OSC 8 完整網址接續執行。

**C1 核心與使用入口：當時原生缺口，後續補驗已完成**：Screen 解析有界 OSC 8，目的地由 Cell 擁有，Frame 只在 Rust 保留 metadata；`links` 在過濾後驗證 HTTP／HTTPS 並投影可點範圍，沒有 metadata 時保留原單列偵測。覆寫、擦除、插入／刪除、區域捲動及 reset 同時更新，混合雙寬字與無效／超限 metadata 不改猜文字；右鍵完整目的地的 Rust 選取契約已加入。限制與行為見 [完整網址](../links.md)。

core 八套 suite／兩份 replay 新增七個 interaction 與一個 filters 案例，獨立 expected 核對跨行目的地、全部切點、逐 byte、BEL／ST、錯誤 scheme、控制字元、最大長度／參數／目標數／bytes 預算、slot 回收、混合雙寬、編輯／捲動及遮黑。50 個 desktop lib、29 個 bin、全 targets desktop Clippy 通過，舊 fixture／expected 未修改。紀錄保留 `.local/verification/protocol-c1`；右鍵入口、原生互動探針與驗證結果另記如下，沒有宣稱實站通過。

C1 使用入口已接上同一 Rust 選取契約：右鍵任一片段後 Ctrl+C／Cmd+C 取得正規化完整目的地，普通拖曳仍為畫面文字；原有版本與過濾准入不另建第二條路徑。新增三個 JS handler 案例與一個 bin 樣本範圍／媒體分類案例，30 個 bin、全 targets Clippy、26 個 JS 與 docs 通過。第一次探針漏匯入 selection、整理後殘留 unused import，以及誤選舊雙色樣本均保留失敗紀錄；樣本選擇改為單一生產 helper，bin 直接測實際分派路徑後通過，沒有更改舊 fixture／expected。

macOS 原生顯示正確的兩段網址及兩段本機圖片標籤，AX 匯出 PNG、開啟設定／Esc 取消、⌘W 與 Close acknowledged／全部 owner join 已驗證。CUA 格線座標點擊在 Raise、啟用視窗及新 screenshot 後仍回 `noWindowsAvailable`，因此跨行兩段命中、兩次原生剪貼簿、跨列拖曳零誤開與兩段媒體預覽 **not_ready**；嚴格 surface-probe.json 保留 `failed`，不把 exit 0 或 PNG 當互動通過。待可用原生輸入 backend 或人工執行同一合成探針，不需真實帳密。候選 App 為 `dist/macos-protocol-c1-verified/PTTStella.app`，上一份 B2 交付仍保留；C1 未整批結案，C2 有界切板流程可獨立接續。證據在 `.local/verification/protocol-c1`，Windows／Linux、OS 真正外開與實站仍未驗證。

**C2 第一批：單一有界切板流程已實作，UI／最近清單接續**。新增具驗證的 BoardName 與純 Rust Switch，由 Session 持有工作、operation Stamp、送出 epoch 與有界期限；主選單→最愛列表、閱讀→文章→板列表、固定搜尋提示、列表已選目標後進入，以及直接進空／一般文章列表分開確認。每步 5 秒、總計 30 秒，進板圖／登入／說明交還手動；原始輸入、有效導覽、取消、逾時與 session 變更停止後續，沒有重試或回滾宣稱。開始 Reply 僅是啟動確認，Reached 狀態才表示讀到正式目標名稱。見 [快速切板契約](../quick-boards.md)。

新增五個 navigation 與六個 production Connection 測試，core 八套 suite／兩份 replay、56 個 desktop lib、30 個 bin 與全 targets desktop Clippy 通過。peer 核對精確步驟、同步分片期間零板名輸入與 Pong、工作期間零保活、舊取消不影響新工作、手動介入、晚到提示、5 秒逾時、重連零重播與工作中 Close acknowledged。write_input 在 await 前更新 watch 的 input epoch／等待狀態；既有檔案 lease 的 input 失效、Vim／登入／清理回歸維持通過。fixture 是自製場景與手寫 expected，原有 expected 未改。證據保留 `.local/verification/boards-c2`；這批只交付 Rust 流程，尚無新的快速切板 UI／最近清單或對應原生／實站完成宣稱。

**C2 第二批：快速切板 UI 與 macOS 鍵盤驗證已完成；最近清單接續**。獨立 JS owner 管理草稿、IME、request ID、快捷鍵與取消；Rust typed event／Packet 沿用統一准入與單一命令等待。⌘K／Ctrl+K 不送到 PTT，未確認草稿取消零 bytes；超長名稱保留完整內容並拒絕，不默默截成另一個板名。舊回覆不能關閉新草稿；session 改變／關閉撤銷等待及後續工作，關窗清理由共同 owner 接手。

56 個 desktop lib、30 個 bin、31 個 JS、全 targets Clippy、docs／diff 通過；新 BoardResult 使用獨立手寫 wire expected，事件正反例納入原先准入 suite，fixture 摘要加入 board-switch 與 UI packets。macOS 真正 WKWebView 啟用 Vim，驗證 ⌘K／Ctrl+K、草稿取消、Enter／按鈕確認、Stella（列表選中後進入）與 Other（不在列表直接到達）、13 字元草稿完整保留及拒絕、第三次工作有效停止、晚到列表零續送與 PNG；⌘W／Close acknowledged、全部 owner join、退出碼 0。首次取消分開工具回合超過 peer 3 秒間隔，工作已續送，該次 cleanup 為 failed 並保留；相同 binary、相同預期，改為在同一次呼叫讀最新 AX 後立即停止，第二次嚴格探針通過。一次 snapshot 暫缺部分文字，匯出／Raise 後恢復，沒有據此宣稱原生合成器或逐像素防撕裂已驗證。

同一候選 App 重跑全部 46 筆原生 Vim 操作（53 個實體鍵），包含同步分片、無重繪、三種進板圖繼續、密集文章、Help 與空列表；最後 Normal，⌘Q／Close acknowledged 與各 owner join。候選為 `dist/macos-boards-ui/PTTStella.app`，strict ad-hoc 與已測 executable 去簽章 hash 核對保留 `.local/verification/boards-c2`。本輪未連 PTT，原生 IME 候選窗、Windows／Linux、實站切板與完整 Mac desktop backend 未驗收；最近清單仍待實作，C1 原生滑鼠工具缺口也未結案。

### 詳細交付計畫

2026-10-17 展開上述研究；本節是待執行計畫，不代表產品已支援。先交付相容性，再交付高頻桌面操作。每一批完成實作與適用驗證即提交，不累積到整個階段結束；不改寫既有歷史、不自動 push。以下為依賴順序與相對工作量，不是日曆工期承諾。

| 里程碑 | 批次 | 使用者可驗收的結果 | 相依／相對工作量 |
| --- | --- | --- | --- |
| A 穩定瀏覽 | A1 控制碼解析、A2 同步輸出、A3 新舊畫面辨識 | 新指令不污染內容，部分重繪不觸發錯誤導覽；主選單到看板／文章往返保持可操作 | A1 → A2 → A3；中／高／中 |
| B 登入與顯示相容 | B1 SGR 66、B2 2FA 手動接續 | 新舊雙色格式一致；遇到驗證碼時知道如何繼續，無須再次送密碼 | A；中／中，兩批可獨立交付 |
| C 高頻桌面操作 | C1 完整網址、C2 快速切板、C3 AID／書籤 | 長網址可用；少數操作即可切常用板、開指定文章及返回書籤 | C1 依賴 A1；C2 依賴 A2／A3；C3 依賴 C2 的有界導覽工作；高／中高／中高 |
| D 站端滑鼠 | D1 模式與本機操作分流、D2 點擊與滾動 | 可使用站端操作熱區，仍能選字、複製與縮放，單一動作不會送兩次 | A／C1，另確認實站部署與操作契約；高 |

**A1：有界且可恢復的控制碼解析。** 主要修改 Screen parser 與 `tests/screen.rs`，保留既有顯示契約。分開辨識 CSI parameter／intermediate／final；未知合法命令完整消費，超長命令進入丟棄狀態直到合法結束或明確中止，不能把剩餘參數畫成文章。所有狀態都保持固定或明確有界儲存；ESC 重啟、CAN／SUB、中間的控制字元及非法順序依協定逐項定義，不用一個「不認得便回 Ground」處理全部情況。OSC／其他字串型控制碼只為後續 metadata 保留清楚入口，不在本批開放剪貼簿、任意外開或其他副作用。

驗收使用自製串流，覆蓋每個單一切點、逐 byte、固定亂數多分片、空參數、private／intermediate、剛好上限／超限、連續未知命令、中斷／重啟及後接正常文字。獨立 expected 核對內容、游標、樣式與控制碼完成狀態；chunk invariance 另作補充，不能只拿同一實作的完整輸入結果當正確答案。既有兩份 replay 與雙色字 expected 保留。提交邊界：解析修正與對應測試。

**A2：同步輸出與可操作性分開。** Screen 唯一知道 DEC2026 開／關及 bytes 的順序；Session 唯一擁有單調時鐘、恢復期限與發布決策，沿用現有事件迴圈，不新增 UI timer 或背景發鍵 task。批次中持續處理所有終端 bytes、Telnet 協商、Ping／Pong、取消及關閉；畫面可保留上一張完整圖，但收到新來源就撤銷舊互動。Connection View 需能表達「圖仍是上一版、來源正在更新」，由 SurfaceState／ExportLease 消費；不得只凍結 `View.frame`，讓舊 revision 看起來仍有效。具體欄位隨實作決定，避免複製第二份 Screen。

同步期間拒絕依畫面計算的 Vim／滑鼠導覽、保活與新 PNG 請求，並撤銷尚未開始寫入的舊匯出／hover lease；原始輸入及退出仍可用。結束後只對完整狀態恢復辨識；同封包多個批次保留完整最終狀態，允許既有 watch 合併顯示，不丟 bytes。重複開始不累加無界巢狀或無限延後恢復；缺結束碼不能讓 UI 永久停住。恢復期限先核對公開協定，再以慢速／缺碼案例選定並寫成常數與測試；測試使用可控制時鐘，不依賴長時間 sleep。逾時釋放顯示後仍須通過完整性與畫面辨識才可導覽，不自動補送被拒絕的操作。原始輸入在部分更新中若失效游標／版本，後續仍以 Session 當下狀態重新判定，不能沿用舊計畫。

除核心案例外，以 loopback 在 begin／畫面片段／end 之間刻意延遲，核對沒有部分 Paint、舊點擊／Vim／匯出／保活沒有副作用，end 後新操作恢復；另測 end 單獨到達、重複 begin、缺 end、斷線、重新連線、批次中關窗與已入列操作。實際 Dioxus host 顯示批次前後圖，核對中途設定視窗與 ⌘W／⌘Q 仍可用。提交邊界：同步狀態、准入、生命週期及整合測試必須一起可驗收。

**A3：新舊操作列與 Vim 回歸。** 以 `navigation` 為畫面辨識唯一擁有者；只有鍵意圖改變時才修改 `vim`，JS 不增加文字辨識或 key-to-byte 對照表。分別檢查主選單、分類／看板／最愛、文章列表、閱讀、進板圖及新說明頁。新說明頁可能提供執行命令，須有自己的保守 context／意圖，不可當成一般文章列表套入閱讀或刪除操作。新增變動 caption／順序／空列表的自製正例，配對正文仿冒標題、彈窗、未映射區、游標不符、半張畫面與六位數編號的負例；不以全面移除舊辨識條件解決問題。

實際視窗延續既有 40 筆 Vim 序列，再加入新格式往返、無重繪邊界與長按跨頁；本機 peer 逐筆核對 bytes，不只看 NORMAL 標籤。若官方來源不足以確認某畫面的完整輸出，該 profile 保持待驗收，先交付已確認的案例；實站只觀察必要操作，不錄製完整 session。提交邊界：每組可獨立說明的辨識修正與正反例；A 完成後先交付一版可使用 App，尚未完成的新功能不混入。

**B1：SGR 66 與既有雙色契約。** 修改 Screen 的樣式轉移，不先導入 UTF-8 或改 renderer 為每字單色。先用公開格式寫出獨立預期，定義延後作用的左右樣式、單寬字、完整雙寬字、行末、覆寫、捲動、SGR reset 及中斷後的狀態；不把 66 後所有參數直接套成整字新色。以相同內容的新舊格式比對字形、左右背景／前景／明暗及後續正常字；實際 WebView 與 PNG 的雙色／幾何基準應一致。提交邊界：核心格式支援與顯示證據；未完成 UTF-8 decoder 時只報告已驗證的 Big5 範圍。

**B2：2FA 先改善手動接續。** 第一版沿用站端驗證碼輸入，不新增保存 OTP 或產生 TOTP 的服務。`login` 定義保守的驗證提示辨識；Session 在密碼釋放後只保留必要的登入流程識別與期限，清楚區分等待碼、碼被拒絕、需人工處理與已進入已辨識畫面。UI 透過現有 typed Packet 與 Fluent 提示使用者在 PTT 輸入；只有實際出現提示才進入該狀態，不因帳密 write 成功顯示「登入成功」。不讓文章中的相似文字觸發引導；無 2FA、同 IP 免驗證、錯碼重試、六位碼／八位救援碼、取消、手動介入、逾時與新 session 分開測試。不自動改站端 2FA 設定。提交邊界：手動接續引導與合成往返；是否實站通過另記錄，無可用人工案例不冒稱完成。

**C1：跨行連結使用站端 metadata。** Screen 解析有界 OSC 8，URL 與 cell 的附著、覆寫、捲動、清除及 session reset 由核心一起維護；`links` 做唯一 HTTP／HTTPS 驗證與可點範圍投影。沒有 metadata 時保留現有同列偵測，不猜接任意相鄰行；同一 cell 不重複產生兩個候選。presentation 遮黑後再產生可點／可複製／可預覽的結果，避免從被隱藏文字洩漏目的地。目的地與顯示文字可不同，需有可查看完整實際 URL 的 UI；URL 不拼進 HTML 或可執行 JavaScript。

先定 URL bytes、活動連結數與整張畫面的總 metadata 預算；超限拒絕連結仍繼續消費控制碼、保留普通文字。測 BEL／ST、逐 byte 分片、開關與巢套替換、空 URL、非法 scheme／控制字元、換行跨頁、覆寫／擦除／捲動、雙寬字、過濾、過期點擊及拖曳。實際 WebView 核對跨行任一片段開啟／複製同一完整 URL、拖曳零外開、明確點擊一次，以及沿用媒體載入的取消／容量限制。提交可拆為「metadata 核心＋生命週期測試」與「使用者入口＋WebView 證據」，兩批均不得宣稱超出實作範圍。

**C2：快速切板先做最小可靠流程。** ⌘K／Ctrl+K 開啟本機對話框，輸入板名或從最近清單選擇，確認後才啟動切換；Esc 關閉且不送站端資料，組字確認的 Enter 不兼任送出。第一版支援已驗證的主選單／列表入口，閱讀頁若要返回再切板，必須逐步等待並辨識新畫面；編輯、提示、登入或未知畫面明確拒絕，保留手動操作。不從任意畫面盲送固定快捷鍵串。

導覽工作由 Session 持有單一有界狀態及期限，使用具名意圖／預期畫面逐步前進；取消只停止尚未送出的步驟，已送 bytes 不宣稱回滾，使用者手動介入立即取消後續自動步驟。沿用同一送出 owner，UI Pending 只等結果，不另建佇列或 detached task。板名依實際站端格式驗證，顯示單獨設定的錯誤；站端無此板、無權限、進板圖與畫面不符都停止，不循環重試。最近清單先定上限 20 筆，只有確認到達目標板的已辨識畫面才更新，按站端確認名稱去重排序；由 Files 保存到獨立且有版本的本機資料，不混入字型 profiles、密碼或完整瀏覽流量。保存失敗不回報切板失敗，分開提示；提供清除清單。

驗收主選單／列表／閱讀可支援入口、中文 IME、大小寫、無此板、取消、逾時、連線中斷、原始輸入介入、快速重複確認、連續進板圖及新 session；peer 比對完整步驟、拒絕後零額外 bytes。明確限制支援入口後先交付，不為了擴大入口建立通用腳本引擎。提交拆為「有界導覽工作＋單一切板契約」、「快速切板 UI＋中英文與實際視窗」及「最近清單＋保存／清除／重啟測試」。

**C3：AID 與書籤分開交付。** 第一批驗證並正規化板名＋AID 及明確支援的 PTT 文章網址，沿用 C2 的逐步等待／取消；不能把任意網址尾段直接當按鍵送出。文章不存在、已刪除或權限不足均停止並保留可操作畫面。第二批才做本機書籤列表：由可信的目前文章識別或使用者明確輸入建立，未知時不從標題／頁碼猜 AID。保存板名、AID、可選顯示名稱與建立時間，不保存整篇文章；以板名＋AID 去重，上限先定 500 筆，支援新增／刪除／開啟與容量提示。沿用 Files 的原子保存、備份與失敗保留，重啟另驗證；跨頁全文與離線文章模型不屬此批。若目前閱讀畫面拿不到可靠 AID，先交付手動新增，不自動補送跨頁抓取命令。

**D：站端滑鼠作獨立增量。** 先實作 mode tracking 與模式變更時取消本機 pointer 交易，再接送出。站端要求回報時不能同時執行本機「點編號送方向鍵」；未協商／使用者關閉此功能則維持已驗證的本機行為。先選定明確的選字覆寫手勢並驗證 macOS／Windows，讓拖曳選取、複製與 Ctrl＋滾輪縮放保留。第一批只實作已確認的點擊／釋放與 SGR 座標格式；1002／1003 的移動回報、觸控板自然捲動與 motion 合併另做，未支援時不得假裝模式已完整可用。座標換算使用 renderer 同一格線，排除黑邊；限定 session、實際 mode、可信事件與送出容量，連線重設清零。驗收重複事件、拖曳離窗、lost capture、hover／選字衝突、縮放／DPR、模式開關及重連；確認一次操作只有一條輸出路徑後才交付。

### 共用驗收、交付與後續決策

- **測試分層**：純 Rust 契約與獨立 expected → production Connection 的本機 WebSocket 往返 → 涉及可見畫面／焦點／事件的實際 Dioxus host → 必要的實站人工案例。一般 gate 只用合成資料與本機服務；失敗與 not_ready 保留，不以改 expected、DOM doubles 或模擬按鍵冒充原生驗收。
- **平台範圍**：本機先驗 macOS 新功能，Windows 需要自己的 WebView2 gate；Linux 未執行即保持未驗證。Mac 完整 desktop backend 的既有 not_ready 不要求一次解完所有 runner，先把每批所需 probe 接入可重跑命令與結構化結果，明列仍缺的互動／codec／IME 能力。
- **共同回歸**：送鍵順序、Vim 無重繪連按／進板圖、貼上取消、IME Enter、過濾後複製／連結／PNG、設定保存／重啟、關窗／⌘Q 及 worker join。每次只跑適用範圍；只有新改動、失敗或未解疑慮才重跑／擴大，不反覆重跑已通過且未受影響的全套。
- **交付條件**：適用的 fmt／Clippy、core、desktop／JS、docs 與 diff 檢查；App 的內容與已測 executable 可核對，簽章有效，保留最新可用包、驗證結果與必要像素基準。每個功能提交附短 body 說明行為與驗證，工作目錄不能長期堆積下一批。具名維護性整理只在支援當前批次時進行，不再混入整體重構。
- **停止與降級**：協定／部署未確認先保持未支援；期限／版本／畫面不符就停止後續送出並說明原因。恢復顯示、切板失敗、本機資料保存失敗、站端登入成功是不同結果，各自回報；不自動重連登入、不補送舊 bytes。平台 runner 或資源 guard 拒絕時回報具體 gate，不能繞過或標為 passed。
- **完成 A 後重新排序一次**：依實站是否仍有誤辨識、2FA 是否影響登入、跨行連結與切板的實際使用頻率，調整 B／C 順序。原生 IME 與睡眠喚醒若出現可重現問題立即優先修；UTF-8 以確認的實際需求與傳輸契約再決定，不跟著站內儲存格式直接改線路。
- **後續候選不併入首輪**：推文草稿先做本機暫存與明確送出；即時推文須先確認閱讀位置、刷新間隔與停止規則，並與 keepalive 共用操作排程，不設第二個自動送鍵 timer。搜尋介面及名單匯入匯出可獨立驗收；多分頁、背景追蹤、發文編輯器待文章／session 所有權另行設計。

**C2 第三批：最近清單、保存／清除與 macOS 重啟驗證完成**。獨立 RecentBoards／Visits 只消費完整、已辨識且不在待回應的文章／空文章列表，記錄正式名稱、忽略大小寫去重、最多 20 筆；板列表選中列、搜尋草稿與進板圖不當成到達，清除後原畫面也不立刻重新加入。選清單只填草稿，仍需「前往」；一併限制 Enter 只在名稱輸入欄確認，避免在取消／清單按鈕上誤送舊草稿。繁中與英文訊息均走 Fluent。

資料是獨立 version 1／4 KiB 本機 recent-boards.json，不進顯示 profiles。Files 擁有一個進行中及一份合併後最新 snapshot，可與互動式 picker 並行；關窗保留最新已接納資料並 join，錯誤分開顯示。一般寫入先驗證及備份原檔後原子替換，損壞／超量只留本次記憶體清單，直到明確清除；清除不備份舊紀錄，而是替換空備份與主檔，任一步失敗都報未完整清除，待清除意圖不會被新到訪合併掉。這是兩個檔案分別原子替換，不是跨檔案交易。

62 個 desktop lib、36 個 bin、33 個 JS、desktop all-targets Clippy 通過。新增案例涵蓋資料版本／格式／容量、大小寫去重、正式到達、同步／待輸入拒絕、重啟、損壞保留及明確恢復、備份與主檔清除失敗、慢 worker 合併不丟清除、picker 期間保存、關閉 drain 與失敗不假報成功。macOS 原生候選 App 完成兩次到達、草稿取消、最近項目只填草稿、超長拒絕、Beta 取消後晚到列表零續送；新程序還原 Other／Stella，清除後主檔及備份皆空，第三程序仍空。另在 Beta 等待中直接 ⌘Q，Close acknowledged、所有 owner join、無額外 bytes；同一候選重跑 46 筆 Vim／53 實體鍵維持 Normal 並正常結束。證據在 `.local/verification/recent-c2`，最新候選為 `dist/macos-recent-boards/PTTStella.app`；本輪仍只使用本機合成服務，不宣稱原生 IME、實站、其他 OS 或 C1 滑鼠驗收完成。接續 C3 AID／書籤。

**C3 識別驗證已實作，導覽與入口接續**。新增 ArticleTarget／Aid，驗證八碼 raw identity、板名、M／G 及新舊文章檔名，將明確支援的 www.ptt.cc 文章網址與 #AID@Board 正規化為同一型別；拒絕控制碼、保留位元、時間超限、非預期 authority、顯式 port、query／fragment、編碼／dot path 與多餘路徑。四個純 Rust 案例使用手寫 expected，另跑 Fluent、core／Clippy／文件 gate；無新 UI 或 App 交付宣稱。站端 # 搜尋只移動選取，已核對資訊框提供 AID／板名的來源，下一批沿用有界工作逐步確認再開啟。契約見 [AID 與指定文章](../article-navigation.md)，證據保留 `.local/verification/aid-c3`；Session 接線、原生操作、書籤與 D 尚未完成。

**C3 導覽核心與單一工作接線已完成，文章入口接續**。新增 Open reducer，組合既有 Switch；從已確認板頁送 #，核對精確空白提示後送八碼 AID／CR 一次。搜尋回到列表後先保留選取列、送 Q，核對完整資訊框幾何、加粗 AID、板名與原列；只有匹配的已請求資訊框可送空白，再等同列返回才送方向右，最後確認首行作者／板名及一般閱讀起頁。不存在／已刪除、權限提示、錯 AID／錯板與選取列變更停止；同步期間零繼續，未知片段不延長每步 5 秒／總計 30 秒。M／G 在同一已辨識格式核對，不宣稱涵蓋所有文摘／閱讀版面。

Session 只持有一個 NavigationJob，Board／Article 是封閉 Destination，沿用同一 write_input、epoch、手動操作優先、保活暫停、取消與清理。原 BoardProgress 改為 View.destination 的 typed progress，Paint.navigation 仍是畫面 context，UI 消費者與獨立 wire expected 已一併核對。開始 Reply 仍僅確認啟動；Reached 才是完成。為免 AID 等待期間漏掉切板到達，Session 發布帶原 generation／serial 的最後 BoardVisit，UI 最近清單只消費一次；錯誤／閱讀／watch 合併保留確認事實，新連線先清舊來源，清單清除不重置消費序號。

六個純文章流程與六個 production Connection 案例、共同進度 wire case，合計 79 個 desktop lib、36 個 bin、34 個 JS，以及 core／Clippy／docs 通過。測試核對每筆獨立 expected bytes、M／G、所有提示分片、同步資訊框與 Pong、五階段取消／原始輸入、舊 generation 不取消目前工作、錯誤與晚到資訊框零續送、真實五秒逾時、重連零重播、工作中 shutdown，以及待回應中仍保存板到達與原 generation。兩個新增負面案例的初期測試問題保留於證據：無效游標未構成合法換列、實際 Other 板的確認事實不應斷言為 Stella；均改用明確正反輸入與獨立預期，沒有放行錯誤頁。

macOS 同一候選 `dist/macos-article-core/PTTStella.app` 通過既有 C2 UI 的兩次到達、超長拒絕與 Beta 取消，最近清單仍為 Other／Stella；同一版本重跑 46 筆 Vim／53 實體鍵保持 Normal，⌘W／⌘Q Close acknowledged 與全部 owner join。PNG 與上一份相同合成板列表基準逐像素相同；這些只證明共同封包／UI 回歸，**不算 AID 原生操作完成**。證據在 `.local/verification/aid-c3/workflow`。最新先前交付 App 仍保留，下一批接 AID 使用者入口及其原生探針，再做書籤；C1 滑鼠缺口、D、實站、原生 IME 及其他 OS 仍未結案。

**C3 指定文章入口與 macOS 原生驗收已完成，書籤接續**。快速前往共用板名、#AID@板名及支援的官方文章網址，事件／結果／命令等待改為 Destination，移除舊 board-only owner 路徑。Rust 先檢查 256 bytes／控制字元再 trim，HTML 上限由常數加一（257 units）產生；合法板名加大量空白與非法尾段即使被截尾也整段拒絕。草稿、IME Enter／229、取消、request ID、session 變更與清理維持共用 owner；最近項目只填草稿。中英標籤與錯誤走 Fluent，獨立 wire expected 一併更新。

80 個 desktop lib、36 個 bin、35 個 JS、core／Clippy／docs 通過。macOS 三個獨立程序使用同一候選 App：兩個 AID 探針均經真正 ⌘K／Ctrl+K 及 Enter／按鈕開啟網址與 AID 目標，觀察缺文後手動空白恢復、混淆主機與 257-unit 長草稿拒絕；第一輪有效取消，晚到 #00000104 資訊框零續送，第二輪在同一工作等待中 ⌘Q，均 Close acknowledged、全部 owner join、正常退出。第一輪長字串快照尚在輸入中（142 units），未將中途快照算成結果；完整 257-unit 值確認後拒絕，重按一次普通 Enter 亦只增加拒絕、未送站端 bytes。第二輪先確認完整值再送 Enter，恰好兩次拒絕。原生輸入時序觀察不冒充 IME 候選窗驗收。

第三程序回歸純板名 Stella／Other、最近項目只填草稿、超長拒絕、Beta 停止與晚到列表，最近清單維持 Other／Stella，Vim Normal 及正常清理。候選 `dist/macos-article-ui/PTTStella.app` 的嚴格 ad-hoc 簽章、去簽章 executable hash 與 PNG 檢查保留 `.local/verification/aid-ui-c3`。以上均為本機合成服務，實站、Windows／Linux 及原生 IME 未驗收；書籤、D 與 C1 原生滑鼠缺口仍未結案。

**C3 書籤資料核心與保存已完成，UI 接續**。新增封裝的 Bookmarks／Entry：只持有明確 ArticleTarget、可選名稱與建立秒數；板名忽略大小寫、AID 區分大小寫去重，更新保留位置／建立時間，最多 500 筆。名稱在 trim 前限制 128 字元／512 bytes、拒絕控制字元；獨立 version 1／512 KiB 文件整份驗證，固定手寫 fixture 核對身分、名稱、時間與序列化。正常新增／更新／刪除返回草稿，不改動原模型。

獨立本機 store 只在既有主檔有效時備份與替換；壞檔／超量／未知版本、備份或主檔失敗均明確拒絕並保留資料。與最近清單共用有界讀取及逐檔原子替換，最近的明確清除／損壞恢復政策維持原契約。88 個 desktop lib（含六個書籤與兩個 storage 案例）、36 個 bin、Fluent JS、core／desktop all-targets Clippy／docs 通過；涵蓋 500 筆邊界、長 Unicode 名稱、重複及非法文件、重啟與刪除備份、壞檔雙副本保留、實際目錄造成的保存失敗與暫存清理。證據在 `.local/verification/bookmarks-c3`。這批尚未接 UI／Files，不宣稱書籤原生操作或新 App；接續手動新增、刪除、選取前往及重啟驗收。

**C3 手動書籤 UI／Files 與 macOS 重啟驗收完成**。工具列新增「文章書籤」，明確輸入 AID／文章網址及選填名稱、選取更新／移除；前往先交給同一快速前往 owner 填草稿，再確認才走既有 Session。Rust State 只在 Files 保存成功後 commit，JS 持有獨立 request／草稿／busy／IME／對話框生命週期；晚到讀取、關閉及重新開啟不重開視窗、不搶焦點或覆蓋新草稿。已開始保存不因關閉 dialog 遺失追蹤；主視窗關閉沿用 Files drain／join，未開始工作可撤銷。壞檔保存／移除停用，普通 Enter 同樣拒絕；已存標籤以 textContent 顯示。

89 個 desktop lib、39 個 bin、42 個 JS、core／desktop all-targets Clippy／Fluent／docs 通過；包含保存前不 commit、錯誤保留、request 回傳、撤銷未開始寫入、晚到與重開競態、readonly／不可信事件、IME／229、選取移除、共用快速前往及清理。共享的合成文章 probe 步驟沒有改 expected，新增 Close 在延遲查詢中已完成後不能再等第二個 frame 的實際 duplex WebSocket 回歸。最初 SaveStatus／參考型別接線編譯錯誤已修正，初期失敗紀錄保留而不計入通過。

macOS 四個獨立程序使用同一 `dist/macos-bookmarks/PTTStella.app`：繁中新增兩筆、中文更名、移除一筆、無 AID 草稿拒絕與取消，重開選取後經快速前往開啟正確文章，畫面可見 Normal，匯出 960×576 PNG；英文重啟還原同一中文名稱及識別，移除最後一筆；第三程序載入空檔仍空；第四程序載入自製壞檔顯示英文警告、保存停用，合法目標加 Enter 亦零修改。第一程序中文 typeText 未填入，改為原生貼上並確認 AX 值後保存，因此共有五次保存／一次拒絕；不把這項工具輸入觀察算成 IME 候選窗驗收。四者皆 Close acknowledged、全部 owner join、退出碼 0，後三個 peer 拒絕所有站端 bytes。

獨立檢查更名保留建立秒數、移除備份為前版、seed 與重啟資料 bytes 相同、空檔與壞檔未自動改寫；四份測試 App 與候選去簽章 executable hash 相同、strict ad-hoc 簽章有效，PNG 獨立解碼完成，未與不同場景假稱像素相等。證據保留 `.local/verification/bookmarks-ui-c3`。C3 第一版完成於明確手動識別及已辨識文章格式，不含自動猜目前 AID／離線文章；本輪未重跑 46 筆 Vim、實站、原生 IME、Windows／Linux。接續 D 站端滑鼠，C1 原生 pointer backend 缺口仍未結案；沒有 push 或公開發佈。

**D 第一批：模式核心與 SGR 編碼完成，送出／UI 接續**。Screen 透過同一有界 CSI parser 持有獨立 Modes，追蹤 9／1000／1001／1002／1003 與 legacy／1005／1006／1015／1016；後者互斥並只重設相符編碼，tracking DECRST 關閉回報。有效變化累加 epoch，off／on 合併仍可觀察；RIS 清模式且保留單調變化，新 session 從 default／0 開始。模式不放進 retained Frame，DEC2026 期間也可由核心觀察；格式支援不等於送出准入。

只有 1000＋1006 可產生三個具名按鈕的 press／release，唯一 encoder 處理 Shift／Alt／Ctrl、零基轉一基與同一 Screen 面積／格線邊界；motion、wheel、其他 tracking／encoding 明確回傳未支援，不產生部分 bytes。公開站端可能在啟動要求 1003，不能當作已支援點擊；協定與後續分流契約見 [站端滑鼠](../station-mouse.md)。六個純核心案例以固定手寫 trace、所有 byte 切分、取消／錯格式／超長／字串、C0、重設／同步、八種修飾鍵與獨立封包 expected 核對。95 個 desktop lib、39 個 bin、core／desktop all-targets Clippy／docs 通過，初期測試內無需 Vec 的 lint 已修正並保留紀錄；證據 `.local/verification/mouse-d-core`。

這批尚未接 Session 的交易 owner／容量／速率、View／Paint、使用者開關、可信 pointer 或本機／站端唯一分流，不宣稱站端滑鼠可用或已原生驗收。最新交付 App 仍是書籤版；接續上述接線與本機 peer，再驗原生 pointer、Shift 選字、Ctrl＋滾輪、黑邊／DPR／模式變更與關窗；C1 pointer backend 缺口一併處理。

**C1 macOS 原生互動補驗完成，指定案例 not_ready 已解除**。使用已驗證的書籤交付 App（19efa34，包含完整網址；不含其後 0545a47 的 D 核心）建立獨立 probe。直接 ⌃⌘F／符合視窗後，CUA 原生座標事件可到達 WebView；以選取行提示確認截圖／輸入間垂直偏移，最後完整程序依序各開啟與複製兩段，嚴格 rows 均為 `[8,9]`。每次 Cmd-C 後在本機書籤草稿用 Cmd-V 貼回，AX 均可見同一完整合成目的地；未保存資料。跨列拖曳為第 9–10 行，沒有額外外開；兩段圖片都由真正 WebView 解碼，第二次記憶體快取，HTTP 只有 `/colours.png` 一次。完整 screenshot 確認紅／藍兩半，差分圖不當作缺像素。

嚴格 surface-probe.json passed、PNG 獨立解碼 960×576 且與原先相同場景基準逐像素相同；⌘Q／Close acknowledged、全部 owner join、退出碼 0。去簽章 executable hash 與 `dist/macos-bookmarks/PTTStella.app` 相同，strict 簽章有效。兩次前置試驗保留為失敗：native-menu Escape 送到 BBS 而被 peer 拒絕（cleanup false），以及座標校對先命中第 10 行使嚴格順序失敗；均未改 expected 或冒算通過。證據在 `.local/verification/osc8-pointer-recovery`，先前 `.local/verification/protocol-c1` 的失敗／not_ready 亦保留。

本次只解除指定 OSC 8 片段開啟／剪貼簿／拖曳／PNG 媒體案例的原生缺口，不宣稱完整 macOS desktop gate、一般視窗所有座標、所有 DPI、影片／GIF、OS 真正外開、原生 IME、實站或其他 OS 通過。A／B／C 的本階段明確子集已交付，D 仍須接 Session 與 UI 並獨立驗收；沒有 push 或公開發佈。

**D 第二批：Session 交易准入完成，UI 分流接續**。Rust 持有單筆 30 秒按鈕交易，以 generation／mode／policy／原始畫面 stamp 驗證按下；同一租約釋放可跨按下引起的重繪，重複／舊事件零輸出。每秒 20 次按下，64 筆共同輸入佇列保留 8 筆新按下餘量；獨立有界 watch 保存偏好／全域撤銷／單筆取消，滿佇列也能撤銷尚未開始的按下。人工操作增加 policy nonce，與控制 serial 分開；斷線／重連／模式切換清交易，不合成 release。View 提供 Rust 推導的 Local／Station／Blocked，DEC2026 期間模式變化不等 retained Frame 才發布。按下等待回應安靜逾時，釋放不重啟等待；持有期間防閒置暫停。

111 個 desktop lib（含八個交易、一個 watch 與七個 Connection 滑鼠案例）、39 個 bin、42 個 JS、core／desktop all-targets Clippy／docs 通過；精確 loopback bytes 覆蓋更新後釋放、queued cancel、off/on 合併、同步期模式改變、原始輸入介入、滿 64 筆控制、重連後舊事件不改狀態、持有時安靜逾時與關窗 Close。最初 no-default-features Clippy 發現 interrupt 只在 desktop 使用，已補運輸中斷的實質 owner 回歸；失敗紀錄保留，無修改 fixture expected。證據 `.local/verification/mouse-d-session`。仍未接 Paint／UI pointer、使用者開關及本機 pointer 導覽來源檢查，尚不能宣稱單一動作分流或原生站端滑鼠完成；最新 App 仍是書籤版，沒有 push。

**D 導覽來源隔離已完成，pointer 接線接續**。以 Source enum 取代 keyboard bool，明確區分 Pointer／Button／Keyboard；JS 的 Navigate 必須標示來源，缺少／未知來源拒絕，也不能在此路徑冒充 Rust Vim。Session 在副作用前核對 Pointer 的當前 route：1000＋1006 與未支援的 1003 均不得再送本機方向鍵，停用後恢復；工具列與鍵盤仍可明確送出。112 個 desktop lib、40 個 bin、43 個 JS、core／desktop all-targets Clippy／docs 通過，證據 `.local/verification/mouse-d-routing`；新增 loopback 獨立核對零 bytes／不改 epoch、按鈕／Vim 的方向鍵及停用後 pointer 回復。這批未接站端 UI pointer，最新交付 App 不變，沒有 push。

**D UI pointer 與分流已接線，原生驗收接續**。Paint 傳同一 Rust Snapshot，獨立 JS owner 管理可信 primary pointer、原始 lease、單調編號與 capture；release 可跨正常畫面更新，模式／policy／世代、失焦、同步／未完成輸出、lost capture、編輯或 modal 撤銷。站端模式不再觸發本機點編號／網址／hover／普通滾輪送鍵，Rust 二次檢查導覽與網址；Shift 在按下時選本機選字，先放開 Shift 仍可結束拖曳，Ctrl＋滾輪保留縮放。工具列可關閉站端回報恢復本機；1003 等未支援模式明示，不當作 1000 支援。press／release 不占用共同命令等待 slot；入列失敗透過 lease 回覆及 watch 撤銷，舊拒絕不能取消新交易。共同 Rust 清理先撤銷 mouse，再停止 UI／connection，JS destroy 清 capture，沒有新 timer。

112 個 desktop lib、42 個 bin、51 個 JS、core／desktop all-targets Clippy／docs 通過；Surface 測試覆蓋顯示 ACK、模式／policy、可信事件、封閉回報型別與獨立 wire expected，八個 JS owner／handler 案例覆蓋三按鈕、跨重繪、取消／capture、Shift、普通滾輪零送鍵與 Ctrl 縮放。初期 router 的獨立 handlers 清單漏列新拒絕封包，已補同一 wire 契約並保留失敗紀錄。證據 `.local/verification/mouse-d-ui`；後續 capture-loss／精確 lease 修正的對應 JS 回歸另跑通過。尚未執行這批 UI 的原生 pointer 或新 App 打包，不宣稱 macOS／Windows、所有 DPR、實站、motion／wheel 已驗收；最新可用 App 仍為書籤版，沒有 push。

**D 限定 macOS 原生點擊案例通過，完整原生邊界接續**。新增 `mouse` 本機 peer：精確要求 col 4／row 4 左／右鍵各一對、使用者停用後點文章編號的一次 CR，以及中鍵按下；中鍵保持 1,200ms 時，peer 送同步 off／on，後續只接受 Close，拒絕已取消的釋放／額外本機 bytes。實際 CUA 視窗依序完成 Shift 雙擊第 5 行、Cmd-C 後在未保存書籤草稿 Cmd-V 讀回同一合成文字、左鍵、1003 不可用提示下點擊／普通滾輪零送出、鍵盤 100→125→100% 縮放、關閉回報點編號、重新開啟、右鍵及中鍵取消。UI 恰好五筆 reports／一次取消／一次複製、開關 `[false,true]`、一次本機導覽；peer 精確 bytes 通過，結束前 mode epoch 6／無 active lease。

嚴格 surface-probe.json passed、Close acknowledged、全部四個 owner join／listeners released、程序退出 0；runner 同時要求有效報告與完整 cleanup，不以退出碼單獨判定。兩次實際匯出的 960×576 PNG 經獨立解碼全部像素相同。`dist/macos-mouse-final/PTTStella.app` 為 arm64 strict ad-hoc 簽章，與測試副本去簽章 executable SHA-256 均 `2e9b55c185c940daf288ac9ba116a494f2b5259e0aacb13fb26ecd5c9ac61e3b`；證據 `.local/verification/mouse-d-native`。112 個 desktop lib、42 個 bin、51 個 JS、core／desktop all-targets Clippy／docs 通過，最後只改 probe CR expected／提示後的對應 gate 另跑。報告擴增時曾超過 json! 遞迴上限，已將 mouse report 分組，不提高全域 limit；最初失敗 log 保留。

第一次原生程序 native-1 的 peer 誤要求本機點編號送 ESC[C，既有獨立導覽測試的契約是 CR；cleanup false 且缺報告，保留為失敗，退出 0 沒有冒算通過。修正手寫 peer 後 native-2 從頭重跑，未改產品送鍵遷就測試。Shift 拖曳尚未實測（只有 Shift 雙擊選字）；Ctrl＋滾輪的工具輸入嘗試未能保持修飾鍵並形成縮放，仍是 not_ready，不能以鍵盤縮放或 handler 單元代替。原生 blur／lost capture／重連、黑邊與所有 DPR、Windows／Linux、實站、motion／station wheel，以及新候選的前版 C1／46 筆 Vim 原生回歸亦未在本案例結案；D 尚有驗收工作，不標記整體 goal 完成，沒有 push。本輪兩份測試副本與第一份中間 App 已在 exclusive build lease 內核對身分及無存活程序後清除；保留新候選、前版／B2 回復版本、全部失敗／驗證報告與必要 PNG，清理 receipt completed。

**最新滑鼠候選的 Vim／C1 原生回歸已完成，手勢補驗仍待完成**。使用同一 `dist/macos-mouse-final/PTTStella.app`（產品 source 87e3cb6、去簽章 executable `2e9b55c185c940daf288ac9ba116a494f2b5259e0aacb13fb26ecd5c9ac61e3b`）建立兩個獨立程序，沒有修改預期或產品 source。Vim 完成 46 筆操作／53 個實體按鍵，涵蓋主選單、看板／最愛、文章、五次往返、gg／計數無重繪、三種進板提示、新說明頁與空列表；每個鍵後觀察 AX，最後畫面 test46、Normal、無 prefix／pending，peer 核對完整精確 bytes。

C1 依序開啟及複製片段 rows `[8,9]`，兩次 Cmd-C 後在未保存書籤草稿 Cmd-V 均讀回同一完整目的地；兩份草稿關閉且零保存。跨列拖曳選取第 9–10 行，沒有額外外開；兩個圖片片段實際解碼，第二次記憶體快取，HTTP 只有 `/colours.png` 一次。快取預覽放大後的完整 screenshot 確認紅／藍兩半，差分樣式截圖不當作缺像素。PNG 獨立解碼 960×576，與原先 C1 基準逐像素相同。兩份嚴格報告 passed、Close acknowledged、全部四個 owner join／listeners released，程序退出 0；三份副本與候選 strict 簽章、去簽章 hash 相同。證據 `.local/verification/mouse-d-regression`。

另一次只作工具能力診斷的 gestures 程序保留失敗：並行 Ctrl＋F20／滾輪仍 100%，不能證明 Ctrl＋滾輪；並行 Shift＋F20／拖曳雖顯示第 5–6 行選取，peer 在等待本機 CR 時收到不符輸入，cleanup false／runner failed。沒有據此宣稱兩項實體手勢驗收通過，也未修改產品或放寬 expected 來接受輔助功能鍵；已詢問使用者是否方便實體補驗。本輪三份測試副本已在 exclusive lease 內核對退出及身分後移除，保留另開的正常 App 程序、交付 App、所有報告（含失敗）及 PNG；receipt completed。這是最新候選的兩項原生回歸，不代替 D 原生手勢、黑邊／DPR、失焦／lost capture／重連、Windows／Linux 或實站；整體 goal 持續。

**D 原生黑邊／格線外釋放補驗**：新增 `mouse_boundary` 嚴格合成探針，共用既有格線、模式與 production Connection，不改產品 pointer 分流。macOS 全螢幕符合視窗時先點左側黑邊，再從零基 col 4／row 4 拖到黑邊放開；peer 精確收到一次 `ESC[<0;5;5M`，之後僅接受 Close。報告只有左鍵按下、一次取消、零本機導覽／開關／連結，最後 active 為空；PNG 匯出、Close acknowledged、全部 owner join／listeners released、退出 0 與嚴格 runner 均通過。這只證明本次尺寸的黑邊與格線外放開，不算離開視窗、失焦、所有 DPR 或實體修飾鍵手勢驗收。

證據保留 `.local/verification/mouse-d-boundary/native`；探針版本的 112 個 desktop lib、42 個 bin、51 個 JS、core／desktop all-targets Clippy／docs 通過。交付 App 仍為 `dist/macos-mouse-final/PTTStella.app`，沒有以新增測試副本取代。使用者已答覆可補驗，另開只連 loopback 的 `mouse` 畫面等待 Shift 拖曳／複製及 Ctrl＋滾輪結果；尚未收到結果，不先記通過，沒有 push。

本次人工補驗程序 `.local/verification/mouse-d-boundary/manual` 在五分鐘 loopback 期限內未收到手勢結果，連線先逾時，後續正常關窗後程序退出。嚴格 runner 為 failed、cleanup false，沒有 surface-probe.json；保留為未完成，不以退出 0 當通過。原黑邊案例的完整有效報告不受此影響。獨立解碼其 PNG 與前版同場景 960×576 基準全部像素相同；第一個圖片檢查 invocation 因系統 Python 沒有 Pillow 未執行成功，改用既有 bundled Python 後才通過，沒有安裝套件或放寬檢查。

**D2 格式層增量，Session／UI 接續**：依 [XTerm motion／wheel](https://invisible-island.net/xterm/ctlseqs/ctlseqs.html#h2-Mouse-Tracking) 與固定公開站端接收器，新增具名 MotionReport／WheelReport；1002 只編碼有按鈕 motion，1003 可含無按鈕 motion，上／下 wheel 沒有 release。三類格式共用座標與 SGR 修飾碼／容量驗證，沒有擴充既有 Session route、按鈕准入或 UI wire，也沒有換交付 App。新增獨立手寫 bytes 核對三按鈕、無按鈕、四組 motion／wheel 的全部八組修飾碼、零基／一基與 8000-cell 邊界，以及不支援 mode／encoding／越界／未知方向／偽 release 拒絕；原按鈕及 mode trace expected 不改。移動去重／有界合併、交易准入、滾輪分流與真正原生操作仍接續，不能以格式測試宣稱 D2 完成。

本批 115 個 desktop lib、42 個 bin、51 個 JS、core／desktop all-targets Clippy／docs 通過，證據 `.local/verification/mouse-d-motion-format`；沒有實站、新原生操作或其他平台的完成宣稱，也沒有 push。

**D2 移動／垂直 wheel 接線增量（2026-10-18），原生視窗接續**：1000／1002／1003＋1006 的按鈕回報沿用原交易；新增 typed Sample／SampleReport 與 trusted／generation／已繪製來源准入，持有時核對原 lease 與按鈕、無持有時要求當前 stamp 與新 transaction。Rust 去除同格 motion，sample sequence 防重放，motion／wheel 合計每秒 30 筆；按鈕釋放不吃 sample 額度，移動不延長持有期限，取消／人工輸入／重連撤銷。共同 64 筆佇列保留 8 筆餘量，沒有新增送出 worker。被動 motion 共用原 socket／Telnet escape 路徑，但不推進命令 epoch 或建立畫面等待，避免移動剛送出就拒絕同圖下一次點擊；wheel 推進 epoch，只在尚無等待時建立期限。

UI 只有一個可取消的 animation-frame callback，每 34ms 送一個最新 sample，wheel 優先；只保留一筆 motion、一筆 wheel 與不足 40px 的垂直餘量，line／page 正規化，沒有慣性尾巴或斷線後補送。Shift、Ctrl 縮放、已選文字、modal 與放大後本機捲動保留；來源／模式／policy 變化、黑邊及 destroy 清 pending。中英文狀態與說明一起更新。原 `mouse` 負面探針改要求仍不支援的 1001，保留相同精確按鈕／本機 CR 序列，沒有將已支援的 1003 繼續當成負例。

新增四個純 owner 案例、兩個 production Connection loopback 與五個 JS 案例，並擴充重連、滿佇列、typed event／顯示 ACK 的正反例。121 個 desktop lib、42 個 bin、56 個 JS、core／desktop all-targets Clippy／docs 通過，原生 D2 還未執行，交付 App 仍保留前版。首次完整 lib run 有新 motion 案例逾時；原 helper 沒有顯示具體等待哪筆 bytes，檢查發現新 mode wait 可讀到 output_pending 的中間 View，已要求完整來源並改善 timeout 診斷後整套通過，沒有放寬 bytes 或產品准入；原失敗與一次新增測試後漏 fmt 的失敗保留於 `.local/verification/mouse-d-continuous`。後補的重連／重複 Shift 按下邊界另重跑對應測試。沒有實站、其他平台或兩項實體修飾手勢的完成宣稱，沒有 push。

**D2 指定 macOS 無按鈕移動／垂直 wheel 原生通過，held drag 待驗**：新增三個有界 `mouse_drag`／`mouse_motion`／`mouse_wheel` loopback 探針，獨立手寫 SGR 允許座標與必要終格／釋放；原生 cadence 可有有限中間格或 wheel 重複，其他輸入拒絕。UI 記錄最多 64 筆 typed sample，超量明確失敗；peer 另限制網路筆數與完成條件。兩個新 raw synthetic 單元驗證三條正例與「達上限仍缺釋放」拒絕，不把 raw 單元當作原生證據。123 個 desktop lib、42 個 bin、56 個 JS、core／desktop all-targets Clippy／docs 通過；首次 bin 編譯漏 helper 模組限定名稱，修正後通過，原 log 保留。

macOS WKWebView 的 `mouse_motion` 完成 col 4／row 4 左鍵點擊／釋放，再以原生指向 col 10／row 4 的水平滾動移動指標；實際只有一筆無按鈕 motion `ESC[<35;11;5M`，水平 wheel 零站端 bytes。`mouse_wheel` 於 col 4／row 4 收到向下 `ESC[<65;5;5M`、向上 `ESC[<64;5;5M` 各一筆，沒有按鈕邊緣、本機導覽或額外釋放。兩者嚴格 surface／runner passed、final mode 1001／無 held、Close acknowledged、全部 owner join／listeners released、退出 0。兩張實際 PNG 各 960×576，獨立解碼後與前版相同自製場景全部像素相同。

這兩個原生程序及兩個失敗拖曳副本、原始 package 的 strict ad-hoc 驗章與去簽章 executable SHA-256 均為 `8e58cdab8fab8d855b0891c3bebc296f6810a94675dc7bb01f7fddf8e8aea2f0`，來源為接線 commit `6d3dd8a` 加本次探針；證據保留 `.local/verification/mouse-d-samples-native`。原生拖曳第一次 anchor 後收到額外 raw Esc `[1b]`，來源未確認；第二次座標工具在全螢幕與一般視窗回 noWindowsAvailable，只有 AX／截圖可用，沒有輸入 anchor。兩次 runner failed／cleanup false／缺 surface report，退出 0 沒有列為通過，不改 expected 放行。held drag、實體 Shift 拖曳／Ctrl＋滾輪、DPR／失焦等其餘邊界、實站與其他平台仍待驗，整體 goal 持續。

本輪四份 native App 副本及舊 boundary App 已在 exclusive build lease 內核對身分、終止 receipt 與無存活程序後移除；保留本次 package、交付／回復 App、全部成功與失敗報告及必要 PNG，cleanup receipt completed。沒有 push，也沒有將尚未回歸的 D2 package 宣稱為最新交付。

**D2 實際按鈕狀態補強，原生驗證接續**：新增三組獨立 pointer doubles，先在原實作重現漏 up 未取消、外來指標清掉合法 pending，以及 unowned buttons 被編成無按鈕 motion。UI 現在依 W3C buttons 位元核對原交易的左／中／右鍵，缺失時取消並丟棄 pending，不合成 release；先隔離外來 pointer，再處理格線及修飾鍵，未持有只接受 buttons=0。原有 held 正例補上實際按下位元，按鈕／wire expected 不改。三組回歸先失敗、修正後通過；第一輪修正後有一個舊正例未填 held 按鈕狀態，補正 fixture 後通過，兩份失敗 log 保留 `.local/verification/mouse-d-buttons`。這批尚無新的原生結果，前述原生 motion／wheel 的既有 binary 證據保留原範圍。

本批 42 個 desktop bin、59 個 JS（含 16 個 pointer owner／handler 案例）、desktop all-targets Clippy／docs／diff 通過；Rust session 核心未改，後續實際按鈕與新 binary 的原生回歸接續，沒有換交付 App 或 push。

**D2 未持有 sample 的取消接線修正**：`pointercancel` 從只取消 held 改為既有全域 revoke，讓 Rust watch 同時撤銷尚未開始送出的無按鈕 sample，UI 緩衝／callback 仍立即清理。新增實際 handler＋production pointer owner 的整合案例，先觀察缺少撤銷 wire 而失敗，修正後確認 generation 正確的 revoke、舊 queued sample 不再延續與新 pending cell 丟棄；Rust 對 queued sample 的 watch 優先案例沿用同一送出 owner。不是已送 bytes 的回滾，也沒有新增 worker 或 timer。證據保留 `.local/verification/mouse-d-cancel`；42 個 desktop bin、60 個 JS（含 17 個 pointer owner／handler 案例）、desktop all-targets Clippy 與 docs 通過。尚未以此版本重跑原生案例，沒有換交付 App 或 push。

**D2 持有往返探針**：新增獨立 `mouse_round_trip`，要求 col 4→10→4、途中一般重繪及起點釋放，沿用既有 peer／畫面 owner 與 16 筆上限。raw loopback 正向與缺少返回 motion 的負向案例通過；這只驗探針，不代替原生拖曳。非預期 binary 診斷改為長度／Escape 分類，合成敏感文字與超量 binary 的測試確認內容不外露。125 個 desktop lib、42 個 bin、core／兩份 replay、Clippy 通過；後續 pointercancel 修正另通過 60 個 JS。

macOS 的兩次往返嘗試均未完成：第一次觀察 AX 回來時已超過五秒持有；第二次移除中途 AX 查詢，工具仍在持有完成 5513ms 後才完成去程 5846ms／回程 6156ms。嚴格 peer 拒絕未經必要 motion 的起點釋放，runner 均 failed、cleanup 未確認，exit 0 不算通過；held motion 在此 backend 維持 not_ready。第二次關窗後的 AX 查詢意外重開無 probe 參數的副本，已關閉並確認無剩餘程序，未登入或送帳密。保留 `.local/verification/mouse-d-round-trip` 的各次失敗、時序與 package；不改 expected、不以本機合成事件冒充實體持有，也未取代交付 App。

**實體修飾鍵補驗結果**：使用者在上述 `package-2` 的獨立本機 `mouse` 畫面，以 Shift 拖曳文章 121→122、⌘C，回覆兩列完整中文與英文內容；另確認實體 Control＋滾輪可縮放。接手時 AX 顯示選取第 5–6 行與 125%，與回覆相符。但 peer 已因不符合第一筆 anchor press 的輸入失敗，非預期內容／來源未收集，不能據此宣稱零多送或完整 mouse probe 通過；也不能將原因歸咎於使用者手勢或產品。PNG／人工結果／程序退出與 failed cleanup 各自保留在 `native-manual-physical`，該次只證明可見選字／複製／縮放結果。

`package-2` 另以原生 drag 驗新版按鈕檢查：anchor 通過，但關閉時 peer 仍等待必要 motion，runner failed／cleanup 未確認；不重試同一未知步驟或放寬預期。這暴露既有 probe 在 cleanup／peer 錯誤時丟掉已收集互動報告的診斷缺口。後續改為保留報告、強制 status failed、disconnect 與未證實的 join 欄位 null，成功的 acknowledged 路徑維持原判定，互動失敗也不被成功 cleanup 改成 passed。新增兩個負向 metadata 案例，44 個 bin 與 desktop all-targets Clippy 通過；原生報告保存及成功路徑回歸接續，未改正常輸入機制。

**失敗報告保存與共同回歸完成**：`package-3` 刻意只點擊、不拖曳，peer 拒絕缺少必要 motion；surface report 仍保存兩筆左鍵邊緣、零 sample／導覽／取消及 full PNG，同時 status failed、cleanup false、disconnect／join 為 null。runner 維持 failed，負向檢查才以這些獨立預期判定報告機制正確。相同 executable 的無按鈕移動、上下 wheel、46 筆 Vim 與 C1 完整網址四個原生案例均 passed、Close acknowledged、全部 owner join。C1 兩列 open／copy 為 `[8,9]`，原生剪貼簿貼回均為完整 URL，跨行拖曳成立、兩次解碼且 HTTP 只下載一次；Vim 最終 Normal、無 prefix／pending。

獨立 Pillow 解碼確認移動／wheel／負向／人工選取縮放的完整行 PNG 與同一 960×576 基準均零差異，C1 PNG 與既有 C1 基準亦零差異。六個 App 的嚴格 ad-hoc 驗章及去簽章 executable SHA-256 相同，為 `43a8be3a5ef7d4d460c94d1621e72c199d505561f406cdfd0f560a66914a232f`；證據保留 `.local/verification/mouse-d-failure-reports` 與 `mouse-d-final-regression`。比對命令曾因共用磁碟預留總額被 guard 拒絕；確認一個持有程序結束後，以相同 isolated／4 GiB 命令重跑通過，未清他人快取或改政策。這些結果不補成 held motion、人工那次零多送、其他 OS、所有 DPR／IME／睡眠或實站通過，交付 App 尚未取代。

**持有往返的人工嘗試與可觀測性補強**：使用者在 `package-3` 的 `native-physical-held` 回覆已完成；接手時 AX 為選取第 5–6 行／125%，peer 已中斷。新 failure report 保留一筆左鍵按下、一次取消／複製、零 motion／導覽，但 peer 拒絕第一個 anchor；因舊記錄只有按鈕／動作，不能判斷座標或修飾鍵，也不推定使用者重複了其他手勢。該次仍 failed、cleanup false、exit 0，不算往返通過。接續將唯一有界記錄改存完整 Report，從中衍生原配對及新增 edges 診斷；不記原始鍵入文字、不改准入與送出機制。往返 probe 的同兩格改以黃色 A／B 明示，精確 peer expected 不變，完成後仍還原原 fixture；補獨立標記 cell／樣式及失敗報告保留座標／修飾鍵案例。125 個 lib、45 個 bin、desktop all-targets Clippy 通過，證據保留 `.local/verification/mouse-d-anchors`；新原生補驗接續。

**完整 edge 診斷的原生驗證與清理完成**：`package-4` 的 `mouse`／`mouse_boundary` 兩個嚴格案例通過。前者保留 col 4／row 4 的左／右按下放開、中鍵按下，所有修飾鍵為 false；未支援 1001 的點擊／輪輸入零多送，Shift 雙擊選行後從 OS 剪貼簿貼回原中文行，停用／重新啟用各一次，本機導覽一次，中鍵模式撤銷取消一次且無釋放。後者先點黑邊，再拖起點至格線外，只有同格一次按下／一次取消，沒有釋放、sample 或本機導覽。兩次皆 Close acknowledged、全部 owner join、無 held，獨立解碼 PNG 與 960×576 基準零差異。Shift 雙擊不能代替實體 Shift 拖曳，這兩個案例亦不代替 held motion。

A／B `native-anchors-held` 的五分鐘 peer 期限到達前未收到任何 edges／samples，未收到該題的完成回覆；已收回副本並保存 failed report／cleanup，不能當成手勢失敗或通過。後續需要可用的實體輸入或能維持按住移動的 native backend；不自動重開人工測試。Windows 的新版 WebView2 驗收與其他明列原生邊界仍缺證據，尚未整批結案。

比對原命令的 isolated／4 GiB 預留曾再次被總額 guard 拒絕。本次是只讀 App 與記憶體解碼，量測最大 executable 為 30,828,848 bytes，唯一暫存檔逐次覆寫且加入 64 MiB 輸入上限；因此重新審核該比對的獨立預留為 1 GiB，仍經同一 guard、100 GiB free floor／60 GiB 總額，沒有調低編譯預留或改政策。相同檢查通過、17 份 App 的嚴格簽章／binary manifests 保存在 `.local/verification/mouse-d-anchors/summary.json`。其後在同一獨佔 lease 內重新核對路徑、身分、binary 雜湊與所有 probe 的終止紀錄，移除 16 份本輪副本／中間包；只保留 `package-4` 測試包，所有 dist 交付、來源、報告／失敗與 PNG、shared target／cache 未動。收據在同目錄 `cleanup-apps.json`；舊 package／副本路徑已清除，不應重跑依賴它們的舊 inspector。沒有 push 或取代既有交付 App。






## 維護性重構

2026-10-17 傳統 BBS 字型：依公開的 PTT 官方終端預設及 Windows 客戶端資源確認 MingLiU／細明體是傳統選項，並非所有客戶端有唯一預設。新增 `windows_classic`（MingLiU／MingLiU）與 `macos_bbs`（Songti TC／Songti TC）；既有 Windows 黑體組合、macOS 黑體／宋體與平台初始值保留。新組合均明確選取，前者缺少時顯示兩角色載入警告及官方取得／授權頁，後者只稱本機宋體替代，不宣稱等同細明體。沒有加入第三方字型檔。

27 個 bin、45 個 desktop lib、23 個 JS、4 個 profile 案例、fmt／Clippy、core 八套 suite／兩個 replay 及 docs 通過。macOS 原生中文程序確認缺 MingLiU 的警告與官方 sink，改選 `macos_bbs` 後英數／中文皆為 Songti TC 並保存；同畫面英數區差 910 pixels、中文區差 2,025，方塊色彩區差零。新英文程序讀回相同 preset，960×576 PNG 與保存前逐像素一致；英文預覽缺 MingLiU 同樣提示，取消保持原檔。兩程序均 acknowledged、worker／listener 清理完成，本機 peer 未收到站端輸入。

交付為 `dist/macos-fonts-delivery/PTTStella.app`（arm64、ad-hoc strict 驗章通過），去除各自簽章後與原生實測 executable 的 SHA-256 相同。報告／PNG 位於 `.local/verification/fonts`。本輪沒有安裝字型、實站登入或跨 OS 像素對照；完整 Mac desktop backend 仍 not_ready，Windows／Linux 字型列舉與顯示需各自實測。已依功能分兩筆提交，沒有 push 或公開發佈。

2026-10-17 字型選擇：新增英數／中文的本機家族選單，font-kit 0.14.3 透過 Files blocking pool 列出名稱、排序去重並限制 4096／256 bytes；清單與載入分開，未知／缺字型不改寫設定。重新讀取保留草稿，晚到或關閉後清單忽略。所選字型有固定 enum 的官方下載／授權／安裝入口，僅 trusted 明確點擊才開 OS 瀏覽器；一般 probe 使用 sink，不下載、安裝或重新散布字型。未知自訂字型向原發行者查詢。

同一 font owner 供預覽／畫面／IME／PNG；雙欄文字先用所選 CJK face，再用所選英數 face，最後 OS monospace，修正缺 CJK 時符號直接落到 OS 的路徑。27 個 bin、45 個 desktop lib、22 個 JS、fmt／Clippy、core 八套 suite／兩份 replay 與 docs 通過。macOS WKWebView 列出 322 個本機家族，可選 Times New Roman／Songti TC，取消後設定與 PNG 原封不動；明確保存後新英文程序讀回相同名稱與相同 960×576 pixels。缺字型警告、重新讀取與中英文官方入口均實際操作。刻意缺 CJK 時 Cascadia／Menlo 的英數區差 768 pixels、圓形符號區差 366，方塊色彩區差零，雙色背景保持藍／紫。兩程序均 Close acknowledged、所有 owner join，沒有站端 bytes。

原生回饋修正內建 face 不在 OS 清單時的空白選項，並將預覽移近字型欄位；修正後英文程序確認選擇提示與預覽。報告／PNG 保留 `.local/verification/fonts`；OS 外開用 sink 驗證目的地，沒有宣稱瀏覽器實際啟動、下載、逐字 glyph 來源或 Windows／Linux 實機通過。完整 Mac desktop backend 維持 not_ready。使用者另要求研究傳統 Windows PTT 字型，接續獨立提交；本批不改既有 preset。

2026-10-17 i18n 基礎：使用者授權採用 Fluent，並要求依功能分批提交。第一批建立繁體中文／英文共用資源、Rust i18n-embed 載入器、fl! key／參數編譯檢查與官方 JS runtime 的 bootstrap helper；尚未串接產品 UI，沒有宣稱中英文視窗已完成。以僅含本批 staged 內容的獨立來源快照驗證：兩個 Rust catalog 測試、兩個 JS 測試、非 desktop 全 targets Clippy 及 docs 通過。兩語 key／參數由官方 Fluent AST 比較，另驗證插值、複數及獨立 loader。語言偏好／UI 串接、核心狀態／錯誤文案及原生回歸繼續進行。

第二批完成 UI／偏好串接：工具列、各對話框、媒體／字型動態提示、原生選單與選檔標題共用翻譯；Localization 為獨立的啟動封包，FTL 不拼入 JavaScript。設定 version 3 保存 language，舊 v1／v2 載入不改檔案，明確保存才備份及遷移；取消／保存失敗不改生效偏好，切換下次啟動套用。僅含本批 staged 內容的獨立來源快照通過 25 個 bin、45 個 desktop lib、16 個 JS、desktop 全 targets Clippy 與 docs。核心狀態／錯誤尚在下一批，原生兩語／重啟與整合回歸仍待完成，不以這次單元檢查宣稱視窗驗收。

第三批將 Connection 狀態、登入／導覽／Vim notice、媒體／輸入／儲存等使用者可見錯誤改為具名 Fluent 訊息。需要擁有翻譯結果的錯誤／notice 由靜態字串改為 String；命令、bytes、辨識文字及既有 fixture／expected 未改。整合來源的 45 個 desktop lib、25 個 bin、desktop 全 targets Clippy 與 core 八套 suite／兩份 replay 通過，並完成候選 arm64 App 打包；原生兩語／重啟、PNG 與 Vim 回歸接續執行。CLI 探針診斷與第三方／OS 錯誤細節不當作可翻譯的站端文字。

本輪整合驗證：18 個 JS 測試通過，包含真正生產媒體控制器配合 DOM／network doubles 的中英文圖片回饋、影片事件、失敗與清理。macOS WKWebView 中文程序確認取消英文草稿不寫檔，明確保存才將 v2 備份並遷移成 v3／en-US；當前 UI 保留中文並提示下次套用。另一程序讀回相同保存 bytes，工具列、設定、合成鑰匙圈登入／取消、原生選單與 PNG 面板標題均為英文。兩程序實際保存的 960×576 PTT PNG 逐像素相同，沒有站端輸入。

英文 window probe 以 80／100ms 短按、轉頁後觀察畫面的節奏完成精確 40 筆 Vim 操作，包含五次切板、無重繪 gg／4j、三次進板圖及長按 l 一秒，保持 NORMAL、無額外 bytes。另三個嘗試分別使用 shift+g 或快速批次呼叫，在 step 3／22／8 未完成預期序列，保留為失敗、不計入通過，也不宣稱極速連發已驗證。英文 filters probe 完成四次保存、兩次揭露、原生複製事件、整頁／行 PNG 及翻頁往返，英文複數與行範圍可見。Pillow 核對遮黑區外像素差零、揭露等於原圖、選取行等於整頁，且返回圖與 i18n 前基準完全相同；上述四個通過程序均正常清理與 Close acknowledged。

原生英文媒體案例因 CUA 座標點擊持續回報 noWindowsAvailable，提升視窗後仍無法操作，HTTP 請求為零，整合報告保留 failed／互動能力 not_ready；沒有以 DOM doubles 宣稱原生解碼／播放通過。完整 Mac desktop backend 亦保留 not_ready，未重驗 Windows／Linux、原生 IME／codec、實站登入與 OS 外開。最終 `dist/macos-i18n-delivery/PTTStella.app` 與實測候選 executable 的 SHA-256 相同、arm64／ad-hoc strict 驗章通過。功能分成三個提交，另以測試／驗證提交收尾；報告與像素保留於 `.local/verification/i18n`，未 push 或公開發佈。

2026-10-17 第三輪：使用者授權將檔案操作移出 UI 事件迴圈，並集中 Rust 畫面與互動狀態。Files 持有單一可追蹤工作，設定／名單載入與保存、設定檔處理及 PNG 解碼／寫入交給 blocking pool；原生 picker 仍由 UI thread 擁有。忙碌拒絕不啟動 factory、不另加佇列；保存成功才更新生效設定／規則。關閉撤銷尚未開始工作，已開始的工作保留到 join；五秒未完成顯示等待，真實失敗併入清理結果。SurfaceState 統一持有待 ACK／已顯示畫面、遮黑、選取、hover 與匯出版本，來源或策略變更由同一處撤銷互動。

獨立複查找到 P2：非同步選檔期間的 PNG 可能失去版本約束。已加入可撤銷 ExportLease 及直接觀察最新 Connection view 的檢查，在解碼後、選檔後及開始寫入前驗證；選取 PNG 另綁 input epoch。JS 清除／重選會發出有作用域的 SelectionCleared，Rust 只撤銷相符選取。八種延期選檔案例覆蓋規則、揭露、frame、session、選取、清除、輸入及顯示設定變更：舊 PNG 均不寫入，後續新請求仍實際保存。Agent 再次唯讀複查確認 P2 可關閉，未發現其他具體阻擋問題；這是程式與證據審閱，沒有代替另一人執行 GUI 或人工驗收。

24 個 bin、42 個 desktop lib、12 個 JS、fmt／Clippy 與 core 八套 suite／兩份 replay 通過。新增合成 fixture 有獨立來源說明，既有 expected 未改。macOS WKWebView 的最終產品程式完成真正原生 PNG Save、設定檔 Save／Open、匯出生效 20／20 與匯入只預覽；picker 前聚焦父視窗，修正背景啟動時 Save 無法使用。名單四次保存、兩次揭露、複製事件、兩種 PNG 及翻頁往返通過。獨立 Pillow 解碼確認 6,123 個變動像素全在預定遮黑區、區外差零，揭露等於原圖、整頁選取行等於整頁、返回後恢復遮黑，且與重構前保留圖逐像素一致。另完成 40 筆原生 Vim 精確 bytes 比對。

真 blocking 慢磁碟工作期間可開啟／取消設定，第二筆保存明確拒絕；⌘Q 後 UI 顯示清理等待，runner release 後才 join／Close acknowledged。曾因操作等待超過 120 秒造成一次合成案例逾時，失敗紀錄保留，另一完整程序重跑通過。原生 picker 開啟時 macOS 攔住 ⌘Q，取消後再次 ⌘Q 正常結束；此案沒有送達 pending-picker shutdown，不將它算成該契約的原生證據，延期 chooser 單元測試另驗證關閉後不啟動寫入。各通過程序均清理完成、退出碼 0；一般測試只使用本機合成服務。

本輪交付包為 `dist/macos-file-state-delivery/PTTStella.app`（arm64／ad-hoc），報告與像素證據保留在 `.local/verification/file-state`。早期 dialogs 僅完成設定／PNG，媒體操作未完成，該整合報告保留為失敗而不計入通過。完整 Mac desktop backend 仍 not_ready；Windows／Linux、完整原生 IME／codec、實站登入與 OS 外開未在本輪重驗，沒有存取使用者帳密、push 或公開發佈。

2026-10-17 第二輪 UI 整理：使用者授權整理共同作用域及 Rust→JS 封包。登入、名單與字型設定改由各自 JS owner 持有欄位／草稿／busy／timer／request；主 surface 只接線、分派及協調同一畫面的輸入／選取／繪製。Rust outbound Packet、具型別 payload／status 與薄 Bridge 取代一般送出路徑的臨時 JSON，統一 `{kind,payload}`，未知封包不再落入 paint。設定載入後再次檢查 epoch／request，取消或停止後不重畫、不搶焦點；原有單一送鍵、准入及共同清理不另建第二條路徑。

本輪 12 個 bin、42 個 desktop lib、11 個 JS、fmt／Clippy 與 core 八套 suite／兩份 replay 通過。新增 wire fixture 是獨立手寫 expected；既有核心 fixture／expected 未改。候選包完成 11 個 macOS WKWebView 程序：前輪九種生命週期／Vim／登入情境，加上完整名單與 UI／媒體整合。設定草稿取消保留設定及 PNG bytes；匯出生效 20／20、匯入只預覽，保存 18／19 後 PNG 有 34,503 pixels 改變；整頁選取行等於整頁 PNG。本機 PNG 紅／藍圖像可見，壞圖解碼失敗後重試成功，HTTP 分別一次正常圖、兩次壞圖，外開使用 probe sink。名單遮黑外像素差零、揭露等於原圖、翻頁返回恢復遮黑，且與重構前保留圖逐像素一致。40 筆 Vim 與合成按鈕登入維持原結果；均完成清理與 acknowledged，沒有連 PTT。

候選測試曾發現名單關閉後瀏覽控制項未立即恢復；修改三個 dialog 的共用 changed 回呼，同時刷新 navigation／export。`dist/macos-ui-contract-verified/PTTStella.app` 重新跑單元／Clippy／JS，原生逐一關閉／取消名單、字型與登入，AX 斷言確認不需額外操作便恢復瀏覽；再完成四次 CRUD、兩次揭露、複製事件、兩種 PNG 及翻頁往返，PNG 與候選／前輪相同。

原 Agent 獨立複查確認 owner／typed outbound 改善成立，未發現阻擋問題；另找到 P3：lifetime 的接收 callback 直接讀取 null 封包會拋出例外，留下未完成 waiter。已在 lifetime 與主迴圈以 optional access 將 malformed 值交回 router 拒絕；新增帶 timeout 的 lifetime→router 測試，核對 null／undefined 拒絕、零 handler 呼叫、後續 notice／stop 與 listener 釋放。Agent 再核對後確認原 P3 可關閉。複查包含程式、測試與來源／binary hash 核對，並非另一人重跑 GUI 或人工產品驗收。

交付包為 `dist/macos-ui-contract-delivery/PTTStella.app`（arm64／ad-hoc），前版保留。P3 修正重跑 12 bin／42 lib／Clippy／11 JS，最後主迴圈 optional access 再跑 JS、重新打包，並以最終 App 執行原生 painted 錯誤退出／⌘Q，確認共同清理完成、Close acknowledged 與退出碼 0。其餘候選 11 案與三個 dialog／完整名單針對性回歸沒有在這兩處 optional access 後整批重跑，證據範圍分開保留。完整 Mac desktop backend 仍 not_ready，Windows／Linux、完整原生 IME／codec、OS 外開及本輪實站行為未驗證；沒有 push 或公開發佈。

2026-10-17 獨立架構審查後續：使用者授權優先修正共同清理、命令等待不阻塞 UI、統一輸入准入。實作以 Resources／Pending 與 protocol admission 三個具名責任為界，補上錯誤退出、慢回覆、取消排隊請求、不可信貼上／確認、組字例外及清理逾時的案例。正常／錯誤返回都停止並 join renderer、Connection 與 credential owner；超時保留等待、顯示未完成狀態。Connection 回覆失敗仍 join，並保留失敗供重複 shutdown 回報；錯誤畫面也可 ⌘W／⌘Q 結束。貼上在改動預覽前檢查信任，確認通過准入後才取走預覽。

複查先發現兩個新增 P2：共同 pending stop Promise 累積 reaction，以及 lazy 命令 future 讓後到原始按鍵搶先入列。已改為單一可替換 stop waiter；Pending 確認空槽後同步取得已入列的 Connection Reply，只把等待回覆留給 select。新增 WebSocket 測試不 poll browse reply 仍比對先 ArrowDown、後 Enter，busy 測試確認拒絕不執行入列 factory。兩萬筆封包測試確認未完成 stop reaction 不累積；替換回舊 helper 的獨立 mutant 在第 1,000 筆被相同測試拒絕。原 Agent 再次唯讀複查後，確認原三項與兩項 P2 均可關閉、沒有其他有充分依據的阻擋問題；另親自核對 54 個來源檔與產物雜湊相符。這是獨立程式／證據審閱，並非另一人重跑 GUI 或人工產品驗收。

最終 fmt／desktop 全 targets Clippy、core 八套 suite／兩份 replay、42 個 desktop lib、10 個 bin、4 個 JS 測試通過，原 fixture／expected 未改。macOS WKWebView 重新執行九個獨立程序：四種錯誤退出、三筆不可信貼上拒絕、真實多行貼上後拒絕合成確認並保留預覽、60 秒合成命令等待期間開啟／取消字型設定及關窗、40 筆 Vim 操作、按鈕登入合成主選單。九者皆完成清理與 Close acknowledged，退出碼 0；本機負面 peer 不接受任何站端輸入。slow 只證明合成 future 等待期間 UI 仍可運作；send 為實際 Eval API 序列化失敗，資料通道失敗的 stop 喚醒另由 fake-channel JS 測試驗證，沒有宣稱真實網路背壓。

最終 App 為 `dist/macos-boundaries-reviewed/PTTStella.app`（arm64／ad-hoc），前版保留。完整 Mac desktop backend 仍 not_ready；未重跑 Windows／Linux、完整原生 IME／媒體或實站登入，本輪沒有存取使用者帳密、push 或公開發佈。其餘 UI 共同作用域與 Rust→JS 封包型別化留待後續獨立驗收。

2026-10-17：使用者要求檢查與改善可讀性、維護性及擴充性。本輪優先整理連線 worker 的狀態轉移、UI bridge／畫面投影與設定所有權；不新增通用框架、後端或產品功能。驗收為原有輸入 bytes、generation／revision／input epoch、防閒置、登入取消／期限、遮黑及關閉清理行為保持一致，既有 expected 不因重構更改。

重構前 core 與 39 個 desktop lib／2 個 bin 測試通過。第一步將 WSS 連線／關閉／有界傳送移至 connection/transport，單一 session 狀態與登入、導覽、保活、畫面發布移至 connection/worker。事件迴圈只負責選擇事件；手動輸入仍只有一條 epoch／escape／送出路徑，等待狀態及登入期限各有共同擁有者。原本巨型 select 的壓縮分支改為可由 rustfmt 維護的具名方法。此步驟 core、39 個 desktop lib／2 個 bin、desktop 全 targets Clippy 與 docs gate 通過；既有 expected 未改。

UI 接著拆為 app／protocol／presentation／preferences／probe／surface：畫面、事件格式、投影與版本檢查、設定所有權及驗收計數各有位置。位置布林 tuple 改為具名 Options；設定預覽／匯入只產生封包，成功保存才更新生效設定及 resolved display，失敗仍保留舊值與來源。新增兩個狀態契約測試，核對預覽／匯入／匯出、備份失敗與錯誤載入的保留行為；沒有新增依賴、改動 JS wire keys 或修改既有 fixture／expected。模組及後續修改入口見 [架構](../architecture.md)。

最終 core（八套 suite／兩個 replay）、39 個 desktop lib、4 個 bin、desktop 全 targets Clippy 與 docs gate 通過。macOS／WKWebView 的獨立 window probe 完成 40 筆原生 Vim 操作，含無重繪連按、長按通過進板圖及六位數列表；設定草稿取消後設定檔與 PNG bytes 不變，明確保存後 18／19 的字高／基線同時寫入檔案與畫面。filter probe 通過四次 CRUD、四次揭露切換、複製事件、兩種 PNG 與 Vim 翻頁；獨立 Pillow 解碼確認遮黑外像素差為零、揭露等於原圖、整頁選取行 PNG 等於整頁、翻頁返回恢復遮黑，且與重構前保留的相同合成案例逐像素一致。合成 login probe 逐筆比對一次帳號／密碼、碎片化提示及無額外輸入，已到達合成主選單。三個程序分別 ⌘W／⌘Q 結束，均 acknowledged／listener released／worker joined；登入的 credential worker 亦 join。

重構 App 保留於 `dist/macos-refactor/PTTStella.app`（arm64／ad-hoc），舊包保留。一般測試與本輪視窗均使用本機合成服務，沒有重新登入 PTT 或碰觸使用者帳密。完整 Mac desktop backend 仍 not_ready；未重跑 Windows／Linux 實機、原生 IME 與媒體完整 gate。沒有效能改善宣稱、push 或公開發佈。

## 增量：進板圖與六位數文章列表的 Vim 操作

2026-10-17：使用者回報 C_Chat 進板圖停在「請按任意鍵繼續」，原先未知 profile 使 Vim 暫停。新增獨立 ContinuePrompt／Continue，要求完整格線、底列提示／色帶與最後一欄游標；只送一個空白。Normal 可用空白鍵、Enter、l／右方向鍵通過並維持 Vim；按住的繼續鍵跨畫面仍持有到 keyup，不誤開下一個項目。圖案區未映射字不參與決策，提示區仍須可映射；確認、輸入及其他未知畫面不放寬。沒有自動跳圖、跨頁補送、滑鼠導覽或保活。

第一候選包實站已通過登入後兩個標準提示與 C_Chat 進板圖；長按 l 一秒未開文章。但隨後列表仍暫停：一般單格 > 緊貼六位數編號時，舊判斷把兩格的「>3」當成無效指標。改由共用 pointer 依單格 >／雙格 ● 各自寬度確認，保留其餘欄位、游標與畫面條件；新增自製密接編號正反例。

29 個 navigation、9 個 Vim 案例及其餘 core suite／兩個 replay 通過；39 個 desktop lib、2 個 bin 測試及 desktop 全 targets Clippy 通過。負面案例核對正文假提示、額外密碼／y/N 文字、未完成控制碼、提示區未映射、長按跨 profile、重複 pending、舊 revision／input epoch 與新頁面的無效 Continue，沒有額外 bytes。

最終 macOS／WKWebView window probe passed：40 筆原生操作包含既有四種列表、五次切板、無更新的 Home／4j／進入、三張自製進板圖的空白鍵／Enter／l，以及其後單格指標密接六位數編號的 j／k／h。l 長按一秒後仍停在文章列表，沒有額外開啟；始終 NORMAL。⌘W 後 acknowledged、listener released、worker joined，peer 無額外輸入。前一候選包另有 37 筆 passed；該輪中 CUA 因使用者切換視窗暫停，核對 test10 後續跑完成，沒有重送；最終 40 筆為另一個完整程序。

產物為 `dist/macos-board-entry-final/PTTStella.app`，arm64／ad-hoc 簽章有效，舊 App 保留。最終包在使用者於正常登入視窗完成登入後，實站確認 C_Chat 另一張進板圖可用長按 l 一秒通過；緊貼六位數編號的 > 指標正確辨識，k／j 移動、l 開啟一般文章、h 返回列表並再 h 返回最愛均為 NORMAL，未連開文章。⌘Q 後 owner 回報 PeerClosedWithoutReply、程序退出碼 0；未保存實站圖案／流量或取出密碼到工具。完整 Mac desktop backend 仍 not_ready，未執行本輪 Windows／Linux 實機驗收，沒有 push／公開發佈。

## 本階段：按需媒體預覽

使用者已授權寫規格後實作。範圍為圖片／動圖、直接影片網址、YouTube 縮圖與外開；一次預覽一個媒體，不提前載入整篇文章。

| 工作 | 完成條件 | 狀態 |
| --- | --- | --- |
| M1 URL 與載入 owner | Rust 分類、取消、錯誤、有界記憶體快取，本機 HTTP 正反例 | 已完成；2 個 URL 核心案例、4 個 loader 案例，包含取消的 TCP EOF 與 body 總期限 |
| M2 圖片／動圖預覽 | 暗底 dialog、等比縮放、播放／停止回首幀、原網址／外開、焦點與清理 | 已完成；Windows 真實 PNG／JPEG／WebP 像素、GIF 紅／藍幀及停止、快取、解碼失敗重抓、關閉焦點通過 |
| M3 影片與 YouTube | 直接影片手動播放，縮圖不擋外開，失敗可恢復 | 已完成；Windows WebM trusted 播放及首幀、Esc 關閉通過；YouTube 用本機縮圖及外開 sink 驗證，未連網站 |
| M4 驗證與交付 | 核心／HTTP／真實 Dioxus gate、現況文件、功能 commit 與 push | 本機完整 gate 已通過；以本階段功能提交交付，既有輸入／PNG／設定重啟／清理亦通過 |

本階段不含第三方網站內嵌播放器、圖床 HTML 擷取、GIFV 猜測轉換、progressive 圖片解碼或磁碟快取。影片 codec、實際網站及其他 OS 的結果另列驗證邊界。

2026-10-16 媒體驗證：Windows WebView2、Rust 1.97.0、Dioxus 0.7.10；`python tooling/check.py all` 及增量 desktop gate 通過，沒有實站媒體請求。媒體預覽與原 BBS canvas／input epoch 隔離；關閉取消 HTTP、撤銷 Blob URL、停止 video，錯誤不改變 BBS 連線狀態。GIF 實際畫面以獨立 System.Drawing 解碼，WebM 以 presented-frame callback／播放時間／尺寸確認；不以 DOM 元素存在當成播放成功。

本機 64×32 PNG 的一次量測：冷開約 44ms、記憶體命中約 33ms、關閉與 HTTP 取消約 12ms，包含 CDP 輸入／輪詢成本。這是新功能自身的合成工作量，沒有等價舊功能基線；不是實站速度或提升倍數。CPU／process-family peak memory 未量測。MP4／OGV、動畫 WebP／PNG、實際 YouTube／圖床、OS 外開、macOS／Linux 尚未驗收。

## 本次修正：中文組字與 Enter

2026-10-16 macOS 中文輸入回報：使用者觀察到獨立組字框，確認後多送 Enter。修正前輸入橋接只檢查 `isComposing`，未處理 legacy WebKit 在 `compositionend` 後送出 `keyCode=229` 的確認鍵；[WebKit 事件順序紀錄](https://bugs.webkit.org/show_bug.cgi?id=165004) 及 [macOS Web App 回報](https://bugs.webkit.org/show_bug.cgi?id=324380) 支持此相容性假設，但尚未擷取使用者的實際事件。

修正於同一輸入邊界排除 IME 按鍵、讓 keydown 唯一管理 Enter，忽略 textarea 的換行編輯；不使用組字結束後的時間窗或吃掉下一次普通 Enter。組字宿主改成游標位置、沿用當格 ANSI 配色的無框單行，寬度依內容與剩餘行寬調整；保留原生選字，確認前的內容不送站端。

Windows 真實 WebView 與模擬舊 WebKit 順序的增量 gate 已通過，包含確認後立即按普通 Enter 的精確 bytes、取消、相同字接續輸入、無框宿主及既有互動回歸；macOS 原生注音／拼音、取消、連續中文與下一次普通 Enter 仍需實機驗收。

## Vim 瀏覽模式

2026-10-16 實作增量：Rust 單一鍵盤策略接管模式、前綴／計數、原生一次與編輯歸屬，Connection 仍序列化送鍵。Windows WebView＋本機精確 bytes 驗證主要瀏覽、原生一次按鍵／Ctrl+d／中文、i/Esc；既有 IME、貼上、選取、PNG、媒體及設定重啟 gate 通過。核心計數／repeat／未知／邊界案例通過。手動 pending 加入回應期限與未確認訊息，空操作不送 bytes。實站及 macOS／Linux 尚待驗收。

以下為最初研究紀錄：

2026-10-16：已依使用者選定的瀏覽範圍，完成 [按鍵衝突與實作方案](../vim-browsing.md)。內容包含模式切換、hjkl、gg/G、計數、Ctrl+d/u/f/b/e/y，以及後續補充的「原生一次」按鈕、f 快捷入口與持續原生輸入。公開站端來源可確認 h／g／G／Ctrl+d/u 等衝突；閱讀 j/k 與方向鍵的邊界行為不同。採本機 Rust 指令解析與既有 Connection owner，分 V1 模式、V2 捲動及 V3 跨平台驗證；本輪只交付研究文件，未改產品按鍵行為。來源副本與 provenance 保存在 repo 外的功能研究目錄，文件 gate（含 19 個 harness 回歸案例）及 diff 檢查已通過；實站與原生 Vim 模式均未驗收。

## 後續建議研究：hover、名單與設定檔

2026-10-16：本輪完成來源／架構研究及待實作規格；沒有改產品互動或套用私人設定。

| 建議 | 決定與入口 | 現況 |
| --- | --- | --- |
| 圖片小預覽 | [hover 方案](../media.md#圖片-hover-小預覽)：跟隨游標／右上固定可切換，單一 request、首幀、點擊升級、不搶焦點 | 已實作；Windows pointer／HTTP、取消、定位與提升驗證通過；其他平台待驗收 |
| 本機黑白名單 | [名單方案](../content-filters.md)：完整 ID、標題字面條件、備註、例外與可揭露遮罩；共享辨識及輸出，保留固定格線 | 一般推文遮黑已實作並完成有限 Mac 原生及實站驗證；封鎖消失、白名單／關鍵字與舊格式匯入仍待設計／驗證 |
| 預設組合與自訂檔 | [設定方案](../preferences-and-profiles.md)：任何平台均可選 Windows／Mac／Linux 組合，自訂另存、載入與匯出／匯入，保留明確選擇 | 已實作選擇意圖、具名可攜檔與 v1 遷移；Windows 保存／重啟像素通過 |

使用者已指定名單分一般遮黑與移除推文兩級，本輪不實作名單；詳見名單方案。先完成設定意圖與保存邊界，再實作 hover 呈現；名單須先補 Rust 的作者／推文範圍與共享呈現結果。Vim、設定檔及 hover 的實作與驗收分列如下；名單本輪保持設計階段。

## 可攜顯示設定檔實作

2026-10-16：已接入 Fonts 選擇意圖、version 2 AppSettings、具名 .pttstella.json、目前生效設定匯出與匯入草稿。任何平台均可選固定 Windows／macOS／Linux，手動字型改動才成為 Custom，切換組合保留字高／基線。舊 v1 不自動覆寫，明確保存時備份原始 bytes。

核心 3 個契約案例、Store 遷移／備份／失敗與 Windows 真實表單／檔案往返通過，包含兩個具名組合、固定 preset 識別、匯入預覽／取消／套用、損壞／超量／未知版本保留原檔，並由第二個 app 程序確認設定與逐像素重啟一致。既有輸入／媒體／PNG／Vim 回歸通過；probe 僅替代原生選檔路徑，原生 picker、macOS／Linux 及逐字 fallback 未實測。

## 圖片 hover 小預覽實作

2026-10-16：已實作 350ms dwell、右上／跟隨游標／關閉及本機保存；單一 Preview、HTTP future、decoder ACK 與 LRU 共用，hover 到完整預覽只提升呈現、不重開下載。hover 不取得焦點、攔截滑鼠、播放影片或更動 ANSI 格線；圖片首幀等比縮小，邊緣轉向並限制在 viewport。

Windows 真實 pointer＋loopback HTTP 已驗證快掠零請求、同連結一次、A→B 的 TCP EOF、已完成及下載中提升不重抓、兩種位置／右下碰邊、關閉偏好、設定重啟及 Canvas／PTT bytes 不變。Rust lease 案例驗證相同來源跨 revision 可續用，新輸入／session／範圍變動／未完成畫面均失效。現有全部離線 gate 通過；真實網站、macOS／Linux、原生 picker 與人工登入仍待驗收。

## 一般黑名單交付

2026-10-16 使用者授權實作第一級：標準推文完整 ID 比對、名單新增／修改／移除與備註、獨立本機保存、遮黑與暫時顯示，以及複製／PNG／連結的一致處理。驗收目標：相似 ID 與引用文字保留；來源 Frame、站端行列與送鍵不變；保存失敗不改生效規則；同一來源 revision 的規則／揭露變更也使舊選取、匯出與媒體授權失效。封鎖後移除推文、白名單與關鍵字仍是後續設計，不在本輪實作。狀態：第一級已完成。5 個 filters 核心、core 八套 suite／兩個 replay、38 個 desktop lib、2 個 bin、全 targets desktop Clippy 與 docs 通過。

macOS／WKWebView 完成名單 CRUD／備註、非法 ID、保存失敗保留原規則、未儲存草稿、新程序重載、系統剪貼簿、遮黑／揭露／翻頁後重設及兩種 PNG。獨立解碼確認命中內容區全黑、其餘像素不變、揭露與原圖一致、選取行等於整頁裁切。最終 filter probe 為 passed，僅兩筆翻頁與 Close，⌘Q 後 ACK／worker／listener 清理；相同 executable 的 34 筆既有 Vim 回歸及 ⌘W 通過。一次中途點到揭露網址使 probe 中止，不算通過；重跑完成。座標滑鼠曾受 CUA 限制，完整流程改用工具列選取，前一候選包的單行選取子案例另已驗證。完整範圍見 [過濾契約](../content-filters.md)。

正常 App 實站 SYSOP 公開公告已確認兩則同作者標準推文遮黑、作者／時間保留、揭露及 Ctrl+b／Ctrl+f 往返恢復遮黑，Vim NORMAL；測後暫時規則與本次新建的檔案／備份已移除，還原原先不存在的名單狀態。⌘Q 回報站端結束連線但未回覆 Close，退出碼 0。未保存實站內容／流量或帳密。產物為 `dist/macos-filters-final/PTTStella.app`，內建字型、arm64／ad-hoc 簽章；舊包保留。完整 Mac desktop backend 仍 not_ready，沒有本輪 Windows／Linux 結果；沒有 push／公開發佈。

## 技術邊界

### 專用帳號實站切板與文章列邊界

2026-10-16：使用者自行登入後，明確授權以專用測試帳號測試切板與 Vim。macOS 原生事件已在實站確認主選單 k／l、最愛 G／gg／4j／l、SYSOP 文章列表 k／l、文章閱讀與 h 返回、再 h 回最愛及 j／l 切換看板。單頁閱讀 Ctrl+f 維持本機邊界拒絕；進板動畫與任意鍵提示維持保守暫停，可用 f＋Space 原生一次通過。沒有發文、推文、寄信、刪文、修改站端設定或記錄原始 session／帳密。

同時在 Test 發現合法空白標題、ID_Problem 發現置底日期欄「重要」，兩者均使舊辨識器暫停。先以各一個自製案例在舊 reducer 重現 Unknown，再修正共用固定欄位解析：標題可只剩已知文章標記，編號／作者與列表錨點仍驗證；「重要」只允許在星號置底列，其餘日期仍須有效。未知日期、一般列的重要標籤、提示與非法作者仍拒絕。沒有複製使用者列表或文章內容進 fixture。

26 個 navigation、core 七套 suite／兩個 replay、desktop 全 targets Clippy 通過。自製滿頁 native fixture 同時加入空白標題與重要置底；兩次 macOS／WKWebView 各 30 筆原生操作與五次切板均為 NORMAL、精確 bytes 且無額外輸入，分別以 ⌘W／⌘Q 結束。使用者直接在 `dist/macos-live-ready/PTTStella.app` 重新登入後，實站 Test 空白標題列表已為 NORMAL，k／j／h 可操作；ID_Problem 重要置底列表也為 NORMAL，2j／l 可開啟重要置底文章，Ctrl+f／Ctrl+b 在 1／7 與 2／7 間往返，h 返回列表、最愛與主選單。此為實站修正後驗證，沒有保存文章內容。按鈕登入已由站端登入後畫面確認成功，帳密未經聊天或檔案。

### 列表無重畫時的 Vim 接續

2026-10-16 實站發現：最愛已在首項時按 gg，PTT 不重畫，舊 owner 將下一筆 4j 拒絕直到五秒期限。先以 production Connection／三種列表無回覆案例重現失敗，再讓已辨識列表的明確鍵盤移動（行、Home／End、翻頁）可接續移動或進入／返回一次；進入／返回後重新等待畫面。仍核對 session／revision、重新分析當前 frame；滑鼠、未知畫面與原生輸入不能取得此例外，閱讀僅保留原有純行移動延續。

36 個 desktop lib、1 個 bin route、desktop 全 targets Clippy、core 七套 suite／兩個 replay 通過。原生 probe 增為 34 筆：前 30 筆跨四種畫面與五次切板，最後故意在首列 Home 與 4j 後不回傳任何 bytes，仍能 l 進板及 h 返回。macOS／WKWebView 兩次獨立流程均 NORMAL、精確 bytes、零額外輸入，分別 ⌘W／⌘Q，關閉回覆 acknowledged、worker joined 與 listener released。最終包保留在 `dist/macos-live-final/PTTStella.app`，未覆蓋舊包。 同一最終包實站再確認：最愛首項連按 gg／4j 立即移到 Test，l 進板；Test 的 k／j／h、切換 ID_Problem、開啟重要置底、Ctrl+f／Ctrl+b 與 h 逐層返回皆正常。由正常 App 明確按登入取用使用者已存 Keychain 項目，站端兩次到達主選單，沒有取出密碼到工具或聊天。

### 關窗的正常斷線握手

2026-10-16：使用者確認要求關閉 App 時正常通知 PTT 斷線。舊 stop 已呼叫 close，但 close 只排入／送出 WebSocket Close，沒有持續讀取完成對端握手。改為明確 1000 Normal close、固定無帳密原因、持續讀取至對端回覆與底層關閉，總期限兩秒；站端未回覆 Close 即結束底層連線另標 peer_closed_without_reply；逾時或其他傳輸錯誤標為 unconfirmed，本機仍釋放 socket。重複 shutdown 沿用已完成結果，沒有把送出成功當作對端已確認。

關窗／⌘Q 在停止新互動後立即 shutdown／join 網路 owner，再等待 renderer 清理回報；不依賴 JavaScript 清理先完成才通知站端。已停止後不再讀取關閉的 view channel。工具列斷線沿同一路徑顯示確認／未確認。worker、Keychain 與 listener 仍清理完成才結束；沒有自動送 BBS 登出指令或回答確認問題。

斷線案例包含獨立 loopback 對端核對正常 Close 碼／固定原因、延遲回覆＋TCP FIN／客戶端 EOF，未回覆即結束底層連線，以及不回覆時兩秒內標為未確認並釋放。兩次原生 ⌘W／⌘Q 報告均為 disconnect=acknowledged、worker join 與 listener released，初版同時通過 30 筆導覽，最終包再通過 34 筆。實站最終包的工具列斷線顯示「已送出關閉通知；站端已結束連線（未回覆 Close）」；再次從正常 App 取用已存帳密、確認主選單登入後按 ⌘Q，owner 回報 PeerClosedWithoutReply，程序正常退出碼 0，沒有殘留程序。這是站端結束底層連線的結果，不是完整 Close ACK，也未直接驗證 PTT 線上名單更新。完整 Mac desktop backend 維持 not_ready，沒有 Windows／Linux 實機結果。舊 App 與中間產物保留，沒有 push／公開發佈。

### 按鈕登入與 macOS 鑰匙圈

2026-10-16：使用者提供專用測試帳號並明確選擇「記住帳密，按登入才送出」。已實作 [登入契約](../login.md)：macOS 工具列登入視窗、預設不保存的勾選、一個預設帳號及移除功能。帳號／密碼全部放在 Keychain generic password，沒有帳密設定檔、明文備份或可攜匯出；已存密碼不回填 WebView。Rust 自有 secret buffer 以 zeroize 清除，不提供 Debug／Clone／Serialize；不宣稱 OS／WebView／TLS 所有暫存都能清除。

送出由同一 Connection owner 序列化：明確按登入，核對當前 stamp 與第 21 列空白帳號提示才送帳號；第 22 列空白密碼提示及前列回顯此次帳號均確認後，才送密碼一次。未完成 ANSI、錯誤提示／畫面、游標／帳號不符拒絕。15 秒到期、取消、手動輸入、導覽及斷線清除等待；啟動／重連不讀取或重送。送出後只報告帳密已送出，後續公告、兩階段驗證、刪除其他連線等確認仍人工操作，沒有宣稱站端接受密碼。Keychain 每次一個 owned native worker，正常關閉與橋接失敗仍 join；OS 權限提示需使用者完成或取消。

驗證：core 七套 suite（新增 3 個 login 案例）／兩個 replay、32 個 desktop lib（含分段提示、過期拒絕、手動取消、真實 15 秒期限與重連不重播）、1 個字型資產 route、全 targets desktop Clippy、fmt 通過。macOS arm64／WKWebView 的獨立 App 使用自製帳密與專用 synthetic Keychain service，兩個程序分別保存／留空取用；peer 精確核對帳號及密碼兩筆輸入、零額外輸入，⌘W／⌘Q 後 worker／listener 清理 passed。另實際確認取消零輸入、錯帳號不能用已存密碼、已登入畫面拒絕再送、移除後重讀不存在。合成項目已移除；正常 Keychain 項目與使用者實站帳號均未操作。

同一最終 executable 再跑主選單／看板／最愛／文章的 30 筆原生 Vim 往返與五次切板，NORMAL、精確 bytes 與清理 passed。最終 App 為 `dist/macos-login-verified/PTTStella.app`，包含前輪切板與內建字型，舊包保留；ad-hoc 簽章有效。完整 Mac desktop backend 仍 not_ready；Windows／Linux credential backend、正式簽署後更新存取、原生 Keychain 拒絕權限與實站登入待各自驗收。使用者將直接在新版 App 輸入真實密碼，聊天及 repo 不接收密碼。沒有 push／公開發佈。

### 看板描述欄文字與連續切板修正

2026-10-16：使用者放大截圖顯示描述欄標頭為「中文測述」，原辨識器硬性要求「中文敘述」，使合法看板／最愛列表判成未知畫面。先以自製滿頁 fixture 在舊 reducer 重現 Unknown，再移除描述欄名稱條件；改核對標題色帶與編號／總數、看板、類別、人氣、板主，保留操作列、固定板名欄及選取指標。缺穩定欄位或項目欄位不符仍拒絕，暫停訊息另區分看板標頭／項目，便於後續定位。

24 個 navigation、27 個 desktop lib、8 個 Vim、全 targets desktop Clippy、fmt、core 六套 suite／兩個 replay 通過。新增描述欄名稱變體及缺欄位／非法列反例；同一 production Connection probe 擴充到 30 筆操作，包含五次看板／文章來回。macOS arm64／WKWebView 隔離 App 使用內建字型，實際顯示「中文測述」滿頁並維持 NORMAL；主選單／看板／最愛／文章後連續進出五次，peer 精確比對全部 bytes 且無額外輸入，⌘W 後 listener／worker 清理 passed。

已建立 `dist/macos-board-header/PTTStella.app`，舊包保留。上述為本機合成往返；使用者實站重試及完整 Mac desktop backend 仍待驗收，沒有 Windows runner 或實站帳號操作結果。沒有 push／公開發佈。

### 實站截圖版面對應修正

2026-10-16：使用者提供滿頁看板／最愛與文章列表截圖，均顯示「未辨識的畫面格式」。確認三個不相容條件：看板前進提示為「閱讀」而非僅「進入」；返回為「回上層」而非僅「回到上層／離開」；文章人氣與日期直接緊貼，原先 whitespace tokenization 會將 `+1610/16` 誤當日期。前一版合成列在這些欄位插入了多餘空白，因此漏掉實站格式。

文章解析已移除 token 日期搜尋，按 1／6／4／6／13 的固定 cell 欄位取編號、狀態人氣、日期、作者及標題。正常、爆文／標記、置底、單位數月日及六位數文章編號共用同一生產解析；legacy 圓形指標蓋住編號首字時，只在指標列接受數字尾段，包含 100000 的 00000。看板仍依自己的標頭及固定板名欄辨識，前進接受進入／閱讀，新增回上層文字。不把「閱讀」一詞當成文章列表判斷，h/l 仍經 Intent／Connection 序列化。

先以兩個自製滿頁案例在舊 reducer 重現失敗，再實作修正；fixture 使用合成作者／標題／板名，不保存私人 session。更新舊合成列到同一欄寬，新增完整 20 列看板與 15 一般＋5 置底的文章頁，原生 probe 也改消費這兩頁。22 個 navigation、27 個 desktop lib、全 targets desktop Clippy、fmt、core 六套 suite／兩個 replay、docs 19 個 gate 回歸通過；非法日期／提示覆蓋／缺前進命令仍暫停。

macOS arm64／WKWebView 的隔離 App 使用內建跨平台字型，在主選單→滿頁看板→最愛→滿頁文章執行原生 j/k/gg/G/l 或 h；兩種問題格式實際觀察均為 NORMAL，peer 比對 20 筆精確 bytes、零額外輸入，⌘W 後 listener 釋放與 worker join 為 passed。這是按截圖格式製作的本機往返，不代表已操作使用者帳號或完成實站重試。完整 desktop gate 的 Mac backend 仍為 not_ready，未取得 Windows runner 結果。

App 重新打包至 `dist/macos-navigation-layout/PTTStella.app`，包含本輪辨識修正與前輪內建字型；保留舊包，需退出舊 App 後開啟新版。沒有 push／公開發佈。

### 隨 App 附帶與載入字型

2026-10-16：使用者希望缺少的字型能隨設定直接套用。加入 `bundled`「內建跨平台」組合：Cascadia Mono 2407.024＋Noto Sans CJK TC Regular 2.004，兩份未修改的官方字型共約 16.4 MiB，OFL 1.1、來源／SHA-256／著作權隨產品保存。Windows 組合也可使用附帶的 Cascadia；Microsoft JhengHei 沒有隨 App 散布，不會靜默替換成 Noto。已保存的設定與平台初始值保留，需明確選擇新組合。

字型只編入 desktop executable，經 Dioxus 同 origin 的固定資產路由提供；沒有字型下載、外部 URL、任意檔案路徑或系統安裝。選取時 FontFace 載入 App 專用 family，預覽／主畫面／組字宿主／PNG 使用同一對應；最多兩個內建載入 promise，失敗可重試並顯示來源，其他字型仍核對 local face。來源與兩份授權可在設定視窗展開閱讀，也附於 macOS Resources／FontLicenses。設定視窗提高可用高度，實際 Mac 畫面確認載入訊息及儲存按鈕可見。

驗證：27 個 desktop lib、1 個 bin 字型資產路由案例、3 個可攜設定案例、全 targets desktop Clippy、fmt、core gate（六套 suite／兩個 replay）、docs 19 個 gate 回歸、JS／Python 語法通過。固定路由核對 MIME／原始 TTF／OTF 標頭、HEAD，以及未知路徑／方法拒絕；下載字型的來源 blob／SHA-256 與 byte 數核對，App notices 與來源相同，ad-hoc strict 驗章通過。

macOS arm64／WKWebView 隔離流程實際確認：未安裝 Cascadia 的 Mac 選 Windows 後只剩正黑體缺失訊息；選內建跨平台兩 face 載入成功。取消預覽的整頁 PNG 與原圖完全相同；保存後 960×576 中英字形均改變（ASCII 標題區 768 像素、中文選單區 5,163 像素），第二個程序讀取 `bundled` 後 PNG bytes 完全一致。兩次流程各跑 20 筆跨 profile 原生 Vim 事件，peer 沒有額外輸入，分別 ⌘W／⌘Q 退出，listener／worker 清理報告 passed。

最終 App 約 27.6 MiB，位於 `dist/macos-fonts/PTTStella.app`；舊包及驗證產物保留。完整 desktop gate 的本機 backend 仍是 not_ready，Windows 選單 gate 的選擇索引改由 option 位置取得以容納新增組合，但本輪沒有 Windows runner／Linux／跨 DPI／所有 glyph 或原生 IME 的實測結果。未輸入實站帳密或操作使用者站端 session。

### 主選單／看板列表的 Vim 支援增量

2026-10-16：使用者回報原生 Command 操作正常，但主畫面、看板列表／我的最愛及文章列表仍出現暫停。確認前兩類根本沒有 profile；文章列表原先僅在底列尋找完整返回／閱讀命令，未涵蓋上下操作列分配。新增 MainMenu／BoardList（含分類入口）與自製 fixture；命令可分布於上方操作列及底列，底列仍須有正常標記／樣式。提示覆蓋、未完成 ANSI、非法指標與操作區未映射資料仍拒絕，按鍵提示現在保留暫停原因。

主選單、看板／最愛與分類均沿同一 Keyboard → Intent → navigation::plan → Connection；j/k、計數、半頁使用方向鍵，h/l 返回／進入，gg/G 使用 Home／End。半頁依內容高度計算。主選單廣告及分類裝飾區的未映射字不再使操作暫停，標題／狀態／選項仍驗證。新增 profile 不提供文章命中區、不啟用滑鼠／防閒置；Connection 以明確 allows_keepalive 限定文章列表／閱讀，測試強行給新 profile UI permission 也無刷新輸入。

驗證：18 個 navigation、8 個 Vim、27 個 desktop lib（包含 20 筆跨 profile 精確 bytes 及新增防閒置拒絕）通過；全 targets desktop Clippy、core gate 的六套 suite／兩個 replay、fmt 通過。macOS arm64／WKWebView 的隔離 App 使用真正原生事件，依序在主選單、看板列表、最愛、上方閱讀命令的文章列表執行 j/k/gg/G 與 l 或 h；本機 peer 核對 20 筆 bytes，全部維持 NORMAL，⌘W 後 listener 釋放與 worker join，window-probe.json 為 passed。分類入口只有核心合成案例，沒有宣稱 Mac 視窗覆蓋該入口或實站登入後已修復。沒有操作使用者帳號的站端按鍵，也沒有錄製私人流量。

App 保留於 `dist/macos-navigation/PTTStella.app`，ad-hoc 簽署／arm64 打包完成，舊 App 保留。完整 Windows desktop gate 在本機 Mac 維持 not_ready；本輪沒有 Windows runner 或實站登入後結果。使用者須退出舊 App 後開啟這份新版再重試。

### macOS 操作修正與兩層輸入對應

2026-10-16：修正一般列表的空白補齊日期（如 `1/ 9`）、投票／鎖文標記及作者 `-` 的刪文列辨識。新增正常與錯誤日期／提示正反例，仍要求標頭、操作列、合法游標及完整格線。暫停會顯示具體原因；未辨識畫面仍不猜測站端動作。這些合成格式已驗證，使用者原先實站看板仍須重試，沒有據此宣稱實站問題完全結案。

依使用者要求維持兩層對應：`Keyboard` 將按鍵轉為瀏覽 Intent，`navigation::plan` 根據畫面產生 PTT bytes；移除 JS 重複的 key-to-byte 表。Mac 注音輸入來源的實測先重現 j 啟動組字；修正為 Normal／暫停使用 readonly 編輯宿主，Rust 只在瀏覽模式解析 Process／非 ASCII key 的實體碼與 Shift。i／f 原生輸入保留組字 owner；核心案例確認原生組字仍可送出，原生候選窗的完整流程本輪沒有重驗。

macOS 15.7.7／arm64／WKWebView 真實原生事件已確認：包含上述三種特殊文章列的看板維持 VIM NORMAL，j／k／gg／G 由同一 Connection 往返，loopback 逐筆只接受 Down／Up／Home／End。原生選單提供 ⌘W、⌘Q、⌃⌘F 及 Zoom；兩次成功視窗流程分別用 ⌘W 與 ⌘Q 關閉，`window-probe.json` 均確認 listener 移除、worker join 與零額外站端輸入。全螢幕進出及 Zoom 有實際視窗觀察。首次全螢幕測試曾被 UI 自動化的焦點／視窗變動打斷且服務端拒絕額外輸入；該次不算通過，後續獨立流程才通過。

字型視窗新增指定 local face 的載入失敗提示；本機選 Windows 時實際確認 Cascadia Mono／Microsoft JhengHei 無法載入，先前外觀相同是系統替代字型。新增 macOS 宋體組合（Menlo／Songti TC），保留原 macOS 黑體（Menlo／PingFang TC）與明確選擇意圖。真實表單切換、預覽、保存與主畫面 PNG 已驗證：960×576、英數首列全部像素相同，中文區域有 8,051 個像素不同，保存值為 `macos_serif`。只證明這台 Mac 的這兩個 face 與案例，不宣稱逐字 fallback 或跨 DPI 完整驗收。

驗證：桌面 feature 的全 targets Clippy、27 個非空 lib 測試、core gate（fmt／Clippy／六套 suite／兩個 replay）、JS 語法及 docs gate；完整 `desktop` gate 在 Mac 仍為 not_ready，沒有 Windows runner 結果。Mac 的既有 HTTP 取消案例原先因 RST 不等於 EOF 失敗；測試現在共用有限期的 FIN／ConnectionReset 關閉斷言，其他錯誤／額外資料仍失敗，下一筆圖片成功下載也必須通過。所有一般測試只使用合成資料與 loopback，沒有執行實站登入。

本次本機資源 guard 曾以全機磁碟預留超限拒絕預設 16 GiB 的 core invocation，該次未執行、不算通過。檢查本專案 namespace 約 0.8 GiB 後，以 4 GiB ceiling 重新執行，保留全機 60 GiB／free-space 100 GiB 規則，沒有刪除其他 owner 的資料或變更 harness。更新 App 保留於 `dist/macos-fixes/PTTStella.app`，舊產物保留；沒有 push／公開發佈。

### macOS 本機 App 打包增量

2026-10-16：新增 [macOS App 打包入口](../macos-app.md) 與 `Dioxus.toml` metadata。同一 Rust 1.97.0／Dioxus 0.7.10 package 在 macOS 15.7.7 Apple Silicon 完成原生 arm64 release 編譯，產出約 11 MiB 的 `PTTStella.app`；plist、僅系統動態依賴、ad-hoc 簽署及 strict 驗章通過。腳本從 Cargo JSON 取得 executable，不假設 target 路徑，並在 guard 結束前將 app 保留於 repo 的 dist。

以 bundle 內 executable 的 `--probe` 確認 macOS WKWebView 視窗能顯示自製中英文、雙色字、調色盤及方塊；本機接收 owner 已啟動。這是啟動觀察，沒有執行完整 Windows CDP 互動序列，提前關閉時因未完成 probe 序列回報 `unexpected loopback input at step 0`，不能算 desktop gate／完整清理驗收。另正常啟動曾顯示已連線，沒有輸入帳密；觀察後關閉並確認程序已退出。未做人工登入、macOS 像素判定、IME／剪貼簿／媒體與其他 OS 版本驗收。

本機 `dx bundle --macos --release --package-types macos --features desktop --bin pttstella --locked --out-dir dist/macos` 曾因 CLI 0.7.9 額外呼叫 nightly 的 unit-graph 探針，被 guard 以 `toolchain changed` 拒絕；該次不是通過。交付腳本改以受同一 guard 管理的固定工具鏈 Cargo build 建立標準 app，沒有改 guard 或切換工具鏈。產物為 ad-hoc 本機測試版，沒有 Developer ID、公證、自訂圖示、DMG 或 Universal Binary；既有 `desktop` gate 的 macOS backend 維持 not_ready。

### 既有核心與傳輸邊界

- 一個 Cargo package：純 Rust 終端核心與 desktop 依賴分離；同一核心供產品及離線 replay 使用。
- 核心管理 ANSI、byte/cell、cursor、局部覆寫／捲動與左右樣式；顯示依 [文字與排版契約](../dual-color.md)。
- Dioxus 開視窗與輸入介面；優先驗證 Canvas 薄橋接，收到的文字不拼成程式碼。實際畫面探針決定是否可採用。
- 原型邏輯格線可先用 80×24，這是初始值而非已驗證站端要求；視窗改變先縮放／留邊，不重新排字。傳輸探針確認需不需要協商其他尺寸。
- 只實作一條經驗證的加密傳輸，優先探針 WSS；確認端點、TLS、握手、payload 與可能的 framing 後才定案。需要改走 SSH 時另驗證 host key，不同時堆多條路線。
- 接收 bytes 完整進核心；送鍵由單一 session 擁有者序列化。generation 隔離舊連線、舊 frame 與晚到 callback，重連不重送帳密。
- 已鎖定 Rust 1.97.0、Dioxus 0.7.10 及 Cargo.lock；這是實測工具鏈，不宣稱 MSRV。
- mapping 來源與授權明確，不使用原版表格；Big5／UAO 差異與未映射狀態需有自己的案例。

## 首次登入的三個階段

| 階段 | 工作 | 完成條件 | 現況 |
| --- | --- | --- | --- |
| T1 核心 | Cargo 骨架、ANSI／cell 狀態、mapping 與離線 replay | 同一核心的非空合成案例通過，包含雙色字與任意 byte 邊界切分 | 基線完成：12 個核心測試、2 個 replay；未知控制碼保留計數，完整相容性未宣稱 |
| T2 視窗 | 真正 Dioxus terminal、固定排版、左右裁切、焦點與必要送鍵 | 看得對、送得對、斷開離線 sink／關閉視窗時 owner 可清理 | Windows WebView2 合成顯示探針通過：紅／綠字形、藍／紫背景、輸入與 listener 清理；IME 實機未驗證 |
| T3 登入 | 一種傳輸、接收／送鍵、手動登入與斷線 | 實際看見登入前與登入後畫面；斷線／錯誤可用且不污染新 session | 傳輸及鍵盤已接上；7 個本機傳輸／TLS 設定案例通過，實站登入前提示已確認，人工登入後畫面待驗證 |

T1 先完成 ASCII 格線與 snapshot 的最小路徑，再依必要案例加入中文／顏色／控制碼。T1／T2 共用 fixture 收斂顯示；T3 的無登入傳輸探針可提前獨立驗證。不要先還原整個參考程式，也不要先建立高階瀏覽器 UI。

T1 驗證 fmt／Clippy、核心測試及生產 replay；T2 需要真實 backend 的字型／frame 完成訊號與畫面判定；T3 的一般測試只用離線 adapter，實站登入是獨立人工流程。必要案例缺少就保持未就緒，不能僅有畫面或編譯成功就說完成。

## 最小驗收

| 案例 | 必須觀察 |
| --- | --- |
| 固定格線、中英文、ANSI 色彩 | 行列、空白、cursor 與字寬正確 |
| 一個中文字左右兩色／背景 | 同一完整字形與左右獨立樣式 |
| 控制碼／雙 byte 中間分割 | 最終狀態與不分割一致，無不相關位置配對 |
| 局部清除、覆寫、捲動 | 受影響畫面與字形配對正確 |
| 框線、方塊、縮放 | 固定排版與接縫正確，不隨視窗重排 |
| 必要按鍵與文字輸入 | 輸出 bytes 正確、焦點可用，不重複送出；無法編碼時明確報錯 |
| 取消、斷線、重連與晚到資料 | UI 仍可操作、清理 owner、舊 generation 不污染新 session |
| 實站手動登入 | 使用者看見站端登入後畫面，沒有自動猜提示／輸入／保存密碼 |

完整 IME／選取複製、所有 ANSI 指令與所有字型不列首次登入原型的完整支援宣稱。若所需登入流程或必要文字輸入用到某能力，就必須實作並驗證，不能以不支援規避。

## 完成與後續

使用者已啟動 [操作功能研究](../feature-research.md) 的實作 goal。各項在保留固定格線／雙色字的前提下獨立交付；登入卡住的實站重試另待使用者回報。

| 操作功能 | 狀態與驗證 |
| --- | --- |
| 選取與純文字複製 | 已實作 Rust 字形選取、滑鼠拖曳、雙擊整行、整頁選取；Windows 真實 WebView Ctrl+C 與系統剪貼簿讀回確認雙色中文字只複製一次。5 個互動核心測試；macOS／Linux 尚未實測 |
| IME 宿主與組字狀態 | 已實作游標位置的無框組字宿主、IME 確認鍵隔離、依編輯事件去重、失焦／session 變更清理；Windows WebView CDP 組字取消、確認與接續相同中文字的往返通過。原生輸入法候選窗及其他平台仍待實機驗收 |
| 單行貼上與多行預覽 | 已實作共用輸入 bytes 上限、單行直接排入、換行正規化、多行原生 dialog 預覽／取消、session 檢查與明確拒絕。8 個互動核心案例及 Windows 原生 Ctrl+V／剪貼簿往返通過；取消、不可編碼、超量、控制碼都未送入測試服務端 |
| 網址開啟與複製 | 已實作單列 HTTP／HTTPS 偵測、字形欄位映射、點擊開啟、右鍵選取後複製。拖曳不開啟，行末可能截斷或含未映射 cell 的候選不開啟。Windows WebView 點擊／拖曳／系統剪貼簿測試通過；開啟 adapter 在 probe 使用確認 sink，不啟動外部瀏覽器，實際 OS 瀏覽器啟動仍待人工驗收 |
| 整頁 PNG | 已以同一 renderer 匯出 960×576、不含選取／游標的完整頁面，限制 PNG 3 MiB／base64 4 MiB，原子替換保存。Windows 真實匯出按鈕、檔案獨立解碼與像素／畫布位置驗證通過；另存原生視窗的人工操作尚待驗收 |
| 選取行 PNG | 已加入目前畫面連續行匯出，完整 80 欄、每行 24 像素，保留空白行與雙色字；Rust 核對選取／畫面識別與對應圖片尺寸。Windows trusted 滑鼠／按鈕測試涵蓋單行、反向多行、首尾與整頁，以獨立解碼後的全部像素與整頁對應區域比對通過；縮放、可見畫布與捲動不受影響。未實作跨頁／不連續多段合併；原生另存與其他平台仍待人工驗收 |
| 字型設定與保存 | 已實作英數／中文字型、預設組合、字高／基線、同一 renderer 的獨立預覽與恢復預設。Rust 驗證、使用者設定目錄、備份與原子替換；損壞／超量／保存失敗保留原檔。Windows 真實表單編輯、取消、保存、拒絕無效值，以及第二個應用程序重新載入的逐像素比較通過；逐字 fallback、跨 DPI 與其他平台仍待實測 |
| 最小滑鼠導覽 | 已實作保守畫面辨識、文章編號點擊、閱讀翻頁／返回及列表滾輪移動。12 個核心正反例、2 個 production Connection 競態案例與 Windows WebView 的 trusted pointer／wheel 往返通過；服務端比對精確 bytes，取消、雙擊延續、組字／dialog／未知畫面及放大捲動均未夾帶額外輸入。hover 僅透明細底線，Canvas 像素不變，閱讀按鈕使用既有 hover 工具列。實站文章、實體 IME、跨 DPI 與 macOS／Linux 尚未驗證 |
| 預設開啟防閒置 | 已實作 [保活契約](../keepalive.md)：Connection 單調時鐘、60 秒閒置刷新、UI／畫面／session 許可、暫停開關、無回應釋放與錯誤。2 個時鐘案例、4 個 production Connection loopback 與 Windows trusted 開關／UI 暫停案例通過；實站長時間效果、實機睡眠與其他平台待驗收 |

當前 terminal 交付完成須 T1–T3 的必要項通過，且明列實測平台、字型、backend 與控制碼／mapping 範圍。Windows 成功只稱 Windows 原型；其他平台不當成已通過。後續是否加看板／文章導覽、現代介面或其他功能，再另行決定。

開工順序為 **Cargo 骨架 → ASCII grid／snapshot／replay → 中文與雙色字 → Dioxus 顯示 → 連線與手動登入**。每個可獨立驗收的子目標完成後提交，狀態只在本計畫更新。

## 目前驗證邊界

2026-10-15：Rust 1.97.0／Dioxus 0.7.10、Windows WebView2。

2026-10-16 增量：最小滑鼠導覽的核心、Connection 及 Windows 桌面合成往返通過；13 個 desktop lib 測試包含 2 個新增導覽競態案例。既有選取、IME、貼上、網址、PNG、設定保存／重啟像素與清理 gate 一併通過。未執行人工 PTT 登入後導覽；原先登入提示卡住仍待使用者實站重試。

同日符號比例修正：移除一般文字逐字橫向拉滿的幾何，改為自然比例置中、超寬等比縮小；連接框線／陰影及幾何方塊保留格線行為。Windows 預設字型的 `●○◎⊙` 筆畫 bounds 由 20×15、20×16、18×9、16×13 改為 16×15、16×16、10×9、14×13；獨立 PNG 解碼、雙色字、16 色、方塊接縫、截圖、桌面互動與設定重啟像素 gate 通過。實測紅／綠字形像素現在為 22／41；下列 32／43 為先前基線。其他字型／DPI／平台尚未全面驗證。Connection 當時只有站端 Ping 的 Pong 回應；其後新增預設開啟的 BBS 保活，見上述獨立驗證範圍。沒有 OS 防休眠計時器。

- 核心：fmt／Clippy、12 個測試與 2 份 replay 的獨立 expected／全部單一 byte 切分；強度可獨立保存在中文字左右兩格。
- 桌面：viewport 填滿視窗，工具列 hover 浮現且不推擠畫面；預設符合視窗並維持比例，提供等比例放大與捲動。Windows DPR 1 的合成探針畫布為 1040×624；100／125／150% 的連續方塊非預期像素均為 0（舊路徑為 1,712），半方塊與四向三角形抽樣通過。紅／綠字形像素為 32／43，16 色及 SGR 1／22／0 通過。listener／resize observer／縮放工具清理與 worker join 通過。字型 stack 為 Cascadia Mono／Microsoft JhengHei／monospace；逐字 fallback 未量測。合成結果不代表已重播使用者截圖中的實站文章，也不宣稱所有 ANSI Art 皆相容。
- 傳輸：本機 production WebSocket 案例涵蓋分割 bytes、協商、無自動登入、送鍵順序／escaping、重連清畫面、拒絕舊輸入、取消握手、站端關閉、異常 framing／容量及離線輸入；另驗證明確 TLS provider 可在 desktop 依賴組合下建立。
- 輸入修正：實際 WebView 點擊畫面可重現焦點離開 textarea；原因為可捲動容器的預設聚焦覆蓋 pointerdown 內設定的焦點。改由整個 viewport 管理點擊並取消內容區的預設聚焦，保留捲軸操作。實際瀏覽器點擊畫面／黑邊／縮放按鈕後 activeElement 均為 keyboard；ASCII、中文字編碼、Enter、方向鍵由 trusted input 到達 Rust。已移除會繞過焦點的合成輸入通過路徑。尚未以實體鍵盤或實站登入驗證。
- 確認提示調查：擴充 mounted probe，讓 ASCII／中文、方向鍵、Enter 經同一生產 Connection 送到本機 WebSocket，收到合成確認提示後再輸入 `n`、Enter，必須收到並繪製完成畫面；完整往返已通過。本機服務端隨後以代碼 1008 與合成原因關閉，瀏覽器確認原因文字且移開滑鼠後仍可見。斷線／錯誤工具列持續顯示，保留站端關閉代碼與原因（未提供時明示），斷線後輸入不覆蓋該原因。hover class 與 Rust 狀態 attribute 分別由各自擁有者管理。
- 實站卡住尚未重現：使用者回報重複登入提示輸入無反應、之後斷線；沒有捕獲該次站端關閉原因，不能宣稱已確認根因或已修復登入。[公開站端程式](https://github.com/ptt/pttbbs/blob/master/mbbsd/mbbsd.c#L384) 的 `multi_user_check` 在選 `n` 且仍達重複登入上限時會主動結束連線；這是來源推論，沒有證明實站部署版本或使用者當時連線數。
- 實站：獨立 `--probe-live` 成功連上 WSS，收到 4,397 bytes 並在同一生產 Screen 確認登入前提示，未送帳密，之後關閉並 join worker。報告不保存原始流量；這不是人工登入或登入後操作證據。

使用者下一步：啟動正常視窗 → 自動連上 PTT → 人工登入 → 確認站端登入後畫面 → 斷線。使用者截圖可觀察到文章閱讀畫面與 ANSI Art 接縫；本次修正在自製合成案例驗證，未錄製私人 session。實機 IME、原生關窗操作及 macOS／Linux 尚未人工驗證，跨 DPI 視覺相容性尚無完整判定。沒有宣稱效能改善，CPU／process-family peak memory 與完整任務 token 成本未量測。[依賴來源](../dependencies.md) 為公開套件。


2026-10-18 測試改善進行中：兩次人工單格 A/B 往返的 UI 觀察均有左鍵 held motion，但 samples 分別跨過指定 B 格至 col 11，peer 拒絕後未顯示 HOLD；保留各次失敗與 cleanup false，不能以本機反白或 PNG 按鈕可用算通過。改為明示 A/B 區域及有界走廊，仍要求 press A → held B → 提示重繪 → held A → release A；UI 另驗返回時提示已繪製，服務端仍獨立檢查 SGR bytes 與 Close。fmt、desktop lib 128 項、desktop bin 46 項、desktop all-targets Clippy、JS 60 項、core 與 docs 通過。新原生驗證待執行，不取代既有失敗紀錄。

2026-10-18 浮球工具入口：依使用者要求取代上緣 hover 熱區，Dioxus Lucide 六分類扇形入口沿用既有功能按鈕；單一 UI owner 管理可拖曳座標、速度衰減、側邊吸附、減少動態效果、小視窗配置與 capture／rAF 清理。原生合成視窗已核對六分類、開啟／取消書籤、100→125→100% 縮放、整頁選取跨收合／重開後仍可匯出行與整頁 PNG；peer 拒絕所有站端 bytes，Close acknowledged／全部 owner join 通過。兩次自動化 drag 未觀察到浮球位移，不當作原生拖曳／慣性通過；相關物理手勢仍待人工。其後補上面板按鈕的焦點保持、返回輸入時收合及預覽提示，最終包已重建，原生核對 PNG 完成後收合並恢復鍵盤、Close acknowledged／全部 owner join。兩份整頁與全行選取 PNG 以獨立 Pillow 解碼為 960×576，全部像素相同且已知藍／紫背景符合；不包含選取或浮球。fmt、desktop lib 128 項、desktop bin 46 項、desktop all-targets Clippy、JS 68 項、core 與 docs 通過；Windows 原生仍待各平台 gate。新的大區域往返探針首輪開啟後未收到任何 press／sample，五分鐘期限到期，保留 failed／cleanup false，不算人工手勢失敗或通過。

2026-10-18 人工回報未出現 HOLD 的實際紀錄：最終浮球測試包收到左鍵 press 與一筆 held motion，均為 col 7／row 8；原 A 左邊界為 col 8，因此 peer 在第一筆就拒絕，cleanup false。不是 B 未命中或逾時，不以斷線後本機反白算通過。測試改為走廊內按下後必須真的訪問 A，綠色 READY 再引導至 B；B 前沒有 A、以 release 代替返回 motion、外來按鈕／修飾鍵／走廊外均仍失敗。返回仍要核對 HOLD frame 的繪製 ACK。失敗直接繪製紅色 FAILED；人工準備等待延長至 15 分鐘，沒有更改正式 30 秒持有期限。新人工驗證待執行。

2026-10-18 浮球慣性修正：兩個新增案例在舊程式均失敗（快速拖曳後最後 1px 抖動使慣性幾乎歸零；pointerup 才帶來終點時完全忽略）。改用最近 120ms／最多 16 筆實際有界位置做速度估算，納入放手終點，短暫滑行衰減後再吸邊；停住後的舊速度不重播，反向依新軌跡，減少動態效果仍直接吸邊。視窗尺寸改變保留原側邊，避免全螢幕後浮球留在測試走廊中央。實體操作手感仍待新版人工驗證。
合成探針另外保留定長浮球放手觀察，區分未收到拖曳、零速度、系統減少動態效果與實際滑行；正常模式不寫入紀錄，不包含 PTT 內容或原始按鍵。
原生合成負例已確認：在 A 單擊（col 16／row 9）缺少 B／返回時立即繪製紅色 FAILED；仍保留 failed、cleanup false。新版本合成 PNG／Close／全部 owner join 通過，typed join 路徑確保即使 Stopped 在主迴圈退出後才抵達，浮球觀察仍由同一清理 owner 保存。原生自動化沒有取得浮球 drag release／coast frame，空 trace 不算慣性通過。fmt、desktop bin 48 項、desktop all-targets Clippy、JS 73 項及 docs 通過；Rust 畫面／Connection 的 129 項 lib、core 與 replay 在前述階段修正 gate 通過。待使用者補驗 A→READY→B→HOLD→A 與實體浮球手感。

2026-10-18 浮球點擊與慣性准入修正：新增回歸在舊版本確認 detail=0 的拖曳補發 click 會開啟選單並取消滑行、邊緣開選單會改變球的座標。改為有期限的拖曳 click 抑制，下一次實體按下或鍵盤啟動立即清除；開選單只調整扇形位置，球保留原位並置於上層，再點一次可收合。測試涵蓋完整 pointerdown/up/click、滑行未中斷、上下邊緣與鍵盤／無障礙後續啟動。76 個 JS 與 49 個 desktop bin 測試、fmt、Clippy、docs 通過；macOS 原生開／關已觀察。前一輪人工紀錄有兩次浮球放手、67 個滑行 frame、最大約 28px 位移，證明曾滑行，不代表後續手感問題已全面解決；新版實體回饋待驗。

2026-10-18 自由試用與嚴格驗收分開：新 tools probe 僅連本機，沒有啟用站端滑鼠或 A/B 驗收；普通拖曳可作本機選字。macOS 原生視窗已觀察單擊開／再點收合、文字拖曳後匯出 PNG、Close acknowledged 與全部 owner join；報告明示 freeplay_only，passed 範圍只含 PNG／清理，不代替慣性手感或嚴格往返通過。mouse_round_trip 的 A→B→HOLD 繪製→A→release 條件未變。使用者上一輪是 col 10／row 9 按下、只拖到 col 21／row 9 放開，未進入 B；failed／cleanup false 保留，不能以本機反白或 PNG 按鈕可用結案。新版自由試用交由使用者驗證反覆拖球與點擊手感。

2026-10-18 浮球選單遮擋修正：舊版新增三個回歸均失敗，包含 62px 分類按鈕相交、面板與扇形同時顯示及小視窗配置未避讓浮球。改用具間距的半圓排列，完整區域放不下時才切成可捲動格狀；功能頁與分類選單互斥，新增中英文返回分類按鈕並保留原分類焦點。面板／格狀共用浮球四側的可用區域，球不因開啟移位，仍可單擊收合。測試覆蓋 45 組尺寸／浮球位置、六個點擊範圍兩兩分離、縮放中保留頁面及返回 click 冒泡不奪走焦點。macOS 原生已觀察 1040×768 的扇形間距，以及實際拖曳縮窗後 480×408 的格狀、功能頁、返回焦點及球收合；PNG／Close acknowledged／全部 owner join 通過。嚴格 A/B 往返與 Windows 原生未以本項代替驗收。

2026-10-18 依使用者要求調整浮球放手手感：所有有實際位移的拖曳保留輕微滑行，慢拖或停頓後放手以最後實際移動方向提供約十多像素的短暫續移；快速拖曳仍採最近速度，不重播停頓前的高速。方向僅由邊界裁切後的實際位置更新，沒有實際位移的點擊不啟動慣性，碰到邊界不越界；減少動態效果仍直接吸邊。回歸先在舊版本確認慢拖連續三次與停頓後續移失敗；新測試用垂直位移分辨滑行與水平吸邊，另驗最後反向、邊界、單擊與清理。實體手感交由新版自由試用確認，不代替嚴格 A/B 往返驗收。

2026-10-19 使用者選定金色 E 像素星座作 App icon：保存帶透明外角的深藍／金色 PNG 母檔、產生提示與來源摘要，Dioxus bundle metadata 指向同一母檔。macOS 打包以 sips／iconutil 產生十個 1x／2x 表示，ICNS 在簽章前寫入 Resources，plist 使用 CFBundleIconFile。兩個 publication 回歸確認轉換失敗不簽署／交付半成品、既有 App 不覆寫；原生轉換並由獨立 Pillow 解碼逐份確認 16–1024px、金色內容及四角透明，strict 簽章通過。圖示版 App 保存於 dist/macos-gold-constellation/PTTStella.app，沿用雜湊已核對的上一份 desktop dev executable，這一步沒有修改或重編 Rust 產品行為。主題與扁平浮球由後續獨立提交處理，未以圖示結果宣稱完整桌面或跨平台驗收。

2026-10-19 深藍／金色操作元件與扁平星座浮球：集中 CSS 主題變數，套用一般、hover、按下、disabled、focus、主要確認／儲存按鈕及表單／dialog；浮球使用同一 E 星座意象的本機 SVG，直接編入 executable，保留按鈕可及名稱、事件 target 與拖曳 owner。PNG 母檔保留金屬切面，SVG 版本只用純色像素星形與連線。83 項 JS（含文字／圖示對比）、49 項 desktop bin、fmt、Clippy、docs 通過；原生已觀察星座主按鈕、分類按鈕、深藍字型視窗、金色套用按鈕及取消不保存。匯出 PNG 與先前同一 fixture 的 960×576 全部 RGB 像素差零，Close acknowledged、全部 owner join、strict 簽章通過。最終候選為 dist/macos-gold-theme/PTTStella.app；Finder 原生定位工具遇到 cgWindowNotFound，沒有據此宣稱 Finder／Dock 視覺驗收，但 plist、ICNS 各尺寸及 App 啟動已獨立核對。完整站端往返、跨平台與人工浮球手感仍依各自既有驗收範圍。

2026-10-19 浮球兼任功能頁返回：進入任一工具分類後，同一顆星座浮球切為金色 Lucide 返回箭頭，取代面板內大型返回按鈕；點擊回到分類選單並恢復原分類焦點與星座，再點一次才收合。可及名稱／提示跟隨中英文，返回狀態不標成展開開關。新增兩個回歸先於舊程式失敗，再確認六類／兩語、完整 pointer 點擊、鍵盤、座標不變及拖曳補發 click 不誤導航；85 項 JS、49 項 desktop bin、fmt、Clippy 通過。macOS 本機合成 App 已觀察顯示／擷取頁的箭頭、返回分類與再次收合，並完成 PNG、Close acknowledged 及全部 owner join。候選為 dist/macos-floating-back/PTTStella.app；使用獨立測試 App，沒有重啟使用者的實站視窗。設定頁的 Lucide 適用性僅評估，未修改其介面；本項不代替慣性手感、嚴格 A/B 往返或其他平台驗收。

2026-10-19 設定頁圖示與短標籤：使用既有 Lucide 套件，語言、字型／本機清單、字高、基線、預覽、來源／授權及檔案／確認操作共用可及的文字 caption；裝飾圖示不重複朗讀或增加 Tab 焦點。保持深藍／金色，主要保存按鈕的圖示沿用深色前景。基線另有可見說明及欄位描述；載入／匯出用短標籤並保留用途提示與常駐範圍說明，來源與授權仍可展開閱讀。85 項 JS、49 項 desktop bin、兩項 Rust i18n、fmt、Clippy 通過。macOS 中英文原生視窗已核對欄位名稱、圖示及底部按鈕；中文取消後重開仍為 20／20，再明確儲存 18／19；英文在草稿 18／19 時匯出仍是生效的 20／20，載入恢復預覽，再儲存 18／19，兩語各自保存語言不變。兩程序均完成 PNG、Close acknowledged 及全部 owner join。CSS 提供窄視窗上下排列與按鈕換行；原生縮窗工具回報 noWindowsAvailable，此項為 not_ready，未冒充小視窗或跨平台驗收。候選為 dist/macos-settings-icons/PTTStella.app，使用獨立合成測試 App，未操作使用者的實站 session。

2026-10-19 浮球不透明度：依使用者確認只調整浮球，功能選單與斷線／錯誤提示維持不透明。設定頁提供 20%–100% 滑桿與百分比，明確套用後保存；hover、鍵盤焦點與按下時恢復清晰。version 4 沿用語言／字型意圖，v1–v3 讀入預設 100%，不在載入時改寫原檔；工具外觀不進可攜顯示 profile。新增有界型別驗證、設定草稿／取消／失敗／恢復預設、兩端封包與幾何不變回歸；87 項 JS、50 項 desktop bin、129 項 desktop lib、5 項 profiles、Rust i18n、fmt、Clippy 通過。macOS 本機原生滑桿可用鍵盤調為 50%，取消後重開仍是 100%，明確保存後浮球變淡而選單清晰；獨立重啟同一設定後滑桿仍是 50%。兩程序 PNG、Close acknowledged 與全部 owner join 通過，未操作使用者的實站 session。其他站點與編碼是下一個獨立功能提交，尚未以本項宣稱支援。

2026-10-19 多站底層：新增具驗證的站點／編碼與有界最愛資料核心，預設仍為 PTT。WebSocket 與 Telnet 由同一 session owner 序列化，切站先關閉／取消舊傳輸、增加 generation 並重設格線及編碼，不重播輸入。Big5-UAO 保持既有路徑，encoding_rs 0.8.42 提供標準 Big5／GBK，中文字仍先佔 byte cell 再配對左右樣式；輸入與貼上可選同一編碼並整段拒絕無法映射文字。Telnet 關閉另標 TransportClosed，不冒稱站端 Close 確認；Binary 協商沿用同一 framing owner。其他站停用 PTT 登入、導覽、保活及名單投影。本機整合確認舊 WS Close、GBK 中文／精確輸入、舊 generation 零重播與 PTT 命令零送出；九套 core suite／兩份 replay、132 項 desktop lib、50 項 bin、Clippy 通過。站點 UI 及實站仍未以底層案例宣稱完成，契約見 [站點與編碼](../sites.md)。

2026-10-19 站點管理介面：浮球「連線 → 站點」提供位址、編碼、站點最愛與獨立預設站；保存由檔案 owner 執行，成功才 commit，失敗保留草稿，壞檔只讀。選取最愛只還原草稿，切站明確關閉舊連線，預設於下次啟動連線且不自動登入。PTT 專用 UI 與 Rust 准入共同停用其他站的鑰匙圈、導覽、名單及保活；一般輸入／貼上使用當前編碼。133 項 desktop lib、55 項 bin、93 項 JS、i18n、fmt、Clippy 及 docs 通過。

macOS arm64 原生合成案例完成非法 https 位址拒絕、保存／移除暫時最愛、保存預設、GBK→UAO→GBK 三次切站及 PNG。兩個獨立 Telnet peers 要求「中文」分別精確為 D6 D0 CE C4 與 A4 A4 A4 E5，切回 GBK 後零額外輸入；每條連線均收到 EOF。第二程序原樣載入已保存站點，自動連至 loopback GBK、確認貼上與 PNG，設定 bytes 未被改寫。兩程序均 TransportClosed、listener／connection／credential／file owner join。首輪原生檢查發現重複站點入口，保留 failed 紀錄；移除重複 ID 後才重跑通過。沒有操作使用者的實站 session，其他平台與原生英文站點視窗尚未驗收。

新增獨立 --probe-site 的登入前診斷，使用同一 Connection／decoder、無按鍵或帳密，報告不存站端文字或原始流量。新增回歸先確認僅三個 Telnet 協商 bytes 會過早結束，再改為要求可見文字；舊實作的 received_bytes=3 與獨立預期 7 不符，新實作通過。最終候選 dist/macos-sites-ui-r3/PTTStella.app 的變更僅補強該 CLI 診斷，站點 UI 原生案例於前一候選完成；最終 strict 簽章通過。

本機明確公開站診斷（2026-10-19）：PTT2 WSS 收到 3,452 bytes 並辨識登入提示，巴哈 WSS 收到 1,039 bytes 可見文字，兩者 unmapped=0；兩站在關閉時底層結束但未回覆 Close，因此整體診斷均保留 failed／peer_closed_without_reply。水木 telnet://bbs.newsmth.net 連線逾時，沒有宣稱實站 GBK 或登入通過。三站未輸入帳密，也未以本機編碼／UI 成功代替公開站登入驗收。水木網路可達性、兩站關閉握手及三站人工登入是已知後續驗收項目。


**2026-10-19 圖示 B 定稿**：採使用者選定的深藍／淡金三聯星，保留三條兩兩連線、下方小星更向外且略大。以專案自製 SVG 與 Icon Composer 材質渲染，連線獨立關閉玻璃效果並遮罩於星體邊界；1024 單位的線寬由 10 加至 24，維持 38% 不透明度。可編輯 `.icon` 與來源摘要保存在 assets/icons，浮球扁平稿同步幾何。32／64 px 原生匯出已目視確認連線與輪廓；App 使用靜態 ICNS，未整合系統動態材質。本機 arm64 候選 `dist/macos-icon-b/PTTStella.app` 已完成 dev／debug=0 打包，2 個打包回歸測試及 26 個主題／浮球 JS 測試通過；10 個 ICNS 尺寸、透明角落、plist 指向與 strict ad-hoc 簽章核對通過，實際 32px ICNS 已目視確認。證據保留 `.local/verification/icon-b`。本輪未重新驗收原生 App 互動，候選亦含尚待原生驗證的圖片預覽尺寸調整；不把渲染或編譯當作完整 GUI 驗收。

2026-10-19 圖片預覽放大：hover 卡片上限改為 560 CSS px／內容寬 55%／高 60%，計入狀態列、內距與邊框；完整預覽移除 1100×800 上限，四邊各留 16 CSS px。修正右上固定預覽仍參照全視窗浮球容器底緣而落到下方的問題，改以文章區右上角錨定；維持比例、原尺寸上限及同一圖片升級重用。26 項媒體／dialog／生命週期／連結 JS 測試通過；新增實際瀏覽器排版頁的六項自製圖片案例全數通過，涵蓋橫／直／小圖、360×300 至 1800×1100 視窗、碰邊、原尺寸／符合與清理。1800×1100 下 hover 圖片為 550×366.7、完整預覽可顯示原始 1200×800。macOS 原生合成測試 App 可啟動，但座標操作工具回報 noWindowsAvailable，嚴格 dialogs 報告仍為失敗，互動驗收列 not_ready；瀏覽器排版不代替 trusted pointer、Rust 准入或實站驗收。證據保留 `.local/verification/glass-media`，最終 arm64 dev／debug=0 App 為 `dist/macos-media-large/PTTStella.app`，打包、系統依賴及 strict ad-hoc 簽章通過；沒有重啟使用者的實站 session。

2026-10-19 網址常駐底線：live renderer 使用 Rust 已投影的連結片段繪製細灰線，hover 只加亮命中片段，不改 ANSI 字形或背景；跨列 OSC 8 共用既有連結／命中資料，PNG renderer 預設不繪製本機提示。6 項連結 JS 回歸與 core gate（13 個命令、2 個 replay）通過。macOS 獨立合成 App 的視窗及全螢幕已觀察兩段連結底線，實際擷取按鈕匯出 960×576 PNG；live 灰線以原生畫面目視核對，獨立像素檢查核對 export 僅有來源黑／青色。初次像素斷言錯把字母下伸部當成非黑失敗，檢查來源圖後改以 fixture 的 ANSI 色彩作獨立預期，未修改產品或圖片；螢幕擷取含有損壓縮，不以其像素當精確 RGB 驗證。正常 Close acknowledged、全部 owner join 通過。完整 osc8 報告仍為 failed，因本次沒有操作開啟／複製／拖曳／媒體案例，不宣稱完整互動 gate 通過。證據保留 `.local/verification/link-underlines`；候選 `dist/macos-link-underlines/PTTStella.app` 已完成 arm64 dev／debug=0 打包與 strict ad-hoc 簽章，未操作使用者實站 session。

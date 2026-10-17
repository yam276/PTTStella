# PTTStella 最簡 terminal 計畫

狀態：**畫面核心、Windows 視窗與 WSS 登入前連線已驗證；macOS 專用帳號已觀察手動登入與部分實站瀏覽，平台與案例範圍分列如下**。第一個驗收平台為 Windows；Rust／Dioxus 維持可攜，macOS／Linux 後續各自驗證。

## 目標

開啟視窗、自動連上 PTT、手動輸入帳號密碼、看見登入後畫面，最後可正常斷線。文字、固定排版與雙色字由核心與 renderer 實作。

使用者已明確補充：只有外觀像 terminal，並非真實系統終端。本產品不開 shell／PTY，不追求通用 terminal 相容。

初期 terminal surface 已擴充本機操作功能、依 [導覽契約](../navigation.md) 的最小滑鼠導覽，以及目前畫面連續行 PNG 截圖。本階段依 [媒體 MVP 規格](../media.md) 加入按需媒體預覽。沒有獨立文章 UI、板名導航按鈕、啟動時自動登入或背景追蹤；後續依使用者明確授權加入 [按鈕登入與鑰匙圈保存](../login.md)。

## 下一階段：站端相容性與桌面操作

2026-10-17 研究與排序；以下尚未實作或驗收。使用者要求將近期站方功能一併納入桌面功能規劃。依公開公告與官方原始碼確認協定，再用自己的核心、互動意圖與合成案例實作；原始碼合併日期不代表 PTT1／PTT2 已部署。下列風險來自程式檢查，尚未證明是過去實站 Vim 暫停的原因。

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

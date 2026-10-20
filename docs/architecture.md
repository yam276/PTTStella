# 最簡 terminal 架構

Rust／Dioxus。先做單一桌面視窗與一條傳輸；第一個實測平台為 Windows，核心保持無 GUI／網路即可執行。工作順序見 [active plan](active/minimal-browser.md)。

```text
connection framing → raw terminal bytes → Rust terminal state
                                               ↓
                                  glyph mapping／render frame
                                               ↓
                                     Dioxus terminal surface

manual keyboard input → session-owned send queue → connection
```

初期一個 Cargo package；終端狀態、encoding、render、transport 與 UI 依需要分模組，不提前建立空框架。desktop 依賴不污染 headless 核心測試。

| 擁有者 | 責任 |
| --- | --- |
| terminal | byte/cell、ANSI、cursor、捲動／覆寫、樣式及 revision |
| encoding | 明確來源的映射與反向輸入編碼；保留未映射／非法狀態 |
| input | 共用編碼後 bytes 上限；貼上換行、控制字元與完整預檢；預覽保留正規化文字及相同送出 bytes |
| text／navigation | 共用完整字形到 UTF-8／cell 的投影；保守辨識列表與閱讀畫面，產生 typed 瀏覽意圖及有限的送鍵計畫 |
| render | 完整字形、固定幾何、左右樣式及 frame ID，不重解 raw stream |
| transport／session | framing、收送、取消、斷線與 generation；Connection owner 依共享 reducer 規劃導覽及明確單次登入，傳輸本身不解析畫面 |
| UI | 連線控制、terminal surface、必要焦點／按鍵、顯示及可見錯誤 |
| media／preview | URL 分類、單一預覽 request／generation、下載 future、解碼確認與有界 LRU；不解讀 BBS 原始 bytes |
| TLS | 為 WSS 與圖片 HTTP client 建立各自的系統信任根／明確 provider 設定，不安裝全域 provider |

## 程式入口與修改邊界

連線與 UI 已依下列責任拆分；同一個 Session 與同一個 surface 迴圈仍負責序列化操作，沒有增加另一份狀態或後端框架。

| 位置 | 主要責任 |
| --- | --- |
| [connection](../src/connection.rs) | 有界命令 API、watch 快照及 worker／probe 的關閉與 join |
| [connection/worker](../src/connection/worker.rs) | Session 唯一擁有 Screen、socket、登入等待、單一目的地導覽與輸入等待；具名方法處理登入、導覽、保活與發布，select 只分派事件 |
| [sites](../src/sites.rs)／[sites/store](../src/sites/store.rs) | 位址正規化、站點／編碼、PTT 能力、有界最愛與獨立預設站；儲存由既有檔案 owner 執行 |
| [navigation_job](../src/navigation_job.rs)／[article_navigation](../src/article_navigation.rs) | Board／Article 的封閉目的地型別及純導覽 reducer；文章組合既有切板、AID 搜尋與資訊核對，Session 唯一排程及發鍵 |
| [bookmarks](../src/bookmarks.rs)／[bookmarks/store](../src/bookmarks/store.rs) | 明確文章識別的有界本機資料、名稱／建立時間、版本驗證、備份與原子替換；不讀取畫面或送鍵 |
| [mouse](../src/mouse.rs) | Screen 擁有的站端模式／epoch 與有界 SGR press／release encoder；只有格式子集，送出 owner／UI 尚待接線 |
| [connection/transport](../src/connection/transport.rs) | WebSocket／Telnet 的具體 socket enum、TLS、有界寫入及正常關閉；不解讀畫面或決定瀏覽動作 |
| [ui/app](../src/ui/app.rs) | Dioxus 畫面、原生關窗事件與啟動接線 |
| [ui/protocol](../src/ui/protocol.rs) | WebView 結構化事件、serde wire 格式及共同輸入准入；沒有副作用 |
| [ui/outbound](../src/ui/outbound.rs)／[ui/bridge](../src/ui/bridge.rs) | Rust→JS 的 tagged Packet 與具型別 payload／結果狀態；一般 bridge 只接受 Packet，原始 Eval 交由 lifecycle join |
| [ui/commands](../src/ui/commands.rs) | 一筆可取消的命令結果 future，由 surface 的 select 輪詢；不另建送鍵 task 或佇列 |
| [ui/files](../src/ui/files.rs) | 單筆互動式檔案工作及有界最近看板 writer、UI thread 的非同步原生 picker，以及 blocking pool 上的有界讀寫／PNG 解碼；關閉撤銷未開始工作、保留並等待已開始工作 |
| [ui/lifecycle](../src/ui/lifecycle.rs) | renderer、媒體、鑰匙圈、檔案及命令等待的共同清理擁有者；正常／錯誤退出都停止並 join |
| [ui/presentation](../src/ui/presentation.rs) | 遮黑後的唯一 Paint、連結／圖片命中、選取及輸出使用的版本核對；字型預覽也使用同一封包 |
| [ui/settings_window](../src/ui/settings_window.rs) | 單一原生設定視窗、可取消輪詢的建立 future、32 筆有界事件橋接及最後有效修改快照；不持有站端連線或直接讀寫檔案 |
| [ui/preferences](../src/ui/preferences.rs) | 生效設定、草稿建構與成功保存後的 commit；初載只由檔案 worker 呼叫，互動時不自行選檔或讀寫 |
| [ui/bookmarks](../src/ui/bookmarks.rs)／[surface-bookmarks](../assets/surface-bookmarks.js) | Rust 保存後 commit 與壞檔保留；JS 草稿、選取、單筆 request、晚到結果及 dialog 清理。前往只填既有目的地草稿 |
| [ui/sites](../src/ui/sites.rs)／[surface-sites](../assets/surface-sites.js) | Rust 保存成功後 commit；JS 管理站點草稿／單筆 request，選最愛不連線，明確切站交給唯一 Connection owner |
| [ui/surface_state](../src/ui/surface_state.rs) | 一筆 in-flight Paint、已確認畫面、dirty、名單投影、選取與 hover；集中繪製確認、版本核對、失效及 PNG lease |
| [ui/probe](../src/ui/probe.rs) | 自製畫面、驗收計數、獨立色彩 expected 與報告建構，不擁有產品的輸入或畫面狀態 |
| [ui/surface](../src/ui/surface.rs) | 協調事件、信任／版本檢查、單筆繪製與各 feature owner 的停止順序 |

新增瀏覽操作先修改 vim／navigation 的純 Rust 契約，再檢查 Session 的送出許可；JavaScript 不增加 key-to-byte 表。新增偏好先擴充 settings 的格式／驗證，再由 Preferences 統一保存與更新，不在事件分支另存第二份生效值。新增 WebView 事件先定義 protocol，再由 surface 交給對應擁有者，保留 generation／revision／input epoch 與 presentation 的拒絕邊界。

事件先經 `SurfaceEvent::admitted`，再變更本機狀態、編碼或入列。Key／Text／Paste／登入／導覽核對 generation；貼上確認在可信事件通過後才取走預覽，並再次核對預覽所屬連線。組字保留兩個明確例外：JS 已確認 trusted start 與相同最後 input 的 legacy composition 結果，以及只撤銷編輯許可的 composition_end。這是 WebView 事件來源契約，沒有將 trusted 欄位當成抵抗頁內任意程式的安全邊界。

登入／滑鼠導覽／Vim 瀏覽只各自建立一筆共同 Pending 中的結果 future；等待期間繼續輪詢畫面、媒體、鑰匙圈與關窗。Pending 確認空槽後，同步呼叫 Connection 的 request API 立即入列並取得 Reply，只將等待結果留給 select，避免後續原始輸入搶先；已有等待時連入列 factory 都不執行，不增加第二層佇列。取消登入、換 session 或關窗會丟棄 reply receiver，使 Connection 拒絕尚未執行的排隊命令。已開始的 write 無法回滾，也不會因結果未確認而重送。

surface 所有返回路徑都進入 Resources 清理：先取消命令、撤銷保活、媒體及檔案許可，再並行要求 renderer 停止、Connection shutdown 及等待不可中止的 OS credential／已開始檔案工作。除了資料通道 stop，靜態控制事件也可喚醒 renderer 的單一 receive；JS 先移除互動 listener，再於 finally 釋放其餘資源並回傳清理確認，Rust drain Stopped 並 join eval。五秒未完成只更新可見狀態，保留正在等待的擁有者，不能宣告清理成功。Connection 即使收不到 shutdown 回覆，仍 join worker／probe 並保存失敗結果，重複呼叫不把失敗改報為無連線。

Files 同時只接納一個互動式檔案工作，忙碌時不執行第二個 factory。最近看板使用同一 Files 下的專屬 writer，只寫獨立資料路徑，一個進行中 snapshot 與一份合併後最新 snapshot；不與 picker 互相等待，也沒有無界佇列。關閉撤銷新工作後，既有及已接納的最新 snapshot 都需 drain／join，失敗納入共同清理結果。設定／名單初載、保存、設定檔匯入匯出及 PNG 解碼／寫入使用 blocking pool；原生 picker 仍在 UI thread 建立及輪詢，明確請求後先聚焦父視窗。UI select 繼續接收畫面、關閉及其他結果，保存成功才 commit 生效設定／規則。關閉許可與 PNG 版本 lease 分開：清理撤銷尚未開始的工作，保留 picker／已開始寫入到完成，五秒後仍顯示等待；取消／過期是正常撤銷，真實失敗會併入清理結果，不能假報成功。

SurfaceState 在 ACK 前保留唯一 in-flight Paint，等待期間只標記 dirty；ACK 後才送最新 watch 的畫面。未知 ACK 不消耗原等待；即使舊 session 的畫面仍在繪製，也要先 drain ACK，舊 snapshot 不能通過目前版本核對。規則／揭露改變同步清除選取、hover 與匯出許可。來源更新可撤銷互動，Vim 無重繪連按仍保留既有的邏輯瀏覽契約。

PNG 開始時核對相同呈現與選取行範圍，再取得可撤銷 ExportLease；解碼後、picker 結果及 worker 開始寫入前再次核對。lease 直接讀最新來源 watch，不等待 UI 消費來源更新；規則／揭露／顯示改變撤銷所有舊 PNG，選取變更與 input epoch 只撤銷相關選取行 PNG。JS clearSelection／新選取在丟舊 cache 前送 SelectionCleared，Rust 按完整 identity／request 撤銷；舊通知不能清掉新選取，整頁 PNG 不因無關清選取而取消。開始寫入後不能假裝回滾，仍等待並回報實際結果。

Rust→JS 封包使用 `{kind, payload}`；Stop／MediaReset 是沒有 payload 的 unit variant。登入、名單與設定檔回覆使用明確結果 enum；導航、貼上、保存、匯出與外開使用各自的 status enum。Preferences 回傳同一 Packet，診斷報告不走 UI 通道。JS 的 [router](../assets/surface-packets.js) 只分派指定種類，拒絕未知種類、缺 payload 與舊式混合欄位，不再把未知回覆當成 Paint。雙端隨同一 App 編入，沒有額外通訊服務或通用訊息框架。

JS 的 [設定](../assets/surface-settings.js)、[登入](../assets/surface-login.js)、[名單](../assets/surface-filters.js)、書籤與站點各自擁有 DOM 欄位、草稿、busy、timer／request 及清理。它們透過具名的 send、prepare／invalidate、restore 與 changed 回呼協作；bindings 交回既有唯一 listener 擁有者統一撤銷。changed 統一刷新瀏覽與輸出控制項，避免關閉一個 dialog 後留著另一功能的暫停狀態。Settings 的 epoch 與 preview request 在 await 字型載入後再次核對；關閉／換草稿／revoke 後的結果不能重畫或搶回焦點。清理先 revoke 各 owner，再等待工作並 destroy，最後分別核對 clean。

surface 仍共同協調 Rust 版本核對，以及 JS 的輸入、選取、繪製、字型載入快取與 pointer 操作；這些是同一畫面需要共享的資料，不複製為另一個 terminal 模型。後續可再按實際修改需求縮小 Rust 事件分支；拆分涉及焦點、組字與取消時，仍須有實際 WebView runner。

優先驗證 Dioxus Desktop 與 Canvas 的薄繪製橋接；實際呈現路線以同一 [顯示契約](dual-color.md) 的 probe 決定。若採 JavaScript，只接收結構化 frame，不持有第二份終端狀態。

UI 正常掛載並完成檔案初載後，只發起一次已保存的預設站連線，初始為 PTT WSS；合成 probe 僅連本機服務。renderer 唯一管理符合視窗／等比例 zoom 及繪製排程；接收的 Screen frame 不因縮放改寫。浮球由獨立的 surface-tools owner 管理座標、慣性、吸邊、capture 與收合；六個 Lucide 分類入口消費同一組功能按鈕，不複製業務事件。bindings 仍由同一 UI lifetime 註冊／移除，停止時撤銷 capture 與唯一 animation frame；Rust 透過獨立 data attribute 及狀態提示維持斷線／錯誤可見。尺寸變更合併為一個 animation callback，背景共用整數像素格線，圖形幾何與文字都使用同一格線與左右 clip。所有輸入／縮放／hover listener 由同一 bindings 清單註冊與移除。

不丟原始 bytes；UI 可合併最新 frame，帶 generation／revision 防止晚到更新。手動送鍵序列化，連線關閉時停止對應擁有者，重連是新 session。

DEC2026 的開始／結束由 Screen 解析；同步期間仍完整作用所有 bytes，但 Frame 不算可導覽的完整控制狀態。Session 以原有發布時鐘持有最長一秒的同步等待，重複開始不延長同一批；缺結束碼時釋放顯示，再依 parser／navigation 判定是否可操作。期限屬產品恢復選擇，並非協定規定。Connection View 的 output_pending 與保留的完整 Frame 分開：新終端資料先更新 watch 的失效狀態，再等待傳輸回覆或發布畫面，讓檔案 worker 能直接拒絕過期 PNG。同步中保留舊圖但不准入導覽、選取、連結、hover 或新匯出；Rust／JS 同時撤銷舊互動，UI 不解讀原始控制碼。原始輸入、協商、Ping／Pong、取消與關閉仍由同一 Session 處理；斷線／重連清除同步等待，不重送操作。

資料路徑為 WebSocket binary frame 或 Telnet TCP bytes → 同一 Telnet framing → Screen；編碼由當前 Target 決定，接收與輸入／貼上共用。每則 WebSocket 訊息上限 64 KiB，TCP 每次讀取上限 8 KiB；不把 WebSocket 文字 frame 猜成 legacy bytes。輸入上限 4 KiB、佇列 64 項；無法編碼或佇列滿會顯示錯誤。連線、送出與關閉都有逾時，送出失敗不重試；連線 owner 管理並 join 握手／收送工作。切站由同一有界命令佇列執行，關閉舊傳輸後才更換 generation、編碼與畫面；PTT 能力以精確目標判定，其他站不做 PTT 辨識／命令或名單投影。UI 只允許一個尚未完成的繪製 frame，其餘保留最新狀態，不堆積 JS 繪製佇列。關閉視窗時先停止 listener 與連線工作，再關閉 host。

滑鼠導覽只建立目前格線畫面的辨識與命中，不建立獨立文章模型、板名導航按鈕、登入腳本或背景追蹤。文字、排版與雙色字由核心與 renderer 實作；操作及視覺契約見 [滑鼠導覽](navigation.md)。

`navigation` 唯一產生畫面種類、文章編號 hit region 與 typed intent 的按鍵計畫；UI 管理一筆 pointer 交易，網址優先，拖曳／取消／失焦／畫面或輸入版本變更即取消。UI 帶上 generation、revision、input epoch，Connection 在序列化送出處核對並從目前 Screen 重新規劃。鍵盤輸入會遞增 epoch；新終端 bytes 到達前暫停滑鼠導覽，避免用舊游標連續計算，但不阻擋手動送鍵。這是本機防重入機制，沒有把新 bytes 當成站端完成動作的保證。JS 不解析 BBS 文字，只消費 Rust 的結構化辨識與命中資料。

選取核心同時產生純文字、字形 spans 與完整行範圍；PNG 消費同一行範圍，不從 DOM 選取框反推。匯出先用同一 renderer 畫固定 960×576 的無游標快照，再按完整行裁切；獨立於顯示縮放／捲動，亦不包含本機提示層。Rust 保留選取 request 與行範圍，保存前核對畫面及選取識別、行數對應高度與共用容量限制。每次只允許一筆匯出，停止時等待編碼工作並移除按鈕 listener。

Connection 同時擁有防閒置的單調時鐘、預設開啟狀態、UI watch 許可及自動刷新等待。UI 僅回報本機操作是否允許，不建立自己的 timer；owner 核對當前 Screen、session／輸入版本及等待狀態後才送一次刷新。手動輸入與自動刷新以不同 PendingInput 變體管理，只有自動等待會因無回應而逾時釋放並停用保活。規則與公開站端來源推論見 [保活契約](keepalive.md)。

媒體依 [預覽契約](media.md) 提供 dialog、hover 與閱讀頁 Enter 展圖。Enter 使用獨立的有界 inline owner，消費目前已繪製、已過濾的圖片連結；同一 Loader 序列下載，每頁去重並以文章內的 URL key 保留有界資源；翻頁只更換已確認頁面的行位置與連結索引，不撤銷同篇 batch。離開閱讀、其他操作或換 session 統一清理。顯示層只插入圖片空間與原 renderer 的行切片，原格線與 PNG 契約不變。Rust Preview 擁有可取消的 HTTP future 與待解碼內容，關閉／換 session 只有一個清理入口；沒有 detached 下載 task。JS 收到通過格式／尺寸／動畫容量檢查的 bytes 後建立 Blob URL，由 WebView 解碼，成功回報後才保留快取；失敗內容不得造成重試一直命中壞圖。JS owner 註冊／移除自己的 listener 與 ResizeObserver，關閉時撤銷 Blob URL、停止 video、移除來源並清空 canvas；本機轉換／播放 promise 在結束時等待完成。影片使用獨立串流來源，不塞進圖片 cache。BBS canvas、輸入、PNG 與媒體渲染不共用解碼鎖或排版狀態。

按鈕登入的 prompt／容量屬 login 核心，Connection 擁有單次等待與送出，Keychain worker 擁有 OS 存取與清理；UI 不接收已存密碼。安全儲存與驗證邊界見 [登入契約](login.md)。

關閉視窗／⌘Q 先停止新互動並立即要求 Connection 結束；網路 owner 送 WebSocket 1000 正常關閉碼，持續讀取至站端回覆與底層關閉，總期限兩秒；依 [Tungstenite 關閉契約](https://docs.rs/tungstenite/0.28.0/tungstenite/protocol/struct.WebSocket.html#method.close) 驅動完整握手。回報區分 acknowledged、peer_closed_without_reply（未回覆 Close 即結束底層連線）、unconfirmed 與 no_connection，Telnet shutdown 另記 transport_closed。重複 shutdown 沿用已完成結果；沒有把送出 Close 或本機 TCP shutdown 當成站端已確認。renderer／Keychain 與網路 worker 都 join 後才完成 App 關閉。這表示傳輸關閉結果，不宣稱站端線上名單即時更新。

合成 probe 的浮球放手觀察附在 renderer finally 的具型別 join 回覆，由同一 Resources 清理 owner 保留；Stopped 通知和 join 的先後不會遺失觀察，也不能把失敗的清理改成成功。正常模式不保存軌跡或觀察檔。

設定視窗由主視窗持有弱 native handle 與建立 future，完成建立後才明確顯示及聚焦；重複開啟沿用同一視窗。Bridge 將設定／預覽／字型清單回覆送到子視窗，其事件白名單不接受站端輸入。生效設定與背景 I/O 仍由 Preferences／Files 擁有；最後有效修改在共同 Files drain 後才覆寫最新設定，避免舊 snapshot 覆蓋主畫面的其他偏好。正常關閉設定先完成 autosave 再隱藏；退出 App 時停止其 JS、join eval 並改為真正關閉 native window。主畫面、PNG 與字型預覽共用 [surface-renderer](../assets/surface-renderer.js)，不複製繪製規則。

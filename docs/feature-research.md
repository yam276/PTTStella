# PTTStella 操作功能研究

2026-10-15。研究範圍為文字選取與複製、IME 與貼上、網址操作、字型設定及 ANSI Art 截圖。沿用 Rust／Dioxus 與固定格線 Canvas，不增加系統 terminal、shell 或 PTY。這些是後續功能設計；目前交付與登入問題狀態仍見 [active plan](active/minimal-browser.md)。

建議先建立依畫面版本選取完整字形的能力，再完成中文輸入與剪貼簿。網址與截圖共用同一份畫面資料，字型設定最後處理，避免輸入、座標與繪製同時變動。

## 現有能力與實際探針

[Screen／Frame](../src/screen.rs) 已提供字形文字、行列、欄寬、左右樣式、游標與 revision；[UI](../src/ui.rs) 另帶 session generation。[renderer](../assets/surface.js) 已有共用整數像素邊界、縮放、捲動與輸入 textarea，可延伸操作功能，不需要再解析 ANSI 或 Big5。

使用目前執行檔的隔離 `--probe`、合成畫面與 loopback CDP，在 Windows WebView2、DPR 1 測得：

| 項目 | 結果 | 設計影響 |
| --- | --- | --- |
| `isSecureContext` | `false` | 不以 secure context API 當必要前提 |
| `navigator.clipboard` | `undefined` | 不能直接採用 `readText`／`writeText` |
| `ClipboardEvent` | 存在 | 可研究使用者觸發的 copy／paste 事件；尚未證明系統剪貼簿讀寫 |
| `setPointerCapture` | 存在 | 可處理拖曳離開字面後的選取生命週期 |
| `document.fonts.load` | 存在 | 可等待字型後繪製；不代表指定字型必定存在 |
| `canvas.toBlob` | 產生 `image/png` | 1040×624 合成畫面為 48,906 bytes；不能當成其他畫面的大小上限 |

探針不讀取系統剪貼簿，也不連 PTT。結果位於 ignored 的 `target/verification/feature-research`。這些是本機能力觀察，不代替 macOS／Linux、實體 IME 或新功能驗收。

## 文字選取與複製

第一版提供線性拖曳、整行與整頁的純文字複製。保留選取範圍內的空白，以 LF 分隔畫面列；不猜測文章段落或自動接回折行。雙欄中文字只輸出一次，選到任一半時將邊界擴至完整字形。無法映射的格子沿用 Frame 的明確替代字，不悄悄省略。

Rust 負責選取區間正規化與文字抽取，直接消費 Frame 的字形配對。renderer 負責指標座標與選取覆層，使用 draw 已計算的同一組格線。CSS 座標先按 canvas 的實際 bounding rect 換算為 backing-store 像素，再查找格線邊界；不能只除以字型大小或假定 DPR 等於縮放比例。黑邊不算文字，捲軸保留原操作。

選取請求帶 generation、revision 與範圍；Rust 需保留一份正在顯示的 Frame，不能直接以較新的 watch 值代替。第一版在顯示新內容版本或重連時清除選取，縮放則保留邏輯範圍。這樣可以避免複製到與畫面不同的資料，也不需要凍結連線或保存歷史頁面。

剪貼簿的首選驗證路線是使用者觸發的 `copy`／`paste` 事件。`copy` handler 必須同步寫入 `clipboardData`，所以 Rust 抽取出的文字要先以版本及範圍識別快取於 renderer；文字尚未準備好時顯示狀態，不能假裝複製成功。標準說明事件內的資料存取方式，以及合成事件不會實際修改系統剪貼簿。[Clipboard Event API](https://www.w3.org/TR/clipboard-apis/#clipboard-event-api)

先用 Ctrl+C／macOS Cmd+C 與實際跨應用貼上驗證這條路線，再決定工具列「複製」按鈕如何接入。不要依賴未提供的非同步 API，也不預先疊多套自動 fallback。若需要原生剪貼簿，`arboard` 3.6.1 是候選而非目前依賴：Windows 操作應序列化；Linux 需維持 Clipboard owner，且純 Wayland 支援取決於額外 feature 與 compositor 協定，不能直接宣稱三平台完成。[arboard Clipboard](https://docs.rs/arboard/3.6.1/arboard/struct.Clipboard.html)、[Linux backend 說明](https://github.com/1Password/arboard/tree/v3.6.1#backend-support)

拖曳使用 pointer capture，並處理 pointerup、pointercancel 與 lostpointercapture，避免滑鼠離開視窗後仍停留在拖曳狀態。[Pointer Events](https://www.w3.org/TR/pointerevents3/#the-setpointercapture-method)

純文字不保留左右色彩；帶色可編輯格式另行設計，不能從 Unicode 文字反推原始 Big5 bytes。保留外觀的需求先由 PNG 滿足。

## 中文輸入與貼上

### IME 組字與游標

目前 textarea 固定在 surface 左上角、1×1 且透明，compositionstart／end 只處理送出文字。下一版應將輸入宿主移到 Frame 游標對應的 CSS 座標，組字時顯示預編輯文字；候選窗的位置交由平台 IME 決定並實測。游標、縮放、捲動與視窗尺寸改變都必須更新位置，組字中不反覆切換焦點。

組字內容只留在輸入宿主，確認後才經 `encoding::encode` 送出一次；取消時不送。用明確的組字狀態與 inputType 處理輸入，不單憑「下一個字串和上一個相同」判定重複，否則可能吃掉連續相同文字。確認候選字的 Enter 不額外當成 PTT 的 Enter；完成組字後的下一次 Enter 才送 CR。組字開始綁定 generation，重連後不把舊組字送進新 session。[UI Events composition events](https://www.w3.org/TR/uievents/#events-compositionevents)、[Input Events Level 2](https://www.w3.org/TR/input-events-2/#input-event-order-during-composition)

驗收需涵蓋 Windows 注音／倉頡或使用者常用 IME、macOS 輸入法及 Linux IBus／Fcitx 的指定環境。CDP 可以測事件路徑，不能證明原生候選窗的位置或平台輸入法操作。

### 剪貼簿文字送出

目前 Ctrl＋英文字母會轉成控制碼；Ctrl+C 是 0x03，Ctrl+V 是 0x16，paste 則一律被阻擋。新增功能必須先分流本機快捷鍵，再處理一般 PTT 控制鍵：有選取時 Ctrl+C 複製，沒有選取時保留原本 0x03；Ctrl+V／Cmd+V 由貼上流程接手，不把 0x16 額外送到 PTT。預覽對話框、工具列與輸入宿主也要有各自的鍵盤焦點範圍。

從 paste 事件只取 `text/plain`，避免 HTML 與文字各送一次。Rust 在同一邊界正規化 CRLF／LF／CR，完成整段 Big5-UAO 編碼與容量檢查後才排入 Connection。第一版維持目前 4,096 個輸入 bytes 的單筆限制；限制以編碼後的 bytes 計算，不能只看 UTF-8 字串長度。超限或存在無法編碼的字元時整段不送，不先切開送出前半段。

單行貼上不附加 Enter；多行顯示預覽與行數，由使用者送出或取消，預覽與實際送出的換行一致。預覽遇到重連就失效。一般貼上拒絕 ESC、NUL 與不支援的控制字元，明確說明原因；ANSI 作品編輯／貼上另列功能，不混進首次文字貼上。

## 網址操作

在 Rust 由 Frame 的完整字形建立每列文字及字元到 cell 的映射，找出候選網址，再用 URL parser 驗證，回傳 URL 與對應矩形。滑鼠拖曳優先於點擊開啟；只在同一畫面版本、沒有形成選取的 click 上執行。renderer 使用結構化資料畫底線或提示，不把文章文字插成 HTML。

第一版只處理同一畫面列內完整的 HTTP／HTTPS URL，提供開啟與複製。尾端標點要有明確案例，不能一律刪掉所有括號。Frame 沒有保留每列是自動折行還是原始換行的資訊，因此先不跨列拼接網址，也不宣稱能還原所有長網址。URL parser 規則參考 [URL Standard](https://url.spec.whatwg.org/)；實作仍應選一個 Rust parser 作唯一擁有者。

Dioxus Desktop 0.7.10 的 navigation handler 對 HTTP／HTTPS／mailto 已直接呼叫系統瀏覽器，之後才走自訂 handler；不能假定自訂 handler 會先攔到所有外連。建議由 Rust 的明確 OpenUrl 動作驗證 HTTP／HTTPS 後呼叫 `webbrowser::open`，將開啟失敗回報 UI。`webbrowser` 1.2.4 已在 lockfile，但產品直接使用時仍須列為直接依賴。[鎖定版本 navigation 實作](https://docs.rs/crate/dioxus-desktop/0.7.10/source/src/webview.rs)

圖片預覽需要額外的下載、格式解碼、快取與資源生命週期，獨立於網址開啟。先讓圖片網址可用系統瀏覽器開啟。

## 字型與顯示設定

第一版提供英數與中文的預設字型組合、手動 family 名稱、字形大小／baseline 校準、恢復預設，以及包含中英混排、左右兩色、框線與方塊的預覽。固定 cell 比例與 80×24 排版保留；字形設定不改寫站端 ANSI 色彩，幾何方塊也不改回依賴字型。

目前 renderer 將每個字形的自然 advance 縮放到 cell 寬，字型替換仍需檢查高度、baseline、裁切與 fallback。沿用 `document.fonts.load`／`ready` 等待字型後重畫，但 `fonts.check` 成功不能獨自證明沒有 fallback。暫不依賴列舉本機所有字型的瀏覽器 API。[CSS Font Loading](https://drafts.csswg.org/css-font-loading/#fontfaceset-interface)

Rust 唯一持有有版本的設定值，驗證後送給 renderer；一般縮放仍是既有畫面狀態，避免 Rust 與 JS 各存一份會互相覆寫的字型設定。持久化使用 OS 使用者設定目錄，可評估 `directories::ProjectDirs`；同目錄暫存後替換並驗證平台行為，失敗時保留前一份設定，UI 明示未保存。設定只含顯示偏好，不含帳密或畫面文字。[ProjectDirs](https://docs.rs/directories/6.0.0/directories/struct.ProjectDirs.html)

## ANSI Art 截圖

第一版匯出目前完整 80×24 畫面的 PNG，包含黑色背景與左右色彩，不包含工具列、選取覆層、IME 預編輯文字或本機游標覆層。預設用邏輯尺寸 960×576，與視窗目前放大、捲動或被遮住的區域無關；更高輸出倍率可另行加入。

先將目前依賴全域 canvas／ctx 的 draw 收斂成同一個可指定 target 的 renderer，再用同一份 Frame、字型與幾何規則畫到離屏 canvas。匯出過程不改變可見畫面的 canvas 尺寸、zoom 或鍵盤焦點。待字型就緒後，以 `toBlob` 取得 PNG；標準支援 PNG 序列化。[Canvas 匯出](https://html.spec.whatwg.org/multipage/canvas.html#dom-canvas-toblob-dev)

原生另存介面可用 lockfile 已有的 `rfd` 0.17.2，但直接使用時需列為直接依賴。優先評估 AsyncFileDialog，取消另存是正常結果；副檔名 filter 在各平台不保證相同行為，實際檔名與 PNG 格式仍須驗證。[rfd FileDialog](https://docs.rs/rfd/0.17.2/rfd/struct.FileDialog.html)

不要直接搬用測試探針的 4 MiB base64 限制。既有最大 canvas 為 16M pixels，原始 RGBA 就是 64 MiB；PNG、base64、JSON 與字串傳輸還會增加記憶體。預設 960×576 的 RGBA 約 2.11 MiB，這是尺寸算式，不是峰值量測。實作應在編碼與 IPC 配置前建立大小預算，只允許一個匯出工作，取消或關窗後釋放 Blob、暫存資料與檔案工作。

## 實作順序與驗收

以下是依賴順序與相對複雜度，並非工時承諾。每個可驗收目標各自 commit。

| 順序 | 功能 | 相對複雜度 | 關鍵驗收 |
| --- | --- | --- | --- |
| 1 | 選取與純文字複製 | 中 | 雙色字只複製一次；反向拖曳、行邊界、空白、縮放／捲動正確；更新畫面不複製錯版本；跨應用貼上文字一致 |
| 2 | IME 輸入宿主與事件狀態 | 高 | 原生候選窗位置、確認／取消、相同字連續輸入、失焦、重連、Enter 不重送；指定平台實機驗收 |
| 3 | 單行貼上與多行預覽 | 中 | Ctrl+V 不附加控制碼；不重複貼上；換行正確；取消／編碼失敗／超限時站端收到零 bytes；預覽不跨 session |
| 4 | 網址開啟與複製 | 中 | 網址與 cell 區域一致；拖曳不誤開；標點、非 HTTP scheme、舊畫面與開啟失敗有案例 |
| 5 | 整頁 PNG | 中 | 匯出完整頁且不改可見畫面；雙色／明暗／方塊一致；另存取消、寫入失敗、大小上限與關窗清理 |
| 6 | 字型設定與保存 | 中高 | 換字型不重排或切字；缺字 fallback、設定損壞／保存失敗、跨 DPI 與重新啟動驗收 |

選取、貼上編碼與 URL 映射先用純 Rust 合成案例驗證，再接同一生產 Connection／renderer 的 mounted probe。系統剪貼簿、原生 IME、另存視窗與各平台 backend 需要額外實際操作；現有合成事件不能代替它們。研究階段沒有新增產品依賴或功能程式。

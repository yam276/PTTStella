# PTTStella 最簡 terminal 計畫

狀態：**畫面核心、Windows 視窗與 WSS 登入前連線已驗證；macOS 專用帳號已觀察手動登入與部分實站瀏覽，平台與案例範圍分列如下**。第一個驗收平台為 Windows；Rust／Dioxus 維持可攜，macOS／Linux 後續各自驗證。

## 目標

目前交付為 `dist/macos-list-edges/PTTStella.app`（2026-10-22，macOS arm64 release、ad-hoc 簽章）：網頁文章自動載入全文，支援取消／繼續，終端機仍逐頁操作；保留中段頁尾辨識修復、固定的 PTT 資訊列（網頁百分比隨閱讀位置更新）及 Vim「實驗中／experimental」標示。既有回歸版修復短文章格線、快取頁尾連結及清除閱讀投影後舊版本仍可使用的問題。包含先前文章模式、滑鼠提示、黑白名單、連續 ID 合併及快速名單／暫時篩選。快速名單已支援右鍵移除，暫時篩選改為推文底色標示；相關原生正文手勢仍待驗收；正文滑鼠由使用者手動驗收，其他平台未以 macOS 結果代替。

開啟視窗、自動連上 PTT、手動輸入帳號密碼、看見登入後畫面，最後可正常斷線。文字、固定排版與雙色字由核心與 renderer 實作。

使用者已明確補充：只有外觀像 terminal，並非真實系統終端。本產品不開 shell／PTY，不追求通用 terminal 相容。

初期 terminal surface 已擴充本機操作功能、依 [導覽契約](../navigation.md) 的最小滑鼠導覽，以及目前畫面連續行 PNG 截圖。本階段依 [媒體 MVP 規格](../media.md) 加入按需媒體預覽。後續依使用者授權加入有界的 [文章閱讀快取](../article-reading.md)，沒有啟動時自動登入或背景追蹤；後續依使用者明確授權加入 [按鈕登入與鑰匙圈保存](../login.md)，以及下列有界快速切板計畫。

## 0.1.0 Windows 原生建置檢查修復（2026-10-22）

repo 改為 Public 後，GitHub 標準 runner 已實際啟動，Release PR policy 通過；先前付款／spending limit 的拒絕不再阻擋本次 run。第一輪 Windows 原生 Inno Setup 發現共用 Code include 的首行使用分號註解，已改為 Pascal 區塊註解；後續 Windows runner 的 11 項原生前置政策 probe 已通過，仍使用合成 registry／process adapter，不冒充真實 Runtime 安裝驗收。

後續 Windows desktop bin 的 overdue_cleanup 測試在 130 項通過、1 項失敗時抓到測試自身的計時競態：10ms timer 與零期限通知在同一 Windows timer tick 可同時就緒，不能用固定睡眠證明通知已發出。改以獨立 oneshot 接收真實 overdue 通知，再確認 cleanup 的 ack sender 尚未關閉，送 ack 後必須收到 joined；保留提前結束失敗與有界等待，不調整生產 retained_wait 或清理期限。macOS 本機 131 項 bin、全 targets Clippy、fmt／docs 通過；Windows 最終結果仍以新 run 為準。

Windows 隨後已通過全部 Rust／JS 測試，原生打包抓到 Inno 7 的 help banner 只寫主版本 7，無法用 /? 檢查完整 7.1.0。依上游 is-7_1_0 的 ISCC.dpr，改用正式 --version 輸出的 engine version 並精確比較，仍拒絕錯版、相鄰版本及空輸出；不放寬下載 SHA-256 或 WebView2 簽章驗證。Python tooling 71、docs 與 actionlint 通過，原生結果以後續完整 verify 為準。

證據 .local/verification/release-v0.1.0（windows-verify-attempt2.log、windows-verify-4d.log、lifecycle-gate.json、windows-9f.log、ISCC-7.1.0.dpr）；0.1.0 發佈與四架構最終建置仍進行中，尚未建立 tag 或 Release。

## 列表與選單可視區首尾 Home／End（2026-10-22）

依使用者要求，已辨識的主選單、系統子選單、看板／最愛／分類、文章列表、信件／精華列表與指令說明頁，內容區頂端／底端的一行範圍提供 Home／End，排除底部狀態列。與閱讀共用同一幾何 owner、16～32 CSS px 窄區及帶橫線箭頭，沒有新增覆蓋層或常駐提示。中間仍依已辨識列進入；網址與左緣返回優先，未知／空列表不提供通用 fallback。pointer 的可視幾何快照擴及這些 profile，縮放／捲動／profile 改變會撤銷舊點擊；既有 Shift、拖曳、來源版本及站端滑鼠拒絕規則保留。

core 13 命令／2 replay、desktop lib 144／bin 131、全 targets Clippy、JS 235、Python tooling 71、fmt／docs 通過。新增生產 JS pointerdown／up／click 測試核對七種 profile 的頂底只送一次 First／Last、底列拒絕、一般列仍 Open、實際可視區縮放／捲動、Back／連結優先、Shift／拖曳／過期／外站路由拒絕與游標不跳提示。Production Connection loopback 對主選單、看板與文章列表逐一核對精確 ESC[H／ESC[F，拒絕重複舊 stamp，等新 frame 才接受下一次；Close acknowledged。

最終 macOS arm64 Dioxus／WKWebView 重新跑 wide_last_row：等寬後末行仍可見，重複 End／PageDown 不移動，來源 PNG 與修改前及系統剪貼簿 RGBA 完全一致，1040×768 固定視窗、一筆 HTTP、同一 batch、全部 owner／thread join、exit 0。交付與測試副本 15 份 Mach-O loadable sections 一致、strict ad-hoc 驗章通過。此 native smoke 未點列表首尾；該原生座標手勢依既有約定由使用者手動驗收，Windows／Linux 原生 UI not_ready，不能以 mock 或 loopback 代替。

證據 .local/verification/list-edges（delivery-gate.json、native、validation.json）；交付 dist/macos-list-edges/PTTStella.app（0.1.0、macOS arm64 release／ad-hoc，未公證）。GitHub 完整發佈建置需包含這次變更後重新執行。

## 等寬圖片後末行仍可到達（2026-10-22）

發佈前使用者回報等寬圖片後，End 顯示 100% 仍漏掉整篇最後一行，已暫緩合併 Release PR。原因為圖片比例產生小數 CSS px 文件高度，原生捲動上限取整後比 renderer 的末端略短，完整行判定因此隱藏最後一整行。只將超出原格線的整篇高度向外取整，增加不足一 CSS px；短文最小格線、圖片實際尺寸、逐行裁切／滾輪規則及固定頁尾不改，不新增空白文字列。

先以生產 layout／readingMove 搭配原生整數捲動上限重現失敗，再涵蓋 DPR 1／1.5／2、三種字高及三種圖片高度共 27 組獨立 expected；每組經真正 Enter 等寬後按 End，最後來源行仍完整可見，重複 PageDown／End／滾輪不得繼續多捲，末端向外取整小於一 CSS px。core 13 命令／2 replay、desktop lib 143／bin 131、全 targets Clippy、JS 231、Python tooling 71、fmt／docs 通過。

macOS arm64 wide_last_row 原生對照使用同一來源與 64×33 紅藍 PNG。修改前等寬＋End 的末行色塊／文字消失；修改後 LAST ARTICLE LINE MUST REMAIN VISIBLE 與完整末行色塊可見，重複 End／PageDown 區域像素不再移動。兩個程序均精確一筆 GET、同一 batch，Close acknowledged、全部 owner／HTTP thread join、exit 0。修改前後的 960×576 來源 PNG 及修正後系統剪貼簿讀回 RGBA 逐像素相同；視窗固定 1040×768。各交付與原生副本的 15 份 Mach-O loadable sections 一致、strict ad-hoc 驗章通過。Windows／Linux 原生 UI 本輪 not_ready；GitHub 發佈仍以後續完整建置為準。

證據 .local/verification/wide-last-row（reproduction.log、delivery-gate.json、before、after、validation.json）；交付 dist/macos-wide-last-row/PTTStella.app（0.1.0、macOS arm64 release／ad-hoc，未公證）。

## 0.1.0 正式版準備（2026-10-22）

依使用者要求將目前功能固定為 0.1.0，Cargo.toml／Cargo.lock、版本及正式版標章一致；About 與各平台打包版本沿實際 Cargo package metadata。README 改為一般使用者的安裝／操作說明，移除開發與驗證文件章節及內部契約入口。簡短中文為主、英文摘要為輔的發佈內容保存於 docs/release-notes/v0.1.0.md；publisher 使用版本專用說明時不另追加自動 changelog，仍維持四架構來源／完整資產／checksum 白名單與 draft 全上傳才公開的要求。

core 13 命令／2 replay、desktop lib 143／bin 130、全 targets Clippy、JS 230、Python tooling 71、fmt／docs 通過；Actionlint 1.7.12 驗證現有 release／release-pr workflows 通過。新增 publication 案例確認中文與英文 notes 逐字送到 draft create 且不附 generate-notes。Cargo offline workspace update 只有本產品版本變更。macOS arm64 release App 的 CFBundleShortVersionString／CFBundleVersion 均為 0.1.0，strict ad-hoc 驗章通過；本輪版本／文件變更未重做正文手勢人工驗收。證據 .local/verification/release-v0.1.0；交付 dist/macos-v0.1.0/PTTStella.app。

本提交為 develop → main Release PR 的待發佈來源，GitHub 四架構建置與正式 Release 需以後續實際 run 結果為準。發佈成功後將 main 回併 develop，再將日常開發提高為 0.2.0-dev.1；不提前移動或手動建立發佈 tag。

## 正文與推文圖片箭頭收在網址內（2026-10-22）

圖片控制項統一靠齊 Rust 已辨識連結的右端、在該文字行垂直置中；方框寬度不超出該段連結，短 OSC 8 標籤的箭頭亦隨尺寸縮小。正文與標準推文共用此路徑，保留單張受信任連結索引／作用域驗證。藍金方框改用 75～85% 不透明度呼吸，hover／鍵盤 focus 恢復清楚顯示，減少動態效果仍停用動畫。移除原本掃描右側佔用文字、滿行時插入一行放按鈕的替代路徑；控制項只疊在網址末端，推文時間欄、緊鄰文字及下一行不移動。網址去重、下載預算、Enter 開啟順序、已開圖片與高亮保留規則不變。

core 13 命令／2 replay、desktop lib 143／bin 130、全 targets Clippy、JS 230、Python tooling 70、fmt／docs 通過。JS 獨立 expected 核對滿行、推文右側中文字／時間、同列多條連結、單欄 OSC 8 標籤與 DPR 1／1.5／2 的精確方框邊界及行號映射，按鈕只送原 Rust 索引；既有 Enter、完整行、GIF／高亮與輸出回歸全數重跑。新增 inline_replies 本機合成探針，Rust 確認標準推文 ANSI、Alice 作者、兩個圖片連結欄位與固定時間欄，避免用普通引用文字冒充推文。初輪 fixture 欄位長度算錯及缺標準推文 ANSI，測試確實拒絕；依自製字串長度與既有推文格式修正 fixture，失敗 log 保留。

macOS arm64 最終 Dioxus／WKWebView 畫面確認兩個方框都在網址末端內，正文 AFTER、推文作者／時間及緊鄰下一行完整，沒有多插空白行。經原生可及性按鈕操作，正文及推文各送一次指定 Link(0)／Link(1)，HTTP 精確為 colours.png 與 missing.png 的 404，同一 batch；50ms Enter 等寬正常。固定 1040×768，960×576 來源 PNG 與系統剪貼簿讀回逐像素相同。Close acknowledged、全部 owner／HTTP thread join、程序 exit 0；15 份 Mach-O loadable sections 與交付一致、strict ad-hoc 驗章通過。此輪驗證按鈕可及性操作，正文座標滑鼠手勢仍依既有約定由使用者手動驗收；Windows／Linux 原生 UI not_ready。

證據 .local/verification/reply-image-controls（delivery-gate.json、final-gate.json、initial／initial-r2、native-initial、native、validation.json）；最終交付 dist/macos-reply-image-controls-final/PTTStella.app（macOS arm64 release／ad-hoc，未公證）。

## YouTube 連結直接開啟瀏覽器（2026-10-22）

使用者回報影片點開只有框內縮圖。YouTube 網頁連結改走既有 OpenLink 外部瀏覽器路徑，原網址與時間／查詢／片段參數不重建；移除 YouTube 專用 target、縮圖下載及介面翻譯分支。點擊仍核對受信任事件、來源版本與投影。直接 MP4／WebM／OGV 的原生 video controls、preload none、codec 失敗與外部開啟後備保留。此輪沒有加入 YouTube IFrame 播放器。

core 13 命令／2 replay、desktop lib 143／bin 129、全 targets Clippy、JS 229、Python tooling 70、fmt／docs 通過。Rust 案例確認 YouTube watch／shorts／live／短網址不產生縮圖 target，原網址保持不變；既有直接圖片／影片、網址安全及版本拒絕案例重跑。Windows desktop_media 探針更新為檢查 YouTube 直達 native external sink，沒有縮圖 HTTP 或 preview 節點，並保留圖片外開與影片播放驗證；本輪無 Windows runner，這個原生探針列 not_ready，不能以 Python 單元測試代替。

macOS arm64 原生 inline smoke passed：兩次 Enter 精確取得 colours.png 與 missing.png 的 404，第三次等寬成功，視窗固定 1040×768。來源 PNG 960×576 與系統剪貼簿讀回逐像素相同；Close acknowledged、全部 owner／HTTP thread join、程序 exit 0，15 份 Mach-O loadable sections 與交付一致、strict ad-hoc 驗章通過。此 smoke 未點 YouTube，實際系統瀏覽器啟動仍留人工驗收；不以圖片 smoke 宣稱影片播放或外開實測通過。Windows／Linux 原生 UI 本輪 not_ready。

證據 .local/verification/youtube-external（delivery-gate.json、native、validation.json）；交付 dist/macos-youtube-external/PTTStella.app（macOS arm64 release／ad-hoc，未公證）。

## 推文高亮保留展圖與閱讀位置（2026-10-22）

使用者回報點 ID 高亮使全部圖片摺疊，文章隨高度縮短跳動。原因為 ReplyFocus 成功分支沿用了永久名單變更的清理行為，主動 close InlineImages 並送 InlineReset。移除這兩個動作；推文高亮只更新背景投影版本，原 article ID／batch／resource keys 保留，新 Paint ACK 後沿原 follow 管線更新。暫時媒體預覽及 hover 租約仍關閉，舊選取／輸出租約照常撤銷。永久名單更新、切篇、離開閱讀、斷線／新 session 的清理分支不變。

core 13 命令／2 replay、desktop lib 143／bin 129、全 targets Clippy、JS 229、Python tooling 70、fmt／docs 通過。Rust 使用生產 SurfaceState 與 InlineImages 串接高亮 Alice、改選 Bob、取消高亮，逐次核對相同 article／batch／entries keys、舊輸出租約無效、新 session 仍清理。JS 生產 inline owner 另外覆蓋新 Paint 尚未 ACK 與 follow 重投影兩個階段，保留解碼 canvas、GIF Blob、等寬、失敗訊息、文件高度與 scrollTop，沒有新增下載／解碼或順便開其他圖；切篇釋放資源。既有完整行顯示、開圖順序與原生操作分流測試全數重跑。

macOS arm64 原生 inline smoke passed：兩次 Enter 精確取得 colours.png 與 missing.png 的 404，同一 batch；第三次等寬成功，視窗固定 1040×768。來源 PNG 960×576 與系統剪貼簿讀回逐像素相同；Close acknowledged、全部 owner／HTTP thread join、程序 exit 0，15 份 Mach-O loadable sections 與交付一致、strict ad-hoc 驗章通過。此 native smoke 的 reply_focus_actions 為 0，不當作點作者高亮的原生回歸驗收；該手勢依既有約定留人工驗收。Windows／Linux 原生 UI 本輪 not_ready。

證據 .local/verification/highlight-images（delivery-gate.json、native、validation.json）；交付 dist/macos-highlight-images/PTTStella.app（macOS arm64 release／ad-hoc，未公證）。

## 非滾輪導覽維持完整文字行（2026-10-22）

網頁文章原本以可視高度 90% 的像素量翻頁，會落在半行。現在鍵盤／Vim、按鈕與正文左鍵移動透過同一 row layout 對齊文字行；圖片間隙仍不取得 terminal cell。PageDown 往原可視區取整、PageUp 反向取整以保留重疊，方向鍵從當前行界移動。末頁若直接對齊實際底部會略過一行，先顯示該行起點，再前進到實際底部。Home／End 保留真正首尾，沒有增加文件空白或改變固定 BBS 視窗。

分數縮放、圖片間隙與末頁無法同時對齊上下邊緣時，只繪製完整文字列，暫不呈現的邊緣列與其展圖控制項下次仍可完整讀到；原資料、選取／複製及來源 PNG 不刪行。被遮住的邊緣區可返回／翻頁／Home／End，不命中隱藏的文字網址或 ID。滾輪保留連續像素位移及原生裁切；下一個離散導覽恢復完整行。模式由 inline layout 擁有，新文章／清理重設；沒有額外 timer、全篇 canvas 或第二份文字解析。

core 13 命令／2 replay、desktop lib 143／bin 128、全 targets Clippy、JS 228、Python tooling 70、fmt／docs 通過。新增 expected 覆蓋完整行上下界、行界落點／來源偏移、浮點滾輪（含 0.25px）與恢復、Home／End／首尾黑邊導覽、圖片間隙、DPR 1／1.5／2 及多種字高。逐頁遍歷不同高度／字高，獨立核對 40 個來源行全部曾完整出現；這個檢查曾抓出分數末頁漏看一行，修正後才通過。初輪 probe 預期把 UAO 雙欄實心塊當單欄、JS double 未完整提供 reading owner，保留失敗 log；沒有藉改產品行寬或放寬完整行驗證結案。

新增 reading_rows 本機合成探針，23 個編號列加紅／綠滿字格色塊，peer 拒絕所有站端輸入。最終 macOS arm64 Dioxus／WKWebView 在 100% 顯示 23 個完整色塊，125% 的 Home、PageDown、End、PageUp 各顯示 18 個完整色塊；末頁最後紅色列可見，沒有半高邊緣色塊。工具收合後曾未取得正文焦點，改用可及性聚焦輸入後重做鍵盤與截圖，不將無效按鍵當通過。視窗固定 1040×768。來源 PNG 960×576 仍有全部 23 行，系統剪貼簿讀回與存檔 RGBA 完全一致；Close acknowledged、全部 owner／HTTP worker join、exit 0，15 份 Mach-O loadable sections 與交付一致、strict ad-hoc 驗章通過。

原生滾輪／正文點擊依既有界線留給使用者手動驗收，Windows／Linux 原生 UI 本輪 not_ready。證據 .local/verification/row-alignment（delivery-gate.json 的 core／lib、final-gate-r2.json、native、validation.json；中途失敗保留）。交付 dist/macos-row-alignment/PTTStella.app（macOS arm64 release／ad-hoc，未公證）。

## 開圖順序、首尾點擊與推文作者命中（2026-10-22）

短按 Enter 改為可見範圍內由上到下優先開啟未開的圖，全部開啟後才依相同順序等寬；同列按連結欄位順序。移除舊的最近中央與第二次 Enter 指定目標／pendingWide 路徑，不保留舊目標優先權。尚未 ACK 時再按仍請求同一圖片，由既有 Rust batch 去重，不提早放大。長按全開、取消／切篇／失焦與捲動撤銷 held timer、GIF 可見播放預算及 Blob 清理保持同一 owner。

可見正文最頂與最底的一行範圍（16～32 CSS px，最多四分之一高度）左鍵改為 Home／End，游標為帶橫線的金色上下箭頭；其餘上半／下半仍翻頁，左緣仍返回。展圖方框內使用同一物理可視區導覽，不虛構 terminal cell；拖曳圖片取消點擊且不產生文字選取。圖片右鍵仍走原連結情境選單。來源 stamp、投影、捲動／視窗幾何及圖片索引改變都撤銷舊點擊；終端機使用既有 First／Last intent，網頁使用既有本機首尾移動，不觸發換篇。

左鍵推文高亮僅命中仍可見的作者 ID 字元，冒號／空白／內容／時間回復一般導覽，網址仍優先。Rust 用已辨識推文及可見 account words 產生作者索引，UI 只核對行／欄範圍，不解析文章或猜 ID。合併與整行遮蔽的隱藏作者不出現在左鍵目標，右鍵推文及清除高亮維持原規則。

core 13 命令／2 replay、desktop lib 143／bin 127、全 targets Clippy、JS 222、Python tooling 70、fmt／docs 通過。新增獨立 expected 覆蓋開啟與等寬的上到下順序、未開優先／不預先放大、非陣列順序與可視範圍；首尾窄區、原分頁與左緣優先、圖片間隙不選字、拖曳／Shift／過期幾何／不同圖片／站端路由拒絕；作者 ID 邊界、內容中同名詞、不完整推文、合併與整行黑名單不洩露命中。既有右鍵連結、GIF、PNG、全文及文章切換測試亦通過。初輪 JS double 缺 pointerHit 及開啟後 layout，修正測試環境後重跑全套，失敗 log 保留。

macOS 原生 short_links 探針 passed：兩次 50ms Enter 依文章順序先 Link(1) 一般網址、再 Link(0) OSC 8，精確 GET colours.png／colours-two.png 各一次，同一 batch；前兩次截圖仍為兩張 64×32，第三次才把上方圖放成 1040px 等寬，後續按鍵沒有額外 HTTP。視窗始終 1040×768。來源 PNG 960×576 不含圖片與控制項，系統圖片剪貼簿讀回與存檔逐像素相同；Close acknowledged、全部 owner／HTTP thread join、exit 0，15 份 Mach-O loadable sections 與交付一致、strict ad-hoc 驗章通過。

正文原生座標點擊（圖片返回／上下與首尾、作者 ID 高亮）依使用者先前約定保留人工驗收；JS 生產 handler 與 Rust 資料驗證不代替該實體操作。Windows／Linux 原生 UI 本輪 not_ready。證據 .local/verification/image-navigation（delivery-gate.json、native、validation.json）；交付 dist/macos-image-navigation/PTTStella.app（macOS arm64 release／ad-hoc，未公證）。

## 設定 Esc、關於資訊、GIF 播放與圖片剪貼簿（2026-10-22）

macOS 設定開啟時啟用原生 Escape menu accelerator，先消費關鍵事件，再由既有設定 owner 執行保存／關閉；不退出全螢幕。WebView capture handler 同時支援其他平台，組字與 repeat 不重複關閉，保存失敗保持開啟。選單翻譯更新保留設定可見狀態，surface 結束解除捷徑。「關於與授權」顯示 Cargo 的作者、實際編譯版本及固定 repo 網址；按鈕沿受信任事件開啟瀏覽器，未提高產品版本。

完整預覽及文章展圖的 GIF 交由 WebView 原生 img 解碼播放，保留首幀 Canvas；hover 仍是首幀。共用播放 owner 管理可見區、失焦、隱藏、使用者暫停與清理；文章內最多兩張、完整預覽最多一張，按距可視區中心排序，另有 64M decoded frame pixels 的播放總量上限。Rust 既有格式／下載／幀數限制先驗證並提供 frame-pixel 成本；切換與取消移除 img src，再撤銷 Blob。沒有 JavaScript 每幀繪製 loop；此資源上限不是原生 process RSS 量測。

擷取工具與情境選單新增複製整頁／選取行 PNG，仍保留另存。沿同一 renderer、80 欄及連續行契約，檢查 PNG 實際格式／尺寸／容量後轉 RGBA，經既有 worker 與持有的原生 clipboard owner 寫入。工作前與寫入前核對 output lease，拒絕取消、session／revision／policy 過期結果。只有明確複製動作寫入，正常模式不保存讀回證據；probe 模式才核對系統剪貼簿並寫本機合成圖片證據。

驗證：core 13 命令／2 replay、desktop lib 143／bin 126、全 targets Clippy、JS 219、Python tooling 70、fmt 及 docs 通過。新增 expected 覆蓋設定 Esc／組字／保存失敗、GIF 可見排序／播放成本／失焦／手動暫停／清理、圖片目標與信任邊界、RGB／RGBA 解碼、錯誤尺寸／截斷、取消與舊 session 拒絕寫入。最終 build marker 已核對，未將較早候選的 aria 標示當作最終包。

macOS arm64 原生 Dioxus／WKWebView 全螢幕下按 Esc，設定消失且前後維持 1920×1080；關於文字與 Cargo metadata 一致。候選包原生 GIF 兩個時間點的紅／藍色幀不同，確認實際播放；最終包自動化未取得前景時顯示暫停，保持失焦保護，前景恢復的實際操作仍待人工驗收。最終重新執行的 inline 探針 passed：精確兩筆 HTTP、同一展圖 batch、整頁及 24 行選取的系統剪貼簿讀回 RGBA 與 PNG 存檔逐像素相同（960×576）；單行 RGBA 另由 Rust 案例覆蓋。Close acknowledged、全部 owner join、程序 exit 0；15 份 Mach-O loadable sections 與交付一致，strict ad-hoc 驗章通過。

保留中途失敗：startup menu wrapper 缺漏、JS DOM double 缺 listener、原大型 JSON 巨集遞迴，以及誤將 generic event admission 當作 export freshness 的測試預期，皆修正後重跑。第一輪最終包探針逾時列 failed，不當作通過，已由 native-delivery-r2 完整取代。正文座標滑鼠、其他程式實際貼圖介面、前景切換恢復播放仍留人工驗收；Windows／Linux 原生 UI 本輪 not_ready。證據 .local/verification/release-polish（delivery-gate-r2.json、native、native-delivery-r2、validation.json）。交付 dist/macos-desktop-polish/PTTStella.app（release／arm64／ad-hoc，未公證）。

## 名單右鍵移除、推文底色與文章邊界方向鍵（2026-10-22）

對可見帳號／文章列表作者右鍵，與目前策略相符的加入項改為移出黑名單或白名單，其他策略仍可切換。Rust 只對已 ACK 的 cell 解析 ID，依目前規則核對移除策略；過期版本、不同策略、隱藏 ID 或輸入 epoch 改變皆拒絕，保存成功後才生效。名單格式仍為 v4，備註與個別顏色沿用既有規則。

推文單擊改為標示該 ID 的全部已辨識推文，藍底保留前景色；該行有藍字時改紫底。再點同 ID 或右鍵取消標示。其他人的文字與網址不再遮黑，永久黑名單／合併 ID 的黑色遮罩仍保留。標示套用於快取後，不丟失當篇歷史；來源／歷史 PNG 使用相同背景，文字與連結內容不改變，離篇與版本清理沿用原本契約。行號預先建立集合，避免長文 glyphs 與推文做全乘積掃描。

新增「圖片與媒體 → 文章首尾以方向鍵切換上一篇／下一篇」，預設關閉，缺欄位的舊設定亦關閉；保存／關閉快照／重啟與分頁恢復預設皆支援。網頁模式依實際可視區首尾，僅獨立 ↑／↓ 可換篇，PageUp／PageDown、j／k、Home／End、滾輪及按鈕不換篇，長按 repeat 不跨篇。全文讀取中拒絕切篇，其他狀態仍要求該邊界已取得。Home＋↑／End＋↓ 由同一命令 owner 先同步站端邊界再切篇；重新驗設定、作用域、stamp／input epoch／ACK／本機路由，拒絕時回傳結果解除 UI 等待。終端機只攔截已辨識文章邊界的普通上下鍵；設定關閉或 repeat 時不送鍵。切篇清除舊文章／媒體，保留取消及晚到結果隔離。

驗證：core 13 命令／2 replay、desktop lib 143／bin 123、全 targets Clippy、JS 211、Python tooling 70、fmt 與 docs 通過。新增獨立 expected 覆蓋移除與策略不符／舊版本／未保存保留生效規則、標示文字／網址與永久遮罩不變、跨頁標示／取消／來源投影，以及首尾方向鍵與其他鍵分流、repeat／載入／關閉設定、偏好遷移與失敗保存。新准入案例先抓出缺少 input epoch 比對，補上後才通過；另修正內部 arrow 來源只可轉成封閉 keyboard source，取消載入後的取頁不傳未知來源。中途測試與 lint 失敗保留，不當作 gate 通過。

macOS 原生設定已確認預設關閉、保存啟用與獨立程序讀回；關閉後網頁／終端機的上下鍵、網頁 PageUp／PageDown／Home／End 與長按保持同篇，loopback 零站端輸入。short_reading 來源 PNG、Close acknowledged 與所有 owner join 通過。兩輪加入字母鍵的探針因非預期站端輸入失敗，保留 failed；與上一版獨立程序比對顯示目前輸入環境下 j 進入注音「ㄨ」組字，取消組字後上一版 PNG／Close 正常，不將該次輸入當作英文 j/k 通過。實站換篇、正文右鍵／點 ID 的原生手勢、Windows／Linux 原生 UI 仍待驗收；不以設定 smoke 代替這些項目。

最終交付的原生 short_reading 另一次通過；來源 PNG 960×576 與上一交付對照全部像素相同。15 份 Mach-O loadable sections 與測試副本一致，strict ad-hoc 驗章通過，Close acknowledged、全部 owner join、程序 exit 0。所有合成副本均已退出。

證據 .local/verification/reply-highlight（gate-delivery.json、native-delivery、native-settings-r3、native-baseline、validation.json）；交付 dist/macos-reply-navigation-final/PTTStella.app 為 macOS arm64 release／ad-hoc，未公證。

## 單張展圖、Enter 長按與等寬（2026-10-22）

網頁文章的不同圖片網址旁新增方形向下箭頭，使用與光棒一致的低對比藍金色及減少動態效果設定。網址滿行或後方有文字時改置該行下方靠右，不遮蓋原文；跨行及重複網址沿用 Rust 原去重／落點規則。箭頭原先用文字字元偏向下緣，依使用者回饋改成 CSS 線條圖示，原生快照核對圖示中心與 24px 方框中心同為 (457,150)。

短按 Enter 在放開時開目前可見、最接近中央且未展開的一張；沒有捲動時再短按，將剛開圖片等比例放大到 BBS 文字區完整寬度。捲動後取消第二次目標；若可見圖片都已開啟，選最接近中央、尚未等寬的可用圖片。解碼前可保留等寬意圖，失敗圖片不做無效放大。長按 500ms 只全開一次，keyup 與 repeat 不再觸發其他動作；失焦、隱藏、撤銷、模式或文章作用域變更清除 held timer。一般尺寸仍保留原尺寸／560px 上限，明確等寬才解除這些顯示限制。

Rust 以封閉的 All／Link index 接受請求；單張請求只加入指定圖片，後續 Paint 只重新投影已開的資源。全開可升級同一 batch 並沿用既有新頁擴展，資源數、24 MiB payload／16M pixels、序列下載、取消、晚到拒絕及 Blob 清理不變。UI 只使用 Rust links／image_links 及自身排版幾何，不重解文章或網址。控制項及圖片不進來源 PNG；寬圖只增加文件高度，不放大固定的 BBS 可視區。

core 13 命令／2 replay、desktop lib 143／bin 120、全 targets Clippy、JS 207、Python tooling 70、fmt 與 docs 通過。Rust 新案例覆蓋單張後再加一張／全開、Follow 不偷下載、非法／非圖索引、終端機與新 connection、真實 loopback 只收到指定 GET；JS 覆蓋單張與第二次等寬、500ms timer、keyup／repeat、捲動／全部可見已開的中央選擇、解碼前意圖、作用域清理與不遮字的控制間隙。渲染控制項的空白檢查按行預分組，避免每個連結重新掃整篇 glyphs。

最終 macOS arm64 Dioxus／WKWebView 兩個 inline 原生程序均 passed。第一輪點箭頭只 GET colours.png，50ms Enter 將 64px 圖片放大至 BBS 的 1040px 寬且無額外 HTTP；700ms 長按才 GET missing.png 的 404，target trace 為 Link(0)／All，共一個 batch。第二輪 50ms Enter 先取最接近中央的 Link(1)，Down 捲動後 50ms Enter 再取 Link(0)，再次短按由 64px 放大到 1040px；target trace 只有兩筆 Link，沒有暗中全開。兩輪均零站端 bytes、完整來源 PNG 960×576、Close acknowledged、所有 connection／credential／file／listener／HTTP owner 清理，程序 exit 0。PNG 除自製網址的兩處隨機 port cell 外，與前一候選包逐像素相同，未包含箭頭或展圖；15 份 Mach-O loadable sections 與交付相同，strict ad-hoc 驗章通過。

保留中途失敗：新增 probe 欄位使原大型 JSON 巨集達遞迴限制，改為在外部插入該欄位；第一輪 JS gate 缺少新 held owner double，補齊後重跑。keyboard 舊候選的預設按鍵持續時間產生多筆 All，且舊 UI helper 綁到已退出副本，誤開正常副本的另存視窗；該輪 failed 不放行，已取消另存並正常退出自有副本。最終改用明確短按／長按及顯式傳入 App 的 helper，兩輪獨立通過，沒有將中途報告的 exit 0 當作 passed。

證據 .local/verification/single-images（final-gates、delivery-gates、delivery-native、delivery-motion、validation-delivery.json）；交付 dist/macos-image-actions/PTTStella.app（arm64 release／ad-hoc，未公證）。Windows／Linux 原生 UI、實站與其他正文 pointer 手勢本輪 not_ready，保留既有使用者人工驗收界線。

## 網址普通右鍵優先顯示連結選單（2026-10-22）

右鍵先核對 Rust 連結索引及已展開圖片的命中，再決定是否開浮球。網址、OSC 8 片段與文章內插圖無需 Shift 便提供複製完整網址、開啟連結／圖片預覽與選取完整網址；網址內的英文字串及所屬推文列不當作帳號或只看 ID 候選。已有反白保留複製選取，空白處的右鍵工具及非網址帳號選單仍沿用既有規則。站端滑鼠普通右鍵與 Shift 本機覆寫、trust、版本及忙碌准入不變；未新增第二條複製或開啟管線。

core 13 命令／2 replay、desktop lib 143／bin 116、全 targets Clippy、JS 200、Python tooling 70、fmt 與 docs 通過。新增生產 handler 獨立 expected 覆蓋索引 0／其他文字連結／插圖、反白、網址與帳號重疊、空白處及保存／貼上忙碌。舊「空白處」測試曾錯用連結索引 0，改為真正無連結的 -1，並另驗索引 0 必須走連結選單。

macOS arm64 實際 Dioxus／WKWebView 的 context_menu 探針在預設右鍵工具開啟、無反白／無 Shift 下，對合成 OSC 8 頁尾連結普通右鍵，已出現連結選單。實際複製完整網址、選取完整網址後複製選取，兩者均原生貼回搜尋欄讀到同一 loopback URL；開啟選項成功解碼固定紅／藍圖片。原生 JPEG 快照的圖片色塊核對採 codec rounding 容差，來源 PNG 維持逐 byte 相等的完整行裁切。兩次選單展圖只建立一個 batch；HTTP 精確為預覽一次 colours.png、展圖一次 colours.png 及 missing.png 的 404。

探針 passed，兩次 context copy、全頁 960×576 與選取行 960×24 PNG、零站端 bytes、Close acknowledged、全部 owner／HTTP worker join、exit 0；15 份 Mach-O loadable sections 與交付相同，strict ad-hoc 驗章通過。驗證腳本初次誤將圖片預覽計入外開 sink，依生產分支修正為 0 並另核對實際預覽圖片；原生 JPEG 快照的精確 RGB 假設也改為明示的編碼容差，來源 PNG 檢查未放寬。

正文文字網址的座標右鍵回 noWindowsAvailable，不能以固定頁尾 AX 操作代替該原生手勢；保留使用者人工驗收。Windows／Linux 原生 UI 本輪 not_ready。證據 .local/verification/link-context；交付 dist/macos-link-context/PTTStella.app（arm64 release／ad-hoc，未公證）。

## 浮球選單開啟時保持不透明（2026-10-22）

浮球透明度以工具 owner 的 `.open` 狀態決定：點浮球或普通右鍵開啟工具時為 100%，進入分類及返回分類亦維持 100%；收合後恢復使用者設定的透明度，不再因閒置 hover／焦點而改變。按住浮球時仍保持可見，沿用既有拖曳、右鍵定位、失焦及隱藏清理，沒有第二份 opacity 狀態。

正式 CSS 與正式 floating owner 的 Chromium 合成頁確認設定 20% 時，idle／收合為 0.2，點開／分類／返回／右鍵為 1；設定值全程仍為 0.2。新增 JS 驗證兩種入口、分類返回及失焦收合。macOS arm64 WKWebView 原生也完成普通右鍵、浮球點開及分類返回，黑底相同浮球區域最高 RGB 從閒置 53 到開啟 255，沒有低透明度選單浮球殘留。完整檢查、兩次原生探針、PNG 逐像素不變、正常退出與交付簽章同下節；Windows／Linux 原生 UI 未驗證。證據 .local/verification/list-tabs/opacity.json、validation.json 與 native 圖片；交付 dist/macos-list-tabs/PTTStella.app。

## 黑白名單分頁與排序（2026-10-22）

設定內新增黑名單／白名單分頁，已儲存清單只列實際帳號，順序為 ID、遮蔽策略或生效顏色、備註；移除「新增帳號」虛擬清單項，改用清單外的新增按鈕。黑名單表單只提供遮內容／整行塗黑，白名單表單顯示個別色及共用色。排序按鈕切換 ID 與分類：黑名單按遮蔽策略、白名單按生效 ANSI 色號分組，同組按 ID。分頁切換清空未保存草稿，排序及即時翻譯保留草稿；兩者不寫入名單。匯入預覽沿用同一分頁／排序，仍須明確套用才保存。

core 13 命令／2 replay、desktop lib 143／bin 116、全 targets Clippy、JS 198、Python tooling 70、fmt 與 docs 通過。新增 JS 獨立 expected 覆蓋分組、排序、無虛擬選項、明確保存、匯入取消、shared color 變更與即時翻譯。macOS arm64 實際 Dioxus／WKWebView 核對兩分頁、排序、保留草稿、獨立新增及更新備註；檔案確認排序零寫入，保存保留既有兩種黑名單、個別色與共用色。切英文及新程序重啟後讀回新增帳號、顏色與備註。

兩次 short_reading 原生探針 passed，peer 零站端輸入，Close acknowledged、所有 owner join、程序 exit 0。來源 PNG 960×576 與前一交付全部像素相同，15 份 Mach-O loadable sections 與交付相同，strict ad-hoc 驗章通過。證據 .local/verification/list-tabs；交付 dist/macos-list-tabs/PTTStella.app（arm64 release／ad-hoc，未公證）。Windows／Linux 原生 UI 本輪 not_ready，既有正文滑鼠人工驗收界線保留。

## 精密儀表設定外觀（2026-10-21）

使用者從三種草圖選擇「精密儀表」。統一設定改為 3px 小圓角外框、品牌標頭、平面分類列表與左側金色光棒，表單及按鈕採直角，分區使用低對比細線。右側新增隨分類／搜尋與即時語言更新的標題；保存回饋併入固定頁尾。保留深藍金色、鍵盤焦點、同一表單 ID 與既有保存／取消 owner，未修改 BBS renderer 或站端行為。窄版採雙欄分類與獨立捲動內容，管理頁仍自動展開。

desktop lib 143／bin 116、全 targets Clippy、JS 193、Python tooling 70、fmt 與 docs 通過，分類／搜尋標題另加入獨立 expected。macOS arm64 實際 Dioxus／WKWebView 已觀察矩形下拉選單、平面側欄、內嵌站點／名單／帳號／授權；繁中→英文→繁中立即切換並保存，檔案與前版備份核對語言及未變動的顯示值。英文連續搜尋 sites 顯示搜尋標題，搜尋離開帳號頁清除密碼草稿。兩次 short_reading 原生探針均 passed，正常 Close acknowledged、所有 owner join、程序 exit 0。最終來源 PNG 960×576 與前一交付全部像素一致，15 份 Mach-O loadable sections 與交付相同，strict ad-hoc 驗章通過。

正式 CSS 另以 320／680px Chromium iframe 合成排版檢查標籤換行、控制項與分區；此結果不是原生視窗縮放或 Windows／Linux UI 驗收。其他平台與原生窄視窗本輪 not_ready，既有正文滑鼠人工驗收界線保留。證據 .local/verification/settings-instrument，交付 dist/macos-settings-instrument-final/PTTStella.app（arm64 release／ad-hoc，未公證）。

## 設定分類內嵌與列表作者右鍵（2026-10-21）

依使用者要求，站點、黑白名單、帳號與隱私、關於與授權直接顯示於統一設定的內容欄，點分類或搜尋結果便展開；浮球／情境選單入口共用同一設定 owner。原表單、原子保存及明確送出規則保留，移除原 settings_launch 關閉後轉開對話框管線。內嵌表單不建立第二個 modal；授權文字不用再點 disclosure。離開帳號頁、開始搜尋與關閉設定清除密碼草稿及保存勾選，晚到結果不得重開已關閉表單或搶走隱藏頁焦點。原生驗證另抓到關閉子 dialog 的 WebView 焦點恢復會打斷搜尋；修正後完整連續輸入 sites，搜尋結果直接開站點表單。

文章列表作者欄也可右鍵加入遮內容保留 ID、整行黑名單或白名單，沿用同一保存／投影。Rust 只提供已辨識列表的完整作者欄候選，欄內留白同屬該作者；標頭／日期／標題不推測帳號，整行遮黑不洩露隱藏 ID。重新核對已 ACK 畫面、session、revision、input epoch 及本機路由；閱讀頁的推文暫時篩選不延伸到列表。

core 13 命令／2 replay（filters 19 項）、desktop lib 143／bin 116、全 targets Clippy、JS 193、Python tooling 70、fmt 與 docs 通過。macOS arm64 實際 Dioxus／WKWebView 的四分類自動展開、站點兩次保存、名單三策略保存／重開、密碼清除及搜尋連續輸入已觀察。account_articles 探針 passed，獨立 PNG 960×576 核對紅色保留作者、標題全黑、整行全黑及白名單亮青；原生剪貼簿預覽讀回同一遮罩、取消零站端 bytes。最終交付 short_reading 探針 passed，PNG、Close acknowledged、全部 owner join、exit 0，15 份 Mach-O loadable sections 與交付一致，strict ad-hoc 驗章通過。

作者字格原生座標右鍵回 noWindowsAvailable，account_rule_actions 為 0；上面的表單保存不代替該操作驗收，仍交使用者手動驗收。Windows／Linux 原生 UI 本輪 not_ready。證據 .local/verification/settings-sections 保留初版及兩次搜尋焦點修正過程，focus-final 的首字失焦與未匯出 PNG 失敗紀錄未放行；ready 為最後通過版本。交付 dist/macos-settings-ready/PTTStella.app（release／arm64／ad-hoc，未公證）；前一版名單遮罩修復另已由使用者回報測試成功。

## 長文跨頁前景色重設造成中途停止（2026-10-21）

使用者回報 Equestrian `#1gm7mfaU` 共 38 頁文章：End 僅到已載內容，繼續載入卻卡在中段。實站舊交付從頭自動取頁可穩定停在第 30／31 頁邊界並顯示 Changed；站端第 30 頁 638～660 行的末列為黃色，第 31 頁 660～682 行的相同首列重設為預設灰。文字相同，原重疊比較仍將前景色差異當作文章改變。未把公開文章內容或私人流量保存成 fixture。

修正僅允許往後新頁的第一個重疊畫面列回到預設前景樣式，仍逐字核對文字、欄位、字寬、映射與背景；保留快取中已證實的完整文章配色，來源 Frame／PNG 原樣。其他顏色、背景、文字或第二個重疊列的樣式變動仍拒絕。Changed／Limit 後的 End／邊界翻頁不再跳過缺口送到站端末頁；取消與一般逾時仍可依原契約繼續／手動讀取。重新取得前景或可見狀態時重送閱讀 gate，避免等新 Paint 才恢復。

新增 38 頁／837 行自製 reducer 案例，在舊實作明確於第 31 頁失敗，修正後通過；包含上述四個負例及來源灰色／快取黃色的獨立 expected。desktop bin 110、desktop lib 143、完整 JS 185、全 targets Clippy、fmt 與 release 打包通過。新 `long_load` 原生探針共享原本全文 fixture 產生器與 owned peer，保留原 `full_load` 的取消／繼續要求；本機 probe 頁數上限改為 64，以容納 38 頁測試，產品容量不變。

macOS arm64 實際 Dioxus／WKWebView 完成 38 頁／837 列，peer 精確接受 37 筆 PageDown；載入途中 End 先到已讀末端，取頁仍能繼續，完成後 End 可見第 837 行，Home 回首頁。未展圖前 HTTP 為空，兩次 Enter 一個 batch，兩張圖及 404 各一次 GET；來源 PNG 960×576 不含展圖色塊，Close acknowledged、全部 owner 與 HTTP server joined、exit 0。15 個 Mach-O sections 與交付相同、strict ad-hoc 簽章通過。證據 `.local/verification/long-article/`；使用者完成新版手動登入後，同篇實站網頁模式已完整通過第 31 頁到第 38/38 頁，End 顯示 100%、來源 804～826 行及最後一則推文；Home 1/38、PageDown 2/38、End 38/38 回讀正常。正常新版視窗保留在文章末端供驗收；沒有保存文章全文、流量或帳密。這是一篇實站結果，Windows／Linux 及其他未驗操作保持待驗。

## 來源頁尾配色與文章捲動末端（2026-10-21）

依使用者要求，資訊列配色跟隨目前閱讀的 PTT 來源頁：Rust 保存逐格 ANSI 樣式，包含雙色字左右半、亮色與反白；JS 隨目前頁碼切換同一頁尾 renderer。樣式納入既有 32 MiB 准入估計，來源格線／文字及 PNG 不改寫。

使用者另回報短文章到底仍能捲出大片空白。短文展圖原先把圖片加在完整補齊格線上，造成補列重複延長文件；改為先使用原有空白，超過來源格線才增加高度。裁切溢位、End／翻頁／滾輪與 layout 收回位置共用 renderer 文件高度，資訊列位於實際文件底端或可視區下緣。保留短文最小 80×24 與長文完整末尾。

獨立 expected 先重現每頁配色、短文圖片重複空白及 stale DOM overflow；完整 desktop bin 109、JS 183、全 targets Clippy、fmt、docs 與 macOS release 打包通過。首次 Rust 測試失敗是新 fixture 在加入頁尾前先上色，修正 fixture 順序後通過；沒有只改產品 expected 放行。

最終交付的 macOS arm64 實際 Dioxus／WKWebView 使用兩個本機自製案例。`short_links` 兩張 100×60 圖片展開後，Home／End／向下鍵的正文與底列區域逐像素相同，peer 零站端輸入；背景視窗的原生滾輪呼叫遭 macOS 拒絕，不計為該案例滾輪通過。`short_overlap` 在全螢幕確認 Home 藍底 1/2 → End 青底 2/2 → Home 藍底；展圖後最後兩則推文仍完整可見，原生向下滾輪到底後正文／底列像素不變。首次即時截圖早於非同步文字重繪，使用完成重繪的快照比較，不宣稱逐幀無閃爍。歷史一般網址原生左鍵一次由 sink 收到；peer 僅接受一次 PageDown，兩次 Enter 一個展圖 batch，兩圖與 404 各一次 GET。來源 PNG 均 960×576，末頁仍為來源青底，不含本機展圖。兩案例 `status=passed`、Close acknowledged、所有 owner 與 HTTP server joined、exit 0。

證據位於 `.local/verification/reading-page-colours/`，15 個 Mach-O sections 與交付相同，strict ad-hoc 簽章通過。結果限本機合成 macOS 案例，未據此結清其他原生滑鼠、實站與跨平台項目；上一節的歷史失敗紀錄仍保留。

## 目前閱讀頁數與基本 hover 提示（2026-10-21，原生滑鼠待人工驗收）

網頁模式目前頁數改依保留的 PTT 來源頁碼與各頁畫面起點更新，選擇最接近可視文字起點的來源頁；總頁數沿用站端，展圖不增加 PTT 頁數。折行末頁仍以實際畫面起點保存，回讀已知來源頁不丟失較後頁碼。與百分比共用單列 renderer／bitmap 更新，不改寫站端格線、PNG 或送出多餘操作。終端機維持原始頁碼。

返回、上下翻頁、選單／文章列、進板繼續及推文選取的正文 title 不再提供基本操作說明，保留原游標、反白、光棒與命中規則。網址只顯示完整目的地；浮球只保留無障礙名稱，不再跳拖曳說明。錯誤、忙碌與容量／匯出限制維持原狀。

修改前的 JS 獨立 expected 明確重現仍為末頁及操作提示殘留。修改後 Rust bin 109、全 targets Clippy／fmt、完整 JS 181 與 docs 通過；新增來源頁碼／折行起點／歷史回讀，以及本機頁碼前後／圖片內捲動、基本 title 清空與網址目的地保留。交付 `dist/macos-page-hints/PTTStella.app` 已建立，原生頁碼 Home 1/2、End 2/2、再次 Home 1/2 已觀察；兩次 Enter 一個展圖 batch、展圖後 Home 1/2（38%），成功兩圖及 404 各一次 GET。原生浮球已無基本 Help 提示。來源 PNG 960×576，排除自有 port 字格後與前一版逐像素相同，保留站端原始頁碼。交付與測試副本的 15 個 Mach-O sections 相同，strict ad-hoc 簽章通過，未公證。

本輪原生是部分完成，不是完整 passed：CUA 正文點擊回 noWindowsAvailable，等待前景焦點期間 loopback 300 秒逾時，short_overlap 報告 failed、cleanup false／disconnect null，未以退出碼 0 放行。頁碼與展圖快照只證明各自觀察結果；歷史網址點擊與箭頭／反白 hover 的完整原生 gate 未完成。已正常關閉自有 App、等待 owned HTTP server join 與程序退出；正文操作保留使用者先前的人工驗收界線。

本輪使用標準 8 GiB resources.py build 准入，未使用先前共用額度例外；證據位於 `.local/verification/reading-page-hints/`。Windows／Linux 與實站新版重啟未在本輪驗證。

## 網頁閱讀百分比跟隨可見位置（2026-10-21）

使用者要求全文完成後，下排百分比應表示目前讀到的位置。網頁模式以可視正文下緣／完整文件內容高度計算整數百分比，含展開圖片；短文章為維持 24 列補齊的空白不算未讀內容。內容末端可見才是 100%，未取得全文時顯示 `--%`，載入控制仍報已取得行數。頁碼、來源行範圍及指令維持站端資訊；終端機百分比不變。此項取代先前只顯示站端百分比的契約。

Rust 由同一完整 glyph/cell 投影提供已驗證的 ASCII 數字欄位，JS 只依所擁有的捲動／展圖幾何更新顯示數字，不解析 PTT 文字或產生新的站端輸入。顯示的整數改變才重建一列 bitmap；來源格線、複製及 PNG 不因本機狀態顯示而改寫。未知／超長／非 ASCII 的數字欄位保守不套用。

新增 JS 獨立 expected 先重現首頁仍為 100%；Rust bin 108、全 targets Clippy／fmt、完整 JS 179 及 docs 通過。涵蓋上下捲動、回首頁、到底、同整數 bitmap 重用、展圖後重新計算、未完成全文，以及短文＋圖片全部可見時不將空白算為未讀。

最終交付 `dist/macos-reading-progress-final/PTTStella.app` 的 macOS arm64 原生 short_overlap：全文完成後 Home 71%、End 100%、Up 96%；兩次 Enter 一個 batch，展圖後 Home 38%、End 100%，載入中為 `--%`。歷史一般網址真正左鍵一次由 sink 收到，peer 僅一個 PageDown，三個圖片網址各一次 GET。含正常視窗固定範圍的前一版及最終全螢幕符合比例的驗證；不是跨平台驗收。PNG 仍 960×576，排除自有 port 字格後與固定尺寸版逐像素相同，未包含展圖或本機百分比。Close acknowledged、全部 owner／HTTP server join、exit 0；15 個 Mach-O sections 與最終副本相同，strict ad-hoc 簽章通過，未公證。

證據同在 `.local/verification/article-overlap/`：progress-before-js.log、progress-checks.json、progress-js-final.log、native-progress-release 與 progress-validation.json。中間一次以中文輸入法按字母後確認組字，送出文字而遭精確 peer 拒絕，保留 failed；重跑使用方向鍵並保持原輸入法，未放寬站端預期。實站新版重啟、正文其他滑鼠手勢及 Windows／Linux 本輪未驗證。

## 全文閱讀可視區固定尺寸（2026-10-21）

使用者回報全文後 BBS 看起來變高。原 native window 1040×768 未變，但長文件把原本置中的 80×24 區域擴至整個工作區。網頁捲動區改以來源欄列及主視窗計算符合尺寸並置中，全文與圖片只增加內部文件高度；縮放留在同一可視區，返回終端機恢復原容器。觀察同一 parent resize，沿用同一 observer 清理，避免用上次可視區尺寸反覆縮小。

新增回歸先失敗，修正後完整 JS 177 通過，涵蓋短／32／441／4096 行、圖片、DPR 2、縮放、寬高視窗及模式返回。最終 arm64 release `dist/macos-fixed-reading-viewport/PTTStella.app` 的原生 short_overlap 在全文與展圖前後均保持 1040×624 的 80×24 可視範圍，底列維持相同位置；Home／End、歷史網址左鍵、圖片及來源 PNG／清理亦通過。交付與測試副本 15 個 Mach-O sections 相同，strict ad-hoc 簽章通過，未公證；Windows／Linux 與正文其他手勢未在本輪驗證。證據同在 `.local/verification/article-overlap/` 的 native-viewport-final 與 viewport-validation.json。

## 短文章末頁與網址回歸（2026-10-21）

使用者回報 C_Question `#1gs1UIuY` 的短文章缺少後半簽名檔及推文，網頁上顯示文章已變更。已從同一實站閱讀確認站端末頁確有內容；末頁回報 9～31 行，起點可以是折行的續段。舊快取將邏輯重疊行數當作畫面重疊長度的下限，導致接頁被拒；另外空白格的不可見前景色不同，也會誤判為變更。

修正以實際畫面列的可見 glyph 邊界對齊，邏輯行數用於重複列的取捨；空白格比較實際背景，非空白文字及雙色前景／背景仍須相同。沒有放寬可見文字改變、缺口與 stale／scope 准入。兩個新增獨立 expected 在修改前均失敗；修改後完整 core 13 commands／2 replay、desktop lib 143／bin 107、全 targets Clippy／fmt、JS 176 與 docs 通過。新增 SurfaceState 案例核對 32 列完整尾端，以及不在站端末頁的一般網址仍由目前 ACK 投影解析，舊來源拒絕。

最終原生 `short_overlap`（同時含下述固定可視區修正）通過：一筆精確 PageDown 後完整保留 1～32 列、end／complete，End 看見簽名檔與最後兩行，Home 回看並真正左鍵點擊歷史一般網址，sink 收到一次。兩次 Enter 僅一個 batch，兩圖與 404 各一次 GET；960×576 來源 PNG 不含展圖，Close acknowledged、全部 owner／HTTP server join、exit 0。使用者亦回報短文章目前可閱讀。實站終端機的一般 PTT 網址曾正常開啟預設 Firefox；sink 仍不當作外部瀏覽器啟動證明。

初次原生流程未取得前景焦點，另一次完成全文但尚未點網址／展圖／匯出便退出，兩份 failed 保留；固定可視區的首次流程誤送 Tab 被精確 peer 拒絕，也保留 failed。最後一次由使用者切到前景後完成上述操作，未放寬 peer 或 gate。

證據在 `.local/verification/article-overlap/`。標準 8 GiB build 因共用預留拒絕；使用者本輪明確指示「你無視此次的共用磁碟」。僅本案執行程序的政策副本暫用 64 GiB 共用准入，保留同一資源登記／實體 lease／target 鎖、8 GiB namespace、Cargo shim、watchdog 及 100 GiB free floor；沒有修改共用政策檔、啟動 coordinator 或清理他人資料。格式／JS／docs 曾以 0.05 GiB isolated lane 獨立先行通過，Rust／打包使用上述已授權例外。

## 可取消的網頁全文載入（2026-10-21）

依使用者明確要求，網頁文章進入即載入全文，終端機文章保持原始逐頁送鍵。Rust 的同一快取保留全文及來源頁面→顯示列對應，移除舊 256 行淘汰實作；PTT 邏輯行號與折行畫面列分開，歷史重繪不重複接頁。從中段進入時先 Home 回開頭，重建顯示列並撤銷舊圖片位置，再逐頁 PageDown。完成須同時保留文章開頭與已確認結尾，不把只看過末頁當全文。

載入使用現有 planner、command owner、Connection 與 Paint ACK，一次一頁；失焦、modal、選字／拖曳、組字、工具及匯出期間暫停續送。取消按鈕或 Esc 可停止、保留已取得文字，晚到的一頁可保留但不再續送；明確按繼續才恢復。單頁等待 6 秒、總步數與容量有界，逾時不自動重試；文章接頁變動與容量限制保留舊內容並提示。上限 4,096 畫面列、估計 payload 32 MiB、連結 target 512 KiB，詳見 [文章閱讀](../article-reading.md)。自動取得的是文字，圖片仍由明確 Enter／情境操作展開；下排仍是站端原始百分比，不改成閱讀位置百分比。

最終離線 gate 通過 core 13 commands／2 replay、desktop lib 143／bin 104、全 targets Clippy／fmt、完整 JS 176 及 docs。測試涵蓋 441 行、取消／晚到／繼續／舊 scope、等待期限、暫停期間回覆、中段 Home、折行兩列重疊及歷史頁重繪、容量保留開頭、終端機無預載與既有名單／網址／短文章交叉回歸。原生 window／filters 探針明確採終端機模式維持既有精確站端命令契約，單頁 UI 探針改用完整 1/1 頁尾；多頁網頁另由 full_load 驗證，未關閉產品載入功能來避開測試。

最終 macOS arm64 真正 WebView 的 full_load：20 頁自製文章分片／延遲回覆，Esc 取消後停在 397 行，觀察仍不前進；明確繼續後至第 20/20 頁 100%，End 看見第 441 行，Home／PageUp／PageDown 回讀零額外站端 bytes。peer 獨立核對 19 筆精確 PageDown；全文取得期間 HTTP 計數為空，兩次 Enter 只建立一個 batch，兩圖及 404 各一個 GET。整篇選取停用超過 24 行的行 PNG；過長文字貼上被既有上限拒絕、零站端文字。terminal_pages 原生保持第一頁，手動 PageDown 才到第二頁，零自動取頁／圖片 HTTP，沒有全文控制列。

兩個最終案例均匯出 960×576 原始站端 PNG，獨立像素檢查無展圖彩色內容；Close acknowledged、listeners／Connection／credential／file worker 及 HTTP server 全部 join、程序 exit 0。最終交付與兩份測試副本 15 個 Mach-O sections 相同，strict ad-hoc 簽章通過，未公證。證據為 `.local/verification/full-article/finish.json`、`native-full-final`、`native-terminal-final` 與 `validation.json`。第一輪 full_load 在操作取消前已完成，報告 failed；第一輪 terminal_pages 測試誤送 Home，被只接受 PageDown 的 peer 拒絕，failed 保留，均未列為通過。

標準 16 GiB ceiling 曾因其他 namespace 預留而拒絕；核對本 namespace 約 6.71 GiB，依賴／編譯器／profile 不變，採較嚴格 8 GiB ceiling 通過，保留全域 60 GiB 與 100 GiB free floor，未清除他人資料。全文實站、正文原生滑鼠／hover／拖曳與 Windows／Linux 本輪未驗證；既有實站 52% 修復結果保留如下，不能代替新全文排程的實站驗收。正常使用者視窗未以合成探針取代，測試副本均已關閉。

## 文章中段 52% 停住與 PageUp／PageDown（2026-10-21）

使用者回報部分文章卡住並提供公開 AID。新增同版型自製案例先重現 `Unknown(Unrecognized)`：共同閱讀辨識器要求頁尾同時包含「←／離開」，但 PTT 的回應／推文頁尾可省略此提示。修正接受行範圍後完整的 `(y)回應(X)推文(h)說明`，各鍵及說明須有狀態列背景；原有有效頁碼、百分比、最多 23 行、完整映射／控制碼與游標限制保持。一般鍵盤、Vim、滑鼠、快取與傳輸共用此結果，不用任意 PageDown fallback。錯鍵／缺鍵、無背景或局部去背景、額外確認／輸入提示及正文游標仍拒絕。

core 13 commands／2 replay、desktop lib 143／bin 96、全 targets Clippy、fmt、JS 172 及 docs 通過。新 production Connection loopback 連續十頁，包含 52% 後的 64% 至 100%，獨立核對九筆 PageDown；末頁與後續確認提示拒絕額外送鍵並正常 Close。既有 `inline_pages` 自製頁尾改用回應／推文版型，macOS arm64 真正 WebView 驗證 PageDown 取下一頁、PageUp／PageDown 回讀、兩次 Enter 一個展圖 batch、兩張圖及 404 各一個 GET、PNG 與全部 owner／server join、exit 0。來源 PNG 960×576，正文排除自有 port 後與前版相同；頁尾差異為此次明確更換的合成版型。

實站另已重現並驗證：舊交付在 EZsoft `#1gfarpNI` 由第 5 頁 40%（仍有←離開）進到第 6 頁 52%（返回提示消失）後，PageDown 無法再取頁。正常關閉舊版並啟動修正版，使用者協助完成登入；同篇依序通過 52%、64%、76%、89%，至第 10/10 頁 100%（第 196～218 行），再用 PageUp／PageDown 回看並返回末端。新版正常視窗保留在此文章供使用者驗收；沒有匯出或記錄私人流量／帳密，公開文章內容不做 fixture。這是一篇實站案例，不能推定所有長文／所有站端版型均通過。

證據在 `.local/verification/reading-stall/`（before、checks、finish、native、validation 與 live-observation）。標準 16 GiB harness 通過，沒有額度例外。Windows／Linux 與正文滑鼠仍待各自驗收。主選單快速前往途中曾停在「我的最愛」選取列，手動 Enter 後才進入列表；同篇驗證改由已辨識列表及原始 AID 定位，該主選單路徑另記待核對，未宣稱本輪已修復。

已讀取 PTTStar Analyzer 既有交接：靜態證據確認相鄰頁合併、本機長文及邊界交回站端；是否進文即自動抓全文的完整排程仍未證實。本次修正辨識，不悄悄啟用全文預抓；後續若採全文載入，應另定義取消、容量、文章變動與站端操作交接。

## 網頁文章保留底列資訊（2026-10-21）

依使用者要求，網頁文章及展圖時固定顯示同一 Rust 投影的最後一列，沿用 PTT 原始字格、ANSI 配色、頁數／百分比與行範圍。百分比仍是目前站端頁面資訊，回看快取或展圖不另外推算進度。正文虛擬列排除該列，以共同 renderer 產生一張有界單列影像，新 Paint／尺寸才重建；本機捲動重用。位置依實際 viewport 邊界計算，不混合 scrollTop 與尚未更新的 surface client rect。選取及連結共用顯示幾何，底列不點穿圖片，翻頁扣除該列高度；短文章及來源 PNG 格線維持原尺寸。清理撤掉影像來源與圖層。

core 13 commands／2 replay、desktop lib 142／bin 96、全 targets Clippy 與 fmt 通過；最終完整 JS 172 項及 docs 通過。新增固定列、站端更新、縮放、短文章、選取、圖片遮擋、清理及非同步捲動座標案例。加入 footer img 後既有媒體解碼測試誤選該 img，修正為選擇實際 decoder，未放寬解碼／錨點條件。先前 canvas／混用座標的原生候選未達標，保留 failed 記錄，不以退出碼 0 當作畫面通過。

最終 macOS arm64 `inline_pages` 合成原生已觀察 50%→100% 資訊列更新、兩張圖片及 404、Home／End／Down 完成重繪後的固定底列。兩次 Enter 共一個展圖 batch，三個 URL 各一次 GET；peer 僅收到一次精確 PageDown，其餘本機捲動零額外站端輸入。即時快照偶爾早於非同步合成完成，驗證使用完成重繪的畫面，不宣稱逐幀無閃爍。來源 PNG 960×576 排除自有 HTTP port 字格後與既有回歸基準相同；Close acknowledged、全部 owner join、server join、exit 0。交付與測試副本 15 個 Mach-O sections 相同，strict ad-hoc 簽章通過；未公證。

證據位於 `.local/verification/reading-footer/`。標準 16 GiB 與 10 GiB ceiling 曾因其他 namespace 預留被拒；核對本 namespace 約 6.71 GiB，變更僅同依賴／同 profile 的嵌入資產，審查後採較嚴格 8 GiB ceiling 完成，保留全域 60 GiB 與 100 GiB free floor，沒有刪除其他資料。正文滑鼠／hover／拖曳仍由使用者人工驗收；實站、Windows／Linux 本輪未驗證。

## Vim 實驗中標示（2026-10-21）

README 與 Vim 文件加註實驗狀態；兩語共用的 `vim-off`、`vim-on`、`context-vim` 文案更新，因此初始按鈕、切換後與右鍵入口皆使用相同標示。沒有改動按鍵或導覽政策。

Rust i18n 4 項、相關 JS 44 項與 docs 通過。macOS arm64 的隔離 `short_reading` 原生程序已觀察中文開／關、同程序保存並切換英文後的 on／off；第二個程序還原英文後按鈕仍為 experimental。兩次均匯出 PNG、Close acknowledged、全部 owner join、exit 0；兩張來源 PNG bytes 相同。交付與測試副本的 15 個 Mach-O sections 相同，strict ad-hoc 簽章通過，未公證。證據位於 `.local/verification/vim-experimental/`。右鍵入口的新增文案經 Fluent 檢查，本輪未另做原生正文右鍵或完整瀏覽回歸；使用者實站與其他平台未操作。

## 既有功能交叉回歸檢查（2026-10-20）

依使用者要求擴大到已做功能，並在驗證後推送 develop。基準為 `f43ef6a`，先前功能提交皆在其祖先鏈，沒有以新功能測試取代既有 suite。新增三個案例先重現失敗：短／長快取頁尾的 OSC 8 metadata 遺失；Vim 原始輸入往返未撤銷舊 presentation／延遲 ACK；清除暫時推文篩選後舊畫面仍被接受。修正保留當前頁尾 metadata，並在閱讀投影／篩選改變時遞增版本、撤銷選取／hover／輸出；空輸入不清除閱讀。一般返回與非閱讀輸入共用清理路徑。

另新增 16 組交叉案例：網頁／終端機、16／23 行文章、無規則／遮內容／整行遮黑／白名單；逐組驗完整文字格線、獨立 expected 網址及圖片候選、連結複製、全文選取與 24 行輸出、暫時揭露和舊版本拒絕。修正前 bin 92 passed／3 failed，修正後完整 bin 96 passed；未修改既有 expected 放行失敗。

| 既有功能 | 本輪驗證 |
| --- | --- |
| ANSI／雙色字／固定格線、選取、編碼、網址、名單、站點、傳輸、保活、登入狀態與導覽准入 | 既有 core 13 commands／2 replay、desktop lib 142、bin 96、全 targets Clippy／fmt 通過；合成及 loopback，不代替實站登入 |
| 滾輪分流、左右／上下游標、拖曳與 Shift、站端滑鼠、IME 事件、快捷鍵、媒體、設定與浮球 | 完整 JS 169 項通過，未以事件 doubles 宣稱實體滑鼠／IME 驗收 |
| 建置、打包、資產、文件及驗證工具 | isolated lane Python tooling 70 項、docs gate 通過 |
| 主選單／看板／最愛／文章列表、反覆切板、計數、命令說明、空列表、任意鍵 | 最新交付副本原生 window probe 46 筆精確操作通過，長按 j 一秒僅關閉一張繼續提示 |
| 網頁文章 j／k、方向鍵、Home／End、展圖／失敗圖片、選取與複製／貼上／PNG | 原生 `inline_pages` 重跑通過；一次 PageDown，j 與 Down／k 回程與 Up／Home 內容區像素相同；兩次 Enter 一個 batch，成功兩圖與 404 各一個 GET；47 行選取禁止行 PNG，Cmd-C／Cmd-V 顯示 47 行預覽，取消零站端文字 |
| 設定保存／重啟、語言與文章模式、Vim 原始輸入往返 | 獨立程序還原 English／Terminal／Top right，再保存 Web；取頁／展圖後 i 清除圖片並切為原始格線、Escape 返回網頁；本機字型清單及 bundled 字型載入已觀察，未以此宣稱所有字型像素驗收 |

三個完成的原生程序均 Close acknowledged、listener／connection／credential／file owner join、exit 0；兩份 960×576 PNG 排除自有 HTTP port 的五個字格後像素完全相同，且均不包含展圖。交付與三份副本 15 個 Mach-O sections 一致並通過 strict ad-hoc 簽章；未公證。

第一輪文章流程在調查 AX 點選後焦點仍停於工具按鈕時超過 300 秒 loopback deadline，報告保留 failed／cleanup false，不算通過。重跑時明確確認匯出完成後焦點回到 BBS 輸入，原生全文複製與預覽成功；不是藉改 expected 或延長期限放行。CUA 右鍵座標仍曾回 noWindowsAvailable，原生正文點擊／hover／拖曳及上輪快速名單完整 gate 維持原有待驗；實站與 Windows／Linux 本輪未執行。測試使用隔離偏好與自製資料，中文輸入來源已還原，所有自有 App／server 程序結束。

證據在 `.local/verification/regression-audit/`：`before.log`、`checks.json`、`tooling.log`、三份原生報告與畫面、`validation.json` 分列通過範圍及失敗嘗試。正常資源 guard 通過，沒有使用額度例外或刪除其他專案資料。

## 短文章網址底線與開圖回歸（2026-10-20）

使用者回報先前功能疑似沒有合併，並指出網址底線與開圖。核對 `f652f44` 與當時遠端 develop 一致；文章模式、固定尺寸、j／k、箭頭、黑白名單及合併推文提交均為其祖先。實際找到 `ca9991c` 的回歸：短文章只提高 Frame.rows 並固定頁尾，沒有補上正文與頁尾間的字格；renderer 看似正確，嚴格 text::rows 卻拒絕不完整格線，導致一般／OSC 8 連結、圖片候選及字串候選同時消失。原尺寸測試對空集合使用 all，未證明空白字格存在；原生尺寸案例也沒有網址。

修正由同一 ArticleCache 投影補齊空白字格，保留真實文章行數、原始 PNG source 與頁尾位置；未放寬網址或文字解析的完整性要求。新增完整格線與一般／OSC 8 圖片、連結複製、ID 字串的交互回歸，覆蓋 1／3／22／23 行。修正前兩項測試明確失敗（短文圖片連結 0 而非 2），修正後 core 13 commands／2 replays、desktop lib 142／bin 92、JS 169、Clippy、fmt 與 docs 全數通過。

macOS arm64 實際 Dioxus／WKWebView 的 `short_links` 三行合成頁：兩條底線可見，兩次 Enter 只有一個展圖 batch，兩張自製圖片均解碼且各一個 HTTP GET；End 本機移至第二張圖與頁尾，peer 零站端輸入。完整 PNG 960×576、空白區全黑；Close acknowledged、listener／connection／credential／file owner 全部 join、程序 exit 0。原生截圖為 CUA 的 JPEG／顯示色域，底線與圖片像素使用明確色差容限及長度／面積檢查；PNG 使用無損精確檢查，不混用。

證據在 `.local/verification/short-links/`，包含修正前失敗摘要、離線 logs、原生前後畫面及 `validation.json`。交付與驗證副本的 15 個 Mach-O loadable sections 一致，strict ad-hoc 簽章通過，未公證；本輪正常資源 guard 通過。原生滑鼠點圖／hover、使用者實站及 Windows／Linux 未在本輪驗證，仍待各自驗收。

## 文章快速名單與暫時只看單一 ID（2026-10-20）

使用者要求文章字串右鍵加入兩種黑名單／白名單，以及左鍵點推文只看該 ID。已實作 Rust 可見完整字串候選與已辨識推文 metadata，三個明確名單選項共用保存 owner；暫時篩選在快取後投影，不改永久名單。再次點同 ID 或右鍵恢復全部；網址／Shift／拖曳優先，其他推文整行塗黑且輸出一致。

離線 core 13 commands／2 replays（filters 16 案例）、desktop lib 142／bin 90、JS 169、全 targets Clippy、fmt 與 docs 通過。新增案例覆蓋完整／過長／非法／遮罩字串、三策略保存前准入與保留備註、input epoch／過期 ACK 拒絕、推文整行篩選與連結移除、合併 ID 的 Rust 歸屬、切換恢復、來源 PNG、快取跨頁／新段／session，以及原生 pointer handler 的網址優先、Shift／拖曳／過期手勢不送操作。初次 bin 編譯有新增 match block 的逗號語法錯誤，修正後完整 bin／Clippy 重跑通過。

原生為部分完成，**不是完整 passed**：macOS arm64 Dioxus／WKWebView 的合成閱讀頁，真正右鍵 Alice 顯示目標 ID 與三個選項，遮內容及白名單兩次保存均正確；獨立檔案確認 mask／allow，PNG 960×576 的兩行內容遮罩與 ID 亮青色正確，排除這些指定範圍後其他像素完全相同。整行黑名單快捷入口、暫時篩選／恢復及其原生 PNG 尚未完成；Rust／JS 結果不代替這些 UI 驗收。正文左鍵／拖曳手感仍留使用者，實站與 Windows／Linux UI 未驗。

第一輪為測試方便把游標移至正文，導致正常閱讀辨識拒絕，已修正合成 probe、保持產品契約。第二輪 CUA 座標操作間歇 `noWindowsAvailable`，全螢幕短暫恢復後再次受阻，loopback 最終逾時且 cleanup report 不完整，程序退出 0 不算通過。工具在關閉後讀取視窗會重新啟動同名副本；已關閉其重啟程序，以全新 bundle identity 第三輪仍遭同一座標限制，因此停止 UI 操作；第三輪 Close acknowledged、完整 cleanup、退出 0，但功能 gate 仍 failed／not_ready。所有測試 App 程序已結束，未遺留正常連線副本。

交付與第二／三輪副本的 15 份 Mach-O sections 一致，strict codesign 通過。證據為 `.local/verification/account-actions/validation.json`（status: partial），離線 `checks`／`checks-fixture-fix`，原生 `native-final`／`native-r2`／`native-r3`。正常 60 GiB aggregate 因其他專案預留拒絕，沿用使用者先前額度例外的 scoped 192 GiB wrapper，保留 100 GiB free floor、16 GiB namespace、locks／watchdog，不改全域政策或清理其他 target。

## 連續推文 ID 合併（2026-10-20）

使用者要求相同 ID 連續推文只保留第一行 ID，後續行 ID 塗黑以增加一體性。只合併已辨識標準推文、忽略 ASCII 大小寫；不同 ID、空行、未知格式皆中斷，推／噓／箭頭、冒號、正文、時間與行列不改。黑白名單先套用，重複 ID 再塗黑；白名單僅組首 ID 標色。網頁快取已讀取的相鄰頁可接續，未讀間隔與新文章不猜測；終端機／單頁 PNG 以各頁為界保留第一筆 ID。Rust metadata 不序列化至 WebView；複製、PNG、連結共用呈現結果。

core 13 commands／2 replays（filters 14 案例）、desktop lib 142／bin 87、全 targets Clippy、JS 166、fmt 與 docs 通過。新增獨立 expected 覆蓋同 ID／大小寫、不同 ID／空行／未知格式、黑白名單及揭露、ID 連結移除、快取跨頁／缺口／新段及來源 PNG 首筆 ID。兩項原有完整 frame expected 因重複 ID 的新呈現失敗，僅調整明確的五個 ID cells，重跑通過。

macOS arm64 原生 Dioxus／WKWebView 自製 loopback 通過：首筆 Alice 保留，下一筆 ALICE 五個字元全黑，推噓符號／冒號／正文／時間維持。獨立 PNG 解碼核對 960×576 與精確遮罩；除重複 ID 與基準圖先前白名單的 Alice2 色彩之外，其餘像素完全相同。原生情境選單複製後 Cmd-V 預覽可讀到「→ █████:」，取消未送站端文字；Close acknowledged、所有 owner join、退出 0。交付與測試副本 15 份 Mach-O sections 一致，strict codesign 通過。跨頁接續與終端機模式本輪為 Rust 測試證據，未冒充原生跨頁驗收。

首次原生 Cmd-V 卡住，process sample 定位到貼上路徑持有 watch read guard 時再次讀取，遇到等待中的 writer 會 deadlock；已將編碼讀取獨立成 statement 先釋放 guard，再準備貼上／核對當前狀態。保留失敗 sample，終止自有卡住程序後，以新 bundle 完整重跑 PNG／複製／預覽／取消／正常關閉通過。原生正文滑鼠、實站與 Windows／Linux UI 仍待各自驗收。

證據在 `.local/verification/merged-replies/validation.json`；離線為 `checks`、`checks-finish`、`checks-paste-fix`，最終原生為 `native-final`，首次失敗為 `merged_replies`。建置走正常資源 guard；重跑腳本遇既有 evidence／bundle 路徑拒絕時改用新輸出位置，未覆蓋舊證據。

## 兩種黑名單與白名單 ID 標色（2026-10-20）

使用者指定發佈前補齊：已辨識文章列表與標準推文皆可選擇只遮標題／內容、保留 ID，或整行塗黑；白名單只以亮青色標示 ID。保留 80×24 行列，不刪行。原站端 Frame 不變，畫面、複製、PNG 與連結共用 Rust 投影；舊規則遷移與明確保存、暫時揭露、規則變更撤銷文章快取一併驗證。未知版型與不完整 ID 不猜測。交付為 `dist/macos-account-policies/PTTStella.app`（arm64 release、ad-hoc 簽章），名單 schema v3，舊 v1／v2 在記憶體遷移、明確保存才改檔。PTTStar Analyzer 的靜態研究證實 B／C 分類及 W ID 標色，但原版另有全域強度設定；匯入預覽明示本產品 B→遮內容、C→整行塗黑、W→ID 標色，白名單優先沿用本產品既有規則。

core 13 commands／2 replays（filters 11 案例）、desktop lib 142／bin 86、全 targets Clippy、JS 166、fmt 與 docs 通過。獨立 expected 覆蓋文章／推文三策略、完整 ID 與保留行列、遮罩複製／連結、舊檔遷移、未知畫面保留，以及網頁／終端機模式的規則變更、來源 PNG 與文章快取撤銷。首次 i18n 舊文案 expected 與新增 JS 測試誤用翻譯 key 各失敗一次，修正測試後分別完整重跑通過。

macOS 原生 Dioxus／WKWebView 的自製文章列表、推文與重啟三個程序通過：表單保存三策略、B／C／W 匯入預覽及確認、移除項目、系統剪貼簿貼上預覽、完整 PNG、暫時揭露與新程序載入。文章剪貼簿預覽完整顯示三筆相關行；推文 AX 預覽截短於第二筆遮罩，完整字串由核心測試核對，未宣稱原生全文逐 byte 讀回。獨立 Pillow 解碼確認文章標題／整行全黑、保留作者與亮青色 ID；推文兩個內容範圍及 Bob 整行全黑，揭露前後其他像素完全相同，白名單色彩保留。重啟後規則 bytes 不變、未建立備份；所有程序 Close acknowledged、listener／connection／credential／file owner join、退出 0。交付與三份原生測試副本的 15 份 Mach-O sections 一致，strict codesign 通過。

證據在 `.local/verification/account-policies/validation.json`，離線為 `checks`／`checks-finish`，原生為 `account_articles-r2`／`account_lists`／`reload`。第一輪文章探針在工具／複製操作期間送出非預期站端輸入而失敗，保留原報告；重跑完整程序後才計通過。正常 60 GiB 與既有 96 GiB 例外均被其他 namespace 的預留額度擋下；依使用者先前明確允許忽略跨專案額度的授權，本輪 scoped wrapper 採 192 GiB aggregate，保留 100 GiB free floor、16 GiB namespace、鎖與 watchdog，未改全域政策或清理他人快取。原生正文滑鼠／hover 與實站三策略仍由使用者驗收；Windows／Linux UI、單行 PNG 與系統選檔視窗未由本輪補驗。

## 任意鍵提示的按鍵與點擊繼續（2026-10-20）

使用者要求「請按任意鍵繼續」確實可用任意一般按鍵及滑鼠點擊關閉。沿用原本嚴格 ContinuePrompt 辨識，不解析第二份站端文字。Rust Keyboard 在 Vim 開／關、原生輸入／原生一次中皆把一般按鍵轉為 Continue，再由既有 planner／Connection 送單一空白；f／i／Escape 不在這張提示切換模式，原生一次消耗該次機會，其餘模式保留。Normal 保留 readonly 宿主的實體字母／數字鍵轉譯（含 CJK 來源的 Process／229）；修飾鍵單按、原生輸入的 Dead／Process、真正組字及其確認不送文字；選取 Escape、複製／貼上、本機視窗／縮放快捷鍵保留原本 owner。

滑鼠整個可見 terminal 格線單次左鍵共用 Continue，先於網址與左緣返回。延用 pointer 交易與輸入／畫面版本，另核對投影及可視幾何；拖曳、Shift、取消、雙擊第二次、更新中、忙碌、modal、站端滑鼠與過期手勢皆不能繼續。新畫面不補送；長按持有到 keyup，不把同一次按鍵解讀成下一頁操作。

離線 core 13 commands／2 replays、desktop lib 142／bin 82、全 targets Clippy、全套 JS 158、docs 及 macOS release 打包通過。新增 Rust 按鍵模式／IME／長按與本機服務的 Keyboard／Pointer 兩來源精確 bytes，JS production handler 核對整格線、網址優先、重複／過期／幾何及本機輸入 owner。原生 WKWebView 完整 46 筆 loopback 回歸通過：三張提示分別用 Normal 長按 j 一秒、Vim 關閉時 x 及 Escape，服務端各只收到一個空白；長按未跳過下一張提示。其後恢復 Normal 完成列表／Help／空列表，Close acknowledged、全部 owner join、退出 0。AX 索引重新編號曾誤開原生選單，取消後確認仍為 test37 才繼續，服務端未收到額外 bytes，未重送已完成步驟。

證據在 `.local/verification/continue-input`，`final-checks/checks.json`、`native-final/window-probe.json`、原生提示／長按／原生模式畫面及 `validation.json` 分列範圍。交付與測試副本 strict 簽章、15 份 loadable Mach-O sections 一致；原生滑鼠點擊仍依使用者指示留待手動驗收，不以 DOM doubles 冒充實體點擊。Windows／Linux、原生 IME 與完整 Mac desktop backend 仍未由本輪驗證。 正常模式副本已用保存的 PTTStella 帳號明確按登入，實站標準提示確認為 VIM NORMAL、本機滑鼠路由；收合工具後停留在該提示，供使用者單次點擊驗收，未代按點擊或宣稱滑鼠通過。先前根列表驗收副本已關閉，避免誤測舊程式。

## 實站根列表辨識修正（2026-10-20）

使用者回報看板列表仍不能滑鼠操作，要求用 PTTStella 帳號實際檢查。舊 App 的共同 Rust 辨識要求進入與返回提示同時存在，但實站我的最愛根列表沒有返回提示，分類根目錄沒有進入提示，導致兩者為 Unknown／VIM 暫停；不是僅缺原生點擊驗收。新增 `board_commands` 的兩種完整操作列條件，沿用原標頭／樣式、項目欄、連續列與游標驗證；普通列表與未知畫面的准入不變。詳見 [導覽契約](../navigation.md)。

自製 fixtures 沒有放入實站帳號、最愛內容或私人流量。新增核心正反例核對省略單向提示、完整操作列、兩種游標、無樣式／缺鍵／錯鍵／提示覆蓋與游標失配；production Connection 的同一測試擴充兩種場景，獨立 expected 為 Up＋CR，換成確認畫面後拒絕舊操作且無額外 bytes。core 13 commands／2 replays、desktop lib 142／bin 82、全 targets Clippy、全套 JS 152、Python tooling 70、docs 及 macOS release 打包通過。

實站使用獨立 App 副本、已保存的 PTTStella 帳號及使用者手動協助；先前開著的其他帳號視窗已還原原列表，未用來進行後續驗收。最終候選在我的最愛與分類根目錄均顯示 `VIM NORMAL`、站端未要求滑鼠／使用本機導覽；兩處 j／k 正確移動，l 進入分類子列表或 SYSOP，進板提示明確按空白，h 從文章列表／子列表返回，最後回到我的最愛。這證明共用 profile 與 planner 可操作；正文座標點擊、hover 手感與實體滾輪仍交使用者手動驗收，未用鍵盤冒充滑鼠事件。

依使用者要求，修正前後畫面使用 WebView 內的同一 Canvas renderer、`toBlob` 與 Rust 匯出 owner，經原生 Save picker 保存 PNG；不是桌面截圖。四份 PNG 均為 960×576；兩種列表的標頭／操作列／頁尾逐像素相同，分類整頁亦相同。我的最愛正文的實站人氣會變動，未宣稱整頁相同。PNG 不含浮球、選取或 hover；如需整個 WebView UI，原生 snapshot API 仍是另一種擷取範圍。

最初新增的 ASCII 指標測試覆寫雙格指標時留下半格未映射，改為完整清除兩格後通過，保留原失敗 gate。第一候選離線通過但實站仍暫停，進一步由原生剪貼簿讀回精確文字，確認實站為 `(v)已讀/未讀`、`(y)只列最愛`，修正錯置的合成標籤並新增負例後再建立最終候選；沒有把早期 App 當成通過。證據在 `.local/verification/board-live`，最終離線 logs 為 `gates-r3`，PNG 為 `favorites-before/after.png` 與 `classes-before/after.png`，原生辨識為 `favorites-r3.ax.txt`／`classes-r3.ax.txt`。實站內容與剪貼簿讀回資料僅留本機，不進 repo；沒有錄製完整 session 或保存密碼。

## 文章點擊翻頁與滑鼠支援界線（2026-10-20）

可見正文上半部點擊上頁、下半部點擊下頁，快取先本機捲動 90% 可視高度、未知邊界才透過既有 planner 取頁。修復快取閱讀停用左側返回的回歸：保留正文各行左側第 17 欄前返回，hover 只顯示可見最左一欄；網址優先，展圖空隙不套正文點擊，拖曳與 Shift 選字沿用既有 owner。按住或放開後改變 stamp、文章、投影、捲動及可視尺寸會撤銷操作；跨上下半部邊界亦不啟動。

離線通過：新增 8 項生產 pointer／幾何／捲動案例，全套 JS 152 項、desktop lib 142／bin 82、core 13 commands／2 replays、fmt、Clippy 及 docs。包含上下半部隨捲動／縮小視窗調整、左緣／網址優先、只有未知邊界單次取頁、首尾／拖曳／Shift／失去 capture／過期／未知／modal／站端滑鼠／未完成輸出不翻頁，以及返回提示只畫一欄。未放寬既有列表辨識條件。

交付副本只連 owned loopback：兩次 Enter 一個展圖 batch，End 移至本機底部、PageDown 精確一次取得分段第二頁，Home 回到第一頁且圖片仍在。三個 HTTP 路徑各一次；整頁 PNG、Close acknowledged、全部 owner join 與程序退出 0 通過。原生座標點擊被 CUA 以 `noWindowsAvailable` 拒絕，因此報告明確區分鍵盤／展圖 smoke passed 與新點擊 native not_ready；沒有用鍵盤代替點擊驗收。證據在 `.local/verification/article-pointer`，本輪未登入實站帳號。

| 畫面 | 已實作的滑鼠行為 | 本輪證據與缺口 |
| --- | --- | --- |
| 主選單／已知系統子選單 | 已辨識單欄項目點選；子選單左側返回 | 既有 core／Connection／JS 回歸；未覆蓋所有實站子介面 |
| 分類／看板／我的最愛 | 編號／板名／類別及游標核對後點選、翻頁、返回 | 現代命令列及縮排分類自製案例回歸；實站不同版型待核對 |
| 信件／精華列表 | 已知 DocumentList 單欄項目與翻頁／返回 | core／Connection／JS 回歸；原生與實站尚未驗證；無新郵件捷徑 |
| 文章列表 | 日期欄右界前返回，作者欄起開啟，整列 hover | 既有回歸；保留原生案例的歷史範圍 |
| 文章閱讀 | 可見上下半部翻頁、左側返回、網址與快取捲動 | 本輪生產 handler 通過；原生點擊與實站待驗 |
| 未知、多欄、表單或編輯器 | 保留原始鍵盤，無通用點擊 Enter | 負例回歸；原因沿用 Rust 的版型／欄位／游標／未完成狀態 |

## 非滑鼠驗收補驗（2026-10-20）

使用者指定原生滑鼠點擊留待手動驗收，先完成其餘可驗項目。本輪沿用 `9203d76` 的交付 App，沒有修改產品程式或重新打包。macOS arm64 release 的獨立 bundle 副本只連 owned loopback；操作使用原生鍵盤與無障礙工具按鈕，不把 AX 按鈕操作當作正文座標點擊。

- 第一頁「整頁選取」正確提供第 1–24 行、完整 80 欄的 PNG；Pillow 獨立解碼後確認 960×576，與同程序的完整站端 PNG 全部像素相同。這補驗了恰好 24 行的整頁選取，不代表任意歷史子範圍或拖曳已驗收。
- 鍵盤 End／PageDown 載入分段第二頁、Home 回看後，整頁選取涵蓋 47 行；選取行 PNG 停用並顯示「一次最多 24 行」。Cmd-C／Cmd-V 經 OS 剪貼簿建立 47 行貼上預覽，可讀到第一頁標題及正文；明確取消後沒有送出文字。AX 會截短預覽文字，未宣稱整份剪貼簿逐 byte 比對。進入原始貼上會依既有路徑撤銷文章／圖片快取，取消後回到目前站端第二頁，不宣稱取消後仍保留歷史快取。
- 從歷史位置匯出的完整 PNG 仍是目前站端第二頁，960×576、全部像素為灰階，不含展圖／選取；第二頁標題行及頁尾與前次交付探針的相同區域逐像素一致。peer 僅收到一次精確 PageDown 與 Close，拒絕全部額外站端輸入；三個圖片 HTTP 路徑各一次。報告 passed、Close acknowledged、listener 與 connection／credential／file owner 全部 join、程序退出 0。
- 交付及副本 strict codesign 通過，15 份 loadable Mach-O sections 完全相同。離線重跑 core 13 commands／2 replays、desktop lib 142／bin 82、Clippy、全套 JS 152、Python tooling 70 及 docs 均通過；這些不是 Windows／Linux 原生結果。

證據保留 `.local/verification/reading-nonpointer`，最終原生報告為 `native-final/validation.json`。第一輪讀回剪貼簿時多按 Escape，嚴格 peer 拒絕預期外輸入，該輪報告保留為 failed；修正操作順序後使用新程序完整重跑才接受。原程序尚未結束時，另一次 isolated admission 曾因磁碟預留額度被拒絕；待原程序及 HTTP owner join 後，以相同命令通過，沒有刪除他人 target 或改動政策。

手動驗收仍需在最新 App 核對：

| 原生滑鼠項目 | 可觀察的結果 |
| --- | --- |
| 不同捲動位置／視窗尺寸的正文上下半部 | 依可見區域決定方向；已載入內容先本機捲動，未知邊界只取一頁，已知首尾不多送鍵 |
| 左側返回與網址／展圖優先 | 正文左側第 17 欄前返回；網址沿用原開啟操作，圖片空隙不翻頁，返回提示只在最左一欄 |
| 拖曳、Shift、按住期間捲動／resize | 保留選字；幾何或來源改變的手勢取消，不誤翻頁／返回 |
| 任意歷史行選取與 PNG | 最多 24 行、完整行寬，不含圖片／選取／游標；超過 24 行可複製但不能匯出行 PNG |

實體滾輪／原生 IME、跨 DPI、Windows／Linux UI 與實站仍待各自驗收；新郵件功能維持延期。

## 文章快取閱讀（2026-10-20）

使用者授權將文章與開圖瀏覽從逐張即時 BBS 刷新改為有界本機快取。本輪交付 `dist/macos-article-cache/PTTStella.app`；新增 Rust `ArticleCache`、雙來源 Paint、按文章行號合併／淘汰、可見行繪製、捲動錨點及本機閱讀鍵盤位置。最近 256 行可本機往返，僅未知邊界取頁；普通 revision 不拆掉展圖。原始輸入／換篇／切站／名單政策仍撤銷，完整 PNG 保持站端 24 行，快取選取 PNG 一次最多 24 行。契約見 [文章閱讀](../article-reading.md)。

離線：core 13 commands／2 replays、desktop lib 142／bin 82、Clippy、144 JS、70 Python、Fluent 與 docs 通過。新增獨立 expected 覆蓋重疊行、缺口、首尾、雙向淘汰、來源／政策撤銷、雙色字、歷史 links／選取與 PNG 範圍、逐頁取界及不進展停止；生產 JS handler 覆蓋有限畫布、前插行／圖片解碼錨點、暫時高度縮小造成的 scrollTop 截斷、鍵盤輸入框位置、滾輪單位與修飾鍵。

原生交付副本使用不同 bundle identity、只連 owned loopback peers：640×1200 紅藍圖、640×900 綠金圖及明確 404。兩次 Enter 一個 batch；讀到邊界只送一次精確 PageDown，第二頁分段回應。Page Up／Home 回看及 Vim `4j`／`4k` 皆為本機，peer 拒絕全部額外 bytes。Vim 實際移動 104px，返回與原圖正文區逐像素相同；回到文章開頭仍能複製第一頁標題，原生貼進本機搜尋欄核對。三個 HTTP 路徑各一次，PNG 為 960×576、純灰階站端內容，不含長圖；Close acknowledged、所有 owner join、程序退出 0。交付與測試副本 15 份 loadable Mach-O sections 雜湊完全相同，交付簽章 strict 驗證成功。

初輪原生發現重繪短暫縮回 24 行，造成捲動截斷；第二輪 peer／清理通過但 Vim 焦點仍跳頁尾，整輪視覺驗收判失敗。兩者皆補生產 handler 回歸並修正後，第三輪才接受；不以程序退出或單份 passed 報告取代操作與像素結果。證據／失敗截圖保留 `.local/verification/article-cache`，最終結果在 `native-delivery/visual-validation.json`。

本輪 CUA 原生座標滑鼠注入回報 `noWindowsAvailable`，實體滾輪、快取拖曳選字／選取行 PNG、原生 IME、不同 DPI、Windows／Linux 與實站仍為 not_ready；已有生產事件單元與 Rust 選取／輸出測試，不冒充上述原生驗收。本機資源保護器曾因其他有效磁碟預留拒絕 gate；拒絕與容量審查有記錄，待額度釋放後完成建置，未刪除其他 target 或繞過 harness。

## 滑鼠瀏覽與可設定右鍵工具（2026-10-20）

本輪交付為 `dist/macos-mouse-browse/PTTStella.app`（arm64、release、ad-hoc 簽章）。Rust 的主選單及看板／最愛／分類 context 補入已驗證的起訖與游標列，滑鼠以同一 planner／connection owner 送有限方向鍵加 CR；可點個人設定、我的最愛、目錄與看板。主選單僅選項區可點，空白／標頭／過期畫面不開啟，未知系統子選單仍使用原始輸入。文章／看板／最愛及閱讀頁滾輪改用 Page Up／Down，保留 Ctrl 縮放、本機放大捲動、選取與編輯准入。

無反白時普通本機右鍵在游標附近開啟原六類扇形工具，收合後回到浮球原位置；有反白或 Shift＋右鍵保留情境選單。站端滑鼠模式仍沿用原協定。設定頁加入「右鍵開啟工具」及「顯示浮球」，都預設開啟；隱藏時只在工具開啟期間暫時顯示中央返回／收合按鈕。關閉右鍵工具後仍能從情境選單進設定，不會失去恢復入口。v5 自舊設定補入預設，取消／失敗不生效，明確保存後才更新；不包含於可攜顯示 profile。hover 改為較明亮藍色與金色邊框／左右 3px 金邊，呼吸維持 80%–100%，減少動態效果時固定亮度；裝飾不進原始 Canvas／PNG。

本機全套 123 JS、core 13 commands／2 replays、desktop lib 137／bin 68、desktop Clippy、兩項 Rust i18n 與 docs 通過。新增案例涵蓋各選項的精確送鍵、空白／非法列、版本及站端路由、翻頁及選取／放大排除、右鍵分流、浮球暫時顯示與位置還原、設定取消／失敗／保存／預設／嚴格欄位及舊版遷移。修正初輪仍使用舊 migration 版本的斷言、wire fixture 次序及錯誤 i18n target 後再通過；保留原始失敗記錄，未放寬送鍵或畫面條件。

macOS 原生 `pointer_rows` 在合成主選單點個人設定、原始 Left 返回、點我的最愛與目錄、滾輪下／上翻頁、點文章整行與左緣返回；本機 peer 嚴格核對七筆導覽及一次原始鍵，共八筆指定輸入，不接受額外 bytes。原生亦確認無反白開扇形、反白與 Shift 保留情境、取消不保存、隱藏仍可右鍵開／收合、關閉右鍵工具恢復情境入口。獨立新程序載入相同設定，兩個開關皆為 false、浮球隱藏且普通右鍵仍可開設定，來源檔 bytes 不變。兩程序均匯出 PNG、正常退出 0，報告 passed、Close acknowledged、listener 與 connection／credential／file owner 全部 join。探針的 `settings_saved` 是既有字高 18／基線 19 的專用條件，本輪未改字高，不以該欄宣稱新開關未保存；開關由實際檔案、重啟介面與截圖獨立確認。

證據保留 `.local/verification/mouse-browse`，包含藍金 hover 與重啟設定截圖、PNG、來源雜湊與清理報告；兩次都僅連本機合成服務，未操作使用者實站 session。Windows／Linux 原生 UI、本輪英文原生設定、完整站端 Shift 右鍵與其他 DPI 尚待驗證；不把 macOS 結果當作完整跨平台 desktop gate 或實站驗收。

## Windows 安裝版與單檔免安裝版（2026-10-20）

依使用者補充，Release 提供 `-setup.exe` 及 `-portable.exe` 擇一下載。兩者消費同一次靜態 CRT 編譯的 App，JS／CSS、翻譯、字型及第三方授權內嵌；免安裝 EXE 不需外帶 CRT DLL 或 Assets 目錄，但須已有 WebView2，不自行補裝。Setup 延續前置檢查及缺少時補裝流程。設定與快取仍位於目前 Windows 帳號的資料目錄。來源 manifest／完整白名單必須具有兩個 EXE；本機 ZIP 留作驗證，不上傳 Release。詳見 [Windows 交付契約](../windows-installer.md)。

打包器保留既有 RUSTFLAGS／CARGO_ENCODED_RUSTFLAGS 並追加 `crt-static`，再以實際 PE imports 拒絕所有非系統 DLL。授權由同一 locked dependency graph 收集，編入後將 App 複製至空目錄執行 `--export-licenses`，逐 byte 核對，任一失敗皆零交付；不以旗標存在宣稱單檔成功。匯出在 UI／WebView／連線建立前處理，拒絕覆寫現有檔案。

本機 Python tooling 67 項、core（13 commands／2 replays）、desktop lib 136／bin 68、desktop Clippy、docs 與 actionlint 通過。macOS arm64 實際編譯並匯出包含 389 筆依賴的 3,082,674 bytes 授權文字，與編入內容 SHA-256 完全相同；另證明既有檔不覆寫及額外參數拒絕。此測試未開 UI／WebView 或連網，只證明共用 CLI 與內嵌文字，不證明 Windows 靜態 CRT 或 GUI。

Windows 原生政策與 desktop gate 在 macOS 回報 not_ready；本輪未真正編譯 Windows EXE／Setup、安裝 Runtime 或驗證單檔 GUI 啟動。GitHub Actions 的付款／spending limit 仍為先前已知阻擋，使用者選擇稍後處理，本輪未重試。Windows 10／11 的靜態連結、空目錄授權匯出、內嵌資產、缺少 WebView2 啟動行為、Setup 補裝與正常斷線仍須原生驗證，不能標為 passed。證據保留 `.local/verification/windows-portable`。

## Windows 單一安裝檔交付（2026-10-20，前一增量）

Windows Release 產品下載改為只有 `-setup.exe`，完整包含 staged App、app-local VC++ runtime DLL、圖示與授權；來源 manifest 與校驗碼仍附於 Release。本機 ZIP 保留作為開發／解壓驗證產物，但發佈收集器不收錄，額外 ZIP／裸 DLL／資產壓縮包會被完整白名單拒絕。JS／CSS、翻譯與內建字型原已由 Rust 內嵌，不需帶原始碼 assets 目錄。這是單一下載的安裝檔，安裝目錄仍包含多個檔案；macOS 是標準 `.app` 套件，Linux 仍須系統 GTK／WebKitGTK 等依賴，詳見 [安裝包契約](../windows-installer.md)。

本機 Python tooling 57 項、docs（27 documents／175 links／21 gate regressions）、actionlint 與 core（13 commands／2 replays）通過。新增案例核對 CRT、圖示、授權與 manifest 雜湊在 Setup 來源目錄內，以及 Release 僅收錄 Setup、缺 Setup 不可由 ZIP 取代。測試使用合成 bytes 與注入 compiler 結果，不能證明真實安裝包內容。Windows 原生政策 gate 在 macOS 回報 not_ready；本輪沒有真正編譯 Setup／安裝 App，也沒有重試受帳戶限制的 GitHub runner。原生編譯、安裝及四架構 workflow 驗證仍待完成。證據保留 `.local/verification/windows-single-setup`。

## Windows Setup 與 WebView2 自動補裝（2026-10-20）

Windows Release 新增 `-setup.exe`，與 ZIP 消費同一次編譯與同一份 staged App；仍使用原本 MSVC／app-local CRT，未混入靜態 CRT 或工具鏈切換。Setup 使用 Inno Setup 7.1.0，繁體中文／英文、目前使用者安裝；前置階段檢查四個 registry view，已有有效 WebView2 時跳過，缺少才執行內附的 Microsoft Evergreen bootstrapper，並再次檢查是否已安裝。失敗停在複製檔案之前，解除安裝保留共用 Runtime 與使用者設定。來源驗證、網路需求與範圍見 [安裝包契約](../windows-installer.md)。

本機全部 Python tooling 55 項、docs、actionlint 及 core 13 commands／2 replays 通過。測試核對 bootstrapper 下載上限／期限、既有檔保留、拒絕錯誤簽章與雜湊、失敗零交付、ZIP／Setup 共用 staged App、交付競爭回滾，以及缺少 Setup 不發佈。沒有執行 Microsoft bootstrapper、操作 registry 或啟動產品。

新增 Windows 原生 Inno 政策 probe，以同一個 production 決策函式與合成 registry／process adapter 驗證 11 項斷言；workflow 把它列為必要 gate 並保留報告。本輪 macOS 沒有 Windows／Inno runner，該 gate 與 desktop 明確回報 not_ready；未實際編譯 Setup，也未證明在無 Runtime 的 Windows 上補裝成功。GitHub Actions 仍受先前已確認的帳戶付款／spending limit 限制，使用者選擇稍後處理，本輪沒有重試或公開發佈。必須另跑原生政策、真正 Setup 編譯與 Windows 10／11 安裝／離線／升級／解除安裝驗收，才能將這些項目標為 passed。證據保留 `.local/verification/windows-installer`。

## 分支與 Release 自動化（2026-10-20）

已從當前 main 建立並推送 `develop`，GitHub 預設分支改為 develop；main 保持 `095ff2f`，沒有提高產品版本或建立 tag／Release。日常開發在 develop，Release PR 才前往 main；標題、來源、遞增 SemVer 與 Cargo.lock 一致性由 PR policy 檢查。main 的 Cargo.toml 更新才自動進入四架構打包，手動 verify 不發佈；全部 archive／來源 manifest／雜湊通過後才從 draft 發佈，詳見 [發佈流程](../releases.md)。

本機 45 項 Python tooling 測試、docs 與 actionlint 通過。`3595aed` 的 clean source 在 macOS arm64 以正式 release profile 建置，ZIP 解壓後 strict codesign、arm64、plist、完整性與所有 SHA-256 通過，包含 389 筆 locked dependency 授權 inventory；沒有啟動 App 或將打包當作 GUI／實站驗收。保留 `dist/release-workflow-macos-arm64` 與 `.local/verification/release-workflow` 報告。

GitHub verify run `39349452136` 在分配 runner 前失敗，零 step，annotation 指向帳戶付款／Actions spending limit；使用者選擇稍後處理。因此 GitHub 四架構建置仍 not_ready，不能宣稱 Windows／Intel Mac／Linux 套件已經由本流程驗證。私人 repo 的 rulesets／branch protection API 另回覆 403，需要 Pro；已提供匯入 JSON，但尚無伺服器端 main 合併／推送限制。使用者處理帳戶後應重跑 verify，方案支援後另啟用 ruleset；目前沒有遠端 Release 或 tag。

## 整行滑鼠導覽與左緣返回（2026-10-19）

最新交付為 `dist/macos-account-lists-final/PTTStella.app`（arm64、dev debug=0、ad-hoc 簽章），包含黑白名單匯入與本節滑鼠操作。文章整行可開啟，網址維持優先；最左一欄第 2–23 行提供返回，hover 顯示垂直提示與向左箭頭游標。可返回畫面限閱讀、文章／看板／空列表與命令說明，未知與主選單不猜測。列光棒採低透明度藍金、2.4 秒呼吸動畫，減少動態效果停用；Shift 與拖曳保持選取，所有本機裝飾不進原始 Canvas 或 PNG。

JS 的整行／返回命中、過期及單次事件、光棒幾何與減少動態效果回歸通過（全套 116）；desktop lib 136／bin 67、core、Clippy、Fluent 與 docs 通過。macOS `pointer_rows` 探針在標題區實際點擊文章 122，接著點最左緣返回列表；本機 peer 精確核對 Down＋Enter、Left 共兩筆，沒有額外輸入。匯出 PNG、Close acknowledged、全部 worker join／listener released、退出碼 0。名單原生案例另完成閱讀頁雙擊選取與真實剪貼簿回讀。

原生畫面證據與報告保留 `.local/verification/account-lists/pointer`。工具沒有純 hover API，零距離捲動被拒絕、移入拖曳也沒有可靠取得完整列動畫；因此呼吸動畫觀感、OS 游標形狀、整行 Shift 拖曳與完整 Windows／Linux 流程仍列人工／平台待驗，未把 JS 或兩筆點擊案例當成這些結果。Windows 導覽 gate 已更新為整行與 Shift 預期，但本次沒有 Windows runner。

## 黑白名單與文字匯入（2026-10-19）

名單新增 Mask／Allow 策略；v1 黑名單載入後遷移至 v2，明確儲存才寫入。UTF-8（可含 BOM）逐行 ID／B:／W: 名單以背景工作讀取，先預覽合併再確認保存；C: 轉為本機遮黑，色碼／底線只保留分類並提示調整數，N: 與未知分類列出略過數。大小寫相同 ID 合併時白名單優先；不載入外部全域顏色或刪行設定。來源檔與本機備份保留，取消與晚到結果不能套用，詳見 [名單契約](../content-filters.md)。

驗證：desktop lib 136／bin 67、core 13 commands／2 replays、JS 116、Fluent、Clippy 通過。macOS 原生 account_lists 先保存 Alice 遮黑，再改為白名單；匯入預覽未寫入、取消恢復，確認後只新增 Bob 遮黑與 Carol 白名單，既有 Alice 例外仍保留。獨立檔案讀回主檔／備份、整頁 960×576／單行 960×24 裁切逐像素一致；只有 Bob 的內容範圍改黑，Alice 原像素不變。原生選取／⌘C 貼回未儲存草稿，確認只有 59 個方塊與作者／時間。peer 拒絕全部站端輸入，Close acknowledged、全部 owner join、程序退出碼 0。

證據保留 `.local/verification/account-lists`。首次 native 探針漏接閱讀 fixture，保留未通過記錄；修正 fixture 路由後重跑完整案例。系統 picker 使用既有 AsyncFileDialog，但這次用固定測試檔路徑，不能宣稱新的選檔視窗人工驗收；來源格式依已確認的靜態契約獨立實作，尚無原程式匯出／重新匯入往返及版本矩陣。私人名單未讀取，測試均為自製資料。

## 自訂右鍵選單（2026-10-19）

最新交付更新為 `dist/macos-context-menu/PTTStella.app`（arm64、dev debug=0、ad-hoc 簽章）。啟動明確停用 WebView 預設右鍵選單，BBS／黑邊改用深藍金色本機選單。依選取、網址、插圖與閱讀狀態提供複製、開啟、展圖、PNG、翻頁／返回，以及既有書籤／黑名單／Vim／設定入口；站端滑鼠仍由原交易管理，Shift＋右鍵才開本機選單。詳見 [右鍵操作契約](../context-menu.md)。

右鍵複製由 Rust 按已 ACK、已過濾投影取得文字或完整目的地，使用背景工作序列寫入系統剪貼簿；不接受 UI 任意文字。與 PNG 共用可撤銷的 OutputLease，保留正常快捷鍵複製。現有管理視窗入口不等同於游標下 ID 自動辨識、白名單、全文搜尋或直接加入目前文章書籤。

驗證：desktop lib 135／bin 66、全部 JS 111、core 13 commands／2 replays、Fluent、Clippy 及 docs 通過。macOS 中文原生案例確認右鍵保留選取、複製後原生貼回草稿、兩次選單展圖只一批、PNG 及選單開啟時關窗。最終英文 `context_menu` 案例以自製 OSC 8 頁尾將焦點置於合法閱讀游標與連結交集；兩次成功 Rust 複製，貼回完整本機 URL；右鍵圖片預覽實際解碼，選單展圖後 Enter 不收合／重複載入。預覽與文章內載入各取一次 colours.png，missing.png 只一次；整頁 960×576、頁尾選取行 960×24 與整頁裁切逐像素相同，無插圖／選單／游標／反白。peer 拒絕所有站端輸入，最終報告 passed、Close acknowledged、全部 owner join、程序退出碼 0。

證據保留 `.local/verification/context-menu`，包括未通過的早期 WebKit 複製案例、游標不符閱讀條件的限定網址案例及最終完整報告；沒有靠程序退出碼或單元測試冒充原生通過。座標工具的 noWindowsAvailable 仍未普遍排除，本次使用原生 accessibility 定位合成文字輸入位置；任意 DPI、完整站端 Shift 右鍵、其他 OS 與實站仍須另驗。背景寫入單元注入 writer，不碰系統剪貼簿。正常 PNG 使用系統另存視窗，本機探針固定輸出路徑不冒充另存人工驗收。

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
| 本機黑白名單 | [名單方案](../content-filters.md)：完整 ID、標題字面條件、備註、例外與可揭露遮罩；共享辨識及輸出，保留固定格線 | 三策略與驗證見本頁「兩種黑名單與白名單 ID 標色」；移除行已由新需求取代，關鍵字不在本輪範圍 |
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

2026-10-19 圖床連線相容修正：使用者指定圖片以舊版 production Loader 重現 Network 失敗；相同 HTTPS URL 在省略 User-Agent 時被中斷，帶 App 身分時回傳 200。圖片本身為有效單幀 PNG、1260×726、66,409 bytes，未超過預覽預算。HTTP client 新增 PTTStella／版本 User-Agent，production Loader 實際下載後通過同一格式／尺寸檢查。新增本機 server 回歸在舊版因缺標頭失敗，修正後核對直接請求及重新導向均帶 App 身分，沒有 Cookie／Authorization／Referer；134 項 desktop lib、fmt 與全 targets Clippy 通過。外網診斷是使用者指定的一次調查，不加入一般 gate，也不保存私人 session；診斷及前後結果留在 `.local/verification/catbox-image`。本輪只驗證下載／圖片檢查及本機回歸，未以之宣稱完整原生預覽互動驗收。

2026-10-19 Windows release 驗證中發現浮球攔截 modal 第一次操作：工具動作經 Rust 非同步開啟 dialog 後，document capture listener 仍把首次欄位點擊或 Escape 當成外部收合，preventDefault 並恢復 BBS 焦點。改為 modal 已開啟時僅收合工具，不消耗事件或移走 dialog 焦點；一般 BBS 外部點擊仍只收合。兩個回歸先於舊程式失敗，修正後完整 99 項 JS 測試通過；Windows 原生 WebView2 自動化已繼續完成字高／基線預覽、取消、保存與貼上，完整 desktop gate 與 ZIP release 結果另列，不以本項宣稱人工 IME、Mac／Linux 或實站登入通過。

2026-10-19 Windows desktop gate 暴露 peer 主動 Close 的結果遺失：receive 分支已 flush 自動回覆並移除 socket，卻未保存 last_close，shutdown 因而回報 NoConnection，嚴格 cleanup gate 正確保留 failed。補強既有 production Connection 測試，要求獨立 loopback peer 真正收到 Close，再核對 shutdown 及重複 shutdown 都為 Acknowledged，舊版先確認 NoConnection 失敗；修正僅在 flush 成功時保存 Acknowledged，失敗／逾時保留 Unconfirmed。空原因及帶 policy 原因均保留站端訊息，完整 Windows desktop lib 133 項通過；沒有改成接受 NoConnection 或放寬 cleanup expected。

2026-10-19 Windows x64 release／ZIP：新增 Windows resource build script，七尺寸 ICO 由既有同源 PNG 轉換，嵌入 EXE 並以 Tao 用於視窗／工作列；winresource 0.1.31 鎖版。打包腳本使用 Cargo JSON 的 x64 release 路徑，檢查 PE GUI subsystem、import DLL 與每份嵌入圖示 payload，從 Visual Studio redistributable 附帶 vcruntime140／vcruntime140_1，並附授權、README、manifest 及 SHA-256。既有 ZIP 不覆寫，以同卷暫存檔完成檢查後原子交付；6 項打包回歸包含競爭建立及早於 1980 的 registry 授權時間戳。
Windows 10 22H2 x64（19045）、WebView2 155.0.4283.62、DPR=1 完成 core 13 命令／2 replay、desktop lib 133 項、bin 55 項、完整 99 項 JS、docs 21 個 gate 回歸及完整 desktop gate。debug EXE 的真實 WebView2 包含 16 色／雙色字／方塊、trusted 鍵盤與組字事件、剪貼簿、單行與多行貼上及拒絕、PNG 獨立像素、浮球分類、設定／profile、hover／圖片動圖／影片、滑鼠導覽及 Vim、第二程序設定重啟像素相同；Close acknowledged、listener／connection／credential／file owner 全部 join。原始失敗與修復過程保留 .local/verification/windows-release；新 runner 以實際浮球分類操作、等待對話框初始焦點及 120ms Vim 節奏，typed 貼上拒絕與 peer 獨立 bytes 條件不放寬。ZIP release EXE 的解壓後 gate 接續驗證，結果另列；本機成功不代替乾淨 Windows、Windows 11、其他 DPI、實體 IME 候選窗、最高速按鍵、實站登入或其他平台驗收。

2026-10-19 Windows ZIP 最終交付驗證完成：從乾淨來源提交 2a38e907ea13f27f31c270a47125ad2a8fb79196 打包 dist/windows/PTTStella-0.1.0-dev.1-windows-x64.zip（19,758,723 bytes，SHA-256 bb3632cc66af55e6583ccf1a0a30df15a1481cb718c78e89d425f88795180256）。解壓至 target/verification/windows-zip-final，604 份 manifest 檔案逐一核對、七份嵌入圖示 payload 相同、PE AMD64／Windows GUI／版本資訊符合；Windows 視窗實際有 small 與 big icon handle，兩個 VC++ runtime DLL 確認由解壓資料夾載入。解壓的 release EXE 以本身目錄為 cwd、不重新編譯，完整同一 GUI gate 及第二程序保存重啟像素比對通過；EXE SHA-256 187e4a5021978e215fe290e66f3a86bcd495d25beb809b9a2dd747b20802c243，Close acknowledged、全部 owner join。驗證原始報告留在 .local/verification/windows-release/release-gui.json，產物旁提供 verification.json 摘要及 .zip.sha256；Windows 10 x64／DPR 1／WebView2 155.0.4283.62 的合成結果不冒充乾淨安裝、Windows 11、其他 DPI、實體 IME、實站登入、原生儲存 picker 或其他平台。ZIP 包含圖示、app-local CRT 與授權，WebView2 須另行安裝，未作 Authenticode 簽署。

2026-10-19 Enter 文章內展圖：依使用者選擇，在已辨識 PTT 閱讀頁於圖片網址下方展開，不以再次 Enter 收合。Rust 從目前已繪製且套用名單的 snapshot 准入，保留原生一次／i／IME／前綴與其他站的原輸入；每頁最多 8 個不同網址、合計 24 MiB payload／16M 像素，序列下載，頁面／session／過濾變更撤銷。JS 以原 renderer 的行切片與獨立圖片間隙排版，選取、網址與 PNG 保留原行列；圖片僅顯示首幀，展開時暫停閒置刷新。

macOS arm64 合成原生 App 已確認一般模式兩次 Enter 與 Vim Normal 一次 Enter，共 3 筆明確請求只開 1 個 batch；HTTP peer 記錄 colours.png／missing.png 各 1 個 GET，畫面在網址下方顯示自製紅藍圖片及 HTTP 404，沒有開 modal 或收合。插圖下方拖曳反白對準文字，選取行 PNG 指向原第 4 行。獨立 Pillow 核對整頁 960×576、選取行 960×24 且逐像素等於整頁第 4 行裁切；全圖僅來源灰階，未混入紅藍插圖。站端 peer 拒絕全部 bytes，關閉後 Close acknowledged、renderer listener／connection／credential／file owner 全部 join。證據在 `.local/verification/inline-images`；未操作使用者實站 session。

驗證包含 135 項 desktop lib、61 項 desktop bin、core／兩份 replay、Clippy、JS 幾何／快捷鍵與取消 decoder／舊 batch 回歸、封包與中英文資源。最初合成 footer 缺少離開提示及 wire examples 順序錯誤被測試拒絕，修正 fixture／順序後通過，未放寬辨識規則。獨立瀏覽器 DOM 測試頁已提供，但 browser 工具拒絕 file URL，該測試列 not_ready；原生實測與 JS 回歸各自記錄，沒有據此宣稱所有視窗大小、實站或其他平台已驗收。

最終候選 `dist/macos-inline-images-final/PTTStella.app` 已完成 arm64 dev／debug=0 打包與 strict ad-hoc 簽章。首輪最終候選在開始 UI 觀察時已斷線，操作順序不完整，保留 failed／cleanup false；同一 executable 重新執行後兩次 Enter 僅一個 batch、兩個網址各一個 GET、PNG 與 Close／全部 owner join 通過。未以重跑覆寫失敗紀錄；本輪產生的中間 App／測試副本在正常程序退出後清理，保留報告、來源 helper、產物雜湊與必要像素圖。

2026-10-19 合併 Windows 修正與最新 main：以 origin/main 的 97d0ca9（文章內展圖、情境選單）整合 Windows 分支四個提交，保留浮球 modal 事件修正、peer Close 結果與 Windows 圖示／ZIP 建置。Cargo.toml 衝突合併 arboard 及 winresource desktop 依賴，Cargo.lock 同時保留兩者；active plan 衝突保留兩邊完整紀錄。src/main.rs 同時保留停用 WebView 原生選單與 Windows icon 設定，未改寫歷史。
合併版 docs、core 13 命令／2 replay、desktop Clippy／lib／bin 66 項／實際 WebView2 gate 通過，另有 113 項 JS 與 8 項 Windows／macOS 打包回歸。初輪 GUI 保留 failed：新版右鍵先開情境選單，舊 Windows driver 仍預期直接選取；改由 trusted pointer 點選「選取完整網址」，再以相同完整 URL 讀回系統剪貼簿，未放寬 bytes 或 pixel expected。重跑 GUI 及第二程序保存重啟像素相同、Close acknowledged、全部 owner join 通過；原始結果留在 .local/verification/merge-main。Mac 原生重打包與互動仍待 Mac 端另驗，本輪未以 Windows 或 Python 的 Mac 打包回歸冒充原生 macOS 驗證；既有 ZIP 是先前來源版本的交付，不冒充此合併版。


2026-10-20 統一設定中心與浮球功能頁：浮球小面板優先沿分類扇形同側展開，空間不足才沿既有有界備援；該修正獨立提交。設定改為單一可重用原生視窗，深藍金色側欄、金色品牌星座及 Lucide／短標籤；固定中英文功能索引可跨語言搜尋、切分類並聚焦欄位。右鍵與浮球設定入口共用同一視窗，macOS 原生 ⌘, 及 WebView Ctrl+,／⌘,；capture 先攔截快捷鍵，不讓 BBS target handler 收到逗號，組字／重複／不可信事件有回歸。站點、黑白名單、帳號透過明確按鈕回到既有管理介面，不因搜尋自動連線或登入。

有效修改在 300ms 後合併自動保存；一次在途與一份最新修改，Files 忙碌後才以最新生效設定套用。無效中間值／組字不寫入，失敗保留編輯及重試；Close 等待保存，恢復預設只作用目前分類。Preferences／Files 保留唯一生效與 I/O 所有權，hover 未修改時不覆蓋主介面的最新位置。退出 App 先 drain 既有檔案，再保存子視窗最後有效修改，停止 timer／listener、join eval 並真正關閉 native 子視窗；建立 future 在取消 select 後仍保留。主畫面／PNG 與字型預覽抽出同一 renderer，未建立第二份格線或連線。設定開啟時 Rust 與 JS 同時撤銷保活 gate。

驗證：core 13 命令／2 replay、desktop lib 137 項、bin 70 項、全 targets Clippy、Rust i18n 2 項、完整 JS 129 項、Python tooling 70 項及 docs 通過。Windows driver 改以同一 owned loopback port 辨識獨立設定 WebView、保留／釋放其連線，核對搜尋、自動保存的實際檔案／像素、無效值、預設與匯入；有界 target discovery 的單元測試不冒充 Windows 原生通過。本機缺 Windows／Linux runner，兩者 native UI 仍為 not_ready。

macOS arm64 原生合成 App 已確認中文介面用 opacity／透明度搜尋、搜尋結果聚焦、不等保存完成連續切換兩個 checkbox、實際偏好檔及關閉／重開保留值。發現子視窗 WindowHides 讓 cleanup 成功後程序殘留，保留該輪 process-exit failed；退出時改 WindowCloses，重新測試設定內修改後 ⌘Q，PNG、Close acknowledged、全部 owner join、程序 exit 0 通過。最終 release 候選另以已保存的英文合成設定啟動，英文介面搜尋「透明度」、定位欄位與圖示、PNG、設定開啟時 ⌘Q 及程序 exit 0 通過；載入／搜尋未改寫設定 bytes。獨立測試 App 改 bundle identity 並重新 ad-hoc 簽署，以全部 Mach-O loadable section 雜湊核對交付程式與測試副本相同；不把不同簽章造成的 executable bytes 差異當成產品差異。工具 fixture 的前三行依語言不同，兩語 PNG 不作相同輸入的像素 gate。證據、失敗嘗試與畫面保留 `.local/verification/settings-center`；最新候選為 `dist/macos-settings-center-delivery/PTTStella.app`，strict ad-hoc 簽章通過，未公證，未操作使用者實站 session。本輪未驗原生檔案 picker／所有尺寸／實體 IME，亦不取代既有待驗項目。

補充：新增匯入後字型仍在載入的回歸，舊路徑因尚未標記待存而遺失最後 snapshot，測試先失敗；改為接受有效匯入後立即排入 autosave，再獨立等待預覽。慢字型／關閉／晚到繪製的同一生產控制器測試通過，不把預覽完成當成保存前提。


2026-10-20 閱讀／滑鼠／外觀修正：文章內展圖改由同篇有界 owner 保存 URL key，Page Up／Down 只替換已確認畫面的 link index 與行位置；每頁 8 張、同篇最多 32 張且共用 24 MiB／16M pixels 限額。離開閱讀或換 session 統一取消，不另建文章內容模型。macOS arm64 最終 release 合成兩頁往返已觀察圖片留存與重新定位，同一成功／404 URL 各僅一個 HTTP GET，兩次 Enter 只一個 batch；peer 精確要求 PageDown、PageUp、Close，PNG、Close acknowledged、全部 owner join 與程序 exit 0 通過。

系統子選單新增有結構驗證的 SystemMenu，主選單接受無裝飾括號的合法項目；最愛／看板描述中的無法解碼字不再停用整頁，但板名與操作欄仍嚴格核對。分類列表的原負例初次回歸失敗，恢復完整項目檢查後 40 項 navigation 通過；未修改負例 expected。新增 production Connection loopback 精確比對子選單移動＋CR、Back 及未知提示拒絕。滑鼠左側四分之一提供返回，網址仍優先、右側提供列表開啟，拖曳／Shift／站端滑鼠與舊版本准入維持既有規則。尚未取得使用者後續的具體漏辨識畫面，沒有宣稱所有系統表單皆可滑鼠操作；此輪子選單不是實站或原生指標點擊驗收。

設定側欄增加焦點外框的保留空間，macOS 原生鍵盤聚焦截圖已確認金框四邊完整。浮球改用 App icon 同源的材質渲染圖，依使用者回饋將金色徽章環向外延伸，保留星尖空間，上緣高光與深金下緣呈現玻璃／金屬反光；返回箭頭沿用同環。56px 操作區、透明度、慣性與 owner 不變。最終原生截圖確認三聯星與返回狀態；外觀不以字串斷言代替視覺驗證。

本輪 core 13 命令／2 replay、desktop lib 138 項、bin 73 項、全 targets Clippy、JS 130 項、Python tooling 70 項及 docs 通過。Python 首次遇到 macOS /var 與 /private/var 的同一路徑別名導致既有測試失敗，改為比對 resolve 後的絕對路徑，保留所有編譯環境／旗標與 DLL 負例斷言；isolated lane 重跑通過。失敗過程、原生畫面與報告保留 `.local/verification/browser-polish`。最新候選為 `dist/macos-browser-polish-metal/PTTStella.app`，strict ad-hoc 簽章、交付與測試副本全部 Mach-O loadable sections 雜湊一致；未公證。Windows／Linux 原生與實站登入本輪未執行，仍為 not_ready，未操作使用者的實站視窗。


2026-10-20 單欄滑鼠導覽與內建設定：新增 DocumentList，由 Rust 分別核對郵件／精華的標題、唯一欄位標頭、指令提示、連續編號與游標。點擊／翻頁／返回共用既有 planner 及 Connection sender，JS 不解析站端文字；多欄標頭、缺列、游標不符、確認提示及過期 stamp 由負例拒絕。新增 2 項 navigation（共 42 項）及 production Connection 測試，獨立比對兩類畫面的上下鍵＋CR、PageDown、Close 與拒絕後零額外 bytes。新郵件提示／快速跳轉未實作。此輪沒有原生點擊或實站信件／精華畫面驗收，實際未支援版型仍需具體畫面另補。

設定從第二個原生 WebView 改為主視窗內的 modal dialog，移除 native window 建立、弱 handle、輔助事件 channel、第二個 eval 與 debug target discovery。保留中英文搜尋、分類、同一 autosave 與字型 renderer；Modal 背景 inert，BBS key／text callback 另有開啟中的對話框拒絕。站點／名單管理待保存與關閉完成後才交接。退出由同一 renderer finally 保存最後修改快照，Files drain 後透過既有 blocking worker 寫入。替換失去用途的 native 建立取消測試，新增主 target 重用、shortcut 不穿透、管理交接等待關閉與最後快照保存回歸。

macOS arm64 release 合成原生驗證已確認同一主視窗的 ⌘,、英文 opacity 搜尋中文介面、定位欄位、連續 checkbox autosave 實際檔案、關閉重開保留值與站點管理交接。匯出 PNG 後在設定內再次修改 checkbox，在約 47ms 的同次自動化操作內直接 ⌘Q（早於 300ms autosave debounce），最後實際設定檔保留新值、Close acknowledged、全部 owner join 及程序 exit 0；peer 拒絕全部站端 bytes。浮球加入 hover 1.2 秒／1.08 倍縮放，按住停止，減少動態效果及無 hover 裝置不啟用；未取得原生 hover 連續影格，不以靜態截圖冒充動畫觀感驗收。

core 13 命令／2 replay、desktop lib 139 項、bin 72 項、全 targets Clippy、JS 133 項、Python tooling 70 項通過。初輪編譯拒絕新 UnknownReason 未窮舉，補齊 Vim 與雙語提示；JS 初輪指出測試 double 缺 document，補齊 double 並新增 modal 零站端送鍵案例後通過，未放寬產品條件。證據與過程保留 `.local/verification/in-app-settings`；候選 `dist/macos-in-app-settings/PTTStella.app` 為 arm64 release／ad-hoc，未公證，Windows／Linux 原生本輪 not_ready。使用者實站 App 與帳密未操作。


2026-10-20 依使用者新附圖修正滑鼠版型與視覺：分類列表選取標記接受第 11 欄及既有最左欄，數字欄排除游標兩格；新版看板／最愛的底列 caption 可作命令樣式錨點，精簡表頭須保留編號或總數／看板／類別，舊版人氣／板主負例仍保留。新增兩份現代指令列自製列表及一份縮排分類資料，navigation 共 43 項、production Connection 精確核對三類的 Up＋CR，未知提示拒絕後僅接受 Close。沒有從使用者私人流量產生 fixture。附圖不是完整串流，實站仍待新版回報，未宣稱所有列表已辨識。

返回命中與光棒起點分離：文章列表止於日期欄右界（零基 17 前），作者欄起可開文章；看板止於板名前（10 前）。列表標頭不攔截返回，光棒涵蓋完整列；返回僅在最左一格顯示細提示，不再以寬色塊覆蓋日期／作者。JS 核對邊界、header no-op、拖曳、網址優先、過期手勢與光棒／gutter 幾何，Windows driver 的整列 expected 改為全寬。這不代表本輪已原生點擊實站列表。

語言熱切換於保存成功後發布：Rust 預先配置兩份不可變 Fluent catalog，以單一原子選擇供所有 worker 使用；Dioxus language signal 及 macOS 原生選單同步更新。JS 保留 translator callable、原子替換驗證過的 bundles，更新搜尋與字型清單／載入提示，固定 App 連線狀態換成新語言，任意診斷文字保留。匯出名稱使用 initial_value，避免 locale render 覆蓋已輸入資料。沒有重建 renderer 或重連。

浮球增加第一層 Settings 直接入口，語意為開啟 dialog，沒有空分類面板。七入口重新計算半圓及間距；浮球、圖示、字級、點擊區及工具頁共用 1–1.75 倍邏輯 viewport 比例，不重複乘 DPR。小視窗備援與 resize 的停靠側、目前分類及慣性 owner 保持。新增七入口、900×700／1280×800／1920×1080／3840×2160 的非重疊、邊界與 resize 幾何測試；實體 4K 及其他 DPI 原生仍 not_ready。

macOS arm64 release 本機合成原生已驗證第一層設定、同一程序中文→英文→中文、原生 Edit／Window 選單、雙語搜尋、英文工具及返回、實際保存語言。最後交付包另驗字型載入提示與清單均更新，Synthetic draft 匯出名稱熱切後保留，固定連線狀態也更新。語言前後及最終 executable 的同一 BBS PNG bytes 完全相同；各 peer 拒絕全部非預期站端 bytes，各程序 PNG、Close acknowledged、全部 owner join 及 exit 0 通過。最初原生觀察發現既有狀態與字型載入提示仍為舊語言，保留截圖並補上 refresh 後另打包驗證，未以早期候選結案。

core 13 命令／2 replay、desktop lib 142 項、bin 72 項、全 targets Clippy、JS 138 項、Python tooling 70 項及 docs 通過；i18n 包含獨立 loader 往返、具名參數、無半載入狀態及 invalid JS 更新不破壞舊 catalog。初輪 Clippy 的 owned comparison 與 router 缺少 localization fixture 均保留失敗索引，補齊封包 shared example／正式 handler 後通過，沒有放寬 unknown packet。證據在 `.local/verification/live-ui`；最新交付為 `dist/macos-live-ui-delivery/PTTStella.app`，release／arm64／strict ad-hoc，未公證。Windows／Linux 原生、實體 4K 與新實站版型本輪未執行；使用者實站 App 未操作。正常退出後清理本輪中間 App／測試副本，保留報告與必要畫面。


2026-10-20 文章模式與主選單光棒：已向 PTTStar Analyzer 取得文章與滑鼠靜態研究交接。依使用者後續收斂，滑鼠增量限於原版已證實支援的行為，未證實的多欄選單、一般確認／輸入表單保留原始鍵盤，不以通用 Enter fallback 掩蓋未知畫面；原版靜態研究不代表本產品所有實站版型均已驗收。主選單／子選單 hover 光棒改完整列寬，命中與返回優先權維持既有規則。

設定「圖片與媒體」新增網頁／終端機文章偏好，version 6 讀入 v1–v5 時預設網頁，只有保存成功才生效。網頁沿用同一 Rust 有界已讀分頁快取，終端機恢復站端格線與翻頁、保留 hover／右上預覽，Rust 與 JS 同時拒絕內嵌展圖及晚到 batch。切換清除取頁／圖片 owner、文章快取、選取與暫時揭露，撤銷舊 presentation，從目前站端頁面重新開始，不自動重定位或預讀全文。

core 13 命令／2 replay、desktop lib 142 項、bin 83 項、全 targets Clippy、JS 162 項與 docs 通過。回歸涵蓋舊設定遷移／未知模式拒絕、保存重試／最後快照、模式切換的舊 ACK／展圖拒絕、網址保留及模式往返。設定索引數量更新及雙語 article-mode 別名缺漏曾使 JS 失敗，補齊後全套通過；保留原始結果。

macOS arm64 最終 release 合成原生已驗證網頁展圖、一次精確 PageDown、網頁→終端機→網頁→終端機、固定格線 PNG、重啟後的終端機及右上預覽偏好。三個成功／404 圖片網址各一個 HTTP GET；模式切換未多送站端 bytes，兩次執行皆 Close acknowledged、全部 owner join、程序 exit 0。重啟讀入與開啟設定未改寫保存 bytes。第一輪 runner 缺 loopback media origin 為 not_ready；第二輪發現舊網頁捲動位置殘留導致終端機上方被捲走，保留 failed 畫面，修正 cached→raw 布局歸零並加入回歸後重新打包實測，不以先前退出碼 0 冒充通過。

證據保留 `.local/verification/article-mode`；交付 `dist/macos-article-mode-final/PTTStella.app` 為 arm64 release／strict ad-hoc，未公證，交付與測試副本 15 份 Mach-O loadable sections 相同。原生滑鼠點擊與 hover 依使用者要求留待人工；未操作使用者實站 session，Windows／Linux 原生仍 not_ready。
2026-10-20 依使用者實站截圖回報修正非文章畫面的滾輪分流：列表即使有 overflow／放大仍送既有 PageUp／PageDown；只有網頁文章投影開啟 scrollbar，其餘畫面固定格線；未知、選取、組字、工具開啟等拒絕情境不得原生捲動或誤送。保留站端滑鼠分流與 Ctrl＋滾輪縮放。短文章投影至少保留來源格線寬高，底列頁尾位置不因內容較少上移，文章有效行數仍使用真實內容範圍。

驗證通過：core 13 命令／2 replay、desktop lib 142 項、bin 85 項、全 targets Clippy、JS 164 項、fmt 與 docs。生產 handler doubles 覆蓋各列表／選單 overflow 與拒絕情境、網頁文章捲動；Rust 覆蓋短文章 1／3／22／23 行與離開閱讀後恢復各列表原始格線。

macOS arm64 release 的 short_reading 本機合成原生案例以三行內容、80×24 來源，完成網頁→終端機→網頁切換。實際 WebView 截圖中三行文字及頁尾的像素邊界一致，空白區保留；PNG 為 960×576，零站端導覽輸入，Close acknowledged、各 owner joined、程序 exit 0。最初案例誤走預設選單，已修正 app 的 fixture 分派並重新建置／驗證；保留該次失敗記錄，不計通過。

交付 `dist/macos-terminal-wheel-final/PTTStella.app`；與原生驗證副本的 15 個可載入 Mach-O sections 相同，兩者嚴格簽章檢查通過。本機證據位於 `.local/verification/terminal-wheel/`。使用者明確授權本輪超過共用額度，僅對本輪調高 aggregate budget，保留 namespace／剩餘空間／lease 檢查，未改全域政策或刪除其他專案資料。原生滾輪／指標與實站體驗仍由使用者人工驗收；Windows／Linux 原生結果為 not_ready。

2026-10-20 修正網頁文章一般鍵盤模式的英文 j／k：原先走 textarea 編輯及原始輸入，繞過本機閱讀；現在與上下方向鍵共用逐行捲動，僅未載入的邊界取頁。Vim Normal、計數、原生一次及輸入模式仍由 Rust 政策處理；中文組字不被當成英文 j／k。JS 生產 handler 回歸涵蓋一般 j／k、方向鍵、Home／End、首尾邊界、Vim／原生模式分流與離開網頁文章後恢復原始按鍵。

core 13 命令／2 replay、desktop lib 142 項、bin 85 項、全 targets Clippy、JS 165 項、fmt 與 docs 通過。macOS arm64 實際 WebView 驗證：一般英文 j 在邊界取得一頁，之後 j／k、上下方向鍵、Home／End 往返；中文輸入法下 Vim 3j／3k、Ctrl+D／U，展圖後 j／k 往返。每組回程的 BBS 內容區像素相同，英文 j 與向下方向鍵的畫面一致；peer 僅收到一次精確 PageDown，三個圖片網址各一次 HTTP GET，PNG 960×576、Close acknowledged、全部 owner join、exit 0。

前兩次原生新版本測試仍使用中文輸入法，j 產生注音而被嚴格 peer 拒絕，保留失敗記錄；後續先在本機設定搜尋欄確認英文輸入，再驗一般模式，切回中文驗 Vim，沒有放寬 peer。合成閱讀 probe 的失敗訊息增加收到的訊息診斷；只在明確啟動的本機 probe 使用。證據在 `.local/verification/reading-keys/`，交付 `dist/macos-reading-keys-r2/PTTStella.app` 與測試副本的 15 個可載入 sections 一致，strict ad-hoc 簽章通過。本輪正常資源 harness 通過，不需額度例外；Windows／Linux 原生及實站仍待驗收。

2026-10-20 依使用者要求調整滑鼠提示：文章可見正文上／下半部使用向上／向下的金色箭頭游標，與返回箭頭共用深色外緣；返回 hover 只留下向左箭頭，刪除左緣框線、底色及其繪製分支。列表／選單的完整列光棒保留；網址、拖曳、Shift、站端滑鼠及拒絕情境沿用原命中與准入，沒有改動實際導覽命令。

core 13 命令／2 replay、desktop lib 142 項、bin 85 項、全 targets Clippy、JS 165 項、fmt 與 docs 通過；既有生產 hover handler 回歸改驗上／下／返回游標分流、返回不繪製 marker，列表全寬光棒仍成立。三份 SVG 游標及 hotspot 已檢查。macOS arm64 release 的 short_reading 原生啟動、PNG 與關閉 smoke 通過，960×576 PNG 與上輪相同 fixture 的全部像素一致，零站端輸入、Close acknowledged、全部 owner join、exit 0；這不代表已原生驗收游標 hover。

交付 `dist/macos-reading-cursors/PTTStella.app`，與原生 smoke 副本的 15 個可載入 sections 相同並通過 strict ad-hoc 簽章；證據在 `.local/verification/reading-cursors/`。標準 harness 因共用磁碟預留額度拒絕後，沿用使用者已授權的單次 aggregate budget 例外，保留其他磁碟／lease 檢查。原生箭頭外觀與滑鼠操作留待使用者人工驗收；Windows／Linux 原生未執行。


2026-10-21 全文取頁與繪圖分離：每次完整來源 View 抵達即由同一 ArticleCache 核對／接入，不等 Paint ACK；同時仍只有一筆站端命令與一個在途 Paint。閱讀准許依 generation／文章數字 ID／名單版本隔離，只接受已知顯示或在途快照；站端 input／output pending 不撤銷文章准許，當前來源完整及序列化 command owner 仍是每次取頁的必要條件。載入期間至多每 250 ms 重建全文投影，完成、取消或離開後即更新；沒有額外送鍵 task。保留取消／晚到頁、切篇、舊 ACK、失焦、選字／組字與模式切換保護。

置底文誤判變更另修正：站端頁尾是邏輯行號，不能直接當物理畫面列；保留最多一個未確認尾端空白列，僅下一頁實際格線重疊時才納入文章，不把補白一律當正文。文字／背景變更仍拒絕。獨立 expected 覆蓋空白接頁、真正變更拒絕與既有折行／末頁 clamp；實站馬術版最底下「長距離賽事區」由第一頁停載修復為 8/8 頁、166 畫面列（末頁邏輯行 143～165），End 可見末尾推文與編輯紀錄。

同篇 Equestrian #1gm7mfaU（38 頁、827 畫面列）macOS arm64 release 實站量測：首次 PageDown 至快取 complete，修改前 27.952／27.871 秒，分離但每頁繪圖版 6.941／6.182 秒，最後合併繪圖版 0.336／0.355 秒；同一 complete revision 的 Paint ACK 分別為 28.310／28.229 秒、7.415／6.559 秒、0.698／0.718 秒。各有效樣本皆精確 37 次 PageDown；採全螢幕、bundled 字型、未展圖的已連線重複讀取，並非冷啟動或 CPU／RSS 測試。排除登入／導覽／等待前景，不完整或取消的樣本不計；沒有保存文章、URL、私人流量或帳密。PTTStar 3～4 秒僅為使用者觀察，不是本案跨產品基準量測。

驗證：core 13 命令／2 replay、desktop lib 143 項、bin 114 項、全 targets Clippy、JS 186 項、fmt 與 docs 通過。新增 withholding ACK 的 38 頁接入、取消後晚到頁與新 session／文章隔離案例。macOS 實際 WebView long_load 37 次精確翻頁、837 列、Home／End 回讀、手動展圖、full_load 取消／繼續及 19 次翻頁均通過；每輪三個合成圖片網址各一次 HTTP GET，PNG 960×576、Close acknowledged、所有 owner join、程序退出。terminal_pages 原生確認只有一次手動 PageDown 才進第二頁，HTTP 零請求、PNG 與完整清理通過。實站同篇首尾頁可見，置底文不再誤判；未擴大宣稱所有 PTT 文章皆可載入。證據與數字量測保留 .local/verification/preload-independent；交付 dist/macos-reading-independent-final/PTTStella.app 為 arm64 release／strict ad-hoc，未公證。Windows／Linux 原生本輪 not_ready，原生滑鼠細節仍由使用者驗收。


2026-10-21 黑白名單推文辨識與顏色：在使用者目前公開文章重現「保存成功但推文仍可見」，測試規則隨後移除。修正 ANSI 等價亮色（SGR 1＋基本色／明確亮色）比較，分隔空白只核對黑背景與非反白，不把不可見前景色當成完整推文成立的必要條件；ID／冒號、可見樣式、時間格式、格線／來源准入仍維持。未知／引用／截斷推文不猜測作者。

保留 ID 的黑名單改為亮紅色；整行塗黑仍不畫出 ID，暫時揭露後以亮紅色辨識。白名單預設共用亮青色，可選 15 種可見 ANSI 前景色，另允許每 ID 覆寫或選回共用色。schema v4 保存全域共用色與 optional entry color，v1～v3 保留原策略並遷移至共用亮青、無覆寫；只在明確保存時改寫，備份／錯誤保留與名單上限不變。快取、來源 PNG、選取／複製及媒體皆消費同一 Rust 投影；連續推文後續 ID 的黑色合併仍優先。

core 13 命令／2 replay（filters 18 項）、desktop lib 143、bin 115、全 targets Clippy、JS 187、fmt 與 docs 通過。macOS arm64 `filter_colors` 實際表單四次保存後，WebView 可見黑名單紅 ID 與內容遮罩、共用綠 ID 及個別紫 ID；獨立 PNG 解碼確認兩列內容全黑、三種 ID RGB 為 red／green／magenta，960×576，名單 v4 寫入值相符。Close acknowledged、全部 owner joined、exit 0。首輪 probe 漏接案例分派而進主選單，保留 failed 記錄，修正分派、bin／Clippy 重跑並另打包後才驗證；沒有用該次退出碼 0 當通過。證據 .local/verification/filter-colors，交付 dist/macos-filter-colors-final/PTTStella.app 為 release／arm64／ad-hoc，未公證。使用者隨後回覆「我測試成功了」，實站名單遮罩修復記為使用者確認；Windows／Linux 原生本輪 not_ready，正文滑鼠仍由使用者驗收。

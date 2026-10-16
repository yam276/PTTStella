# Vim 瀏覽模式研究與實作方案

2026-10-16。狀態：**主要瀏覽操作已實作；Windows 本機往返通過**。使用者選定先做模式切換、`hjkl`、`gg/G`、計數與翻頁，並納入 `Ctrl+d/u`，另要求類似 Vimium 的暫時原生熱鍵按鈕，並提出使用 f 作切換入口；本方案將 f 定義為「原生一次」。本輪只處理瀏覽。以下列出已接入的操作與後續驗收邊界。

目前辨識主功能表、分類看板、看板列表／我的最愛、有日期欄的文章列表與一般閱讀頁。主選單及看板／最愛使用相同瀏覽意圖：j/k、計數與半頁送上下方向鍵，h/l 返回／進入，gg/G 送 Home／End，Ctrl+f/b 送 PgDn／PgUp；主選單的 PgDn／PgUp 由 PTT 解釋為尾項／首項。半頁依各畫面的內容高度計算。這些新增 profile 只開放鍵盤，不擴大滑鼠點擊或防閒置範圍。

主功能表要求標題、底列狀態、連續選項與第 20 欄的指標；看板／最愛要求標題色帶、編號／總數、看板、類別、人氣、板主等穩定欄位、操作列、固定板名欄與選取游標；描述欄名稱不作辨識條件，避免「中文測述」等顯示文字使正常切板暫停。分類看板使用自己的標題、七列上方區域及編號欄；主選單廣告與分類裝飾的未映射字不參與輸入決策，操作區仍須完整可映射。文章列表的命令提示可分散在上方操作列與底列，不能只在底列尋找「閱讀」。提示窗、非法游標、未完成 ANSI 等仍暫停；按鍵後的訊息會保留具體原因。

介面依 [主選單輸出／游標](https://github.com/ptt/pttbbs/blob/908bb0631a4a6db41eac85713637adf1a6058fde/mbbsd/menu.c#L534)、[選單命令](https://github.com/ptt/pttbbs/blob/908bb0631a4a6db41eac85713637adf1a6058fde/mbbsd/menu.c#L734)、[看板欄位／目錄](https://github.com/ptt/pttbbs/blob/908bb0631a4a6db41eac85713637adf1a6058fde/mbbsd/board.c#L1587) 及 [上下兩列命令配置](https://github.com/ptt/pttbbs/blob/908bb0631a4a6db41eac85713637adf1a6058fde/mbbsd/psb.c#L1110) 編寫自製 fixture。來源不代表實站部署版本；驗證結果以 active plan 為準。

文章列表按固定 cell 欄位讀日期／作者，接受緊貼人氣與日期、置底（包含日期欄「重要」）、空白標題與六位數編號；看板列表的「[→][r]閱讀／[←][q]回上層」也沿同一 BoardList 意圖轉譯。這些文字變體不會讓 h/l 恢復送出原始字母。詳見 [辨識契約](navigation.md)。

## 按鍵衝突的來源

核對公開 ptt/pttbbs commit `908bb0631a4a6db41eac85713637adf1a6058fde`；沒有登入實站或確認實站部署版本。公開原檔與 digest 保存在 repo 外的功能研究目錄；產品只記錄介面事實與自己的設計。

| PTT 畫面／按鍵 | 公開程式的行為 | 設計影響 |
| --- | --- | --- |
| 閱讀 `h` | 顯示按鍵說明 | 本機攔截後送「返回」，不能再送字母 h |
| 閱讀 `j/k` | 上下捲動一行，邊界不切換文章 | 用於閱讀的單行及半頁捲動 |
| 閱讀 `↓/↑` | 下捲／上捲；邊界可讀下一篇／上一篇 | 不拿來組成閱讀的多行捲動 |
| 閱讀 `→`／空白 | 翻頁，邊界可離開／換文章 | l 的翻頁改用受邊界限制的 PgDn |
| 列表 `g/G` | g 是板主文摘操作，G 是搜尋標記文章 | gg/G 必須本機解析，再送 Home／End |
| 列表 `Ctrl+d` | 批次刪除已標記文章 | Vim 模式絕不轉送原始 0x04 |
| 列表 `Ctrl+u` | 可落到全域的線上使用者名單 | Vim 模式轉成半頁移動，不能送 0x15 |
| 列表與閱讀的數字 | 開啟跳號／跳頁輸入 | 3j 的 3 留在本機，不能先送站端 |
| 閱讀 `l` | 在啟用 BBSLua 的組態可執行文章程式 | l 也必須映射成明確的瀏覽動作 |

來源：[閱讀外層命令與優先順序](https://github.com/ptt/pttbbs/blob/908bb0631a4a6db41eac85713637adf1a6058fde/mbbsd/more.c#L114)、[pmore 捲動實作](https://github.com/ptt/pttbbs/blob/908bb0631a4a6db41eac85713637adf1a6058fde/mbbsd/pmore.c#L2626)、[pmore 按鍵表](https://github.com/ptt/pttbbs/blob/908bb0631a4a6db41eac85713637adf1a6058fde/mbbsd/pmore.c#L2966)、[列表導覽](https://github.com/ptt/pttbbs/blob/908bb0631a4a6db41eac85713637adf1a6058fde/mbbsd/read.c#L1110)、[列表其他命令](https://github.com/ptt/pttbbs/blob/908bb0631a4a6db41eac85713637adf1a6058fde/mbbsd/bbs.c#L4303)、[Ctrl+d](https://github.com/ptt/pttbbs/blob/908bb0631a4a6db41eac85713637adf1a6058fde/mbbsd/read.c#L1443)、[全域 Ctrl+u](https://github.com/ptt/pttbbs/blob/908bb0631a4a6db41eac85713637adf1a6058fde/mbbsd/psb.c#L224)。

這些功能由站端依畫面解釋按鍵；換字型、terminal 或 Rust UI 框架不會改變它們。客戶端應在送 bytes 前解析 Vim 動作。這個範圍不需要嵌入 Vim／Neovim 或新增 PTY。

## 建議的操作範圍

| 本機按鍵 | 已識別文章列表 | 已識別閱讀頁 |
| --- | --- | --- |
| h | 返回上一層，送左方向鍵 | 返回文章列表，送左方向鍵 |
| j / k | 下一項／上一項 | 下捲／上捲一行，使用站端 j／k |
| l / Enter | 開啟選取文章 | 下一頁，到底停住 |
| gg / G | 第一項／最後一項，送 Home／End | 文章開頭／結尾，送 Home／End |
| Ctrl+d / Ctrl+u | 下移／上移半個列表視窗的項目數 | 下捲／上捲半個閱讀視窗的行數 |
| Ctrl+f / Ctrl+b | 下頁／上頁 | 下頁／上頁，到文章邊界停住 |
| Ctrl+e / Ctrl+y | 下一項／上一項 | 下捲／上捲一行 |
| 3j、5k 等 | 本機計數後移動項目 | 本機計數後捲動行數 |
| f | 下一個完整按鍵／組合鍵使用 PTT 原生熱鍵，用完恢復 Vim 選擇 | 同左 |
| i | 切到原生 PTT 輸入，不送字母 i | 同左 |
| Esc / Ctrl+[ | 清掉未完成的計數／gg；留在瀏覽模式 | 同左 |

這一版以 BBS 的層級及閱讀視窗承接 Vim 鍵位。h/l 在此是返回／進入或翻頁；文章沒有新增逐字移動的本機游標。j/k 與 Ctrl+e/y 在閱讀時都捲動視窗，沒有模擬 Vim 編輯游標和視窗各自移動的差異。整頁翻動使用 PTT 的頁距，不能宣稱與 Vim 的所有視窗細節完全一致。

列表的上一項在公開實作中可從首項繞到尾項；現有辨識器只有可見項目與游標，沒有全域列表位置。第一版單步移動沿用 PTT 列表慣例並明列差異；多步移動必須測試回繞，不可宣稱已實作 Vim 的不回繞邊界。嚴格的全域邊界需要可靠的位置事實，不能把文章編號當成連續列表索引，尤其有刪文、置底、搜尋與排序時。

計數先支援 j/k、Ctrl+e/y、Ctrl+d/u。`3<Ctrl+d>` 表示改用三行／項的捲動量，並供接續 Ctrl+d/u 使用；不是翻三個半頁。預設捲動量為內容區高度的一半取整數，至少一行；23 行閱讀區為 11 行。列表排除標頭與 footer，不能照搬閱讀區的 11。切換列表／閱讀 profile 或 session 時還原該 profile 的預設量；像素縮放不改邏輯行數。依據 [Vim 捲動命令](https://vimhelp.org/scroll.txt.html#CTRL-D) 與 [scroll 選項](https://vimhelp.org/options.txt.html#%27scroll%27)。

首版不接管 `0/^/$` 的逐字行內移動、`w/b/e`、Visual、yank、文字物件、編輯或巨集；也不把 `100G`、帶計數的 gg、跨多頁的 Ctrl+f/b 巨集偽裝成已支援。未支援的 Normal 命令明確提示、不轉送字面值。原生 PTT 搜尋、推文及其他站端功能可切換原生輸入使用；本輪不做本機全文搜尋。

## 模式與輸入所有權

預設維持原生 PTT。工具列提供切換，啟用後用低對比小字標示 `VIM NORMAL`、`PTT INPUT` 或暫停原因；不覆蓋 ANSI 內容，不使用發光色條。PTT 原生說明另外提供明確入口，避免 h 被挪用後無法求助。

Normal 只在已辨識的一般文章列表／閱讀頁接管，未映射畫面、登入、表單、搜尋提示及編輯器不猜測。已啟用的 Normal 遇到未知畫面時轉為暫停，不把剛被攔截的 g、G、Ctrl+d 等重新作為原始輸入送出。使用者可隨時由工具列切回原生操作。

暫停提示會區分畫面未接收完成、未映射字格、未辨識格式、游標未確認與格線不支援。一般列表的空白補齊日期、投票／鎖文／刪文列已列入辨識案例，不以整頁暫停迴避這些常見格式。

輸入維持兩層對應：`Keyboard` 把按鍵與計數轉成瀏覽 `Intent`，`navigation::plan` 再按目前畫面轉成 PTT bytes。例如 j 的「向下移動」在列表使用方向鍵，在閱讀使用站端單行捲動；Ctrl+d 不會在 Normal 變成原始刪除控制碼。JS 只分類按鍵／文字事件與管理編輯預設行為，沒有第二份 key-to-byte 表。原生模式仍由 Rust 編碼站端輸入；macOS 的 Command 視窗快捷鍵屬原生選單，不進 PTT 送鍵佇列。

Normal／暫停時輸入宿主為 readonly，瀏覽按鍵同步阻止文字編輯，避免系統中文輸入法把 j 等指令變成組字。WebKit 以 Process／非 ASCII key 回報時，Rust 僅在瀏覽模式按 `code` 與 Shift 解析英文字母／未按 Shift 的數字；組字進行中不搶輸入。i／原生一次會解除 readonly，中文組字仍走原本的 editing owner，不拿實體鍵碼代替使用者輸入的中文字。

i 進入的原生輸入允許既有按鍵、IME 與貼上流程。只有當畫面重新確認為可瀏覽且沒有組字時，Esc 才能本機回到 Normal；其他畫面的 Esc 保留給站端，工具列仍可切換。原生輸入是站端完整操作，不能稱作 session 唯讀。

### 暫時使用原生熱鍵

提供兩個清楚的入口，借用 Vimium 的 [passNextKey](https://vimium.github.io/commands/#passNextKey) 與 [i／Insert mode](https://github.com/philc/vimium#keyboard-bindings) 的分工：

| 入口 | 行為 | 例子 |
| --- | --- | --- |
| 原生一次 | 工具列按鈕或 Vim 的 f 啟用，下一個完整按鍵／組合鍵使用既有原生 PTT 輸入流程，然後恢復 Vim 模式選擇 | f h，PTT 收到 h，開站端說明 |
| 原生輸入 | 工具列或 Normal 的 i 啟用，維持原生輸入；以工具列返回，或於已辨識瀏覽頁按 Esc 返回 | 搜尋詞、推文內容或連續原生熱鍵 |

原生一次啟用後立即把編輯焦點還給畫面，標示「下一鍵：PTT 原生」；再點一次可取消。單按 Ctrl／Shift／Alt／Command 不消耗機會；Ctrl+d 是一個組合鍵，不能把 Ctrl 與 d 各算一次。該次按鍵的 repeat 與 keyup 由同一交易持有至釋放，不可恢復 Normal 後讓同一次長按又觸發 Vim 指令。

f 在 Vim 的 Normal／暫停狀態可啟用原生一次；原生輸入狀態中的 f 照常送站端，不攔截成切換。因此 f f 會送一個原生 f，f G 會送原生 G，f Ctrl+d 會送原生 Ctrl+d。前綴計數不套用於 f；不支援的組合整筆清除並提示，不能把數字送出。Vimium 自身的 [f 是連結操作](https://github.com/philc/vimium#keyboard-bindings)；這裡採用使用者提出的 f 入口，快捷鍵說明要寫明本產品的原生一次語義。

可列印字元須等其 input 完成才結束原生交易；不能 keydown 後先切 Normal，導致 h 的文字事件被當作另一筆命令。若開始 IME 組字，該次完整組字確認／取消才結束交易，遵守既有 Enter 隔離。已啟用原生一次時，Esc 也可直接送給站端；取消待用狀態用按鈕，不搶走這個原生 Esc。

原生一次可供暫停中的未知畫面使用，讓使用者手動操作；它不代表該畫面已被辨識。使用後若站端顯示說明／提示等未知畫面，Vim 狀態仍為暫停，不能立刻猜測新的 hjkl 意義。失焦、重連、關閉 dialog 或選擇其他模式會取消未使用的機會，不能留到另一個 session；已開始的輸入交易仍須由原 owner 清理。

「原生」指 PTTStella 既有輸入規則：系統快捷鍵、選取複製與貼上流程仍各自處理，不把 Ctrl+V 偷換成原始 0x16。原生一次的 Ctrl+d 則會按原生規則送出 0x04；這是使用者明確要求的站端熱鍵，不能再混用 Vim 半頁語義。

Vim 指令由 keydown 產生一次；被接管的可列印按鍵必須同步 preventDefault，避免又從 beforeinput／input 送一次。Rust 擁有模式、計數、g 前綴與命令解析，WebView 只根據帶版本的模式快照分流、傳結構化鍵值。IME 的 Process／229／composition 仍由既有組字 owner 處理，不得把確認候選字的 Enter 或組字文字當作 Vim 指令。

模式或 session 改變、失焦、組字開始、貼上、dialog 與滑鼠操作都取消未完成的前綴。單獨 g 不送出；下一鍵不符合時清除並提示，不能在逾時後補送 g。Normal 的貼上要求先切換原生輸入，不偷偷成為一串瀏覽指令或站端文字。

## Rust 與 Connection 的改動位置

1. 在純 Rust 新增小型瀏覽輸入 reducer，輸入為模式／畫面 context／結構化按鍵，輸出為本機狀態或 typed Intent。不要將第二份命令 parser 寫進 JavaScript。
2. 擴充現有 [navigation](../src/navigation.rs) 的 Intent，覆蓋行／項移動、列表翻頁、Home／End、列表返回及閱讀半頁；現有 plan 只支援滑鼠所需的少數動作，不能直接把所有按鍵餵進它。錯誤文字也要去掉只有滑鼠適用的描述。
3. 內容區高度與可確認的頁碼事實由同一個 analyze 邊界產生；現有 reading parser 已解析卻丟棄的頁碼，若新動作需要就保留下來，不讓 UI 再讀 footer。文章／列表身分仍未知時保持未知，不能以畫面種類相同代表同一篇文章。
4. 所有送鍵繼續走 [Connection](../src/connection.rs) 的單一序列 owner，核對 generation／revision／input epoch 及模式版本。半頁在閱讀使用有界的 j／k 批次，不能送原始 Ctrl+d/u，也不開 timer 每隔幾毫秒送鍵。
5. 計數以 checked arithmetic 解析；依 `MAX_INPUT_BYTES` 與每個動作的 bytes 成本，在配置前計算上限。超限整筆拒絕，不能截斷、溢位或排無限跨頁工作。
6. 連按需有明確的容量、取消及次序。可在一次有效畫面交易中組成同方向的有界移動；不要把滑鼠的「等待畫面時拒絕第二次點擊」原樣套到鍵盤，導致按住 j 大量遺失操作。跨頁的剩餘動作不得在新文章、彈窗或 session 繼續播放。

閱讀在 `first_line=1` 或 `percent=100` 的相應邊界直接 no-op，不送會換文章的方向鍵、空白或右鍵。gg/G 與其他可能無回覆的動作還需處理等待生命週期：目前 pending input 依 revision 改變才釋放，單純 write 成功不等於站端完成；不能在 Home 已位於首頁、End 已位於尾頁時永久卡住。無更新須有界結束等待、顯示結果未確認、取消後續操作，不重送、不冒充收到站端確認。

模式、前綴、計數及待送瀏覽操作期間暫停既有保活；清理沿用同一 owner，不增加第二個保活 timer。滑鼠導覽、PNG、ANSI 配色與雙色字仍消費原 Frame。

## 分段實作與驗收

| 階段 | 交付與必要證據 |
| --- | --- |
| V1 模式與基本移動 | 原生／Normal／暫停、原生一次按鈕與 f，h/j/k/l、gg/G、Esc；核心獨立 expected、Windows 真實 keydown 到 loopback 精確 bytes |
| V2 計數與捲動 | 3j、5k、Ctrl+d/u/f/b/e/y、半頁高度與計數記憶；文章邊界不換篇、列表回繞差異、連按及 no-op 不卡住 |
| V3 整合與跨平台 | IME／貼上／dialog／滑鼠／保活、重連及清理回歸；Mac 實際 Control 鍵與原生 IME、Linux runner 分別驗證 |

負面案例必須證明：h 不漏成說明、G 不漏成標記搜尋、g 前綴與數字不漏送、Ctrl+d/u 不漏原始控制碼、l 不漏成站端字母；模式切換競態、未知畫面、半個 ANSI frame、舊 session、無回覆與佇列滿都不能延後執行舊指令。正常原生模式仍應能輸入原本的 h、G、i 與控制鍵。

原生一次另外驗證：按鈕與 f 入口等價、f h 恰好送一次 h、下一次 h 回到 Vim；f f 只送一個原生 f；Ctrl+d 只在明確啟用後才送原始控制碼；modifier-only 不消耗；長按不跨模式重複；Esc 的原生送出與按鈕取消各自正確；中文完整組字、焦點恢復、失焦與重連取消均不漏送。

2026-10-16 已接入 Rust Keyboard policy 與單一 Connection：Vim 開關、模式提示、f／按鈕原生一次、i 原始輸入、計數與表列導覽。原生文字仍由 WebView 的編輯事件送出，按鍵事件只決定該次編輯的歸屬；一般字母不在 keydown 自行編碼。正常模式的文字編輯不送站端，組字必須先 i 或 f。

Windows 真實 WebView＋loopback 已逐筆驗證 f h、f Ctrl+d、3j、Ctrl+u、gg/G、h/l、原生一次中文組字、i/Esc 及既有輸入回歸。核心另涵蓋 modifier、repeat、取消、未知畫面、計數容量與閱讀邊界。未收到畫面更新的手動 pending 最晚在回應期限後釋放並顯示結果未確認；不重送。已辨識列表的 j/k、gg/G 與翻頁可延續同一未更新畫面的明確鍵盤操作，並允許接著進入／返回一次；進入／返回後需等待新畫面。閱讀頁僅保留純行移動的延續。每筆仍重新驗證 generation／revision 與當前 context，滑鼠不能使用此例外，不累積跨頁 macro。macOS 原生鍵盤與專用帳號的有限切板／閱讀案例已驗證，範圍見 active plan；原生 IME、Linux 及其他實站格式仍待驗收。後續實作須在 [唯一 active plan](active/minimal-browser.md) 更新實測結果，依功能分別提交。

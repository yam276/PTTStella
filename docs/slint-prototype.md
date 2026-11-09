# PTTStella Slint 離線原型

本 worktree 從 `develop` 的 `5ea9ad6a498331d0cbebc7d9803e0a51df46e5c3` 建立，包含文章自動開圖的 Dioxus 基準。原型是選用的 `pttstella-slint` 入口，預設產品入口仍是 `pttstella`。候選入口已接上共用 Connection、原生輸入、登入表單、保活、選取與 PNG，並可明確選用現有站點設定連線。文章工作區、原生媒體、草稿編輯與完整設定介面仍在移植中。下方依階段保留已驗範圍與待驗項目。完整驗收項目見 [Slint 研究計畫](slint-evaluation-plan.md)。

## 執行

```sh
cargo run --locked --release --features slint-research --bin pttstella-slint
cargo test --locked --release --features slint-research --bin pttstella-slint
cargo clippy --locked --release --features slint-research --bin pttstella-slint -- -D warnings
```

上方按鈕可切換 100%、125%、150% 與全螢幕。全螢幕可用 Esc 返回，放大後可拖動畫面以看其餘欄列。下方是獨立的原生輸入欄，按 Enter 或「驗證輸入」呼叫既有 UAO 貼上驗證，畫面只回報 byte 數或錯誤。可用 `--zoom 125 --capture NEW.png` 寫出視窗擷圖，或用 `PTTSTELLA_SLINT_TIMINGS=1` 觀察格線 raster 時間。擷圖旗標屬於原型診斷，產品 PNG 功能仍待移植。

## 依賴與繪圖

固定 Rust 1.97.0、Slint 1.18.1、Winit 0.30.13 和 skia-safe 0.153.3。`slint-research` 關閉 Slint 預設 features，明確選用 Winit、Skia、accessibility、std 與 compat-1-2。macOS 原型預設 `winit-skia-software`，可用 `SLINT_BACKEND=winit-skia` 比對 Metal。Windows 與 Linux 使用 Winit + Skia 的平台選擇，renderer 實機驗收待補。四個目標的 Cargo 依賴樹與 macOS arm64 Mach-O 連結清單均未出現 Dioxus Desktop、Wry、WebView2 或 WebKit。[Slint Winit 文件](https://docs.slint.dev/latest/docs/slint/guide/backends-and-renderers/backend_winit/)

macOS 15.7.7 的 dyld trace 顯示 AppKit 啟動時動態載入 WritingToolsUI，接著載入 WebKit 與 JavaScriptCore。Cargo 依賴與 Mach-O 直接連結清單均無這些項目。停用 Slint 預設選單的實驗在完整簽名 App 中仍出現相同的 OS 載入，因此交付版保留原生選單。原生 Slint 測試期間沒有 WebView 內容視窗或瀏覽器子程序。完全無 WebKit 映射的 macOS 執行期條件尚未達成。[Apple Writing Tools 說明](https://developer.apple.com/documentation/appkit/writing-tools)

Rust `Screen` 解讀 UAO 與 ANSI。Skia 從 `Frame` 的完整字形與左右樣式畫到 Slint `SharedPixelBuffer`。背景先畫，以共用的整數實體像素邊界裁切前景。左右半格共用字型、原點和 baseline。格線依 viewport、縮放與實體像素倍率重算。系統字型與 glyph metrics 在合成畫面載入時解析一次。幾何變更合併到下一次 event loop，同尺寸重用畫面，閒置沒有週期性終端 raster。像素上限 16 Mi pixels、每邊 8192。現階段仍會在尺寸變更時重畫整個格線。連線與長文接入後，須另量測動態更新、可視列與合成成本。

Slint 可採用桌面 Royalty-Free 2.0 條款。原型上方「關於」可開啟 `AboutSlint` 署名元件。正式公開包仍須依實際平台依賴產出並核對第三方授權與 notices，包括 Skia 及其相依套件。此輪只交付本機 ad-hoc 簽名的研究 App。[Slint 授權](https://slint.dev/terms-and-conditions)

## 效能取樣

macOS 15.7.7 arm64、M3 Ultra、1920×1080 實體 1×。使用同一 release binary、同一 1000×690 視窗、943×566 終端 raster 和合成畫面，輪流啟動三次各 renderer，未控制系統快取。下表是中位數。

| 指標 | Winit + Skia Metal | Winit + Skia 軟體 |
| --- | ---: | ---: |
| 啟動至 Rust 交付第一張格線 | 167.1 ms | 193.4 ms |
| 終端 CPU raster | 1.579 ms | 1.332 ms |
| 閒置 physical footprint | 214.0 MiB | 131.6 MiB |
| 啟動後 peak physical footprint | 863.3 MiB | 138.0 MiB |
| 閒置 RSS | 269.5 MiB | 271.5 MiB |

兩個 backend 的 100% 內容擷圖位元組相同。取樣中兩者均無子程序或 IP socket，閒置取樣的累計 CPU 時間增加 0–0.01 秒。軟體模式的 physical footprint 約少 39%，首張格線交付的中位數約慢 26 ms，RSS 相近。首張格線的時間以 binary 程序啟動至 Rust 日誌為界，不含 OS compositor 呈現。此取樣只比較 Slint 內兩種 renderer，Dioxus 同工負載、冷啟動分布、互動 p95 和動態媒體需另做量測。

## 驗收狀態

| 範圍 | 結果 |
| --- | --- |
| macOS arm64 原生畫面 | `passed`，實體 1×、原生擷圖與獨立像素檢查。覆蓋雙色「中」、不對稱背景、ANSI 色帶、括號、實心方塊、斜角、基本框線及最後一列。125%、150% 另有 Skia 視窗擷圖。原生操作驗證 150% 拖到底、全螢幕、Esc 恢復及關閉 exit 0。 |
| 2× | `passed`，Slint 強制倍率擷圖；實體 Retina 換幕 `not_ready`。 |
| macOS arm64 繁中輸入 | `passed`，已啟用的威注音以原生鍵盤事件輸入 ㄓㄨㄥ，經空白、方向鍵及 Enter 得到「中」，既有輸入函式回報 2 bytes。平台貼上快捷鍵、ANSI 預覽及終端格內組字仍 `not_ready`。AX 探針未找到 Slint 輸入欄，因此以原生座標、候選窗擷圖及視窗可見文字驗證。本輪尚未完成輔助使用驗收。 |
| Windows x64、macOS Intel、Linux x64 | `not_ready`，已檢查依賴圖。打包、各平台字型與原生輸入待驗。 |
| 功能移植與效能平替 | `not_ready`，先完成計畫要求的畫面與輸入退出條件。 |

Rust 核心 gate 共 14 個命令與 2 個 replay，Slint 像素與幾何測試 2 項，既有 JavaScript 380 項。Dioxus 的 Rust lib 220 項、bin 179 項、multi-instance 10 項通過，另有原本的 ignored 案例。兩個入口的 Clippy 皆通過。原始取樣、擷圖、探針與清理紀錄在此 worktree 的 `.local/verification/slint-phase-1/`。

## S0 共用原生格線探針

`desktop-services` 提供兩個桌面入口共用的 Connection、站點、設定、儲存、媒體及密碼庫服務。`desktop` 另啟用 Dioxus，`slint-research` 另啟用 Slint 和 Skia。兩個原生入口的格線探針共用 [513 bytes 合成測資](../src/replay.rs)、bundled 字型及只接受正常 Close 的本機 WebSocket peer。Headless 測試在每個 byte 邊界分割輸入，並核對獨立格線預期。

```sh
cargo build --locked --release --features slint-research --bin pttstella-slint
cargo build --locked --release --features desktop --bin pttstella
target/release/pttstella-slint --probe-grid --capture NEW-WINDOW.png --source-png NEW-GRID.png
target/release/pttstella --probe-surface NEW-PROBE-DIRECTORY parity_grid
```

執行 Dioxus 探針前，在隔離輸出目錄新增以下 `probe-settings.json`。於原生視窗開啟「擷取 → 匯出 PNG」，取得 `export.png` 後關窗，檢查 `surface-probe.json` 的 `status: passed` 與 `disconnect: acknowledged`。macOS 原生工具若讀不到裸執行檔的 AX 視窗，將它放入獨立測試 App bundle，以相同參數啟動。

```json
{
  "version": 6,
  "display": {
    "fonts": { "kind": "preset", "preset": "bundled" },
    "font_size": 23,
    "baseline": 20,
    "font_width": 100
  },
  "hover": "off",
  "language": "zh-TW"
}
```

macOS arm64 兩版皆收到合成格線並確認正常 Close。Dioxus 由原生 AX 點選匯出，Slint 同時取得實際視窗和來源格線。獨立 PNG 解碼核對兩版的 960×576、16 色、雙色「中」的左右筆畫／背景與最後一格。字形反鋸齒各有差異，本輪未測同負載效能。證據與本機 runner 保存在 `.local/verification/slint-s0/`。

## S1 終端游標的原生輸入法探針

```sh
target/release/pttstella-slint --probe-ime
```

點第 22 行的淺藍底線。原生 Slint 文字元件放在 Rust Screen 游標對應的實體像素區域，處理 preedit 和候選窗。鎖定版 Winit 的事件觀察器只在視窗與該輸入區有焦點時驗證已提交文字。探針回報次數與 UAO bytes 數，保持離線。

macOS arm64 實測威注音候選窗錨定第 22 行。合成原生鍵盤事件輸入 ㄓㄨㄥ 並按 Esc 取消，再重新組字與選字。只有一次「中」提交，編碼為 2 bytes，視窗正常關閉。擷圖、單筆提交紀錄及探針留在 `.local/verification/slint-s1/`。此證據涵蓋 IME 的錨點和提交邊界，終端 raw key／剪貼簿路由仍待整合。

Windows x64 離線包由 Apple Silicon macOS 使用 Rust 1.97、cargo-xwin 0.23.1、Xcode Clang、Rust LLD 和 Windows SDK 交叉建置。最初的 ARM Linux 容器在編譯初期停住，macOS 建置重用已下載的 SDK。成品為 PE32+ x86-64，匯入表無 WebView，依賴 Microsoft Visual C++ x64 runtime。ZIP、SHA-256、PE 匯入表及產物清單均已核對。Windows 原生啟動、注音與兩種 renderer 的結果仍為 `not_ready`，由使用者在 Windows 上執行包內步驟驗收。 [cargo-xwin](https://github.com/rust-cross/cargo-xwin) · [Microsoft Visual C++ Runtime](https://learn.microsoft.com/en-us/cpp/windows/latest-supported-vc-redist/)

## S2 連線與手動輸入

```sh
cargo build --locked --release --features slint-research --bin pttstella-slint
target/release/pttstella-slint --probe-session
target/release/pttstella-slint --probe-input
```

`--probe-session` 的第一個本機 WebSocket 連線先顯示 `SESSION 1/2`，再由 peer 關閉。點「重新連線」後應顯示 `SESSION 2/2`，再關窗。`--probe-input` 先點終端區，在繁中輸入來源下送 Ctrl+X、Ctrl+Z、Ctrl+A，切英數輸入 `p`，再用注音組一個「中」並取消，重新組字並確認一次。應顯示 `INPUT 5/5 VERIFIED`。兩個探針都須正常關窗並回報 `close: acknowledged`。peer 拒收多餘資料或錯誤順序，輸入探針分別核對 `18`、`1a`、`01`、`70`、`a4 a4` 五筆 binary message。

兩個入口共用 Connection watch、單一 writer、編碼與 Rust 鍵盤規則。Slint 合併 raster 更新並拒絕過期 generation／revision。組字期間由原生 IME 顯示候選，提交時核對原本代次與目前焦點，已取消或跨代次的輸入不送出。切站途中禁止輸入，舊傳輸關閉後才會開始連新站。

另有明確啟用的研究入口 `target/release/pttstella-slint --connect`。它只讀研究版 namespace 的 `settings.json` 和 `sites.json`（目前為 `PTTStellaSlintResearch`），以該處保存的字型設定顯示，開窗連一次保存的預設站，並可從選單切換已有的收藏站或重連。無效預設站使用既有 PTT fallback，資料檔維持原狀。原型目前支援手動按鍵及有 preedit 的 IME 提交。站點編輯仍待接入，登入與保活的移植見下方紀錄。一般探針只連本機，這輪未執行 `--connect` 的實站登入。

Rust 的本機 peer 測試已核對五筆精確 bytes、重連畫面代次、舊代次輸入攔截和正常 Close。macOS arm64 原生截圖已取得第一代畫面。後續 App 可顯示視窗，但自動化工具無法取得前景，第二代畫面、原生鍵盤及原生組字到 peer 仍為 `not_ready`。`.local/verification/slint-s2/` 保存截圖、完整失敗日誌與重跑步驟。

## 原生剪貼簿貼上探針

```sh
target/release/pttstella-slint --probe-clipboard
```

點終端後按 Ctrl+V 兩次，各等畫面更新。已辨識的 PTT 編輯器會收到 `16`，畫面顯示 `EDITOR CLIPBOARD 2/4 VERIFIED`。將以下合成文字放入系統剪貼簿。第一個 `\x1b`、第二個 `\x1b` 及 `\r\n` 均為實際控制字元，可由 shell 的 `printf` 產生。

```sh
printf 'ANSI clipboard check:\033[31m中\033[0m\r\nEND'
```

按 Cmd+V 或 Ctrl+Shift+V。預覽應有兩行，ESC 顯示成 `␛`。取消一次並確認頁面維持 `2/4`。再貼上、點「送出文字」，等 `3/4` 後按 Ctrl+V，確認 `4/4` 並正常關窗。peer 驗證三次站端預覽命令、中間恰好一筆 ANSI 貼上與 Close。此探針與 Dioxus 共用測資和獨立的預期 bytes。

實作以單筆背景工作讀取純文字。一般文字沿用站點編碼與容量檢查，單行不附加 Enter。多行或合法 ANSI SGR 必須確認，只有辨識到的 PTT 編輯器接受 ANSI。開始貼上、確認及 Connection 寫入前都比對 generation、revision 與 input epoch。失焦、改變視窗尺寸、切站或畫面更新會使待送內容失效。原生右鍵貼上已接入下述 S3 選單，沿用同一確認與 stamp 檢查。

可用 `--probe-input` 補驗單行系統貼上。完成前三個控制鍵後，把沒有尾端換行的 `p` 放入剪貼簿，於終端貼上並確認畫面到 `4/5`，再完成原本的注音步驟。

本機 Rust peer 已驗證單行、精確 ANSI bytes、舊畫面／舊連線拒絕、重複提交拒絕與正常關閉。系統剪貼簿、預覽視窗焦點及取消操作仍待原生桌面驗收。測試紀錄在 `.local/verification/slint-paste/`。

## 前景保活

Slint 接既有 Connection 的唯一保活計時器，正式間隔維持 60 秒。原生視窗有焦點、終端輸入有焦點、目前 Frame 已交付 raster，且 Rust 辨識為列表或閱讀頁時才給許可。組字、剪貼簿讀取、貼上預覽、登入視窗、原生工作、關於視窗、拖曳、縮放重畫、切站或失焦都撤銷許可。UI 回報 generation、revision 和 input epoch，Connection 在送出前再核對畫面與操作狀態。暫停／恢復即使合併成同一 watch 更新也會重計間隔。連線列提供「防閒置」切換。

```sh
target/release/pttstella-slint --probe-keepalive
```

此本機探針用同一 Connection，把閒置間隔縮成 3 秒。先讓視窗保持背景超過 4 秒，應保持合成文章列表。再點終端，1–2 秒內切到其他應用程式並停留超過 4 秒，返回後應重新等待滿 3 秒才顯示 `KEEPALIVE VERIFIED`。peer 只接受一筆精確 `0c 0c`，之後需正常關窗。可在刷新前另驗關於視窗、組字及防閒置開關，最後讓終端恢復允許狀態完成探針。

本機 Rust 測試已用 production Connection 驗證背景零刷新、重計間隔、合併暫停／恢復、精確兩個 Ctrl+L、未知畫面停止與 Close。原生桌面失焦、跨螢幕及睡眠喚醒仍待驗收。中斷前的紀錄位於 `.local/verification/slint-keepalive/`，後續共用核心回歸記於 `.local/verification/slint-login/`。

## 原生登入表單

`--connect` 的「登入」開啟 Slint 原生表單。只有按登入才準備帳密，預設不保存，勾選後由既有 OS vault adapter 保存到 Slint 研究版專用 service，不讀正式版項目。重新開啟只顯示帳號，已存密碼不回傳 UI。留空按登入時由 Rust 取用，未勾選且提供密碼時沿用完全不讀寫 vault 的單次登入路徑。移除動作只作用於本 App 的預設項目。取消、送出及關閉清空 UI 密碼欄，切站會使待送結果失效。每次只有一個 native vault worker，閒置時沒有輪詢 timer，關窗仍持有並 join 未完成的原生工作。

表單提交前與 vault 完成後，都核對已呈現的完整畫面 stamp 與精確 PTT 帳號提示，再交給既有 Connection 單次登入。帳號、密碼分段提示及 2FA 手動引導沿用 Rust 核心。原始終端鍵盤登入繼續可用。

```sh
target/release/pttstella-slint --probe-login
target/release/pttstella-slint --probe-login-2fa
```

兩個入口只連本機，使用 Slint 專用 synthetic vault service，與 Dioxus 探針和兩種正式／研究登入 service 都不同。帳號 `StellaTest` 與密碼 `synthetic+42` 是合成測資。第一個入口按登入後，peer 核對帳號與密碼各一筆，看到合成主選單後關窗。第二個入口還需在終端明確輸入 `123456` 與 Enter，看到拒絕後輸入 `ABCDEFGH` 與 Enter，進入主選單後關窗。peer 拒收重送帳密或額外按鍵。明確勾選保存會寫 synthetic vault，完成驗收後以表單移除該筆。

Rust 測試用未勾選保存的同一 native worker／consent reducer，核對完整登入、分段密碼提示、舊請求拒絕及兩次手動驗證。該自動測試不存取系統密碼庫。[登入契約](login.md)

macOS arm64 已以同一個 Slint 生成元件和 Winit＋Skia 軟體後端擷取原生表單，檢查帳號欄、密碼遮罩、保存勾選預設關閉及取消／登入按鈕。對該實際元件派送合成 Tab、文字及 Enter，核對一次含密碼提交、清空後一次空白提交，並保留遮罩與清空後的擷圖。此 visual runner 只檢查元件，沒有啟動 Connection 或 vault。OS 可信按鍵、原生焦點切換、保存／重開／移除、各 OS vault 與實站登入仍待驗收。來源、runner、擷圖及 gate 紀錄在 `.local/verification/slint-login/`。

## 選取、複製與 PNG

```sh
target/release/pttstella-slint --probe-selection
```

此入口顯示同一 parity grid，只連接受 Close 的本機 peer。拖曳選取後，Ctrl+C／Cmd+C 複製純文字，加入 Shift 複製含 SGR 66 的 ANSI。沒有本機選取時，Ctrl+C 保留原始終端控制鍵。終端操作列提供純文字、ANSI、全選、清除、整頁與選取行 PNG 的存檔或圖片剪貼簿操作。Esc 清除選取。放大後可用滾輪捲動，或按「拖曳捲動」暫時切換到原型既有的拖曳平移，按「選取文字」恢復。

選取沿用 Rust `selection::extract` 的完整字形與連續行範圍。Slint 的反白矩形與 raster 分層，同一 cell 內移動不重算文字，拖曳不重畫終端。格線命中沿用 renderer 的整數實體像素邊界。輸入、失焦、resize、切站、來源更新、斷線或關閉都使舊選取與尚未送出的輸出失效。斷線後仍可對最後確認的畫面重新選取並匯出。

PNG 由背景 worker 使用同一 Rust renderer 產生。整頁固定 960×576，選取行固定 960px 寬，每行 24px，包含完整行寬，排除反白、游標、IME 和工具。原生另存視窗有 parent，選檔後與原子寫入前再次檢查來源，副檔名須為 `.png`。同時只允許一個輸出工作，關閉時撤銷 lease 並 join 自己的 worker。剪貼簿 owner 由視窗保留，與原生貼上共用，支援 X11 寫入後的 ownership。Dioxus 和 Slint 的輸出 lease 共用相同 Rust 檢查。

Rust 回歸已核對反向拖曳雙色字只產生一個「中」、ANSI 重放保留左右樣式、分數像素邊界、舊 input epoch／revision／連線拒絕，以及獨立解碼的 960×24 行 PNG、固定左右背景與游標排除。可信滑鼠拖曳、OS 剪貼簿讀回、另存視窗取消與跨平台操作仍待原生驗收。證據和 runner 保存在 `.local/verification/slint-selection/`。

macOS arm64 的原生元件驗證已掛載生產 `Output` owner、renderer 和 Slint `TouchArea`，並連到只接受 Close 的同一 Connection peer。從 Slint 視窗事件入口派送合成按下／移動／放開，核對反向拖曳得到一個雙欄字形，再清除選取並正常關閉。獨立 ImageIO 解碼比對兩張原生擷圖，差異只在該字的 24×24 反白區。此測試未寫入系統剪貼簿，OS 可信滑鼠與原生另存操作仍待驗收。


## S3 Vim 模式與 session 邊界

Slint 原生輸入 owner 現在提供 Vim 開關、原生一次按鈕與模式／計數狀態。沿用共用 `vim::Keyboard` 的 Normal、原始輸入、原生一次、暫停、計數與提示行策略。`i` 轉原始輸入，已辨識畫面下 `Esc` 或 Ctrl+[ 返回 Normal，`f` 只交付下一筆原生操作。Normal／暫停時，終端 TextInput 設為唯讀並對同一 Winit window 關閉 IME；原始輸入與原生一次恢復 IME。這個設定只在終端欄位擁有焦點時套用，登入與其他文字欄位保持各自的原生輸入所有權。模式、前綴與按住狀態同步保活 gate。

來源改代次會清除未完成前綴、按住與組字 lease、上一個 session 的半頁捲動距離，保留使用者已選模式。Dioxus 也改用同一 `new_session`。已辨識的驗證碼提示沿用原始輸入交接，切到非精確 PTT 目標則關閉 Vim。模式切換不送 bytes。自行關閉 IME 產生的 Disabled 事件保留實體按鍵的按住保護，避免原生一次後的 repeat 變成 Vim 命令；沒有組字 lease 的晚到 Commit 不消耗新掛上的原生一次。

```sh
cargo build --locked --release --features slint-research --bin pttstella-slint
target/release/pttstella-slint --probe-vim
```

此探針重用既有 46 步嚴格本機 peer。開窗後手動啟用 Vim，等右上 `testNN` 更新後才做下一個操作。00–18 每五步依序 `j`、`k`、`gg`、`G`、`l`，19 用 `h`。20–29 偶數步 `l`、奇數步 `h`。30 用 `gg`，31 用 `4j`，這兩步刻意沒有 redraw；32 用 `l`、33 用 `h`、34–36 各按並放開 `j`，37–45 依序 `j`、`k`、`h`、`j`、`k`、`gg`、`G`、`h`、`h`。到 `test46` 後關窗。錯誤或多送的 bytes 會使 peer 失敗，提前關閉不能取得通過結果。此探針要求逐步的實際輸入，不能用單純開窗取代。

Rust 回歸已經由 Slint 的 Input 狀態、共用 Keyboard 與 production Connection 核對這 46 步精確 bytes，包括分段同步畫面、兩筆沒有 redraw 的連續命令與正常 Close。另有 CJK／Unidentified 實體鍵對映、原生一次／IME Disabled 的 repeat 保護、一次組字提交、晚到 Commit、跨代次前綴及捲動重設、驗證碼與非 PTT 交接測試。這些是 headless 本機測試，不代表 Winit 收到 OS 可信按鍵。

macOS arm64 以實際 Input owner、同一生成 UI、Winit＋Skia 軟體後端與只接受 Close 的本機 peer，派送模式切換回呼，核對唯讀、pending、modal 拒絕和三種模式的原生截圖。沒有派送站端按鍵或寫入帳密／剪貼簿。OS 按鍵、CJK 輸入來源下的 `i`／`Esc`／`f` 往返、切焦點／換站與 Windows／Linux 仍為 `not_ready`。當時尚未移植的滑鼠導覽、連結與站端滑鼠後續記於下節；右鍵選單和浮球仍待移植。本輪 runner、截圖、原始 gate 輸出及清理紀錄在 `.local/verification/slint-vim/`。

本輪 Slint bin 17 項、共用 lib 221 項通過。Dioxus lib 221 項、bin 179 項、multi-instance 10 項通過，保留原本 3／1／1 項 ignored；兩個入口的 all-targets Clippy、既有 JavaScript 380 項、core 的 14 個命令與 2 個 replay、docs gate 均通過。這些結果不取代上述尚未執行的原生輸入案例。


## S3 原生滑鼠導覽與網址

原生格線與留白現由同一個 Slint TouchArea 接收，Rust 以已呈現的 `Frame`、共用 Context 和實體像素格線判定。文章／看板／單欄選單的整列開啟、各 profile 左緣返回、閱讀上／下半翻頁、可視區和格線首尾窄帶、列表整條狀態列 End，以及黑邊的返回／進入／上下頁，沿用既有 typed intent 和 Connection planner。未知畫面沒有 Enter fallback。連結從共用 `links::detect` 取得，包含 plain URL 與 OSC 8；網址優先於返回與翻頁，ContinuePrompt 優先於網址。一般連結明確點擊後才由正常連線入口交給 OS browser，合成探針只回顯目的地。

按下與放開由同一選字 owner 保存 stamp、viewport、raster 尺寸與位移。相同命中且未跨 3 邏輯像素才准入點擊；Shift、拖曳、cancel、組字、modal、失焦、畫面／input epoch／縮放／捲動變更都撤銷。拖曳仍用原選取與 PNG 路徑，普通文字雙擊可選整行。首次點擊已啟動連結或導覽時，Slint 的 double-clicked 在第二次 release 之前消費延續事件，不在新畫面重送或選字。站端滑鼠協商為 Station／Blocked 時先擋本機導覽，Shift 仍可本機選字；原生 SGR 回報 owner 的後續接線與驗證見下節。

一般滾輪對已辨識列表／選單／閱讀／說明頁送 Page Up／Down，選取、組字、modal 或拖曳期間不送。Ctrl＋滾輪以 25% 階段縮放，放大溢出後交還 Flickable 做本機捲動，拖曳捲動模式維持明確開關。來源更新期間不重複排入指令。未開始的排隊導覽以 Connection 的原子 cancellation token 立即撤銷，與 Slint future 何時再 poll 分開；已開始的傳輸依既有規則處理，不能回滾 bytes 或宣稱站端成功。關窗撤銷 token 和 owned UI job。

hover 光棒是獨立的整列 28% 藍底、金框與左右 3px 金邊；返回與翻頁使用原生方向游標，網址顯示完整目的地。Context／連結按 stamp 快取，同一行內移動重用同一 model，不重畫 terminal raster。這輪光棒為靜態，OS 減少動態效果感知、呼吸效果與金色自訂箭頭仍待接入。右鍵選單、浮球和推文 ID 優先操作屬後續 S3／S4 項目，尚未以這輪結果結案。

```sh
cargo build --locked --release --features slint-research --bin pttstella-slint
target/release/pttstella-slint --probe-pointer
```

原生人工重跑順序為點 `(U)ser`、在合成未知設定頁按原始左鍵盤方向鍵返回、點 `(F)avorite`、點第一個看板列、滾輪向下、滾輪向上、點第三篇文章整列、點左緣返回，再關窗。等待每個新畫面安裝後才做下一步。peer 依序核對 Down＋CR、Left、CR、Up＋CR、PageDown、PageUp、Down＋CR、Left 和 Close，提前關閉或額外 bytes 都不會通過。網址可點或雙擊，探針不開外部網站。最末頁再往下不得送鍵。

Rust 測試覆蓋 profile／標頭／左右分界、全狀態列、黑邊、分數像素與裁切、網址／繼續優先、未知／空頁／zero／NaN 滾輪拒絕，以及 Shift／拖曳／雙擊取消、幾何和 source 變更。另以單執行緒 production worker 和嚴格 Close-only peer 證明 cancellation 在 reply 仍存活且 UI future 尚未 poll 時生效，input epoch 與站端 bytes 均不變。

macOS arm64 以實際 Output owner、同一生成 UI、Winit＋Skia 軟體後端派送合成 toolkit pointer／wheel，完成上述 8 個精確 peer 階段，核對實際拖曳選字、整行雙擊、失效 presentation、pending 重複點擊、未知畫面、最末頁不送鍵、網址優先與雙擊只觸發一次。browser callback 被測試 owner 接收，沒有外開網站或寫入剪貼簿。相同行移動確認 model 身分不變。OS 可信滑鼠、跨 DPI／實體縮放捲動和 Windows／Linux 仍為 `not_ready`。來源、runner、native 截圖與 gate 紀錄在 `.local/verification/slint-pointer/`；原有雙色字選取元件亦重跑，獨立 ImageIO 比對仍只有一個 24×24 的 576 像素反白區改變。

本輪 Slint bin 23 項、共用 lib 222 項通過。Dioxus lib 222 項、bin 179 項、multi-instance 10 項及既有 JavaScript 380 項通過，保留原本的 ignored 案例。兩個入口 all-targets Clippy、core 的 14 個命令與 2 個 replay、docs gate 均通過。ImageIO 核對 hover 前後只有指定的 960×24 整列改變，其他終端與 UI 像素相同；該浮層未寫入 Rust Frame 或 PNG source。臨時 native runner 已退出並移除，active target、最新 Windows S1 包和獨特驗證證據保留，exact paths 與空間取樣見此階段的 `cleanup.json`。


## S3 站端滑鼠 owner

原生 TouchArea 現先交由 `StationMouse` 分流，再交回本機 Output。完全消費共用 Rust 的 Local／Station／Blocked、tracking／encoding、mode epoch、policy epoch 及 active lease。1000／1002／1003＋1006 的三鍵 press／release、1002 held motion、1003 passive motion 和垂直 wheel 走同一個既有 Connection writer，不在 Slint 重解控制碼或組合封包。未支援模式阻擋本機誤導航，工具列可停用回報以恢復本機操作，偏好依既有每次啟動及重連規則處理。

Shift 在按下時鎖定本機選字，放開 Shift 後仍可完成同一拖曳，不切回站端。Ctrl＋滾輪及放大後 viewport 捲動留在本機。Command 不作 SGR modifier，Alt／Ctrl 依共用 encoder 處理。一般按鍵、組字、貼上、modal、失焦、隱藏、resize／DPI、縮放、pan、capture cancel 與關窗撤銷舊工作。原生按鈕快照目前來自同一個 Winit observer 已觀測的三鍵按下／放開，move 再比對 held mask；這不代表已驗證 OS 漏事件時的實體按鈕狀態，真正失焦／capture／多指標及裝置邊界仍須原生驗收。

按鈕交易保留原 lease，正常 frame／input epoch 更新不取消。main 在安裝新畫面時暫時清除 size／presentation，StationMouse 保留最後已確認的格線尺寸供原交易放開與 motion 使用；新 press 和 passive sample 仍要求完整當前 presentation。實際 viewport、raster 位移或尺寸變更會撤銷。已排入 release 尚未反映至 active snapshot 時，不建立新 press。模式 off／on 即使結束在相同模式仍依 epoch 取消；不補出合成 release，不重試舊事件。

UI 只有一筆 held（最長 30 秒）、最新 motion、最新 wheel 和不足 40 邏輯像素的餘量。共用一個 owned single-shot timer，每 34ms 至多送最新 sample，wheel 優先，release 前可送最後一筆原交易 sample。monotonic 與 wall clock 同時限制 sample 為 250ms 內，UI 卡住、時鐘跳動或睡眠後丟棄舊 buffer。取消同時撤銷既有優先 watch policy，涵蓋已入列的無按鈕 sample；共同 Session 保留 20 press／秒、30 sample／秒、queue release 餘量及 30 秒 hold 的既有限制。關閉後停止 timer、撤銷 policy，後續 toggle callback 不再改偏好。沒有新增送出執行緒或輪詢 timer。

滑鼠移動不重畫格線、不在每個事件重建 Fluent 狀態字串；只在回報狀態或 held 改變時更新相應 UI。Local 模式的普通按鍵／幾何清理不重複寫 mouse revoke watch。共用 `Geometry::for_ui` 維持本機選取、導覽與站端 cell 命中的相同座標來源。

```sh
cargo build --locked --release --features slint-research --bin pttstella-slint
target/release/pttstella-slint --probe-station-mouse
target/release/pttstella-slint --probe-station-mouse-drag
target/release/pttstella-slint --probe-station-mouse-motion
target/release/pttstella-slint --probe-station-mouse-wheel
target/release/pttstella-slint --probe-station-mouse-round-trip
```

這五個入口重用原產品嚴格 peer，不改獨立 bytes expected。基本案例在零基 col 4／row 4 左鍵點擊，1001 時點擊／滾輪不送，Shift 拖曳選字，停用後在目前文章整列點擊一次（CR），重新啟用後同格右鍵點擊及中鍵按住，模式 off／on 後才放中鍵，不能送該 release，最後關窗。Drag 從 col 4／row 4 按住左鍵移至 col 10／row 4 再放開；Motion 先點同一 anchor，等 1003 後移至 col 10／row 4；Wheel 在 anchor 向下、等提示後向上。Round-trip 按住左鍵進入黃色 A，等 READY 再去青色 B，等重繪後回 A 才放開。看到最末的 unavailable／完成場景後關窗，各 peer 要求 Close 並拒絕多餘 bytes，不能以只開窗取代。

8 項 reducer 案例核對原 lease 跨重繪、Shift 釋放、最新 sample／wheel 優先、34ms 節奏、版本失效、250ms／wall clock 過期、漏按鈕、Meta release、幾何／policy／hold timeout、Block／cancel 清理、未呈現與未支援模式，以及最後 motion 在 release 之前。macOS arm64 另用真正 Winit＋Skia 軟體視窗、生成 TouchArea、production Output／StationMouse 和真正 timer 完成五個嚴格 peer 流程。Round-trip 在兩次正常重繪與暫時 presentation 清空後仍保留相同 active lease，放開後無 held，全部 Close acknowledged；finish 後 callback 不能再切偏好。本機導覽八階段及雙色字選取元件也重跑通過。

這些 native component runner 使用 toolkit 合成 pointer／modifier 與明確標示的合成按鈕快照，沒有 OS 可信滑鼠、剪貼簿或 browser 寫入。基本 runner 早期等待泛用 Blocked、以直接 toggle callback 留下舊 double-click 時序、以及上一筆 release 尚未處理就連續 press 的失敗日誌保留；後續明確等 tracking、完整畫面及 active／epoch，從同列另一位置測整列開啟，沒有修改 peer expected 或產品送鍵去配合。runner 的 submodule 路徑修正與失敗日誌亦保留。來源、五組日誌、原生截圖、gates 與 exact cleanup 在 `.local/verification/slint-station-mouse/`。

OS 可信 Shift／Ctrl＋滾輪、原生失焦／capture、實體 DPI／裝置、系統睡眠與 Windows／macOS x64／Linux 仍為 `not_ready`，不能由上述合成事件認定完成。一般右鍵選單、浮球、文章工作區、媒體／編輯器及跨平台包／資料與同負載效能仍按完整計畫推進。

本輪 Slint bin 31 項、共用 lib 222 項通過。Dioxus lib 222 項、bin 179 項、multi-instance 10 項及既有 JavaScript 380 項通過，保留原本 ignored 案例；兩個入口 all-targets Clippy 和 docs gate 通過。既有兩組 native overlay 的 ImageIO 比對維持 576 與 23040 像素的既定範圍。臨時驗證執行檔在五個程序退出後移除，保留最新測試包、active build cache 與獨特證據；完全相同的 PNG 以 `screenshot-aliases.json` 對照，exact removed paths、du 配置量和 filesystem free 取樣分列於 `cleanup.json`。


## S3 原生右鍵選單

終端內容以 Slint 視窗內的選單浮層取代 DOM 選單，不建立 WebView 或額外 OS popup。右鍵按下／放開須來自同一已呈現 frame、選取、格線、滑鼠模式與 route，移動達 3 logical px 即取消。保留原選取、清除舊左鍵 gesture／hover／待導覽，選單與等待期間暫停保活及站端滑鼠。站端已接管滑鼠時，Shift＋右鍵走本機選單，即使先放 Shift，右鍵 release 仍屬同一本機 gesture。

原產品固定的項目順序、可用性與優先規則移入 `assets/context-menu-policy.json`。Rust `context_menu` 解析一次為封閉型別；既有 Dioxus 選單與媒體控制器消費同份資料，保留原來的動作 owner。Slint 只註冊目前已有 owner 的動作：純文字／ANSI 複製、完整網址複製與選取、瀏覽器外開、整頁選字、整頁／連續選取行 PNG 的檔案及剪貼簿輸出、返回／前後頁與編輯器貼上。圖片、帳號政策、推文、收藏、站端上色及設定入口仍須在 S4–S7 對應 owner 移植後註冊；本輪不以隱藏未移植項目宣稱功能完整。

Shift＋右鍵必須等同一 Connection writer 確認撤銷，才能保存選單來源。priority watch 先使排隊的舊滑鼠樣本失效，有界 command 只回覆撤銷後已發布的快照；等待、選單和動作執行都驗證 generation／revision／input epoch、幾何、選取及滑鼠政策。晚到回覆以 request identity 隔離。核心同時修正快速首包的生命週期發布順序：先發布 Connected 的完整快照，再允許待繪通知更新滑鼠模式，避免新模式搭配舊世代／離線狀態。保留既有 frame 合併與待繪期間禁止互動的契約。

選單取得自己的 FocusScope，Up／Down 跳過停用項目，Home／End 到首尾，Enter／Space 只在非 repeat 時執行；Escape／Tab 關閉並恢復終端焦點。外點消耗整個 gesture；尺寸、焦點、畫面或選取變動會撤銷快照。失焦不搶回焦點，關閉中不復活。項目有 accessibility label／enabled／default action，實際平台 AX／screen reader 驗收仍為 `not_ready`。選單在小視窗內保留邊界、超高時可捲動，無額外常駐 timer。

可用同一 Cargo 入口手動驗證：

```sh
cargo run --release --locked --features slint-research --bin pttstella-slint -- --probe-context-menu
cargo run --release --locked --features slint-research --bin pttstella-slint -- --probe-context-menu-station
```

若要重跑完整 App 接線驗證，可另加 `--verify-context-menu` 與尚不存在的輸出目錄。它使用同一二進位、session watcher、繪圖／輸入／選取／選單 owner，實際派送 toolkit 事件；會自動完成後關窗，由原 peer 驗證零額外 bytes 與 Close。這條路徑補抓了初輪新 probe enum 漏掛 owner 的問題，不能以獨立元件組裝取代入口驗證。

```sh
cargo run --release --locked --features slint-research --bin pttstella-slint -- --probe-context-menu --verify-context-menu target/context-menu-local
cargo run --release --locked --features slint-research --bin pttstella-slint -- --probe-context-menu-station --verify-context-menu target/context-menu-station
```

兩個入口均只連本機、自製 `navigation.json` 的 `reading_middle` 畫面；後者開啟 1003／1006，使用 Shift＋右鍵。peer 只接受正常 Close，禁止額外 terminal bytes。這兩個入口適合測選單、選字和本機輸出，不能用其 Close 結果認定導航、外開、剪貼簿或 PNG 動作已被實際操作；站端導航沿用專用嚴格 peer 驗證。

macOS arm64 的實際生成 Slint UI／Winit＋Skia 軟體後端，以 toolkit 事件和顯式合成 foreground／button 狀態驗證：網址選單與選取保留、選取完整網址、Home／End／方向鍵／Enter／Escape／Tab、背景整頁選字、外點關閉且零站端 bytes、frame 失效、失焦、縮窗撤銷及小視窗邊界；另驗證 1003 下撤銷確認後選單不因晚到政策失效、停用外開項目跳選。來源、截圖及成功／失敗輸出在 `.local/verification/slint-context-menu/`。原生 OS 可信滑鼠／鍵盤、IME 往返、AX、真實剪貼簿與 picker／外開及其他三架構仍為 `not_ready`，不能由合成事件或 screenshot 宣稱通過。

完整 App 的兩個 toolkit 驗證均通過，含待繪後呈現、Shift 放開後右鍵仍屬本機、停用項目點擊不關閉／不執行、鍵盤選取、保留選取、外點、失焦、縮放後重繪、小視窗與 Tab 關閉。兩次嚴格 peer Close 均 acknowledged，證據 `full-app-final-local/`／`full-app-final-station/` 及各自 log。共用 lib 225、Slint bin 31、Dioxus bin 179／multi-instance 10、JavaScript 380、兩入口 all-targets Clippy、core 14 命令／2 replay、fmt 與 docs gate 通過；既有 ignored 個別保留，不算人工驗收。共用規則提交為 `e9dbc83`，核心撤銷確認與生命週期修正為 `4dd2efd`；Slint 累積移植仍為 worktree 未提交內容，尚未整合或推送。清理自己的 `target/debug`，Cargo 宣告 2.6 GiB、du 配置量與 filesystem free 前後分開保存在本節 `cleanup.json`；保留 active release cache、最新 Windows S1 包及不明 guest output，沒有移除 worktree／分支。


## S7 研究版資料與密碼庫邊界

在接入浮球偏好前核對到 `--connect` 的站點／設定載入仍呼叫正式 `Store::system()`，原生登入表單在非 probe 模式也使用正式 vault service。先前一般 loopback gate 沒有跑 `--connect` 或保存實際帳密，不能用它們證明研究入口隔離。這輪移除上述連結；不讀取、複製或遷移使用者的正式檔案／帳密，也不刪除任何原有 keychain 項目。

`profile::Profile` 用 `directories::ProjectDirs::from("", "", "PTTStellaSlintResearch")` 決定研究設定目錄，再以既有 `Store::at` 接 settings／sites 的同一格式、容量、OS 鎖、備份與欄位合併。研究檔案不存在時使用預設值，不回頭找正式設定。未加入任何啟動自動匯入或保存。可先用下列命令列出實際路徑；它不開視窗、不連網、不讀 store，也不開 OS credential provider。

```sh
target/release/pttstella-slint --show-profile
```

Vault 的封閉 `Scope` 明確分開正式、既有 synthetic probe、Slint research 和 Slint synthetic probe 四種 service。保留原 `Vault::new(false/true)` 的兩個 legacy service 和二進位記錄格式，Dioxus 行為不變；Slint 登入只選其兩個 scope。研究 service 分別為 `io.github.yam276.PTTStella.slint-research.login.ws.ptt.cc.v1` 與 `io.github.yam276.PTTStella.slint-research.login.synthetic-probe.v1`。保存仍須明確勾選，普通單次密碼沿用不讀取 provider 的路徑。S9 若獲核可才切換成原產品身份及原資料／vault。

合成測試在 tempdir 建立「產品副本」與候選目錄，核對候選缺檔不讀副本、保存不改副本或其備份；明確複製 v1 設定後，載入保持原始 bytes，第一次候選保存備份逐 byte 相同，來源不變；候選毀損不回讀來源。另核對系統路徑身份相異及四個 vault service 互異、legacy identity 精確保留。這些案例不接使用者 store 或 OS vault；不等於已完成舊 Dioxus 執行檔往返、四架構 native vault 或完整 S7。研究入口來源、路徑輸出和 gate 證據保存在 `.local/verification/slint-profile-isolation/`。

載入入口補驗曾抓到資料讀取順帶呼叫一次性的全程序語系初始化，第二次讀取因此失敗。已把啟動責任放回 main：先讀研究設定、初始化語系，再載入同一 profile 的站點；資料讀取本身可重跑，站點錯誤也保留所選語系。失敗 log 保留，不用降低測試或重置全域狀態處理。

本輪共用 lib 226、Slint bin 34、Dioxus bin 179／multi-instance 10 通過，保留原本 ignored；讀取入口補強後另跑三個 profile 案例。兩入口 all-targets Clippy、core gate 14 命令／2 replay、fmt／diff-check／docs 通過。Vault scope 的獨立提交為 `ebb4e6f`，未推送。浮球入口及完整 S7／S8 仍未完成；這一輪是接入保存偏好前的必要隔離修正。


## S3 原生浮球與工具分類

浮球使用既有星座 icon 和金色圓環，與 BBS raster 分層。幾何及拖曳／慣性 reducer 位於共用 `floating_tools`，Slint `tools` adapter 擁有焦點、操作、唯一活動動畫 timer 與停止條件。模型不解析 bytes、不開連線或新增 BBS writer。五個第一層位置依既有半橢圓、邊界、停靠側和 `clamp(min(width/1280,height/800),1,1.75)` 的 logical 尺度排列；小視窗改成可捲動格狀配置，分類頁沿同側放置。

拖曳閾值為 6 logical px，放開端點納入最多 16 筆／120ms 的最近樣本，以同一回歸速度與衰減完成短慣性及停靠。慢拖保留最後實際方向的小幅滑行，減少動態時直接停靠。只有活動慣性使用 16ms single-shot timer；結束、失焦、取消、resize、DPI、縮放或關窗會停止。wall clock 和 monotonic clock 一起拒絕停滯／睡眠後的續跑。原生 OS 減少動態偏好尚未接上，研究版預設減少動態；明確工具探針才另外切成動畫路徑驗證，不把此預設當作效能改善。

目前「導覽」接共用 planner 的返回／前後頁，以及既有 Vim、站端滑鼠、縮放、拖曳捲動與全螢幕 callback；「擷取」接同一選取／文字／ANSI／PNG owner；「連線」接既有重連、斷線、登入／取消與保活 owner。動作執行前重驗可用性，pointer 按下保存 action、category、stamp 與 viewport，來源、幾何或分類撤銷後放開不能執行。滑出按鈕、工具頁捲動與 DPI／縮放會取消持有的操作。「收藏」及「設定」保留五個位置中的停用項目，尚未註冊對應原生對話框；完整移植依 S4／S7 繼續。研究工具列目前仍保留，尚未以本輪宣稱工具入口移植全部完成。

以 typed `AppSettings` 傳入已保存的浮球 opacity／visibility／右鍵工具偏好。只有浮球可變不透明度，hover、焦點、按下與工具展開時恢復完全不透明。普通背景右鍵依偏好暫時開啟工具，關閉後復原原停靠點；隱藏浮球時仍暫時顯示中央返回／收合按鈕。完整 URL、已選文字、已辨識編輯器及 Shift＋右鍵保留內容選單；站端普通右鍵仍由原 station owner 優先處理。推文／ID、圖片和標題收藏分流仍待 S4／S5 owner。

工具開啟或拖曳期間封鎖 terminal raw 輸入及站端滑鼠並暫停保活。Up／Down 跳過停用項目，Home／End 選首尾，Enter／Space 僅非 repeat 執行，Left 或中央返回恢復原分類選取，Escape／Tab 和外點關閉且消耗該次手勢，再恢復終端焦點。右鍵內容選單與浮球共用鍵盤順序 helper；完整 App 探針共用一個 verification task owner 和輸入／截圖 helper。

```sh
cargo run --release --locked --features slint-research --bin pttstella-slint -- --probe-tools
cargo run --release --locked --features slint-research --bin pttstella-slint -- --probe-tools-station
cargo run --release --locked --features slint-research --bin pttstella-slint -- --probe-tools --verify-tools target/slint-tools-local
cargo run --release --locked --features slint-research --bin pttstella-slint -- --probe-tools-station --verify-tools target/slint-tools-station
```

輸出目錄須尚不存在。兩個入口使用自製 `reading_middle` 畫面和只接受 Close 的本機 peer，opacity fixture 為 20%；station 入口另啟用 1003／1006。完整 App 的 toolkit 案例涵蓋正文連結仍可操作、分類／鍵盤、整頁選取、pointer 動作、未呈現來源及幾何撤銷、外點零漏送、慣性結束、背景右鍵、隱藏後暫顯、暫停靠復原、關閉右鍵工具後回內容選單、小視窗格狀與失焦關閉。檢查打開工具時 terminal image 保持相等，peer 獨立拒絕多餘 bytes 並確認 Close。沒有操作真實剪貼簿、檔案 picker、瀏覽器或 vault。

回歸抓到收合浮球的 FocusScope 原先仍覆蓋整窗，會擋住正文右鍵；已把收合命中範圍限於浮球，只有展開時覆蓋工具層。修正後兩個 context-menu 與兩個 tools 完整 App 探針通過，失敗 trace 保存在 `.local/verification/slint-tools/context-audit.log`。macOS arm64 Winit／Skia software 的 toolkit 與擷圖證據在該目錄；可信 OS capture／按鍵、IME 往返、跨螢幕 DPI、OS 動態偏好、AX、Windows／macOS x64／Linux 仍為 `not_ready`。收藏／設定、動態提示、完整文章／媒體／編輯器、資料回退與同負載效能仍按完整 S0–S8 計畫推進。

本輪共用 lib 231、Slint bin 35、Dioxus bin 179／multi-instance 10，另有無 UI feature 的五個 reducer 案例通過。兩入口 all-targets Clippy、core 14 命令／2 replay、fmt／diff-check／docs 通過，既有 ignored 維持原狀。共用 reducer 的提交為 `2e1db51`；Slint adapter 與累積候選內容仍留於 worktree，沒有推送、切入口或發佈。`.local/verification/slint-tools/source.patch` 以初始 `5ea9ad6` 為基準，涵蓋已提交共用修正及目前候選的 tracked／untracked 來源，另以隔離 Git index 驗證能套用；fixture／來源／binary／證據雜湊及未完成範圍記於 `verification.json`。

驗收輸出目錄現在於任何 connection／UI task 建立前預先建立。重跑碰到既有目錄時，程式提早拒絕覆寫，保留檔案逐筆 SHA-256 不變，也不再等待尚未啟動的 probe peer。該錯誤路徑與四個最後的完整 App 案例另記於 `existing-output-check.json` 和 `current-tools-*`／`current-context-*`。清理已被取代的 PNG、臨時 Git index 和本輪 `target/debug`，保留 active release cache、最新 Windows S1 包、不明 guest output 與獨特失敗紀錄；exact paths、Cargo 宣告量、du 配置量及 filesystem free 前後見 `cleanup.json`。


## S4：共用文章快取與有界可視列投影

`ArticleCache`、`Reading` 與 `presentation::Paint` 已從 Dioxus 私有 UI 模組移至共用 Rust library；Dioxus adapter 直接 re-export 同一實作，原有取頁、折行重疊、取消／恢復、期限、4,096 列／32 MiB、分段接續與原始列封存邏輯保留。草稿作用域判斷亦移至既有 `drafts` 模組，沒有第二份前端近似判斷。這些模組隨 `desktop-services` 提供，Slint 不需要引入 Dioxus 或 WebView。

新增 `ArticleCache::window`，每個投影 tile 最多 256 個正文列，加一列來源頁尾。呼叫者提供可視列與緩衝範圍，回傳實際正文範圍及總高度；與全文投影共用 `render_rows`，只複製指定列的 glyph、雙色樣式、遮罩及推文。短文章的空白格、唯一 live footer、頁尾 OSC 8 完整連結與 24 列原始 PNG 來源均保留。tile 的列號是本機座標，`rows.start` 用於還原文章位置；全文尋找及連結索引仍需綁定整段已確認投影，不能從單一 tile 推測全文。舊 generation／名單版本、投影後封包、空範圍及超限請求均拒絕。

共用 library 中保留原有 30 項文章快取測試與 1 項投影新鮮度測試，新增可視列／短文章案例，並在原始列封存測試核對 tile 不洩漏遮罩原文。獨立 expected 核對歷史第 251–277 列、舊雙半顏色、尾段裁切、唯一頁尾與連結位置。這是 Slint 原生長文 viewport 的資料接口；Slint 捲動 UI、取頁 permission／owner 接線、搜尋、推文與收藏尚未由本節驗收，完整 S4 維持進行中。


## S4：Slint 原生長文 viewport 首輪

`--probe-article` 現在可開啟原生長文視窗，`--verify-article NEW_DIR` 自動驗證同一個 App 元件。這個明確的離線入口把 Dioxus 原有 `long_load` 的 38 頁自製 fixture 經同一 `Screen`／`ArticleCache` 接成 837 列，沒有站端取頁、HTTP、帳密、剪貼簿或使用者資料操作。fixture 函式移至共用 `replay`，兩個入口沿用同一份 bytes；拒絕超出既定頁數的 fixture 請求。`--connect` 的文章取頁及新 viewport 接線仍待完成，這個案例不能當作 Slint `long_load` peer 驗收。

viewport 以 Rust 計算正文列與列內偏移，固定一列原始頁尾，橫向另保留欄位錨點。只保存可視正文與上下兩列的單列影像；相同尺寸的留存列直接重用，只繪製新進入的列。缺少連續列時才透過 `ArticleCache::window` 批次建立有界投影，再拆成單列，沒有全文大小的畫布。每張影像及合計可視影像維持既有 8,192 邊長／16M pixels 預算。縮放／DPI 改變時更新影像尺寸及幾何；文章行號、列內偏移與橫向欄位位置保留，末端依實際正文高度收斂。Skia renderer 的獨立投影共用 reference-counted 字型資產，不能改掉 terminal 的來源 Frame。頁尾仍呈現站端來源，依本機閱讀位置更新其百分比／頁數的投影尚待接上。

原生元件支援滾輪、拖曳捲軸、PageUp／PageDown 一行銜接、Home／End、j／k 與上下鍵。捲軸限左鍵，抓住 thumb 時保留原抓取偏移；縮放或幾何變更撤銷拖曳。組字／modal gate 暫停本機閱讀控制。唯一零延遲 single-shot timer 合併繪製，沒有常駐 polling；關閉停止 timer 並清空影像 model／列快取。

實際截圖抓到正文容器未明確指定 x／y，被 Slint 置中而遮住最後一列。保留修正前元件與 `native-negative` 的可視像素負向驗證（5,038 個不符像素）；固定正文與捲軸位置後，以同一末列來源 raster 對照真正 window snapshot。驗證還核對原歷史第 661 列的黃色、縮放／resize 錨點、每行捲動只繪製新列、小數捲動不重建、原始 terminal 圖與投影分離。原始資料與目前結果在 `.local/verification/slint-article-viewport/`。

本節僅屬 macOS arm64 Winit／Skia software 的 toolkit／像素證據。可信 OS 鍵鼠、IME lifecycle／焦點、AX、實體跨 DPI、Windows／macOS x64／Linux，以及目前文章的取頁 permission、取消／恢復、手動輸入隔離、選字／複製／PNG／連結與媒體、搜尋／推文／收藏仍待各自接線及驗收。完整 S0–S8 goal 保持進行，沒有切換正式入口或更新 Windows S1 包。


本輪最後 `native-current` 以實際元件派送 wheel、方向鍵／翻頁／首尾鍵、左右鍵與捲軸拖曳，核對一次逐行移動只新增一張單列影像、小數移動零重繪、列內／橫向錨點、組字 gate 及關閉前尚有排程時的 timer／model／列快取清理。末列的 512×26 個可視像素與獨立生成的第 837 列來源 raster 比對，容許單 channel 最多 1 的轉換差，零不符像素。最終小視窗保留 18 張列影像、486,720 個正文 pixels；這是 raster 配置量，並非程序或 process-family 記憶體量測。`--capture` 的文章入口亦實際驗證，舊輸出目錄與錯誤 probe 模式均在 UI 啟動前拒絕，原檔逐筆 SHA-256 不變。

共用 lib 264／Slint bin 39、Dioxus bin 148／multi-instance 10、JavaScript 380 通過，原本 ignored 個別保留。兩入口 all-targets Clippy、core 14 命令／2 replay、fmt／diff 與 docs gate 通過。新的隱藏文章元件未攔截其他入口：浮球／站端浮球、右鍵／站端右鍵四個完整 App 案例均重跑，嚴格 peer Close acknowledged。共用 fixture 提交為 `b35ff9f`，native viewport 與累積 Slint 移植仍保存在 worktree 和本節 `source.patch`，未整合或推送。已被取代的本輪截圖及本輪 `target/debug` 按 owned-output 規則清理；保留最新驗證、負向像素證據、active release cache、最新 Windows S1 包及不明 guest target，exact paths、Cargo 宣告量、du 與 filesystem free 前後取樣見本節 `cleanup.json`。

補驗只增加 4px 寬、字格與正文高度均不變的 resize：按住舊捲軸 thumb 後移動不得改變錨點。元件直接在 width／height 變更撤銷抓取，`native-current` 通過；最新四個其他入口回歸與文章 `--capture` 也使用同一最終 binary。


## S4：共用投影 owner 與原生取頁循環

`SurfaceState` 與推文更新 reducer 移至共用 Rust library，Dioxus 保留原 protocol adapter 和既有整合測試；`CopyTarget`、本機閱讀 intent 及可撤銷擷取 lease 一併共用。原生列影像由同一個完整的 in-flight／已確認投影裁出，保留連推合併、遮罩與同 ID 樣式，不能從較早的 cache row 旁路讀出原文。新回歸以獨立的 ALICE 合併 expected 核對圖塊，並拒絕舊名單政策與舊世代。這個單位提交為 `4cc7b62`；exact staged Rust／manifest／locale tree `d88146c88ef44ad8eddb95e5bbc54d62372678b4` 已在獨立暫存來源上編譯 desktop lib／bin，字型及資產只以本產品原目錄的 symlink 共用，沒有複製資產或依賴其他專案。

新增 `--probe-full-load`、`--probe-long-load`、`--probe-terminal-pages` 與 `--probe-stalled-article`，搭配 `--verify-article-session NEW_DIR` 重跑真正的 Slint App、同一 `Connection` 和嚴格本機 peer。一般全文必須恰 19 筆 PageDown／441 列，長文恰 37 筆／837 列；每頁故意等候並切成兩段回覆，既有 Rust cache 核對重疊與頁尾。載入時最多每 250 ms 建立完整投影，取頁在收到確認的 View 後前進，不依賴每頁 UI ACK；初始與換 scope 仍需由已繪投影授權。保留 generation／文章 ID／政策版本與實際 input stamp 隔離，未知或部分頁不當成新的完整內容。

一個 native loop 同時擁有 watch、UI 通知、單一在途 Reply future 與最早期限，沒有逐命令 sender task。取消先撤銷 writer 的 atomic ticket，再釋放對應等待；已送頁稍後到達仍可接入，取消後不自動繼續。UI 失焦或操作中不取下一頁，但仍處理回覆以清掉已完成的網路等待。明確繼續才恢復取消的工作；回到可操作狀態則可恢復原本僅暫停的載入。圖塊保持先前的列／列內／欄位錨點，新增內容與來源頁尾更新不重畫已保留的正文列。原生 window observer 沿用既有焦點／遮蔽／指標政策，沒有掛載 terminal 文字欄時不准它送 raw keys。`--connect` 尚未啟用這個文章 owner，等待輸入／選取與輸出接線完成。

停止時不把 Slint 的 `abort()` 當成 join：owner 撤銷命令、喚醒 loop，退出時釋放 Reply future 和 watch，再等待明確的 loop-done 狀態。正常關窗先完成這段清理才離開 event loop；驗證案例同樣等待 owner 結束後才關閉 transport。停滯 peer 只接受一筆 PageDown 後不回覆，六秒期限後必須顯示停止、零重試，再正常 Close。這些仍屬 macOS arm64 Winit／Skia software 的本機協定與合成 toolkit 證據，可信 OS 輸入／IME／原生焦點尚待驗收。

終端機案例先停留不動、零預讀，再只透過既有 grid wheel handler 手動送一筆 PageDown，收到第二頁並確認繪製後 Close。全文案例另核對取消後的晚到頁、模擬背景期間的接頁／零續送、回前景不自動恢復已取消工作，以及明確繼續；長文保留第 661 列黃色與兩端文字，回讀快取不得追加站端 bytes。完整 S4 仍需實際使用者輸入、快速取消／繼續的交錯、未知邊緣、跨段／切篇、選取／連結／PNG、local footer 進度、搜尋／推文／收藏，接著繼續 S5–S8。


停滯案例已通過：維持 23 列、只送一次 PageDown，至少五秒後才觀察到六秒 deadline 的停止結果，沒有重試，peer Close acknowledged。每個成功案例在離開 UI loop 前會等待取頁 loop 的 normal completion，並確認 Reply 已釋放、watch 已退出、loop-done 與 handle 清空。共用 owner 原有的 38 項狀態測試保留在 Dioxus adapter，新增最終投影的原生圖塊隱私回歸；推文更新的 3 項 reducer 測試移至共用 lib，沒有宣稱 native 推文功能已完成。來源與最後證據在 `.local/verification/slint-article-session/`。

最後四個 `*-accepted` 案例使用同一最終 binary，全部確認取頁 loop normal completion 與 peer Close acknowledged。停滯案例另在沒有新網路畫面的情況下切換名單政策，刻意留下舊 in-flight 投影，確認它被撤銷、新政策可重新呈現且不多送 PageDown。離線 viewport 的末列獨立像素檢查及浮球／右鍵回歸保留於本節證據目錄。共用 lib 267／Slint bin 39、Dioxus bin 146／multi-instance 10、JavaScript 380、兩入口 all-targets Clippy、core 14 命令／2 replay、fmt／diff／docs 通過；ignored 維持原狀。舊輸出目錄重跑與錯誤模式均提早拒絕，原輸出逐筆 SHA-256 不變。

來源快照、fixture／binary／證據雜湊、必要待驗項目記於本節 `verification.json` 與 `source.patch`，補丁從初始 `5ea9ad6` 可在隔離 Git index 重現。清理已被取代的本輪擷圖、exact-staged 暫存來源與本輪 `target/debug`；原來源與資產 symlink 的目標保留。active release cache、最新 Windows S1 包、不明 guest output 及尚未整合的工作樹保留，exact removed paths、du 配置量、Cargo 宣告量與 filesystem free 前後分別記於 `cleanup.json`。完整 S0–S8 goal 持續進行，下個入口為現行文章的 typed 輸入路由、選取／輸出與 raw fallback，再接搜尋及其餘產品功能；沒有啟用正式入口、整合、推送或發佈。


## S4：文章鍵盤、原生組字與站端回退

`--probe-article-input --verify-article-session NEW_DIR` 使用兩頁、45 列的共用自製 fixture，peer 依序只接受一筆 PageDown、一筆 `q`、一次「中」的 UAO bytes 與 Close。延遲回覆期間立即取消再繼續，必須仍只有一筆 PageDown；快取中的 j、Home／End、底端空白及取消組字均不得多送站端命令。舊 generation 的 raw input 直接拒絕。這個 peer 不連公開站或媒體。

文章與終端共用同一 native input observer、Rust `Keyboard` 和 Connection writer。`ArticleView::navigate` 接收 typed `Intent`，toolkit 與 Winit 路由消費同一列幾何及一行銜接；Vim 計數與 Ctrl+B／Ctrl+F 的既有結果也由這個入口轉為本機捲動。文章不重畫隱藏終端，只用已確認文章投影授權閱讀鍵，不能要求一個刻意省略的終端 raster ACK。要送出的 raw bytes 先取消取頁 ticket、撤銷文章投影，再由原 writer 送一次，保留「返回網頁閱讀」入口；晚到舊頁不得使 raw 操作期間的取頁自動復活。Vim 原生模式／一次原生亦透過同一交接狀態，完整裝置交錯仍待驗。

文章 FocusScope 現在持有可及性標記的原生 TextInput，從第一個 preedit 起就由 Slint 與 OS IME 擁有。組字文字在可視文章首列的獨立層顯示，原文不被改寫；組字 gate 暫停取頁與閱讀鍵。未提交及取消為零 bytes，提交才沿站點編碼並切回終端。代次改變清掉舊組字交接。傳輸更新仍被接收，待組字結束才重建需要更新的投影。這個記事不代表候選彈窗、跨螢幕 DPI 或所有輸入法已通過。

共用快取原先取消時清空 outstanding response，立即繼續會允許重送 PageDown。`3468cd2` 保留已請求頁的回覆／期限 fence；回覆到達才解除，取消中的 deadline 到期只清理一次且維持 Cancelled。新增核心獨立 expected，並擴充 desktop reducer 案例驗證立即繼續零第二筆、切回 terminal 清理期限。這是兩前端共用修正，不是 Slint 另建取頁 reducer。

macOS arm64 的 CUA 原生按鍵與 AX／擷圖已重現並修正兩個接線缺口：隱藏終端 ACK 使文章按鍵被擋，以及沒有原生 TextInput 時中文來源直接產生注音字元。加入 TextInput 後，實際 j 在注音來源顯示 preedit「ㄨ」，Esc 取消仍留在文章；切英數後 j 本機移一列，Home／End 可達首尾。q 回到合成主選單，再用注音 5／j／slash、Space、Return 得到「中」並提交一次，peer 精確核對、正常 Close，程序 exit 0。原生 q 亦抓到隱藏文章 resize 仍排程 raster 的錯誤；hidden view 現在停止繪製，最後畫面與合成案例都核對沒有 stale raster notice。一般 AX 已能識別兩個輸入欄及按鈕；完整 screen-reader／Tab／候選彈窗驗收未完成。

曾試額外匯入 Slint 字型，完整 16,435,884-byte 內附 CJK 字型在 binary 出現兩份，已移除該實驗。最終維持既有 native 字型 fallback，binary 中同一整份字型僅出現一次，檔案由 71,657,648 bytes 回到 55,096,080 bytes；這是靜態產物檢查，不能換算為 runtime 或 process-family 記憶體收益。trace 僅在明確環境變數 `PTTSTELLA_SLINT_INPUT_TRACE` 啟用，最多 128 筆長度／已辨識 intent／gate boolean，不記輸入文字。

本節證據位於 `.local/verification/slint-article-input/`，含舊失敗 log、最後 native peer／toolkit 結果、來源補丁、雜湊與 owned cleanup 紀錄。`--connect` 的文章仍待選取／複製／PNG／連結等投影輸出接線後啟用；未知邊緣取頁、跨段／相鄰文章、搜尋、推文／收藏及 S5–S8 完整目標繼續推進。Windows 包仍是原先 S1，不以這輪 macOS 結果宣稱其他平台完成。

本輪最終同一 binary 的一般全文、兩頁 raw／快速取消繼續、837 列 viewport，以及浮球／右鍵回歸通過。原生實際輸入與合成 toolkit 的結果分開保存。共用 lib 268、Slint bin 39、Dioxus bin 146／multi-instance 10、JavaScript 380、兩入口 Clippy、core 14 命令／2 replay、fmt／diff／docs 通過，原有 ignored 保留。單一字型的最終 binary 為 55,096,080 bytes；來源與 binary SHA-256 見本節 `verification.json`。清理本輪被取代的 toolkit 擷圖、暫存 index、初期重複 build log 和已無使用者的 dev profile；保留最新 native App、active release、各成功結果、獨特失敗紀錄、Windows S1 包、不明 guest target 與未整合工作樹，exact paths／du／Cargo 宣告量／filesystem free 分別記於 `cleanup.json`。


## S4 文章輸出與終端投影接線（2026-11-07）

文章的滑鼠選取、純文字／ANSI 複製、整頁與最多 24 個選取行的 PNG、圖片剪貼簿，以及一般／OSC 8 網址操作，現在共用既有 `SurfaceState` 的已確認投影、選取 request 與 output lease。歷史行按需裁出最多 24 列交給同一 Rust renderer；不為匯出複製整份長文或配置整篇 bitmap。整頁仍輸出站端目前 80×24 source page，選取歷史則消費完整投影。清除選取只撤銷選取工作，整頁工作保留；換來源、政策、幾何或 raw 交接會撤銷對應工作。工具列明示 24 行上限，超長選取仍可複製文字。

文章 pointer 與 pinned footer 使用同一實體像素幾何。Footer 命中其完整最後一列，沒有穿透到下方歷史。右鍵選單記錄政策版本，不能在相同 source revision 的政策變更後執行舊網址。實際生成元件的 Shift 反向拖曳曾被前景全窗 FocusScope 攔截，已將鍵盤 scope 移到正文後方，讓原生 TextInput 保有焦點且正常寬度僅 1px；組字時才顯示輸入寬度。負向元件與失敗擷圖保留，直接呼叫 handler 的診斷不能把失敗案例變成通過。

同一 owner 也開始負責終端模式與 raw fallback 的過濾投影。Connection watch 只更新來源版本及狀態，不再用原始 Frame 覆蓋已過濾的 renderer。終端 packet 等幾何與 raster 安裝、來源 generation／revision 相符後才 ACK；選取、複製、網址及 PNG 都消費同一 confirmed Paint。離開完整的閱讀畫面或換 generation 時釋放 raw hold，Vim 原生／一次原生與組字交接另保留其 ownership。一般 `--connect` 入口已接上共用文章 owner、保存的閱讀模式及研究 profile 的 filters.json；無法讀取規則時在連線前回報錯誤。沒有讀取或改寫正式 PTTStella 的 filters。

macOS arm64 的 toolkit 案例以兩頁 45 列嚴格 loopback peer 核對「中」的兩半樣式 1／4／2／5、歷史行 PNG 960×24、遮蔽列全黑、整頁首行 23、footer 的完整 OSC 8 URL、選取／政策／縮放撤銷，以及 raw terminal 的複製／PNG／連結遮蔽。探針在政策重建後刻意保持背景，避免輸出案例的 peer 收到另一次全文定位；前景載入由獨立全文案例驗證。第一個 native layout 先以 snapshot 解算，再計算 toolkit pointer 座標，拒絕把尚在重排的幾何當成可操作畫面。一次掛起的 stack sample 找到文章 rendered 回呼持有 watch read guard，再回呼 Output 取得讀鎖，而 writer 正等待；已改為先取 generation 並釋放 guard，右鍵來源亦先取 policy 再借用 watch。失敗證據及修正後的 peer Close 結果各自保留。

macOS 原生 CUA 另外驗證了實際拖曳「中」、Cmd+C 純文字、Cmd+Shift+C 帶 SGR 66 的 ANSI、原生 Save sheet 存出 960×24 PNG，以及圖片剪貼簿。PNG 與獨立像素核對過的合成 PNG 完全相同；圖片剪貼簿在明確 UI 複製後由 ignored 原生讀回測試確認兩半藍／紫背景。這份 OS 證據的 binary 身分與最新 toolkit binary 分開記錄，不能當成 Windows、候選彈窗或完整輔助使用驗收。

共用 fixture 提交 `033141f`／`8368e2b` 讓兩頁都含嚴格格式的可遮蔽推文；兩頁重疊、閱讀命令與雙色 glyph 另有獨立 expected。共用選取的 `f112afd` 修正每個選取行重掃全文 glyph 的成本，排序過的生產 Frame 使用 row slice，未排序的公開 Frame 保留原遍歷順序。相同 837×80 工作量、release、3 次暖機與 7 批各 15 次的 warm wall 中位數由 27.999341ms 降到 0.222186ms。這同時改善 Dioxus 與 Slint，沒有量 CPU、冷啟動或 process-family memory，不能據此宣稱 S8 前端效能比較通過。

證據、來源補丁與各 binary 雜湊保存在 `.local/verification/slint-article-output/`。本節完成目前文章輸出及過濾終端橋接；一般連線的實站登入、文章保活／貼上整合、未知邊緣取頁、跨段／相鄰文章、搜尋、推文／收藏，以及 S5–S8 媒體、編輯器、設定／回退、四架構與同負載效能仍持續實作。Windows 仍是原先 S1 包，尚未更新；沒有切換正式入口、整合、推送或發佈。


本輪回歸另發現合成 foreground 與 Slint 視窗 activation 不一致時，正文 TextInput 的 has-focus 可為 false；不能把這種失焦狀態當成載入器漏送。文章探針現在明確派送 `WindowActiveChanged`，typed 路由與捲動之後先完成 native layout，再明確聚焦測試輸入欄；恢復載入案例亦明示合成的焦點事件。正式程式的前景／焦點准入保持原規則，不把 toolkit 結果當成可信 OS focus 證據。過期 source packet 的 timer 現在單獨 retire，保留仍有效的文章元件；載入按鈕的焦點恢復延至新投影安裝後，仍要求前景、可操作與目前 scope。


最後文章輸出、typed 輸入、terminal 手動翻頁、441 列全文、837 列長文及 stalled deadline 六案通過；一般／站端右鍵與浮球四案亦通過，各自嚴格 peer Close acknowledged。文章案與 UI 案的二進位身分分別保存在 `final/regressions.json`、`ui-settled/regressions.json`；最後差異只在右鍵／浮球探針先 snapshot，等待 300ms 原生啟動焦點／resize 事件，再施加明確的合成 activation，不屬於產品執行路徑或效能樣本。共用 lib 273、Slint bin 41、Dioxus bin 146／multi-instance 10、JavaScript 380、兩入口 all-targets Clippy、core 14 命令／2 replay、fmt／diff／docs 通過；既有 ignored 個別保留，圖片剪貼簿 ignored 案另有原生執行成功 log。

最新原生候選 App 已更新並通過 strict ad-hoc 簽章檢查；舊 OS 動作證據的 binary 身分不改寫成最新版。來源 patch 從初始 `5ea9ad6` 在隔離 Git index 重播得到相同 tree。清理舊 exact-staged source、與最後成功 PNG 位元相同的中間成功證據，以及已無 open user 的本輪 dev profile；不清理失敗 trace／獨特負向圖、OS 輸出、原始資產、active release cache、Windows S1 包、不明 guest target 或尚未整合的 worktree。exact paths、symlink 目標、Cargo 宣告清除量、du 與 filesystem free 前後分別記於 `cleanup.json`。


## S4 文章保活與原生貼上（2026-11-07）

保活現在接受目前文章的已確認 Rust 投影，不再要求被隱藏終端的 raster ACK。仍核對 generation／revision／input epoch、目前政策、原生幾何及 Connection 的狀態；前景與目前文章 TextInput 的焦點缺一不可。文章和終端共用原來的 UI 暫停條件及 Connection 唯一 timer，沒有新增 UI polling timer。切換保活及關閉浮球／貼上對話框會回到目前可見的文章或終端輸入欄。

文章及終端投影多了一個狀態封包重用路徑：同一 source frame、政策、字型設定、文章 scope、列數及已安裝幾何皆相同，才重用既有 pixels，並在釋放 view slot 後 ACK。keep-alive-ready 這類 metadata 不再清掉 raster 或引發「待畫 → 暫停 → 再允許」回授。改變 source、樣式、scope、設定或幾何仍走原先完整重繪。完整 23 列的保活探針在選取與背景各超過三秒、切換 IME／貼上／輸出／pointer／浮球 gate 後，raster count 仍為 1。這證明指定 workload 沒有因 permit metadata 重繪，不是 S8 的 CPU／memory 或前端對照結果。

貼上從目前 confirmed Paint 取得 source stamp 與政策 identity。讀取或預覽期間留在文章，取消、讀取失敗、無法編碼與政策失效都不送 bytes。只有內容通過同一 prepare_paste／prepare_editor_paste、必要預覽被明確確認且 source 再次相符時，才取消全文取頁、切回 raw 並交給原來的 request_paste single writer。貼上的 j 是 raw 文字，不會變成本機捲動；PTT 編輯器的 Ctrl+V 路由維持原判斷。政策改變即使沒有新 revision，也會撤銷讀取與預覽，晚到結果不覆蓋較新的通知。

每個視窗最多一個實際 clipboard read worker；讀取及送出等待有 owner，視窗結束會 abort UI completion 並 join 讀取 worker。取消讀取不假稱 OS provider 已停止：worker 結束前不接受第二筆讀取。合成驗收把唯一的資料來源換成自製文字／可釋放的有界 channel，仍使用相同 production worker、預覽／取消／確認 callback 與 writer；一般 gate 不讀或寫系統剪貼簿。

新共用 fixture／strict peer 提交 `8585c33` 提供完整 1..23／100% 閱讀頁、短期限的原 timer，以及精確的 j、A\r中、Close 檢查。macOS arm64 toolkit keepalive 核對選取／背景超過 idle 仍零刷新、操作 gate 暫停、恢復重新等待完整週期、只送一次 0c 0c，收到未知畫面後撤銷。paste 核對取消及 stale read／policy preview 零 bytes、IME 期間零 clipboard read、失敗留在文章、只有明確確認送出兩筆獨立 expected：106 與 65／13／164／164；pending worker 與 article actor 都正常結束。exact-staged common source 另編譯並通過三個獨立 fixture expected，不把 working tree 的額外 native API 當成已提交內容。

macOS 實際 CUA 操作同一 native App，透過原生 pasteboard 與 Cmd+V 貼上 j，畫面切成 raw 並顯示返回網頁閱讀。點返回後，貼上 A 與中兩行，實際 dialog 顯示「貼上 2 行文字」；取消後焦點回到文章，重貼再點送出才回到合成主選單。peer 精確檢查上述兩筆 bytes，Close acknowledged、process exit 0。AX／畫面位於 CUA 工具紀錄，受控 dialog PNG 另在本節目錄。AX 查詢曾在關閉後自動重開 offline App，該視窗亦已關閉並確認最終 inventory 無此 App；沒有留下額外 session。這不是 OS IME 組字／候選窗或其他平台的證據，stderr 的 IMK／CapsLock 系統訊息亦保留，未當成產品錯誤或忽略實際失敗。

同一 binary 的既有十個文章輸出／輸入／terminal_pages／441 與 837 列長文／停滯／一般與站端右鍵及浮球回歸通過。共用 lib 274、Slint bin 41、Dioxus bin 146／multi-instance 10、JavaScript 380、兩入口 release all-targets Clippy、core 14 命令／2 replay 通過，原有 ignored 保留。證據、各 binary／App 身分、source patch 與清理清單在 `.local/verification/slint-article-actions/`。

本輪沒有啟動正式 PTT、接觸正式 profile／vault、更新 Windows 包或改變正式入口。S4 的搜尋、邊緣／跨段／相鄰文章、推文與收藏／快速導覽／更新仍待接線；S5–S8 的媒體與播放器、編輯器、完整設定及回退、四架構與同負載效能繼續沿完整計畫推進。


本節清理已驗收的 exact-staged 暫存來源及本輪 dev profile，保留原 symlink 的 source／assets 目標。舊 `.local/verification/slint-article-output/Native-Article-Output.app` 已由本節的 `Native-Article-Actions.app` 取代並移除；舊輸出／OS 證據和原 binary hash 保留，receipt 標示 superseded。active release cache、distinct IME 驗收 App、Windows S1 包、不明 guest target 與未整合 worktree 保留。exact removed paths、open-file 盤點、du、Cargo 宣告及 filesystem free 前後見本節 `cleanup.json`。


## S4 文章尋找：共用 Rust 索引與原生搜尋列

搜尋的字面比對、Unicode 小寫映射、整字格範圍、隱藏色／過濾排除、頁尾排除、命中上限和目前筆數，現在由 `article_find::Find` 統一管理。`SurfaceState` 只允許已 ACK、當前 generation／revision／input epoch／過濾規則的投影建立索引；來源 pending 立即撤銷高亮，換站、斷線或離開閱讀範圍清除查詢與索引。同篇重新投影保留查詢及命中錨點，純重新投影不跳頁。未搜尋及空查詢不建立索引，清空或關閉釋放索引；查詢上限 256 UTF-16 units，最多 4096 筆結果，沿用全文 4096 行／32 MiB 邊界。

Dioxus 的 `article-find.js` 僅負責可信事件、120 ms debounce、焦點與 Rust 回覆的顯示，不再掃描 glyph 文字。typed `article_find` 封包帶完整來源與遞增 request，Rust 只在結果改變時傳 hits，換筆只傳 active delta；adapter 拒收晚到來源／request／version。搜尋高亮只供顯示，複製、連結與 PNG 沿用原始已過濾畫面。

Slint 以原生 `LineEdit` 提供可及性標籤、上一筆／下一筆、Enter／Shift+Enter、Esc 與 Ctrl／Cmd+F；保留 Ctrl+Cmd+F。Winit 在 BBS 鍵盤 gate 之前路由搜尋焦點及組字，Vim NORMAL 不會停用搜尋欄的 IME。中文輸入來源的編輯快捷鍵以 physical key 交給同一 Slint 文字元件的 select/copy/cut/paste/undo/redo；首次 OS 測試發現 Cmd+V 變成注音文字，已修正並重測。開啟搜尋暫停背景取頁、保活與站端滑鼠，關閉後復原文章或終端焦點。文章只重畫可視列，終端採 paint-only invalidation，不把搜尋當幾何變更。

```sh
SLINT_BACKEND=winit-skia-software target/release/pttstella-slint --probe-article-find --verify-article-session NEW_DIR
SLINT_BACKEND=winit-skia-software target/release/pttstella-slint --probe-terminal-find --verify-article-session NEW_DIR
```

兩個完整 App 案例用自製合成完整頁面，在 cached article 與 terminal 模式各獨立核對三筆「中」的位置 `(5,6..8)`、`(6,0..2)`、`(6,3..5)`，被遮推文與隱藏字不命中。toolkit 文字輸入、換筆／wrap、組字鍵優先、取消 debounce、釋放索引、PNG bytes 不變與焦點復原均通過。嚴格 loopback peer 拒收搜尋文字，只接受明確 typed 站端 `/`，然後正常 Close；沒有預載要求。這些合成事件不當作 OS 輸入法證據。

macOS arm64 的實際 OS 操作另以獨立簽名 App 驗證 Cmd+F、原生剪貼簿貼「中」、Enter／Shift+Enter、注音 `5 j slash space` 組字、Esc 取消後搜尋仍開啟、重新組字後 Enter 僅提交一次，以及 Vim NORMAL 中的同一路徑。關閉搜尋、復原 PTT INPUT 後明確貼 `/`，測試站轉到 MainMenu，關窗 Close acknowledged。Dioxus 真正 WKWebView host 也驗證 `ROW` 的 46 筆命中、換筆、字面 `.*` 零命中與重新開啟清空；其 `reading_rows` 全案例要求的匯出操作沒有執行，該原始 report 保留 failed，僅採用本節實際觀察到的搜尋與 acknowledged Close 證據。

本節證據在 `.local/verification/slint-article-find/`：Rust／JS gates、兩模式完整 App、OS 正負向紀錄、binary hashes、來源 patch 和清理紀錄分開保存。Windows 包仍是原先 S1，這輪沒有跨平台原生驗收或同負載效能結論。未知邊緣取頁、跨段與相鄰文章、推文／名單管理、收藏／最近看板及 S5–S8 媒體、編輯器、資料回退、四架構與效能仍待完整實作驗收。沒有切換正式入口、整合、推送或發佈。


本節回歸另重現文章 actor 的來源交接競態：舊快照 reducer 與較新快照 queue 混用時，舊文章可能綁上新 revision，留下無 native ACK 的 in-flight packet。保活首次報「unknown refresh reply was not painted」，identity-only 診斷確認 terminal pending／shown 為空、core 卻仍在途；原始失敗與曾誤用舊 binary 的 runner 都保留並清楚標示。修正為同一不可變 observed View 完成一次 actor step，不在 step 中另讀新 watch。來源暫時不可用時的搜尋 draft 也會等 ACK 再嘗試，避免遺失已提交查詢。共用 Rust 搜尋與 Dioxus adapter 提交為 `a338bf1`；Slint adapter 與累積移植仍保存在獨立 worktree 及可重播 source patch，未整合或推送。


終端搜尋的重跑又確認 native 失焦會在 debounce 前正確攔下查詢。adapter 現在明確停止背景計時器、保留 bounded draft，解除阻擋後重新 debounce；probe 維持其明示的 synthetic activation，並獨立驗證背景超過 120 ms 時仍零結果、恢復後同一查詢出現三筆命中。沒有放寬失焦 gate。最終重跑使用 `accepted/results.json` 的同一 binary；先前 `regression`、`diagnostic-runs`、`snapshot-runs`、`snapshot-accepted-runs` 與 `query-diagnostics` 的失敗不能冒充本次通過，原始證據保留。


最終來源的共用 lib 283、Slint bin 42、Dioxus bin 148、multi-instance 10 與 JavaScript 382 項通過，原本 ignored 分別保留；兩入口 Clippy、core 14 個命令／2 replay、fmt／diff／docs gate 通過。`a338bf1` 的 exact staged tree `27156e8d260a7977692448d57dd3522f8b39f182` 在獨立來源目錄另跑 lib 281、Dioxus 148、multi-instance 10、JavaScript 382 與 Clippy 通過；兩種來源樹的測試數量分開列出，不以 worktree 額外內容代替提交的可建置性。最終全 App 回歸另見本節 `final/results.json`，含同一 binary 下的 441／837 行、raw input、文章／終端輸出、停止取頁、兩種右鍵／浮球、貼上、三次終端搜尋、cached 搜尋與三次保活。


2026-11-08 最終 native App 已更新並驗 strict ad-hoc 簽章。這個最新版的 terminal 模式另經 OS 實際 Cmd+F、貼「中」、換筆、注音取消／提交及 Vim NORMAL 重測，三筆高亮正常；關閉搜尋後明確原生貼 `/` 到 MainMenu，關窗收到 acknowledged Close。`os-terminal-search.json` 綁定最新 App binary hash；較早 cached article 及 Dioxus 的 OS 紀錄保留其原 binary 身分。

## S1／S5 原生影片可行性（2026-11-08，macOS arm64）

新增獨立 `native-player-probe` feature 與 `pttstella-slint-player`。一般 `slint-research` 和 Dioxus 入口不因此需要 libmpv；本節是播放器實驗，尚未接上產品的媒體預覽、下載政策及文章 scope。使用 Slint Winit／Skia 的 OpenGL 3.3 notifier，libmpv 直接畫到同一 GPU framebuffer，再由 Slint 疊上控制列。沒有逐幀轉成 Slint bitmap，也沒有啟動外部播放器。此路徑目前需要 OpenGL，不代表一般軟體 renderer 已能播影片。

鎖定 `libmpv2-sys 4.0.1` 與 `glow 0.18.0`，本機 runtime 為 Homebrew mpv 0.42.0。ABI 邊界使用確切 C struct：FLAG 為 `i32`，FBO 包含 `internal_format` 第四欄。GL proc loader 只供同步 context create 使用；控制 worker 只做 client API，Slint 同一 GL context／thread 擁有 render API。callback 只發 bounded wake，不重入 mpv。保存／復原共享 GL state，關窗時先在 current GL context 釋放 render，再取消 HTTP、join control，最後釋放 core 和 join fixture。callback allocation 保留到對應 owner 結束。

控制 channel 容量 8，每次最多處理 64 筆事件，UI／frame wake 合併，不用持續 polling timer。demux 前向／後向分別設 16／4 MiB，decoder threads 各 2；draw surface 限每維 8192 及總計 16M pixels。這些不是完整的解碼記憶體上限證明：高解析度來源、解碼中間畫布及同時多媒體的峰值仍待 S5 驗證。實驗只接受明示 port 的 loopback HTTP 影片，內建 fixture server 限一份 1..16 MiB 檔案、64 requests、8 KiB header 和 socket timeout。

```sh
# 本機須另有 libmpv 開發庫；可指定 MPV_LIB_DIR，或由 pkg-config 找 mpv。
cargo build --locked --release --features native-player-probe --bin pttstella-slint-player
python3 tooling/generate_native_player_fixtures.py NEW_MEDIA --ffmpeg /path/to/ffmpeg-with-theora
target/release/pttstella-slint-player --fixture NEW_MEDIA/quadrants.mp4 NEW_OUT
target/release/pttstella-slint-player --fixture NEW_MEDIA/quadrants.mp4 NEW_STALL_OUT --stall
cargo test --locked --release --features native-player-probe --bin pttstella-slint-player
```

fixture generator 自製 320×180／24 fps／3 秒四色、移動白方塊及低音量 440 Hz 聲音，標示 CC0，無下載素材或字型。manifest 記錄 FFmpeg 命令、版本、長度與 SHA-256。三個格式為 H.264／AAC MP4、VP9／Opus WebM、Theora／Vorbis OGV。第一次 FFmpeg 缺 Theora encoder 的失敗保留；完整三格式改用本機 keg-only ffmpeg-full 生成，沒有修改 shell PATH。

macOS arm64 真實原生操作已觀察三格式播放、跳到 1 秒、四色畫面與正常退出；MP4、WebM 的 `hwdec-current` 回報 videotoolbox，OGV 回報 no。明確按「核對四色」才在 AfterRendering 讀回四個 RGBA pixels（每次 16 bytes，最多 8 次），使用獨立四色預期及 ±30 codec 容差；一般播放不讀回像素。MP4 在 960×640 與原生 zoom 後的 1920×942 均通過，三格式最終報告 GL errors 為空。MP4 另實測播放至 0.25 秒後按暫停，停止在 0.29 秒。三格式都回報一聲道；沒有人工聽音證據，不宣稱實際出聲驗收通過。

下載取消案例在 HTTP 已收到一次請求、只送 header 而正文仍為零 bytes 時關窗，HTTP／control／mpv core 正常結束。損壞 MP4 顯示 event error -17，明確再按播放才收到第二次請求，仍可正常退出。最初重複 cancel 會在 server 已退出後再次 connect，造成 Connection refused 並漏寫報告；已改成冪等取消，並用真實 socket 測成功、stall、worker 原始失敗與重複取消。初始 GL state query 的 1280、讀取 App 外 fixture 時的 macOS open 停滯，以及編譯／Clippy 失敗均保留。open 停滯只確定在 File::open，未證實成因；改為 App Resources 內 fixture 後成功，沒有調整系統權限。

證據在 `.local/verification/slint-native-player/`，不同 binary 的原生結果保留自己的身分；新增 loading 提示後另補驗 MP4、實際暫停和 stall 關窗。六項新測試、共用 lib 283／Slint bin 42／multi-instance 10（原 ignored 保留）通過。播放器 Clippy、來源 patch、binary linkage、fixtures、原生動作 receipt 及清理清單亦在同處。CUA 畫面位於工具紀錄，沒有將其冒充磁碟上的原生 screenshot artifact。

目前 App 仍動態連結本機 Homebrew runtime，不能交付為乾淨機器可用包。read-only `otool -L` closure 為 49 個 Homebrew dylibs、65,424,352 bytes，尚未包含 dlopen 路徑及安裝驗收。下一步須完成可重現 runtime bundling／codec 授權清單、來源尺寸及取消預算、GPU／軟體後端取捨，並接回既有 URL／快取／generation owner；GIF、圖片、編輯器與 Windows／Linux／macOS x64 仍依完整計畫繼續。S1／S5 及整體 S0–S8 皆未結案，沒有同負載效能、跨平台播放或正式切換結論。

### 原生 runtime 封裝與隔離驗證（2026-11-08）

共用的 `tooling/native_runtime_macos.py` 提交為 `b8577a4`。它以 Mach-O load commands 分開 ID、imports、weak imports 和 rpaths，遞迴收集實際需要的庫；限制 128 個、總計 384 MiB，拒絕 basename 衝突、遺失來源及 allowed prefix 外的非系統依賴。複製後將 ID／imports 改為 `@rpath/<basename>`，刪除開發機 rpaths，只加入 `@executable_path/../Frameworks`。逐檔 ad-hoc 簽章後再驗 App。這次完整盤點補上先前漏掉的 Brotli common 庫，共 50 個 dylibs、66,438,752 bytes；前一節 49 個是當時較弱的盤點結果，沒有改寫成已完整。

`tooling/bundle_native_player_macos.py` 只建立獨立播放器實驗包，不切換入口或發佈。已有同名 App 時拒絕覆寫。自帶三個經 manifest SHA-256 核對的 CC0 影片，Rust／字型／Skia notices 與 42 個原生套件的授權文字、Homebrew recipe、安裝 receipt、SBOM、來源 URL／checksum 一併保存。LuaJIT 的大寫 COPYRIGHT 現在會納入。GLib 安裝 keg 缺授權文字，從其 SBOM 指定的官方 2.90.1 source archive 下載並核對 SHA-256，選出 LICENSES 等文字；不執行或展開 archive 的程式／symlink／上層路徑。這只是來源與 notices 盤點，尚未完成所有套件的對應原始碼交付及正式散布條件驗收。

```sh
python3 tooling/bundle_native_player_macos.py \
  --executable target/release/pttstella-slint-player \
  --media NEW_MEDIA --skia-license PATH_TO_BUILT_SKIA_LICENSE \
  --output NEW_BUNDLE_DIR --allow-prefix /opt/homebrew \
  --source-archives VERIFIED_SOURCE_ARCHIVE_DIR

# 顯式 toolkit 驗證，仍建立真正原生視窗／GPU renderer。
APP/Contents/MacOS/native --fixture APP/Contents/Resources/media/quadrants.mp4 NEW_OUT --verify
APP/Contents/MacOS/native --fixture APP/Contents/Resources/media/quadrants.mp4 NEW_STALL_OUT --verify --stall
```

`--verify` 只允許 owned fixture。它限時 20 秒，從同一 UI action callback 驗零自動 HTTP、播放、1 秒跳轉／暫停、四色像素、恢復時鐘、再次暫停及正常關窗。只有這個明確 probe 開啟 20 ms 有界檢查 timer，一般播放器不建立它。報告必須等 render teardown、control join、core destroy 與 HTTP join 後才可通過；GL error 或 cleanup 異常仍失敗。這些 toolkit 事件不能當 OS 鍵盤／滑鼠或實際出聲證據。

macOS `sandbox-exec` 的測試程序內禁止讀取 `/opt/homebrew` 和 `/usr/local`；相同 profile 下讀取已安裝 mpv license 的負控制得到 Operation not permitted。封裝後三格式和只送 header 的 stall 案例全數通過 `--verify`，dyld trace 的 50 個非系統庫全部來自 App Frameworks，沒有 Homebrew 或其他外部非系統映射。這不是另一台乾淨機器的安裝證明。首次受限啟動的程序存活，但 CUA 無法依 path／ID／名稱綁定視窗；保留 sample／dyld log 後只終止該自有 PID，沒有冒稱正常退出。

一般啟動另用 CUA 實際按播放、跳轉、四色、暫停和 AboutSlint，再正常關窗；初始 HTTP 0，按播放才變 1，GPU 四色與 VideoToolbox 成功，實際暫停停在 0.29 秒，所有 owner 結束。dyld 同樣只有包內非系統庫。仍觀察到 macOS 間接映射系統 WebKit／WebKitLegacy／WebCore，沒有將靜態 import 無 WebView 說成執行期零 WebKit；內容的繪製路徑仍是 libmpv／Slint，S8 的 OS 框架差異與接受判定保留。

回歸又抓到取消 wake socket 的競態：worker 已 accept 並關閉 wake 時，macOS connect 可能回 ConnectionReset／ConnectionAborted。現在只將這兩種及 ConnectionRefused 視為自有 wake 已終止，不吞媒體請求或 worker 的原始失敗。重複取消的測試擴為 64 次，保留失敗紀錄並以修正二進位重新跑封裝案例。六項 Rust 測試、四項 Python loader／archive／fixture 測試及 native-player all-targets Clippy 通過；共用模組的三項測試亦從 exact staged source 獨立執行。

本節 manifest、每份 binary 的身分、原生／toolkit 範圍、dyld audit、source patch 與 obsolete App 清理仍在 `.local/verification/slint-native-player/`。最新交付是 `bundled-runtime-final/Native-Player-Bundled.app`，只供本機播放器驗證；一般 Slint 入口尚未使用它。乾淨安裝、實際音訊、解碼尺寸／中間配置上限、軟體後端、產品媒體政策、GIF／圖片及其餘平台仍待完成，完整 S0–S8 目標維持 active。

## S5 主視窗圖片與動圖預覽（2026-11-08）

Slint 主視窗的圖片連結現在開啟原生完整預覽。URL 從已確認、已過濾的 Rust projection 取得，callback 只接受 link index；明確「瀏覽器開啟」仍走原外開路由。實站入口尚未操作，合成 probe 以外的其他既有 gate 保留原外開觀察路徑，避免測試因新增圖片功能去下載 example.com。影片目前仍走 Slint 原型原有的外開，前節播放器尚未接到此完整預覽，沒有宣稱影片平替完成。

新 `media::native_image::Decoder` 提交為 `6d9b186`。它從同一 `Arc<loader::Image>` 的 bytes 解碼，不複製原始下載內容或 collect 全部動畫幀；PNG／JPEG／WebP 靜態圖輸出 RGBA，GIF／APNG／動畫 WebP 用 incremental Frames。解碼前重驗來源尺寸、byte／frame 上限與原始 header，decoder 設每維 8192 和 192 MiB allocation limit，仍受原 loader 的 16M pixels、GIF 24 MiB／512 幀及總幀像素准入限制。靜態圖套用 EXIF 方向，旋轉後的顯示尺寸取實際 pixels。五項獨立 expected 驗證 PNG 左紅右藍、GIF 紅／藍與 300 ms、WebP 半透明藍、手製 EXIF orientation=6 的 JPEG，以及錯誤尺寸拒絕；同一 staged module 在 committed HEAD 的 desktop feature 下另跑五項及 all-targets Clippy。

完整預覽首次使用才建立一個媒體 thread／current-thread runtime；原來的 Loader／Preview 管 HTTP、redirect、TLS、容量、成功解碼後的 cache 及取消。像素解壓縮、首幀／動畫和 SharedPixelBuffer 轉換都在此 thread，BBS 使用原有另一個 runtime，Slint UI 只安裝結果與操作幾何。命令 channel 容量 8，watch 只保留最新結果，一次一個 decoder；重建循環或首幀前先釋放舊 decoder 的畫布。一般靜態圖／停播不建 UI polling timer，動畫依 delay 喚醒（目前保守最小 20 ms）。這是配置／工作數的邊界，尚未測實際大型圖峰值或 S8 效能。

開啟與重試各有新的 UI request ID，晚到結果不得覆蓋新預覽；成功解碼的同 URL 原始內容可從原 Loader cache 重用。來源沿用共用 `media::hover::Lease`：generation／input epoch 不變、來源已完整且同一 filtered link 仍存在，才續用。新的使用者手勢仍要求已安裝 geometry ACK；已授權圖片不因單純 resize 撤銷。文章路徑由同一 SurfaceState 的 current Paint 提供 links，raw 合成路徑由同一 View.frame 提供，沒有重解 BBS 串流。輸出／政策通知也會檢查媒體，不只依靠動畫下一幀。

預覽有符合、原尺寸、1%–800% 比例範圍、游標錨點滾輪與拖曳；手動停播／失焦回首幀，前景恢復可繼續。開啟時暫停文章背景取頁、保活、BBS 鍵盤／站端滑鼠及其他工具。背景終端元件暫不顯示，保留其 layout；Tab 因此只循環預覽控制項，Esc／關閉後回到目前終端或文章焦點。UI completion callback 先撤銷，再通知真正的媒體 worker 關閉 HTTP／decoder 並 join；capture 與一般關窗路徑都執行同一 finish。原生媒體 fixture server 已從播放器移到共用 `media::probe_server`，播放器兩項測試另編譯確認，沒有第二份近似 server。

```sh
cargo run --locked --release --features slint-research --bin pttstella-slint -- --probe-media
cargo test --locked --release --features slint-research --bin pttstella-slint media_worker::tests
```

`--probe-media` 自帶 CC0 PNG／GIF 的 loopback HTTP 與同一 Connection 嚴格 BBS peer。三項真實 worker 測試驗證：成功 decode 後重開 cache 是同一 encoded Arc 且僅一次 HTTP；GIF 停播 650 ms 沒有新 frame、恢復後繼續且不重抓；第一份 HTTP 卡在 header 後時換成新 request，只有新圖能抵達，close 清掉結果且 join。共用 lease 測試仍獨立核對換 input epoch／generation、移動或消失 link 與不完整資料拒絕。

macOS arm64 的實際 CUA 操作已驗 PNG 點擊、四次放大、拖曳、重試、Esc 與 GIF 播放／紅首幀／恢復。切到另一個自有診斷視窗時，GIF 按鈕變播放且畫面回紅首幀，回到前景後再次變播放中；沒有操作使用者其他 App。原生工具在關閉輔助視窗時曾回 frontmostApplicationChanged，重新查同一 inventory 確認它已退出，再回到仍存活的圖片視窗，沒有重啟 probe。較早 no-store PNG 重試得到兩次 HTTP；cache 版本顯示「記憶體快取」且最終 PNG／GIF 各一次。這些原 native-os／native-cache-os log 保留自己的 binary 身分。

最終來源的另一輪 CUA 核對背景終端控制項從 AX 樹移除、Tab 回到同一縮小按鈕、不進入終端、原生 zoom 放大後預覽仍開啟、快取重試、Esc 恢復終端欄、GIF 停播紅首幀。預覽內實際 j／Return 沒有送到站端；正常關窗 BBS Close acknowledged，兩個 HTTP server 均 joined、PNG／GIF 各一次。畫面與 AX 位於 CUA 工具紀錄，沒有宣稱已輸出 screenshot 檔。

證據與最新 `Native-Images.app` 在 `.local/verification/slint-native-images/`。尚未完成 hover／升級零重抓、圖片原尺寸複製／原始檔另存、文內單張／全開／自動開圖、兩張可視 GIF 的共同預算、動畫 disposal／blend／有限循環／短 delay 的完整對照、錯誤與大型資源的原生案例，以及影片主視窗接線。Windows／Linux／macOS x64、乾淨安裝、S6 編輯器、資料回退及 S8 同負載效能仍待完成；本節不把 S1／S5 或整體目標標為通過。

### 原尺寸圖片輸出與圖片選單（2026-11-08）

共用 `media::picture` 提交為 `5a78ad1`。Dioxus 及 Slint 現在共用原尺寸首幀／EXIF 方向解碼、原始格式副檔名檢查與原始 bytes 原子替換；移除 Dioxus 原來的近似解碼實作，配置限制與原生 Decoder 一致為 192 MiB。這是 decoder 配置限制，並非整個程序峰值記憶體驗收。引入共用 arboard 輸出時，編譯器在原 UAO 呼叫出現泛型 trait 推導溢位；已明示固定的 `[u8; 2]`，保留原始失敗 log，沒有增加 recursion limit。獨立 staged tree `6ff654d38345159a7059b77836f6f050be68ba6e` 的 desktop lib 288、Dioxus bin 148 及 Clippy 通過，原 ignored 保留；與候選 worktree 的額外服務／探針測試分開計數。

Slint 完整圖片預覽新增原尺寸複製、另存原始檔與複製完整網址按鈕；右鍵沿用既有 `ContextMenuOverlay`，提供相同四個圖片選項，方向鍵／Home／End／Enter 可操作，Esc 或 Tab 只關選單，再次 Esc 才關預覽。另存使用 parented 原生 picker，不重抓、不把畫面縮放結果當作輸出；GIF 複製首幀，另存保留原本動畫。選檔前取得目前 Rust 輸出租約，文章模式由同一 SurfaceState 註冊，原始終端模式由同一 View 建立；worker 在解碼後、picker 回覆後及 OS 寫入前重驗。關閉、重試、換來源、規則或 session 失效會撤銷；同一輸出 owner 一次一項工作並 join，不另建第二份剪貼簿 owner。

新增的共用測試用獨立紅色首幀預期及檔案 bytes 核對 GIF 首幀／原檔，並驗證租約拒絕和錯誤副檔名時既有檔案保持不變。現有 Dioxus 圖片輸出正／負向兩例也沿新共用 owner 通過。最新 worktree 的 lib 294、Slint bin 45、media integration 3、multi-instance 10 與兩前端 Clippy 通過；忽略項目沒有改成 passed。

macOS arm64 實際 CUA 先以 binary `ee6e22d8ae37e8e3357367d92f1c5982f91788c80dc31720f1648fa9bba7503b` 驗 PNG 放大後右鍵、Esc、Return 複製、完整網址、原生另存 PNG，以及 GIF 選單 Down／Return 另存。PNG／GIF 儲存檔逐 byte 等於 CC0 fixture。明確 `--probe-media` 才在實際複製後讀回 OS 剪貼簿，獨立核對 64×32 每個 RGBA：PNG 左紅右藍、GIF 全紅；正常入口不做這個讀回。兩 HTTP 各一次、BBS Close acknowledged、HTTP joined。該輪發現 GIF 換幀會蓋掉操作結果，沒有把提示驗收判為通過。

修正後 binary `30e21cd5899dd5903120acab5f45169e6c8c80dc109d3e6c24ea34f8beb7af61` 另驗 GIF 複製、取消另存、選單方向鍵複製 URL、原檔儲存及持續播放時的提示保留，再驗 PNG 放大後複製。原生 `j`／Return 未送到嚴格 peer，Esc 後焦點回終端，正常關窗，PNG／GIF 同樣各一次且 owner 結束。兩個 binary 的 native log／receipt 分開保留；CUA 畫面只在工具紀錄，不冒稱有 screenshot 檔。AppKit 的 IMKCFRunLoopWakeUpReliable 訊息保留，沒有因此將 acknowledged Close 判成失敗或宣稱已找出其成因。

最新實驗包為 `.local/verification/slint-native-images/Native-Pictures.app`，取代同階段舊的 `Native-Images.app`。本階段 `picture-source.patch`／JSON、`picture-verification.json` 及清理紀錄保留來源與 binary 身分；前一階段 source／verification 檔不改寫。hover／免重抓升級、文內展圖、完整動畫語義、影片整合、大圖拖曳邊界及資源峰值仍待完成，尤其本節未對 pan clamp 與大型圖作原生驗收。其他平台、S6–S8 與整體 S0–S8 仍 active；沒有整合主 checkout、推送、切換入口或發佈。

### GIF 循環及短延遲修正（2026-11-08）

檢查鎖定的 image 0.25.10／gif 0.14.2 原始碼時，確認 image 的 GIF `loop_count` 將沒有 extension 的預設值映射成無限循環，且有限值未包含初次播放。共用修正提交為 `709a9a6`：沿既有 Loader 的有界 GIF record walker 同時取得幀數與 repetition，沒有第二份近似 parser 或原始 bytes 快取。缺 extension 只播一輪、值 0 無限、值 N 總共 N+1 輪；能讀取位於最後一幀之後的 extension，comment 內相同 byte pattern 不授權循環。此語義核對 [WebKit GIFImageDecoder](https://github.com/WebKit/WebKit/blob/main/Source/WebCore/platform/image-decoders/gif/GIFImageDecoder.cpp) 與 [Go GIF decoder](https://github.com/golang/go/blob/master/src/image/gif/reader.go) 的原始碼，尚非本機 Dioxus host 的逐幀比較。

原本保守 20ms 下限改為瀏覽器常見的 0–10ms 延長至 100ms，其餘保留來源延遲，對照 [Chromium DeferredImageDecoder](https://chromium.googlesource.com/chromium/src/+/HEAD/third_party/blink/renderer/platform/graphics/deferred_image_decoder.cc)。CC0 標準庫產生器新增 `once.gif`（沒有 repetition、兩幀 0／10ms）及 `twice.gif`（repeat=1、每幀 30ms）。原來 PNG／循環 GIF fixture 最終未變。整個產生器執行時曾產生不同 PNG bytes，已復原既有 PNG；沒有更動 PNG 產生邏輯，也未證實差異成因。重現檢查只比較這兩份新增 GIF 及原 motion.gif 的 generator bytes。

Slint 媒體 worker 另區分自然播放結束與使用者／失焦停播。自然結束保留末幀，不再排 timer，也不因重新前景而自行重播；明確播放才重新建立同一原始內容的 decoder 並重設輪數。手動停止及播放中失焦仍回首幀。真實 socket／worker 案例對上述兩種 fixture 分別核對 2／4 個 frame、藍色末幀、前背景變更不重播、明確播放後再次完成 2／4 幀，以及全程一次 HTTP、正常 join；這是實際 worker 時鐘與計數證據，不是 OS 呈現時戳。

macOS arm64 的 native App binary `c74ca198e289c5140a352701c4ab79bd55ab51508a9ea020783ca205be19327f` 加入第三個 owned loopback「Once」連結。首次 PID 95137 的座標操作回報 changed app／noWindowsAvailable；AX 可讀，sample 顯示主 event loop，不能據此宣稱視窗實際收到點擊。正常關窗嘗試前後又觀察到舊 PID 消失、同一 executable 出現新 PID 96416，沒有足夠證據判定成因或首次正常退出；首次原生驗收不採用。操作工具的 optional launch API 不存在，也未執行該呼叫。保留 sample、工具紀錄及確切限制，未放寬前景保護。

後一次現存 PID 的座標操作成功：單輪 GIF 開啟後停藍色末幀、顯示播放鈕，按播放後仍可正常到末幀，複製回讀紅色首幀；短動畫在 CUA 觀察前已結束，沒有以這個畫面證明中間播放輪數。另回歸循環 GIF 的藍色播放畫面、手動停回紅色及恢復播放。preview 內 j／Return 沒有送站端，Esc 回終端焦點，正常關窗 Close acknowledged；Once／GIF 各一次 HTTP、PNG 零次、三個 fixture server 均 joined。只有此輪 log 列為通過，來源與 binary 身分保留在 `.local/verification/slint-native-animation/`。

最新 worktree lib 296、Slint bin 46、media integration 3、multi-instance 10 與 Clippy 通過。共用提交另從 exact tree `b95dbf6e5eaae6b0f05ea8ffb31adf43a9b70cbe` 跑 desktop lib 290、Dioxus bin 148 及 all-targets Clippy，原 ignored 保留。最新 main-window 實驗包為該處 `Native-Animation.app`，取代前節 `Native-Pictures.app`；兩階段 source patch 與 OS receipts 仍分別保留。尚未宣稱 disposal／blend、WebP／APNG、實際瀏覽器時序、大圖資源、跨平台或完整 S5 通過；後續仍依 S0–S8 目標繼續。

### 原生 hover 與完整預覽升級（2026-11-08）

Slint 主視窗新增右上／跟隨游標／關閉小預覽，350ms 停留後才建立媒體 worker 及請求；同一已確認連結內移動只更新位置，不重設倒數。Source 從同一 Rust 已確認 geometry／過濾 projection 命中，圖片外的連結不會因 hover 外開。拖曳／選取、組字、送鍵、站端滑鼠、對話框、失焦及失效來源會取消；失焦後只重新前景不會復活舊指標。已開始的 hover 可經過尚未移動的左鍵按下，在放開同一連結時升級，避免按下就撤銷下載。

共用 source correlation 提交為 `1efecce`。`Lease::same_source` 比較 generation／input epoch／完整 URL 與行欄範圍，允許不相關 repaint 的 revision／index／sequence 差異；它只比對兩份已授權命中，不取代完整性、規則、呈現及指標准入。獨立 staged tree `72829bf15eba304ad9e11dc5e55b5fc6f6fcbf26` 的三項 media integration 與 headless all-targets Clippy 通過；擴充 expected 核對 URL query、起訖欄、行、generation 及 input epoch 改變必須拒絕。

Media worker 現在明確接收 Hover／Inspect 和 Promote，同一個既有 Preview／Loader／cache 擁有下載。Promote 直接沿用 Preview 的 in-flight future、request、原始 Arc 與已解碼首幀，不從 cache 重新 open，也不複製編碼內容。hover 不排動畫 timer，GIF 維持首幀；升級後才依完整預覽政策播放。兩個新增真實 socket／worker 案例分別驗成功解碼後的 GIF 靜止 650ms、升級後同一 Arc 播放，以及只送 HTTP header 後升級、再釋出正文仍只有原先那個請求。這兩例使用 no-store，不能用 cache hit 掩蓋重新請求。

小卡不含 TouchArea 或可取得焦點的輸入元件，圖片維持比例、不放大超過原尺寸。卡片上限為內容 viewport 的 55% 寬／60% 高及 560px 寬；右上留 12px，跟隨距指標 16px，碰邊反向再 clamp。偏好控制列另占一列，提供滑過會下載的說明；原生約 640px 寬視窗另確認控制項及預覽可用，沒有將合成四角屬性檢查說成實際邊緣 hit test。

偏好寫入沿既有 Settings Store 的欄位合併、原子寫入及 OS lock，由目前 output worker 執行，一次一項、關閉 join。保存成功才生效；失敗保留所選草稿及原生效值並提供重試。正常 `--connect` 只用 SlintResearch 設定路徑；`--probe-media` 每次啟動都用持有到 worker drain 後的 owned TempDir，不讀寫 production store 或 vault。因此本節不宣稱已完成實際 App 重啟、舊產品資料回退或完整 S7 的關窗失敗提示驗收。

```sh
cargo build --locked --release --features slint-research --bin pttstella-slint
SLINT_BACKEND=winit-skia-software target/release/pttstella-slint \
  --probe-media --verify-media NEW_DIR
```

`--verify-media` 僅接受 owned media probe。它使用真實 Slint component 的 toolkit pointer／click 和明示 synthetic foreground，核對快掠零 worker、350ms 前零請求、同連結移動不重設倒數、首幀、同 request／Arc 升級、GIF hover 不播放、Off 零新增及一般連結提示保留、跟隨與四角合成位置、失焦取消、重新前景不重開，以及 source Frame 完全不變。設定案例另模擬另一個 Store 同欄先寫入，確認失敗不套用、不丟草稿、不覆蓋外部值。最終 `accepted/result.json` 通過，PNG／GIF 各一次 HTTP、Once 零次、BBS Close acknowledged、三個 server joined。`accepted/*.png` 是 Slint snapshot，已實際檢視版面；不是 OS 螢幕呈現時間或原生輸入證據。

首次 toolkit runner 沒有先取得輸入焦點而失敗，修正為先等 frame、排空 startup resize／focus，再只在明示 probe 中做 toolkit activation／focus；沒有放寬正常模式。前幾輪編譯錯誤及舊 toolkit 產物保留，不當作最新通過。第一個 native binary 又重現 Off 清掉一般連結網址提示的 adapter bug；已縮小取消範圍，加入上述回歸。`first-native-app.json` 保留該 binary 身分及未通過範圍，不以它宣稱關閉模式完整通過。

最新 native App binary `4cc738018dd724c0f8746ff517575956f8f6af909a44aa80e6a26aa78a829817` 經 macOS arm64 實際 CUA 驗證 Off 保存、停留仍有完整 URL 提示、明確點擊仍開 PNG、跟隨保存、GIF 紅色首幀、點擊升級到藍色播放、預覽內 j／Return 不送站端、Esc 回終端，以及原生拖曳縮窄視窗後的卡片定位。實際 hover 由區域外拖曳進入連結後放開來定位；這個定位手勢本身會影響焦點，不能證明被動 OS pointer move 不改焦點。工具沒有可用的零位移 scroll，正值微量 scroll 也未取得 hover，沒有拿這些嘗試當成功。被動事件的焦點不變目前只有 toolkit 證據，OS 被動指標／組字仍待補。

兩次 native launch 各使用自己的 `native-run-*/trace.log`，避免重新綁定時覆寫前一輪。最新一輪 GIF 僅一次 HTTP，hover 升級沒有重抓；PNG 在明確開啟及稍後窄視窗 hover 共兩次，當時 UI 沒有 cache 標示，不用這一組證明 cache hit 或推定其原因。cache 重用採短時間 toolkit 與 worker 證據。正常關窗 Close acknowledged，所有 fixture server joined。CUA 畫面只在工具紀錄；不宣稱另存了 OS screenshot 檔。

最新 lib 296、Slint bin 48、media integration 3、multi-instance 10 及兩入口 all-targets Clippy 通過，原 ignored 保留。證據在 `.local/verification/slint-native-hover/`，最新包為 `Native-Hover.app`，取代前節 `Native-Animation.app`；來源 snapshot、stage tree、各 binary／toolkit 範圍與 exact removed paths 分別記錄。hover adapter 及累積 Slint 移植仍未整合主 checkout，共用 source helper 已獨立提交；沒有推送或發佈。

S5 仍缺文內單張／全開／自動開圖、兩張可視 GIF 的共同預算、完整 disposal／blend／WebP／APNG 對照、大型資源／錯誤／pan 邊界及影片主入口整合。S4 其餘導覽／收藏、S1 編輯器風險、S6–S8 及四架構原生、回退、同負載效能也未結案；完整 S0–S8 目標維持 active。


## S5 展圖批次、圖片間隙與播放排程（2026-11-08）

既有 Dioxus `InlineImages` 已移至共用 Rust `media::inline`（`74b4059`），舊入口只 re-export 同一 owner。原有單張／全開、URL 去重、文章作用域、依序下載及 32 URL／32 MiB／16M 首幀像素預算保持原算法；同步入口借用已確認的 links，原生 worker handoff 只複製 link projection，不複製整篇格線。原有 12 個案例及新增的 handoff 案例均在 exact staged source 通過，Dioxus 對應測試亦通過。

共用 `article_flow`（`16f87ad`）用固定最多 32 個間隙保存文字列及圖片內比例錨點。Slint viewport 已消費該幾何：只產生可視及緩衝列，以 f64 文件座標相減後才轉為局部 f32；圖片間隙不取得 terminal cell，也不落入文字翻頁。滑鼠與選取快照共用 Rc 座標表，避免每次移動複製整張表。`--probe-article --verify-article NEW_DIR` 的實際 Slint 元件驗證加入／改高間隙後的文字及圖片錨點、拒絕非法間隙、既有列快取重用與第 837 列畫面像素。合成空間不是已完成的圖片顯示或原生 OS 手勢驗收。

完整預覽的增量 decoder／frame delay／有限循環／暫停首幀／自然結尾保留末幀已抽到原生 `Playback`，主預覽沿用該 owner。仍待把文章的兩張中央可視 GIF 接入同一排程及整體資源准入；沒有複製另一套動圖 reducer。實際文內圖片、網址箭頭、短／長 Enter、等寬、自動展圖、圖片選單與多圖峰值均仍為 not_ready，不以這批基礎工作結案 S5。證據在 `.local/verification/slint-inline/`，完整 goal 持續。

### 合成長文等待與診斷時鐘

使用者回報的等待來自測試包合成長文。既有 `full_load`／`long_load` peer 每頁故意等待 500 ms，兩半資料之間再等 150 ms，兩前端相同；這是取消、失焦與晚到頁的測試條件，並非正式連線中的固定逐頁延遲。保留原測試。共用探針另支援明確 `PTTSTELLA_READING_PROBE_FAST=1` 的立即回覆模式，僅影響這兩個合成 peer 的建立；沒有另交付快速示範包。

`fe6b7c2` 將有界、預設關閉的 `ReadingTiming` 移至共用 Rust，兩入口沿用 `PTTSTELLA_READING_TIMING=NEW.jsonl`。ACK 計時取實際已安裝 packet 的文章範圍及載入狀態，不能把較新快取的 complete 套在舊畫面上。Slint 取頁也調成 Dioxus 的順序：先送下一筆已核對請求，再建立畫面投影，仍只有一筆命令及原 foreground／scope 保護。

本機立即回覆的初步 38 頁／837 列診斷，Slint 五次取頁 complete 約 479–521 ms、Dioxus 一次約 523 ms；資料在 `.local/verification/slint-reading-speed/`。這批次數、視窗／字型和並行建置條件不符合 S8 的固定比較條件，不能據此宣稱速度改善、跨平台效能或畫面延遲 passed。兩版的取頁時間與畫面 ACK 分列，OS 螢幕呈現尚未量測。使用者確認不需針對刻意等待另做示範交付，工作回到 S5。


## S5 原生文內圖片與可視 GIF（2026-11-08）

`--connect` 已接入文內圖片控制，`--probe-inline` 提供隔離的兩頁／45 列文章及三個本機 HTTP 端點。Rust 已確認且過濾的連結索引產生網址末端箭頭；去重及跨列放置直接沿用共用 batch 的 target helper（`5a2a8fd`）。單張箭頭只排入該 URL，下載與首幀解碼沿用同一媒體執行緒，未展圖不建立 worker。圖片尺寸變動透過原生 flow 的列／圖片比例錨點補償，不進 terminal cells 或來源 PNG。

已取得的原始 bytes 和首幀以 Arc／SharedPixelBuffer 共用。原生 `Playback` 同時供完整預覽和文內 GIF 使用；同一執行緒最多保留兩張中央可視 GIF 的播放 decoder，總 animation_pixels 成本至多 134,217,728，離開可視／失焦／modal 時釋放播放集合並回首幀。靜態首幀仍受 batch 的 16M 像素總預算；沒有預先解出全部幀。高幀數預算案例用自製 512×512、306 幀 GIF，先經正式 HTTP loader 驗出 80,216,064 個幀像素；兩份都能保留首幀，但動畫集合只准一份。這是准入驗證，沒有宣稱程序或 GPU 峰值已通過 S8。

GIF frame 更新只修改可見圖片 model 中真正改變的像素。全文 links 以不可變 Arc projection 共用；控制位置只在投影更新時建立，動畫不複製 links、重掃全文、重排文章或重畫文字列。完整預覽在同一 actor 開啟前先停文章動畫，關閉後不需要另一筆站端／滑鼠事件就能恢復。點內嵌圖片或明確點它的網址，均驗證當前 scope／ACK／完整 link 後交付同一原始 Arc，沒有第二次下載。

實作中修正兩個由完整視窗／原生操作抓出的問題：Slint 版面值尚未穩定時，立即回呼可能留下過時的幾何；現在用一個有 owner、零延遲且可合併的更新 timer，再重驗實際已安裝的 row anchor／raster key，沒有放寬舊幾何拒絕。另有停用箭頭將點擊穿透給底下網址的問題；控制項現在攔住事件，由 Rust 判定是否允許，已開與選取中的箭頭不會意外開預覽。拖曳及修飾鍵不開圖；最終 toolkit 回歸另以 2.5×2.5 邏輯像素的斜向拖曳驗證圓形 3px 門檻，這個門檻微調尚未重跑 OS 手勢，App 保留上一個已原生驗證的 binary。箭頭上的 Shift 選字轉給同一文字選取 owner，圖片區及文字區共用滾輪／Ctrl 縮放處理。

本機完整元件 `--probe-inline --verify-media NEW_DIR` 驗證 PNG 真正畫面像素、單張隔離、GIF 動畫、點圖片與明確網址的原圖重用、modal／失焦／離開可視的暫停、縮放後拒絕舊手勢、已開／選取狀態的箭頭不穿透，以及原生一次 handoff 清空控制和圖片。peer 恰收一筆 PageDown 及 Close，PNG／GIF 各一筆 HTTP、未選的第三張零筆，全部 HTTP workers joined。它使用 synthetic toolkit pointer／foreground、typed navigation，與 OS 原生操作分列。

macOS arm64 的 `Native-Inline.app` 另有實際點擊、PNG 紅藍像素、GIF 藍色幀、完整預覽、停止首幀、Esc 焦點復原、Home／End、圖片區滾輪與 125% 縮放觀察。初版原生 run 各圖片端點一筆；後續真實選取抓出箭頭穿透，保留 PNG 兩筆的負向 run；最終版重測已開箭頭、真實網址選取、選取中的箭頭及明確網址預覽，PNG 仍恰一筆、其餘零筆。Vim 原生一次在獨立 run 清掉內嵌圖片並呈現真實 80×24。Cua 在第一次關窗後 rebind 出一個背景探針，該 run 在未翻頁前結束而被 strict peer 拒絕，保留記錄並單獨列為不完整，沒有混入通過結果。實際 Cmd+W 曾產生注音 preedit，尚未具備完整 macOS 原生 menu 快捷鍵驗收；原生關閉鈕可結束且 joins／ACK 通過。

當前證據、最新 App、原生分 run trace、SHA、source patch 及重跑命令在 `.local/verification/slint-native-inline/`；先前合成空間證據 `.local/verification/slint-inline/` 不覆寫。仍待短／長 Enter、等寬、自動展圖設定保存／生效、圖片右鍵選單、多圖實測峰值、原生影片／編輯器整合及四個目標原生驗收。S4 的本機頁尾行範圍／頁碼／百分比還需接線，目前來源頁尾不等於本機可視範圍；S7 原生 menu／完整 AX／Tab 亦未結案。這不是 S5、S8 或完整候選版通過。

### Enter 展圖與等寬裁切（2026-11-08）

Slint 已沿用既有媒體操作契約：短按 Enter 在放開時選可視的第一個未展開索引，尚未收到 batch 確認仍選相同索引；可見圖片都已開啟後才逐張等寬。網址列已在畫面外但圖片仍可見時，仍列入同一文章行／欄順序。等寬解除原尺寸及 560 邏輯像素高度上限，下載、batch、原圖及播放 owner 不變。長按 500ms 送同一 batch 的 All 一次，後續取到的新內容亦沿原本全開模式處理。

Winit 的真實 Enter 在原生 TextInput／站端鍵盤之前攔截，先結束 keyboard 借用才呼叫 UI；Slint 的 capture key callback 則讓完整元件測試走同一 owner。只有文章本機導覽、已安裝幾何、前景／焦點及鍵盤政策均允許才建立一個 timer。Vim 原生一次、i 輸入、計數／前綴、組字、修飾鍵、沒有圖片及站端滑鼠保留原路徑。下載造成幾何變動可撤銷手勢授權，但實體 held 狀態保留至放開，避免長按全開後的 repeat／keyup 又成為短按。失焦可能沒有 keyup，故另清除 held；關窗停止 timer。沒有新增持續 polling。

大長圖的文章高度仍用既有 f64／u64 flow 與錨點。交付 Slint 之前先求可見的來源像素列，外層矩形只佔目前 viewport，內層只繪製帶整列誤差的來源切片。沒有把整張放大後的高度轉成單一巨大 f32 widget 或另配文章大小的 bitmap。獨立 expected 測試涵蓋 1×8192 圖片等寬到 8160、捲到第 7000 個來源像素，以及小數裁切；完整視窗另核對正常 PNG 放大／部分捲出的實際紅藍像素。極端圖的 OS 畫面、配置及程序／GPU 峰值尚未驗收。

這一段曾抓到 TextInput 先吃 Enter、全開的幾何回覆使 repeat 被誤認新短按，以及測試未等待捲動幾何 ACK 的問題。保留各自負向 log；分別改為 capture 階段、held／授權分開及明確等待實際幾何，沒有放寬 producer 的舊幾何拒絕。另有一次診斷 runner 在 build 尚未結束時先跑到舊執行檔，那組身分取樣不足的紀錄排除；最終三組均在 build 完成後、啟動前固定 SHA。

`.local/verification/slint-inline-enter/` 保存新的完整來源 patch、重放 tree、SHA、工具／原生紀錄。`PTTSTELLA_INLINE_KEYS_PROBE=short` 或 `long` 配合 `--probe-inline --verify-media NEW_DIR` 驗短按放開、可見順序、圖片本身可見的等寬與真實 widget pixels，或 500ms 全開／repeat／keyup、失焦／縮放／捲動取消。另重跑原本 pointer／選取／modal／來源清理案例。三組 strict peer 都只收一筆 PageDown 和 Close，三圖片全開案例各端點一筆 HTTP，所有 workers joined。預設 pointer receipt 的 pending 清單只代表該案例沒有走鍵盤；鍵盤範圍以獨立 short／long receipt 為準。

最終 macOS arm64 binary `3820f5fd983e1837d3d8a086cad0ecccb2da0e8432d0d6ed3d45b33ef4b1437b` 在真實 OS 視窗複驗長按全開、放開維持原尺寸、下一次短按才等寬及正常關窗；三端點各一筆且全部 join。前一個 binary 的獨立原生 run 另有短按單張、圖片捲到中間的裁切及下一張可見 GIF 等寬，沒有合併為最新 hash 的全部 OS 證據。原生畫面僅存 CUA 工具紀錄，磁碟 PNG 是 toolkit screenshot。最新 `Native-Inline-Keys.app` 取代同 fixture 的 `Native-Inline.app`；舊 source patch、trace 和 metadata 保留，清理 exact path 與空間前後另記。

release lib 315／Slint bin 56、Slint all-targets Clippy、fmt／diff 與文件 gate 通過，原 ignored 仍保留。先前 Dioxus／JavaScript gate 未因這段純原生程式重跑。接下來仍是自動展圖設定、圖片選單、媒體／編輯器整合與其餘 S0–S8；跨平台原生 IME／Enter、極端圖資源峰值、本機頁尾、原生選單、回退及同負載效能未通過。本節沒有切換入口、整合主 checkout、推送或發佈。

### 自動展圖設定與原圖快取（2026-11-08）

Slint 的「自動開啟文章圖片」已接入既有 AppSettings／Store，預設關閉。勾選先走同一輸出 worker 保存，成功後才套用；失敗保留目前生效值、使用者勾選草稿及重試入口。hover 與自動展圖現在共用一個媒體設定保存入口及 baseline，合併其他程序改過的設定後同步套用兩個媒體偏好，不建立第二份 JSON 格式或獨立寫檔流程。目前入口仍是原型工具列，完整設定中心的版面／分類及其餘設定屬 S7，未把這一列當作設定中心完成。

自動值不再只在 mount 時讀一次。保存後由現有媒體 worker 的原子政策值通知同一 actor；舊的投影命令不能把新關閉的值改回去，也不需要等新的站端 frame 才取消 HTTP／佇列／播放集合。UI 用共用 batch 的自動來源標記拒絕晚到的自動結果，清掉自動圖片間隙；明確手動 All 已把批次提升為手動時則保留。關閉後箭頭／Enter 仍可開圖。沒有額外 thread、HTTP owner、快取或持續 polling；設定檔 IO 沿原有輸出 worker 並 join。

共用提交 `223b51c` 增加 `InlineImages::automatic_batch` 及成功解碼原圖的 LRU 准入。准入驗證仍是目前 batch、resource index 和同一 Arc，Slint 首幀成功後才回報；失敗或舊 batch 不加入。沿現有 Loader 的 8 張／24 MiB、TTL／Age／no-store 和精確 URL key 規則，沒有另留編碼快取。完整視窗第一次驗證發現舊 InlineImages 未回報解碼成功，導致開關自動後 PNG 也重抓；這個負向 run 保留，沒有改成容忍錯誤的 expected。Dioxus 目前仍維持其原有未回報文內解碼 ACK 的行為，新共用入口尚未被它呼叫；沒有宣稱兩版都新增跨批次快取。

exact staged shared source 的 14 項 inline 測試與 desktop all-targets Clippy 通過，包括無 ACK 重抓、舊 batch ACK 拒絕、當前成功 ACK 後同 Arc 跨 scope 重用，以及自動來源／明確 All 提升。真實 worker 測試另在只送 header 的下載中關閉自動：該請求正文零 bytes，後續 URL 零請求，舊 follow 不復活；手動圖及 All 不受後來的自動關閉撤銷，全部 owner join。

`PTTSTELLA_INLINE_SETTINGS_PROBE=1` 配合 `--probe-inline --verify-media NEW_DIR` 跑隔離設定及真正 Slint 元件。測試從關閉／零 worker 開始，保存前不生效、外部 hover／字寬更新合併、新 Store 讀回相容資料、文內三圖、關閉清空、手動 PNG 原 Arc 重用、重新啟用、長按提升 All 後關閉保留，以及不支援版本的外部檔案導致保存失敗。失敗檔案保持逐 byte 不變，恢復自有 fixture 後明確重試才成功，來源 BBS frame 保持不變。設定路徑只由明確 probe 的 owned TempDir 交付，正常入口不讀這個驗證開關。

macOS arm64 的 `Native-Inline-Settings.app` 以 binary `346eda5c4320a94383fd8da8178f49db8a3eb860a0774565b6b3d65e24d6f066` 實際勾選、關閉、點箭頭、重新勾選、長按全開及再次關閉，畫面與提示符合上述規則。strict peer 恰收一筆 PageDown／Close；PNG 一筆、no-store 的兩個 GIF 各兩筆，全部 HTTP joined。這是正確 cache policy 的結果，不將 no-store 的重抓列為失敗。畫面在 CUA 工具歷史；磁碟 PNG 屬 toolkit snapshot。真正跨程序／OS 重啟保存及四架構驗收仍待後續，不以新 Store 讀回替代它們。

本段證據、來源重放、最新 App、原生 trace 及清理在 `.local/verification/slint-inline-settings/`。原本 hover、pointer、短／長 Enter 四組完整元件回歸通過。release lib 317／Slint bin 57，desktop bin 136／multi-instance 10、兩前端 Clippy、core 的 release 同等十套件與兩個 replay、JS 382 皆通過，原 ignored 保留。shell PATH 沒有 node 的初次嘗試沒有執行測試，改用既有 bundled Node 24.22.1 後通過，未另安裝 runtime。

最新 App 取代同 fixture 的 `Native-Inline-Keys.app`，保留歷史 source patch／原生紀錄。文內圖片選單與選單全開、原生影片／編輯器、完整設定中心／原生 menu、跨平台、回退及 S8 同負載效能繼續。完整 S0–S8 goal 維持 active，沒有整合主 checkout、推送、切換入口或發佈。

### 文內圖片選單、共用輸出及原生驗證（2026-11-08）

文內圖片沿用共用情境選單 policy 和原有 ContextMenu owner，提供複製原尺寸圖片、另存原始檔、複製完整網址及外開。按下／放開驗證連結索引、tag、目前文章幾何與來源快照，不把圖片區轉成 terminal cell；右拖曳取消，Shift 右鍵仍是圖片選單。載入中／損壞圖的複製和另存停用，網址操作保留。換來源、幾何、原生一次或失焦會撤銷；GIF 換幀沿同一原始 Arc，不關閉選單。

文章背景選單的「展開本頁圖片」走同一 batch 的 All；網址選單仍只列連結操作。浮動工具開啟時以 Shift＋背景右鍵進內容選單。原生 Shift 事件曾有 Slint pointer 和 Winit 狀態不一致，現在從原有鍵盤 owner 讀取 Winit Shift，和 pointer modifier 一起判定，不建立第二份可變鍵盤狀態。

完整預覽與文內選單共用 Output::picture_output；只有一份父視窗 picker、剪貼簿與原始 bytes 保存流程。取得來源、解碼後、picker 回覆後及 OS 寫入前重驗輸出租約。picker 期間輸出忙碌會暫停閱讀、保活和動畫；取消及 worker 實際 join 後恢復，並在鍵盤借用以外刷新鍵盤政策，修正輸出完成後 Vim 按鈕仍停用的問題。可視圖片 model 掛載一次，逐列更新／新增／移除；GIF 只更換變動像素，不重掃 links 或重排、重畫文章。

這一段曾把工具預覽中的黑區判成 Slint 黑圖，現撤回這項失敗判定。最小原生案例的同一份 OS JPEG 擷取，解碼後黑區位置實際是 `[236,46,24,255]` 紅色或正常灰色文字列；renderer 診斷也讀到正確來源與 Skia 像素。這證明僅看工具預覽不足以判定失敗，尚未證明舊 binary 有實際繪製缺陷。先前試過的 ready gate、獨立文字層、補發 redraw、裁切分支及延後 metadata 均已撤回，不作為產品修正或效能改動。原負向來源／日誌保留並加上判定更正，不改成聲稱舊 binary 全部通過。

最新一般候選仍為未加 renderer override 的 `c64c805b8fa37df89a1f3d2cd1947b886dff465ce879102164369372b46b5882`。macOS arm64／Skia software 實際 OS 操作從空批次選單全開，連續五個冷程序均保存原始 JPEG、其解碼後無損 PNG 和像素結果。PNG 左右兩半及兩張 GIF 的內部區域各 576／576／1344／1344 像素全符合獨立紅藍圖樣；同版另驗輸出後與自動展圖，共七份擷取。既有 `image` crate 的獨立解碼器另驗 JPEG 和 PNG，原生交接後已無圖片的擷取必須被檢查器拒絕。檢查只用自製 fixture、已知視窗及座標，有 JPEG／ColorSync 的色差容許，不宣稱所有平台、DPI、圖像或任意視窗都已通過。PNG 只轉檔，不重畫或修改擷取像素；數值與原始檔才是判定依據。

同一 hash 的原生操作另通過 PNG／GIF 原始另存、PNG 與 GIF 首幀的真實剪貼簿讀回、選檔取消、完整預覽原圖重用／複製／另存、Esc 焦點與 Vim 按鈕恢復，以及選單 j／Return 不送到站端。原生一次清空圖片、回到 80×24。另以勾選自動、完整網址複製、外開的本機 probe sink、關閉自動清空圖形驗證設定與網址路徑；probe 不啟動瀏覽器或連外。六個程序各只收一筆 PageDown／Close，三個 HTTP 端點各一筆，全部 joined；完整預覽沒有第二次下載。三份原生儲存檔逐 byte 等於 fixture。

同一生產 binary 的十一組完整元件涵蓋 basic／all／stall／error、auto、pointer、短／長 Enter、完整預覽及兩種情境選單。release lib 317／Slint bin 57 的同來源結果保留，新增診斷 example 和全部 targets 的 Clippy 通過；未冒充 Dioxus／JS 本輪重跑。原生擷取與更正在 `.local/verification/slint-first-paint/`，選單實作與既有 toolkit 證據在 `.local/verification/slint-inline-menu/`。獨立重跑入口為 [slint_picture_repro](../examples/slint_picture_repro.rs)；它也能在不啟動 GUI 下檢查保存的固定 native-inline 擷取。

最新 Native-Inline-Menu.app 取代同 fixture 的 Native-Inline-Settings.app；舊來源／紀錄及必要 raw hover／圖片／獨立播放器包保留，清理 exact paths 和空間另記。此處只完成 macOS arm64 原型的文內圖片選單／共用輸出範圍。影片主視窗整合、編輯器、完整設定／原生 menu、本機頁尾、可編譯提交序列、四架構原生、資料回退及 S8 效能繼續；完整 S0–S8 goal active，沒有切換 Dioxus、整合主 checkout、推送或發佈。


## S5 共用影片串流與主視窗播放（2026-11-08）

本輪把獨立播放器的 control／libmpv ABI／GL 狀態保存移入 `src/native_video/`，主視窗與獨立 probe 使用同一個 session。`native-video` 仍是明確候選 feature；既有 Dioxus 入口和一般 Slint 軟體 renderer 的診斷 build 保留。啟用此 feature 的 Slint 候選選擇 Winit／Skia OpenGL 3.3；無 GPU／renderer 重建、其他平台及整體候選門檻尚未驗收。

### 原生主視窗

直連 MP4／WebM／OGV 走原有已確認連結 lease、generation 與 sequence，打開同一個媒體 overlay，先顯示可播放狀態，明確按播放後才載入。Slint 顯示播放／暫停、秒數及可拖曳進度；圖片專屬縮放、複製及原始檔另存不出現在影片介面，完整網址及外開繼續走既有 owner。影片開啟仍暫停文章 GIF。失焦只暫停，不因恢復焦點自動播放。

影片 frame 留在 GPU：libmpv render 到有界 RGBA8 FBO，再在 Slint AfterRendering blit 到實際 viewport 的 physical-pixel 範圍。控制項仍由 Slint 繪製，沒有影片 frame 到 Slint bitmap 的 CPU 複製。沒有新影格且尺寸未變時沿用同一 FBO；動作由 decoder callback／UI redraw 喚醒，不增加影片輪詢 timer。表面上限 8,192 邊長及 16,777,216 像素，尺寸變動、退出預覽及 teardown 在同一 current GL context 釋放。此設計的效能收益仍須 S8 同負載量測，不能由實作方式直接推定。

一個 UI owner 維持一個 session 與一個可覆蓋的 pending request；圖形 context 先退場，再取消串流、join 控制與 runtime，最後才回收 core。Article 的原有關窗 shutdown 會一併等待媒體；一般 terminal probe 的 renderer teardown 後也檢查完整 join。快照不持有 core，core 只向 UI 移交一次；關閉回報與錯誤不以清空計數代替。

### Rust 來源通道

`media::video_stream` 綁定隨機 192-bit token 的 127.0.0.1 臨時端點，bind 不送來源請求。影片 worker 在同一 sleeping runtime poll listener／連線 futures；只允許精確 route 的 GET／HEAD 與單一合法 Range，不轉送 cookie、authorization 或 referer。圖片與影片共用 `media::http` 的 app UA、TLS、redirect 次數及拒絕 HTTPS 降級／內嵌帳密政策。最多四個本機連線、4 KiB 請求表頭、每次寫入至多 64 KiB；不快取整支影片。回應核對 MIME、encoding、Content-Range／Length 與實際 body；HEAD 使用來源 Content-Length 表頭，不能拿零長度 body hint 取代。

libmpv 關閉 config、scripts、ytdl、自動載入檔案與 references，使用 lavf 且 nested protocol whitelist 為 none。自己的 loopback 來源由 mpv stream 層讀取；容器不能另行載入播放清單所指的網路來源。兩份自製 M3U／HLS 偽裝 `.mp4` 以實際主視窗與獨立 trap listener 驗證拒絕及零 nested request。這些負向案例不代表所有容器、平台或惡意檔案已全面驗證。

### 驗證與限制

證據集中於 `.local/verification/slint-video/`。同機自製 320×180 四色／三秒／mono tone fixtures 沿用先前 CC0 manifest，不使用站端或私人媒體。明確 `--probe-media` 才會讀取 `PTTSTELLA_VIDEO_FIXTURE`，其輸出設定使用已存在的隔離 TempDir；`--verify-media` 在此 fixture 下走影片整合案例。`PTTSTELLA_VIDEO_STALL`／`PTTSTELLA_VIDEO_REJECT` 僅用來選擇明確驗證分支，正常連線入口不讀取這些 fixture。

已取得共用 HTTP／媒體回歸、實際 libmpv session、主視窗 MP4／WebM／OGV 的 toolkit 操作及 GPU 四點獨立預期、失焦暫停、停滯取消與圖片替換。UI 替換案例在同一回合撤銷待啟動請求；已建立 session 的啟動取消由共用 session 測試分開證明，不能將兩者混寫。主視窗 GPU 的驗證點使用實際 physical viewport，沒有借用獨立播放器的邊界或輸出。macOS 原生滑鼠／鍵盤另核對開啟、播放／暫停、進度跳轉、Esc 回終端焦點、重開及播放時關窗；原始 JPEG 擷取四個 24×24 內部區域各 576／576 像素吻合。具體 binary hash、每輪範圍與是否重跑在 `verification.json`；較早 hash 的輸入證據不能冒充最新二進位驗收。

保留負向紀錄：首輪 HEAD 長度錯誤曾使串流單測失敗；首輪整合在撤銷後持續自我喚醒，`sample` 證明主執行緒忙轉，修正為只撤銷仍待完成的作用域後重跑。原生 App 第一種 launcher 從 bundle 外開啟 Documents 中的 fixture 卡在 `open`；原因僅推定與系統檔案存取協調有關，沒有代替使用者授權 OS 權限。改為內附同一份 18 KiB CC0 fixture 與 provenance 後原生啟動及操作通過。失敗 log／sample 保留，沒有當成 passed 或效能數據。最終 App 曾在原地覆寫 Mach-O 後無法由 CUA 取得狀態且沒有存活程序；以新 inode 原子替換、重簽與核對 SHA 後重新啟動成功。未證明系統中止原因，沒有將其歸因於產品影片流程。

`Native-Video.app` 是使用現有 Homebrew dylib 的 macOS arm64 本機診斷 App，並非完整可攜候選包。先前內附 50 個 dylib 的獨立播放器包仍保留作不同的 runtime 封裝證據，沒有冒充新主視窗已封裝。實際音訊聽感、來源／codec 交付條件、GL context 重建／軟體 fallback、跨平台原生影片、完整編輯器、S7／S8、來源提交序列及回退與效能均繼續。完整 S0–S8 goal 維持 active；沒有 S9 入口切換、推送或發佈。


## S6 原文文件與共用草稿（2026-11-08）

這一階段完成的是原生編輯器的文件／保存核心與舊前端回歸，**尚未完成 Slint 編輯介面、原生 IME 或 Slint 的帶回流程**。既有功能與 S6 全部驗收條件沒有縮減。

### 文件與投影

`composer::document::Document` 持有一份 UTF-8 原文、標題、選取及有界交易歷史，彩色／隱藏色碼／原文三種模式共用同一文件。切換模式不重建文件或歷史；未完成 SGR、字面的 `␛`、真正 ESC 及尾空行保留。原文 ESC 的顯示符號只用於投影，不能回寫成字面字元。命令攜帶草稿 token＋單調 revision；新稿即使也從 revision 0 開始，舊稿的晚到編輯、選取及 undo 仍會拒絕。revision／token 用 checked increment，耗盡不繞回。

可見字格映射採 UTF-8 原文邊界，中文字任一半格命中都展開成完整字形；藏起來的 SGR 以明確的前／後 affinity 處理插入和刪除。矩形依每列欄位轉換為原文範圍，上色共用既有 `composer::color`／`editor_color::local_ranges`，一次交易可 undo／redo，保留未選區樣式和空行。轉成既有 UTF-16 色彩介面時，只用一個游標依序通過排序範圍，不為每列重掃全文前綴。原文模式上色後依原文變更映射選取；測試曾抓到沿用舊畫面欄位會選中新增的 ESC 語法，現已修正。

逐列快取保存同一生產 `Screen` 的 rendition checkpoint 和原文字元映射。修改只重新解碼受影響列，再沿後續樣式延續，直到狀態與舊尾段相同；未改列保留同一個 Arc。一般文字修改一列、換色傳到下一個 reset 為止、增刪行及 undo／redo 皆有獨立預期。字串替換和行索引仍有有界掃描，這不是 O(1) 輸入或 S8 效能通過的宣稱。文件仍限制 128 KiB／4,096 行；貼上先檢查容量再配置，歷史上限為 256 次交易及 1 MiB 字串 payload，達限只移除較舊 undo，不丟目前文件。

### 共用保存 owner 與雙色字修正

將原 `src/ui/drafts.rs` 的單一磁碟工作、合併後最新快照、baseline／衝突副本、關閉補存與既有 wire replies 移到 `drafts::session`。Dioxus 只重匯出相同型別，Slint 可用 `State::with_store` 明確指定隔離 profile；沒有第二套保存邏輯或資料格式。共同測試證明範本作用域、超出可視區的內文、保存／重開、損壞拒絕、關閉最後快照、舊 token 拒絕及原 JSON reply 相容；多程序衝突與不同稿保存沿用原 gate。

先修正共同 `composer::project` 每個字前插入 CR 會取消 SGR 66 單 byte 鎖色的問題。字元投影改用同一 `Screen` 的 source glyph 入口，真正的 LF／Tab 仍取消 latch；terminal byte stream 的 parser／feed 不變。負向單測先重現中文被投成整字藍色，再核對紅／藍左右樣式與控制邊界。

實際 Dioxus host 另抓到圖形 Canvas 被全域 `canvas {position:absolute; inset:0; display:block}` 覆蓋，八個字疊在編輯區左上。已在既有 graphic widget 明確設 static／inline-block／inset:auto，沿用既有格寬與格高，重建受版本控制的 runtime 及 provenance。DOM 測試消費真實產品的全域 Canvas rule，先重現 absolute，再驗修正；DOM double 不宣稱實際幾何。

### 實際舊前端與驗證範圍

本機 macOS arm64 的 `Dioxus-Composer.app` 只跑既有 `--probe-surface ... drafts`／`drafts_restart`、合成 peer 和隔離資料；沒有實站、正式 profile 或密碼庫存取。修正後原始 OS JPEG 顯示八組圖形字各自對齊，十六個半格各 30／30 內部像素符合紅藍交替；中文字「田」左區 39 個紅色、右區 40 個藍色，無反向顏色像素。不是把新 Slint 文件模型的單測當成舊 host 的畫面。

同一程序以真實滑鼠／鍵盤完成匯入範本、貼上、切模式、一般文字上色、標題／正文保存及恢復。另以新程序讀取一份 318-byte 自製草稿副本，在介面按帶回及明確確認後，重新核對合成站端範本並送入；最後停在站端編輯器，未發表。兩輪 `surface-probe.json` 都 passed，worker／listener／關閉回報也完整。含 SGR 66 的重新上色仍依既有模糊色碼政策拒絕且保留原稿；普通上色和手動原文編輯可繼續，未為測試放寬政策。最初為定位 Canvas 問題而保留的 custom-body 程序不符合 drafts 固定完成樣本，原 `status:failed` 留存，沒有改列為 passed。

證據在 `.local/verification/slint-editor/`：核心負向／修正、八組文件測試、共用草稿、Dioxus／Slint 完整 Rust 回歸、全 targets Clippy、382 組 JS、CodeMirror DOM、原始擷取／像素、兩輪完成的 probe 結果、來源 patch／commit 與清理紀錄分列。Rust gate 的 explicit ignored 仍逐項列出，未執行不算 passed；這輪沒有 Slint 原生編輯視窗／IME 或新四架構包，也沒有 S8 時間／CPU／記憶體對照。

提交分開為 `1e3d082`（SGR 66）、`d19a4b5`（文件模型）、`9eddea6`（共用保存 owner）及 `2090530`（圖形 Canvas）；使用者指定的獨立 worktree 持續保留。共用保存提交保留當時 HEAD 的 desktop cfg；尚未提交的 desktop-services／Slint feature 遷移留在工作目錄，完整前端的可編譯提交序列仍需在 S8 交付前完成。下一步是把文件模型接上 79×23 虛擬化 Slint 編輯區、原生 IME／caret／拖曳、標題和同一歷史、保存排程及既有範本／帶回 worker，再逐項跑 S6 原生案例。完整 S0–S8 goal 持續；沒有 S9 切換、推送或發佈。


## S6 原生編輯主視窗初版（2026-11-08）

Slint 主視窗現在掛載同一個 `composer::document::Document`，不是另一個獨立示範或 WebView 編輯器。研究版的站端編輯器及看板列表提供本機草稿入口；先由 Connection 取得完整範本和已確認 scope，才允許新建，列表只列同 scope 的既有稿。`--probe-drafts` 使用原有嚴格 loopback peer，不連 PTT；一般候選使用 `PTTStellaSlintResearch` 的隔離草稿路徑，不讀寫正式產品資料或密碼庫。

### 格線與輸入

`editor::viewport` 只建立 79×23 的可見 Frame，消費 Document 的投影。Tab 展開成帶原樣式的單格空白；橫向捲動保留跨左邊界的完整中文字，由同一 Skia painter 平移後裁切，不丟掉半個字。像素測試以同一來源完整圖的獨立裁切預期，核對整數及非整數尺寸。文字、圖形、SGR 66 和三種模式共用終端 renderer／字型資產，沒有第二套 ANSI parser 或字形 painter。

游標、跨行與整行選取、Alt 拖曳矩形、鍵盤移動、刪除、標題及正文共用 Rust 原文／歷史。全選是完整原文操作，包含外側隱藏 SGR；普通局部文字編輯繼續保留邊界樣式。矩形取代保留各行未選區並為一次 undo，容量在配置拼接字串前檢查。移動／拖曳只更新選取覆層；revision、模式、viewport 和尺寸未變時沿用同一張圖，完整元件驗證 image identity。沒有輸入或待保存時沒有編輯器輪詢 timer。這些是有界配置和更新策略，尚非 S8 CPU／記憶體或快於 Dioxus 的實測結論。

實際 caret 處的原生 TextInput 處理 preedit／commit；未提交文字不進原文或保存。正文的正常字元提交、剪貼簿、特殊鍵與標題的實體快捷鍵接在既有單一 Winit observer；本機編輯先於 Vim／BBS，斷線仍可編輯。原生標題保留自己的文字選取／copy／cut／paste，但 undo／redo 進同一 Document 歷史。編輯 modal 阻擋終端導覽、搜尋、選單、媒體操作及保活。

### 保存與帶回

一個有界命令 channel 和一個 latest-edit watch 把 UI 快照交給原有 `drafts::session::State`，沿用一筆磁碟工作、合併最新保存、baseline、衝突副本及原子替換。一般修改停止兩秒後保存，連續自動保存至少間隔五秒；標題／色票間的內部焦點不觸發 flush，組字不提交。明確保存、失焦／隱藏及關閉補存；離開 UI event loop 後仍等待最後快照和 disk actor 結束。磁碟失敗會傳回錯誤，沒有以清空 handle 當作完成。actor 的最新快照關閉測試不依賴 UI reply consumer；失敗寫入也核對實際 join。

關閉編輯器釋放文件、最後 raster、選取、清單及 retained frame 的 glyphs，保留有界字型 metrics 作重用。帶回只有在已保存、同 scope 的完整範本仍存在時提供，先顯示明確確認；使用原有 Connection replacement，重新讀取／比較站端來源，再核對空白、送一次正文及末位置。沒有新增直接送站端鍵的編輯器路徑，沒有發表操作。

### 分立驗證與負向案例

`.local/verification/slint-editor-native/` 保存 binary 雜湊、完整工作目錄來源 patch、Rust／Clippy／build logs、完整元件與 OS 原始擷取。完整元件 fresh／restart 用同一生產入口和嚴格 peer，核對範本尾空行、原生文字元件提交、三模式原文不變、跨欄拖曳、一般上色／一次 undo、SGR 66 模糊上色拒絕、長文有界可視格線、選取不重畫、關閉釋放及新程序恢復帶回，最後停在站端編輯器；兩程序均收到 Close acknowledged。

macOS arm64 的原生 App 另用真實滑鼠／鍵盤驗證多行 ANSI 貼上、威注音 preedit／Esc 取消、空白選字／Enter 提交、繁中輸入來源下的實體 Cmd 快捷鍵、標題共用歷史、拖曳及上色／undo／redo。取消後保存的原文逐 byte 不變；提交後只增加一個「中」，未多送換行。關窗前仍顯示尚未保存的最後標題，退出後磁碟核對完整標題和原文，連線確認關閉且 App 程序消失。native 與 toolkit 的 hash／範圍分開記錄，沒有用合成 activation 宣稱 OS 焦點。擷取看得到真實 caret 上的 preedit；工具未觀測到獨立候選清單視窗，不把這個缺項寫成已通過。

保留四種負向紀錄：最初 Slint 元件的 global callback／conditional id 被編譯器拒絕；第一輪完整元件抓到全選留下外側色碼；第一次原生流程抓到站端編輯畫面無法由 current_board 推得 scope 而隱藏入口；第二次原生流程抓到標題的 Cmd+A／V 在繁中輸入來源被當文字。分別改用元件函式、完整原文全選、既有 draft_post_scope 及實體鍵轉原生欄位操作後重驗；不改 expected 把失敗算成功。型別與單元素 Range 的 Clippy 負向 log 亦保留。

這是 S6 的第一段原生主視窗整合，**不是 S6 或 S0–S8 結案**。仍需移植與驗證浮動右鍵色票及三種明確選取模式、完整站端框選上色入口、擷取／帶回的可見取消與部分失敗流程、原生組字失焦／隱藏／跨 DPI、rich editor 輔助使用、磁碟失敗／強制終止／多程序的完整前端案例。S7 完整設定／頁尾與原生操作、四架構原生、播放器完整 runtime、回退、可編譯提交序列和 S8 同負載效能繼續。最新 Native-Editor.app 只是合成資料／隔離目錄的本機診斷，保留其他不同功能的必要 App；沒有整合 canonical checkout、S9 切換、推送或發佈。

## S6 色票、明確選取與可見取消（2026-11-08）

原生本機編輯器新增文字／整行／矩形選取入口，矩形不再需要按住 Alt；Alt 拖曳仍保留。單擊只定位，超過三個邏輯像素才開始拖曳，雙欄中文命中仍展開整字。整行操作保留完整原始 SGR 與換行，行模式的 Backspace 在沒有選取時仍只刪一個字形。這些行為由同一個 Viewport 管理，沒有新增 ANSI parser。

浮動色票是覆蓋層，不占格線空間或重畫文章；提供 16 前景／8 背景色、原生按鈕語意與鍵盤方向、Tab、Home／End、Enter／Space、Esc。正文選取後右鍵、色票按鈕和 Shift+F10／ContextMenu 都使用同一份租約。租約核對草稿 token、revision、顯示模式和實際原文範圍；即使選取變動沒有增加 revision，舊色票也不能修改新選區。組字、切模式、捲動、失焦和關閉會撤銷租約。色票外的滑鼠／滾輪事件只關閉浮層，避免同一次操作穿透正文。

擷取與帶回共用一個 source 工作擁有者，介面在工作中提供「取消工作」，請求後顯示停止中。取消以 oneshot 撤銷等待中的 Connection source 請求，既有 worker 繼續擁有站端操作與取消；沒有新增直接送鍵路徑。通知明確保留本機稿，提醒站端可能已有部分修改，不能宣稱 bytes 回滾或自動重送。原有 `drafts` 合成 peer 另有明確選用的 `drafts_slow`：只把編輯器回應延遲 350ms，供真實滑鼠點取消，不改一般 fixture 或產品時序。新增入口以 `b011edd` 分開提交；合成長文既有的 500ms＋150ms 逐頁延遲依使用者指示保留，不列作效能缺陷。

### 驗證與負向證據

證據在 `.local/verification/slint-editor-controls/`。release library 為 341 passed／4 ignored，Slint 為 68 passed／1 ignored；all-targets Clippy、Slint＋native-video 編譯及 core 的 14 個命令／兩組 replay 通過。完整元件使用最新重建二進位分別跑 fresh、restart、queued capture cancel、已進入站端步驟的 capture cancel 和 queued transfer cancel，五輪均通過且收到 Close acknowledged。取消案例核對 source task／cancel handle 退場、本機原文未變、晚到結果不能開新稿、穩定後沒有追加站端回應；queued transfer 另核對零新增回應。這些不能代替清空後取消、部分送出或 transport error 驗收。

macOS arm64 真實滑鼠／鍵盤另核對整行點擊／拖曳、色票鍵盤／Esc／外點關閉、無 Alt 的矩形上色及 Cmd+Z／redo。原始 OS JPEG 的獨立解碼核對 `A中B`／`C文D`：外側四個 ASCII 只有白色字、沒有綠／藍像素，兩個中文字分別有綠色字與藍底；保存原文只在矩形所選中文字插入色碼，一次 undo 還原原文。60 行正文捲離末行後開始威注音組字，畫面先顯示真正 caret 再放 preedit；取消後保存逐 byte 不變，重新選字提交後原文只增加一個「中」、沒有新增換行，標題保存亦核對。

原生操作先抓到上色後焦點未回正文，導致 Cmd+Z 沒生效，不能把後續錯誤底色誤判為矩形 producer 失敗。新增完整元件的 body-focus assertion 先失敗，再修正成功上色後明確回焦並重新驗證。編譯器的屬性名稱衝突、Clippy 的 Range 陣列診斷、較早取消未點中工作時段、焦點失敗的原始畫面及原文均留作負向紀錄。最後一個原生手動程序超過合成 peer 的 900 秒上限，資料保存成功但關閉為 timeout，沒有列為 Close passed；同版先前原生取消程序及新版五輪完整元件的關閉證據分開記錄。重建後的 toolkit 與原生 App 雜湊不同，各自標記實際測試範圍，不把較早 App 操作冒充新二進位驗收。

### 尚未完成

矩形操作的原文已能一次 undo／redo；多重選區幾何尚未跟著歷史還原，不能宣稱完整選取歷史通過。完整站端框選上色入口、部分清空／送出失敗、原生組字失焦／隱藏／跨 DPI、候選清單視窗、rich editor 輔助使用、前端磁碟失敗／強制終止／多程序及鍵盤按住跨浮層的邊界繼續。S7 完整設定／原生操作、四架構原生、完整影片 runtime／音訊、資料回退、來源可編譯提交序列和 S8 同負載效能仍待完成。完整 S0–S8 goal active，沒有 S9 切換、canonical checkout 整合、推送或發佈。

## S6 選區與正文一起還原（2026-11-08）

先用獨立預期重現矩形取代後 undo 的缺陷：`A中B`／`C文D` 的兩個中文字應仍是分立選區，實際卻變成包含換行與中間未選字元的一段連續範圍。下一次貼上會刪到未選文字。修正將文字、全選、整行及帶原投影模式的矩形形狀放進同一個 Document 交易；Viewport 只保留視窗、拖曳及後續選取模式，不再另存矩形或全選狀態。矩形取代也由 Document 執行，沒有第二個 undo stack。

歷史每次保存固定大小的選區狀態，沿用既有 256 次交易與字串預算；不保存各列範圍副本。undo／redo 在相同原文快取上重建選區，切換模式後還原仍以建立矩形時的投影解出原文範圍。標題交易保留選區，整行／全選上色保留外側原始 SGR 與換行邊界；原文模式上色改變色碼欄位時，保留映射後文字選區，undo 可還原原矩形。矩形模式單擊可正常輸入，零寬跨列矩形可一次插入；有界容量仍在拼接前檢查。共用核心與測試以 `93657ea` 提交，前端移植的可編譯提交序列仍待完成。

`.local/verification/slint-editor-history/` 保存最初失敗、修正、來源 patch 與驗證。release library 345 passed／4 ignored、Slint 70 passed／1 ignored、all-targets Clippy、Slint＋native-video 編譯、core、docs 與 diff 檢查通過。五組完整元件重跑 fresh、restart 和三種取消；fresh 額外驗證矩形上色 undo／redo 後，由真正掛載的 TextInput 接受新字，只替換原先選中的兩個中文字，均收到 Close acknowledged。

macOS arm64 的新 Native-Editor-History.app 另用實際拖曳、貼上、Cmd+Z／Shift+Cmd+Z、Shift+F10 色票和保存核對：取代 X、undo、redo、undo 後改 Y，以及上色 undo 後改 Z；外側 `A`／`B`／`C`／`D` 與原換行均逐 byte 保留。原始擷取顯示兩列獨立 highlight；關窗收到 Close acknowledged 且 App 程序消失。工具事件與 OS 操作分開記錄；App 內 Mach-O 經 ad-hoc 簽名後的 SHA 與 Cargo 原始檔不同，兩份雜湊及簽名前來源明確分列，不要求簽名後仍逐 byte 相同。

收尾檢查另補上完全位於短行／空行右側的空矩形保護，避免零字形選取退回一般文字範圍而刪除換行，以 `2c201ec` 分開提交。最新 library 為 346 passed／4 ignored，Slint 為 70 passed／1 ignored，Clippy 與五組完整元件重新通過。更新的原生 App 另實測空框貼上後 `A\nB` 逐 byte 不變、正常矩形 undo 後改 Y 仍保留外側文字；關閉確認與程序退場通過。兩輪二進位和實際操作範圍分開記錄。

這一段完成已知的選區歷史缺陷，**不是 S6 或候選版結案**。完整站端上色入口、部分清空／送出錯誤、原生組字失焦／隱藏／跨 DPI、候選清單與 rich editor 輔助使用、前端磁碟／強制終止／多程序、S7 設定及原生操作、S8 四架構／影片 runtime／音訊／資料回退／效能與來源提交序列繼續。使用者要求保留的合成長文延遲不變；完整 S0–S8 goal 維持 active，沒有切換 Dioxus、整合 canonical checkout、推送或發佈。

## S6 原生站端框選上色（2026-11-08）

站端編輯器的選取情境選單現在提供「文字上色」。原生介面共用本機草稿的 16／8 色票與同一份 Fluent 標籤，提供預覽、數字色碼、還原預設及明確套用；點色票只改本機預覽，不直接送站端命令。方向鍵選色、Tab 前往動作、Esc／外點取消，動作區顯示鍵盤焦點。內容可捲動以保留小視窗操作空間；任意 DPI／小視窗原生驗收仍待完成。

新的擁有者只租用 Output 已呈現的 stamp、policy、geometry 和選區。從首尾完整 glyph 重建既有 Selection 後，必須重新 extract 且 rows／spans 完全相同；不能補入選取中缺少的列或切半中文。套用前再次核對來源、連線、輸入 epoch、滑鼠路由與唯讀狀態。選區／畫面變更會撤銷預覽；本機撰寫、尋找、媒體、輸入法組字等介面不允許開啟。上色期間提供可見取消，失焦或切 session 也會要求停止。只有既有 `Connection::request_editor_color`／Flow 擁有站端 bytes、逐步來源核對、取消與部分失敗；原有 79 格來源及 WRAPMARGIN 等限制未放寬，沒有 UI 直接送鍵或另一套色碼 parser。

色票套用／關閉後，Winit observer 保留 Enter、Space、Esc 等啟用鍵的邊緣，避免尚未放開的同一按鍵重複進入 BBS；仍使用原有單一輸入 observer。這是固定大小位元狀態，沒有新輪詢 timer。本機草稿的色票也共用此保護。所有文字與欄位顯示仍由 Slint，背景 Source 由相同 Rust renderer 繪圖；候選版效能仍須 S8 同負載實測，不能從實作方式直接宣稱收益。

### 合成與實際操作分立

`.local/verification/slint-station-color/` 記錄 release library 346 passed／4 ignored、Slint 72 passed／1 ignored、all-targets Clippy、core、Slint＋native-video 編譯、16 組 Fluent／媒體語言回歸及文件檢查。四組完整元件使用嚴格 peer：正常上色、已進入站端步驟後取消、不支援的 SGR 66、以及插入第一筆完整重設碼後撤銷確認。每組都先驗證預覽／取消零站端輸入與選區變更拒絕，最後核對工作退場及 Close。共用色票的直接消費者另跑五組本機草稿 fresh／restart／取消回歸，均通過並收到 Close acknowledged。

部分寫入案例不是把錯誤訊息當作證明：peer 要求第一、二行完全保留，只有第三行末尾多出一個 `ESC[m`；之後不准收到任何命令。確認被撤銷時，既有 Flow 等待有界 step deadline 後停止，提示已送色碼可能保留、不會重送；沒有宣稱回滾或模式已恢復。取消案例的原文字須完整且確實有進入工作，不支援案例另核對原顯示／插入模式。正常完成則核對完整原始色碼、選區內外文字及原模式。新增負向 peer 以 `8f0d7ad` 分開提交，原正常 peer 保留。

macOS arm64 的 Native-Station-Color.app 用實際滑鼠拖曳三行、右鍵、色票預覽／預設／Esc、方向鍵選色及 Tab 套用；長按 Enter 八秒後回到終端焦點，peer 仍只接受一次預期色碼流程、沒有多餘換行。這個 OS 操作沒有另外量測硬體 repeat event 次數；重複邊緣本身由相同生產 reducer 的獨立測試驗證。另兩個新程序實際點工作中的取消，以及等候部分寫入停止提示；三輪均正常 Close 且程序消失。原始 OS JPEG 以獨立 decoder 核對：未選 P 有 52 個白色字像素，選中「中」「文」「Q」「第三行」分別有 97／99／61／311 個紅色像素，各區零其他分類顏色。這些是本機軟體 renderer 證據，不代替其他架構或實站。

負向紀錄保留：最初 Slint 的屬性／重複 callback 被編譯器拒絕；新探針第一次漏掛 native input，使拖曳無法形成選區；部分錯誤 fixture 最初在裸 ESC 及錯誤插入順序注入，兩次都被嚴格 Close 檢查拒絕。最後改成獨立的完整 `ESC[m` 與第三行期望，沒有改生產 Flow、跳過 Close 或把失敗改列通過。

這一段完成站端上色的原生入口與上述範圍，**不是 S6／S0–S8 結案**。本機草稿的清空／送出部分錯誤、關窗中的工作、磁碟失敗／強制終止／多程序、原生組字失焦／隱藏／跨 DPI、輔助使用及背景工具列的完整 modal 語意仍待驗證或補齊。S7 全部設定／原生操作、四架構原生、影片 runtime／音訊、資料回退、來源可編譯提交序列及 S8 同負載效能繼續。Native-Station-Color.app 與前一個本機草稿 App 是不同必要 fixture，均保留；無 S9 切換、canonical checkout 整合、推送或發佈。

## S6 草稿部分失敗、斷線與工作中關窗（2026-11-08）

既有正常／延遲草稿站台移至同一個獨立合成 peer，新增七個明確故障邊界：再次讀取時範本被改動、只完成新標題、只刪除原第一行、只收到新正文第一行、收到新正文第一行後直接中斷 TCP，以及讀取／帶回前重讀範本期間關窗。peer 在故障後拒絕任何新的 Binary 命令；關窗與範本變動案例禁止取代寫入。每一例都要求確實進入指定步驟，並在 Close 或刻意斷線前逐 byte 核對獨立的標題／正文期望，沿用拒絕發表命令的站端模型。輸入封包、累積 bytes 和站端等待都有界。共用測試以 `a63f5ff` 提交，未改動生產 SourceFlow 或縮短其 deadline。

Slint 完整元件在同一主視窗掛載編輯器、保存 actor、Connection 和生產 Flow。前三種部分執行和正文部分執行的確認被撤銷後，Flow 等待既有 step deadline 停止；提示可能已有內容送達、不會重送，UI 保留本機文件及已保存原文。範本變動則明確拒絕取代。TCP 案例故意沒有 WebSocket Close handshake，預期結果明列 `no_connection`，沒有放寬其他案例的 `acknowledged` 檢查；斷線後仍能修改和保存同一本機稿。這些是選定命令前綴的故障注入，沒有宣稱任意網路切分都已覆蓋，也沒有宣稱站端內容或顯示模式已回滾。

工作中退出的元件測試先記 `exit_requested`，只有正常 main cleanup 回來後才另寫通過紀錄：UI source／reply／update 工作已撤銷，保存 actor 實際 join，帶回案例的本機稿仍完整，未完成的初次讀取則沒有產生空草稿。這不把 renderer 文件配置視為已提前釋放，也不把合成 event-loop exit 當成視窗管理器操作。五種既有 fresh／restart／取消草稿案例另重跑，皆收到 Close acknowledged。

macOS arm64 的 Native-Draft-Failures.app 另外以原生按鍵和 AX 操作驗證：Ctrl+P 進入合成站端編輯器後，讀取範本中點系統關窗；第二個程序選取自製已保存草稿、明確確認帶回，在重讀範本中關窗。兩個程序的 peer 核對站端原文未被取代，均收到 Close acknowledged 且程序消失。第三個程序故意在正文第一行後中斷 TCP，實際畫面顯示本機稿及停止警示，原生 Cmd+A／ANSI 多行貼上追加「離線保留」後按保存，關窗再由磁碟逐 byte 核對；其 `no_connection` 是刻意注入的結果。自製草稿種子和每次新建的暫存 profile 隔離，沒有存取 production store／vault 或連 PTT。

`.local/verification/slint-draft-failures/` 保存七組完整元件、五組回歸、三輪原生 AX／原始 JPEG／peer log、來源 patch、雜湊及清理紀錄。release library 346 passed／4 ignored、Slint 72 passed／1 ignored、all-targets Clippy、Slint＋native-video 編譯及 core 通過；這一輪沒有重新驗證實際 Dioxus host、影片 runtime 或音訊。簽名前 Cargo 輸入和 ad-hoc 簽名後 payload 的 SHA 分開記錄。

這一段完成上述草稿失敗／關窗範圍，**不是 S6 或候選版結案**。前端磁碟失敗／強制終止／多程序、原生 IME 候選／失焦／隱藏／跨 DPI、完整 modal 輔助使用及混合 Live 工作退場仍待驗證。S7 設定／原生操作、四架構原生包、影片 runtime／音訊、資料往返與回退、來源可編譯提交序列和 S8 同負載效能繼續；合成長文刻意延遲保留。完整 S0–S8 goal active，無 S9 切換、canonical checkout 整合、推送或發佈。

## S6 草稿儲存故障、程序終止與多開（2026-11-08）

新的完整主視窗驗證使用原有 Document、Slint 編輯器、保存 actor 和 kernel-lock／baseline 交易，沒有生產磁碟 mock。備份路徑被自製空目錄阻擋，以及主資料被刻意改成無效 JSON 時，編輯器的「關閉」會嘗試 flush；失敗後仍掛載原文、取消關閉、顯示錯誤且不允許帶回站端，舊檔逐 byte 保留。移除診斷障礙或恢復原始檔後，再次編輯並明確保存，最新原文與原始備份均正確，沒有憑空建立衝突副本。這裡驗的是備份替換失敗與資料驗證失敗，不代表 ENOSPC 或所有 I/O 錯誤。

強制終止案例先等到 UI 確認保存並核對磁碟，接著製造尚在 debounce 的修改，寫出包含自身 PID 的 ready 紀錄並維持事件迴圈。父程序核對實際程序命令與自己的輸出目錄後，對該 PID 送 SIGKILL、等待終止，確認原檔 bytes 不變，再啟動新 Slint 程序。恢復後是最後確認保存的版本，仍能繼續修改和保存；不宣稱尚未落盤的輸入會恢復，也不等同任意 atomic persist 中途或實體斷電的覆蓋。

兩程序測試先讓兩個獨立 Slint 主視窗讀取相同自製 baseline，再由有界診斷 barrier 放行。同稿案例固定先保存 A，再讓持有舊 baseline 的 B 保存；B 顯示「已保存為衝突副本」，後續修改沿用同一副本，不另增第三份。不同稿案例放行兩個不同 ID，保留雙方原文、revision 與非衝突旗標。父程序獨立解析最終 JSON，核對完整中文／ANSI、ID、份數及 revision。這是實際程序／保存 owner 的驗證，輸入與前景仍是 toolkit 診斷事件，不冒充兩個 OS 視窗的人工作業或其他平台 kernel lock。

共同站台 fixture 新增只允許讀取範本的入口：必須進入捕捉步驟、禁止取代標題或清空正文，Close 時要求原站端標題／正文完全相同。以 `d0a3f67` 提交。完整主視窗八個程序情境（其中一個預期被強制終止）和兩份最終多開檔案期望通過，另重跑五組正常／重啟／取消草稿回歸。release library 346 passed／4 ignored、Slint 72 passed／1 ignored、multi_instance 10 passed／1 ignored、all-targets Clippy、Slint＋native-video 編譯、core、docs 及 diff 分列於 `.local/verification/slint-draft-storage/`；沒有重新驗證 Dioxus host 或影片 runtime／音訊。

macOS arm64 另用自製已保存草稿，在原生視窗貼上 ANSI 多行、點編輯器關閉，觀察真實備份寫入錯誤及保留正文；修復只限該 profile 的空目錄障礙，再按保存。**第一輪的原生結果未通過完整原文核對**：最初位置為 4:13，稍後成為 4:11，保存檔少了末字「敗」。該輪沒有事件紀錄，無法歸因於保存、IME、工具、使用者或其他輸入，不能因第二輪通過就宣稱已修好。第一個 App、AX、原始 JPEG、雜湊及負向紀錄保留。

第二輪加入只有明確 `--probe-drafts`、隔離 profile、指定儲存案例和 trace 開關同時成立才啟用的有界事件紀錄，每個 owner 最多 192 筆，只記 named key／IME 長度／動作與文件 bytes 長度，不記文字、標題或帳密。一般／Live 不啟用，未啟用時不建立 JSON 細節，也沒有新 timer 或輪詢。該輪相同步驟的原文／末字與備份逐 byte 相符，只有一次文件修改、沒有刪除鍵事件，Close acknowledged 且程序退出。它證明第二輪的儲存失敗／重試範圍，**沒有證明第一輪異常的原因**；trace 也不區分人、工具或硬體產生的 OS 事件。

這些是 S6 的儲存／程序邊界證據，**不是 S6 或候選版結案**。末字異常的可追蹤重現、原生 IME 候選／失焦／隱藏／跨 DPI、modal 輔助使用及混合 Live 退場仍待驗證；S7 完整設定／原生操作、四架構原生包、影片 runtime／音訊、資料回退、可編譯來源提交序列與 S8 同負載效能繼續。完整 S0–S8 goal active，無 S9 切換、canonical checkout 整合、推送或發佈。

## S7 共用偏好狀態與啟動 Store 交接（2026-11-08）

`settings::preferences::Preferences` 現在共同管理已生效 AppSettings、解析後 DisplaySettings、目前編輯器的提交 baseline 及載入警告。Dioxus adapter 留下 packet／預覽 sample，Slint 媒體消費同一 Rust 型別；只有既有檔案 owner 的成功結果才 commit，不增加背景 worker、timer 或另一套磁碟交易。開啟編輯器重設其 baseline，保存回覆仍以使用者實際提交的值更新編輯 baseline，不能拿合併了外部修改的磁碟結果冒充 UI 已看見的值。外部語言修改、連續字高修改及回復原值的測試核對外部欄位不被寫回舊值；失敗載入／備份寫入失敗保留原檔、警告及有效值。

Slint 啟動接線另修正 Store 觀測遺失：先前載入研究設定後，Media 再建立另一個未讀取的 Store，無法證明損壞檔仍是啟動時觀察的原檔，因此明確恢復會被共同交易拒絕。現在啟動時取得 Preferences，沿 Live 路徑直接移交同一狀態和 Store；媒體設定不在 UI 掛載時重讀。研究 profile 的回歸核對原始損壞 bytes 保留、明確保存後備份原文。這是同一生產 startup factory 的 Rust 測試與接線證據，沒有連 PTT 或宣稱實站恢復已驗。合成媒體 probe 保持自製設定和明確隔離 Store。

共用型別與 Dioxus adapter 以 `19afa3f` 分開提交；Slint 消費者仍隨完整前端來源 patch 保存。這段只接上既有 hover／自動展圖的生效處理，**不是 Slint 完整設定中心**；所有分類、搜尋、字型清單／預覽／可攜檔、語言及其他有效設定的全前端通知仍須繼續接線。圖片／PNG 的共用 I/O owner 及既有字型 renderer 不變。

`.local/verification/slint-settings-state/` 記錄目前 release library 348 passed／4 ignored、Slint 72 passed／1 ignored、Dioxus 132 passed／1 ignored、兩版 multi_instance 各 10 passed／1 ignored、兩版 all-targets Clippy、382 組 JS 零 skipped、core、Slint＋native-video 編譯、docs 和 diff。Slint hover、automatic 及 inline 三組完整元件另核對 peer／HTTP／Close、保存前不生效、同欄衝突、無關欄位合併及失敗重試；輸入和前景是 toolkit 事件，不冒充 OS 操作。

macOS arm64 實際 Dioxus App 另外驗字型列舉三次、字高 18 及字寬 110 的連續保存；自製備份路徑障礙使字高 19 保存失敗，原檔仍是 18／110，介面保留 19 草稿和重試入口。只修復該合成 profile 的障礙，再按重試、關閉／重開設定，仍為 19／110；明確 PNG 匯出及正常 Close／全部 cleanup／程序退場通過。字型來源動作由 fonts probe sink 消費，沒有開瀏覽器或下載字型。`surface-probe.json` 的一般 `settings_saved` 旗標只認字高 18／基線 19 的另一個固定 workload，本例不是該組值；本輪保存證據來自 AX、原檔／備份及重開值，不把該旗標改成 true。該 fonts case 自己要求的 catalog／source／PNG／Close 條件均通過。

負向紀錄保留：Dioxus 第一次編譯抓到移出欄位後仍讀 `self.display` 的預覽呼叫，已改用共同狀態；第一次 OS 操作用重新編號前的索引誤關設定，再把測試文字送至本機 peer，被嚴格拒絕，不列為產品通過。錯誤視窗確實提示重新開啟，第二次關閉後程序退出，再用全新 profile 完成上述正向／故障／重試驗證。

### 本輪 IME 嘗試的限制

同一個已保留的草稿 App 做了原生按鍵嘗試，但本輪只收到直接英數或注音符號，沒有 IME preedit。CUA 不支援單獨 Shift 按鍵，SystemUIServer／輸入來源 agent／唯音的 AX 讀取逾時，無法把這組操作列為組字、候選或失焦取消通過。原始事件、恢復後合成原稿、正常 Close 和精確範圍保存在 `.local/verification/slint-editor-ime-edges/`；兩次 Caps Lock／Ctrl+Space 也不能單憑次數證明最初與最後的 active input-source 狀態相同。此嘗試未直接修改系統偏好檔。使用者後續確認當時有其他操作，因此有輸入／焦點干擾的可能，但第一輪末字異常沒有事件紀錄，仍無法確認原因，沒有宣稱修好或要求其他工作停用。

完整 S0–S8 goal 維持 active。S6 末字／IME／可及性及混合 Live 邊界、S7 設定與系統整合、四架構原生包、影片 runtime／音訊、資料回退、完整前端可編譯提交序列與 S8 同負載效能繼續；未切換 Dioxus、整合 canonical checkout、推送或發佈。


## S7 原生設定視窗與保存交接（2026-11-08）

狀態：**設定視窗初版及指定原生往返通過，S7／整體平替尚未完成**。同一共用偏好型別由 Slint Preferences owner 持有，媒體快捷開關與原生欄位交給同一 Output 檔案工作；沒有第二份 JSON writer。提供一般、外觀與字型、鍵盤與滑鼠、圖片與媒體、帳號與隱私、關於與授權分類。已接入文章模式／首尾、字型組合與自訂家族／字高／基線／字寬、浮球三項偏好、hover／自動開圖、分類重設、固定中英文搜尋、原生字型清單及官方來源、顯示 profile 選檔。帳號按鈕明確交回既有登入 owner；站點與名單管理尚未移植，不用空白分類宣稱完成。

有效欄位在300ms後保存，只保留一筆寫入和最新待存修改。新的編輯會重設 deadline；Output 完成事件喚醒待存工作，不在閒置時輪詢。native preedit 與無效中間值不保存；失敗保留草稿與重試，Store 成功後才改有效值。關閉設定等待待存工作；App event loop 結束後 join 同一 worker、取回已完成但未回呼的結果，再保存最後有效修改，關閉子視窗並停止 timer。合成關閉案例有一筆 in-flight 21 與尚未到期22，結案從實際 Store 與有效值核對22，並確認 queue／worker／timer／子視窗皆已清空。

繪圖設定更新接進主畫面、文章 session、文章 row／footer cache、編輯器與 PNG 使用的 DisplaySettings；字形快取重建而保留原投影與格線身分。只改數值沿用已解碼 face；文章與編輯器 fork 共用 Skia reference-counted assets。獨立 renderer 回歸核對變更前後像素不同、同設定 fresh renderer 與兩種舊 fork 的輸出一致。**這些程式路徑／單元結果尚不能代替 Live 同窗文章、編輯器與 PNG 的原生驗收。** Fluent 選擇與設定視窗文字已接入；主視窗所有標籤的即時重翻譯、完整搜尋索引／非文字欄位聚焦及全部系統入口仍須完成。

本輪證據在本機 `.local/verification/slint-settings-window/`。最終 CLI SHA-256 `5f7203e8425e2e096624bb5c3096dce40dc76b722f43e63827e48847677e31c1`，macOS arm64 ad-hoc App payload `05a63cf77ffb730d95df35b1a90226ffb98ca67a8369d7b17fcd20938689654d`；簽署前後分列，不能互換。`accepted-settings`／`accepted-shutdown`／`accepted-automatic`／`accepted-inline` 四個完整元件程序通過嚴格 BBS／HTTP／Close，涵蓋 debounce、忙碌保留最新修改、備份故障／修復／重試、無效值與合成 composition、重開、媒體既有回歸及關閉交接；組字與前景是合成條件，沒有冒充 OS IME。

最終實際 macOS CUA 以原生⌘,開啟設定，⌘A／貼上保存字高18、字寬110；原生另存的 profile 由獨立 JSON 檢查確認 `bundled` 與18／110。先保存20，再從原生 picker 匯入該檔，欄位恢復18／110；關閉重開維持相同值，最後紅色關窗取得嚴格 Close acknowledged、HTTP workers joined 與程序消失。搜尋基線取得欄位焦點、取消另存後再保存／再次開 picker、325個 OS 字型家族，另有第三／第二包實測，hash 分列於個別 metadata，不套稱最終包全部實測。

保留的負向案例與修正：第一包⌘A／⌘V成為一般字母，無效23av未寫入；設定子視窗改共用既有 Rust text-edit shortcut 判斷並派送給實際聚焦欄位。第二包取消 picker 未釋放 busy，補上所有返回前的共用 end_picker；其後另一次未先觀察 picker 的操作把G送進數值欄位，屬操作負向，不當產品通過證據。第三包匯出字型意圖與表單不同，原因是合成記憶體 seed 為 bundled 而隔離檔案缺席，Store 正常欄位合併保留了磁碟預設；現在合成檔案與模型先使用同一 seed，新增只改數值保留字型意圖檢查。Live 啟動維持載入原 Store 的交接，沒有改成寫入測試 seed。第二包⌘Q退出沒有正常 Close audit，**OS Quit 清理仍未通過**；不能以程序消失代替所有 worker 已 join。

最終 Slint release：共用 library348 passed／4 ignored、bin76／1、multi_instance10／1；all-target Clippy與 slint-research＋native-video all-target 編譯通過。Dioxus library348／4、bin132／1、multi_instance10／1及 all-target Clippy通過；382組 JS、核心14 commands／2 replays亦通過。combined feature 曾因新增 probe 參數未同步 dispatch 而編譯失敗，已修正並重跑通過，原失敗 log 保留。

下一步仍包括站點／名單管理、完整語言與工具入口、OS Quit／modal／IME／可及性、啟動文章首尾偏好及 Live 各 consumer 同步驗收、全部 profile 負向與跨版本資料往返、四架構打包和同負載效能。原型與 desktop-services 的完整來源仍待可編譯的提交序列；本輪保留完整 source patch 與從5ea9ad6重放的樹，不當成已提交／整合。沒有推送、切換正式入口或發佈。


## S7 原生 App 選單與正常退出（2026-11-08）

狀態：**macOS App 選單退出、設定待存修改與草稿貼入後退出的指定案例通過；OS 所有退出入口尚未完成**。原先系統預設⌘Q會直接結束，未進 Rust teardown；現在由 macOS 專用 App menu 的 action 呼叫主視窗退出 callback，再走既有保存、worker join 與 Connection Close。原生⌘,、關於、服務、隱藏及顯示選項一併保留。主視窗／設定視窗實際取得焦點後，只排一筆零延遲安裝，配合 Slint 的每窗預設選單；沒有閒置輪詢。選單擁有 target 與 items，結束先清 selector／weak target 並停 timer，再釋放物件；不替換 Winit application delegate，也不取代 Slint 的原生文字右鍵選單 handler。

此段使用已鎖定版本 objc2／AppKit／Foundation；新增 unsafe 僅限 Objective-C NSObject subclass／init 與 Cocoa action ABI，逐處記錄主執行緒、sender 型別、物件壽命及移除次序。曾考慮的 muda 全域事件 handler 會與原生文字選單衝突，實測前已移除，沒有保留第二個全域 handler 或 vendor fork。實際設定欄位的 Cocoa 右鍵「全選」仍正常。Dock Quit、系統終止及所有 picker／modal 退出尚未驗收，不能把自訂 App menu 的通過外推至這些入口。

啟動文章偏好另修正為將整份 AppSettings 交給 ArticleSession constructor；先設定閱讀模式與首尾導航，再允許第一份投影。完整元件探針在任何 runtime 編輯前，核對第一個 displayed paint 已含 article_boundary_navigation=true。原生明確「開啟本機撰寫」會要求 host 視窗取得焦點，避免只給 widget 焦點而原生按鍵仍送往其他 App；這只在使用者開啟動作執行，正常啟動仍不搶焦點。

證據在 `.local/verification/slint-native-lifecycle/`。選單與設定兩個 macOS arm64 App 的 Cargo 輸入 SHA-256 為 `94691afb167db473af43099a1b5dbb076ce5d19af272ba0f6d09ef767773526b`，ad-hoc payload 為 `e731a16d5238c682b5ca873914d591e3edff99858a40ab2d103c0f468a0974ee`。第一個實際點 App menu 退出；第二個開設定、原生右鍵全選字高、輸入17後立即⌘Q。兩者均獨立核對正常 Close acknowledged、HTTP workers joined、程序消失；設定檔與有效值為17，pending／in-flight／timer／子視窗清空，menu target／action 也有退場 audit。沒有在輸入與⌘Q之間加入觀測，故不把操作前的 pending 狀態冒充已量測；另有合成 shutdown fixture 專門驗 in-flight／pending 同時存在時的排空。

加入編輯器 host 聚焦後，最新 CLI SHA-256 為 `d21e48953ec6bca997b54c7ee412ac5de1b1efbbfecd7946e61849cc51ad9d65`，Native-Editor-Quit.app ad-hoc payload 為 `85a30db2f7475dc334e989a67d0d123aa4a987bffa2b3e3b9e1a3a4a5bf12b74`。實際Ctrl+P進入嚴格合成範本、擷取完整來源、選自製已存草稿，貼入一個Q後立即⌘Q；獨立磁碟比對確認只有最前多出一個Q，原ANSI／中文與標題完整，body SHA-256為 `3d3c9b76d92941dedd759579363decbd50ee30f431030f769a19fdffe244e8c2`。LocalOnly peer禁止替換站端原文，核對原標題／正文並收到Close，程序消失。這不代替原生組字候選、舊末字異常原因或強制終止時未保存輸入的驗證。最終四組完整元件（啟動首尾、設定回歸、設定關閉交接、編輯回歸）均取得正常Close，未更改長文刻意延遲。

最終Slint release回歸為library348 passed／4 ignored、bin76／1、multi_instance10／1；all-target Clippy及Slint＋native-video all-target編譯通過。Dioxus同輪library348／4、bin132／1、multi_instance10／1與all-target Clippy通過；最後僅修改Slint的編輯器聚焦，未因此重跑不受影響的Dioxus。382組JavaScript零skip，文件40篇／233個本機連結／21個gate回歸與diff檢查通過。已確認canonical develop仍僅領先origin一筆及原有未追蹤計畫，未修改canonical來源。

### 保留的讀取停滯與操作負向

第一個 Native-Quit-drafts.app 使用位於Documents worktree內的自製草稿，讀取一直未返回。sample明確顯示storage worker停在File::open／kernel __open；當時主UI仍在事件迴圈。⌘Q已進Rust Editor.finish／Storage.finish，主執行緒等待該工作，故此案例沒有Close、未通過。不能由路徑或沒有tccd log就斷言是TCC；系統權限原因仍未確認。原程序、App、資料及兩份sample保留，不因讀取逾時而重開、強制結束或改寫其檔案。另用明確隔離的暫存目錄繼續獨立驗證，不是把原失敗改列通過。正常產品草稿位於Library/Application Support，仍需處理OS I/O等待時的可見關閉狀態與其他原生入口。

第一個暫存目錄操作直接從板面開已存草稿，漏了LocalOnly peer要求的Ctrl+P與來源擷取；退出被嚴格peer拒絕，記為操作負向，沒有放寬peer。新程序照完整流程才取得上述通過。使用者確認更早的末字異常期間有其他操作，這只增加輸入干擾的可能性，不能證明原因或已修復；舊負向App與文字不入log的有界trace繼續保留。

完整S0–S8 goal維持active。完整語言／管理／搜尋入口、Dock／modal／OS等待退出、Live各consumer同步、IME／可及性、四架構完整包、影片runtime與音訊、舊資料往返／回退、前端可編譯提交序列及同負載效能仍須完成。沒有切換Dioxus入口、整合canonical checkout、推送或發佈。


## S7 介面語言同步（2026-11-08）

狀態：**目前主視窗與編輯器文案已接入語言同步，指定完整元件及macOS原生往返通過；完整S7尚未完成。** 新增同一個UiLabels／Rust入口，啟動與成功保存語言後更新固定文字。移除Slint編輯器、圖片箭頭、輸出選項與終端可及性標籤中的固定中文；原有登入、媒體、尋找、貼上與兩種色票的一次性設定改由此入口負責。浮球與右鍵保留原owner／選項順序，更新文字而不重新建立操作來源。離線CLI探針的開發文字不列為產品翻譯。

同一套預先載入Fluent catalog增加只供呈現的指定key轉譯：僅更換owner列出的標準狀態，保留任意診斷、參數、OS／第三方與站端原文，不以翻譯結果做輸入准入。共用helper及獨立往返／未知訊息測試以`4578b4f`分開提交。Connection固定狀態可跨舊watch值重譯；以單一目前訊息／語言快取避免每個bytes/frame更新重複format兩份catalog，沒有新增timer。搜尋的本地結果數即使版本未變也會換文字，查詢與pending/preedit不被提交或撤銷。文章載入提示只重算標籤，不執行取頁或改閱讀錨點；編輯器只更新游標位置文字，不改文件、歷史或bitmap。

證據在`.local/verification/slint-language/`。目前CLI SHA-256為`2ab532579981bf2c47bd7b554cc07bc744c2c81a74db42dd15c3da232cc09477`；macOS arm64 Native-Language.app ad-hoc payload為`37a9af53e9ebb931a18e5633020b8f62f882ae40d4c7beb1ea56eeaf7fe04b05`。四個完整App程序分列：設定透過實際Store保存中英往返，核對其他偏好不變、原生window ID／查詢／輸出選項保留、搜尋結果標題更新、BBS Frame與PNG bytes相同；開啟中的ANSI草稿直接診斷選語言，核對正文／標題／version／選區／顯示模式／viewport／bitmap／焦點不變；網頁與終端兩種文章搜尋核對查詢／索引版本／作用中命中／總數、合成preedit／焦點保留且沒有新增取頁。後三者是直接呼叫生產refresh方法的診斷選擇，不冒充從設定保存的混合Live流程或OS IME。四個既有完整workload各自完成嚴格peer的Close acknowledged；設定案例的圖片／GIF HTTP與worker回歸仍通過。

macOS另用真實原生設定下拉選English，觀察Settings及保存提示；關閉後主視窗的全螢幕、媒體設定、輸出、終端AX標籤、Vim、站端滑鼠、連線及浮球都為英文。選「Copy selected rows PNG」後再從原生設定切回繁中，主視窗仍選「複製選取行 PNG」。實際⌘Q後確認程序消失、保存值zh-TW、設定queue／timer／子視窗與menu action／target退場，獨立peer Close及零請求HTTP workers joined。沒有對已關閉App呼叫getAX或重新啟動；沒有操作production資料、vault或PTT。登入彈窗、正在播放的GIF／影片與其他平台的語言往返仍未有原生操作證據。

負向與修正分開保留：第一次檢查抓到語言列舉名拼寫錯誤；Clippy抓到抽方法後多餘借用及字串型別轉換，已修正。完整設定測試起初把建立中的native Window ID `None`與實際建立後的ID比對，誤判重建；診斷明確顯示immediate=None、realized=Some，現在先等待真正native window再檢查同一ID，未放寬不重建／保留查詢條件。第一、二輪未完成的peer／HTTP不列為通過。accepted-widget-runs的Close欄位從實際JSON audit解析，不以已淘汰的文字log句型比對。

上一輪Documents讀取停滯的PID55322本輪確認已不存在，log以`loopback probe timed out`結束，自製草稿SHA仍為`61ec8c2cb8d129b46d954d5883772bc88470931d447956bf8cb4830542344758`。這是失敗案例的終止證據，沒有Close或恢復成功的證明；未由Agent強制結束／重啟或改寫資料，也不能據此確認TCC或其他原因。原App、檔案、sample及log保留，先前詢問系統提示只作診斷資訊，不再依賴該程序繼續運行。

最終Slint release為共用library349 passed／4 ignored、bin76／1、multi_instance10／1；all-target Clippy及Slint＋native-video all-target編譯通過。Dioxus同輪library349／4、bin132／1、multi_instance10／1及all-target Clippy通過；最後只改Slint的視窗等待／連線標籤快取與probe，未重跑不受影響的舊host。382組JS零skip、core14命令／2重放、docs40篇／234本機連結／21回歸及diff通過。英文設定與編輯器bitmap已目視檢查；不代替實體DPI或screen reader。

完整S0–S8 goal保持active。完整搜尋與站點／名單／書籤管理、所有工具入口、原生登入／媒體語言、混合Live等效本機fixture、Dock／modal／OS等待退出、IME候選／可及性、四架構完整包、影片runtime／音訊、資料回退、可編譯的前端提交序列及同負載效能仍須完成。S9沒有切換入口、推送或發佈。


## S2／S7 原生站點管理（2026-11-08）

狀態：**站點表單、保存交易與指定macOS原生案例通過；有效切站的完整App驗證及完整S2／S7仍未完成。** Dioxus的站點編輯狀態移到`sites::session`，兩版共用同一個edit baseline、Target驗證、最愛新增／改名／移除、獨立預設站、損壞檔唯讀及成功結果發布。`ui::sites`只留下原有封包與Feedback；兩個測試一起移到共用模組，沒有靠複製測試增加通過數。共用變更以`3e09e7f`提交；該提交保留原本desktop feature入口，worktree的desktop-services遷移仍是待提交的另一項工作。

Slint新增按需建立的原生站點視窗，入口為主視窗、浮球「連線」分類及設定「一般」。提供PTT WSS／SSH預填、編碼、最愛、手動位址、保存／移除／設預設、明確連線與重載。UTF-8欄位有128／512 bytes上限，過大或控制字元保留前值並提示；真正位址／傳輸／編碼的接受與拒絕仍交共用Rust。舊明文最愛保留可見及可移除，不能新增為一般最愛／預設／連線。保存預設不切換本次連線，移除最愛不改獨立預設。啟動列表與管理器持有同一Model，快速選站在送出時重取成功保存後的選項，不捕捉過期靜態列表。

視窗沿用Output唯一檔案worker及共同Store的鎖／合併／備份／atomic replace，不新增常駐thread或輪詢timer。設定與站點共同使用ResultSlot保存一份worker結果；事件迴圈已結束時，先join同一worker再取回結果，避免只依賴未執行的UI回呼。關閉站點表單會等已開始寫入，失敗保留欄位與錯誤。整個App退出時由main統一隱藏兩個子視窗，再讓各owner結束；沒有用第二個root close handler覆蓋設定清理。設定到登入／站點的交接改用具名enum，仍先排空有效待存偏好。

證據在`.local/verification/slint-stations/`。第一輪CLI為`2e72e5a0bff2c415d3313735376472970e62a57eceee1603b38a3eadf40cc7b6`，Native-Stations.app ad-hoc payload為`ac996cd7db03cf26e55cdaaf0a2c7d367fe3449a8eaca6e9a81c74cf95fe5bb8`。完整元件驗證一般CRUD與編碼、明文／SSH錯誤編碼拒絕、原始備份、備份寫入故障／保留／重試、外部不同最愛合併、移除保留預設、舊明文移除、損壞唯讀／修復重載、重複開啟保留同一視窗與草稿、關窗排空寫入。輸入與前景為toolkit事件，不冒充OS IME；同輪設定及in-flight／pending關閉交接回歸通過，各自完成嚴格Close。

第一個macOS實際App從原生欄位貼入「原生站點」、`wss://native-os.example/bbs`並選GBK，明確保存最愛與設為預設；目前連線仍是原PTT／UAO。關閉後從⌘,的設定一般交接回站點，選最愛讀回同一名稱／位址／GBK，最後在站點視窗⌘Q。獨立磁碟副本核對最愛與預設均完全相符；Close acknowledged、HTTP workers全部join且零請求、設定／站點子視窗和待存結果清空，程序消失。最愛下拉末項第一次AX點擊因可視範圍沒有選中；改用真實Down／Return才選中，不將第一次操作算成功。沒有連到輸入的example位址、production store、vault或PTT。

### 管理視窗對主視窗的隔離

檢查直接消費者後，補上站點開啟時主視窗原生Keyboard／Ime的早期攔截，並讓貼上來源、登入／本機草稿入口、舊快速切站／重新連線及焦點復原共用管理視窗門檻。正在讀的剪貼簿先撤銷其准入；輸入法組字時不開啟新管理表單。這些不把已送到站端的bytes當成可回滾。有效的「連線切換」仍明確交給既有Connection序列化流程，而非從模態門檻偷偷放行raw輸入。

最新CLI為`1ac7bdc3f2b3005848ecfc610d14f141bb8eb2dd3257b5ea74c425a9e92b26d5`，Native-Stations-Modal.app payload為`147fe3d7326dde7acc825070c71c46f9db3f27d26381981318495c3d5d301092`。最新完整元件重跑站點、設定與草稿，均嚴格Close通過。另在真實macOS站點視窗輸入未保存站名，以⌘`切到主視窗、聚焦終端，實際按q及貼上自製文字；畫面保持連線且沒有接收內容。⌘,未開第二個管理表單，切回站點仍是原未保存值。⌘Q後peer核對零BBS輸入及Close，磁碟仍為原seed、未保存名稱不存在，owner／視窗正常退場。這只證明指定普通按鍵／貼上案例，不外推到候選組字、held key、OS所有退出或原生有效切站。

最終Slint release共用library351 passed／4 ignored、bin77／1、multi_instance10／1；all-target Clippy及Slint＋native-video all-target編譯通過。Dioxus同輪library351／4、bin130／1、multi_instance10／1與all-target Clippy通過；兩個測試移到library所以bin減少。最後只加Slint管理門檻，未重跑不受影響的Dioxus。core14 commands／2 replays與382組JS零skip通過。初次Clippy指出hide分支可合併，已修正；沒有因調整expected放寬站端bytes、Store或Close條件。

下一步先補有效loopback切站、GBK原始輸入／舊連線Close、斷線與正在dial切換、啟動預設重開與跨程序衝突的完整App證據。站點語言往返、欄位原生IME／Tab／輔助使用及完整搜尋入口也仍待驗。其他名單／書籤管理、混合Live等效fixture、Dock／modal退出、四架構完整包、影片runtime／音訊、資料回退、前端可編譯提交序列及同負載效能繼續。完整S0–S8 goal active，S9未切換／推送／發佈。


## S2 本機切站與雙編碼出站（2026-11-08）

狀態：**指定完整App切站與macOS原生雙編碼輸入通過；完整S2／S0–S8尚未結案。** 既有Connection與Slint站點表單未另做近似傳輸；這輪新增兩版可共用的獨立合成peer及Slint完整App入口。`connection::site_probe`以`e00088f`提交，只能由明確探針建構127.0.0.1 listeners。普通啟動與一般測試不會呼叫它，也沒有因測試修改TLS／SSH或一般明文拒絕策略。

Peer第一站為UAO，第二站為GBK，第三站回UAO，另有刻意不回WebSocket upgrade的本機listener。每站送固定ANSI雙色「中文」與不同站名，從同一生產Screen核對字形及左右色。舊站Close回覆刻意延後150ms，在此期間若新的TCP已到達就拒絕；新站依固定期望核對真正收到的Binary bytes、長度、順序與最後Close。Native案例只接受第二站`6b d6 d0 ce c4`（實體k及GBK中文）、第三站`a4 a4 a4 e5`（UAO中文），第一站零輸入。期望不是呼叫生產encoder計算；WS frame可分片，但累積結果必須完全相同。錯誤資料不寫入audit，僅保存符合已知合成前綴的hex；message／frame為4KiB、write buffer為8KiB、事件最多32筆、總等待最多300s。唯一peer thread持有current-thread runtime；成功或失敗皆join，未完成取消不算通過。

三個完整Slint App程序分別測：已連線時表單連續切站、先從浮球明確斷線再切、以及第一站關閉後dial卡在upgrade、取消該dial並切往GBK／UAO。第三例獨立要求舊dial的EOF，不以取消計數代替socket結束。控制器在新連線請求後故意排入舊generation的STALE輸入；peer任何BBS輸入都會拒絕，因此通過代表這些bytes沒有到任何站。各程序核對實際新Target、generation、80×24的已安裝格線、中文雙色字、非PTT站的自動功能關閉；退出後才完成第三站Close與peer join。觀察generation依序為2→4→6、2→3→5→7及2→4→6→8。這些是完整元件的toolkit意圖，不冒充原生按鍵或IME。

macOS arm64另外使用Native-Site-Switch.app，從實際最愛下拉選本機GBK peer、按「連線切換」，在終端實際按k並貼上中文。peer回覆及實際畫面均為GBK INPUT EXACT；再從站點表單選UAO peer、明確切換並貼上中文，回覆為UAO INPUT EXACT。最後⌘Q後三站皆Close、peer thread join、程序消失。獨立讀取peer最終JSON核對收到的hex完全為`[空,6bd6d0cec4,a4a4a4e5]`，也核對舊Close先於新accept；沒有發生自動登入、PTT操作或正式store／vault存取。普通實體按鍵與原生剪貼簿均走生產native adapter；這不是原生中文preedit／候選驗證。原始OS畫面經CUA觀察，另存的`site-switch.png`屬toolkit snapshot，兩者不混稱。

證據在`.local/verification/slint-site-switch/`。三個完整程序與原生包的CLI輸入SHA-256均為`b85200cc51769fc5d09f64e4f1b4af7bbd43c26e806dbb36e66cc12a4c528c16`，ad-hoc簽名後payload為`29a3bb6897982217e605e77950b96559186d8f6a93d986df92b0bcab6eaac297`。追加負向unit後重建CLI仍為同一hash。三個verifier負向測試實際送錯Big5到GBK站、在舊站未Close前新TCP、取消仍在accept的owner，分別要求拒絕、拒絕及join／不宣稱成功。最初temporary slice在select future中的生命週期被編譯器拒絕，改為有明確壽命的本地陣列；保留錯誤log，未放寬任何傳輸或Close期望。

最終release：共用library354 passed／4 ignored、Slint bin77／1、Dioxus bin130／1、兩版multi_instance各10／1；兩版all-target Clippy、Slint＋native-video all-target編譯、core14命令／2replay及382組JS零skip通過。初始fixture、負向測試、三種完整程序、原生操作、退出及檔案／子視窗清理各自分列，不以一個passed代替全部。

仍須補同一endpoint不同編碼、完整App預設站重啟／不登入、其他錯誤與多開資料往返，以及各平台原生操作。這些plain WS是明確隔離測試例外，不是一般產品明文放行，也不代替WSS系統信任、PTT SSH、OS vault或實站人工登入的證據。名單／書籤管理、完整搜尋、responsive OS I/O退出、原生IME／輔助使用、四架構包、影片／音訊、資料回退、前端可編譯提交序列與同負載效能仍須完成。完整S0–S8 goal維持active，S9未切換／推送／發佈。


## S0 可獨立編譯的提交序列（2026-11-08）

狀態：**共用服務及第一個 Slint 前端檢查點已提交；較新的完整前端仍在同一 worktree，提交序列與 S0–S8 繼續。** 這輪不把整份 WIP 一次提交，而是在 task-owned 隔離來源副本組出各筆確切內容，沿用本 worktree 的 Cargo target 與原生鎖。每筆通過對應 gate 才把該內容的 blob 放入 real index，核對 Git 身分、origin、tree 及提交後空 index。沒有改寫歷史或修改 canonical develop。

| 提交 | 範圍 |
| --- | --- |
| `d319667` | desktop-services 共用 Connection／storage／media／presentation；desktop 保留原入口 |
| `35066c5` | 共用字型、貼上確認、credential Job 輪詢、Vim 世代重設與 Dioxus 直接消費者 |
| `f4d1ba9` | 同一 writer 核對貼上完整 stamp、明確撤銷 navigation、合併通知中的保活暫停及畫面租約撤銷 |
| `a5a5bdf` | 圖片與原生影片共用既有 HTTP 信任／redirect 政策 |
| `646c42f` | 有界按需影片 relay 與獨立 range／stall 合成服務 |
| `6404301` | 可選 libmpv controls／session／GL surface；一般 desktop 不啟用 |
| `998d5b7` | 嚴格原生輸入、文章停滯及兩前端 parity-grid 探針 |
| `665efc7` | Slint 終端／輸入／選取／輸出／登入／浮球及快取文章的第一個前端檢查點 |

新增兩個真正 loopback 回歸：同一 thread 在 worker 尚未 poll 前排入原生鍵，再要求使用舊 input_epoch 的貼上，peer 只收到原鍵；以有效新 stamp 貼上固定 UAO bytes 後，再撤銷另一個有效貼上 reply，確認沒有額外 bytes。另一例讓原閒置期限經過，再在同一次 worker poll 前送暫停／恢復，獨立要求先無 refresh、之後才收到固定 Ctrl+L。刻意在隔離來源分別移除 stamp 核對與暫停 activity，兩個測試確實各以 assertion 失敗；來源已恢復，未修改生產 worktree 的 guard。原有明確取消 navigation 的回歸也隨提交保存。

每筆證據在 `.local/verification/slint-commits/`，包含 candidate／commit tree、原始 staged patch、實際命令及 logs。最初 services-only 與 Dioxus library 各338 passed／4 ignored；輸入 helper 後339／4；Connection 後342／4；HTTP 範圍31個media測試；relay 後356／4。各次 multi_instance 為10／1。headless helper 回歸106／2與screen21／0；加入parity測試後screen22／0。libmpv三個實際core／session測試均通過，不需要GL繪圖或外網；不代替已分開記錄的播放、音訊或runtime包驗收。所有對應 Clippy／check 成功；各 ignored／filtered 的邊界仍在原始log，沒有把它們當已執行。

第一個Slint檢查點取自先前已留存的文章輸出snapshot，只移入該階段的前端內容，保留目前共用owner。該確切提交來源的release為library356／4、bin41／1、multi_instance10／1，Slint all-target Clippy、Dioxus all-target編譯與Slint＋native-video all-target編譯通過。重新建出的完整App文章輸出與終端翻頁各完成嚴格peer的Close acknowledged。CLI SHA-256為`71aab6f41a599360d20c07b991bf912fa2d1ce4c0a2d56bff1ed2b7728ed5b01`；此為中間提交的toolkit證據，不冒充最新主視窗、原生OS操作或完整平替包。最新站點原生App仍保留，未被中間檢查點取代。

鎖檔逐步審核：relay只增加root對既有getrandom 0.3.4的直接相依；native-video只新增glow 0.18.0與libmpv2-sys 4.0.1。第一次offline解析曾把iana-time-zone的Windows相依從既有windows-core 0.61.2選到0.62.2；提交前已在隔離來源及完整WIP恢復原鎖定，locked編譯／三個core測試再通過。Slint新增270個package；把dependency名稱先解析成name＋version後核對，既有package版本／checksum及已解析相依都未替換，變動為新feature啟用的可選相依與名稱消歧義。macOS兩份正常／build依賴圖分別確認desktop-services及slint-research無Dioxus／Wry／WebView crate；不外推至OS間接載入或其他平台。

後續先把已驗證的文章操作／尋找、圖片／GIF／hover／文內展圖、影片UI、編輯器、設定／menu／語言／站點分成可編譯檢查點，再獨立整理打包與文件。最新完整來源仍需完成名單／書籤管理、完整搜尋、混合Live等效fixture、OS I/O／modal退出、原生IME候選／可及性、四架構完整包、runtime／音訊、資料回退與同負載效能。完整S0–S8 goal維持active，S9未切換／整合canonical／推送或發佈。

本輪最後已從最新完整worktree重建，CLI恢復為`b85200cc51769fc5d09f64e4f1b4af7bbd43c26e806dbb36e66cc12a4c528c16`，與前輪原生切站的Cargo輸入完全相同；Native-Site-Switch.app的已簽名payload雜湊也核對不變。最新Slint release為library356 passed／4 ignored、bin77／1、multi_instance10／1，all-target Clippy與Slint＋native-video all-target編譯通過。已確認隔離來源全部存入HEAD、無未追蹤內容或open files後，移除`.local/verification/slint-commits/validation-source`副本，du為26,944KiB；精確路徑、filesystem前後與保留例外見cleanup-owned.json。仍在使用的混合Cargo profile與最新原生包保留，不將空間變化全部歸因於本輪清理。


## S4／S5 前端提交與影片失敗清理（2026-11-08）

狀態：**文章操作／尋找、圖片／hover、文內展圖及影片UI已接續整理成四筆可獨立編譯提交；完整前端提交序列與S0–S8繼續。** 沿用同一份task-owned隔離來源、既有Cargo target與原生鎖，只從留存snapshot移入相關前端內容，保留已提交的共用Rust owner與當前完整worktree。各筆都核對origin／Git身分、通過確切來源的前端release測試、all-target Clippy與實際本機完整App案例後才提交。沒有重置最新worktree、改寫舊提交、整合canonical或推送。

| 提交 | 範圍與這輪完整App驗證 |
| --- | --- |
| `232c034` | 文章原始輸入／貼上／保活、兩種閱讀模式的共用Rust尋找；web／terminal尋找、貼上及保活四程序 |
| `ce2558e` | 原生圖片／GIF預覽、原圖輸出及保存hover偏好；同一媒體owner、精確HTTP與worker清理 |
| `b3b9f35` | 可視文內圖／GIF、圖片間隙與錨點、Enter／等寬裁切、自動展圖與選單；inline、自動設定、長Enter與All四程序 |
| `f4973a0` | 主視窗GL影片區、明確播放／seek／暫停與既有libmpv／relay；MP4像素／控制、停滯取消，以及探針失敗時的GL退場 |

證據在`.local/verification/slint-frontend-commits/`，每項保留candidate／commit tree、staged patch、實際命令／環境、binary SHA、合成輸入與scope。文章操作版前端42 passed／1 ignored，圖片／hover版48／1，文內版57／1，影片版57／1；這些bin測試與完整App是分開證據，沒有因名稱含native就算作OS按鍵或跨平台通過。各個成功的完整App程序取得嚴格peer的Close acknowledged及已join的HTTP workers。

### 影片負向與探針修正

第一個stalled影片探針退出時回報renderer尚未退場，遮住原驗證錯誤；此輪沒有正常Close或完整core清理證據，仍列failed。修正探針後，不論驗證成功或失敗，都在Slint事件迴圈仍能提供GL context時await同一Media.shutdown，再返回原始結果；只有驗證及清理都成功才寫video.json。錯誤紀錄僅保存已知合成步驟、foreground／准入／renderer與session布林狀態，不記正文、URL或帳密。這是明確探針的退出路徑修正，未將一般App／Dock／選單關閉全部宣稱已修復。

接著的失敗JSON明確顯示foreground=false、play_allowed=false、core paused且零HTTP：當次Play沒有前景准入，生產保護正確拒絕；不能由此判定下載壞掉。探針現在先等啟動的Focused／Occluded事件穩定，再明確派發toolkit前景操作、讓既有permit binding經過一個UI turn後才按第一次Play。這些只在明確探針中執行，不改一般產品的失焦／隱藏／背景播放政策，也不是S8啟動效能數據。

Agent第一次以字串插入這段啟動等待時，誤放在seek之後、pause之前，造成了不符合立即暫停契約的時間位置檢查失敗；已移到第一次Play前。該錯誤修訂的log／JSON／App識別與原生AX觀察分別保留，不當成產品seek故障或原生MP4通過。此前一次原生前景stalled App確有完整session／Close／HTTP清理；那份binary僅代表其當時scope，與最後修正後的CLI分開。

最後修正來源CLI SHA-256為`4629d36af0ce33c1b4609ad4709c9f633c9640b19bd3d27391ff276fffbb2fc0`。MP4完整程序核對四象限GPU pixels、seek／pause／resume、失焦暫停且恢復焦點不自動播放、影片換圖片與退場；stalled程序核對一個上游請求、零bytes、取消一個stream、control joined及core destroyed，兩者均完成正常Close。另用同一binary在renderer建立但尚未Play時刻意返回錯誤：exit=1、原訊息保留、零上游請求、stream stopped／control joined／core destroyed，且沒有成功video.json。這個負向只證明影片錯誤清理；外層失敗分支沒有另印BBS Close outcome，故不將它算成負向Close ACK驗收。

CUA曾實際選取兩個獨立原生App並觀察退出，沒有對已關閉App再取AX或重啟同一輸出目錄。stalled App在取得原生前景後通過，其餘操作仍為toolkit。另一個MP4 App曾在AX顯示播放與約1.125秒，隨後被上述錯置等待的檢查拒絕，保留為failed；沒有把可見影片或App消失單獨當成整體通過。OS媒體控制、音訊可聽性、GL context丟失、所有正常退出入口及跨平台runtime仍待各自驗收。

下一步繼續原生編輯器／色票／框選上色、草稿失敗與儲存、設定／menu／語言及站點的可編譯提交；其後獨立提交打包器與文件。名單／書籤管理、完整搜尋、混合Live等效fixture、一般active-video App Quit／modal與OS I/O退出、IME候選／可及性、四架構完整包、資料回退及同負載效能仍未完成。原有長文500ms＋150ms逐頁合成延遲不變。完整S0–S8 goal維持active；S9未切換或發佈。

本輪最後從最新完整worktree驗證`slint-research,native-video`：library359 passed／4 ignored、bin77／1、multi_instance10／1與all-target Clippy通過。恢復普通Slint CLI後仍為前輪站點原生驗收輸入的`b85200cc51769fc5d09f64e4f1b4af7bbd43c26e806dbb36e66cc12a4c528c16`；optional-video探針的改動與普通binary分開列。已確認exact source全部存入HEAD、無symlink／未知檔或open files，移除本輪validation-source（du27,312KiB）。另將失敗Native-Focused-MP4.app的非payload檔案及重建manifest保存，確認signed native逐byte與保留App相同後移除重複包（du58,816KiB）；差異與filesystem前後見cleanup-owned.json，未把空間變動全部歸因於本輪。最新必要包、唯一失敗證據及持續使用的Cargo profile保留。


## S6／S7 完整前端提交與可重現草稿驗收（2026-11-08）

狀態：**目前已有的完整 Slint 前端、共用服務、播放器探針及打包工具已全部存入可編譯提交；S0–S8 功能與跨平台驗收尚未完成。** 本輪沿用同一 worktree 的 Cargo target，在 task-owned 來源副本組出每筆確切內容，通過相應 gate 才提交。沒有整合 canonical develop、推送或改動正式入口。

| 提交 | 內容與本輪驗證 |
| --- | --- |
| `c7eae76` | 原文草稿編輯器；fresh／新程序 restart 各自核對標題與完整 ANSI 原文 |
| `e7c6449` | 浮動色票、選區歷史與取消；色票／undo／redo、讀取中與帶回中取消 |
| `8d57ae1` | 站端框選上色；完整、取消與部分執行的精確 ANSI／格線 |
| `cb39a0f` | 草稿七種中斷：範本變更、改標題後、刪除後、寫入後、斷線、讀取中及帶回中關窗 |
| `11c3216` | 儲存故障、確認保存後強制終止、新程序恢復及兩程序同稿／不同稿保存 |
| `d5eaed7` | 原生設定視窗、成功後套用與關窗保存交接 |
| `b764375` | 原生 App 選單的 Rust 退出入口、啟動時首尾偏好；指定設定待存退出與首份文章投影 |
| `9ed3b4e` | Fluent 語言同步；設定／草稿及兩種閱讀搜尋保留內容與查詢 |
| `2b3f4a3` | 原生站點／最愛／獨立預設與單一檔案 owner；CRUD、損壞修復及設定交接 |
| `19e680e` | 完整 App 的已連線／已斷線／取消握手三種切站探針 |
| `d9ef72f` | 共用 backend 的獨立原生播放器與圖片診斷；實際 MP4 驗證 |
| `4ea82c6` | feature 對應 notices、隔離 S1 Windows 探針及 macOS 播放器打包工具 |
| `3172b86` | 公開、僅需 Python 標準庫的草稿儲存驗收父程序 |

每筆 candidate tree、staged patch、命令、log、binary SHA 及提交 metadata 保存在 `.local/verification/slint-editor-commits/`。草稿七種中斷中，斷線要求明確 `no_connection`，另六種要求嚴格 peer 的 Close acknowledged；沒有以斷線冒充 Close。編輯器／控制／上色／失敗儲存／設定／語言／站點的 bin 測試依序為64、70、72、72、76、76、77 passed，各有1 ignored，完整程序與 unit 分列。每筆對應 all-target Clippy／編譯均成功；新增 macOS 直接相依只引用原有鎖定 package，未替換既有版本、checksum 或解析邊。

### 從獨立 checkout 重跑儲存案例

先以普通 Slint feature 建置，再指定**全新**輸出目錄。此父程序不依賴 `.local` 的私人腳本、RTK、App 包或帳密；本輪 Agent 執行命令仍遵循本機 RTK 規則。

```sh
cargo build --locked --release --no-default-features --features slint-research --bin pttstella-slint
python3 tooling/verify_slint_draft_storage.py --executable target/release/pttstella-slint --output /path/to/new-draft-evidence
```

Windows 將 executable 改為對應 `.exe`。這只是可攜的驅動方式，尚不代表 Windows 或 Linux 已實際通過。父程序只操作自己啟動的 direct child；強制終止前核對 fresh marker 的 PID 與仍在執行的子程序，拒絕其他 PID。正常與失敗出口都 join 自己的子程序後才寫結果；既有輸出不覆蓋，marker／log 有界。兩個 ownership 測試實際確認錯誤 PID 不會殺掉任一程序及既有檔案保留。

本輪 macOS arm64 的公開 runner 在 `public-storage/report.json` 通過8個獨立程序：備份寫入故障、損壞資料、crash-stage／recover、同稿 a／b、不同稿 a／b。7個正常程序完成嚴格 Close，另1個在確認保存後且 pending debounce 尚未 flush 的指定邊界刻意 kill；父程序另做2組磁碟獨立 expected 核對。同稿保留衝突副本，不同稿合併；確認原文、ID、revision 與 conflict flag。這不是任意 mid-write crash、斷電或 OS 輸入／IME 驗收。原先末字消失的 OS 紀錄仍保留；使用者確認當時有其他操作，原因尚未證實，不能由這輪合成操作宣稱已修復。

最新根目錄確切來源通過 release：library359 passed／4 ignored、Slint bin77／1、播放器 bin1／0、multi_instance10／1及screen22／0；`native-player-probe` all-target Clippy 成功。Python 共93測試通過。恢復普通 Slint binary 後 SHA-256 仍為 `b85200cc51769fc5d09f64e4f1b4af7bbd43c26e806dbb36e66cc12a4c528c16`。本輪中間提交的 toolkit 程序沒有取代上一輪分 hash 的原生操作證據；未新增原生 Windows 包。現有 Windows 打包腳本只覆蓋 S1，不是完整 S8 候選交付。

下一步補正常產品 active-video 的 App Quit／關窗清理、名單與書籤管理、完整設定搜尋、混合 Live 等效 fixture、同 endpoint 編碼與預設站重啟／不登入、OS I/O／modal 退出、原生 IME 候選與可及性，再完成四架構原生完整包、runtime／音訊、舊資料往返及同負載效能。現有功能已提交不等於功能 parity 或 S8 通過；完整 goal active，S9 未切換／發佈。


## S5／S7 正常影片退出與根視窗所有權（2026-11-08）

狀態：**正常根視窗 Close 與 App Quit 的指定 GL 清理已通過，修正提交為 `1b2bb96`；Dock、modal、OS I/O、其他平台等退出邊界仍待驗。** 前一輪只有影片 verifier 自己 await shutdown。檢查正常入口發現 App Quit 直接停止事件迴圈，而主視窗的 close handler 又覆蓋了 ArticleSession 先前安裝的非同步 handler，因此正常帶著 renderer 退出仍可能在 Media.finish 回報尚未退場。

新增單一 Exit owner，根視窗 Close、原生 App 選單及正常 probe 完成都走同一 callback。第一次請求立即撤銷主視窗輸入／媒體／保活等准入，重複請求不再建立工作；延後的貼上、設定／站點開窗、重新連線及切站亦拒絕 closing root。依原有 owner 停止文章工作、隱藏管理視窗，再釋放根元件，讓 renderer 在自己 current GL context 的 RenderingTeardown 釋放 surface／mpv render references。唯一明確的事件迴圈繼續等待 video session，之後才 quit；既有草稿、設定、站點、檔案及連線的同步 join 仍在 main／session 統一執行。移除被覆蓋的文章 close handler 和只為該 handler 保存的 media weak link。

Video.step 在 UI 消失後仍能取消 pending、消費 Stopped、join session 與保存清理結果，不能因 weak UI 無法 upgrade 就跳過資源退場；也不替已消失的 UI 建立新 renderer。Exit 的 future 只持有依賴，不持有它自己的 owner／JoinHandle，避免退出中的 Rc cycle。普通閒置期間沒有新增 thread、timer、輪詢或逐 frame 工作。

### 本輪負向與界線

第一個版本誤以為 hide 會銷毀 renderer，ready-quit 在30秒期限後由父程序終止自己直接啟動的 child，沒有 Close 或成功清理報告。已保留 `failed-hide.patch`、tree、binary hash、準備 marker 和原始 log，未當成通過。查鎖定 Slint backend 原始碼確認：這個 macOS／Winit 路徑只隱藏原生視窗，仍保存 surface。後續改為由 Exit 唯一持有根元件，hide 後 drop，實際驗證最小化也不依賴恢復視窗或 redraw。

下一個 playing-close 探針在剛回報 loaded／playing 時立刻做 GPU 檢查而失敗；正常退出 owner 仍完成 core destroyed、control joined 和 HTTP stopped。該次沒有保存原始 GPU samples，不能確定第一幀的實際像素或把它宣稱為產品顯示缺陷。最後探針先要求播放位置超過0.2秒，再做同一四象限獨立 expected 檢查並保存 samples；沒有修改顏色期望或容差。這是明確探針的等待條件，不是產品啟動延遲或 S8 效能數據。最初 probe closure 的泛型型別錯誤已在 runtime 前修正；另一次文章回歸使用了不存在的 CLI flag，只得到 usage，改用既有 `--probe-stalled-article` 後才列通過。

### 完整程序及原生操作

證據在 `.local/verification/slint-app-exit/`。最後 optional-video CLI SHA-256 為 `5eff144ad294bc611c415c62ff1f6f63f4acf21030e073be50d7aee436c45c82`。四個正常完整程序各自在退出前確認 renderer／session 仍由生產 owner 持有，然後走正常入口，不由 exit probe 呼叫 Media.shutdown：

| 案例 | 獨立結果 |
| --- | --- |
| ready-quit | 尚未 Play，零上游請求；正常 Quit 後 GL／core／control／relay 皆結束 |
| playing-close | 四象限 GPU pixels 正確、播放中根視窗 Close；一個影片 HTTP 請求 |
| stalled-quit | 一個 HTTP 請求、零 bytes、一個取消中的 stream；正常 Quit 完成 join |
| minimized-quit | Winit backend 確認 minimized 後 Quit，沒有恢復視窗才能清理的依賴 |

四例都重複要求退出，核對立即撤銷 permit 及延後設定／站點／媒體請求無法重新進入；事後 audit 要求 renderer retired、pending／offer 空、session／video task／image worker 清空。每例皆有嚴格 BBS Close acknowledged 和 HTTP workers joined。原有 MP4 GPU、seek／pause／resume、失焦不自動恢復及影片換圖片的完整程序亦以同一 binary 通過。

兩個不同原生 App 分別用 CUA 實際 Raise、seek 到0、點播放，再以 ⌘Q 或標題列原生 close button 退出。兩次均觀察到「播放中」，最後全域 App inventory 中兩程序都消失；獨立 log 與 JSON 核對相同的 GL／session 清理、HTTP join 及 BBS Close。LaunchServices 的退出狀態碼沒有直接觀測，不捏造 exit=0。native 與 toolkit 證據分列，沒有把它們當成音訊可聽性、Dock Quit、OS modal、其他 OS 或完整 runtime 包通過。

恢復普通 Slint CLI 後 SHA-256 為 `d4293939fa958902db8507917d0742ff03c90a8f2da49476b0cd11ec31d1075e`。設定第一份在途＋最新待存修改、完整切站、停滯文章取消，以及草稿讀取／帶回中退出各完成嚴格 Close；公開儲存 driver 再跑8程序，7個正常 Close、1個指定邊界 kill、2組獨立磁碟核對通過。這些證明根視窗釋放後既有資料 owner 仍能排空，不等於任意 OS I/O 阻塞已解決。確切最新 Rust 來源通過 release library359 passed／4 ignored、Slint bin77／1、multi_instance10／1、screen22／0，以及普通 Slint／optional-video 的 all-target Clippy／編譯與 fmt。

打包工具的93項 Python 與文件 gate 已在本輪前半段通過；本輪沒有修改 workflow。補查 actionlint 時本機找不到執行檔，記為 `not_ready`，沒有把完整打包 gate 宣稱通過。四架構完整包、依賴／notices／乾淨機器及音訊仍依 S8 待驗。

清理紀錄分列：前半段移除 `.local/verification/slint-editor-commits/validation-source`（du28,516KiB，471個 tracked 檔逐 byte 與 HEAD 相同，另24個自產 Python cache）和已 join 的 replay index。此階段在確認兩個 native payload 逐 byte 相同、無 open files 後，把 Native-Video-Quit.app 的非 payload 檔案與完整重建 manifest 保存到 `native-quit-app-metadata`，再移除重複 App（du62,608KiB）；保留 Native-Video-Close.app 和先前站點輸入基準。兩個 cleanup-owned.json 分別記 exact path、UID、symlink／lsof 檢查、filesystem 前後與保留例外，空間差值不全歸因於清理。失敗 snapshot 的小型 alternate index 已另移除，但當時未記單獨 filesystem 前後，該量測缺口明列於 ledger。持續使用的 Cargo profiles、唯一失敗證據、未整合 worktree／commits 及所有權不明的 OS 暫存保留。

接續完整名單／書籤管理與設定搜尋，並補混合 Live 等效、同 endpoint 編碼／預設站重啟、其他原生退出／IME／可及性、舊資料雙版本往返、四架構候選及同負載效能。完整 S0–S8 goal active；S9 未切換、整合 canonical、推送或發佈。


## S7 共用搜尋目錄、原生定位與具名設定檔（2026-11-08）

狀態：**目前可用的23個 Slint 設定目標已有搜尋與原生焦點路徑；全功能設定搜尋仍須接上未完成的名單管理。** 本輪沒有把缺少管理介面的結果做成無作用按鈕，也沒有將局部目錄覆蓋率算成完整 S7／S8。

| 提交 | 內容 |
| --- | --- |
| `bae1852` | 從既有 Dioxus 提取24項固定目錄至 `assets/settings-catalog.json`，兩版共用 closed targets／page／Fluent label 和 alias keys；Rust 新增有界索引 |
| `8195c7c` | JS 在128 UTF-16 units 邊界不製造孤立 high surrogate，與原生完整 Unicode scalar 行為對齊 |
| `b2af0a4` | 原生搜尋結果切分類、聚焦與捲入可視範圍、單一可取消焦點 timer、暫時操作開關及具名 profile 匯入／匯出 |
| `a1dee9a` | 搜尋欄經一般 focus 路徑重新取得焦點時，也取消舊定位 |

Rust 索引在設定視窗更新語言／標籤時由固定產品 metadata 建立，查詢重用既有索引；一般閒置時不重建。查詢前截至128 UTF-16 units、不切 scalar，再以 NFKC、大小寫和空白分詞處理，每個詞都須命中產品標籤或別名的子字串。大小寫、全形 `ＢＡＳＥＬＩＮＥ` 與 ligature `ﬂoating` 都有獨立 expected。查詢空白／未知／私有名稱／HTML 片段不產生結果，不讀終端、帳密、字型清單、最愛站名或使用者設定檔名稱。Dioxus 保留既有 adapter 及導覽行為，Rust／JS 消費同一 JSON；不增加 WebView 依賴至 Slint。

NFKC 使用已鎖定的 ICU normalizer 2.3.0。逐 package 比較 Cargo.lock，package 數、版本、checksum 與既有解析邊完全相同，只增加 root 對該既有 package 的直接相依；沒有為此更新其他套件。索引最多64項，實際24；原生未完成的 filter manager 沒有對應 widget，因此暫不顯示該結果。站點與暫時操作也依實際可用的 owner／狀態決定，不提供無法取得焦點的 disabled 目標。

搜尋結果涵蓋語言、字型組合／英數／中文／字高／基線／寬度／重設、顯示 profile 名稱、縮放、Vim、站端滑鼠、防閒置、浮球透明度／右鍵／可見、文章模式／首尾、hover／自動開圖、站點、帳號與關於。選結果只改導覽與焦點；實際縮放／暫時開關仍須再明確按鈕啟動，先排空設定待存修改，再交給原有 root owner。開關顯示用 typed bool，不靠解析中英文通知文字。這些暫時狀態不新增至 AppSettings 或 display profile。

焦點排程由 Preferences 唯一持有，新的結果取代舊 timer；換查詢、回搜尋欄、切分類、關設定與 App 退出會取消。回呼再次核對同一頁、空查詢、視窗開啟、非組字／非退出等條件。原生 ComboBox／CheckBox／Button 與文字欄分別取得焦點；Cmd/Ctrl 的文字編輯快捷鍵只由真正的文字欄消費。搜尋不重新填入表單，不丟無效中間值、使用者名稱或待存草稿。

### 完整元件、負向與基準

證據在 `.local/verification/slint-settings-search/`。第一輪46目標中的站點定位失敗：當時普通 media probe 沒有掛站點 owner，索引卻給了無法聚焦的按鈕。已保存該確切修改檔／SHA／binary 與 log；修正結果可用性檢查，並為明確的設定搜尋 fixture 建立隔離站點 store。接著暫時開關檢查遇到防閒置未切換，進一步查明 media probe 沒有掛 KeepAlive owner；不是由標籤或一次 timeout 推定傳輸故障。明確設定探針現在掛上與 live 同一 KeepAlive／Login owner，普通產品路徑不換 owner、不放寬准入。新增 typed 狀態同時用於按鈕 checked，避免把翻譯文字當成執行結果。最初增加 changed callback 時曾與既有 Vim handler 重複，編譯器拒絕後合併同一 handler；未放寬測試。

`b2af0a4` 對應的三個完整 Slint 程序為搜尋、原有完整設定與 in-flight＋latest pending 的關閉保存。搜尋用手寫23個 query／page／field expected，在中英文各完成一次，共46次；另驗 whitespace／未知／注入樣式文字、私有名稱不進索引、過時焦點不搶新查詢、無效字高草稿保留、四種暫時操作的明確啟動與恢復、無效 profile 名稱在 picker／worker 前拒絕，以及匯入名稱／通知。AppSettings 與原 seed bytes 還原一致，三程序各有嚴格 peer Close acknowledged 及 HTTP join。此檢查點普通 Slint CLI 為 `d83be7824453d9ac7e505cbd9d3fe1ca0a24e9641bc95747956d30a55d714773`。

共用變更先通過 core14命令／2 replay、Dioxus release library358 passed／4 ignored、bin130／1、multi_instance10／1與 all-target Clippy。最後 Slint release library358／4、bin76／1（舊私有索引測試移除，由共用2個測試與完整元件接手）、multi_instance10／1、all-target Clippy、Slint＋native-video all-target check、fmt 與兩版 release build 通過。383組 JS 零 skip 通過。這些計數不含原生 OS、所有平台或效能驗收。

收尾另補一般 focus 的競態：建立待處理的結果定位後，以欄位本身的正常 focus 路徑回搜尋欄，不經原先已有取消功能的 focus-search callback。第一次 probe 直接對尚未建立的條件 widget 要求焦點，只得到 setup failure，不算產品回歸；改由既有延後定位完成 setup 後，未修正來源明確以「pending result stole ordinary native search focus」失敗。`a1dee9a` 在 focused-target 回0時取消同一 timer；相同完整程序在中英文、46個目標與 runtime／profile 案例全部通過，嚴格 Close acknowledged。bin76／1、all-target Clippy 及 optional-video check 再通過。最新普通 CLI 為 `0a6da4c46fd1dfd13c3ec2858cec3c181b8aa3d1773e682909910a778e1d7c98`；負向 log／binary hash、獨立 expected 與 test patch 另存，沒有把 setup failure 當成已修復的產品缺陷。

### macOS 實際視窗與設定檔

第一個原生 Slint App SHA-256 為 `57525d1a3925b706be37c83ba971d887788fd150c40486c3489d7729a536d86d`（input 與 ad-hoc signed payload 相同）。CUA 從 ⌘, 開設定，原生貼上全形查詢、點結果後 AX 明確顯示基線欄位20取得焦點。再以「設定檔」查詢定位名稱欄，貼入自製「原生搜尋驗收」，經原生 Save picker 寫入 task-owned `/private/tmp` 目錄。獨立 JSON 核對 name、format／version、bundled 字型、23／20／100；檔案251 bytes，SHA-256 `b0c3ca1b40f646cbc6e254595c4b12764cf74e03da96ab4f187c66ea2dc6991f`。改欄位為另一名稱，再經原生 Open picker 讀同一檔，AX 顯示名稱還原。⌘Q 後 settings／sites／menu 清理、零HTTP、BBS Close 與程序消失分列，沒有觀測 LaunchServices exit code。

這輪另看到相同 display 匯入後仍留下先前的匯出通知。最後來源將成功匯入抽成單一 typed consumer，填回檔案名稱、排入原有自動保存，並清楚顯示「已載入顯示設定；有效變更會自動儲存」。新的完整元件用同一 consumer 驗證通知與名稱；上述原生 App 為通知修正前的 hash，兩者 scope 不混稱。profile 名稱先用共用 PortableProfile validator 驗證，無效名稱不開 picker；匯出仍只含有效 display 與使用者名稱，不含 app 偏好或字型檔。原生 IME 候選和其他平台沒有由貼上結果推定通過。

Dioxus 另用真實 WKWebView 對照 App，輸入與 signed payload 為 `c61c260f9895a1ab37b2a62896a3ba8c56533f346eaae47bdc8c71434bcc79a1`。實際 ⌘,、貼入全形查詢、點結果後，AX 顯示基線 number field20取得焦點，證明合併 script 的共用 JSON 初始化與 adapter 正常。之後從浮球擷取輸出 parity-grid PNG、原生關窗，`surface-probe.json` 為 passed、full_png=true，`cleanup.json` 為 run_completed／cleanup_completed=true、disconnect=acknowledged，程序消失。第一個 PNG 嘗試因 Agent 沒有依舊 Dioxus probe 契約先建立輸出目錄而得到明確缺 parent 錯誤；建立唯一指定的自有空目錄、手動關通知並明確重試後才列通過。這是 harness 準備修正，沒有修改或偷偷重送產品 I/O。

本輪所有操作只用隔離合成 store／loopback／自製設定檔。未取用 production 資料、vault 或 PTT。兩個實際 App、獨立 negative 證據、最後 CLI 與 source patch 保留；唯一 `/private/tmp/pttstella-slint-search-d1slhunf` 已核對 resolved path、UID、單一內容、與證據副本逐 byte 相同及 lsof 無使用後移除，du4KiB，filesystem 差4,096 bytes。exact path、前後 free 與保留例外見 cleanup-owned.json；不將 shared filesystem delta 全歸因於自己。Cargo profiles 仍供整體目標使用，worktree／未整合 commits 保留。

接續完整名單／書籤管理，並接上最後的名單搜尋目標。混合 Live 等效、完整原生輸入／可及性、四架構候選、音訊／runtime、舊資料雙版本往返、同負載效能及完整候選重跑仍未完成。S0–S8 goal active，S9 未切換、整合 canonical、推送或發佈。


## S7 原生名單管理與完整設定分類（2026-11-08）

共用匯入准入提交 `3e3ffde`，原生表單提交 `2f3e01e`。既有 Dioxus 匯入 owner 移到 `filters::preview::Import`，仍核對 request、取消與原規則快照，保留白名單優先及匯入色覆寫規則；兩個原有案例跟著移入 library。Slint 表單沿用 `Rules`、文字 parser、同一 `Store::save_from` 和 Output 的單一檔案 worker，article owner 保持唯一有效規則來源。只有保存成功的結果才更新 presentation，預覽不改檔案或畫面。

表單提供黑／白名單分頁、ID／分類排序、新增／更新／移除、兩種黑名單遮罩、15 色共用及個別覆寫，以及系統選檔、預覽、取消與明確合併。切分頁清除未保存欄位，排序保留草稿。讀取錯誤保持原有效規則並將表單設為唯讀，明確重試讀回有效檔案才恢復；保存失敗保留欄位和原有效規則。匯入取消／關閉後的成功或錯誤都不能恢復舊預覽。退出保留在途結果槽，加入精確 worker 後再收結果，不依賴 UI callback。

設定補齊一般、外觀、輸入、媒體、站點、名單、帳號、關於八分類，固定索引24項皆可定位到原生欄位／按鈕。名單結果僅定位，明確按管理按鈕才關閉設定、交接表單；原 autosave drain 路徑維持。站點與名單共用原生文字視窗的 IME／編輯捷徑橋接，以同一 manager-open 准入阻擋 BBS 輸入、保活、滑鼠和其他相斥操作。沒有新增 idle timer 或檔案 worker。

證據根目錄 `.local/verification/slint-filters/`：

- 共用重構：core 14 命令／2 replay；Dioxus release library360／bin128／multi-instance10通過，另4／1／1明確 ignored；desktop all-target Clippy通過。完整 JS383通過，零 skip。
- 原生來源：Slint release library360／bin76／multi-instance10通過，另4／1／1 ignored；all-target Clippy、選用 native-video 的 all-target check、fmt及 release build通過。後續只增加診斷斷言／退出稽核，原 bin76回歸仍通過，最終 all-target Clippy另驗證編譯。
- 最終 CLI `743955772777c95d8edb617609ed2b04d6b1d846dd8596209803d75d6067c631`：`final-component/` 與 `final-web/` 明確核對各自實際閱讀模式、兩種遮罩的 ACK 投影、CRUD／大小寫更新／顏色、排序、預覽取消／確認、晚到成功與錯誤、備份受阻保留草稿、壞檔唯讀及重讀。兩案及 `final-shutdown/` 均為0取頁、peer零輸入、正常 Close acknowledged、HTTP零請求且全部 joined。退出 audit核對 window／preview／結果槽清理及磁碟與有效規則一致；shutdown案另驗證事件迴圈退出時仍在途的授權保存沒有遺失。
- `settings-search/` 在前一 CLI `213adf0dd89ff866e7cd6a244874234a4c15b1cb1bf5df7a2ade0a08c38fc206` 完成24項×中英文＝48次定位及既有焦點／設定檔／runtime案例；同 payload 的站點表單與待保存設定退出回歸亦通過。此後的來源差異是明確 filter probe模式與清理斷言，不將不同 payload 說成相同 executable。
- `Native-Filters.app` 的 input同為上述 `213ad…`，簽署後 native SHA256為 `5d51154f243e6951b6bcf20d099b8ddff368cc8c5070c002c1db33548af3942b`，對應 `candidate-before-component.patch` 及其基準。macOS arm64實際 ⌘,、搜尋 blacklist、結果聚焦與明確交接、繁中貼上保存／重新選取均可觀察；系統選檔讀入自製94-byte文字，先預覽2筆、略過1筆、調整2筆，再明確合併。讀回主檔獨立預期 NativeA／NativeB／NativeW三筆、白名單ANSI10，備份為先前只有NativeA的一筆，原 bytes已保留。名單開啟時 ⌘Q，後續 App inventory無該 App、peer Close acknowledged、HTTP零請求且 joined；LaunchServices退出碼沒有直接觀察，不宣稱 exit0。

系統選檔的 Go To 路徑第一次 paste／Return維持舊文字，改以原生 AX欄位填入路徑並確認後成功，沒有推論其原因。這輪沒有驗證注音候選或把繁中貼上當成 IME。使用者已確認先前草稿異常時有其他操作，舊末字異常原因仍未定，不宣稱已修復。建置期曾因 Window保留的 color屬性拒絕編譯，改名為 entry-color後才通過；沒有把建置失敗算成執行通過。

清理盤點記於同目錄 `cleanup-owned.json`。保留必要 App、初始 payload 的來源對照、最終 PNG／合成資料、正常與故障證據及仍供 goal使用的 Cargo profiles；中間被取代的成功探針輸出按確切路徑移除。原 worktree與提交尚未整合，保留；canonical develop、正式入口與遠端未修改。帳號右鍵、暫時揭露、書籤及其餘管理功能、混合 Live、原生 IME／DPI、多平台、資料回退與同負載效能仍待完成，S0–S8 goal持續，S9未執行。


## S4 帳號右鍵、揭露與推文標示（2026-11-08）

功能提交 `511a048` 接入三策略／移除的原生內容選單、列表作者欄、暫時揭露、同 ID 推文底色及取消。Menu target由當前 ACK Paint的 account_words／reply metadata和原生 cell geometry產生，保留 generation／revision／presentation／input epoch、滑鼠路由與選取／幾何快照；UI不重新解析原文。規則保存交給既有名單 manager與單一 Output worker，沿用 Store三方合併與備份，成功才 commit到 article owner，保存失敗維持原有效規則與檔案並顯示錯誤。

切篇／切站等來源 scope變更、永久名單與揭露走同一 presentation失效流程，撤銷舊輸出與媒體。只有 reducer成功接受的推文底色變更，才重綁同 generation／article的已確認呈現版本，保留文章錨點、圖片間隙、已解碼原始 Arc、GIF、等寬選擇與錯誤狀態；舊 hover／預覽與點擊租約仍撤銷。文章列像素快取重建以畫出新底色，已展開圖片不重抓。滑鼠命中不 clone整個來源 Frame；用核心按列排序的 word metadata限縮至命中列。

補測連點同一 ID時發現真正的事件順序問題：Slint 1.18.1在 Released中先回呼 clicked／double_clicked，再送 pointer_event(up)。原通用雙擊處理提前清掉第二次 gesture，導致不能再次點 ID取消。現僅為已辨識作者且沒有 Shift的點擊保留該次放開；其他雙擊與選取路徑不變。修正前的狀態紀錄確認來源、cell、作者 metadata、前景及准入均有效，修正後兩種閱讀模式的連點、右鍵切換／取消及 Shift拖曳選取均通過。依賴版本與本機來源位置記於下述證據，沒有改 Slint套件。

證據位於 `.local/verification/slint-account-actions/`：

- 功能 CLI SHA256 `3e8012a744a225ccbe35a1626ca68cda62213618438d0260f7c977c644ebebbd` 的 `terminal-complete/`／`web-complete/`，驗證右鍵策略與移除、保留既有備註、備份受阻不改有效／磁碟規則、列表作者欄留白對應完整 ID、遮蔽作者不從 hit metadata出現、來源行 PNG完全黑色、選取文字與連結沒有原文旁路、揭露撤銷舊選取／輸出租約。真實本機 peer送來新列表後，舊選單及 target均不能寫規則，臨時揭露重設。
- 兩案的左鍵 ID優先於返回，連點與右鍵均可取消；合併 ID的黑色遮罩不被底色揭露。Shift拖曳確認原作者字串的行／欄選取。網頁案另以真實 worker狀態和原始 Arc核對三個已解碼資源、非空等寬選擇及閱讀位置保留，並從實際 image model看到 GIF像素繼續改變。兩案均為0自動取頁，只有探針明確推進來源的一筆 `1b5b367e`；peer正常 Close acknowledged。網頁 PNG／GIF／once各只有1筆 HTTP，terminal則0筆，全部 joined。這是單頁閱讀→列表的合成負載，不是完整混合 Live、可信 OS手勢或效能驗收。
- 同一 payload的 `form-regression/`、既有兩種右鍵路由及 `article-output/` 回歸通過。library360／bin76／multi-instance10通過，另4／1／1明確 ignored；all-target Clippy、選用 native-video的 all-target check、fmt與 release build通過。沒有修改本輪共用 library／Dioxus／JS選定來源，因此未把先前 desktop／JS結果冒充重跑。
- 隨後 `d405f9a` 修正內容選單既有 Settings action沒有對應 label、退回空字串的問題。CLI `90f1eea307e71f0f8924b2f180cb7999a11afd5b25a4667733c5d0b7e0da6985` 的 `background-label-local/`／`background-label-station/` 重跑原兩種右鍵流程，新增獨立的背景選單非空標籤及本地化設定文字斷言，並保留實際視窗 PNG；all-target Clippy與fmt通過。這個較晚 payload未被說成前述帳號案例的相同 executable。

負向證據分開保留。`terminal-first`是新探針漏加 terminal letterbox offset而點錯列；`terminal-point-fix`是自製推文時間欄未切回標準 ANSI白色，修正 fixture後才有標準推文 metadata，沒有放寬辨識。`terminal-final`／`click-diagnostic`則是上述真實雙擊失去第二次放開，兩份來源 patch及紀錄可重建。建置曾拒絕探針誤用不存在的 frame_count欄位，修正後才重建。所有失敗均未列為通過，沒有因退出／清理錯誤把失敗當成成功。

清理明細為同目錄 `cleanup-owned.json`：移除被最終案例取代的成功探針、對應 log／run metadata及中間成功 patch，保留失敗來源、最終 PNG／結果與已封存的建置拒絕紀錄；精確路徑、UID、symlink、內容與 lsof盤點一併記錄。du合計2224KiB，filesystem free由243301842944到243304148992 bytes，變化2306048 bytes，分開記帳而不把其他並行空間變化歸因於本次。仍使用的 Cargo profiles、未整合 worktree與提交保留，沒有移動 canonical develop、推送或發佈。

接續書籤、快速前往／最近看板、推文導覽／更新與完整混合功能矩陣；可信 OS手勢／IME、四架構包、舊資料往返與同負載效能仍待完成。S0–S8 goal維持 active，S9未切換／發佈。


## S4／S7 書籤管理與離線閱讀（2026-11-09）

`e704fc1267e345d30744a9109cd6fd97976aec92` 將 Dioxus adapter 的書籤狀態、檔案交易草稿、離線投影、Acquisition 與 FromList 移至 [共用 session](../src/bookmarks/session.rs)。Dioxus 只保留命令送出和 Feedback 對應，兩個狀態測試隨擁有者移動，直接消費者的檔案測試核對 Result。格式仍是既有 v1／v2、500 筆／32 MiB、單篇 4096 行／8 MiB，沒有另建 Slint 格式或放寬壞檔驗證。

`577f927` 新增 [原生管理 owner](../src/bin/slint_research/bookmarks_manager.rs) 與 [視窗](../src/bin/slint_research/bookmarks.slint)。研究版在自己的 data_local_dir 使用同一 Store。第一次開啟才由既有 Output 單一檔案 worker 載入；編輯、移除、重試及讀取共用一個結果槽，沒有另一個 reader、檔案 queue 或 idle polling。有效模型只提交已成功保存的結果。關閉讀取撤銷 epoch，晚到結果不開窗；關閉時仍在保存則等待結果，失敗保留表單修改。App Quit 經既有退出 owner 隱藏管理視窗、join worker 並收集結果。

未選取時清單明示「新增書籤」，保存 identity 與表單目的地各自管理；移除跟隨選取書籤。清單標示可用的本機副本。離線閱讀只接收已驗證 Archive 的當前 Rules 投影，沒有 Connection、站端 Stamp 或媒體下載器。沿用同一 Renderer、80×23 viewport、22 行重疊翻頁，以及方向鍵、Home／End、空白及滾輪；只快取目前頁的 raster，回表單或關閉會釋放閱讀模型和 raster。manager-open 會暫停主畫面輸入、保活與媒體互動。文章收藏的身分取得雖已共用，原生的目前文章／列表文章收藏、前往及最近看板仍待接線，沒有從畫面猜 AID。

完整元件以 [probe](../src/bin/slint_research/bookmarks_manager/probe.rs) 和本機嚴格 peer 驗證。最新普通 CLI SHA-256 是 `8d5da509de93cff69c37ccfd0a2558128815337d3d1cdd9bb62ef996b39f1ce6`；`.local/verification/slint-bookmarks/accepted/` 的三個程序分別覆蓋：

- 表單新增／大小寫去重／更名、編輯目的地後移除保存 identity、45 行離線副本、當前整行遮罩及中文左右不同色、80×23／22 行重疊、實際 toolkit PageDown、Home／End、備份失敗後保留末字與重試、取消後晚到讀取、壞檔唯讀及明確修復重載。
- 新程序讀取上一程序保存的副本，先正常斷線，再從浮球進入離線閱讀；沒有重連，資料時間與 45 行不變。
- 新程序在明確保存已啟動、尚未完成時正常 App Quit；關窗後獨立讀檔核對「退出前保存末字」、副本及 worker／結果槽／視窗／閱讀模型均已清理。

三個程序均嚴格 Close acknowledged，合成站台沒有額外按鍵，PNG／GIF／Once HTTP 全為零且 worker joined。普通浮球的本機／站端滑鼠與名單表單亦以同一 CLI 單獨重跑通過。文章內右鍵維持內容選單；一般 Unknown 背景開浮球，兩種路徑各有獨立情境。這些是 toolkit／本機 peer 證據，並非可信 OS 輸入或 S8 效能數據。可在 NEW_DIR 重跑第一組，再以 `restart`／`save-exit` 與其保存檔重跑後兩組：

```sh
PTTSTELLA_SLINT_BOOKMARKS_PROBE=1 target/release/pttstella-slint --probe-media --verify-media NEW_DIR
PTTSTELLA_SLINT_BOOKMARKS_PROBE=restart PTTSTELLA_SLINT_BOOKMARKS_SOURCE=NEW_DIR/bookmarks.json target/release/pttstella-slint --probe-media --verify-media NEW_RESTART_DIR
PTTSTELLA_SLINT_BOOKMARKS_PROBE=save-exit PTTSTELLA_SLINT_BOOKMARKS_SOURCE=NEW_DIR/bookmarks.json target/release/pttstella-slint --probe-media --verify-media NEW_EXIT_DIR
```

macOS arm64 實際 CUA 有三個分立檢查點。第一包發現原型 Tools 的分類出版、選取及鍵盤路徑仍寫死 0／2／3，收藏無法開啟；已改為單一 category_available，依已掛載 owner 決定。第二包驗證繁中貼上保存、閱讀、PageDown／Home、重開及管理窗中 ⌘Q，另觀察未選取卻顯示第一項及舊讀取提示。最後 `Native-Bookmarks-Current.app` 的 input 是 `87847b881efd0a9df057bd9241792f772d597e059f99f7d725dc5d4e248d3e2b`、簽署後 SHA-256 是 `8ee14d7e319f3fbeff27ae2e12a40796104e06489f6cf78faf959c3eef8907ca`：實際浮球開管理，新增狀態清楚、選取副本、⌘A／貼上「最終書籤驗收末字」、明確保存、PageDown 1–23 → 23–45、Escape 返回已清除讀取提示。浮球設定分類也能交接實際設定視窗，從設定 ⌘Q 後 App inventory 已無該 App。獨立 JSON 核對完整名稱、created_at=100、45 行及前一代備份，Close ACK 與零 HTTP／join 分列。LaunchServices exit code 沒有直接觀測。這份 OS 包之後只修改兩個 probe 的背景案例；普通最新 CLI 與此 OS 包的 hash 分開記錄。

負向紀錄均保留：第一個 bookmark flag 未掛載 article owner，錯跑一般 media verifier，exit 0 不算書籤通過，現在缺 owner 直接拒絕；斷線後的過早合成浮球操作失敗，補上新畫面 ACK 及前景就緒等待；舊 Tools probe 仍預期文章背景開浮球，依既有內容選單優先契約修正並補 Unknown 背景案例。另外一次與其他自有原生探針重疊時失敗，同一 binary 單獨重跑通過；只能推定可能受焦點干擾，沒有證明原因或更改正式失焦保護。最後 OS getApp 首次 timeout，但 inventory 顯示程序已啟動，選取既有 bundle ID 成功，沒有重新啟動同一輸出。編譯時的 consumer Result、Slint scope 及型別錯誤亦保留原 log。

驗證：Slint release lib／bin／multi_instance 合計 448 passed、6 ignored；Dioxus 同組合計 498 passed、6 ignored；兩個 feature 的 all-target Clippy、fmt、native-video all-target check、383 組 JS 回歸及文件 gate 通過。Dioxus Clippy 的既有第三方 block 0.1.6 future-incompat 提示另記，不是本專案警告。未新增 Windows／Linux／Intel 原生結果，未驗 OS IME 候選或完整資料回退，沒有因繁中貼上通過宣稱舊草稿末字異常已修復。

本輪 exact cleanup、程序與來源重放記錄在 `.local/verification/slint-bookmarks/`。`cleanup-owned.json` 逐項列出移除的 30 個路徑，包含兩個已被取代的 App、冗餘成功輸出及已封存的三份 RTK 原 log；檢查 resolved path、symlink、UID、內容與開啟 handle 後移除。du 為 132,316 KiB；filesystem free 243,219,046,400 → 243,354,611,712 bytes，兩者分開，不將並行釋放空間全歸因本輪。最新必要 App、原生資料、成功與獨特失敗證據保留。debug／release profiles 留供 active goal 後續共用；worktree 尚未整合，亦保留。完整 S0–S8 goal 繼續；S9 未切換、整合或發佈。


## S4／S7 快速前往與最近看板（2026-11-09）

`576f40e4d28d2b2fa8ed70b6195664a351dc61c6` 將 [最近看板狀態](../src/recent_boards/session.rs) 及 [待寫佇列](../src/recent_boards/writer.rs) 移到 desktop-services。Dioxus 保留 Files 的 enqueue adapter；packet 直接序列化同一 Snapshot，原五個狀態／writer 測試隨擁有者移動。兩個前端共用 Session 確認的 BoardVisit、generation／serial 去重、最多 20 筆、4 KiB 格式、最新待寫合併，以及失敗後仍保留的清除屏障。

`42e86a9e95435f7fd1ab6a83dbebdf8ca8177987` 新增 [原生快速前往](../src/bin/slint_research/destination/mod.rs)、[視窗](../src/bin/slint_research/destination.slint) 及 [最近看板的原生排程](../src/bin/slint_research/destination/recent.rs)。浮球導覽、⌘K／Ctrl+K、最近項目與書籤前往均只建立本機草稿；選取最近項目不送鍵，書籤前往跟隨保存的 identity，未提交的目的地修改不改變交接目標。非組字的明確 Enter／按鈕才要求開始，超過板名限制的草稿保留給 Rust 拒絕，總長仍在 trim 前限制 256 bytes。取消清空草稿；舊 request ID 的 receipt 不會關閉新視窗。

原生 ArticleSession 的同一 receipt 欄位接納 Destination，沒有第二個 UI 命令 queue 或送鍵 task。沒有 article scope 的選單／列表不能撤銷這種 receipt；成功只表示開始，實際到達仍讀 Session 的 Progress。立即 Reached 即使先於 receipt 被觀察，也會在收取 receipt 後重新核對，不留下永久 busy。切站、手動操作、取消與晚到結果維持原 operation／generation 的隔離，Session 的每步 5 秒／全程 30 秒與辨識流程不變。原生輔助窗的 Enter／Space／Esc／Tab 實體邊緣亦交給既有 modal hold guard，避免關窗後同一按鍵續送 BBS。

最近資料在 event loop 啟動前由已 join 的 blocking worker 載入。保存與互動式 picker 分離，只有一個 native file worker 及共用 Queue 中的一份最新待寫資料。thread、結果槽與 UI waiter 分別有 owner；正常退出先 drain，backend 錯誤出口也會 join／收取，沒有 detached 保存。到訪觀察只複製小型 BoardVisit，不再複製 terminal Frame；隱藏的導覽窗不重建 UI model。保存失敗只改最近清單的訊息，不把已成功切板改判失敗，也不自動重送失敗 snapshot。

`.local/verification/slint-destination/component-audited/` 的兩個完整 App 使用普通 CLI SHA-256 `bd8e8dab27c30ec2d77e5e89602e0c48092e42b64c835da57957f2ead5c6b733`，來源為 `576f40e` 加 `source.patch`，patch SHA-256 `aabced1b8557db73b5e9211f0311fef29d78bc7c62587c1894e0582c8e8a4f98`。獨立嚴格 peer 核對既有完整流程：

- boards：未送出的取消、過長／外站 URL 拒絕且草稿保留、組字期間不提交、Enter 到 Stella、同板立即 Reached 而零額外鍵、最近項目只填草稿、到 Other、最近順序、主檔及備份清空、備份失敗後原檔不變且訊息可見、明確重試、書籤編輯目的地後仍交接保存 identity、Beta 工作取消及 3.3 秒晚到畫面不續送。最後明確建立一份待收集工作及一份待寫 clear；不再 await 就要求退出，關窗後核對最新 clear 的結果已收取、兩檔均空、receipt／視窗／worker／pending 均釋放。
- articles：先後由 AID 及真實語法的 PTT URL 開啟兩篇，保留 Q 資訊核對等完整步驟；第三篇不存在時停止，只有測試中明確的使用者空白鍵才回到列表。第四篇送出 Q（input epoch=22）後取消，延遲資訊回來不再續送。

兩個 App 都 Close acknowledged、零媒體 HTTP 且 worker joined。相同普通 CLI 另通過原有完整文章取頁、文章遮罩／輸出，以及三程序書籤管理／新程序斷線讀取／退出保存回歸。合成長文原有延遲未修改；程序 elapsed 只是驗收紀錄，沒有當成 S8 效能量測。來源與結果在暫停／繼續後再次核對 hash，沒有重新啟動已完成的程序。

```sh
PTTSTELLA_SLINT_DESTINATION_PROBE=boards target/release/pttstella-slint --probe-media --verify-media NEW_BOARDS_DIR
PTTSTELLA_SLINT_DESTINATION_PROBE=articles target/release/pttstella-slint --probe-media --verify-media NEW_ARTICLES_DIR
```

macOS arm64 的 `Native-Destination.app` input 同為上述 CLI，ad-hoc signed SHA-256 為 `69e3be7e414918c0acc863f18f22917b4b37d982c702a8f2e1ab6510b4854948`。實際 CUA 從 root 欄位 ⌘K 打開並聚焦草稿，貼 Stella、按住 Return 700ms，主畫面顯示已到達；再次明確送同板立即到達且沒有舊 busy，再以按鈕前往 Other。原生 popup 顯示 Other／Stella 的順序，選 Other 只填草稿；明確清除後顯示主檔與備份已清除。最後輸入 Beta、按前往後立即 ⌘Q，後續 App inventory 已無程序，嚴格 peer Close ACK、所有 HTTP 零且 joined；獨立 JSON 核對兩份檔案都為 version 1 空清單，close marker 證明 receipt、視窗及 recent worker 已釋放。LaunchServices exit code 未直接觀測。700ms hold 沒有量到實際 repeat event 數，不據此宣稱原生 IME／重複鍵矩陣通過；OS AID／URL、書籤 Go 與其他平台也未以這份 OS 包驗收。

Rust release Slint lib／bin／multi_instance 合計 453 passed、6 ignored；Dioxus 同組合計 498 passed、6 ignored；recent suite 11 passed。兩 feature 的 all-target Clippy、fmt、native-video host all-target check 與 383 組 JS 通過；Dioxus 既有 block 0.1.6 future-incompat note 保留。原生型別檢查時的 private diagnostic getter 與 collapsible-if 編譯拒絕原 log 已封存，沒有列為 runtime 通過。

本輪 `cleanup-owned.json` 列出移除的 13 個 exact paths：被較完整案例取代的第一輪成功輸出／來源副本及已封存的 RTK 原 log。先核對 resolved path、symlink、UID、內容及 lsof；du 264 KiB，filesystem free 187,958,575,104 → 187,958,853,632 bytes，兩者分開，不將其他任務釋放量歸入。最新 App、完整元件／OS／直接 consumer 的證據與唯一失敗 log 保留。active goal 仍會使用 debug／release profiles，worktree 尚未整合，均保留。

目前／列表文章收藏仍須接原生 Acquisition／FromList 的全篇及身分核對；推文導覽／更新、混合 Live、四架構原生／IME、資料回退與效能仍未結案。完整 S0–S8 goal active，S9 未切換、整合或發佈。


## S4 目前與列表文章收藏（2026-11-09）

`502cdb798949b297a2b832acc3ad4ff72fcae97c` 在獨立 Slint worktree 接上目前文章及指定列表標題的收藏。共用的 [Acquisition／FromList](../src/bookmarks/session.rs) 與 [SurfaceState](../src/surface_state.rs) 仍擁有完整全文、原始列、核對結果及保存條件；原生 adapter 只租用已確認的 presentation／stamp／article 或 clicked cell，經既有 ArticleSession 的單一 command receipt 發出 typed Refresh／ListArticle。沒有另建連線、取頁器、ANSI parser 或站端 writer。

目前文章限 Web、PTT、本機滑鼠路由、完整首行至結尾並有實際畫面 ACK；先 pin Arc 原始列，再由 Q／AID／板名、前三行及尾頁重入核對。初次需要讀入書籤時，沿用有界單一 file worker，完成後重驗原 lease，不開對話框或改用新來源。列表標題保存實際 clicked row，作者／網址／選取及圖片維持共同 policy 的優先權；不退回游標另一列。FromList 等同一 Session 完成身分核對與同一全文 loader 的完整 ACK，再用第二次目前文章核對，結果必須等於列表取得的 target。35秒只限初始列表導覽，取得 target 後解除這個 timer，後續取頁仍用既有 loader 的期限／容量；目前核對為30秒。原生新增的 timer 有 owner、弱 callback、取消與退出條件，沒有輪詢 task。

浮球收藏分類分開提供「文章書籤」、「收藏目前文章」及核對期間的取消；列表標題右鍵使用共同 bookmark-list action，閱讀背景可收藏目前文章。失焦、raw handoff、modal／選取／組字、世代／operation／scope 改變、錯誤或明確取消會撤銷尚未寫入的 acquisition；不把收到舊回覆當成成功。通過核對後才交既有 ArchiveWrite／Store 原子交易，檔案交易開始後收集其實際結果，退出也 join，沒有假裝取消可以回滾已完成的寫入。離線 reader 仍只持 LocalArticle 與 renderer，80×23／22行翻頁，不持有站端 stamp、送鍵或媒體下載路徑。

證據在 worktree `.local/verification/slint-capture/`。最終 CLI SHA-256 為 `bb761185fe120165ff4d6a4ce53b25dc9f5fc93b448b96d54facbf3fce396b50`；相對 `9d4cefb` 的來源 patch SHA-256 為 `2b42641fa4edb76eb6f533884e2394bccfb30bda1fc0f3a8e1203396c31431b0`，原生 Git 以獨立 index 重放 tree，與 feature commit 的 `f61f63f107d2b2d536265a54b6d859a641a7e90e` 相同，最後核對不改 real index。初次 audit 以 git write-tree 讀 expected 而更新 native cache-tree，造成 index bytes assertion 失敗；已改以 committed tree 作唯讀比較，保留這項工具紀錄，沒有改寫 feature commit。

重跑方式（NEW_DIR 必須尚不存在）：

```sh
PTTSTELLA_SLINT_CAPTURE_PROBE=current target/release/pttstella-slint --probe-media --verify-media NEW_CURRENT_DIR
PTTSTELLA_SLINT_CAPTURE_PROBE=headerless target/release/pttstella-slint --probe-media --verify-media NEW_HEADERLESS_DIR
PTTSTELLA_SLINT_CAPTURE_PROBE=list target/release/pttstella-slint --probe-media --verify-media NEW_LIST_DIR
PTTSTELLA_SLINT_CAPTURE_PROBE=cancel target/release/pttstella-slint --probe-media --verify-media NEW_CANCEL_DIR
PTTSTELLA_SLINT_CAPTURE_PROBE=blur target/release/pttstella-slint --probe-media --verify-media NEW_BLUR_DIR
```

最終五案全數 exit0、Close acknowledged，檔案 worker／timer／window 清理通過，HTTP 全為零且 server joined。目前／無標頭逐筆核對9步；列表初始光棒在第二列，真正 Slint TouchArea 的 toolkit 右鍵指定第一列，拒絕舊 stamp 與非標題 cell，再核對4＋9步並保存對應的無標頭45列。成功案另外以新 Store 讀回唯一 `#00000101@Stella`、45行與原始末列 `REPLY 0045`，無標頭保留前三行且 label 為空，再從本機 reader 到23～45行。取消／失焦在 Q 送出後停住，等待6.3秒涵蓋晚到回覆，沒有 bookmarks 檔、續送或下載。這些是 toolkit／synthetic foreground 的完整元件驗證，不冒充實體 OS 右鍵或候選窗。

同一最終 CLI 的直接消費者回歸包含快速前往 boards／articles、完整20頁全文與遮罩／輸出，以及既有書籤管理、全新程序斷線重開與 save-exit。全部最終成功並 join；全文仍恰有19筆取頁，保留500ms＋150ms／頁的合成延遲，13.86秒的本輪案例 wall time 不代表 S8 效能。Rust Slint 453 passed／6 ignored、desktop 498 passed／6 ignored，兩入口 all-target Clippy、fmt、host native-video all-target check、JavaScript383項通過；desktop 仍僅有既有 block 0.1.6 future-incompat note。一般測試沒有連實站、讀取私人帳密或送出文章。

macOS arm64 的 `Native-Capture.app` 為 strict ad-hoc、未公證的本機診斷包，簽章後 native SHA-256 `73848e79536314cdea5ce4bd147dc97e8ed8ec6da90d59a18092131f436aac8f`。CUA 實際點文章取得前景，全文45行後由浮球「收藏 → 收藏目前文章」核對保存；再選清單的該筆本機副本，實際 PageDown 到23～45行，OS screenshot 可見末行 REPLY 0045 與推文色彩。Escape 返回、關閉對話框、Cmd+Q 後 inventory 已無 App；獨立 JSON 核對45行、正確板名／AID，peer9步、Close ACK、HTTP0與清理 marker 皆吻合。LaunchServices 沒有提供 exit code，不把 absence 當成 exit0。IMK mach-port warning 留在 log，不據此推定 IME 結果。

負向與限制分開保留。第一個驗證目錄預建造成 AlreadyExists；新 probe 漏掛文章 owner，在未開始核對時被 fail closed，已修正 fixture 接線。初次原始末行 expected 漏空白，檔案實際為 `REPLY 0045`，修正獨立 expected；列表曾把 article-number 的 end=8 當成標題邊界，改用 title_start 至實際欄寬，最終 TouchArea 指定列通過。第一輪無標頭在第二次 Q 前中止，加入診斷後最終 binary 完整重跑通過，未推定取消原因或聲稱修好焦點問題。書籤 restart 一輪 toolkit PageDown 未移至22，原 binary 隔離重跑及 save-exit 通過，失敗保留而未改 production guard。

另外兩個新程序皆已將先前實際收藏的資料由共用 Store 解碼並保存到各自獨立路徑，解碼結果逐欄相同、45行及原時間戳保留；但原生工具顯示輸入 AX focus 後，浮球仍 disabled，第一次還有裁切／noWindowsAvailable。改用自己的 in-bundle Mach-O 也未完成原生重開操作，兩次均由 Cmd+Q 正常關閉、Close ACK、零站端 bytes／HTTP 並確認 absence。這兩個 UI 範圍保持 not_ready；不能由已知有其他操作就判定原因。列表實體右鍵、這個新程序原生重開、完整焦點／IME、跨平台、混合 Live 及 S8 效能仍待驗。舊草稿末字異常也沒有因此結案。

本批驗收後檢查 resolved path、UID、symlink、內容雜湊、lsof 與已退出程序，刪除11個精確自有過時路徑（列於 cleanup-owned.json）：初次空目錄、被最終結果取代的5組正向 payload、兩個重開實驗 App 及三個重複 runner。重開包的最小 launcher／plist／簽章紀錄保留，重複 native Mach-O 與仍保留的 Native-Capture.app 雜湊完全相同；獨特負向資料／log 未刪。du 合計65,740 KiB；filesystem free 219,341,672,448 → 219,409,010,688 bytes，觀察差額67,338,240 bytes，未把其他工作並行釋放都歸因本批。獨立 replay index49,200 bytes 另刪並記錄，沒有刪 lock。保留最新 App、最終案例與回歸、唯一失敗證據，以及 active goal 會沿用的 target/debug／release；未清其他工作產物或停止他人程序。

canonical develop 仍只領先 origin/develop 一筆及原未提交計畫；本批只在已授權 detached Slint worktree 提交，沒有 push、PR、整合、正式入口切換或發佈。接續推文導覽／更新與其餘 S0–S8 缺項，S9 仍另行核可。

## S4 原生推文定位與身分側標（2026-11-09）

Slint 直接消費共用 `reply_navigation::Index`，不重新判斷原始 bytes、原PO 或登入帳號。ArticleSession 擁有同 generation／article scope 的群組與目前列，ArticleView 只計算實際已確認幾何的可見側標及本機置中捲動。按原PO／自己／目前標記的人只改群組，上一則／下一則依可見錨點循環；沒有新增站端 writer、下載或 timer。原生背景右鍵提供推文定位，帳號字詞選單仍優先提供帳號操作。

側標不進 Rust frame、PNG 或複製結果。作者金色、自己青色，兩者皆符合時分列細軌；只在外側留白足夠時顯示完整角色字樣與連推段首，否則用既有保留空白欄細軌。只提供完整可見列，最多64個；圖片間隙、部分行與 DPI 共用 ArticleFlow。整列遮罩不保留索引／標記，明確還原後由同一 Rust 投影恢復；名單更新產生新 article scope 會關閉舊工具列。

已確認的同篇索引在重繪期間保留工具列版面，但 `ready` 必須取得目前 geometry ACK 才允許開啟／操作，避免顯示／隱藏按鈕導致反覆重排。開啟定位先取得來源租約，只有搜尋列確實開啟時才清理搜尋並交接。群組／前後／關閉按鈕與原生焦點接線，輸入法組字、失焦及原來源失效時不前進；按鍵 modal edge 保留到 keyup，避免關閉後 Enter／Escape 重複穿透至站端。關閉／切模式清除控制項、Rc 索引、側標與來源。

最終無修改 CLI SHA-256 `0429c1665289b24b62780ae33e73fdba5b13e5192201eb982f77a8e5eacbf427`。`.local/verification/slint-reply-navigation/component-accepted/` 完整 App toolkit 測試核對 author `[3,4,10,18]`、Session 確認 mine `[6,14]`、focused `[7]`；實際 Slint 右鍵事件、群組不跳動、循環前後、搜尋交接、遮罩／還原、50%／200% 縮放與本機捲動、組字／背景拒絕、規則 scope 重設、關閉／重開與終端模式清理皆通過。strict peer 全程零站端輸入、Close ACK、HTTP0且 joined。擷圖已檢視：窄邊細軌與寬邊完整角色字樣；群組操作前後 Rust 匯出 frame 完全相同。

同來源 release native bin79項通過／1項明確 ignored，全 target Clippy `-D warnings`、`native-player-probe` 全 target check、fmt 通過。較早同批 lib367／bin79／multi10通過（共456／6 ignored）、JS383通過；最後只改 native adapter／probe，故不以重建 desktop 冒充 Dioxus host 驗證。直接消費者回歸：目前收藏9步、列表指定首篇13步、全文19次取頁及文章輸出皆 exit0／Close ACK；全文 fixture 的500ms＋每頁150ms延遲維持。列表第一次回歸在第3步前取消、相同 binary 獨立重跑通過，原因尚未確認，保留兩份紀錄，不以重跑覆蓋負向結果。

初輪編譯錯誤、閒置搜尋清理錯誤導致幾何 ACK 失效、規則 scope 與關閉重排後 probe 未等待 ACK等中間結果，分別保留來源／hash／log。最終另有重繪中保留 chrome 且禁止 stale 操作的明確測試。一次呼叫者預建 verification 目錄被入口 AlreadyExists 拒絕；影片 gate 曾使用不存在的 feature，改用 Cargo 已宣告的 `native-player-probe` 後通過；這些不列入成功。

macOS strict ad-hoc `Native-Replies.app` 可正常顯示23行合成文章與金／青側標。CUA 的 AX 文章欄點擊後再點「推文定位」仍失去前景可操作性；Raise／標題點擊未恢復，座標工具回報 `noWindowsAvailable`。因此實體按鈕、Tab、長按及 OS IME 操作仍 `not_ready`，不放寬生產焦點保護。首份固定輸出路徑 launcher 的重啟覆寫 log，不能證明該輪清理；後改成每次 invocation 的獨立目錄，兩份 retry log 均 Close ACK、strict peer零額外輸入、HTTP joined，CmdQ後 inventory及pgrep皆無本 App。退出結果與 UI 操作驗收分開。IMK warning 保留，不據此判定輸入法。

這批沒有整合、push、PR、正式入口切換或發佈。S4 推文更新、推文定位實體操作、列表實體右鍵／新程序離線操作與混合 Live 尚未結案；S0–S8 goal 持續。

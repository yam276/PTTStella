# Slint Windows x64 完整候選

## 2026-11-13 草稿工具列候選

便攜 ZIP 位於 `.local/verification/slint-draft-history/pair/windows/`，離線 Setup 在 `../PTTStella-candidates/windows-b57eae8-offline/`。與兩個 Mac 和 Linux 候選同取 `b57eae8`。ZIP SHA-256 為 `c305cf67e43880444acb86fd315551f529a6d6e70ef9af00cab5613c61791cc2`，Setup 為 `4bef56bc362b244af8c3742739051de1b9ccdadc91b82bedf7f3709ba9328998`。核對 PE 匯入、ZIP CRC、925 項便攜內容及 Setup 解包的 926 項逐位元一致。原生安裝、升級、卸載及草稿 IME 待驗。

## 2026-11-13 草稿與文章被動驗收候選

便攜 ZIP 位於 `.local/verification/slint-passive-drafts-reading/pair/windows/`，與同層 Apple Silicon DMG 都使用乾淨提交 `675b150`。ZIP SHA-256 為 `dad3410a6ac28d4116144b6245c68eff737f0ab223beac9a3a88a10ea58f0915`，包內主程式為 `cd56f91dddd2347e14c58fdf43a12a9b1cdee2c18cacc9c0a6f9662805db0001`。已核對 CRC、925 份內容與 PE 匯入。

同提交的離線 Setup 位於 `../PTTStella-candidates/windows-675b150-offline/`，SHA-256 為 `d613195a93a52bfa17ee689ccd5050c58fcf3a2e265915eb91ead165819d586c`。沿用 Inno Setup 7.1.0 和獨立的目前使用者安裝識別，解包後 926 個檔案與便攜版逐位元一致，另核對 Setup 的 AMD64 PE 和系統 DLL 匯入。

自動草稿及文章驗收以 toolkit 事件操作測試視窗。手動 `Run-Drafts-IME.cmd` 仍用原生輸入，可照既有方式測注音和日文。Windows 原生離線安裝、升級、卸載與 IME 待驗。保留 `9376627` 的 Setup 供升級對照。[Apple Silicon 驗收](slint-evaluation-plan.md#2026-11-13-草稿與文章被動視窗驗收)

## 2026-11-13 文章逐行對齊候選

便攜 ZIP 在 `.local/verification/slint-article-arrow-alignment/pair/windows/`，離線 Setup 在 `../PTTStella-candidates/windows-9376627-offline/`。兩包和 Apple Silicon DMG 都使用同一乾淨提交 `9376627`。ZIP SHA-256 為 `e23dac113c48648b93c8ee383e943327c61e18f7bb416b0e04a421d2a1501f44`，Setup 為 `21900bdf0261e94d3b05445d2cedbdf8f90b39de3c7e047fa6cc5ccafbd0f9b5`。

已核對 ZIP CRC、925 份 payload 與 PE 匯入。以固定 Inno Setup 7.1.0 編譯並解包核對，包括 `candidate.json` 在內的 926 個檔案與 ZIP 逐位元一致。沿用包內 Visual C++ runtime 和獨立安裝識別。保留 `4d7d174` Setup 供升級對照，原生離線安裝、升級、卸載與 IME 待驗。

## 2026-11-13 收藏載入焦點修正的離線 Setup

Windows x64 離線 Setup、便攜 ZIP 與 Apple Silicon 候選均使用 `4d7d174`。Setup 在 `../PTTStella-candidates/windows-4d7d174-offline/`，便攜 ZIP 在 `.local/verification/slint-current-shell/pair-bookmark-load/windows/`。安裝識別與目前使用者安裝位置沿用前版。

| 檔案 | SHA-256 |
| --- | --- |
| Windows Setup | `3be68a3376404a2a9ff191206032ac4a551e9609691802ca144aad6e4a581db9` |
| Windows 便攜 ZIP | `ed392e4d15360bcc90ea32a89a3ed8dc5de4dbb7e64d0a3059c5ec2cee2d9189` |
| Windows 包內執行檔 | `a9cac3cc47f729db470bb241cd031d5669e6d500144b2f6708a329bf4971803b` |

以固定 Inno Setup 7.1.0 編譯。Setup 解包後與便攜 ZIP 的 926 個檔案逐位元一致，包含 `candidate.json`。Setup 的 AMD64 PE 一般與延遲匯入均為 Windows 系統 DLL。原生離線安裝、升級、卸載與 IME 待驗，保留 `d37c7fb` Setup 供升級對照。封裝與核對收據在 `.local/verification/slint-current-shell/windows-setup-bookmark-load/`。

## 2026-11-13 浮球與收藏修正的離線 Setup

Windows x64 Setup、便攜 ZIP 與 Apple Silicon 候選均對應 `d37c7fb`。Setup 位於 `../PTTStella-candidates/windows-d37c7fb-offline/`，便攜 ZIP 在 `.local/verification/slint-current-shell/pair-focus/windows/`。安裝程式沿用前版的獨立安裝識別與目前使用者安裝位置。

| 檔案 | SHA-256 |
| --- | --- |
| Windows Setup | `b8c969c1fab3e795e2e0e53a360c9d3773af15e0a3df62187c8eac97672a4138` |
| Windows 便攜 ZIP | `1c8dea0687d7d8e3a707b16f65f6ed938d53cfa219e8cb8310ab411f75d968bf` |
| Windows 包內執行檔 | `4c9abc18a632e80958934de3c2c65b08113e1ed9380b1474b8940bde9c8de425` |

使用固定 Inno Setup 7.1.0 編譯，解包逐檔核對 926 個檔案，包含 `candidate.json`。Setup 的 AMD64 PE 一般與延遲匯入均為 Windows 系統 DLL。原生安裝、升級、卸載與 IME 待驗。保留 `4e28e82` Setup 供升級對照。編譯、解包與清理收據在 `.local/verification/slint-current-shell/windows-setup/`。

## 2026-11-12 草稿組字修正的離線 Setup

Setup 位於工作樹旁的 `../PTTStella-candidates/windows-4e28e82-offline/`，便攜 ZIP 仍在 `.local/verification/slint-editor-ime-edge/pair/windows/`。兩者與同層 `pair/macos/` 的 Apple Silicon 候選皆使用乾淨提交 `4e28e8244a3bf6c89d68caccabd26117ee001e0f`，含正文靠右組字的空間修正。

| 檔案 | SHA-256 |
| --- | --- |
| Windows Setup | `7c1b49a603f05ea568ba5055bcfa6b19c1ceb8b9e8fefea0279f3aea7f3579dd` |
| Windows 便攜 ZIP | `8e93341ab06a4dc7b5fe78ac46496955c18999e45ce34c2b1700489c47aaf449` |
| Windows 包內執行檔 | `08f8de0a39014f33ab75af22123d5bde25c1dc7d8eb72371f2dc48fa96b3dfc0` |

以固定 Inno Setup 7.1.0 直接封裝已核對的 ZIP 內容。解包比對 926 個檔案與原 ZIP 逐位元相同，包含 `candidate.json`。Setup 的 x64 PE 匯入為 Windows 系統 DLL，程式與 app-local CRT 另核對 83 個匯入符號。Setup 保留 `100b75f` 候選的獨立安裝識別和目前使用者安裝路徑。編譯、解包、逐檔雜湊、CRT 核對及清理紀錄在 `.local/verification/slint-windows-ime-setup/`，固定編譯器安裝檔保留在 `target/pttstella-installer-tools/` 供重跑。

原生驗收可在隔離 Windows 帳戶或 VM 先裝 `100b75f`。用安裝目錄的 `Show-Profile.cmd` 確認研究版路徑，在該資料目錄放一個合成檢查檔。再離線執行 `4e28e82` Setup，核對開始選單入口及安裝目錄中 `pttstella-slint.exe` 的 SHA-256。從安裝目錄執行 `Run-Offline.cmd` 與 `Run-Drafts-IME.cmd`，測本機合成連線、草稿保存與重開。卸載後核對候選程式和開始選單入口已移除、研究資料中的合成檢查檔仍在。Windows 原生安裝、升級、卸載及實體候選窗位置均待驗。

## 2026-11-12 離線 Setup 與便攜 ZIP

兩包位於工作樹旁的 `../PTTStella-candidates/windows-100b75f-offline/`。主程式沿用已與 Apple Silicon 成對的乾淨 `100b75f67e1fcd34f79c6531865a6eebe1272bb3`，SHA-256 仍為 `31c60eb3f16a05d08abed4cef47bc83d00b3438fd4f156ce8a99ec70eb926f1f`。增加三個 app-local DLL（`msvcp140.dll`、`vcruntime140.dll`、`vcruntime140_1.dll`）和其來源清單，其他應用程式檔案逐檔一致。Setup 與 ZIP 核對 926 個檔案逐位元相同，包含 `candidate.json`。PE 匯入核對只需要包內 DLL 和 Windows 系統 DLL，另比對 83 個 Visual C++ 匯入符號。

| 檔案 | SHA-256 |
| --- | --- |
| `PTTStella-Slint-Candidate-0.5.0-dev.3-windows-x64-setup.exe` | `41a3933712423079feb1d09c8a002b027473aacffbe6aeeae335dd748ba5606c` |
| `PTTStella-Slint-Candidate-0.5.0-dev.3-windows-x64.zip` | `c116ef2940a0b3fc8906a9db6464a1ce53e5ccb4a199a2786069836c348767bc` |

Setup 以現有固定 Inno Setup 7.1.0 編譯。獨立安裝識別 `io.github.yam276.pttstella.slint.candidate`，放在 `%LOCALAPPDATA%\Programs\PTTStella Slint Candidate`，開始選單有獨立入口。以同一使用者安裝，不需額外前置程式或線上下載。安裝目錄固定，安裝完成後由使用者開啟。正式 Dioxus 安裝、資料與憑證回退留待正式身分演練。

隔離且無外部網路的 Wine 10 成功執行解包後的 `--show-profile`，載入三個隨附 DLL，回報研究版資料路徑與憑證範圍。Setup 的一般執行及指定 Dioxus 路徑兩案均在 Inno 的平台版本檢查停止，未執行候選安裝目錄檢查。軟體繪圖格線探針在 Slint 的更新率計算遇到除以零，未執行後續媒體探針。Windows 原生安裝、升級、卸載、路徑拒絕及 GUI 仍待驗。

重跑時，向 `tooling/bundle_slint_windows.py` 傳入既有候選建置紀錄、素材及 libmpv，再加 `--installer`、各 app-local DLL 的 `--runtime-dll`，以及 `--crt-provenance`。Inno 入口使用 `INNO_SETUP_COMPILER`。CRT 來源是 Microsoft Visual Studio 2022 的可重散發 VSIX，包內紀錄 URL、VSIX 與 DLL 雜湊。Microsoft CDN 回傳的目錄內容與 channel 宣告的大小／摘要有差異，實際 VSIX 的 SHA-256 與目錄及下載 URL 相符，兩組目錄摘要一併保留。靜態 CRT 試作的 Skia 預建組合回傳 HTTP 404，改沿用已驗主程式。[Microsoft app-local DLL 說明](https://learn.microsoft.com/en-us/cpp/windows/determining-which-dlls-to-redistribute)

逐檔比對、Wine 原始紀錄及清理收據位於 `.local/verification/slint-windows-setup/`。本輪移除專用 Wine 環境、解包副本與重複封裝，保留固定 Cargo target、獨特診斷及先前供原生驗收的候選。

## 2026-11-12 文內圖片無障礙同步候選

Windows x64 ZIP 與 Apple Silicon App／DMG 位於 `.local/verification/slint-current-gif-comparison/pair/`，同取乾淨提交 `100b75f67e1fcd34f79c6531865a6eebe1272bb3`。Windows 以既有 cargo-xwin 和 libmpv DLL 建置，CRC、921 項內容雜湊及 PE x64 一般／延遲匯入已核對。原 Apple Silicon ZIP 於保留 App／DMG 並核對所有 ZIP 內容後清理。

| 檔案 | SHA-256 |
| --- | --- |
| `PTTStella-Slint-Candidate-0.5.0-dev.3-windows-x64.zip` | `f551d0f40dc395d7bf2db12fa4a09a908ff4910d445910c2f6f2e4594fe9c69b` |
| 包內 `pttstella-slint.exe` | `31c60eb3f16a05d08abed4cef47bc83d00b3438fd4f156ce8a99ec70eb926f1f` |

保留草稿 IME、影片與綜合驗收入口。Windows 原生圖片無障礙與草稿 IME 仍待實機驗收。原先交付的 Windows 包仍可照原步驟驗收，收據依包內執行檔雜湊歸檔。

## 2026-11-12 原生互動與影片同步候選

Windows x64 ZIP 與 Apple Silicon App／DMG／ZIP 位於 `.local/verification/slint-toolkit-input/pair/`，取自同一乾淨提交 `9c68e60005eee9346644dcc1d3572b24ab0ba9dc`。Windows 沿用 Apple Silicon 的 cargo-xwin 與已驗證的 libmpv DLL，CRC、921 項內容雜湊及 PE x64 一般／延遲匯入核對通過。

| 檔案 | SHA-256 |
| --- | --- |
| `PTTStella-Slint-Candidate-0.5.0-dev.3-windows-x64.zip` | `500f12585eb1fdb69686a3c3713ccb595e84e8412b38988b3e0ba235ae398dce` |
| 包內 `pttstella-slint.exe` | `3af23447817d9e8dc0aba2eeae63c72c03932bced6a2512c0b4395b96ccb7a5c` |

保留 `Check-All.cmd`、`Run-Drafts-IME.cmd` 和 `Run-Video.cmd`。本包的 Windows 原生結果待驗。原先交付的草稿 IME 包仍保留，結果以回報所用的執行檔雜湊歸檔。

## 2026-11-12 設定焦點與推文選單的同步候選

Windows x64 ZIP 在 `.local/verification/slint-native-interactions/pair/windows/`，與 Apple Silicon App／DMG 取自同一乾淨提交 `54964e98afa167275b1eb7bccbf12010340d95da`。沿用 Apple Silicon 的 cargo-xwin 與既有 libmpv DLL，CRC、921 項內容雜湊及 PE x64 一般／延遲匯入核對通過。

| 檔案 | SHA-256 |
| --- | --- |
| `PTTStella-Slint-Candidate-0.5.0-dev.3-windows-x64.zip` | `cc06a2a2cbbf52173f0aa57ca75b399fd0ccae1224357672c7f57f337eb5d96d` |
| 包內 `pttstella-slint.exe` | `5b0ae218fd896dff22273748d4737fb4b55e43969c9e8c26f970b0ec4868a923` |

這版修正設定搜尋進入內嵌黑白名單的焦點，保留 `Check-All.cmd`、`Run-Drafts-IME.cmd` 和 `Run-Video.cmd`。Windows 原生結果依實際執行檔雜湊歸檔，原先交付的草稿 IME 包仍可照原步驟驗收。[Apple Silicon 同源結果](slint-evaluation-plan.md#3-推文更新選單與設定s3s4s7)

## 2026-11-12 含原生影片控制的同步候選

新增音量與靜音的 Windows x64 ZIP 在 `.local/verification/slint-audio-pair/windows/`，與 Apple Silicon App／DMG 同取乾淨 `73fd1afd53e1d3f1a0b83f6c7271c769c7de8c0a`。沿用 macOS arm64 的 cargo-xwin 交叉編譯及既有 libmpv DLL。ZIP SHA-256 為 `e817412a549c55e579307ffd453351d64780ad588040817a1924a9b55c120351`，包內 exe 為 `513d68471155298379bb8191f955d80db5aa6ab432f37fd152c51fbea6359f52`。已核對 CRC、921 項內容雜湊、PE x64 與一般／延遲匯入。

在 Windows 解壓至新資料夾後，`Run-Video.cmd` 可開本機合成影片，驗收音量、靜音、恢復、進度與關窗後停止。草稿仍可照下一節的 `Run-Drafts-IME.cmd` 步驟驗收。兩個 Windows ZIP 都保留，各自依執行檔雜湊記錄實機結果。本包的原生結果待驗。[Apple Silicon 封裝驗證與回退](slint-store-frontends.md#2026-11-12-含影片控制的候選與-dioxus-回退包)

## 2026-11-12 草稿 IME 與文章翻頁同步候選

使用者稍後驗收的 Windows x64 ZIP 位於 `.local/verification/slint-draft-article-pair/windows/`，與該輪 Apple Silicon App 同取乾淨 `af4850648a2f99064a9f0c3194be9f292d5ddbe1`。macOS arm64 以 Rust 1.97.0、[cargo-xwin 0.23.1](https://github.com/rust-cross/cargo-xwin/releases/tag/v0.23.1) 及 LLD 交叉編譯 Windows，沿用已核對的 libmpv DLL。ZIP SHA-256 為 `599f94ecce001968848c801b88e1415952b1509d9d9badde1c67d5be81dd594e`，包內 exe 為 `f06b7329dcff8051c3b0de887c57a83273655454cb62e5d7b05fe02366f68f6f`。CRC、921 項內容雜湊、PE x64 與一般／延遲匯入均通過。

解壓至新資料夾，照 README 執行 `Run-Drafts-IME.cmd`。入口開本機合成看板，第一次建立「草稿輸入法驗收」，再執行會保留已存稿。在第三行用 Microsoft 注音組字並 Esc 取消，確認候選窗位置，再提交「中」後立即保存。切日文「**あ**」，輸入 `konnichiha` 提交「こんにちは」後立即關閉草稿。重開 App 再進同一稿，第三行應為「中こんにちは」，上方 ANSI 紅字保持。保留 `evidence/draft-ime/` 主檔、備份及兩次 `evidence/run-*.log`。這輪 Windows 實機留待稍後，前次原生回報仍以原 exe 雜湊歸檔。

## 2026-11-11 固定來源的 Windows 實機待驗包

最新測試 ZIP 位於 Slint worktree 的 `.local/verification/slint-cutover-rehearsal-b1085ab/windows/package/`。Windows x64 與同輪 Apple Silicon 演練固定來源 `b1085ab2c5b8397d100fcf2df9e28b6da79cb8ce`。從乾淨來源用 macOS arm64、Rust 1.97.0、cargo-xwin 0.23.1 及 LLD 交叉編譯 `x86_64-pc-windows-msvc`，沿用 worktree Cargo target。封裝沿用經 SHA-256 核對的 libmpv 0.42.0 LGPL DLL。

| 檔案 | SHA-256 |
| --- | --- |
| `PTTStella-Slint-Candidate-0.5.0-dev.3-windows-x64.zip` | `3e38bc06e9563bbdfc078f96b6142099badbd19291918c6aa4f38cac61c49801` |
| 包內 `pttstella-slint.exe` | `e4e5c8511fbbb5dc5c4eaced14cd1899a91ce34cde344df72f5aa15fc0e7b38b` |

ZIP CRC、執行檔與 DLL 雜湊、PE x64 及一般／延遲 import 檢查通過。import 清單沒有 WebView 相關 DLL，需要 Visual C++ x64 Redistributable 的 `msvcp140.dll` 及 `vcruntime140.dll`。解壓到新資料夾後執行 `Check-All.cmd` 與 `Run-IME.cmd`，保留 `evidence` 的收據。[同輪 Apple Silicon 演練](slint-store-frontends.md#2026-11-11-同一正式-bundle-id-的-apple-silicon-隔離演練)

實機回報 `Downloads/evidence(1)` 的來源與 exe SHA-256 均吻合。Windows 10 x64 build 19045、PowerShell 5.1 的 13 案有 12 案通過，整體為 `failed`。唯一失敗是 `settings-search` 的 `privacy` 結果未取得欄位 19 的焦點。原始 log 已收到唯一的 WebSocket Close ACK，外層腳本將探針的失敗狀態報成 `Strict peer Close failed`。修正共用頁首的焦點所有權後，原本已聚焦的頁首也會同步新的目標欄位。Apple Silicon 的中英文 48 個搜尋目標通過。設定搜尋驗完回到終端焦點後結束，圖片懸停保留在既有媒體探針。Windows 修正版待重驗。

使用者另回報切換 Microsoft 日文 IME 後，`Run-IME.cmd` 測試視窗打不出字。舊 log 有六次 Big5-UAO 編碼成功的提交，未記輸入法、組字或編碼失敗事件。新版測試視窗重新取得前景時恢復終端輸入焦點，Windows 會重新啟用該輸入欄的 IME。提交後底部顯示原生事件與編碼結果，日誌只記焦點、組字狀態及位元組數。合成測試確認「こんにちは」為 10 bytes、「中」為 2 bytes，含無法映射字元時仍顯示已收到原生提交。Microsoft 日文 IME 的實機結果待重驗。

日文重驗請選平假名「あ」，點第 22 行，輸入 `konnichiha` 並按 Enter。若底部未顯示提交，保留該次 `evidence/run-*.log`。本機結果與新版包存於 `.local/verification/slint-windows-ime-current/`。

## 舊版候選與實機回報

使用者待驗包保持 `.local/verification/slint-input-cross/windows-package/`，產品來源475897f（288份建置輸入），exe `69542cb7…`、ZIP `60f7c551…`；使用者會稍後驗收，原生仍not_run。既有13案與注音回報只屬舊04249364…exe，未轉填。此待驗包尚未包含後續字型／貼上與保留raster修正。

2026-11-11 另備新包於 `.local/verification/slint-renderer-api-current/cross-current/windows-package/`。乾淨 `ff0ae84` 交叉 build 及 all-target Clippy 通過。PE `3a8de2a9…` 含文章焦點及固定狀態列探針修正，`Check-All.cmd` 的兩個 shell 案例使用修正後的檢查。新版 ZIP SHA-256 為 `c4aed301277519179974c1736b0c593add5654ebe376e669baa31ef932f98175`。919 個 payload 中有 917 個與前一另備候選相同，執行檔和候選資訊已更新。已核對一般及延遲 PE imports，仍需 VC runtime，原生實機待驗。新版和 `69542cb7` 各有下載入口與獨立雜湊。[本輪封裝驗收](slint-evaluation-plan.md#2026-11-11-固定狀態列探針的-windows-與-linux-封裝)

這是 S8 內部驗收包，含現有 Rust 核心、原生 Slint 主程式與 libmpv，尚未切換正式 Dioxus 入口。已收到 Windows 10 x64 的原生元件證據；13 案經修正腳本後重驗既有回報通過。完整 IME 矩陣、DPI、可聽音訊、硬體解碼、乾淨安裝、資料回退與效能仍待驗（Microsoft 注音三項已有使用者人工回報），尚不能平替。包內 `native_acceptance: not_run` 是打包時狀態，後續實機結果見下節。

## 先前交付與編譯方法

當時交付包在 Slint worktree 的 `.local/verification/slint-windows-current/package-current/`：

| 檔案 | SHA-256 |
| --- | --- |
| `PTTStella-Slint-Candidate-0.5.0-dev.3-windows-x64.zip` | `c33737bc730cd6c3a176eef37a753a2e6b3bf24c230c8de4f923de1a5f275a7a` |
| 包內 `pttstella-slint.exe` | `04249364f01389873704de7166fd1ffc30f29bc83174aa12d3e7326f6c7caa91` |
| 包內 `libmpv-2.dll` | `94fb87247732c59915cd59808bc582e843a71c22214b7b6c1e5563aa8149cef4` |

Rust／Cargo 來源是乾淨提交 `c40f7a5`，已包含通知、正常影片退出、設定搜尋先決條件與系統動畫偏好接線。285份來源 fingerprint 在建置前後一致；主程式為66,856,448 bytes。打包器及批次檢查 helper 是本輪另行變更，`package-final-command.json` 保留當時的 dirty provenance 與兩個輸入檔hash，提交後再核對；不將後來的 commit 冒充建置時狀態。

這份 PE 是在 macOS arm64 用 Rust、cargo-xwin、Windows SDK 與 LLD **交叉編譯**，建置時沒有 Windows runner 執行；後續使用者提供的原生證據另列。沿用已安裝的專用 xwin SDK，將官方 libmpv 的原始 COFF import archive 以 `mpv.lib` 名稱交給 MSVC target 的 LLD；沒有把 macOS dylib 放進 Windows 包。完整命令、工具／環境與實際exit在本批 `cross-build.json`／`clippy.json`。精簡重現入口為：

```text
MPV_LIB_DIR=<已核對的 import archive 目錄> CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 cargo xwin build --release --locked --no-default-features --features native-player-probe --bin pttstella-slint --target x86_64-pc-windows-msvc --target-dir <本批自有絕對路徑>
```

libmpv 沿用已核對的 [mpv v0.42.0 官方 LGPL Windows x64 資產](https://github.com/mpv-player/mpv/releases/tag/v0.42.0)，DLL bytes 與前包相同；來源URL、archive／DLL hash及授權在 `licenses/libmpv-provenance.json`。這不是精確第三方對應來源的完整交付／重播，`source_delivery: not_ready` 保留。

PE 一般／延遲匯入已檢查：主程式是 AMD64 Windows GUI，直接連到 `libmpv-2.dll`，沒有 WebView／WebKit／Wry 匯入；仍需系統 `MSVCP140.dll`／`VCRUNTIME140.dll`，見 [Microsoft VC++ Redistributable 說明](https://learn.microsoft.com/en-us/cpp/windows/latest-supported-vc-redist/)。這只證明靜態及延遲匯入閉包，不能代替原生啟動、動態載入或無 WebView2 主機驗收。最低 OS 與缺服務／缺 GPU fallback 也仍需實測。

## 在 Windows 執行

核對 ZIP SHA-256，解壓到新的可寫資料夾，不覆蓋舊包。先執行 `Check-All.cmd`，無須帳密，僅啟動合成畫面、本機 BBS／HTTP fixture，不登入 PTT、不改 OS 動畫偏好。每次建立新的 `evidence/native-<UTC>-<GUID>/`，即時保存 `native-results.json` 與逐案log／receipt。不要將 loopback 的 toolkit 操作記成真正 OS 鍵鼠或 IME。

| 批次案例 | 必要結果 |
| --- | --- |
| software／default shell，2案 | shell與通知動作、hover／Tab／時序，以及 timer／deadline 退出 |
| software／default motion，2案 | 真實 Windows UISettings 初值、通知政策、observer移除與UI listener join；不切換 OS 值 |
| 版本檢查，1案 | 本機三次HTTP、重試／舊offer、停滯請求取消、worker／listener join |
| 設定搜尋，1案 | 兩語言48個不同目的欄位、草稿／隱私與視窗關閉 |
| 目前文章收藏，1案 | 真正保存結果、擷取／file worker／window 退出 |
| MP4／WebM／OGV各兩種renderer，6案 | 已進入播放、非空四色像素、renderer退休／session join；software走Quit，default走關窗 |

每案獨立等待退出，90秒逾時就記失敗；終止只針對自己啟動的PID，無法確認停止時保留PID並停止新增案例。stdout／stderr同步排空避免pipe卡死，退出後還有有界讀取等待。判定必須有exit0、恰好一筆對應fixture的嚴格Close ACK、非空且符合型別的receipt；影片另外依固定四色／30級容差重新核對像素，不能只相信`passed`欄位。

motion的來源確實不可用且正常清理時，單列 `not_ready`，不當成功。Windows事件要求Windows10 version2004／build19041以上。[Microsoft UISettings 事件要求](https://learn.microsoft.com/en-us/uwp/api/windows.ui.viewmanagement.uisettings.animationsenabledchanged?view=winrt-26100) 其他錯誤仍列failed。整組結果只有全部13案通過才是 `component_cases_passed_manual_pending`／exit0；有failed回1，有not_ready回2，不以全skip或部分成功冒充整體完成。

批次入口使用系統 `powershell.exe`，不修改執行原則。若本機不允許執行`.ps1`，保留 `evidence/checks-*.log`，可改用 `Check-Shell.cmd`／`Check-Shell-Software.cmd`、`Check-Motion.cmd`／`Check-Motion-Software.cmd`，各自保存結果，不必為驗收更改系統政策。這份script採PowerShell5.1可用語法／Process API；目前只在主機PowerShell7.6.9驗證parser與自製receipt fixtures，原版已在Windows PowerShell5.1實際執行；本次helper修正僅在主機PowerShell7.6.9重驗既有原生回報，尚無修正版在5.1的重跑。

手動原生驗收使用下列入口，結果另列，不能由批次推定：

| 入口 | 人工操作 |
| --- | --- |
| `Run-Offline.cmd` | 浮球／設定／關於、正常關窗、縮放、失焦／恢復 |
| `Run-Grid.cmd`／`Run-Grid-Software.cmd` | 80×24、雙色字、末列、100／125／150%、跨螢幕DPI、全螢幕 |
| `Run-IME.cmd` | Microsoft注音組字、候選位置、取消／選字／一次提交；檢查合成bytes |
| `Run-Reading.cmd` | 文章兩模式、捲動、尋找、選取／複製及快捷鍵 |
| `Run-Drafts.cmd` | 繁中與末字、保存、關窗、新程序讀回；合成稿在`evidence/drafts` |
| `Run-Video.cmd`／`Run-Video-Software.cmd` | 明確播放、可聽音訊、暫停／拖曳與關閉 |
| `Show-Profile.cmd` | 只列隔離資料／vault scope，不讀寫store |

正常雙擊exe會連研究設定的預設站（初始PTT WSS），不自動登入；上列合成入口不需要實站帳密。請回報Windows版本、顯示倍率、執行入口及`native-results.json`的status／失敗或not_ready案名。無WebView2的啟動需在原本沒有該runtime的環境另驗，不要求卸載日常使用的WebView2。密碼庫、舊資料回退、實站與同負載效能依 [完整計畫](slint-evaluation-plan.md) 另驗。

## 2026-11-10 Windows 實機回報與腳本修正

使用者提供 Downloads/evidence，透過 Finder 複製到 `.local/verification/slint-windows-current/evidence/`，原始 101 份檔案保持不變。OS 為 Windows 10 x64 build 19045，PowerShell 5.1.19041.6456；顯示倍率未提供。執行檔 hash 與交付包相符。

原始 `native-results.json` 為 failed：10 案 passed，三種格式的 default 影片都在驗收 helper 讀取不存在的 `frame_serial` 時失敗。這是 helper 將軟體 frame 的格式套到 OpenGL framebuffer 回報；三案原生程序皆 exit0、未逾時、已退出，四色像素、資源退休及 Close ACK 皆存在。原始 failed 不覆寫。

修正為依 `render_path` 驗證：software 要求正整數 frame serial／size；OpenGL 要求非負座標與正整數寬高的 rect。兩路都保留非空四色、固定 expected、30 級容差、正常退出／join 和嚴格 Close ACK。新增缺 serial／rect、零尺寸、負座標及未知路徑的反例。以修正版對全部 13 案原始回報與 log 離線重驗通過，記為 `existing_native_receipts_passed_manual_pending`；這不是重新執行 Windows 主程式，也不是人工操作通過。完整回報與原始檔 hash 在 `receipt-recheck.json`。

最新 ZIP 只更新 `Check-Native.ps1` 與相應 `candidate.json` hash，其餘 915 份 ZIP 項目保持逐 byte 一致，未重建 Rust。原包 ZIP hash `c6dac792bdfabf96e7384af9dea2cf7426e37d929a48f2873d6ce4a3fcc8f735`、錯誤 helper、manifest 與逐檔 hash 保存於 `runner-fix/`。舊 ZIP 與打包時暫時提取的 exe／DLL 已清除，僅留最新必要交付；exact paths、du、open-file 檢查及 filesystem free 前後在 `runner-fix/cleanup.json`。

這批證明實機啟動、shell、OS 動畫初值與 listener 清理、版本檢查、設定搜尋、收藏與三格式 software／OpenGL 影片元件。它不證明實體鍵鼠、IME、DPI、可聽音訊、硬體解碼、OS 動畫值變更、無 WebView2 主機、乾淨安裝、資料回退或效能；修正版 helper 的 Windows 5.1 重跑也另列待驗。

## 2026-11-10 Microsoft 注音人工回報

使用者依同一 Windows 包的 `Run-IME.cmd` 提示，回覆「三項都正常」：Microsoft 注音候選窗位置、Esc 取消與選字後單次提交。這三項列為 `human_reported_passed`，紀錄在 `.local/verification/slint-windows-current/user-ime-20261110.json`。

這是使用者的原生操作回報，並未另收到或讀取這一輪的 IME bytes log、截圖或新的執行檔 hash；不把它擴成已核對2 bytes、完整組字／快捷鍵／失焦／DPI矩陣，其他輸入法及音訊仍待驗。先前自動元件證據的 Windows 10 build19045／PowerShell5.1 與這份人工回覆分開保存，顯示倍率仍未提供。

## 本機證據與清理

Windows release build／all-target Clippy通過；Python166項包含PowerShell helper的正向及空trace、重複／錯fixture的Close、假布林、假OS來源、未join、空／錯像素與重複搜尋目的地反例。這些是host fixture／打包證據，沒有執行Windows主程式。最終ZIP916份payload逐檔hash一致，helper／launcher bytes與原始碼一致，校驗檔、build record與實際PE相符。

較早的docs gate與尚在產生的打包staging並行，正確攔到暫存DLL；保留失敗，待producer完成及清理後重跑，不放寬隔離規則。第一次actionlint只有PATH缺少工具，改用canonical checkout已安裝的精確工具路徑；不將未執行算passed。

已移除本批target（Cargo原生clean8237檔／2.8GiB，du2992204KiB）、重複DLL輸入、本批第一版包與被取代的舊Windows完整包；最終包從`package-final`正常rename為`package-current`。兩版必要metadata／授權封存1822項，沒有保留舊完整exe／DLL；2878份來源、asset、fixture、各平台最新包及必要來源hash不變。獨立無影片S1配置與自有mpv import archive仍有用途，另列保留。沒有新Rust重建，所有target都已移除；worktree du1377536KiB。filesystem available 211087757312→214248906752 bytes僅為並行觀測。exact paths、owner／resolved path／symlink／lsof、Cargo／du與保留理由見 `.local/verification/slint-windows-current/cleanup/ledger.json`。

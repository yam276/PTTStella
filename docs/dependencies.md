# 直接依賴與來源

下列是直接 Cargo 依賴；精確下載身分由 Cargo.lock 的 registry checksum 記錄。

| 依賴 | 用途 | 上游授權／來源 |
| --- | --- | --- |
| serde／serde_json | 產品 frame 與自己編寫的 replay fixture | MIT OR Apache-2.0；[serde](https://serde.rs/)、[serde_json](https://docs.rs/serde_json/) |
| zeroize 1.9.1／security-framework 3.7.0 | Rust 自有密碼 buffer 清除／macOS Keychain 安全 API | Apache-2.0 OR MIT；[zeroize](https://docs.rs/zeroize/1.9.1/)，MIT OR Apache-2.0；[security-framework](https://docs.rs/security-framework/3.7.0/) |
| uao-rs 0.1.0 | Big5-UAO 字元配對／輸入映射 | MIT；[上游](https://github.com/zenixls2/uao-rs)，其 README 記錄 MozTW UAO 來源 |
| url 2.5.8／webbrowser 1.2.4 | HTTP／HTTPS 網址解析與使用系統瀏覽器開啟 | MIT OR Apache-2.0；[url](https://docs.rs/url/2.5.8/)、[webbrowser](https://docs.rs/webbrowser/1.2.4/) |
| i18n-embed 0.16.0／i18n-embed-fl 0.10.1／rust-embed 8.13.0 | Fluent 翻譯載入、編譯期訊息／參數檢查及離線資源內嵌 | MIT；[i18n-embed](https://github.com/kellpossible/cargo-i18n)，MIT；[rust-embed](https://github.com/pyros2097/rust-embed) |
| fluent-syntax 0.12.0（dev） | 以官方 AST 驗證兩語訊息／參數一致，不自行剖析 Fluent 語法 | Apache-2.0 OR MIT；[上游](https://github.com/projectfluent/fluent-rs) |
| font-kit 0.14.3 | 背景列出 OS 已安裝字型家族；不複製字型或替代 WebView renderer | MIT OR Apache-2.0；[上游](https://github.com/servo/font-kit)、[API](https://docs.rs/font-kit/0.14.3/) |
| Dioxus 0.7.10 | Rust 桌面 UI 與系統 WebView | MIT OR Apache-2.0；[上游](https://github.com/DioxusLabs/dioxus) |
| Tokio／base64 | 非同步橋接、PNG 匯出與合成畫面 probe 的傳輸 | MIT；[Tokio](https://tokio.rs/)，MIT OR Apache-2.0；[base64](https://docs.rs/base64/) |
| rfd 0.17.2／tempfile 3.27.0 | 原生另存視窗與同目錄原子檔案替換 | MIT；[rfd](https://docs.rs/rfd/0.17.2/)，MIT OR Apache-2.0；[tempfile](https://docs.rs/tempfile/3.27.0/) |
| directories 6.0.0 | OS 使用者設定目錄 | MIT OR Apache-2.0；[ProjectDirs](https://docs.rs/directories/6.0.0/directories/struct.ProjectDirs.html) |
| tokio-tungstenite 0.28.0／futures-util | 單一 WebSocket 傳輸與序列化收送 | MIT；[tokio-tungstenite](https://docs.rs/tokio-tungstenite/0.28.0/)，MIT OR Apache-2.0；[futures-util](https://docs.rs/futures-util/) |
| rustls 0.23／rustls-native-certs 0.8 | TLS、系統信任根與明確指定的 ring provider | Apache-2.0 OR ISC OR MIT；[rustls](https://docs.rs/rustls/0.23.46/)，MIT OR Apache-2.0 OR ISC；[native-certs](https://docs.rs/rustls-native-certs/) |
| reqwest 0.13.5 | 可取消、有總期限的串流圖片 HTTP，不啟用 cookie／內容解壓縮 | MIT OR Apache-2.0；[上游文件](https://docs.rs/reqwest/0.13.5/)，rustls-no-provider 配合本產品明確 TLS 設定 |
| imagesize 0.15.0 | PNG／JPEG／GIF／WebP 的尺寸檢查，不解碼 pixels | MIT；[上游文件](https://docs.rs/imagesize/0.15.0/)，只啟用四種所需格式 |

uao-rs 的 decode 只餵固定兩個 bytes；encode 逐字往返核對，避免上游靜默省略無法編碼的內容。測試包含標準中文、UAO 符號與無法編碼的輸入；不宣稱全部映射已獨立驗證。

Rust 1.97.0 是本次選定並實測的工具鏈，不是證明可用的最低版本。程式不是系統 terminal，不載入 shell／PTY；只是 PTT 專用的格線與顯示控制碼模型。

[UI wire 範例](../tests/fixtures/ui-packets.json) 為本專案自行編寫、可隨產品散布的合成 expected；僅含虛構帳號／文字及 example.com 網址，沒有私人流量、帳密或外部產品資料。DOM／clock doubles 與 native UI probe 同樣使用自製資料；翻譯 runtime 的第三方來源另列於本文件。

[選取撤銷 wire](../tests/fixtures/selection-cleared.json) 同樣為獨立手寫的合成 expected，供 JS 與 Rust 分別核對。[純黑匯出 PNG](../tests/fixtures/export-black.png) 以 Python 標準函式庫 struct／zlib 依 PNG 格式自行產生：960×576、8-bit RGB、每列 filter 0、全黑 pixels、IHDR／IDAT／IEND。兩份均由本專案製作、可隨產品散布，無外部資產或私人內容；PNG 僅驗證非同步檔案工作及版本撤銷，不替代 renderer 的像素基準。

WSS 使用系統信任根、驗證站端憑證與主機名稱；TLS 設定屬於這條連線，不安裝 process-wide provider。原因是桌面相依套件可能同時啟用兩個 Rustls provider。端點為 `wss://ws.ptt.cc/bbs`；[PTT 官方公告](https://www.ptt.cc/bbs/SYSOP/M.1496571808.A.608.html) 記錄 WebSocket 服務，當前可用性由獨立實站探針驗證。

## App 附帶字型

- Cascadia Mono 2407.024：Microsoft 官方 v2407.24 release 的 `ttf/CascadiaMono.ttf`，714,804 bytes，未修改；[上游](https://github.com/microsoft/cascadia-code/releases/tag/v2407.24)、[OFL 1.1](../assets/fonts/Cascadia-OFL.txt)。
- Noto Sans CJK TC Regular 2.004：notofonts 官方 Sans2.004 的繁體中文臺灣 OTF，16,435,884 bytes，未修改；[上游](https://github.com/notofonts/noto-cjk/tree/Sans2.004)、[OFL 1.1](../assets/fonts/NotoSansCJK-OFL.txt)。

來源 URL、Noto Git blob 與兩檔 SHA-256 見 [資產 manifest](../assets/fonts/sources.json)，著作權見 [NOTICE](../assets/fonts/NOTICE.txt)。僅在 desktop 內編入，透過有界的固定資產路由載入；執行時不需下載。Microsoft JhengHei 不包含在產品中；[Microsoft FAQ](https://learn.microsoft.com/en-us/typography/fonts/font-faq) 說明 Windows 一般字型授權不包含隨 App 散布的權利。

## Fluent JavaScript

附帶未修改的官方 `@fluent/bundle` 0.19.1 standalone runtime，Apache-2.0；[上游](https://github.com/projectfluent/fluent.js)、[授權](../assets/vendor/fluent/LICENSE)、[下載來源／npm integrity／檔案 SHA-256](../assets/vendor/fluent/SOURCE.json)。僅隨程式離線載入，執行時沒有 CDN 或翻譯服務請求。`locales` 的繁體中文及英文文案為本專案自行編寫，Rust 與 JS 消費同一份 Fluent 資源。

[語言 bootstrap wire](../tests/fixtures/localization.json) 為獨立手寫的最小中英文例子，Rust 比對 serialization，JS 分別消費相同資源；不是由 serializer 產生 expected。

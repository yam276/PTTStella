# 直接依賴與來源

下列是直接 Cargo 依賴；精確下載身分由 Cargo.lock 的 registry checksum 記錄。

| 依賴 | 用途 | 上游授權／來源 |
| --- | --- | --- |
| serde／serde_json | 產品 frame 與自己編寫的 replay fixture | MIT OR Apache-2.0；[serde](https://serde.rs/)、[serde_json](https://docs.rs/serde_json/) |
| uao-rs 0.1.0 | Big5-UAO 字元配對／輸入映射 | MIT；[上游](https://github.com/zenixls2/uao-rs)，其 README 記錄 MozTW UAO 來源 |
| Dioxus 0.7.10 | Rust 桌面 UI 與系統 WebView | MIT OR Apache-2.0；[上游](https://github.com/DioxusLabs/dioxus) |
| Tokio／base64 | 非同步橋接與合成畫面 probe 的 PNG 傳輸 | MIT；[Tokio](https://tokio.rs/)，MIT OR Apache-2.0；[base64](https://docs.rs/base64/) |
| tokio-tungstenite 0.28.0／futures-util | 單一 WebSocket 傳輸與序列化收送 | MIT；[tokio-tungstenite](https://docs.rs/tokio-tungstenite/0.28.0/)，MIT OR Apache-2.0；[futures-util](https://docs.rs/futures-util/) |
| rustls 0.23／rustls-native-certs 0.8 | TLS、系統信任根與明確指定的 ring provider | Apache-2.0 OR ISC OR MIT；[rustls](https://docs.rs/rustls/0.23.46/)，MIT OR Apache-2.0 OR ISC；[native-certs](https://docs.rs/rustls-native-certs/) |

uao-rs 的 decode 只餵固定兩個 bytes；encode 逐字往返核對，避免上游靜默省略無法編碼的內容。測試包含標準中文、UAO 符號與無法編碼的輸入；不宣稱全部映射已獨立驗證。

Rust 1.97.0 是本次選定並實測的工具鏈，不是證明可用的最低版本。程式不是系統 terminal，不載入 shell／PTY；只是 PTT 專用的格線與顯示控制碼模型。

WSS 使用系統信任根、驗證站端憑證與主機名稱；TLS 設定屬於這條連線，不安裝 process-wide provider。原因是桌面相依套件可能同時啟用兩個 Rustls provider。端點為 `wss://ws.ptt.cc/bbs`；[PTT 官方公告](https://www.ptt.cc/bbs/SYSOP/M.1496571808.A.608.html) 記錄 WebSocket 服務，當前可用性由獨立實站探針驗證。

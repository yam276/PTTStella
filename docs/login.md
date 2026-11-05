# 按鈕登入與帳密保存

使用者明確選擇「記住帳密，按登入才送出」。按鈕登入只在 PTT 的 WSS 與 SSH 入口使用，兩者共用同一筆已存帳密。啟動與重連時由使用者在帳號提示按登入。

## 操作

工具列的「登入」開啟視窗。首次輸入帳號、密碼，預設不保存；明確勾選「將此帳號與密碼存入系統密碼庫」才寫入。保存的是一個預設帳號，下一次明確保存會替換這筆帳密。保存只表示系統密碼庫寫入完成，不代表 PTT 已接受密碼。

再次開啟可看見帳號，密碼欄保持空白；留空並按登入才取用已存密碼。改成另一帳號時必須重新輸入密碼，不會沿用其他帳號的密碼。「移除已存帳密」只移除本 App 的這筆密碼庫項目，不變更站端密碼，也不登出目前連線。未勾選且輸入密碼時完全不呼叫密碼庫；保存失敗不送出登入。密碼庫無法使用時，仍可輸入帳密取消勾選單次登入，或使用原始鍵盤手動登入。

## 平台密碼庫

| 平台 | 後端 | 條件 |
| --- | --- | --- |
| macOS | Keychain generic password | 保留既有 service／default 項目及 security-framework 實作，不搬移既有帳密。 |
| Windows | Credential Manager generic credential | 使用目前 Windows 使用者的認證庫；可從控制台的認證管理員查看／移除。 |
| Linux | Secret Service 預設 collection | 需要工作階段 D-Bus 與相容的桌面服務，例如 GNOME Keyring；依服務提示解鎖。沒有服務或拒絕存取時明確失敗，不降級至檔案。 |

Windows／Linux 使用鎖定的 [keyring 3.6.3](https://docs.rs/keyring/3.6.3/keyring/) 安全 API，明確建構原生後端，不依賴可被切換的全域 mock builder；Linux 啟用 Secret Service／Tokio／crypto-rust，僅在既有獨立 worker 使用其同步介面，D-Bus secret 使用 DH 加密 session，不退回 plain。兩平台均存入相同有界二進位帳號＋密碼資料；查詢屬性只有固定 service 與 default，PTT 帳號也包含在 secret 內。OS 密碼庫的保護取決於使用者的登入工作階段與服務設定，不是 App 的獨立密碼保險箱；Linux 的落盤加密與解鎖由服務負責。參考 [Microsoft 認證管理員](https://support.microsoft.com/en-us/windows/security/credential-manager-in-windows) 與 [Secret Service 規格](https://specifications.freedesktop.org/secret-service/latest/)。

後端錯誤只回傳固定分類，macOS 保留 OS status code，不回傳原始錯誤物件、payload 或屬性。讀取遇到缺服務、鎖定／拒絕、重複項目及損壞資料時，不冒充「沒有保存」或自動覆寫原資料。移除不存在的項目視為成功。各視窗仍只在使用者明確保存時替換唯一預設帳密；不自動合併不同密碼，也不將密碼存進共用設定。

## 開發版的重複系統授權

測試帳號的密碼由使用者在 App 保存至鑰匙圈；自動化只操作登入按鈕，讓 Rust 取用，毋須將密碼交給聊天、環境變數或一般檔案。若顯示 macOS 鑰匙圈授權視窗，要求的是該鑰匙圈的密碼，由使用者在系統視窗輸入；不要當成 PTT 密碼欄。

同一受信任版本可由使用者在系統提示選擇「始終允許」，見 [Apple 的存取選項](https://support.apple.com/en-euro/guide/keychain-access/kyca1243/mac)。目前打包腳本使用 ad-hoc 簽章，實際 designated requirement 綁定該程式的 cdhash；重新編譯後不能保證沿用前版授權。Apple [TN3127](https://developer.apple.com/documentation/technotes/tn3127-inside-code-signing-requirements) 說明 ad-hoc 身分綁定特定程式版本。跨版本減少提示須使用穩定的簽署身分及 bundle identifier，另驗更新後鑰匙圈存取；僅固定路徑或識別碼不足以證明已解決。本輪未建立憑證、修改鑰匙圈 ACL 或降低系統保護。

## 儲存與暫存

帳號與密碼一起放入 macOS Keychain 的一筆 generic password；不建立帳密 JSON、明文備份或自製加密檔，也不加入可攜顯示設定、PNG 或診斷報告。Keychain 提供加密儲存及 OS 的存取管理，見 [Apple Keychain 說明](https://developer.apple.com/documentation/security/using-the-keychain-to-manage-user-secrets)。以鎖定的 [security-framework 3.7.0 安全 API](https://docs.rs/security-framework/3.7.0/security_framework/passwords/index.html) 讀、寫及刪除，錯誤只顯示 OS status code，不輸出密碼或 credential payload。

只有開啟登入視窗、按登入或移除時才呼叫系統密碼庫；啟動不讀取。OS 若要求允許存取，由使用者操作系統提示；拒絕不降級成一般檔案。每次至多一個 native worker，平時不阻塞 BBS 收送，關閉及橋接錯誤仍持有並 join；系統權限提示無法由程式中止，需先完成或取消該提示。

密碼輸入欄使用 password 類型、停用 autocomplete，取消／送出／關閉時清空；已存密碼不回傳 WebView。第一次輸入的密碼經結構化事件送至 Rust，不拼接到 JavaScript。Rust 自有密碼 buffer 以 zeroize 清除，Credentials 不提供 Debug、Clone 或 Serialize。這不能保證清除 OS、WebView、TLS 及傳輸函式庫內的所有副本，也不能防止已遭入侵的使用者環境讀取執行中資料。

## 單次登入流程

Connection 是唯一送出者。按登入時核對 generation、revision、input epoch、連線狀態、沒有等待中的操作，以及目前游標位於空白帳號提示。只送帳號與 CR，等待同一連線第 22 列的空白密碼提示，且前一列已回顯此次帳號，才送密碼與 CR 一次。辨識使用核心已完成的 80×24 Frame，不在 JS 或傳輸 framing 重解原始串流。

帳號提示為第 21 列「請輸入代號，或以 guest 參觀，或以 new 註冊:」，密碼提示為第 22 列「請輸入您的密碼:」。游標前文字須完全符合、後方空白、該列可映射，已辨識的瀏覽畫面不接受；部分輸入、提示文字出現在其他列、未完成 ANSI 均拒絕。依 [PTT 登入列與收送](https://github.com/ptt/pttbbs/blob/908bb0631a4a6db41eac85713637adf1a6058fde/mbbsd/mbbsd.c#L610)、[密碼提示](https://github.com/ptt/pttbbs/blob/908bb0631a4a6db41eac85713637adf1a6058fde/include/common.h) 及 [帳號／密碼容量](https://github.com/ptt/pttbbs/blob/908bb0631a4a6db41eac85713637adf1a6058fde/include/pttstruct.h#L49) 製作自製測試，並非實站部署版本證明。帳號限 2–12 英數且英文開頭；訪客／註冊指令拒絕。密碼限 1–72 個可列印 ASCII，保留空格，不接受換行、ESC 或控制碼；不支援的內容可取消後用原始輸入操作。

等待最多 15 秒；逾時、取消、手動輸入、導覽、斷線或離開登入畫面時清除等待中的密碼，重連不恢復。帳號已送出不能回滾；取消只阻止後續密碼。送出密碼後不自動重試，也不自動回答重複登入、刪除其他連線、兩階段驗證、公告或任何後續確認。介面先報告「帳密已送出」，成功與否由站端畫面確認。

密碼送出後只保留最長 120 秒、無秘密資料的手動引導。依公開 [logind 提示](https://github.com/ptt/pttbbs/blob/68b88d8753d69000033037a0c84a4ee3596b937a/daemon/logind/logind.c) 與 [mbbsd 流程](https://github.com/ptt/pttbbs/blob/68b88d8753d6/mbbsd/mbbsd.c)，僅在完整 80×24 未辨識畫面、第 22 列精確空白 2FA 提示及對應游標位置時交還原始輸入；相容六位限時數字／八位救援碼的兩種空格格式，前後錯碼訊息須完全符合。此處依公開原始碼製作合成案例，不代表已確認實站部署。

辨識後切到 PTT 原始輸入，驗證碼由使用者輸入並按 Enter，不另建欄位、不保存碼或 TOTP secret，也不代答其他後續確認。錯碼時只提示自行重試；進入已辨識瀏覽畫面就結束引導，可按 Esc 回 Vim。返回帳號／密碼提示、取消、期限到達或斷線也停止引導，重連不恢復。120 秒是本機引導期限，並非站端驗證碼的有效期；逾時／取消後仍可原始操作，已送出內容不能回滾。工具列「取消登入」同時取消等待密碼與後續引導。

## 驗證邊界

一般 core gate 包含 login suite 的輸入容量／控制碼與提示正反例；desktop lib 用同一 production Connection 核對分段密碼提示、精確兩筆輸入、錯畫面／過期請求拒絕、手動取消、15 秒期限、晚到提示及重連不重播。

`--probe-login OUTPUT_DIRECTORY` 明確使用 owned loopback、獨立 WebView 資料目錄與專用 synthetic Keychain service，從不讀寫正常帳號項目。只使用自製帳密；peer 逐筆比對後必須收到 Close，任何額外輸入失敗。macOS 原生視窗驗證保存、重啟留空取用、錯帳號／非登入畫面拒絕及移除後重讀；產物報告不含帳密。實站、Windows／Linux 原生 UI、桌面解鎖及正式 Developer ID 更新後的存取結果各自驗收，不能以本機往返宣稱實站登入通過。

`--probe-surface OUTPUT_DIRECTORY login_2fa` 使用相同隔離環境與 production Connection。peer 只接受一次合成帳密、使用者明確輸入的六位錯碼與八位救援碼，接著只接受 Close；報告要求 UI 曾看到驗證／錯碼／瀏覽狀態，且明確 Esc 後回到 Vim Normal。這不代替實站 2FA 或原生 IME 驗收。


## 閱讀時的帳號身分

Session另持有有界、無密碼的AccountIdentity，僅供網頁文章標示「自己」的推文。標準密碼提示前一列必須精確回顯合法帳號，之後同generation實際辨識到主選單才確認；沒有候選、訪客、其他站點、部分控制碼、文章中的同名文字及新generation均不能確認。按鈕登入與手動登入消費同一來源，不讀鑰匙圈帳號作為登入成功證據，不自動登入，也不將帳號加入診斷事件。空帳號提示、斷線及stop會清除，連線View不保留已結束session的身分。

## 密碼庫回歸與獨立驗證

`credential-store` feature 可在沒有 WebView／GTK 的環境編譯同一生產密碼庫；desktop 自動包含此 feature。`cargo test --locked --features credential-store --lib login::vault` 使用每測試獨立記憶體 store 驗證同一 consent reducer，涵蓋不勾選、留空取用、替換／不替換、帳號不符、超量／損壞資料、保存失敗不產生 Ready 及移除失敗保留。一般測試不讀寫使用者密碼庫。

明確執行 `cargo test --locked --features credential-store --lib login::vault::tests::native_roundtrip -- --ignored --exact` 才測真正 OS 後端：獨立唯一 synthetic service、固定合成帳密、不勾選零保存、勾選寫入、另一個程序讀回、錯帳號拒絕、兩次移除與確認不存在。可能需要使用者處理 OS 解鎖提示；不使用正式 service，不連 PTT。Linux 可在獨立容器的 D-Bus／GNOME Keyring 中驗證，不改使用者桌面密碼庫。`native_read_child` 僅由父程序啟動。

Linux 缺服務另明確執行：`DBUS_SESSION_BUS_ADDRESS=unix:path=/nonexistent-pttstella-synthetic-bus cargo test --locked --features credential-store --lib login::vault::tests::native_service_unavailable -- --ignored --exact`，需確認讀取與勾選保存失敗，但不勾選單次登入仍能準備帳密。交叉編譯、記憶體測試與隔離容器不能冒充 Windows 認證庫、Linux 真實桌面解鎖或三平台實站登入。

GitHub 的 Desktop Release 在打包前使用相同測試。Windows runner 直接操作認證管理員。Linux runner 建立獨立的 D-Bus 工作階段和暫存 GNOME Keyring，並另外執行缺服務案例。

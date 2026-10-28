# 按鈕登入與帳密保存

使用者明確選擇「記住帳密，按登入才送出」。正常啟動只連到固定的 `wss://ws.ptt.cc/bbs`；不在啟動、重連、失敗或返回帳號提示時自動送帳密。

## 操作

macOS 工具列的「登入」開啟視窗。首次輸入帳號、密碼，預設不保存；明確勾選「將此帳號與密碼存入 macOS 鑰匙圈」才寫入。保存的是一個預設帳號，下一次明確保存會替換這筆帳密。保存只表示鑰匙圈寫入完成，不代表 PTT 已接受密碼。

再次開啟可看見帳號，密碼欄保持空白；留空並按登入才取用已存密碼。改成另一帳號時必須重新輸入密碼，不會沿用其他帳號的密碼。「移除已存帳密」只移除本 App 的這筆鑰匙圈項目，不變更站端密碼，也不登出目前連線。其他平台尚未提供安全儲存，保留原始鍵盤手動登入。

## 開發版的重複系統授權

測試帳號的密碼由使用者在 App 保存至鑰匙圈；自動化只操作登入按鈕，讓 Rust 取用，毋須將密碼交給聊天、環境變數或一般檔案。若顯示 macOS 鑰匙圈授權視窗，要求的是該鑰匙圈的密碼，由使用者在系統視窗輸入；不要當成 PTT 密碼欄。

同一受信任版本可由使用者在系統提示選擇「始終允許」，見 [Apple 的存取選項](https://support.apple.com/en-euro/guide/keychain-access/kyca1243/mac)。目前打包腳本使用 ad-hoc 簽章，實際 designated requirement 綁定該程式的 cdhash；重新編譯後不能保證沿用前版授權。Apple [TN3127](https://developer.apple.com/documentation/technotes/tn3127-inside-code-signing-requirements) 說明 ad-hoc 身分綁定特定程式版本。跨版本減少提示須使用穩定的簽署身分及 bundle identifier，另驗更新後鑰匙圈存取；僅固定路徑或識別碼不足以證明已解決。本輪未建立憑證、修改鑰匙圈 ACL 或降低系統保護。

## 儲存與暫存

帳號與密碼一起放入 macOS Keychain 的一筆 generic password；不建立帳密 JSON、明文備份或自製加密檔，也不加入可攜顯示設定、PNG 或診斷報告。Keychain 提供加密儲存及 OS 的存取管理，見 [Apple Keychain 說明](https://developer.apple.com/documentation/security/using-the-keychain-to-manage-user-secrets)。以鎖定的 [security-framework 3.7.0 安全 API](https://docs.rs/security-framework/3.7.0/security_framework/passwords/index.html) 讀、寫及刪除，錯誤只顯示 OS status code，不輸出密碼或 credential payload。

只有開啟登入視窗、按登入或移除時才呼叫 Keychain；啟動不讀取。OS 若要求允許存取，由使用者操作系統提示；拒絕不降級成一般檔案。每次至多一個 native worker，平時不阻塞 BBS 收送，關閉及橋接錯誤仍持有並 join；系統權限提示無法由程式中止，需先完成或取消該提示。

密碼輸入欄使用 password 類型、停用 autocomplete，取消／送出／關閉時清空；已存密碼不回傳 WebView。第一次輸入的密碼經結構化事件送至 Rust，不拼接到 JavaScript。Rust 自有密碼 buffer 以 zeroize 清除，Credentials 不提供 Debug、Clone 或 Serialize。這不能保證清除 OS、WebView、TLS 及傳輸函式庫內的所有副本，也不能防止已遭入侵的使用者環境讀取執行中資料。

## 單次登入流程

Connection 是唯一送出者。按登入時核對 generation、revision、input epoch、連線狀態、沒有等待中的操作，以及目前游標位於空白帳號提示。只送帳號與 CR，等待同一連線第 22 列的空白密碼提示，且前一列已回顯此次帳號，才送密碼與 CR 一次。辨識使用核心已完成的 80×24 Frame，不在 JS 或傳輸 framing 重解原始串流。

帳號提示為第 21 列「請輸入代號，或以 guest 參觀，或以 new 註冊:」，密碼提示為第 22 列「請輸入您的密碼:」。游標前文字須完全符合、後方空白、該列可映射，已辨識的瀏覽畫面不接受；部分輸入、提示文字出現在其他列、未完成 ANSI 均拒絕。依 [PTT 登入列與收送](https://github.com/ptt/pttbbs/blob/908bb0631a4a6db41eac85713637adf1a6058fde/mbbsd/mbbsd.c#L610)、[密碼提示](https://github.com/ptt/pttbbs/blob/908bb0631a4a6db41eac85713637adf1a6058fde/include/common.h) 及 [帳號／密碼容量](https://github.com/ptt/pttbbs/blob/908bb0631a4a6db41eac85713637adf1a6058fde/include/pttstruct.h#L49) 製作自製測試，並非實站部署版本證明。帳號限 2–12 英數且英文開頭；訪客／註冊指令拒絕。密碼限 1–72 個可列印 ASCII，保留空格，不接受換行、ESC 或控制碼；不支援的內容可取消後用原始輸入操作。

等待最多 15 秒；逾時、取消、手動輸入、導覽、斷線或離開登入畫面時清除等待中的密碼，重連不恢復。帳號已送出不能回滾；取消只阻止後續密碼。送出密碼後不自動重試，也不自動回答重複登入、刪除其他連線、兩階段驗證、公告或任何後續確認。介面先報告「帳密已送出」，成功與否由站端畫面確認。

密碼送出後只保留最長 120 秒、無秘密資料的手動引導。依公開 [logind 提示](https://github.com/ptt/pttbbs/blob/68b88d8753d69000033037a0c84a4ee3596b937a/daemon/logind/logind.c) 與 [mbbsd 流程](https://github.com/ptt/pttbbs/blob/68b88d8753d6/mbbsd/mbbsd.c)，僅在完整 80×24 未辨識畫面、第 22 列精確空白 2FA 提示及對應游標位置時交還原始輸入；相容六位限時數字／八位救援碼的兩種空格格式，前後錯碼訊息須完全符合。此處依公開原始碼製作合成案例，不代表已確認實站部署。

辨識後切到 PTT 原始輸入，驗證碼由使用者輸入並按 Enter，不另建欄位、不保存碼或 TOTP secret，也不代答其他後續確認。錯碼時只提示自行重試；進入已辨識瀏覽畫面就結束引導，可按 Esc 回 Vim。返回帳號／密碼提示、取消、期限到達或斷線也停止引導，重連不恢復。120 秒是本機引導期限，並非站端驗證碼的有效期；逾時／取消後仍可原始操作，已送出內容不能回滾。工具列「取消登入」同時取消等待密碼與後續引導。

## 驗證邊界

一般 core gate 包含 login suite 的輸入容量／控制碼與提示正反例；desktop lib 用同一 production Connection 核對分段密碼提示、精確兩筆輸入、錯畫面／過期請求拒絕、手動取消、15 秒期限、晚到提示及重連不重播。

`--probe-login OUTPUT_DIRECTORY` 明確使用 owned loopback、獨立 WebView 資料目錄與專用 synthetic Keychain service，從不讀寫正常帳號項目。只使用自製帳密；peer 逐筆比對後必須收到 Close，任何額外輸入失敗。macOS 原生視窗驗證保存、重啟留空取用、錯帳號／非登入畫面拒絕及移除後重讀；產物報告不含帳密。實站、Windows／Linux 安全儲存及正式 Developer ID 更新後的存取結果各自驗收，不能以本機往返宣稱實站登入通過。

`--probe-surface OUTPUT_DIRECTORY login_2fa` 使用相同隔離環境與 production Connection。peer 只接受一次合成帳密、使用者明確輸入的六位錯碼與八位救援碼，接著只接受 Close；報告要求 UI 曾看到驗證／錯碼／瀏覽狀態，且明確 Esc 後回到 Vim Normal。這不代替實站 2FA 或原生 IME 驗收。

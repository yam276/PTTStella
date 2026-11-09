# Slint 通知時序與錯誤優先

此項是 [Slint 評估計畫](slint-evaluation-plan.md) S7 的局部驗收，不代表 S7 或平替候選整體完成。

## 行為與來源

`assets/notification-policy.json` 是兩個前端共用的政策：複製成功保留 2000ms、一般成功保留 3000ms，再淡出 500ms；錯誤與未分類提示沒有期限。可見錯誤拒絕晚到的 copy／success，直到使用者關閉或另一則持續提示／錯誤取代。分類由操作結果指定，不比對 Fluent 翻譯文字。Dioxus 的 DOM timer 沿用原有事件實作，讀取同一政策；沒有新增 WebView 依賴。

`src/notification.rs` 管理原生時序；`src/bin/slint_research/notice.rs` 持有唯一 Slint timer、弱 root、單一文字與當前 callback ticket，沒有通知佇列或 idle polling。相同文字重新發布也重設期限，舊 timer ticket 不能清除新通知。滑鼠或通知按鈕焦點任一仍在時，閱讀與淡出時鐘都暫停，畫面恢復不透明；兩者都離開才用剩餘時間恢復。減少動態效果時保留閱讀時間、略過淡出；偏好在已暫停的淡出期間改變，等互動結束才清除。

主畫面的連線／斷線、session warning 與登入等待仍由各自 owner 保持可見。通知浮層不占 terminal viewport，成功淡出不會把這些持續狀態一起藏掉。自動到期不轉移鍵盤焦點；明確按通知關閉按鈕才在原有 scope 允許且按鈕持有焦點時回復終端。程式關閉先撤銷當前期限，退出完成後停止 owner 並檢查 timer 已停；若 Exit 無法排程而恢復視窗，通知仍能回報錯誤並允許稍後重試。

文字／ANSI、網址與 PNG 複製、PNG／圖片保存、文章收藏、設定檔匯出及站端上色在來源端指定成功類型。輸入、I/O、連線與幾何失敗指定 error。設定及名單自動保存的進度／成功留在管理視窗；保存或套用失敗仍是持續錯誤。草稿帶回 PTT 後的確認發表指示保持 persistent，避免指示自動消失。媒體預覽既有 inline status 沿用媒體 owner，不冒充主畫面通知。

## 驗證與邊界

本批證據位於 `.local/verification/slint-notification/`，來源基準是 `9a13990ee1a7719ac46160054c2bcbfdfc9d0a57` 加本批 patch。`src/notification/tests.rs` 使用獨立時間／預期，驗證固定期限、錯誤優先、重複文字、hover／focus 分立暫停、淡出暫停、動態偏好切換及 teardown。既有 Dioxus 通知八項案例與完整 JS suite 分開驗證，不能以純 reducer 測試代替視窗事件。

原生檢查入口：

```text
pttstella-slint --software-renderer --verify-shell <新的證據目錄>
pttstella-slint --verify-shell <新的證據目錄>
```

這兩案使用完整 production owners／同一本機嚴格 peer。`notice_probe.rs` 經 Slint toolkit 移動指標至標準按鈕、以 Tab 遍歷取得通知焦點，再分別離開 hover／焦點，核對 timer 及剩餘時序。另驗相同文字重新計時、錯誤不被晚到成功／舊期限清除、減少動態效果、未知提示無 timer、自動消失保留焦點。關窗前刻意留下真實執行中的通知 timer，退出後 `notice-closed.json` 才驗停止；`notice-result.json` 的 pending 狀態必須連同退出 receipt、exit 0 與 Close ACK 才能算通過。

這是 macOS arm64 的 toolkit／合成 foreground 證據，不是原生 OS 鍵鼠、VoiceOver、IME、Windows／Linux 或其他架構驗收。`tools-reduced-motion` 目前仍沿用原型保守的 true 預設；探針顯式切換兩個值。尚未接上三平台的即時系統偏好，所以不能宣稱系統動畫偏好移植完成。macOS 的正式接線應讀取 [NSWorkspace 的 reduced-motion 屬性及變更通知](https://developer.apple.com/documentation/appkit/nsworkspace/accessibilitydisplayshouldreducemotion)，Linux 的跨桌面標準可參考 [Settings portal](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.Settings.html)；舊 portal／缺服務的可用性與 Windows 原生值仍須各自實測。

效能比較、完整可及性／modal 矩陣、四架構來源同步及資料回退仍依 active plan 驗收。本批測試的刻意等待不代表文章載入或正常啟動變慢。完成後依 [產物清理流程](development-harness.md#產物清理與結案) 清掉自有 target、被替換包及重複正向圖片，保留最新必要候選與最小證據，不重建 Rust 來證明清理。

擴大回歸時，首個 settings-search 案在第二語言的 privacy 目標未取得焦點；程序正常收到 Close，但案例為 failed。相同候選重跑以及通知變更前的 `9a13990` 候選各自通過兩語言 48 個目標，不能由此推定首次失敗原因或把間歇問題結案。首次失敗 log、兩版 SHA／命令與重跑結果均保留在本批證據，完整設定焦點驗收仍待追查。另一個封裝檢查失敗是 verifier 誤比對 `otool` 首行輸入檔名；排除檔名標頭後 51 個 Mach-O 的 load-command 內容均無 developer prefix，App／ZIP／唯讀 DMG 的 payload 一致，沒有為該誤報修改 loader。

最後的四行版面修正讓隱藏按鈕列不再保留 32px 高度，並重新建置／Clippy／封裝。最終 arm64 原始 CLI SHA256 為 `9c023276b560e5cba7bc60bb4f281f2996f252e2ffdb0f1663a2e797192370a1`，封裝 executable 為 `b5e82341b7c7797fae98a911e696d96374aa969bb33a6732d36d336f4fb891ce`，ZIP 為 `2fcef6b3ae88153470bc7dbfd84a4715e58c1bf05030e0d78f44ee4483325f95`，DMG 為 `941eaf901ea9319abcab4a3041a56aa036b28f47f08ab1fc02ee71cd23e3ea74`。`layout-acceptance.json` 的五案包含 software／OpenGL shell、版本檢查、設定搜尋及目前文章收藏，皆以拒讀 Homebrew 的封裝 App 執行，exit 0、嚴格 Close ACK 和各自的 owner 退出／結果 receipt 已逐項核對。最終搜尋兩語言 48 目標通過，仍不把先前間歇失敗結案。1176 份 App／ZIP／唯讀 DMG payload 相符，51 個 Mach-O 均為 arm64，ad-hoc 簽章有效但未公證。

Rust reducer／主程式測試分別 382／83 passed（4／1 ignored 另列）；Dioxus 配置的 library／binary／multi-instance 分別 377／121／10 passed（4／1／1 ignored 另列），兩配置 all-target Clippy、core gate 14 命令／2 replay、383 個 JS、165 個 Python 及文件檢查通過。最後變更只有上述 Slint 版面，Rust 測試所覆蓋的同一 Rust／JS 政策未再改動，版面以最後的 Clippy／五個完整視窗案例及實際 snapshot 覆核。其他三架構仍是較早來源的候選，尚未同步此次通知變更，不能宣稱四架構同版驗收。

# Slint 版本檢查與退出回歸

Slint 設定的「關於與授權」已接入既有 `updates::Checker`：正常 Live 啟動查一次正式版，也可按檢查；有新版本時才提供下載入口，下載與安裝維持人工。HTTP失敗不宣稱最新、不顯示錯誤提示，保留先前有效提示並可手動重試，沿用Dioxus既有契約。本批在macOS15.7.7 arm64驗證，不代表整個S7／S8完成。

## 實作邊界

- 共用[版本檢查器](../src/updates.rs)維持SemVer、排除draft／prerelease、8秒／256KiB上限、無redirect及不採用遠端JSON的URL。下載與repository按鈕沒有UI傳入URL參數，只由Rust選取固定來源。
- Native worker使用既有Tokio runtime、一個有界命令槽及一份最新watch狀態，不新增執行緒、idle poll或自動重試。重複按鍵不啟動第二請求；關窗先撤銷排隊意圖，再取消HTTP並等待worker與Slint狀態listener真正退出。非正常backend退出無法證明listener join時回報清理未完成，不以清空handle當成功。
- 設定頁保留查詢中、可下載、最新／較新開發版與靜默失敗狀態；語言由現有Fluent投影。離頁、組字、設定已關閉或app正在退出時拒絕操作，版本功能不改設定檔或送BBS bytes。
- 一般合成／benchmark不建立公開版本檢查工作。明確的`releases`案例只接受帶port的IPv4 HTTP loopback，忽略ambient proxy且重試仍用同一來源；它不是可設定的產品更新server。每個client沿用明確rustls provider；正式TLS載入與信任值維持原樣，沒有安裝全域provider。

## 發現並修正的退出問題

加入更新owner的非同步join後，完整視窗測試已完成版本操作卻在退出45秒後逾時。實際取樣觀測CPU98.1%，主執行緒堆疊集中在 `Video::step → close → Notify`。當root消失、尚未進到影片shutdown時，閒置watcher每次step又叫close喚醒自己。現在在root消失時一次標記closed並撤銷，交給原watcher完成或排空，避免自喚醒迴圈；普通預覽關閉與仍存在的視窗不因此變成永久closed。

最小反向重現patch、原45秒失敗、取樣及來源／binary hash保留於 `.local/verification/slint-release-check/`。反向patch重現行為，不宣稱重建原失敗binary的每個byte；舊完整執行檔不保留。第一輪新fixture未明配rustls provider造成的兩個卡住測試及後續明確panic也保留，已修正並為fixture等待加deadline。

## 驗證與重跑

合成版本檢查使用實際SettingsWindow、正式Checker／worker及正常App Quit路徑。HTTP peer獨立要求三次請求：啟動回覆含惡意`html_url`的新版本、一次429失敗重試、一次停滯body；最後必須觀測EOF／reset。20次重複callback不追加請求，手動重試經Slint按鈕焦點及空白鍵路徑，另核對離頁／關閉拒絕、固定下載／repository目標、設定bytes不變及兩種task join。這些是toolkit事件，未驗OS鍵盤、IME、真實browser外開與accessibility inspector。

```sh
PTTSTELLA_SLINT_SETTINGS_PROBE=releases \
  '/path/PTTStella Slint Candidate.app/Contents/MacOS/native' \
  --software-renderer --probe-media --verify-media NEW_DIRECTORY
```

省略`--software-renderer`可驗OpenGL。`release-actions.json`只有`passed_pending_exit_audit`；還需`release-closed.json`的三個HTTP請求／join證據、程序exit0及BBS peer嚴格Close ACK，才能把該案例列為通過。

| 範圍 | 結果 |
| --- | --- |
| Rust library／binary | 377／83 passed，另4／1 ignored；ignored是需人工vault／clipboard或效能環境，沒有算passed。影片修正後的binary gate另重跑83案通過。 |
| 更新與一般shell | CPU／OpenGL更新兩案及CPU／OpenGL一般shell兩案，逐案receipt與Close ACK。更新案含toolkit焦點／空白鍵。 |
| 播放中退出 | CPU Quit、OpenGL Close兩案有非空四色像素、renderer退休、session join及Close ACK。單次MP4通過不結案既有arm64偶發錯色。 |
| 完整候選 | 禁止讀取`/opt/homebrew`的sandbox下，CPU／OpenGL更新、shell、MP4四案通過，沒有DYLD／LD搜尋覆寫。 |
| 其他gate | all-target Clippy、165個Python測試、fmt／文件／workflow檢查依實際紀錄。 |

## 最新arm64候選與清理

最新交付仍在 `.local/verification/slint-shell-chrome-final/package-current/`，取代原c9413d3主程式候選。

| 產物 | SHA-256 |
| --- | --- |
| App `Contents/MacOS/native` | `bbfde96110519ab1d1149b807dc57532bfe6a45df219795a88b473eb1c119ba4` |
| `PTTStella-Slint-Candidate-0.5.0-dev.3-macos-arm64.zip` | `a8e14af95902fa8043001d644767b933d9310b5fc9871677a27f870d0a82c475` |
| `PTTStella-Slint-Candidate-0.5.0-dev.3-macos-arm64.dmg` | `e5985ca35334972203b841f38feb9e0fb088e676e65427ea0861be645080d0ea` |

1176份regular payload在App／ZIP／唯讀DMG逐檔一致；51個Mach-O含主程式與50個dylib均為arm64，loader閉包、ad-hoc簽章deep／strict與隔離profile通過。未公證，完整原生操作及產品來源交付仍not_run／not_ready。`79b8bcf`基準的dirty Rust／Slint來源含新模組已完整保存patch及hash；驗收後的提交另以source-integration紀錄連結，payload內原始來源資料不改寫。版本仍為0.5.0-dev.3，沒有發佈或S9切換。

驗收後用Cargo原生clean移除8264檔／宣告2.5GiB（target du2666732KiB），移除舊arm64包（du277948KiB）且只留小型metadata；新候選移至穩定位置後1176份payload hash不變。另清掉兩個已中斷probe的精確OS暫存設定目錄（先核對程序時間、owner／resolved path／內容／open files並保存最小內容），23張byte-identical PNG改以保留位置／SHA映射，13張獨特PNG保留。最終10289份受保護檔案與12份Rust／Slint來源hash一致，worktree du1500908KiB，本批證據4904KiB，新候選278616KiB。

filesystem available 217673043968→220457906176bytes只記並行觀測，不將全部差額歸因於本批。清完target後，以最新包在無Homebrew sandbox再次跑CPU版本／空白鍵／HTTP取消／Close ACK通過，沒有重建Rust。exact paths、inventory、Cargo／du／free及保留例外在 `slint-release-check/cleanup/ledger.json`。必要候選、來源archive、獨特失敗、未整合worktree、使用者資料、已安裝工具與共用／不明cache保留，其他聊天未動。

Windows、Linux與Intel Mac候選仍有各自原先的source ref，這輪Rust變更尚待同步建置及各平台原生驗收；原生焦點／IME／可及性、通知淡出與錯誤優先、資料往返及同負載效能繼續按[總計畫](slint-evaluation-plan.md)完成。

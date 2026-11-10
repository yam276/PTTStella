# Slint macOS x64 候選包

2026-11-10 最新候選已改用 `.local/verification/slint-merged-id-spaces/x64-package/`，包含重複推文 ID 空格修正（產品來源 `26d9947`）。檔案、雜湊及本輪範圍見[最新四架構候選](slint-evaluation-plan.md#2026-11-10-最新候選)。以下原生／模擬與 SHA 紀錄屬先前候選，不能套用成新二進位的實機結果；舊包的必要 metadata／notice 另封存在本批 `retired-package-metadata/`。

完整 `native-player-probe`／`pttstella-slint` 已封裝為 Intel Mac 候選 App／ZIP／DMG，包含自行從來源建置的 mpv／FFmpeg runtime。此包在 Apple Silicon Mac 的 Rosetta 通過搬移及分立元件檢查；Intel 實機、OS 鍵鼠／IME、可聽音訊、最低 OS、乾淨機器與效能仍未驗收，不替代[總計畫](slint-evaluation-plan.md)的 S8 門檻。

## 交付與啟動

Slint worktree 的 `.local/verification/slint-macos-x64-candidate/package-current/` 保留唯一最新完整候選：

| 項目 | SHA-256 |
| --- | --- |
| ZIP `PTTStella-Slint-Candidate-0.5.0-dev.3-macos-x86_64.zip` | `c4842e94c960fd81dc354bbc610bcae7428f719a2999fd72ce4b22618a35103f` |
| DMG `PTTStella-Slint-Candidate-0.5.0-dev.3-macos-x86_64.dmg` | `b6d9945ceea987869bc0568b8149273092f900c88dfd0a59655a0069a68e70f6` |
| App `Contents/MacOS/native` | `3908f73e6f12f9b2ca54ff0b24530a557a8df56bbc21fffef9b2a39d976b60df` |

解壓或掛載後使用 `PTTStella Slint Candidate.app`，不覆蓋正式 PTTStella。此包使用 ad-hoc 簽名，未公證。正常啟動會自動連研究設定的預設站、不自動登入；設定／資料使用 `PTTStellaSlintResearch`，密碼庫使用 `SlintResearch`。不自動讀取或搬移正式版資料。

可從 `Contents/MacOS/native` 執行 `--show-profile`（不開 store／vault）、`--probe-grid`、`--probe-ime`，或以 `--verify-shell NEW_DIR` 執行只連本機的合成 shell。需要軟體繪製時加 `--software-renderer`；原生 OS 輸入法驗收須人工操作候選窗，不能以 toolkit callback 取代。

ZIP 與唯讀掛載 DMG 的1159個regular payload逐檔hash與App相同；48個Mach-O全為x64，含主程式與47個dylib。主程式唯一RPATH為 `@executable_path/../Frameworks`，dylib繼承它，沒有開發目錄匯入。完整codesign deep／strict、DMG校驗與掛載後隔離profile通過。`package-report.json`區分Rust來源與封裝來源，保留實際dirty狀態及各層hash；`native_acceptance`仍為`not_run`。

## Runtime 來源與建置

來源／recipe／patch／逐套件建置與驗收證據在 `.local/verification/slint-macos-x64-source/`。已核對的來源archive是本批必要交付，不是過期編譯快取。

- mpv0.42.0：官方archive SHA-256 `bcfebbfa2b65a6d88dc97ea18117831072373e38b5785a2afb874ed4df5070dd`。[官方版本](https://github.com/mpv-player/mpv/releases/tag/v0.42.0)
- FFmpeg9.0.2：官方archive SHA-256 `8c3850283eb25fa026482078a04051e0be17347b09ef81a0849bec15a96e002e`。[官方來源](https://ffmpeg.org/releases/ffmpeg-9.0.2.tar.xz)
- libplacebo7.387.0：commit `275f7dea27924cdd34ab2e6c20e448b6bc4fab65`；libjxl0.11.2：commit `332feb17d17311c748445f7ee75c4fb55cc38530`，均含實際submodule來源及ref。來源匯出逐檔比對，archive SHA記在manifest。
- 基線recipe取自[IINA deps-buildscripts](https://github.com/iina/deps-buildscripts/tree/962ba3b429355c47f243fddb1f3e495c2a9ca05a)，此批改用mpv0.42／libplacebo7.387，保留完整32項依賴與影片格式；不宣稱這是IINA舊預編譯runtime的精確recipe。

第一輪31項依賴完成，最後mpv因無Vulkan配置仍引用未編入的Swift `MetalLayer` 而失敗。依[mpv實際來源條件](https://github.com/mpv-player/mpv/blob/v0.42.0/meson.build)為該引用加 `HAVE_VULKAN` guard，patch／雜湊／失敗log保留；只重編mpv約20秒成功，沒有重建整組依賴或關掉解碼／OpenGL功能。另修正mujs的build-relative install ID為 `@rpath/libmujs.dylib`，同步修正既有libmpv相依名稱；recipe在消費者連結前即設定ID，post-install命令與前後hash另記。長絕對ID超出原load-command空間的失敗也保留。

實際封裝後C API回報 `mpv v0.42.0`、client API131078（2.6）、FFmpeg9.0.2；FFmpeg `avcodec_license()`／`avutil_license()`都回報 `GPL version 3 or later`。實際輸出保存在 `runtime-version.json`。這是二進位觀測，不用通用Credits文字替代授權／來源交付判定。先前IINA provider的實測0.41.0／FFmpeg9.0.1、版本不符及loader反例仍保留最小證據，該provider的CLI／dylib已由本候選取代。

32份來源與成功build record逐一綁定archive SHA、reachable library SHA及檢查時snapshot；實際提取116份notice（含FreeType FTL／GPLv2文字）。無明確owner、失敗build、來源／輸出不符、重複owner、越界path及過大輸入會拒絕。`--source-manifest`接入共用runtime collector，不偽造Homebrew receipt。來源archive上限128MiB對應含submodule的libjxl匯出，每份notice及總量另有較小限制。**完整產品對應來源交付仍為 `not_ready`**：Rust／其他平台完整source payload與乾淨環境重播尚未驗收，這批runtime清單不會自動將該gate改為passed。

原Rust完整CLI來自 `d37190d`，Rust patch為空；原建置worktree含helper修改，dirty如實保留。沿用原69,874,676-byte CLI，只改輸入RPATH及ad-hoc簽章，再由packager搬進App，沒有重建Rust。原始CLI SHA為 `b1e810584025eb0d4fc65992045c7eb99ae5b25bdd0e4e9bbed6831453731e64`，原建置／post-link／最終App三層hash可追溯。Rust請求deployment target11.0，本批native依賴請求12.0；48個Mach-O load-command宣告的最高minimum為12.0，詳見 `minimum-os-load-commands.json`；這些設定不等於已驗最低macOS版本。

## 搬移與驗收範圍

複製完整候選至含空格路徑，同時暫移來源建置prefix與舊IINA prefix，使兩套開發runtime原路徑均不可用。無DYLD／LD搜尋覆寫，CPU／OpenGL各驗shell、文章／終端尋找、MP4／WebM／OGV，另驗軟體草稿，13案全部exit0及嚴格Close ACK；逐案receipt在 `slint-macos-x64-source/relocated-acceptance.json`。

影片有非空四色像素、獨立色差核對、render退休、control join及core destroy；尋找有三命中、Pending／背景保留／恢復、來源／PNG不變及索引釋放。草稿probe未驗新程序還原。所有案例只用toolkit事件及synthetic foreground；不能代表Intel硬體、OS焦點／IME、可聽音訊、GPU效能或完整功能矩陣，也不結案先前arm64 MP4偶發錯色。

另以封裝App執行一次DYLD shell追蹤，只啟用載入列印，沒有改搜尋路徑。System WebKit／JavaScriptCore及WritingTools相關映射按實際log記在 `dyld-packaged-audit.json`；不能宣稱執行期零WebKit，也不能由該trace推定WebView實例或呼叫成因。App沒有打包WebView runtime，產品依賴與原生輸入仍按S8各自驗收。

## 清理與尚待驗收

前一輪Cargo完整clean的8281檔／2.5GiB與IINA DMG移除紀錄仍在 `slint-macos-x64-candidate/cleanup/`。本批中途已移除36個編譯／來源副本目錄，來源archive與必要診斷保留；exact paths及空間觀測在 `slint-artifact-cleanup-20261110-source/ledger.json`。

本批驗收後清理與保留例外記在 `slint-macos-x64-source/cleanup/ledger.json`。 移除11個已無使用者的精確路徑，du合計499156KiB；11382份保留檔案先核對一致，再移除24張byte-identical generated PNG（allocated1544192bytes），每張保留位置及SHA另列，16張獨特圖片保留。最終11358份受保護檔案及32份來源archive再驗hash一致。filesystem available 219832209408→220345688064bytes僅為刪除區間觀測，不全歸因於本批；最終worktree du1495272KiB、來源交付292272KiB、x64候選297812KiB，Cargo target16KiB。清完後從最終候選重跑OpenGL MP4，四色像素／render退休／session join／Close ACK通過，證明不依賴已刪CLI／runtime／target。保留最新完整候選、必要原始來源archive／recipes／patch、授權與獨特失敗；不留完整舊CLI／runtime、臨時App、native objects或bootstrap安裝。未整合Slint worktree、已安裝Rust targets、共用SDK／registry、其他聊天及正式版使用者資料保留。

後續仍須Intel實機／最低OS、原生輸入法／DPI／鍵鼠／檔案與密碼庫、完整功能與資料往返、來源乾淨重播、同負載效能、四平台總驗收及回退證據。S0–S8持續，S9入口切換／發佈未執行。

# Slint 與原 Dioxus 的資料往返

這是 S7／S8 的部分證據，尚未完成整體資料回退驗收。2026-11-10 已在 macOS arm64 以實際 App 的設定／站點控制，完成 v1、v6 合成資料的 Slint → 原 Dioxus → Slint 往返。其他 store、平台正式路徑、密碼庫與完整產品切換仍待驗。

## 同一份資料的入口

新增診斷入口：

```text
pttstella-slint --software-renderer --probe-store NEW_EVIDENCE_DIR MARKED_STORE_ROOT
```

此入口使用既有 Benchmark 本機服務與完整管理 owner，讀取既有 Preferences，而不先寫入預設設定。沿用生產 Store 的 schema、鎖、備份、合併與錯誤；UI 不實作第二份格式。`Profile::system()` 的一般設定／資料目錄與 SlintResearch 密碼庫身份保持原狀。

資料根目錄必須已存在，且 `.store-compatibility.json` 為不超過 1024 bytes 的普通檔案，內容精確為：

```json
{"format":"PTTStella.StoreCompatibility","version":1,"synthetic":true}
```

同一目錄使用原 Dioxus 探針的檔名：`probe-settings.json`、`probe-sites.json`、`probe-filters.json`、`probe-bookmarks.json`、`probe-drafts.json`、`probe-recent-boards.json`。根目錄、marker 及既有 store／一代備份／lock 不接受 symlink 或非普通檔案。缺少 store 時沿用既有預設讀取；壞檔與未來版本保留原 bytes／警告，不偷偷重設。

這是明確標記的合成副本入口，不會自動搬移正式資料，也不代表可以直接把使用中的產品目錄拿來驗收。原 Dioxus 使用其既有 `pttstella --probe-surface MARKED_STORE_ROOT tools`，兩個程序依序退出後才啟動下一個。兩邊讀寫相同檔案，不在階段間轉換 schema 或重新播種。

## 可重現資料與操作

fixture 直接重用 `tooling/verify_store_compatibility.py` 的自製 CC0 資料。從 repo 根目錄，選一個不存在的新路徑建立：

```python
from pathlib import Path
import sys
sys.path.insert(0, "tooling")
from verify_store_compatibility import fixture, encoded, MARKER

root = Path("NEW_SYNTHETIC_STORE_ROOT")
root.mkdir()  # Existing paths fail; never overwrite an old profile.
expected, settings_disk = fixture(6, auto_expand=True)  # Repeat with v1 separately.
(root / ".store-compatibility.json").write_bytes(encoded(MARKER))
names = {"settings": "settings", "sites": "sites", "filters": "filters",
         "bookmarks": "bookmarks", "drafts": "drafts", "recent": "recent-boards"}
for key, name in names.items():
    value = settings_disk if key == "settings" else expected[key]
    (root / ("probe-" + name + ".json")).write_bytes(encoded(value))
(root / "display.pttstella.json").write_bytes(encoded(expected["profile"]))
```

使用原生 App 選單開設定，先核對讀入值，再以原生欄位操作保存字高、選原有 WSS 最愛並保存繁中站名；不按「連線切換」。原 Dioxus tools 案例另需明確匯出整頁 PNG，才符合其原有煙霧測試。正常 Quit 後核對程序 exit、連線 Close／worker 清理與完整 JSON，下一個前端才能重開。保存前後各留獨立快照，備份應逐 byte 等於上一份主檔；不能只看到 UI 成功提示。

## 實測結果

證據在 `.local/verification/slint-frontend-roundtrip/frontend-results.json`，各階段有 `run.json`、原始 log、獨立 expected 與 acceptance。Slint 來源為 `842264e` 加本批確切 source patch／285 份 fingerprint；raw CLI SHA-256 為 `8bd1b0333f3c0634169990b96d7013c746a9e28ec49b901d651999dabbdd49ae`。原 Dioxus 從乾淨 `5ea9ad6a498331d0cbebc7d9803e0a51df46e5c3` 建置，無生產來源 patch；raw CLI SHA-256 為 `35c3e27506280f430b364b82355e31f7b101a254fcadccc289ac7063ed6a44e3`。

| 起始資料 | Slint 首次保存 | 原 Dioxus 讀回／保存 | Slint 新程序讀回 |
| --- | --- | --- | --- |
| settings v6，英文、Terminal、自動開圖 | 字高 21.5 → 22.5，站名 `Slint PTT 末字` | 讀到 22.5／完整站名；保存字高 23、站名 `Dioxus PTT 末字` | 讀到 23／完整站名；再保存 20、`Slint 回程末字` |
| settings v1，繁中、Web 預設 | 讀到 21.5／v1 預設；保存為 v6、字高 22、站名 `Slint v1 末字` | 讀到 22／完整站名；保存 23、`Dioxus v1 末字` | 讀到 23／完整站名、Web／繁中；只讀且 JSON／備份 hash 不變 |

六輪皆正常 exit0。四輪 Slint 各有恰好一筆 `slint_full_parity_grid` Close acknowledged；兩輪原 Dioxus 的 surface report 為 passed、disconnect acknowledged，listeners、file／connection／credential worker 皆清理完成，PNG 為 true。兩張 PNG 另核對 chunk CRC 與 zlib 解壓，不以檔名代替圖片證據。

每次保存後完整六個 store 的值符合獨立 expected；同端點 UAO／GBK 最愛、預設站、名單、收藏 archive、草稿 ANSI／尾空白、最近看板與可攜設定未受無關修改。後五份檔案各自與 seed 的 SHA-256 一致，這只證明 byte-preservation，不代表它們已完成 GUI 操作。v6 三次保存，以及 v1 原 Dioxus 保存的一代備份，已逐 byte 核對前一份主檔；v1 首次 Slint 保存的備份未在後續覆寫前另存，不補造該項驗收。

## 保留的失敗與待驗

- 原 Dioxus HTML 字高欄位 `step=1`：能顯示 Slint 的 22.5，但試存 23.5 會顯示欄位不完整／超界且不寫檔。以 23 保存成功，只驗到整數路徑；小數字高的完整 GUI 回退仍未通過，不能從 store 接受浮點數推定 UI 相容。
- 首輪有一般入口被操作工具額外啟動，該輪不宣稱全程零外網／vault。後續固定參數 host 只執行明確合成探針，重複 LaunchServices 啟動不回退到一般 Live。
- 長輪次超過既有 peer 的 300 秒，以及兩個 240 秒 controller deadline，皆保留 failed／終止紀錄。後續短輪次的成功不改寫先前失敗。
- 原生貼上與 App 選單不是實體 IME、完整鍵鼠、主視窗 foreground／pointer、可及性或跨平台證據。名單、收藏、草稿、最近看板、可攜設定的完整 UI 往返與正式平台路徑／vault 仍須另驗。

Rust library 389 passed／4 ignored、Slint bin 89 passed／1 ignored，release all-target Clippy 與 CLI build 通過；ignored 項不計成已執行。新增測試涵蓋有界精確 marker、舊檔不重播種、壞檔／未來版保留、非普通檔案／symlink 拒絕及 CLI 互斥。這些 gate 與上列原生證據分開計算。

本批建置 target 與臨時 baseline worktree 已用 Cargo／Git 原生流程移除，必要 raw CLI 各留一份供後續 UI 與封裝；臨時 App 在確認無程序／open files 後逐個清理。exact paths、resolved／UID／symlink、du、filesystem free 前後與保留例外在本批 `compile-cleanup/ledger.json`、`host-cleanup/ledger.json` 及 `final-cleanup/ledger.json`。最後清除5份臨時App與9個本批專用WebKit／cache／系統暫存目錄；重複generated PNG的保留位置與SHA另記 `final-cleanup/duplicate-pngs.json`。既有各平台最新候選未被這份未封裝診斷 CLI 取代；沒有整合正式入口或發佈。

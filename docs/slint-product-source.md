# Slint 候選產品來源還原

候選交付中的程式、Slint UI、字型、locale、fixture、工具與文件，須能從獨立來源包還原。原始建置的 commit／dirty／patch、產品來源核對與第三方來源閉包是不同證據；不能把其中一項通過填成全部完成。

## 2026-11-11 產品來源包

本機交付位於 `/Users/ladesine/Documents/PTTStella-Slint-Deliverables/product-source-20261111-9a06515/`，包含單一 23,220,799-byte Git bundle、`sources.json`、獨立還原工具及 README。三組來源皆直接從 bundle 還原，不需要 patch。既有 docs gate 禁止 repo 內的 `.bundle`，因此來源交付保留於 repo 外，不放寬檢查。bundle 含 Git ancestry，不包含 worktree 的 `.local`、編譯 target 或未追蹤使用者資料。還原只讀此交付，不連網、不使用原 checkout 的 Git objects，也不覆寫既有目的地。

| 選項 | 對應候選 | 產品核對 commit | 完整 tracked 檔案 | 建置輸入核對 |
| --- | --- | --- | --- | --- |
| `macos-arm64-current` | Mac arm64 | `9a06515` | 580 | 380 |
| `linux-current` | Linux x64 | `84e98a3` | 546 | 289 |
| `windows-intel-current` | Windows x64／Mac x64 | `475897f` | 545 | 288 |

上述 commit 是已核對產品檔案的版本。arm64 的實際原建置是 `aff5ac8` 加 snapshot／raster 修補；乾淨 `9a06515` 再執行 Cargo 時命中快取，0.32 秒結束且 raw SHA 不變，不能改稱重新編譯。Linux仍是 `c67efdb` 加patch；各包原始 build record 保持不變。新 arm64 封裝 native SHA 為 `f211c341c2b7404e44af0ff8fe3b83ffc53e1eb22531b6e089352ac4ebbfbafb`。manifest 綁定封裝內執行檔 SHA，還原後核對完整 Git tree、所有 tracked 檔案的 SHA-256 與檔案清單，不只核對少數 Rust 檔案。

從來源包目錄執行（Python 3.11+、Git；目的地須尚不存在）：

```sh
python3 reconstruct_slint_source.py --manifest sources.json \
  --candidate macos-arm64-current --output /ABS/PATH/new-source
```

一般 Slint 配置可在還原的 checkout 執行：

```sh
cargo test --release --locked --no-default-features \
  --features slint-research --bin pttstella-slint
```

Rust 版本依 `rust-toolchain.toml`；公開 crates、Skia 與平台系統依賴仍需正常取得。本批以 macOS arm64 的公開 Cargo cache／已安裝工具鏈，從新還原目錄建置，103 項測試通過、1 項 helper ignored。sandbox 拒讀原 checkout 的來源，只允許同一固定 compiler cache；log 顯示 root 與 vendored renderer 都從還原目錄編譯，未使用研究 runtime 或交叉 SDK。三組來源完整還原均通過；前輪離線 metadata 與建置結果另存歷史紀錄。這不是冷依賴建置、播放器配置重建、逐 byte 二進位重現或 Windows／Intel／Linux 原生編譯驗收。

新 bundle SHA-256 為 `44cff20e28bf51c85bb1c2533705df67a9a1d5fe1ff96db0500f405b8cf82b26`，包含三個完整來源版本及新增 renderer 修補／授權。先在原 worktree 子目錄還原的建置因 Cargo 搜尋 ancestor manifest 而被 sandbox 拒絕；移動同一 checkout 到原 worktree 之外的來源交付目錄後通過，沒有修改 manifest 來繞過核對。之後依序重用同一臨時路徑核對 Linux、Windows／Intel，580／546／545 檔與完整 tree 全部相符，再確認 Git clean／逐檔 SHA／無 open files 後移除。日常編譯沿用固定 worktree／Cargo cache。舊 `product-source-20261110-09a13da/` 與共用其 immutable bundle 的 `product-source-20261110-c906776/` 保留供歷史交付對照。

## 工具驗證與剩餘範圍

[還原工具](../tooling/reconstruct_slint_source.py) 僅接受 bounded regular manifest／bundle／patch，拒絕路徑跳脫、payload symlink 與既有目的地。它使用明確 SHA-1 Git object format、停用外部 Git 設定與 hooks，還原後比對 tree、檔案 hash 及完整 tracked inventory。

[六項自製 fixture 測試](../tooling/test_reconstruct_slint_source.py) 涵蓋移除原 checkout 後的完整還原、patch 與新檔、損壞或 symlink bundle、既有目的地保護、路徑／必要清單拒絕，以及錯 tree／hash／遺漏檔案拒絕。fixture 不依賴產品的私有資料或原 repo Git history。

產品來源包已還原，不代表完整第三方對應來源已交付。播放器與其靜態／動態依賴、Skia、各平台工具與可重跑 recipe／來源閉包仍須逐項綁定；候選原 `source_delivery: not_ready` 維持不變。詳細來源、實際命令、工具鏈、還原結果、獨立建置與本批清理紀錄在 `slint-renderer-api-current/`；前輪兩組紀錄在 `slint-source-reconstruction/`。整體 S0–S8 及 S9 邊界依 [主計畫](slint-evaluation-plan.md)。

後續已完成 [macOS x64 的 32 套件乾淨來源重建](slint-native-source-replay.md)，並核對 47 個 runtime library；首輪 MP4 fixture 逾時仍未定因，解碼重複性與完整跨平台來源交付維持 `not_ready`。

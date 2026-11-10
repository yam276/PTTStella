# Slint 候選產品來源還原

候選交付中的程式、Slint UI、字型、locale、fixture、工具與文件，須能從獨立來源包還原。原始建置的 commit／dirty／patch、產品來源核對與第三方來源閉包是不同證據；不能把其中一項通過填成全部完成。

## 2026-11-10 產品來源包

本機交付位於 `/Users/ladesine/Documents/PTTStella-Slint-Deliverables/product-source-20261110-c906776/`，包含單一 Git bundle、`sources.json`、獨立還原工具及 README。既有 docs gate 禁止 repo 內的 `.bundle`，因此來源交付保留於 repo 外，不放寬檢查。bundle 含 Git ancestry，不包含 worktree 的 `.local`、編譯 target 或未追蹤使用者資料。還原只讀此交付，不連網、不使用原 checkout 的 Git objects，也不覆寫既有目的地。

| 選項 | 對應候選 | 產品核對 commit | 完整 tracked 檔案 | 建置輸入核對 |
| --- | --- | --- | --- | --- |
| `arm64-linux-current` | Mac arm64／Linux x64 | `84e98a3` | 546 | 289 |
| `windows-intel-current` | Windows x64／Mac x64 | `475897f` | 545 | 288 |

上述 commit 是已核對產品檔案的版本。arm64／Linux 的實際原建置仍是 `c67efdb` 加已保存 patch，不能改稱乾淨 `84e98a3` 建置；各包的原始 build record 保持不變。manifest 綁定封裝內執行檔 SHA，還原後核對完整 Git tree、所有 tracked 檔案的 SHA-256 與檔案清單，不只核對少數 Rust 檔案。

從來源包目錄執行（Python 3.11+、Git；目的地須尚不存在）：

```sh
python3 reconstruct_slint_source.py --manifest sources.json \
  --candidate arm64-linux-current --output /ABS/PATH/new-source
```

一般 Slint 配置可在還原的 checkout 執行：

```sh
cargo test --release --locked --no-default-features \
  --features slint-research --bin pttstella-slint
```

Rust 版本依 `rust-toolchain.toml`；公開 crates、Skia 與平台系統依賴仍需正常取得。本批以 macOS arm64 的公開 Cargo cache／已安裝工具鏈，從新還原目錄建置，95 項測試通過、1 項 helper ignored；沒有配置原產品 checkout、研究 runtime 或交叉 SDK 作為此建置的輸入。兩組來源的離線 `cargo metadata --no-deps` 亦通過。這不是播放器配置重建、逐 byte 二進位重現或 Windows／Intel／Linux 原生編譯驗收。

## 工具驗證與剩餘範圍

[還原工具](../tooling/reconstruct_slint_source.py) 僅接受 bounded regular manifest／bundle／patch，拒絕路徑跳脫、payload symlink 與既有目的地。它使用明確 SHA-1 Git object format、停用外部 Git 設定與 hooks，還原後比對 tree、檔案 hash 及完整 tracked inventory。

[六項自製 fixture 測試](../tooling/test_reconstruct_slint_source.py) 涵蓋移除原 checkout 後的完整還原、patch 與新檔、損壞或 symlink bundle、既有目的地保護、路徑／必要清單拒絕，以及錯 tree／hash／遺漏檔案拒絕。fixture 不依賴產品的私有資料或原 repo Git history。

產品來源包已還原，不代表完整第三方對應來源已交付。播放器與其靜態／動態依賴、Skia、各平台工具與可重跑 recipe／來源閉包仍須逐項綁定；候選原 `source_delivery: not_ready` 維持不變。詳細來源、實際命令、工具鏈、還原結果、獨立建置與本批清理紀錄在 `slint-source-reconstruction/`。整體 S0–S8 及 S9 邊界依 [主計畫](slint-evaluation-plan.md)。

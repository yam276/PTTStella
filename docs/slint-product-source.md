# Slint 候選產品來源還原

候選交付中的程式、Slint UI、字型、locale、fixture、工具與文件，須能從獨立來源包還原。原始建置的 commit／dirty／patch、產品來源核對與第三方來源閉包是不同證據；不能把其中一項通過填成全部完成。

## 2026-11-11 產品來源包

本機目前交付位於 `/Users/ladesine/Documents/PTTStella-Slint-Deliverables/product-source-20261111-6ad8e39/`，包含單一 23,220,799-byte Git bundle、五份patch、`sources.json`、獨立還原工具及 README。`filter-current` 從bundle的9a加產品patch還原6ad，`filter-cross-current` 加另一patch還原61d，`cross-current` 加10,009-byte文件patch還原9db，其餘三組直接還原。新cross descriptor擴充既有交付，沒有另建來源包目錄；先前manifest保存在驗收證據。bundle以hardlink沿用前份immutable檔案，沒有重複完整bundle。既有 docs gate 禁止 repo 內的 `.bundle`，因此來源交付保留於 repo 外，不放寬檢查。bundle 含 Git ancestry，不包含 worktree 的 `.local`、編譯 target 或未追蹤使用者資料。還原只讀此交付，不連網、不使用原 checkout 的 Git objects，也不覆寫既有目的地。

| 選項 | 對應候選 | 產品核對 commit | 完整 tracked 檔案 | 建置輸入核對 |
| --- | --- | --- | --- | --- |
| `reading-cross-current` | 最新兩Mac／另備Windowsfe8872f6 | `7466c31` | 580 | 380 |
| `reading-current` | 最新 Linux150c1a0d | `fbcfe2e` | 580 | 380 |
| `filter-current` | 歷史 Mac arm64／x64 | `6ad8e39` | 580 | 380 |
| `filter-cross-current` | 歷史 Linux4fa／Windowsac788a45 | `61d1c3f` | 580 | 380 |
| `macos-arm64-current` | 歷史 Mac arm64 | `9a06515` | 580 | 380 |
| `cross-current` | 歷史 Linux／Windows備妥包／Mac x64 | `9db9eda` | 580 | 380 |
| `linux-84e-baseline` | 歷史 Linux x64 | `84e98a3` | 546 | 289 |
| `windows-intel-475-baseline` | 待使用者測試的 Windows69542cb7／歷史 Mac x64 | `475897f` | 545 | 288 |

最新Mac與另備Windows使用 `reading-cross-current`，同一bundle加 `product-reading-cross.patch` 還原乾淨7466完整580檔／tree `164f4be07417edde599b4f538e70e82b8ae2f6c6`，綁定封裝ARMf50ffb46／Intelb9fa63fa／Windowsfe8872f6。三份原始build record保留；與Linuxfbc完整tree只差三份文件，其餘577檔及380份產品選集相同。本輪沒有重做獨立還原來源編譯；前輪fbc一般Slint106項／1 helper ignored屬於相同非文件來源。所有既有descriptor與immutable bundle保持不變，新manifest SHA為 `41093833cc6378214fc235a538e3dca1484b5af7760f201dd024ba7499c7f1bc`。還原checkout使用同一repo外暫存路徑，核對後移除，沒有新Cargo target。

最新 `reading-current` 在同一交付加入 `product-reading.patch`，保留 immutable bundle 與既有六份descriptor，還原已提交fbcfe2e的完整580檔／tree `f3a4ec30192e3ecf0f510d6494a33bf5d8d6565b`，綁定Linux ELF150c1a0d。原編譯仍記為f40e793 dirty，測過的完整七檔patch與f40e793到fbcfe2e逐byte相同，沒有轉稱乾淨來源重建。

本批另從還原checkout完成macOS arm64一般Slint配置獨立建置：106 passed／1 explicit native helper ignored。sandbox拒讀原worktree來源（Cargo.toml負向control確實被拒），只允許固定Cargo target快取；還原目錄移到repo外同一可重用暫存路徑，避免Cargo搜尋ancestor manifest。原始command／log與580檔SHA核對保存在 `slint-renderer-api-current/cross-current/filter-package-current/chrome-current/refined/`，驗後還原checkout移除。這不是冷依賴建置、native-player／其他架構來源閉包、原生操作或逐byte二進位重現。該輪sources.json SHA為 `400bef10edf0702be2a2d9f4dde750c90a4a5f353ac89c970da1b30840178b17`。

上述 commit 是已核對產品檔案的版本。最新兩Mac實際原建置為a63c303 dirty，加frozen三檔patch；該patch與已提交6ad8e39產品／測試修改逐byte相同。ARM raw59c1c58f／封裝0b2f5786、Intel raw64a2caf1／封裝ae54f0bd，各原始build record保留。本批從新目錄還原filter-current完整580檔及tree `6c9d5b29336cbc36e7eef2a80f481b846d160c05`；只做來源還原，未重做獨立編譯。manifest 綁定封裝內執行檔完整SHA，核對完整 Git tree、所有 tracked 檔案的 SHA-256 與檔案清單，不只核對少數 Rust 檔案。

Linux4fa0a14e與備妥Windowsac788a45從乾淨61d1c3f完成release／all-target Clippy。`filter-cross-current`已在同一可重用暫存路徑重新還原完整580檔／tree `4c9a8f5b97da144e7385d24f472fc764bf6831df`，與Mac6ad只有兩份文件不同，其餘578檔及380份產品選集相同；驗後移除還原目錄。這不是獨立來源編譯或Windows原生結果，Linux七過三敗的實際scope與未解PNG／QEMU反例另見主計畫。後續未採用的probe準備／trace實驗只保存在驗收patch及單一未解CLI，未混入此乾淨來源或封裝SHA。

歷史arm64的原建置是 `aff5ac8` 加 snapshot／raster 修補；乾淨 `9a06515` 再執行Cargo時命中快取，0.32秒結束且raw SHA不變，不能改稱重新編譯。歷史Linux基準則是 `c67efdb` 加patch。舊ARM封裝f211c341等mapping保留供歷史證據核對。

先前三個交叉編譯目標從乾淨9db9eda完成 release／all-target Clippy；Windows raw487c428b、舊Intel rawbb3185a3／封裝e232fd01、Linux8fa36bcd分別綁定manifest。它們與ARM9a的380份建置輸入全部相同，完整tracked tree僅差兩份文件。上段c67加patch是歷史Linux基準，不是最新Linux包。`cross-current`已另從同一immutable bundle＋文件patch核對580檔及完整tree，還原暫存目錄驗後移除；沒有再建新Cargo target或宣稱新的獨立跨平台建置。

從來源包目錄執行（Python 3.11+、Git；目的地須尚不存在）：

```sh
python3 reconstruct_slint_source.py --manifest sources.json \
  --candidate reading-current --output /ABS/PATH/new-source
```

一般 Slint 配置可在還原的 checkout 執行：

```sh
cargo test --release --locked --no-default-features \
  --features slint-research --bin pttstella-slint
```

Rust 版本依 `rust-toolchain.toml`；公開 crates、Skia 與平台系統依賴仍需正常取得。先前9a來源以 macOS arm64 的公開 Cargo cache／已安裝工具鏈，從新還原目錄建置，103 項測試通過、1 項 helper ignored。sandbox 拒讀原 checkout 的來源，只允許同一固定 compiler cache；log 顯示 root 與 vendored renderer 都從還原目錄編譯，未使用研究 runtime 或交叉 SDK。這份歷史獨立建置證據不能直接套用6ad新patch；6ad本批只有完整來源還原與原worktree回歸／封裝證據。這不是冷依賴建置、播放器配置重建、逐 byte 二進位重現或 Windows／Intel／Linux 原生編譯驗收。

bundle SHA-256 為 `44cff20e28bf51c85bb1c2533705df67a9a1d5fe1ff96db0500f405b8cf82b26`，包含三個完整來源版本及新增 renderer 修補／授權；9db來源包與前一個 `product-source-20261111-9a06515/` 以hardlink共用此immutable bundle，交付時是完整regular file。先前在原 worktree 子目錄還原的建置因 Cargo 搜尋 ancestor manifest 而被 sandbox 拒絕；移動同一 checkout 到原 worktree 之外的來源交付目錄後通過，沒有修改 manifest 來繞過核對。之後依序重用同一臨時路徑核對 Linux、Windows／Intel，580／546／545 檔與完整 tree 全部相符，再確認 Git clean／逐檔 SHA／無 open files 後移除。日常編譯沿用固定 worktree／Cargo cache。舊 `product-source-20261110-09a13da/` 與共用其 immutable bundle 的 `product-source-20261110-c906776/` 保留供歷史交付對照。

## 工具驗證與剩餘範圍

[還原工具](../tooling/reconstruct_slint_source.py) 僅接受 bounded regular manifest／bundle／patch，拒絕路徑跳脫、payload symlink 與既有目的地。它使用明確 SHA-1 Git object format、停用外部 Git 設定與 hooks，還原後比對 tree、檔案 hash 及完整 tracked inventory。

[六項自製 fixture 測試](../tooling/test_reconstruct_slint_source.py) 涵蓋移除原 checkout 後的完整還原、patch 與新檔、損壞或 symlink bundle、既有目的地保護、路徑／必要清單拒絕，以及錯 tree／hash／遺漏檔案拒絕。fixture 不依賴產品的私有資料或原 repo Git history。

產品來源包已還原，不代表完整第三方對應來源已交付。播放器與其靜態／動態依賴、Skia、各平台工具與可重跑 recipe／來源閉包仍須逐項綁定；候選原 `source_delivery: not_ready` 維持不變。詳細來源、實際命令、工具鏈、還原結果、獨立建置與本批清理紀錄在 `slint-renderer-api-current/`；前輪兩組紀錄在 `slint-source-reconstruction/`。整體 S0–S8 及 S9 邊界依 [主計畫](slint-evaluation-plan.md)。

後續已完成 [macOS x64 的 32 套件乾淨來源重建](slint-native-source-replay.md)，並核對 47 個 runtime library；首輪 MP4 fixture 逾時仍未定因，解碼重複性與完整跨平台來源交付維持 `not_ready`。

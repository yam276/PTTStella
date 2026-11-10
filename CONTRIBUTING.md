# 參與貢獻

## 回報問題

請在 GitHub Issues 附上 PTTStella 版本、作業系統與架構、重現步驟、預期結果和實際結果。擷圖及紀錄請先移除帳密與私人內容。[回報問題](https://github.com/yam276/PTTStella/issues)

## 開發

日常開發以 `develop` 為基準，PR 也以 `develop` 為目標。`main` 保留正式版號，由維護者依發佈流程更新。[發佈流程](docs/releases.md)

Rust 版本由 `rust-toolchain.toml` 固定。文件與打包檢查使用 Python 3.11+，介面測試使用 Node.js 24。各平台的系統相依可參考 CI。[建置流程](.github/workflows/release.yml)

```sh
cargo run --locked --features desktop --bin pttstella
```

## 驗證

提交前執行 `git diff --check`，並按變更範圍選擇檢查。

| 變更範圍 | 檢查方式 |
| --- | --- |
| 文件 | `python tooling/check.py docs` |
| Rust 核心 | `python tooling/check.py core` |
| Slint 依賴／feature | `python tooling/verify_slint_dependencies.py --output <新證據目錄>`、`python -m unittest discover -s tooling -p test_verify_slint_dependencies.py`；檢查四 target／兩配置的選用依賴，不代表編譯或原生驗收 |
| Slint 繪圖／版面失效 | `cargo test --locked --release --features slint-renderer-audit --test slint_rendering`，再以 `slint-renderer-audit,native-player-probe` 重跑；獨立完整／局部 Skia 畫面逐像素比較，不代表 OS 呈現、IME 或原生視窗驗收。Slint worktree 沿用 `.local/slint-build.toml` 的固定快取。 |
| Slint 產品來源還原 | `python -m unittest discover -s tooling -p test_reconstruct_slint_source.py`；實際交付另依[來源還原](docs/slint-product-source.md)從新目錄核對完整 tree／檔案與建置 |
| 桌面介面 | `cargo test --locked --features desktop --lib --bin pttstella --test multi_instance`、`cargo clippy --locked --features desktop --all-targets -- -D warnings`、`node --test tooling/test_*.mjs` |
| Windows 原生視窗 | `python tooling/check.py desktop` |
| 打包或發佈 | `python -m unittest discover -s tooling -p 'test_*.py'`、文件檢查、`actionlint .github/workflows/*.yml`，再以 GitHub Actions 的 `verify` 模式核對各平台產物。 |

一般測試使用合成資料和本機服務。涉及畫面、鍵盤、滑鼠或輸入法時，請在受影響平台用實際 App 驗證，並在 PR 註明已測及待驗的範圍。缺少必要環境時記為 `not_ready`。[驗證與產物清理](docs/development-harness.md)

## 提交

- 每筆提交對應一項可獨立驗證的變更，使用 Conventional Commits，並記錄驗證結果。
- 新增資產或測試資料時，記錄來源、授權及版本。測試資料需可散布，並有獨立的預期結果。

維護者可在[開發紀錄](docs/active/minimal-browser.md)查看功能驗收狀態與歷次測試結果。

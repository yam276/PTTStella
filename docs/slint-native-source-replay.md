# macOS x64 runtime 來源重建

## 2026-11-10 結果

以既有 32 份已核對來源封存、含 submodule 的 libjxl／libplacebo exports、patch／recipe，在全新 build／install prefix 重建。archive 採 hardlink 重用 bytes；沒有複用舊 install 或已封裝 dylib。兩個 replay override 僅優先採用已驗 hash 的 recursive source export，原配方與歷史收據保留。

先建立隔離 NASM／automake／libtool 及固定版本的 Python 工具。主 runtime 建置在禁止 network 的 sandbox 執行 `bash build.sh x86_64`，**32／32 完成、exit 0**，腳本回報 10 分 26 秒。47 個輸出對應原 package owner，載入閉包的 47 個 dylib basename 與目前 Intel 候選一致，全為 x86_64；非系統相依均來自新 prefix。歷史／新 output hash 分列，沒有逐 byte 重現的主張。

實際 API 回報 mpv v0.42.0、client API 131078、FFmpeg 9.0.2，符合來源 manifest。版本 collector 首版漏掉顯示前綴 `v` 而 assertion failed，原程式 exit 0／JSON 保留；按來源版本校正前綴映射。這與下列像素案的失敗分開記錄。

## 解碼反例與驗收界線

自製 C fixture 使用公開 headers；client／render API 分屬獨立 thread，採 64-byte 對齊且有界的 rgb0 buffer。播放器禁用使用者設定／scripts，用 software render、null audio 核對自製 320×180 三格式片段的四個固定 RGB 點。它驗 runtime 元件，不代表 Slint GUI、GPU、實際音訊或效能通過。

| 案例 | 首輪原 fixture | checkpoint 診斷 fixture |
| --- | --- | --- |
| MP4 | **failed**：25 秒後 SIGTERM，無結果 JSON | passed：EOF，72 幀符合像素 |
| WebM | passed：EOF，72 幀符合 | passed：EOF，72 幀符合 |
| OGV | passed：EOF，71 幀符合 | passed：EOF，72 幀符合 |
| MP4 刻意將紅色 expected 改成藍色 | 預期負例：exit 8、EOF、72 幀、0 幀符合 | 同一負例符合預期 |

原本失敗的同一 executable（SHA `6a617154…`）另開三個程序皆 exit 0、EOF／72 幀符合，約 4.48／3.25／3.25 秒。清理後的同 bytes 最小重現也通過。首輪停在哪一步仍未知，不能回填首輪通過，也不能稱為修正了產品 MP4 或既有 GPU 偶發問題。**重複性維持 `not_ready`**；首輪程式／來源、原始結果與有界診斷保留。這些時間未登記為 cold／warm 或 S8 效能數據。

## 本機來源交付

```text
/Users/ladesine/Documents/PTTStella-Slint-Deliverables/macos-x64-runtime-source-20261110-1f104ed/
```

在 Apple Silicon Mac、`/opt/homebrew` 環境依 README 執行：

```sh
cd replay
python3 bootstrap.py
python3 run-runtime.py
python3 validate-runtime.py
```

交付包含 runtime 封存、工具來源／wheel／metadata、patch／recipe、fixture、驗證程式與逐檔 manifest；Apple SDK／clang、Python 3.13、CMake／pkg-config／autoconf 是明列主機前提。這次為 Apple Silicon 交叉編譯與 Rosetta 執行。Intel 原生硬體、其他架構、Rust／Skia 的完整第三方閉包仍待驗，整體 `source_delivery: not_ready` 保留。

目前四份候選的主程式及 runtime 維持原樣，本次產物供來源驗證。下載依 [主計畫](slint-evaluation-plan.md#2026-11-10-最新候選)，產品 Rust／Slint 來源另見 [產品來源還原](slint-product-source.md)。

## 清理

新展開來源、build／install、工具／venv、冗餘 input／fixture 副本及成功診斷 binary 共 11 個根目錄／檔案、64,223 個 regular files 已移除。本批無 Cargo profile，沒有重建 Rust。archive hardlink 不代表多存一份 archive bytes。

未解反例只保留原 C executable 與 47 個 raw runtime library，共 64,770,576 bytes；原 load path 透過明確 compatibility symlink 指向這份唯一閉包。這是本機最小重現，README 說明其範圍。2,902 個原來源／候選保護檔案 hash 不變，共享工具／SDK／registry／VM、其他工作與使用者資料保留。

實際 command／PID／source owner／closure／像素／收據及 exact removed paths、resolved／UID／symlink／lsof、du／filesystem free 在 `.local/verification/slint-runtime-source-replay/`。free 為並行觀測，未將差額全歸因本批。3,151,611-byte build log 已無損 gzip 為 169,418 bytes，解壓 SHA 核對後移除原重複文字。S0–S8 持續，S9 未執行。

# 自製媒體 fixture

`colours.png`：64×32，左半純紅、右半純藍。`motion.gif`：64×32，整張純紅／純藍兩幀，每幀 300ms，循環播放。由 [標準庫產生器](../../../tooling/generate_media_fixtures.py) 建立；本專案作者以 CC0-1.0 提供，不含第三方影像。

桌面測試另外使用 WebView Canvas 產生 JPEG／WebP，以及 MediaRecorder 產生紅／藍 WebM，只保存於 target/verification。這些均為自製合成內容，不下載實站媒體。

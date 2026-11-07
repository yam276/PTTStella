# 自製媒體 fixture

`colours.png`：64×32，左半純紅、右半純藍。`motion.gif`：64×32，整張純紅／純藍兩幀，每幀 300ms，循環播放。由 [標準庫產生器](../../../tooling/generate_media_fixtures.py) 建立；本專案作者以 CC0-1.0 提供，不含第三方影像。

桌面測試另外使用 WebView Canvas 產生 JPEG／WebP，以及 MediaRecorder 產生紅／藍 WebM，只保存於 target/verification。這些均為自製合成內容，不下載實站媒體。

`once.gif`：同一紅／藍兩幀，但沒有循環 extension，原始延遲為 0／10ms；預期只播一輪，瀏覽器慣例將這兩種延遲延長為 100ms。`twice.gif`：Netscape repeat=1（初次加一次重播，共兩輪），每幀 30ms。同一標準庫產生器建立，仍以 CC0-1.0 提供；預期循環次數與延遲不從被測 decoder 計算。

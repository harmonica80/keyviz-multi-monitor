# v1.0.34 更新說明（測試版）

## 螢幕放大鏡第二階段：即時放大＋放大時可繪圖

- **即時放大**：改用 Windows 內建的全螢幕放大功能（Magnification API `MagSetFullscreenTransform`）。放大的是即時畫面，影片、動畫會繼續播放，不再是凍結的截圖。
- **放大時可以使用繪圖工具列**：放大狀態下直接點工具列的畫筆、箭頭、框選、文字等工具即可在畫面上標註。繪圖、復原（Ctrl+Z）、Delete、移入編輯物件（Alt）、圓形選單都照常可用；工具列與筆跡也會一起被放大顯示。
- **操作方式**（與第一階段相同）：
  - F6 或工具列放大鏡按鈕開啟；滾輪往前放大、往後縮小（1～8 倍）。
  - 畫面跟著滑鼠移動，游標下的那一點固定不動，所以畫筆一定畫在游標所在的位置。
  - 再按 F6、Esc 或放大鏡按鈕離開。
  - 游標工具時的「檢視模式」下，滑鼠只用來移動與縮放，按右鍵也能離開。選用繪圖工具時，右鍵仍是圓形選單。
  - 圓形選單開著時，Esc 先關閉選單，再按一次 Esc 才離開放大。
- **自動備援**：若這台電腦無法使用即時放大（例如 Windows 放大鏡正在執行），會自動改用第一階段的凍結畫面放大，並在工具列提示。開啟 JSON 檔編輯時，也一律用凍結畫面放大。

## 待實機確認

- 多螢幕：Windows 全螢幕放大會同時作用在所有螢幕。游標所在的螢幕會正確顯示放大的內容，其他螢幕（例如投影機）的顯示需要實測。
- 程式異常結束時，Windows 應會自動取消放大；若畫面停在放大狀態，重新啟動 Keyviz 再按一次 F6 後離開即可。

## 變更檔案

- `src-tauri/src/app/drawing_zoom.rs`：`ZoomSession.live`，新增 `start_live_magnifier`、`apply_live_zoom`、`stop_live_magnifier`、`zoom_tick`、`zoom_view_only`，凍結模式保留為備援。
- `src-tauri/src/app/native_drawing.rs`：即時放大時不再擋住繪圖指令（只有檢視模式擋）；訊息迴圈在即時放大時每 15ms 依游標更新放大位置。
- `src-tauri/src/app/event.rs`：圓形選單開啟時，Esc 不會離開放大。
- `src-tauri/Cargo.toml`：新增 windows crate feature `Win32_UI_Magnification`。
- 前端：選工具時不再自動離開放大（凍結模式由原生端處理），更新說明文字。

# v1.0.36 更新說明（測試版）

（放大時看到的條紋色塊，經使用者確認可能是同時執行的其他螢幕擷取工具造成，暫不處理。）

## 存成 PNG／JSON：自動選擇有繪圖的螢幕

- 只有一個螢幕有繪圖物件時，自動擷取那個螢幕，不論工具列在哪裡。
- 兩個以上的螢幕都有繪圖時，會先詢問：「擷取所有螢幕」或「只擷取工具列所在的螢幕」。擷取所有螢幕時，每個螢幕各存成一個檔案（例如 `20261117-01.png`、`20261117-02.png`），儲存成功的對話方塊會列出所有檔名。
- 都沒有繪圖時，擷取工具列所在的螢幕。
- JSON 每個檔案只包含該螢幕上的繪圖物件，日後各自開啟編輯。
- 開啟 JSON 編輯後再存檔，仍然以原檔名覆蓋原本那個螢幕。

## 存檔時不再擷取滑鼠的圓形外框

- 存成 PNG／JSON 時，跟著滑鼠的圓形外框（滑鼠標示）會被排除，不會出現在圖檔裡；螢幕上照常顯示。

## 變更檔案

- `src-tauri/src/app/drawing_export.rs`：`monitors_with_drawings`、`toolbar_monitor`、`export_targets`、`export_monitor`；`export_screen` 新增 `all_drawn` 參數。
- `src-tauri/src/app/drawing_export.rs`／`drawing_zoom.rs`：`capture_screen` 一律排除滑鼠圓形外框視窗（類別 `KeyvizNativeCursorOverlay`，`is_cursor_highlight_window`）。
- `src-tauri/src/app/export_io.rs`：`ExportResult` 新增 `paths`。
- `src-tauri/src/app/native_drawing.rs`：`ExportMonitorCount` 指令。
- `src-tauri/src/app/commands.rs`、`lib.rs`：`drawing_export` 新增 `allDrawn` 參數，新增 `drawing_export_monitor_count`。
- 前端：存檔前詢問、列出多個檔名。

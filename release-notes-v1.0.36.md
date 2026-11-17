# v1.0.36 更新說明（正式版，含 v1.0.34、v1.0.35 測試版內容）

## 螢幕放大鏡改為即時放大（v1.0.34、v1.0.35）

- 放大的是即時畫面，影片會繼續播放。放大時可以直接用工具列的繪圖工具標註；使用游標工具時，可以照常操作電腦。
- 縮放：游標工具時按 **Ctrl+滾輪**，繪圖工具時直接用滾輪。
- 畫面移動：預設在滑鼠靠近邊緣時才平滑移動，較穩定。可在設定 →「一般」改為「隨滑鼠即時移動」。
- 指標移到另一個螢幕時，畫面會切換到那個螢幕。Windows 的全螢幕放大會同時放大所有螢幕，無法只放大單一螢幕。
- 離開：F6、Esc 或工具列的放大鏡按鈕。
- 無法使用即時放大時（例如 Windows 放大鏡正在執行），會自動改用凍結畫面放大。

（測試時看到的條紋色塊，經確認可能是同時執行的其他螢幕擷取工具造成，暫不處理。）

## 存成 PNG／JSON：自動選擇有繪圖的螢幕

- 只有一個螢幕有繪圖物件時，自動擷取那個螢幕，不論工具列在哪裡。
- 兩個以上的螢幕都有繪圖時，會先詢問：「擷取所有螢幕」或「只擷取工具列所在的螢幕」。擷取所有螢幕時，每個螢幕各存成一個檔案（例如 `20261117-01.png`、`20261117-02.png`），儲存成功的對話方塊會列出所有檔名。
- 都沒有繪圖時，擷取工具列所在的螢幕。
- JSON 每個檔案只包含該螢幕上的繪圖物件，日後各自開啟編輯。
- 開啟 JSON 編輯後再存檔，仍然以原檔名覆蓋原本那個螢幕。

## 存檔時不再擷取滑鼠的圓形外框

- 存成 PNG／JSON 時，跟著滑鼠的圓形外框（滑鼠標示）會被排除，不會出現在圖檔裡；螢幕上照常顯示。

## 設定頁調整

- 「圓形選單設定」的說明文字由「六色色盤」改為「四色色盤」。
- 點選色塊（工具列顏色、圓形選單的快捷顏色）時，會先跳出 **12 種預設顏色**可直接點選；按「自訂…」才會開啟原本的完整選色器。
- 移除「滑鼠移入編輯物件」設定區塊（移入編輯一律對所有物件類型生效，仍以 Alt 開關）。
- 「自訂繪圖工具」清單新增「螢幕放大鏡」，可以和其他工具一樣調整順序或隱藏。舊設定會自動把它加在最後，也就是原本的位置。

## 開啟 JSON 編輯時的修正

- 修正：開啟 JSON 後，工具列周圍會露出一塊即時桌面（例如影片桌布），而且跟著工具列移動。現在工具列後方也顯示 JSON 的凍結畫面。
- 開啟 JSON 視為重新開始編輯，還原（Ctrl+Z）只會復原開啟之後的修改，不會再把開啟前、屬於其他螢幕或其他檔案的繪圖物件還原回來。

## 變更檔案

- `src-tauri/src/app/drawing_export.rs`：`monitors_with_drawings`、`toolbar_monitor`、`export_targets`、`export_monitor`；`export_screen` 新增 `all_drawn` 參數。
- `src-tauri/src/app/drawing_export.rs`／`drawing_zoom.rs`：`capture_screen` 一律排除滑鼠圓形外框視窗（類別 `KeyvizNativeCursorOverlay`，`is_cursor_highlight_window`）。
- `src-tauri/src/app/export_io.rs`：`ExportResult` 新增 `paths`。
- `src-tauri/src/app/native_drawing.rs`：`ExportMonitorCount` 指令。
- `src-tauri/src/app/commands.rs`、`lib.rs`：`drawing_export` 新增 `allDrawn` 參數，新增 `drawing_export_monitor_count`。
- 前端：存檔前詢問、列出多個檔名。
- `native_drawing.rs`：工具列穿透區在有 JSON 背景時改顯示背景；`drawing_export.rs` 的 `import_json` 開檔後重設還原紀錄。
- `src/components/settings/preset-color-picker.tsx`（新）：12 色預設＋自訂選色。
- `src/lib/drawing-tools.tsx`：新增 `ToolbarItemId`（工具＋`zoom`）、`TOOLBAR_ITEM_BY_ID`、`isDrawingToolItem`；工具列 store 的 items 納入 `zoom`，圓形選單只送出繪圖工具。
- `src/components/settings/drawing-toolbar.tsx`：移除移入編輯區塊，改用新選色器。

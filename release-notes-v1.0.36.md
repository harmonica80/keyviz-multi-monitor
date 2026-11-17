# v1.0.36 更新說明（測試版）

## 1. 放大時偶爾出現條紋色塊（嘗試修正）

- 現象：放大時，滑鼠指標右下方偶爾出現一大塊彩色橫條紋。
- 推測原因：Windows 全螢幕放大會另外繪製一份放大的滑鼠指標圖。Keyviz 換繪圖工具時，會把舊的自訂指標（畫筆、橡皮擦等）銷毀；若放大功能還在使用那份指標圖，就可能畫出亂碼。
- 修正：即時放大期間，被替換掉的指標先保留，離開放大後才銷毀。
- 這是根據畫面位置（色塊貼著指標）推測的原因，尚未在實機重現。若仍出現，請回報當時使用的工具、底下的程式，以及 `keyviz.log`。

## 2. 存成 PNG／JSON：自動選擇有繪圖的螢幕

- 只有一個螢幕有繪圖物件時，自動擷取那個螢幕，不論工具列在哪裡。
- 兩個以上的螢幕都有繪圖時，會先詢問：「擷取所有螢幕」或「只擷取工具列所在的螢幕」。擷取所有螢幕時，每個螢幕各存成一個檔案（例如 `20261117-01.png`、`20261117-02.png`），儲存成功的對話方塊會列出所有檔名。
- 都沒有繪圖時，擷取工具列所在的螢幕。
- JSON 每個檔案只包含該螢幕上的繪圖物件，日後各自開啟編輯。
- 開啟 JSON 編輯後再存檔，仍然以原檔名覆蓋原本那個螢幕。

## 變更檔案

- `src-tauri/src/app/drawing_export.rs`：`monitors_with_drawings`、`toolbar_monitor`、`export_targets`、`export_monitor`；`export_screen` 新增 `all_drawn` 參數。
- `src-tauri/src/app/export_io.rs`：`ExportResult` 新增 `paths`。
- `src-tauri/src/app/native_drawing.rs`：`ExportMonitorCount` 指令；即時放大時延後 `destroy_retired_cursors`。
- `src-tauri/src/app/commands.rs`、`lib.rs`：`drawing_export` 新增 `allDrawn` 參數，新增 `drawing_export_monitor_count`。
- 前端：存檔前詢問、列出多個檔名。

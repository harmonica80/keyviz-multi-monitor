# v1.0.44 更新說明（測試版）

- **文字外框**：選取「文字」工具時，顏色區上方出現「外框」核取方塊與色塊。勾選後，新增（或編輯中）的文字會在字的周圍加上外框，預設為**白色**；點色塊可改選外框顏色（白、黑、黃、紅、藍、綠）。設定會儲存，存成 JSON 時一併記錄（`"outline"`），舊檔案預設無外框。外框粗細隨字級縮放。

## 技術備註
- Rust：`DrawingItem::Text` 與 `EditSession` 新增 `outline: Option<COLORREF>`；`draw_text` 先用外框色在 8 個方向偏移描繪，再以文字色描繪；新增 `DrawingCommand::SetTextOutline`、`drawing_set_text_outline`。
- 前端：store 版本 13（`textOutline`）；`drawing-toolbar-view.tsx` 新增 `text-outline` unit。
- 僅影響新增或正在編輯的文字；已存在的文字不會因切換勾選而改變。

# v1.0.43 更新說明（測試版）

1. **工具列預設位置新增「上方」「下方」**：設定 → 繪圖工具列 → 「工具列開啟位置」現在有左側、右側、上方、下方。選上方或下方時，繪圖工具列開啟就是**橫式**，置中貼在主要螢幕的上緣（下緣貼在系統工作列之上）。切換左／右則回到直式。每次開啟繪圖時都會依此設定重新擺放。
2. **序號標記可切換為圓角矩形**：選取「序號標記」工具時，顏色區上方出現兩個小圖示：圓形、圓角矩形。選擇後新增的序號都用該樣式，游標預覽也跟著改變。樣式會儲存在設定中，且存入 JSON（`"rounded": true`），舊檔案預設為圓形。

## 技術備註
- Rust：`DrawingItem::Number` 新增 `rounded`；新增 `DrawingCommand::SetNumberStyle`、`drawing_set_number_style` 命令、`rounded_square_points`（GDI+ 多邊形，與圓形相同外框尺寸）；JSON 匯出入 `rounded`。
- 前端：store 版本 12（`side` 擴充為 left/right/top/bottom、新增 `numberStyle`）；`screen-drawing.tsx` 的 `applySideLayout` 在載入、設定變更、重新開啟時套用橫式／直式並貼邊。
- Windows 專屬程式碼僅靠 CI 編譯驗證，請實機確認。

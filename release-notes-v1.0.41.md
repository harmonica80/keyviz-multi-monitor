# v1.0.41 更新說明（測試版）

1. **橫式／直式感應範圍**：放開拖曳時，依游標位置決定（以螢幕比例計算，與解析度、縮放無關）：
   - 上緣約 6%、下緣約 9.5%（含系統工作列）→ 橫式並貼齊該邊（下緣貼在工作列之上）。
   - 左緣約 9%、右緣約 8.5%（整個高度，四個角落歸左右）→ 直式並貼齊該側。
   - 中間區域 → 維持目前的橫式／直式，工具列自由擺放，不貼齊。
2. **跨螢幕下方空白**：調整視窗大小時，改用網頁實際的 `devicePixelRatio` 計算實體像素（原本用 Windows DPI，縮放剛改變時兩者可能暫時不同）；拖曳結束與縮放變更後，持續檢查約 3.5 秒，視窗與內容大小不符就重新調整。

## 技術備註
- `resize_drawing_toolbar` 新增參數 `device_pixel_ratio`（前端傳 `window.devicePixelRatio`，缺少時退回 `GetDpiForWindow`）。
- `screen-drawing.tsx`：`handleToolbarDrag` 改為區域判斷；`verifyBurstRef` 在拖曳結束、`onScaleChanged` 後多次 `verifyFit`。

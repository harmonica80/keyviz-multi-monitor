# v1.0.40 更新說明（測試版）

依 v1.0.39 實機測試回饋調整。

1. **設定頁**：左右側、圖示大小的選項，目前選中的值改用實心主色底色顯示。
2. **設定頁順序**：「工具列開啟位置」與「工具列圖示大小」移到「工具列顏色」之前（頁面最上方）。
3. **圓形選單**：右側「文字」扇區切成兩半——上半仍是文字，下半是新的「截圖並釘選」（剪刀＋圖釘）。點選後與工具列的截圖按鈕相同。
4. **橫式觸發範圍加大**：游標放開時，距離螢幕**工作區**（不含系統工作列）上緣或下緣 130 px 內就會變橫式，不必再拖到系統工作列；橫式貼齊時也會停在工作列之上。
5. **跨螢幕空白**：調整視窗大小時改用 Windows 即時 DPI（`GetDpiForWindow`），且縮放比例改變後會連續重新量測；另外視窗大小與內容不符時會自動修正（最多 5 次），避免工具列下方出現空白區塊。

## 技術備註
- `drawing_radial.rs`：新增 `RADIAL_SNIP_SLOT = 20`、`radial_slots()`、`RadialAction::Snip`；Text 扇區改為 0°–30°，Snip 為 30°–60°；點選時發出 `drawing-radial-snip` 給工具列頁面呼叫 `snip_start`（儲存資料夾設定在前端）。`drawing_radial_render.rs` 新增 `draw_radial_snip_icon`。
- `lib.rs::resize_drawing_toolbar` 以 `GetDpiForWindow` 取得縮放。
- 前端 `screen-drawing.tsx`：`verifyFit`、`onScaleChanged` 多次重量、工作區判斷。
- Windows 專屬程式碼（圓形選單、DPI）僅靠 CI 編譯與測試驗證，請實機確認。

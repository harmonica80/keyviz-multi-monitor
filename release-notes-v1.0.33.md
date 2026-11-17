# v1.0.33 更新說明

## 新功能：螢幕放大鏡（類似 ZoomIt，第一階段）

- **開啟方式**：按 **F6**（可在設定 →「螢幕放大鏡」更改），或點繪圖工具列上的放大鏡按鈕。
  - 不必先開繪圖工具列：在一般桌面按 F6 也能直接放大；離開放大鏡後就回到原本的桌面。
  - 在繪圖中按 F6／放大鏡按鈕：離開後回到原本的繪圖工具與筆跡。
- **操作**：
  - 以滑鼠所在位置為中心放大（預設 2 倍）。
  - 滾輪往前放大、往後縮小（1～8 倍，每格 1.25 倍）；縮到 1 倍即原畫面。
  - 移動滑鼠可查看其他位置，游標下的那一點固定不動（與 ZoomIt 相同）。
  - 再按一次 F6、按 Esc、按滑鼠右鍵，或再點一次放大鏡按鈕即可離開。
- **放大的是按下當下的畫面快照（凍結畫面）**：會包含目前的繪圖物件，但不含 Keyviz 自己的工具列、按鍵顯示與點擊特效（擷取時自動排除，螢幕上照常顯示）。需要 Windows 10 2004 以上。
- 放大期間暫停繪圖、圓形選單、Ctrl+Z 與 Delete；在工具列點選其他繪圖工具會先離開放大鏡。

## 工具列顏色改為四個、可自訂

- 繪圖工具列的顏色由固定六色改為**四色**（預設紅、綠、藍、黑），可在設定 →「繪圖工具列」→「工具列顏色」自行指定。
- 選取物件時（移入編輯物件模式或框選工具），工具列的勾勾會自動移到**該物件的顏色**；顏色不在這四色中、或選了多個不同顏色的物件時，不顯示勾勾。取消選取後，勾勾回到目前畫筆的顏色。
- 修正：工具列高度原本固定上限 820 像素，加上放大鏡按鈕後底部按鈕被截斷；現在上限改為工具列所在螢幕的高度。

## 快速鍵選擇說明

- 預設 F6：和 F7（回到游標）、F8（開關繪圖）排在一起；在瀏覽器／Office 中 F6 只會切換焦點，不會改到內容。
- 未採用：Ctrl+1（與瀏覽器切換分頁、Word 單行間距衝突）、Win+加號（Windows 內建放大鏡）、F4（Alt+F4 關視窗時會誤觸；Excel 會切換絕對參照）。

## 變更檔案

- `src-tauri/src/app/zoom_math.rs`（新，跨平台、含單元測試）：放大倍率與雙線性內插放大運算。
- `src-tauri/src/app/drawing_zoom.rs`（新，`include!` 進 `native_drawing.rs`）：`ZoomSession`、進入／離開放大、滑鼠平移與滾輪、`zoom_active()`。
- `src-tauri/src/app/native_drawing.rs`：`DrawingCommand::Zoom`、`set_zoom()`；放大期間的指令過濾；`refresh_overlay_surface` 的放大分支與 `present_overlay_surface` 拆分；滑鼠／右鍵／游標處理。
- `src-tauri/src/app/drawing_export.rs`：`capture_screen` 新增 `all_own_windows` 參數（放大鏡用，排除本程式所有可見視窗；PNG 存檔行為不變）。
- `src-tauri/src/app/event.rs`：放大快速鍵（Hook＋備援輪詢，共用防重入時間窗）；放大中 Esc 只離開放大鏡。
- `src-tauri/src/lib.rs`、`commands.rs`、`state.rs`：`toggle_screen_zoom`、`drawing_set_zoom` 指令、`drawing_zoom_shortcut` 設定（預設 F6）。
- 前端：工具列放大鏡按鈕、設定頁快速鍵列、i18n。
- 工具列顏色：`src/lib/radial-colors.ts`（`DEFAULT_TOOLBAR_COLORS`／`normalizeToolbarColors`）、`stores/drawing_toolbar.ts`（`toolbarColors`，persist version 9）、`settings/drawing-toolbar.tsx`、`drawing-toolbar-view.tsx`（`colors` 屬性）。
- 勾選物件顏色：`native_drawing.rs` 的 `DrawingSelectionPayload` 新增 `color`（`selected_common_color`），換色後重送；前端選取中以物件顏色顯示勾勾。
- 工具列高度：`lib.rs` 的 `resize_drawing_toolbar` 上限改為所在螢幕高度。

# Keyviz 鍵盤按鍵顯示器與螢幕繪圖

這是基於 [mulaRahul/keyviz](https://github.com/mulaRahul/keyviz) 修改的 Windows 版本，適合教學、簡報、錄影與直播展示使用。此版本加強了多螢幕支援、繁體中文介面、滑鼠醒目提示，以及可直接在螢幕上標註的繪圖工具。

This is a customized Windows build based on [mulaRahul/keyviz](https://github.com/mulaRahul/keyviz). It is designed for tutorials, presentations, screen recording, and live teaching workflows, with multi-monitor support, Traditional Chinese UI, cursor highlighting, and on-screen drawing tools.

## 下載最新正式版

**[下載 Keyviz Windows 免安裝版（keyviz-portable.exe）](https://github.com/harmonica80/keyviz-multi-monitor/releases/latest/download/keyviz-portable.exe)**

目前最新正式版為 **v1.0.25**。上方連結直接下載正式版 EXE，不必登入 GitHub，也不必從 Actions 下載測試產物。

**v1.0.25** 統一工具列與圓形選單的六款主題，預設為 01 珍珠白，並採用圓角工具列及圓形選取效果。保留左 Alt＋滑鼠右鍵、隱藏工具列繼續繪圖，以及鍵盤顯示與設定快捷入口。

- [v1.0.25 正式版與更新說明](https://github.com/harmonica80/keyviz-multi-monitor/releases/tag/v1.0.25)
- [v1.0.25 Windows 建置與自動測試紀錄](https://github.com/harmonica80/keyviz-multi-monitor/actions/runs/42447234949)
- [v1.0.25 對應原始碼](https://github.com/harmonica80/keyviz-multi-monitor/tree/v1.0.25)
- 更新前請先結束舊版，再執行下載的 `keyviz-portable.exe`，並確認標題列顯示 `v1.0.25`。

The latest stable release is **v1.0.25**. The direct EXE download above does not require GitHub sign-in. It unifies toolbar and circular-menu themes with Pearl as the default and rounded controls. Left Alt + right mouse, hidden-toolbar drawing, keyboard-display and settings controls remain available. Exit the old version before running the new EXE and confirm `v1.0.25` in the title bar. The v1.0.25 tag contains the exact source used for this build.

## v1.0.26 測試版：一致預覽與自訂圓形選單顏色

- 設定頁的工具列預覽與實際工具列共用顯示元件，主題、按鈕順序、顯示項目、色盤與底部功能保持一致；預覽不執行繪圖操作。
- 「設定 → 繪圖工具列 → 圓形選單設定」可自訂兩個快捷顏色，預設為紅色與藍色，設定會保存；恢復預設工具列時一併還原。
- 兩個快捷色不改動工具列的六色色盤，也不跟隨主題變色。
- 選單中央移除叉號，改為空白，仍可點擊中央、按 Esc 或右鍵關閉；外圈叉號印章保留。
- 本節為測試版功能，正式下載仍維持 v1.0.25。

The v1.0.26 test build shares the real toolbar renderer with the settings preview, adds two persistent customizable quick colors (red/blue defaults), and leaves the dismissible radial hub blank. The cross-mark drawing tool remains unchanged. Stable downloads remain v1.0.25.

## v1.0.25：統一繪圖主題

- 設定 → 繪圖工具列 → 繪圖工具主題，可同步切換工具列與圓形選單配色。
- 六款依序為：01 珍珠白（預設）、02 石墨黑、03 深海藍、04 鵝黃色、05 亮橘色、06 暖沙色。
- 珍珠白與鵝黃色底色為 `#fcfbfc`；亮橘色採淺粉紅底色搭配橘色重點。
- 圓角工具列、圓形工具選取效果及圓角選單功能按鈕；不使用霧面模糊材質。
- 舊圓形選單專用樣式已由統一主題取代，升級後預設為 01；工具順序、顯示項目、快捷鍵與繪圖顏色不變。
- 已晉升正式版，沿用測試通過的同一個 EXE；47 項原生測試、47 項快捷鍵檢查及主題偏好測試通過。

The v1.0.25 stable build unifies toolbar and circular-menu colors with six themes (Pearl by default), rounded controls, and shared palette data. Legacy menu-only styles reset to Pearl; tool order, visibility, shortcuts and drawing colors are preserved. The stable EXE is identical to the tested artifact.

## 功能介紹

### 圓形選單與隱藏工具列

- 工具列原關閉列左右分為「隱藏」與「關閉」，不增加高度。隱藏保留目前工具與標註；關閉仍結束繪圖並清除標註。
- 程式啟動後即可在游標模式按左 Alt＋滑鼠右鍵叫出選單，不必先開啟工具列；一般右鍵與其他繪圖工具的 Alt／右鍵操作維持原功能。
- 選單面板包含紅色、黑色、復原、重做、眼睛圖示切換工具列、鍵盤按鍵顯示切換及設定入口；選色或復原／重做後保持開啟，選工具後關閉。
- 舊圓形選單專用樣式已由統一主題取代，可在設定 → 繪圖工具列 → 繪圖工具主題選擇六款配色並保存。
- 隱藏後可用 F7 或自訂回到游標快捷鍵返回正常滑鼠操作，再以左 Alt＋右鍵選工具。六種樣式均有局部柔和陰影。
- 47 項原生測試及 47 項快捷鍵相關檢查通過；不同電腦的鍵鼠、焦點、多螢幕及錄影互動仍需驗證。

The circular menu supports Left Alt + right mouse immediately after startup, hidden-toolbar drawing, an eye toggle, keyboard-display and settings controls, plus shadows on all six styles.

### 文字大小

文字預設與最小大小為 15，最大為 45（字型像素高度 60～180），可用滑鼠滾輪放大至原預設的 3 倍，工具列大小控制同步支援新範圍。既有文字不會自動改大小；調整時才套用新範圍，其他工具不變。

- 37 項原生測試及 47 項快捷鍵檢查通過，不同電腦的實際操作仍需驗證。

### 文字編輯功能

- 新文字預設大小為 15，可在 15～45 之間調整，不自動改變既有文字。
- 啟用物件編輯與文字類型偏好後，可雙擊文字修改內容，保留位置、旋轉、顏色及群組，支援復原／重做。
- 文字內容可點擊中間插入／刪除、拖曳選取、方向鍵移動、Shift 選取與 Ctrl+A 全選；文字物件可在編輯／選取時換色，顏色套用至整個文字物件。
- 已通過 37 項原生測試與 47 項快捷鍵相關檢查；不同電腦的實際鍵鼠操作仍需驗證。

以下整理 v1.0.25 正式版的功能；各版本更新內容請參閱對應的 Release 說明。

- 即時顯示鍵盤按鍵與快速鍵，方便教學、簡報、錄影與直播。
- 支援多螢幕環境，可選擇按鍵顯示所在螢幕與位置。
- 免安裝單一執行檔，Windows 可直接執行。
- 設定介面支援繁體中文與英文切換，預設為繁體中文。
- 系統通知列右鍵選單會跟隨語系顯示中文或英文。
- 可自訂按鍵篩選、顯示歷史記錄、排列方向、最大顯示數量與顯示/隱藏快速鍵。
- 內建多種按鍵樣式，並可在設定畫面預覽。
- 可調整按鍵顯示時間、動畫效果與動畫速度。
- 滑鼠游標醒目效果可調整大小、顏色、透明度與線條粗細。
- 內建螢幕繪圖工具，可直接在影片、網頁、簡報或任何視窗上標註。
- 螢幕繪圖採用原生 Windows 透明繪圖視窗，不會抓取靜態畫面，影片可持續播放。
- 繪圖工具包含游標、框選物件、畫筆、橡皮擦、直線、箭頭、矩形、圓形、文字、序號標記、勾選與叉號印章，並提供顏色、大小、復原、重做與清空操作。
- 繪圖工具列可移動，並可在設定中選擇開啟於主要螢幕的左側或右側（預設右側）。
- 可拖曳調整工具列按鈕順序，並選擇要顯示或隱藏的工具；游標工具固定保留。
- 可依物件類型開啟或關閉「滑鼠移入編輯物件」；移入物件後可拖曳移動、以滾輪縮放。畫筆、矩形與圓形只在線條附近觸發，方便在空白內部繼續繪圖。
- 雙圈純圖示工具選單可在滑鼠附近開啟，支援自訂快捷鍵（預設 `Q`）及畫布右鍵操作，減少移動到工具列的距離（詳見下方）。
- 繪圖色塊會在目前選取顏色上顯示勾選標記，並保留原本顏色。
- 快速鍵 `F8` 可開啟或關閉螢幕繪圖。
- 在螢幕繪圖模式下按 `F7` 可切回游標/滑鼠模式。
- `Delete` 只在繪圖工具狀態下作用：框選工具刪除選取物件，其他繪圖工具清空標註；游標工具不處理 Delete。文字輸入時也不刪除繪圖。
- 支援繪製、移動、縮放、旋轉、序號加減、群組、刪除與清空的復原／重做，重做快捷鍵為 `Ctrl + Shift + Z`。
- v1.0.16 起，單按並放開 `Alt` 切換「滑鼠移入編輯物件」開／關，不必持續按住。每次開始繪圖及切回游標時關閉；各物件類型偏好仍生效，切換繪圖工具保留本次狀態。繪圖工具提示附有 Alt 操作說明，游標與框選工具除外。v1.0.17 補上繪圖視窗取得焦點後的 Alt 處理。
- v1.0.17 起，文字輸入時按右鍵會先完成文字再開啟圓形選單；工具列區域從繪圖輸入層排除，避免被文字工具取得焦點後的視窗擋住。
- 開啟或重新切回設定視窗時先切換游標工具，保留繪圖；開啟過程暫不接受切換繪圖工具，避免輸入層擋住設定。
- 圓形選單下方偏左的垃圾桶可清除全部標註，之後可用復原取回；原有直線工具保留在相鄰的下方偏右區域。
- 工具列與關於頁提供「恢復透明顯示層」，重新建立繪圖及游標醒目視窗並保留標註。
- 「設定 → 關於 → 匯出診斷資料」可匯出螢幕、繪圖狀態、視窗、滑鼠 hook 與近期錯誤資訊，協助排查教室電腦的顯示問題。

## 復原快捷鍵模式限制

- 游標工具：Ctrl+Z 與 Ctrl+Shift+Z 由目前應用程式處理，不改變螢幕繪圖。
- 繪圖工具：快捷鍵仍可復原／重做繪圖；設定視窗在前景或正在輸入文字標註時不觸發。
- 工具列上的復原／重做按鈕仍可在游標模式使用；自訂復原快捷鍵也遵循上述限制。
- [v1.0.15 操作檢查項目](https://github.com/harmonica80/keyviz-multi-monitor/blob/codex/fix-drawing-toolbar-editable/release-notes-v1.0.15.md)

## 雙圈圓形工具選單

使用繪圖工具時，無須記住每個工具的快捷鍵，也不用把滑鼠移回工具列：

1. 按 `F8` 開啟繪圖工具列，選擇畫筆或其他繪圖工具。
2. 按 `Q`，或在畫布按滑鼠右鍵，在滑鼠附近開啟雙圈選單。剛從工具列切換工具後，也可直接按 Q，不必先點畫布。
3. 點選圖示切換工具並收起選單；只是移過圖示不會切換工具。
4. 再按快捷鍵、按 `Esc`、按右鍵、點中央叉號或點選單外側，可取消選單，不會新增繪圖。選單開啟時 Esc 只取消選單，未開選單時 Esc 可離開繪圖工具列。

| 圈別 | 工具 |
| --- | --- |
| 內圈 | 畫筆、箭頭、序號標記、橡皮擦、矩形、圓形 |
| 外圈 | 勾選記號、叉號記號、文字、直線、清除畫面、框選物件、游標 |

在「**設定 → 繪圖工具列 → 圓形工具選單快捷鍵**」可調整：

- **鍵盤快捷鍵**：預設 Q，可改為其他按鍵或搭配 Ctrl、Shift、Alt。點選錄製區後按 Backspace 或 Delete，可停用鍵盤觸發。
- **滑鼠右鍵開啟圓形選單**：預設啟用，可獨立關閉；關閉右鍵觸發後，鍵盤快捷鍵仍可使用。設定會保存，恢復預設工具列時會還原為 Q 與右鍵啟用。

選單保留固定的雙圈位置，隱藏的工具留下空格，不會讓其他工具重新排列。v1.0.16 將外圈底部區域分成直線與清除兩格，清除按鈕固定顯示。小尺寸選單直徑為 156 邏輯像素，依螢幕縮放比例調整，靠近邊緣時會向螢幕內側移動。

**操作範圍：**游標模式、設定視窗、按住左鍵繪圖，以及黑幕分層測試期間，不開啟選單。文字標註輸入期間不以鍵盤快捷鍵開啟選單；v1.0.17 起可按右鍵完成文字並開啟選單。右鍵開啟只適用於繪圖畫布，不改動工具列既有右鍵功能，例如恢復顯示按鈕的分層測試。

此版本已通過自動化檢查，但不同教室電腦、多螢幕與錄影軟體的實際互動仍需驗證；此功能不代表所有黑幕或置頂問題均已解決。

## Features

This list describes the v1.0.25 stable release, including unified drawing themes, the circular menu and hidden toolbar, text sizes 15–45, double-click editing, caret navigation, text selection, and whole-object text color changes. See each Release's notes for version-specific changes.

- Real-time keyboard and shortcut visualization for tutorials, presentations, screen recording, and live demos.
- Multi-monitor support with configurable display and overlay position.
- Portable Windows single executable; no installer required.
- Settings UI supports Traditional Chinese and English, with Traditional Chinese as the default language.
- System tray menu follows the selected language.
- Configurable key filtering, key history, layout direction, maximum display count, and show/hide shortcut.
- Multiple keycap styles with visual previews in the settings window.
- Adjustable key display duration, animation type, and animation speed.
- Cursor highlight with adjustable size, color, opacity, and line thickness.
- Built-in screen drawing tools for annotating videos, webpages, slides, or any application window.
- Native Windows transparent drawing overlay, so videos keep playing while you draw on top.
- Drawing tools include pointer, object selection, pen, eraser, line, arrow, rectangle, ellipse, text, numbered markers, check/cross stamps, colors, sizes, undo, redo, and clear.
- Movable drawing toolbar with a setting to open it on the left or right side of the primary display (right by default).
- Reorder toolbar tools and choose which tools are visible; the pointer tool always remains available.
- Hover-edit preferences for each object type allow dragging and wheel resizing. Pen, rectangle, and ellipse hit testing follows the outline rather than the empty interior.
- Selected drawing color is marked with a check while keeping the original color visible.
- Press `F8` to toggle screen drawing.
- Press `F7` while screen drawing is active to return to pointer/mouse mode.
- `Delete` only affects drawings while a drawing tool is active: the selection tool deletes selected objects, while other drawing tools clear annotations. Pointer mode and text entry do not delete drawings.
- Undo/redo covers drawing and object edits; redo uses `Ctrl + Shift + Z`. Since v1.0.16, tap and release `Alt` to toggle hover editing. New sessions and pointer mode reset it to off; object-type preferences still apply. Alt combinations do not toggle it.
- Settings opening/focus returns to pointer mode before further drawing input. A clear-all button in the lower-left part of the circular menu shares the toolbar's undoable clear action; Line remains next to it.
- Since v1.0.17, right-clicking while typing commits text and opens the circular menu. The input surfaces exclude the toolbar region to keep its buttons clickable after canvas activation. Pointer and selection tooltips omit the Alt hint.
- Restore transparent overlays without discarding annotations, and export diagnostic JSON from Settings > About.

### Undo/Redo Shortcut Scope

Since v1.0.15, undo/redo keyboard shortcuts only edit drawings with a drawing tool selected. Pointer mode leaves these keys to the current application. Foreground settings and active text annotation entry do not trigger drawing history shortcuts. Explicit toolbar undo/redo buttons still work in pointer mode.

### Two-Ring Circular Menu

- Press the configurable shortcut (default `Q`) or right-click the drawing canvas to open a compact icon menu near the pointer. Q also works immediately after clicking a toolbar tool, without first clicking the canvas.
- Click an icon to select a tool. Press the shortcut again, Esc, or the right mouse button, or click the center/outside area to dismiss the menu without drawing.
- Inner ring: pen, arrow, number marker, eraser, rectangle, ellipse. Outer ring: check mark, cross mark, text, line, object selection, pointer.
- Configure the shortcut and the independent right-click toggle in Settings > Drawing Toolbar. Right-click opening is enabled by default; resetting the toolbar restores it and Q.
- The 156-logical-pixel menu scales with display DPI. Hidden tools leave empty sectors so other icons retain their positions.
- Pointer mode, settings, text entry, held-left-button drawing, and layer diagnostics do not open the menu. Existing toolbar context menus remain unchanged.
- Windows hardware, mixed-DPI monitors, and recording workflows still require real-world testing; this feature does not claim to resolve every overlay issue.

## 下載與使用

Windows 使用者可直接執行：

```text
release/keyviz-portable.exe
```

不需要安裝。使用者設定會儲存在目前 Windows 使用者設定檔中。

## Download And Use

For Windows, run the portable executable directly:

```text
release/keyviz-portable.exe
```

No installer is required. User settings are stored in the current Windows user profile.

## 建置方式

```powershell
npm install
npm run build
npx tauri build --no-bundle
```

執行檔會產生於：

```text
src-tauri\target\release\keyviz.exe
```

更多免安裝版本的建置說明請參考 [PORTABLE.md](PORTABLE.md)。

## Build

```powershell
npm install
npm run build
npx tauri build --no-bundle
```

The executable is generated at:

```text
src-tauri\target\release\keyviz.exe
```

See [PORTABLE.md](PORTABLE.md) for more portable build notes.

## 授權與致謝

本專案基於開放原始碼專案 [Keyviz](https://github.com/mulaRahul/keyviz) 修改，感謝 Rahul Mula 與所有貢獻者。

## Credits

This project is based on the open-source [Keyviz](https://github.com/mulaRahul/keyviz). Thanks to Rahul Mula and all contributors.

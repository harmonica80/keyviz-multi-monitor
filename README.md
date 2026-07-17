# Keyviz 鍵盤按鍵顯示器與螢幕繪圖

這是基於 [mulaRahul/keyviz](https://github.com/mulaRahul/keyviz) 修改的 Windows 版本，適合教學、簡報、錄影與直播展示使用。此版本加強了多螢幕支援、繁體中文介面、滑鼠醒目提示，以及可直接在螢幕上標註的繪圖工具。

This is a customized Windows build based on [mulaRahul/keyviz](https://github.com/mulaRahul/keyviz). It is designed for tutorials, presentations, screen recording, and live teaching workflows, with multi-monitor support, Traditional Chinese UI, cursor highlighting, and on-screen drawing tools.

## 功能介紹

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
- 繪圖工具包含游標模式、畫筆、橡皮擦、直線、箭頭、矩形、圓形、文字、顏色、筆刷粗細、復原與清空。
- 繪圖工具列可移動，預設顯示於畫面右側。
- 繪圖色塊會在目前選取顏色上顯示勾選標記，並保留原本顏色。
- 快速鍵 `F8` 可開啟或關閉螢幕繪圖。
- 在螢幕繪圖模式下按 `F7` 可切回游標/滑鼠模式。
- 在螢幕繪圖模式下按 `Del` 可清空目前螢幕上的繪圖。

## Features

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
- Drawing tools include pointer mode, pen, eraser, line, arrow, rectangle, ellipse, text, colors, brush sizes, undo, and clear.
- Movable drawing toolbar, shown on the right side by default.
- Selected drawing color is marked with a check while keeping the original color visible.
- Press `F8` to toggle screen drawing.
- Press `F7` while screen drawing is active to return to pointer/mouse mode.
- Press `Del` while screen drawing is active to clear the current drawings.

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

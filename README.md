# Keyviz 鍵盤按鍵顯示器與螢幕繪圖

Keyviz 鍵盤按鍵顯示器與螢幕繪圖是一個以
[mulaRahul/keyviz](https://github.com/mulaRahul/keyviz) 為基礎改造的教學、
簡報與螢幕錄製輔助工具。這個版本著重於 Windows 免安裝使用、多螢幕按鍵顯示、
滑鼠游標醒目效果，以及可直接在螢幕上標註的繪圖工具。

This project is a customized build based on
[mulaRahul/keyviz](https://github.com/mulaRahul/keyviz). It is designed for
tutorials, presentations, screen recording, and live teaching workflows, with
portable Windows usage, multi-monitor key visualization, cursor highlighting,
and on-screen annotation tools.

## 功能介紹

- 鍵盤按鍵即時顯示，適合教學影片、直播、簡報與操作示範。
- 支援多螢幕環境，按鍵顯示可在目前螢幕配置下正確呈現。
- Windows 免安裝單一執行檔，下載後即可執行。
- 設定介面支援繁體中文與英文切換。
- 提供多種按鍵外觀樣式，可在設定中預覽並選擇。
- 滑鼠游標醒目效果，可調整大小、顏色、透明度與線條粗細。
- 內建螢幕繪圖工具，可使用畫筆、橡皮擦、直線、箭頭、矩形、圓形與文字標註。
- 工具列可移動，方便在錄影或簡報時避開重要畫面。

## Features

- Real-time keyboard visualization for tutorials, live demos, presentations, and
  screen recording.
- Multi-monitor support so key overlays display correctly across current screen
  layouts.
- Portable Windows single executable; no installer required.
- Settings UI with Traditional Chinese and English language switching.
- Multiple keycap styles with visual previews in the settings window.
- Cursor highlight with adjustable size, color, opacity, and line thickness.
- Built-in screen drawing tools, including pen, eraser, line, arrow, rectangle,
  ellipse, and text annotation.
- Movable drawing toolbar for easier use during recording or presentation.

## 下載與使用

Windows 使用者可直接執行免安裝版：

```text
release/keyviz-portable.exe
```

程式本身不需要安裝；設定資料會保存在目前 Windows 使用者的應用程式資料目錄中。

## Download And Use

For Windows, run the portable executable directly:

```text
release/keyviz-portable.exe
```

No installer is required. User settings are stored in the current Windows user
profile.

## 建置方式

```powershell
npm install
npm run build
npx tauri build --no-bundle
```

產生的執行檔位於：

```text
src-tauri\target\release\keyviz.exe
```

更多免安裝版資訊請參考 [PORTABLE.md](PORTABLE.md)。

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

## 致謝 / Credits

本專案修改自開源工具 [Keyviz](https://github.com/mulaRahul/keyviz)，感謝原作者
Rahul Mula 與所有貢獻者。

This project is based on the open-source
[Keyviz](https://github.com/mulaRahul/keyviz). Thanks to Rahul Mula and all
contributors.

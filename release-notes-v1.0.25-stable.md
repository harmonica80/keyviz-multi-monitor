# Keyviz 鍵盤按鍵顯示器與螢幕繪圖 v1.0.25

## 繁體中文

- 工具列與圓形選單統一由「繪圖工具主題」管理，取代原本的圓形選單專用樣式。
- 六款主題：01 珍珠白（預設）、02 石墨黑、03 深海藍、04 鵝黃色、05 亮橘色、06 暖沙色。
- 珍珠白與鵝黃色底色為 `#fcfbfc`；亮橘色為淺粉紅底搭配橘色重點。
- 圓角工具列、圓形選取效果與圓角選單功能按鈕，深色主題同步調整圖示對比。
- 保留圓形選單柔和陰影；工具列維持不透明原生視窗，不新增霧面模糊或 layered 透明材質。
- 舊樣式設定升級後回到 01 珍珠白，保留工具順序、顯示項目、快捷鍵、物件編輯偏好與繪圖顏色。
- 工具列移到不同 DPI 螢幕時更新原生圓角裁切。

下載：[keyviz-portable.exe](https://github.com/harmonica80/keyviz-multi-monitor/releases/download/v1.0.25/keyviz-portable.exe)

升級前請先結束舊版，再開啟新版，確認標題列為 v1.0.25。

## English

- Six unified drawing themes for the toolbar and circular menu, with Pearl as the default.
- Rounded controls, theme-aware icons and DPI-aware native toolbar clipping.
- Legacy menu-only styles reset to Pearl; tool order, visibility, shortcuts, hover preferences and drawing colors are retained.
- Keeps opaque toolbar rendering and existing radial shadows, without blur or new layered toolbar transparency.

## Verification

- Promotes the exact tested EXE; no rebuild or code changes during promotion.
- Source: `5be5a6c3dd86cdb53f0879232353c20d6d5023a7`.
- [Windows build and tests](https://github.com/harmonica80/keyviz-multi-monitor/actions/runs/42447234949): 47 native tests, 47 shortcut checks, and palette/preference regression tests passed.
- Browser UI checks used a mocked Tauri backend. Six native menu preview sets were inspected.
- Windows 10/11, mixed-DPI and recording workflows remain hardware-dependent and require real-world testing; this release does not claim to resolve every historical overlay issue.
- AMD64 portable EXE; 6,424,576 bytes; file/product version 1.0.25.
- SHA-256: `6AB355CF686C4553959CBAB8DDE2F10ABA295ABFDF559D417BC5ACCA973812C7`.

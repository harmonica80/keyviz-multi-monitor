# Keyviz 鍵盤按鍵顯示器與螢幕繪圖 v1.0.26

## 繁體中文

- 設定頁的工具列預覽與實際工具列共用顯示元件，主題、按鈕順序、顯示項目、色盤、復原／重做與底部功能外觀一致。預覽不執行繪圖操作。
- 新增「設定 → 繪圖工具列 → 圓形選單設定」，可自訂並保存兩個快捷顏色，預設為紅色與藍色；恢復預設工具列時一併還原。
- 快捷色獨立於六款主題與工具列六色色盤，不會隨主題變色。
- 圓形選單中央移除叉號並維持空白，點擊中央仍可關閉選單；Esc、右鍵與外圈叉號印章功能保留。
- 自訂黑色色塊在原生透明合成中維持不透明。

下載：[keyviz-portable.exe](https://github.com/harmonica80/keyviz-multi-monitor/releases/download/v1.0.26/keyviz-portable.exe)

更新前請先結束舊版，啟動後確認標題列為 v1.0.26。

## English

- Shared renderer keeps the settings preview identical to the current toolbar, including visible order, theme and utility buttons.
- Two persistent customizable radial quick colors, red/blue by default; independent from toolbar swatches and themes.
- Blank radial hub still dismisses the menu. Cross-mark drawing, Esc and right-click behavior remain available.
- Custom black swatches remain opaque.

## Verification

- Promotes the exact tested EXE; no rebuild or program-code changes during promotion.
- Source: `c9665f7ad9711735e4fd7e31ad0ebcc80f1aea32`.
- [Windows build and tests](https://github.com/harmonica80/keyviz-multi-monitor/actions/runs/42452533922): 49 native tests, 47 shortcut checks and theme/color preference regression tests passed.
- Browser UI checks used a mocked Tauri backend; preview appearance parity and color change/sync/reset passed. Native preview images were inspected.
- AMD64 portable EXE; file/product version 1.0.26; 6,430,720 bytes.
- SHA-256: `D28CA9F1F2ABA9EFB3A7BD0F47E01FC8D7610902C2321603772BF0B294F2A060`.
- Windows 10/11, mixed-DPI and recording workflows remain hardware-dependent; this release does not claim to fix all historical overlay issues.

# Keyviz 鍵盤按鍵顯示器與螢幕繪圖 v1.0.19

**[下載 Windows 免安裝版 keyviz-portable.exe](https://github.com/harmonica80/keyviz-multi-monitor/releases/download/v1.0.19/keyviz-portable.exe)**

更新前請先結束舊版，執行新版後確認標題列與關於頁顯示 v1.0.19。本次沿用已完成 Windows 建置及自動測試的 v1.0.19 檔案，未另外修改程式碼。

## 文字功能更新

- 新文字預設大小由 5 增為 8（增加 3 級），既有文字維持原大小。
- 啟用物件編輯與文字類型偏好後，可雙擊文字編輯內容；保留原位置、旋轉與群組，支援復原／重做。
- 可點擊文字中間插入或刪除，滑鼠拖曳選取，使用方向鍵、Home/End、Shift 選取及 Ctrl+A 全選。
- 輸入會替換選取範圍；Backspace 向前刪除、Delete 向後刪除，不再只能從尾端退格。
- 雙擊文字後先全選，可直接替換整段，也可點到中間局部修改。
- 編輯或選取文字物件時可以換色，套用至整個文字物件；不包含單一物件內的局部多色文字。
- 保留雙圈工具選單、Alt 編輯開關、右鍵選單、工具列點擊修正、游標模式復原快捷鍵保護與其他既有功能。

## 驗證

- [Windows 建置紀錄](https://github.com/harmonica80/keyviz-multi-monitor/actions/runs/41603054559)
- 原始碼：`bb0a3ecbc0a6880a3cf7369b639c16394cf759b7`
- 36 項原生測試、47 項快捷鍵相關檢查通過。
- FileVersion / ProductVersion：`1.0.19`；EXE 大小：6,379,008 bytes。
- SHA-256：`F8385EC5F337022A4EE34DD21A57375B6D913EA2657A7DED0138DD0793089AB2`

不同電腦、多螢幕、輸入法與錄影環境的互動仍需實測；本次發佈不代表所有黑幕或置頂問題均已解決。

## English

This portable Windows release adds larger default text, double-click content editing, caret navigation, mouse text selection, selection replacement, and whole-text-object color changes. It uses the same verified v1.0.19 test binary. Exit the old app before running the new EXE. Hardware-specific and input-method interactions still require testing.

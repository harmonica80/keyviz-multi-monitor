# Keyviz 鍵盤按鍵顯示器與螢幕繪圖 v1.0.17

## 下載與更新

**[下載 Windows 免安裝版 keyviz-portable.exe](https://github.com/harmonica80/keyviz-multi-monitor/releases/download/v1.0.17/keyviz-portable.exe)**

請先結束舊版，再執行新版 EXE，確認標題列與關於頁顯示 v1.0.17。這次正式版沿用已完成雲端建置與自動測試的 v1.0.17 檔案，未另外更動程式碼。

## 功能與修正

- 雙圈圖示工具選單：預設 Q 或畫布右鍵開啟，可自訂快捷鍵；下方偏左提供可復原的清除畫面功能。
- 單按並放開 Alt 開啟／關閉滑鼠移入編輯物件；每次開始繪圖與切回游標時關閉，各物件類型偏好仍生效。
- 修正繪圖視窗取得焦點後的 Alt 接收路徑，避免同一次按鍵重複切換。游標與框選工具不顯示 Alt 提示。
- 文字輸入中按右鍵，先完成文字再開啟圓形選單；空白文字不建立物件，Q 仍可輸入文字。
- 工具列區域從繪圖輸入視窗中排除，避免繪圖／文字視窗取得焦點後攔住工具列按鈕。
- 開啟設定時先切回游標工具；游標模式下 Ctrl+Z／Ctrl+Shift+Z 不再操作繪圖歷史。
- 保留多螢幕、鍵盤按鍵顯示、游標醒目、向量繪圖、序號與勾叉印章、自訂工具列、復原／重做及診斷資料匯出。

## 驗證資訊

- [Windows 建置紀錄](https://github.com/harmonica80/keyviz-multi-monitor/actions/runs/41562333642)
- 原始碼提交：`1304574b07e5ffff75a65186f4b46ca98112d664`
- 32 項原生測試及 47 項快捷鍵相關檢查通過。
- FileVersion / ProductVersion：`1.0.17`
- EXE 大小：6,369,792 bytes。
- SHA-256：`5E5F9F62D3F8F3C5030FA54E02D8C00F60FDF83CE6766D0B449438B1874DA73D`

不同教室電腦、多螢幕與錄影軟體的實際互動仍需實測；本次發佈不代表所有黑幕或置頂問題均已解決。

## English

Portable Windows release with Alt-tap object editing, a configurable Q/right-click circular tool menu, undoable clear-all, text commit before the right-click menu, and a toolbar input exclusion region. Pointer-mode undo/redo shortcuts remain with the foreground application. Exit the old version before running the new EXE. The binary matches the verified v1.0.17 test build; hardware-specific classroom and recording scenarios still require testing.

# v1.0.32 更新說明

## 修正：繪圖中 Ctrl+Z 還原／Ctrl+Shift+Z 重做失效

- 根本原因與 v1.0.31 修正的 F7／Delete／ESC 相同：還原快速鍵原本只靠低階鍵盤 Hook 偵測，在部分環境（如系統管理員視窗為前景、Hook 逾時、輸入法干擾）Hook 會漏掉按鍵事件，Ctrl+Z 因此沒有反應。
- 現在備援輪詢（直接讀取 Windows 實際鍵盤狀態 `GetAsyncKeyState`）也會偵測還原／重做快速鍵，與 Hook 路徑共用防重入時間窗，兩條路徑偵測到同一次按鍵只會執行一次。
- 還原／重做的防重入時間窗為 150ms（其他快速鍵為 300ms）：只需要涵蓋 Hook 與輪詢看到同一次按鍵的時間差，同時保留使用者連按 Ctrl+Z 逐步還原的操作。

## 變更檔案

- `src-tauri/src/app/event.rs`：新增 `LAST_UNDO_SHORTCUT_MS`、`claim_shortcut_window`；Hook 的還原分支加入防重入；`start_clear_shortcut_poller` 新增還原／重做偵測（邊緣觸發）。

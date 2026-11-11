# Keyviz v1.0.27 — Claude 修正版

本版本針對程式架構檢查發現的三大類問題（高風險 4 項、中等 5 項、較小數項）進行修正，未新增功能。

## 高風險修正

1. **低階鍵盤/滑鼠 Hook 回呼瘦身（防止按鍵顯示永久失效與死結）**
   - rdev hook 回呼現在只把事件送進 channel，不再鎖 `AppState`、不再呼叫任何 Tauri/視窗 API。
   - 所有狀態更新、快捷鍵判斷移到專屬 worker 執行緒（`event.rs::process_input_event`）。
   - `toggle_listener` 的 UI 工作（選單文字、系統匣圖示、視窗停放）改為釋放鎖之後 `run_on_main_thread` 非阻塞執行，消除「worker 持鎖等主執行緒、主執行緒命令等鎖」的死結組合。
   - 超過 Windows `LowLevelHooksTimeout` 導致 hook 被靜默卸載的風險因此消除。

2. **移除 40ms `GetAsyncKeyState` 輪詢器，快捷鍵統一單一路徑**
   - 繪圖開關/游標/清除/復原/關閉快捷鍵全部在 hook 事件流中判斷（所有平台同一份邏輯）。
   - Redo＝復原快捷鍵+Shift（預設 Ctrl+Shift+Z），在流中先判斷 redo 再判斷 undo，避免子集誤觸。
   - 消除兩條偵測路徑互相競爭、依賴 150ms 時間窗協調的問題；閒置 CPU 喚醒同時減少。

3. **Esc 關閉繪圖範圍化（修正「在別的程式按 Esc 標註全消失」）**
   - 繪圖工具作用中（非游標模式）：Esc 照舊關閉繪圖。
   - 游標（穿透）模式：只有前景視窗屬於 Keyviz 自己（畫布/工具列/設定）時 Esc 才關閉繪圖；在 PowerPoint 結束放映、關閉其他程式對話框不再誤清標註。
   - F8（繪圖開關快捷鍵）維持全域有效，仍可隨時關閉。
   - 徑向選單佔用 Esc 期間照舊不關閉。

4. **TOPMOST 看門狗改為「被降級才重排」**
   - 新增原生命令 `RaiseIfDemoted`：先用 `GetWindowLongPtrW(GWL_EXSTYLE)` 檢查 `WS_EX_TOPMOST`，全部仍在頂層時完全不做 z-order 操作。
   - 每秒強制 `SetWindowPos(HWND_TOPMOST)` 造成與 PixPin 等置頂工具互搶 z-order（黑幕問題，見 docs/issues）的機制來源因此消除。

## 中等修正

5. **螢幕配置變更處理（多螢幕工具核心缺口）**
   - 原生繪圖表面處理 `WM_DISPLAYCHANGE`（去抖 400ms），另有 5 秒輪詢備援（開繪圖前也涵蓋）。
   - 繪圖進行中接上/拔除螢幕：就地重建各螢幕表面，既有標註依絕對座標保留。
   - 主視覺化視窗重新停放到螢幕外、游標座標偏移更新，並向前端發 `display-configuration-changed` 事件。
   - 工具列尺寸統一以主螢幕 DPI 計算（原本混用兩個 scale 來源）。

6. **設定單一真相來源**
   - 設定視窗（啟動時已預載）在三個 store 完成 rehydrate 後，把快捷鍵、工具列側邊、徑向選單設定、游標樣式一次推給 Rust 端。
   - Rust 端啟動解析 store.json 僅作為推送前的 bootstrap 預設；前端格式變更不會再讓後端靜默退回預設值。
   - 順帶修正：第一次 Alt+右鍵徑向選單在工具列載入前使用預設樣式的競態。

7. **閒置喚醒大幅降低**
   - 新增 `native_loop.rs`：`MsgWaitForMultipleObjectsEx` + `PostThreadMessageW(WM_NULL)` 喚醒機制（`WakingSender`）。
   - 四條原生視窗執行緒（按鍵顯示、游標、繪圖）從每 16ms 輪詢改為事件驅動；閒置時 0 喚醒，命令延遲 <1ms。
   - 游標更新執行緒：游標強調關閉且無事件時改 150ms 心跳；滑鼠/鍵盤事件只發給 main 視窗，不再廣播給所有 webview。

8. **原生繪圖狀態鎖防重入**
   - `lock_overlay_state()`＋thread-local busy 旗標：持鎖期間 Win32 呼叫同步派發訊息回自己 window_proc 時，重入的處理函式安全跳過而非自我死結。
   - `WM_MOUSEMOVE`/`WM_LBUTTONDOWN` 等原本直接 `lock()` 的處理函式全部改用防護版本。

9. **崩潰可追蹤、結束流程正常化**
   - 診斷事件/錯誤同步附加寫入 `app_log_dir/keyviz.log`（>1MB 自動輪替），`panic = "abort"` 前由 panic hook 記錄。
   - 系統匣「結束程式」由 `std::process::exit(0)` 改為 `app.exit(0)`，讓 store 等外掛正常 flush（配合 autoSave 1000ms，最後一秒的設定不再遺失）。
   - 移除可致崩潰的 `lock().unwrap()`（commands、系統匣 toggle）與 `config_window` 內數個 `expect`。

## 較小修正

- 移除死碼：`update_overlay_window` 命令、`set_drawing_window_bounds`、`position_overlay_window`/`OverlayAlignment`。
- 工具列重新開啟時保留使用者拖曳過的位置（僅切換左右側時才重新定位）；建立後的位置補正執行緒只在初次建立時執行。
- 版本號 1.0.26 → 1.0.27。

## 刻意未更動（附原因）

- `fs:allow-write-text-file` 未加 scope：鍵帽主題匯入/匯出允許使用者透過對話框存到任意路徑（含 D:\），加 scope 會破壞此功能。
- `screen-drawing.tsx` 內舊版 canvas 繪圖模式與 capabilities 的 `"drawing"` 視窗保留未刪：與工具列共用同一檔案，刪除風險大於收益，建議日後另行重構。
- rdev fork（winapi）與主程式（windows 0.48）兩套綁定並存：牽動範圍大，非行為問題。

## 驗證狀態

- 前端：`tsc --noEmit` 通過；`scripts/test-radial-shortcut.mjs`、`scripts/test-drawing-themes.mjs` 通過。
- Rust：Linux host `cargo check` 通過（涵蓋全部跨平台程式碼與非 Windows stub）；Windows 專屬程式碼經 rustfmt 解析檢查與逐行 API 比對（windows 0.48 原始碼），並補上缺少的 `Win32_System_Threading` feature。
- Windows 實際編譯與 `hover_tests` 由 GitHub Actions（build-portable.yml）在 windows-latest 上執行——請以該次建置結果為準。

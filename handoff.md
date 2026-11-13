# Handoff：Claude 修正記錄（v1.0.26 → v1.0.27 → v1.0.28 → v1.0.29）

給後續接手的 ChatGPT / 其他 AI / 開發者。本文件記錄 Claude 對本專案做了哪些修改、改了哪些檔案、哪些刻意沒改、驗證到什麼程度。
詳細敘述見 `release-notes-v1.0.27.md`；已知議題見 `KNOWN_ISSUES.md`。

## 基準與結果
- 基準：本機 `outputs\keyviz-current-work`（原始資料夾未被動過）。
- 修正版：本機 `outputs\Claude修正`，同內容已合併到 GitHub `main`（分支 `claude-fix-1.0.27`）。
- 注意：合併前 GitHub `main` 停在 v1.0.26，比本機 current-work 少了不少檔案（例如徑向選單、繪圖主題相關檔），所以這次合併相對舊 `main` 的差異很大，並非全是 Claude 的修改。**Claude 實際改動請以下方清單為準。**
- 版本號：1.0.26 → 1.0.27（`package.json`、`src-tauri/Cargo.toml`、`Cargo.lock`、`tauri.conf.json`）。

## 新增的檔案
| 檔案 | 用途 |
|---|---|
| `src-tauri/src/app/native_loop.rs` | 事件驅動訊息迴圈：`WakeHandle`、`WakingSender<T>`（手動實作 `Clone`，不要求 `T: Clone`）、`wait_for_input`。取代原本每 16ms 輪詢。 |
| `release-notes-v1.0.27.md` | 本版修正說明。 |
| `handoff.md` | 本文件。 |
| `src-tauri/gen/schemas/*` | 建置產生的 schema（原資料夾沒有，可視需要從版本控制移除）。 |

## 修改的檔案（相對 keyviz-current-work）
| 檔案 | 修改內容 |
|---|---|
| `src-tauri/src/app/event.rs` | 重寫：rdev hook 回呼只送 channel，worker 執行緒處理；移除 `GetAsyncKeyState` 40ms 輪詢器，所有繪圖快捷鍵（開關／游標／清除／復原／重做／關閉）統一在事件流判斷；Esc 關閉繪圖改為「繪圖工具作用中，或前景視窗是 Keyviz 自己」才生效；輸入事件只 `emit_to("main")`；游標更新執行緒閒置時改 150ms 心跳。 |
| `src-tauri/src/app/state.rs` | `toggle_listener` 先改狀態、隱藏按鍵覆蓋層，其餘 UI（選單文字、系統匣圖示、視窗停放、事件）改用 `run_on_main_thread`，避免鎖與主執行緒互等的死結。 |
| `src-tauri/src/app/native_drawing.rs` | 改用 `WakingSender`；訊息迴圈改為「處理訊息→取空 channel→等待」；新增 `lock_overlay_state()`（thread-local 防重入）；新增 `RaiseIfDemoted`（只在被降級時才重排 TOPMOST）；`Resize` 可在顯示中就地重建各螢幕表面並保留標註；處理 `WM_DISPLAYCHANGE`（400ms 去抖）；補非 Windows stub。 |
| `src-tauri/src/app/drawing_radial.rs` | sender 型別改為 `WakingSender<DrawingCommand>`，測試同步更新。 |
| `src-tauri/src/app/native_cursor.rs` | 改用 `WakingSender`＋喚醒式迴圈。 |
| `src-tauri/src/app/native_keys_software.rs` | 改用 `WakingSender`＋喚醒式迴圈；佇列命令只保留最新的。 |
| `src-tauri/src/app/diagnostics.rs` | 新增 `init_file_logging`（寫 `app_log_dir/keyviz.log`，>1MB 輪替）、panic hook、`record_event`/`record_error` 同步寫檔。 |
| `src-tauri/src/app/commands.rs` | 移除死碼 `update_overlay_window`；`set_toggle_shortcut`/`set_drawing_shortcuts` 改回傳 `Result<(), String>`，去掉 `unwrap`。 |
| `src-tauri/src/app/window.rs` | 移除 `OverlayAlignment`、`position_overlay_window`；`config_window` 以 `record_error` 取代 `expect`。 |
| `src-tauri/src/app/mod.rs` | 註冊 `native_loop` 模組。 |
| `src-tauri/src/lib.rs` | 移除 `set_drawing_window_bounds`；`create_drawing_toolbar` 新增 `reposition` 參數（保留使用者拖曳位置）、DPI 統一取主螢幕；1Hz 看門狗改呼叫 `raise_if_demoted()`；新增 `handle_display_change`（500ms 去抖）與 `start_monitor_watcher`（5 秒輪詢備援）；setup 呼叫 `init_file_logging`；系統匣「結束」改 `app.exit(0)`，toggle 不再 `unwrap`。 |
| `src-tauri/Cargo.toml` / `Cargo.lock` | 加入 `Win32_System_Threading` feature；版本 1.0.27。 |
| `src-tauri/tauri.conf.json`、`package.json` | 版本 1.0.27。 |
| `src/pages/settings.tsx` | 三個 store（`useKeyEvent`、`useDrawingToolbar`、`useKeyStyle`）rehydrate 完成後，`pushSettingsToBackend()` 把快捷鍵、工具列側邊、徑向選單、游標樣式推給 Rust（前端為單一真相來源）。 |
| `KNOWN_ISSUES.md` | 加上 v1.0.27 後續說明。 |
| `.github/workflows/build-portable.yml` | 僅在推送觸發分支清單加入 `claude-fix-1.0.27`（其餘不變）。 |

## v1.0.28 追加修正（相對 v1.0.27）
| 檔案 | 修改內容 |
|---|---|
| `src-tauri/src/app/native_drawing.rs` | `SetColor` 改為對所有選取物件生效（新增 `recolor_drawing`，橡皮擦線條除外，有變更才記錄復原）；懸停編輯模式下游標在工具列上時保留選取；新增 `hover_sticky`：編輯物件模式點到物件後保持選取，直到下一次點擊重新選取（`ToggleHoverEdit` 重置）。 |
| `src-tauri/src/app/drawing_radial.rs` | 右鍵圓形選單「框選物件」圖示改為與工具列相同（lucide `SquareDashedMousePointer` 的線段版）。 圖示（含清除、工具圖示）改以 `with_icon_transform` 繪製（0.1px 區域單位＋GDI+ 世界座標縮放），不再四捨五入到整數像素。 |
| `src-tauri/src/app/event.rs`、`state.rs` | 新增 `prune_stale_keys`、`key_physically_down`（`GetAsyncKeyState`）與 `AppState.pressed_key_meta`：每次新按鍵前，清除超過 300ms 且 OS 已不再按著的按鍵並通知前端放開，避免輸入法切換時漏掉的放開事件使修飾鍵「卡住」、篩選失效。記錄 `Released stale keys: ...`。**根因為依邏輯推論，未能在開發環境重現**，若仍失效請檢查 `keyviz.log`。 |
| `src-tauri/src/app/native_drawing.rs`（Delete） | `DeleteSelectionOrClear`：選取工具或任何工具下已有選取物件時只刪選取；沒選取才清除全部。另新增 `PointF`、`GdipDrawPolygon/GdipFillPolygon/GdipTranslateWorldTransform/GdipScaleWorldTransform` 宣告與 `ICON_TRANSFORM`（thread-local），`with_gdiplus_graphics` 會套用。 |
| `src-tauri/src/app/drawing_radial_render.rs` | 扇形、中心圓、右下快速面板改用浮點多邊形（`sector_points_f`／`panel_points_f`／`point_f`，弧線取樣加倍），分隔線粗細隨 DPI；右下面板圖示（含色塊圓）走 `with_icon_transform`，線寬 1.5px×scale。整數版 `sector_points`／`panel_points` 僅保留給測試。**未實機驗證**，若仍模糊請提供螢幕解析度與縮放比例。 |
| `src/App.css` | 「隱藏工具列」與底部三個按鈕 hover/active 背景改為 `border-radius: 10px`（與關閉鈕一致）。 |
| `.github/workflows/build-portable.yml` | 新增 `v*` 標籤觸發編譯並自動發佈 Release；`workflow_dispatch` 新增 `release_tag` 輸入，可把 exe 附到已存在的 Release。 |
| `package.json`、`src-tauri/Cargo.toml`、`Cargo.lock`、`tauri.conf.json` | 版本 1.0.28。 |
| `release-notes-v1.0.28.md`、`README.md` | 新增／更新說明。 |

## v1.0.29 追加（相對 v1.0.28）
| 檔案 | 修改內容 |
|---|---|
| `src-tauri/src/app/drawing_radial.rs` | 圓形選單閘門 `GetAsyncKeyState(0xa4) || (0xa5)`：左／右 Alt 都可叫出；`extra` 清單移除 0xa5（Ctrl 仍在，AltGr 不誤觸）。 |
| `src-tauri/src/app/export_io.rs`（新） | 與平台無關：`write_numbered`（`YYYYMMDD-NN.ext` 自動遞增、`create_new` 避免覆蓋）、PNG 編解碼（`png` crate）、base64；含單元測試。 |
| `src-tauri/src/app/drawing_export.rs`（新，`include!` 進 `native_drawing.rs` 的 `platform`） | `export_screen`：以 `SetWindowDisplayAffinity(0x11)` 暫時把自家 overlay／工具列排除於擷取，`BitBlt|CAPTUREBLT` 擷取游標所在螢幕；繪圖物件先畫在透明圖層再合成（橡皮擦行為與畫面一致）。JSON 座標為螢幕左上角為 0,0 的相對座標。`import_json`：還原繪圖物件（可復原）並以 `ScreenBackground` 當底圖。 |
| `src-tauri/src/app/native_drawing.rs` | 新增 `DrawingCommand::Export/Import`、`OverlayState.background`；alpha 轉換迴圈在透明處改取底圖；`Clear` 在無繪圖物件時移除底圖，`Hide` 重置。 |
| `src-tauri/src/app/commands.rs`、`lib.rs` | 新增命令 `drawing_export(kind)`、`drawing_import(path)`、`drawing_export_dir`。 |
| `src/components/drawing-toolbar-view.tsx`、`src/pages/screen-drawing.tsx`、`src/App.css`、`src/lib/i18n.ts` | 工具列新增三顆按鈕（存 PNG／存 JSON／開啟 JSON，開啟用 `@tauri-apps/plugin-dialog`）與中文字串；說明文字改為「左或右 Alt」。 |
| `drawing_export.rs`（續） | `blend_layer_pixel`：圖層像素為預乘 alpha，與 `UpdateLayeredWindow` 相同方式混合到螢幕圖（先前強制不透明造成黑邊）。儲存資料夾由設定 `saveDir`（空字串＝Downloads）傳入 `drawing_export(kind, dir)`；`export_io::export_directory` 解析。 |
| `src/stores/drawing_toolbar.ts`、`settings/drawing-toolbar.tsx` | 新增 `saveDir`（persist 版本 8）；Q 快捷鍵移除（`radialShortcut` 一律為空，欄位僅為相容舊設定檔）；快捷顏色區塊縮小。 |
| `Cargo.toml`／`Cargo.lock` | 新增直接依賴 `png 0.17`、`base64 0.22`（原本已是間接依賴），windows 啟用 `Win32_System_SystemInformation`。版本 1.0.29。 |

**未實機驗證**：擷取排除（需 Win10 2004+）、DWM 套用延遲（目前 `Sleep(180)`）、多螢幕座標換算、底圖疊合。若截圖含有 Keyviz 自己的視窗，優先檢查 `capture_screen`。

## v1.0.30 追加（相對 v1.0.29）
| 檔案 | 修改內容 |
|---|---|
| `export_io.rs`、`drawing_export.rs`、`native_drawing.rs`、`commands.rs`、`lib.rs` | `drawing_export` 回傳 `ExportResult{path, closed}`；`OverlayState.opened_json` 記住開啟的 JSON，存 JSON 時覆蓋原檔並清除底圖／繪圖／歷史（`closed=true`）；新增命令 `reveal_in_explorer`（`explorer /select,`）。 |
| `src/pages/screen-drawing.tsx`、`capabilities/default.json`、`lib/i18n.ts` | 以 `@tauri-apps/plugin-dialog` 的 `ask`／`message` 顯示儲存成功／開啟成功對話方塊（新增 `dialog:allow-ask`、`dialog:allow-message`）；`closed` 時前端切回游標工具。 |

## v1.0.31 追加（相對 v1.0.30）
| 檔案 | 修改內容 |
|---|---|
| `src-tauri/src/app/event.rs` | `start_clear_shortcut_poller` 擴充為備援輪詢：回到滑鼠模式（F7）、清除、關閉（Esc）；`claim_shortcut(&AtomicU64)` 各動作獨立防重複（hook 與輪詢誰先到誰執行）。drawing_visible 時才輪詢（滑鼠模式 80ms、繪圖工具 40ms）。F7 失效根因為推測（hook 漏事件，與 Delete 同型），未實機驗證。 |
| `src/pages/screen-drawing.tsx`、`lib/i18n.ts` | 開啟 JSON 成功後切到框選物件工具；說明改為三點條列。 |
| `src/components/settings/general.tsx` | `ShortcutRow` 說明文字改為整列寬度。 |

發佈流程備註：Claude 的雲端環境無法推送標籤或建立 Release。流程為：在 GitHub 網頁建立 Release／標籤 `v1.0.28`（目標 `main`），再到 Actions 手動執行 `Build portable exe`，`release_tag` 填 `v1.0.28`，exe 會自動附到該 Release。

## 刻意沒改（以及原因）
- `fs:allow-write-text-file` 未加 scope：鍵帽主題匯入／匯出透過對話框存任意路徑，加 scope 會壞。
- `screen-drawing.tsx` 的舊 canvas 繪圖模式與 capabilities 的 `"drawing"` 視窗：與工具列共用檔案，刪除風險大於收益，建議日後單獨重構。
- rdev fork（winapi）與主程式（windows 0.48）兩套綁定並存：牽動範圍大，非行為問題。

## 驗證狀態
- 前端：`tsc --noEmit` 通過；`scripts/test-radial-shortcut.mjs`、`scripts/test-drawing-themes.mjs` 通過。
- GitHub Actions（windows-latest）：編譯成功，`cargo test hover_tests` 通過。第一次編譯失敗是 `WakingSender<T>` 用了 `derive(Clone)`（會要求 `T: Clone`），已改成手動實作。
- **尚未做實機測試**：多螢幕顯示、拔插螢幕後覆蓋層重建、各繪圖快捷鍵、設定存檔後重開還原、Esc 在其他程式不誤清標註。實測有問題請附 `keyviz.log`。

## 接手注意
- 不要把 hook 回呼改回在回呼內鎖 `AppState` 或呼叫 Tauri/視窗 API（會觸發 hook timeout 與死結）。
- 往 `native_*` 執行緒送命令請用 `WakingSender::send`，直接用裸 `mpsc::Sender` 會讓執行緒睡到 1 秒安全逾時才處理。
- 在持有 `OVERLAY_STATE` 鎖期間呼叫會同步派發訊息的 Win32 API 是已知重入來源，處理函式請一律用 `lock_overlay_state()`。

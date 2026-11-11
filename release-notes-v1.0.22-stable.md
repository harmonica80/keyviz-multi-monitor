# Keyviz 鍵盤按鍵顯示器與螢幕繪圖 v1.0.22

**[下載 Windows 免安裝版 keyviz-portable.exe](https://github.com/harmonica80/keyviz-multi-monitor/releases/download/v1.0.22/keyviz-portable.exe)**

請先結束舊版，執行新版後確認標題列與關於頁顯示 v1.0.22。此正式版沿用已完成建置與自動測試的 v1.0.22 EXE，未另外修改程式碼。

## 本次更新

- 文字工具預設與最小大小為 15，最大為 45，字型像素高度為 60～180；最大高度為預設的 3 倍。
- 滑鼠滾輪可在輸入前及輸入中調整文字大小，工具列大小控制同步支援 15～45。
- 已放置文字的滾輪縮放也遵循新範圍；舊文字不會自動變大，在使用者調整時才套用新範圍。
- 其他工具大小範圍不變，並保留文字雙擊編輯、游標移動、拖曳選字、換色、復原／重做與既有繪圖功能。

## 驗證資訊

- [Windows 建置紀錄](https://github.com/harmonica80/keyviz-multi-monitor/actions/runs/42195106532)
- 原始碼提交：`20ca3d70a4d5eae1c82e8775777376a1575e3282`
- 37 項原生測試及 47 項快捷鍵相關檢查通過。
- FileVersion / ProductVersion：`1.0.22`；大小：6,379,008 bytes。
- SHA-256：`87FF42AA64BF53372EAFA67D2129E3E1C8BD5D1EEC69ABEBAA277AF58B5A30A1`

不同電腦、多螢幕與輸入法等實際互動仍需驗證；本次發佈不代表所有黑幕或置頂問題均已解決。

## English

Text now defaults to size 15 and supports a 15–45 range, with wheel resizing up to three times the default height. The toolbar size slider uses the same range. Other tools retain their previous limits. The binary is identical to the verified v1.0.22 test build; exit the old app before running the new EXE.

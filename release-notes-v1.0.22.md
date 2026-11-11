# Keyviz v1.0.22

- 文字工具預設與最小大小為 15，最大為 45；字型像素高度範圍 60～180，最大為原預設的 3 倍。
- 滾輪可在輸入前及輸入中調整文字大小，工具列大小控制同步為 15～45。
- 已放置文字的滾輪縮放也遵循 15～45；舊文字不會自動改大小，在使用者調整時才套用新範圍。
- 其他工具範圍不變，當次文字大小仍可記憶。

## 建置驗證

- [Windows 建置 42195106532](https://github.com/harmonica80/keyviz-multi-monitor/actions/runs/42195106532)：成功，37 項原生測試與 47 項快捷鍵相關檢查通過。
- 建置來源：`20ca3d70a4d5eae1c82e8775777376a1575e3282`。
- 交付檔：`keyviz-portable-v1.0.22-text-range-test.exe`；Actions 中為同內容的 `keyviz-portable.exe`。
- FileVersion / ProductVersion：`1.0.22`；大小 6,379,008 bytes。
- SHA-256：`87FF42AA64BF53372EAFA67D2129E3E1C8BD5D1EEC69ABEBAA277AF58B5A30A1`。
- 下載與交付檔雜湊一致；文字滾輪、輸入及工具切換等實際操作仍需使用者確認。正式 Release 保持 v1.0.19。

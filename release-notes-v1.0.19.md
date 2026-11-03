# Keyviz v1.0.19

- 文字編輯加入游標位置與選取範圍：點擊文字中間、滑鼠拖曳選取、方向鍵、Home/End、Shift 選取、Ctrl+A 全選。
- 輸入取代選取內容，Backspace 向前刪除，Delete 向後刪除，不再只能從尾端退格。選取會顯示底色，游標顯示目前插入位置。
- 雙擊既有文字後先全選，方便直接替換整段，也可點擊中間局部修改。
- 顏色按鈕套用至編輯中的整個文字物件，確認後保留；物件選取但尚未輸入時，也可直接換色。並非單一文字物件內的多色文字功能。
- 修改內容與顏色可復原／重做，取消編輯保留原物件。

待 Windows 原生建置及實測：中文與英文的中間插入／刪除、拖曳選取與反向拖曳、旋轉文字、改色後確認／取消、工具列互動及復原／重做。

## 建置驗證

- [Windows 建置 41603054559](https://github.com/harmonica80/keyviz-multi-monitor/actions/runs/41603054559)：成功；36 項原生測試及 47 項快捷鍵相關檢查通過。
- 來源提交：`bb0a3ecbc0a6880a3cf7369b639c16394cf759b7`。
- 交付檔：`keyviz-portable-v1.0.19-text-selection-test.exe`，Actions 內為相同內容的 `keyviz-portable.exe`。
- FileVersion / ProductVersion：`1.0.19`；大小 6,379,008 bytes。
- SHA-256：`F8385EC5F337022A4EE34DD21A57375B6D913EA2657A7DED0138DD0793089AB2`。
- 下載與交付檔雜湊一致。上述實際操作仍需使用者測試；正式 Release 維持 v1.0.17。

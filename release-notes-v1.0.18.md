# Keyviz v1.0.18

- 新文字工具的預設大小由 5 提高至 8；使用者當次調整的大小仍會保留，既有文字不變。
- 啟用 Alt 物件編輯及文字類型偏好後，可雙擊文字進入內容編輯，使用輸入／退格修改，Enter 或點畫布確認。保留原物件的位置、旋轉、顏色與群組，修改支援復原／重做。
- 編輯期間原物件暫不重複顯示，取消時保留原文；清空內容並確認則刪除此文字，仍可復原。
- 物件編輯關閉、文字類型偏好關閉及游標模式，不啟動文字雙擊編輯。

待 Windows 原生建置與實測：新文字大小、雙擊編輯、取消、右鍵確認、工具切換、群組／旋轉文字及復原／重做。

## 建置驗證

- [Windows 建置 41599603305](https://github.com/harmonica80/keyviz-multi-monitor/actions/runs/41599603305)：成功；34 項原生測試與 47 項快捷鍵檢查通過。
- 來源提交：`a25550e7be06635a14c850a5014b0827ee8f1cb2`。
- 交付檔：`keyviz-portable-v1.0.18-text-edit-test.exe`，Actions 內為相同內容的 `keyviz-portable.exe`。
- FileVersion / ProductVersion：`1.0.18`；大小 6,371,840 bytes。
- SHA-256：`55DED97ED1AF9D670D161122D07A3B137DAE3B7677CD36A8FF2C1383A685ABD3`。
- 下載與交付檔雜湊一致。上述實際操作仍需使用者測試；正式 Release 保持 v1.0.17。

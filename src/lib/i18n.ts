import { create } from "zustand";
import { createJSONStorage, persist } from "zustand/middleware";

export type Locale = "en" | "zh-TW";

const zhTW: Record<string, string> = {
  "Keyviz Keyboard Visualizer": "Keyviz 鍵盤按鍵顯示器（支援多螢幕）",
  "Teacher Chiu Learning Website": "述文老師學習網",
  "Keyviz Open Source Website": "Keyviz 開放原始碼網站",
  "Open Keyviz source code": "開啟 Keyviz 原始碼網站",
  "Open Teacher Chiu Learning Website": "開啟述文老師學習網",
  "About": "關於",
  "General": "一般",
  "Appearance": "外觀",
  "Keycap": "按鍵樣式",
  "Mouse": "滑鼠",
  "Language": "語言",
  "English": "英文",
  "Traditional Chinese": "繁體中文",
  "JSON Files": "JSON 檔案",
  "Invalid file format": "檔案格式無效",
  "Imported successfully": "匯入成功",
  "Error importing file": "匯入檔案時發生錯誤",
  "Exported successfully": "匯出成功",
  "Error exporting file": "匯出檔案時發生錯誤",
};

interface LocaleState {
  locale: Locale;
  setLocale: (locale: Locale) => void;
}

export const useLocale = create<LocaleState>()(
  persist(
    (set) => ({
      locale: "en",
      setLocale: (locale) => set({ locale }),
    }),
    {
      name: "keyviz-locale",
      storage: createJSONStorage(() => localStorage),
    },
  ),
);

export const translate = (
  locale: Locale,
  key: string,
  values?: Record<string, string | number>,
) => {
  let translated = locale === "zh-TW" ? zhTW[key] ?? key : key;
  for (const [name, value] of Object.entries(values ?? {})) {
    translated = translated.split(`{${name}}`).join(String(value));
  }
  return translated;
};

export const useTranslation = () => {
  const locale = useLocale((state) => state.locale);
  const t = (key: string, values?: Record<string, string | number>) =>
    translate(locale, key, values);

  return { locale, t };
};

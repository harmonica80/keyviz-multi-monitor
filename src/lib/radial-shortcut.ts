const modifiers = new Set(["Control", "ControlLeft", "ControlRight", "Shift", "ShiftLeft", "ShiftRight", "Alt", "AltLeft", "AltRight"]);
const canonical = (key: string) => key.replace(/(Left|Right)$/, "");

export function radialShortcutError(keys: string[], existing: string[][] = []): string | null {
  if (keys.length === 0) return null;
  const main = keys.filter((key) => !modifiers.has(key));
  const supported = /^(Key[A-Z]|Num[0-9]|F([1-9]|1[0-9]|2[0-4])|Space| |Tab|Minus|Equal|LeftBracket|RightBracket|BackSlash|SemiColon|Quote|Comma|Dot|Slash|BackQuote)$/;
  if (main.length !== 1 || !supported.test(main[0])) return "Use a letter, number, function key or punctuation, optionally with Ctrl, Shift or Alt.";
  if (keys.some((key) => key.startsWith("Alt")) && ["Tab", "F4"].includes(main[0])) return "This shortcut is reserved by Windows.";
  const normalized = keys.map(canonical);
  if (existing.some((shortcut) => shortcut.length > 0 && shortcut.map(canonical).every((key) => normalized.includes(key)))) {
    return "This combination conflicts with another Keyviz shortcut.";
  }
  return null;
}

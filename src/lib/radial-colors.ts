export const DEFAULT_RADIAL_COLORS: [string, string] = ["#ef2b2d", "#2d37d6"];

export const normalizeRadialColors = (value: unknown): [string, string] => {
  const colors = Array.isArray(value) ? value : [];
  return DEFAULT_RADIAL_COLORS.map((fallback, index) =>
    typeof colors[index] === "string" && /^#[\da-f]{6}$/i.test(colors[index])
      ? colors[index].toLowerCase() : fallback,
  ) as [string, string];
};

/** The four colors shown on the drawing toolbar (user-configurable in settings). */
export const DEFAULT_TOOLBAR_COLORS: [string, string, string, string] = ["#ef2b2d", "#16c43b", "#2d37d6", "#111111"];

export const normalizeToolbarColors = (value: unknown): [string, string, string, string] => {
  const colors = Array.isArray(value) ? value : [];
  return DEFAULT_TOOLBAR_COLORS.map((fallback, index) =>
    typeof colors[index] === "string" && /^#[\da-f]{6}$/i.test(colors[index])
      ? colors[index].toLowerCase() : fallback,
  ) as [string, string, string, string];
};

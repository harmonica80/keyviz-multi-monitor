export const DEFAULT_RADIAL_COLORS: [string, string] = ["#ef2b2d", "#2d37d6"];

export const normalizeRadialColors = (value: unknown): [string, string] => {
  const colors = Array.isArray(value) ? value : [];
  return DEFAULT_RADIAL_COLORS.map((fallback, index) =>
    typeof colors[index] === "string" && /^#[\da-f]{6}$/i.test(colors[index])
      ? colors[index].toLowerCase() : fallback,
  ) as [string, string];
};

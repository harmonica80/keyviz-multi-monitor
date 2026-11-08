import type { CSSProperties } from "react";
import themes from "./drawing-themes.json";

export const DRAWING_THEMES = themes;
export type DrawingThemeId = "pearl" | "graphite" | "ocean" | "butter" | "orange" | "sand";

export const normalizeDrawingTheme = (value: unknown): DrawingThemeId =>
  themes.some(({ id }) => id === value) ? value as DrawingThemeId : "pearl";

export const drawingThemeVariables = (value: unknown): CSSProperties => {
  const theme = themes.find(({ id }) => id === normalizeDrawingTheme(value))!;
  return {
    "--tool-fill": theme.fill,
    "--tool-ink": theme.ink,
    "--tool-accent": theme.accent,
    "--tool-selected-ink": theme.selectedInk,
    "--tool-divider": theme.divider,
  } as CSSProperties;
};

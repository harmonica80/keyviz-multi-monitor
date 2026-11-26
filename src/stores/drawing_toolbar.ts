import {
  TOOLBAR_ITEM_IDS,
  type DrawingTool,
  type ToolbarItemId,
} from "@/lib/drawing-tools";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { createJSONStorage, persist } from "zustand/middleware";

import { createSyncedStore } from "./sync";
import { tauriStorage } from "./storage";
import { normalizeRadialRightClick } from "@/lib/radial-shortcut";
import { normalizeDrawingTheme, type DrawingThemeId } from "@/lib/drawing-themes";
import { DEFAULT_RADIAL_COLORS, DEFAULT_TOOLBAR_COLORS, normalizeRadialColors, normalizeToolbarColors } from "@/lib/radial-colors";

export const DRAWING_TOOLBAR_STORE = "drawing_toolbar_store";

export interface DrawingToolbarItem {
  id: ToolbarItemId;
  visible: boolean;
}

export type DrawingToolbarSide = "left" | "right" | "top" | "bottom";

/** Outline drawn around the glyphs of new text; white by default once enabled. */
export interface TextOutline {
  enabled: boolean;
  color: string;
}
export const DEFAULT_TEXT_OUTLINE: TextOutline = { enabled: false, color: "#000000" };
export const normalizeTextOutline = (value: unknown): TextOutline => {
  const raw = (value ?? {}) as Partial<TextOutline>;
  const color = typeof raw.color === "string" && /^#[0-9a-fA-F]{6}$/.test(raw.color)
    ? raw.color.toLowerCase()
    : DEFAULT_TEXT_OUTLINE.color;
  return { enabled: raw.enabled === true, color };
};

/** Look of the number marker: a circle or a rounded rectangle. */
export type NumberStyle = "circle" | "rounded";
export const normalizeNumberStyle = (value: unknown): NumberStyle =>
  value === "rounded" ? "rounded" : "circle";
// Keep the persisted field and native command name compatible with older builds.
export type RadialStyle = DrawingThemeId;
export const normalizeRadialStyle = normalizeDrawingTheme;

export type ZoomPanMode = "edge" | "follow";
const normalizeZoomPanMode = (value: unknown): ZoomPanMode => (value === "follow" ? "follow" : "edge");

/** Size of the single-icon toolbar buttons: full, two thirds, or one half. */
export type ToolbarIconSize = "full" | "twoThirds" | "half";
export const TOOLBAR_ICON_SCALE: Record<ToolbarIconSize, number> = { full: 1, twoThirds: 2 / 3, half: 1 / 2 };
/** New installs (and a reset toolbar) start with 2/3-size single icons. */
export const DEFAULT_ICON_SIZE: ToolbarIconSize = "twoThirds";
export const normalizeIconSize = (value: unknown): ToolbarIconSize =>
  value === "full" || value === "half" ? value : DEFAULT_ICON_SIZE;

interface DrawingToolbarState {
  radialShortcut: string[];
  /** Folder for saved PNG / JSON files; empty means the Downloads folder. */
  saveDir: string;
  radialRightClick: boolean;
  radialStyle: RadialStyle;
  radialColors: [string, string];
  /** Screen magnifier panning: glide near the edges, or follow the pointer. */
  zoomPanMode: ZoomPanMode;
  /** Size of the single-icon buttons (tools, group, clear); grouped small icons never shrink. */
  iconSize: ToolbarIconSize;
  /** The four color swatches on the drawing toolbar. */
  toolbarColors: [string, string, string, string];
  hoverEdit: Partial<Record<DrawingTool, boolean>>;
  items: DrawingToolbarItem[];
  side: DrawingToolbarSide;
  numberStyle: NumberStyle;
  textOutline: TextOutline;
}

interface DrawingToolbarActions {
  setRadialShortcut: (shortcut: string[]) => void;
  setSaveDir: (dir: string) => void;
  setRadialRightClick: (enabled: boolean) => void;
  setRadialStyle: (style: RadialStyle) => void;
  setRadialColor: (index: 0 | 1, color: string) => void;
  setToolbarColor: (index: 0 | 1 | 2 | 3, color: string) => void;
  setZoomPanMode: (mode: ZoomPanMode) => void;
  setIconSize: (size: ToolbarIconSize) => void;
  setHoverEdit: (id: DrawingTool, enabled: boolean) => void;
  setItems: (items: DrawingToolbarItem[]) => void;
  setToolVisible: (id: ToolbarItemId, visible: boolean) => void;
  setSide: (side: DrawingToolbarSide) => void;
  setNumberStyle: (style: NumberStyle) => void;
  setTextOutline: (outline: TextOutline) => void;
  resetToolbar: () => void;
}

export type DrawingToolbarStore = DrawingToolbarState & DrawingToolbarActions;

// Older settings without the magnifier get it appended (its original place, after the tools).
export const DEFAULT_DRAWING_TOOLBAR_ITEMS: DrawingToolbarItem[] = TOOLBAR_ITEM_IDS.map(
  (id) => ({ id, visible: true }),
);
export const DEFAULT_DRAWING_TOOLBAR_SIDE: DrawingToolbarSide = "right";

const validToolIds = new Set<ToolbarItemId>(TOOLBAR_ITEM_IDS);

export const normalizeDrawingToolbarItems = (
  items: DrawingToolbarItem[] | undefined,
): DrawingToolbarItem[] => {
  const normalized: DrawingToolbarItem[] = [];
  const seen = new Set<ToolbarItemId>();

  for (const item of items ?? []) {
    if (!validToolIds.has(item.id) || seen.has(item.id)) continue;
    normalized.push({
      id: item.id,
      visible: item.id === "pointer" ? true : item.visible !== false,
    });
    seen.add(item.id);
  }

  for (const item of DEFAULT_DRAWING_TOOLBAR_ITEMS) {
    if (!seen.has(item.id)) normalized.push({ ...item });
  }

  return normalized;
};

const normalizeDrawingToolbarSide = (side: unknown): DrawingToolbarSide =>
  side === "left" || side === "top" || side === "bottom" ? side : DEFAULT_DRAWING_TOOLBAR_SIDE;

// The Q circular-menu shortcut was removed; the field stays only for old settings files.
const normalizeRadialShortcut = (_value?: unknown): string[] => [];

const normalizeSaveDir = (value: unknown): string =>
  typeof value === "string" ? value.trim() : "";

const createDrawingToolbarStore = createSyncedStore<DrawingToolbarStore>(
  DRAWING_TOOLBAR_STORE,
  (set) => ({
    radialShortcut: [],
    saveDir: "",
    radialRightClick: true,
    radialStyle: "pearl",
    radialColors: [...DEFAULT_RADIAL_COLORS],
    toolbarColors: [...DEFAULT_TOOLBAR_COLORS],
    zoomPanMode: "edge",
    setZoomPanMode: (mode) => set({ zoomPanMode: normalizeZoomPanMode(mode) }),
    iconSize: DEFAULT_ICON_SIZE,
    setIconSize: (size) => set({ iconSize: normalizeIconSize(size) }),
    setToolbarColor: (index, color) => set((state) => {
      const colors = [...state.toolbarColors];
      colors[index] = color;
      return { toolbarColors: normalizeToolbarColors(colors) };
    }),
    setRadialColor: (index, color) => set((state) => {
      const colors = [...state.radialColors];
      colors[index] = color;
      return { radialColors: normalizeRadialColors(colors) };
    }),
    setRadialStyle: (style) => set({ radialStyle: normalizeRadialStyle(style) }),
    setRadialRightClick: (enabled) => set({ radialRightClick: enabled }),
    setRadialShortcut: (shortcut) => set({ radialShortcut: normalizeRadialShortcut(shortcut) }),
    setSaveDir: (dir) => set({ saveDir: normalizeSaveDir(dir) }),
    hoverEdit: {},
    setHoverEdit: (id, enabled) => set((state) => ({ hoverEdit: { ...state.hoverEdit, [id]: enabled } })),
    items: DEFAULT_DRAWING_TOOLBAR_ITEMS.map((item) => ({ ...item })),
    side: DEFAULT_DRAWING_TOOLBAR_SIDE,
    numberStyle: "circle",
    textOutline: { ...DEFAULT_TEXT_OUTLINE },
    setTextOutline: (outline) => set({ textOutline: normalizeTextOutline(outline) }),
    setNumberStyle: (style) => set({ numberStyle: normalizeNumberStyle(style) }),
    setItems: (items) => set({ items: normalizeDrawingToolbarItems(items) }),
    setToolVisible: (id, visible) =>
      set((state) => ({
        items: normalizeDrawingToolbarItems(
          state.items.map((item) =>
            item.id === id ? { ...item, visible: id === "pointer" ? true : visible } : item,
          ),
        ),
      })),
    setSide: (side) => set({ side: normalizeDrawingToolbarSide(side) }),
    resetToolbar: () =>
      set({
        radialShortcut: [],
        radialRightClick: true,
        radialStyle: "pearl",
        radialColors: [...DEFAULT_RADIAL_COLORS],
        toolbarColors: [...DEFAULT_TOOLBAR_COLORS],
        iconSize: DEFAULT_ICON_SIZE,
        hoverEdit: {},
        items: DEFAULT_DRAWING_TOOLBAR_ITEMS.map((item) => ({ ...item })),
        side: DEFAULT_DRAWING_TOOLBAR_SIDE,
      }),
  }),
  (config) =>
    persist(config, {
      name: DRAWING_TOOLBAR_STORE,
      storage: createJSONStorage(() => tauriStorage),
      version: 13,
      migrate: (persistedState) => {
        const state = persistedState as Partial<DrawingToolbarState>;
        return {
          radialShortcut: normalizeRadialShortcut(state.radialShortcut),
          saveDir: normalizeSaveDir(state.saveDir),
          radialRightClick: normalizeRadialRightClick(state.radialRightClick),
          radialStyle: normalizeRadialStyle(state.radialStyle),
          radialColors: normalizeRadialColors(state.radialColors),
          toolbarColors: normalizeToolbarColors(state.toolbarColors),
          zoomPanMode: normalizeZoomPanMode(state.zoomPanMode),
          iconSize: normalizeIconSize(state.iconSize),
          items: normalizeDrawingToolbarItems(state.items),
          side: normalizeDrawingToolbarSide(state.side),
          numberStyle: normalizeNumberStyle(state.numberStyle),
          textOutline: normalizeTextOutline(state.textOutline),
          hoverEdit: state.hoverEdit ?? {},
        };
      },
      merge: (persistedState, currentState) => {
        const state = persistedState as Partial<DrawingToolbarState>;
        return {
          ...currentState,
          radialShortcut: normalizeRadialShortcut(state.radialShortcut),
          saveDir: normalizeSaveDir(state.saveDir),
          radialRightClick: normalizeRadialRightClick(state.radialRightClick),
          radialStyle: normalizeRadialStyle(state.radialStyle),
          radialColors: normalizeRadialColors(state.radialColors),
          toolbarColors: normalizeToolbarColors(state.toolbarColors),
          zoomPanMode: normalizeZoomPanMode(state.zoomPanMode),
          iconSize: normalizeIconSize(state.iconSize),
          hoverEdit: state.hoverEdit ?? {},
          items: normalizeDrawingToolbarItems(state.items),
          side: normalizeDrawingToolbarSide(state.side),
          numberStyle: normalizeNumberStyle(state.numberStyle),
          textOutline: normalizeTextOutline(state.textOutline),
        };
      },
    }),
);

export const useDrawingToolbar = createDrawingToolbarStore(
  getCurrentWindow().label === "settings",
);

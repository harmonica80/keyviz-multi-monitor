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

export type DrawingToolbarSide = "left" | "right";
// Keep the persisted field and native command name compatible with older builds.
export type RadialStyle = DrawingThemeId;
export const normalizeRadialStyle = normalizeDrawingTheme;

export type ZoomPanMode = "edge" | "follow";
const normalizeZoomPanMode = (value: unknown): ZoomPanMode => (value === "follow" ? "follow" : "edge");

interface DrawingToolbarState {
  radialShortcut: string[];
  /** Folder for saved PNG / JSON files; empty means the Downloads folder. */
  saveDir: string;
  radialRightClick: boolean;
  radialStyle: RadialStyle;
  radialColors: [string, string];
  /** Screen magnifier panning: glide near the edges, or follow the pointer. */
  zoomPanMode: ZoomPanMode;
  /** The four color swatches on the drawing toolbar. */
  toolbarColors: [string, string, string, string];
  hoverEdit: Partial<Record<DrawingTool, boolean>>;
  items: DrawingToolbarItem[];
  side: DrawingToolbarSide;
}

interface DrawingToolbarActions {
  setRadialShortcut: (shortcut: string[]) => void;
  setSaveDir: (dir: string) => void;
  setRadialRightClick: (enabled: boolean) => void;
  setRadialStyle: (style: RadialStyle) => void;
  setRadialColor: (index: 0 | 1, color: string) => void;
  setToolbarColor: (index: 0 | 1 | 2 | 3, color: string) => void;
  setZoomPanMode: (mode: ZoomPanMode) => void;
  setHoverEdit: (id: DrawingTool, enabled: boolean) => void;
  setItems: (items: DrawingToolbarItem[]) => void;
  setToolVisible: (id: ToolbarItemId, visible: boolean) => void;
  setSide: (side: DrawingToolbarSide) => void;
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
  side === "left" ? "left" : DEFAULT_DRAWING_TOOLBAR_SIDE;

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
        hoverEdit: {},
        items: DEFAULT_DRAWING_TOOLBAR_ITEMS.map((item) => ({ ...item })),
        side: DEFAULT_DRAWING_TOOLBAR_SIDE,
      }),
  }),
  (config) =>
    persist(config, {
      name: DRAWING_TOOLBAR_STORE,
      storage: createJSONStorage(() => tauriStorage),
      version: 10,
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
          items: normalizeDrawingToolbarItems(state.items),
          side: normalizeDrawingToolbarSide(state.side),
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
          hoverEdit: state.hoverEdit ?? {},
          items: normalizeDrawingToolbarItems(state.items),
          side: normalizeDrawingToolbarSide(state.side),
        };
      },
    }),
);

export const useDrawingToolbar = createDrawingToolbarStore(
  getCurrentWindow().label === "settings",
);

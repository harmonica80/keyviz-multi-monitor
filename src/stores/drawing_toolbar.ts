import {
  DRAWING_TOOLS,
  type DrawingTool,
} from "@/lib/drawing-tools";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { createJSONStorage, persist } from "zustand/middleware";

import { createSyncedStore } from "./sync";
import { tauriStorage } from "./storage";
import { normalizeRadialRightClick, radialShortcutError } from "@/lib/radial-shortcut";
import { normalizeDrawingTheme, type DrawingThemeId } from "@/lib/drawing-themes";

export const DRAWING_TOOLBAR_STORE = "drawing_toolbar_store";

export interface DrawingToolbarItem {
  id: DrawingTool;
  visible: boolean;
}

export type DrawingToolbarSide = "left" | "right";
// Keep the persisted field and native command name compatible with older builds.
export type RadialStyle = DrawingThemeId;
export const normalizeRadialStyle = normalizeDrawingTheme;

interface DrawingToolbarState {
  radialShortcut: string[];
  radialRightClick: boolean;
  radialStyle: RadialStyle;
  hoverEdit: Partial<Record<DrawingTool, boolean>>;
  items: DrawingToolbarItem[];
  side: DrawingToolbarSide;
}

interface DrawingToolbarActions {
  setRadialShortcut: (shortcut: string[]) => void;
  setRadialRightClick: (enabled: boolean) => void;
  setRadialStyle: (style: RadialStyle) => void;
  setHoverEdit: (id: DrawingTool, enabled: boolean) => void;
  setItems: (items: DrawingToolbarItem[]) => void;
  setToolVisible: (id: DrawingTool, visible: boolean) => void;
  setSide: (side: DrawingToolbarSide) => void;
  resetToolbar: () => void;
}

export type DrawingToolbarStore = DrawingToolbarState & DrawingToolbarActions;

export const DEFAULT_DRAWING_TOOLBAR_ITEMS: DrawingToolbarItem[] = DRAWING_TOOLS.map(
  ({ id }) => ({ id, visible: true }),
);
export const DEFAULT_DRAWING_TOOLBAR_SIDE: DrawingToolbarSide = "right";

const validToolIds = new Set<DrawingTool>(DRAWING_TOOLS.map(({ id }) => id));

export const normalizeDrawingToolbarItems = (
  items: DrawingToolbarItem[] | undefined,
): DrawingToolbarItem[] => {
  const normalized: DrawingToolbarItem[] = [];
  const seen = new Set<DrawingTool>();

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

const normalizeRadialShortcut = (value: unknown): string[] =>
  Array.isArray(value) && value.every((key) => typeof key === "string") && !radialShortcutError(value)
    ? value : ["KeyQ"];

const createDrawingToolbarStore = createSyncedStore<DrawingToolbarStore>(
  DRAWING_TOOLBAR_STORE,
  (set) => ({
    radialShortcut: ["KeyQ"],
    radialRightClick: true,
    radialStyle: "pearl",
    setRadialStyle: (style) => set({ radialStyle: normalizeRadialStyle(style) }),
    setRadialRightClick: (enabled) => set({ radialRightClick: enabled }),
    setRadialShortcut: (shortcut) => set({ radialShortcut: normalizeRadialShortcut(shortcut) }),
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
        radialShortcut: ["KeyQ"],
        radialRightClick: true,
        radialStyle: "pearl",
        hoverEdit: {},
        items: DEFAULT_DRAWING_TOOLBAR_ITEMS.map((item) => ({ ...item })),
        side: DEFAULT_DRAWING_TOOLBAR_SIDE,
      }),
  }),
  (config) =>
    persist(config, {
      name: DRAWING_TOOLBAR_STORE,
      storage: createJSONStorage(() => tauriStorage),
      version: 6,
      migrate: (persistedState) => {
        const state = persistedState as Partial<DrawingToolbarState>;
        return {
          radialShortcut: normalizeRadialShortcut(state.radialShortcut),
          radialRightClick: normalizeRadialRightClick(state.radialRightClick),
          radialStyle: normalizeRadialStyle(state.radialStyle),
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
          radialRightClick: normalizeRadialRightClick(state.radialRightClick),
          radialStyle: normalizeRadialStyle(state.radialStyle),
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

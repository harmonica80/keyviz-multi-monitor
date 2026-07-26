import {
  DRAWING_TOOLS,
  type DrawingTool,
} from "@/lib/drawing-tools";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { createJSONStorage, persist } from "zustand/middleware";

import { createSyncedStore } from "./sync";
import { tauriStorage } from "./storage";

export const DRAWING_TOOLBAR_STORE = "drawing_toolbar_store";

export interface DrawingToolbarItem {
  id: DrawingTool;
  visible: boolean;
}

interface DrawingToolbarState {
  items: DrawingToolbarItem[];
}

interface DrawingToolbarActions {
  setItems: (items: DrawingToolbarItem[]) => void;
  setToolVisible: (id: DrawingTool, visible: boolean) => void;
  resetToolbar: () => void;
}

export type DrawingToolbarStore = DrawingToolbarState & DrawingToolbarActions;

export const DEFAULT_DRAWING_TOOLBAR_ITEMS: DrawingToolbarItem[] = DRAWING_TOOLS.map(
  ({ id }) => ({ id, visible: true }),
);

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

const createDrawingToolbarStore = createSyncedStore<DrawingToolbarStore>(
  DRAWING_TOOLBAR_STORE,
  (set) => ({
    items: DEFAULT_DRAWING_TOOLBAR_ITEMS.map((item) => ({ ...item })),
    setItems: (items) => set({ items: normalizeDrawingToolbarItems(items) }),
    setToolVisible: (id, visible) =>
      set((state) => ({
        items: normalizeDrawingToolbarItems(
          state.items.map((item) =>
            item.id === id ? { ...item, visible: id === "pointer" ? true : visible } : item,
          ),
        ),
      })),
    resetToolbar: () =>
      set({ items: DEFAULT_DRAWING_TOOLBAR_ITEMS.map((item) => ({ ...item })) }),
  }),
  (config) =>
    persist(config, {
      name: DRAWING_TOOLBAR_STORE,
      storage: createJSONStorage(() => tauriStorage),
      version: 1,
      migrate: (persistedState) => {
        const state = persistedState as Partial<DrawingToolbarState>;
        return { items: normalizeDrawingToolbarItems(state.items) };
      },
      merge: (persistedState, currentState) => {
        const state = persistedState as Partial<DrawingToolbarState>;
        return {
          ...currentState,
          items: normalizeDrawingToolbarItems(state.items),
        };
      },
    }),
);

export const useDrawingToolbar = createDrawingToolbarStore(
  getCurrentWindow().label === "settings",
);

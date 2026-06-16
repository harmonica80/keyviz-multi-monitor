import { Alignment } from "@/types/style";
import { createJSONStorage, persist } from "zustand/middleware";
import { tauriStorage } from "./storage";
import { createSyncedStore } from "./sync";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { readTextFile, writeTextFile } from "@tauri-apps/plugin-fs";
import { open, save } from "@tauri-apps/plugin-dialog";
import { toast } from "sonner";
import { translate, useLocale } from "@/lib/i18n";

export const KEY_STYLE_STORE = "key_style_store";
export type KeycapStyle = "clean" | "outline" | "raised" | "dark" | "retro" | "mint" | "rose";

export interface AppearanceSettings {
    monitor: string | null;
    flexDirection: "row" | "column";
    alignment: Alignment;
    marginX: number;
    marginY: number;
    animation: "none" | "fade" | "zoom" | "float" | "slide";
    animationDuration: number;
    style: KeycapStyle;
}

export interface LayoutSettings {
    showIcon: boolean;
    showSymbol: boolean;
    showPressCount: boolean;
    iconAlignment: "flex-start" | "center" | "flex-end";
}

export interface ColorSettings {
    color: string;
    secondaryColor: string;
    useGradient: boolean;
}

export interface ModifierSettings {
    highlight: boolean;
    color: string;
    secondaryColor: string;
    textColor: string;
    borderColor: string;
}

export interface TextSettings {
    size: number;
    color: string;
    caps: "uppercase" | "capitalize" | "lowercase";
    variant: "icon" | "text" | "text-short";
    alignment: Alignment;
}

export interface BorderSettings {
    enabled: boolean;
    color: string;
    width: number;
    radius: number;
}

export interface BackgroundSettings {
    enabled: boolean;
    color: string;
}

export interface MouseSettings {
    size: number;
    color: string;
    opacity: number;
    thickness: number;
    keepHighlight: boolean;
    showIndicator: boolean;
    keepIndicator: boolean;
    indicatorSize: number;
    indicatorOffsetX: number;
    indicatorOffsetY: number;
}

export interface KeyStyleState {
    appearance: AppearanceSettings;
    layout: LayoutSettings;
    color: ColorSettings;
    modifier: ModifierSettings;
    text: TextSettings;
    border: BorderSettings;
    background: BackgroundSettings;
    mouse: MouseSettings;
}

interface KeyStyleActions {
    setAppearance: (appearance: Partial<AppearanceSettings>) => void;
    setLayout: (layout: Partial<LayoutSettings>) => void;
    setColor: (color: Partial<ColorSettings>) => void;
    setModifier: (modifier: Partial<ModifierSettings>) => void;
    setText: (text: Partial<TextSettings>) => void;
    setBorder: (border: Partial<BorderSettings>) => void;
    setBackground: (background: Partial<BackgroundSettings>) => void;
    setMouse: (mouse: Partial<MouseSettings>) => void;
    import: () => Promise<void>;
    export: () => Promise<void>;
}

export type KeyStyleStore = KeyStyleState & KeyStyleActions;

const createKeyStyleStore = createSyncedStore<KeyStyleStore>(
    KEY_STYLE_STORE,
    (set, get) => ({
        appearance: {
            monitor: null,
            flexDirection: "column",
            alignment: "bottom-center",
            marginX: 100,
            marginY: 100,
            animation: "none",
            animationDuration: 0.05,
            style: "clean",
        },
        layout: {
            showIcon: true,
            showSymbol: true,
            showPressCount: true,
            iconAlignment: "flex-end",
        },
        color: {
            color: "#ffffff",
            secondaryColor: "#1a1a1a",
            useGradient: true,
        },
        modifier: {
            highlight: false,
            color: "#3a86ff",
            secondaryColor: "#000000",
            textColor: "#000000",
            borderColor: "#000000",
        },
        text: {
            size: 32,
            color: "#000000",
            caps: "capitalize",
            variant: "text-short",
            alignment: "center",
        },
        border: {
            enabled: true,
            width: 2,
            color: "#1a1a1a",
            radius: 0.5,
        },
        background: {
            enabled: false,
            color: "#ffffff99",
        },
        mouse: {
            size: 80,
            color: "#ff0000",
            opacity: 50,
            thickness: 6,
            keepHighlight: true,
            showIndicator: true,
            keepIndicator: true,
            indicatorSize: 50,
            indicatorOffsetX: 50,
            indicatorOffsetY: 50,
        },

        setAppearance: (appearance) => set((state) => ({ appearance: { ...state.appearance, ...appearance } })),
        setLayout: (layout) => set((state) => ({ layout: { ...state.layout, ...layout } })),
        setColor: (color) => set((state) => ({ color: { ...state.color, ...color } })),
        setModifier: (modifier) => set((state) => ({ modifier: { ...state.modifier, ...modifier } })),
        setText: (text) => set((state) => ({ text: { ...state.text, ...text } })),
        setBorder: (border) => set((state) => ({ border: { ...state.border, ...border } })),
        setBackground: (background) => set((state) => ({ background: { ...state.background, ...background } })),
        setMouse: (mouse) => set((state) => ({ mouse: { ...state.mouse, ...mouse } })),

        import: async () => {
            const t = (key: string) => translate(useLocale.getState().locale, key);
            try {
                const filePath = await open({
                    multiple: false,
                    filters: [{
                        name: t('JSON Files'),
                        extensions: ['json']
                    }]
                });
                if (!filePath || typeof filePath !== 'string') return;

                const content = await readTextFile(filePath);
                const parsedData: KeyStyleState = JSON.parse(content);

                if (
                    !parsedData.appearance || !parsedData.layout || !parsedData.color ||
                    !parsedData.modifier || !parsedData.text || !parsedData.border ||
                    !parsedData.background || !parsedData.mouse
                ) {
                    toast.warning(t("Invalid file format"), { description: filePath });
                    return;
                }
                const importedMouse = {
                    ...parsedData.mouse,
                    opacity: parsedData.mouse.opacity ?? 100,
                    thickness: parsedData.mouse.thickness ?? 10,
                } as MouseSettings & { showClicks?: boolean };
                delete importedMouse.showClicks;
                set(() => ({
                    appearance: parsedData.appearance,
                    layout: parsedData.layout,
                    color: parsedData.color,
                    modifier: parsedData.modifier,
                    text: parsedData.text,
                    border: parsedData.border,
                    background: parsedData.background,
                    mouse: importedMouse,
                }));
                toast.success(t("Imported successfully"), { description: filePath });
            } catch (err) {
                toast.error(t("Error importing file"), {
                    description: err instanceof Error ? err.message : String(err),
                })
            }
        },
        export: async () => {
            const t = (key: string) => translate(useLocale.getState().locale, key);
            const state = get();
            const exportData: KeyStyleState = {
                appearance: state.appearance,
                layout: state.layout,
                color: state.color,
                modifier: state.modifier,
                text: state.text,
                border: state.border,
                background: state.background,
                mouse: state.mouse,
            };
            try {
                const filePath = await save({
                    defaultPath: "key_style.json",
                    filters: [{ name: t("JSON Files"), extensions: ["json"] }],
                });
                if (!filePath) return;
                await writeTextFile(filePath, JSON.stringify(exportData, null, 2));
                toast.success(t("Exported successfully"), { description: filePath });
            } catch (err) {
                toast.error(t("Error exporting file"), {
                    description: err instanceof Error ? err.message : String(err),
                })
            }
        }
    }),
    (config) => persist(config, {
        name: KEY_STYLE_STORE,
        storage: createJSONStorage(() => tauriStorage),
        version: 4,
        migrate: (persistedState, version) => {
            const state = persistedState as KeyStyleState;
            const mouse = { ...state.mouse } as MouseSettings & { showClicks?: boolean };
            delete mouse.showClicks;
            const appearance = { ...state.appearance };
            const useDefaultAppearanceUpgrade =
                appearance.animation === "fade" && appearance.animationDuration === 0.25;
            const useDefaultMouseUpgrade =
                mouse.size === 150 &&
                mouse.color === "#009dff" &&
                mouse.opacity === 100 &&
                mouse.thickness === 10 &&
                mouse.keepHighlight === false;

            return {
                ...state,
                appearance: {
                    ...appearance,
                    style: version < 2 ? "clean" : state.appearance.style,
                    animation:
                        version < 4 && useDefaultAppearanceUpgrade ? "none" : appearance.animation,
                    animationDuration:
                        version < 4 && useDefaultAppearanceUpgrade ? 0.05 : appearance.animationDuration,
                },
                background: {
                    ...state.background,
                    enabled: version < 1 ? false : state.background.enabled,
                },
                mouse: {
                    ...mouse,
                    size: version < 4 && useDefaultMouseUpgrade ? 80 : mouse.size,
                    color: version < 4 && useDefaultMouseUpgrade ? "#ff0000" : mouse.color,
                    opacity:
                        version < 4 && useDefaultMouseUpgrade
                            ? 50
                            : version < 3
                              ? 100
                              : mouse.opacity,
                    thickness:
                        version < 4 && useDefaultMouseUpgrade
                            ? 6
                            : version < 3
                              ? 10
                              : mouse.thickness,
                    keepHighlight:
                        version < 4 && useDefaultMouseUpgrade ? true : mouse.keepHighlight,
                },
            };
        },
    }),
);

export const useKeyStyle = createKeyStyleStore(getCurrentWindow().label === "settings");

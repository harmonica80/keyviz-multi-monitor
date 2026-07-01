import { keymaps } from "@/lib/keymaps";
import { useKeyStyle, type KeycapStyle } from "@/stores/key_style";
import { motion } from "motion/react";
import type { CSSProperties } from "react";
import type { KeycapProps } from ".";

export interface ModernKeycapTheme {
    name: string;
    key: CSSProperties;
    pressed: {
        y: number;
        boxShadow: string;
        background?: string;
    };
}

export const modernKeycapThemes: Record<KeycapStyle, ModernKeycapTheme> = {
    clean: {
        name: "Clean",
        key: {
            color: "#202124",
            background: "#ffffff",
            border: "1px solid #e2e7ee",
            boxShadow: "0 8px 22px rgba(15, 23, 42, 0.06)",
        },
        pressed: { y: 2, boxShadow: "0 3px 9px rgba(15, 23, 42, 0.08)" },
    },
    outline: {
        name: "Outline",
        key: {
            color: "#24272b",
            background: "rgba(255, 255, 255, 0.82)",
            border: "2px solid #30343a",
            boxShadow: "none",
        },
        pressed: { y: 2, boxShadow: "none", background: "#f3f4f6" },
    },
    raised: {
        name: "Raised",
        key: {
            color: "#24272b",
            background: "linear-gradient(180deg, #ffffff 0%, #f7f9fb 100%)",
            border: "1px solid #dde3ea",
            boxShadow: "0 7px 0 #cfd5dc, 0 13px 24px rgba(15, 23, 42, 0.13)",
        },
        pressed: { y: 5, boxShadow: "0 2px 0 #cfd5dc, 0 5px 10px rgba(15, 23, 42, 0.1)" },
    },
    dark: {
        name: "Dark",
        key: {
            color: "#ffffff",
            background: "linear-gradient(180deg, #303948 0%, #202838 100%)",
            border: "1px solid #3b4554",
            boxShadow: "0 6px 0 #0e1420, 0 12px 22px rgba(0, 0, 0, 0.3)",
        },
        pressed: { y: 4, boxShadow: "0 2px 0 #0e1420, 0 5px 10px rgba(0, 0, 0, 0.25)" },
    },
    retro: {
        name: "Retro",
        key: {
            color: "#3b3423",
            background: "linear-gradient(180deg, #fff7dc 0%, #f6e8b8 100%)",
            border: "1px solid #e4cd8f",
            boxShadow: "0 6px 0 #bd984b, 0 12px 22px rgba(130, 93, 25, 0.18)",
        },
        pressed: { y: 4, boxShadow: "0 2px 0 #bd984b, 0 5px 10px rgba(130, 93, 25, 0.14)" },
    },
    mint: {
        name: "Mint",
        key: {
            color: "#103f37",
            background: "linear-gradient(180deg, #ffffff 0%, #edfff9 100%)",
            border: "1px solid #8ce7d2",
            boxShadow: "0 6px 0 #4bc8a8, 0 12px 22px rgba(29, 170, 135, 0.16)",
        },
        pressed: { y: 4, boxShadow: "0 2px 0 #4bc8a8, 0 5px 10px rgba(29, 170, 135, 0.12)" },
    },
    rose: {
        name: "Rose",
        key: {
            color: "#53222f",
            background: "linear-gradient(180deg, #ffffff 0%, #fff2f5 100%)",
            border: "1px solid #ffc1cf",
            boxShadow: "0 6px 0 #ef86a1, 0 12px 22px rgba(218, 77, 114, 0.15)",
        },
        pressed: { y: 4, boxShadow: "0 2px 0 #ef86a1, 0 5px 10px rgba(218, 77, 114, 0.12)" },
    },
};

export const ModernKeyPreview = ({
    style,
    label,
    wide = false,
}: {
    style: KeycapStyle;
    label: string;
    wide?: boolean;
}) => {
    const theme = modernKeycapThemes[style];
    return (
        <span
            style={{
                ...theme.key,
                minWidth: wide ? 72 : 56,
                height: 50,
                paddingInline: 16,
                borderRadius: 9,
                display: "inline-flex",
                alignItems: "center",
                justifyContent: "center",
                fontSize: 18,
                fontWeight: 500,
            }}
        >
            {label}
        </span>
    );
};

export const ModernKeycap = ({ event, isPressed }: KeycapProps) => {
    const style = useKeyStyle(state => state.appearance.style);
    const text = useKeyStyle(state => state.text);
    const display = keymaps[event.name];
    const theme = modernKeycapThemes[style];
    const Icon = display.icon;
    const forceIcon = display.category === "mouse" && Icon;
    const label = text.variant === "text" ? display.label : display.shortLabel ?? display.label;
    const horizontalPadding = event.isModifier() ? text.size * 0.85 : text.size * 0.65;

    return (
        <motion.div
            animate={isPressed
                ? {
                    y: theme.pressed.y,
                    boxShadow: theme.pressed.boxShadow,
                    background: theme.pressed.background ?? theme.key.background,
                }
                : {
                    y: 0,
                    boxShadow: theme.key.boxShadow,
                    background: theme.key.background,
                }}
            transition={{ duration: 0.08 }}
            style={{
                ...theme.key,
                minWidth: text.size * (event.isModifier() ? 2.8 : forceIcon ? 1.9 : 2),
                height: text.size * 1.9,
                paddingInline: horizontalPadding,
                borderRadius: Math.max(8, text.size * 0.28),
                display: "inline-flex",
                alignItems: "center",
                justifyContent: "center",
                boxSizing: "border-box",
                color: theme.key.color,
                fontSize: text.size * 0.72,
                fontWeight: 500,
                lineHeight: 1,
                textTransform: text.caps,
                whiteSpace: "nowrap",
            }}
        >
            {forceIcon ? <Icon size={text.size * 0.9} strokeWidth={2.2} /> : label}
        </motion.div>
    );
};

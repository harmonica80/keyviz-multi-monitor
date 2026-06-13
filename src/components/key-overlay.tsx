import { easeInQuint, easeOutQuint } from "@/lib/utils";
import { useKeyEvent } from "@/stores/key_event";
import { useKeyStyle } from "@/stores/key_style";
import { AnimatePresence, motion, Variants } from "motion/react";
import { useEffect, useMemo, useRef } from "react";
import { Keycap } from "./keycaps";
import { invoke } from "@tauri-apps/api/core";
import { modernKeycapThemes } from "./keycaps/modern";


const fadeVariants: Variants = {
    visible: { opacity: 1 },
    hidden: { opacity: 0 },
}

export const KeyOverlay = () => {
    const containerRef = useRef<HTMLDivElement>(null);
    const pressedKeys = useKeyEvent(state => state.pressedKeys);
    const groups = useKeyEvent(state => state.groups);
    const showHistory = useKeyEvent(state => state.showEventHistory);

    const appearance = useKeyStyle(state => state.appearance);
    const text = useKeyStyle(state => state.text);
    const border = useKeyStyle(state => state.border);
    const background = useKeyStyle(state => state.background);
    const safePadding = Math.max(10, Math.ceil(text.size * 0.35));
    const separatorColor = modernKeycapThemes[appearance.style].key.color;

    const containerStyle = {
        flexDirection: appearance.flexDirection,
        gap: text.size * 0.5,
        padding: safePadding,
        boxSizing: "content-box" as const,
    };

    const groupStyle = {
        display: "flex",
        alignItems: "center",
        columnGap: text.size * 0.35,
        ...(background.enabled && {
            paddingInline: text.size * 0.4,
            paddingBlock: text.size * 0.4,
            background: background.color,
            borderRadius: border.radius * (text.size * 1.75),
        }),
    }

    const variants = useMemo<Variants>(() => {
        switch (appearance.animation) {
            case "none":
                return {
                    visible: {},
                    hidden: {}
                };
            case "fade":
                return fadeVariants;
            case "zoom":
                return {
                    visible: { scale: 1, opacity: 1 },
                    hidden: { scale: 0, opacity: 0 }
                };
            case "float":
                return {
                    visible: { opacity: 1, y: 0 },
                    hidden: { opacity: 0, y: text.size }
                };
            case "slide":
                return {
                    visible: { opacity: 1, x: 0 },
                    hidden: { opacity: 0, x: text.size }
                };
        }
    }, [appearance.animation, text.size]);

    useEffect(() => {
        const container = containerRef.current;
        if (!container) return;

        let animationFrame = 0;
        const updateWindow = () => {
            cancelAnimationFrame(animationFrame);
            animationFrame = requestAnimationFrame(() => {
                const rect = container.getBoundingClientRect();
                invoke("update_overlay_window", {
                    width: Math.ceil(rect.width),
                    height: Math.ceil(rect.height),
                    alignment: appearance.alignment,
                    marginX: appearance.marginX,
                    marginY: appearance.marginY,
                }).catch((error) => {
                    console.error("Failed to update overlay window:", error);
                });
            });
        };

        const observer = new ResizeObserver(updateWindow);
        observer.observe(container);
        window.addEventListener("keyviz-monitor-changed", updateWindow);
        updateWindow();

        return () => {
            cancelAnimationFrame(animationFrame);
            observer.disconnect();
            window.removeEventListener("keyviz-monitor-changed", updateWindow);
        };
    }, [
        appearance.alignment,
        appearance.flexDirection,
        appearance.marginX,
        appearance.marginY,
        appearance.monitor,
        appearance.style,
        groups,
        text.size,
    ]);

    if (appearance.animation === "none") {
        return (
            <div
                ref={containerRef}
                className="inline-flex"
                style={containerStyle}
            >
                {groups.map((group, groupIndex) => (
                    <div
                        key={group.createdAt}
                        style={groupStyle}
                        className=""
                    >
                        {group.keys.map((event, keyIndex) => (
                            <div key={event.name} className="inline-flex items-center" style={{ gap: text.size * 0.35 }}>
                                {keyIndex > 0 && <span style={{ fontSize: text.size * 0.7, color: separatorColor }}>+</span>}
                                <Keycap
                                    event={event}
                                    lastest={group.keys.length - 1 === keyIndex}
                                    isPressed={groups.length - 1 === groupIndex && event.in(pressedKeys)}
                                />
                            </div>
                        ))}
                    </div>
                ))}
            </div>
        );
    }

    return (
        <div
            ref={containerRef}
            className="inline-flex"
            style={containerStyle}
        >
            <AnimatePresence>
                {groups.map((group, groupIndex) => (
                    <motion.div
                        key={group.createdAt}
                        layout={showHistory ? "position" : false}
                        variants={fadeVariants}
                        initial="hidden"
                        animate="visible"
                        exit="hidden"
                        style={groupStyle}
                        className=""
                        transition={{
                            ease: [easeOutQuint, easeInQuint],
                            duration: showHistory ? appearance.animationDuration : 0
                        }}
                    >
                        <AnimatePresence>
                            {group.keys.map((event, keyIndex) => (
                                <motion.div
                                    key={event.name}
                                    layout="position"
                                    variants={variants}
                                    initial="hidden"
                                    animate="visible"
                                    exit="hidden"
                                    transition={{
                                        ease: [easeOutQuint, easeInQuint],
                                        duration: appearance.animationDuration,
                                        layout: { duration: appearance.animationDuration / 3, ease: easeOutQuint },
                                    }}
                                >
                                    <div className="inline-flex items-center" style={{ gap: text.size * 0.35 }}>
                                        {keyIndex > 0 && <span style={{ fontSize: text.size * 0.7, color: separatorColor }}>+</span>}
                                        <Keycap
                                            event={event}
                                            lastest={group.keys.length - 1 === keyIndex}
                                            isPressed={groups.length - 1 === groupIndex && event.in(pressedKeys)}
                                        />
                                    </div>
                                </motion.div>
                            ))}
                        </AnimatePresence>
                    </motion.div>
                ))}
            </AnimatePresence>
        </div>
    );
};

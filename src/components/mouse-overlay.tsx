import { easeInOutExpo } from "@/lib/utils";
import { useKeyEvent } from "@/stores/key_event";
import { useKeyStyle } from "@/stores/key_style";
import { motion } from "motion/react";
import { useEffect, useRef, useState } from "react";

const MIN_CLICK_DISPLAY_MS = 220;

export const MouseOverlay = () => {
    const pressedMouseButton = useKeyEvent(state => state.pressedMouseButton);
    const style = useKeyStyle(state => state.mouse);
    const animationDuration = useKeyStyle(state => state.appearance.animationDuration);
    const [showClick, setShowClick] = useState(false);
    const timeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);
    const pressTimestampRef = useRef<number | null>(null);

    useEffect(() => {
        if (pressedMouseButton) {
            setShowClick(true);
            pressTimestampRef.current = Date.now();
            if (timeoutRef.current) clearTimeout(timeoutRef.current);
        } else if (showClick && pressTimestampRef.current) {
            const remaining = MIN_CLICK_DISPLAY_MS - (Date.now() - pressTimestampRef.current);
            if (remaining <= 0) {
                setShowClick(false);
            } else {
                timeoutRef.current = setTimeout(() => setShowClick(false), remaining);
            }
        }

        return () => {
            if (timeoutRef.current) clearTimeout(timeoutRef.current);
        };
    }, [pressedMouseButton, showClick]);

    return (
        <div className="fixed inset-0 flex items-center justify-center overflow-hidden">
            <motion.div
                initial={false}
                animate={{
                    opacity: 1,
                    scale: showClick ? 0.72 : 1,
                }}
                transition={{ duration: animationDuration, ease: easeInOutExpo }}
                style={{
                    position: "absolute",
                    inset: 12,
                    boxSizing: "border-box",
                    border: `10px solid ${style.color}`,
                    borderRadius: "50%",
                }}
            />
        </div>
    );
};

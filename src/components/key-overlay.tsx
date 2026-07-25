import { keymaps } from "@/lib/keymaps";
import { useKeyEvent } from "@/stores/key_event";
import { useKeyStyle } from "@/stores/key_style";
import { invoke } from "@tauri-apps/api/core";
import { useEffect, useMemo } from "react";

let overlayUpdateSequence = 0;

const transformLabel = (label: string, caps: "uppercase" | "capitalize" | "lowercase") => {
  if (caps === "uppercase") return label.toUpperCase();
  if (caps === "lowercase") return label.toLowerCase();
  return label.replace(/\b\w/g, (character) => character.toUpperCase());
};

export const KeyOverlay = () => {
  const pressedKeys = useKeyEvent((state) => state.pressedKeys);
  const groups = useKeyEvent((state) => state.groups);
  const appearance = useKeyStyle((state) => state.appearance);
  const text = useKeyStyle((state) => state.text);
  const background = useKeyStyle((state) => state.background);

  const nativeGroups = useMemo(
    () =>
      groups.map((group) => ({
        keys: group.keys.map((event) => {
          const display = keymaps[event.name];
          const mouseKind = display?.category === "mouse" ? event.name : null;
          const rawLabel =
            text.variant === "text"
              ? display?.label ?? event.name
              : display?.shortLabel ?? display?.label ?? event.name;
          return {
            label: mouseKind ? "" : transformLabel(rawLabel, text.caps),
            modifier: event.isModifier(),
            mouseKind,
            pressed: event.in(pressedKeys),
          };
        }),
      })),
    [groups, pressedKeys, text.caps, text.variant],
  );

  useEffect(() => {
    const updateSequence = ++overlayUpdateSequence;
    void invoke("update_native_key_overlay", {
      visual: {
        updateSequence,
        visible: nativeGroups.length > 0,
        groups: nativeGroups,
        flexDirection: appearance.flexDirection,
        alignment: appearance.alignment,
        marginX: appearance.marginX,
        marginY: appearance.marginY,
        style: appearance.style,
        textSize: text.size,
        backgroundEnabled: background.enabled,
        backgroundColor: background.color,
      },
    }).catch((error) => console.error("Failed to update native key overlay:", error));
  }, [
    appearance.alignment,
    appearance.flexDirection,
    appearance.marginX,
    appearance.marginY,
    appearance.monitor,
    appearance.style,
    background.color,
    background.enabled,
    nativeGroups,
    text.size,
  ]);

  useEffect(
    () => () => {
      void invoke("hide_native_key_overlay");
    },
    [],
  );

  return null;
};

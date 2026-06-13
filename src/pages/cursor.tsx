import { MouseOverlay } from "@/components/mouse-overlay";
import { KEY_EVENT_STORE, type KeyEventStore, useKeyEvent } from "@/stores/key_event";
import { KEY_STYLE_STORE, type KeyStyleStore, useKeyStyle } from "@/stores/key_style";
import { listenForUpdates } from "@/stores/sync";
import type { EventPayload } from "@/types/event";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useEffect, useLayoutEffect } from "react";

export function CursorOverlay() {
    const onEvent = useKeyEvent(state => state.onEvent);

    type CursorSettingsPayload = {
        showClicks: boolean;
        keepHighlight: boolean;
        size: number;
        color: string;
    };

    const applyCursorSettings = (settings: CursorSettingsPayload) => {
        useKeyStyle.setState(state => ({
            mouse: {
                ...state.mouse,
                showClicks: settings.showClicks,
                keepHighlight: settings.keepHighlight,
                size: settings.size,
                color: settings.color,
            },
        }));
    };

    useLayoutEffect(() => {
        const elements = [document.documentElement, document.body, document.getElementById("root")];
        elements.forEach(element => {
            if (!element) return;
            element.classList.add("visualization-page", "cursor-page");
            element.style.setProperty("background", "transparent", "important");
            element.style.setProperty("background-color", "transparent", "important");
        });

        return () => {
            elements.forEach(element => {
                if (!element) return;
                element.classList.remove("visualization-page", "cursor-page");
                element.style.removeProperty("background");
                element.style.removeProperty("background-color");
            });
        };
    }, []);

    useEffect(() => {
        const unlistenPromises = [
            listen<EventPayload>("input-event", event => onEvent(event.payload)),
            listen<CursorSettingsPayload>("cursor-settings", event => applyCursorSettings(event.payload)),
            listenForUpdates<KeyEventStore>(KEY_EVENT_STORE, useKeyEvent.setState),
            listenForUpdates<KeyStyleStore>(KEY_STYLE_STORE, useKeyStyle.setState),
        ];
        invoke<CursorSettingsPayload>("get_cursor_settings")
            .then(applyCursorSettings)
            .catch(error => console.error("Failed to load cursor settings:", error));

        return () => {
            unlistenPromises.forEach(promise => promise.then(unlisten => unlisten()));
        };
    }, [onEvent]);

    return <MouseOverlay />;
}

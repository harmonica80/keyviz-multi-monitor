import { ColorInput } from "@/components/ui/color-picker";
import { Item, ItemActions, ItemContent, ItemDescription, ItemGrid, ItemTitle } from "@/components/ui/item";
import { NumberInput } from "@/components/ui/number-input";
import { Switch } from "@/components/ui/switch";
import { useTranslation } from "@/lib/i18n";
import { useKeyEvent } from "@/stores/key_event";
import { useKeyStyle } from "@/stores/key_style";
import { Cursor02Icon, Drag03Icon, MouseLeftClick05Icon } from "@hugeicons/core-free-icons";
import { HugeiconsIcon } from "@hugeicons/react";
import { invoke } from "@tauri-apps/api/core";
import { useEffect } from "react";

export const MouseSettings = () => {
    const { t } = useTranslation();
    const dragThreshold = useKeyEvent(state => state.dragThreshold);
    const setDragThreshold = useKeyEvent(state => state.setDragThreshold);
    const mouse = useKeyStyle(state => state.mouse);
    const setMouse = useKeyStyle(state => state.setMouse);

    const updateMouse = (patch: Partial<typeof mouse>) => {
        const nextMouse = { ...mouse, ...patch };
        setMouse(patch);
        invoke("set_cursor_settings", {
            showClicks: nextMouse.showClicks,
            keepHighlight: nextMouse.keepHighlight,
            size: nextMouse.size,
            color: nextMouse.color,
        }).catch(error => console.error("Failed to update cursor settings:", error));
    };

    useEffect(() => {
        invoke("set_cursor_settings", {
            showClicks: mouse.showClicks,
            keepHighlight: mouse.keepHighlight,
            size: mouse.size,
            color: mouse.color,
        }).catch(error => console.error("Failed to update cursor settings:", error));
    }, [mouse.showClicks, mouse.keepHighlight, mouse.size, mouse.color]);

    return (
        <div className="flex flex-col gap-y-4 p-6">
            <h1 className="text-xl font-semibold">{t("Mouse")}</h1>

            <Item variant="muted">
                <ItemContent>
                    <ItemTitle>
                        <HugeiconsIcon icon={Cursor02Icon} size="1em" /> {t("Cursor Highlight")}
                    </ItemTitle>
                    <ItemDescription>
                        {t("The cursor ring uses a small click-through overlay and does not block other windows.")}
                    </ItemDescription>
                </ItemContent>
            </Item>

            <ItemGrid>
                <Item variant="muted">
                    <ItemContent>
                        <ItemTitle>
                            <HugeiconsIcon icon={MouseLeftClick05Icon} size="1em" /> {t("Show Clicks")}
                        </ItemTitle>
                        <ItemDescription>{t("Animate a ring upon mouse press")}</ItemDescription>
                    </ItemContent>
                    <ItemActions>
                        <Switch checked={mouse.showClicks} onCheckedChange={showClicks => updateMouse({ showClicks })} />
                    </ItemActions>
                </Item>

                <Item variant="muted">
                    <ItemContent>
                        <ItemTitle>{t("Always Highlight")}</ItemTitle>
                        <ItemDescription>{t("Permanently show the ring around the cursor")}</ItemDescription>
                    </ItemContent>
                    <ItemActions>
                        <Switch checked={mouse.keepHighlight} onCheckedChange={keepHighlight => updateMouse({ keepHighlight })} />
                    </ItemActions>
                </Item>

                <Item variant="muted">
                    <ItemContent><ItemTitle>{t("Size")}</ItemTitle></ItemContent>
                    <ItemActions>
                        <NumberInput className="w-28 h-8" minValue={32} value={mouse.size} onChange={size => updateMouse({ size })} />
                    </ItemActions>
                </Item>

                <Item variant="muted">
                    <ItemContent><ItemTitle>{t("Color")}</ItemTitle></ItemContent>
                    <ItemActions>
                        <ColorInput value={mouse.color} onChange={color => updateMouse({ color })} />
                    </ItemActions>
                </Item>
            </ItemGrid>

            <h2 className="mt-2 text-sm font-medium text-muted-foreground">{t("Event")}</h2>
            <Item variant="muted">
                <ItemContent>
                    <ItemTitle>
                        <HugeiconsIcon icon={Drag03Icon} size="1em" /> {t("Drag Threshold")}
                    </ItemTitle>
                    <ItemDescription>{t("Minimum distance in pixels to show Drag event")}</ItemDescription>
                </ItemContent>
                <ItemActions>
                    <NumberInput className="w-32 h-8" value={dragThreshold} onChange={setDragThreshold} />
                </ItemActions>
            </Item>
        </div>
    );
};

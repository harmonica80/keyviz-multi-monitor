import { ColorInput } from "@/components/ui/color-picker";
import { Item, ItemActions, ItemContent, ItemDescription, ItemGrid, ItemTitle } from "@/components/ui/item";
import { NumberInput } from "@/components/ui/number-input";
import { Switch } from "@/components/ui/switch";
import { useTranslation } from "@/lib/i18n";
import { useKeyStyle } from "@/stores/key_style";
import { Cursor02Icon } from "@hugeicons/core-free-icons";
import { HugeiconsIcon } from "@hugeicons/react";
import { invoke } from "@tauri-apps/api/core";
import { useEffect } from "react";

export const MouseSettings = () => {
    const { t } = useTranslation();
    const mouse = useKeyStyle(state => state.mouse);
    const setMouse = useKeyStyle(state => state.setMouse);

    const updateMouse = (patch: Partial<typeof mouse>) => {
        const nextMouse = { ...mouse, ...patch };
        setMouse(patch);
        invoke("set_cursor_settings", {
            keepHighlight: nextMouse.keepHighlight,
            size: nextMouse.size,
            color: nextMouse.color,
            opacity: nextMouse.opacity,
            thickness: nextMouse.thickness,
        }).catch(error => console.error("Failed to update cursor settings:", error));
    };

    useEffect(() => {
        invoke("set_cursor_settings", {
            keepHighlight: mouse.keepHighlight,
            size: mouse.size,
            color: mouse.color,
            opacity: mouse.opacity,
            thickness: mouse.thickness,
        }).catch(error => console.error("Failed to update cursor settings:", error));
    }, [mouse.keepHighlight, mouse.size, mouse.color, mouse.opacity, mouse.thickness]);

    return (
        <div className="flex flex-col gap-y-4 p-6">
            <h1 className="text-xl font-semibold">{t("Mouse")}</h1>

            <Item variant="muted">
                <ItemContent>
                    <ItemTitle>
                        <HugeiconsIcon icon={Cursor02Icon} size="1em" /> {t("Cursor Highlight")}
                    </ItemTitle>
                    <ItemDescription>
                        {t("Display a transparent click-through ring around the cursor without blocking other windows.")}
                    </ItemDescription>
                </ItemContent>
            </Item>

            <ItemGrid>
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

                <Item variant="muted">
                    <ItemContent><ItemTitle>{t("Opacity")}</ItemTitle></ItemContent>
                    <ItemActions>
                        <NumberInput
                            className="w-28 h-8"
                            minValue={10}
                            maxValue={100}
                            value={mouse.opacity}
                            onChange={opacity => updateMouse({ opacity })}
                        />
                    </ItemActions>
                </Item>

                <Item variant="muted">
                    <ItemContent><ItemTitle>{t("Line Thickness")}</ItemTitle></ItemContent>
                    <ItemActions>
                        <NumberInput
                            className="w-28 h-8"
                            minValue={1}
                            maxValue={30}
                            value={mouse.thickness}
                            onChange={thickness => updateMouse({ thickness })}
                        />
                    </ItemActions>
                </Item>
            </ItemGrid>
        </div>
    );
};

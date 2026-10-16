import { invoke } from '@tauri-apps/api/core';

import { ShortcutRecorder } from '@/components/shortcut-recorder';
import { Button } from '@/components/ui/button';
import {
    Drawer,
    DrawerContent,
    DrawerDescription,
    DrawerHeader,
    DrawerTitle,
    DrawerTrigger,
} from "@/components/ui/drawer";
import { Item, ItemActions, ItemContent, ItemDescription, ItemHeader, ItemTitle } from "@/components/ui/item";
import { NumberInput } from '@/components/ui/number-input';
import { Switch } from "@/components/ui/switch";
import { ToggleGroup, ToggleGroupItem } from '@/components/ui/toggle-group';
import { cn } from "@/lib/utils";
import { useTranslation } from "@/lib/i18n";
import { KeyEventState, useKeyEvent } from "@/stores/key_event";
import { KeyStyleState, useKeyStyle } from "@/stores/key_style";
import { ArrowHorizontalIcon, ArrowVerticalIcon, FilterHorizontalIcon, FilterIcon, LayerIcon, PaintBoardIcon, ToggleOnIcon } from "@hugeicons/core-free-icons";
import { HugeiconsIcon } from "@hugeicons/react";
import { CustomFilter } from '../custom-filter';


export const GeneralSettings = () => {
    const { t } = useTranslation();
    const {
        filter, setFilter,
        allowedKeys,
        showEventHistory, setShowEventHistory,
        maxHistory, setMaxHistory,
        toggleShortcut, setToggleShortcut,
        drawingToggleShortcut, setDrawingToggleShortcut,
        drawingPointerShortcut, setDrawingPointerShortcut,
        drawingClearShortcut, setDrawingClearShortcut,
        drawingUndoShortcut, setDrawingUndoShortcut,
        drawingCloseShortcut, setDrawingCloseShortcut,
    } = useKeyEvent();

    const direction = useKeyStyle(state => state.appearance.flexDirection);
    const setAppearance = useKeyStyle(state => state.setAppearance);

    return <div className="flex flex-col gap-y-4 p-6">
        <h1 className="text-xl font-semibold">{t("General")}</h1>

        <Item variant="muted">
            <ItemContent>
                <ItemTitle>
                    <HugeiconsIcon icon={FilterIcon} size="1em" /> {t("Filter")}
                </ItemTitle>
                <ItemDescription>
                    {filter === 'none' && t("No filter applied, all keys will be shown.")}
                    {filter === 'modifiers' && t("Only modifier keys will be shown.")}
                    {filter === 'custom' && t("Custom filter applied, {count} keys allowed.", { count: allowedKeys.length })}
                </ItemDescription>
            </ItemContent>
            <ItemActions>
                {
                    filter === 'custom' &&
                    <Drawer>
                        <DrawerTrigger asChild>
                            <Button variant="outline" size="icon-sm">
                                <HugeiconsIcon icon={FilterHorizontalIcon} />
                            </Button>
                        </DrawerTrigger>
                        <DrawerContent>
                            <DrawerContent>
                                <DrawerHeader>
                                    <DrawerTitle>{t("Custom Filter")}</DrawerTitle>
                                    <DrawerDescription>{t("Select which keys to display. Hold down Ctrl to toggle related keys.")}</DrawerDescription>
                                </DrawerHeader>
                                <CustomFilter />
                            </DrawerContent>
                        </DrawerContent>
                    </Drawer>
                }
                <ToggleGroup
                    size="sm"
                    type="single"
                    variant="outline"
                    value={filter}
                    onValueChange={(value) => {
                        if (value) setFilter(value as KeyEventState["filter"]);
                    }}
                >
                    <ToggleGroupItem value="none" aria-label="No Filter">{t("Off")}</ToggleGroupItem>
                    <ToggleGroupItem value="modifiers" aria-label="Modifiers Only">{t("Hotkeys")}</ToggleGroupItem>
                    <ToggleGroupItem value="custom" aria-label="Custom Filter">{t("Custom")}</ToggleGroupItem>
                </ToggleGroup>
            </ItemActions>
        </Item>

        <Item variant="muted">
            <ItemContent>
                <ItemTitle>
                    <HugeiconsIcon icon={PaintBoardIcon} size="1em" /> {t("Screen Drawing")}
                </ItemTitle>
                <ItemDescription>
                    {t("Draw and annotate directly across all displays.")}
                </ItemDescription>
            </ItemContent>
            <ItemActions>
                <Button variant="outline" size="sm" onClick={() => invoke("open_screen_drawing")}>
                    {t("Start Drawing")}
                </Button>
            </ItemActions>
        </Item>

        <Item variant="muted">
            <ItemHeader>
                <ItemContent>
                    <ItemTitle>
                        <HugeiconsIcon icon={LayerIcon} size="1em" /> {t("History")}
                    </ItemTitle>
                    <ItemDescription>
                        {t("Keep previously pressed keystrokes in the view")}
                    </ItemDescription>
                </ItemContent>
                <ItemActions>
                    <Switch aria-label={t("History")} checked={showEventHistory} onCheckedChange={setShowEventHistory} />
                </ItemActions>
            </ItemHeader>

            <fieldset disabled={!showEventHistory} aria-label={t("History")}
                className={cn("flex min-w-0 w-full flex-col gap-3 border-t pt-3 md:flex-row transition-opacity", !showEventHistory && "opacity-50")}>
            <Item className="flex-7 bg-background/60">
                <ItemContent>
                    <ItemTitle>{t("Direction")}</ItemTitle>
                </ItemContent>
                <ItemActions>
                    <ToggleGroup
                        size="sm"
                        type="single"
                        variant="outline"
                        disabled={!showEventHistory}
                        value={direction}
                        onValueChange={(value) => {
                            if (value) setAppearance({ flexDirection: value as KeyStyleState["appearance"]["flexDirection"] });
                        }}
                    >
                        <ToggleGroupItem value="row" aria-label="Horizontal">
                            <HugeiconsIcon icon={ArrowHorizontalIcon} strokeWidth={2} size={10} /> {t("Row")}
                        </ToggleGroupItem>
                        <ToggleGroupItem value="column" aria-label="Vertical">
                            <HugeiconsIcon icon={ArrowVerticalIcon} strokeWidth={2} /> {t("Column")}
                        </ToggleGroupItem>
                    </ToggleGroup>
                </ItemActions>
            </Item>
            <Item className="flex-5 bg-background/60">
                <ItemContent>
                    <ItemTitle>{t("Max Count")}</ItemTitle>
                </ItemContent>
                <ItemActions className="max-w-20">
                    <NumberInput aria-label={t("Max Count")} isDisabled={!showEventHistory} className="h-8" value={maxHistory} onChange={setMaxHistory} minValue={2} maxValue={12} />
                </ItemActions>
            </Item>
            </fieldset>
        </Item>

        <Item variant="muted">
            <ItemHeader className="flex-col items-start">
                <ItemTitle>
                    <HugeiconsIcon icon={ToggleOnIcon} size="1em" /> {t("Toggle Shortcut")}
                </ItemTitle>
                <ItemDescription>
                    {t("Global shortcut to show/hide the key visualizer, click box to set")}
                </ItemDescription>
            </ItemHeader>
            <ItemContent>
                <ShortcutRecorder value={toggleShortcut} onChange={shortcut => {
                    setToggleShortcut(shortcut);
                    invoke('set_toggle_shortcut', { shortcut });
                }} />
            </ItemContent>
        </Item>

        <Item variant="muted">
            <ItemHeader className="flex-col items-start">
                <ItemTitle>
                    <HugeiconsIcon icon={PaintBoardIcon} size="1em" /> {t("Screen Drawing Shortcut")}
                </ItemTitle>
                <ItemDescription>
                    {t("Global shortcut to toggle screen drawing")}
                </ItemDescription>
            </ItemHeader>
            <ItemContent>
                <div className="flex flex-col gap-3 rounded-xl bg-background p-3">
                    <ShortcutRow
                        label={t("Toggle Screen Drawing")}
                        value={drawingToggleShortcut}
                        onChange={(shortcut) => {
                            setDrawingToggleShortcut(shortcut);
                            invoke("set_drawing_shortcuts", { toggleShortcut: shortcut });
                        }}
                    />
                    <ShortcutRow
                        label={t("Return To Mouse Mode")}
                        value={drawingPointerShortcut}
                        onChange={(shortcut) => {
                            setDrawingPointerShortcut(shortcut);
                            invoke("set_drawing_shortcuts", { pointerShortcut: shortcut });
                        }}
                    />
                    <ShortcutRow
                        label={t("Clear Screen Drawing")}
                        value={drawingClearShortcut}
                        onChange={(shortcut) => {
                            setDrawingClearShortcut(shortcut);
                            invoke("set_drawing_shortcuts", { clearShortcut: shortcut });
                        }}
                    />
                    <ShortcutRow
                        label={t("Undo Screen Drawing")}
                        value={drawingUndoShortcut}
                        onChange={(shortcut) => {
                            setDrawingUndoShortcut(shortcut);
                            invoke("set_drawing_shortcuts", { undoShortcut: shortcut });
                        }}
                    />
                    <ShortcutRow
                        label={t("Close Screen Drawing")}
                        value={drawingCloseShortcut}
                        onChange={(shortcut) => {
                            setDrawingCloseShortcut(shortcut);
                            invoke("set_drawing_shortcuts", { closeShortcut: shortcut });
                        }}
                    />
                </div>
            </ItemContent>
        </Item>
    </div>;
}

const ShortcutRow = ({
    label,
    value,
    onChange,
}: {
    label: string;
    value: string[];
    onChange: (shortcut: string[]) => void;
}) => (
    <div className="grid gap-2 md:grid-cols-[10rem_1fr] md:items-center">
        <span className="text-sm text-muted-foreground">{label}</span>
        <ShortcutRecorder value={value} onChange={onChange} />
    </div>
);

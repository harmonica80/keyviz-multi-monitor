import {
  DRAWING_TOOL_BY_ID,
  type DrawingTool,
} from "@/lib/drawing-tools";
import { useTranslation } from "@/lib/i18n";
import {
  type RadialStyle,
  useDrawingToolbar,
} from "@/stores/drawing_toolbar";
import { DRAWING_THEMES, drawingThemeVariables } from "@/lib/drawing-themes";
import { Button } from "@/components/ui/button";
import { DrawingToolbarView } from "@/components/drawing-toolbar-view";
import { ShortcutRecorder } from "@/components/shortcut-recorder";
import { radialShortcutError } from "@/lib/radial-shortcut";
import { useKeyEvent } from "@/stores/key_event";
import { Item, ItemActions, ItemContent, ItemDescription, ItemTitle } from "@/components/ui/item";
import { Switch } from "@/components/ui/switch";
import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group";
import { invoke } from "@tauri-apps/api/core";
import {
  Check,
  ChevronDown,
  ChevronUp,
  GripVertical,
  PanelLeft,
  PanelRight,
  RotateCcw,
} from "lucide-react";
import { useEffect, useRef, useState } from "react";

interface ToolbarDragSession {
  pointerId: number;
  sourceId: DrawingTool;
  targetId: DrawingTool;
}

export const DrawingToolbarSettings = () => {
  const { t } = useTranslation();
  const items = useDrawingToolbar((state) => state.items);
  const side = useDrawingToolbar((state) => state.side);
  const hoverEdit = useDrawingToolbar((state) => state.hoverEdit);
  const setHoverEdit = useDrawingToolbar((state) => state.setHoverEdit);
  const setItems = useDrawingToolbar((state) => state.setItems);
  const setToolVisible = useDrawingToolbar((state) => state.setToolVisible);
  const setSide = useDrawingToolbar((state) => state.setSide);
  const resetToolbar = useDrawingToolbar((state) => state.resetToolbar);
  const radialShortcut = useDrawingToolbar((state) => state.radialShortcut);
  const setRadialShortcut = useDrawingToolbar((state) => state.setRadialShortcut);
  const radialRightClick = useDrawingToolbar((state) => state.radialRightClick);
  const setRadialRightClick = useDrawingToolbar((state) => state.setRadialRightClick);
  const radialStyle = useDrawingToolbar((state) => state.radialStyle);
  const setRadialStyle = useDrawingToolbar((state) => state.setRadialStyle);
  const radialColors = useDrawingToolbar((state) => state.radialColors);
  const setRadialColor = useDrawingToolbar((state) => state.setRadialColor);
  const [shortcutError, setShortcutError] = useState<string | null>(null);
  const [draggedTool, setDraggedTool] = useState<DrawingTool | null>(null);
  const [dropTarget, setDropTarget] = useState<DrawingTool | null>(null);
  const dragSession = useRef<ToolbarDragSession | null>(null);

  useEffect(() => {
    void invoke("set_drawing_toolbar_side", { side }).catch((error) => {
      console.error("Failed to update drawing toolbar side:", error);
    });
  }, [side]);

  const reorder = (sourceId: DrawingTool, targetId: DrawingTool) => {
    if (sourceId === targetId) return;
    const next = items.map((item) => ({ ...item }));
    const sourceIndex = next.findIndex((item) => item.id === sourceId);
    const originalTargetIndex = next.findIndex((item) => item.id === targetId);
    if (sourceIndex < 0 || originalTargetIndex < 0) return;
    const [moved] = next.splice(sourceIndex, 1);
    let targetIndex = next.findIndex((item) => item.id === targetId);
    if (sourceIndex < originalTargetIndex) targetIndex += 1;
    next.splice(targetIndex, 0, moved);
    setItems(next);
  };

  const moveBy = (id: DrawingTool, offset: number) => {
    const index = items.findIndex((item) => item.id === id);
    const targetIndex = index + offset;
    if (index < 0 || targetIndex < 0 || targetIndex >= items.length) return;
    const next = items.map((item) => ({ ...item }));
    [next[index], next[targetIndex]] = [next[targetIndex], next[index]];
    setItems(next);
  };

  const finishDrag = () => {
    const session = dragSession.current;
    if (session) reorder(session.sourceId, session.targetId);
    dragSession.current = null;
    setDraggedTool(null);
    setDropTarget(null);
  };

  const cancelDrag = () => {
    dragSession.current = null;
    setDraggedTool(null);
    setDropTarget(null);
  };

  return (
    <div className="flex flex-col gap-y-4 p-6">
      <div className="flex flex-wrap items-center justify-between gap-3">
        <div>
          <h1 className="text-xl font-semibold">{t("Drawing Toolbar")}</h1>
          <p className="mt-1 text-sm text-muted-foreground">
            {t("Reorder drawing tools and choose which buttons appear in the toolbar.")}
          </p>
        </div>
        <Button variant="outline" size="sm" onClick={() => { resetToolbar(); setShortcutError(null); }}>
          <RotateCcw className="mr-2 size-4" />
          {t("Restore Default Toolbar")}
        </Button>
      </div>

      <h2 className="text-lg font-semibold">{t("Circular Menu Settings")}</h2>
      <Item variant="muted">
        <ItemContent>
          <ItemDescription>{t("Choose the two quick colors shown in the circular menu. These colors do not change the toolbar palette.")}</ItemDescription>
          <div className="mt-3 grid gap-3 sm:grid-cols-2">
            {([0, 1] as const).map((index) => (
              <label key={index} className="flex items-center justify-between gap-3 rounded-lg border p-3">
                <span>{t(index === 0 ? "Quick Color 1" : "Quick Color 2")}</span>
                <input
                  type="color"
                  value={radialColors[index]}
                  onChange={(event) => setRadialColor(index, event.currentTarget.value)}
                  aria-label={t(index === 0 ? "Quick Color 1" : "Quick Color 2")}
                  className="h-9 w-12 cursor-pointer rounded border"
                />
              </label>
            ))}
          </div>
          <ItemDescription>{t("Click the blank center to dismiss the circular menu.")}</ItemDescription>
        </ItemContent>
      </Item>

      <Item variant="muted">
        <ItemContent>
          <ItemTitle>{t("Circular Tool Menu Shortcut")}</ItemTitle>
          <ItemDescription>{t("Press Q while drawing to open the two-ring icon menu beside the mouse. Click a tool to switch; press Esc or right-click to cancel. In pointer mode, settings or text entry, this shortcut is inactive.")}</ItemDescription>
          <ShortcutRecorder value={radialShortcut} onChange={(keys) => {
            const state = useKeyEvent.getState();
            const error = radialShortcutError(keys, [state.toggleShortcut, state.drawingToggleShortcut,
              state.drawingPointerShortcut, state.drawingClearShortcut, state.drawingUndoShortcut,
              state.drawingCloseShortcut, ["ControlLeft", "ShiftLeft", "KeyZ"]]);
            setShortcutError(error);
            if (!error) setRadialShortcut(keys);
          }} placeholder={t("Disabled (click to set)")} />
          <ItemDescription>{t("Default: Q. Click to record a new shortcut; Backspace or Delete disables it. Hidden tools leave empty sectors to keep positions stable.")}</ItemDescription>
          {shortcutError && <p role="alert" className="text-sm text-destructive">{t(shortcutError)}</p>}
          <label className="mt-3 flex items-center justify-between gap-3 rounded-lg border p-3">
            <span>{t("Right-click to Open Circular Menu")}</span>
            <Switch checked={radialRightClick} onCheckedChange={setRadialRightClick} aria-label={t("Right-click to Open Circular Menu")} />
          </label>
          <ItemDescription>{t("While using a drawing tool, right-click the canvas to open the menu; right-click again to cancel. Pointer mode, text entry and toolbar context menus are unchanged. The keyboard shortcut remains available.")}</ItemDescription>
          <ItemDescription>{t("In pointer mode, hold Left or Right Alt and click the right mouse button to open the circular menu. Press F7 (or your configured Return To Mouse Mode shortcut) to return to pointer mode, even when the toolbar is hidden.")}</ItemDescription>
        </ItemContent>
      </Item>

      <Item variant="muted">
        <ItemContent>
          <ItemTitle>{t("Drawing Theme")}</ItemTitle>
          <ItemDescription>{t("Use one theme for the toolbar and circular menu. Default: 01 Pearl. Drawing colors are unchanged.")}</ItemDescription>
          <div className="mt-3 grid grid-cols-2 gap-3 sm:grid-cols-3" role="group" aria-label={t("Drawing Theme")}>
            {DRAWING_THEMES.map(({ id, label }) => (
              <button
                key={id}
                type="button"
                aria-pressed={radialStyle === id}
                onClick={() => setRadialStyle(id as RadialStyle)}
                className={`radial-style-card rounded-lg border p-3 text-sm transition-colors hover:border-primary focus-visible:outline-2 focus-visible:outline-primary ${radialStyle === id ? "border-primary bg-primary/5 ring-1 ring-primary" : "bg-background"}`}
              >
                <RadialStylePreview style={id as RadialStyle} />
                <span className="mt-2 flex items-center justify-center gap-1">
                  {t(label)}
                  {radialStyle === id && <Check className="size-3" aria-hidden="true" />}
                </span>
              </button>
            ))}
          </div>
        </ItemContent>
      </Item>

      <Item variant="muted">
        <ItemContent>
          <ItemTitle>{t("Toolbar Opening Side")}</ItemTitle>
          <ItemDescription>
            {t("Choose which side of the primary display the drawing toolbar opens on.")}
          </ItemDescription>
        </ItemContent>
        <ItemActions>
          <ToggleGroup
            type="single"
            variant="outline"
            size="sm"
            value={side}
            onValueChange={(value) => setSide(value as "left" | "right")}
          >
            <ToggleGroupItem value="left" aria-label={t("Left Side")}>
              <PanelLeft />
              {t("Left Side")}
            </ToggleGroupItem>
            <ToggleGroupItem value="right" aria-label={t("Right Side")}>
              <PanelRight />
              {t("Right Side")}
            </ToggleGroupItem>
          </ToggleGroup>
        </ItemActions>
      </Item>

      <Item variant="muted">
        <ItemContent>
          <ItemTitle>{t("Hover to Edit Objects")}</ItemTitle>
          <ItemDescription>{t("Tap Alt to enable or disable hover editing for this drawing session. Sessions start with hover editing off. Enabled object types can be moved or resized with the wheel. Pointer mode never intercepts Alt; the selection tool still supports manual selection.")}</ItemDescription>
          <div className="mt-3 grid gap-3 sm:grid-cols-2">
            {(["number", "check-mark", "cross-mark", "text", "ellipse", "rectangle", "line", "arrow", "pen"] as DrawingTool[]).map((id) => (
              <label key={id} className="flex items-center justify-between gap-3 rounded-lg border p-3">
                <span>{t(DRAWING_TOOL_BY_ID[id].label)}</span>
                <Switch checked={hoverEdit[id] !== false} onCheckedChange={(enabled) => setHoverEdit(id, enabled)} aria-label={`${t("Hover to Edit Objects")}: ${t(DRAWING_TOOL_BY_ID[id].label)}`} />
              </label>
            ))}
          </div>
        </ItemContent>
      </Item>

      <Item variant="muted">
        <ItemContent>
          <ItemTitle>{t("Customize Drawing Tools")}</ItemTitle>
          <ItemDescription>
            {t("Drag the rows or use the arrow buttons to change their order.")}
          </ItemDescription>
        </ItemContent>
      </Item>

      <div className="grid items-start gap-5 lg:grid-cols-[minmax(0,1fr)_13rem]">
        <div className="flex flex-col gap-2">
          {items.map((item, index) => {
            const definition = DRAWING_TOOL_BY_ID[item.id];
            const Icon = definition.icon;
            return (
              <div
                key={item.id}
                data-toolbar-tool={item.id}
                className={`flex items-center gap-3 rounded-xl border bg-card p-3 transition ${
                  dropTarget === item.id ? "border-primary ring-2 ring-primary/20" : ""
                } ${item.visible ? "" : "opacity-55"} ${
                  draggedTool === item.id ? "scale-[0.99] opacity-70" : ""
                }`}
              >
                <button
                  type="button"
                  title={t("Drag to reorder")}
                  aria-label={`${t("Drag to reorder")}: ${t(definition.label)}`}
                  className="grid size-7 shrink-0 touch-none select-none place-items-center rounded-md text-muted-foreground hover:bg-muted active:cursor-grabbing"
                  style={{ cursor: draggedTool === item.id ? "grabbing" : "grab" }}
                  onPointerDown={(event) => {
                    if (event.button !== 0) return;
                    event.preventDefault();
                    event.currentTarget.setPointerCapture(event.pointerId);
                    dragSession.current = {
                      pointerId: event.pointerId,
                      sourceId: item.id,
                      targetId: item.id,
                    };
                    setDraggedTool(item.id);
                    setDropTarget(item.id);
                  }}
                  onPointerMove={(event) => {
                    const session = dragSession.current;
                    if (!session || session.pointerId !== event.pointerId) return;
                    const target = document
                      .elementFromPoint(event.clientX, event.clientY)
                      ?.closest<HTMLElement>("[data-toolbar-tool]");
                    const targetId = target?.dataset.toolbarTool as DrawingTool | undefined;
                    if (!targetId || !DRAWING_TOOL_BY_ID[targetId]) return;
                    session.targetId = targetId;
                    setDropTarget(targetId);
                  }}
                  onPointerUp={(event) => {
                    const session = dragSession.current;
                    if (!session || session.pointerId !== event.pointerId) return;
                    finishDrag();
                  }}
                  onPointerCancel={cancelDrag}
                >
                  <GripVertical className="size-5" />
                </button>
                <div className="grid size-10 shrink-0 place-items-center rounded-lg border bg-background">
                  <Icon />
                </div>
                <div className="min-w-0 flex-1">
                  <p className="font-medium">{t(definition.label)}</p>
                  {!definition.canHide && (
                    <p className="text-xs text-muted-foreground">
                      {t("Pointer is always available so drawing mode can be exited safely.")}
                    </p>
                  )}
                </div>
                <div className="flex shrink-0 items-center gap-1">
                  <Button
                    variant="ghost"
                    size="icon-sm"
                    disabled={index === 0}
                    title={t("Move Up")}
                    aria-label={t("Move Up")}
                    onClick={() => moveBy(item.id, -1)}
                  >
                    <ChevronUp />
                  </Button>
                  <Button
                    variant="ghost"
                    size="icon-sm"
                    disabled={index === items.length - 1}
                    title={t("Move Down")}
                    aria-label={t("Move Down")}
                    onClick={() => moveBy(item.id, 1)}
                  >
                    <ChevronDown />
                  </Button>
                  <Switch
                    checked={item.visible}
                    disabled={!definition.canHide}
                    aria-label={`${t(definition.label)} ${t(item.visible ? "Shown" : "Hidden")}`}
                    onCheckedChange={(visible) => setToolVisible(item.id, visible)}
                  />
                </div>
              </div>
            );
          })}
        </div>

        <div className="sticky top-6 rounded-xl border bg-muted/40 p-4">
          <p className="mb-3 text-sm font-medium">{t("Toolbar Preview")}</p>
          <div className="mx-auto w-12">
            <DrawingToolbarView toolbarItems={items} radialStyle={radialStyle} preview />
          </div>
        </div>
      </div>
    </div>
  );
};

const PREVIEW_TOOLS: DrawingTool[] = ["pen", "arrow", "number", "eraser", "rectangle", "ellipse", "check-mark", "cross-mark", "text", "line", "select", "pointer"];

const RadialStylePreview = ({ style }: { style: RadialStyle }) => (
  <span className="drawing-theme-preview" style={drawingThemeVariables(style)} aria-hidden="true">
    <span className="theme-preview-toolbar">
      {["pointer", "pen", "arrow", "number", "rectangle"].map((id) => {
        const Icon = DRAWING_TOOL_BY_ID[id as DrawingTool].icon;
        return <span key={id} className={id === "pen" ? "active" : ""}><Icon /></span>;
      })}
    </span>
    <span className="radial-style-preview">
    <span className="radial-preview-ring radial-preview-outer" />
    <span className="radial-preview-ring radial-preview-inner" />
    {PREVIEW_TOOLS.map((id, index) => {
      const outer = index >= 6;
      const angle = (index % 6) * Math.PI / 3 - Math.PI / 2;
      const radius = outer ? 41 : 20;
      const Icon = DRAWING_TOOL_BY_ID[id].icon;
      return (
        <span key={id} className="radial-preview-icon" style={{ left: `${50 + Math.cos(angle) * radius}%`, top: `${50 + Math.sin(angle) * radius}%` }}>
          <Icon />
        </span>
      );
    })}
    <span className="radial-preview-center" />
    </span>
  </span>
);

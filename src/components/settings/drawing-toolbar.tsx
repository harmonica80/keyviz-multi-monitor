import {
  DRAWING_TOOL_BY_ID,
  type DrawingTool,
} from "@/lib/drawing-tools";
import { useTranslation } from "@/lib/i18n";
import {
  useDrawingToolbar,
} from "@/stores/drawing_toolbar";
import { Button } from "@/components/ui/button";
import { Item, ItemActions, ItemContent, ItemDescription, ItemTitle } from "@/components/ui/item";
import { Switch } from "@/components/ui/switch";
import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group";
import { invoke } from "@tauri-apps/api/core";
import {
  Check,
  ChevronDown,
  ChevronUp,
  GripHorizontal,
  GripVertical,
  PanelLeft,
  PanelRight,
  Redo2,
  RotateCcw,
  Trash2,
  X,
} from "lucide-react";
import { type ButtonHTMLAttributes, useEffect, useRef, useState } from "react";

interface ToolbarDragSession {
  pointerId: number;
  sourceId: DrawingTool;
  targetId: DrawingTool;
}

export const DrawingToolbarSettings = () => {
  const { t } = useTranslation();
  const items = useDrawingToolbar((state) => state.items);
  const side = useDrawingToolbar((state) => state.side);
  const setItems = useDrawingToolbar((state) => state.setItems);
  const setToolVisible = useDrawingToolbar((state) => state.setToolVisible);
  const setSide = useDrawingToolbar((state) => state.setSide);
  const resetToolbar = useDrawingToolbar((state) => state.resetToolbar);
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
        <Button variant="outline" size="sm" onClick={resetToolbar}>
          <RotateCcw className="mr-2 size-4" />
          {t("Restore Default Toolbar")}
        </Button>
      </div>

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
          <div className="mx-auto flex w-14 flex-col gap-1 rounded-md bg-[#f7f7f7] p-1 shadow-sm">
            <PreviewButton className="min-h-6 bg-[#e8e8e8]">
              <GripHorizontal className="size-5" />
            </PreviewButton>
            <PreviewButton className="bg-[#c83737] text-white">
              <X />
            </PreviewButton>
            {items.filter((item) => item.visible).map((item) => {
              const definition = DRAWING_TOOL_BY_ID[item.id];
              const Icon = definition.icon;
              return (
                <PreviewButton key={item.id} title={t(definition.label)}>
                  <Icon />
                </PreviewButton>
              );
            })}
            <div className="grid grid-cols-2 justify-center gap-px py-1">
              {["#ef2b2d", "#16c43b", "#2d37d6", "#d1af4b", "#ffffff", "#111111"].map(
                (color, index) => (
                  <span
                    key={color}
                    className="relative block aspect-square border border-[#777]"
                    style={{ background: color }}
                  >
                    {index === 0 && <Check className="absolute inset-0 size-full text-white" />}
                  </span>
                ),
              )}
            </div>
            <div className="flex h-6 items-center rounded-sm border border-[#9a9a9a] bg-white px-1">
              <div className="h-1.5 w-full rounded-full bg-gradient-to-r from-[#787d7d] from-40% to-white to-40%" />
            </div>
            <PreviewButton disabled>
              <Redo2 className="scale-x-[-1]" />
            </PreviewButton>
            <PreviewButton>
              <Trash2 />
            </PreviewButton>
          </div>
        </div>
      </div>
    </div>
  );
};

const PreviewButton = ({
  children,
  className = "",
  ...props
}: ButtonHTMLAttributes<HTMLButtonElement>) => (
  <button
    type="button"
    tabIndex={-1}
    className={`grid min-h-9 w-full place-items-center rounded-sm border border-[#9a9a9a] bg-white text-[#333] [&>svg]:size-6 ${className}`}
    {...props}
  >
    {children}
  </button>
);

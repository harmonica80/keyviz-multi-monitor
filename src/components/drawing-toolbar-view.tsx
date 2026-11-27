import { TOOLBAR_ITEM_BY_ID, type DrawingTool } from "@/lib/drawing-tools";
import { drawingThemeVariables } from "@/lib/drawing-themes";
import { useTranslation } from "@/lib/i18n";
import type { useOverlayProbe } from "@/lib/overlay-probe";
import type { DrawingToolbarItem, RadialStyle } from "@/stores/drawing_toolbar";
import { DEFAULT_TOOLBAR_COLORS } from "@/lib/radial-colors";
import { visibleUnitCount } from "@/lib/toolbar-layout";
import type { NumberLabel, NumberStyle, TextOutline } from "@/stores/drawing_toolbar";
import { Check, EyeOff, FileJson, FolderOpen, GripHorizontal, Group, ImageDown, Keyboard, KeyboardOff, MoreHorizontal, Redo2, Undo2, RefreshCw, Settings, Trash2, X } from "lucide-react";
import { useLayoutEffect, useRef, useState } from "react";
import type { CSSProperties, ReactNode, Ref } from "react";

export const DRAWING_TOOLBAR_COLORS: readonly string[] = DEFAULT_TOOLBAR_COLORS;

type ToolbarCommand =
  | { type: "tool"; value: DrawingTool }
  | { type: "color"; value: string }
  | { type: "width"; value: number }
  | { type: "group" | "undo" | "redo" | "clear" };

const OUTLINE_COLORS = ["#000000", "#ffffff", "#facc15", "#ef2b2d", "#2f3fe0", "#17c13f"];

export type ToolbarOrientation = "vertical" | "horizontal";
export type PanelSide = "left" | "right" | "top" | "bottom";

interface DrawingToolbarViewProps {
  toolbarRef?: Ref<HTMLElement>;
  /** Wraps the toolbar and its "…" panel; the window is sized to this element. */
  shellRef?: Ref<HTMLDivElement>;
  orientation?: ToolbarOrientation;
  /** 1, 2/3 or 1/2: scales the single-icon buttons only. */
  iconScale?: number;
  /** Longest allowed toolbar in px along its main axis (0 = unlimited). */
  maxMain?: number;
  /** Open "…" panel and the side of the toolbar it extends to. */
  morePanel?: { open: boolean; side: PanelSide };
  onToggleMore?: () => void;
  /** Number marker look; the switcher appears while the number tool is active. */
  numberStyle?: NumberStyle;
  onNumberStyle?: (style: NumberStyle) => void;
  /** Digits (123) or letters (ABC) inside the number marker. */
  numberLabel?: NumberLabel;
  onNumberLabel?: (label: NumberLabel) => void;
  /** Text outline option, shown while the text tool is active. */
  textOutline?: TextOutline;
  onTextOutline?: (outline: TextOutline) => void;
  /** Reports whether some tools are collapsed into the "…" panel. */
  onOverflowChange?: (overflow: boolean) => void;
  toolbarItems: DrawingToolbarItem[];
  radialStyle: RadialStyle;
  /** Toolbar swatches; the check marks the one equal to `color` (none when it is not listed). */
  colors?: readonly string[];
  preview?: boolean;
  tool?: DrawingTool;
  color?: string;
  width?: number;
  minWidth?: number;
  maxWidth?: number;
  canUndo?: boolean;
  canRedo?: boolean;
  closeShortcutLabel?: string;
  undoShortcutLabel?: string;
  selectionState?: { count: number; grouped: boolean };
  operationHint?: { text: string } | null;
  probeMenu?: boolean;
  probe?: ReturnType<typeof useOverlayProbe>;
  keyDisplayEnabled?: boolean;
  zoomActive?: boolean;
  zoomShortcutLabel?: string;
  onToggleZoom?: () => void;
  onSnip?: () => void;
  sendCommand?: (command: ToolbarCommand) => void;
  setProbeMenu?: (open: boolean) => void;
  onDrag?: () => void;
  onHide?: () => void;
  onClose?: () => void;
  onToggleKeyDisplay?: () => void;
  onSettings?: () => void;
  onRecover?: () => void;
  onSavePng?: () => void;
  onSaveJson?: () => void;
  onOpenJson?: () => void;
}

const noop = () => {};

export function DrawingToolbarView({
  toolbarRef, shellRef, orientation = "vertical", iconScale = 1, maxMain = 0,
  morePanel = { open: false, side: "right" }, onToggleMore, numberStyle = "circle", onNumberStyle, numberLabel = "digits", onNumberLabel,
  textOutline = { enabled: false, color: "#000000" }, onTextOutline, onOverflowChange,
  toolbarItems, radialStyle, colors = DRAWING_TOOLBAR_COLORS, preview = false,
  tool = "pen", color = DRAWING_TOOLBAR_COLORS[0], width = 5,
  minWidth = 1, maxWidth = 15, canUndo = false, canRedo = false,
  closeShortcutLabel = "", undoShortcutLabel = "",
  selectionState = { count: 0, grouped: false }, operationHint = null,
  probeMenu = false, probe = { busy: false, result: null, run: async () => {} },
  keyDisplayEnabled = true, zoomActive = false, zoomShortcutLabel = "", onToggleZoom, onSnip,
  sendCommand = noop, setProbeMenu = noop,
  onDrag, onHide, onClose, onToggleKeyDisplay, onSettings, onRecover,
  onSavePng, onSaveJson, onOpenJson,
}: DrawingToolbarViewProps) {
  const { t } = useTranslation();
  const toolButtons = toolbarItems.filter((item) => item.visible).map((item) => TOOLBAR_ITEM_BY_ID[item.id]);
  const COLORS = colors;
  const activeColor = color.toLowerCase();
  const horizontal = orientation === "horizontal";
  const [outlinePalette, setOutlinePalette] = useState(false);

  // Every part of the toolbar is a "unit" so the last ones can move into the "…" panel.
  const units: { key: string; divider?: boolean; node: ReactNode }[] = [];
  units.push({
    key: "handle",
    node: (
      <button className="drawing-drag-handle" title={t("Move Toolbar")} onPointerDown={onDrag}>
        <GripHorizontal />
      </button>
    ),
  });
  units.push({
    key: "window",
    node: (
      <div className="drawing-window-actions">
        <button
          title={t("Hide toolbar (keep drawings and current tool)")}
          aria-label={t("Hide toolbar (keep drawings and current tool)")}
          onClick={onHide}
        >
          <EyeOff />
        </button>
        <button
          className="drawing-close"
          title={`${t("Exit Drawing")}${closeShortcutLabel ? ` (${closeShortcutLabel})` : ""}`}
          aria-label={t("Exit Drawing")}
          onClick={onClose}
        >
          <X />
        </button>
      </div>
    ),
  });
  toolButtons.forEach(({ id, label, icon: Icon }) => {
    let node: ReactNode;
    if (id === "zoom") {
      node = (
        <button
          className={`drawing-single drawing-zoom-button ${zoomActive ? "active" : ""}`}
          aria-pressed={zoomActive}
          aria-label={t("Screen magnifier")}
          title={`${t("Screen magnifier")}${zoomShortcutLabel ? ` (${zoomShortcutLabel})` : ""}\n${t("Wheel: zoom in/out · Esc or right-click: leave")}`}
          onClick={onToggleZoom}
        >
          <Icon />
        </button>
      );
    } else if (id === "snip") {
      node = (
        <button
          className="drawing-single drawing-snip-button"
          aria-label={t("Screenshot and pin")}
          title={`${t("Screenshot and pin")}\n${t("Drag to select an area, then pin it as a sticky note, save, or copy it")}`}
          onClick={onSnip}
        >
          <Icon />
        </button>
      );
    } else {
      node = (
        <button
          className={`drawing-single ${tool === id ? "active" : ""}`}
          title={`${t(label)}${id !== "pointer" && id !== "select" ? ` (${t("Press Alt to toggle object editing mode")})` : ""}`}
          onClick={() => void sendCommand({ type: "tool", value: id })}
        >
          <Icon />
        </button>
      );
    }
    units.push({ key: `tool-${id}`, node });
  });
  if (tool === "select") {
    units.push({
      key: "group",
      node: (
        <button
          className={`drawing-single ${selectionState.grouped ? "active" : ""}`}
          disabled={selectionState.count < 2}
          aria-pressed={selectionState.grouped}
          title={t("Group / Ungroup")}
          onClick={() => void sendCommand({ type: "group" })}
        >
          <Group />
        </button>
      ),
    });
  }
  if (tool === "text") {
    units.push({
      key: "text-outline",
      node: (
        <div className="drawing-text-outline">
          <label title={t("Text outline")}>
            <input
              type="checkbox"
              checked={textOutline.enabled}
              onChange={(event) => onTextOutline?.({ ...textOutline, enabled: event.target.checked })}
            />
            <span>{t("Outline")}</span>
          </label>
          <button
            className="drawing-outline-swatch"
            disabled={!textOutline.enabled}
            title={t("Outline color")}
            aria-label={t("Outline color")}
            style={{ "--drawing-color": textOutline.color } as CSSProperties}
            onClick={() => setOutlinePalette((open) => !open)}
          />
          {outlinePalette && textOutline.enabled && (
            <div className="drawing-outline-palette">
              {OUTLINE_COLORS.map((value) => (
                <button
                  key={value}
                  className={value === textOutline.color ? "active" : ""}
                  title={value}
                  style={{ "--drawing-color": value } as CSSProperties}
                  onClick={() => {
                    onTextOutline?.({ ...textOutline, color: value });
                    setOutlinePalette(false);
                  }}
                />
              ))}
            </div>
          )}
        </div>
      ),
    });
  }
  if (tool === "number") {
    units.push({
      key: "number-style",
      node: (
        <div className="drawing-number-style" role="group" aria-label={t("Number style")}>
          <button
            className={numberStyle === "circle" ? "active" : ""}
            aria-pressed={numberStyle === "circle"}
            title={t("Circle number")}
            aria-label={t("Circle number")}
            onClick={() => onNumberStyle?.("circle")}
          >
            <svg viewBox="0 0 20 20" aria-hidden="true">
              <circle cx="10" cy="10" r="8" fill="currentColor" />
              <text x="10" y="14" textAnchor="middle" fontSize="11" fontWeight="600" className="drawing-number-digit">1</text>
            </svg>
          </button>
          <button
            className={numberStyle === "rounded" ? "active" : ""}
            aria-pressed={numberStyle === "rounded"}
            title={t("Rounded rectangle number")}
            aria-label={t("Rounded rectangle number")}
            onClick={() => onNumberStyle?.("rounded")}
          >
            <svg viewBox="0 0 20 20" aria-hidden="true">
              <rect x="2" y="2" width="16" height="16" rx="5" fill="currentColor" />
              <text x="10" y="14" textAnchor="middle" fontSize="11" fontWeight="600" className="drawing-number-digit">1</text>
            </svg>
          </button>
        </div>
      ),
    });
    units.push({
      key: "number-label",
      node: (
        <div className="drawing-number-style drawing-number-label">
          <button
            title={t("Number label")}
            aria-label={t("Number label")}
            onClick={() =>
              onNumberLabel?.(numberLabel === "digits" ? "letters" : numberLabel === "letters" ? "roman" : "digits")
            }
          >
            <svg viewBox="0 0 42 24" aria-hidden="true">
              <text x="21" y="17.5" textAnchor="middle" fontSize="16" fontWeight="700" className="drawing-number-digit">
                {numberLabel === "digits" ? "123" : numberLabel === "letters" ? "ABC" : "I II"}
              </text>
            </svg>
          </button>
        </div>
      ),
    });
  }
  units.push({ key: "div-colors", divider: true, node: <div className="drawing-divider" /> });
  units.push({
    key: "colors",
    node: (
      <div className="drawing-colors">
        {COLORS.map((value) => (
          <button
            key={value}
            className={`drawing-color ${activeColor === value.toLowerCase() ? "active" : ""}`}
            title={value}
            style={{ "--drawing-color": value } as CSSProperties}
            onClick={() => void sendCommand({ type: "color", value })}
          >
            {activeColor === value.toLowerCase() && <Check className="drawing-color-check" />}
          </button>
        ))}
      </div>
    ),
  });
  units.push({ key: "div-width", divider: true, node: <div className="drawing-divider" /> });
  units.push({
    key: "width",
    node: (
      <label className="drawing-width-slider" title={`${t("Line Thickness")} ${width}`}>
        <input
          aria-label={t("Line Thickness")}
          type="range"
          min={minWidth}
          max={maxWidth}
          step={1}
          value={width}
          style={{
            "--drawing-width-progress": `${Math.min(100, Math.max(0, ((width - minWidth) / (maxWidth - minWidth)) * 100))}%`,
          } as CSSProperties}
          onChange={(event) => void sendCommand({ type: "width", value: Number(event.target.value) })}
        />
      </label>
    ),
  });
  units.push({
    key: "history",
    node: (
      <div className="drawing-history-actions">
        <button
          disabled={!canUndo}
          title={`${t("Undo")}${undoShortcutLabel ? ` (${undoShortcutLabel})` : ""}`}
          onClick={() => void sendCommand({ type: "undo" })}
        >
          <Undo2 />
        </button>
        <button
          disabled={!canRedo}
          title={`${t("Redo")} (Ctrl + Shift + Z)`}
          onClick={() => void sendCommand({ type: "redo" })}
        >
          <Redo2 />
        </button>
      </div>
    ),
  });
  units.push({
    key: "clear",
    node: (
      <button className="drawing-single" title={t("Clear All")} onClick={() => void sendCommand({ type: "clear" })}>
        <Trash2 />
      </button>
    ),
  });
  units.push({ key: "div-files", divider: true, node: <div className="drawing-divider" /> });
  units.push({
    key: "files",
    node: (
      <div className="drawing-file-actions">
        <button title={t("Save screen as PNG")} aria-label={t("Save screen as PNG")} onClick={onSavePng}><ImageDown /></button>
        <button title={t("Save screen as JSON (editable)")} aria-label={t("Save screen as JSON (editable)")} onClick={onSaveJson}><FileJson /></button>
        <button title={t("Open JSON to edit")} aria-label={t("Open JSON to edit")} onClick={onOpenJson}><FolderOpen /></button>
      </div>
    ),
  });
  units.push({ key: "div-footer", divider: true, node: <div className="drawing-divider" /> });
  units.push({
    key: "footer",
    node: (
      <div className="drawing-footer-actions">
        {probeMenu ? <>
          <button disabled={probe.busy} title={t("Test Display Layer")} onClick={() => void probe.run("display")}>D</button>
          <button disabled={probe.busy} title={t("Test Input Layer")} onClick={() => void probe.run("input")}>I</button>
          <button title={t("Back")} onClick={() => setProbeMenu(false)}><X /></button>
        </> : <>
          <button
            className={!keyDisplayEnabled ? "active" : ""}
            aria-pressed={!keyDisplayEnabled}
            title={t(keyDisplayEnabled ? "Stop Key Display" : "Show Key Display")}
            onClick={onToggleKeyDisplay}
          >
            {keyDisplayEnabled ? <KeyboardOff /> : <Keyboard />}
          </button>
          <button title={t("Open Settings")} onClick={onSettings}>
            <Settings />
          </button>
          <button title={t("Restore display; right-click for layer tests")} onContextMenu={(event) => {
            event.preventDefault(); setProbeMenu(true);
          }} onClick={onRecover}><RefreshCw /></button>
        </>}
      </div>
    ),
  });

  // Units that do not fit (toolbar longer than `maxMain`) move into the "…" panel.
  const measureRef = useRef<HTMLDivElement>(null);
  const [visibleCount, setVisibleCount] = useState(units.length);
  const signature = units.map((unit) => unit.key).join("|");
  useLayoutEffect(() => {
    const container = measureRef.current;
    if (preview || !container || !(maxMain > 0)) {
      setVisibleCount(units.length);
      return;
    }
    const compute = () => {
      const sizes = Array.from(container.children).map((child) =>
        horizontal ? (child as HTMLElement).offsetWidth : (child as HTMLElement).offsetHeight,
      );
      setVisibleCount(
        visibleUnitCount(sizes, units.map((unit) => Boolean(unit.divider)), {
          padding: horizontal ? 16 : 23,
          gap: 1,
          limit: maxMain,
          moreSize: Math.ceil(38 * iconScale),
        }),
      );
    };
    compute();
    const observer = new ResizeObserver(compute);
    observer.observe(container);
    return () => observer.disconnect();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [signature, horizontal, maxMain, iconScale, preview, toolbarItems, probeMenu]);

  const count = Math.min(visibleCount, units.length);
  const overflow = !preview && count < units.length;
  const hiddenUnits = overflow ? units.slice(count).filter((unit, index) => !(index === 0 && unit.divider)) : [];
  const onOverflowRef = useRef(onOverflowChange);
  onOverflowRef.current = onOverflowChange;
  useLayoutEffect(() => {
    onOverflowRef.current?.(overflow);
  }, [overflow]);

  const renderUnits = (list: typeof units) =>
    list.map((unit) => (
      <div key={unit.key} className={`drawing-unit${unit.divider ? " drawing-unit-divider" : ""}`}>
        {unit.node}
      </div>
    ));
  const panelOpen = overflow && morePanel.open;
  const shellStyle = { ...drawingThemeVariables(radialStyle), "--tb-scale": iconScale, "--tb-max": maxMain > 0 ? `${maxMain}px` : "none" } as CSSProperties;

  return (
    <div ref={shellRef} className={`drawing-shell panel-${panelOpen ? morePanel.side : "none"}`} style={shellStyle}>
      <aside ref={toolbarRef} className={`drawing-toolbar ${orientation}`} aria-label={t("Screen Drawing")} inert={preview || undefined}>
        {renderUnits(units.slice(0, count))}
        {overflow && (
          <div className="drawing-unit">
            <button
              className={`drawing-single drawing-more ${panelOpen ? "active" : ""}`}
              aria-expanded={panelOpen}
              aria-label={t("More tools")}
              title={t("More tools")}
              onClick={onToggleMore}
            >
              <MoreHorizontal />
            </button>
          </div>
        )}
        {(operationHint || probeMenu) && <output className="drawing-operation-hint" role="status">
          {probeMenu ? (probe.busy ? t("Testing for 5 seconds...") : probe.result
            ? t(probe.result.error ?? "Test finished. Before and after reports saved.")
            : t("Choose D or I to test one layer for 5 seconds.")) : operationHint?.text}
        </output>}
      </aside>
      {panelOpen && (
        <div className={`drawing-toolbar drawing-panel ${orientation}`}>
          {renderUnits(hiddenUnits)}
        </div>
      )}
      {!preview && (
        <div ref={measureRef} className={`drawing-toolbar drawing-measure ${orientation}`} aria-hidden="true" inert>
          {renderUnits(units)}
        </div>
      )}
    </div>
  );
}

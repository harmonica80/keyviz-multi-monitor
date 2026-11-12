import { DRAWING_TOOL_BY_ID, type DrawingTool } from "@/lib/drawing-tools";
import { drawingThemeVariables } from "@/lib/drawing-themes";
import { useTranslation } from "@/lib/i18n";
import type { useOverlayProbe } from "@/lib/overlay-probe";
import type { DrawingToolbarItem, RadialStyle } from "@/stores/drawing_toolbar";
import { Check, EyeOff, FileJson, FolderOpen, GripHorizontal, Group, ImageDown, Keyboard, KeyboardOff, Redo2, Undo2, RefreshCw, Settings, Trash2, X } from "lucide-react";
import type { CSSProperties, Ref } from "react";

export const DRAWING_TOOLBAR_COLORS = ["#ef2b2d", "#16c43b", "#2d37d6", "#d1af4b", "#ffffff", "#111111"];

type ToolbarCommand =
  | { type: "tool"; value: DrawingTool }
  | { type: "color"; value: string }
  | { type: "width"; value: number }
  | { type: "group" | "undo" | "redo" | "clear" };

interface DrawingToolbarViewProps {
  toolbarRef?: Ref<HTMLElement>;
  toolbarItems: DrawingToolbarItem[];
  radialStyle: RadialStyle;
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
  toolbarRef, toolbarItems, radialStyle, preview = false,
  tool = "pen", color = DRAWING_TOOLBAR_COLORS[0], width = 5,
  minWidth = 1, maxWidth = 15, canUndo = false, canRedo = false,
  closeShortcutLabel = "", undoShortcutLabel = "",
  selectionState = { count: 0, grouped: false }, operationHint = null,
  probeMenu = false, probe = { busy: false, result: null, run: async () => {} },
  keyDisplayEnabled = true, sendCommand = noop, setProbeMenu = noop,
  onDrag, onHide, onClose, onToggleKeyDisplay, onSettings, onRecover,
  onSavePng, onSaveJson, onOpenJson,
}: DrawingToolbarViewProps) {
  const { t } = useTranslation();
  const toolButtons = toolbarItems.filter((item) => item.visible).map((item) => DRAWING_TOOL_BY_ID[item.id]);
  const COLORS = DRAWING_TOOLBAR_COLORS;
  return (
    <aside ref={toolbarRef} className="drawing-toolbar" style={drawingThemeVariables(radialStyle)} aria-label={t("Screen Drawing")} inert={preview || undefined}>
      <button
        className="drawing-drag-handle"
        title={t("Move Toolbar")}
        onPointerDown={onDrag}
      >
        <GripHorizontal />
      </button>
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
      {toolButtons.map(({ id, label, icon: Icon }) => (
        <button
          key={id}
          className={tool === id ? "active" : ""}
          title={`${t(label)}${id !== "pointer" && id !== "select" ? ` (${t("Press Alt to toggle object editing mode")})` : ""}`}
          onClick={() => void sendCommand({ type: "tool", value: id })}
        >
          <Icon />
        </button>
      ))}
      {tool === "select" && (
        <button
          className={selectionState.grouped ? "active" : ""}
          disabled={selectionState.count < 2}
          aria-pressed={selectionState.grouped}
          title={t("Group / Ungroup")}
          onClick={() => void sendCommand({ type: "group" })}
        >
          <Group />
        </button>
      )}
      <div className="drawing-divider" />
      <div className="drawing-colors">
        {COLORS.map((value) => (
          <button
            key={value}
            className={`drawing-color ${color === value ? "active" : ""}`}
            title={value}
            style={{ "--drawing-color": value } as CSSProperties}
            onClick={() => void sendCommand({ type: "color", value })}
          >
            {color === value && <Check className="drawing-color-check" />}
          </button>
        ))}
      </div>
      <div className="drawing-divider" />
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
      <div className="drawing-history-actions"><button
        disabled={!canUndo}
        title={`${t("Undo")}${undoShortcutLabel ? ` (${undoShortcutLabel})` : ""}`}
        onClick={() => void sendCommand({ type: "undo" })}
      >
        <Undo2 />
      </button>
      <button disabled={!canRedo} title={`${t("Redo")} (Ctrl + Shift + Z)`}
        onClick={() => void sendCommand({ type: "redo" })}><Redo2 /></button></div>
      {(operationHint || probeMenu) && <output className="drawing-operation-hint" role="status">
        {probeMenu ? (probe.busy ? t("Testing for 5 seconds...") : probe.result
          ? t(probe.result.error ?? "Test finished. Before and after reports saved.")
          : t("Choose D or I to test one layer for 5 seconds.")) : operationHint?.text}
      </output>}
      <button title={t("Clear All")} onClick={() => void sendCommand({ type: "clear" })}>
        <Trash2 />
      </button>
      <div className="drawing-divider" />
      <div className="drawing-file-actions">
        <button title={t("Save screen as PNG")} aria-label={t("Save screen as PNG")} onClick={onSavePng}><ImageDown /></button>
        <button title={t("Save screen as JSON (editable)")} aria-label={t("Save screen as JSON (editable)")} onClick={onSaveJson}><FileJson /></button>
        <button title={t("Open JSON to edit")} aria-label={t("Open JSON to edit")} onClick={onOpenJson}><FolderOpen /></button>
      </div>
      <div className="drawing-divider" />
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
        <button
          title={t("Open Settings")}
          onClick={onSettings}
        >
          <Settings />
        </button>
      <button title={t("Restore display; right-click for layer tests")} onContextMenu={(event) => {
        event.preventDefault(); setProbeMenu(true);
      }} onClick={onRecover}><RefreshCw /></button>
      </>}
      </div>
    </aside>
  );
}


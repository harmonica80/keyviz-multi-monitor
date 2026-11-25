import { useTranslation } from "@/lib/i18n";
import { DrawingToolbarView, DRAWING_TOOLBAR_COLORS, type PanelSide, type ToolbarOrientation } from "@/components/drawing-toolbar-view";
import { useOverlayProbe } from "@/lib/overlay-probe";
import { keymaps } from "@/lib/keymaps";
import { radialToolbarKeyAction } from "@/lib/radial-shortcut";
import { drawingThemeVariables } from "@/lib/drawing-themes";
import {
  DRAWING_TOOL_BY_ID,
  isDrawingToolItem,
  type DrawingTool,
} from "@/lib/drawing-tools";
import {
  DRAWING_TOOLBAR_STORE,
  TOOLBAR_ICON_SCALE,
  type DrawingToolbarStore,
  useDrawingToolbar,
} from "@/stores/drawing_toolbar";
import { useKeyEvent } from "@/stores/key_event";
import { listenForUpdates } from "@/stores/sync";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { ask, message, open as openFileDialog } from "@tauri-apps/plugin-dialog";
import { availableMonitors, currentMonitor, cursorPosition, getCurrentWindow } from "@tauri-apps/api/window";
import { toolbarLimit } from "@/lib/toolbar-layout";
import {
  KeyboardEvent as ReactKeyboardEvent,
  PointerEvent,
  useCallback,
  useEffect,
  useRef,
  useState,
} from "react";

type Point = { x: number; y: number };
type TextEditor = { start: Point; value: string; color: string; width: number };
type Tool = DrawingTool;
type Drawing =
  | { tool: "pen" | "eraser"; points: Point[]; color: string; width: number }
  | { tool: "line" | "arrow" | "rectangle" | "ellipse" | "check-mark" | "cross-mark"; start: Point; end: Point; color: string; width: number }
  | { tool: "text"; start: Point; text: string; color: string; width: number }
  | { tool: "number"; center: Point; value: number; color: string; width: number };
type DrawingCommand =
  | { type: "tool"; value: Tool }
  | { type: "color"; value: string }
  | { type: "width"; value: number }
  | { type: "clear" }
  | { type: "group" }
  | { type: "undo" }
  | { type: "redo" };
type ToolChangedPayload = { tool: Tool };
/** `color`: shared color of the selected objects (null when mixed or nothing is selected). */
type DrawingSelectionPayload = { count: number; grouped: boolean; color?: string | null };

const COLORS = DRAWING_TOOLBAR_COLORS;
const WIDTHS = [2, 5, 9, 15];
const MIN_WIDTH = 1;
const MAX_WIDTH = WIDTHS[WIDTHS.length - 1];

const keyboardEventKey = (event: KeyboardEvent): string => {
  if (event.code.startsWith("Key")) return event.code;
  if (event.code.startsWith("Digit")) return `Num${event.code.slice(5)}`;
  if (event.code.startsWith("Numpad")) return `Kp${event.code.slice(6)}`;
  return event.key.length === 1 ? event.key : event.key;
};

const shortcutMatchesEvent = (event: KeyboardEvent, shortcut: string[]) => {
  if (shortcut.length === 0) return false;
  const mainKey = keyboardEventKey(event);
  return shortcut.every((key) => {
    if (key === "ControlLeft" || key === "ControlRight" || key === "Control") return event.ctrlKey;
    if (key === "ShiftLeft" || key === "ShiftRight" || key === "Shift") return event.shiftKey;
    if (key === "Alt" || key === "AltLeft" || key === "AltRight") return event.altKey;
    if (key === "MetaLeft" || key === "MetaRight" || key === "Meta") return event.metaKey;
    return key === mainKey || (key.startsWith("Num") && mainKey === `Kp${key.slice(3)}`);
  });
};

const formatShortcut = (shortcut: string[]) =>
  shortcut.map((key) => keymaps[key]?.label ?? key).join(" + ");

const drawTaperedArrow = (
  context: CanvasRenderingContext2D,
  start: Point,
  end: Point,
  width: number,
) => {
  const dx = end.x - start.x;
  const dy = end.y - start.y;
  const distance = Math.hypot(dx, dy);
  if (distance < 2) return;
  const ux = dx / distance;
  const uy = dy / distance;
  const nx = -uy;
  const ny = ux;
  const headLength = Math.min(Math.min(Math.max(26, 24 + width * 0.7), 36), distance * 0.38);
  const headHalf = Math.min(Math.min(Math.max(14, 11 + width * 0.9), 25), distance * 0.2);
  const startHalf = 0.7;
  const shaftHalf = Math.min(Math.max(4, 3 + width * 0.45), 10);
  const point = (along: number, normal: number) => ({
    x: start.x + ux * along + nx * normal,
    y: start.y + uy * along + ny * normal,
  });
  const headBase = distance - headLength;
  const points = [
    point(0, startHalf),
    point(headBase, shaftHalf),
    point(headBase, headHalf),
    end,
    point(headBase, -headHalf),
    point(headBase, -shaftHalf),
    point(0, -startHalf),
  ];
  context.beginPath();
  context.moveTo(points[0].x, points[0].y);
  points.slice(1).forEach((pointValue) => context.lineTo(pointValue.x, pointValue.y));
  context.closePath();
  context.fill();
};

const renderDrawing = (context: CanvasRenderingContext2D, drawing: Drawing) => {
  context.save();
  context.lineCap = "round";
  context.lineJoin = "round";
  context.lineWidth = drawing.width;
  context.strokeStyle = drawing.color;
  context.fillStyle = drawing.color;

  if ("points" in drawing) {
    if (drawing.points.length < 2) {
      context.restore();
      return;
    }
    context.globalCompositeOperation =
      drawing.tool === "eraser" ? "destination-out" : "source-over";
    context.lineWidth = drawing.tool === "eraser" ? drawing.width * 12 : drawing.width;
    context.beginPath();
    context.moveTo(drawing.points[0].x, drawing.points[0].y);
    drawing.points.slice(1).forEach((point) => context.lineTo(point.x, point.y));
    context.stroke();
    context.restore();
    return;
  }

  if (drawing.tool === "text") {
    context.font = `${Math.max(18, drawing.width * 4)}px "Microsoft JhengHei", sans-serif`;
    context.fillText(drawing.text, drawing.start.x, drawing.start.y);
    context.restore();
    return;
  }

  if (drawing.tool === "number") {
    const radius = 14 + drawing.width * 2;
    context.beginPath();
    context.arc(drawing.center.x, drawing.center.y, radius, 0, Math.PI * 2);
    context.fill();
    context.fillStyle = "#ffffff";
    context.font = `${drawing.value >= 100 ? radius : Math.max(16, radius * 1.2)}px "Microsoft JhengHei", sans-serif`;
    context.textAlign = "center";
    context.textBaseline = "middle";
    context.fillText(String(drawing.value), drawing.center.x, drawing.center.y);
    context.restore();
    return;
  }

  const shapeWidth = drawing.end.x - drawing.start.x;
  const shapeHeight = drawing.end.y - drawing.start.y;
  if (drawing.tool === "arrow") {
    drawTaperedArrow(context, drawing.start, drawing.end, drawing.width);
    context.restore();
    return;
  }
  if (drawing.tool === "check-mark") {
    context.beginPath();
    context.moveTo(drawing.start.x, drawing.start.y + shapeHeight * 0.52);
    context.lineTo(drawing.start.x + shapeWidth * 0.38, drawing.end.y);
    context.lineTo(drawing.end.x, drawing.start.y);
    context.stroke();
    context.restore();
    return;
  }
  if (drawing.tool === "cross-mark") {
    context.beginPath();
    context.moveTo(drawing.start.x, drawing.start.y);
    context.lineTo(drawing.end.x, drawing.end.y);
    context.moveTo(drawing.end.x, drawing.start.y);
    context.lineTo(drawing.start.x, drawing.end.y);
    context.stroke();
    context.restore();
    return;
  }
  context.beginPath();
  if (drawing.tool === "line") {
    context.moveTo(drawing.start.x, drawing.start.y);
    context.lineTo(drawing.end.x, drawing.end.y);
  } else if (drawing.tool === "rectangle") {
    context.rect(drawing.start.x, drawing.start.y, shapeWidth, shapeHeight);
  } else {
    context.ellipse(
      drawing.start.x + shapeWidth / 2,
      drawing.start.y + shapeHeight / 2,
      Math.abs(shapeWidth / 2),
      Math.abs(shapeHeight / 2),
      0,
      0,
      Math.PI * 2,
    );
  }
  context.stroke();
  context.restore();
};

export default function ScreenDrawing() {
  const { t } = useTranslation();
  const mode = new URLSearchParams(window.location.search).get("mode");
  const isToolbar = mode === "drawing-toolbar";
  const toolbarRef = useRef<HTMLElement>(null);
  const shellRef = useRef<HTMLDivElement>(null);
  const [orientation, setOrientation] = useState<ToolbarOrientation>("vertical");
  const [moreOpen, setMoreOpen] = useState(false);
  const [panelSide, setPanelSide] = useState<PanelSide>("right");
  const [monitorLogical, setMonitorLogical] = useState<{ width: number; height: number } | null>(null);
  const panelSideRef = useRef<PanelSide>("right");
  panelSideRef.current = panelSide;
  const orientationRef = useRef<ToolbarOrientation>("vertical");
  orientationRef.current = orientation;
  // After switching orientation the toolbar snaps to the screen edge it was dropped on.
  const snapRef = useRef<{
    until: number;
    edge: "top" | "bottom" | "left" | "right";
    monitor: { x: number; y: number; width: number; height: number; scale: number };
    cursorX: number;
    cursorY: number;
  } | null>(null);
  const forceResizeRef = useRef<() => void>(() => undefined);
  const startSnipRef = useRef<() => void>(() => undefined);
  const verifyBurstRef = useRef<() => void>(() => undefined);
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const drawingsRef = useRef<Drawing[]>([]);
  const activeRef = useRef<Drawing | null>(null);
  const textEditorRef = useRef<TextEditor | null>(null);
  const textInputRef = useRef<HTMLInputElement>(null);
  const nextNumberRef = useRef(1);
  const [tool, setTool] = useState<Tool>("pen");
  const [color, setColor] = useState(COLORS[0]);
  const [width, setWidth] = useState(WIDTHS[1]);
  const minWidth = tool === "text" ? 15 : MIN_WIDTH;
  const maxWidth = tool === "text" ? 45 : MAX_WIDTH;
  const [canUndo, setCanUndo] = useState(false);
  const [canRedo, setCanRedo] = useState(false);
  const [operationHint, setOperationHint] = useState<{ text: string } | null>(null);
  const [probeMenu, setProbeMenu] = useState(false);
  const probe = useOverlayProbe();
  useEffect(() => {
    if (!operationHint) return;
    const timer = window.setTimeout(() => setOperationHint(null), 1800);
    return () => window.clearTimeout(timer);
  }, [operationHint]);
  const [keyDisplayEnabled, setKeyDisplayEnabled] = useState(true);
  const [zoomActive, setZoomActive] = useState(false);
  const [selectionState, setSelectionState] = useState<DrawingSelectionPayload>({
    count: 0,
    grouped: false,
  });
  const [textEditor, setTextEditor] = useState<TextEditor | null>(null);
  const toolbarItems = useDrawingToolbar((state) => state.items);
  const iconSize = useDrawingToolbar((state) => state.iconSize);
  const side = useDrawingToolbar((state) => state.side);
  const numberStyle = useDrawingToolbar((state) => state.numberStyle);
  const setNumberStyle = useDrawingToolbar((state) => state.setNumberStyle);
  const hoverEdit = useDrawingToolbar((state) => state.hoverEdit);
  const radialShortcut = useDrawingToolbar((state) => state.radialShortcut);
  const radialRightClick = useDrawingToolbar((state) => state.radialRightClick);
  const radialStyle = useDrawingToolbar((state) => state.radialStyle);
  const radialColors = useDrawingToolbar((state) => state.radialColors);
  useEffect(() => {
    if (!isToolbar) return;
    const variables = drawingThemeVariables(radialStyle);
    for (const [key, value] of Object.entries(variables)) {
      document.documentElement.style.setProperty(key, String(value));
    }
    return () => {
      for (const key of Object.keys(variables)) document.documentElement.style.removeProperty(key);
    };
  }, [isToolbar, radialStyle]);
  useEffect(() => {
    if (!isToolbar) return;
    void invoke("set_drawing_radial_menu", {
      shortcut: radialShortcut,
      rightClick: radialRightClick,
      style: radialStyle,
      colors: radialColors,
      tools: toolbarItems.filter((item) => item.visible && isDrawingToolItem(item.id)).map((item) => item.id),
    }).catch(console.error);
  }, [isToolbar, radialShortcut, radialRightClick, radialStyle, radialColors, toolbarItems]);
  useEffect(() => {
    if (!isToolbar) return;
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.target instanceof Element && event.target.closest("textarea, input:not([type='range']), [contenteditable='true']")) return;
      const action = radialToolbarKeyAction(event, radialShortcut, tool === "pointer");
      if (!action) return;
      event.preventDefault();
      event.stopPropagation();
      // Suppress browser button/slider defaults even on auto-repeat.
      if (!event.repeat) void invoke("drawing_radial_menu_action", { cancel: action === "cancel" }).catch(console.error);
    };
    window.addEventListener("keydown", onKeyDown, true);
    return () => window.removeEventListener("keydown", onKeyDown, true);
  }, [isToolbar, radialShortcut, tool]);
  useEffect(() => {
    void invoke("set_drawing_hover_edit", { preferences: hoverEdit }).catch(console.error);
  }, [hoverEdit]);
  const isTextEditorOpen = textEditor !== null;
  const drawingUndoShortcut = useKeyEvent((state) => state.drawingUndoShortcut);
  const drawingCloseShortcut = useKeyEvent((state) => state.drawingCloseShortcut);
  const closeShortcutLabel = formatShortcut(drawingCloseShortcut);
  const undoShortcutLabel = formatShortcut(drawingUndoShortcut);
  const drawingZoomShortcut = useKeyEvent((state) => state.drawingZoomShortcut);
  const toolbarColors = useDrawingToolbar((state) => state.toolbarColors);
  const zoomShortcutLabel = formatShortcut(drawingZoomShortcut);

  const notifyHistory = useCallback(() => {}, []);

  useEffect(() => {
    if (!isToolbar) return;
    void invoke<boolean>("get_key_display_enabled")
      .then(setKeyDisplayEnabled)
      .catch((error) => console.error("Failed to read key display state:", error));
    const listeningListener = listen<boolean>("listening-toggle", (event) => {
      setKeyDisplayEnabled(event.payload);
    });
    return () => {
      void listeningListener.then((unlisten) => unlisten());
    };
  }, [isToolbar]);

  useEffect(() => {
    const toolListener = listen<ToolChangedPayload>("drawing-tool-changed", (event) => {
      setTool(event.payload.tool);
    });
    const zoomListener = listen<boolean>("drawing-zoom-changed", (event) => {
      setZoomActive(event.payload);
    });
    return () => {
      void toolListener.then((unlisten) => unlisten());
      void zoomListener.then((unlisten) => unlisten());
    };
  }, []);

  const redraw = useCallback(() => {
    const canvas = canvasRef.current;
    const context = canvas?.getContext("2d");
    if (!canvas || !context) return;
    context.clearRect(0, 0, canvas.width, canvas.height);
    drawingsRef.current.forEach((drawing) => renderDrawing(context, drawing));
    if (activeRef.current) renderDrawing(context, activeRef.current);
  }, []);

  const updateTextEditor = useCallback((editor: TextEditor | null) => {
    textEditorRef.current = editor;
    setTextEditor(editor);
  }, []);

  const finishTextEditor = useCallback(
    (save: boolean) => {
      const editor = textEditorRef.current;
      if (!editor) return;
      if (save && editor.value.trim()) {
        drawingsRef.current.push({
          tool: "text",
          start: editor.start,
          text: editor.value.trim(),
          color: editor.color,
          width: editor.width,
        });
        redraw();
        notifyHistory();
      }
      updateTextEditor(null);
      void invoke("activate_drawing_toolbar");
    },
    [notifyHistory, redraw, updateTextEditor],
  );

  useEffect(() => {
    if (!isTextEditorOpen) return;

    void invoke("activate_drawing_canvas").finally(() => {
      window.requestAnimationFrame(() => {
        textInputRef.current?.focus();
      });
    });
  }, [isTextEditorOpen]);

  const resizeCanvas = useCallback(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const scale = window.devicePixelRatio || 1;
    canvas.width = Math.round(window.innerWidth * scale);
    canvas.height = Math.round(window.innerHeight * scale);
    canvas.style.width = `${window.innerWidth}px`;
    canvas.style.height = `${window.innerHeight}px`;
    canvas.getContext("2d")?.setTransform(scale, 0, 0, scale, 0, 0);
    redraw();
  }, [redraw]);

  useEffect(() => {
    document.documentElement.classList.add("drawing-page");
    document.body.classList.add("drawing-page");
    if (isToolbar) {
      document.documentElement.classList.add("drawing-toolbar-page");
      document.body.classList.add("drawing-toolbar-page");
    }
    return () => {
      document.documentElement.classList.remove("drawing-page", "drawing-toolbar-page");
      document.body.classList.remove("drawing-page", "drawing-toolbar-page");
    };
  }, [isToolbar]);

  const refreshMonitor = useCallback(async () => {
    try {
      const monitor = await currentMonitor();
      if (monitor) {
        setMonitorLogical({
          width: monitor.size.width / monitor.scaleFactor,
          height: monitor.size.height / monitor.scaleFactor,
        });
      }
    } catch (error) {
      console.error("Failed to read the toolbar monitor:", error);
    }
  }, []);

  useEffect(() => {
    if (isToolbar) void refreshMonitor();
  }, [isToolbar, refreshMonitor]);

  // Orientation, icon size or the open "…" panel change the toolbar size: re-measure.
  useEffect(() => {
    if (!isToolbar) return;
    const frame = window.requestAnimationFrame(() => window.requestAnimationFrame(() => forceResizeRef.current()));
    return () => window.cancelAnimationFrame(frame);
  }, [isToolbar, orientation, moreOpen, panelSide, iconSize, monitorLogical]);

  /** Puts the toolbar in the layout of the chosen opening side (top/bottom = horizontal). */
  const applySideLayout = useCallback(async (target: string) => {
    setMoreOpen(false);
    if (target !== "top" && target !== "bottom") {
      snapRef.current = null;
      setOrientation("vertical");
      return;
    }
    try {
      const monitor = await currentMonitor();
      if (monitor) {
        const area = monitor.workArea ?? { position: monitor.position, size: monitor.size };
        snapRef.current = {
          until: Date.now() + 3000,
          edge: target,
          monitor: {
            x: area.position.x,
            y: area.position.y,
            width: area.size.width,
            height: area.size.height,
            scale: monitor.scaleFactor,
          },
          cursorX: area.position.x + area.size.width / 2,
          cursorY: area.position.y,
        };
      }
    } catch (error) {
      console.error("Failed to prepare the toolbar side layout:", error);
    }
    setOrientation("horizontal");
    // Already horizontal: the size does not change, so ask for a measure to trigger the snap.
    [80, 400].forEach((delay) => window.setTimeout(() => forceResizeRef.current(), delay));
  }, []);

  useEffect(() => {
    if (isToolbar) void applySideLayout(side);
  }, [isToolbar, side, applySideLayout]);

  useEffect(() => {
    if (!isToolbar) return;
    const unlisten = listen("drawing-toolbar-reset-orientation", () => {
      void applySideLayout(useDrawingToolbar.getState().side);
    });
    return () => void unlisten.then((fn) => fn());
  }, [isToolbar, applySideLayout]);

  // Keep the native overlay's number marker style in sync with the saved choice.
  useEffect(() => {
    if (!isToolbar) return;
    void invoke("drawing_set_number_style", { rounded: numberStyle === "rounded" }).catch(console.error);
  }, [isToolbar, numberStyle]);

  /** Dragging ends: near the top/bottom edge of a screen the toolbar becomes horizontal. */
  const handleToolbarDrag = useCallback(async () => {
    await invoke("start_drawing_toolbar_drag").catch(console.error);
    verifyBurstRef.current();
    try {
      const [cursor, monitors] = await Promise.all([cursorPosition(), availableMonitors()]);
      const monitor = monitors.find((item) =>
        cursor.x >= item.position.x && cursor.x < item.position.x + item.size.width &&
        cursor.y >= item.position.y && cursor.y < item.position.y + item.size.height);
      if (!monitor) return;
      void refreshMonitor();
      // Sensing zones (fractions of the screen): left/right strips -> vertical, top/bottom
      // strips -> horizontal, the middle keeps whatever the toolbar is and lets it float.
      const { x: mx, y: my } = monitor.position;
      const { width: mw, height: mh } = monitor.size;
      const inLeft = cursor.x - mx < mw * 0.09;
      const inRight = mx + mw - cursor.x < mw * 0.085;
      const inTop = cursor.y - my < mh * 0.06;
      const inBottom = my + mh - cursor.y < mh * 0.095;
      const area = monitor.workArea ?? { position: monitor.position, size: monitor.size };
      let target = orientationRef.current;
      let edge: "top" | "bottom" | "left" | "right" = "top";
      if (inLeft || inRight) {
        target = "vertical";
        edge = inLeft ? "left" : "right";
      } else if (inTop || inBottom) {
        target = "horizontal";
        edge = inTop ? "top" : "bottom";
      } else {
        return;
      }
      if (target === orientationRef.current) return;
      snapRef.current = {
        until: Date.now() + 1500,
        edge,
        monitor: {
          x: area.position.x,
          y: area.position.y,
          width: area.size.width,
          height: area.size.height,
          scale: monitor.scaleFactor,
        },
        cursorX: cursor.x,
        cursorY: cursor.y,
      };
      setMoreOpen(false);
      setOrientation(target);
    } catch (error) {
      console.error("Failed to decide the toolbar orientation:", error);
    }
  }, [refreshMonitor]);

  /** Opens/closes the "…" panel on the side of the toolbar that has room on screen. */
  const toggleMore = useCallback(async () => {
    if (moreOpen) {
      setMoreOpen(false);
      return;
    }
    try {
      const win = getCurrentWindow();
      const [monitor, position, size] = await Promise.all([currentMonitor(), win.outerPosition(), win.outerSize()]);
      if (monitor) {
        const centerX = position.x + size.width / 2 - (monitor.position.x + monitor.size.width / 2);
        const centerY = position.y + size.height / 2 - (monitor.position.y + monitor.size.height / 2);
        setPanelSide(orientationRef.current === "vertical"
          ? (centerX < 0 ? "right" : "left")
          : (centerY < 0 ? "bottom" : "top"));
      }
    } catch (error) {
      console.error("Failed to decide the panel side:", error);
    }
    setMoreOpen(true);
  }, [moreOpen]);

  useEffect(() => {
    if (isToolbar) {
      let lastKey = "";
      const applySnap = async () => {
        const snap = snapRef.current;
        const shell = shellRef.current;
        if (!snap || !shell) return;
        if (Date.now() > snap.until) {
          snapRef.current = null;
          return;
        }
        const rect = shell.getBoundingClientRect();
        const w = Math.ceil(rect.width * snap.monitor.scale);
        const h = Math.ceil(rect.height * snap.monitor.scale);
        const margin = Math.round(8 * snap.monitor.scale);
        const { x, y, width, height } = snap.monitor;
        const clampX = (value: number) => Math.min(Math.max(value, x + margin), Math.max(x + margin, x + width - w - margin));
        const clampY = (value: number) => Math.min(Math.max(value, y + margin), Math.max(y + margin, y + height - h - margin));
        let targetX = clampX(snap.cursorX - Math.round(w / 2));
        let targetY = y + margin;
        if (snap.edge === "bottom") targetY = y + height - h - margin;
        else if (snap.edge === "left") targetX = x + margin;
        else if (snap.edge === "right") targetX = x + width - w - margin;
        if (snap.edge === "left" || snap.edge === "right") targetY = clampY(y + margin);
        await invoke("place_drawing_toolbar", { x: targetX, y: targetY }).catch(console.error);
      };
      const resizeToolbar = (force = false) => {
        const shell = shellRef.current;
        if (!shell) return;
        // Not scrollHeight/scrollWidth: they would include the hidden measuring copy of
        // every tool, which is taller than the visible toolbar and left a blank strip.
        const rect = shell.getBoundingClientRect();
        const width = Math.ceil(rect.width);
        const height = Math.ceil(rect.height);
        const key = `${width}x${height}`;
        if (width <= 0 || height <= 0 || (!force && key === lastKey)) return;
        lastKey = key;
        void invoke("resize_drawing_toolbar", {
          width,
          height,
          anchorRight: panelSideRef.current === "left",
          anchorBottom: panelSideRef.current === "top",
          devicePixelRatio: window.devicePixelRatio,
        }).then(() => applySnap()).catch(console.error);
      };
      forceResizeRef.current = () => resizeToolbar(true);
      // When the window ends up with a different size than the content (e.g. after a DPI
      // change between monitors), measure again. Bounded so it can never loop.
      let fixes = 0;
      const verifyFit = (limited: boolean) => {
        const shell = shellRef.current;
        if (!shell) return;
        const rect = shell.getBoundingClientRect();
        const off = Math.abs(window.innerHeight - rect.height) > 2 || Math.abs(window.innerWidth - rect.width) > 2;
        if (!off) {
          fixes = 0;
          return;
        }
        if (!limited || fixes < 5) {
          fixes += 1;
          resizeToolbar(true);
        }
      };
      // After a drag or a DPI change the webview can report its new scale a moment late:
      // keep checking for a few seconds so the window always ends up the size of its content.
      verifyBurstRef.current = () => {
        [100, 300, 700, 1200, 2000, 3500].forEach((delay) => window.setTimeout(() => verifyFit(false), delay));
      };
      let verifyTimer = 0;
      const onWindowResize = () => {
        window.clearTimeout(verifyTimer);
        verifyTimer = window.setTimeout(() => verifyFit(true), 150);
      };
      window.addEventListener("resize", onWindowResize);
      const observer = new ResizeObserver(() => resizeToolbar());
      if (shellRef.current) observer.observe(shellRef.current);
      window.requestAnimationFrame(() => window.requestAnimationFrame(() => resizeToolbar()));
      [50, 200, 600, 1200].forEach((delay) =>
        window.setTimeout(() => resizeToolbar(), delay),
      );
      const resizeRequestListener = listen("drawing-toolbar-resize-request", () => {
        resizeToolbar(true);
      });
      const radialSnipListener = listen("drawing-radial-snip", () => startSnipRef.current());
      const scaleListener = getCurrentWindow().onScaleChanged(() => {
        window.requestAnimationFrame(() => {
          void refreshMonitor();
          [0, 150, 500].forEach((delay) => window.setTimeout(() => resizeToolbar(true), delay));
          verifyBurstRef.current();
        });
      });
      const historyListener = listen<{ canUndo: boolean; canRedo: boolean }>("drawing-history", (event) => {
        setCanUndo(event.payload.canUndo);
        setCanRedo(event.payload.canRedo);
      });
      const hintListener = listen<{ object: boolean; tool: Tool; width: number }>("drawing-operation-hint", (event) => {
        setOperationHint({ text: event.payload.object ? t("Resizing Object")
          : `${t(DRAWING_TOOL_BY_ID[event.payload.tool].label)}: ${event.payload.width}` });
      });
      const zoomErrorListener = listen<string>("drawing-zoom-error", (event) => {
        setOperationHint({ text: t(event.payload) });
      });
      const hoverToggleListener = listen<boolean>("drawing-hover-edit-toggled", (event) => {
        setOperationHint({ text: t(event.payload ? "Object editing enabled (Alt)" : "Object editing disabled (Alt)") });
      });
      const widthListener = listen<{ width: number }>("drawing-width-changed", (event) => {
        setWidth(event.payload.width);
      });
      const colorListener = listen<string>("drawing-color-changed", (event) => {
        setColor(event.payload);
      });
      const selectionListener = listen<DrawingSelectionPayload>(
        "drawing-selection-changed",
        (event) => setSelectionState(event.payload),
      );
      const nativeCloseListener = listen("native-drawing-close", () => {
        void invoke("close_screen_drawing");
      });
      const toolbarSettingsListener = listenForUpdates<DrawingToolbarStore>(
        DRAWING_TOOLBAR_STORE,
        useDrawingToolbar.setState,
      );
      return () => {
        observer.disconnect();
        window.removeEventListener("resize", onWindowResize);
        window.clearTimeout(verifyTimer);
        void resizeRequestListener.then((unlisten) => unlisten());
        void radialSnipListener.then((unlisten) => unlisten());
        void scaleListener.then((unlisten) => unlisten());
        void historyListener.then((unlisten) => unlisten());
        void hintListener.then((unlisten) => unlisten());
        void zoomErrorListener.then((unlisten) => unlisten());
        void hoverToggleListener.then((unlisten) => unlisten());
        void widthListener.then((unlisten) => unlisten());
        void colorListener.then((unlisten) => unlisten());
        void selectionListener.then((unlisten) => unlisten());
        void nativeCloseListener.then((unlisten) => unlisten());
        void toolbarSettingsListener.then((unlisten) => unlisten());
      };
    }

    window.setTimeout(resizeCanvas, 100);
    window.addEventListener("resize", resizeCanvas);
    const commandListener = listen<DrawingCommand>("drawing-command", (event) => {
      const command = event.payload;
      if (command.type === "tool") {
        finishTextEditor(true);
        setTool(command.value);
      }
      if (command.type === "color") setColor(command.value);
      if (command.type === "width") setWidth(command.value);
      if (command.type === "clear") {
        updateTextEditor(null);
        drawingsRef.current = [];
        activeRef.current = null;
        nextNumberRef.current = 1;
        redraw();
        notifyHistory();
      }
      if (command.type === "undo") {
        drawingsRef.current.pop();
        redraw();
        notifyHistory();
      }
    });
    const closeListener = listen("drawing-close-request", () => {
      void invoke("close_screen_drawing");
    });
    const onKeyDown = (event: KeyboardEvent) => {
      if (shortcutMatchesEvent(event, drawingCloseShortcut)) {
        event.preventDefault();
        void invoke("close_screen_drawing");
      }
      if (shortcutMatchesEvent(event, drawingUndoShortcut)) {
        event.preventDefault();
        drawingsRef.current.pop();
        redraw();
        notifyHistory();
      }
    };
    window.addEventListener("keydown", onKeyDown);
    void invoke("set_drawing_click_through", { enabled: true });
    return () => {
      window.removeEventListener("resize", resizeCanvas);
      window.removeEventListener("keydown", onKeyDown);
      void commandListener.then((unlisten) => unlisten());
      void closeListener.then((unlisten) => unlisten());
    };
  }, [drawingCloseShortcut, drawingUndoShortcut, finishTextEditor, isToolbar, notifyHistory, redraw, resizeCanvas, updateTextEditor]);

  const splitPath = (path: string) => {
    const cut = Math.max(path.lastIndexOf("\\"), path.lastIndexOf("/"));
    return { dir: cut >= 0 ? path.slice(0, cut) : "", name: path.slice(cut + 1) };
  };
  const saveScreen = async (kind: "png" | "json") => {
    try {
      const dir = useDrawingToolbar.getState().saveDir || null;
      // Saves the screen that has drawings; with drawings on several screens, ask first.
      const drawnScreens = await invoke<number>("drawing_export_monitor_count").catch(() => 0);
      const allDrawn = drawnScreens >= 2 && await ask(
        t("Drawings are on {count} screens. Capture all of them? Each screen is saved as its own file.").replace("{count}", String(drawnScreens)),
        { title: t("Save screen"), kind: "info", okLabel: t("Capture all screens"), cancelLabel: t("Only the toolbar's screen") },
      );
      const result = await invoke<{ path: string; paths?: string[]; closed: boolean }>("drawing_export", { kind, dir, allDrawn });
      const paths = result.paths?.length ? result.paths : [result.path];
      const { dir: folder } = splitPath(paths[0]);
      if (result.closed) {
        // The opened JSON was overwritten and closed: back to normal desktop use.
        setTool("pointer");
        void invoke("drawing_set_tool", { tool: "pointer" });
      }
      const lines = [`${t("Folder")}: ${folder}`, ...paths.map((path) => `${t("File")}: ${splitPath(path).name}`)];
      if (result.closed) lines.push("", t("The original file was overwritten and closed. You are back to normal desktop use."));
      const openFolder = await ask(lines.join("\n"), {
        title: t("Saved successfully"),
        kind: "info",
        okLabel: t("Open Folder"),
        cancelLabel: t("Close"),
      });
      if (openFolder) await invoke("reveal_in_explorer", { path: paths[0] });
    } catch (error) {
      console.error(error);
      setOperationHint({ text: t(String(error)) });
    }
  };
  const openJson = async () => {
    try {
      const defaultPath = await invoke<string>("drawing_export_dir", {
        dir: useDrawingToolbar.getState().saveDir || null,
      }).catch(() => undefined);
      const selected = await openFileDialog({
        multiple: false,
        defaultPath,
        filters: [{ name: "Keyviz JSON", extensions: ["json"] }],
      });
      if (typeof selected !== "string") return;
      await invoke("drawing_import", { path: selected });
      // Select tool: edit objects directly, and Esc closes the session.
      setTool("select");
      await invoke("drawing_set_tool", { tool: "select" });
      await message(
        `${splitPath(selected).name}

${t("1. JSON file opened. Use the Select tool to move, resize or recolor objects, or press Delete to remove them.\n2. When finished, press Save as JSON: it overwrites the original file and closes it.\n3. To leave without saving, press Esc.")}`,
        { title: t("Opened successfully"), kind: "info", okLabel: t("OK") },
      );
    } catch (error) {
      console.error(error);
      setOperationHint({ text: t(String(error)) });
    }
  };

  const toggleZoom = () => {
    void invoke("drawing_set_zoom", { enabled: null }).catch((error) => {
      console.error(error);
      setOperationHint({ text: t(String(error)) });
    });
  };

  const startSnip = () => {
    void invoke("snip_start", { dir: useDrawingToolbar.getState().saveDir || null }).catch((error) => {
      console.error(error);
      setOperationHint({ text: t(String(error)) });
    });
  };

  startSnipRef.current = startSnip;

  const sendCommand = async (command: DrawingCommand) => {
    if (command.type === "tool") {
      // Live zoom keeps every tool usable; the native side leaves a frozen-snapshot zoom itself.
      await invoke("drawing_set_tool", { tool: command.value }).catch((error) => {
        console.error(error);
        setOperationHint({ text: t("Wait for settings to finish opening") });
      });
      return;
    }
    if (command.type === "color") {
      setColor(command.value);
      await invoke("drawing_set_color", { color: command.value });
      return;
    }
    if (command.type === "width") {
      setWidth(command.value);
      await invoke("drawing_set_width", { width: command.value });
      return;
    }
    if (command.type === "clear") {
      await invoke("drawing_clear");
      return;
    }
    if (command.type === "undo") {
      await invoke("drawing_undo");
      return;
    }
    if (command.type === "redo") {
      await invoke("drawing_redo");
      return;
    }
    if (command.type === "group") {
      await invoke("drawing_toggle_group");
    }
  };

  useEffect(() => {
    if (!isToolbar || tool === "pointer") return;
    const activeTool = toolbarItems.find((item) => item.id === tool);
    if (activeTool?.visible !== false) return;
    setTool("pointer");
    void invoke("drawing_set_tool", { tool: "pointer" });
  }, [isToolbar, tool, toolbarItems]);

  const onPointerDown = (event: PointerEvent<HTMLCanvasElement>) => {
    if (tool === "pointer" || tool === "select") return;
    const start = { x: event.clientX, y: event.clientY };
    if (tool === "text") {
      finishTextEditor(true);
      updateTextEditor({ start, value: "", color, width });
      return;
    }
    if (tool === "number") {
      drawingsRef.current.push({
        tool: "number",
        center: start,
        value: nextNumberRef.current,
        color,
        width,
      });
      nextNumberRef.current += 1;
      redraw();
      notifyHistory();
      void invoke("activate_drawing_toolbar");
      return;
    }
    if (tool === "check-mark" || tool === "cross-mark") {
      const radius = 14 + width * 2;
      drawingsRef.current.push({
        tool,
        start: { x: start.x - radius, y: start.y - radius },
        end: { x: start.x + radius, y: start.y + radius },
        color,
        width,
      });
      redraw();
      notifyHistory();
      void invoke("activate_drawing_toolbar");
      return;
    }
    event.currentTarget.setPointerCapture(event.pointerId);
    activeRef.current =
      tool === "pen" || tool === "eraser"
        ? { tool, points: [start], color, width }
        : { tool, start, end: start, color, width };
  };

  const onPointerMove = (event: PointerEvent<HTMLCanvasElement>) => {
    if (!activeRef.current) return;
    const point = { x: event.clientX, y: event.clientY };
    if ("points" in activeRef.current) activeRef.current.points.push(point);
    else if ("end" in activeRef.current) activeRef.current.end = point;
    redraw();
  };

  const finishDrawing = (event: PointerEvent<HTMLCanvasElement>) => {
    if (!activeRef.current) return;
    if (event.currentTarget.hasPointerCapture(event.pointerId)) {
      event.currentTarget.releasePointerCapture(event.pointerId);
    }
    drawingsRef.current.push(activeRef.current);
    activeRef.current = null;
    redraw();
    notifyHistory();
    void invoke("activate_drawing_toolbar");
  };

  const onTextEditorKeyDown = (event: ReactKeyboardEvent<HTMLInputElement>) => {
    if (event.key === "Enter") {
      event.preventDefault();
      finishTextEditor(true);
    } else if (event.key === "Escape") {
      event.preventDefault();
      event.stopPropagation();
      finishTextEditor(false);
    }
  };

  if (!isToolbar) {
    return (
      <main className="drawing-root">
        <canvas
          ref={canvasRef}
          className={`drawing-canvas drawing-cursor-${tool}`}
          onPointerDown={onPointerDown}
          onPointerMove={onPointerMove}
          onPointerUp={finishDrawing}
          onPointerCancel={finishDrawing}
        />
        {textEditor && (
          <input
            ref={textInputRef}
            className="drawing-text-editor"
            value={textEditor.value}
            aria-label={t("Enter text")}
            style={{
              left: textEditor.start.x,
              top: textEditor.start.y,
              color: textEditor.color,
              fontSize: Math.max(18, textEditor.width * 4),
            }}
            onChange={(event) =>
              updateTextEditor({ ...textEditor, value: event.currentTarget.value })
            }
            onKeyDown={onTextEditorKeyDown}
            onPointerDown={(event) => event.stopPropagation()}
            onWheel={(event) => {
              event.preventDefault();
              const nextWidth = Math.min(
                45,
                Math.max(15, textEditor.width + (event.deltaY < 0 ? 1 : -1)),
              );
              setWidth(nextWidth);
              updateTextEditor({ ...textEditor, width: nextWidth });
            }}
          />
        )}
      </main>
    );
  }

  return (
    <DrawingToolbarView
      toolbarRef={toolbarRef}
      shellRef={shellRef}
      orientation={orientation}
      iconScale={TOOLBAR_ICON_SCALE[iconSize]}
      maxMain={monitorLogical ? toolbarLimit(orientation === "horizontal" ? monitorLogical.width : monitorLogical.height) : 0}
      morePanel={{ open: moreOpen, side: panelSide }}
      numberStyle={numberStyle}
      onNumberStyle={setNumberStyle}
      onToggleMore={() => void toggleMore()}
      onOverflowChange={(overflow) => { if (!overflow) setMoreOpen(false); }}
      toolbarItems={toolbarItems}
      radialStyle={radialStyle}
      tool={tool}
      width={width}
      minWidth={minWidth}
      maxWidth={maxWidth}
      canUndo={canUndo}
      canRedo={canRedo}
      closeShortcutLabel={closeShortcutLabel}
      undoShortcutLabel={undoShortcutLabel}
      selectionState={selectionState}
      operationHint={operationHint}
      probeMenu={probeMenu}
      probe={probe}
      keyDisplayEnabled={keyDisplayEnabled}
      colors={toolbarColors}
      color={selectionState.count > 0 ? selectionState.color ?? "" : color}
      zoomActive={zoomActive}
      zoomShortcutLabel={zoomShortcutLabel}
      onToggleZoom={toggleZoom}
      onSnip={startSnip}
      sendCommand={sendCommand}
      setProbeMenu={setProbeMenu}
      onDrag={() => void handleToolbarDrag()}
      onHide={() => void invoke("hide_drawing_toolbar").catch(console.error)}
      onClose={() => void invoke("close_screen_drawing")}
      onToggleKeyDisplay={() => {
        void invoke<boolean>("toggle_key_display")
          .then(setKeyDisplayEnabled)
          .catch((error) => console.error("Failed to toggle key display:", error));
      }}
      onSettings={() => void invoke("open_settings_window")}
      onSavePng={() => void saveScreen("png")}
      onSaveJson={() => void saveScreen("json")}
      onOpenJson={() => void openJson()}
      onRecover={() => {
        void invoke("drawing_recover").then(() => setOperationHint({ text: t("Display recovery requested") }))
          .catch(() => setOperationHint({ text: t("Display recovery failed") }));
      }}
    />
  );
}

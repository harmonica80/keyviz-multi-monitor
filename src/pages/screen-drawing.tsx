import { useTranslation } from "@/lib/i18n";
import { keymaps } from "@/lib/keymaps";
import { useKeyEvent } from "@/stores/key_event";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import {
  ArrowUpRight,
  Check,
  Circle,
  Eraser,
  GripHorizontal,
  Minus,
  MousePointer2,
  Pencil,
  Redo2,
  Square,
  Trash2,
  Type,
  X,
} from "lucide-react";
import {
  CSSProperties,
  KeyboardEvent as ReactKeyboardEvent,
  PointerEvent,
  useCallback,
  useEffect,
  useRef,
  useState,
} from "react";

type Point = { x: number; y: number };
type TextEditor = { start: Point; value: string; color: string; width: number };
type Tool = "pointer" | "pen" | "eraser" | "line" | "arrow" | "rectangle" | "ellipse" | "text";
type Drawing =
  | { tool: "pen" | "eraser"; points: Point[]; color: string; width: number }
  | { tool: "line" | "arrow" | "rectangle" | "ellipse"; start: Point; end: Point; color: string; width: number }
  | { tool: "text"; start: Point; text: string; color: string; width: number };
type DrawingCommand =
  | { type: "tool"; value: Tool }
  | { type: "color"; value: string }
  | { type: "width"; value: number }
  | { type: "clear" }
  | { type: "undo" };
type ToolChangedPayload = { tool: Tool };

const COLORS = ["#ef2b2d", "#16c43b", "#2d37d6", "#d1af4b", "#ffffff", "#111111"];
const WIDTHS = [2, 5, 9, 15];
const MIN_WIDTH = WIDTHS[0];
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

const drawArrowHead = (
  context: CanvasRenderingContext2D,
  start: Point,
  end: Point,
  width: number,
) => {
  const angle = Math.atan2(end.y - start.y, end.x - start.x);
  const length = Math.max(14, width * 3);
  context.beginPath();
  context.moveTo(end.x, end.y);
  context.lineTo(
    end.x - length * Math.cos(angle - Math.PI / 6),
    end.y - length * Math.sin(angle - Math.PI / 6),
  );
  context.moveTo(end.x, end.y);
  context.lineTo(
    end.x - length * Math.cos(angle + Math.PI / 6),
    end.y - length * Math.sin(angle + Math.PI / 6),
  );
  context.stroke();
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
    context.lineWidth = drawing.tool === "eraser" ? drawing.width * 3 : drawing.width;
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

  const shapeWidth = drawing.end.x - drawing.start.x;
  const shapeHeight = drawing.end.y - drawing.start.y;
  context.beginPath();
  if (drawing.tool === "line" || drawing.tool === "arrow") {
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
  if (drawing.tool === "arrow") {
    drawArrowHead(context, drawing.start, drawing.end, drawing.width);
  }
  context.restore();
};

export default function ScreenDrawing() {
  const { t } = useTranslation();
  const mode = new URLSearchParams(window.location.search).get("mode");
  const isToolbar = mode === "drawing-toolbar";
  const toolbarRef = useRef<HTMLElement>(null);
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const drawingsRef = useRef<Drawing[]>([]);
  const activeRef = useRef<Drawing | null>(null);
  const textEditorRef = useRef<TextEditor | null>(null);
  const textInputRef = useRef<HTMLInputElement>(null);
  const [tool, setTool] = useState<Tool>("pen");
  const [color, setColor] = useState(COLORS[0]);
  const [width, setWidth] = useState(WIDTHS[1]);
  const [canUndo, setCanUndo] = useState(false);
  const [textEditor, setTextEditor] = useState<TextEditor | null>(null);
  const isTextEditorOpen = textEditor !== null;
  const drawingUndoShortcut = useKeyEvent((state) => state.drawingUndoShortcut);
  const drawingCloseShortcut = useKeyEvent((state) => state.drawingCloseShortcut);
  const closeShortcutLabel = formatShortcut(drawingCloseShortcut);
  const undoShortcutLabel = formatShortcut(drawingUndoShortcut);

  const notifyHistory = useCallback(() => {}, []);

  useEffect(() => {
    const toolListener = listen<ToolChangedPayload>("drawing-tool-changed", (event) => {
      setTool(event.payload.tool);
    });
    return () => {
      void toolListener.then((unlisten) => unlisten());
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

  useEffect(() => {
    if (isToolbar) {
      let lastHeight = 0;
      const resizeToolbar = () => {
        const toolbar = toolbarRef.current;
        if (!toolbar) return;
        const height = Math.ceil(Math.max(
          toolbar.getBoundingClientRect().height,
          toolbar.scrollHeight,
          toolbar.offsetHeight,
        ));
        if (height <= 0 || height === lastHeight) return;
        lastHeight = height;
        void invoke("resize_drawing_toolbar", { height });
      };
      const observer = new ResizeObserver(resizeToolbar);
      if (toolbarRef.current) observer.observe(toolbarRef.current);
      window.requestAnimationFrame(() => window.requestAnimationFrame(resizeToolbar));
      [50, 200, 600, 1200].forEach((delay) => window.setTimeout(resizeToolbar, delay));
      const historyListener = listen<{ canUndo: boolean }>("drawing-history", (event) => {
        setCanUndo(event.payload.canUndo);
      });
      const widthListener = listen<{ width: number }>("drawing-width-changed", (event) => {
        setWidth(event.payload.width);
      });
      const nativeCloseListener = listen("native-drawing-close", () => {
        void invoke("close_screen_drawing");
      });
      return () => {
        observer.disconnect();
        void historyListener.then((unlisten) => unlisten());
        void widthListener.then((unlisten) => unlisten());
        void nativeCloseListener.then((unlisten) => unlisten());
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

  const sendCommand = async (command: DrawingCommand) => {
    if (command.type === "tool") {
      setTool(command.value);
      await invoke("drawing_set_tool", { tool: command.value });
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
    }
  };

  const onPointerDown = (event: PointerEvent<HTMLCanvasElement>) => {
    if (tool === "pointer") return;
    const start = { x: event.clientX, y: event.clientY };
    if (tool === "text") {
      finishTextEditor(true);
      updateTextEditor({ start, value: "", color, width });
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
          />
        )}
      </main>
    );
  }

  const toolButtons: Array<{ value: Tool; label: string; icon: typeof Pencil }> = [
    { value: "pointer", label: t("Pointer"), icon: MousePointer2 },
    { value: "pen", label: t("Pen"), icon: Pencil },
    { value: "eraser", label: t("Eraser"), icon: Eraser },
    { value: "line", label: t("Line"), icon: Minus },
    { value: "arrow", label: t("Arrow"), icon: ArrowUpRight },
    { value: "rectangle", label: t("Rectangle"), icon: Square },
    { value: "ellipse", label: t("Ellipse"), icon: Circle },
    { value: "text", label: t("Text"), icon: Type },
  ];

  return (
    <aside ref={toolbarRef} className="drawing-toolbar" aria-label={t("Screen Drawing")}>
      <button
        className="drawing-drag-handle"
        title={t("Move Toolbar")}
        onPointerDown={() => void invoke("start_drawing_toolbar_drag")}
      >
        <GripHorizontal />
      </button>
      <button
        className="drawing-close"
        title={`${t("Exit Drawing")}${closeShortcutLabel ? ` (${closeShortcutLabel})` : ""}`}
        onClick={() => void invoke("close_screen_drawing")}
      >
        <X />
      </button>
      <button title={t("Clear All")} onClick={() => void sendCommand({ type: "clear" })}>
        <Trash2 />
      </button>
      {toolButtons.map(({ value, label, icon: Icon }) => (
        <button
          key={value}
          className={tool === value ? "active" : ""}
          title={label}
          onClick={() => void sendCommand({ type: "tool", value })}
        >
          <Icon />
        </button>
      ))}
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
          min={MIN_WIDTH}
          max={MAX_WIDTH}
          step={1}
          value={width}
          style={{
            "--drawing-width-progress": `${((width - MIN_WIDTH) / (MAX_WIDTH - MIN_WIDTH)) * 100}%`,
          } as CSSProperties}
          onChange={(event) => void sendCommand({ type: "width", value: Number(event.target.value) })}
        />
      </label>
      <button
        disabled={!canUndo}
        title={`${t("Undo")}${undoShortcutLabel ? ` (${undoShortcutLabel})` : ""}`}
        onClick={() => void sendCommand({ type: "undo" })}
      >
        <Redo2 className="drawing-undo" />
      </button>
    </aside>
  );
}

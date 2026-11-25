import { invoke } from "@tauri-apps/api/core";
import { Copy, FileJson, Pin, Save, X } from "lucide-react";
import { PointerEvent as ReactPointerEvent, ReactNode, useCallback, useEffect, useRef, useState } from "react";
import { useTranslation } from "@/lib/i18n";

/**
 * Full-screen selection window of the screenshot tool. It shows the frozen screen, lets the
 * user drag a rectangle (move it, resize it from 8 handles) and offers: cancel, pin as a sticky
 * note, save image, save as JSON, copy image. Pixels are cropped and saved by the Rust side.
 */

type Rect = { x: number; y: number; w: number; h: number };
type Handle = "n" | "s" | "e" | "w" | "ne" | "nw" | "se" | "sw";
type Drag =
  | { kind: "create"; startX: number; startY: number }
  | { kind: "move"; startX: number; startY: number; origin: Rect }
  | { kind: "resize"; handle: Handle; startX: number; startY: number; origin: Rect };

const HANDLES: { id: Handle; fx: number; fy: number; cursor: string }[] = [
  { id: "nw", fx: 0, fy: 0, cursor: "nwse-resize" },
  { id: "n", fx: 0.5, fy: 0, cursor: "ns-resize" },
  { id: "ne", fx: 1, fy: 0, cursor: "nesw-resize" },
  { id: "e", fx: 1, fy: 0.5, cursor: "ew-resize" },
  { id: "se", fx: 1, fy: 1, cursor: "nwse-resize" },
  { id: "s", fx: 0.5, fy: 1, cursor: "ns-resize" },
  { id: "sw", fx: 0, fy: 1, cursor: "nesw-resize" },
  { id: "w", fx: 0, fy: 0.5, cursor: "ew-resize" },
];

const MIN_SIZE = 4;
const TOOLBAR_W = 5 * 36 + 4 * 2 + 8 + 1;
const TOOLBAR_H = 40;

const clamp = (value: number, min: number, max: number) => Math.min(Math.max(value, min), max);
const fileName = (path: string) => path.split(/[\\/]/).pop() ?? path;

function resized(origin: Rect, handle: Handle, dx: number, dy: number, maxW: number, maxH: number): Rect {
  let left = origin.x;
  let top = origin.y;
  let right = origin.x + origin.w;
  let bottom = origin.y + origin.h;
  if (handle.includes("w")) left = clamp(left + dx, 0, maxW);
  if (handle.includes("e")) right = clamp(right + dx, 0, maxW);
  if (handle.includes("n")) top = clamp(top + dy, 0, maxH);
  if (handle.includes("s")) bottom = clamp(bottom + dy, 0, maxH);
  return {
    x: Math.min(left, right),
    y: Math.min(top, bottom),
    w: Math.abs(right - left),
    h: Math.abs(bottom - top),
  };
}

export default function SnipPage() {
  const { t } = useTranslation();
  const [imageUrl, setImageUrl] = useState<string | null>(null);
  const [rect, setRect] = useState<Rect | null>(null);
  const [notice, setNotice] = useState<{ text: string; error?: boolean } | null>(null);
  const [busy, setBusy] = useState(false);
  const dragRef = useRef<Drag | null>(null);
  const imageRef = useRef<HTMLImageElement>(null);

  useEffect(() => {
    let url: string | null = null;
    let cancelled = false;
    invoke<ArrayBuffer>("snip_get_image")
      .then((buffer) => {
        if (cancelled) return;
        url = URL.createObjectURL(new Blob([buffer], { type: "image/png" }));
        setImageUrl(url);
      })
      .catch((error) => setNotice({ text: t(String(error)), error: true }));
    return () => {
      cancelled = true;
      if (url) URL.revokeObjectURL(url);
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const cancel = useCallback(() => {
    void invoke("snip_cancel");
  }, []);

  /** Image pixels per CSS pixel (the window shows the whole frozen screen). */
  const scale = () => {
    const natural = imageRef.current?.naturalWidth || window.innerWidth;
    return natural / window.innerWidth;
  };

  const run = useCallback(
    async (kind: "png" | "json" | "copy" | "pin") => {
      if (!rect || busy || rect.w < 1 || rect.h < 1) return;
      setBusy(true);
      const k = scale();
      try {
        const result = await invoke<{ path: string | null }>("snip_action", {
          kind,
          x: rect.x * k,
          y: rect.y * k,
          w: rect.w * k,
          h: rect.h * k,
        });
        if (kind === "png" || kind === "json") {
          setNotice({ text: t("Saved: {name}").replace("{name}", fileName(result.path ?? "")) });
          window.setTimeout(cancel, 1300);
        }
        // copy and pin close the window on the Rust side
      } catch (error) {
        console.error(error);
        setNotice({ text: t(String(error)), error: true });
        setBusy(false);
      }
    },
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [rect, busy, cancel],
  );

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        event.preventDefault();
        cancel();
      } else if (event.key.toLowerCase() === "c" && (event.ctrlKey || event.metaKey)) {
        event.preventDefault();
        void run("copy");
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [cancel, run]);

  const bounds = () => ({ w: window.innerWidth, h: window.innerHeight });

  const onPointerDown = (event: ReactPointerEvent<HTMLDivElement>) => {
    if (event.button === 2) {
      cancel();
      return;
    }
    if (event.button !== 0 || busy) return;
    const target = event.target as HTMLElement;
    if (target.closest("[data-snip-toolbar]")) return;
    const handle = target.closest<HTMLElement>("[data-handle]")?.dataset.handle as Handle | undefined;
    event.currentTarget.setPointerCapture(event.pointerId);
    const { clientX: x, clientY: y } = event;
    if (rect && handle) {
      dragRef.current = { kind: "resize", handle, startX: x, startY: y, origin: rect };
    } else if (rect && x >= rect.x && x <= rect.x + rect.w && y >= rect.y && y <= rect.y + rect.h) {
      dragRef.current = { kind: "move", startX: x, startY: y, origin: rect };
    } else {
      dragRef.current = { kind: "create", startX: x, startY: y };
      setRect({ x, y, w: 0, h: 0 });
    }
    setNotice(null);
  };

  const onPointerMove = (event: ReactPointerEvent<HTMLDivElement>) => {
    const drag = dragRef.current;
    if (!drag) return;
    const { w: maxW, h: maxH } = bounds();
    const x = clamp(event.clientX, 0, maxW);
    const y = clamp(event.clientY, 0, maxH);
    if (drag.kind === "create") {
      setRect({
        x: Math.min(drag.startX, x),
        y: Math.min(drag.startY, y),
        w: Math.abs(x - drag.startX),
        h: Math.abs(y - drag.startY),
      });
    } else if (drag.kind === "move") {
      const dx = event.clientX - drag.startX;
      const dy = event.clientY - drag.startY;
      setRect({
        ...drag.origin,
        x: clamp(drag.origin.x + dx, 0, maxW - drag.origin.w),
        y: clamp(drag.origin.y + dy, 0, maxH - drag.origin.h),
      });
    } else {
      setRect(resized(drag.origin, drag.handle, event.clientX - drag.startX, event.clientY - drag.startY, maxW, maxH));
    }
  };

  const onPointerUp = () => {
    if (!dragRef.current) return;
    dragRef.current = null;
    setRect((current) => (current && (current.w < MIN_SIZE || current.h < MIN_SIZE) ? null : current));
  };

  const ready = rect !== null && rect.w >= MIN_SIZE && rect.h >= MIN_SIZE;
  const k = imageUrl ? scale() : 1;
  const { w: viewW, h: viewH } = bounds();

  // Toolbar under the selection (right aligned); above it, or inside it, when there is no room.
  let toolbarStyle = {};
  if (rect && ready) {
    let top = rect.y + rect.h + 8;
    if (top + TOOLBAR_H > viewH) top = rect.y - TOOLBAR_H - 8;
    if (top < 0) top = Math.max(0, rect.y + rect.h - TOOLBAR_H - 8);
    const left = clamp(rect.x + rect.w - TOOLBAR_W, 4, Math.max(4, viewW - TOOLBAR_W - 4));
    toolbarStyle = { left, top };
  }
  const labelTop = rect && rect.y >= 30 ? rect.y - 28 : (rect?.y ?? 0) + 4;

  const buttons: { key: string; title: string; icon: ReactNode; onClick: () => void; accent?: boolean }[] = [
    { key: "cancel", title: t("Cancel"), icon: <X size={18} />, onClick: cancel },
    { key: "pin", title: t("Pin as sticky note"), icon: <Pin size={18} />, onClick: () => void run("pin") },
    { key: "png", title: t("Save image"), icon: <Save size={18} />, onClick: () => void run("png") },
    { key: "json", title: t("Save as JSON"), icon: <FileJson size={18} />, onClick: () => void run("json") },
    { key: "copy", title: t("Copy image"), icon: <Copy size={18} />, onClick: () => void run("copy") },
  ];

  return (
    <div
      className="snip-root"
      onPointerDown={onPointerDown}
      onPointerMove={onPointerMove}
      onPointerUp={onPointerUp}
      onPointerCancel={onPointerUp}
      onContextMenu={(event) => event.preventDefault()}
    >
      <style>{`
        html, body, #root { margin: 0; width: 100%; height: 100%; overflow: hidden; background: #000; }
        .snip-root { position: fixed; inset: 0; cursor: crosshair; user-select: none; -webkit-user-select: none; font-family: "Segoe UI", "Microsoft JhengHei", sans-serif; }
        .snip-root img { position: absolute; inset: 0; width: 100%; height: 100%; pointer-events: none; -webkit-user-drag: none; }
        .snip-dim { position: absolute; inset: 0; background: rgba(0,0,0,.45); pointer-events: none; }
        .snip-box { position: absolute; box-sizing: border-box; border: 2px solid #2b7cff; box-shadow: 0 0 0 100vmax rgba(0,0,0,.45); pointer-events: none; }
        .snip-handle { position: absolute; width: 11px; height: 11px; margin: -5.5px 0 0 -5.5px; border-radius: 50%; background: #fff; border: 2px solid #2b7cff; box-sizing: border-box; }
        .snip-size { position: absolute; padding: 3px 8px; font: 13px/18px Consolas, "Segoe UI", monospace; color: #fff; background: rgba(30,30,30,.92); border-radius: 3px; pointer-events: none; white-space: nowrap; }
        .snip-toolbar { position: absolute; display: flex; gap: 2px; padding: 3px; background: #f7f7f7; border: 1px solid #2b7cff; box-shadow: 0 2px 8px rgba(0,0,0,.35); cursor: default; }
        .snip-toolbar button { width: 34px; height: 32px; display: grid; place-items: center; background: transparent; border: 0; border-radius: 3px; color: #444; cursor: pointer; }
        .snip-toolbar button:hover { background: #dbe8ff; color: #0b57d0; }
        .snip-toolbar button:disabled { opacity: .4; cursor: default; }
        .snip-toolbar .snip-sep { width: 1px; margin: 4px 3px; background: #c9c9c9; }
        .snip-hint { position: absolute; left: 50%; top: 18px; transform: translateX(-50%); padding: 6px 14px; color: #fff; background: rgba(30,30,30,.85); border-radius: 6px; font-size: 14px; pointer-events: none; }
        .snip-notice { position: absolute; left: 50%; bottom: 28px; transform: translateX(-50%); padding: 8px 16px; color: #fff; background: rgba(20,110,50,.95); border-radius: 6px; font-size: 14px; pointer-events: none; max-width: 80vw; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
        .snip-notice.error { background: rgba(170,30,30,.95); white-space: normal; }
      `}</style>
      {imageUrl && <img ref={imageRef} src={imageUrl} alt="" draggable={false} />}
      {!rect && <div className="snip-dim" />}
      {!rect && <div className="snip-hint">{t("Drag to select an area · Esc to cancel")}</div>}
      {rect && (
        <>
          <div className="snip-box" style={{ left: rect.x - 1, top: rect.y - 1, width: rect.w + 2, height: rect.h + 2 }} />
          {rect.w > 0 && rect.h > 0 && (
            <div className="snip-size" style={{ left: rect.x, top: labelTop }}>
              {Math.round(rect.w * k)} × {Math.round(rect.h * k)} px
            </div>
          )}
        </>
      )}
      {rect && ready &&
        HANDLES.map((handle) => (
          <div
            key={handle.id}
            data-handle={handle.id}
            className="snip-handle"
            style={{ left: rect.x + rect.w * handle.fx, top: rect.y + rect.h * handle.fy, cursor: handle.cursor }}
          />
        ))}
      {rect && ready && (
        <div className="snip-toolbar" data-snip-toolbar style={toolbarStyle}>
          {buttons.map((button, index) => (
            <span key={button.key} style={{ display: "contents" }}>
              {index === 1 && <div className="snip-sep" />}
              <button title={button.title} aria-label={button.title} disabled={busy} onClick={button.onClick}>
                {button.icon}
              </button>
            </span>
          ))}
        </div>
      )}
      {notice && <div className={`snip-notice ${notice.error ? "error" : ""}`}>{notice.text}</div>}
    </div>
  );
}

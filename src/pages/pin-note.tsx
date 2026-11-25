import { invoke } from "@tauri-apps/api/core";
import { Copy, PinOff, Save } from "lucide-react";
import { PointerEvent as ReactPointerEvent, WheelEvent as ReactWheelEvent, useEffect, useState } from "react";
import { useTranslation } from "@/lib/i18n";

/**
 * A pinned screenshot (sticky note): always on top, the wheel resizes it, dragging moves it, and
 * while the pointer is over it a bar appears with: unpin, save image, copy image.
 */

const fileName = (path: string) => path.split(/[\\/]/).pop() ?? path;

export default function PinNotePage() {
  const { t } = useTranslation();
  const id = Number(new URLSearchParams(window.location.search).get("id") ?? "0");
  const [imageUrl, setImageUrl] = useState<string | null>(null);
  const [hover, setHover] = useState(false);
  const [notice, setNotice] = useState<string | null>(null);

  useEffect(() => {
    let url: string | null = null;
    let cancelled = false;
    invoke<ArrayBuffer>("snip_note_image", { id })
      .then((buffer) => {
        if (cancelled) return;
        url = URL.createObjectURL(new Blob([buffer], { type: "image/png" }));
        setImageUrl(url);
      })
      .catch(console.error);
    return () => {
      cancelled = true;
      if (url) URL.revokeObjectURL(url);
    };
  }, [id]);

  useEffect(() => {
    if (!notice) return;
    const timer = window.setTimeout(() => setNotice(null), 1600);
    return () => window.clearTimeout(timer);
  }, [notice]);

  const unpin = () => void invoke("snip_note_close", { id });

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape" || event.key === "Delete") unpin();
      else if (event.key.toLowerCase() === "c" && event.ctrlKey) void copy();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const copy = async () => {
    try {
      await invoke("snip_note_copy", { id });
      setNotice(t("Copied to the clipboard"));
    } catch (error) {
      setNotice(t(String(error)));
    }
  };

  const save = async () => {
    try {
      const path = await invoke<string>("snip_note_save", { id });
      setNotice(t("Saved: {name}").replace("{name}", fileName(path)));
    } catch (error) {
      setNotice(t(String(error)));
    }
  };

  const onPointerDown = (event: ReactPointerEvent<HTMLDivElement>) => {
    if (event.button !== 0 || (event.target as HTMLElement).closest("[data-note-bar]")) return;
    void invoke("snip_note_drag", { id });
  };

  const onWheel = (event: ReactWheelEvent<HTMLDivElement>) => {
    const steps = -Math.sign(event.deltaY) * Math.min(1, Math.abs(event.deltaY) / 100);
    if (steps === 0) return;
    void invoke("snip_note_zoom", { id, steps, x: event.clientX, y: event.clientY });
  };

  const stop = (event: ReactPointerEvent) => event.stopPropagation();

  return (
    <div
      className="note-root"
      onPointerDown={onPointerDown}
      onWheel={onWheel}
      onPointerEnter={() => setHover(true)}
      onPointerLeave={() => setHover(false)}
      onContextMenu={(event) => event.preventDefault()}
      title={t("Wheel: resize · Drag: move")}
    >
      <style>{`
        html, body, #root { margin: 0; width: 100%; height: 100%; overflow: hidden; background: #fff; }
        .note-root { position: fixed; inset: 0; overflow: hidden; user-select: none; -webkit-user-select: none; box-sizing: border-box; outline: 1px solid rgba(43,124,255,.8); outline-offset: -1px; font-family: "Segoe UI", "Microsoft JhengHei", sans-serif; }
        .note-root img { position: absolute; inset: 0; width: 100%; height: 100%; pointer-events: none; -webkit-user-drag: none; }
        .note-bar { position: absolute; top: 4px; left: 50%; transform: translateX(-50%); display: flex; gap: 2px; padding: 3px; background: rgba(30,30,30,.88); border-radius: 6px; box-shadow: 0 1px 6px rgba(0,0,0,.4); }
        .note-bar button { width: 30px; height: 28px; display: grid; place-items: center; background: transparent; border: 0; border-radius: 4px; color: #f2f2f2; cursor: pointer; }
        .note-bar button:hover { background: rgba(255,255,255,.22); }
        .note-notice { position: absolute; left: 4px; right: 4px; bottom: 4px; padding: 3px 8px; font-size: 12px; color: #fff; background: rgba(20,110,50,.92); border-radius: 4px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; text-align: center; pointer-events: none; }
      `}</style>
      {imageUrl && <img src={imageUrl} alt="" draggable={false} />}
      {(hover || notice) && (
        <div className="note-bar" data-note-bar onPointerDown={stop} style={{ visibility: hover ? "visible" : "hidden" }}>
          <button title={t("Unpin")} aria-label={t("Unpin")} onClick={unpin}>
            <PinOff size={17} />
          </button>
          <button title={t("Save image")} aria-label={t("Save image")} onClick={() => void save()}>
            <Save size={17} />
          </button>
          <button title={t("Copy image")} aria-label={t("Copy image")} onClick={() => void copy()}>
            <Copy size={17} />
          </button>
        </div>
      )}
      {notice && <div className="note-notice">{notice}</div>}
    </div>
  );
}

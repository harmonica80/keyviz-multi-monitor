import { invoke } from "@tauri-apps/api/core";
import { useState } from "react";

export type ProbeResult = { before: string; after: string | null; error: string | null };

export function useOverlayProbe() {
  const [busy, setBusy] = useState(false);
  const [result, setResult] = useState<ProbeResult | null>(null);
  const run = async (layer: "display" | "input") => {
    if (busy) return;
    setBusy(true);
    setResult(null);
    try {
      setResult(await invoke<ProbeResult>("drawing_probe_layer", { layer }));
    } catch (error) {
      setResult({ before: "", after: null, error: String(error) });
    } finally {
      setBusy(false);
    }
  };
  return { busy, result, run };
}

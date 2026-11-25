/**
 * Pure layout helpers for the drawing toolbar (kept free of React/Tauri so they can be tested).
 *
 * The toolbar is a row of "units" (single buttons, small button groups, dividers...) along its
 * main axis (vertical toolbar: height, horizontal toolbar: width). When the units do not fit in
 * `limit` pixels, the last ones move into the "…" panel.
 */

export interface FitOptions {
  /** Padding of the toolbar along the main axis. */
  padding: number;
  /** Gap between neighbouring units. */
  gap: number;
  /** Longest allowed toolbar (0 or less = unlimited). */
  limit: number;
  /** Main-axis size of the "…" button. */
  moreSize: number;
}

/** Share of the screen the toolbar may take along its main axis. */
export const TOOLBAR_MAX_SHARE = 0.75;

export function toolbarLimit(monitorMainSize: number): number {
  return Math.floor(Math.max(0, monitorMainSize) * TOOLBAR_MAX_SHARE);
}

/** Total main-axis length of all units laid out in one line. */
export function totalLength(sizes: number[], padding: number, gap: number): number {
  if (sizes.length === 0) return padding;
  return padding + sizes.reduce((sum, size) => sum + size, 0) + gap * (sizes.length - 1);
}

/**
 * How many leading units stay on the toolbar. Everything fits -> `sizes.length`; otherwise as
 * many as fit next to the "…" button, never ending on a divider and never fewer than one.
 */
export function visibleUnitCount(sizes: number[], dividers: boolean[], options: FitOptions): number {
  const { padding, gap, limit, moreSize } = options;
  if (!(limit > 0) || totalLength(sizes, padding, gap) <= limit) return sizes.length;
  let used = padding + moreSize;
  let count = 0;
  for (const size of sizes) {
    const next = used + gap + size;
    if (next > limit) break;
    used = next;
    count += 1;
  }
  while (count > 1 && dividers[count - 1]) count -= 1;
  return Math.max(1, count);
}

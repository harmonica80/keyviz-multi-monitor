import {
  ArrowUpRight,
  Circle,
  Eraser,
  Minus,
  MousePointer2,
  Pencil,
  Square,
  SquareDashedMousePointer,
  Type,
} from "lucide-react";
import type { ComponentType } from "react";
import { ZoomIn } from "lucide-react";

export type DrawingTool =
  | "pointer"
  | "select"
  | "pen"
  | "eraser"
  | "line"
  | "arrow"
  | "rectangle"
  | "ellipse"
  | "text"
  | "number"
  | "check-mark"
  | "cross-mark"
  | "mosaic";

export interface DrawingToolDefinition {
  id: DrawingTool;
  label: string;
  icon: ComponentType;
  canHide: boolean;
}

export const NumberMarkerIcon = () => (
  <svg viewBox="0 0 24 24" aria-hidden="true">
    <circle cx="12" cy="12" r="8.5" fill="none" stroke="currentColor" strokeWidth="1.8" />
    <text
      x="12"
      y="12.5"
      fill="currentColor"
      fontSize="10"
      textAnchor="middle"
      dominantBaseline="middle"
    >
      1
    </text>
  </svg>
);

const SymbolToolIcon = ({ symbol }: { symbol: string }) => (
  <svg viewBox="0 0 24 24" aria-hidden="true">
    <text
      x="12"
      y="12.5"
      fill="currentColor"
      fontSize="19"
      fontWeight="600"
      textAnchor="middle"
      dominantBaseline="middle"
    >
      {symbol}
    </text>
  </svg>
);

/** Mosaic: a checker of filled blocks. */
export const MosaicIcon = () => (
  <svg viewBox="0 0 24 24" aria-hidden="true" fill="currentColor">
    <rect x="3" y="3" width="6" height="6" />
    <rect x="15" y="3" width="6" height="6" />
    <rect x="9" y="9" width="6" height="6" />
    <rect x="3" y="15" width="6" height="6" />
    <rect x="15" y="15" width="6" height="6" />
  </svg>
);

export const CheckMarkIcon = () => <SymbolToolIcon symbol="✓" />;
export const CrossMarkIcon = () => <SymbolToolIcon symbol="✕" />;

export const DRAWING_TOOLS: DrawingToolDefinition[] = [
  { id: "pointer", label: "Pointer", icon: MousePointer2, canHide: false },
  { id: "pen", label: "Pen", icon: Pencil, canHide: true },
  { id: "arrow", label: "Arrow", icon: ArrowUpRight, canHide: true },
  { id: "number", label: "Number Marker", icon: NumberMarkerIcon, canHide: true },
  { id: "check-mark", label: "Check Mark", icon: CheckMarkIcon, canHide: true },
  { id: "cross-mark", label: "Cross Mark", icon: CrossMarkIcon, canHide: true },
  { id: "rectangle", label: "Rectangle", icon: Square, canHide: true },
  { id: "ellipse", label: "Ellipse", icon: Circle, canHide: true },
  { id: "eraser", label: "Eraser", icon: Eraser, canHide: true },
  { id: "select", label: "Select Objects", icon: SquareDashedMousePointer, canHide: true },
  { id: "mosaic", label: "Mosaic", icon: MosaicIcon, canHide: true },
  { id: "text", label: "Text", icon: Type, canHide: true },
  { id: "line", label: "Line", icon: Minus, canHide: true },
];

export const DRAWING_TOOL_BY_ID = Object.fromEntries(
  DRAWING_TOOLS.map((tool) => [tool.id, tool]),
) as Record<DrawingTool, DrawingToolDefinition>;

/** Scissors with a pin: "cut out part of the screen and pin it as a sticky note". */
export function SnipPinIcon() {
  return (
    <svg
      viewBox="0 0 24 24"
      width="1em"
      height="1em"
      fill="none"
      stroke="currentColor"
      strokeWidth="2"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
    >
      <g transform="translate(0 5) scale(0.72)">
        <circle cx="6" cy="6" r="3" />
        <path d="M8.12 8.12 12 12" />
        <path d="M20 4 8.12 15.88" />
        <circle cx="6" cy="18" r="3" />
        <path d="M14.8 14.8 20 20" />
      </g>
      <g transform="translate(10.5 -1) scale(0.58)">
        <path d="M12 17v5" />
        <path d="M9 10.76a2 2 0 0 1-1.11 1.79l-1.78.9A2 2 0 0 0 5 15.24V16a1 1 0 0 0 1 1h12a1 1 0 0 0 1-1v-.76a2 2 0 0 0-1.11-1.79l-1.78-.9A2 2 0 0 1 15 10.76V7a1 1 0 0 1 1-1 2 2 0 0 0 0-4H8a2 2 0 0 0 0 4 1 1 0 0 1 1 1z" />
      </g>
    </svg>
  );
}

/** Toolbar rows: every drawing tool plus the magnifier and screenshot buttons (not drawing tools). */
export type ToolbarItemId = DrawingTool | "zoom" | "snip";

export interface ToolbarItemDefinition {
  id: ToolbarItemId;
  label: string;
  icon: ComponentType;
  canHide: boolean;
}

export const TOOLBAR_ITEM_BY_ID = {
  ...DRAWING_TOOL_BY_ID,
  zoom: { id: "zoom", label: "Screen magnifier", icon: ZoomIn, canHide: true },
  snip: { id: "snip", label: "Screenshot and pin", icon: SnipPinIcon, canHide: true },
} as Record<ToolbarItemId, ToolbarItemDefinition>;

export const TOOLBAR_ITEM_IDS: ToolbarItemId[] = [...DRAWING_TOOLS.map(({ id }) => id), "zoom", "snip"];

/** Toolbar items that are drawing tools (for the circular menu and tool switching). */
export const isDrawingToolItem = (id: ToolbarItemId): id is DrawingTool => id !== "zoom" && id !== "snip";

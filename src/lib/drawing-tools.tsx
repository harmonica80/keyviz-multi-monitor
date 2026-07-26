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
  | "number";

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

export const DRAWING_TOOLS: DrawingToolDefinition[] = [
  { id: "pointer", label: "Pointer", icon: MousePointer2, canHide: false },
  { id: "pen", label: "Pen", icon: Pencil, canHide: true },
  { id: "arrow", label: "Arrow", icon: ArrowUpRight, canHide: true },
  { id: "number", label: "Number Marker", icon: NumberMarkerIcon, canHide: true },
  { id: "rectangle", label: "Rectangle", icon: Square, canHide: true },
  { id: "ellipse", label: "Ellipse", icon: Circle, canHide: true },
  { id: "eraser", label: "Eraser", icon: Eraser, canHide: true },
  { id: "select", label: "Select Objects", icon: SquareDashedMousePointer, canHide: true },
  { id: "text", label: "Text", icon: Type, canHide: true },
  { id: "line", label: "Line", icon: Minus, canHide: true },
];

export const DRAWING_TOOL_BY_ID = Object.fromEntries(
  DRAWING_TOOLS.map((tool) => [tool.id, tool]),
) as Record<DrawingTool, DrawingToolDefinition>;

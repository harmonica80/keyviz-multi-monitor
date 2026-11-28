import { useRef, useState } from "react";
import { useTranslation } from "@/lib/i18n";
import { Check, Palette } from "lucide-react";

import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover";

/** Twelve ready-made colors offered before the full color picker. */
export const PRESET_COLORS = [
  "#ef2b2d", "#f97316", "#facc15", "#16c43b", "#14b8a6", "#0ea5e9",
  "#2d37d6", "#8b5cf6", "#ec4899", "#92400e", "#ffffff", "#111111",
];

interface PresetColorPickerProps {
  value: string;
  onChange: (color: string) => void;
  label: string;
}

/** Swatch button: opens the 12 preset colors; "Custom…" opens the system color picker. */
export const PresetColorPicker = ({ value, onChange, label }: PresetColorPickerProps) => {
  const { t } = useTranslation();
  const [open, setOpen] = useState(false);
  const customInput = useRef<HTMLInputElement>(null);
  const current = value.toLowerCase();

  return (
    <span className="relative inline-flex">
      <Popover open={open} onOpenChange={setOpen}>
        <PopoverTrigger asChild>
          <button
            type="button"
            aria-label={label}
            title={label}
            className="h-3.5 w-6 cursor-pointer rounded border"
            style={{ backgroundColor: value }}
          />
        </PopoverTrigger>
        <PopoverContent align="end" className="w-auto">
          <div className="grid grid-cols-6 gap-1.5">
            {PRESET_COLORS.map((color) => (
              <button
                key={color}
                type="button"
                aria-label={color}
                title={color}
                className="relative flex size-7 items-center justify-center rounded-md border"
                style={{ backgroundColor: color }}
                onClick={() => {
                  onChange(color);
                  setOpen(false);
                }}
              >
                {current === color && (
                  <Check className={`size-4 ${color === "#ffffff" || color === "#facc15" ? "text-black" : "text-white"}`} />
                )}
              </button>
            ))}
          </div>
          <button
            type="button"
            className="flex items-center justify-center gap-2 rounded-md border px-2 py-1.5 text-sm hover:bg-muted"
            onClick={() => {
              customInput.current?.click();
              setOpen(false);
            }}
          >
            <Palette className="size-4" />
            {t("Custom…")}
          </button>
        </PopoverContent>
      </Popover>
      <input
        ref={customInput}
        type="color"
        value={value}
        onChange={(event) => onChange(event.currentTarget.value)}
        aria-hidden
        tabIndex={-1}
        className="pointer-events-none absolute bottom-0 right-0 size-0 opacity-0"
      />
    </span>
  );
};

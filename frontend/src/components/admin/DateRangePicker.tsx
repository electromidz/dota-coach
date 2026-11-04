"use client";

import { Segmented } from "@/components/ui/Segmented";
import { RANGE_PRESETS, isoDay, presetRange, type DateRange, type RangePreset } from "@/lib/admin/range";

const INPUT =
  "min-h-9 rounded-lg border border-glass-edge bg-surface-2/60 px-2 text-ink outline-none focus-neon";

/**
 * Presets plus a custom From/To. The inputs always show the range in force —
 * a preset fills them in — so what is being measured is never a guess.
 * Editing either input turns the range into a custom one.
 */
export function DateRangePicker({
  value,
  onChange,
}: {
  value: DateRange;
  onChange: (next: DateRange) => void;
}) {
  const today = isoDay(new Date());

  return (
    <div className="flex flex-wrap items-center gap-3 text-xs text-ink-faint">
      <Segmented<RangePreset | "custom">
        label="Date range preset"
        value={value.preset}
        options={[
          ...RANGE_PRESETS.map((p) => ({ value: p.value, label: p.label })),
          ...(value.preset === "custom" ? [{ value: "custom" as const, label: "Custom" }] : []),
        ]}
        onChange={(preset) => {
          if (preset !== "custom") onChange(presetRange(preset));
        }}
      />

      <fieldset className="flex flex-wrap items-center gap-2">
        <legend className="sr-only">Custom date range</legend>
        <label className="flex items-center gap-1.5">
          From
          <input
            type="date"
            value={value.from}
            max={value.to}
            onChange={(e) =>
              e.target.value && onChange({ preset: "custom", from: e.target.value, to: value.to })
            }
            className={INPUT}
          />
        </label>
        <label className="flex items-center gap-1.5">
          To
          <input
            type="date"
            value={value.to}
            min={value.from}
            max={today}
            onChange={(e) =>
              e.target.value && onChange({ preset: "custom", from: value.from, to: e.target.value })
            }
            className={INPUT}
          />
        </label>
      </fieldset>
    </div>
  );
}

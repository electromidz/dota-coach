import { cn } from "@/lib/utils";

/** A compact pill group for two to four mutually exclusive choices. */
export function Segmented<T extends string>({
  label,
  options,
  value,
  onChange,
}: {
  label: string;
  options: { value: T; label: string }[];
  value: T;
  onChange: (value: T) => void;
}) {
  return (
    <div
      role="radiogroup"
      aria-label={label}
      className="inline-flex rounded-xl border border-glass-edge bg-surface-2/60 p-1"
    >
      {options.map((option) => {
        const active = option.value === value;
        return (
          <button
            key={option.value}
            type="button"
            role="radio"
            aria-checked={active}
            onClick={() => onChange(option.value)}
            className={cn(
              "focus-neon min-h-9 cursor-pointer whitespace-nowrap rounded-lg px-3 text-xs transition-colors duration-200",
              active
                ? "bg-ink/10 text-ink"
                : "text-ink-faint hover:text-ink",
            )}
          >
            {option.label}
          </button>
        );
      })}
    </div>
  );
}

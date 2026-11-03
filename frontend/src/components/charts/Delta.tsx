import { cn } from "@/lib/utils";

/**
 * ▲ +6 / ▼ −12, coloured by whether the move is good.
 *
 * `lowerIsBetter` flips the colour, never the arrow: fewer deaths still points
 * down, it is just green.
 */
export function Delta({
  value,
  format = (v) => Math.round(v).toString(),
  suffix,
  lowerIsBetter = false,
  className,
}: {
  value: number;
  format?: (absolute: number) => string;
  /** Trailing context, e.g. "vs Legend". Rendered in muted ink. */
  suffix?: string;
  lowerIsBetter?: boolean;
  className?: string;
}) {
  const flat = Math.abs(value) < 1e-9;
  const up = value > 0;
  const good = flat ? null : up !== lowerIsBetter;

  return (
    <span
      className={cn(
        "inline-flex items-baseline gap-1 font-mono text-xs tabular-nums",
        className,
      )}
    >
      <span
        className={cn(
          good === null ? "text-ink-faint" : good ? "text-string" : "text-error",
        )}
      >
        <span aria-hidden>{flat ? "●" : up ? "▲" : "▼"}</span>{" "}
        {flat ? "" : up ? "+" : "−"}
        {format(Math.abs(value))}
      </span>
      {suffix ? (
        <span className="font-sans text-ink-faint">{suffix}</span>
      ) : null}
    </span>
  );
}

import { Icon, type IconName } from "@/components/ui/Icon";
import { cn } from "@/lib/utils";

/**
 * One headline number. The right form for a single value — a one-bar bar chart
 * would say the same thing with more ink.
 *
 * The value wears a text token, never a series colour; the small icon beside
 * it carries the identity.
 */
export function StatTile({
  label,
  value,
  icon,
  tone = "number",
  className,
}: {
  label: string;
  value: string;
  icon: IconName;
  tone?: "number" | "string" | "function" | "error";
  className?: string;
}) {
  const toneClass = {
    number: "text-number",
    string: "text-string",
    function: "text-function",
    error: "text-error",
  }[tone];

  return (
    <div
      className={cn(
        "glass soft-raised flex flex-col gap-1 rounded-card p-3.5",
        className,
      )}
    >
      <span className="flex items-center gap-1.5 text-[0.6875rem] uppercase tracking-wider text-ink-faint">
        <Icon name={icon} className={cn("size-3.5", toneClass)} />
        {label}
      </span>
      <span className="font-mono text-2xl tabular-nums text-ink">{value}</span>
    </div>
  );
}

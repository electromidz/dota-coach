import { Icon, type IconName } from "@/components/ui/Icon";
import type { Tone } from "@/lib/admin/format";
import { cn } from "@/lib/utils";

const TONES: Record<Tone, string> = {
  neutral: "border-glass-edge text-ink-faint",
  info: "border-function/40 text-function",
  good: "border-string/40 text-string",
  warn: "border-warn/40 text-warn",
  bad: "border-error/40 text-error",
  accent: "border-keyword/40 text-keyword",
};

/**
 * A status as a word in a pill. The word carries the meaning; the tone only
 * reinforces it, so it reads the same without colour.
 *
 * `label` names *what* the status is about ("Account", "Subscription") for
 * screen readers and as a hover title, so two badges side by side are never
 * ambiguous.
 */
export function StatusBadge({
  children,
  tone = "neutral",
  label,
  icon,
  className,
}: {
  children: React.ReactNode;
  tone?: Tone;
  label?: string;
  icon?: IconName;
  className?: string;
}) {
  return (
    <span
      title={label}
      className={cn(
        "inline-flex items-center gap-1 whitespace-nowrap rounded-lg border px-2 py-0.5 text-xs",
        TONES[tone],
        className,
      )}
    >
      {icon ? <Icon name={icon} className="size-3.5" /> : null}
      {label ? <span className="sr-only">{label}: </span> : null}
      {children}
    </span>
  );
}

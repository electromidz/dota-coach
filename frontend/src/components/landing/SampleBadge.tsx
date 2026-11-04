import { cn } from "@/lib/utils";

/** Marks a panel's numbers as illustrative, not anyone's real account. */
export function SampleBadge({ className }: { className?: string }) {
  return (
    <span
      className={cn(
        "inline-flex shrink-0 items-center rounded-full border border-glass-edge bg-surface-2/70 px-2 py-0.5 text-[0.625rem] font-medium uppercase tracking-wider text-ink-faint",
        className,
      )}
    >
      Sample data
    </span>
  );
}

/** Marks a feature that needs the subscription. */
export function ProBadge({ className }: { className?: string }) {
  return (
    <span
      className={cn(
        "inline-flex shrink-0 items-center rounded-full bg-keyword px-2 py-0.5 text-[0.625rem] font-semibold uppercase tracking-wider text-base",
        className,
      )}
    >
      Pro
    </span>
  );
}

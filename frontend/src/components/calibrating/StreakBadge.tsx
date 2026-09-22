import { Icon } from "@/components/ui/Icon";
import type { Streak } from "@/lib/types";
import { cn } from "@/lib/utils";

/**
 * The current unbroken run of ranked results.
 *
 * Win and loss are *status*, not a category pair, so the badge carries a word
 * and an icon as well as a colour — the same rule `FormStrip` follows. A
 * streak with no direction (`kind: null`) means there is nothing to report,
 * and says so rather than rendering "Win 0".
 */
export function StreakBadge({
  streak,
  className,
}: {
  streak: Streak;
  className?: string;
}) {
  if (streak.kind === null || streak.count === 0) {
    return (
      <p className={cn("text-sm text-ink-faint", className)}>
        No recent ranked matches.
      </p>
    );
  }

  const won = streak.kind === "win";

  return (
    <div className={cn("flex items-center gap-3", className)}>
      <span
        className={cn(
          "flex size-11 shrink-0 items-center justify-center rounded-xl font-display text-xl tabular-nums",
          // 15% keeps the numeral well clear of 4.5:1 against its own tinted
          // tile, matching the ratios `FormStrip` measured.
          won ? "bg-mark-win/15 text-string" : "bg-mark-loss/15 text-error",
        )}
      >
        {streak.count}
      </span>

      <span className="flex flex-col">
        <span
          className={cn(
            "flex items-center gap-1.5 text-sm font-semibold",
            won ? "text-string" : "text-error",
          )}
        >
          <Icon name={won ? "trophy" : "skull"} className="size-4" />
          {won ? "Win streak" : "Loss streak"}
        </span>
        <span className="text-xs text-ink-faint">
          Ranked matches, most recent first
        </span>
      </span>
    </div>
  );
}

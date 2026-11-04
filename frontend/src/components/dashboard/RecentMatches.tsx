import Link from "next/link";

import { ChartCard } from "@/components/ui/ChartCard";
import { HeroPortrait } from "@/components/ui/HeroPortrait";
import { RelativeTime } from "@/components/ui/RelativeTime";
import type { MatchView } from "@/lib/types";
import { cn, formatDuration } from "@/lib/utils";

/** The five newest games as compact rows, each opening its match page. */
export function RecentMatches({
  matches,
  title = "Recent matches",
  seeAll = "See all →",
}: {
  matches: MatchView[];
  title?: string;
  seeAll?: string;
}) {
  return (
    <ChartCard
      title={title}
      action={
        <Link href="/matches" className="focus-neon rounded text-xs text-function hover:text-ink">
          {seeAll}
        </Link>
      }
    >
      <ul className="-my-1 flex flex-col divide-y divide-glass-edge">
        {matches.slice(0, 5).map((m) => (
          <li key={m.id}>
            <Link
              href={`/matches/${m.id}`}
              className="focus-neon flex items-center gap-3 rounded py-2 transition-colors hover:bg-surface-2/40"
            >
              <HeroPortrait heroId={m.hero_id} heroName={m.hero_name} size="sm" className="shrink-0" />
              <span className="flex min-w-0 flex-1 flex-col">
                <span className="truncate text-sm text-ink">{m.hero_name}</span>
                <span className="truncate text-[0.6875rem] text-ink-faint">
                  {m.position ? `${m.position_label} · ` : ""}
                  {formatDuration(m.duration_seconds)} · <RelativeTime iso={m.started_at} />
                </span>
              </span>
              <span className="flex shrink-0 flex-col items-end">
                <span className="font-mono text-sm tabular-nums text-number">
                  {m.kills}/{m.deaths}/{m.assists}
                </span>
                <span className="font-mono text-[0.6875rem] tabular-nums text-ink-faint">
                  {m.gpm.toLocaleString("en-US")} GPM
                </span>
              </span>
              <span
                className={cn(
                  "w-9 shrink-0 rounded-md py-0.5 text-center text-[0.6875rem] font-semibold",
                  m.won ? "bg-mark-win/15 text-string" : "bg-mark-loss/15 text-error",
                )}
              >
                {m.won ? "Won" : "Lost"}
              </span>
            </Link>
          </li>
        ))}
      </ul>
    </ChartCard>
  );
}

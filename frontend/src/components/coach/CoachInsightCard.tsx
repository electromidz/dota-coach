import { PeerBullet } from "@/components/coach/PeerBullet";
import { InfoTip } from "@/components/ui/InfoTip";
import { citedBy, firstSentence, hasPeerFigure, heroOf, INSIGHT_CLASS } from "@/lib/coach";
import type { Evidence, Insight } from "@/lib/types";
import { cn, fixOrdinals } from "@/lib/utils";

/**
 * A strength or weakness: title, one sentence, and the evidence as charts.
 *
 * Cited benchmark figures become bullets (you vs median vs top 20%). Anything
 * else cited is a small chip whose sentence sits in a tooltip.
 */
export function CoachInsightCard({
  insight,
  evidence,
}: {
  insight: Insight;
  evidence: Evidence[];
}) {
  const cited = citedBy(insight, evidence);
  const peers = cited.filter(hasPeerFigure);
  const others = cited.filter((item) => !hasPeerFigure(item));
  const text = fixOrdinals(
    insight.explanation || insight.why_it_matters || insight.what_happened || "",
  );
  const sentence = firstSentence(text);

  return (
    <article className="glass flex min-w-0 flex-col gap-3 rounded-card p-4">
      <header className="flex items-start gap-2">
        <span
          className={cn(
            "shrink-0 rounded-full border px-2 py-0.5 text-[0.625rem] uppercase tracking-wider",
            INSIGHT_CLASS[insight.kind],
          )}
        >
          {insight.kind_label}
        </span>
        <h3 className="min-w-0 flex-1 font-display text-sm leading-snug text-ink">
          {fixOrdinals(insight.title)}
        </h3>
        {text.length > sentence.length ? (
          <InfoTip label={`More about ${insight.title}`} align="end">
            {text}
          </InfoTip>
        ) : null}
      </header>

      {sentence ? (
        <p className="line-clamp-2 text-xs leading-relaxed text-ink-muted">{sentence}</p>
      ) : null}

      {peers.length > 0 ? (
        <ul
          className="flex flex-col gap-2.5"
          aria-label={heroOf(peers[0]) ? `On ${heroOf(peers[0])}` : undefined}
        >
          {heroOf(peers[0]) ? (
            <li className="text-[0.625rem] uppercase tracking-wider text-ink-faint">
              On {heroOf(peers[0])} · vs peers
            </li>
          ) : null}
          {peers.map((item) => (
            <PeerBullet key={item.id} item={item} />
          ))}
        </ul>
      ) : null}

      {others.length > 0 ? (
        <ul className="flex flex-wrap gap-1.5">
          {others.map((item) => (
            <li
              key={item.id}
              className="flex items-center gap-1 rounded-md border border-glass-edge px-2 py-0.5 text-[0.6875rem] text-ink-muted"
            >
              {item.label}
              <InfoTip label={item.label} align="end">
                {fixOrdinals(item.statement)}
              </InfoTip>
            </li>
          ))}
        </ul>
      ) : null}
    </article>
  );
}

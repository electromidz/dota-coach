"use client";

import { formatMetricValue } from "@/components/charts/BulletRow";
import { formatValue } from "@/components/charts/SeriesChart";
import { InfoTip } from "@/components/ui/InfoTip";
import { SampleBadge } from "@/components/ui/SampleBadge";
import { sharedSample } from "@/components/coach/EvidenceList";
import { groupEvidence } from "@/lib/coach";
import type { Evidence } from "@/lib/types";
import { fixOrdinals, ordinal } from "@/lib/utils";

/**
 * The data behind the analysis, collapsed by default.
 *
 * Measured figures are a table (metric · you · median · top 20% · pct ·
 * sample). Everything else is a row whose sentence sits behind an (i).
 */
export function MeasuredData({ evidence }: { evidence: Evidence[] }) {
  const groups = groupEvidence(evidence);
  if (groups.length === 0) return null;

  return (
    <details className="glass group rounded-card p-4 [&_summary::-webkit-details-marker]:hidden">
      <summary className="focus-neon flex cursor-pointer list-none items-center justify-between gap-3 rounded text-sm text-ink">
        <span className="font-display text-sm uppercase tracking-[0.2em] text-ink-faint">
          What the coach measured
        </span>
        <span className="text-xs text-function group-open:hidden">Show the data behind this</span>
        <span className="hidden text-xs text-function group-open:inline">Hide</span>
      </summary>

      <div className="mt-4 flex flex-col gap-5">
        {groups.map((group) => {
          const shared = sharedSample(group.kind, group.items);
          const measured = group.items.filter((item) => item.figure);
          const facts = group.items.filter((item) => !item.figure);
          return (
            <div key={group.label} className="flex flex-col gap-1.5">
              <h3 className="text-[0.6875rem] font-semibold uppercase tracking-[0.16em] text-ink-faint">
                {group.label}
              </h3>
              {measured.length > 0 ? (
                <div className="overflow-x-auto sm:overflow-visible">
                  <table className="w-full min-w-[30rem] text-xs sm:min-w-0">
                    <thead className="text-left text-[0.625rem] uppercase tracking-wider text-ink-faint">
                      <tr>
                        <th scope="col" className="py-1 pr-2 font-normal">Metric</th>
                        <th scope="col" className="px-2 py-1 text-right font-normal">
                          {group.kind === "progress" ? "Now" : "You"}
                        </th>
                        <th scope="col" className="px-2 py-1 text-right font-normal">
                          {group.kind === "progress" ? "Before" : "Median"}
                        </th>
                        <th scope="col" className="px-2 py-1 text-right font-normal">
                          {group.kind === "progress" ? "Change" : "Top 20%"}
                        </th>
                        <th scope="col" className="px-2 py-1 text-right font-normal">Pct</th>
                        <th scope="col" className="py-1 pl-2 text-right font-normal">
                          <span className="sr-only">Sample</span>
                        </th>
                      </tr>
                    </thead>
                    <tbody className="font-mono tabular-nums">
                      {measured.map((item) => (
                        <Row key={item.id} item={item} shared={shared} />
                      ))}
                    </tbody>
                  </table>
                </div>
              ) : null}
              {facts.length > 0 ? (
                <ul className="flex flex-wrap gap-1.5">
                  {facts.map((item) => (
                    <li
                      key={item.id}
                      className="flex items-center gap-1 rounded-md border border-glass-edge px-2 py-0.5 text-[0.6875rem] text-ink-muted"
                    >
                      {item.label.replace(/ since your last session$/i, "")}
                      <InfoTip label={item.label}>{fixOrdinals(item.statement)}</InfoTip>
                      {item.id.startsWith("kit.") ? null : (
                        <SampleBadge
                          sample={shared?.sample ?? item.sample}
                          confidence={shared?.confidence ?? item.confidence}
                        />
                      )}
                    </li>
                  ))}
                </ul>
              ) : null}
            </div>
          );
        })}
      </div>
    </details>
  );
}

function Row({
  item,
  shared,
}: {
  item: Evidence;
  /** One count for a section whose rows share their matches. */
  shared: { sample: number; confidence: Evidence["confidence"] } | null;
}) {
  const f = item.figure;
  const fmt = (v: number) => (f?.unit ? formatValue(v, f.unit) : formatMetricValue(v));
  const dash = <span className="text-ink-faint">—</span>;
  // A hero kit is a fact about the hero, not a sample of games.
  const isFact = item.id.startsWith("kit.");

  return (
    <tr className="border-t border-border/50">
      <th scope="row" className="py-1.5 pr-2 text-left font-sans font-normal text-ink-muted">
        <span className="inline-flex items-center gap-1.5">
          <span className="truncate">{item.label.replace(/ vs peers$| since your last session$/i, "")}</span>
          <InfoTip label={item.label}>{fixOrdinals(item.statement)}</InfoTip>
        </span>
      </th>
      <td className="px-2 py-1.5 text-right text-ink">{f ? fmt(f.value) : dash}</td>
      <td className="px-2 py-1.5 text-right text-ink-muted">
        {f?.previous != null ? fmt(f.previous) : f?.median != null ? fmt(f.median) : dash}
      </td>
      <td className="px-2 py-1.5 text-right text-ink-muted">
        {f?.previous != null
          ? f.percent_change != null
            ? `${f.percent_change >= 0 ? "+" : "−"}${Math.abs(Math.round(f.percent_change * 100))}%`
            : `${f.value - f.previous >= 0 ? "+" : "−"}${
                f.unit === "percentile" || f.unit === "score"
                  ? `${Math.round(Math.abs(f.value - f.previous))} pts`
                  : fmt(Math.abs(f.value - f.previous))
              }`
          : f?.top_20 != null
            ? fmt(f.top_20)
            : dash}
      </td>
      <td className="px-2 py-1.5 text-right text-ink">
        {f?.percentile != null ? ordinal(f.percentile) : dash}
      </td>
      <td className="py-1.5 pl-2 text-right">
        {isFact ? null : (
          <SampleBadge
            sample={shared?.sample ?? item.sample}
            confidence={shared?.confidence ?? item.confidence}
          />
        )}
      </td>
    </tr>
  );
}

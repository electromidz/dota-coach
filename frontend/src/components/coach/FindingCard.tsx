import { formatMetricValue } from "@/components/charts/BulletRow";
import { Card } from "@/components/ui/Card";
import { citedBy, INSIGHT_CLASS } from "@/lib/coach";
import type { Evidence, EvidenceFigure, Insight } from "@/lib/types";
import { cn, fixOrdinals, formatDuration, ordinal } from "@/lib/utils";

const SEVERITY_CLASS = {
  major: "border-error/50 bg-error/10 text-error",
  minor: "border-glass-edge bg-surface-2 text-ink-muted",
} as const;

/** A figure's value in its own terms: a clock for item timings. */
export function formatFigure(figure: EvidenceFigure, value: number): string {
  return figure.metric === "item_seconds" ? formatDuration(Math.round(value)) : formatMetricValue(value);
}

/**
 * One single-match finding, compact: type, severity, title, the better play in
 * one line, and the cited figures as mini bars. The narrative sits in a
 * disclosure underneath.
 */
export function FindingCard({ insight, evidence }: { insight: Insight; evidence: Evidence[] }) {
  const cited = citedBy(insight, evidence);
  const measured = cited.filter((e) => e.figure);
  const better = insight.better_action ?? insight.explanation;
  const story = insight.what_happened || insight.why_it_matters;

  return (
    <Card className="flex flex-col gap-3">
      <div className="flex flex-wrap items-center gap-2">
        <span
          className={cn(
            "rounded-full border px-2 py-0.5 text-[0.625rem] uppercase tracking-wider",
            INSIGHT_CLASS[insight.kind],
          )}
        >
          {insight.kind_label}
        </span>
        {insight.severity ? (
          <span
            className={cn(
              "rounded-full border px-2 py-0.5 text-[0.625rem] uppercase tracking-wider",
              SEVERITY_CLASS[insight.severity],
            )}
          >
            {insight.severity}
          </span>
        ) : null}
        {insight.timestamp ? (
          <span className="font-mono text-xs tabular-nums text-operator">{insight.timestamp}</span>
        ) : null}
      </div>

      <h3 className="font-display text-base leading-snug text-ink">{fixOrdinals(insight.title)}</h3>

      {better ? (
        <p
          className={cn("text-sm text-ink-muted", story ? "truncate" : "line-clamp-2")}
          title={fixOrdinals(better)}
        >
          <span className="mr-1.5 text-[0.625rem] uppercase tracking-wider text-function">Better play</span>
          {fixOrdinals(better)}
        </p>
      ) : null}

      {measured.length > 0 ? (
        <ul className="m-0 flex list-none flex-col gap-2 p-0">
          {measured.map((item) => (
            <MiniBullet key={item.id} item={item} />
          ))}
        </ul>
      ) : null}

      {story ? (
        <details className="group text-sm">
          <summary className="focus-neon w-fit cursor-pointer list-none rounded text-xs text-function hover:text-ink">
            <span className="group-open:hidden">What happened</span>
            <span className="hidden group-open:inline">Hide</span>
          </summary>
          <dl className="mt-2 flex flex-col gap-2">
            <Part label="What happened" body={insight.what_happened} />
            <Part label="Why it mattered" body={insight.why_it_matters} />
            <Part label="Better play" body={better} />
          </dl>
        </details>
      ) : null}
    </Card>
  );
}

function Part({ label, body }: { label: string; body: string | null }) {
  if (!body) return null;
  return (
    <div className="flex flex-col gap-0.5">
      <dt className="text-[0.625rem] uppercase tracking-wider text-ink-faint">{label}</dt>
      <dd className="m-0 leading-relaxed text-ink-muted">{fixOrdinals(body)}</dd>
    </div>
  );
}

/**
 * A cited figure as a bar on its own value scale: the bar is this reading,
 * the solid tick the peer median, the hollow tick the player's baseline.
 */
export function MiniBullet({ item }: { item: Evidence }) {
  const f = item.figure!;
  const refs = [f.median, f.baseline, f.top_20].filter((v): v is number => v !== null && v !== undefined);
  const ceiling = Math.max(f.value, ...refs) * 1.1 || 1;
  const at = (v: number) => `${Math.max(0, Math.min(1, v / ceiling)) * 100}%`;
  const label = item.label.replace(/ vs peers$/, "");

  const aria = [`${label}: ${formatFigure(f, f.value)}`];
  if (f.median !== null) aria.push(`peer median ${formatFigure(f, f.median)}`);
  if (f.baseline != null) aria.push(`your average ${formatFigure(f, f.baseline)}`);
  if (f.percentile !== null) aria.push(`${ordinal(f.percentile)} percentile`);

  return (
    <li className="grid grid-cols-[6.5rem_1fr_auto] items-center gap-3">
      <span className="truncate text-xs text-ink-muted" title={label}>
        {label}
      </span>
      <div role="img" aria-label={`${aria.join("; ")}.`} className="relative h-1.5 rounded-full bg-mark-track">
        <div className="absolute inset-y-0 left-0 rounded-full bg-you" style={{ width: at(f.value) }} />
        {f.median !== null ? (
          <span aria-hidden className="absolute -inset-y-1 w-0.5 rounded bg-rank" style={{ left: at(f.median) }} />
        ) : null}
        {f.baseline != null ? (
          <span
            aria-hidden
            className="absolute top-1/2 size-2.5 -translate-x-1/2 -translate-y-1/2 rounded-full border-2 border-ink-muted bg-surface-2"
            style={{ left: at(f.baseline) }}
          />
        ) : null}
      </div>
      <span className="flex items-center gap-1.5 font-mono text-[0.6875rem] tabular-nums">
        <span className="text-ink">{formatFigure(f, f.value)}</span>
        {f.percentile !== null ? (
          <span
            className={cn(
              "rounded px-1 text-[0.625rem]",
              f.percentile >= 70
                ? "bg-mark-win/15 text-string"
                : f.percentile <= 30
                  ? "bg-mark-loss/15 text-error"
                  : "bg-mark-track text-ink-muted",
            )}
          >
            {ordinal(f.percentile)}
          </span>
        ) : f.median !== null && f.metric === "item_seconds" ? (
          <span className="text-ink-faint">p50 {formatFigure(f, f.median)}</span>
        ) : null}
      </span>
    </li>
  );
}

/**
 * Everything the coach was shown that carries a number, as one table:
 * metric · this game · your baseline · peer median · percentile · sample.
 * Statements without a number follow as a short list.
 */
export function MeasuredTable({ evidence }: { evidence: Evidence[] }) {
  const rows = evidence.filter(
    (e) => e.figure && (e.id.startsWith("match.") || e.id.startsWith("benchmark.")),
  );
  const rest = evidence.filter((e) => !rows.includes(e));

  return (
    <div className="flex flex-col gap-3">
      {rows.length > 0 ? (
        <div className="-mx-1 overflow-x-auto">
          <table className="w-full min-w-[32rem] border-collapse text-xs">
            <thead>
              <tr className="text-left text-[0.625rem] uppercase tracking-wider text-ink-faint">
                <th className="px-1 py-1.5 font-normal">Metric</th>
                <th className="px-1 py-1.5 text-right font-normal">This game</th>
                <th className="px-1 py-1.5 text-right font-normal">Your baseline</th>
                <th className="px-1 py-1.5 text-right font-normal">Peer median</th>
                <th className="px-1 py-1.5 text-right font-normal">Pct</th>
                <th className="px-1 py-1.5 text-right font-normal">Sample</th>
              </tr>
            </thead>
            <tbody className="font-mono tabular-nums">
              {rows.map((e) => {
                const f = e.figure!;
                const isMatch = e.id.startsWith("match.");
                const baseline = isMatch ? (f.baseline ?? null) : f.value;
                return (
                  <tr key={e.id} className="border-t border-glass-edge">
                    <td className="px-1 py-1.5 font-sans text-ink-muted">{e.label.replace(/ vs peers$/, "")}</td>
                    <td className="px-1 py-1.5 text-right text-ink">{isMatch ? formatFigure(f, f.value) : "—"}</td>
                    <td className="px-1 py-1.5 text-right text-ink-muted">
                      {baseline !== null ? formatFigure(f, baseline) : "—"}
                    </td>
                    <td className="px-1 py-1.5 text-right text-ink-muted">
                      {f.median !== null ? formatFigure(f, f.median) : "—"}
                    </td>
                    <td className="px-1 py-1.5 text-right text-ink-muted">
                      {f.percentile !== null ? ordinal(f.percentile) : "—"}
                    </td>
                    <td className="px-1 py-1.5 text-right text-ink-faint">{e.sample}</td>
                  </tr>
                );
              })}
            </tbody>
          </table>
        </div>
      ) : null}
      {rest.length > 0 ? (
        <ul className="m-0 flex list-none flex-col gap-1.5 p-0">
          {rest.map((e) => (
            <li key={e.id} className="text-xs leading-relaxed text-ink-muted">
              <span className="mr-1.5 text-[0.625rem] uppercase tracking-wider text-ink-faint">{e.label}</span>
              {fixOrdinals(e.statement)}
            </li>
          ))}
        </ul>
      ) : null}
    </div>
  );
}

"use client";

import Link from "next/link";
import { useEffect, useState } from "react";

import { KpiTile } from "@/components/charts/KpiTile";
import { TrendLine } from "@/components/charts/TrendLine";
import { WinLossStrip } from "@/components/charts/WinLossStrip";
import { MatchCard, PeerBar, StandoutBadge } from "@/components/matches/MatchCard";
import { SignedOut } from "@/components/shell/SignedOut";
import { Alert } from "@/components/ui/Alert";
import { Button } from "@/components/ui/Button";
import { Card } from "@/components/ui/Card";
import { ChartCard } from "@/components/ui/ChartCard";
import { Combobox } from "@/components/ui/Combobox";
import { HeroPortrait } from "@/components/ui/HeroPortrait";
import { Icon } from "@/components/ui/Icon";
import { InfoTip } from "@/components/ui/InfoTip";
import { RelativeTime } from "@/components/ui/RelativeTime";
import { Segmented } from "@/components/ui/Segmented";
import { ApiError, getMatches } from "@/lib/api";
import { matchBadges } from "@/lib/match-badges";
import type {
  CoachableRole,
  MatchesMode,
  MatchListResponse,
  MatchListSummary,
  MatchResultFilter,
  MatchSort,
  MatchView,
} from "@/lib/types";
import { useSession } from "@/lib/session-context";
import { cn, formatDuration } from "@/lib/utils";

const PAGE_SIZE = 20;

/** Each answers a question a player asks of their own history. Win/loss is
 *  absent because it is the Result filter. */
const SORTS: { value: MatchSort; label: string }[] = [
  { value: "newest", label: "Newest first" },
  { value: "oldest", label: "Oldest first" },
  { value: "gpm_desc", label: "Highest GPM" },
  { value: "gpm_asc", label: "Lowest GPM" },
  { value: "kda_desc", label: "Highest KDA" },
];

const RESULTS: { value: MatchResultFilter; label: string }[] = [
  { value: "all", label: "All" },
  { value: "win", label: "Wins" },
  { value: "loss", label: "Losses" },
];

const MODES: { value: MatchesMode; label: string }[] = [
  { value: "eligible", label: "Ranked + All Pick" },
  { value: "turbo", label: "Turbo" },
  { value: "all", label: "All" },
];

type Days = "all" | "7" | "30" | "90";
const DATES: { value: Days; label: string }[] = [
  { value: "all", label: "Any date" },
  { value: "7", label: "7d" },
  { value: "30", label: "30d" },
  { value: "90", label: "90d" },
];

const POSITIONS: { value: CoachableRole; short: string; label: string }[] = [
  { value: "carry", short: "1", label: "Carry" },
  { value: "mid", short: "2", label: "Mid" },
  { value: "offlane", short: "3", label: "Offlane" },
  { value: "soft_support", short: "4", label: "Soft Support" },
  { value: "hard_support", short: "5", label: "Hard Support" },
];

const MODE_LABEL: Record<MatchesMode, string> = {
  eligible: "Ranked + All Pick",
  ranked: "Ranked",
  turbo: "Turbo",
  all: "every mode",
};

/** The mode in `?mode=`; anything unknown is the default. */
function modeFromUrl(): MatchesMode {
  if (typeof window === "undefined") return "eligible";
  const value = new URLSearchParams(window.location.search).get("mode");
  return value === "turbo" || value === "all" ? value : "eligible";
}

/** Keep `?mode=` in step without a navigation; the default is left out. */
function writeModeToUrl(mode: MatchesMode) {
  const url = new URL(window.location.href);
  if (mode === "eligible") url.searchParams.delete("mode");
  else url.searchParams.set("mode", mode);
  window.history.replaceState(window.history.state, "", url);
}

export function MatchList() {
  const { session } = useSession();
  /** The latest response: summary, totals and filter options. */
  const [data, setData] = useState<MatchListResponse | null>(null);
  /** Every row loaded so far; "Load more" appends the next page. */
  const [rows, setRows] = useState<MatchView[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [page, setPage] = useState(1);
  const [loading, setLoading] = useState(false);
  const [mode, setModeState] = useState<MatchesMode>(modeFromUrl);

  /** Filters. Each is the value the backend takes; `undefined` means "any". */
  const [heroId, setHeroId] = useState<number | undefined>();
  const [role, setRole] = useState<CoachableRole | undefined>();
  const [result, setResult] = useState<MatchResultFilter>("all");
  const [sort, setSort] = useState<MatchSort>("newest");
  const [days, setDays] = useState<Days>("all");

  // One request per change; filtering, sorting and paging are server-side so
  // the totals on screen always describe the list on screen. Rows already
  // shown stay, dimmed, while the next request runs.
  useEffect(() => {
    // A failed profile read is reported once by the shell; this page
    // still loads its own data.
    if (session.kind === "loading" || session.kind === "anonymous") return;
    let cancelled = false;

    setLoading(true);
    const params = {
      mode,
      heroId,
      role,
      result,
      sort,
      ...(days === "all" ? {} : { days: Number(days) }),
    };
    getMatches(page, PAGE_SIZE, params)
      .then((response) => {
        if (cancelled) return;
        setData(response);
        setRows((previous) => (page === 1 ? response.matches : [...previous, ...response.matches]));
        setError(null);
      })
      .catch((e) => {
        if (cancelled) return;
        setError(e instanceof ApiError ? e.message : "Could not load your match history.");
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });

    // Guards against an out-of-order response overwriting a newer one.
    return () => {
      cancelled = true;
    };
  }, [session.kind, page, mode, heroId, role, result, sort, days]);

  if (session.kind === "loading") return <ListSkeleton />;
  if (session.kind === "anonymous") return <SignedOut />;

  if (error) return <Alert>{error}</Alert>;
  if (!data) return <ListSkeleton />;

  /** Anything that narrows the list. Sort is not a filter — it reorders. */
  const filtered = heroId !== undefined || role !== undefined || result !== "all" || days !== "all";

  /** Every filter change starts again from the first page. */
  function change<T>(set: (value: T) => void) {
    return (value: T) => {
      set(value);
      setPage(1);
    };
  }

  /** A new mode is a new population: the hero picked in the old one may
   *  have no games here, so the hero filter resets with it. */
  function setMode(next: MatchesMode) {
    setModeState(next);
    setHeroId(undefined);
    setPage(1);
    writeModeToUrl(next);
  }

  function clearFilters() {
    setHeroId(undefined);
    setRole(undefined);
    setResult("all");
    setSort("newest");
    setDays("all");
    setPage(1);
  }

  const { heroes, roles } = data.filters;
  const roleCount = new Map(roles.map((r) => [r.value, r.matches]));
  const heroLabel = heroes.find((h) => Number(h.value) === heroId)?.label;

  const chips = [
    heroId !== undefined && { key: "hero", label: heroLabel ?? "Hero", clear: () => change(setHeroId)(undefined) },
    role && { key: "role", label: POSITIONS.find((p) => p.value === role)?.label ?? role, clear: () => change(setRole)(undefined) },
    result !== "all" && { key: "result", label: result === "win" ? "Wins" : "Losses", clear: () => change(setResult)("all") },
    days !== "all" && { key: "days", label: `Last ${days} days`, clear: () => change(setDays)("all") },
  ].filter(Boolean) as { key: string; label: string; clear: () => void }[];

  return (
    <div className="flex flex-col gap-4 pb-4">
      <section aria-label="Filter and sort matches" className="glass flex flex-col gap-3 rounded-card p-3 sm:p-4">
        <div className="flex flex-wrap items-center gap-2">
          <Segmented label="Which games" value={mode} onChange={setMode} options={MODES} />
          <Segmented label="Date range" value={days} onChange={change(setDays)} options={DATES} />
        </div>

        <div className="grid grid-cols-2 gap-2 sm:flex sm:flex-wrap sm:items-end">
          <Field label="Hero" className="col-span-2 sm:col-span-1">
            <Combobox
              label="Hero"
              anyLabel="All heroes"
              value={heroId === undefined ? undefined : String(heroId)}
              onChange={(value) => change(setHeroId)(value === undefined ? undefined : Number(value))}
              options={heroes
                .filter((hero) => hero.matches > 0)
                .sort((a, b) => b.matches - a.matches)
                .map((hero) => ({ value: hero.value, label: hero.label, hint: String(hero.matches) }))}
            />
          </Field>

          <Field label="Result">
            <Segmented label="Result" value={result} onChange={change(setResult)} options={RESULTS} />
          </Field>

          <Field label="Sort">
            <select
              aria-label="Sort"
              value={sort}
              onChange={(e) => change(setSort)(e.target.value as MatchSort)}
              className="focus-neon min-h-11 w-full min-w-0 cursor-pointer rounded-xl border border-glass-edge bg-surface-2/60 px-3 text-sm text-ink outline-none sm:w-auto"
            >
              {SORTS.map((option) => (
                <option key={option.value} value={option.value}>
                  {option.label}
                </option>
              ))}
            </select>
          </Field>
        </div>

        {/* Positions as chips, with the games each would leave in this mode. */}
        <div role="radiogroup" aria-label="Position" className="flex flex-wrap gap-1.5">
          <PositionChip active={role === undefined} onClick={() => change(setRole)(undefined)}>
            All positions
          </PositionChip>
          {POSITIONS.map((p) => {
            const count = roleCount.get(p.value) ?? 0;
            return (
              <PositionChip
                key={p.value}
                active={role === p.value}
                disabled={count === 0 && role !== p.value}
                onClick={() => change(setRole)(p.value)}
                title={p.label}
              >
                <span className="font-mono">Pos {p.short}</span>
                <span className="hidden sm:inline">{p.label}</span>
                <span className="font-mono text-ink-faint">{count}</span>
              </PositionChip>
            );
          })}
        </div>

        {chips.length > 0 || sort !== "newest" ? (
          <div className="flex flex-wrap items-center gap-1.5">
            {chips.map((chip) => (
              <button
                key={chip.key}
                type="button"
                onClick={chip.clear}
                aria-label={`Remove filter: ${chip.label}`}
                className="focus-neon flex cursor-pointer items-center gap-1 rounded-full bg-ink/10 px-2.5 py-1 text-xs text-ink hover:bg-ink/15"
              >
                {chip.label}
                <Icon name="close" className="size-3" />
              </button>
            ))}
            <Button variant="ghost" onClick={clearFilters} className="min-h-8 px-3 text-xs">
              Clear filters
            </Button>
          </div>
        ) : null}
      </section>

      <p className="flex items-center gap-1.5 font-mono text-xs tabular-nums text-ink-faint" aria-live="polite">
        {filtered ? `${data.total.toLocaleString("en-US")} of ` : ""}
        {data.mode_total.toLocaleString("en-US")} {data.mode_total === 1 ? "match" : "matches"} · {MODE_LABEL[mode]}
        {data.hidden_turbo + data.hidden_other > 0 ? (
          <InfoTip label="Games not shown">
            {data.hidden_turbo > 0 ? <span>+{data.hidden_turbo} Turbo hidden</span> : null}
            {data.hidden_other > 0 ? <span>+{data.hidden_other} other games hidden (other modes or older)</span> : null}
          </InfoTip>
        ) : null}
      </p>

      {data.summary.matches > 0 ? <SummaryStrip summary={data.summary} /> : null}

      {rows.length === 0 ? (
        <EmptyState filtered={filtered} mode={mode} onClear={clearFilters} onShowEverything={() => setMode("all")} />
      ) : (
        <>
          {/* Cards on a phone, a dense table once there is width for one. */}
          <ul className={cn("grid gap-3 lg:hidden", loading && "opacity-60 transition-opacity")}>
            {rows.map((match, i) => (
              <MatchCard key={match.id} match={match} index={i} mode={mode} badges={matchBadges(match, data.summary)} />
            ))}
          </ul>
          <MatchTable rows={rows} summary={data.summary} mode={mode} loading={loading} />

          {rows.length < data.total ? (
            <Button
              variant="ghost"
              disabled={loading}
              onClick={() => setPage((p) => p + 1)}
              className="self-center px-6 text-sm"
            >
              {loading ? "Loading…" : `Load more · ${(data.total - rows.length).toLocaleString("en-US")} left`}
            </Button>
          ) : null}
        </>
      )}
    </div>
  );
}

/** KPIs, the last 20 results and a rolling win rate, for the current filter. */
function SummaryStrip({ summary }: { summary: MatchListSummary }) {
  const losses = summary.matches - summary.wins;
  return (
    <section aria-label="Summary for this filter" className="grid gap-3 sm:grid-cols-3 lg:grid-cols-6">
      <KpiTile
        label="Win rate"
        value={summary.win_rate === null ? "—" : `${Math.round(summary.win_rate)}%`}
        comparison={
          <span className="font-mono text-xs tabular-nums text-ink-muted">
            {summary.wins}W – {losses}L{summary.capped ? ` · newest ${summary.matches}` : ""}
          </span>
        }
      />
      <KpiTile label="KDA" value={summary.avg_kda === null ? "—" : summary.avg_kda.toFixed(2)} comparison={<span className="text-xs text-ink-faint">average</span>} />
      <KpiTile
        label="GPM"
        value={summary.avg_gpm === null ? "—" : Math.round(summary.avg_gpm).toLocaleString("en-US")}
        comparison={<span className="text-xs text-ink-faint">average</span>}
      />
      <ChartCard
        title={`Rolling ${summary.rolling}-game win rate`}
        className="sm:col-span-3"
        action={<WinLossStrip results={summary.recent_results} label={`Last ${summary.recent_results.length}`} />}
      >
        <TrendLine
          points={summary.rolling_win_rate.map((value, i) => ({
            value,
            label: `${summary.rolling} games ending at game ${i + 1} of ${summary.rolling_win_rate.length}`,
          }))}
          label="Rolling win rate"
          format={(v) => `${Math.round(v)}%`}
          reference={{ value: 50, label: "50%" }}
        />
      </ChartCard>
    </section>
  );
}

function MatchTable({
  rows,
  summary,
  mode,
  loading,
}: {
  rows: MatchView[];
  summary: MatchListSummary;
  mode: MatchesMode;
  loading: boolean;
}) {
  return (
    <div className={cn("glass hidden overflow-hidden rounded-card lg:block", loading && "opacity-60 transition-opacity")}>
      <table className="w-full text-sm">
        <thead>
          <tr className="border-b border-glass-edge text-left text-[0.625rem] uppercase tracking-wider text-ink-faint">
            <th className="px-3 py-2 font-medium">Hero</th>
            <th className="px-2 py-2 font-medium">Result</th>
            <th className="px-2 py-2 text-right font-medium">K/D/A</th>
            <th className="w-32 px-2 py-2 font-medium">GPM vs peers</th>
            <th className="w-32 px-2 py-2 font-medium">XPM vs peers</th>
            <th className="px-2 py-2 text-right font-medium">Length</th>
            <th className="px-2 py-2 text-right font-medium">When</th>
            <th className="px-3 py-2 text-right font-medium">
              <span className="inline-flex items-center gap-1">
                MMR est.
                <InfoTip label="About MMR est.">
                  A modeled change for ranked games only. Valve does not publish per-match MMR.
                </InfoTip>
              </span>
            </th>
          </tr>
        </thead>
        <tbody className="divide-y divide-glass-edge">
          {rows.map((m) => (
            <tr key={m.id} className="group relative transition-colors hover:bg-surface-2/40">
              <td className="px-3 py-2">
                {/* The whole row opens the match; this link carries it. */}
                <Link href={`/matches/${m.id}`} className="focus-neon flex items-center gap-2.5 rounded after:absolute after:inset-0">
                  <HeroPortrait heroId={m.hero_id} heroName={m.hero_name} size="sm" />
                  <span className="flex min-w-0 flex-col">
                    <span className="flex items-center gap-1.5">
                      <span className="truncate text-ink">{m.hero_name}</span>
                      {matchBadges(m, summary).map((b) => (
                        <StandoutBadge key={b} label={b} />
                      ))}
                    </span>
                    <span className="flex items-center gap-1 text-[0.6875rem] text-ink-faint">
                      {m.position ? (
                        <span className="rounded bg-surface-2 px-1 font-mono text-ink-muted" title={m.position_label}>
                          P{POSITIONS.find((p) => p.value === m.position)?.short}
                        </span>
                      ) : null}
                      {m.position_label}
                      {mode !== "eligible" && !m.eligible ? <span title={m.mode_label}>· {m.mode_label}</span> : null}
                    </span>
                  </span>
                </Link>
              </td>
              <td className="px-2 py-2">
                <span
                  className={cn(
                    "rounded px-1.5 py-0.5 text-[0.625rem] font-semibold uppercase tracking-wider",
                    m.won ? "bg-mark-win/15 text-string" : "bg-mark-loss/15 text-error",
                  )}
                >
                  {m.won ? "Win" : "Loss"}
                </span>
              </td>
              <td className="px-2 py-2 text-right">
                <span className="block font-mono tabular-nums text-number">
                  {m.kills}/{m.deaths}/{m.assists}
                </span>
                <span className="block font-mono text-[0.6875rem] tabular-nums text-ink-faint">
                  {m.kda?.toFixed(1) ?? "—"} KDA
                </span>
              </td>
              <td className="px-2 py-2">
                <PeerBar label="GPM" value={m.gpm} peer={m.peer_gpm} hero={m.hero_name} />
              </td>
              <td className="px-2 py-2">
                <PeerBar label="XPM" value={m.xpm} peer={m.peer_xpm} hero={m.hero_name} />
              </td>
              <td className="px-2 py-2 text-right font-mono text-xs tabular-nums text-ink-muted">
                {formatDuration(m.duration_seconds)}
              </td>
              <td className="px-2 py-2 text-right text-xs text-ink-muted">
                <RelativeTime iso={m.started_at} />
              </td>
              <td className="px-3 py-2 text-right font-mono text-xs tabular-nums">
                {m.mmr_delta_estimate === null ? (
                  <span className="text-ink-faint">—</span>
                ) : (
                  <span className={m.mmr_delta_estimate >= 0 ? "text-string" : "text-error"}>
                    {m.mmr_delta_estimate >= 0 ? "+" : "−"}
                    {Math.abs(m.mmr_delta_estimate)}
                  </span>
                )}
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}

function PositionChip({
  active,
  disabled = false,
  onClick,
  title,
  children,
}: {
  active: boolean;
  disabled?: boolean;
  onClick: () => void;
  title?: string;
  children: React.ReactNode;
}) {
  return (
    <button
      type="button"
      role="radio"
      aria-checked={active}
      disabled={disabled}
      onClick={onClick}
      title={title}
      className={cn(
        "focus-neon flex min-h-9 cursor-pointer items-center gap-1.5 rounded-full border px-3 text-xs transition-colors",
        active
          ? "border-ink/30 bg-ink/10 text-ink"
          : "border-glass-edge text-ink-muted hover:text-ink",
        disabled && "cursor-not-allowed opacity-40",
      )}
    >
      {children}
    </button>
  );
}

/**
 * Nothing to show, and why: a filter that matched nothing is undone, an
 * empty mode is widened.
 */
function EmptyState({
  filtered,
  mode,
  onClear,
  onShowEverything,
}: {
  filtered: boolean;
  mode: MatchesMode;
  onClear: () => void;
  onShowEverything: () => void;
}) {
  if (filtered) {
    return (
      <Card className="flex flex-wrap items-center justify-between gap-3">
        <p className="text-sm text-ink">No matches for these filters</p>
        <Button variant="ghost" onClick={onClear} className="px-4 text-sm">
          Clear filters
        </Button>
      </Card>
    );
  }

  return (
    <Card className="flex flex-wrap items-center justify-between gap-3">
      <p className="text-sm text-ink-muted">
        {mode === "all" ? "No matches stored yet" : `No ${MODE_LABEL[mode]} games`}
      </p>
      {mode !== "all" ? (
        <Button variant="ghost" onClick={onShowEverything} className="px-4 text-sm">
          Show all games
        </Button>
      ) : null}
    </Card>
  );
}

function Field({
  label,
  className,
  children,
}: {
  label: string;
  className?: string;
  children: React.ReactNode;
}) {
  return (
    <div className={cn("flex min-w-0 flex-col gap-1 text-xs text-ink-faint", className)}>
      <span aria-hidden>{label}</span>
      {children}
    </div>
  );
}

function ListSkeleton() {
  return (
    <div className="flex flex-col gap-4" aria-busy="true" aria-live="polite">
      <span className="sr-only">Loading matches…</span>
      <div className="glass h-36 animate-pulse rounded-card" />
      <div className="grid gap-3 sm:grid-cols-3 lg:grid-cols-6">
        {[0, 1, 2].map((i) => (
          <div key={i} className="glass h-24 animate-pulse rounded-card" />
        ))}
        <div className="glass h-24 animate-pulse rounded-card sm:col-span-3" />
      </div>
      {[0, 1, 2, 3, 4].map((i) => (
        <div key={i} className="glass h-16 animate-pulse rounded-card" />
      ))}
    </div>
  );
}

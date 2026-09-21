"use client";

import { useCallback, useEffect, useState } from "react";

import { ComparisonContext } from "@/components/benchmark/ComparisonContext";
import { BulletRow } from "@/components/charts/BulletRow";
import { SignedOut } from "@/components/shell/SignedOut";
import { Alert } from "@/components/ui/Alert";
import { Card } from "@/components/ui/Card";
import { HeroPortrait } from "@/components/ui/HeroPortrait";
import { ApiError, getBenchmark, getStats } from "@/lib/api";
import { CONFIDENCE_NOTE } from "@/lib/confidence";
import { useSession } from "@/lib/session-context";
import type {
  BenchmarkResponse,
  CoachableRole,
  HeroStats,
  RankBracket,
} from "@/lib/types";
import { cn } from "@/lib/utils";

/**
 * Where the chosen bracket lives between page views.
 *
 * `sessionStorage`, not the database: which peer group someone is curious about
 * right now is a browsing state, and the product has no user-preference table
 * to put it in. It also dies with the tab, which is the right lifetime — the
 * bracket a player wants to see defaults back to their own rank tomorrow.
 */
const BRACKET_KEY = "benchmark.bracket";

function savedBracket(): RankBracket | undefined {
  // Absent during server rendering, and can be disabled outright in the
  // browser. Not being able to remember the choice is not a reason to fail the
  // page, and the server's answer — "no bracket" — is also the initial markup,
  // so there is nothing for hydration to disagree about.
  if (typeof window === "undefined") return undefined;

  try {
    return (sessionStorage.getItem(BRACKET_KEY) as RankBracket | null) ?? undefined;
  } catch {
    return undefined;
  }
}

export function Benchmark() {
  const { session } = useSession();
  const [heroes, setHeroes] = useState<HeroStats[]>([]);
  const [data, setData] = useState<BenchmarkResponse | null>(null);
  const [selected, setSelected] = useState<number | undefined>();
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  /** `undefined` follows the coaching role; `"all"` opts out of role scoping. */
  const [roleScope, setRoleScope] = useState<CoachableRole | "all" | undefined>();
  /** `undefined` compares against the player's own rank. Restored on the first
   *  render rather than in an effect, so the picker never shows "your rank"
   *  over another bracket's numbers for a frame. */
  const [bracket, setBracket] = useState<RankBracket | undefined>(savedBracket);

  const load = useCallback(
    async (
      heroId?: number,
      role?: CoachableRole | "all",
      target?: RankBracket,
    ) => {
      setLoading(true);
      try {
        // Replaced wholesale rather than merged: a bracket switch changes every
        // peer number on the page, and a half-updated response would show one
        // bracket's medians under another's heading.
        setData(await getBenchmark(heroId, role, target));
        setError(null);
      } catch (e) {
        setError(
          e instanceof ApiError ? e.message : "Could not load your benchmarks.",
        );
      } finally {
        setLoading(false);
      }
    },
    [],
  );

  useEffect(() => {
    if (session.kind !== "signed-in") return;

    // The hero list drives the picker; the benchmark defaults to the
    // most-played, which is the only hero likely to clear the sample floor.
    getStats()
      .then((stats) => setHeroes(stats.heroes))
      .catch(() => setHeroes([]));

    // Read again rather than carried over from the initializer: the state
    // above already holds it, and passing it here keeps the restore to a
    // single request instead of a default one followed by a correction.
    void load(undefined, undefined, savedBracket());
  }, [session.kind, load]);

  if (session.kind === "loading") return <BenchmarkSkeleton />;
  if (session.kind === "anonymous") return <SignedOut />;
  if (session.kind === "error") {
    return <Alert title="Cannot reach the service">{session.message}</Alert>;
  }

  if (error) return <Alert>{error}</Alert>;
  if (!data) return <BenchmarkSkeleton />;

  async function pick(heroId: number) {
    setSelected(heroId);
    await load(heroId, roleScope, bracket);
  }

  async function setScope(role: CoachableRole | "all" | undefined) {
    setRoleScope(role);
    // The hero resets with the scope: the most-played hero in one role is
    // frequently not the most-played in another, and keeping a stale pick would
    // silently benchmark a hero the new scope barely contains.
    setSelected(undefined);
    await load(undefined, role, bracket);
  }

  /** The hero and role stay put: only the peer group changes. */
  async function setTargetBracket(next: RankBracket | undefined) {
    setBracket(next);
    try {
      if (next) sessionStorage.setItem(BRACKET_KEY, next);
      else sessionStorage.removeItem(BRACKET_KEY);
    } catch {
      // See `savedBracket` — remembering is a nicety, not a requirement.
    }
    await load(selected, roleScope, next);
  }

  const confidence = data.results[0]?.confidence;
  const caveat = confidence ? CONFIDENCE_NOTE[confidence] : "";
  /** The bracket the peer values on screen actually describe. */
  const resolved = data.context.bracket;
  /** Asked for a bracket, got a wider cohort. Never silent when it was chosen. */
  const fellBack = bracket !== undefined && resolved.fell_back;
  const ownRank = data.brackets.find((option) => option.is_player_rank);

  return (
    <div className="flex flex-col gap-5 pb-4">
      {heroes.length > 1 ? (
        <nav aria-label="Choose a hero" className="flex gap-2 overflow-x-auto pb-1">
          {heroes.map((hero) => {
            const active = (selected ?? data.hero_id) === hero.hero_id;
            return (
              <button
                key={hero.hero_id}
                type="button"
                onClick={() => pick(hero.hero_id)}
                aria-current={active ? "true" : undefined}
                className={cn(
                  "focus-neon flex min-h-11 shrink-0 cursor-pointer items-center gap-2",
                  "rounded-xl border px-3 py-2 transition-colors duration-200 ease-out",
                  active
                    ? "border-function/60 bg-function/10 text-ink"
                    : "border-glass-edge text-ink-muted hover:text-ink",
                )}
              >
                <HeroPortrait
                  heroId={hero.hero_id}
                  heroName={hero.hero_name}
                  size="sm"
                />
                <span className="whitespace-nowrap text-xs">
                  {hero.hero_name}
                  <span className="ml-1.5 font-mono text-ink-faint">
                    {hero.matches}
                  </span>
                </span>
              </button>
            );
          })}
        </nav>
      ) : null}

      {/* Which of the player's own matches are being compared. The peer side
          cannot be narrowed — see the context block below — but this side can,
          and defaults to the role they are being coached on. */}
      {data.context.role || roleScope === "all" ? (
        <nav aria-label="Comparison scope" className="flex flex-wrap gap-2">
          <ScopeChip
            label={
              data.context.role_label
                ? `${data.context.role_label} only`
                : "Coaching role"
            }
            active={roleScope !== "all"}
            onClick={() => void setScope(undefined)}
          />
          <ScopeChip
            label="All roles"
            active={roleScope === "all"}
            onClick={() => void setScope("all")}
          />
        </nav>
      ) : null}

      {/* Which peers to measure against. Separate from the scope chips above
          because it changes the *other* side of the comparison: those narrow
          the player's own matches, this changes who they are held up to. */}
      <div className="flex flex-wrap items-end gap-3">
        <label className="flex flex-col gap-1 text-xs text-ink-faint">
          Compare against
          <select
            value={bracket ?? ""}
            onChange={(e) =>
              void setTargetBracket(
                e.target.value === ""
                  ? undefined
                  : (e.target.value as RankBracket),
              )
            }
            className="focus-neon min-h-11 cursor-pointer rounded-xl border border-glass-edge bg-surface-2/60 px-3 text-sm text-ink outline-none"
          >
            {/* Not a bracket of its own: it is "whatever my rank is", which is
                the all-ranks distribution for an unranked player. */}
            <option value="">
              {ownRank ? `Your rank (${ownRank.label})` : "Your rank (unranked)"}
            </option>
            {data.brackets.map((option) => (
              <option key={option.value} value={option.value}>
                {option.label}
                {option.is_player_rank ? " — your rank" : ""}
              </option>
            ))}
          </select>
        </label>

        <p className="pb-3 text-xs text-ink-faint">
          Peer values below cover{" "}
          <span className="text-ink-muted">{resolved.label}</span>.
        </p>
      </div>

      {/* A bracket the provider has no data for falls back to all ranks. That
          is a real answer to a different question, so it is stated outright
          rather than left to the collapsed caveats — the reader picked this
          bracket, and has to know they are not looking at it. */}
      {fellBack ? (
        <Alert tone="info" title="No data for that bracket">
          The benchmark provider publishes no distribution for {data.hero_name}{" "}
          in{" "}
          {data.brackets.find((option) => option.value === bracket)?.label ??
            "that bracket"}
          , so the peer values below cover every rank instead. Nothing has been
          estimated from a neighbouring bracket.
        </Alert>
      ) : null}

      {data.note ? <Alert tone="info">{data.note}</Alert> : null}

      {data.results.length > 0 ? (
        <>
          <Card className="flex flex-col gap-1">
            <p className="text-sm text-ink">
              {data.hero_name} ·{" "}
              <span className="font-mono tabular-nums text-number">
                {data.sample}
              </span>{" "}
              {data.sample === 1 ? "match" : "matches"}
              <span className="text-ink-faint"> vs {resolved.label}</span>
            </p>
            {caveat ? (
              <p className="text-xs leading-relaxed text-ink-faint">{caveat}</p>
            ) : null}
          </Card>

          <Card
            className={cn(
              "flex flex-col gap-6",
              loading && "opacity-60 transition-opacity",
            )}
          >
            {data.results.map((result) => (
              <BulletRow key={result.metric} result={result} />
            ))}
          </Card>
        </>
      ) : null}

      {/* The honesty note the whole page rests on — composed by the backend,
          which is the only place that knows what the provider actually
          delivered. A second copy of these claims here would be a second thing
          to keep true. */}
      <Card className="flex flex-col gap-2">
        <h2 className="text-xs font-semibold uppercase tracking-[0.2em] text-ink-faint">
          What this compares against
        </h2>
        <ComparisonContext context={data.context} />
      </Card>
    </div>
  );
}

function ScopeChip({
  label,
  active,
  onClick,
}: {
  label: string;
  active: boolean;
  onClick: () => void;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      aria-current={active ? "true" : undefined}
      className={cn(
        "focus-neon min-h-11 cursor-pointer rounded-xl border px-3 py-2 text-xs",
        "transition-colors duration-200 ease-out",
        active
          ? "border-function/60 bg-function/10 text-ink"
          : "border-glass-edge text-ink-muted hover:text-ink",
      )}
    >
      {label}
    </button>
  );
}

function BenchmarkSkeleton() {
  return (
    <div className="flex flex-col gap-4" aria-busy="true" aria-live="polite">
      <span className="sr-only">Loading benchmarks…</span>
      <div className="h-14 animate-pulse rounded-card bg-surface-2" />
      <div className="h-80 animate-pulse rounded-card bg-surface-2" />
    </div>
  );
}

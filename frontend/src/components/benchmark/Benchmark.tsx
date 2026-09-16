"use client";

import { useCallback, useEffect, useState } from "react";

import { ComparisonContext } from "@/components/benchmark/ComparisonContext";
import { BulletRow } from "@/components/charts/BulletRow";
import { SignedOut } from "@/components/shell/SignedOut";
import { Alert } from "@/components/ui/Alert";
import { Card } from "@/components/ui/Card";
import { HeroPortrait } from "@/components/ui/HeroPortrait";
import { ApiError, getBenchmark, getStats } from "@/lib/api";
import { useSession } from "@/lib/session-context";
import type { BenchmarkResponse, CoachableRole, HeroStats } from "@/lib/types";
import { cn } from "@/lib/utils";

/** How a `Confidence` reads to someone who has not read the spec. */
const CONFIDENCE_NOTE: Record<string, string> = {
  insufficient:
    "Too few matches on this hero to place you in the distribution. Percentiles appear once you have five.",
  low: "Based on a small number of matches, so treat the percentiles as indicative.",
  adequate: "",
};

export function Benchmark() {
  const { session } = useSession();
  const [heroes, setHeroes] = useState<HeroStats[]>([]);
  const [data, setData] = useState<BenchmarkResponse | null>(null);
  const [selected, setSelected] = useState<number | undefined>();
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  /** `undefined` follows the coaching role; `"all"` opts out of role scoping. */
  const [roleScope, setRoleScope] = useState<CoachableRole | "all" | undefined>();

  const load = useCallback(
    async (heroId?: number, role?: CoachableRole | "all") => {
      setLoading(true);
      try {
        setData(await getBenchmark(heroId, role));
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
    void load();
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
    await load(heroId, roleScope);
  }

  async function setScope(role: CoachableRole | "all" | undefined) {
    setRoleScope(role);
    // The hero resets with the scope: the most-played hero in one role is
    // frequently not the most-played in another, and keeping a stale pick would
    // silently benchmark a hero the new scope barely contains.
    setSelected(undefined);
    await load(undefined, role);
  }

  const confidence = data.results[0]?.confidence;
  const caveat = confidence ? CONFIDENCE_NOTE[confidence] : "";

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

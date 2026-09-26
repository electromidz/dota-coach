"use client";

import { Suspense, useEffect, useState } from "react";

import { EstablishedRankPanel } from "@/components/calibrating/EstablishedRankPanel";
import { MatchHistoryTable } from "@/components/calibrating/MatchHistoryTable";
import { MethodologyNote } from "@/components/calibrating/MethodologyNote";
import { MomentumChart } from "@/components/calibrating/MomentumChart";
import { RankCard } from "@/components/calibrating/RankCard";
import { RankDistributionBars } from "@/components/calibrating/RankDistributionBars";
import { RolePreferenceBars } from "@/components/calibrating/RolePreferenceBars";
import { StreakBadge } from "@/components/calibrating/StreakBadge";
import { TrajectoryChart } from "@/components/calibrating/TrajectoryChart";
import { PageSkeleton } from "@/components/shell/PageSkeleton";
import { SignedOut } from "@/components/shell/SignedOut";
import { Alert } from "@/components/ui/Alert";
import { Card } from "@/components/ui/Card";
import { ApiError, getCalibration, getRankDistribution } from "@/lib/api";
import { useSession } from "@/lib/session-context";
import type { CalibrationResponse, RankDistribution } from "@/lib/types";

/**
 * The calibrating screen.
 *
 * Renders exactly what the API returned. Nothing here derives a confidence, a
 * streak or a trajectory point — including which points are estimated, which
 * is a server decision precisely so a client cannot quietly present a modeled
 * value as a measured one.
 */
export function Calibrating() {
  const { session } = useSession();
  const [data, setData] = useState<CalibrationResponse | null>(null);
  const [error, setError] = useState<string | null>(null);
  /** The 409 the API answers before anything has been synced. It is a state
   *  to explain, not a failure to apologise for, so it is kept apart from a
   *  real error. */
  const [needsSync, setNeedsSync] = useState(false);
  /** Fetched separately, because it is the only part of this screen that
   *  needs the benchmark provider. A `null` here empties one card; it never
   *  costs the rank, the trajectory or the momentum curve. */
  const [brackets, setBrackets] = useState<RankDistribution | null>(null);

  useEffect(() => {
    if (session.kind !== "signed-in") return;
    let cancelled = false;

    getCalibration()
      .then((response) => {
        if (!cancelled) setData(response);
      })
      .catch((e: unknown) => {
        if (cancelled) return;

        if (e instanceof ApiError && e.code === "PRECONDITION_UNMET") {
          setNeedsSync(true);
          return;
        }
        setError(
          e instanceof ApiError ? e.message : "Could not load your rank.",
        );
      });

    getRankDistribution()
      .then((response) => {
        if (!cancelled) setBrackets(response);
      })
      // Deliberately swallowed: this panel is the optional one. Its absence is
      // rendered as its absence, not as a failure of the page around it.
      .catch(() => undefined);

    return () => {
      cancelled = true;
    };
  }, [session.kind]);

  if (session.kind === "loading") return <PageSkeleton />;
  if (session.kind === "anonymous") return <SignedOut />;
  if (session.kind === "error") {
    return <Alert title="Cannot reach the service">{session.message}</Alert>;
  }

  // The panels below need a calibration; the match history does not. It reads
  // its own endpoint, so a rank that cannot be calibrated yet — or a calibration
  // that failed outright — must not take the player's games down with it.
  if (needsSync) {
    return (
      <div className="flex flex-col gap-4">
        <Alert title="Nothing to calibrate yet">
          Sync your matches from the overview, and your rank history starts
          building from the next one.
        </Alert>
        <MatchHistoryCard />
      </div>
    );
  }
  if (error) {
    return (
      <div className="flex flex-col gap-4">
        <Alert>{error}</Alert>
        <MatchHistoryCard />
      </div>
    );
  }
  if (!data) return <PageSkeleton />;

  return (
    <div className="flex flex-col gap-4">
      <RankCard
        rank={data.established_rank}
        confidence={data.confidence}
        thresholdPct={data.methodology.confidence_threshold_pct}
      />

      <Card className="flex flex-col gap-4">
        <div>
          <p className="font-semibold text-ink">Rank over time</p>
          <p className="mt-1 text-sm text-ink-muted">
            Recorded readings, with the stretches between them modeled.
          </p>
        </div>

        <TrajectoryChart points={data.trajectory} />

        <MethodologyNote methodology={data.methodology} />
      </Card>

      <Card className="flex flex-col gap-4">
        <div>
          <p className="font-semibold text-ink">Recent momentum</p>
          <p className="mt-1 text-sm text-ink-muted">
            Modeled movement across your last {data.momentum.window} ranked
            matches, relative to where the window started.
          </p>
        </div>

        <MomentumChart momentum={data.momentum} />
      </Card>

      {brackets ? (
        <Card>
          <EstablishedRankPanel
            rank={data.established_rank}
            confidence={data.confidence}
            consistency={brackets.consistency}
            metrics={brackets.own_bracket_metrics}
            resemblancePct={
              brackets.resemblance.find((r) => r.is_player_bracket)
                ?.percentage ?? null
            }
          />
        </Card>
      ) : null}

      {brackets ? (
        <Card className="flex flex-col gap-4">
          <div>
            <p className="font-semibold text-ink">Rank distribution</p>
            <p className="mt-1 text-sm text-ink-muted">
              How closely your {brackets.hero_name} figures resemble each
              bracket&rsquo;s real peers.
            </p>
          </div>

          <RankDistributionBars distribution={brackets} />
        </Card>
      ) : null}

      <div className="grid grid-cols-1 gap-4 sm:grid-cols-2">
        <Card className="flex flex-col gap-4">
          <p className="font-semibold text-ink">Current streak</p>
          <StreakBadge streak={data.streak} />
        </Card>

        <Card className="flex flex-col gap-4">
          <p className="font-semibold text-ink">Roles you queue</p>
          <RolePreferenceBars roles={data.role_preference} />
        </Card>
      </div>

      <MatchHistoryCard />
    </div>
  );
}

/**
 * The games behind everything above.
 *
 * Deliberately at the bottom: the screen answers "where does my rank sit" first
 * and "which games got me here" second. The MMR column is the same modeled
 * estimate the momentum curve plots, and the note says so — a table of precise
 * numbers is exactly where a disclosed model would otherwise start reading as
 * Valve's own.
 *
 * `Suspense` is not optional here: the table reads the page number from the URL
 * with `useSearchParams`, and Next requires a boundary around a component that
 * does so on a prerendered page.
 */
function MatchHistoryCard() {
  return (
    <Card className="flex flex-col gap-4">
      <div>
        <p className="font-semibold text-ink">Match history</p>
        <p className="mt-1 text-sm text-ink-muted">
          Every game we have stored. Rating compares each match against your own
          usual game on that hero; the MMR column is the same estimate as the
          momentum curve, not a figure Valve publishes.
        </p>
      </div>

      <Suspense fallback={<TableFallback />}>
        <MatchHistoryTable />
      </Suspense>
    </Card>
  );
}

function TableFallback() {
  return (
    <div className="h-64 animate-pulse rounded bg-surface-2/60" aria-busy="true">
      <span className="sr-only">Loading matches…</span>
    </div>
  );
}

"use client";

import { useEffect, useState } from "react";

import { Paywall } from "@/components/billing/Paywall";
import { MethodologyNote } from "@/components/calibrating/MethodologyNote";
import { MomentumCard } from "@/components/calibrating/MomentumChart";
import { BracketRadar, RankBreakdown } from "@/components/calibrating/RankBreakdown";
import { RankHeader } from "@/components/calibrating/RankHeader";
import { TrajectoryChart } from "@/components/calibrating/TrajectoryChart";
import { RecentMatches } from "@/components/dashboard/RecentMatches";
import { SignedOut } from "@/components/shell/SignedOut";
import { Alert } from "@/components/ui/Alert";
import { ChartCard } from "@/components/ui/ChartCard";
import { ApiError, getCalibration, getMatches, getRankDistribution } from "@/lib/api";
import { useSession } from "@/lib/session-context";
import type { CalibrationResponse, MatchView, RankDistribution } from "@/lib/types";

/**
 * The Rank page.
 *
 * Renders exactly what the API returned. Nothing here derives a confidence, a
 * streak or a trajectory point — including which points are estimated, which
 * is a server decision so a client cannot present a modeled value as a
 * measured one. All methodology lives in one (i), on Rank over time.
 */
export function Calibrating() {
  const { session } = useSession();
  const [data, setData] = useState<CalibrationResponse | null>(null);
  const [error, setError] = useState<string | null>(null);
  /** The 409 before anything has been synced: a state to explain. */
  const [needsSync, setNeedsSync] = useState(false);
  const [paywalled, setPaywalled] = useState(false);
  /** Needs the benchmark provider; null empties one card, never the page. */
  const [brackets, setBrackets] = useState<RankDistribution | null>(null);
  const [recent, setRecent] = useState<MatchView[] | null>(null);

  useEffect(() => {
    // A failed profile read is reported once by the shell; this page
    // still loads its own data.
    if (session.kind === "loading" || session.kind === "anonymous") return;
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
        if (e instanceof ApiError && e.isPaymentRequired) {
          setPaywalled(true);
          return;
        }
        setError(e instanceof ApiError ? e.message : "Could not load your rank.");
      });

    // Both optional: their absence is drawn as their absence.
    getRankDistribution()
      .then((response) => {
        if (!cancelled) setBrackets(response);
      })
      .catch(() => undefined);
    getMatches(1, 5, { mode: "ranked" })
      .then((response) => {
        if (!cancelled) setRecent(response.matches);
      })
      .catch(() => undefined);

    return () => {
      cancelled = true;
    };
  }, [session.kind]);

  if (session.kind === "loading") return <RankSkeleton />;
  if (session.kind === "anonymous") return <SignedOut />;

  const recentCard =
    recent && recent.length > 0 ? (
      <RecentMatches matches={recent} title="Recent ranked" seeAll="See all in Matches →" />
    ) : null;

  // The rank needs a calibration; the recent games do not, so a rank that
  // cannot be calibrated yet never takes them down with it.
  if (needsSync) {
    return (
      <div className="flex flex-col gap-4">
        <Alert title="Nothing to calibrate yet">Sync your matches from the overview first.</Alert>
        {recentCard}
      </div>
    );
  }
  if (paywalled) {
    return (
      <div className="flex flex-col gap-4">
        <Paywall message="Rank calibration needs an active trial or subscription." />
        {recentCard}
      </div>
    );
  }
  if (error) {
    return (
      <div className="flex flex-col gap-4">
        <Alert>{error}</Alert>
        {recentCard}
      </div>
    );
  }
  if (!data) return <RankSkeleton />;

  return (
    <div className="flex flex-col gap-4">
      <RankHeader data={data} />

      <ChartCard title="Rank over time" info={<MethodologyNote methodology={data.methodology} />}>
        <TrajectoryChart points={data.trajectory} />
      </ChartCard>

      <MomentumCard momentum={data.momentum} />

      {brackets ? <BracketRadar brackets={brackets} /> : null}

      <RankBreakdown data={data} />

      {recentCard}
    </div>
  );
}

/** Shaped like the page: header tiles, two wide charts, a row of three. */
function RankSkeleton() {
  return (
    <div className="flex flex-col gap-4" aria-busy="true" aria-live="polite">
      <span className="sr-only">Loading your rank…</span>
      <div className="grid grid-cols-2 gap-3 lg:grid-cols-5">
        {[0, 1, 2, 3, 4].map((i) => (
          <div key={i} className="glass h-28 animate-pulse rounded-card" />
        ))}
      </div>
      <div className="glass h-[22rem] animate-pulse rounded-card" />
      <div className="glass h-72 animate-pulse rounded-card" />
      <div className="grid gap-4 lg:grid-cols-3">
        {[0, 1, 2].map((i) => (
          <div key={i} className="glass h-48 animate-pulse rounded-card" />
        ))}
      </div>
    </div>
  );
}

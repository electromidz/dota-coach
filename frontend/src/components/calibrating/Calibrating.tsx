"use client";

import { useEffect, useState } from "react";

import { MethodologyNote } from "@/components/calibrating/MethodologyNote";
import { MomentumChart } from "@/components/calibrating/MomentumChart";
import { RankCard } from "@/components/calibrating/RankCard";
import { RolePreferenceBars } from "@/components/calibrating/RolePreferenceBars";
import { StreakBadge } from "@/components/calibrating/StreakBadge";
import { TrajectoryChart } from "@/components/calibrating/TrajectoryChart";
import { PageSkeleton } from "@/components/shell/PageSkeleton";
import { SignedOut } from "@/components/shell/SignedOut";
import { Alert } from "@/components/ui/Alert";
import { Card } from "@/components/ui/Card";
import { ApiError, getCalibration } from "@/lib/api";
import { useSession } from "@/lib/session-context";
import type { CalibrationResponse } from "@/lib/types";

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

    return () => {
      cancelled = true;
    };
  }, [session.kind]);

  if (session.kind === "loading") return <PageSkeleton />;
  if (session.kind === "anonymous") return <SignedOut />;
  if (session.kind === "error") {
    return <Alert title="Cannot reach the service">{session.message}</Alert>;
  }

  if (needsSync) {
    return (
      <Alert title="Nothing to calibrate yet">
        Sync your matches from the overview, and your rank history starts
        building from the next one.
      </Alert>
    );
  }
  if (error) return <Alert>{error}</Alert>;
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
    </div>
  );
}

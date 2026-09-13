"use client";

import { useCallback, useEffect, useState } from "react";

import { MatchCard } from "@/components/matches/MatchCard";
import { SignedOut } from "@/components/shell/SignedOut";
import { Alert } from "@/components/ui/Alert";
import { Button } from "@/components/ui/Button";
import { Card } from "@/components/ui/Card";
import { Icon } from "@/components/ui/Icon";
import { ApiError, getMatches } from "@/lib/api";
import type { MatchListResponse } from "@/lib/types";
import { useSession } from "@/lib/session-context";

const PAGE_SIZE = 20;

export function MatchList() {
  const { session } = useSession();
  const [data, setData] = useState<MatchListResponse | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [page, setPage] = useState(1);
  const [loading, setLoading] = useState(false);

  const load = useCallback(async (target: number) => {
    setLoading(true);
    try {
      setData(await getMatches(target, PAGE_SIZE));
      setError(null);
    } catch (e) {
      setError(
        e instanceof ApiError ? e.message : "Could not load your match history.",
      );
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    if (session.kind === "signed-in") void load(1);
  }, [session.kind, load]);

  async function goTo(target: number) {
    setPage(target);
    await load(target);
    // A new page starts at the top, the way a native list does.
    window.scrollTo({ top: 0, behavior: "smooth" });
  }

  if (session.kind === "loading") return <ListSkeleton />;
  if (session.kind === "anonymous") return <SignedOut />;
  if (session.kind === "error") {
    return <Alert title="Cannot reach the service">{session.message}</Alert>;
  }

  if (error) return <Alert>{error}</Alert>;
  if (!data) return <ListSkeleton />;

  if (data.matches.length === 0) {
    return (
      <Card>
        <p className="text-sm leading-relaxed text-ink-muted">
          No matches stored yet. Sync from the Overview tab to pull your recent
          games.
        </p>
      </Card>
    );
  }

  return (
    <div className="flex flex-col gap-4 pb-4">
      <p className="font-mono text-xs tabular-nums text-ink-faint">
        {data.total} matches stored
      </p>

      <ul className="flex flex-col gap-3">
        {data.matches.map((match, i) => (
          <MatchCard key={match.id} match={match} index={i} />
        ))}
      </ul>

      {data.total_pages > 1 ? (
        <nav
          aria-label="Match history pages"
          className="flex items-center justify-between gap-3 pt-1"
        >
          <Button
            variant="ghost"
            disabled={page <= 1 || loading}
            onClick={() => goTo(page - 1)}
            className="px-4 text-sm"
          >
            <Icon name="chevron-left" className="size-4" />
            Prev
          </Button>

          <span className="font-mono text-xs tabular-nums text-ink-faint">
            {page} / {data.total_pages}
          </span>

          <Button
            variant="ghost"
            disabled={page >= data.total_pages || loading}
            onClick={() => goTo(page + 1)}
            className="px-4 text-sm"
          >
            Next
            <Icon name="chevron-right" className="size-4" />
          </Button>
        </nav>
      ) : null}
    </div>
  );
}

function ListSkeleton() {
  return (
    <div className="flex flex-col gap-3" aria-busy="true" aria-live="polite">
      <span className="sr-only">Loading matches…</span>
      {[0, 1, 2, 3, 4].map((i) => (
        <div key={i} className="h-[7.5rem] animate-pulse rounded-card bg-surface-2" />
      ))}
    </div>
  );
}

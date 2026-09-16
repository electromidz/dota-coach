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
import { cn } from "@/lib/utils";

const PAGE_SIZE = 20;

export function MatchList() {
  const { session } = useSession();
  const [data, setData] = useState<MatchListResponse | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [page, setPage] = useState(1);
  const [loading, setLoading] = useState(false);
  /** Everything the player played, or only what the coach reads. */
  const [scope, setScope] = useState<"all" | "competitive">("all");

  const load = useCallback(
    async (target: number, listScope: "all" | "competitive") => {
      setLoading(true);
      try {
        setData(await getMatches(target, PAGE_SIZE, listScope));
        setError(null);
      } catch (e) {
        setError(
          e instanceof ApiError
            ? e.message
            : "Could not load your match history.",
        );
      } finally {
        setLoading(false);
      }
    },
    [],
  );

  useEffect(() => {
    if (session.kind === "signed-in") void load(1, "all");
  }, [session.kind, load]);

  async function goTo(target: number) {
    setPage(target);
    await load(target, scope);
    // A new page starts at the top, the way a native list does.
    window.scrollTo({ top: 0, behavior: "smooth" });
  }

  async function switchScope(next: "all" | "competitive") {
    setScope(next);
    // Page one: the second page of one population is not the second page of
    // the other, and silently keeping the number would land the reader
    // somewhere arbitrary.
    setPage(1);
    await load(1, next);
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
      <Card className="flex flex-col gap-3">
        <p className="text-sm leading-relaxed text-ink-muted">
          {scope === "competitive"
            ? "None of your stored matches are Ranked or public All Pick, so there is nothing here for the coach to read."
            : "No matches stored yet. Sync from the Overview tab to pull your recent games."}
        </p>
        {scope === "competitive" ? (
          <Button
            variant="ghost"
            onClick={() => void switchScope("all")}
            className="self-start px-4 text-sm"
          >
            Show everything you played
          </Button>
        ) : null}
      </Card>
    );
  }

  return (
    <div className="flex flex-col gap-4 pb-4">
      <div className="flex flex-col gap-2">
        <nav aria-label="Match population" className="flex flex-wrap gap-2">
          <ScopeTab
            label="Everything you played"
            active={scope === "all"}
            onClick={() => void switchScope("all")}
          />
          <ScopeTab
            label="What the coach reads"
            active={scope === "competitive"}
            onClick={() => void switchScope("competitive")}
          />
        </nav>

        <p
          className="font-mono text-xs tabular-nums text-ink-faint"
          aria-live="polite"
        >
          {data.total} {data.total === 1 ? "match" : "matches"}
          {scope === "competitive"
            ? " · Ranked and public All Pick only"
            : " stored · every mode"}
        </p>
      </div>

      {/* One column on a phone, because a match card is already dense. Two and
          then three once the shell widens, so a 20-match page is one screen
          instead of five.
          These track the *column's* width, not the window's: below `lg` the
          shell is still `max-w-lg`, so an earlier split would put two cards
          into 512px. */}
      <ul className="grid gap-3 lg:grid-cols-2 xl:grid-cols-3 xl:gap-4">
        {data.matches.map((match, i) => (
          <MatchCard key={match.id} match={match} index={i} />
        ))}
      </ul>

      {data.total_pages > 1 ? (
        <nav
          aria-label="Match history pages"
          className="flex items-center justify-between gap-3 pt-1 lg:justify-center lg:gap-6"
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

function ScopeTab({
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

function ListSkeleton() {
  return (
    <div
      className="grid gap-3 lg:grid-cols-2 xl:grid-cols-3 xl:gap-4"
      aria-busy="true"
      aria-live="polite"
    >
      <span className="sr-only">Loading matches…</span>
      {[0, 1, 2, 3, 4, 5].map((i) => (
        <div key={i} className="glass h-[7.5rem] animate-pulse rounded-card" />
      ))}
    </div>
  );
}

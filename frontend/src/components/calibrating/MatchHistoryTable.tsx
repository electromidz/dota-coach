"use client";

import { useCallback, useEffect, useState } from "react";
import { useRouter, useSearchParams } from "next/navigation";

import { MatchHistoryRow } from "@/components/calibrating/MatchHistoryRow";
import { Pagination } from "@/components/calibrating/Pagination";
import { Alert } from "@/components/ui/Alert";
import { Icon } from "@/components/ui/Icon";
import { ApiError, getMatches, syncMatches } from "@/lib/api";
import type { MatchesMode, MatchListResponse } from "@/lib/types";
import { cn, timeAgo } from "@/lib/utils";

const PAGE_SIZE = 20;

/** How long to wait before re-reading a page the server said it is refreshing. */
const SYNC_POLL_MS = 5_000;

const MODES: { value: MatchesMode; label: string }[] = [
  { value: "all", label: "All" },
  { value: "ranked", label: "Ranked" },
  { value: "turbo", label: "Turbo" },
];

/**
 * The rank tab's match history.
 *
 * Every figure in it is the server's — the rating, the estimated delta, the
 * totals and which games count as ranked. This component fetches, pages and
 * renders; it computes nothing a number could be argued with.
 *
 * The page lives in the URL, so a row someone wants to come back to survives a
 * reload and can be linked. A mode is not in the URL: it is a glance, and
 * carrying it would make `?page=4` mean different rows depending on what was
 * selected when the link was copied.
 */
export function MatchHistoryTable() {
  const router = useRouter();
  const searchParams = useSearchParams();
  const page = pageFrom(searchParams.get("page"));

  const [mode, setMode] = useState<MatchesMode>("all");
  /** Bumped to re-run the fetch without changing what is being asked for. */
  const [reload, setReload] = useState(0);
  /** Set when a refresh was refused or failed; the table itself still stands. */
  const [refreshNote, setRefreshNote] = useState<string | null>(null);
  const [refreshing, setRefreshing] = useState(false);
  /** Whether the one look back after a background sync has already happened. */
  const [polled, setPolled] = useState(false);

  /** Everything the list is a function of, as one value. */
  const asked = `${page}|${mode}|${reload}`;
  /** What is on screen, and which request produced it. */
  const [shown, setShown] = useState<{
    asked: string;
    data: MatchListResponse;
  } | null>(null);
  const [error, setError] = useState<ApiError | string | null>(null);

  const data = shown?.data ?? null;
  // Derived rather than a third piece of state: "displaying something older than
  // what was asked for" *is* what loading means here, and a flag would be a
  // second answer to the same question that can disagree with this one.
  const loading = shown?.asked !== asked && error === null;

  useEffect(() => {
    let cancelled = false;

    getMatches(page, PAGE_SIZE, "all", { mode })
      .then((response) => {
        if (cancelled) return;
        setShown({ asked, data: response });
        setError(null);
      })
      .catch((e: unknown) => {
        if (cancelled) return;
        setError(e instanceof ApiError ? e : "Could not load your matches.");
      });

    // Guards against a slower earlier response landing after a newer one, which
    // is what a quick run through the mode tabs produces.
    return () => {
      cancelled = true;
    };
  }, [page, mode, asked]);

  // The server refreshes stale history behind the response, so the rows just
  // served are the old ones. Exactly one look back, not a poll: a sync that is
  // still running would otherwise have this component re-reading every five
  // seconds for as long as the tab is open, and Refresh is right there.
  useEffect(() => {
    if (!data?.syncing || polled) return;

    const timer = setTimeout(() => {
      setPolled(true);
      setReload((n) => n + 1);
    }, SYNC_POLL_MS);

    return () => clearTimeout(timer);
  }, [data?.syncing, polled]);

  const goTo = useCallback(
    (target: number) => {
      const params = new URLSearchParams(searchParams);
      if (target <= 1) {
        params.delete("page");
      } else {
        params.set("page", String(target));
      }

      const query = params.toString();
      router.replace(query ? `?${query}` : "?", { scroll: false });
    },
    [router, searchParams],
  );

  function changeMode(next: MatchesMode) {
    setMode(next);
    // Page four of the ranked list is not page four of the Turbo one.
    goTo(1);
  }

  async function refresh() {
    setRefreshing(true);
    setRefreshNote(null);

    try {
      await syncMatches();
      setReload((n) => n + 1);
    } catch (e) {
      // A refused refresh is not a broken page. The backend says how long to
      // wait, or that the profile is not readable; both are worth showing
      // verbatim above a table that still works.
      setRefreshNote(
        e instanceof ApiError ? e.message : "Could not refresh right now.",
      );
    } finally {
      setRefreshing(false);
    }
  }

  if (error) return <ErrorState error={error} />;

  const from = data && data.total > 0 ? (page - 1) * data.limit + 1 : 0;
  const to = data ? Math.min(page * data.limit, data.total) : 0;

  return (
    <div className="flex flex-col gap-3">
      <div className="flex flex-wrap items-center justify-between gap-2">
        <nav aria-label="Match kind" className="flex gap-1.5">
          {MODES.map((option) => (
            <button
              key={option.value}
              type="button"
              onClick={() => changeMode(option.value)}
              aria-current={mode === option.value ? "true" : undefined}
              className={cn(
                "focus-neon min-h-9 cursor-pointer rounded-lg border px-2.5 text-xs",
                "transition-colors duration-200 ease-out",
                mode === option.value
                  ? "border-function/60 bg-function/10 text-ink"
                  : "border-glass-edge text-ink-muted hover:text-ink",
              )}
            >
              {option.label}
            </button>
          ))}
        </nav>

        <div className="flex items-center gap-2">
          {data?.last_synced_at ? (
            <span className="font-mono text-[0.625rem] tabular-nums text-ink-faint">
              synced {timeAgo(data.last_synced_at)}
            </span>
          ) : null}
          <button
            type="button"
            onClick={refresh}
            disabled={refreshing}
            className={cn(
              "focus-neon flex min-h-9 cursor-pointer items-center gap-1.5 rounded-lg border border-glass-edge",
              "px-2.5 text-xs text-ink-muted transition-colors duration-200 ease-out hover:text-ink",
              "disabled:cursor-not-allowed disabled:opacity-60",
            )}
          >
            <Icon
              name="refresh"
              className={cn("size-3.5", refreshing && "animate-spin")}
            />
            Refresh
          </button>
        </div>
      </div>

      {refreshNote ? (
        <p className="text-xs text-number" aria-live="polite">
          {refreshNote}
        </p>
      ) : null}

      <p
        className="font-mono text-xs tabular-nums text-ink-faint"
        aria-live="polite"
      >
        {data
          ? data.total === 0
            ? "No matches to display"
            : `Displaying games ${from}–${to} of ${data.total} valid matches`
          : "Loading matches…"}
        {data?.syncing ? " · checking for newer games" : ""}
      </p>

      {/* A header row above nothing is furniture, so an empty page drops the
          table and keeps only the explanation below.
          Its own scroll container: a wide table must never make the page scroll
          sideways. */}
      <div
        className={cn(
          "-mx-1 overflow-x-auto",
          data && data.total === 0 && "hidden",
        )}
      >
        <table className="w-full min-w-[34rem] border-collapse text-left">
          <thead>
            <tr className="text-[0.625rem] uppercase tracking-wider text-ink-faint">
              <Th className="pl-1">Hero</Th>
              <Th>Result</Th>
              <Th>K / D / A</Th>
              <Th>GPM</Th>
              <Th className="hidden md:table-cell">XPM</Th>
              <Th>Time</Th>
              <Th>Rating</Th>
              <Th>MMR</Th>
              <Th className="hidden md:table-cell">Match</Th>
            </tr>
          </thead>

          {/* The previous page stays on screen, dimmed, while the next loads: a
              skeleton on every page step would lose the reader's place. */}
          <tbody className={cn(loading && data && "opacity-60 transition-opacity")}>
            {data ? (
              data.matches.map((match) => (
                <MatchHistoryRow key={match.id} match={match} />
              ))
            ) : (
              <SkeletonRows />
            )}
          </tbody>
        </table>
      </div>

      {data && data.total === 0 ? <EmptyState mode={mode} /> : null}

      {data ? (
        <Pagination
          page={page}
          totalPages={data.total_pages}
          disabled={loading}
          onChange={goTo}
        />
      ) : null}
    </div>
  );
}

/** A page number from the URL, ignoring anything that is not one. */
function pageFrom(value: string | null): number {
  const parsed = Number(value);
  return Number.isInteger(parsed) && parsed >= 1 ? parsed : 1;
}

function Th({
  children,
  className,
}: {
  children: React.ReactNode;
  className?: string;
}) {
  return (
    <th scope="col" className={cn("px-3 pb-2 font-normal", className)}>
      {children}
    </th>
  );
}

function SkeletonRows() {
  return (
    <>
      {Array.from({ length: PAGE_SIZE }, (_, i) => (
        <tr key={i} className="border-t border-glass-edge" aria-hidden>
          <td colSpan={9} className="py-2">
            <div className="h-6 animate-pulse rounded bg-surface-2/60" />
          </td>
        </tr>
      ))}
    </>
  );
}

function EmptyState({ mode }: { mode: MatchesMode }) {
  return (
    <p className="text-sm leading-relaxed text-ink-muted">
      {mode === "all"
        ? "No matches stored yet. Refresh above, or sync from the overview, to pull your recent games."
        : `No ${mode} games in your stored history. Try All.`}
    </p>
  );
}

/**
 * Why the table is not there.
 *
 * The private-profile case is separated out because it is the one failure the
 * player can fix themselves, and the fix is a specific setting with a specific
 * name. A generic "could not load" would leave them nowhere.
 */
function ErrorState({ error }: { error: ApiError | string }) {
  const code = error instanceof ApiError ? error.code : null;

  if (code === "NOT_FOUND" || code === "UPSTREAM_UNAVAILABLE") {
    return (
      <Alert title="We cannot read your match history">
        Dota hides a player&rsquo;s matches unless they allow it. In Dota 2, open
        Settings → Options → Advanced Options and turn on{" "}
        <strong className="text-ink">Expose Public Match Data</strong>, then
        refresh here.
      </Alert>
    );
  }

  if (code === "PRECONDITION_UNMET") {
    return (
      <Alert title="Nothing synced yet">
        Sync your matches from the overview and this table fills in from your
        next game onwards.
      </Alert>
    );
  }

  return <Alert>{error instanceof ApiError ? error.message : error}</Alert>;
}

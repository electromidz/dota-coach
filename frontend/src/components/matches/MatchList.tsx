"use client";

import { useEffect, useState } from "react";

import { MatchCard } from "@/components/matches/MatchCard";
import { SignedOut } from "@/components/shell/SignedOut";
import { Alert } from "@/components/ui/Alert";
import { Button } from "@/components/ui/Button";
import { Card } from "@/components/ui/Card";
import { Icon } from "@/components/ui/Icon";
import { Combobox } from "@/components/ui/Combobox";
import { InfoTip } from "@/components/ui/InfoTip";
import { Segmented } from "@/components/ui/Segmented";
import { ApiError, getMatches } from "@/lib/api";
import type {
  MatchesMode,
  CoachableRole,
  FilterOption,
  MatchListResponse,
  MatchResultFilter,
  MatchSort,
} from "@/lib/types";
import { useSession } from "@/lib/session-context";
import { cn } from "@/lib/utils";

const PAGE_SIZE = 20;

/**
 * The orderings offered, and the labels they carry.
 *
 * Fewer than the match row has numeric fields, deliberately: each of these
 * answers a question a player asks of their own history. Win/loss is absent
 * because it is the Result *filter* — sorting by a field you can filter on
 * shows the same rows in a worse order.
 */
const SORTS: { value: MatchSort; label: string }[] = [
  { value: "newest", label: "Newest first" },
  { value: "oldest", label: "Oldest first" },
  { value: "gpm_desc", label: "Highest GPM" },
  { value: "gpm_asc", label: "Lowest GPM" },
  { value: "kda_desc", label: "Highest KDA" },
];

const RESULTS: { value: MatchResultFilter; label: string }[] = [
  { value: "all", label: "All results" },
  { value: "win", label: "Wins" },
  { value: "loss", label: "Losses" },
];

const MODES: { value: MatchesMode; label: string }[] = [
  { value: "eligible", label: "Ranked + All Pick" },
  { value: "turbo", label: "Turbo" },
  { value: "all", label: "All" },
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
  const [data, setData] = useState<MatchListResponse | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [page, setPage] = useState(1);
  const [loading, setLoading] = useState(false);
  /** Which games: Ranked + All Pick by default. Mirrored in `?mode=`. */
  const [mode, setModeState] = useState<MatchesMode>(modeFromUrl);

  /** Filters. Each is the value the backend takes; `undefined` means "any". */
  const [heroId, setHeroId] = useState<number | undefined>();
  const [role, setRole] = useState<CoachableRole | undefined>();
  const [result, setResult] = useState<MatchResultFilter>("all");
  const [sort, setSort] = useState<MatchSort>("newest");

  // One effect for every input the list depends on: filtering, sorting and
  // paging are all server-side, so each change is exactly one request and the
  // `total`/`total_pages` on screen always describe the list on screen.
  //
  // The previous page stays rendered while the next one loads — dimmed rather
  // than replaced by a skeleton, because a filter change that blanked the
  // screen would lose the reader's place on every keystroke of a dropdown.
  useEffect(() => {
    if (session.kind !== "signed-in") return;
    let cancelled = false;

    setLoading(true);
    getMatches(page, PAGE_SIZE, { mode, heroId, role, result, sort })
      .then((response) => {
        if (cancelled) return;
        setData(response);
        setError(null);
      })
      .catch((e) => {
        if (cancelled) return;
        setError(
          e instanceof ApiError
            ? e.message
            : "Could not load your match history.",
        );
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });

    // Guards against an out-of-order response overwriting a newer one, which
    // is the failure a fast sequence of filter changes produces.
    return () => {
      cancelled = true;
    };
  }, [session.kind, page, mode, heroId, role, result, sort]);

  if (session.kind === "loading") return <ListSkeleton />;
  if (session.kind === "anonymous") return <SignedOut />;
  if (session.kind === "error") {
    return <Alert title="Cannot reach the service">{session.message}</Alert>;
  }

  if (error) return <Alert>{error}</Alert>;
  if (!data) return <ListSkeleton />;

  /** Anything that narrows the list. Sort is not a filter — it reorders. */
  const filtered = heroId !== undefined || role !== undefined || result !== "all";

  function goTo(target: number) {
    setPage(target);
    // A new page starts at the top, the way a native list does.
    window.scrollTo({ top: 0, behavior: "smooth" });
  }

  /** Every filter change lands on page one: page four of one list is not page
   *  four of another, and keeping the number would land the reader nowhere. */
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
    setPage(1);
  }

  const { heroes, roles } = data.filters;

  return (
    <div className="flex flex-col gap-4 pb-4">
      <div className="flex flex-col gap-3">
        <Segmented
          label="Which games"
          value={mode}
          onChange={setMode}
          options={MODES}
        />

        {/* Two-up on a phone so four controls cost one thumb-height rather
            than four; a single row once there is width for it. */}
        <div
          role="group"
          aria-label="Filter and sort matches"
          className="grid grid-cols-2 gap-2 sm:flex sm:flex-wrap sm:items-end"
        >
          <Field label="Hero">
            <Combobox
              label="Hero"
              anyLabel="All heroes"
              value={heroId === undefined ? undefined : String(heroId)}
              onChange={(value) => {
                setHeroId(value === undefined ? undefined : Number(value));
                setPage(1);
              }}
              options={heroes
                .filter((hero) => hero.matches > 0)
                .sort((a, b) => b.matches - a.matches)
                .map((hero) => ({
                  value: hero.value,
                  label: hero.label,
                  hint: String(hero.matches),
                }))}
            />
          </Field>

          <Field label="Role">
            <Select
              value={role ?? ""}
              onChange={change((value: string) =>
                setRole(value === "" ? undefined : (value as CoachableRole)),
              )}
              placeholder="All roles"
              options={roles}
            />
          </Field>

          <Field label="Result">
            <NativeSelect
              value={result}
              onChange={change((value: string) =>
                setResult(value as MatchResultFilter),
              )}
            >
              {RESULTS.map((option) => (
                <option key={option.value} value={option.value}>
                  {option.label}
                </option>
              ))}
            </NativeSelect>
          </Field>

          <Field label="Sort">
            <NativeSelect
              value={sort}
              onChange={change((value: string) => setSort(value as MatchSort))}
            >
              {SORTS.map((option) => (
                <option key={option.value} value={option.value}>
                  {option.label}
                </option>
              ))}
            </NativeSelect>
          </Field>

          {filtered || sort !== "newest" ? (
            <Button
              variant="ghost"
              onClick={clearFilters}
              className="col-span-2 px-4 text-sm sm:col-span-1"
            >
              Clear filters
            </Button>
          ) : null}
        </div>

        <p
          className="flex items-center gap-1.5 font-mono text-xs tabular-nums text-ink-faint"
          aria-live="polite"
        >
          {filtered ? `${data.total} of ` : ""}
          {data.mode_total} {data.mode_total === 1 ? "match" : "matches"} ·{" "}
          {MODE_LABEL[mode]}
          {data.hidden_turbo + data.hidden_other > 0 ? (
            <InfoTip label="Games not shown">
              {data.hidden_turbo > 0 ? <span>+{data.hidden_turbo} Turbo hidden</span> : null}
              {data.hidden_other > 0 ? (
                <span>+{data.hidden_other} other games hidden (other modes or older)</span>
              ) : null}
            </InfoTip>
          ) : null}
        </p>
      </div>

      {data.matches.length === 0 ? (
        <EmptyState
          filtered={filtered}
          mode={mode}
          onClear={clearFilters}
          onShowEverything={() => setMode("all")}
        />
      ) : (
        <>
          {/* One column on a phone, because a match card is already dense. Two
              and then three once the shell widens, so a 20-match page is one
              screen instead of five.
              These track the *column's* width, not the window's: below `lg` the
              shell is still `max-w-lg`, so an earlier split would put two cards
              into 512px. */}
          <ul
            className={cn(
              "grid gap-3 lg:grid-cols-2 xl:grid-cols-3 xl:gap-4",
              loading && "opacity-60 transition-opacity",
            )}
          >
            {data.matches.map((match, i) => (
              <MatchCard key={match.id} match={match} index={i} mode={mode} />
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
        </>
      )}
    </div>
  );
}

/**
 * Nothing to show, and why.
 *
 * Three different reasons with three different answers: a filter that matched
 * nothing is undone, a competitive population with no eligible games is
 * widened, and an empty history is synced. Collapsing them into one message
 * would leave two of the three with no way forward.
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
  children,
}: {
  label: string;
  children: React.ReactNode;
}) {
  return (
    <label className="flex min-w-0 flex-col gap-1 text-xs text-ink-faint">
      {label}
      {children}
    </label>
  );
}

const SELECT_CLASS =
  "focus-neon min-h-11 w-full min-w-0 cursor-pointer rounded-xl border border-glass-edge " +
  "bg-surface-2/60 px-3 text-sm text-ink outline-none sm:w-auto";

function NativeSelect({
  value,
  onChange,
  children,
}: {
  value: string;
  onChange: (value: string) => void;
  children: React.ReactNode;
}) {
  return (
    <select
      value={value}
      onChange={(e) => onChange(e.target.value)}
      className={SELECT_CLASS}
    >
      {children}
    </select>
  );
}

/**
 * A select over backend-supplied options, with the count beside each.
 *
 * The counts are the point: they come from the player's own matches, so the
 * list never offers a hero they have not played, and a one-game hero is
 * visibly a one-game hero before it is picked.
 */
function Select({
  value,
  onChange,
  placeholder,
  options,
}: {
  value: string;
  onChange: (value: string) => void;
  placeholder: string;
  options: FilterOption[];
}) {
  return (
    <NativeSelect value={value} onChange={onChange}>
      <option value="">{placeholder}</option>
      {options.map((option) => (
        <option key={option.value} value={option.value}>
          {option.label} ({option.matches})
        </option>
      ))}
    </NativeSelect>
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

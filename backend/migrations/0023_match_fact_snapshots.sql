-- The timestamped record of one match, as one provider reported it.
--
-- A cache, not a source of truth — the same shape and the same reasoning as
-- `hero_meta_snapshots` and `benchmark_snapshots`. Dropping this table loses
-- nothing but provider calls.
--
-- Why it is cached at all, when `coaching_cache` already exists: that table
-- holds exactly one row per player (its writer prunes the player's other
-- entries), which is right for a single coaching context and wrong for a
-- growing set of per-match payloads. The two would evict each other on every
-- write.
--
-- Why it is worth caching:
--
--   * A finished match never changes. Fetching it twice returns the same bytes.
--   * STRATZ rate-limits per token, not per user — 2,000 calls an hour for the
--     whole deployment on a default token. Opening a match page must not spend
--     one, or a handful of users reading their own history exhausts the budget
--     for everybody.
--   * `GET /api/matches/:id/analysis` is a read that costs nothing by design.
--     A provider call on that path would make opening a match page fail when
--     the provider is down, which is exactly the coupling the phase forbids.
--
-- The payload is the *normalized* domain value, not the raw GraphQL response.
-- That differs from `hero_meta_snapshots`, deliberately: the raw response is
-- ten players' event streams for a match we only ever read one player out of,
-- and storing the other nine would be storing other people's match data we have
-- no use for. A parser change costs a re-fetch of one match, which is cheap.
--
-- Freshness is a predicate the caller supplies, because the right answer
-- depends on the row: a parsed match is immutable and may be reused for weeks,
-- while an unparsed one is re-asked within hours — Valve parses replays after
-- the fact, so "no timeline available" is a statement about now, and a match
-- that has since been parsed should gain its timeline without anyone
-- intervening. `parsed` is stored as a column rather than dug out of the JSON
-- so that decision is an index lookup.

CREATE TABLE match_fact_snapshots (
    id          UUID        PRIMARY KEY DEFAULT gen_random_uuid(),

    -- Which provider produced this. A second implementation's rows sit
    -- alongside these rather than overwriting them.
    provider    TEXT        NOT NULL,

    -- Valve's match id, not this application's `matches.id`. The payload is a
    -- fact about the Dota match, and it is keyed the way the provider keys it.
    -- Deliberately not a foreign key to `matches`: the analysis path can be
    -- asked about a match before the row exists, and a provider cache must not
    -- depend on our own sync having caught up.
    match_id    BIGINT      NOT NULL,

    -- The payload describes one player's view of the match, so the player is
    -- part of the identity. Also the provider's own identifier rather than ours,
    -- for the same reason `rank_snapshots` uses it.
    account_id  BIGINT      NOT NULL,

    -- False when the provider had no parsed replay. The whole point of the
    -- column: it is what decides whether this row may still be reused.
    parsed      BOOLEAN     NOT NULL,

    payload     JSONB       NOT NULL,
    fetched_at  TIMESTAMPTZ NOT NULL DEFAULT now(),

    -- One current snapshot per provider, match and player; a refresh replaces it.
    UNIQUE (provider, match_id, account_id)
);

-- The read path is "this provider's row for this match and this player", which
-- the unique constraint above already indexes. This one serves the sweep a
-- future operator will want: the oldest rows, regardless of who they belong to.
CREATE INDEX match_fact_snapshots_fetched_at_idx ON match_fact_snapshots (fetched_at);

-- Phase 6: hero meta snapshots.
--
-- Cache, not a source of truth — the same reasoning as `benchmark_snapshots`:
-- the meta for a rank bracket is identical for every user, so it is fetched
-- once and shared, and the raw provider payload is stored verbatim so a parser
-- change never requires a re-fetch.
--
-- Deliberately absent: `hero_pool` and `hero_recommendations` tables. Both are
-- pure functions of `matches` + `match_metrics` + this snapshot, so persisting
-- them would only create a second, staler answer. They are computed on read.

CREATE TABLE hero_meta_snapshots (
    id          UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    -- Which provider produced this. STRATZ rows sit alongside OpenDota ones.
    provider    TEXT        NOT NULL,
    -- The context the payload answers, serialized by the provider. OpenDota
    -- returns every bracket in one document and so uses a single key; a
    -- provider that segments by patch/role/window encodes that here instead of
    -- needing a migration per dimension.
    context_key TEXT        NOT NULL,
    payload     JSONB       NOT NULL,
    fetched_at  TIMESTAMPTZ NOT NULL DEFAULT now(),

    -- One current snapshot per provider and context; a refresh replaces it.
    UNIQUE (provider, context_key)
);

-- Phase 5: benchmark distributions.
--
-- Cache, not a source of truth. The distribution for a hero is identical for
-- every user, so it is fetched once and shared; a stale row is refreshed on
-- read rather than expired by a job.
--
-- The provider payload is stored verbatim. Parsing lives in Rust and can
-- change without a migration, and keeping the raw shape means a parser bug is
-- fixable without re-fetching everything.

CREATE TABLE benchmark_snapshots (
    id         UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    -- Which provider produced this. STRATZ rows can sit alongside OpenDota
    -- ones without collision.
    provider   TEXT        NOT NULL,
    hero_id    INT         NOT NULL,
    payload    JSONB       NOT NULL,
    fetched_at TIMESTAMPTZ NOT NULL DEFAULT now(),

    -- One current snapshot per provider and hero; a refresh replaces it.
    UNIQUE (provider, hero_id)
);

-- Reads are always "the current snapshot for this provider and hero, if it is
-- still fresh", which the unique constraint's index already serves.

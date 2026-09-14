-- Phase 8: the long-term player model.
--
-- Most of the model is derived: strengths, weaknesses, role affinity and form
-- are all pure functions of matches, metrics and the benchmark snapshot, so
-- they are computed on read rather than stored twice.
--
-- Patterns are different, and that difference is the reason this migration
-- exists. Two things about a pattern cannot be recomputed from the current
-- history:
--
--   * when this backend first noticed it, and
--   * that it used to be true and no longer is.
--
-- The second is the whole point of a model that "evolves as new matches
-- arrive" — a resolved pattern is invisible in the data that resolved it.

CREATE TABLE player_models (
    id              UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    -- One model per player; recomputation updates it in place.
    dota_player_id  UUID        NOT NULL UNIQUE REFERENCES dota_players (id) ON DELETE CASCADE,

    -- Bumped when a detector or trait rule changes, so a stored row is
    -- identifiable as having come from an older definition.
    model_version   INT         NOT NULL,
    -- What the model was built from. A change here is what makes it stale.
    matches_analyzed BIGINT     NOT NULL,
    -- sparse | developing | established. How far the model's claims carry.
    confidence      TEXT        NOT NULL,

    computed_at     TIMESTAMPTZ NOT NULL DEFAULT now(),
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE player_patterns (
    id              UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    dota_player_id  UUID        NOT NULL REFERENCES dota_players (id) ON DELETE CASCADE,

    -- Detector slug, e.g. `high_death_rate`. Stable by contract: it is stored,
    -- and renaming one would orphan its history.
    pattern_id      TEXT        NOT NULL,

    -- Matches the condition held in, and matches it could be *checked* in.
    -- Two denominators on purpose: most laning signals only exist on a parsed
    -- replay, and collapsing them would make two parsed games out of forty
    -- look like a habit.
    occurrences     BIGINT      NOT NULL,
    measured        BIGINT      NOT NULL,
    rate            REAL        NOT NULL,
    recent_rate     REAL,

    -- active | improving | resolved.
    status          TEXT        NOT NULL,

    -- Timestamps of the matches themselves.
    first_seen_at   TIMESTAMPTZ,
    last_seen_at    TIMESTAMPTZ,
    -- When this backend first recorded the pattern. Never overwritten, which
    -- is what lets the coach say how long something has been true.
    first_detected_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    -- Set when it stopped clearing the threshold, cleared if it returns.
    resolved_at     TIMESTAMPTZ,
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now(),

    UNIQUE (dota_player_id, pattern_id)
);

-- Reads are always "this player's patterns", usually filtered by status.
CREATE INDEX player_patterns_player_status_idx
    ON player_patterns (dota_player_id, status);

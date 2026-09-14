-- Phase 9: training focus.
--
-- Stored for two reasons, both of which a recompute-on-read design would lose:
--
--   * **Stability.** A focus recomputed on every request would change whenever
--     a match landed. That is a feed, not a training plan. The chosen focus is
--     written down and kept until it is finished or its evidence disappears.
--   * **Progress.** "Is this improving?" is measured against where the player
--     stood *when the focus was set*. That baseline only exists if it was
--     captured at the time.
--
-- The series itself is not stored: it is a pure function of matches, metrics
-- and the focus, so persisting it would only create a staler second answer.

CREATE TABLE training_focus (
    id              UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    dota_player_id  UUID        NOT NULL REFERENCES dota_players (id) ON DELETE CASCADE,

    -- Stable key — `pattern.high_death_rate`, `benchmark.gold_per_min`.
    focus_key       TEXT        NOT NULL,
    title           TEXT        NOT NULL,
    -- Why this one and not another, as composed at selection time.
    why             TEXT        NOT NULL,

    -- benchmark | pattern.
    source          TEXT        NOT NULL,
    -- A variant of domain::training::FocusMeasure.
    measure         TEXT        NOT NULL,
    -- Set only when `measure` is `pattern_rate`.
    pattern_id      TEXT,
    higher_is_better BOOLEAN    NOT NULL,

    -- Where the player stood when this was set, and what "done" is.
    baseline_value  REAL        NOT NULL,
    target_value    REAL        NOT NULL,
    -- The selection score, kept so a past choice can be explained later.
    score           REAL        NOT NULL,

    -- active | achieved | retired.
    status          TEXT        NOT NULL,
    started_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    -- Set when the focus stopped being active, either way.
    ended_at        TIMESTAMPTZ,
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- One active focus per player, enforced rather than hoped for: "one primary
-- training focus at a time" is the product rule this phase exists to keep.
-- A partial index, so the finished ones can accumulate as history.
CREATE UNIQUE INDEX training_focus_one_active_idx
    ON training_focus (dota_player_id)
    WHERE status = 'active';

-- The history read: "what has this player worked on", newest first.
CREATE INDEX training_focus_history_idx
    ON training_focus (dota_player_id, started_at DESC);

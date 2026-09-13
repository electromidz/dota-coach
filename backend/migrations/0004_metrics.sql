-- Phase 4: deterministic analytics.
--
-- Two layers, deliberately separated:
--
--   matches        raw facts as the provider reported them
--   match_metrics  values *derived* from those facts, version-stamped
--
-- Nothing here is ever computed by the LLM, and every derived value can be
-- recomputed from the row above it.

-- ---------------------------------------------------------------------------
-- Raw facts the metrics engine needs that Phase 3 did not capture.
-- ---------------------------------------------------------------------------

ALTER TABLE matches
    -- Team totals, for participation rates. Available on any match detail.
    ADD COLUMN team_kills   INT,
    ADD COLUMN team_deaths  INT,

    -- Time-sliced snapshots. These come from the per-minute arrays that only
    -- exist on *parsed* replays, so they stay NULL for most public matches
    -- rather than being guessed at.
    ADD COLUMN last_hits_at_10 INT,
    ADD COLUMN last_hits_at_15 INT,
    ADD COLUMN gold_at_10      INT,
    ADD COLUMN gold_at_15      INT,
    ADD COLUMN xp_at_10        INT,
    ADD COLUMN xp_at_15        INT,

    -- Item timings, in seconds from the horn. Parsed replays only; negative
    -- values in the provider's purchase log (pre-horn shopping) are discarded
    -- before they reach here.
    ADD COLUMN bkb_seconds   INT,
    ADD COLUMN blink_seconds INT,
    ADD COLUMN midas_seconds INT,

    -- 0-1, the provider's own measure. Parsed replays only.
    ADD COLUMN teamfight_participation REAL,

    -- True when the provider had a parsed replay, which is what gates every
    -- column above. Distinct from `detail_synced`, which only says the match
    -- detail endpoint answered.
    ADD COLUMN replay_parsed BOOLEAN NOT NULL DEFAULT FALSE;

-- ---------------------------------------------------------------------------
-- Derived metrics.
-- ---------------------------------------------------------------------------

CREATE TABLE match_metrics (
    id        UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    -- One row per match. Recomputing replaces it rather than appending.
    match_id  UUID NOT NULL UNIQUE REFERENCES matches (id) ON DELETE CASCADE,

    -- Bumped whenever a formula changes, so stale rows are identifiable and
    -- can be recomputed without guessing which definition produced them.
    metrics_version INT NOT NULL,

    -- Always computable from the summary.
    kda                  REAL NOT NULL,
    kills_per_10         REAL NOT NULL,
    deaths_per_10        REAL NOT NULL,
    assists_per_10       REAL NOT NULL,
    last_hits_per_min    REAL NOT NULL,
    hero_damage_per_min  REAL,
    tower_damage_per_min REAL,

    -- Needs team totals.
    kill_participation REAL,

    -- Needs a parsed replay.
    gold_advantage_at_10 REAL,

    computed_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Aggregations read "all metrics for this player's matches"; the join goes
-- through matches, so this index carries the lookup.
CREATE INDEX match_metrics_version_idx ON match_metrics (metrics_version);

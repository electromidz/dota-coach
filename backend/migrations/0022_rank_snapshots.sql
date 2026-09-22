-- Rank history: one row per player per day.
--
-- `dota_players.rank_tier` is a single mutable column — it holds where the
-- player stands now and nothing about where they stood. A trajectory chart
-- needs points over time, so the sync that refreshes the rank also writes it
-- down here.
--
-- Keyed by `dota_account_id`, not the `dota_players.id` UUID every other child
-- table uses. That is deliberate: the account id is the identifier the
-- provider itself uses, it is what `sync` already has in hand when the profile
-- comes back, and a rank history is a fact about the Dota account rather than
-- about this application's row for it. The column is `BIGINT NOT NULL UNIQUE`
-- (see 0001_init / 0003_auth), so it is a legal foreign key target.
CREATE TABLE rank_snapshots (
    id               UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    dota_account_id  BIGINT      NOT NULL
                                 REFERENCES dota_players (dota_account_id)
                                 ON DELETE CASCADE,

    -- Both nullable: a private profile reports no rank, and recording "we
    -- looked and there was nothing" is the honest row. A gap in the chart is
    -- correct where a fabricated carry-forward would not be.
    rank_tier        SMALLINT,
    leaderboard_rank INT,

    captured_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- At most one snapshot per player per calendar day. A player syncing six times
-- an afternoon should move the day's point, not stack six identical ones.
--
-- `AT TIME ZONE 'UTC'` is load-bearing, not decoration: casting a `timestamptz`
-- straight to `date` reads the session TimeZone setting, which makes the
-- expression STABLE rather than IMMUTABLE, and Postgres refuses it in an index
-- ("functions in index expression must be marked IMMUTABLE"). Pinning the zone
-- makes the cast immutable and makes "a day" mean the same thing regardless of
-- which connection wrote the row.
CREATE UNIQUE INDEX rank_snapshots_player_day_key
    ON rank_snapshots (dota_account_id, ((captured_at AT TIME ZONE 'UTC')::date));

-- The read path: one player's history, newest first.
CREATE INDEX idx_rank_snapshots_player
    ON rank_snapshots (dota_account_id, captured_at DESC);

-- Phase 2: player enrichment + synchronized match history.
--
-- Metrics, insights and profile tables arrive with Phases 3 and 5; this
-- migration only covers what the sync pipeline needs.

ALTER TABLE users
    ADD COLUMN avatar_url     TEXT,
    -- OpenDota rank_tier: tens digit = medal, ones digit = star.
    ADD COLUMN rank_tier      INT,
    ADD COLUMN last_synced_at TIMESTAMPTZ;

CREATE TABLE matches (
    id               UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id          UUID        NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    -- Dota's own match id, as reported by the provider.
    match_id         BIGINT      NOT NULL,

    hero_id          INT         NOT NULL,
    hero_name        TEXT        NOT NULL,
    -- Estimated from lane role and creep score; see domain::r#match::derive_role.
    role             TEXT        NOT NULL,
    lane_role        SMALLINT,

    won              BOOLEAN     NOT NULL,
    duration_seconds INT         NOT NULL CHECK (duration_seconds >= 0),

    kills            INT         NOT NULL DEFAULT 0,
    deaths           INT         NOT NULL DEFAULT 0,
    assists          INT         NOT NULL DEFAULT 0,
    gpm              INT         NOT NULL DEFAULT 0,
    xpm              INT         NOT NULL DEFAULT 0,
    last_hits        INT         NOT NULL DEFAULT 0,

    -- Only present once the full match detail has been fetched.
    denies           INT,
    net_worth        INT,
    hero_damage      INT,
    tower_damage     INT,
    hero_healing     INT,

    game_mode        INT,
    lobby_type       INT,
    party_size       INT,

    started_at       TIMESTAMPTZ NOT NULL,
    -- False when only the recent-matches summary was available.
    detail_synced    BOOLEAN     NOT NULL DEFAULT FALSE,

    created_at       TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at       TIMESTAMPTZ NOT NULL DEFAULT now(),

    -- One row per (player, Dota match): re-syncing is idempotent.
    UNIQUE (user_id, match_id)
);

-- Every read path is "this player's matches, newest first".
CREATE INDEX matches_user_started_at_idx ON matches (user_id, started_at DESC);

CREATE TRIGGER matches_set_updated_at
    BEFORE UPDATE ON matches
    FOR EACH ROW
    EXECUTE FUNCTION set_updated_at();

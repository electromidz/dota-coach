-- Phase 11: the coaching profile.
--
-- The one thing about coaching that cannot be derived: which role the player
-- chose to work on. Everything else in the coaching scope — which matches are
-- eligible, which of them are in that role, what the window contains — is a
-- pure function of the stored matches and is recomputed on every read.
--
-- So this table is deliberately small, and in particular it does **not** store
-- the match ids the coaching dataset resolves to. Those change every time the
-- player syncs; persisting them would create a second answer to "which matches
-- is this advice about", and the stale one would win whenever somebody read it.
--
-- `recommended_role` and `analyzed_matches` are the exception: they record what
-- the system advised and how much evidence it had *at the moment of choosing*.
-- Neither can be reconstructed later — the analysis moves as matches arrive —
-- and both are needed to answer "why am I being coached on this", including in
-- the case the product cares most about, where the player overrode the advice.

CREATE TABLE coaching_profiles (
    id               UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    -- One profile per player: a second selected role would make "the coaching
    -- scope" ambiguous, which is the whole thing this phase exists to fix.
    dota_player_id   UUID        NOT NULL UNIQUE
                                 REFERENCES dota_players (id) ON DELETE CASCADE,

    -- A variant of domain::role::CoachableRole, stored as its slug.
    selected_role    TEXT        NOT NULL,
    -- What the system recommended when this choice was made. NULL when it had
    -- nothing solid enough to recommend — which is a real state, not a missing
    -- value: the player chose with no advice on offer.
    recommended_role TEXT,
    -- Eligible matches the recommendation rested on at selection time.
    analyzed_matches INT         NOT NULL DEFAULT 0 CHECK (analyzed_matches >= 0),

    -- When the current role was chosen. Reset when the role changes, because
    -- "how have I done since I started working on this" is measured from there.
    selected_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    created_at       TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at       TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TRIGGER coaching_profiles_set_updated_at
    BEFORE UPDATE ON coaching_profiles
    FOR EACH ROW
    EXECUTE FUNCTION set_updated_at();

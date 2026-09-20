-- Coaching sessions: an immutable snapshot of where a player stood.
--
-- The product needs to say "your CS and deaths have improved since your last
-- coaching session". That is arithmetic over two points in time, and neither
-- point exists today:
--
--   * The role performance score (the "54" in "performance: 54") is recomputed
--     on every request and never written down.
--   * `coaching_analyses` does store a point-in-time snapshot, but its evidence
--     is *prose* — "your gold per minute averages 512" as an English sentence,
--     with no numeric field. Comparing two of them would mean parsing text.
--
-- So this table stores numbers. `metrics` is the load-bearing column: a list of
-- {key, value, sample, unit} that a later comparison can subtract without
-- reading a word of English.
--
-- Why a session is not the same thing as an analysis:
--
--   * An analysis is what a model said, and only exists when someone paid for a
--     model call. A session is what was measured, and must exist for everyone —
--     progress history is not a premium feature.
--   * `analysis_id` links the two when a model did run. NULL is the normal case.
--
-- Why it is immutable: a historical session must remain readable against the
-- numbers that produced it. Rewriting session #1 with today's figures would
-- erase the improvement it is there to demonstrate. Enforced by a trigger
-- below rather than by convention, because "do not update this table" is not a
-- rule code can be trusted to remember.
--
-- The current profile is deliberately *not* a second table. It is the newest
-- row here, so the current state and a historical snapshot are the same shape
-- and cannot drift apart.

CREATE TABLE coaching_sessions (
    id                   UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    dota_player_id       UUID        NOT NULL REFERENCES dota_players (id) ON DELETE CASCADE,

    -- A CoachableRole slug. Sessions are per role: a Carry session and a
    -- Support session describe different datasets and must never be compared.
    role                 TEXT        NOT NULL,
    -- Per (player, role), so "Session #12" is a real, stable number the UI can
    -- show rather than an index into a list that shifts.
    sequence             INT         NOT NULL CHECK (sequence >= 1),

    -- What was measured. The ids, not the payloads: the matches are already
    -- stored, and copying them here would be a second, divergent copy.
    analyzed_match_count INT         NOT NULL CHECK (analyzed_match_count >= 0),
    analyzed_match_ids   UUID[]      NOT NULL,
    -- The window's upper edge, so the next session knows what "new since" means
    -- without re-deriving it from the array.
    newest_match_at      TIMESTAMPTZ,

    -- The role score, 0-100, finally written down. NULL when the role had no
    -- eligible matches to score — absent, not zero.
    performance          REAL,

    -- [{key, label, value, sample, unit, higher_is_better}]. Numbers, which is
    -- the whole point of this table.
    metrics              JSONB       NOT NULL,
    -- PlayerTrait snapshots, as they read at the time.
    strengths            JSONB       NOT NULL,
    weaknesses           JSONB       NOT NULL,
    benchmark_snapshot   JSONB       NOT NULL,
    hero_snapshot        JSONB       NOT NULL,

    -- The focus that was active when this was taken. ON DELETE SET NULL rather
    -- than CASCADE: losing a focus row must not delete the history that
    -- referenced it.
    training_focus_id    UUID        REFERENCES training_focus (id) ON DELETE SET NULL,
    -- The model's reading of this session, if one was ever generated.
    analysis_id          UUID        REFERENCES coaching_analyses (id) ON DELETE SET NULL,

    created_at           TIMESTAMPTZ NOT NULL DEFAULT now(),

    -- Two sessions cannot claim the same number for the same role.
    UNIQUE (dota_player_id, role, sequence)
);

-- Every read is "this player's sessions in this role, newest first".
CREATE INDEX coaching_sessions_recent_idx
    ON coaching_sessions (dota_player_id, role, created_at DESC);

-- ---------------------------------------------------------------------------
-- Immutability
-- ---------------------------------------------------------------------------

-- A session is a historical fact. Every measured column is frozen on insert.
--
-- Exactly two transitions are legitimate, and both are about *references*
-- rather than measurements:
--
--   * `analysis_id` NULL → set, once. The snapshot is taken deterministically
--     and a model may be asked to interpret it minutes or days later.
--   * `training_focus_id` set → NULL. This one is not a caller at all: it is
--     the `ON DELETE SET NULL` referential action firing when a focus row is
--     removed. Rejecting it would make deleting a training focus fail, which
--     is how an immutability rule turns into an outage somewhere unrelated.
--
-- Policing the exact shape of both is what stops "attach the AI summary" from
-- quietly becoming "rewrite history": an UPDATE that touches a measured column
-- is rejected even when it also sets `analysis_id`, and an `analysis_id` that
-- is already set can never be swapped for another.
CREATE OR REPLACE FUNCTION coaching_sessions_reject_mutation() RETURNS TRIGGER AS $$
BEGIN
    -- Everything except the two reference columns must be byte-identical.
    IF ROW(NEW.id, NEW.dota_player_id, NEW.role, NEW.sequence,
           NEW.analyzed_match_count, NEW.analyzed_match_ids, NEW.newest_match_at,
           NEW.performance, NEW.metrics, NEW.strengths, NEW.weaknesses,
           NEW.benchmark_snapshot, NEW.hero_snapshot, NEW.created_at)
       IS DISTINCT FROM
       ROW(OLD.id, OLD.dota_player_id, OLD.role, OLD.sequence,
           OLD.analyzed_match_count, OLD.analyzed_match_ids, OLD.newest_match_at,
           OLD.performance, OLD.metrics, OLD.strengths, OLD.weaknesses,
           OLD.benchmark_snapshot, OLD.hero_snapshot, OLD.created_at)
    THEN
        RAISE EXCEPTION
            'coaching_sessions is immutable: session % may not be updated',
            OLD.id
            USING ERRCODE = 'restrict_violation';
    END IF;

    -- An analysis may be attached, never replaced or removed.
    IF NEW.analysis_id IS DISTINCT FROM OLD.analysis_id
       AND OLD.analysis_id IS NOT NULL
    THEN
        RAISE EXCEPTION
            'coaching_sessions is immutable: session % already has an analysis',
            OLD.id
            USING ERRCODE = 'restrict_violation';
    END IF;

    -- A focus reference may be cleared by the foreign key, never repointed.
    IF NEW.training_focus_id IS DISTINCT FROM OLD.training_focus_id
       AND NEW.training_focus_id IS NOT NULL
    THEN
        RAISE EXCEPTION
            'coaching_sessions is immutable: session % may not change its training focus',
            OLD.id
            USING ERRCODE = 'restrict_violation';
    END IF;

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER coaching_sessions_immutable
    BEFORE UPDATE ON coaching_sessions
    FOR EACH ROW
    EXECUTE FUNCTION coaching_sessions_reject_mutation();

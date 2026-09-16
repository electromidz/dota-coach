-- Phase 12: coaching becomes role-scoped.
--
-- Once a player has chosen a role, every piece of coaching is about that role
-- and nothing else. Two stored things have to learn about roles for that to
-- hold, and both are here.
--
-- ## Analyses
--
-- A player-wide analysis is now a *role* analysis. The role enters the context
-- hash, so two roles cannot collide on the cache key — but the hash is only
-- half of it: `latest()` reads "the newest analysis for this player", and
-- without a column to filter on it would happily hand a Carry analysis to a
-- player who has since switched to Support. So the role is stored, and read
-- back on.
--
-- NULL means "written before this migration", which is genuinely career-wide
-- and genuinely not attributable to a role. Those rows stay readable and are
-- simply never returned for a role-scoped read.
--
-- ## Training focus
--
-- The focus is "the one thing to work on". Under role coaching that is one
-- thing *per role*: a player working on Carry and a player working on Support
-- are not being asked to fix the same problem, and the same person switching
-- between them should not lose the goal they had.
--
-- The uniqueness rule therefore moves from "one active focus per player" to
-- "one active focus per player per role". The old index has to be dropped
-- first: a player with an active Carry focus and an active Support focus
-- violates it, which is exactly the state this phase makes legal.

ALTER TABLE coaching_analyses
    ADD COLUMN role TEXT;

ALTER TABLE coaching_analyses
    DROP CONSTRAINT coaching_analyses_scope_check;

ALTER TABLE coaching_analyses
    ADD CONSTRAINT coaching_analyses_scope_check
    CHECK (scope IN ('player', 'match', 'role'));

-- The read path for `GET /api/coach`: newest analysis for this player in this
-- role. `NULLS NOT DISTINCT` is not needed here — the query filters on the
-- role explicitly, including on NULL for the legacy rows.
CREATE INDEX coaching_analyses_role_recent_idx
    ON coaching_analyses (dota_player_id, role, generated_at DESC);

ALTER TABLE training_focus
    ADD COLUMN role TEXT;

DROP INDEX training_focus_one_active_idx;

-- One active focus per player per role. NULL is a distinct value for a unique
-- index in Postgres, which would let several legacy career focuses be active
-- at once; `NULLS NOT DISTINCT` keeps the old rule intact for them.
CREATE UNIQUE INDEX training_focus_one_active_per_role_idx
    ON training_focus (dota_player_id, role) NULLS NOT DISTINCT
    WHERE status = 'active';

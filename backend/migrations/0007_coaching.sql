-- Phase 7: AI coaching.
--
-- Two tables, split along the line the domain draws:
--
--   coaching_analyses   what was asked, and the evidence that was sent
--   coaching_insights   what the model answered, after validation
--
-- The evidence is stored as JSONB because it is genuinely a document: a
-- point-in-time snapshot of the sentences the model was shown, kept so an old
-- analysis can still be read against the numbers that produced it. The
-- insights are normalized, because "show me every weakness ever identified for
-- this player" is a query the coaching layer will need.

CREATE TABLE coaching_analyses (
    id             UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    dota_player_id UUID        NOT NULL REFERENCES dota_players (id) ON DELETE CASCADE,

    -- NULL for a career analysis; set for a single match.
    match_id       UUID        REFERENCES matches (id) ON DELETE CASCADE,
    scope          TEXT        NOT NULL CHECK (scope IN ('player', 'match')),

    -- SHA-256 over the prompt version, scope, model and evidence. Identical
    -- inputs have an identical answer, so a repeat request is served from here
    -- rather than spending a model call and a slice of the user's daily budget.
    context_hash   TEXT        NOT NULL,
    -- The model that actually answered, as the provider reported itself.
    model          TEXT        NOT NULL,
    summary        TEXT        NOT NULL,
    evidence       JSONB       NOT NULL,

    generated_at   TIMESTAMPTZ NOT NULL DEFAULT now(),

    -- NULLS NOT DISTINCT so career analyses (match_id IS NULL) collide with
    -- each other; the default NULL-is-distinct behaviour would let the cache
    -- fill with duplicates of the same question.
    UNIQUE NULLS NOT DISTINCT (dota_player_id, match_id, context_hash)
);

-- Two read paths: "the newest analysis for this player" and the rate limiter's
-- "how many has this player generated recently".
CREATE INDEX coaching_analyses_recent_idx
    ON coaching_analyses (dota_player_id, generated_at DESC);

CREATE TABLE coaching_insights (
    id            UUID   PRIMARY KEY DEFAULT gen_random_uuid(),
    analysis_id   UUID   NOT NULL REFERENCES coaching_analyses (id) ON DELETE CASCADE,
    -- The model's own ordering, most important first. Preserved rather than
    -- re-sorted: the ranking is part of the answer.
    position      INT    NOT NULL,

    -- One of the six kinds in domain::coaching::InsightKind. Checked in Rust
    -- before it reaches here; the constraint is the backstop.
    kind          TEXT   NOT NULL,
    title         TEXT   NOT NULL,
    explanation   TEXT   NOT NULL,
    -- Evidence ids, every one of which existed in the parent analysis's
    -- evidence at validation time.
    evidence_refs TEXT[] NOT NULL,

    UNIQUE (analysis_id, position)
);

CREATE INDEX coaching_insights_kind_idx ON coaching_insights (kind);

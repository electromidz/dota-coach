-- Phase 13: the generated training plan.
--
-- The plan is what turns an analysis into work: an ordered handful of things
-- to do in the next few games, each tied to the measured weakness it exists to
-- fix. Stored the same way insights are, and for the same reason — "show me
-- everything this player has been asked to work on" is a query, not a
-- document — and under the same guarantee: a step reaches this table only
-- after its citations were checked against the evidence the model was given,
-- and after every figure in its text was found in that evidence.
--
-- `position` is renumbered on the way in, so a plan always reads 1..n with no
-- holes where a rejected step used to be.

CREATE TABLE coaching_plan_steps (
    id            UUID   PRIMARY KEY DEFAULT gen_random_uuid(),
    analysis_id   UUID   NOT NULL REFERENCES coaching_analyses (id) ON DELETE CASCADE,
    position      INT    NOT NULL CHECK (position >= 1),

    title         TEXT   NOT NULL,
    -- What to actually do about it.
    action        TEXT   NOT NULL,
    -- Evidence ids, every one of which existed in the parent analysis's
    -- evidence at validation time.
    evidence_refs TEXT[] NOT NULL,

    UNIQUE (analysis_id, position)
);

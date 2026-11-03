-- Measurable goals for a training plan.
--
-- One row per plan step that cites a benchmarked metric: what to move, where
-- it stood when the goal was set, and the target (the peer median, or the
-- top-20% line once past the median). Stored with the analysis that set them
-- so the next analysis can say which were hit, without recomputing a target
-- that a later change to the rules might have chosen differently.
CREATE TABLE coaching_goals (
    id               UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    analysis_id      UUID        NOT NULL REFERENCES coaching_analyses (id) ON DELETE CASCADE,
    position         INT         NOT NULL CHECK (position >= 1),
    evidence_id      TEXT        NOT NULL,
    metric           TEXT        NOT NULL,
    label            TEXT        NOT NULL,
    hero_id          INT,
    higher_is_better BOOLEAN     NOT NULL,
    baseline         REAL        NOT NULL,
    target           REAL        NOT NULL,
    target_label     TEXT        NOT NULL,
    created_at       TIMESTAMPTZ NOT NULL DEFAULT now(),

    UNIQUE (analysis_id, position)
);

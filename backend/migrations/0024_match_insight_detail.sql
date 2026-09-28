-- The three-part form a single-match insight is written in.
--
-- `coaching_insights.explanation` is one paragraph of interpretation, which is
-- the right shape for a career or role analysis. Reading one match asks a
-- different question, and it is really three:
--
--     what happened          the concrete event, with the second it happened at
--     why it mattered        the gameplay consequence
--     what to do instead     the practical alternative
--
-- A paragraph can answer all three, and in practice it answers the third one
-- worst — which is the only one the player can act on. So they are stored
-- separately, and the column layout is what makes them separate on the page
-- rather than a formatting convention in a template.
--
-- Nullable, and `explanation` stays `NOT NULL`, so every row written before this
-- migration is still valid and still renders. An insight carries one shape or
-- the other; the validator in `services::coaching` refuses one that has neither,
-- and refuses a partial three-part form — two thirds of it is a paragraph with a
-- heading missing, and the missing third is always the advice.

ALTER TABLE coaching_insights
    -- `major` or `minor`. A judgement about this one game, and the reason the
    -- page can lead with the three things that mattered instead of listing
    -- everything that happened. Null for every analysis that predates the split,
    -- and for any answer where the model did not say — which is deliberately not
    -- the same as `minor`.
    ADD COLUMN severity       TEXT,

    -- `m:ss`, and only ever a moment that appeared verbatim in the evidence the
    -- model was shown. An unsupported timestamp is stripped before it reaches
    -- here: it is the most convincing thing this pipeline could fabricate,
    -- because a reader cannot tell an invented reading of a replay from a real
    -- one without opening the game.
    ADD COLUMN timestamp      TEXT,

    ADD COLUMN what_happened  TEXT,
    ADD COLUMN why_it_matters TEXT,
    ADD COLUMN better_action  TEXT;

-- The backstop for the two values Rust already checks. Deliberately permissive
-- about null: absent means the model did not answer, which is a legal state.
ALTER TABLE coaching_insights
    ADD CONSTRAINT coaching_insights_severity_check
    CHECK (severity IS NULL OR severity IN ('major', 'minor'));

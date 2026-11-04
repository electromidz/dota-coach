-- Who decided a match's position: the estimator, or the player.
--
-- The estimate is right most of the time and wrong often enough to matter
-- (a Mirana support read as "Offlane" from one lane's numbers). A player can
-- correct it; the correction is written to `role` itself so every page that
-- filters by position uses it, and the original estimate is kept beside it so
-- the correction can be undone and audited.
ALTER TABLE matches
    ADD COLUMN role_source    TEXT NOT NULL DEFAULT 'estimated'
        CHECK (role_source IN ('estimated', 'user')),
    ADD COLUMN role_estimated TEXT;

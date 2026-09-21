-- Rank-segmented benchmark snapshots.
--
-- OpenDota's `/benchmarks` does accept a `bracket` parameter (1 Herald … 8
-- Immortal, omitted for all ranks) and returns genuinely different percentile
-- buckets for each. An earlier reading of the API concluded otherwise because
-- it passed `rank`, which the endpoint ignores.
--
-- So the cache key is no longer (provider, hero). A Divine distribution and an
-- all-ranks one are different documents for the same hero and must not
-- overwrite each other.

ALTER TABLE benchmark_snapshots
    -- 0 is "all ranks". NOT NULL rather than a nullable bracket because NULL
    -- never conflicts in a unique index, which would leave the all-ranks row
    -- unconstrained and free to duplicate.
    ADD COLUMN bracket SMALLINT NOT NULL DEFAULT 0
        CHECK (bracket BETWEEN 0 AND 8);

-- Every existing row predates bracket support, so it is an all-ranks payload
-- and the default above already describes it correctly.

-- The old constraint was declared inline, so its name is Postgres-generated.
-- Dropping it by whatever it is actually called is safer than hard-coding the
-- name this database is *expected* to have used.
DO $$
DECLARE
    old_name TEXT;
BEGIN
    SELECT con.conname
      INTO old_name
      FROM pg_constraint con
      JOIN pg_class rel ON rel.oid = con.conrelid
     WHERE rel.relname = 'benchmark_snapshots'
       AND con.contype = 'u'
       AND con.conkey = ARRAY(
               SELECT att.attnum
                 FROM pg_attribute att
                WHERE att.attrelid = rel.oid
                  AND att.attname IN ('provider', 'hero_id')
                ORDER BY att.attnum
           );

    IF old_name IS NOT NULL THEN
        EXECUTE format(
            'ALTER TABLE benchmark_snapshots DROP CONSTRAINT %I', old_name
        );
    END IF;
END
$$;

ALTER TABLE benchmark_snapshots
    ADD CONSTRAINT benchmark_snapshots_provider_hero_bracket_key
        UNIQUE (provider, hero_id, bracket);

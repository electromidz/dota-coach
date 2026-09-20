-- A cache for the coaching context, and only a cache.
--
-- Reading `GET /api/coach` costs roughly twenty queries, several of them
-- aggregates over a hundred-match window, plus five per-hero benchmark
-- lookups. None of that changes between two page loads a second apart, and
-- all of it is a pure function of rows already stored elsewhere.
--
-- Everything here is reconstructable. Dropping this table loses nothing but
-- time: the next request recomputes from `matches`, `match_metrics`,
-- `coaching_sessions` and the provider snapshots, which remain the source of
-- truth. Nothing is ever read from here that could not be derived.
--
-- Invalidation is by key derivation, not by eviction.
--
-- The key carries a fingerprint of everything the cached value depends on —
-- when the player last synced, which role they are on, the active training
-- focus, the newest coaching session, and how fresh the provider snapshots
-- are. When any of those move, the key moves with them and the old entry
-- simply stops being found. That is the same trick `benchmark_snapshots`
-- already uses for freshness, and it is deliberate: forgetting to evict is
-- the classic way a coaching cache starts telling a player about last week,
-- and a key that cannot match cannot be forgotten about.
--
-- The TTL below is therefore a backstop rather than the mechanism. It bounds
-- how long a row nobody will ever look up again takes up space, and covers
-- anything a future input forgets to put in the fingerprint.

CREATE TABLE coaching_cache (
    -- The full derived key: namespace, version, player, role, fingerprint.
    key        TEXT        PRIMARY KEY,

    -- Scoped so a player's entries can be pruned as a group, and so a cache
    -- row cannot outlive the player it describes.
    dota_player_id UUID    NOT NULL REFERENCES dota_players (id) ON DELETE CASCADE,

    payload    JSONB       NOT NULL,
    fetched_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Pruning reads "this player's other entries", which is what keeps the table
-- at roughly one row per player per role rather than one per sync.
CREATE INDEX coaching_cache_player_idx ON coaching_cache (dota_player_id, fetched_at DESC);

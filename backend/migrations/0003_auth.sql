-- Phase 3: Steam authentication, application accounts, Dota identity linking.
--
-- Phases 1-2 modelled `users` as the Dota player, because there was no login.
-- Now that a Steam identity is proven server-side, the two are separated:
--
--   users        - the application account, keyed by the proven SteamID64
--   dota_players - the Dota identity linked to that account (1:1 for now)
--   sessions     - server-side sessions; the browser only ever holds a token
--
-- Existing rows are migrated in place rather than dropped.

-- 1. The old `users` table is really the Dota player. Rename it, along with
--    every constraint, index and trigger that carries the old name, so the new
--    `users` table can claim those names.
ALTER TABLE users RENAME TO dota_players;
ALTER INDEX users_pkey RENAME TO dota_players_pkey;
ALTER INDEX users_steam_id_key RENAME TO dota_players_steam_id_key;
ALTER INDEX users_account_id_key RENAME TO dota_players_dota_account_id_key;
ALTER TRIGGER users_set_updated_at ON dota_players RENAME TO dota_players_set_updated_at;

ALTER TABLE dota_players RENAME COLUMN account_id TO dota_account_id;

-- `matches` pointed at the old table; the foreign key survives the rename, only
-- the column and constraint names need to catch up.
ALTER TABLE matches RENAME COLUMN user_id TO dota_player_id;
ALTER TABLE matches RENAME CONSTRAINT matches_user_id_fkey TO matches_dota_player_id_fkey;
ALTER TABLE matches
    RENAME CONSTRAINT matches_user_id_match_id_key TO matches_dota_player_id_match_id_key;
ALTER INDEX matches_user_started_at_idx RENAME TO matches_player_started_at_idx;

-- 2. The application account. `steam_id` is written only from a verified
--    OpenID assertion, never from client input.
CREATE TABLE users (
    id            UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    steam_id      BIGINT      NOT NULL UNIQUE,
    -- Steam profile display fields, refreshed from the Dota provider on sync.
    persona_name  TEXT,
    avatar_url    TEXT,
    profile_url   TEXT,
    last_login_at TIMESTAMPTZ,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at    TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TRIGGER users_set_updated_at
    BEFORE UPDATE ON users
    FOR EACH ROW
    EXECUTE FUNCTION set_updated_at();

-- 3. Give every pre-existing player an account so no data is orphaned.
INSERT INTO users (steam_id, persona_name, avatar_url)
SELECT steam_id, persona_name, avatar_url
  FROM dota_players
    ON CONFLICT (steam_id) DO NOTHING;

-- 4. Link players to accounts. One Dota account per user for now; the unique
--    constraint is what enforces that, and dropping it is all a future
--    multi-account feature would need.
ALTER TABLE dota_players ADD COLUMN user_id UUID REFERENCES users (id) ON DELETE CASCADE;

UPDATE dota_players p
   SET user_id = u.id
  FROM users u
 WHERE u.steam_id = p.steam_id;

ALTER TABLE dota_players ALTER COLUMN user_id SET NOT NULL;
ALTER TABLE dota_players ADD CONSTRAINT dota_players_user_id_key UNIQUE (user_id);

-- 5. Steam profile fields belong to the account, not the Dota identity.
ALTER TABLE dota_players
    DROP COLUMN persona_name,
    DROP COLUMN avatar_url;

-- 6. Server-side sessions.
--
--    Only the SHA-256 of the token is stored: a database leak does not hand an
--    attacker usable session cookies.
CREATE TABLE sessions (
    id           UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id      UUID        NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    token_hash   TEXT        NOT NULL UNIQUE,
    expires_at   TIMESTAMPTZ NOT NULL,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    last_seen_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX sessions_user_id_idx ON sessions (user_id);
-- Supports the expiry sweep.
CREATE INDEX sessions_expires_at_idx ON sessions (expires_at);

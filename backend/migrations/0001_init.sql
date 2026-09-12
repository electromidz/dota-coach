-- Phase 1: foundation only. Match, metric, insight and profile tables arrive
-- with the sync pipeline in Phase 2.

CREATE EXTENSION IF NOT EXISTS "pgcrypto";

CREATE TABLE users (
    id           UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    -- 64-bit SteamID64 as entered by the user.
    steam_id     BIGINT      NOT NULL UNIQUE,
    -- 32-bit Dota account id (SteamID64 - 76561197960265728), used by the
    -- OpenDota API. Derived on insert, stored so provider calls are cheap.
    account_id   BIGINT      NOT NULL UNIQUE,
    persona_name TEXT,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at   TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE OR REPLACE FUNCTION set_updated_at() RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = now();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER users_set_updated_at
    BEFORE UPDATE ON users
    FOR EACH ROW
    EXECUTE FUNCTION set_updated_at();

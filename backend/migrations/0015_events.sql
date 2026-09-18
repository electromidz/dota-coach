-- Phase 15: a raw event stream for product analytics.
--
-- One append-only table, not a table per event type: the admin panel's
-- questions ("how many logged in this week", "what happened right before a
-- trial expired") are all "count/group these rows", and a generic shape
-- answers all of them without a schema migration per new event. `metadata` is
-- JSONB rather than a fixed set of columns for the same reason — its shape
-- varies by `type` and nothing here needs to query inside it yet.
--
-- No foreign-key cascade concerns beyond `ON DELETE CASCADE`: a deleted
-- account's history should not survive it.

CREATE TABLE events (
    id         UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id    UUID        NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    type       TEXT        NOT NULL,
    metadata   JSONB       NOT NULL DEFAULT '{}',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- A user's own timeline, newest first.
CREATE INDEX events_user_created_idx ON events (user_id, created_at DESC);
-- Stats aggregation: "how many `purchase` events this month".
CREATE INDEX events_type_created_idx ON events (type, created_at DESC);

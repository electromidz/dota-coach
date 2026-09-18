-- Phase 14: admin access and account status.
--
-- `is_admin` gates the admin panel; nothing else in the application reads it.
-- There is no self-serve promotion path — the first admin is set directly in
-- the database, the same way any other operator-only fact would be.
--
-- `status` is a support/moderation switch, independent of billing
-- entitlement: a disabled account should be refused before its subscription
-- is even considered, not treated as a lapsed trial.

ALTER TABLE users
    ADD COLUMN is_admin BOOLEAN NOT NULL DEFAULT false,
    ADD COLUMN status    TEXT    NOT NULL DEFAULT 'active';

ALTER TABLE users
    ADD CONSTRAINT users_status_check CHECK (status IN ('active', 'disabled'));

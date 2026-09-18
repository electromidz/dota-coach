-- Phase 17: who did what to which account or voucher, and when.
--
-- Every mutating admin action writes one row here before answering the
-- request — see `services::audit::record`. Best-effort like `events`: a
-- write failure here is logged, never allowed to fail the action itself, so
-- an audit hiccup can never be the reason an admin couldn't disable a
-- malicious account.

CREATE TABLE admin_audit_log (
    id          UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    -- Nullable and `SET NULL`, like `vouchers.created_by`: the admin's own
    -- account being deleted later must not erase the historical record that
    -- they took this action.
    admin_id    UUID        REFERENCES users (id) ON DELETE SET NULL,
    action      TEXT        NOT NULL,
    -- 'user' or 'voucher' today; kept as free text like `events.type` so a
    -- future target kind needs no migration.
    target_type TEXT        NOT NULL,
    target_id   UUID        NOT NULL,
    metadata    JSONB       NOT NULL DEFAULT '{}',
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- "Everything this admin has done."
CREATE INDEX admin_audit_log_admin_idx ON admin_audit_log (admin_id, created_at DESC);
-- "Everything that has happened to this account or voucher."
CREATE INDEX admin_audit_log_target_idx ON admin_audit_log (target_type, target_id, created_at DESC);

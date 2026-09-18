-- Phase 16: vouchers, and where a subscription's access actually came from.
--
-- `subscriptions.source` answers a question the row could not answer before:
-- did this account get access by waiting out a trial, paying, redeeming a
-- code, or an admin granting it directly? Every existing row predates this
-- feature and has only ever been a trial, so the default backfills exactly.
--
-- `vouchers` and `voucher_redemptions` are deliberately two tables rather
-- than one: a voucher is a thing an admin made, a redemption is a thing a
-- user did to it, and `UNIQUE(voucher_id, user_id)` is what makes "you can
-- only redeem a given code once" a constraint the database enforces under
-- concurrency, not a check the application race-conditions against.

ALTER TABLE subscriptions
    ADD COLUMN source TEXT NOT NULL DEFAULT 'trial';

ALTER TABLE subscriptions
    ADD CONSTRAINT subscriptions_source_check
    CHECK (source IN ('trial', 'payment', 'voucher', 'admin'));

CREATE TABLE vouchers (
    id            UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    code          TEXT        NOT NULL UNIQUE,
    duration_days INT         NOT NULL CHECK (duration_days > 0),
    max_uses      INT         NOT NULL CHECK (max_uses > 0),
    used_count    INT         NOT NULL DEFAULT 0 CHECK (used_count >= 0),
    -- `NULL` means the voucher never expires on its own — `active` is still
    -- the switch that turns it off.
    expires_at    TIMESTAMPTZ,
    active        BOOLEAN     NOT NULL DEFAULT true,
    -- The admin's own label ("Discord giveaway March 2026"), never shown to
    -- the redeeming user.
    note          TEXT,
    -- Nullable and `SET NULL`: deleting the admin who made a voucher must not
    -- take the voucher, or anyone's redemption of it, down with them.
    created_by    UUID        REFERENCES users (id) ON DELETE SET NULL,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE voucher_redemptions (
    id           UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    voucher_id   UUID        NOT NULL REFERENCES vouchers (id) ON DELETE CASCADE,
    user_id      UUID        NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    redeemed_at  TIMESTAMPTZ NOT NULL DEFAULT now(),

    -- The actual concurrency guarantee: two simultaneous redemption attempts
    -- by the same user can both pass every application-level check and only
    -- one can still win the insert. Its index also serves "every redemption
    -- of this voucher" (leftmost column), so no separate index on
    -- `voucher_id` alone is needed.
    UNIQUE (voucher_id, user_id)
);

-- "Every voucher this account has redeemed" — the admin user-detail page.
CREATE INDEX voucher_redemptions_user_idx ON voucher_redemptions (user_id, redeemed_at DESC);

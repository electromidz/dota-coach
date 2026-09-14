-- Phase 10: trial, subscription and crypto payments.
--
-- Three tables, each answering one question the frontend is never allowed to
-- answer for itself:
--
--   * `subscriptions` — may this account use premium features, and until when?
--   * `payments`      — what did we ask the provider to collect, and for how much?
--   * `billing_webhook_events` — has this provider notification already been
--     applied? Idempotency is a stored fact, not an assumption about delivery.
--
-- Money is stored in integer minor units (cents). A crypto provider quotes the
-- coin amount at its own precision, but the *price* is a fixed fiat figure and
-- the only figure the webhook is validated against, so it never needs to be a
-- float.

CREATE TABLE subscriptions (
    id                       UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    -- One subscription per account: a second row would make "is this user
    -- entitled?" a question with two answers.
    user_id                  UUID        NOT NULL UNIQUE REFERENCES users (id) ON DELETE CASCADE,

    -- trialing | active | expired | cancelled | past_due.
    status                   TEXT        NOT NULL,
    plan                     TEXT        NOT NULL,

    -- The trial window, anchored to account creation so materialising this row
    -- late cannot hand anybody extra free days.
    trial_started_at         TIMESTAMPTZ NOT NULL,
    trial_ends_at            TIMESTAMPTZ NOT NULL,

    -- The paid window. Null until a payment has actually settled.
    current_period_start     TIMESTAMPTZ,
    current_period_end       TIMESTAMPTZ,

    -- Which payment provider owns the remote state, and its identifiers. Kept
    -- as opaque text: the domain never interprets them.
    provider                 TEXT,
    provider_customer_id     TEXT,
    provider_subscription_id TEXT,

    created_at               TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at               TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- The expiry sweep: "whose trial or paid window has run out?"
CREATE INDEX subscriptions_expiry_idx
    ON subscriptions (status, trial_ends_at, current_period_end);

CREATE TABLE payments (
    id                  UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id             UUID        NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    subscription_id     UUID        REFERENCES subscriptions (id) ON DELETE SET NULL,

    provider            TEXT        NOT NULL,
    -- The provider's id for this charge. Null only in the window between
    -- reserving the row and the provider answering, so the order id we send is
    -- always a row that already exists.
    provider_payment_id TEXT,

    -- What we asked for, in minor units of `currency` — the figure the webhook
    -- is checked against.
    amount_cents        BIGINT      NOT NULL,
    currency            TEXT        NOT NULL,
    -- The coin the user chose to pay in, when the provider reports one.
    pay_currency        TEXT,

    -- pending | confirming | paid | failed | expired | refunded.
    status              TEXT        NOT NULL,
    -- Hosted checkout page. Never a secret; it is where the user is sent.
    payment_url         TEXT,

    created_at          TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at          TIMESTAMPTZ NOT NULL DEFAULT now(),
    -- Set exactly once, when the charge settles.
    completed_at        TIMESTAMPTZ
);

-- A provider payment id maps to exactly one charge, so a webhook can never be
-- applied to two rows.
CREATE UNIQUE INDEX payments_provider_payment_idx
    ON payments (provider, provider_payment_id)
    WHERE provider_payment_id IS NOT NULL;

CREATE INDEX payments_user_idx
    ON payments (user_id, created_at DESC);

-- Every accepted provider notification, recorded before it is applied.
--
-- The unique key is what makes processing idempotent: providers retry, and a
-- retry of "finished" must not buy a second month. A duplicate loses the insert
-- race and is acknowledged without being applied again.
CREATE TABLE billing_webhook_events (
    id                  UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    provider            TEXT        NOT NULL,
    -- Provider payment id plus the status it reported: the same charge moving
    -- through `confirming` and then `paid` is two events, but either one
    -- redelivered is not.
    event_key           TEXT        NOT NULL,
    payment_id          UUID        REFERENCES payments (id) ON DELETE SET NULL,
    status              TEXT        NOT NULL,
    -- The verified body, kept for dispute and debugging. Written only after the
    -- signature check, so this table never stores unauthenticated input.
    payload             JSONB       NOT NULL,
    received_at         TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE UNIQUE INDEX billing_webhook_events_key_idx
    ON billing_webhook_events (provider, event_key);

-- Conversational coaching.
--
-- The separation this table exists to enforce:
--
--   structured data   what is true about the player
--   conversation      what was said about it
--
-- Chat history is context for the next reply and nothing else. No statistic is
-- ever read back out of a message, and deleting every row here would cost the
-- product its memory of the discussion while costing it nothing about the
-- player. That is the point: a number the coach mentioned in March must come
-- from `coaching_sessions` when it is mentioned again, not from a sentence it
-- said once.
--
-- One conversation per player per role, rather than a thread list. Coaching a
-- role is an ongoing relationship, and a player who has been working on their
-- Carry for two months has one history of that, not fourteen. Switching role
-- switches conversation, for the same reason the evidence switches: a Carry
-- discussion is not about the same games.

CREATE TABLE coaching_conversations (
    id             UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    dota_player_id UUID        NOT NULL REFERENCES dota_players (id) ON DELETE CASCADE,
    -- A CoachableRole slug.
    role           TEXT        NOT NULL,

    created_at     TIMESTAMPTZ NOT NULL DEFAULT now(),
    -- Carried so the list can order without touching the messages.
    last_message_at TIMESTAMPTZ,

    UNIQUE (dota_player_id, role)
);

CREATE TABLE coaching_messages (
    id              UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    conversation_id UUID        NOT NULL REFERENCES coaching_conversations (id) ON DELETE CASCADE,

    -- Transcript order, and the reason it is not `created_at`: a question and
    -- its reply are written in one transaction, where `now()` is the
    -- transaction's start and therefore identical for both. Ordering by the
    -- timestamp left the two tied and the tiebreak decided by a random UUID,
    -- which put the coach's answer before the question about half the time.
    seq             BIGSERIAL   NOT NULL,

    -- 'player' or 'coach'. Stored rather than inferred from position, because
    -- a failed generation leaves a player turn with no reply after it.
    speaker         TEXT        NOT NULL CHECK (speaker IN ('player', 'coach')),
    content         TEXT        NOT NULL,

    -- Which evidence the reply was grounded in, for a coach turn. The same
    -- provenance a stored insight carries: every figure in the text traces to
    -- one of these ids, and the UI can show what it was reading.
    evidence_refs   TEXT[]      NOT NULL DEFAULT '{}',
    -- The model that answered, as it reported itself. NULL for player turns.
    model           TEXT,

    created_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Every read is "this conversation's messages, in transcript order"; the
-- daily-limit count is "this player's coach turns since a timestamp".
CREATE INDEX coaching_messages_thread_idx
    ON coaching_messages (conversation_id, seq);
CREATE INDEX coaching_messages_recent_idx
    ON coaching_messages (created_at) WHERE speaker = 'coach';

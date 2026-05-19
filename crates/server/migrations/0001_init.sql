CREATE EXTENSION IF NOT EXISTS "pgcrypto";

CREATE TABLE IF NOT EXISTS devices (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id TEXT NOT NULL,
    device_id TEXT NOT NULL,
    identity_key BYTEA NOT NULL,
    signed_prekey BYTEA NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (user_id, device_id)
);

CREATE TABLE IF NOT EXISTS one_time_prekeys (
    id BIGSERIAL PRIMARY KEY,
    device_ref UUID NOT NULL REFERENCES devices(id) ON DELETE CASCADE,
    key_id INTEGER NOT NULL,
    pubkey BYTEA NOT NULL,
    consumed_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (device_ref, key_id)
);

CREATE TABLE IF NOT EXISTS messages (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    message_id TEXT NOT NULL UNIQUE,
    from_device UUID NOT NULL REFERENCES devices(id) ON DELETE CASCADE,
    to_device UUID NOT NULL REFERENCES devices(id) ON DELETE CASCADE,
    envelope_bytes BYTEA NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    delivered_at TIMESTAMPTZ,
    acked_at TIMESTAMPTZ
);

CREATE INDEX IF NOT EXISTS idx_prekeys_available
    ON one_time_prekeys (device_ref)
    WHERE consumed_at IS NULL;

CREATE INDEX IF NOT EXISTS idx_messages_pending
    ON messages (to_device, created_at)
    WHERE delivered_at IS NULL;

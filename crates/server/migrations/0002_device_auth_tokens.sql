CREATE TABLE IF NOT EXISTS device_auth_tokens (
    id BIGSERIAL PRIMARY KEY,
    device_ref UUID NOT NULL REFERENCES devices(id) ON DELETE CASCADE,
    token_hash TEXT NOT NULL UNIQUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    expires_at TIMESTAMPTZ NOT NULL,
    revoked_at TIMESTAMPTZ
);

CREATE INDEX IF NOT EXISTS idx_device_auth_tokens_active
    ON device_auth_tokens (device_ref, expires_at)
    WHERE revoked_at IS NULL;

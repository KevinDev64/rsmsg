CREATE TABLE IF NOT EXISTS registration_invites (
    id UUID PRIMARY KEY,
    secret_hash TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    expires_at TIMESTAMPTZ NOT NULL DEFAULT NOW() + INTERVAL '2 days',
    used_at TIMESTAMPTZ,
    used_by_user_id BIGINT REFERENCES users(id) ON DELETE SET NULL
);

CREATE INDEX IF NOT EXISTS idx_registration_invites_unused_expires_at
    ON registration_invites (expires_at)
    WHERE used_at IS NULL;

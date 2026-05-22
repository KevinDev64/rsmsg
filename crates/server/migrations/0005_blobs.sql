CREATE TABLE IF NOT EXISTS blobs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    owner_device UUID NOT NULL REFERENCES devices(id) ON DELETE CASCADE,
    data BYTEA NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_blobs_owner_device_created_at
    ON blobs (owner_device, created_at);

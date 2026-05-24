CREATE TABLE IF NOT EXISTS user_blocks (
    blocker_user_id TEXT NOT NULL REFERENCES users(user_id) ON DELETE CASCADE,
    blocked_user_id TEXT NOT NULL REFERENCES users(user_id) ON DELETE CASCADE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (blocker_user_id, blocked_user_id),
    CHECK (blocker_user_id <> blocked_user_id)
);

CREATE INDEX IF NOT EXISTS idx_user_blocks_blocked_user_id
    ON user_blocks (blocked_user_id);

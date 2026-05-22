ALTER TABLE devices
    ADD COLUMN IF NOT EXISTS signing_identity_key BYTEA,
    ADD COLUMN IF NOT EXISTS signed_prekey_signature BYTEA;

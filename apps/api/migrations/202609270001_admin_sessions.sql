CREATE TABLE admin_account (
    id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    username TEXT NOT NULL UNIQUE,
    password_hash TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT admin_account_singleton CHECK (id = 1),
    CONSTRAINT admin_account_username_nonempty CHECK (length(trim(username)) > 0),
    CONSTRAINT admin_account_password_hash_nonempty CHECK (length(password_hash) > 0)
);

CREATE TABLE admin_session (
    id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    admin_id BIGINT NOT NULL REFERENCES admin_account(id) ON DELETE CASCADE,
    token_hash BYTEA NOT NULL UNIQUE,
    csrf_token_hash BYTEA NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    last_seen_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    idle_expires_at TIMESTAMPTZ NOT NULL,
    absolute_expires_at TIMESTAMPTZ NOT NULL,
    revoked_at TIMESTAMPTZ,
    CONSTRAINT admin_session_token_hash_length CHECK (octet_length(token_hash) = 32),
    CONSTRAINT admin_session_csrf_hash_length CHECK (octet_length(csrf_token_hash) = 32),
    CONSTRAINT admin_session_expiry_order CHECK (idle_expires_at <= absolute_expires_at)
);

CREATE INDEX admin_session_admin_active_idx
    ON admin_session (admin_id, idle_expires_at, absolute_expires_at)
    WHERE revoked_at IS NULL;

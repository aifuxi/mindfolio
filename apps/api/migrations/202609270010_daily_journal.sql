CREATE TABLE daily_journal (
    business_date DATE PRIMARY KEY,
    body TEXT NOT NULL CHECK (length(body) BETWEEN 1 AND 100000),
    version BIGINT NOT NULL DEFAULT 1 CHECK (version > 0),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

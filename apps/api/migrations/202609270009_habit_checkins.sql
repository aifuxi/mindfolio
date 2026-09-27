CREATE TABLE habit_checkin (
    id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    habit_id BIGINT NOT NULL REFERENCES habit(id),
    business_date DATE NOT NULL,
    completed BOOLEAN NOT NULL,
    note TEXT NOT NULL DEFAULT '' CHECK (length(note) <= 2000),
    version BIGINT NOT NULL DEFAULT 1 CHECK (version > 0),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (habit_id, business_date)
);

CREATE INDEX habit_checkin_date_idx ON habit_checkin(business_date, habit_id);

CREATE TABLE habit (
    id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    created_on DATE NOT NULL DEFAULT (now() AT TIME ZONE 'Asia/Shanghai')::date,
    deleted_at TIMESTAMPTZ,
    version BIGINT NOT NULL DEFAULT 1 CHECK (version > 0),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE habit_setting (
    id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    habit_id BIGINT NOT NULL REFERENCES habit(id),
    effective_on DATE NOT NULL,
    name TEXT NOT NULL CHECK (length(btrim(name)) BETWEEN 1 AND 120),
    cadence TEXT NOT NULL CHECK (cadence IN ('daily', 'weekly')),
    weekly_target SMALLINT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (habit_id, effective_on),
    CHECK ((cadence = 'daily' AND weekly_target IS NULL) OR
           (cadence = 'weekly' AND weekly_target BETWEEN 1 AND 7))
);

CREATE INDEX habit_setting_history_idx ON habit_setting(habit_id, effective_on DESC);

CREATE EXTENSION IF NOT EXISTS btree_gist;

CREATE TABLE habit_pause (
    id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    habit_id BIGINT NOT NULL REFERENCES habit(id),
    start_on DATE NOT NULL,
    end_on DATE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CHECK (end_on IS NULL OR end_on >= start_on),
    EXCLUDE USING gist (habit_id WITH =, daterange(start_on, COALESCE(end_on, 'infinity'::date), '[)') WITH &&)
);

CREATE UNIQUE INDEX habit_pause_open_idx ON habit_pause(habit_id) WHERE end_on IS NULL;
CREATE INDEX habit_pause_history_idx ON habit_pause(habit_id, start_on DESC);

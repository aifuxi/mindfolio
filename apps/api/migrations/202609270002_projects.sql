CREATE TABLE project (
    id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    name TEXT NOT NULL,
    version BIGINT NOT NULL DEFAULT 1,
    completed_at TIMESTAMPTZ,
    archived_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT project_name_valid CHECK (name = trim(name) AND char_length(name) BETWEEN 1 AND 120),
    CONSTRAINT project_version_positive CHECK (version > 0)
);

CREATE INDEX project_active_order_idx ON project (created_at DESC, id DESC)
    WHERE archived_at IS NULL;

CREATE INDEX project_archived_order_idx ON project (created_at DESC, id DESC)
    WHERE archived_at IS NOT NULL;

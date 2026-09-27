CREATE EXTENSION IF NOT EXISTS pg_trgm;

CREATE TABLE task_tag (
    id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    name TEXT NOT NULL,
    CONSTRAINT task_tag_name_valid CHECK (name = trim(name) AND char_length(name) BETWEEN 1 AND 40)
);

CREATE UNIQUE INDEX task_tag_name_ci_idx ON task_tag (lower(name));

CREATE TABLE task_tag_link (
    task_id BIGINT NOT NULL REFERENCES task(id) ON DELETE CASCADE,
    tag_id BIGINT NOT NULL REFERENCES task_tag(id) ON DELETE CASCADE,
    PRIMARY KEY (task_id, tag_id)
);

CREATE INDEX task_tag_link_tag_task_idx ON task_tag_link (tag_id, task_id);
CREATE INDEX task_title_trgm_idx ON task USING gin (title gin_trgm_ops);
CREATE INDEX task_description_trgm_idx ON task USING gin (description gin_trgm_ops);
CREATE INDEX task_status_order_idx ON task (status, created_at DESC, id DESC);
CREATE INDEX task_priority_order_idx ON task (priority, created_at DESC, id DESC);
CREATE INDEX task_planned_date_order_idx ON task (planned_date, created_at DESC, id DESC);
CREATE INDEX task_due_date_order_idx ON task (due_date, created_at DESC, id DESC);

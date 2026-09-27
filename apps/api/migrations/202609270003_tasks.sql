CREATE TABLE task (
    id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    project_id BIGINT REFERENCES project(id),
    title TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    status TEXT NOT NULL DEFAULT 'todo',
    priority TEXT,
    planned_date DATE,
    due_date DATE,
    in_backlog BOOLEAN NOT NULL DEFAULT FALSE,
    version BIGINT NOT NULL DEFAULT 1,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT task_title_valid CHECK (title = trim(title) AND char_length(title) BETWEEN 1 AND 200),
    CONSTRAINT task_description_length CHECK (char_length(description) <= 20000),
    CONSTRAINT task_status_valid CHECK (status IN ('todo', 'in_progress', 'completed', 'canceled')),
    CONSTRAINT task_priority_valid CHECK (priority IS NULL OR priority IN ('low', 'medium', 'high', 'urgent')),
    CONSTRAINT task_backlog_requires_project CHECK (NOT in_backlog OR project_id IS NOT NULL),
    CONSTRAINT task_version_positive CHECK (version > 0)
);

CREATE INDEX task_inbox_order_idx ON task (created_at DESC, id DESC)
    WHERE project_id IS NULL;

CREATE INDEX task_project_order_idx ON task (project_id, created_at DESC, id DESC)
    WHERE project_id IS NOT NULL;

CREATE TABLE task_completion (
    id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    task_id BIGINT NOT NULL,
    task_title TEXT NOT NULL,
    project_name TEXT,
    completed_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT task_completion_title_nonempty CHECK (char_length(task_title) > 0)
);

CREATE INDEX task_completion_task_time_idx ON task_completion (task_id, completed_at DESC, id DESC);
CREATE INDEX task_completion_time_idx ON task_completion (completed_at DESC, id DESC);

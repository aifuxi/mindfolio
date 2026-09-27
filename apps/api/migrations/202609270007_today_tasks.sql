CREATE INDEX task_today_planned_idx ON task (planned_date, created_at DESC, id DESC)
    WHERE status IN ('todo', 'in_progress');

CREATE INDEX task_today_due_idx ON task (due_date, created_at DESC, id DESC)
    WHERE status IN ('todo', 'in_progress');

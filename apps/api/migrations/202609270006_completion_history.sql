CREATE INDEX task_completion_business_date_idx
    ON task_completion (((completed_at AT TIME ZONE 'Asia/Shanghai')::date), completed_at DESC, id DESC);

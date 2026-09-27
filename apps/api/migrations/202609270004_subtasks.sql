ALTER TABLE task ADD COLUMN parent_id BIGINT REFERENCES task(id) ON DELETE RESTRICT;

ALTER TABLE task ADD CONSTRAINT task_not_own_parent CHECK (parent_id IS NULL OR parent_id <> id);

CREATE INDEX task_parent_order_idx ON task (parent_id, created_at DESC, id DESC)
    WHERE parent_id IS NOT NULL;

CREATE FUNCTION check_task_hierarchy() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE
    parent_task task%ROWTYPE;
BEGIN
    IF NEW.parent_id IS NOT NULL THEN
        SELECT * INTO parent_task FROM task WHERE id = NEW.parent_id FOR SHARE;
        IF NOT FOUND OR parent_task.parent_id IS NOT NULL THEN
            RAISE EXCEPTION '子任务只能有一级' USING ERRCODE = '23514';
        END IF;
        IF NEW.project_id IS DISTINCT FROM parent_task.project_id THEN
            RAISE EXCEPTION '子任务与父任务的项目必须一致' USING ERRCODE = '23514';
        END IF;
    END IF;
    IF EXISTS (
        SELECT 1 FROM task AS child
        WHERE child.parent_id = NEW.id
          AND (NEW.parent_id IS NOT NULL OR child.project_id IS DISTINCT FROM NEW.project_id)
    ) THEN
        RAISE EXCEPTION '父任务的层级或项目与子任务不一致' USING ERRCODE = '23514';
    END IF;
    RETURN NEW;
END
$$;

CREATE CONSTRAINT TRIGGER task_hierarchy_valid
    AFTER INSERT OR UPDATE OF parent_id, project_id ON task
    DEFERRABLE INITIALLY IMMEDIATE
    FOR EACH ROW EXECUTE FUNCTION check_task_hierarchy();

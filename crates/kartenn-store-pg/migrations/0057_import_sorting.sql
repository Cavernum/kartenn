-- The sort of an import moves to its background task (spec 006, H18; task 006-27):
-- `start` makes the import `sorting` and its task due; the task sorts and cuts the
-- elements in batches, then makes it `ready`. A sort that fails brings the import back to
-- `uploading` and keeps its problem (`invalid-import` and its violations) in `error`,
-- cleared by the next start.
ALTER TABLE import DROP CONSTRAINT import_state_check;
ALTER TABLE import ADD CONSTRAINT import_state_check
    CHECK (state IN ('uploading', 'sorting', 'ready', 'running', 'paused', 'done', 'cancelled'));
ALTER TABLE import ADD COLUMN error jsonb;

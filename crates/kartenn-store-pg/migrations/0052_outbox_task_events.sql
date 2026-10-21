-- Events of the background tasks of modules (spec 016, REQ-016-15): emitted outside any
-- changeset, they have none. Each keeps its own CloudEvents `id` in its payload.

ALTER TABLE outbox ALTER COLUMN changeset_id DROP NOT NULL;

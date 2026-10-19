-- Imports deposited by chunks (spec 006, REQ-006-17 to 19; task 006-21): an import is
-- `uploading` while its numbered chunks arrive, `ready` once started (sorted and cut in
-- batches), then run as the background task `import:<id>`.
ALTER TABLE import DROP CONSTRAINT import_state_check;
UPDATE import SET state = 'ready' WHERE state = 'queued';
ALTER TABLE import ADD CONSTRAINT import_state_check
    CHECK (state IN ('uploading', 'ready', 'running', 'paused', 'done', 'cancelled'));

-- A chunk is kept under its number; a resent chunk is recognised by its digest (SHA-256 of
-- its elements as received), `size` counts its elements.
CREATE TABLE import_chunk (
    import_id uuid NOT NULL REFERENCES import (id),
    n         integer NOT NULL CHECK (n >= 0),
    digest    bytea NOT NULL,
    size      integer NOT NULL CHECK (size >= 0),
    elements  jsonb NOT NULL,
    PRIMARY KEY (import_id, n)
);

GRANT SELECT, INSERT ON import_chunk TO kartenn_app;
-- The tasks of the imports come and go with them.
GRANT SELECT, INSERT, UPDATE, DELETE ON job TO kartenn_app;

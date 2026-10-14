-- Detail of the differential read by pages in a stable order (spec 003, REQ-003-19).
ALTER TABLE changeset_diff ADD COLUMN seq bigint GENERATED ALWAYS AS IDENTITY;
CREATE INDEX changeset_diff_page ON changeset_diff (changeset_id, seq);

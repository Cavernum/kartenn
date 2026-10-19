-- The list of the changesets by key (spec 006, REQ-006-16; task 006-17): newest first by
-- system time, then identifier; the filters by state and author have their own indexes
-- (0003).
CREATE INDEX changeset_keyset ON changeset (submitted_at, id);

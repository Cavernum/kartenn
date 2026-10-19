-- Closures added by a cascade (REQ-004-26), journaled with their changeset so that an
-- idempotent replay rebuilds the first response from the journal (006/REQ-006-15).
ALTER TABLE changeset ADD COLUMN cascaded jsonb NOT NULL DEFAULT '[]';

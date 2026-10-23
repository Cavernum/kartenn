-- The author's issuer and groups, kept with an import when it starts or resumes (spec 005,
-- REQ-005-22; spec 006, REQ-006-18): its batches run in a background task, without the
-- token where the groups live, and are submitted with the rights they give, as the
-- approval of a pending changeset checks its author's rights with the snapshot kept in
-- its `Pending`. NULL: the identity alone (imports started before this migration).
ALTER TABLE import ADD COLUMN author_subject jsonb;

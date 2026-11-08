-- The detail of an anomaly (spec 021, A36, A39): the values its message cites, structured
-- for a client that composes its own sentence, written with the violation and refreshed
-- with its message. Its shape is the rule's; for `prop.resolution.expected_mismatch`,
-- `{property, resolved, expected, source, revision}`. Null for a rule that structures
-- nothing, and for an anomaly opened before this migration until it is evaluated anew.
ALTER TABLE anomaly ADD COLUMN detail jsonb;

-- Written by the table-wide INSERT of 0007; refreshed with the message.
GRANT UPDATE (detail) ON anomaly TO kartenn_app;

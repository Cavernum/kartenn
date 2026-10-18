-- Pagination by key of the prefixes (spec 006, REQ-006-7; task 006-10): the order of a
-- list, domain then value then identifier, read from an index on both sides of a dated
-- read (current rows and history), without sorting the whole collection.
CREATE INDEX prefix_keyset ON prefix (domain_id, prefix, id);
CREATE INDEX prefix_history_keyset ON prefix_history (domain_id, prefix, id);

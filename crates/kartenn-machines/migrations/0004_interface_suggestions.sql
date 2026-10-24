-- The list of the interfaces and their suggestions by prefix (spec 018, REQ-018-23; task
-- 018-16): the order of the list (machine, name, identifier) read from an index, and the
-- lower-cased name compared by `LIKE 'q%'`, whatever the collation of the database.

CREATE INDEX interface_keyset ON interface (machine_id, name, id);
CREATE INDEX interface_name_pattern ON interface (lower(name) text_pattern_ops);

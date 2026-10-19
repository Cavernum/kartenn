-- Version of an object (006/design « Version d'un objet », task 006-16): the last
-- changeset that wrote the object or its property assignments, whatever the period. It is
-- the strong ETag of a current read and what `expected` is compared with (REQ-004-11,
-- 006/REQ-006-14). Written in the transaction of the changeset, before its journal (the
-- reference is checked at commit, and a simulation never commits); not versioned.
CREATE TABLE object_version (
    object_id    uuid PRIMARY KEY,
    changeset_id uuid NOT NULL REFERENCES changeset (id) DEFERRABLE INITIALLY DEFERRED
);
GRANT SELECT, INSERT, UPDATE ON object_version TO kartenn_app;

-- From the current rows: the latest changeset among an object's own rows and its
-- assignments, by submission.
INSERT INTO object_version (object_id, changeset_id)
SELECT DISTINCT ON (w.id) w.id, w.changeset_id
FROM (
    SELECT id, changeset_id FROM addressing_domain
    UNION ALL SELECT id, changeset_id FROM prefix
    UNION ALL SELECT id, changeset_id FROM ip_range
    UNION ALL SELECT id, changeset_id FROM address
    UNION ALL SELECT id, changeset_id FROM entity
    UNION ALL SELECT id, changeset_id FROM site
    UNION ALL SELECT id, changeset_id FROM property_declaration
    UNION ALL SELECT id, changeset_id FROM property_enum_value
    UNION ALL SELECT object_id, changeset_id FROM property_assignment
) w
JOIN changeset c ON c.id = w.changeset_id
ORDER BY w.id, c.submitted_at DESC, c.id DESC;

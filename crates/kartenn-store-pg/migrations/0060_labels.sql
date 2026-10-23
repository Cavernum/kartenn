-- Labels by language (spec 018, R8, R22, Q-018-5 to 7; task 018-6; ADR 0040): a column
-- `labels` `{<language>: <text>}` on each owning table of the core, versioned with its row
-- (on the table and its history); the revision of the labels, which a changeset writing
-- labels increments and the dictionary `GET /v1/labels` serves.
--
-- The labels kept until now become labels of `fr`: the only language of the data of the
-- v0.2 (built-in roles, objects of the policy, `DEFAULT_LOCALE` of the policy). The
-- columns `label` go with the writes that replace them (task 018-7).

CALL kartenn_add_versioned_column('property_declaration',
    'labels jsonb NOT NULL DEFAULT ''{}'' CHECK (jsonb_typeof(labels) = ''object'')');
CALL kartenn_add_versioned_column('property_enum_value',
    'labels jsonb NOT NULL DEFAULT ''{}'' CHECK (jsonb_typeof(labels) = ''object'')');
CALL kartenn_add_versioned_column('role',
    'labels jsonb NOT NULL DEFAULT ''{}'' CHECK (jsonb_typeof(labels) = ''object'')');

-- The same rows, labelled: not a new version (no changeset), hence without the trigger.
ALTER TABLE role DISABLE TRIGGER versioning;
UPDATE role SET labels = jsonb_build_object('fr', label);
ALTER TABLE role ENABLE TRIGGER versioning;
UPDATE role_history SET labels = jsonb_build_object('fr', label);

-- One row: the revision of the labels stored (R22). A changeset that writes a column
-- `labels` increments it in its transaction, which orders such changesets between them.
CREATE TABLE label_revision (
    id        smallint PRIMARY KEY CHECK (id = 1),
    revision  bigint NOT NULL CHECK (revision >= 0)
);
INSERT INTO label_revision (id, revision) VALUES (1, 0);
REVOKE ALL ON label_revision FROM PUBLIC;
GRANT SELECT, UPDATE ON label_revision TO kartenn_app;

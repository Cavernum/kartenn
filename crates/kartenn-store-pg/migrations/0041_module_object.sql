-- Objects of the business modules and the migrations of each module (spec 008, REQ-008-2,
-- 3; ADR 0026, clauses 3 and 4). Each version of an object of a module is registered here,
-- in the transaction of the changeset that writes it, beside the module's own table: the
-- core reaches it for versions, external identifiers, anomalies, history, properties and
-- undo. The kind is qualified (`vlan.vlan`); the parent is an object of the same module.

CREATE TABLE module_object (
    id            uuid NOT NULL,
    module        text NOT NULL CHECK (module ~ '^[a-z][a-z0-9_]*$' AND module <> 'core'),
    kind          text NOT NULL CHECK (kind LIKE module || '.%'),
    parent_id     uuid,
    domain_id     uuid,
    valid_period  tstzrange NOT NULL CHECK (NOT isempty(valid_period)),
    sys_start     timestamptz NOT NULL DEFAULT now(),
    changeset_id  uuid NOT NULL,
    PRIMARY KEY (id, valid_period WITHOUT OVERLAPS),
    CHECK (parent_id IS DISTINCT FROM id)
);
CALL kartenn_make_versioned('module_object');
CREATE INDEX module_object_by_parent ON module_object (parent_id) WHERE parent_id IS NOT NULL;

-- The migrations of each module applied to this database, by the migration job, with the
-- SHA-256 of their SQL: a changed migration stops the job (REQ-008-2).
CREATE TABLE module_migration (
    module      text NOT NULL,
    version     bigint NOT NULL,
    name        text NOT NULL,
    checksum    bytea NOT NULL,
    applied_at  timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (module, version)
);
GRANT SELECT ON module_migration TO kartenn_app;

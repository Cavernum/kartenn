-- Properties (spec 003, task 003-6; REQ-003-5, 12, 13, 25; ADR 0016 amended by P9).
-- Declarations, obligations and closed-list values are objects with an id; assignments
-- and resolved origins are identified by (object_id, property_id), hence the keyed
-- variant of kartenn_make_versioned below. No foreign keys: carriers are of several
-- kinds and integrity is checked by the service, as for the core objects.

-- Versioning keyed by any columns (ADR 0017): the history index follows the key.
CREATE PROCEDURE public.kartenn_make_versioned(target regclass, key_columns text[])
LANGUAGE plpgsql SET search_path = pg_catalog, public AS $$
DECLARE
    nsp  text := (SELECT n.nspname FROM pg_class c JOIN pg_namespace n ON n.oid = c.relnamespace WHERE c.oid = target);
    tbl  text := (SELECT relname FROM pg_class WHERE oid = target);
    hist text := tbl || '_history';
    missing text;
BEGIN
    SELECT string_agg(c, ', ') INTO missing
    FROM unnest(key_columns || ARRAY['valid_period', 'sys_start', 'changeset_id']) AS c
    WHERE NOT EXISTS (SELECT FROM pg_attribute WHERE attrelid = target AND attname = c AND NOT attisdropped);
    IF missing IS NOT NULL THEN
        RAISE 'table % lacks the versioning columns %', tbl, missing;
    END IF;
    EXECUTE format('CREATE TABLE %I.%I (LIKE %I.%I, sys_end timestamptz NOT NULL) PARTITION BY RANGE (sys_end)',
                   nsp, hist, nsp, tbl);
    EXECUTE format('CREATE TABLE %I.%I PARTITION OF %I.%I DEFAULT', nsp, hist || '_default', nsp, hist);
    EXECUTE format('CREATE INDEX ON %I.%I (%s, sys_start)', nsp, hist,
                   (SELECT string_agg(quote_ident(k), ', ') FROM unnest(key_columns) AS k));
    CALL public.kartenn_versioning_trigger(target);
    EXECUTE format('REVOKE ALL ON %I.%I FROM PUBLIC', nsp, hist);
    EXECUTE format('GRANT SELECT ON %I.%I TO kartenn_app', nsp, hist);
    EXECUTE format('GRANT SELECT, INSERT, UPDATE, DELETE ON %I.%I TO kartenn_app', nsp, tbl);
END $$;

CREATE OR REPLACE PROCEDURE public.kartenn_make_versioned(target regclass)
LANGUAGE plpgsql SET search_path = pg_catalog, public AS $$
BEGIN
    CALL public.kartenn_make_versioned(target, ARRAY['id']);
END $$;

-- REQ-003-1 to 3: key `<namespace>.<name>`; the namespace says who declares it (P1).
CREATE TABLE property_declaration (
    id              uuid NOT NULL,
    key             text NOT NULL CHECK (key ~ '^[a-z][a-z0-9_]*\.[a-z][a-z0-9_]*$'),
    -- `enum`, `text`, `int`, `bool`, `date`, `fqdn`, `ref`, or a type compiled by a module.
    value_type      text NOT NULL CHECK (value_type ~ '^[a-z][a-z0-9_.]*$'),
    type_params     jsonb NOT NULL DEFAULT '{}' CHECK (jsonb_typeof(type_params) = 'object'),
    cardinality     text NOT NULL CHECK (cardinality IN ('single', 'set')),
    carriers        text[] NOT NULL CHECK (cardinality(carriers) > 0),
    -- null: not heritable; `addressing`, `entity`, `site`, or a tree of a module (P3).
    inherit_tree    text,
    expected_source text,
    expected_mode   text CHECK (expected_mode IN ('suggest', 'default', 'check')),
    valid_period    tstzrange NOT NULL CHECK (NOT isempty(valid_period)),
    sys_start       timestamptz NOT NULL DEFAULT now(),
    changeset_id    uuid NOT NULL,
    PRIMARY KEY (id, valid_period WITHOUT OVERLAPS),
    CONSTRAINT property_key_unique EXCLUDE USING GIST (key WITH =, valid_period WITH &&),
    CONSTRAINT property_expected_source CHECK ((expected_source IS NULL) = (expected_mode IS NULL))
);
CALL kartenn_make_versioned('property_declaration');

-- REQ-003-16, P4: a resolved value is mandatory in a domain, for an entity, or for a type.
CREATE TABLE property_obligation (
    id           uuid NOT NULL,
    property_id  uuid NOT NULL,
    scope_kind   text NOT NULL CHECK (scope_kind IN ('domain', 'entity', 'object_type')),
    scope_id     uuid,
    scope_type   text,
    valid_period tstzrange NOT NULL CHECK (NOT isempty(valid_period)),
    sys_start    timestamptz NOT NULL DEFAULT now(),
    changeset_id uuid NOT NULL,
    PRIMARY KEY (id, valid_period WITHOUT OVERLAPS),
    CONSTRAINT property_obligation_scope CHECK (
        CASE scope_kind WHEN 'object_type' THEN scope_type IS NOT NULL AND scope_id IS NULL
                        ELSE scope_id IS NOT NULL AND scope_type IS NULL END),
    CONSTRAINT property_obligation_unique EXCLUDE USING GIST (
        property_id WITH =, scope_kind WITH =, (coalesce(scope_id::text, scope_type)) WITH =,
        valid_period WITH &&)
);
CALL kartenn_make_versioned('property_obligation');

-- REQ-003-7: closed-list values, each with its own validity.
CREATE TABLE property_enum_value (
    id           uuid NOT NULL,
    property_id  uuid NOT NULL,
    key          text NOT NULL CHECK (key ~ '^[a-z0-9]([a-z0-9-]*[a-z0-9])?$'),
    valid_period tstzrange NOT NULL CHECK (NOT isempty(valid_period)),
    sys_start    timestamptz NOT NULL DEFAULT now(),
    changeset_id uuid NOT NULL,
    PRIMARY KEY (id, valid_period WITHOUT OVERLAPS),
    CONSTRAINT property_enum_key_unique EXCLUDE USING GIST (
        property_id WITH =, key WITH =, valid_period WITH &&)
);
CALL kartenn_make_versioned('property_enum_value');

-- REQ-003-12, 13: one assignment per object and property at any time; `cleared` stops
-- inheritance without a value. Sets are sorted and deduplicated by the service (REQ-003-9).
CREATE TABLE property_assignment (
    object_id    uuid NOT NULL,
    object_type  text NOT NULL,
    property_id  uuid NOT NULL,
    kind         text NOT NULL CHECK (kind IN ('value', 'cleared')),
    v_uuid       uuid,
    v_uuids      uuid[],
    v_text       text,
    v_texts      text[],
    v_int        bigint,
    v_ints       bigint[],
    v_bool       boolean,
    v_date       date,
    v_dates      date[],
    -- `input`, or `source:<id>@<version>` for a default value (REQ-003-22).
    provenance   text NOT NULL CHECK (provenance = 'input' OR provenance ~ '^source:[^@]+@.+$'),
    valid_period tstzrange NOT NULL CHECK (NOT isempty(valid_period)),
    sys_start    timestamptz NOT NULL DEFAULT now(),
    changeset_id uuid NOT NULL,
    PRIMARY KEY (object_id, property_id, valid_period WITHOUT OVERLAPS),
    CONSTRAINT property_assignment_value CHECK (
        num_nonnulls(v_uuid, v_uuids, v_text, v_texts, v_int, v_ints, v_bool, v_date, v_dates)
        = CASE kind WHEN 'value' THEN 1 ELSE 0 END)
);
CALL kartenn_make_versioned('property_assignment', ARRAY['object_id', 'property_id']);
-- Value of an origin (join) and filters (REQ-003-25).
CREATE INDEX property_assignment_by_property ON property_assignment (property_id, object_id);
CREATE INDEX property_assignment_uuid ON property_assignment (property_id, v_uuid) WHERE v_uuid IS NOT NULL;
CREATE INDEX property_assignment_text ON property_assignment (property_id, v_text) WHERE v_text IS NOT NULL;
CREATE INDEX property_assignment_int ON property_assignment (property_id, v_int) WHERE v_int IS NOT NULL;
CREATE INDEX property_assignment_date ON property_assignment (property_id, v_date) WHERE v_date IS NOT NULL;
CREATE INDEX property_assignment_uuids ON property_assignment USING GIN (v_uuids) WHERE v_uuids IS NOT NULL;
CREATE INDEX property_assignment_texts ON property_assignment USING GIN (v_texts) WHERE v_texts IS NOT NULL;

-- REQ-003-11 to 15, P9: origin of each heritable property, per object and segment of
-- application time; null when no ancestor carries an assignment. The value is read by
-- joining the origin's assignment.
CREATE TABLE property_origin (
    object_id        uuid NOT NULL,
    property_id      uuid NOT NULL,
    origin_object_id uuid,
    valid_period     tstzrange NOT NULL CHECK (NOT isempty(valid_period)),
    sys_start        timestamptz NOT NULL DEFAULT now(),
    changeset_id     uuid NOT NULL,
    PRIMARY KEY (object_id, property_id, valid_period WITHOUT OVERLAPS)
);
CALL kartenn_make_versioned('property_origin', ARRAY['object_id', 'property_id']);
-- Filtering starts from the origins carrying the value (P9, spike 003).
CREATE INDEX property_origin_by_origin ON property_origin (property_id, origin_object_id);

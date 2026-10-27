-- The VLAN in the core (spec 026, REQ-026-1 to 3; decisions C1 to C4): the groups of VLAN
-- and the VLANs leave the module `vlan` (spec 008) for the core.
--
-- Their tables move from the schema `vlan` to the schema of the core, versions and
-- histories included (`ALTER TABLE … SET SCHEMA`, no copy): the bitemporal history stays
-- intact. A database without the module gets the same tables, as its migrations 0001 and
-- 0002 created them. The objects leave the record of the modules (`module_object`) and
-- the module its registry (`module_migration`). The names follow the core (C3): the
-- property `vlan.vlan` becomes `core.vlan`, the types `vlan.group` and `vlan.vlan` the
-- kinds `vlan_group` and `vlan`, the rules `vlan.*` the rules `core.vlan.*`, the
-- operations `vlan.<name>` those of the core; the journal of the changesets, the
-- anomalies, the drafts and the imports read back under the new names. The rows of the
-- versioned tables are renamed in place, their system time untouched (the versioning
-- trigger is held), so that a read at any date (`valid_at`, `as_of`) finds the same
-- objects. The custom roles take the new permissions by the system changeset of the
-- migration job (`rbac::rename_permissions`), as for migration 0071; the audit journal
-- stays historical. Every statement is a no-op the second time.

-- ---------------------------------------------------------------- the tables

DO $$
DECLARE
    part oid;
BEGIN
    IF to_regclass('vlan.vlan_group') IS NOT NULL THEN
        -- A partitioned history moves without its partitions: each moves on its own.
        FOR part IN
            SELECT i.inhrelid FROM pg_inherits i
             WHERE i.inhparent IN (to_regclass('vlan.vlan_group_history'),
                                   to_regclass('vlan.vlan_history'))
        LOOP
            EXECUTE format('ALTER TABLE %s SET SCHEMA public', part::regclass);
        END LOOP;
        ALTER TABLE vlan.vlan_group_history SET SCHEMA public;
        ALTER TABLE vlan.vlan_history SET SCHEMA public;
        ALTER TABLE vlan.vlan_group SET SCHEMA public;
        ALTER TABLE vlan.vlan SET SCHEMA public;
    ELSIF to_regclass('public.vlan_group') IS NULL THEN
        -- `group` is a reserved word: the table of the groups is `vlan_group`.
        CREATE TABLE public.vlan_group (
            id            uuid NOT NULL,
            slug          text NOT NULL CHECK (slug ~ '^[a-z0-9]([a-z0-9-]{0,61}[a-z0-9])?$'),
            anchor_kind   text NOT NULL CHECK (anchor_kind IN ('entity', 'site', 'domain')),
            anchor_id     uuid NOT NULL,
            vid_min       smallint NOT NULL DEFAULT 1,
            vid_max       smallint NOT NULL DEFAULT 4094,
            valid_period  tstzrange NOT NULL CHECK (NOT isempty(valid_period)),
            sys_start     timestamptz NOT NULL DEFAULT now(),
            changeset_id  uuid NOT NULL,
            PRIMARY KEY (id, valid_period WITHOUT OVERLAPS),
            CHECK (1 <= vid_min AND vid_min <= vid_max AND vid_max <= 4094),
            CONSTRAINT vlan_group_slug_unique
                EXCLUDE USING GIST (slug WITH =, valid_period WITH &&)
        );
        CALL public.kartenn_make_versioned('public.vlan_group');
        CREATE INDEX vlan_group_keyset ON public.vlan_group (slug, id);
        CREATE TABLE public.vlan (
            id            uuid NOT NULL,
            group_id      uuid NOT NULL,
            vid           smallint NOT NULL CHECK (vid BETWEEN 1 AND 4094),
            name          text NOT NULL CHECK (char_length(name) BETWEEN 1 AND 64),
            status        text NOT NULL CHECK (status IN ('active', 'reserved', 'deprecated')),
            valid_period  tstzrange NOT NULL CHECK (NOT isempty(valid_period)),
            sys_start     timestamptz NOT NULL DEFAULT now(),
            changeset_id  uuid NOT NULL,
            PRIMARY KEY (id, valid_period WITHOUT OVERLAPS),
            CONSTRAINT vlan_vid_unique
                EXCLUDE USING GIST (group_id WITH =, vid WITH =, valid_period WITH &&),
            CONSTRAINT vlan_name_unique
                EXCLUDE USING GIST (group_id WITH =, name WITH =, valid_period WITH &&)
        );
        CALL public.kartenn_make_versioned('public.vlan');
        CREATE INDEX vlan_by_group_vid ON public.vlan (group_id, vid);
    END IF;
    -- The schema of the module, empty once its tables left.
    IF to_regnamespace('vlan') IS NOT NULL
       AND NOT EXISTS (SELECT FROM pg_class WHERE relnamespace = to_regnamespace('vlan'))
       AND NOT EXISTS (SELECT FROM pg_proc WHERE pronamespace = to_regnamespace('vlan')) THEN
        DROP SCHEMA vlan;
    END IF;
END $$;

-- ---------------------------------------------------------------- the registry of modules

-- The record of the objects of the module ends without a trace: the tables of the core
-- hold the versions now. The trigger writes the deleted rows to the history first.
DELETE FROM module_object WHERE module = 'vlan';
DELETE FROM module_object_history WHERE module = 'vlan';
DELETE FROM module_migration WHERE module = 'vlan';

-- ---------------------------------------------------------------- the names

-- A name of the module as the core names it now, or itself: an operation, a type, a
-- rule, a permission, the property (`ctx`, the member that holds the string, tells the
-- type `vlan.vlan` from the property).
CREATE OR REPLACE FUNCTION pg_temp.k026_name(s text, ctx text) RETURNS text
LANGUAGE sql IMMUTABLE AS $$
    SELECT CASE
        WHEN s = 'vlan.vlan' THEN
            CASE WHEN ctx IN ('key', 'property', 'property_key', 'property_keys')
                 THEN 'core.vlan' ELSE 'vlan' END
        WHEN s LIKE 'prop.vlan.vlan.%' THEN 'prop.core.vlan.' || substr(s, 16)
        ELSE coalesce((SELECT m.new FROM (VALUES
            ('vlan.create_group', 'create_vlan_group'),
            ('vlan.update_group', 'update_vlan_group'),
            ('vlan.create_vlan', 'create_vlan'),
            ('vlan.update_vlan', 'update_vlan'),
            ('vlan.group', 'vlan_group'),
            ('vlan.vid.invalid', 'core.vlan.vid_invalid'),
            ('vlan.vid.out_of_range', 'core.vlan.vid_out_of_range'),
            ('vlan.vid.duplicate', 'core.vlan.vid_duplicate'),
            ('vlan.name.invalid', 'core.vlan.name_invalid'),
            ('vlan.name.duplicate', 'core.vlan.name_duplicate'),
            ('vlan.group.anchor', 'core.vlan.group_anchor'),
            ('vlan.prefix.not_network', 'core.vlan.prefix_not_network'),
            ('vlan.site.mismatch', 'core.vlan.site_mismatch'),
            ('vlan.domain.mismatch', 'core.vlan.domain_mismatch'),
            ('vlan.read', 'core.vlan.read'),
            ('vlan.edit', 'core.vlan.edit'),
            ('vlan.apply', 'core.vlan.apply'),
            ('vlan.approval.self', 'core.vlan.approval.self'),
            ('vlan.group.read', 'core.vlan_group.read'),
            ('vlan.group.edit', 'core.vlan_group.edit'),
            ('vlan.group.apply', 'core.vlan_group.apply'),
            ('vlan.group.approval.self', 'core.vlan_group.approval.self')
        ) AS m(old, new) WHERE m.old = s), s)
    END
$$;

-- `j` with every string renamed by `k026_name`, the member that holds it as context.
CREATE OR REPLACE FUNCTION pg_temp.k026(j jsonb, ctx text DEFAULT NULL) RETURNS jsonb
LANGUAGE plpgsql IMMUTABLE AS $$
BEGIN
    CASE jsonb_typeof(j)
        WHEN 'object' THEN
            RETURN (SELECT coalesce(jsonb_object_agg(e.k, pg_temp.k026(e.v, e.k)), '{}'::jsonb)
                      FROM jsonb_each(j) AS e(k, v));
        WHEN 'array' THEN
            RETURN (SELECT coalesce(jsonb_agg(pg_temp.k026(e.v, ctx) ORDER BY e.n), '[]'::jsonb)
                      FROM jsonb_array_elements(j) WITH ORDINALITY AS e(v, n));
        WHEN 'string' THEN
            RETURN to_jsonb(pg_temp.k026_name(j #>> '{}', ctx));
        ELSE
            RETURN j;
    END CASE;
END $$;

-- The versioned tables are renamed under a held trigger: no new version, no history row.
ALTER TABLE property_declaration DISABLE TRIGGER versioning;
ALTER TABLE property_obligation DISABLE TRIGGER versioning;
ALTER TABLE property_assignment DISABLE TRIGGER versioning;

UPDATE property_declaration
   SET key = pg_temp.k026_name(key, 'key'),
       carriers = ARRAY(SELECT pg_temp.k026_name(c, 'kind') FROM unnest(carriers) AS c),
       type_params = pg_temp.k026(type_params)
 WHERE key = 'vlan.vlan'
    OR carriers && ARRAY['vlan.group', 'vlan.vlan']
    OR type_params ->> 'target' IN ('vlan.group', 'vlan.vlan');
UPDATE property_declaration_history
   SET key = pg_temp.k026_name(key, 'key'),
       carriers = ARRAY(SELECT pg_temp.k026_name(c, 'kind') FROM unnest(carriers) AS c),
       type_params = pg_temp.k026(type_params)
 WHERE key = 'vlan.vlan'
    OR carriers && ARRAY['vlan.group', 'vlan.vlan']
    OR type_params ->> 'target' IN ('vlan.group', 'vlan.vlan');

UPDATE property_obligation SET scope_type = pg_temp.k026_name(scope_type, 'kind')
 WHERE scope_type IN ('vlan.group', 'vlan.vlan');
UPDATE property_obligation_history SET scope_type = pg_temp.k026_name(scope_type, 'kind')
 WHERE scope_type IN ('vlan.group', 'vlan.vlan');

UPDATE property_assignment SET object_type = pg_temp.k026_name(object_type, 'kind')
 WHERE object_type IN ('vlan.group', 'vlan.vlan');
UPDATE property_assignment_history SET object_type = pg_temp.k026_name(object_type, 'kind')
 WHERE object_type IN ('vlan.group', 'vlan.vlan');

ALTER TABLE property_declaration ENABLE TRIGGER versioning;
ALTER TABLE property_obligation ENABLE TRIGGER versioning;
ALTER TABLE property_assignment ENABLE TRIGGER versioning;

UPDATE external_id SET object_type = pg_temp.k026_name(object_type, 'kind')
 WHERE object_type IN ('vlan.group', 'vlan.vlan');

-- The journal of the changesets: its operations, its request, the violations, anomalies
-- and requirements it recorded, its differential and its cascade.
UPDATE changeset_operation SET payload = pg_temp.k026(payload)
 WHERE payload::text LIKE '%vlan.%';
UPDATE changeset
   SET request = pg_temp.k026(request),
       violations = pg_temp.k026(violations),
       anomalies = pg_temp.k026(anomalies),
       required_rights = pg_temp.k026(required_rights),
       diff_summary = pg_temp.k026(diff_summary),
       cascaded = pg_temp.k026(cascaded)
 WHERE request::text LIKE '%vlan.%' OR violations::text LIKE '%vlan.%'
    OR anomalies::text LIKE '%vlan.%' OR required_rights::text LIKE '%vlan.%'
    OR diff_summary::text LIKE '%vlan.%' OR cascaded::text LIKE '%vlan.%';
UPDATE changeset_diff SET property_key = 'core.vlan' WHERE property_key = 'vlan.vlan';
UPDATE approval_policy
   SET property_keys = ARRAY(SELECT pg_temp.k026_name(k, 'property_keys')
                               FROM unnest(property_keys) AS k)
 WHERE 'vlan.vlan' = ANY (property_keys);
UPDATE anomaly SET rule_id = pg_temp.k026_name(rule_id, 'rule') WHERE rule_id LIKE 'vlan.%';
UPDATE draft SET operations = pg_temp.k026(operations) WHERE operations::text LIKE '%vlan.%';
UPDATE import_chunk SET elements = pg_temp.k026(elements) WHERE elements::text LIKE '%vlan.%';

-- ---------------------------------------------------------------- the trees

-- The tree of the VLANs: a group, then its VLANs. Their properties inherit along it, as
-- those of the objects of a module along the parents of their record (REQ-008-6).
CREATE OR REPLACE VIEW vlan_tree AS
    SELECT id, NULL::uuid AS parent_id, valid_period FROM vlan_group
    UNION ALL
    SELECT id, group_id, valid_period FROM vlan;
GRANT SELECT ON vlan_tree TO kartenn_app;

-- The carriers of the properties gain the groups and the VLANs, with the domain of their
-- anchor (a domain scope reaches them), version by version of their group for a VLAN.
CREATE OR REPLACE VIEW carrier_object AS
    SELECT id, domain_id, value, kind, valid_period FROM addressing_object
    UNION ALL
    SELECT id, NULL::uuid, NULL::inet, 'entity', valid_period FROM entity
    UNION ALL
    SELECT id, NULL::uuid, NULL::inet, 'site', valid_period FROM site
    UNION ALL
    SELECT id, domain_id, NULL::inet, kind, valid_period FROM module_object
    UNION ALL
    SELECT id, CASE WHEN anchor_kind = 'domain' THEN anchor_id END, NULL::inet,
           'vlan_group', valid_period
      FROM vlan_group
    UNION ALL
    SELECT v.id, CASE WHEN g.anchor_kind = 'domain' THEN g.anchor_id END, NULL::inet,
           'vlan', v.valid_period * g.valid_period
      FROM vlan v
      JOIN vlan_group g ON g.id = v.group_id AND g.valid_period && v.valid_period;

-- The site of an object at `at` (migration 0042): a VLAN has the site of its group, as an
-- object of a module the site of its nearest ancestor that carries one.
CREATE OR REPLACE FUNCTION public.kartenn_site(object uuid, at timestamptz) RETURNS uuid
LANGUAGE sql STABLE PARALLEL SAFE SET search_path = pg_catalog, public AS $$
    WITH RECURSIVE o AS (SELECT kind, domain_id, value FROM addressing_object
               WHERE id = object AND valid_period @> at),
         network AS (
            SELECT p.id FROM prefix p, o
            WHERE p.domain_id = o.domain_id AND p.nature = 'network' AND p.valid_period @> at
              AND o.kind IN ('prefix', 'range', 'address') AND p.prefix >>= o.value
            LIMIT 1),
         tree AS (
            SELECT id, parent_id, valid_period FROM module_object
            UNION ALL
            SELECT id, parent_id, valid_period FROM vlan_tree),
         up(id, depth) AS (
            SELECT m.id, 0 FROM tree m WHERE m.id = object AND m.valid_period @> at
            UNION ALL
            SELECT m.parent_id, up.depth + 1
            FROM up JOIN tree m ON m.id = up.id AND m.valid_period @> at
            WHERE m.parent_id IS NOT NULL AND up.depth < 1000)
    SELECT CASE
        WHEN EXISTS (SELECT 1 FROM site s WHERE s.id = object AND s.valid_period @> at)
            THEN object
        ELSE coalesce(
            (SELECT a.v_uuid
             FROM network n
             JOIN property_declaration d ON d.key = 'core.site' AND d.valid_period @> at
             JOIN property_assignment a ON a.object_id = n.id AND a.property_id = d.id
                                       AND a.valid_period @> at AND a.kind = 'value'
             LIMIT 1),
            (SELECT a.v_uuid
             FROM up
             JOIN property_declaration d ON d.key = 'core.site' AND d.valid_period @> at
             JOIN property_assignment a ON a.object_id = up.id AND a.property_id = d.id
                                       AND a.valid_period @> at AND a.kind = 'value'
             ORDER BY up.depth
             LIMIT 1))
    END
$$;

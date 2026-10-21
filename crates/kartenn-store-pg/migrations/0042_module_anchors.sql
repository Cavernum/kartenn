-- Anchors and access control of the objects of modules (spec 008, REQ-008-6, 7; task 008-5;
-- ADR 0014, 0021, 0026 clause 4).
--
-- An object of a module carries properties like an object of the core: it joins the
-- carriers, with its domain when its type is anchored in one (a domain scope reaches it),
-- and no value (a prefix scope never does).
CREATE OR REPLACE VIEW carrier_object AS
    SELECT id, domain_id, value, kind, valid_period FROM addressing_object
    UNION ALL
    SELECT id, NULL::uuid, NULL::inet, 'entity', valid_period FROM entity
    UNION ALL
    SELECT id, NULL::uuid, NULL::inet, 'site', valid_period FROM site
    UNION ALL
    SELECT id, domain_id, NULL::inet, kind, valid_period FROM module_object;

-- The site of an object at `at` (migration 0028), and for an object of a module the
-- `core.site` assigned to it or, failing that, to its nearest ancestor in the tree of its
-- module: the site of a VLAN is the one of its group, as the site of an address is the
-- one of its network. Its owner needs no change: `core.owner` is inherited along the tree
-- of the module and materialised (`property_origin`).
CREATE OR REPLACE FUNCTION public.kartenn_site(object uuid, at timestamptz) RETURNS uuid
LANGUAGE sql STABLE PARALLEL SAFE SET search_path = pg_catalog, public AS $$
    WITH RECURSIVE o AS (SELECT kind, domain_id, value FROM addressing_object
               WHERE id = object AND valid_period @> at),
         network AS (
            SELECT p.id FROM prefix p, o
            WHERE p.domain_id = o.domain_id AND p.nature = 'network' AND p.valid_period @> at
              AND o.kind IN ('prefix', 'range', 'address') AND p.prefix >>= o.value
            LIMIT 1),
         up(id, depth) AS (
            SELECT m.id, 0 FROM module_object m WHERE m.id = object AND m.valid_period @> at
            UNION ALL
            SELECT m.parent_id, up.depth + 1
            FROM up JOIN module_object m ON m.id = up.id AND m.valid_period @> at
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

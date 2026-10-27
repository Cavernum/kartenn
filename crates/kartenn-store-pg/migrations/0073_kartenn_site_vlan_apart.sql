-- The site of an object at `at` (migration 0072), its VLANs read apart: the common table
-- `tree` of 0072, the objects of the modules and the VLANs in one union, was materialised
-- whole at each call, and even inlined the recursion joined the union row by row. The read
-- predicate calls this function row by row, which took a page of machines from 37 ms to
-- 14 s on the full bench. The recursion goes back to the tree of the modules alone, as in
-- 0042, and a VLAN, one level under its group, is read by its identifier.
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
            WHERE m.parent_id IS NOT NULL AND up.depth < 1000),
         vlan_up(id, depth) AS (
            SELECT g.id, 0 FROM vlan_group g WHERE g.id = object AND g.valid_period @> at
            UNION ALL
            SELECT v.id, 0 FROM vlan v WHERE v.id = object AND v.valid_period @> at
            UNION ALL
            SELECT v.group_id, 1 FROM vlan v WHERE v.id = object AND v.valid_period @> at),
         carriers AS (SELECT id, depth FROM up UNION ALL SELECT id, depth FROM vlan_up)
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
             FROM carriers c
             JOIN property_declaration d ON d.key = 'core.site' AND d.valid_period @> at
             JOIN property_assignment a ON a.object_id = c.id AND a.property_id = d.id
                                       AND a.valid_period @> at AND a.kind = 'value'
             ORDER BY c.depth
             LIMIT 1))
    END
$$;

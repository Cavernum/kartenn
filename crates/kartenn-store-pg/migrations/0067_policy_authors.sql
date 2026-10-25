-- Authors of the approval policies (spec 025, REQ-025-13, G10; task 025-6): a role, held
-- through a group, or a group of kartenn (`{"role": key}`, `{"group": key}`); never an
-- account. No compatibility, but no silent loss (G9): the entries naming an account
-- (`{"person": id}`) or a group of an issuer (`{"group": [issuer, group]}`) are dropped,
-- each policy listed in a warning of the migration; a policy left without any trigger is
-- disabled, and listed, rather than kept enabled holding nothing.

DO $$
DECLARE
    p record;
BEGIN
    FOR p IN
        SELECT id, name,
               jsonb_agg(a ORDER BY n)
                   FILTER (WHERE a ? 'person' OR jsonb_typeof(a -> 'group') IS NOT DISTINCT FROM 'array')
                   AS dropped,
               COALESCE(jsonb_agg(a ORDER BY n) FILTER (
                   WHERE NOT (a ? 'person' OR jsonb_typeof(a -> 'group') IS NOT DISTINCT FROM 'array')), '[]')
                   AS kept
        FROM approval_policy, jsonb_array_elements(authors) WITH ORDINALITY AS e (a, n)
        GROUP BY id, name
        HAVING bool_or(a ? 'person' OR jsonb_typeof(a -> 'group') IS NOT DISTINCT FROM 'array')
        ORDER BY name
    LOOP
        UPDATE approval_policy SET authors = p.kept WHERE id = p.id;
        RAISE WARNING 'approval policy "%": authors % dropped, an author is a role or a group of kartenn (REQ-025-13)',
            p.name, p.dropped;
    END LOOP;
    FOR p IN
        UPDATE approval_policy SET enabled = false
        WHERE enabled AND min_operations IS NULL AND min_diff_objects IS NULL
          AND property_keys = '{}' AND perimeters = '[]' AND authors = '[]'
          AND module_triggers = '[]'
        RETURNING name
    LOOP
        RAISE WARNING 'approval policy "%": no trigger left, disabled (REQ-025-13)', p.name;
    END LOOP;
END
$$;

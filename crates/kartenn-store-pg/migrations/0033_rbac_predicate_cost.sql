-- Cost of the predicates of access control (spec 005, REQ-005-29; bench 005-16), with the
-- same meaning as migrations 0029 and 0030.
--
-- `kartenn_visible` tests the cheap scopes first (global, domain, prefix subtrees) and
-- resolves the owner or the site of the object only when an entity, a site or a
-- confinement asks for it: the owner climbs the origin of `core.owner`. CASE fixes the
-- order of evaluation. Never null (see 0029).
CREATE OR REPLACE FUNCTION public.kartenn_visible(
    object uuid, domain_id uuid, value inet, at timestamptz,
    global boolean, entities uuid[], sites uuid[], domains uuid[],
    prefix_domains uuid[], prefixes cidr[], confinement uuid[])
RETURNS boolean
LANGUAGE sql STABLE PARALLEL SAFE SET search_path = pg_catalog, public AS $$
    SELECT CASE
        WHEN confinement IS NOT NULL
             AND NOT coalesce(public.kartenn_owner(object, at) = ANY(confinement), false)
            THEN false
        WHEN global THEN true
        WHEN domain_id = ANY(domains) THEN true
        WHEN cardinality(prefixes) > 0
             AND EXISTS (SELECT 1 FROM unnest(prefix_domains, prefixes) AS p(d, n)
                         WHERE p.d = domain_id AND value <<= p.n)
            THEN true
        WHEN cardinality(entities) > 0
             AND coalesce(public.kartenn_owner(object, at) = ANY(entities), false)
            THEN true
        WHEN cardinality(sites) > 0
             AND coalesce(public.kartenn_site(object, at) = ANY(sites), false)
            THEN true
        ELSE false
    END
$$;

-- A read scope (the JSON of `ReadScope`) as one row of typed arrays per kind, for a query
-- to materialise once: `kartenn_readable` rebuilt the arrays from the JSON on every row,
-- a cost of the rows times the bindings.
CREATE FUNCTION public.kartenn_scopes(scopes jsonb)
RETURNS TABLE (
    kind text, global boolean, entities uuid[], sites uuid[], domains uuid[],
    prefix_domains uuid[], prefixes cidr[], confinement uuid[])
LANGUAGE sql IMMUTABLE PARALLEL SAFE SET search_path = pg_catalog, public AS $$
    SELECT k.kind,
           coalesce((s ->> 'global')::boolean, false),
           ARRAY(SELECT jsonb_array_elements_text(s -> 'entities'))::uuid[],
           ARRAY(SELECT jsonb_array_elements_text(s -> 'sites'))::uuid[],
           ARRAY(SELECT jsonb_array_elements_text(s -> 'domains'))::uuid[],
           ARRAY(SELECT jsonb_array_elements_text(s -> 'prefix_domains'))::uuid[],
           ARRAY(SELECT jsonb_array_elements_text(s -> 'prefixes'))::cidr[],
           CASE WHEN s -> 'confinement' IS NULL OR jsonb_typeof(s -> 'confinement') = 'null'
                THEN NULL
                ELSE ARRAY(SELECT jsonb_array_elements_text(s -> 'confinement'))::uuid[] END
    FROM jsonb_each(scopes) AS k(kind, s)
$$;
GRANT EXECUTE ON FUNCTION public.kartenn_scopes(jsonb) TO kartenn_app;

DROP FUNCTION public.kartenn_readable(uuid, text, uuid, inet, timestamptz, jsonb);

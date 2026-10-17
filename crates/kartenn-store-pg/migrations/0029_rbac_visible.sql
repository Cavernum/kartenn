-- Whether an object is in a visibility (spec 005, REQ-005-18 to 20, 24; ADR 0021): the
-- same predicate decides `authorize_many` and filters the lists. A visibility is the union
-- of the scopes of the subject's bindings for one permission (global, entities and sites
-- with their descendants, domains, prefix subtrees), bounded by the confinement of its
-- issuer (the entities it may reach, with their descendants; null: no bound).
-- `domain_id` and `value` are those of `carrier_object` (null for an entity or a site).
CREATE FUNCTION public.kartenn_visible(
    object uuid, domain_id uuid, value inet, at timestamptz,
    global boolean, entities uuid[], sites uuid[], domains uuid[],
    prefix_domains uuid[], prefixes cidr[], confinement uuid[])
RETURNS boolean
LANGUAGE sql STABLE PARALLEL SAFE SET search_path = pg_catalog, public AS $$
    -- Never null: a comparison with an object without owner, domain or site is null, and a
    -- null would read as "not refused" in `WHERE NOT kartenn_visible(…)` (fail-open).
    WITH owner AS (SELECT public.kartenn_owner(object, at) AS id)
    SELECT coalesce((global
            OR (SELECT id FROM owner) = ANY(entities)
            OR (cardinality(sites) > 0 AND public.kartenn_site(object, at) = ANY(sites))
            OR domain_id = ANY(domains)
            OR EXISTS (SELECT 1 FROM unnest(prefix_domains, prefixes) AS p(d, n)
                       WHERE p.d = domain_id AND value <<= p.n))
       AND (confinement IS NULL OR (SELECT id FROM owner) = ANY(confinement)), false)
$$;
GRANT EXECUTE ON FUNCTION public.kartenn_visible(
    uuid, uuid, inet, timestamptz, boolean, uuid[], uuid[], uuid[], uuid[], cidr[], uuid[])
    TO kartenn_app;

-- Whether an object of `kind` may be read under a read scope (spec 005, REQ-005-24): a
-- list mixes kinds of objects, each with its read permission; `scopes` maps a kind to the
-- visibility of `core.<kind>.read` (`global`, `entities`, `sites`, `domains`,
-- `prefix_domains`, `prefixes`, `confinement`, as in `kartenn_visible`). A kind absent from
-- the scopes is not readable. Never null (see migration 0029).
CREATE FUNCTION public.kartenn_readable(
    object uuid, kind text, domain_id uuid, value inet, at timestamptz, scopes jsonb)
RETURNS boolean
LANGUAGE sql STABLE PARALLEL SAFE SET search_path = pg_catalog, public AS $$
    SELECT coalesce((
        SELECT public.kartenn_visible(
            object, domain_id, value, at,
            coalesce((s ->> 'global')::boolean, false),
            ARRAY(SELECT jsonb_array_elements_text(s -> 'entities'))::uuid[],
            ARRAY(SELECT jsonb_array_elements_text(s -> 'sites'))::uuid[],
            ARRAY(SELECT jsonb_array_elements_text(s -> 'domains'))::uuid[],
            ARRAY(SELECT jsonb_array_elements_text(s -> 'prefix_domains'))::uuid[],
            ARRAY(SELECT jsonb_array_elements_text(s -> 'prefixes'))::cidr[],
            CASE WHEN s -> 'confinement' IS NULL OR jsonb_typeof(s -> 'confinement') = 'null'
                 THEN NULL
                 ELSE ARRAY(SELECT jsonb_array_elements_text(s -> 'confinement'))::uuid[] END)
        FROM (SELECT scopes -> kind AS s) AS k
        WHERE s IS NOT NULL), false)
$$;
GRANT EXECUTE ON FUNCTION public.kartenn_readable(uuid, text, uuid, inet, timestamptz, jsonb)
    TO kartenn_app;

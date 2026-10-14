-- IANA special-purpose registries as reference data, one copy per registry version
-- (REQ-002-34). Loaded by the migration job from the registries embedded in
-- kartenn-core (kartenn_store_pg::registry::load).
CREATE TABLE special_registry (
    registry_version     text NOT NULL,
    block                cidr NOT NULL,
    name                 text NOT NULL,
    rfc                  text NOT NULL,
    terminated           text,
    globally_reachable   text NOT NULL CHECK (globally_reachable IN ('true', 'false', 'n/a', 'unspecified')),
    PRIMARY KEY (registry_version, block)
);
CREATE INDEX special_registry_block ON special_registry USING gist (block inet_ops);
GRANT SELECT ON special_registry TO kartenn_app;

-- Public space (002/A3): no active entry contains the value, or the most specific one is
-- Globally Reachable. Same definition as kartenn_core::registry::SpecialRegistry::is_public.
CREATE FUNCTION public.kartenn_is_public(v inet, version text) RETURNS boolean
LANGUAGE sql STABLE PARALLEL SAFE SET search_path = pg_catalog, public AS $$
    SELECT coalesce((
        SELECT r.globally_reachable = 'true'
        FROM public.special_registry r
        WHERE r.registry_version = version AND r.terminated IS NULL AND r.block >>= v
        ORDER BY masklen(r.block) DESC
        LIMIT 1
    ), true)
$$;

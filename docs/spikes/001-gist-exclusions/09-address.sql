-- Spike 001 (c): per-domain address uniqueness and partial public uniqueness (A3, A11).
DROP TABLE IF EXISTS address;
CREATE TABLE address (
    id uuid PRIMARY KEY, domain_id uuid NOT NULL, address inet NOT NULL,
    usage text NOT NULL DEFAULT 'host', public boolean NOT NULL DEFAULT false,
    valid_period tstzrange NOT NULL DEFAULT tstzrange('2024-01-01', NULL),
    CONSTRAINT address_unique_per_domain EXCLUDE USING GIST (
        domain_id WITH =, address inet_ops WITH =, valid_period WITH &&),
    CONSTRAINT address_public_unique EXCLUDE USING GIST (
        address inet_ops WITH =, valid_period WITH &&) WHERE (public AND usage <> 'anycast')
);
CREATE TEMP TABLE public_domain AS SELECT DISTINCT domain_id FROM prefix
WHERE family(prefix) = 4 AND prefix <<= '20.0.0.0/6' OR family(prefix) = 6 AND prefix <<= '2a00::/16';
\timing on
\echo '-- load 10^6 addresses under both exclusions'
INSERT INTO address (id, domain_id, address, public)
SELECT a.id, a.domain_id, a.address, a.domain_id IN (SELECT domain_id FROM public_domain) FROM address_src a;
\timing off
SELECT count(*) FILTER (WHERE public) AS public, count(*) AS total,
       pg_size_pretty(pg_relation_size('address_unique_per_domain')) AS idx_domain,
       pg_size_pretty(pg_relation_size('address_public_unique')) AS idx_public
FROM address;
\set ON_ERROR_STOP off
\echo '-- same public address, other domain, host: must fail'
INSERT INTO address (id, domain_id, address, public)
SELECT gen_random_uuid(), gen_random_uuid(), address, true FROM address WHERE public LIMIT 1;
\echo '-- same private address, other domain: must succeed'
INSERT INTO address (id, domain_id, address)
SELECT gen_random_uuid(), gen_random_uuid(), address FROM address WHERE NOT public LIMIT 1;
\echo '-- public address anycast in two domains: must succeed'
INSERT INTO address (id, domain_id, address, usage, public) VALUES
  (gen_random_uuid(), gen_random_uuid(), '45.0.0.53', 'anycast', true),
  (gen_random_uuid(), gen_random_uuid(), '45.0.0.53', 'anycast', true);
\echo '-- anycast in one domain, host in another: NOT caught by the partial exclusion (application rule)'
INSERT INTO address (id, domain_id, address, usage, public) VALUES
  (gen_random_uuid(), gen_random_uuid(), '45.0.0.53', 'host', true);
SELECT count(*) AS rows_for_45_0_0_53 FROM address WHERE address = '45.0.0.53';

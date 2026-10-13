-- Spike 001 (b), variant D: exact diff through numeric (no exception, no precision loss
-- before the final float8 conversion). Probes forced through LATERAL index lookups.
DROP TABLE IF EXISTS range_d;
DROP TYPE IF EXISTS inetrange_d;
DROP FUNCTION IF EXISTS inet_exact_diff(inet, inet), inet_as_numeric(inet);

CREATE FUNCTION inet_as_numeric(x inet) RETURNS numeric
LANGUAGE sql IMMUTABLE STRICT PARALLEL SAFE AS $$
    SELECT sum(get_byte(inet_send(x), 4 + i)::numeric * (256::numeric ^ (n - 1 - i)))
    FROM (SELECT get_byte(inet_send(x), 3) AS n) AS len, generate_series(0, n - 1) AS i
$$;

CREATE FUNCTION inet_exact_diff(x inet, y inet) RETURNS float8
LANGUAGE sql IMMUTABLE STRICT PARALLEL SAFE AS $$
    SELECT CASE
        WHEN family(x) = family(y) THEN (inet_as_numeric(x) - inet_as_numeric(y))::float8
        WHEN family(x) > family(y) THEN 1e300::float8
        ELSE -1e300::float8
    END
$$;

CREATE TYPE inetrange_d AS RANGE (subtype = inet, subtype_diff = inet_exact_diff);
CREATE TABLE range_d (
    id uuid PRIMARY KEY, domain_id uuid NOT NULL, r inetrange_d NOT NULL,
    valid_period tstzrange NOT NULL DEFAULT tstzrange('2024-01-01', NULL),
    EXCLUDE USING GIST (domain_id WITH =, r WITH &&, valid_period WITH &&)
);
\echo '-- sanity'
SELECT inet_exact_diff('2001:db8::10', '2001:db8::1') AS v6_15, inet_exact_diff('10.0.0.5', '10.0.0.1') AS v4_4;
\timing on
\echo '-- load, variant D'
INSERT INTO range_d SELECT id, domain_id, inetrange_d(first, last, '[]') FROM range_src;
\timing off
ANALYZE range_a; ANALYZE range_c; ANALYZE range_d;
CREATE TEMP TABLE probe AS
SELECT domain_id, address FROM address_src TABLESAMPLE SYSTEM (1) REPEATABLE (7) LIMIT 2000;
\echo '-- LATERAL probe, variant A (no diff)'
EXPLAIN (ANALYZE, TIMING OFF, SUMMARY ON)
SELECT count(*) FROM probe p CROSS JOIN LATERAL (
  SELECT 1 FROM range_a r WHERE r.domain_id = p.domain_id AND r.r @> p.address AND r.valid_period @> now()) x;
\echo '-- LATERAL probe, variant C (float8 diff)'
EXPLAIN (ANALYZE, TIMING OFF, SUMMARY ON)
SELECT count(*) FROM probe p CROSS JOIN LATERAL (
  SELECT 1 FROM range_c r WHERE r.domain_id = p.domain_id AND r.r @> p.address::inet AND r.valid_period @> now()) x;
\echo '-- LATERAL probe, variant D (numeric diff)'
EXPLAIN (ANALYZE, TIMING OFF, SUMMARY ON)
SELECT count(*) FROM probe p CROSS JOIN LATERAL (
  SELECT 1 FROM range_d r WHERE r.domain_id = p.domain_id AND r.r @> p.address AND r.valid_period @> now()) x;

-- Spike 001 (b), variant C: exception-free subtype_diff computed from the address bytes.
-- inet_send(x) = family, bits, is_cidr, nb, then nb address bytes (4 or 16).
DROP TABLE IF EXISTS range_b, range_c;
DROP TYPE IF EXISTS inetrange_b, inetrange_c;
DROP FUNCTION IF EXISTS inet_approx_diff(inet, inet), inet_as_float8(inet);

-- Unsigned value of the address as float8 (exact up to 2^53, approximate beyond:
-- enough for a GiST penalty hint). Never raises.
CREATE FUNCTION inet_as_float8(x inet) RETURNS float8
LANGUAGE sql IMMUTABLE STRICT PARALLEL SAFE AS $$
    SELECT CASE family(x)
        WHEN 4 THEN ('x' || encode(substr(inet_send(x), 5, 4), 'hex'))::bit(32)::bigint::float8
        ELSE
            ( ('x' || encode(substr(inet_send(x), 5, 8), 'hex'))::bit(64)::bigint::float8
              + CASE WHEN get_byte(inet_send(x), 4) >= 128 THEN 18446744073709551616.0::float8 ELSE 0 END
            ) * 18446744073709551616.0::float8
            + ('x' || encode(substr(inet_send(x), 13, 8), 'hex'))::bit(64)::bigint::float8
            + CASE WHEN get_byte(inet_send(x), 12) >= 128 THEN 18446744073709551616.0::float8 ELSE 0 END
    END
$$;

CREATE FUNCTION inet_approx_diff(x inet, y inet) RETURNS float8
LANGUAGE sql IMMUTABLE STRICT PARALLEL SAFE AS $$
    SELECT CASE
        WHEN family(x) = family(y) THEN inet_as_float8(x) - inet_as_float8(y)
        WHEN family(x) > family(y) THEN 1e300::float8
        ELSE -1e300::float8
    END
$$;

CREATE TYPE inetrange_c AS RANGE (subtype = inet, subtype_diff = inet_approx_diff);
CREATE TABLE range_c (
    id uuid PRIMARY KEY, domain_id uuid NOT NULL, r inetrange_c NOT NULL,
    valid_period tstzrange NOT NULL DEFAULT tstzrange('2024-01-01', NULL),
    EXCLUDE USING GIST (domain_id WITH =, r WITH &&, valid_period WITH &&)
);

\echo '-- sanity of inet_as_float8'
SELECT inet_as_float8('0.0.0.1') AS one, inet_as_float8('255.255.255.255') AS max_v4,
       inet_approx_diff('2001:db8::10', '2001:db8::1') AS small_v6_diff,
       inet_approx_diff('ffff::', '::1') > 0 AS far_v6_positive;
\timing on
\echo '-- load, variant C'
INSERT INTO range_c SELECT id, domain_id, inetrange_c(first, last, '[]') FROM range_src;
\timing off
\set ON_ERROR_STOP off
\echo '-- overlapping range in the same domain and period: must fail'
INSERT INTO range_c (id, domain_id, r)
SELECT gen_random_uuid(), domain_id, inetrange_c(lower(r) + 50, upper(r) + 50, '[]') FROM range_c LIMIT 1;
\echo '-- adjacent range in the same domain: must succeed'
INSERT INTO range_c (id, domain_id, r)
SELECT gen_random_uuid(), domain_id, inetrange_c(upper(r) + 1, upper(r) + 10, '[]') FROM range_c LIMIT 1;
\echo '-- same range in another domain: must succeed'
INSERT INTO range_c (id, domain_id, r) SELECT gen_random_uuid(), gen_random_uuid(), r FROM range_c LIMIT 1;
SELECT count(*) AS rows_in_range_c FROM range_c;
\echo '-- probe: ranges containing 2000 sample addresses'
EXPLAIN (ANALYZE, TIMING OFF, SUMMARY ON)
SELECT count(*) FROM address_src a JOIN range_c r
  ON r.domain_id = a.domain_id AND r.r @> a.address AND r.valid_period @> now()
WHERE a.id IN (SELECT id FROM address_src TABLESAMPLE SYSTEM (1) REPEATABLE (7) LIMIT 2000);

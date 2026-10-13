-- Spike 001 (b): non-overlapping address ranges per domain and period.
-- Variant A: range type without subtype_diff. Variant B: with an approximate diff.
DROP TABLE IF EXISTS range_a, range_b, range_src;
DROP TYPE IF EXISTS inetrange_a, inetrange_b;
DROP FUNCTION IF EXISTS inet_approx_diff(inet, inet);

CREATE TYPE inetrange_a AS RANGE (subtype = inet);

-- inet - inet returns bigint and overflows for distant IPv6 addresses: the diff is only a
-- GiST penalty hint, so distant pairs saturate instead of raising an error.
CREATE FUNCTION inet_approx_diff(x inet, y inet) RETURNS float8
LANGUAGE plpgsql IMMUTABLE STRICT PARALLEL SAFE AS $$
BEGIN
    IF family(x) <> family(y) THEN
        RETURN CASE WHEN family(x) > family(y) THEN 1e38 ELSE -1e38 END;
    END IF;
    RETURN (x - y)::float8;
EXCEPTION WHEN numeric_value_out_of_range THEN
    RETURN CASE WHEN x > y THEN 9.2e18 ELSE -9.2e18 END;
END $$;
CREATE TYPE inetrange_b AS RANGE (subtype = inet, subtype_diff = inet_approx_diff);

CREATE TABLE range_src (id uuid, domain_id uuid, first inet, last inet);
CREATE TABLE range_a (
    id uuid PRIMARY KEY, domain_id uuid NOT NULL, r inetrange_a NOT NULL,
    valid_period tstzrange NOT NULL DEFAULT tstzrange('2024-01-01', NULL),
    EXCLUDE USING GIST (domain_id WITH =, r WITH &&, valid_period WITH &&)
);
CREATE TABLE range_b (
    id uuid PRIMARY KEY, domain_id uuid NOT NULL, r inetrange_b NOT NULL,
    valid_period tstzrange NOT NULL DEFAULT tstzrange('2024-01-01', NULL),
    EXCLUDE USING GIST (domain_id WITH =, r WITH &&, valid_period WITH &&)
);

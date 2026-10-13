-- Spike 001 (e): smallest containing prefix (deduced parent, D34) for sample addresses.
DROP TABLE IF EXISTS sample;
CREATE TEMP TABLE sample AS
SELECT domain_id, address FROM address_src TABLESAMPLE SYSTEM (1) REPEATABLE (42) LIMIT 2000;
EXPLAIN (ANALYZE, BUFFERS, TIMING OFF, SUMMARY ON)
SELECT s.address, p.prefix
FROM sample s
CROSS JOIN LATERAL (
    SELECT prefix FROM prefix
    WHERE domain_id = s.domain_id AND prefix >>= s.address AND valid_period @> now()
    ORDER BY masklen(prefix) DESC LIMIT 1
) p;

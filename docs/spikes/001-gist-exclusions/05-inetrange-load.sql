\timing on
\echo '-- load, variant A (no subtype_diff)'
INSERT INTO range_a SELECT id, domain_id, inetrange_a(first, last, '[]') FROM range_src;
\echo '-- load, variant B (approximate subtype_diff)'
INSERT INTO range_b SELECT id, domain_id, inetrange_b(first, last, '[]') FROM range_src;
\timing off
\set ON_ERROR_STOP off
\echo '-- overlapping range in the same domain and period: must fail'
INSERT INTO range_b (id, domain_id, r)
SELECT gen_random_uuid(), domain_id, inetrange_b(lower(r) + 50, upper(r) + 50, '[]') FROM range_b LIMIT 1;
\echo '-- adjacent range in the same domain: must succeed'
INSERT INTO range_b (id, domain_id, r)
SELECT gen_random_uuid(), domain_id, inetrange_b(upper(r) + 1, upper(r) + 10, '[]') FROM range_b LIMIT 1;
\echo '-- same range in another domain: must succeed'
INSERT INTO range_b (id, domain_id, r) SELECT gen_random_uuid(), gen_random_uuid(), r FROM range_b LIMIT 1;
\echo '-- probe: ranges containing sample addresses (variant A then B)'
EXPLAIN (ANALYZE, TIMING OFF, SUMMARY ON)
SELECT count(*) FROM address_src a JOIN range_a r
  ON r.domain_id = a.domain_id AND r.r @> a.address AND r.valid_period @> now()
WHERE a.id IN (SELECT id FROM address_src TABLESAMPLE SYSTEM (1) REPEATABLE (7) LIMIT 2000);
EXPLAIN (ANALYZE, TIMING OFF, SUMMARY ON)
SELECT count(*) FROM address_src a JOIN range_b r
  ON r.domain_id = a.domain_id AND r.r @> a.address AND r.valid_period @> now()
WHERE a.id IN (SELECT id FROM address_src TABLESAMPLE SYSTEM (1) REPEATABLE (7) LIMIT 2000);

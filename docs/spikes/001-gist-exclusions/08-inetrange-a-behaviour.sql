-- Spike 001 (b): behaviour of variant A (range type without subtype_diff).
\set ON_ERROR_STOP off
\echo '-- overlapping range in the same domain and period: must fail'
INSERT INTO range_a (id, domain_id, r)
SELECT gen_random_uuid(), domain_id, inetrange_a(lower(r) + 50, upper(r) + 50, '[]') FROM range_a LIMIT 1;
\echo '-- contained range in the same domain and period: must fail'
INSERT INTO range_a (id, domain_id, r)
SELECT gen_random_uuid(), domain_id, inetrange_a(lower(r) + 1, lower(r) + 2, '[]') FROM range_a LIMIT 1;
\echo '-- adjacent range in the same domain: must succeed'
INSERT INTO range_a (id, domain_id, r)
SELECT gen_random_uuid(), domain_id, inetrange_a(upper(r) + 1, upper(r) + 10, '[]') FROM range_a LIMIT 1;
\echo '-- same range, same domain, disjoint period: must succeed'
INSERT INTO range_a (id, domain_id, r, valid_period)
SELECT gen_random_uuid(), domain_id, r, tstzrange('2020-01-01', '2021-01-01') FROM range_a LIMIT 1;
\echo '-- IPv6 overlapping range: must fail'
INSERT INTO range_a (id, domain_id, r)
SELECT gen_random_uuid(), domain_id, inetrange_a(lower(r) + 10, upper(r) - 10, '[]') FROM range_a WHERE family(lower(r)) = 6 LIMIT 1;
\echo '-- mixed families in one range: must fail (bounds out of order or rejected)'
SELECT inetrange_a('10.0.0.1', '::1', '[]');

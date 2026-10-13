-- Spike 001 (a): behaviour of the prefix exclusion. Each statement runs on its own.
\set ON_ERROR_STOP off
\echo '-- same domain, same prefix, overlapping period: must fail'
INSERT INTO prefix (id, domain_id, prefix, nature, valid_period)
SELECT gen_random_uuid(), domain_id, prefix, nature, tstzrange('2025-01-01', NULL) FROM prefix WHERE prefix = '10.0.0.0/8' LIMIT 1;
\echo '-- other domain, same prefix: must succeed'
INSERT INTO prefix (id, domain_id, prefix, nature) VALUES (gen_random_uuid(), gen_random_uuid(), '10.0.0.0/8', 'container');
\echo '-- same domain, same prefix, disjoint period: must succeed'
UPDATE prefix SET valid_period = tstzrange('2024-01-01', '2024-06-01') WHERE id = (SELECT id FROM prefix WHERE prefix = '20.2.0.0/10' LIMIT 1);
INSERT INTO prefix (id, domain_id, prefix, nature, valid_period)
SELECT gen_random_uuid(), domain_id, prefix, nature, tstzrange('2024-06-01', NULL) FROM prefix WHERE prefix = '20.2.0.0/10' LIMIT 1;
\echo '-- same domain, nested prefix (different value): must succeed'
INSERT INTO prefix (id, domain_id, prefix, nature)
SELECT gen_random_uuid(), domain_id, '10.255.255.0/25', 'network' FROM prefix WHERE prefix = '10.0.0.0/8' LIMIT 1;

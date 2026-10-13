-- Spike 003, variant O: materialise only the origin (object carrying the applicable
-- assignment); the value is read through the assignment. A value change on an origin
-- rewrites nothing; only origin changes are written.
DROP TABLE IF EXISTS resolved_origin;
CREATE TABLE resolved_origin (
    object_id uuid NOT NULL, property_id int NOT NULL, origin_id uuid NOT NULL,
    PRIMARY KEY (object_id, property_id)
);
INSERT INTO resolved_origin SELECT object_id, property_id, origin_id FROM property_resolved WHERE origin_id IS NOT NULL;
CREATE INDEX resolved_origin_by_origin ON resolved_origin (property_id, origin_id);
ANALYZE resolved_origin;
SELECT pg_size_pretty(pg_total_relation_size('resolved_origin')) AS size_origin_only,
       pg_size_pretty(pg_total_relation_size('property_resolved')) AS size_with_values;
\timing on
\echo '== Case 1 (O): insert a /10 container carrying prop 1 under one 10.0.0.0/8 root'
BEGIN;
CREATE TEMP TABLE chg ON COMMIT DROP AS
SELECT gen_random_uuid() AS id, domain_id, inet '10.64.0.0/10' AS value
FROM obj WHERE value = inet '10.0.0.0/8' LIMIT 1;
INSERT INTO obj SELECT id, domain_id, value, true FROM chg;
INSERT INTO property_assignment
SELECT c.id, 1, (SELECT id FROM enum_value WHERE property_id = 1 AND key = 'dev') FROM chg c;
\echo '-- only prop 1 can change (the only property assigned on the new prefix); targets are'
\echo '-- objects inside it whose origin is an ancestor of it (shallower than the new prefix)'
CREATE TEMP TABLE moved ON COMMIT DROP AS
SELECT r.object_id, r.origin_id AS old_origin, c.id AS new_origin
FROM chg c
JOIN obj o ON o.domain_id = c.domain_id AND o.value <<= c.value
JOIN resolved_origin r ON r.object_id = o.id AND r.property_id = 1
JOIN obj org ON org.id = r.origin_id
WHERE org.value >> c.value;
\echo '-- differential by value'
SELECT ov.key AS before, nv.key AS after, count(*)
FROM moved m
JOIN property_assignment oa ON oa.object_id = m.old_origin AND oa.property_id = 1
JOIN property_assignment na ON na.object_id = m.new_origin AND na.property_id = 1
JOIN enum_value ov ON ov.id = oa.v JOIN enum_value nv ON nv.id = na.v
WHERE oa.v IS DISTINCT FROM na.v GROUP BY 1, 2;
\echo '-- write origin changes'
UPDATE resolved_origin r SET origin_id = m.new_origin FROM moved m
WHERE r.object_id = m.object_id AND r.property_id = 1;
ROLLBACK;
\echo '== Case 2 (O): change prop 1 on the root of every domain: differential, no rewrite'
BEGIN;
CREATE TEMP TABLE roots ON COMMIT DROP AS
SELECT o.id FROM obj o WHERE o.is_prefix
  AND NOT EXISTS (SELECT 1 FROM obj p WHERE p.is_prefix AND p.domain_id = o.domain_id AND p.value >> o.value);
UPDATE property_assignment a SET v = (SELECT id FROM enum_value WHERE property_id = 1 AND key = 'global')
FROM roots r WHERE a.object_id = r.id AND a.property_id = 1;
SELECT count(*) AS objects_reclassified FROM resolved_origin r WHERE r.property_id = 1 AND r.origin_id IN (SELECT id FROM roots);
ROLLBACK;
\echo '== Filter (O): page of 100 addresses of one domain with prop 1 = prod'
EXPLAIN (ANALYZE, TIMING OFF, SUMMARY ON)
SELECT o.id, o.value FROM enum_value e
JOIN property_assignment a ON a.property_id = 1 AND a.v = e.id
JOIN resolved_origin r ON r.property_id = 1 AND r.origin_id = a.object_id
JOIN obj o ON o.id = r.object_id
WHERE e.property_id = 1 AND e.key = 'prod' AND NOT o.is_prefix
  AND o.domain_id = (SELECT domain_id FROM obj WHERE value = inet '10.0.0.0/8' LIMIT 1)
LIMIT 100;
\echo '== Explained resolution (O) of 2000 addresses: value, origin, chain length'
EXPLAIN (ANALYZE, TIMING OFF, SUMMARY ON)
SELECT o.id, a.v, r.origin_id,
       (SELECT count(*) FROM obj anc WHERE anc.is_prefix AND anc.domain_id = o.domain_id
          AND anc.value >>= o.value AND anc.value <<= org.value) AS chain
FROM (SELECT * FROM obj WHERE NOT is_prefix LIMIT 2000) o
JOIN resolved_origin r ON r.object_id = o.id AND r.property_id = 1
JOIN obj org ON org.id = r.origin_id
JOIN property_assignment a ON a.object_id = r.origin_id AND a.property_id = 1;
\timing off

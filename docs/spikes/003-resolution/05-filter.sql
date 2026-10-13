-- Spike 003, variant O: filtered page through the origins of the domain (an object and its
-- origin are always in the same domain), then the objects resolved to those origins.
\timing on
\echo '== Filter (O2): 100 addresses of one domain with prop 1 = prod'
EXPLAIN (ANALYZE, TIMING OFF, SUMMARY ON)
WITH dom AS (SELECT domain_id FROM obj WHERE value = inet '10.0.0.0/8' LIMIT 1),
origins AS (
    SELECT a.object_id FROM property_assignment a
    JOIN enum_value e ON e.id = a.v AND e.key = 'prod'
    JOIN obj org ON org.id = a.object_id AND org.domain_id = (SELECT domain_id FROM dom)
    WHERE a.property_id = 1
)
SELECT o.id, o.value FROM origins g
JOIN resolved_origin r ON r.property_id = 1 AND r.origin_id = g.object_id
JOIN obj o ON o.id = r.object_id AND NOT o.is_prefix
LIMIT 100;
\echo '== Filter (O2), rare value: 100 addresses of one domain with prop 1 = rec'
EXPLAIN (ANALYZE, TIMING OFF, SUMMARY ON)
WITH dom AS (SELECT domain_id FROM obj WHERE value = inet '10.0.0.0/8' LIMIT 1),
origins AS (
    SELECT a.object_id FROM property_assignment a
    JOIN enum_value e ON e.id = a.v AND e.key = 'rec'
    JOIN obj org ON org.id = a.object_id AND org.domain_id = (SELECT domain_id FROM dom)
    WHERE a.property_id = 1
)
SELECT o.id, o.value FROM origins g
JOIN resolved_origin r ON r.property_id = 1 AND r.origin_id = g.object_id
JOIN obj o ON o.id = r.object_id AND NOT o.is_prefix
LIMIT 100;
\echo '== Case 2 (O): count of objects reclassified by a value change on every root'
SELECT count(*) AS objects_reclassified FROM resolved_origin r
WHERE r.property_id = 1 AND r.origin_id IN (
    SELECT o.id FROM obj o WHERE o.is_prefix
      AND NOT EXISTS (SELECT 1 FROM obj p WHERE p.is_prefix AND p.domain_id = o.domain_id AND p.value >> o.value));
\timing off

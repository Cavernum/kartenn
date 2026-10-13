-- Spike 003: initial full materialisation (set-based, nearest ancestor-or-self carrying
-- an assignment), then non-heritable properties copied as is.
\timing on
\echo '-- heritable properties 1..3'
INSERT INTO property_resolved (object_id, property_id, v, origin_id)
SELECT o.id, p.p, x.v, x.origin_id
FROM obj o
CROSS JOIN generate_series(1, 3) AS p(p)
CROSS JOIN LATERAL (
    SELECT a.v, a.object_id AS origin_id
    FROM obj anc
    JOIN property_assignment a ON a.object_id = anc.id AND a.property_id = p.p
    WHERE anc.is_prefix AND anc.domain_id = o.domain_id AND anc.value >>= o.value
    ORDER BY masklen(anc.value) DESC
    LIMIT 1
) x;
\echo '-- non-heritable properties 4..10'
INSERT INTO property_resolved (object_id, property_id, v, origin_id)
SELECT object_id, property_id, v, object_id FROM property_assignment WHERE property_id >= 4;
\timing off
ANALYZE property_resolved;
SELECT count(*) AS resolved_rows, pg_size_pretty(pg_total_relation_size('property_resolved')) AS size
FROM property_resolved;

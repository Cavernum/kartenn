-- Spike 003: materialised resolution of heritable properties (ADR 0016).
-- Objects: the 192 219 prefixes and 996 717 addresses of the reference dataset.
-- 10 properties, 3 heritable along the addressing tree; values are closed-list ids.
DROP TABLE IF EXISTS obj, property_assignment, property_resolved, enum_value;
CREATE TABLE obj AS
    SELECT id, domain_id, prefix::inet AS value, true AS is_prefix FROM prefix
    UNION ALL
    SELECT id, domain_id, address, false FROM address_src;
ALTER TABLE obj ADD PRIMARY KEY (id);
CREATE INDEX obj_prefix_tree ON obj USING gist (domain_id, value inet_ops) WHERE is_prefix;
CREATE INDEX obj_value ON obj USING gist (domain_id, value inet_ops);

CREATE TABLE enum_value (id uuid PRIMARY KEY, property_id int NOT NULL, key text NOT NULL);
INSERT INTO enum_value
SELECT gen_random_uuid(), p, k FROM generate_series(1, 10) p, unnest(ARRAY['prod','dev','rec','global']) k;

CREATE TABLE property_assignment (
    object_id uuid NOT NULL, property_id int NOT NULL, v uuid,
    PRIMARY KEY (object_id, property_id)
);
-- Heritable properties 1..3: assigned on every domain root container and on ~3 % of
-- the other prefixes; non-heritable 4..10: assigned on ~20 % of all objects.
INSERT INTO property_assignment
SELECT o.id, p, (SELECT id FROM enum_value e WHERE e.property_id = p
                 ORDER BY hashtext(o.id::text || p) LIMIT 1)
FROM obj o, generate_series(1, 3) p
WHERE o.is_prefix AND (masklen(o.value) IN (8, 10, 12, 48) OR abs(hashtext(o.id::text || p)) % 100 < 3);
INSERT INTO property_assignment
SELECT o.id, p, (SELECT id FROM enum_value e WHERE e.property_id = p ORDER BY hashtext(o.id::text || p) LIMIT 1)
FROM obj o, generate_series(4, 10) p
WHERE abs(hashtext(o.id::text || p)) % 100 < 20;
ANALYZE obj; ANALYZE property_assignment; ANALYZE enum_value;

CREATE TABLE property_resolved (
    object_id uuid NOT NULL, property_id int NOT NULL, v uuid, origin_id uuid,
    PRIMARY KEY (object_id, property_id)
);
CREATE INDEX property_resolved_filter ON property_resolved (property_id, v);
SELECT (SELECT count(*) FROM obj) AS objects, (SELECT count(*) FROM property_assignment) AS assignments;

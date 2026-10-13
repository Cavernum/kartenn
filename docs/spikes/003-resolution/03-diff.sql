-- Spike 003: differential of a changeset (REQ-003-19, 26), inside a transaction rolled back.
\timing on
\echo '== Case 1: insert a /10 container carrying prop 1 under the 10.0.0.0/8 root of one domain'
BEGIN;
CREATE TEMP TABLE chg ON COMMIT DROP AS
SELECT gen_random_uuid() AS id, domain_id, inet '10.64.0.0/10' AS value
FROM obj WHERE value = inet '10.0.0.0/8' LIMIT 1;
INSERT INTO obj SELECT id, domain_id, value, true FROM chg;
INSERT INTO property_assignment
SELECT c.id, 1, (SELECT id FROM enum_value WHERE property_id = 1 AND key = 'dev') FROM chg c;
\echo '-- step 1: affected objects (structural scope = objects contained in the new prefix)'
CREATE TEMP TABLE affected ON COMMIT DROP AS
SELECT o.id, o.domain_id, o.value FROM obj o, chg c
WHERE o.domain_id = c.domain_id AND o.value <<= c.value;
SELECT count(*) AS affected FROM affected;
\echo '-- step 2: new resolution of the heritable properties for the affected objects'
CREATE TEMP TABLE fresh ON COMMIT DROP AS
SELECT o.id AS object_id, p.p AS property_id, x.v, x.origin_id
FROM affected o CROSS JOIN generate_series(1, 3) AS p(p)
CROSS JOIN LATERAL (
    SELECT a.v, a.object_id AS origin_id FROM obj anc
    JOIN property_assignment a ON a.object_id = anc.id AND a.property_id = p.p
    WHERE anc.is_prefix AND anc.domain_id = o.domain_id AND anc.value >>= o.value
    ORDER BY masklen(anc.value) DESC LIMIT 1) x;
\echo '-- step 3: differential (value changes on existing objects), by property and values'
SELECT f.property_id, old.key AS before, new.key AS after, count(*)
FROM fresh f JOIN property_resolved r USING (object_id, property_id)
LEFT JOIN enum_value old ON old.id = r.v LEFT JOIN enum_value new ON new.id = f.v
WHERE r.v IS DISTINCT FROM f.v
GROUP BY 1, 2, 3 ORDER BY 1, 4 DESC;
\echo '-- step 4: write changed rows (value or origin) and the new object rows'
UPDATE property_resolved r SET v = f.v, origin_id = f.origin_id
FROM fresh f WHERE r.object_id = f.object_id AND r.property_id = f.property_id
  AND (r.v IS DISTINCT FROM f.v OR r.origin_id IS DISTINCT FROM f.origin_id);
INSERT INTO property_resolved SELECT f.* FROM fresh f
WHERE NOT EXISTS (SELECT 1 FROM property_resolved r WHERE r.object_id = f.object_id AND r.property_id = f.property_id);
ROLLBACK;

\echo '== Case 2: change prop 1 on the root of every domain (about 10^6 objects)'
BEGIN;
UPDATE property_assignment a SET v = (SELECT id FROM enum_value WHERE property_id = 1 AND key = 'global')
FROM obj o WHERE a.object_id = o.id AND a.property_id = 1 AND o.is_prefix AND masklen(o.value) IN (8, 10, 12, 48)
  AND NOT EXISTS (SELECT 1 FROM obj p WHERE p.is_prefix AND p.domain_id = o.domain_id AND p.value >> o.value);
\echo '-- step 1+2: affected = objects whose current origin is one of the changed roots (value-only change)'
CREATE TEMP TABLE fresh2 ON COMMIT DROP AS
SELECT r.object_id, r.property_id, a.v, r.origin_id
FROM property_resolved r JOIN property_assignment a ON a.object_id = r.origin_id AND a.property_id = r.property_id
WHERE r.property_id = 1 AND r.v IS DISTINCT FROM a.v;
SELECT count(*) AS changed_objects FROM fresh2;
\echo '-- step 4: write'
UPDATE property_resolved r SET v = f.v FROM fresh2 f
WHERE r.object_id = f.object_id AND r.property_id = f.property_id;
ROLLBACK;
\timing off

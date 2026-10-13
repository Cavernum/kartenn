-- Spike 004: current reads versus reads "as of" a past system time (budget: at most ×2).
CREATE TEMP TABLE ids AS SELECT id FROM address_cur TABLESAMPLE SYSTEM (1) REPEATABLE (11) LIMIT 2000;
ANALYZE ids;
\echo '-- (1) current, by id, via the WITHOUT OVERLAPS primary key (GiST)'
EXPLAIN (ANALYZE, TIMING OFF, SUMMARY ON)
SELECT count(*) FROM ids i CROSS JOIN LATERAL (
  SELECT * FROM address_cur a WHERE a.id = i.id AND a.valid_period @> now()) x;
CREATE INDEX address_cur_id_btree ON address_cur (id);
ANALYZE address_cur;
\echo '-- (2) current, by id, with an extra B-tree on id'
EXPLAIN (ANALYZE, TIMING OFF, SUMMARY ON)
SELECT count(*) FROM ids i CROSS JOIN LATERAL (
  SELECT * FROM address_cur a WHERE a.id = i.id AND a.valid_period @> now()) x;
\echo '-- (3) as_of 2025-03-15, by id: current UNION ALL history'
EXPLAIN (ANALYZE, TIMING OFF, SUMMARY ON)
SELECT count(*) FROM ids i CROSS JOIN LATERAL (
  SELECT id, usage FROM address_cur a
   WHERE a.id = i.id AND a.sys_start <= timestamptz '2025-03-15' AND a.valid_period @> now()
  UNION ALL
  SELECT id, usage FROM address_cur_history h
   WHERE h.id = i.id AND h.sys_start <= timestamptz '2025-03-15' AND h.sys_end > timestamptz '2025-03-15'
     AND h.valid_period @> now()) x;
\echo '-- (4) current page: addresses of one /24 network'
EXPLAIN (ANALYZE, TIMING OFF, SUMMARY ON)
SELECT * FROM address_cur WHERE domain_id = (SELECT domain_id FROM address_cur LIMIT 1)
  AND address << '10.0.0.0/8' AND valid_period @> now() ORDER BY address LIMIT 100;

-- Spike 004: 10^6 current addresses and ~2×10^6 historical versions spread over 3 years,
-- loaded directly (the trigger cannot backdate now()); triggers attached afterwards.
\timing on
\echo '-- current rows'
INSERT INTO address_cur (id, domain_id, address, valid_period, sys_start, changeset_id)
SELECT id, domain_id, address, tstzrange('2023-10-01', NULL),
       timestamptz '2026-09-01' + (random() * interval '40 days'), gen_random_uuid()
FROM address_src;
\echo '-- historical rows: 0 to 4 versions per address, contiguous system periods'
INSERT INTO address_cur_history (id, domain_id, address, usage, valid_period, sys_start, changeset_id, sys_end)
SELECT a.id, a.domain_id, a.address, 'reserved', a.valid_period,
       timestamptz '2023-10-01' + (g.k - 1) * (a.sys_start - timestamptz '2023-10-01') / g.n,
       gen_random_uuid(),
       timestamptz '2023-10-01' + g.k * (a.sys_start - timestamptz '2023-10-01') / g.n
FROM address_cur a
CROSS JOIN LATERAL (SELECT (abs(hashtext(a.id::text)) % 5) AS n) AS c
CROSS JOIN LATERAL (SELECT k, c.n AS n FROM generate_series(1, c.n) AS k) AS g;
\timing off
ANALYZE address_cur; ANALYZE address_cur_history;
SELECT (SELECT count(*) FROM address_cur) AS current_rows,
       (SELECT count(*) FROM address_cur_history) AS history_rows,
       pg_size_pretty(pg_total_relation_size('address_cur')) AS current_size,
       (SELECT pg_size_pretty(sum(pg_total_relation_size(inhrelid))) FROM pg_inherits
        WHERE inhparent = 'address_cur_history'::regclass) AS history_size;

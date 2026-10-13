#!/usr/bin/env bash
# Spike 001 (d): two transactions insert the same prefix in the same domain concurrently.
# Expected: the second blocks on the first, then fails once the first commits.
set -u
U=${1:?database url}
D=$(psql -Atq "$U" -c "SELECT gen_random_uuid()")
psql -q "$U" -c "SELECT 1" >/dev/null
(psql -q "$U" -v ON_ERROR_STOP=1 <<SQL
BEGIN;
INSERT INTO prefix (id, domain_id, prefix, nature) VALUES (gen_random_uuid(), '$D', '192.168.0.0/16', 'container');
SELECT 'T1 inserted at ' || clock_timestamp();
SELECT pg_sleep(3);
COMMIT;
SELECT 'T1 committed at ' || clock_timestamp();
SQL
) 2>&1 | grep -E 'T1|ERROR' &
sleep 1
(psql -q "$U" <<SQL
SELECT 'T2 starts at ' || clock_timestamp();
INSERT INTO prefix (id, domain_id, prefix, nature) VALUES (gen_random_uuid(), '$D', '192.168.0.0/16', 'container');
SELECT 'T2 finished at ' || clock_timestamp();
SQL
) 2>&1 | grep -E 'T2|ERROR'
wait

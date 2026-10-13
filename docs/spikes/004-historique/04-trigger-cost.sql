-- Spike 004: cost of the versioning trigger on a 10^5-row UPDATE, and INV-12.
CREATE TEMP TABLE upd AS SELECT id FROM address_cur TABLESAMPLE SYSTEM (12) REPEATABLE (5) LIMIT 100000;
CREATE INDEX ON upd (id); ANALYZE upd;
\timing on
\echo '-- (a) no trigger'
BEGIN; UPDATE address_cur a SET usage = 'gateway' FROM upd u WHERE a.id = u.id; ROLLBACK;
\echo '-- (b) generic trigger (dynamic SQL)'
CREATE TRIGGER versioning BEFORE INSERT OR UPDATE OR DELETE ON address_cur
    FOR EACH ROW EXECUTE FUNCTION kartenn_versioning_dynamic();
BEGIN; UPDATE address_cur a SET usage = 'gateway' FROM upd u WHERE a.id = u.id; ROLLBACK;
DROP TRIGGER versioning ON address_cur;
\echo '-- (c) per-table trigger (static SQL)'
CREATE TRIGGER versioning BEFORE INSERT OR UPDATE OR DELETE ON address_cur
    FOR EACH ROW EXECUTE FUNCTION address_cur_versioning();
BEGIN; UPDATE address_cur a SET usage = 'gateway' FROM upd u WHERE a.id = u.id; ROLLBACK;
\timing off
\echo '-- (d) INV-12: one transaction, one system time; history row closed at the same instant'
BEGIN;
UPDATE address_cur SET usage = 'vip' WHERE id IN (SELECT id FROM upd LIMIT 3);
SELECT count(DISTINCT sys_start) AS distinct_sys_start, bool_and(sys_start = now()) AS equals_tx_time
FROM address_cur WHERE id IN (SELECT id FROM upd LIMIT 3);
SELECT count(*) AS history_rows_closed_now FROM address_cur_history WHERE sys_end = now();
ROLLBACK;

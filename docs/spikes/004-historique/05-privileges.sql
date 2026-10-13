-- Spike 004: the application role writes current rows; only the trigger writes history.
DROP TRIGGER IF EXISTS versioning ON address_cur;
ALTER FUNCTION kartenn_versioning_dynamic() SECURITY DEFINER;
CREATE TRIGGER versioning BEFORE INSERT OR UPDATE OR DELETE ON address_cur
    FOR EACH ROW EXECUTE FUNCTION kartenn_versioning_dynamic();
GRANT USAGE ON SCHEMA public TO kartenn_app;
GRANT SELECT, INSERT, UPDATE, DELETE ON address_cur TO kartenn_app;
GRANT SELECT ON address_cur_history TO kartenn_app;
\set ON_ERROR_STOP off
SET ROLE kartenn_app;
\echo '-- update through the application role: must succeed and historise'
BEGIN;
UPDATE address_cur SET usage = 'vip' WHERE id = (SELECT id FROM address_cur LIMIT 1);
SELECT count(*) AS historised FROM address_cur_history WHERE sys_end = now();
COMMIT;
\echo '-- direct insert into history: must fail'
INSERT INTO address_cur_history SELECT *, now() FROM address_cur LIMIT 1;
\echo '-- update of history: must fail'
UPDATE address_cur_history SET usage = 'forged' WHERE id = (SELECT id FROM address_cur_history LIMIT 1);
\echo '-- delete of history: must fail'
DELETE FROM address_cur_history WHERE id = (SELECT id FROM address_cur_history LIMIT 1);
RESET ROLE;

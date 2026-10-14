-- History copies columns by name (ADR 0017, amendment). The first version copied
-- `($1).*` by position: after an ALTER TABLE ... ADD COLUMN the history twin has sys_end
-- before the new column and every update or delete of the table failed.
--
-- The trigger receives the column list of the current table as argument, fixed when the
-- trigger is (re)created: no catalog lookup per row. `($1).*` expands in the same order.

CREATE OR REPLACE FUNCTION public.kartenn_versioning() RETURNS trigger
LANGUAGE plpgsql SECURITY DEFINER SET search_path = pg_catalog, public AS $$
BEGIN
    IF TG_OP = 'INSERT' THEN
        NEW.sys_start := now();
        RETURN NEW;
    END IF;
    EXECUTE format('INSERT INTO %I.%I (%s, sys_end) SELECT ($1).*, now()',
                   TG_TABLE_SCHEMA, TG_TABLE_NAME || '_history', TG_ARGV[0])
        USING OLD;
    IF TG_OP = 'UPDATE' THEN
        NEW.sys_start := now();
        RETURN NEW;
    END IF;
    RETURN OLD;
END $$;

-- (Re)creates the versioning trigger of a table with its current column list.
CREATE PROCEDURE public.kartenn_versioning_trigger(target regclass)
LANGUAGE plpgsql SET search_path = pg_catalog, public AS $$
DECLARE
    nsp  text := (SELECT n.nspname FROM pg_class c JOIN pg_namespace n ON n.oid = c.relnamespace WHERE c.oid = target);
    tbl  text := (SELECT relname FROM pg_class WHERE oid = target);
    cols text := (SELECT string_agg(quote_ident(attname), ', ' ORDER BY attnum)
                  FROM pg_attribute WHERE attrelid = target AND attnum > 0 AND NOT attisdropped);
BEGIN
    EXECUTE format('DROP TRIGGER IF EXISTS versioning ON %I.%I', nsp, tbl);
    EXECUTE format('CREATE TRIGGER versioning BEFORE INSERT OR UPDATE OR DELETE ON %I.%I
                    FOR EACH ROW EXECUTE FUNCTION public.kartenn_versioning(%L)', nsp, tbl, cols);
END $$;
REVOKE ALL ON PROCEDURE public.kartenn_versioning_trigger(regclass) FROM PUBLIC;

CREATE OR REPLACE PROCEDURE public.kartenn_make_versioned(target regclass)
LANGUAGE plpgsql SET search_path = pg_catalog, public AS $$
DECLARE
    nsp  text := (SELECT n.nspname FROM pg_class c JOIN pg_namespace n ON n.oid = c.relnamespace WHERE c.oid = target);
    tbl  text := (SELECT relname FROM pg_class WHERE oid = target);
    hist text := tbl || '_history';
    missing text;
BEGIN
    SELECT string_agg(c, ', ') INTO missing
    FROM unnest(ARRAY['id', 'valid_period', 'sys_start', 'changeset_id']) AS c
    WHERE NOT EXISTS (SELECT FROM pg_attribute WHERE attrelid = target AND attname = c AND NOT attisdropped);
    IF missing IS NOT NULL THEN
        RAISE 'table % lacks the versioning columns %', tbl, missing;
    END IF;
    EXECUTE format('CREATE TABLE %I.%I (LIKE %I.%I, sys_end timestamptz NOT NULL) PARTITION BY RANGE (sys_end)',
                   nsp, hist, nsp, tbl);
    EXECUTE format('CREATE TABLE %I.%I PARTITION OF %I.%I DEFAULT', nsp, hist || '_default', nsp, hist);
    EXECUTE format('CREATE INDEX ON %I.%I (id, sys_start)', nsp, hist);
    CALL public.kartenn_versioning_trigger(target);
    EXECUTE format('REVOKE ALL ON %I.%I FROM PUBLIC', nsp, hist);
    EXECUTE format('GRANT SELECT ON %I.%I TO kartenn_app', nsp, hist);
    EXECUTE format('GRANT SELECT, INSERT, UPDATE, DELETE ON %I.%I TO kartenn_app', nsp, tbl);
END $$;

-- Adds a column to a versioned table and to its history, then refreshes the trigger.
-- `column_ddl` is a column definition (`name type [constraints]`), written by migrations
-- only; a NOT NULL column needs a default for the existing history rows.
CREATE PROCEDURE public.kartenn_add_versioned_column(target regclass, column_ddl text)
LANGUAGE plpgsql SET search_path = pg_catalog, public AS $$
DECLARE
    nsp  text := (SELECT n.nspname FROM pg_class c JOIN pg_namespace n ON n.oid = c.relnamespace WHERE c.oid = target);
    tbl  text := (SELECT relname FROM pg_class WHERE oid = target);
BEGIN
    EXECUTE format('ALTER TABLE %I.%I ADD COLUMN %s', nsp, tbl, column_ddl);
    EXECUTE format('ALTER TABLE %I.%I ADD COLUMN %s', nsp, tbl || '_history', column_ddl);
    CALL public.kartenn_versioning_trigger(target);
END $$;
REVOKE ALL ON PROCEDURE public.kartenn_add_versioned_column(regclass, text) FROM PUBLIC;

-- Existing versioned tables: same trigger, now with their column list.
DO $$
DECLARE
    t regclass;
BEGIN
    FOR t IN SELECT tgrelid::regclass FROM pg_trigger
             WHERE tgname = 'versioning' AND tgfoid = 'public.kartenn_versioning'::regproc
    LOOP
        CALL public.kartenn_versioning_trigger(t);
    END LOOP;
END $$;

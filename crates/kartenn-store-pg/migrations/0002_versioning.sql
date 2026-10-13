-- System time (ADR 0017, spike 004): every versioned table has a history twin filled by a
-- generic trigger. Rules from the spikes: functions used by triggers, types or indexes
-- fix their search_path and contain no EXCEPTION block.

-- The application role exists in every environment (created by the deployment, devenv in
-- development). It writes current rows only; history is written by the trigger.
DO $$
BEGIN
    IF NOT EXISTS (SELECT FROM pg_catalog.pg_roles WHERE rolname = 'kartenn_app') THEN
        RAISE 'role kartenn_app must exist before the migrations run';
    END IF;
END $$;

-- BEFORE INSERT: system start is the transaction timestamp (INV-12).
-- BEFORE UPDATE / DELETE: the replaced row goes to <table>_history, closed now.
CREATE FUNCTION public.kartenn_versioning() RETURNS trigger
LANGUAGE plpgsql SECURITY DEFINER SET search_path = pg_catalog, public AS $$
BEGIN
    IF TG_OP = 'INSERT' THEN
        NEW.sys_start := now();
        RETURN NEW;
    END IF;
    EXECUTE format('INSERT INTO %I.%I SELECT ($1).*, now()', TG_TABLE_SCHEMA, TG_TABLE_NAME || '_history')
        USING OLD;
    IF TG_OP = 'UPDATE' THEN
        NEW.sys_start := now();
        RETURN NEW;
    END IF;
    RETURN OLD;
END $$;
REVOKE ALL ON FUNCTION public.kartenn_versioning() FROM PUBLIC;

-- Makes a current table versioned: history twin partitioned by month of sys_end, with a
-- default partition, an (id, sys_start) index, the trigger and the grants.
-- The table must have the columns id, valid_period, sys_start and changeset_id.
CREATE PROCEDURE public.kartenn_make_versioned(target regclass)
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
    EXECUTE format('CREATE TRIGGER versioning BEFORE INSERT OR UPDATE OR DELETE ON %I.%I
                    FOR EACH ROW EXECUTE FUNCTION public.kartenn_versioning()', nsp, tbl);
    EXECUTE format('REVOKE ALL ON %I.%I FROM PUBLIC', nsp, hist);
    EXECUTE format('GRANT SELECT ON %I.%I TO kartenn_app', nsp, hist);
    EXECUTE format('GRANT SELECT, INSERT, UPDATE, DELETE ON %I.%I TO kartenn_app', nsp, tbl);
END $$;

-- Creates the monthly history partitions of [from_month, from_month + months) that do not
-- exist yet. Run by a maintenance job ahead of time, so that the default partition stays
-- empty.
CREATE PROCEDURE public.kartenn_history_partitions(target regclass, from_month date, months integer)
LANGUAGE plpgsql SET search_path = pg_catalog, public AS $$
DECLARE
    nsp  text := (SELECT n.nspname FROM pg_class c JOIN pg_namespace n ON n.oid = c.relnamespace WHERE c.oid = target);
    hist text := (SELECT relname FROM pg_class WHERE oid = target) || '_history';
    m date := date_trunc('month', from_month)::date;
BEGIN
    FOR i IN 0 .. months - 1 LOOP
        IF to_regclass(format('%I.%I', nsp, hist || '_' || to_char(m, 'YYYYMM'))) IS NULL THEN
            EXECUTE format('CREATE TABLE %I.%I PARTITION OF %I.%I FOR VALUES FROM (%L) TO (%L)',
                           nsp, hist || '_' || to_char(m, 'YYYYMM'), nsp, hist, m, (m + interval '1 month')::date);
        END IF;
        m := (m + interval '1 month')::date;
    END LOOP;
END $$;

GRANT USAGE ON SCHEMA public TO kartenn_app;

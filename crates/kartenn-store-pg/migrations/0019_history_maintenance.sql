-- Maintenance of the history tables (spec 004, REQ-004-17, 38; task 004-25; ADR 0017):
-- a GiST index on the system period of every history, and monthly partitions created
-- ahead of time by a job, so that the default partition stays empty.

-- Index of the system period, for the readings "as asserted at" (as_of).
CREATE PROCEDURE public.kartenn_history_index(target regclass)
LANGUAGE plpgsql SET search_path = pg_catalog, public AS $$
DECLARE
    nsp  text := (SELECT n.nspname FROM pg_class c JOIN pg_namespace n ON n.oid = c.relnamespace WHERE c.oid = target);
    hist text := (SELECT relname FROM pg_class WHERE oid = target) || '_history';
BEGIN
    EXECUTE format('CREATE INDEX IF NOT EXISTS %I ON %I.%I USING gist (tstzrange(sys_start, sys_end))',
                   hist || '_system_period', nsp, hist);
END $$;
REVOKE ALL ON PROCEDURE public.kartenn_history_index(regclass) FROM PUBLIC;

-- Versioned tables from now on get the index with their history.
CREATE OR REPLACE PROCEDURE public.kartenn_make_versioned(target regclass, key_columns text[])
LANGUAGE plpgsql SET search_path = pg_catalog, public AS $$
DECLARE
    nsp  text := (SELECT n.nspname FROM pg_class c JOIN pg_namespace n ON n.oid = c.relnamespace WHERE c.oid = target);
    tbl  text := (SELECT relname FROM pg_class WHERE oid = target);
    hist text := tbl || '_history';
    missing text;
BEGIN
    SELECT string_agg(c, ', ') INTO missing
    FROM unnest(key_columns || ARRAY['valid_period', 'sys_start', 'changeset_id']) AS c
    WHERE NOT EXISTS (SELECT FROM pg_attribute WHERE attrelid = target AND attname = c AND NOT attisdropped);
    IF missing IS NOT NULL THEN
        RAISE 'table % lacks the versioning columns %', tbl, missing;
    END IF;
    EXECUTE format('CREATE TABLE %I.%I (LIKE %I.%I, sys_end timestamptz NOT NULL) PARTITION BY RANGE (sys_end)',
                   nsp, hist, nsp, tbl);
    EXECUTE format('CREATE TABLE %I.%I PARTITION OF %I.%I DEFAULT', nsp, hist || '_default', nsp, hist);
    EXECUTE format('CREATE INDEX ON %I.%I (%s, sys_start)', nsp, hist,
                   (SELECT string_agg(quote_ident(k), ', ') FROM unnest(key_columns) AS k));
    CALL public.kartenn_history_index(target);
    CALL public.kartenn_versioning_trigger(target);
    EXECUTE format('REVOKE ALL ON %I.%I FROM PUBLIC', nsp, hist);
    EXECUTE format('GRANT SELECT ON %I.%I TO kartenn_app', nsp, hist);
    EXECUTE format('GRANT SELECT, INSERT, UPDATE, DELETE ON %I.%I TO kartenn_app', nsp, tbl);
END $$;

-- A month whose rows already sit in the default partition is left there: attaching its
-- partition would fail. The job runs ahead of time, so this only concerns the months
-- before its first run.
CREATE OR REPLACE PROCEDURE public.kartenn_history_partitions(target regclass, from_month date, months integer)
LANGUAGE plpgsql SET search_path = pg_catalog, public AS $$
DECLARE
    nsp  text := (SELECT n.nspname FROM pg_class c JOIN pg_namespace n ON n.oid = c.relnamespace WHERE c.oid = target);
    hist text := (SELECT relname FROM pg_class WHERE oid = target) || '_history';
    m date := date_trunc('month', from_month)::date;
    occupied boolean;
BEGIN
    FOR i IN 0 .. months - 1 LOOP
        IF to_regclass(format('%I.%I', nsp, hist || '_' || to_char(m, 'YYYYMM'))) IS NULL THEN
            EXECUTE format('SELECT EXISTS (SELECT 1 FROM %I.%I WHERE sys_end >= %L AND sys_end < %L)',
                           nsp, hist || '_default', m, (m + interval '1 month')::date)
                INTO occupied;
            IF NOT occupied THEN
                EXECUTE format('CREATE TABLE %I.%I PARTITION OF %I.%I FOR VALUES FROM (%L) TO (%L)',
                               nsp, hist || '_' || to_char(m, 'YYYYMM'), nsp, hist, m,
                               (m + interval '1 month')::date);
            END IF;
        END IF;
        m := (m + interval '1 month')::date;
    END LOOP;
END $$;

-- The job: partitions of the current month and the `months` - 1 next ones, for every
-- versioned table (those with a history twin). Run with the owner of the schema, by the
-- migration job then periodically.
CREATE PROCEDURE public.kartenn_history_maintenance(months integer)
LANGUAGE plpgsql SET search_path = pg_catalog, public AS $$
DECLARE
    t regclass;
BEGIN
    FOR t IN
        SELECT c.oid::regclass FROM pg_class c JOIN pg_namespace n ON n.oid = c.relnamespace
        WHERE n.nspname = 'public' AND c.relkind = 'r'
          AND EXISTS (SELECT FROM pg_class h WHERE h.relnamespace = c.relnamespace
                                             AND h.relname = c.relname || '_history' AND h.relkind = 'p')
        ORDER BY c.relname
    LOOP
        CALL public.kartenn_history_partitions(t, now()::date, months);
    END LOOP;
END $$;
REVOKE ALL ON PROCEDURE public.kartenn_history_maintenance(integer) FROM PUBLIC;

-- Existing histories get their index.
DO $$
DECLARE
    t regclass;
BEGIN
    FOR t IN
        SELECT c.oid::regclass FROM pg_class c JOIN pg_namespace n ON n.oid = c.relnamespace
        WHERE n.nspname = 'public' AND c.relkind = 'r'
          AND EXISTS (SELECT FROM pg_class h WHERE h.relnamespace = c.relnamespace
                                             AND h.relname = c.relname || '_history' AND h.relkind = 'p')
    LOOP
        CALL public.kartenn_history_index(t);
    END LOOP;
END $$;

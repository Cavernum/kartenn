-- A version asserted after the start of the transaction that replaces it (spec 021, task
-- 22p; ADR 0015, precision of 2026-11-10; ADR 0017). System time is the start of the
-- transaction (INV-12): a changeset that waited for its locks, while another one started
-- later wrote and committed, would end that version before it began, a system period
-- that the GiST index of the history refuses as an inverted range, a raw error. The
-- trigger names the case instead: a serialization failure (SQLSTATE 40001) whose message
-- starts with `kartenn.version.stale` and the object, which the pipeline replays in a
-- new transaction, then refuses in conflict (`changeset.operation.conflict`).

CREATE OR REPLACE FUNCTION public.kartenn_versioning() RETURNS trigger
LANGUAGE plpgsql SECURITY DEFINER SET search_path = pg_catalog, public AS $$
BEGIN
    IF TG_OP = 'INSERT' THEN
        NEW.sys_start := now();
        RETURN NEW;
    END IF;
    IF OLD.sys_start > now() THEN
        RAISE 'kartenn.version.stale % of %.% asserted at %, after the start of this transaction at %',
              coalesce(to_jsonb(OLD) ->> 'id', to_jsonb(OLD) ->> 'object_id'),
              TG_TABLE_SCHEMA, TG_TABLE_NAME, OLD.sys_start, now()
              USING ERRCODE = 'serialization_failure';
    END IF;
    IF OLD.sys_start <> now() THEN
        EXECUTE format('INSERT INTO %I.%I (%s, sys_end) SELECT ($1).*, now()',
                       TG_TABLE_SCHEMA, TG_TABLE_NAME || '_history', TG_ARGV[0])
            USING OLD;
    END IF;
    IF TG_OP = 'UPDATE' THEN
        NEW.sys_start := now();
        RETURN NEW;
    END IF;
    RETURN OLD;
END $$;

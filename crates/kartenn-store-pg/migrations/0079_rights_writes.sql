-- The writes of rights, counted (spec 006, task 41; spec 024, M18, INV-63). A snapshot of
-- the statistics kept under the rights of its caller reads those rights before and after
-- its computation; a right given then withdrawn between the two, the same at both ends,
-- would leave under the key without it a count made with it. The computation reads
-- `kartenn_rights_written()` before and after itself instead: any write of the tables an
-- authorization decision reads, committed or in flight in between, makes the two differ,
-- and the snapshot is not kept. Whoever writes (the pipeline, the account of last resort,
-- a direct write in the database) is counted, by trigger.
--
-- A sequence rather than a counter row: `nextval` does not wait for the transactions
-- that write rights concurrently, nor serialize them. Its value is read outside the
-- snapshot of the reader (a sequence is not versioned): a write whose trigger has run is
-- seen at once, before its commit. `CACHE 1`: every `nextval` moves the stored value.
--
-- What a counter alone does not see is a write in flight at the first read: its trigger
-- already run (the value read includes it) but its commit after, possibly visible to the
-- computation and not to the rights read before it. The trigger therefore holds, until
-- the end of its transaction, a shared advisory lock, which writers of rights share
-- without waiting for one another; the first read tries the exclusive lock without
-- waiting (it never stalls a writer): taken, no write of rights is in flight and every
-- write counted is committed; refused, it answers NULL and the snapshot is not kept.
--
-- The tables: the roles and their permissions, the bindings, the groups and their
-- members (`kartenn_member_of`), the attachments of the issuers (the confinement of a
-- subject) and the declarations of the properties (whether a `prop.*` permission is
-- sensitive, `rbac::visibilities_on`). The trees of entities and sites, and the prefixes
-- a binding names, are objects: their writes move the outbox, which makes a snapshot due
-- again; counting them would leave unkept every computation during an import.

CREATE SEQUENCE public.kartenn_rights_writes AS bigint CACHE 1 NO CYCLE;
REVOKE ALL ON SEQUENCE public.kartenn_rights_writes FROM PUBLIC;

-- The advisory lock key (single-key space): "kartRigh".
CREATE FUNCTION public.kartenn_rights_written_trigger() RETURNS trigger
LANGUAGE plpgsql SECURITY DEFINER SET search_path = pg_catalog, public AS $$
BEGIN
    PERFORM pg_advisory_xact_lock_shared(7737591478723635048);
    PERFORM nextval('public.kartenn_rights_writes');
    RETURN NULL;
END $$;
REVOKE ALL ON FUNCTION public.kartenn_rights_written_trigger() FROM PUBLIC;

-- The count of the writes of rights, NULL while one is in flight (its transaction not
-- ended).
CREATE FUNCTION public.kartenn_rights_written() RETURNS bigint
LANGUAGE plpgsql VOLATILE SECURITY DEFINER SET search_path = pg_catalog, public AS $$
BEGIN
    IF NOT pg_try_advisory_xact_lock(7737591478723635048) THEN
        RETURN NULL;
    END IF;
    RETURN (SELECT CASE WHEN is_called THEN last_value ELSE 0 END
              FROM public.kartenn_rights_writes);
END $$;
REVOKE ALL ON FUNCTION public.kartenn_rights_written() FROM PUBLIC;
GRANT EXECUTE ON FUNCTION public.kartenn_rights_written() TO kartenn_app;

-- Once per statement, rows or none, and for a truncation.
CREATE TRIGGER rights_written AFTER INSERT OR UPDATE OR DELETE OR TRUNCATE ON role
    FOR EACH STATEMENT EXECUTE FUNCTION public.kartenn_rights_written_trigger();
CREATE TRIGGER rights_written AFTER INSERT OR UPDATE OR DELETE OR TRUNCATE ON role_permission
    FOR EACH STATEMENT EXECUTE FUNCTION public.kartenn_rights_written_trigger();
CREATE TRIGGER rights_written AFTER INSERT OR UPDATE OR DELETE OR TRUNCATE ON binding
    FOR EACH STATEMENT EXECUTE FUNCTION public.kartenn_rights_written_trigger();
CREATE TRIGGER rights_written AFTER INSERT OR UPDATE OR DELETE OR TRUNCATE ON rbac_group
    FOR EACH STATEMENT EXECUTE FUNCTION public.kartenn_rights_written_trigger();
CREATE TRIGGER rights_written AFTER INSERT OR UPDATE OR DELETE OR TRUNCATE ON group_member
    FOR EACH STATEMENT EXECUTE FUNCTION public.kartenn_rights_written_trigger();
CREATE TRIGGER rights_written AFTER INSERT OR UPDATE OR DELETE OR TRUNCATE ON issuer_attachment
    FOR EACH STATEMENT EXECUTE FUNCTION public.kartenn_rights_written_trigger();
CREATE TRIGGER rights_written AFTER INSERT OR UPDATE OR DELETE OR TRUNCATE ON property_declaration
    FOR EACH STATEMENT EXECUTE FUNCTION public.kartenn_rights_written_trigger();

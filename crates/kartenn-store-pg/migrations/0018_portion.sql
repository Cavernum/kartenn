-- Modification over a portion of the application time (spec 004, REQ-004-14, 15; task
-- 004-8; ADR 0017, amendment of 2026-10-15).

-- A version written and then replaced in the same transaction never was visible outside
-- it: it is not copied to the history, whose system period would be empty.
CREATE OR REPLACE FUNCTION public.kartenn_versioning() RETURNS trigger
LANGUAGE plpgsql SECURITY DEFINER SET search_path = pg_catalog, public AS $$
BEGIN
    IF TG_OP = 'INSERT' THEN
        NEW.sys_start := now();
        RETURN NEW;
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

-- Applies `patch` (columns and their new values, as JSON) to the versions of `object` in
-- `target` over [from_time, to_time) ("FOR PORTION OF"): each version that the patch
-- changes is deleted and written back in up to three pieces, before and after in its old
-- state, inside with the patch; then touching versions of the same state, one of them
-- written here, are merged. Deleting before inserting keeps the exclusions of the table.
-- Returns the number of versions over the portion; 0 when the object does not exist over
-- all of it, and nothing is written.
CREATE FUNCTION public.kartenn_portion(
    target regclass, object uuid, from_time timestamptz, to_time timestamptz,
    patch jsonb, changeset uuid)
RETURNS integer
LANGUAGE plpgsql SET search_path = pg_catalog, public AS $$
DECLARE
    portion tstzrange := tstzrange(from_time, to_time);
    -- What is not the state of a version.
    meta text[] := ARRAY['valid_period', 'sys_start', 'changeset_id'];
    v jsonb;
    prev jsonb;
    vp tstzrange;
    piece tstzrange;
    touched integer := 0;
    covered boolean;
BEGIN
    -- The object must exist over the whole portion, as in the model of 004-3.
    EXECUTE format(
        'SELECT coalesce(range_agg(valid_period), ''{}'') @> $2 FROM %s WHERE id = $1 AND valid_period && $2',
        target)
        INTO covered USING object, portion;
    IF NOT covered THEN
        RETURN 0;
    END IF;
    FOR v IN EXECUTE format(
        'SELECT to_jsonb(t) FROM %s t WHERE id = $1 AND valid_period && $2', target)
        USING object, portion
    LOOP
        touched := touched + 1;
        CONTINUE WHEN (v - meta) || patch = v - meta;
        vp := (v ->> 'valid_period')::tstzrange;
        EXECUTE format('DELETE FROM %s WHERE id = $1 AND valid_period = $2', target)
            USING object, vp;
        FOREACH piece IN ARRAY ARRAY[
            vp * tstzrange(NULL, from_time),
            CASE WHEN to_time IS NULL THEN 'empty'::tstzrange ELSE vp * tstzrange(to_time, NULL) END]
        LOOP
            CONTINUE WHEN isempty(piece);
            EXECUTE format('INSERT INTO %s SELECT * FROM jsonb_populate_record(NULL::%s, $1)',
                           target, target)
                USING v || jsonb_build_object('valid_period', piece, 'changeset_id', changeset);
        END LOOP;
        EXECUTE format('INSERT INTO %s SELECT * FROM jsonb_populate_record(NULL::%s, $1)',
                       target, target)
            USING v || patch || jsonb_build_object('valid_period', vp * portion,
                                                   'changeset_id', changeset);
    END LOOP;

    FOR v IN EXECUTE format(
        'SELECT to_jsonb(t) FROM %s t WHERE id = $1 ORDER BY lower(valid_period)', target)
        USING object
    LOOP
        IF prev IS NOT NULL
           AND upper((prev ->> 'valid_period')::tstzrange) = lower((v ->> 'valid_period')::tstzrange)
           AND prev - meta = v - meta
           AND ((prev ->> 'sys_start')::timestamptz = now() OR (v ->> 'sys_start')::timestamptz = now())
        THEN
            EXECUTE format('DELETE FROM %s WHERE id = $1 AND valid_period IN ($2, $3)', target)
                USING object, (prev ->> 'valid_period')::tstzrange, (v ->> 'valid_period')::tstzrange;
            prev := prev || jsonb_build_object(
                'valid_period', tstzrange(lower((prev ->> 'valid_period')::tstzrange),
                                          upper((v ->> 'valid_period')::tstzrange)),
                'changeset_id', changeset);
            EXECUTE format('INSERT INTO %s SELECT * FROM jsonb_populate_record(NULL::%s, $1)',
                           target, target)
                USING prev;
            prev := prev || jsonb_build_object('sys_start', now());
        ELSE
            prev := v;
        END IF;
    END LOOP;
    RETURN touched;
END $$;
REVOKE ALL ON FUNCTION public.kartenn_portion(regclass, uuid, timestamptz, timestamptz, jsonb, uuid) FROM PUBLIC;
GRANT EXECUTE ON FUNCTION public.kartenn_portion(regclass, uuid, timestamptz, timestamptz, jsonb, uuid) TO kartenn_app;

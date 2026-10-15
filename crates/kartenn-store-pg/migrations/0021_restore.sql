-- Restoring an object as asserted before a changeset (spec 004, REQ-004-34, 35; task
-- 004-20).

-- Over [from_time, to_time), the current versions of the object `key` (columns and
-- values that identify it, as JSON: {"id": …} or {"object_id": …, "property_id": …}) of
-- `target` become the versions asserted just before `at`: those current or historical
-- whose system period contained the instant before `at`. Outside the portion, current
-- versions are kept, cut at its bounds; touching versions of the same state are merged.
-- Returns the number of versions restored.
CREATE FUNCTION public.kartenn_restore(
    target regclass, key jsonb, at timestamptz, from_time timestamptz, to_time timestamptz,
    changeset uuid)
RETURNS integer
LANGUAGE plpgsql SET search_path = pg_catalog, public AS $$
DECLARE
    portion tstzrange := tstzrange(from_time, to_time);
    meta text[] := ARRAY['valid_period', 'sys_start', 'changeset_id'];
    hist text := target::text || '_history';
    v jsonb;
    prev jsonb;
    vp tstzrange;
    piece tstzrange;
    restored integer := 0;
BEGIN
    -- The versions to restore, read before anything is written.
    CREATE TEMP TABLE IF NOT EXISTS kartenn_restore_before (v jsonb) ON COMMIT DROP;
    TRUNCATE kartenn_restore_before;
    EXECUTE format(
        'INSERT INTO kartenn_restore_before
         SELECT to_jsonb(c) FROM %s c WHERE to_jsonb(c) @> $1 AND c.sys_start < $2
           AND c.valid_period && $3
         UNION ALL
         SELECT to_jsonb(h) - ''sys_end'' FROM %s h WHERE to_jsonb(h) @> $1 AND h.sys_start < $2
           AND h.sys_end >= $2 AND h.valid_period && $3', target, hist)
        USING key, at, portion;

    -- Current versions over the portion: removed, their outside pieces kept.
    FOR v IN EXECUTE format(
        'SELECT to_jsonb(t) FROM %s t WHERE to_jsonb(t) @> $1 AND valid_period && $2', target)
        USING key, portion
    LOOP
        vp := (v ->> 'valid_period')::tstzrange;
        EXECUTE format('DELETE FROM %s t WHERE to_jsonb(t) @> $1 AND valid_period = $2', target)
            USING key, vp;
        FOREACH piece IN ARRAY ARRAY[
            vp * tstzrange(NULL, from_time),
            CASE WHEN to_time IS NULL THEN 'empty'::tstzrange ELSE vp * tstzrange(to_time, NULL) END]
        LOOP
            CONTINUE WHEN isempty(piece);
            EXECUTE format('INSERT INTO %s SELECT * FROM jsonb_populate_record(NULL::%s, $1)',
                           target, target)
                USING v || jsonb_build_object('valid_period', piece, 'changeset_id', changeset);
        END LOOP;
    END LOOP;

    -- The versions before, clipped to the portion.
    FOR v IN SELECT b.v FROM kartenn_restore_before b LOOP
        vp := (v ->> 'valid_period')::tstzrange * portion;
        CONTINUE WHEN isempty(vp);
        EXECUTE format('INSERT INTO %s SELECT * FROM jsonb_populate_record(NULL::%s, $1)',
                       target, target)
            USING v || jsonb_build_object('valid_period', vp, 'changeset_id', changeset);
        restored := restored + 1;
    END LOOP;

    -- Touching versions of the same state, one of them written here, merge.
    FOR v IN EXECUTE format(
        'SELECT to_jsonb(t) FROM %s t WHERE to_jsonb(t) @> $1 ORDER BY lower(valid_period)', target)
        USING key
    LOOP
        IF prev IS NOT NULL
           AND upper((prev ->> 'valid_period')::tstzrange) = lower((v ->> 'valid_period')::tstzrange)
           AND prev - meta = v - meta
           AND ((prev ->> 'sys_start')::timestamptz = now() OR (v ->> 'sys_start')::timestamptz = now())
        THEN
            EXECUTE format('DELETE FROM %s t WHERE to_jsonb(t) @> $1 AND valid_period IN ($2, $3)', target)
                USING key, (prev ->> 'valid_period')::tstzrange, (v ->> 'valid_period')::tstzrange;
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
    RETURN restored;
END $$;
REVOKE ALL ON FUNCTION public.kartenn_restore(regclass, jsonb, timestamptz, timestamptz, timestamptz, uuid) FROM PUBLIC;
GRANT EXECUTE ON FUNCTION public.kartenn_restore(regclass, jsonb, timestamptz, timestamptz, timestamptz, uuid) TO kartenn_app;

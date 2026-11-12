-- The retention of the journal of the pods observed (REQ-019-16; ADR 0043, decisions 4,
-- 5 and 8, amended on 2026-11-12; 019/Q-019-20, Q-019-24, Q-019-27; task 019-6c): the
-- days of the closed observations stay common to the clusters, the purge goes by cluster,
-- under a bound it writes for each one; the verification recomputes the batches the purge
-- has not touched. The journal holds against a compromised role of the application
-- (Q-019-27): that role still has neither DELETE nor TRUNCATE nor UPDATE of a batch, and
-- what it writes cites only a batch of its own transaction.

-- The earliest instant of a batch: the smallest start it opened and end it set, its own
-- instant when it wrote nothing. Every row the batch wrote ends at or after it, so that a
-- batch whose earliest instant is not before the purge bound of its cluster lost no row
-- to the purge: it is whole, and its fingerprint is recomputed (Q-019-24). It is sealed in
-- the event of the batch (`observation.batch.recorded`, detail `earliest`).
ALTER TABLE observation_batch ADD COLUMN earliest timestamptz;
UPDATE observation_batch b
   SET earliest = least(b.received_at,
                        (SELECT min(lower(o.observed)) FROM pod_observation o
                          WHERE o.opened_batch = b.id),
                        (SELECT min(upper(o.observed)) FROM pod_observation o
                          WHERE o.closed_batch = b.id));
ALTER TABLE observation_batch ALTER COLUMN earliest SET NOT NULL,
    ADD CONSTRAINT observation_batch_earliest CHECK (earliest <= received_at);

-- A batch is received at the instant of its transaction (Q-019-27): its event is recorded
-- then, and the month of that instant tells whether the audit has purged it.
CREATE FUNCTION observation_batch_received_now() RETURNS trigger
LANGUAGE plpgsql SET search_path = pg_catalog AS $$
BEGIN
    IF NEW.received_at IS DISTINCT FROM now() THEN
        RAISE EXCEPTION 'kube.observation_batch: a batch is received at the instant of its transaction'
            USING ERRCODE = 'insufficient_privilege';
    END IF;
    RETURN NEW;
END
$$;
CREATE TRIGGER observation_batch_received_now BEFORE INSERT ON observation_batch
    FOR EACH ROW EXECUTE FUNCTION observation_batch_received_now();

-- Whether `batch` was written by the current transaction.
CREATE FUNCTION observation_batch_of_this_transaction(batch uuid) RETURNS boolean
LANGUAGE sql STABLE SET search_path = pg_catalog AS $$
    SELECT EXISTS (SELECT 1 FROM kube.observation_batch b
                    WHERE b.id = batch AND b.xmin = pg_current_xact_id()::xid)
$$;
GRANT EXECUTE ON FUNCTION observation_batch_of_this_transaction(uuid) TO kartenn_app;

-- An observation opens under a batch of its own transaction, and closes under one
-- (Q-019-27): no row is ever ascribed to an older batch, whose fingerprint is sealed. A
-- closing moves the row to the partition of its day, which fires this trigger there with
-- the closing batch set.
CREATE FUNCTION pod_observation_of_its_batch() RETURNS trigger
LANGUAGE plpgsql SET search_path = pg_catalog AS $$
BEGIN
    IF (NEW.closed_batch IS NULL AND NOT kube.observation_batch_of_this_transaction(NEW.opened_batch))
       OR (NEW.closed_batch IS NOT NULL
           AND NOT kube.observation_batch_of_this_transaction(NEW.closed_batch))
    THEN
        RAISE EXCEPTION 'kube.pod_observation: a row opens or closes under a batch of its own transaction'
            USING ERRCODE = 'insufficient_privilege';
    END IF;
    RETURN NEW;
END
$$;
CREATE TRIGGER pod_observation_of_its_batch BEFORE INSERT OR UPDATE ON pod_observation
    FOR EACH ROW EXECUTE FUNCTION pod_observation_of_its_batch();

-- The purge bound of a cluster (Q-019-20): every closed observation of the cluster that
-- ended before `purged_before` has been deleted. It only moves forward, written by
-- `observation_purge` alone, each move sealed in `observation.journal.purged`; the
-- verification checks it against those events.
CREATE TABLE observation_purge_bound (
    cluster_id     uuid NOT NULL PRIMARY KEY,
    purged_before  timestamptz NOT NULL,
    purged_at      timestamptz NOT NULL
);
GRANT SELECT ON observation_purge_bound TO kartenn_app;

-- The purge of the journal, by the owner of the schema, at the instant of its
-- transaction (never at a date its caller would give), one row by cluster for its event:
-- - the retention of a cluster is the largest of its versions in force, in valid time
--   and in system time (its history), within their own retention up to now: a retention
--   lowered, or rewritten by a correction, applies once the former one has run out
--   (Q-019-27); a cluster without such a version keeps that of its latest version;
-- - its bound moves to now minus that retention, never back;
-- - its closed observations ended before the bound are deleted, an open one never;
-- - the days from today to a week ahead are created, so that a batch seldom creates one
--   under the lock of the closed observations (task 019-6a);
-- - a day is dropped once every cluster has passed its end.
-- The creations and the drops come last: their locks on the closed observations are held
-- only to the commit, and no batch waits for the deletions, which touch closed rows alone.
CREATE FUNCTION observation_purge()
RETURNS TABLE (cluster_id uuid, bound_before timestamptz, bound_after timestamptz,
               deleted bigint, dropped date[], created integer)
LANGUAGE plpgsql SECURITY DEFINER SET search_path = pg_catalog AS $$
#variable_conflict use_column
DECLARE
    part record;
    n bigint;
    after timestamptz;
    passed timestamptz;
    today date := (now() AT TIME ZONE 'UTC')::date;
    days_dropped date[] := '{}';
    days_created integer := 0;
    moved record;
    ids uuid[] := '{}';
    befores timestamptz[] := '{}';
    afters timestamptz[] := '{}';
    counts bigint[] := '{}';
BEGIN
    FOR moved IN
        WITH versions AS (
            SELECT k.id, k.retention_days, k.valid_period, tstzrange(k.sys_start, NULL) AS known
              FROM kube.cluster k
            UNION ALL
            SELECT h.id, h.retention_days, h.valid_period, tstzrange(h.sys_start, h.sys_end)
              FROM kube.cluster_history h),
        recent AS (
            SELECT v.id, max(v.retention_days) AS days
              FROM versions v
             WHERE v.valid_period && tstzrange(now() - make_interval(days => v.retention_days), now(), '[]')
               AND v.known && tstzrange(now() - make_interval(days => v.retention_days), now(), '[]')
             GROUP BY v.id),
        latest AS (
            SELECT DISTINCT ON (k.id) k.id, k.retention_days AS days
              FROM kube.cluster k
             ORDER BY k.id, upper(k.valid_period) DESC NULLS FIRST)
        SELECT l.id, coalesce(r.days, l.days) AS days, p.purged_before AS before
          FROM latest l
          LEFT JOIN recent r ON r.id = l.id
          LEFT JOIN kube.observation_purge_bound p ON p.cluster_id = l.id
    LOOP
        INSERT INTO kube.observation_purge_bound AS p (cluster_id, purged_before, purged_at)
        VALUES (moved.id, now() - make_interval(days => moved.days), now())
        ON CONFLICT ON CONSTRAINT observation_purge_bound_pkey DO UPDATE
           SET purged_before = greatest(p.purged_before, excluded.purged_before),
               purged_at = excluded.purged_at
        RETURNING p.purged_before INTO after;
        DELETE FROM kube.pod_observation_closed o
         WHERE o.cluster_id = moved.id AND upper(o.observed) < after;
        GET DIAGNOSTICS n = ROW_COUNT;
        ids := ids || moved.id;
        befores := befores || moved.before;
        afters := afters || after;
        counts := counts || n;
    END LOOP;
    FOR d IN 0..7 LOOP
        IF to_regclass(format('kube.pod_observation_%s', to_char(today + d, 'YYYYMMDD'))) IS NULL
        THEN
            PERFORM kube.observation_partition(today + d);
            days_created := days_created + 1;
        END IF;
    END LOOP;
    SELECT min(b.purged_before) INTO passed FROM kube.observation_purge_bound b;
    FOR part IN
        SELECT c.relname, to_date(substr(c.relname, 17), 'YYYYMMDD') AS day
          FROM pg_catalog.pg_inherits i
          JOIN pg_catalog.pg_class c ON c.oid = i.inhrelid
         WHERE i.inhparent = 'kube.pod_observation_closed'::regclass
           AND c.relname ~ '^pod_observation_[0-9]{8}$'
           AND (to_date(substr(c.relname, 17), 'YYYYMMDD') + 1)::timestamp AT TIME ZONE 'UTC'
               <= passed
         ORDER BY c.relname
    LOOP
        EXECUTE format('DROP TABLE kube.%I', part.relname);
        days_dropped := days_dropped || part.day;
    END LOOP;
    RETURN QUERY
        SELECT t.id, t.before, t.after, t.n, days_dropped, days_created
          FROM unnest(ids, befores, afters, counts) AS t (id, before, after, n)
         ORDER BY t.id;
END
$$;
REVOKE ALL ON FUNCTION observation_purge() FROM PUBLIC;
GRANT EXECUTE ON FUNCTION observation_purge() TO kartenn_app;

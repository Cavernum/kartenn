-- The retention of the journal of the pods observed (REQ-019-16; ADR 0043, decisions 5
-- and 8, amended on 2026-11-11; 019/Q-019-20, Q-019-24; task 019-6c): the days of the
-- closed observations stay common to the clusters, the purge goes by cluster, under a
-- bound it writes for each one; the verification recomputes only the batches the purge
-- has not touched. The role of the application still has neither DELETE nor TRUNCATE.

-- The earliest instant of a batch: the smallest start it opened and end it set, its own
-- instant when it wrote nothing. Every row the batch wrote ends at or after it, so that a
-- batch whose earliest instant is not before the purge bound of its cluster lost no row
-- to the purge: it is whole, and its fingerprint is recomputed (Q-019-24).
ALTER TABLE observation_batch ADD COLUMN earliest timestamptz;
UPDATE observation_batch b
   SET earliest = least(b.received_at,
                        (SELECT min(lower(o.observed)) FROM pod_observation o
                          WHERE o.opened_batch = b.id),
                        (SELECT min(upper(o.observed)) FROM pod_observation o
                          WHERE o.closed_batch = b.id));
ALTER TABLE observation_batch ALTER COLUMN earliest SET NOT NULL,
    ADD CONSTRAINT observation_batch_earliest CHECK (earliest <= received_at);

-- The purge bound of a cluster (Q-019-20): every closed observation of the cluster that
-- ended before `purged_before` has been deleted. It only moves forward, written by
-- `observation_purge` alone; the verification reads it.
CREATE TABLE observation_purge_bound (
    cluster_id     uuid NOT NULL PRIMARY KEY,
    purged_before  timestamptz NOT NULL,
    purged_at      timestamptz NOT NULL
);
GRANT SELECT ON observation_purge_bound TO kartenn_app;

-- The purge of the journal, by the owner of the schema, at the instant of its
-- transaction (never at a date its caller would give): for each cluster, the bound moves
-- to the retention of the cluster (its version in force, else its latest), never back;
-- the closed observations of the cluster that ended before it are deleted, an open one
-- never; a day is dropped once every cluster has passed its end; the days from today to
-- a week ahead are created, so that a batch seldom creates one under the lock of the
-- closed observations (task 019-6a). The creations and the drops come last: their locks
-- on the closed observations are held only to the commit, and no batch waits for the
-- deletions, which touch closed rows alone.
CREATE FUNCTION observation_purge(OUT deleted bigint, OUT dropped integer, OUT created integer)
LANGUAGE plpgsql SECURITY DEFINER SET search_path = pg_catalog AS $$
DECLARE
    bound record;
    part record;
    n bigint;
    passed timestamptz;
    today date := (now() AT TIME ZONE 'UTC')::date;
BEGIN
    deleted := 0;
    dropped := 0;
    created := 0;
    INSERT INTO kube.observation_purge_bound AS p (cluster_id, purged_before, purged_at)
    SELECT DISTINCT ON (k.id) k.id, now() - make_interval(days => k.retention_days), now()
      FROM kube.cluster k
     ORDER BY k.id, k.valid_period @> now() DESC, upper(k.valid_period) DESC NULLS FIRST
    ON CONFLICT (cluster_id) DO UPDATE
       SET purged_before = greatest(p.purged_before, excluded.purged_before),
           purged_at = excluded.purged_at;
    FOR bound IN SELECT cluster_id, purged_before FROM kube.observation_purge_bound LOOP
        DELETE FROM kube.pod_observation_closed
         WHERE cluster_id = bound.cluster_id AND upper(observed) < bound.purged_before;
        GET DIAGNOSTICS n = ROW_COUNT;
        deleted := deleted + n;
    END LOOP;
    FOR d IN 0..7 LOOP
        IF to_regclass(format('kube.pod_observation_%s', to_char(today + d, 'YYYYMMDD'))) IS NULL
        THEN
            PERFORM kube.observation_partition(today + d);
            created := created + 1;
        END IF;
    END LOOP;
    SELECT min(purged_before) INTO passed FROM kube.observation_purge_bound;
    FOR part IN
        SELECT c.relname
          FROM pg_catalog.pg_inherits i
          JOIN pg_catalog.pg_class c ON c.oid = i.inhrelid
         WHERE i.inhparent = 'kube.pod_observation_closed'::regclass
           AND c.relname ~ '^pod_observation_[0-9]{8}$'
           AND (to_date(substr(c.relname, 17), 'YYYYMMDD') + 1)::timestamp AT TIME ZONE 'UTC'
               <= passed
         ORDER BY c.relname
    LOOP
        EXECUTE format('DROP TABLE kube.%I', part.relname);
        dropped := dropped + 1;
    END LOOP;
END
$$;
REVOKE ALL ON FUNCTION observation_purge() FROM PUBLIC;
GRANT EXECUTE ON FUNCTION observation_purge() TO kartenn_app;

-- The journal of the pods observed (REQ-019-16, 019/X7, ADR 0043; design « Données »;
-- task 019-6a): outside the model, neither versioned nor written by a changeset, written
-- by batches of the connector, append only for the role of the application.

-- A batch accepted. Its fingerprint is sealed in the audit (ADR 0043, decision 5): this
-- copy only serves the search and the verification.
CREATE TABLE observation_batch (
    id            uuid NOT NULL PRIMARY KEY,
    cluster_id    uuid NOT NULL,
    author        uuid NOT NULL,
    received_at   timestamptz NOT NULL,
    opened        integer NOT NULL CHECK (opened >= 0),
    closed        integer NOT NULL CHECK (closed >= 0),
    -- SHA-256 of the canonical content (RFC 8785), as `observation.batch.recorded` has it.
    content_hash  bytea NOT NULL CHECK (octet_length(content_hash) = 32)
);
CREATE INDEX observation_batch_by_cluster ON observation_batch (cluster_id, received_at);
GRANT SELECT, INSERT ON observation_batch TO kartenn_app;

-- A pod seen with an IP (one row per IP), or without one in `hostNetwork` (the address is
-- the node's), over the period `[seen, gone)`, open while the pod lives. Partitioned by
-- the end of the period (task 019-6a): the open observations in one partition, the
-- closed ones by day of their end, so that a purge drops whole days past the retention
-- and never an open observation (ADR 0043, decision 8). An expression as the key admits no
-- primary key: one open observation per (cluster, pod, IP) is unique in the partition of
-- the open ones, the validation of a batch refuses the rest.
CREATE TABLE pod_observation (
    cluster_id    uuid NOT NULL,
    namespace     text NOT NULL CHECK (namespace <> ''),
    pod_name      text NOT NULL CHECK (pod_name <> ''),
    pod_uid       uuid NOT NULL,
    workload_id   uuid,
    node_id       uuid,
    ip            inet CHECK (ip IS NULL OR masklen(ip) = CASE family(ip) WHEN 4 THEN 32 ELSE 128 END),
    host_network  boolean NOT NULL,
    observed      tstzrange NOT NULL CHECK (NOT isempty(observed) AND NOT lower_inf(observed)
                                            AND lower_inc(observed) AND NOT upper_inc(observed)),
    -- The batch that opened it, the one that set its end: a batch accepted, so that no
    -- end is set outside the fingerprint of a batch.
    opened_batch  uuid NOT NULL REFERENCES observation_batch (id),
    closed_batch  uuid REFERENCES observation_batch (id),
    CHECK (host_network = (ip IS NULL)),
    CHECK (upper_inf(observed) = (closed_batch IS NULL))
) PARTITION BY LIST (upper_inf(observed));
CREATE TABLE pod_observation_open PARTITION OF pod_observation FOR VALUES IN (true);
-- The days are created by `observation_partition`, as a batch closes in them.
CREATE TABLE pod_observation_closed PARTITION OF pod_observation FOR VALUES IN (false)
    PARTITION BY RANGE (upper(observed));
CREATE UNIQUE INDEX pod_observation_open_key
    ON pod_observation_open (cluster_id, pod_uid, ip) NULLS NOT DISTINCT;
-- Who had an IP at an instant; where a workload ran.
CREATE INDEX pod_observation_by_ip ON pod_observation USING GIST (cluster_id, ip inet_ops, observed);
CREATE INDEX pod_observation_by_workload ON pod_observation (workload_id, observed);
-- The observations of the pods of a batch, open or closed, read by its checks.
CREATE INDEX pod_observation_by_pod ON pod_observation (cluster_id, pod_uid);
-- The verification of a batch reads its rows back.
CREATE INDEX pod_observation_by_opened ON pod_observation (opened_batch);
CREATE INDEX pod_observation_by_closed ON pod_observation (closed_batch) WHERE closed_batch IS NOT NULL;

-- Append only (ADR 0043, decision 4): the role of the application inserts, and sets the
-- end of a period still open, nothing else; no DELETE, no TRUNCATE, and no right on the
-- partitions themselves. The trigger refuses any other update, whoever makes it.
GRANT SELECT, INSERT ON pod_observation TO kartenn_app;
GRANT UPDATE (observed, closed_batch) ON pod_observation TO kartenn_app;

CREATE FUNCTION pod_observation_append_only() RETURNS trigger
LANGUAGE plpgsql SET search_path = pg_catalog AS $$
BEGIN
    IF NOT upper_inf(OLD.observed) OR OLD.closed_batch IS NOT NULL
       OR upper_inf(NEW.observed) OR NEW.closed_batch IS NULL
       OR lower(NEW.observed) IS DISTINCT FROM lower(OLD.observed)
       OR (NEW.cluster_id, NEW.namespace, NEW.pod_name, NEW.pod_uid, NEW.workload_id,
           NEW.node_id, NEW.ip, NEW.host_network, NEW.opened_batch)
          IS DISTINCT FROM
          (OLD.cluster_id, OLD.namespace, OLD.pod_name, OLD.pod_uid, OLD.workload_id,
           OLD.node_id, OLD.ip, OLD.host_network, OLD.opened_batch)
    THEN
        RAISE EXCEPTION 'kube.pod_observation is append only: an update only sets the end of an open period'
            USING ERRCODE = 'insufficient_privilege';
    END IF;
    RETURN NEW;
END
$$;
CREATE TRIGGER pod_observation_append_only BEFORE UPDATE ON pod_observation
    FOR EACH ROW EXECUTE FUNCTION pod_observation_append_only();

-- The partition of the day `day` (UTC) of the closed observations, created if missing,
-- by the owner of the schema: the role of the application cannot create a table. The
-- lock serialises two batches that close in the same new day.
CREATE FUNCTION observation_partition(day date) RETURNS void
LANGUAGE plpgsql SECURITY DEFINER SET search_path = pg_catalog AS $$
DECLARE
    name text := format('pod_observation_%s', to_char(day, 'YYYYMMDD'));
BEGIN
    IF to_regclass(format('kube.%I', name)) IS NOT NULL THEN
        RETURN;
    END IF;
    -- A lock on the table, not an advisory one: taking it reads the invalidations of the
    -- catalogue, so that a day another batch created meanwhile is seen.
    LOCK TABLE ONLY kube.pod_observation_closed IN SHARE UPDATE EXCLUSIVE MODE;
    IF to_regclass(format('kube.%I', name)) IS NOT NULL THEN
        RETURN;
    END IF;
    EXECUTE format(
        'CREATE TABLE kube.%I PARTITION OF kube.pod_observation_closed FOR VALUES FROM (%L) TO (%L)',
        name, day::timestamp AT TIME ZONE 'UTC', (day + 1)::timestamp AT TIME ZONE 'UTC');
    EXECUTE format('REVOKE ALL ON kube.%I FROM PUBLIC', name);
END
$$;
REVOKE ALL ON FUNCTION observation_partition(date) FROM PUBLIC;
GRANT EXECUTE ON FUNCTION observation_partition(date) TO kartenn_app;

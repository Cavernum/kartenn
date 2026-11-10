-- The workloads the connector mirrors (REQ-019-8, 019/X3, X4; design « Données »; task
-- 019-4a): the root controller of the pods, tied to a fixed namespace, its kind and name
-- unique in the namespace at every instant. The Kubernetes UID is an attribute: a
-- workload deleted and created again under the same name keeps its identity, a new
-- version notes the new UID. Only the tracked fields are versioned: the tracked labels of
-- the pod template, the service account, the declared ports (declared, never proven
-- listening) and `hostNetwork`.

CREATE TABLE workload (
    id               uuid NOT NULL,
    namespace_id     uuid NOT NULL,
    kind             text NOT NULL CHECK (kind IN ('deployment', 'statefulset', 'daemonset',
                                                   'cronjob', 'job', 'replicaset', 'pod')),
    name             text NOT NULL,
    uid              uuid,
    -- The tracked labels of the pod template only (X4).
    labels           jsonb NOT NULL DEFAULT '{}',
    service_account  text,
    -- [{port, protocol, name}] as declared by the containers, a sorted set.
    ports            jsonb NOT NULL DEFAULT '[]',
    host_network     boolean NOT NULL DEFAULT false,
    absent_since     timestamptz,
    valid_period     tstzrange NOT NULL CHECK (NOT isempty(valid_period)),
    sys_start        timestamptz NOT NULL DEFAULT now(),
    changeset_id     uuid NOT NULL,
    PRIMARY KEY (id, valid_period WITHOUT OVERLAPS),
    CONSTRAINT workload_name_unique EXCLUDE USING GIST
        (namespace_id WITH =, kind WITH =, name WITH =, valid_period WITH &&)
);
CALL public.kartenn_make_versioned('workload');
-- The workloads of a namespace, asked whenever it closes (reference `kube.workload` →
-- `kube.namespace`, closed in cascade).
CREATE INDEX workload_by_namespace ON workload (namespace_id);
CREATE INDEX workload_by_uid ON workload (uid) WHERE uid IS NOT NULL;
CREATE INDEX workload_labels ON workload USING GIN (labels);

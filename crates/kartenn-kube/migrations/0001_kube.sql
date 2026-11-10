-- The kube module (spec 019, REQ-019-4 to 7; design « Données »), in the schema `kube`
-- the module migrator creates and puts first in the search path: the clusters an
-- administrator declares, and the nodes and namespaces the connector mirrors, versioned
-- (ADR 0017) like the tables of the 009. The constraints are the last line of defence,
-- the rules of the module give the messages. The workloads, services and ingresses come
-- with task 019-4, the observation journal with task 019-6.

-- A cluster, with its designated domains and ranges (Q-019-5), the retention of its
-- observations, its own tracked label keys, the window of the connector's batches and
-- the group of the connector (025, X5: a group, never an identity).
CREATE TABLE cluster (
    id                    uuid NOT NULL,
    slug                  text NOT NULL,
    pod_domain_id         uuid NOT NULL,
    service_domain_id     uuid NOT NULL,
    -- The addresses LoadBalancer and externalIPs.
    external_domain_id    uuid,
    -- Containers of the core.
    pod_container_ids     uuid[] NOT NULL DEFAULT '{}',
    -- Networks of the core.
    service_network_ids   uuid[] NOT NULL DEFAULT '{}',
    -- `machines.cluster`, shown only.
    machines_cluster_id   uuid,
    retention_days        smallint NOT NULL DEFAULT 30 CHECK (retention_days BETWEEN 1 AND 400),
    -- In addition to those of the module.
    tracked_label_keys    text[] NOT NULL DEFAULT '{}',
    batch_window_seconds  smallint NOT NULL DEFAULT 10 CHECK (batch_window_seconds BETWEEN 1 AND 300),
    batch_max_operations  integer NOT NULL DEFAULT 500 CHECK (batch_max_operations BETWEEN 1 AND 10000),
    create_node_machines  boolean NOT NULL DEFAULT false,
    -- A group of kartenn (025), X5; never an identity.
    connector_group_id    uuid,
    valid_period          tstzrange NOT NULL CHECK (NOT isempty(valid_period)),
    sys_start             timestamptz NOT NULL DEFAULT now(),
    changeset_id          uuid NOT NULL,
    PRIMARY KEY (id, valid_period WITHOUT OVERLAPS),
    CONSTRAINT cluster_slug_unique EXCLUDE USING GIST (slug WITH =, valid_period WITH &&)
);
CALL public.kartenn_make_versioned('cluster');

-- A node, tied to its cluster, named as in Kubernetes, unique in the cluster at every
-- instant; the machine `physical` or `virtual` it designates, one living node per
-- machine. `range_ids` stays empty until task 019-3c.
CREATE TABLE node (
    id            uuid NOT NULL,
    cluster_id    uuid NOT NULL,
    name          text NOT NULL,
    uid           uuid,
    -- `machines.machine`, physical or virtual.
    machine_id    uuid,
    -- [{type, value}] as read.
    addresses     jsonb NOT NULL DEFAULT '[]',
    -- Networks of the core (pod ranges).
    range_ids     uuid[] NOT NULL DEFAULT '{}',
    zone          text,
    absent_since  timestamptz,
    valid_period  tstzrange NOT NULL CHECK (NOT isempty(valid_period)),
    sys_start     timestamptz NOT NULL DEFAULT now(),
    changeset_id  uuid NOT NULL,
    PRIMARY KEY (id, valid_period WITHOUT OVERLAPS),
    CONSTRAINT node_name_unique EXCLUDE USING GIST
        (cluster_id WITH =, name WITH =, valid_period WITH &&),
    CONSTRAINT node_machine_unique EXCLUDE USING GIST
        (machine_id WITH =, valid_period WITH &&) WHERE (machine_id IS NOT NULL)
);
CALL public.kartenn_make_versioned('node');
CREATE INDEX node_by_machine ON node (machine_id) WHERE machine_id IS NOT NULL;
CREATE INDEX node_by_uid ON node (uid) WHERE uid IS NOT NULL;

-- A namespace, tied to its cluster, its name unique in the cluster at every instant, with
-- its tracked labels only (X4).
CREATE TABLE namespace (
    id            uuid NOT NULL,
    cluster_id    uuid NOT NULL,
    name          text NOT NULL,
    uid           uuid,
    labels        jsonb NOT NULL DEFAULT '{}',
    absent_since  timestamptz,
    valid_period  tstzrange NOT NULL CHECK (NOT isempty(valid_period)),
    sys_start     timestamptz NOT NULL DEFAULT now(),
    changeset_id  uuid NOT NULL,
    PRIMARY KEY (id, valid_period WITHOUT OVERLAPS),
    CONSTRAINT namespace_name_unique EXCLUDE USING GIST
        (cluster_id WITH =, name WITH =, valid_period WITH &&)
);
CALL public.kartenn_make_versioned('namespace');
CREATE INDEX namespace_by_uid ON namespace (uid) WHERE uid IS NOT NULL;
CREATE INDEX namespace_labels ON namespace USING GIN (labels);

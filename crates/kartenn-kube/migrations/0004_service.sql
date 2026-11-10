-- The services the connector mirrors (REQ-019-9, 14, 019/X3, X4, X6; design « Données »;
-- task 019-4b): tied to a fixed namespace, their name unique in the namespace at every
-- instant, the Kubernetes UID an attribute. The cluster IPs and the external addresses
-- (LoadBalancer, `externalIPs`) are addresses of the core of usage `vip`, in the service
-- domain and in the external domain of the cluster, which the service owns (closed with
-- it); the selection is the set of workloads of the pods of its EndpointSlices, computed
-- by the connector.

CREATE TABLE service (
    id                    uuid NOT NULL,
    namespace_id          uuid NOT NULL,
    name                  text NOT NULL,
    uid                   uuid,
    type                  text NOT NULL CHECK (type IN ('cluster_ip', 'node_port',
                                                        'load_balancer', 'external_name')),
    headless              boolean NOT NULL DEFAULT false,
    external_name         text,
    -- [{port, target_port, protocol, app_protocol, node_port, name}], a sorted set.
    ports                 jsonb NOT NULL DEFAULT '[]',
    cluster_address_ids   uuid[] NOT NULL DEFAULT '{}',
    external_address_ids  uuid[] NOT NULL DEFAULT '{}',
    workload_ids          uuid[] NOT NULL DEFAULT '{}',
    absent_since          timestamptz,
    valid_period          tstzrange NOT NULL CHECK (NOT isempty(valid_period)),
    sys_start             timestamptz NOT NULL DEFAULT now(),
    changeset_id          uuid NOT NULL,
    PRIMARY KEY (id, valid_period WITHOUT OVERLAPS),
    CONSTRAINT service_name_unique EXCLUDE USING GIST
        (namespace_id WITH =, name WITH =, valid_period WITH &&)
);
CALL public.kartenn_make_versioned('service');
-- The services of a namespace, asked whenever it closes (`kube.service` →
-- `kube.namespace`, closed in cascade).
CREATE INDEX service_by_namespace ON service (namespace_id);
CREATE INDEX service_by_uid ON service (uid) WHERE uid IS NOT NULL;
-- The services that select a workload: the other way of the selection, and the services
-- a closed workload leaves (`kube.service` → `kube.workload`, cleared).
CREATE INDEX service_by_workload ON service USING GIN (workload_ids);
-- The service that owns an address, asked whenever an address of the core closes.
CREATE INDEX service_cluster_addresses ON service USING GIN (cluster_address_ids);
CREATE INDEX service_external_addresses ON service USING GIN (external_address_ids);

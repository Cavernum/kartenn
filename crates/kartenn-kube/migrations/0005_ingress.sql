-- The ingresses the connector mirrors (REQ-019-10, 14, 019/X3, X4; design « Données »;
-- task 019-4c): tied to a fixed namespace, their name unique in the namespace at every
-- instant, the Kubernetes UID an attribute. Their routes name a service of the namespace
-- by its name, not by a reference: a route to a service absent from the namespace is a
-- warning (`kube.ingress.backend_missing`), as the cluster accepts it. Their addresses
-- (`status.loadBalancer.ingress`) are addresses of the core of usage `vip`, in the
-- external domain of the cluster (its service domain without one), which the ingress owns
-- (closed with it).

CREATE TABLE ingress (
    id            uuid NOT NULL,
    namespace_id  uuid NOT NULL,
    name          text NOT NULL,
    uid           uuid,
    class         text,
    -- [{host, path, path_type, service, port}], a sorted set; the default backend is the
    -- route without host nor path.
    rules         jsonb NOT NULL DEFAULT '[]',
    tls_hosts     text[] NOT NULL DEFAULT '{}',
    address_ids   uuid[] NOT NULL DEFAULT '{}',
    absent_since  timestamptz,
    valid_period  tstzrange NOT NULL CHECK (NOT isempty(valid_period)),
    sys_start     timestamptz NOT NULL DEFAULT now(),
    changeset_id  uuid NOT NULL,
    PRIMARY KEY (id, valid_period WITHOUT OVERLAPS),
    CONSTRAINT ingress_name_unique EXCLUDE USING GIST
        (namespace_id WITH =, name WITH =, valid_period WITH &&)
);
CALL public.kartenn_make_versioned('ingress');
-- The ingresses of a namespace, asked whenever it closes (`kube.ingress` →
-- `kube.namespace`, closed in cascade) and whenever one of its services is written (the
-- routes to check again).
CREATE INDEX ingress_by_namespace ON ingress (namespace_id);
CREATE INDEX ingress_by_uid ON ingress (uid) WHERE uid IS NOT NULL;
-- The ingress that owns an address, asked whenever an address of the core closes.
CREATE INDEX ingress_addresses ON ingress USING GIN (address_ids);

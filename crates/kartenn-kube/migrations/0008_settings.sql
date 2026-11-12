-- The settings of the module (spec 019, REQ-019-34, Q-019-35; design « Données »; task
-- 019-7c): three types of the module, global, written by changeset under
-- `kube.settings.edit` and read under `kube.cluster.read`. A mapping gives a value of an
-- axis to the workloads that carry a label of its key and value; the admitted values of
-- an axis in a namespace bound what a label may give there (019/X11); the tracked labels
-- are the keys the connector keeps in every cluster (019/X4). The handler checks the
-- syntax of the keys and values and the axis before writing.

CREATE TABLE label_mapping (
    id            uuid NOT NULL,
    label_key     text NOT NULL,
    label_value   text NOT NULL,
    axis_key      text NOT NULL,
    axis_value    text NOT NULL,
    valid_period  tstzrange NOT NULL CHECK (NOT isempty(valid_period)),
    sys_start     timestamptz NOT NULL DEFAULT now(),
    changeset_id  uuid NOT NULL,
    PRIMARY KEY (id, valid_period WITHOUT OVERLAPS),
    -- One value of an axis for a label at every instant; the derivation of a workload
    -- reads the mappings of its labels by this index.
    CONSTRAINT label_mapping_unique EXCLUDE USING GIST
        (label_key WITH =, label_value WITH =, axis_key WITH =, valid_period WITH &&)
);
CALL public.kartenn_make_versioned('label_mapping');

CREATE TABLE admitted_values (
    id            uuid NOT NULL,
    namespace_id  uuid NOT NULL,
    axis_key      text NOT NULL,
    -- A sorted set; an empty one closes the object instead.
    axis_values   text[] NOT NULL CHECK (cardinality(axis_values) > 0),
    valid_period  tstzrange NOT NULL CHECK (NOT isempty(valid_period)),
    sys_start     timestamptz NOT NULL DEFAULT now(),
    changeset_id  uuid NOT NULL,
    PRIMARY KEY (id, valid_period WITHOUT OVERLAPS),
    CONSTRAINT admitted_values_unique EXCLUDE USING GIST
        (namespace_id WITH =, axis_key WITH =, valid_period WITH &&)
);
CALL public.kartenn_make_versioned('admitted_values');
-- The admitted values of a namespace, asked whenever it closes (`kube.admitted_values` →
-- `kube.namespace`, refused while they live) and by the derivation of its workloads.
CREATE INDEX admitted_values_by_namespace ON admitted_values (namespace_id);

CREATE TABLE tracked_label (
    id            uuid NOT NULL,
    label_key     text NOT NULL,
    valid_period  tstzrange NOT NULL CHECK (NOT isempty(valid_period)),
    sys_start     timestamptz NOT NULL DEFAULT now(),
    changeset_id  uuid NOT NULL,
    PRIMARY KEY (id, valid_period WITHOUT OVERLAPS),
    CONSTRAINT tracked_label_unique EXCLUDE USING GIST
        (label_key WITH =, valid_period WITH &&)
);
CALL public.kartenn_make_versioned('tracked_label');

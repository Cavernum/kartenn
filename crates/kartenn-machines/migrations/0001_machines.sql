-- The machines module (spec 009, REQ-009-2 to 5; design « Données »), in the schema
-- `machines` the module migrator creates and puts first in the search path. Clusters of
-- virtualisation and machines, both versioned (ADR 0017): the constraints are the last
-- line of defence, the rules of the module give the messages.

CREATE TABLE cluster (
    id            uuid NOT NULL,
    slug          text NOT NULL CHECK (slug ~ '^[a-z0-9]([a-z0-9-]{0,61}[a-z0-9])?$'),
    valid_period  tstzrange NOT NULL CHECK (NOT isempty(valid_period)),
    sys_start     timestamptz NOT NULL DEFAULT now(),
    changeset_id  uuid NOT NULL,
    PRIMARY KEY (id, valid_period WITHOUT OVERLAPS),
    CONSTRAINT cluster_slug_unique EXCLUDE USING GIST (slug WITH =, valid_period WITH &&)
);
CALL public.kartenn_make_versioned('cluster');

-- The name of a machine is unique per resolved owner (M3): checked by the module on the
-- final state, since the owner is a resolved property, not a column.
CREATE TABLE machine (
    id                uuid NOT NULL,
    name              text NOT NULL
                      CHECK (name = lower(name) AND char_length(name) BETWEEN 1 AND 253),
    kind              text NOT NULL
                      CHECK (kind IN ('physical', 'virtual', 'container', 'appliance')),
    host_machine_id   uuid,
    host_cluster_id   uuid,
    member_cluster_id uuid,
    valid_period      tstzrange NOT NULL CHECK (NOT isempty(valid_period)),
    sys_start         timestamptz NOT NULL DEFAULT now(),
    changeset_id      uuid NOT NULL,
    PRIMARY KEY (id, valid_period WITHOUT OVERLAPS),
    CHECK (num_nonnulls(host_machine_id, host_cluster_id) <= 1),
    CHECK (kind IN ('virtual', 'container') OR num_nonnulls(host_machine_id, host_cluster_id) = 0),
    CHECK (kind = 'physical' OR member_cluster_id IS NULL),
    CHECK (host_machine_id IS DISTINCT FROM id)
);
CALL public.kartenn_make_versioned('machine');
CREATE INDEX machine_by_name ON machine (name);
CREATE INDEX machine_by_host ON machine (host_machine_id) WHERE host_machine_id IS NOT NULL;
CREATE INDEX machine_by_cluster ON machine (host_cluster_id) WHERE host_cluster_id IS NOT NULL;
CREATE INDEX machine_by_member ON machine (member_cluster_id) WHERE member_cluster_id IS NOT NULL;

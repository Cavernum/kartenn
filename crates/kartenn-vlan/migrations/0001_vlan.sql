-- The VLAN module (spec 008, REQ-008-12 to 16; design « Données du module »), in the
-- schema `vlan` the module migrator creates and puts first in the search path. Groups of
-- VLAN, numbering spaces anchored on an entity, a site or an addressing domain; VLANs in
-- a group. Both versioned (ADR 0017): the exclusions are the last line of defence, the
-- rules of the module give the messages.

-- `group` is a reserved word: the table of the groups is `vlan_group`.
CREATE TABLE vlan_group (
    id            uuid NOT NULL,
    slug          text NOT NULL CHECK (slug ~ '^[a-z0-9]([a-z0-9-]{0,61}[a-z0-9])?$'),
    anchor_kind   text NOT NULL CHECK (anchor_kind IN ('entity', 'site', 'domain')),
    anchor_id     uuid NOT NULL,
    vid_min       smallint NOT NULL DEFAULT 1,
    vid_max       smallint NOT NULL DEFAULT 4094,
    valid_period  tstzrange NOT NULL CHECK (NOT isempty(valid_period)),
    sys_start     timestamptz NOT NULL DEFAULT now(),
    changeset_id  uuid NOT NULL,
    PRIMARY KEY (id, valid_period WITHOUT OVERLAPS),
    CHECK (1 <= vid_min AND vid_min <= vid_max AND vid_max <= 4094),
    CONSTRAINT vlan_group_slug_unique EXCLUDE USING GIST (slug WITH =, valid_period WITH &&)
);
CALL public.kartenn_make_versioned('vlan_group');

CREATE TABLE vlan (
    id            uuid NOT NULL,
    group_id      uuid NOT NULL,
    vid           smallint NOT NULL CHECK (vid BETWEEN 1 AND 4094),
    name          text NOT NULL CHECK (char_length(name) BETWEEN 1 AND 64),
    status        text NOT NULL CHECK (status IN ('active', 'reserved', 'deprecated')),
    valid_period  tstzrange NOT NULL CHECK (NOT isempty(valid_period)),
    sys_start     timestamptz NOT NULL DEFAULT now(),
    changeset_id  uuid NOT NULL,
    PRIMARY KEY (id, valid_period WITHOUT OVERLAPS),
    CONSTRAINT vlan_vid_unique
        EXCLUDE USING GIST (group_id WITH =, vid WITH =, valid_period WITH &&),
    CONSTRAINT vlan_name_unique
        EXCLUDE USING GIST (group_id WITH =, name WITH =, valid_period WITH &&)
);
CALL public.kartenn_make_versioned('vlan');
CREATE INDEX vlan_by_group_vid ON vlan (group_id, vid);

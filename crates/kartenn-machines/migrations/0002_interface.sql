-- The interfaces of the machines (spec 009, REQ-009-6, 7, 12; design « Données »): each
-- tied to a fixed machine, its parent in the tree of the module, whose anchors it
-- inherits. Versioned (ADR 0017). The 802.1Q columns hold the mode of the interface
-- (M9); their rules come with the VLAN of an interface (task 009-4).

CREATE TABLE interface (
    id              uuid NOT NULL,
    machine_id      uuid NOT NULL,
    name            text NOT NULL CHECK (char_length(name) BETWEEN 1 AND 64),
    mac             macaddr,
    enabled         boolean NOT NULL DEFAULT true,
    vlan_mode       text NOT NULL DEFAULT 'none' CHECK (vlan_mode IN ('none', 'access', 'trunk')),
    access_vlan_id  uuid,
    native_vlan_id  uuid,
    tagged_vlan_ids uuid[] NOT NULL DEFAULT '{}',
    valid_period    tstzrange NOT NULL CHECK (NOT isempty(valid_period)),
    sys_start       timestamptz NOT NULL DEFAULT now(),
    changeset_id    uuid NOT NULL,
    PRIMARY KEY (id, valid_period WITHOUT OVERLAPS),
    CONSTRAINT interface_name_unique
        EXCLUDE USING GIST (machine_id WITH =, name WITH =, valid_period WITH &&),
    -- An EUI-48 of an interface is unicast: the group bit of its first octet is 0 (RFC 9542
    -- §2.1); the module checks it first.
    CHECK (mac IS NULL OR (mac & '01:00:00:00:00:00'::macaddr) = '00:00:00:00:00:00'::macaddr)
);
CALL public.kartenn_make_versioned('interface');
CREATE INDEX interface_by_machine ON interface (machine_id);
CREATE INDEX interface_by_mac ON interface (mac) WHERE mac IS NOT NULL;

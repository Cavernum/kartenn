-- The attachments of addresses of the core to interfaces (spec 009, REQ-009-9 to 11; design
-- « Données »): dated rows, not objects of the module, shown in the history of their
-- interface. An address is on one interface at a time unless its usage is shared (`vip`,
-- `anycast`), copied in `shared` and rewritten when the usage changes; at most one primary
-- address per family and interface. Versioned (ADR 0017). The module refuses first what
-- these constraints refuse: a failed statement would abort the changeset.

CREATE TABLE attachment (
    id                 uuid NOT NULL,
    interface_id       uuid NOT NULL,
    address_id         uuid NOT NULL,
    primary_for_family boolean NOT NULL DEFAULT false,
    family             smallint NOT NULL CHECK (family IN (4, 6)),
    shared             boolean NOT NULL,
    valid_period       tstzrange NOT NULL CHECK (NOT isempty(valid_period)),
    sys_start          timestamptz NOT NULL DEFAULT now(),
    changeset_id       uuid NOT NULL,
    PRIMARY KEY (id, valid_period WITHOUT OVERLAPS),
    CONSTRAINT attachment_once
        EXCLUDE USING GIST (interface_id WITH =, address_id WITH =, valid_period WITH &&),
    CONSTRAINT attachment_taken
        EXCLUDE USING GIST (address_id WITH =, valid_period WITH &&) WHERE (NOT shared),
    CONSTRAINT attachment_primary
        EXCLUDE USING GIST (interface_id WITH =, family WITH =, valid_period WITH &&)
        WHERE (primary_for_family)
);
CALL public.kartenn_make_versioned('attachment');
CREATE INDEX attachment_by_address ON attachment (address_id);
CREATE INDEX attachment_by_interface ON attachment (interface_id);

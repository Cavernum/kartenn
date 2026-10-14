-- Core objects of spec 002 (entities, sites, domains, prefixes, addresses), versioned
-- (ADR 0017). Exclusion constraints on inet/cidr name inet_ops explicitly: the default
-- btree_gist classes for these types are defective (ADR 0004, spike 001).
-- Ranges come in a later migration (design amendment of spike 001 pending).
-- Owner entity and site are core properties (ADR 0014), not columns.

CREATE TABLE entity (
    id            uuid NOT NULL,
    slug          text NOT NULL CHECK (slug ~ '^[a-z0-9]([a-z0-9-]{0,61}[a-z0-9])?$'),
    parent_id     uuid,
    valid_period  tstzrange NOT NULL CHECK (NOT isempty(valid_period)),
    sys_start     timestamptz NOT NULL DEFAULT now(),
    changeset_id  uuid NOT NULL,
    PRIMARY KEY (id, valid_period WITHOUT OVERLAPS),
    CONSTRAINT entity_slug_unique EXCLUDE USING GIST (slug WITH =, valid_period WITH &&),
    CHECK (parent_id IS DISTINCT FROM id)
);
CALL kartenn_make_versioned('entity');

CREATE TABLE site (
    id            uuid NOT NULL,
    slug          text NOT NULL CHECK (slug ~ '^[a-z0-9]([a-z0-9-]{0,61}[a-z0-9])?$'),
    parent_id     uuid,
    valid_period  tstzrange NOT NULL CHECK (NOT isempty(valid_period)),
    sys_start     timestamptz NOT NULL DEFAULT now(),
    changeset_id  uuid NOT NULL,
    PRIMARY KEY (id, valid_period WITHOUT OVERLAPS),
    CONSTRAINT site_slug_unique EXCLUDE USING GIST (slug WITH =, valid_period WITH &&),
    CHECK (parent_id IS DISTINCT FROM id)
);
CALL kartenn_make_versioned('site');

CREATE TABLE addressing_domain (
    id            uuid NOT NULL,
    slug          text NOT NULL CHECK (slug ~ '^[a-z0-9]([a-z0-9-]{0,61}[a-z0-9])?$'),
    valid_period  tstzrange NOT NULL CHECK (NOT isempty(valid_period)),
    sys_start     timestamptz NOT NULL DEFAULT now(),
    changeset_id  uuid NOT NULL,
    PRIMARY KEY (id, valid_period WITHOUT OVERLAPS),
    CONSTRAINT addressing_domain_slug_unique EXCLUDE USING GIST (slug WITH =, valid_period WITH &&)
);
CALL kartenn_make_versioned('addressing_domain');

CREATE TABLE prefix (
    id               uuid NOT NULL,
    domain_id        uuid NOT NULL,
    prefix           cidr NOT NULL,
    nature           text NOT NULL CHECK (nature IN ('container', 'network')),
    point_to_point   boolean NOT NULL DEFAULT false,
    public           boolean NOT NULL,
    registry_version text NOT NULL,
    valid_period     tstzrange NOT NULL CHECK (NOT isempty(valid_period)),
    sys_start        timestamptz NOT NULL DEFAULT now(),
    changeset_id     uuid NOT NULL,
    PRIMARY KEY (id, valid_period WITHOUT OVERLAPS),
    -- REQ-002-15; the same GiST index serves the deduced-parent lookup (spike 001).
    CONSTRAINT prefix_unique_per_domain EXCLUDE USING GIST (
        domain_id WITH =, prefix inet_ops WITH =, valid_period WITH &&),
    CONSTRAINT prefix_p2p_network CHECK (NOT point_to_point OR nature = 'network')
);
CALL kartenn_make_versioned('prefix');

CREATE TABLE address (
    id               uuid NOT NULL,
    domain_id        uuid NOT NULL,
    address          inet NOT NULL,
    usage            text NOT NULL CHECK (usage IN ('host', 'gateway', 'vip', 'anycast', 'reserved')),
    public           boolean NOT NULL,
    registry_version text NOT NULL,
    valid_period     tstzrange NOT NULL CHECK (NOT isempty(valid_period)),
    sys_start        timestamptz NOT NULL DEFAULT now(),
    changeset_id     uuid NOT NULL,
    PRIMARY KEY (id, valid_period WITHOUT OVERLAPS),
    CONSTRAINT address_is_host CHECK (masklen(address) = CASE family(address) WHEN 4 THEN 32 ELSE 128 END),
    -- REQ-002-26.
    CONSTRAINT address_unique_per_domain EXCLUDE USING GIST (
        domain_id WITH =, address inet_ops WITH =, valid_period WITH &&),
    -- REQ-002-32 last line of defence; anycast against a non-anycast occurrence is an
    -- application rule (core.public.overlap).
    CONSTRAINT address_public_unique EXCLUDE USING GIST (
        address inet_ops WITH =, valid_period WITH &&) WHERE (public AND usage <> 'anycast')
);
CALL kartenn_make_versioned('address');

-- External identifiers of connectors (REQ-002-41): one object per (source, external id).
CREATE TABLE external_id (
    source       text NOT NULL CHECK (source <> ''),
    external_id  text NOT NULL CHECK (external_id <> ''),
    object_id    uuid NOT NULL,
    object_type  text NOT NULL,
    PRIMARY KEY (source, external_id)
);
CREATE INDEX external_id_by_object ON external_id (object_id);
GRANT SELECT, INSERT, DELETE ON external_id TO kartenn_app;

-- Ranges of addresses inside a network (spec 002, REQ-002-21 to 24; 002/A12).
-- The range type has no subtype_diff: spike 001 measured a faster and safer GiST index
-- without it. inet orders IPv4 before IPv6, so a mixed-family range would be accepted:
-- a CHECK forbids it. Bounds are inclusive ('[]') host values.

CREATE TYPE public.inetrange AS RANGE (subtype = inet);

CREATE TABLE ip_range (
    id               uuid NOT NULL,
    domain_id        uuid NOT NULL,
    range            inetrange NOT NULL,
    usage            text NOT NULL CHECK (usage IN ('dhcp', 'reserved', 'static')),
    public           boolean NOT NULL,
    registry_version text NOT NULL,
    valid_period     tstzrange NOT NULL CHECK (NOT isempty(valid_period)),
    sys_start        timestamptz NOT NULL DEFAULT now(),
    changeset_id     uuid NOT NULL,
    PRIMARY KEY (id, valid_period WITHOUT OVERLAPS),
    CONSTRAINT ip_range_bounds CHECK (
        NOT isempty(range) AND lower_inc(range) AND upper_inc(range)
        AND family(lower(range)) = family(upper(range))
        AND masklen(lower(range)) = CASE family(lower(range)) WHEN 4 THEN 32 ELSE 128 END
        AND masklen(upper(range)) = CASE family(upper(range)) WHEN 4 THEN 32 ELSE 128 END),
    -- REQ-002-22: ranges of a domain (hence of a network) are disjoint at any time.
    CONSTRAINT ip_range_disjoint EXCLUDE USING GIST (
        domain_id WITH =, range WITH &&, valid_period WITH &&)
);
CALL kartenn_make_versioned('ip_range');

-- Spike 001 (a): temporal uniqueness of prefixes per domain with inet_ops.
DROP TABLE IF EXISTS prefix;
CREATE TABLE prefix (
    id            uuid PRIMARY KEY,
    domain_id     uuid NOT NULL,
    prefix        cidr NOT NULL,
    nature        text NOT NULL,
    valid_period  tstzrange NOT NULL DEFAULT tstzrange('2024-01-01', NULL),
    CONSTRAINT prefix_unique_per_domain EXCLUDE USING GIST (
        domain_id WITH =,              -- btree_gist, uuid
        prefix inet_ops WITH =,        -- built-in GiST class, never btree_gist for cidr
        valid_period WITH &&
    )
);

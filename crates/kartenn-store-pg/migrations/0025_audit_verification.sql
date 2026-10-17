-- Verification of the audit journal across retention (spec 012, REQ-012-14, 16; task
-- 012-10). A batch mixes criticalities and months, which are purged at different times:
-- each member keeps the fingerprint of its event, so that a batch stays verifiable after
-- the purge of some of its events; memberships are not purged. Each purge is recorded,
-- so that a missing event outside a purged partition is a deletion.
ALTER TABLE audit.sealed ADD COLUMN event_hash bytea NOT NULL CHECK (octet_length(event_hash) = 32);

CREATE TABLE audit.purge (
    criticality text NOT NULL CHECK (criticality IN ('low', 'normal', 'high')),
    month       date NOT NULL CHECK (month = date_trunc('month', month)),
    purged_at   timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (criticality, month)
);
REVOKE ALL ON audit.purge FROM PUBLIC;
GRANT SELECT ON audit.purge TO kartenn_app;

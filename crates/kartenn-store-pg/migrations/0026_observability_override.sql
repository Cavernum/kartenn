-- Filters of the technical outputs changed at run time (spec 012, REQ-012-27; task
-- 012-17): one override per output, until its expiry; every instance reads them and
-- returns to its configuration once expired. Written with its audit event.
CREATE TABLE observability_override (
    output     text PRIMARY KEY,
    filter     text NOT NULL CHECK (filter <> ''),
    expires_at timestamptz NOT NULL,
    set_by     uuid,
    set_at     timestamptz NOT NULL DEFAULT now()
);
GRANT SELECT, INSERT, UPDATE, DELETE ON observability_override TO kartenn_app;

-- Changesets, audit, idempotency, approval policies, outbox and imports (spec 004).

-- People referenced by the audit through a stable internal id; identity fields can be
-- pseudonymised without touching the history (REQ-004-33).
CREATE TABLE person (
    id                uuid PRIMARY KEY,
    display_name      text NOT NULL,
    email             text,
    issuer            text,
    subject           text,
    departed_at       timestamptz,
    pseudonymized_at  timestamptz,
    UNIQUE (issuer, subject)
);

CREATE TABLE changeset (
    id             uuid PRIMARY KEY,
    author_id      uuid NOT NULL REFERENCES person (id),
    validator_id   uuid REFERENCES person (id),
    motive         text NOT NULL CHECK (motive <> ''),
    state          text NOT NULL CHECK (state IN
                     ('applied', 'refused', 'pending', 'rejected', 'withdrawn', 'expired', 'failed_revalidation')),
    kind           text NOT NULL CHECK (kind IN ('direct', 'approval', 'proposal')),
    default_period tstzrange NOT NULL CHECK (NOT isempty(default_period)),
    retroactive    boolean NOT NULL DEFAULT false,
    submitted_at   timestamptz NOT NULL DEFAULT now(),
    applied_at     timestamptz,
    expires_at     timestamptz,
    policy_id      uuid,
    rejection      text,
    required_rights jsonb,
    violations     jsonb NOT NULL DEFAULT '[]',
    anomalies      jsonb NOT NULL DEFAULT '[]',
    diff_summary   jsonb NOT NULL DEFAULT '{}',
    CHECK (validator_id IS NULL OR validator_id <> author_id),
    CHECK (state <> 'pending' OR expires_at IS NOT NULL),
    CHECK (state <> 'rejected' OR rejection IS NOT NULL)
);
CREATE INDEX changeset_by_author ON changeset (author_id, submitted_at);
CREATE INDEX changeset_by_state ON changeset (state, submitted_at);
CREATE INDEX changeset_pending ON changeset (kind, expires_at) WHERE state = 'pending';
CREATE INDEX changeset_retroactive ON changeset (submitted_at) WHERE retroactive;

CREATE TABLE changeset_operation (
    changeset_id uuid NOT NULL REFERENCES changeset (id),
    ordinal      integer NOT NULL CHECK (ordinal >= 0),
    payload      jsonb NOT NULL,
    PRIMARY KEY (changeset_id, ordinal)
);

-- Detail of the differential of resolved property values (003/REQ-003-19).
CREATE TABLE changeset_diff (
    changeset_id uuid NOT NULL REFERENCES changeset (id),
    property_key text NOT NULL,
    object_id    uuid NOT NULL,
    before       jsonb,
    after        jsonb
);
CREATE INDEX changeset_diff_by_changeset ON changeset_diff (changeset_id, property_key);
CREATE INDEX changeset_diff_by_object ON changeset_diff (object_id);

-- Idempotent submissions (REQ-004-10): same identity and key within 24 h.
CREATE TABLE idempotency_key (
    identity_id  uuid NOT NULL REFERENCES person (id),
    key          text NOT NULL CHECK (key <> ''),
    request_hash bytea NOT NULL,
    changeset_id uuid NOT NULL REFERENCES changeset (id),
    expires_at   timestamptz NOT NULL,
    PRIMARY KEY (identity_id, key)
);

-- Opt-in approval policies (004/C6): none exists by default.
CREATE TABLE approval_policy (
    id               uuid PRIMARY KEY,
    name             text NOT NULL UNIQUE,
    min_operations   integer CHECK (min_operations > 0),
    min_diff_objects integer CHECK (min_diff_objects > 0),
    property_keys    text[] NOT NULL DEFAULT '{}',
    perimeters       jsonb NOT NULL DEFAULT '[]',
    authors          jsonb NOT NULL DEFAULT '[]',
    enabled          boolean NOT NULL DEFAULT true
);

-- Outbox (ADR 0018): written in the changeset transaction; the relay only reads rows of
-- finished transactions (xid below the snapshot xmin), in seq order.
CREATE TABLE outbox (
    seq          bigint GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    xid          xid8 NOT NULL DEFAULT pg_current_xact_id(),
    changeset_id uuid NOT NULL REFERENCES changeset (id),
    domain_ids   uuid[] NOT NULL DEFAULT '{}',
    payload      jsonb NOT NULL
);
CREATE TABLE outbox_cursor (
    consumer text PRIMARY KEY,
    last_seq bigint NOT NULL DEFAULT 0
);

-- Imports in batches (004/C3).
CREATE TABLE import (
    id         uuid PRIMARY KEY,
    author_id  uuid NOT NULL REFERENCES person (id),
    source     text NOT NULL,
    state      text NOT NULL CHECK (state IN ('queued', 'running', 'paused', 'done', 'cancelled')),
    created_at timestamptz NOT NULL DEFAULT now(),
    report     jsonb NOT NULL DEFAULT '{}'
);
CREATE TABLE import_batch (
    import_id    uuid NOT NULL REFERENCES import (id),
    idx          integer NOT NULL CHECK (idx >= 0),
    operations   jsonb NOT NULL,
    changeset_id uuid REFERENCES changeset (id),
    state        text NOT NULL CHECK (state IN ('queued', 'applied', 'failed')),
    PRIMARY KEY (import_id, idx)
);

-- The application role appends to the audit and moves states forward; it never deletes.
GRANT SELECT, INSERT ON person, changeset_operation, changeset_diff, outbox TO kartenn_app;
GRANT UPDATE (display_name, email, issuer, subject, departed_at, pseudonymized_at) ON person TO kartenn_app;
GRANT SELECT, INSERT ON changeset TO kartenn_app;
GRANT UPDATE (state, validator_id, applied_at, rejection, violations, anomalies, diff_summary) ON changeset TO kartenn_app;
GRANT SELECT, INSERT, DELETE ON idempotency_key TO kartenn_app;
GRANT SELECT, INSERT, UPDATE ON approval_policy, outbox_cursor, import, import_batch TO kartenn_app;

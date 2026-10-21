-- The flow registry module (spec 011, REQ-011-1 to 4, 16, 20, 23, 25; design « Données »),
-- in the schema `flow` the module migrator creates and puts first in the search path.
-- Versioned tables (ADR 0017); the evaluations are a derived state versioned in system
-- time, like `property_origin` (ADR 0016): an `as_of` in the past reads the history. The
-- exclusions and checks are the last line of defence, the rules of the module give the
-- messages.

-- A requested flow; its endpoints are polymorphic (address, prefix, interface, machine),
-- checked by the module (`flow.endpoint.invalid`), so without foreign key.
CREATE TABLE flow (
    id                uuid NOT NULL,
    kind              text NOT NULL CHECK (kind IN ('requested', 'observed')),
    source_kind       text NOT NULL CHECK (source_kind IN ('address', 'prefix', 'interface', 'machine')),
    source_id         uuid NOT NULL,
    destination_kind  text NOT NULL CHECK (destination_kind IN ('address', 'prefix', 'interface', 'machine')),
    destination_id    uuid NOT NULL,
    service_id        uuid NOT NULL,
    -- At least one reference: [{"system": uuid, "id": text, "label": text?}].
    external_refs     jsonb NOT NULL CHECK (jsonb_typeof(external_refs) = 'array'
                                            AND jsonb_array_length(external_refs) >= 1),
    justification     text CHECK (char_length(justification) BETWEEN 1 AND 4000),
    requester_id      uuid NOT NULL,
    -- {"external": {...}} or {"text": "…"} (N26, N31).
    on_behalf_of      jsonb CHECK (jsonb_typeof(on_behalf_of) = 'object'),
    responsibles      jsonb NOT NULL DEFAULT '[]' CHECK (jsonb_typeof(responsibles) = 'array'),
    data_nature       uuid[] NOT NULL DEFAULT '{}',
    personal_data     boolean NOT NULL DEFAULT false,
    entity_id         uuid NOT NULL,
    reviewed_by       uuid,
    valid_period      tstzrange NOT NULL CHECK (NOT isempty(valid_period)),
    sys_start         timestamptz NOT NULL DEFAULT now(),
    changeset_id      uuid NOT NULL,
    PRIMARY KEY (id, valid_period WITHOUT OVERLAPS)
);
CALL public.kartenn_make_versioned('flow');
CREATE INDEX flow_by_source ON flow (source_id);
CREATE INDEX flow_by_destination ON flow (destination_id);
CREATE INDEX flow_by_service ON flow (service_id);
CREATE INDEX flow_by_requester ON flow (requester_id);
CREATE INDEX flow_by_entity ON flow (entity_id);

-- A waiver of a denied flow (REQ-011-9): its period is its validity, the end mandatory;
-- one waiver per flow at a time.
CREATE TABLE flow_waiver (
    flow_id        uuid NOT NULL,
    reason         text NOT NULL CHECK (char_length(btrim(reason)) BETWEEN 1 AND 4000),
    accepted_risk  text NOT NULL CHECK (char_length(btrim(accepted_risk)) BETWEEN 1 AND 4000),
    granted_by     uuid NOT NULL,
    valid_period   tstzrange NOT NULL CHECK (NOT isempty(valid_period) AND NOT upper_inf(valid_period)),
    sys_start      timestamptz NOT NULL DEFAULT now(),
    changeset_id   uuid NOT NULL,
    PRIMARY KEY (flow_id, valid_period WITHOUT OVERLAPS)
);
CALL public.kartenn_make_versioned('flow_waiver', ARRAY['flow_id']);

-- The addresses of the endpoints, expanded, by segment of period (kept with the
-- attachments of spec 009). A prefix endpoint is an « address » here: the node itself.
CREATE TABLE flow_endpoint_address (
    flow_id       uuid NOT NULL,
    side          text NOT NULL CHECK (side IN ('source', 'destination')),
    address_id    uuid NOT NULL,
    valid_period  tstzrange NOT NULL CHECK (NOT isempty(valid_period)),
    sys_start     timestamptz NOT NULL DEFAULT now(),
    changeset_id  uuid NOT NULL,
    PRIMARY KEY (flow_id, side, address_id, valid_period WITHOUT OVERLAPS)
);
CALL public.kartenn_make_versioned('flow_endpoint_address', ARRAY['flow_id', 'side', 'address_id']);
CREATE INDEX flow_endpoint_by_address ON flow_endpoint_address (address_id);

-- The evaluation of each pair of addresses for the flow's service (spec 010), by segment.
CREATE TABLE flow_evaluation (
    flow_id                 uuid NOT NULL,
    source_address_id       uuid NOT NULL,
    destination_address_id  uuid NOT NULL,
    verdict                 text NOT NULL CHECK (verdict IN ('allowed', 'denied')),
    -- Allowed under reserve: a conditional authorisation without its pattern (REQ-010-32).
    pending                 boolean NOT NULL DEFAULT false,
    outcome                 text NOT NULL CHECK (outcome IN ('automatic', 'review', 'noncompliant')),
    reason                  text NOT NULL,
    decisive_id             uuid,
    combination             jsonb NOT NULL DEFAULT '{}',
    unmet_conditions        uuid[] NOT NULL DEFAULT '{}',
    score                   integer,
    policy_revision         bigint NOT NULL,
    valid_period            tstzrange NOT NULL CHECK (NOT isempty(valid_period)),
    sys_start               timestamptz NOT NULL DEFAULT now(),
    changeset_id            uuid NOT NULL,
    PRIMARY KEY (flow_id, source_address_id, destination_address_id, valid_period WITHOUT OVERLAPS),
    CHECK (NOT pending OR verdict = 'allowed')
);
CALL public.kartenn_make_versioned('flow_evaluation',
     ARRAY['flow_id', 'source_address_id', 'destination_address_id']);

-- The summary of a flow's evaluations (REQ-010-15), by segment.
CREATE TABLE flow_summary (
    flow_id       uuid NOT NULL,
    summary       text NOT NULL CHECK (summary IN ('allowed', 'partial', 'denied')),
    outcome       text NOT NULL CHECK (outcome IN ('automatic', 'review', 'noncompliant')),
    allowed       integer NOT NULL CHECK (allowed >= 0),
    denied        integer NOT NULL CHECK (denied >= 0),
    valid_period  tstzrange NOT NULL CHECK (NOT isempty(valid_period)),
    sys_start     timestamptz NOT NULL DEFAULT now(),
    changeset_id  uuid NOT NULL,
    PRIMARY KEY (flow_id, valid_period WITHOUT OVERLAPS),
    CHECK ((summary = 'allowed') = (allowed > 0 AND denied = 0)),
    CHECK ((summary = 'denied') = (allowed = 0))
);
CALL public.kartenn_make_versioned('flow_summary', ARRAY['flow_id']);
CREATE INDEX flow_summary_by_summary ON flow_summary (summary);
CREATE INDEX flow_summary_by_outcome ON flow_summary (outcome);

-- The evaluation at registration, frozen (REQ-011-23): never rewritten.
CREATE TABLE flow_registration (
    flow_id          uuid PRIMARY KEY,
    summary          text NOT NULL CHECK (summary IN ('allowed', 'partial', 'denied')),
    outcome          text NOT NULL CHECK (outcome IN ('automatic', 'review', 'noncompliant')),
    decisive_id      uuid,
    policy_revision  bigint NOT NULL,
    reviewed_by      uuid,
    registered_at    timestamptz NOT NULL DEFAULT now(),
    changeset_id     uuid NOT NULL
);

-- A request rejected at review (REQ-011-22, 25): a non-conformity of its requester.
CREATE TABLE flow_rejection (
    changeset_id   uuid PRIMARY KEY,
    request        jsonb NOT NULL CHECK (jsonb_typeof(request) = 'object'),
    evaluation     jsonb NOT NULL CHECK (jsonb_typeof(evaluation) = 'object'),
    reason         text NOT NULL CHECK (char_length(btrim(reason)) BETWEEN 1 AND 4000),
    rule_id        uuid,
    reviewed_by    uuid NOT NULL,
    requester_id   uuid NOT NULL,
    entity_id      uuid NOT NULL,
    rejected_at    timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX flow_rejection_by_requester ON flow_rejection (requester_id);
CREATE INDEX flow_rejection_by_entity ON flow_rejection (entity_id);

-- Deadlines already notified (REQ-011-7): one notice per flow, kind and end.
CREATE TABLE flow_notice (
    flow_id      uuid NOT NULL,
    kind         text NOT NULL CHECK (kind IN ('expiring_30', 'expiring_7', 'expired',
                                               'waiver_expiring_30', 'waiver_expiring_7',
                                               'waiver_expired')),
    period_end   timestamptz NOT NULL,
    notified_at  timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (flow_id, kind, period_end)
);

-- The non-versioned tables are written by the module in the transaction of a changeset.
GRANT SELECT, INSERT ON flow_registration, flow_rejection, flow_notice TO kartenn_app;

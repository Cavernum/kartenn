-- The flow policy module (spec 010, REQ-010-1 to 10, 26, 29 to 37; design « Données »), in
-- the schema `policy` the module migrator creates and puts first in the search path. Every
-- object of the policy is versioned (ADR 0017) and written by changeset (S16): the
-- constraints are the last line of defence, the rules of the module give the messages.
--
-- Axes, values and the values cited by rules are named by their keys (the key of a
-- property and of a closed-list value never change, REQ-003-5); flow types, services,
-- patterns and rules are cited by identifier, so that a label or a key may change.

-- REQ-010-29: a property designated as an axis of the policy, with its carrier.
CREATE TABLE policy_axis (
    id            uuid NOT NULL,
    property_key  text NOT NULL CHECK (property_key ~ '^[a-z][a-z0-9_]*\.[a-z][a-z0-9_]*$'),
    carrier       text NOT NULL CHECK (carrier IN ('tree', 'machine')),
    inference     boolean NOT NULL DEFAULT false,
    valid_period  tstzrange NOT NULL CHECK (NOT isempty(valid_period)),
    sys_start     timestamptz NOT NULL DEFAULT now(),
    changeset_id  uuid NOT NULL,
    PRIMARY KEY (id, valid_period WITHOUT OVERLAPS),
    CONSTRAINT policy_axis_unique EXCLUDE USING GIST (property_key WITH =, valid_period WITH &&)
);
CALL public.kartenn_make_versioned('policy_axis');

-- REQ-010-2, 30: the attributes of a value of an axis (`policy.environment` among them,
-- S2), never read by a verdict.
CREATE TABLE policy_value (
    id            uuid NOT NULL,
    property_key  text NOT NULL CHECK (property_key ~ '^[a-z][a-z0-9_]*\.[a-z][a-z0-9_]*$'),
    value_key     text NOT NULL CHECK (value_key ~ '^[a-z0-9]([a-z0-9-]*[a-z0-9])?$'),
    label         text,
    description   text,
    transverse    boolean NOT NULL DEFAULT false,
    rank          integer,
    valid_period  tstzrange NOT NULL CHECK (NOT isempty(valid_period)),
    sys_start     timestamptz NOT NULL DEFAULT now(),
    changeset_id  uuid NOT NULL,
    PRIMARY KEY (id, valid_period WITHOUT OVERLAPS),
    CONSTRAINT policy_value_unique
        EXCLUDE USING GIST (property_key WITH =, value_key WITH =, valid_period WITH &&)
);
CALL public.kartenn_make_versioned('policy_value');

-- REQ-010-4.
CREATE TABLE policy_flow_type (
    id            uuid NOT NULL,
    key           text NOT NULL CHECK (key ~ '^[a-z0-9]([a-z0-9-]{0,61}[a-z0-9])?$'),
    label         text NOT NULL,
    description   text,
    valid_period  tstzrange NOT NULL CHECK (NOT isempty(valid_period)),
    sys_start     timestamptz NOT NULL DEFAULT now(),
    changeset_id  uuid NOT NULL,
    PRIMARY KEY (id, valid_period WITHOUT OVERLAPS),
    CONSTRAINT policy_flow_type_key_unique EXCLUDE USING GIST (key WITH =, valid_period WITH &&)
);
CALL public.kartenn_make_versioned('policy_flow_type');

-- REQ-010-5: a service and its entries `{transport, ports | icmp_type, icmp_code |
-- protocol}`, kept with the service so that a version of a service is one row; the index
-- of the ports is the snapshot's (design « Instantané »).
CREATE TABLE policy_service (
    id            uuid NOT NULL,
    key           text NOT NULL CHECK (key ~ '^[a-z0-9]([a-z0-9-]{0,61}[a-z0-9])?$'),
    label         text NOT NULL,
    flow_type_id  uuid NOT NULL,
    entries       jsonb NOT NULL CHECK (jsonb_typeof(entries) = 'array'),
    valid_period  tstzrange NOT NULL CHECK (NOT isempty(valid_period)),
    sys_start     timestamptz NOT NULL DEFAULT now(),
    changeset_id  uuid NOT NULL,
    PRIMARY KEY (id, valid_period WITHOUT OVERLAPS),
    CONSTRAINT policy_service_key_unique EXCLUDE USING GIST (key WITH =, valid_period WITH &&)
);
CALL public.kartenn_make_versioned('policy_service');
CREATE INDEX policy_service_by_flow_type ON policy_service (flow_type_id);

-- REQ-010-9, S24.
CREATE TABLE policy_pattern (
    id            uuid NOT NULL,
    key           text NOT NULL CHECK (key ~ '^[a-z0-9]([a-z0-9-]{0,61}[a-z0-9])?$'),
    label         text NOT NULL,
    texts         jsonb NOT NULL DEFAULT '{}' CHECK (jsonb_typeof(texts) = 'object'),
    valid_period  tstzrange NOT NULL CHECK (NOT isempty(valid_period)),
    sys_start     timestamptz NOT NULL DEFAULT now(),
    changeset_id  uuid NOT NULL,
    PRIMARY KEY (id, valid_period WITHOUT OVERLAPS),
    CONSTRAINT policy_pattern_key_unique EXCLUDE USING GIST (key WITH =, valid_period WITH &&)
);
CALL public.kartenn_make_versioned('policy_pattern');

-- REQ-010-8, S23 to S25: the terms of the selectors as `{property key: [value keys]}`,
-- the axes of « same value » as property keys, the texts as `{locale: {body, url}}` (S29).
CREATE TABLE policy_rule (
    id                    uuid NOT NULL,
    key                   text NOT NULL CHECK (key ~ '^[a-z0-9]([a-z0-9-]{0,61}[a-z0-9])?$'),
    label                 text NOT NULL,
    effect                text NOT NULL CHECK (effect IN ('allow', 'deny')),
    source                jsonb NOT NULL DEFAULT '{}' CHECK (jsonb_typeof(source) = 'object'),
    destination           jsonb NOT NULL DEFAULT '{}' CHECK (jsonb_typeof(destination) = 'object'),
    same                  text[] NOT NULL DEFAULT '{}',
    flow_type_ids         uuid[] NOT NULL DEFAULT '{}',
    service_ids           uuid[] NOT NULL DEFAULT '{}',
    pattern_id            uuid,
    required_pattern_ids  uuid[] NOT NULL DEFAULT '{}',
    texts                 jsonb NOT NULL DEFAULT '{}' CHECK (jsonb_typeof(texts) = 'object'),
    valid_period          tstzrange NOT NULL CHECK (NOT isempty(valid_period)),
    sys_start             timestamptz NOT NULL DEFAULT now(),
    changeset_id          uuid NOT NULL,
    PRIMARY KEY (id, valid_period WITHOUT OVERLAPS),
    CONSTRAINT policy_rule_key_unique EXCLUDE USING GIST (key WITH =, valid_period WITH &&),
    CONSTRAINT policy_rule_flow_selector
        CHECK (cardinality(flow_type_ids) + cardinality(service_ids) > 0),
    CONSTRAINT policy_rule_conditional_deny
        CHECK (effect = 'allow' OR cardinality(required_pattern_ids) = 0)
);
CALL public.kartenn_make_versioned('policy_rule');
CREATE INDEX policy_rule_by_service ON policy_rule USING GIN (service_ids);
CREATE INDEX policy_rule_by_flow_type ON policy_rule USING GIN (flow_type_ids);

-- REQ-010-36, S30: the kind among the closed list of the code, its parameters checked by
-- the kind (`policy.condition.params_invalid`).
CREATE TABLE policy_condition (
    id             uuid NOT NULL,
    key            text NOT NULL CHECK (key ~ '^[a-z0-9]([a-z0-9-]{0,61}[a-z0-9])?$'),
    kind           text NOT NULL,
    params         jsonb NOT NULL DEFAULT '{}' CHECK (jsonb_typeof(params) = 'object'),
    rule_ids       uuid[] NOT NULL DEFAULT '{}',
    flow_type_ids  uuid[] NOT NULL DEFAULT '{}',
    texts          jsonb NOT NULL DEFAULT '{}' CHECK (jsonb_typeof(texts) = 'object'),
    valid_period   tstzrange NOT NULL CHECK (NOT isempty(valid_period)),
    sys_start      timestamptz NOT NULL DEFAULT now(),
    changeset_id   uuid NOT NULL,
    PRIMARY KEY (id, valid_period WITHOUT OVERLAPS),
    CONSTRAINT policy_condition_key_unique EXCLUDE USING GIST (key WITH =, valid_period WITH &&)
);
CALL public.kartenn_make_versioned('policy_condition');

-- REQ-010-37, S32.
CREATE TABLE policy_signal (
    id            uuid NOT NULL,
    key           text NOT NULL CHECK (key ~ '^[a-z0-9]([a-z0-9-]{0,61}[a-z0-9])?$'),
    kind          text NOT NULL,
    params        jsonb NOT NULL DEFAULT '{}' CHECK (jsonb_typeof(params) = 'object'),
    weight        integer NOT NULL,
    valid_period  tstzrange NOT NULL CHECK (NOT isempty(valid_period)),
    sys_start     timestamptz NOT NULL DEFAULT now(),
    changeset_id  uuid NOT NULL,
    PRIMARY KEY (id, valid_period WITHOUT OVERLAPS),
    CONSTRAINT policy_signal_key_unique EXCLUDE USING GIST (key WITH =, valid_period WITH &&)
);
CALL public.kartenn_make_versioned('policy_signal');

-- S10: a role of interface (009) whose machines are exempt from the check of their
-- declared environment.
CREATE TABLE policy_exempt_role (
    id            uuid NOT NULL,
    role_key      text NOT NULL CHECK (role_key ~ '^[a-z0-9]([a-z0-9-]*[a-z0-9])?$'),
    valid_period  tstzrange NOT NULL CHECK (NOT isempty(valid_period)),
    sys_start     timestamptz NOT NULL DEFAULT now(),
    changeset_id  uuid NOT NULL,
    PRIMARY KEY (id, valid_period WITHOUT OVERLAPS),
    CONSTRAINT policy_exempt_role_unique EXCLUDE USING GIST (role_key WITH =, valid_period WITH &&)
);
CALL public.kartenn_make_versioned('policy_exempt_role');

-- REQ-010-40 to 54 (section G): a rule of inference, an exception when it has a parent;
-- `domain_id` carried for the rights. Its operations come with the inference (010-14, 15).
CREATE TABLE policy_inference_rule (
    id            uuid NOT NULL,
    key           text NOT NULL CHECK (key ~ '^[a-z0-9]([a-z0-9-]{0,61}[a-z0-9])?$'),
    axis_key      text NOT NULL CHECK (axis_key ~ '^[a-z][a-z0-9_]*\.[a-z][a-z0-9_]*$'),
    parent_id     uuid,
    priority      integer NOT NULL DEFAULT 0,
    criteria      jsonb NOT NULL DEFAULT '[]' CHECK (jsonb_typeof(criteria) = 'array'),
    value_keys    text[] NOT NULL CHECK (cardinality(value_keys) > 0),
    domain_id     uuid,
    valid_period  tstzrange NOT NULL CHECK (NOT isempty(valid_period)),
    sys_start     timestamptz NOT NULL DEFAULT now(),
    changeset_id  uuid NOT NULL,
    PRIMARY KEY (id, valid_period WITHOUT OVERLAPS),
    CONSTRAINT policy_inference_rule_key_unique
        EXCLUDE USING GIST (key WITH =, valid_period WITH &&)
);
CALL public.kartenn_make_versioned('policy_inference_rule');

-- REQ-010-26, S17: the revisions of the policy and of the inference, one row, raised
-- once by each changeset that writes them (the key of the snapshot cache).
CREATE TABLE policy_revision (
    singleton       boolean PRIMARY KEY DEFAULT true CHECK (singleton),
    policy          bigint NOT NULL DEFAULT 0,
    inference       bigint NOT NULL DEFAULT 0,
    policy_by       uuid,
    inference_by    uuid
);
INSERT INTO policy_revision DEFAULT VALUES;
GRANT SELECT, UPDATE ON policy_revision TO kartenn_app;

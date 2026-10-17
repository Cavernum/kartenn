-- Authentication and RBAC (spec 005, task 005-3; REQ-005-3, 11, 14, 18, 20; ADR 0021).

-- Identities: people (issuer, subject), service accounts, break-glass accounts, and the
-- system author. The state (disabled, sessions revoked after) is changed by changesets.
ALTER TABLE person
    ADD COLUMN kind text NOT NULL DEFAULT 'person'
        CHECK (kind IN ('person', 'service_account', 'breakglass', 'system')),
    ADD COLUMN disabled_at timestamptz,
    ADD COLUMN revoked_after timestamptz;
UPDATE person SET kind = 'system' WHERE id = '00000000-0000-7000-8000-000000000001';
GRANT UPDATE (disabled_at, revoked_after) ON person TO kartenn_app;

-- Roles and their entries (catalogue permissions, `prop.*.<action>`, `*`), versioned.
CREATE TABLE role (
    id            uuid NOT NULL,
    key           text NOT NULL CHECK (key ~ '^[a-z][a-z0-9_]*$'),
    label         text NOT NULL CHECK (label <> ''),
    builtin       boolean NOT NULL DEFAULT false,
    valid_period  tstzrange NOT NULL CHECK (NOT isempty(valid_period)),
    sys_start     timestamptz NOT NULL DEFAULT now(),
    changeset_id  uuid NOT NULL,
    PRIMARY KEY (id, valid_period WITHOUT OVERLAPS),
    CONSTRAINT role_key_unique EXCLUDE USING GIST (key WITH =, valid_period WITH &&)
);
CALL kartenn_make_versioned('role');

CREATE TABLE role_permission (
    role_id       uuid NOT NULL,
    permission    text NOT NULL CHECK (permission ~ '^(\*|[a-z][a-z0-9_]*\.([a-z0-9_.*]+)\.[a-z_]+)$'),
    valid_period  tstzrange NOT NULL CHECK (NOT isempty(valid_period)),
    sys_start     timestamptz NOT NULL DEFAULT now(),
    changeset_id  uuid NOT NULL,
    PRIMARY KEY (role_id, permission, valid_period WITHOUT OVERLAPS)
);
CALL kartenn_make_versioned('role_permission', ARRAY['role_id', 'permission']);

-- Bindings: a subject (an identity, a group of an issuer, a whole issuer) holds a role on
-- a scope (global, entity, site, domain, prefix subtree), over its valid period.
CREATE TABLE binding (
    id            uuid NOT NULL,
    subject_kind  text NOT NULL CHECK (subject_kind IN ('identity', 'group', 'issuer')),
    subject_id    uuid,
    issuer        text,
    group_name    text,
    role_id       uuid NOT NULL,
    scope_kind    text NOT NULL CHECK (scope_kind IN ('global', 'entity', 'site', 'domain', 'prefix')),
    scope_id      uuid,
    valid_period  tstzrange NOT NULL CHECK (NOT isempty(valid_period)),
    sys_start     timestamptz NOT NULL DEFAULT now(),
    changeset_id  uuid NOT NULL,
    PRIMARY KEY (id, valid_period WITHOUT OVERLAPS),
    CONSTRAINT binding_subject CHECK (CASE subject_kind
        WHEN 'identity' THEN subject_id IS NOT NULL AND issuer IS NULL AND group_name IS NULL
        WHEN 'group' THEN subject_id IS NULL AND issuer IS NOT NULL AND group_name IS NOT NULL
        ELSE subject_id IS NULL AND issuer IS NOT NULL AND group_name IS NULL END),
    CONSTRAINT binding_scope CHECK ((scope_kind = 'global') = (scope_id IS NULL))
);
CALL kartenn_make_versioned('binding');
-- The bindings of a subject, found from each of its three forms.
CREATE INDEX binding_by_identity ON binding (subject_id) WHERE subject_kind = 'identity';
CREATE INDEX binding_by_issuer ON binding (issuer, group_name) WHERE subject_kind <> 'identity';

-- What an issuer may reach (K4, K13): entities (their subtrees), or everything (null).
CREATE TABLE issuer_attachment (
    issuer        text NOT NULL CHECK (issuer <> ''),
    entity_id     uuid,
    valid_period  tstzrange NOT NULL CHECK (NOT isempty(valid_period)),
    sys_start     timestamptz NOT NULL DEFAULT now(),
    changeset_id  uuid NOT NULL,
    -- A global attachment and the entity attachments of an issuer coexist; each is unique.
    CONSTRAINT issuer_attachment_unique EXCLUDE USING GIST (
        issuer WITH =, (coalesce(entity_id, '00000000-0000-0000-0000-000000000000'::uuid)) WITH =,
        valid_period WITH &&)
);
CALL kartenn_make_versioned('issuer_attachment', ARRAY['issuer']);

-- Secrets of the service accounts (K8): an HMAC of a generated secret, never the secret;
-- two active at most per account (rotation).
CREATE TABLE service_secret (
    id          uuid PRIMARY KEY,
    account_id  uuid NOT NULL REFERENCES person (id),
    digest      bytea NOT NULL CHECK (octet_length(digest) = 32),
    key_id      text NOT NULL,
    created_at  timestamptz NOT NULL DEFAULT now(),
    expires_at  timestamptz,
    revoked_at  timestamptz
);
CREATE INDEX service_secret_by_account ON service_secret (account_id) WHERE revoked_at IS NULL;
CREATE FUNCTION public.kartenn_service_secret_limit() RETURNS trigger
LANGUAGE plpgsql SET search_path = pg_catalog, public AS $$
BEGIN
    IF (SELECT count(*) FROM service_secret
        WHERE account_id = NEW.account_id AND revoked_at IS NULL
          AND (expires_at IS NULL OR expires_at > now())) > 2 THEN
        RAISE 'a service account holds two active secrets at most'
            USING ERRCODE = 'check_violation';
    END IF;
    RETURN NULL;
END $$;
CREATE CONSTRAINT TRIGGER service_secret_limit AFTER INSERT ON service_secret
    FOR EACH ROW EXECUTE FUNCTION public.kartenn_service_secret_limit();
GRANT SELECT, INSERT ON service_secret TO kartenn_app;
GRANT UPDATE (revoked_at) ON service_secret TO kartenn_app;

-- Break-glass accounts (K9): written by the owner of the schema (subcommand), never by
-- the application, which only records the last TOTP step and the failures.
CREATE TABLE breakglass_account (
    account_id      uuid PRIMARY KEY REFERENCES person (id),
    password_hash   text NOT NULL CHECK (password_hash LIKE '$argon2id$%'),
    totp_secret     bytea NOT NULL,
    totp_key_id     text NOT NULL,
    last_totp_step  bigint NOT NULL DEFAULT 0,
    failures        integer NOT NULL DEFAULT 0 CHECK (failures >= 0),
    valid_until     timestamptz NOT NULL
);
REVOKE ALL ON breakglass_account FROM PUBLIC;
GRANT SELECT ON breakglass_account TO kartenn_app;
GRANT UPDATE (last_totp_step, failures) ON breakglass_account TO kartenn_app;

-- The owning entity of an object at `at` (ADR 0014, 0021): an entity owns itself; an
-- addressing object has its resolved `core.owner`; a site, none.
CREATE FUNCTION public.kartenn_owner(object uuid, at timestamptz) RETURNS uuid
LANGUAGE sql STABLE PARALLEL SAFE SET search_path = pg_catalog, public AS $$
    SELECT CASE
        WHEN EXISTS (SELECT 1 FROM entity e WHERE e.id = object AND e.valid_period @> at)
            THEN object
        ELSE (SELECT a.v_uuid
              FROM property_declaration d
              JOIN property_origin o ON o.property_id = d.id AND o.object_id = object
                                    AND o.valid_period @> at
              JOIN property_assignment a ON a.object_id = o.origin_object_id
                                        AND a.property_id = d.id AND a.valid_period @> at
                                        AND a.kind = 'value'
              WHERE d.key = 'core.owner' AND d.valid_period @> at
              LIMIT 1)
    END
$$;

-- The site of an object at `at`: a site is its own; a network has its `core.site`; a range
-- or an address, the one of the network that holds it; the others, none.
CREATE FUNCTION public.kartenn_site(object uuid, at timestamptz) RETURNS uuid
LANGUAGE sql STABLE PARALLEL SAFE SET search_path = pg_catalog, public AS $$
    WITH o AS (SELECT kind, domain_id, value FROM addressing_object
               WHERE id = object AND valid_period @> at),
         network AS (
            SELECT p.id FROM prefix p, o
            WHERE p.domain_id = o.domain_id AND p.nature = 'network' AND p.valid_period @> at
              AND o.kind IN ('prefix', 'range', 'address') AND p.prefix >>= o.value
            LIMIT 1)
    SELECT CASE
        WHEN EXISTS (SELECT 1 FROM site s WHERE s.id = object AND s.valid_period @> at)
            THEN object
        ELSE (SELECT a.v_uuid
              FROM network n
              JOIN property_declaration d ON d.key = 'core.site' AND d.valid_period @> at
              JOIN property_assignment a ON a.object_id = n.id AND a.property_id = d.id
                                        AND a.valid_period @> at AND a.kind = 'value'
              LIMIT 1)
    END
$$;
GRANT EXECUTE ON FUNCTION public.kartenn_owner(uuid, timestamptz) TO kartenn_app;
GRANT EXECUTE ON FUNCTION public.kartenn_site(uuid, timestamptz) TO kartenn_app;

-- Rights by groups (spec 025, REQ-025-1, 2, 4; task 025-1): a binding serves a group kept
-- by kartenn, whose members are identities and groups of an issuer; never an identity nor
-- a whole issuer (G1, G3).

-- Groups, versioned like the roles (G2). `group` is a reserved word.
CREATE TABLE rbac_group (
    id            uuid NOT NULL,
    key           text NOT NULL CHECK (key ~ '^[a-z][a-z0-9_-]*$' AND char_length(key) <= 128),
    labels        jsonb NOT NULL DEFAULT '{}' CHECK (jsonb_typeof(labels) = 'object'),
    builtin       boolean NOT NULL DEFAULT false,
    valid_period  tstzrange NOT NULL CHECK (NOT isempty(valid_period)),
    sys_start     timestamptz NOT NULL DEFAULT now(),
    changeset_id  uuid NOT NULL,
    PRIMARY KEY (id, valid_period WITHOUT OVERLAPS),
    CONSTRAINT rbac_group_key_unique EXCLUDE USING GIST (key WITH =, valid_period WITH &&)
);
CALL kartenn_make_versioned('rbac_group');

-- Members of a group (G2, G6): an identity, or a group of an issuer as its tokens name it;
-- never another group of kartenn. A member appears once in a group at any time.
CREATE TABLE group_member (
    id            uuid NOT NULL,
    group_id      uuid NOT NULL,
    member_kind   text NOT NULL CHECK (member_kind IN ('identity', 'idp_group')),
    -- An identity by its identifier, as `binding.subject_id` was: no foreign key.
    identity_id   uuid,
    issuer        text,
    group_name    text,
    valid_period  tstzrange NOT NULL CHECK (NOT isempty(valid_period)),
    sys_start     timestamptz NOT NULL DEFAULT now(),
    changeset_id  uuid NOT NULL,
    PRIMARY KEY (id, valid_period WITHOUT OVERLAPS),
    CONSTRAINT group_member_kind CHECK (CASE member_kind
        WHEN 'identity' THEN identity_id IS NOT NULL AND issuer IS NULL AND group_name IS NULL
        ELSE identity_id IS NULL AND coalesce(issuer, '') <> ''
             AND coalesce(group_name, '') <> '' END),
    CONSTRAINT group_member_unique EXCLUDE USING GIST (
        group_id WITH =, member_kind WITH =,
        (coalesce(identity_id, '00000000-0000-0000-0000-000000000000'::uuid)) WITH =,
        (coalesce(issuer, '')) WITH =, (coalesce(group_name, '')) WITH =,
        valid_period WITH &&)
);
CALL kartenn_make_versioned('group_member');
-- The groups of a subject, found from each of its two forms.
CREATE INDEX group_member_by_identity ON group_member (identity_id)
    WHERE member_kind = 'identity';
CREATE INDEX group_member_by_idp_group ON group_member (issuer, group_name)
    WHERE member_kind = 'idp_group';

-- A binding serves a group (by its logical key, like `role_id`). The former subject
-- columns stay, nullable, for the rows written before: they grant nothing any more, and
-- the migration job reports them on groups then drops them (task 025-4). A new row names
-- its group alone.
CALL kartenn_add_versioned_column('binding', 'group_id uuid');
ALTER TABLE binding ALTER COLUMN subject_kind DROP NOT NULL;
ALTER TABLE binding_history ALTER COLUMN subject_kind DROP NOT NULL;
ALTER TABLE binding DROP CONSTRAINT binding_subject;
ALTER TABLE binding ADD CONSTRAINT binding_subject CHECK (CASE
    WHEN group_id IS NOT NULL
        THEN subject_kind IS NULL AND subject_id IS NULL AND issuer IS NULL AND group_name IS NULL
    WHEN subject_kind = 'identity'
        THEN subject_id IS NOT NULL AND issuer IS NULL AND group_name IS NULL
    WHEN subject_kind = 'group'
        THEN subject_id IS NULL AND issuer IS NOT NULL AND group_name IS NOT NULL
    WHEN subject_kind = 'issuer'
        THEN subject_id IS NULL AND issuer IS NOT NULL AND group_name IS NULL
    ELSE false END);
CREATE INDEX binding_by_group ON binding (group_id);

-- The rights of a subject are read as asserted before the writes of the current
-- transaction (REQ-025-7, G5): a changeset that adds its author to a group, or binds a
-- group of its author, gives the author nothing within itself. Outside a transaction that
-- writes, these views are the tables: the rows the transaction moved to the history end
-- at its instant (`sys_end = now()`), its new rows start at it.
CREATE VIEW binding_asserted WITH (security_invoker = true) AS
    SELECT id, group_id, role_id, scope_kind, scope_id, valid_period
    FROM binding WHERE sys_start < now()
    UNION ALL
    SELECT id, group_id, role_id, scope_kind, scope_id, valid_period
    FROM binding_history WHERE sys_start < now() AND sys_end = now();
CREATE VIEW role_asserted WITH (security_invoker = true) AS
    SELECT id, key, valid_period FROM role WHERE sys_start < now()
    UNION ALL
    SELECT id, key, valid_period FROM role_history WHERE sys_start < now() AND sys_end = now();
CREATE VIEW role_permission_asserted WITH (security_invoker = true) AS
    SELECT role_id, permission, valid_period FROM role_permission WHERE sys_start < now()
    UNION ALL
    SELECT role_id, permission, valid_period FROM role_permission_history
    WHERE sys_start < now() AND sys_end = now();
CREATE VIEW rbac_group_asserted WITH (security_invoker = true) AS
    SELECT id, key, builtin, valid_period FROM rbac_group WHERE sys_start < now()
    UNION ALL
    SELECT id, key, builtin, valid_period FROM rbac_group_history
    WHERE sys_start < now() AND sys_end = now();
CREATE VIEW group_member_asserted WITH (security_invoker = true) AS
    SELECT id, group_id, member_kind, identity_id, issuer, group_name, valid_period
    FROM group_member WHERE sys_start < now()
    UNION ALL
    SELECT id, group_id, member_kind, identity_id, issuer, group_name, valid_period
    FROM group_member_history WHERE sys_start < now() AND sys_end = now();
GRANT SELECT ON binding_asserted, role_asserted, role_permission_asserted, rbac_group_asserted,
    group_member_asserted TO kartenn_app;

-- The groups of kartenn a subject belongs to at `at` (REQ-025-4, 5): directly, or by one
-- of the groups `groups` of its issuer `issuer` (those of its session, or of the snapshot
-- of a pending changeset), in groups in force then; as asserted before the writes of the
-- current transaction.
CREATE FUNCTION public.kartenn_member_of(who uuid, who_issuer text, who_groups text[], at timestamptz)
RETURNS SETOF uuid
LANGUAGE sql STABLE PARALLEL SAFE SET search_path = pg_catalog, public AS $$
    SELECT m.group_id FROM group_member_asserted m
    JOIN rbac_group_asserted g ON g.id = m.group_id AND g.valid_period @> at
    WHERE m.valid_period @> at
      AND ((m.member_kind = 'identity' AND m.identity_id = who)
           OR (m.member_kind = 'idp_group' AND m.issuer = who_issuer
               AND m.group_name = ANY(who_groups)))
$$;
GRANT EXECUTE ON FUNCTION public.kartenn_member_of(uuid, text, text[], timestamptz)
    TO kartenn_app;

-- The bindings of the former subjects reported on groups of kartenn (spec 025, REQ-025-14,
-- G9; task 025-4), then the former subject columns dropped from `binding`.
--
-- Done here rather than by the migration job after the migrations: the columns can only
-- go once every row is reported, and the job runs after every pending migration. The
-- report is a changeset of the system author written in SQL, as `breakglass::create`
-- writes its own; the migration job lists it and audits it (`rbac.migration.reported`)
-- from `rbac_migration_report`.
--
-- - A binding in force or to come of a group of an issuer: on the group
--   `idp-<issuer>-<group>`, whose only member is that group of the issuer (one per couple).
-- - Of an identity: on the group `compte-<name of the identity>`, its only member.
--   Converted in place: same identifier, same period; the former version stays in the
--   history with its former columns.
-- - Of a whole issuer: listed, then out of the table (its version in force ends now).
-- - A binding closed before (never converted): out of the table, kept in the history.
-- Groups and memberships start with the earliest of the bindings they carry. A key is the
-- name lowered, every other run of characters as `-`; one too long, empty or already taken
-- ends with 8 hexadecimal digits of the md5 of its source.

CREATE TABLE rbac_migration_report (
    seq           bigint GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    changeset_id  uuid NOT NULL REFERENCES changeset (id),
    -- `group`: a group created, with its former subject; `closed`: a binding of a whole
    -- issuer closed.
    kind          text NOT NULL CHECK (kind IN ('group', 'closed')),
    group_id      uuid,
    group_key     text,
    -- The former subject: `identity` (`identity_id`), `group` (`issuer`, `group_name`) or
    -- `issuer` (`issuer`).
    subject_kind  text NOT NULL CHECK (subject_kind IN ('identity', 'group', 'issuer')),
    identity_id   uuid,
    issuer        text,
    group_name    text,
    -- `group`: the bindings reported on it; `closed`: the binding.
    bindings      integer,
    binding_id    uuid,
    role_key      text,
    scope_kind    text,
    scope_id      uuid,
    -- When the migration job listed it.
    reported_at   timestamptz,
    CHECK ((kind = 'group') = (group_id IS NOT NULL AND group_key IS NOT NULL))
);
REVOKE ALL ON rbac_migration_report FROM PUBLIC;
-- The undo gives a binding of a former subject back on its group.
GRANT SELECT ON rbac_migration_report TO kartenn_app;

CREATE FUNCTION pg_temp.key_part(raw text) RETURNS text
LANGUAGE sql IMMUTABLE AS $$
    SELECT trim(BOTH '-' FROM regexp_replace(lower(coalesce(raw, '')), '[^a-z0-9_]+', '-', 'g'))
$$;

CREATE TEMP TABLE old_binding ON COMMIT DROP AS
    SELECT b.id, b.subject_kind, b.subject_id, b.issuer, b.group_name, b.role_id,
           b.scope_kind, b.scope_id, b.valid_period
    FROM binding b
    WHERE b.group_id IS NULL AND (upper_inf(b.valid_period) OR upper(b.valid_period) > now());

DO $$
DECLARE
    v_report uuid := uuidv7();
    v_since timestamptz;
BEGIN
    IF NOT EXISTS (SELECT FROM old_binding) THEN
        RETURN;
    END IF;
    SELECT min(coalesce(lower(valid_period), now())) INTO v_since
    FROM old_binding WHERE subject_kind <> 'issuer';
    v_since := least(coalesce(v_since, now()), now());
    INSERT INTO changeset (id, author_id, motive, state, kind, default_period, retroactive,
                           applied_at)
    VALUES (v_report, '00000000-0000-7000-8000-000000000001', 'bindings reported on groups 025',
            'applied', 'direct', tstzrange(v_since, NULL), v_since < now(), now());

    -- One group per former subject that is not a whole issuer.
    CREATE TEMP TABLE new_group ON COMMIT DROP AS
        SELECT uuidv7() AS id, s.*, NULL::text AS key
        FROM (SELECT o.subject_kind, o.subject_id, o.issuer, o.group_name,
                     min(coalesce(lower(o.valid_period), now())) AS since,
                     count(*)::integer AS bindings,
                     CASE o.subject_kind
                         WHEN 'identity' THEN 'compte-' || pg_temp.key_part(
                             (SELECT p.display_name FROM person p WHERE p.id = o.subject_id))
                         ELSE 'idp-' || pg_temp.key_part(o.issuer) || '-'
                              || pg_temp.key_part(o.group_name) END AS wanted,
                     md5(coalesce(o.subject_id::text, o.issuer || ' ' || o.group_name)) AS seed
              FROM old_binding o WHERE o.subject_kind <> 'issuer'
              GROUP BY o.subject_kind, o.subject_id, o.issuer, o.group_name) s;
    UPDATE new_group n SET key = CASE
        WHEN char_length(n.wanted) <= 128
             AND n.wanted ~ '^[a-z][a-z0-9_-]*[a-z0-9_]$'
             AND (SELECT count(*) FROM new_group o WHERE o.wanted = n.wanted) = 1
             AND NOT EXISTS (SELECT FROM rbac_group g WHERE g.key = n.wanted
                             AND g.valid_period && tstzrange(n.since, NULL))
            THEN n.wanted
        ELSE left(rtrim(n.wanted, '-'), 119) || '-' || left(n.seed, 8) END;

    INSERT INTO rbac_group (id, key, valid_period, changeset_id)
        SELECT id, key, tstzrange(since, NULL), v_report FROM new_group;
    INSERT INTO group_member (id, group_id, member_kind, identity_id, issuer, group_name,
                              valid_period, changeset_id)
        SELECT uuidv7(), id, CASE subject_kind WHEN 'identity' THEN 'identity' ELSE 'idp_group' END,
               subject_id, issuer, group_name, tstzrange(since, NULL), v_report
        FROM new_group;
    UPDATE binding b
       SET group_id = n.id, subject_kind = NULL, subject_id = NULL, issuer = NULL,
           group_name = NULL, changeset_id = v_report
      FROM old_binding o
      JOIN new_group n ON n.subject_kind = o.subject_kind
                      AND n.subject_id IS NOT DISTINCT FROM o.subject_id
                      AND n.issuer IS NOT DISTINCT FROM o.issuer
                      AND n.group_name IS NOT DISTINCT FROM o.group_name
     WHERE b.id = o.id AND b.valid_period = o.valid_period;
    INSERT INTO rbac_migration_report (changeset_id, kind, group_id, group_key, subject_kind,
                                       identity_id, issuer, group_name, bindings)
        SELECT v_report, 'group', id, key, subject_kind, subject_id, issuer, group_name, bindings
        FROM new_group ORDER BY key;

    -- The bindings of a whole issuer: listed, then out of the table below (a binding names
    -- a group from now on). Their version in force ends in the history at this instant; a
    -- closure written here would not be kept there (a version replaced in its own
    -- transaction is not, 0018), so the report and its audit are their trace.
    INSERT INTO rbac_migration_report (changeset_id, kind, subject_kind, issuer, binding_id,
                                       role_key, scope_kind, scope_id)
        SELECT v_report, 'closed', 'issuer', o.issuer, o.id,
               (SELECT r.key FROM role r WHERE r.id = o.role_id
                UNION ALL SELECT r.key FROM role_history r WHERE r.id = o.role_id LIMIT 1),
               o.scope_kind, o.scope_id
        FROM old_binding o WHERE o.subject_kind = 'issuer' ORDER BY o.issuer, o.id;
END $$;

-- What remains of the former form: the bindings of a whole issuer just listed, and those
-- closed before.
DELETE FROM binding WHERE group_id IS NULL;

-- The former subject columns leave the table; its history keeps them (the former versions
-- stay readable as they were written).
DROP INDEX binding_by_identity;
DROP INDEX binding_by_issuer;
ALTER TABLE binding DROP CONSTRAINT binding_subject;
ALTER TABLE binding DROP COLUMN subject_kind, DROP COLUMN subject_id, DROP COLUMN issuer,
    DROP COLUMN group_name;
ALTER TABLE binding ALTER COLUMN group_id SET NOT NULL;
-- The trigger copies the columns of the table by name: its list loses the former ones.
CALL kartenn_versioning_trigger('binding');

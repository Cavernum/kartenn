-- Two paths of rights (spec 024, REQ-024-9 to 13; task 024-1, 2): every change is a
-- changeset, submitted pending under `<resource>.edit`, applied under `<resource>.apply`
-- (someone else's) or `<resource>.apply_own` (one's own). The roles take the new names by
-- the system changeset of the migration job (`rbac::rename_permissions`), as for spec 018;
-- the constraint of `role_permission.permission` already admits them.
--
-- The author may now be the validator of their changeset: applied in the request that
-- submits it (`?apply=own`), or validated later under `apply_own`.
ALTER TABLE changeset DROP CONSTRAINT changeset_check;

-- The pending changesets journaled before name the write permissions their author lacked:
-- their edit permissions now, whose application their validator needs. Those held by a
-- policy name none; the validation checks them anew (REQ-004-22).
UPDATE changeset
SET required_rights = jsonb_set(
        required_rights,
        '{required}',
        (SELECT coalesce(jsonb_agg(jsonb_build_array(
                    regexp_replace(e ->> 0, '\.write$', '.edit'), e -> 1)), '[]'::jsonb)
         FROM jsonb_array_elements(required_rights -> 'required') AS e))
WHERE state = 'pending' AND jsonb_typeof(required_rights -> 'required') = 'array';

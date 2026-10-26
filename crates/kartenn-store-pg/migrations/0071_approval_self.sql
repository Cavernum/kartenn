-- The application of one's own changeset is the action `approval.self` (task 024-11,
-- fifth review): `<resource>.approval.self`, formerly `<resource>.apply_own`; the request
-- field is `approval_self` and the code of the lack `changeset.approval.self.missing`.
-- The custom roles take the new names by the system changeset of the migration job
-- (`rbac::rename_permissions`), as for specs 018 and 024; the delivered roles by their
-- synchronisation. The constraint of `role_permission.permission` already admits an
-- action of two segments.
--
-- The journal of the changesets keeps what a replay and the validation read back: the
-- reason that kept a changeset pending (its code, the permissions its message names), the
-- requirements and the violations recorded, and the request replayed at the validation.
UPDATE changeset
SET required_rights = replace(
        replace(
            replace(required_rights::text, 'changeset.apply_own.missing',
                    'changeset.approval.self.missing'),
            '.apply_own', '.approval.self'),
        'lacks apply_own', 'lacks approval.self')::jsonb
WHERE required_rights::text LIKE '%apply\_own%';

UPDATE changeset
SET violations = replace(violations::text, 'changeset.apply_own.missing',
                         'changeset.approval.self.missing')::jsonb
WHERE violations::text LIKE '%changeset.apply\_own.missing%';

UPDATE changeset
SET request = (request - 'apply_own')
              || jsonb_build_object('approval_self', request -> 'apply_own')
WHERE request ? 'apply_own';

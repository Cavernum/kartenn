-- Writing on behalf of a requester (spec 016, REQ-016-12, 13; G10; task 016-12): the
-- identities `requester` (no secret, no binding, never logged in), the requester a
-- changeset was written for, and the columns the application may now change.

ALTER TABLE person DROP CONSTRAINT person_kind_check;
ALTER TABLE person ADD CONSTRAINT person_kind_check
    CHECK (kind IN ('person', 'service_account', 'breakglass', 'system', 'requester'));
-- The application never changes the kind of an identity (rbac_schema): a requester who
-- later logs in through its issuer keeps its row and its kind.

ALTER TABLE changeset ADD COLUMN on_behalf_of uuid REFERENCES person (id);

-- The validation of a pending changeset records the operations of review its validator
-- joined (REQ-016-11, task 016-11).
GRANT UPDATE (request) ON changeset TO kartenn_app;

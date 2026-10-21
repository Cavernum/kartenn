-- REQ-010-48, I9, Q13: the progress of the background re-evaluation of the deviations of
-- the inference (task `policy.inference_check`), one row. A pass starts when the revision
-- of the rules of inference moves past `checked`; it covers the origin nodes of the
-- domains the rules written since `since` cite (all of them when one of those rules cites
-- no domain), by batches, and records `checked` and `since` once over.
CREATE TABLE policy_inference_check (
    singleton    boolean PRIMARY KEY DEFAULT true CHECK (singleton),
    checked      bigint NOT NULL DEFAULT 0,
    since        timestamptz NOT NULL DEFAULT '1970-01-01T00:00:00Z',
    pass         bigint,
    pass_since   timestamptz,
    scope        uuid[],
    origin       uuid,
    after_origin uuid,
    after_heir   uuid,
    deviating    bigint NOT NULL DEFAULT 0
);
INSERT INTO policy_inference_check DEFAULT VALUES;
GRANT SELECT, UPDATE ON policy_inference_check TO kartenn_app;

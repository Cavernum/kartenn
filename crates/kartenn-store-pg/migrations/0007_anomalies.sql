-- Anomalies: warnings recorded against objects (REQ-002-35, 36). An anomaly is open while
-- its rule fires on the object; it can be acknowledged with a motive; a later change of the
-- object (another changeset) makes an acknowledged anomaly visible again.
CREATE TABLE anomaly (
    id               uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    rule_id          text NOT NULL,
    object_id        uuid NOT NULL,
    message          text NOT NULL,
    opened_at        timestamptz NOT NULL DEFAULT now(),
    opened_by        uuid NOT NULL REFERENCES changeset (id),
    closed_at        timestamptz,
    acknowledged_by  uuid REFERENCES person (id),
    acknowledged_at  timestamptz,
    motive           text CHECK (motive IS NULL OR motive <> ''),
    acked_changeset  uuid,
    CHECK ((acknowledged_by IS NULL) = (motive IS NULL))
);
CREATE UNIQUE INDEX anomaly_one_open ON anomaly (rule_id, object_id) WHERE closed_at IS NULL;
CREATE INDEX anomaly_by_object ON anomaly (object_id) WHERE closed_at IS NULL;
GRANT SELECT, INSERT ON anomaly TO kartenn_app;
GRANT UPDATE (message, closed_at, acknowledged_by, acknowledged_at, motive, acked_changeset) ON anomaly TO kartenn_app;

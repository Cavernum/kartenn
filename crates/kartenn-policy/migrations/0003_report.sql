-- REQ-010-22, 23: the audit report, asked for a date (`valid_at`) and produced by the
-- background task `policy.report` by batches, one report at a time. Each rubric is a list
-- of items numbered from 1; `counts` holds the number of items of each rubric (null for a
-- rubric the build cannot answer, the flows of the 011 absent). A report is purged 30 days
-- after it was asked for (design « Rapport d'audit »).
CREATE TABLE policy_report (
    id            uuid PRIMARY KEY,
    valid_at      timestamptz NOT NULL,
    requested_by  uuid,
    requested_at  timestamptz NOT NULL DEFAULT now(),
    state         text NOT NULL DEFAULT 'pending' CHECK (state IN ('pending', 'running', 'done')),
    done_at       timestamptz,
    counts        jsonb NOT NULL DEFAULT '{}' CHECK (jsonb_typeof(counts) = 'object'),
    -- Progress: the rubric being produced, the part within it (axis, table) and the last
    -- key read.
    rubric        integer NOT NULL DEFAULT 0,
    part          integer NOT NULL DEFAULT 0,
    after_key     uuid,
    CHECK ((state = 'done') = (done_at IS NOT NULL))
);
-- One report at a time (design « Rapport d'audit »).
CREATE UNIQUE INDEX policy_report_one_running ON policy_report ((true)) WHERE state <> 'done';
CREATE INDEX policy_report_by_date ON policy_report (requested_at);

CREATE TABLE policy_report_item (
    report_id  uuid NOT NULL REFERENCES policy_report (id) ON DELETE CASCADE,
    rubric     text NOT NULL,
    seq        bigint NOT NULL CHECK (seq > 0),
    subject    uuid,
    detail     jsonb NOT NULL DEFAULT '{}' CHECK (jsonb_typeof(detail) = 'object'),
    PRIMARY KEY (report_id, rubric, seq)
);

GRANT SELECT, INSERT, UPDATE, DELETE ON policy_report, policy_report_item TO kartenn_app;

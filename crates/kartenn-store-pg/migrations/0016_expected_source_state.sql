-- Version of each source of expected values whose deviations were last re-evaluated
-- (spec 003, REQ-003-24; task 003-15). A source whose version differs is re-evaluated.
CREATE TABLE expected_source_state (
    source       text PRIMARY KEY,
    version      text NOT NULL,
    evaluated_at timestamptz NOT NULL DEFAULT now()
);
GRANT SELECT, INSERT, UPDATE ON expected_source_state TO kartenn_app;

-- A pending changeset keeps its request, replayed as submitted when it is validated
-- (spec 004, REQ-004-22, 42; tasks 004-12 to 14): operations, motive and the period as
-- given (none means the time of the validation, as for any submission).
ALTER TABLE changeset ADD COLUMN request jsonb;
ALTER TABLE changeset ADD CONSTRAINT changeset_pending_request
    CHECK (state <> 'pending' OR request IS NOT NULL);
CREATE INDEX changeset_pending_proposals ON changeset (submitted_at)
    WHERE state = 'pending' AND kind = 'proposal';

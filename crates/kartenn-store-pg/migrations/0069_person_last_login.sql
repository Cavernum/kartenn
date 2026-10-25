-- The groups of its issuer the last session of a person carried, and its opening (spec
-- 025, REQ-025-11, Q-025-1; task 025-5): the record of an identity shows them to explain
-- a membership by a group of the issuer. The session itself is a sealed cookie (ADR 0020)
-- the server does not keep; these columns are written at each login.

ALTER TABLE person ADD COLUMN last_login_at timestamptz;
ALTER TABLE person ADD COLUMN last_login_groups text[];
GRANT UPDATE (last_login_at, last_login_groups) ON person TO kartenn_app;

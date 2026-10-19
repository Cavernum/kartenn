-- The background tasks (spec 006, REQ-006-38; task 006-20): one row per periodic task,
-- run by one instance at a time under a lease taken in the database; a holder that
-- stops leaves its lease to expire, and the task runs elsewhere.
CREATE TABLE job (
    name         text PRIMARY KEY CHECK (name <> ''),
    due_at       timestamptz NOT NULL,
    period       interval NOT NULL CHECK (period > interval '0'),
    lease_until  timestamptz,
    holder       uuid,
    heartbeat_at timestamptz,
    CHECK ((lease_until IS NULL) = (holder IS NULL))
);
CREATE INDEX job_due ON job (due_at);

INSERT INTO job (name, due_at, period) VALUES
    ('expire-pending', now(), interval '1 minute'),
    ('reevaluate-expected', now(), interval '5 minutes'),
    ('public-drift', now(), interval '1 day'),
    ('idempotency-purge', now(), interval '1 hour');

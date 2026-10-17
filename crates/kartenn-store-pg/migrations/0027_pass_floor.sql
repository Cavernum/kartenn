-- Floors of the passes that hand each row once, in sequence order (spec 012, REQ-012-15;
-- ADR 0018, 0019): sealing (`audit.seal`), each audit output (`audit.relay/<name>`), each
-- outbox consumer (`outbox/<name>`). A pass reads the rows it has not handled from its
-- floor up, instead of the whole history; every row below the floor is handled.
--
-- The floor rises in two steps, so that a running transaction holding a lower number is
-- never skipped: a pass notes the highest visible number (`pending_max`) and the
-- `pg_snapshot_xmax` of its snapshot (`pending_xmax`); once `pg_snapshot_xmin` has passed
-- it, every transaction that could hold a number up to `pending_max` has ended, and the
-- floor rises to the first number up to it not handled yet. A long transaction anywhere
-- delays the rise, never a delivery. Once an hour (`swept_at`) a pass reads from zero, so
-- that a row missed by the floor (a number drawn before its transaction had an id) is
-- handled late, never lost.
CREATE TABLE pass_floor (
    pass         text PRIMARY KEY,
    floor        bigint NOT NULL DEFAULT 0,
    pending_max  bigint,
    pending_xmax xid8,
    swept_at     timestamptz NOT NULL DEFAULT now()
);
GRANT SELECT, INSERT, UPDATE ON pass_floor TO kartenn_app;

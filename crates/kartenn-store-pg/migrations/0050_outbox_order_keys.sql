-- Ordering keys of the outbox (spec 016, REQ-016-14; ADR 0018, amendment 4): the events
-- of changesets that share a key are delivered in their order of application. The keys of
-- a changeset are its addressing domains (`domain:<uuid>`) and the lock keys of its module
-- operations (ADR 0026, clause 14), which serialise those changesets as a domain lock
-- does. `domain_ids` stays, read by the consumers of v0.1, until a later removal.

ALTER TABLE outbox ADD COLUMN order_keys text[] NOT NULL DEFAULT '{}';

-- The existing events, by batches of 10 000 so that no single statement rewrites the
-- whole table at once.
DO $$
DECLARE
    low bigint := (SELECT min(seq) FROM outbox);
    high bigint := (SELECT max(seq) FROM outbox);
BEGIN
    WHILE low IS NOT NULL AND low <= high LOOP
        UPDATE outbox
           SET order_keys = ARRAY(SELECT 'domain:' || d FROM unnest(domain_ids) d)
         WHERE seq >= low AND seq < low + 10000 AND cardinality(domain_ids) > 0;
        low := low + 10000;
    END LOOP;
END
$$;

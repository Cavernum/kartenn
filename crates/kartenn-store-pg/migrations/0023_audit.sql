-- Audit journal (spec 012, REQ-012-11 to 16; ADR 0019; task 012-4). Events, the
-- membership of each event in a sealed batch, the seals, and the deliveries per output.
-- The application role may only insert and read: no update, no delete, no truncate.
-- Events, memberships and deliveries are partitioned by criticality, then by month of
-- recording, so that the retention of a criticality drops whole partitions (012/L22).

CREATE SCHEMA audit;
REVOKE ALL ON SCHEMA audit FROM PUBLIC;
GRANT USAGE ON SCHEMA audit TO kartenn_app;

CREATE TABLE audit.event (
    seq             bigint GENERATED ALWAYS AS IDENTITY,
    id              uuid NOT NULL,
    name            text NOT NULL,
    severity        text NOT NULL CHECK (severity IN ('TRACE', 'DEBUG', 'INFO', 'WARN', 'ERROR')),
    criticality     text NOT NULL CHECK (criticality IN ('low', 'normal', 'high')),
    occurred_at     timestamptz NOT NULL,
    recorded_at     timestamptz NOT NULL DEFAULT now(),
    actor_kind      text NOT NULL CHECK (actor_kind IN ('person', 'service_account', 'system', 'unknown')),
    actor_id        uuid,
    auth_method     text,
    issuer          text,
    claimed         text,
    source_ip       inet,
    user_agent      text CHECK (octet_length(user_agent) <= 256),
    target_kind     text,
    target_ids      uuid[],
    outcome         text NOT NULL CHECK (outcome IN ('success', 'failure', 'denied')),
    reason          text,
    motive          text,
    changeset_id    uuid,
    trace_id        text,
    span_id         text,
    request_id      text,
    service_version text NOT NULL,
    instance_id     uuid NOT NULL,
    details         jsonb NOT NULL DEFAULT '{}',
    -- REQ-012-9: every event is attributed; an unknown actor carries the fingerprint of
    -- the identifier it claimed.
    CONSTRAINT event_attributed CHECK (
        CASE actor_kind
            WHEN 'person' THEN actor_id IS NOT NULL
            WHEN 'service_account' THEN actor_id IS NOT NULL
            WHEN 'unknown' THEN claimed IS NOT NULL
            ELSE true
        END),
    PRIMARY KEY (criticality, recorded_at, seq)
) PARTITION BY LIST (criticality);

-- Membership of an event in a seal: an event is sealed once.
CREATE TABLE audit.sealed (
    seal_seq    bigint NOT NULL,
    event_seq   bigint NOT NULL,
    criticality text NOT NULL,
    recorded_at timestamptz NOT NULL,
    PRIMARY KEY (criticality, recorded_at, event_seq)
) PARTITION BY LIST (criticality);

-- Delivery of an event to an output of the audit (sink).
CREATE TABLE audit.delivery (
    sink         text NOT NULL,
    event_seq    bigint NOT NULL,
    criticality  text NOT NULL,
    recorded_at  timestamptz NOT NULL,
    delivered_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (criticality, recorded_at, sink, event_seq)
) PARTITION BY LIST (criticality);

-- Seals, chained: hash = SHA-256(prev_hash ‖ count ‖ event hashes), computed by
-- kartenn_core::audit::jcs::batch_hash. Kept after the purge of their events.
CREATE TABLE audit.seal (
    seq        bigint GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    id         uuid NOT NULL UNIQUE,
    created_at timestamptz NOT NULL DEFAULT now(),
    prev_hash  bytea NOT NULL CHECK (octet_length(prev_hash) = 32),
    hash       bytea NOT NULL CHECK (octet_length(hash) = 32),
    count      integer NOT NULL CHECK (count > 0),
    signature  bytea,
    key_id     text,
    CHECK ((signature IS NULL) = (key_id IS NULL))
);

-- Key of the HMAC of claimed identifiers (REQ-012-9), the same on every instance: 244
-- random bits from two version-4 UUIDs (pg_strong_random), unless a key file is
-- configured.
CREATE TABLE audit.instance_key (
    id       smallint PRIMARY KEY CHECK (id = 1),
    hmac_key bytea NOT NULL CHECK (octet_length(hmac_key) = 32)
);
INSERT INTO audit.instance_key
    VALUES (1, decode(replace(gen_random_uuid()::text || gen_random_uuid()::text, '-', ''), 'hex'));

-- One partition per criticality, each partitioned by month with a default partition.
DO $$
DECLARE
    t text;
    c text;
BEGIN
    FOREACH t IN ARRAY ARRAY['event', 'sealed', 'delivery'] LOOP
        FOREACH c IN ARRAY ARRAY['low', 'normal', 'high'] LOOP
            EXECUTE format('CREATE TABLE audit.%I PARTITION OF audit.%I FOR VALUES IN (%L) PARTITION BY RANGE (recorded_at)',
                           t || '_' || c, t, c);
            EXECUTE format('CREATE TABLE audit.%I PARTITION OF audit.%I DEFAULT',
                           t || '_' || c || '_default', t || '_' || c);
        END LOOP;
    END LOOP;
END $$;

CREATE INDEX event_by_seq ON audit.event (seq);
CREATE INDEX event_by_actor ON audit.event (actor_id, recorded_at);
CREATE INDEX event_by_name ON audit.event (name, recorded_at);
CREATE INDEX event_by_target ON audit.event USING gin (target_ids);
CREATE INDEX sealed_by_event ON audit.sealed (event_seq);
CREATE INDEX delivery_by_event ON audit.delivery (sink, event_seq);

REVOKE ALL ON ALL TABLES IN SCHEMA audit FROM PUBLIC;
GRANT SELECT, INSERT ON audit.event, audit.sealed, audit.delivery, audit.seal TO kartenn_app;
GRANT SELECT ON audit.instance_key TO kartenn_app;

-- Monthly partitions ahead of time, as for the histories (004-25): a month already holding
-- rows in the default partition is left there.
CREATE PROCEDURE public.kartenn_audit_partitions(from_month date, months integer)
LANGUAGE plpgsql SET search_path = pg_catalog, public AS $$
DECLARE
    t text;
    c text;
    parent text;
    m date;
    occupied boolean;
BEGIN
    FOREACH t IN ARRAY ARRAY['event', 'sealed', 'delivery'] LOOP
        FOREACH c IN ARRAY ARRAY['low', 'normal', 'high'] LOOP
            parent := t || '_' || c;
            m := date_trunc('month', from_month)::date;
            FOR i IN 0 .. months - 1 LOOP
                IF to_regclass(format('audit.%I', parent || '_' || to_char(m, 'YYYYMM'))) IS NULL THEN
                    EXECUTE format('SELECT EXISTS (SELECT 1 FROM audit.%I WHERE recorded_at >= %L AND recorded_at < %L)',
                                   parent || '_default', m, (m + interval '1 month')::date)
                        INTO occupied;
                    IF NOT occupied THEN
                        EXECUTE format('CREATE TABLE audit.%I PARTITION OF audit.%I FOR VALUES FROM (%L) TO (%L)',
                                       parent || '_' || to_char(m, 'YYYYMM'), parent, m,
                                       (m + interval '1 month')::date);
                    END IF;
                END IF;
                m := (m + interval '1 month')::date;
            END LOOP;
        END LOOP;
    END LOOP;
END $$;
REVOKE ALL ON PROCEDURE public.kartenn_audit_partitions(date, integer) FROM PUBLIC;

-- The maintenance job also keeps the audit partitions ahead.
CREATE OR REPLACE PROCEDURE public.kartenn_history_maintenance(months integer)
LANGUAGE plpgsql SET search_path = pg_catalog, public AS $$
DECLARE
    t regclass;
BEGIN
    FOR t IN
        SELECT c.oid::regclass FROM pg_class c JOIN pg_namespace n ON n.oid = c.relnamespace
        WHERE n.nspname = 'public' AND c.relkind = 'r'
          AND EXISTS (SELECT FROM pg_class h WHERE h.relnamespace = c.relnamespace
                                             AND h.relname = c.relname || '_history' AND h.relkind = 'p')
        ORDER BY c.relname
    LOOP
        CALL public.kartenn_history_partitions(t, now()::date, months);
    END LOOP;
    CALL public.kartenn_audit_partitions(now()::date, months);
END $$;

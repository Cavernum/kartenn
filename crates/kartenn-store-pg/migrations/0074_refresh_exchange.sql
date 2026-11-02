-- The exchanges of refresh tokens shared by the instances (spec 021, A24, A25; D9): the
-- claim of an instance that presents a token to its issuer, under a lease, then the result
-- of its exchange, kept a short window for the requests that left with the token it
-- turned. A row is named by a fingerprint of the token (SHA-256, `kartenn-auth`), never by
-- the token; the result is sealed by a key derived from the key ring, bound to that
-- fingerprint. A failure is not kept. The rows past their end are purged by the task
-- `refresh-exchange-purge`, and read as absent meanwhile.
CREATE TABLE refresh_exchange (
    key     bytea PRIMARY KEY CHECK (octet_length(key) = 32),
    -- The instance exchanging the token, while no result is kept.
    holder  uuid,
    -- The result, sealed (AES-256-GCM), once kept.
    sealed  text,
    -- The end of the lease of `holder`, or of the window of `sealed`.
    ends_at timestamptz NOT NULL,
    CHECK ((holder IS NULL) <> (sealed IS NULL))
);

GRANT SELECT, INSERT, UPDATE, DELETE ON refresh_exchange TO kartenn_app;

-- The purge of the exchanges past their end, every minute.
INSERT INTO job (name, due_at, period)
VALUES ('refresh-exchange-purge', now(), interval '1 minute');

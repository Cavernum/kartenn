-- ADR 0018 (amended): a single last-delivered cursor can skip an event whose transaction
-- was still running when a later sequence number was delivered. Deliveries are recorded
-- per consumer instead; the relay delivers undelivered events of finished transactions in
-- sequence order. Within a domain the order is the application order (domain lock).
DROP TABLE outbox_cursor;
CREATE TABLE outbox_delivery (
    consumer      text NOT NULL,
    seq           bigint NOT NULL REFERENCES outbox (seq),
    delivered_at  timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (consumer, seq)
);
GRANT SELECT, INSERT ON outbox_delivery TO kartenn_app;

-- Spike 001 (c'): same data, constraints added after a plain bulk load.
DROP TABLE IF EXISTS address2;
CREATE TABLE address2 (LIKE address INCLUDING DEFAULTS);
\timing on
\echo '-- bulk load without constraints'
INSERT INTO address2 SELECT * FROM address;
\echo '-- add per-domain exclusion afterwards'
ALTER TABLE address2 ADD CONSTRAINT address2_unique_per_domain EXCLUDE USING GIST (
    domain_id WITH =, address inet_ops WITH =, valid_period WITH &&);
\echo '-- add partial public exclusion afterwards'
ALTER TABLE address2 ADD CONSTRAINT address2_public_unique EXCLUDE USING GIST (
    address inet_ops WITH =, valid_period WITH &&) WHERE (public AND usage <> 'anycast');
\timing off
\echo '-- single-row insert latency into the loaded table (100 inserts)'
\timing on
INSERT INTO address2 (id, domain_id, address)
SELECT gen_random_uuid(), domain_id, address + 1000000 FROM address2 WHERE family(address) = 6 LIMIT 100;
\timing off

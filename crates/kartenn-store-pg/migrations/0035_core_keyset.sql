-- Pagination by key of the other objects of the core (spec 006, REQ-006-7; task 006-12a),
-- as 0034 for the prefixes: slug then identifier for the entities, sites and domains;
-- domain, value then identifier for the ranges and addresses; on both sides of a dated
-- read (current rows and history).
CREATE INDEX entity_keyset ON entity (slug, id);
CREATE INDEX entity_history_keyset ON entity_history (slug, id);
CREATE INDEX site_keyset ON site (slug, id);
CREATE INDEX site_history_keyset ON site_history (slug, id);
CREATE INDEX addressing_domain_keyset ON addressing_domain (slug, id);
CREATE INDEX addressing_domain_history_keyset ON addressing_domain_history (slug, id);
CREATE INDEX ip_range_keyset ON ip_range (domain_id, range, id);
CREATE INDEX ip_range_history_keyset ON ip_range_history (domain_id, range, id);
CREATE INDEX address_keyset ON address (domain_id, address, id);
CREATE INDEX address_history_keyset ON address_history (domain_id, address, id);

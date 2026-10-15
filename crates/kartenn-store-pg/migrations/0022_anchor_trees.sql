-- Inheritance along the entity and site trees (spec 003, REQ-003-11, 16; task 003-18).
-- Every object that may carry a property, whatever its tree: the addressing objects, then
-- entities and sites (no domain, no value).
CREATE VIEW carrier_object AS
    SELECT id, domain_id, value, kind, valid_period FROM addressing_object
    UNION ALL
    SELECT id, NULL::uuid, NULL::inet, 'entity', valid_period FROM entity
    UNION ALL
    SELECT id, NULL::uuid, NULL::inet, 'site', valid_period FROM site;
GRANT SELECT ON carrier_object TO kartenn_app;

-- The descendants of a moved entity or site, level by level.
CREATE INDEX entity_by_parent ON entity (parent_id) WHERE parent_id IS NOT NULL;
CREATE INDEX site_by_parent ON site (parent_id) WHERE parent_id IS NOT NULL;

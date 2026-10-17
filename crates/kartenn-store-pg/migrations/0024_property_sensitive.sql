-- Sensitive properties (spec 003, REQ-003-27, P17; spec 012, REQ-012-18): a versioned
-- flag of the declaration; reading the value of a sensitive property is audited at high
-- criticality.
CALL kartenn_add_versioned_column('property_declaration', 'sensitive boolean NOT NULL DEFAULT false');

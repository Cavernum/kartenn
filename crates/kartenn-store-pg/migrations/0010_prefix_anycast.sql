-- Anycast networks (spec 002, A13, REQ-002-46, 49): a public network marked anycast may
-- exist in several domains if every occurrence is anycast; the cross-domain rule is an
-- application rule (core.public.overlap). Only networks carry the mark.
CALL kartenn_add_versioned_column('prefix', 'anycast boolean NOT NULL DEFAULT false');
ALTER TABLE prefix ADD CONSTRAINT prefix_anycast_network CHECK (NOT anycast OR nature = 'network');

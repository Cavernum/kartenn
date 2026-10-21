-- Who set an obligation (spec 008, REQ-008-28; besoin N8): `module` for those of a
-- declaration, which the synchronisation of the module brings in line with its code;
-- `admin` for those an administrator adds by changeset on a property of a module, which
-- the synchronisation never reads, rewrites nor removes. Both are enforced alike.

CALL kartenn_add_versioned_column(
    'property_obligation',
    'origin text NOT NULL DEFAULT ''module'' CHECK (origin IN (''module'', ''admin''))');

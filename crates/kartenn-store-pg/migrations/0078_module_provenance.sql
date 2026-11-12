-- The provenance of a value a module derives (spec 019, Q-019-34; ADR 0026, amendment
-- 14): `module:<module>#<motif>`, the module named as the registry names it
-- (`[a-z][a-z0-9_]*`), the motif saying what gave the value (for kube,
-- `label:<key>`, the label of the workload). Besides `input` (typed) and
-- `source:<id>@<version>` (a default value, REQ-003-22), as 0011 admitted them. Only an
-- operation a module plan adds carries it, of the name of that module: the pipeline
-- checks it (`prop.assignment.provenance`); the database keeps the form. The history
-- (`LIKE` without constraints) checks nothing.
ALTER TABLE property_assignment DROP CONSTRAINT property_assignment_provenance_check;
ALTER TABLE property_assignment ADD CONSTRAINT property_assignment_provenance_check CHECK (
    provenance = 'input'
    OR provenance ~ '^source:[^@]+@.+$'
    OR provenance ~ '^module:[a-z][a-z0-9_]*#.+$'
);

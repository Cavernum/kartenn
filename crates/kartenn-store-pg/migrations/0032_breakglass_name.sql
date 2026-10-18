-- The name of a break-glass account is its login (spec 005, REQ-005-14): unique among
-- the break-glass accounts.
CREATE UNIQUE INDEX person_breakglass_name ON person (display_name) WHERE kind = 'breakglass';

-- Permission names of spec 018 (REQ-018-1, R2, R5; task 018-1): a permission is
-- `<module>.<resource>.<action>`, or `<module>.<action>` when the resource is named after
-- the module (`flow.read`). A role entry is `*`, a catalogue permission of two segments
-- or more, `prop.<key>.<action>` or `prop.*.<action>`. The history (`LIKE` without
-- constraints) carries no check.
ALTER TABLE role_permission DROP CONSTRAINT role_permission_permission_check;
ALTER TABLE role_permission ADD CONSTRAINT role_permission_permission_check
    CHECK (permission ~ '^(\*|[a-z][a-z0-9_]*(\.[a-z0-9_.*]+)?\.[a-z_]+)$');

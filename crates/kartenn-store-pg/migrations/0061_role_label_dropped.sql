-- The label of a role gives way to its labels by language (spec 018, R20, Q-018-2; task
-- 018-7): a role written since `0059_labels` without labels takes its label first.

ALTER TABLE role DISABLE TRIGGER versioning;
UPDATE role SET labels = jsonb_build_object('fr', label) WHERE labels = '{}';
ALTER TABLE role ENABLE TRIGGER versioning;
UPDATE role_history SET labels = jsonb_build_object('fr', label) WHERE labels = '{}';

ALTER TABLE role DROP COLUMN label;
ALTER TABLE role_history DROP COLUMN label;
-- The trigger copies the columns of the table by name: its list loses `label`.
CALL kartenn_versioning_trigger('role');

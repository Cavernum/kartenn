-- The labels of the policy are its tables `labels` and those of the closed-list values of
-- the core (spec 018, R20, Q-018-2, Q-018-7; task 018-7): the columns `label` go, after a
-- last copy of the rows written since `0004_labels` without labels.

DO $$
DECLARE
    t text;
BEGIN
    FOREACH t IN ARRAY ARRAY['policy_flow_type', 'policy_service', 'policy_pattern', 'policy_rule'] LOOP
        EXECUTE format('ALTER TABLE %I DISABLE TRIGGER versioning', t);
        EXECUTE format('UPDATE %I SET labels = jsonb_build_object(''fr'', label) WHERE labels = ''{}''', t);
        EXECUTE format('ALTER TABLE %I ENABLE TRIGGER versioning', t);
        EXECUTE format('UPDATE %I SET labels = jsonb_build_object(''fr'', label) WHERE labels = ''{}''',
                       t || '_history');
        EXECUTE format('ALTER TABLE %I DROP COLUMN label', t);
        EXECUTE format('ALTER TABLE %I DROP COLUMN label', t || '_history');
        EXECUTE format('CALL public.kartenn_versioning_trigger(%L)', 'policy.' || t);
    END LOOP;
END $$;

-- The label of a value of an axis was copied to the core by `0004_labels`.
ALTER TABLE policy_value DROP COLUMN label;
ALTER TABLE policy_value_history DROP COLUMN label;
CALL public.kartenn_versioning_trigger('policy.policy_value');

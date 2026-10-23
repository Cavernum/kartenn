-- Labels by language (spec 018, R8, Q-018-6, Q-018-7; task 018-6; ADR 0040): a column
-- `labels` `{<language>: <text>}` on each owning table of the policy, versioned with its
-- row. The labels kept until now become labels of `fr`, the only language of the data of
-- the v0.2. The label of a value of an axis (`policy_value.label`) becomes a label of the
-- closed-list value of the core (`property_enum_value.labels`, Q-018-7). The columns
-- `label` go with the writes that replace them (task 018-7).

CALL public.kartenn_add_versioned_column('policy_flow_type',
    'labels jsonb NOT NULL DEFAULT ''{}'' CHECK (jsonb_typeof(labels) = ''object'')');
CALL public.kartenn_add_versioned_column('policy_service',
    'labels jsonb NOT NULL DEFAULT ''{}'' CHECK (jsonb_typeof(labels) = ''object'')');
CALL public.kartenn_add_versioned_column('policy_pattern',
    'labels jsonb NOT NULL DEFAULT ''{}'' CHECK (jsonb_typeof(labels) = ''object'')');
CALL public.kartenn_add_versioned_column('policy_rule',
    'labels jsonb NOT NULL DEFAULT ''{}'' CHECK (jsonb_typeof(labels) = ''object'')');
CALL public.kartenn_add_versioned_column('policy_condition',
    'labels jsonb NOT NULL DEFAULT ''{}'' CHECK (jsonb_typeof(labels) = ''object'')');
CALL public.kartenn_add_versioned_column('policy_signal',
    'labels jsonb NOT NULL DEFAULT ''{}'' CHECK (jsonb_typeof(labels) = ''object'')');

-- The same rows, labelled: not a new version (no changeset), hence without the trigger.
DO $$
DECLARE
    t text;
BEGIN
    FOREACH t IN ARRAY ARRAY['policy_flow_type', 'policy_service', 'policy_pattern', 'policy_rule'] LOOP
        EXECUTE format('ALTER TABLE %I DISABLE TRIGGER versioning', t);
        EXECUTE format('UPDATE %I SET labels = jsonb_build_object(''fr'', label)', t);
        EXECUTE format('ALTER TABLE %I ENABLE TRIGGER versioning', t);
        EXECUTE format('UPDATE %I SET labels = jsonb_build_object(''fr'', label)', t || '_history');
    END LOOP;
END $$;

-- The label of a value of an axis, on each version of the closed-list value of the same
-- key: the latest version of the value of the policy that has one.
CREATE TEMPORARY TABLE axis_value_label ON COMMIT DROP AS
SELECT DISTINCT ON (d.id, v.value_key) d.id AS property_id, v.value_key, v.label
FROM policy_value v JOIN public.property_declaration d ON d.key = v.property_key
WHERE v.label IS NOT NULL
ORDER BY d.id, v.value_key, lower(v.valid_period) DESC;
ALTER TABLE public.property_enum_value DISABLE TRIGGER versioning;
UPDATE public.property_enum_value e
SET labels = e.labels || jsonb_build_object('fr', l.label)
FROM axis_value_label l
WHERE l.property_id = e.property_id AND l.value_key = e.key;
ALTER TABLE public.property_enum_value ENABLE TRIGGER versioning;
UPDATE public.property_enum_value_history e
SET labels = e.labels || jsonb_build_object('fr', l.label)
FROM axis_value_label l
WHERE l.property_id = e.property_id AND l.value_key = e.key;

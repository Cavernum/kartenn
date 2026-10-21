-- External references (spec 016, REQ-016-3, G9): a value of type `external_ref` is
-- stored as `{"system": <closed-list value of core.external_system>, "id", "label"}`, a
-- set as a JSON array of such objects. Equality and filters compare by containment of
-- `{system, id}`, which ignores the label.

CALL kartenn_add_versioned_column(
    'property_assignment',
    'v_json jsonb CHECK (v_json IS NULL OR jsonb_typeof(v_json) = ''object'')');
CALL kartenn_add_versioned_column(
    'property_assignment',
    'v_jsons jsonb CHECK (v_jsons IS NULL OR (jsonb_typeof(v_jsons) = ''array'' AND jsonb_array_length(v_jsons) > 0))');

ALTER TABLE property_assignment DROP CONSTRAINT property_assignment_value;
ALTER TABLE property_assignment ADD CONSTRAINT property_assignment_value CHECK (
    num_nonnulls(v_uuid, v_uuids, v_text, v_texts, v_int, v_ints, v_bool, v_date, v_dates,
                 v_json, v_jsons)
    = CASE kind WHEN 'value' THEN 1 ELSE 0 END);

-- Filters (REQ-003-25): containment of `{system, id}`.
CREATE INDEX property_assignment_json ON property_assignment
    USING GIN (v_json jsonb_path_ops) WHERE v_json IS NOT NULL;
CREATE INDEX property_assignment_jsons ON property_assignment
    USING GIN (v_jsons jsonb_path_ops) WHERE v_jsons IS NOT NULL;

-- The value as the API shows it: the system by its key.
CREATE OR REPLACE FUNCTION public.kartenn_external_json(v jsonb) RETURNS jsonb
LANGUAGE sql STABLE SET search_path = pg_catalog, public AS $$
    SELECT CASE WHEN v IS NULL THEN NULL
                ELSE jsonb_set(v, '{system}', coalesce(
                    (SELECT to_jsonb(e.key) FROM public.property_enum_value e
                      WHERE e.id = (v->>'system')::uuid LIMIT 1),
                    v->'system'))
           END
$$;

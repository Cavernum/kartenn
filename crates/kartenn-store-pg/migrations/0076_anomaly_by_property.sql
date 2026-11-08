-- One open anomaly per rule, object and property (spec 021, A39, task 11n): a deviation
-- between resolved and expected values (`prop.resolution.expected_mismatch`) is about one
-- property of its object, so two properties in deviation on one object are two anomalies.
-- The property is the `property` member of the detail (0075); a rule whose detail names
-- none keeps one anomaly per object.

-- A deviation opened before 0075 has no detail yet: its property is the key its message
-- starts with (`<key>: resolved …`, a key holding no colon, REQ-003-2), so that it keeps
-- its identity, its acknowledgement and its opening when it is evaluated anew, and that
-- the `property` filter finds it. Its values stay unknown until then.
UPDATE anomaly
SET detail = jsonb_build_object('property', split_part(message, ':', 1))
WHERE rule_id = 'prop.resolution.expected_mismatch'
  AND detail IS NULL
  AND position(':' IN message) > 1;

DROP INDEX anomaly_one_open;
CREATE UNIQUE INDEX anomaly_one_open ON anomaly (rule_id, object_id, coalesce(detail ->> 'property', ''))
    WHERE closed_at IS NULL;

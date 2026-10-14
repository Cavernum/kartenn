-- Covering index of the origins by origin (spec 003, REQ-003-25, 26; task 003-16): a
-- filtered page reads the objects resolved to the matching origins of a domain from the
-- index alone (bench: 88 ms with heap fetches, 25 ms without).
DROP INDEX property_origin_by_origin;
CREATE INDEX property_origin_by_origin
    ON property_origin (property_id, origin_object_id, object_id) INCLUDE (valid_period);

-- The list of the groups of VLAN and their suggestions by prefix (spec 018, REQ-018-23;
-- task 018-16): the order of the list (slug, identifier) read from an index, which stops
-- at the page rather than sorting every group the reader may read.

CREATE INDEX vlan_group_keyset ON vlan_group (slug, id);

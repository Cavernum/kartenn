-- Suggestions by prefix (spec 018, REQ-018-23; tasks 018-15, 16): the text of a prefix
-- and of an address in the collation `C`, compared by `LIKE 'q%'` (a range of the index,
-- whatever the collation of the database) and in the order of which a page of
-- suggestions goes: the text follows the domain, so the order of the list (domain, then
-- value) would walk every domain before the rows of a rare prefix. The slugs (entities,
-- sites, domains) stay without: short lists, in the order of their slug already.

CREATE INDEX prefix_text_suggested ON prefix ((text(prefix) COLLATE "C"), domain_id, id);
CREATE INDEX address_host_suggested ON address ((host(address) COLLATE "C"), domain_id, id);

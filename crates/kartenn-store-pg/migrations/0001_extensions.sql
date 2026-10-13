-- btree_gist: GiST equality on scalar columns (uuid) inside exclusion constraints.
-- Never used for inet/cidr: its gist_inet_ops/gist_cidr_ops classes are defective
-- (docs/reference/postgresql-18-reseau.md); network columns name inet_ops explicitly.
CREATE EXTENSION IF NOT EXISTS btree_gist;

-- Borrowed closed-list values (spec 016, REQ-016-2, G5): a closed list whose
-- `type_params` names `values_of` shares the values of that declaration, which stay
-- indexed by their owner. Every read of the values of a property goes through its owner.

CREATE OR REPLACE FUNCTION public.kartenn_values_owner(property uuid) RETURNS uuid
LANGUAGE sql STABLE SET search_path = pg_catalog, public AS $$
    SELECT coalesce(
        (SELECT o.id
           FROM public.property_declaration d
           JOIN public.property_declaration o ON o.key = d.type_params->>'values_of'
          WHERE d.id = property
          ORDER BY upper(o.valid_period) DESC NULLS FIRST
          LIMIT 1),
        property)
$$;

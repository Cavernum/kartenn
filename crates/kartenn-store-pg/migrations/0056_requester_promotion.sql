-- A requester named by a service account (spec 016, REQ-016-12, G10, INV-60) holds the
-- issuer and subject of a real person: when that person logs in through the issuer, the
-- row becomes a person, under the name the issuer gives (the name the service account
-- chose never prevails). The application still never changes the kind of an identity
-- (rbac_schema): this function allows the one transition requester → person, nothing else.
CREATE FUNCTION public.kartenn_promote_requester(p_issuer text, p_subject text) RETURNS void
LANGUAGE sql SECURITY DEFINER SET search_path = pg_catalog, public AS $$
    UPDATE public.person SET kind = 'person'
     WHERE issuer = p_issuer AND subject = p_subject AND kind = 'requester';
$$;

REVOKE EXECUTE ON FUNCTION public.kartenn_promote_requester(text, text) FROM PUBLIC;
GRANT EXECUTE ON FUNCTION public.kartenn_promote_requester(text, text) TO kartenn_app;

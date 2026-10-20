# Notes de référence

Notes de lecture des sources primaires (RFC, registres IANA, drafts IETF, documentation
officielle). Elles servent de base aux specs ; elles ne décident rien à elles seules.

## Conventions

Chaque note contient :

1. **Sources** : URL, statut du document, date de consultation.
2. **Ce que dit la source** : exigences reprises avec leur section. Les mots-clés
   normatifs (MUST, SHOULD, MAY…) gardent le sens de RFC 2119 / RFC 8174 et restent en
   anglais pour ne pas en altérer la force.
3. **Pièges** : ce qui surprend ou diverge entre implémentations.
4. **Implications pour kartenn** : propositions, marquées comme telles, à trancher dans
   la spec concernée. Elles renvoient aux décisions `D<k>` de
   [`specs/001-cadrage/decisions.md`](../../specs/001-cadrage/decisions.md).

## Index

| Note | Sujet | Spec |
|---|---|---|
| [adressage-ipv4.md](adressage-ipv4.md) | Format IPv4, CIDR, adresses réservées, /31 | 002 |
| [adressage-ipv6.md](adressage-ipv6.md) | Architecture d'adressage IPv6, ULA, zones, /127 | 002 |
| [representation-textuelle.md](representation-textuelle.md) | Analyse stricte et forme canonique des adresses et préfixes | 002 |
| [registres-usage-special.md](registres-usage-special.md) | Registres IANA des adresses à usage spécial | 002 |
| [ipv8-draft.md](ipv8-draft.md) | État de draft-thain-ipv8 | 002 (point d'extension) |
| [postgresql-18-reseau.md](postgresql-18-reseau.md) | Types `inet`/`cidr`, opérateurs, index, contraintes d'exclusion ; apports de la 19 (bêta) | 002 |
| [temporel-sql2011-postgresql.md](temporel-sql2011-postgresql.md) | Modèle temporel SQL:2011 et support réel de PostgreSQL 18 | 002, 004 |
| [noms-dates-texte.md](noms-dates-texte.md) | Noms d'hôte, IDNA2008 / UTS #46, dates RFC 3339, texte Unicode | 003 |
| [journalisation-audit.md](journalisation-audit.md) | Syslog, journald, OpenTelemetry, Trace Context, référentiels d'audit (OWASP, ASVS, NIST, ANSSI) | 012 |
| [authentification-autorisation.md](authentification-autorisation.md) | OAuth 2.0 (RFC 9700), BFF (RFC 10017), OIDC, jetons JWT (RFC 8725, 9068), TOTP, Argon2, NIST SP 800-63B | 005 |
| [api-http.md](api-http.md) | Sémantique HTTP, cache, Problem Details, pagination, idempotence, débit, CSRF, montée en charge des API REST | 006 |
| [web-navigateur.md](web-navigateur.md) | CSP Level 3, SRI, import maps et `modulepreload`, Custom Elements et Shadow DOM, cookies sur le bouclage, IndexedDB et `BroadcastChannel`, WCAG 2.2 et APG, vue-i18n sans `eval`, prise en charge par les navigateurs | 007 |
| [services-transports.md](services-transports.md) | Numéros de protocole, ports et noms de service (RFC 6335), ICMP | 010 |

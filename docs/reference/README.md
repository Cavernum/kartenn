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
| [vlan-mac-ieee-802.md](vlan-mac-ieee-802.md) | VID IEEE 802.1Q (valeurs réservées), adresses MAC EUI-48 (RFC 9542) | 008, 009 |
| [oauth-clients-natifs.md](oauth-clients-natifs.md) | Client OAuth natif public : bouclage et PKCE (RFC 8252, 7636), flux d'appareil (RFC 8628), rotation des jetons de rafraîchissement (RFC 9700), nonce OIDC, audience (RFC 9068, 8707) | 023 |
| [systeme-de-design.md](systeme-de-design.md) | Critères WCAG 2.2 du système de design, motifs APG, couches de cascade, `prefers-reduced-motion`, déterminisme des captures Playwright, licences OFL, ISC et MPL | 020 |
| [mouvement-interface.md](mouvement-interface.md) | Mouvement : WCAG 2.2 (SC 2.3.3, 2.2.2, 2.3.1), `prefers-reduced-motion`, `@starting-style` et `allow-discrete`, View Transitions, prise en charge ; politique de kartenn | 020 |
| [mise-en-page-adaptative.md](mise-en-page-adaptative.md) | Adaptation à la largeur : WCAG 2.2 (1.4.10 Reflow et son exception des tableaux, 1.4.4, 1.4.12, 1.3.4, 2.5.8), requêtes de conteneur, syntaxe d'intervalle, unités `dv*`, `auto-fit`, `subgrid`, `:has()`, prise en charge ; politique de kartenn | 020 |
| [push-serveur.md](push-serveur.md) | Canal push serveur → client : SSE, MCP Streamable HTTP, mandataires, alternatives, `LISTEN`/`NOTIFY` | 022 |
| [langues-negociation.md](langues-negociation.md) | Étiquettes BCP 47 (RFC 5646), Lookup (RFC 4647), `Accept-Language`, `Content-Language`, `Vary`, cache d'une réponse par langue | 018 |

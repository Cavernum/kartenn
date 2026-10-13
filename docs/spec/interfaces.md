---
status: approved
updated: 2026-10-14
---

# Interfaces

Les détails normatifs viendront des specs 005 (authentification, RBAC), 006 (conventions
REST) et 007 (shell et plugins). Ce document fixe les principes déjà décidés.

## API REST (D7, ADR 0002)

Seule frontière publique : Web, CLI, TUI, MCP et connecteurs passent tous par elle.

| Sujet | Principe | Source |
|---|---|---|
| Transport | HTTPS uniquement ; JSON (RFC 8259) | D13, RFC 9110 |
| Contrat | OpenAPI 3.1 généré depuis le code ; SDK clients générés (TypeScript, Rust) | D7 |
| Ressources | Une ressource par type d'objet du noyau et des modules ; désignation par UUID ou clé naturelle (slug, (domaine, préfixe)…) | 002/A9 |
| Écriture | Par **changeset** (opérations multiples, atomiques) ; raccourcis unitaires acceptables s'ils produisent un changeset | D11 |
| Simulation | Soumettre un changeset sans l'appliquer → validation, anomalies, différentiel | 003/REQ-003-20 |
| Temps | Paramètres `valid_at` (temps applicatif) et `as_of` (temps système), « maintenant » par défaut | D10 |
| Concurrence | ETag / `If-Match` (RFC 9110 §13) sur les objets ; conflit → 412 | Cadrage (« Noyau ou module », point A.8) |
| Erreurs | `application/problem+json` (RFC 9457) ; identifiants de règles stables ; objets en cause | 002/REQ-002-37 |
| Pagination | Par curseur ; tri stable | Cadrage (« Noyau ou module », point A.10) |
| Filtrage | Champs du noyau et valeurs résolues des propriétés | 003/REQ-003-25 |
| Capacités | Endpoint listant les modules actifs, leurs versions, propriétés et plugins UI | D7, D17 |
| Versionnement | À décider (Q-ouv-7) | — |

## Authentification (D13, ADR 0009)

- **Humains** : OIDC, authorization code + PKCE (RFC 7636), plusieurs émetteurs de
  confiance configurés ; le serveur valide les JWT (signature via JWKS en cache,
  `iss`, `aud`, expiration ; RFC 8725). Les claims (groupes) sont mappés sur des liaisons
  RBAC.
- **Machines** (CLI, connecteurs, scripts) : comptes de service en **Basic** (RFC 7617)
  sur TLS ; secrets hachés en Argon2id (RFC 9106).
- **Secours** : comptes humains locaux en Basic, réservés au break-glass.

## Autorisation (D36, 003/P6–P7, ADR 0010)

- Permissions déclarées par les modules (`<module>.<objet>.<action>`, et
  `prop.<clé>.write` par propriété).
- Rôle = ensemble de permissions ; liaison = sujet × rôle × périmètre (entité, site,
  domaine, sous-arbre de préfixes), héritée vers le bas.
- Une écriture exige le droit sur les objets visés **et** sur les effets indirects
  (différentiel des propriétés).
- Politiques d'approbation dans le même modèle.

## SDK de plugins UI (D17, ADR 0011)

- Plugin = module ES chargé dynamiquement, qui enregistre des Custom Elements.
- Le SDK TypeScript versionné fournit : client API généré, contexte (utilisateur,
  `valid_at`, `as_of`, objet courant), points d'extension (onglets, vues, panneaux
  d'objet).
- Un plugin n'a que les droits de l'utilisateur ; chargement soumis à CSP et SRI.
- Licence du SDK : question ouverte Q-ouv-1.

## Connecteurs (D1, D7)

- Processus externes, clients REST, authentifiés par compte de service.
- Importent par changesets (lots) ; référencent les objets par identifiant externe
  (source, id externe → UUID, REQ-002-41).

## Configuration et exploitation (D35)

- Configuration 12-factor : fichier et variables d'environnement.
- Sondes : `/health/live`, `/health/ready`.
- Observabilité : traces, métriques et logs JSON via OpenTelemetry (OTLP).

---
status: proposed
date: 2026-10-18
---

# 0022 — Contrat HTTP de l'API

## Context

L'API REST est la seule frontière publique (D7, ADR 0002) : Web, CLI, MCP et connecteurs
en dépendent, et leurs SDK sont générés de son contrat. Les instances sont sans état
(D9), le modèle est bitemporel (D10) et toute écriture est un changeset (D11). Décisions
006/H1 à H15 ; sources : [`docs/reference/api-http.md`](../reference/api-http.md).

## Decision

- Version majeure dans le chemin (`/v1`), évolution additive seulement ; retrait annoncé
  par `Deprecation` (RFC 9745) et `Sunset` (RFC 8594). Contrat OpenAPI 3.1 tiré du code,
  comparé par un test au contrat de référence du dépôt.
- Collections à plat, un objet par UUID ; clé naturelle en filtre ; objets complets avec
  un résumé des objets liés ; ni `fields=` ni `expand=`.
- Lectures datées par `valid_at` et `as_of` ; une lecture au passé figé (au-delà de la
  durée maximale d'une transaction d'écriture) est `immutable`, les autres `no-cache` et
  ETag.
- Listes par curseur opaque signé (HMAC du trousseau), sans total ; filtres en
  paramètres conjonctifs, et méthode `QUERY` (RFC 10008) pour les filtres longs.
- Écritures par `POST /v1/changesets` et par routes de ressource qui enveloppent un
  changeset d'une opération ; `PATCH` en JSON Merge Patch (RFC 7396) ; `If-Match`
  obligatoire (428, 412) ; `Idempotency-Key`.
- Erreurs en Problem Details (RFC 9457), `type` en URN `urn:kartenn:problem:<slug>` ;
  liste complète des violations d'un changeset refusé (422) ; 404 pour un objet
  invisible.

## Alternatives

- Version dans le type de média ou versions datées par requête : `Vary`, outils mal à
  l'aise, couches de traduction à maintenir.
- Offset et total : coût linéaire, pages qui glissent, total révélateur sous RBAC.
- JSON Patch (RFC 6902) : second langage d'opérations à côté des changesets.
- `expand=` : coût non borné, RBAC à chaque expansion.

## Consequences

- `QUERY` n'a pas de filtre de méthode dans axum ni de place dans OpenAPI 3.1 : route
  par `any()` et extension `x-query`, jusqu'à OpenAPI 3.2.
- Un proxy qui ne relaie pas `QUERY` laisse `GET` disponible jusqu'à 8 Kio d'URL.
- Le contrat de référence est un fichier à régénérer à chaque ajout.

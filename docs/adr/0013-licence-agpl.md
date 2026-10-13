---
status: accepted
date: 2026-10-13
---

# 0013 — Licence AGPL-3.0

## Context

Choix de licence du projet. Décision D37 ([journal](../../specs/001-cadrage/decisions.md)).

## Decision

kartenn est publié sous AGPL-3.0.

## Alternatives

- Apache-2.0, MIT OR Apache-2.0 : adoption plus large, mais pas d'obligation de
  publication pour un service modifié.
- Propriétaire : projet fermé.

## Consequences

- Toute version modifiée exploitée en service doit publier ses sources.
- Le SDK de plugins et les clients générés pourraient relever d'une licence plus
  permissive pour permettre des extensions propriétaires (Q-ouv-1).
- Les dépendances doivent être compatibles avec l'AGPL.

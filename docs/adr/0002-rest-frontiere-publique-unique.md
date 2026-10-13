---
status: accepted
date: 2026-10-13
---

# 0002 — L'API REST, seule frontière publique

## Context

Authentification, RBAC, validation, audit et limitation de débit doivent s'appliquer
partout de la même façon. Décision D7 ([journal](../../specs/001-cadrage/decisions.md)).

## Decision

Le Web, la CLI, la TUI, le serveur MCP et les connecteurs sont tous des **clients de
l'API REST**. Aucun accès direct à la couche service hors du serveur.

## Alternatives

- MCP dans le processus serveur : un saut réseau de moins, mais deux surfaces à sécuriser
  et à garder alignées.
- Connecteurs dans le processus : performance brute, mais compilés (pas d'ajout à
  chaud) et contournant l'audit et la limitation de débit de l'API.

## Consequences

- L'API doit offrir des changesets par lots performants (imports de 10⁶ objets < 15 min)
  et un endpoint de capacités.
- Les outils de kartenn consomment eux-mêmes l'API, ce qui en force la qualité.
- Un SDK client généré depuis l'OpenAPI sert la CLI, le MCP et les connecteurs.

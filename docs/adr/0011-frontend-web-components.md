---
status: accepted
date: 2026-10-13
---

# 0011 — Shell Vue 3 et plugins Web Components

## Context

Le frontend doit accepter des extensions ajoutées ou mises à jour sans reconstruction
(ex. un onglet de visualisation). Décisions D6, D17 ([journal](../../specs/001-cadrage/decisions.md)).

## Decision

Shell en Vue 3 + TypeScript. Plugins = modules ES chargés dynamiquement qui enregistrent
des Custom Elements ; SDK TypeScript versionné (client API, contexte, points
d'extension) ; plugins déclarés par l'endpoint de capacités ; CSP et SRI.

## Alternatives

- React + Module Federation : contrat lié à la chaîne de build et aux versions partagées.
- Svelte : très léger, écosystème plus petit pour tableaux et graphes.

## Consequences

- Un plugin peut être écrit avec n'importe quel framework.
- Le SDK devient un contrat public versionné ; sa licence est à décider (Q-ouv-1).

---
status: proposed
date: 2026-10-14
---

# 0017 — Temps système : table courante, table d'historique, déclencheur

## Context

Le modèle bitemporel (ADR 0005) exige un temps système immuable, identique pour toutes
les versions d'un changeset, et des lectures « à la date X » au plus deux fois plus
lentes que la lecture courante. PostgreSQL 18 ne fournit pas de tables versionnées
système. Les contraintes métier ne doivent porter que sur les versions courantes.

## Decision

Chaque type d'objet versionné a une table courante (versions applicatives actuellement
affirmées, contraintes métier ordinaires) et une table d'historique (versions dont
l'affirmation est terminée, partitionnée par `sys_end`). Un déclencheur générique
`BEFORE INSERT/UPDATE/DELETE` pose `sys_start = now()` et copie toute version remplacée
ou supprimée dans l'historique avec `sys_end = now()`. Le rôle applicatif ne peut ni
modifier ni supprimer l'historique.

## Alternatives

- Table unique avec contraintes et index partiels : index en double, table courante
  alourdie par l'historique, prédicat à ne jamais oublier.
- Historisation par la couche service seule : contournable par une écriture SQL directe.
- Extension externe : dépendance pour une fonction triviale.

## Consequences

- Le temps système vient toujours de la base, même avec N instances (ADR 0012).
- Les lectures `as_of` passées font une union courante + historique.
- Chaque migration de module crée la paire de tables et attache le déclencheur.
- Le découpage des périodes (« FOR PORTION OF ») reste à la charge de la couche service.

---
status: accepted
date: 2026-10-13
---

# 0005 — Modèle bitemporel natif

## Context

Il faut planifier des changements, corriger le passé et savoir ce que kartenn affirmait
à une date donnée (audit infalsifiable). Décisions D8, D10 ([journal](../../specs/001-cadrage/decisions.md)) ; référence
SQL:2011 ([note](../reference/temporel-sql2011-postgresql.md)).

## Decision

Chaque objet, affectation de propriété et déclaration est versionné selon deux axes :
temps applicatif (`valid_period`, fourni par l'utilisateur) et temps système
(`system_period`, horodatage de la transaction, immuable). Périodes `[début, fin)`.
Les invariants portent sur le temps applicatif, pour les versions courantes côté
système. Les requêtes acceptent `valid_at` et `as_of`.

## Alternatives

- Audit simple : perd la reconstitution d'état.
- Journal et rejeu : reconstitution lente, pas de requête directe.
- Temps système seul : ni planification ni correction rétroactive.
- Temps applicatif seul : une correction efface ce que l'outil affirmait (audit non fiable).

## Consequences

- Le temps système et le découpage de périodes sont à la charge de kartenn
  (PostgreSQL 18 ne les fournit pas).
- Le choix entre table courante + historique et table unique fera l'objet d'un ADR
  (spec 004), sur critère de performance (lecture datée ≤ ×2).
- Toutes les contraintes d'unicité deviennent temporelles.

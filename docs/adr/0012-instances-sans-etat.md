---
status: accepted
date: 2026-10-13
---

# 0012 — Instances sans état, base comme point de cohérence

## Context

Le serveur doit monter en charge horizontalement, avec des invariants stricts. Décision
D9 ([journal](../../specs/001-cadrage/decisions.md)).

## Decision

N instances interchangeables, sans état local faisant autorité ; jetons auto-portés ;
tâches de fond réparties par verrous en base ; migrations par un job dédié. PostgreSQL
est le point de cohérence : un primaire unique pour les écritures, des réplicas en
lecture (sauf lecture de ses propres écritures).

## Alternatives

- Écritures réparties : imposerait des invariants distribués, contraire à D1.
- Instances avec sessions locales : affinité de session, perte d'état au redémarrage.

## Consequences

- Les écritures ne montent pas en charge horizontalement (50 écritures/s visées,
  suffisant à 10⁶ objets).
- Toute horloge de référence est celle de la base.

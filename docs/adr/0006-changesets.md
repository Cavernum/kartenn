---
status: accepted
date: 2026-10-13
---

# 0006 — Toute écriture passe par un changeset

## Context

Écritures unitaires, lots, imports de connecteurs et revues doivent partager les mêmes
garanties ; certaines écritures (prestataires, classification de sécurité) exigent une
approbation. Décision D11 ([journal](../../specs/001-cadrage/decisions.md)).

## Decision

Toute écriture est un **changeset** : opérations multiples, validées puis appliquées
atomiquement ; appliqué dans le même appel par défaut, mis en **approbation** si une
politique RBAC l'exige, puis revalidé à l'application. Le changeset porte auteur, motif,
période applicative, différentiel et anomalies ; c'est l'unité du temps système, de
l'audit et de l'outbox. Il peut être simulé sans être appliqué.

## Alternatives

- Écriture directe : ajouter une revue plus tard imposerait de refaire le pipeline.
- Branches à la Git : fusion bitemporelle et invariants globaux trop complexes.

## Consequences

- Un seul pipeline d'écriture à optimiser et à sécuriser.
- Les changesets en attente peuvent être invalidés par des changements concurrents
  (contrôle optimiste à l'application).

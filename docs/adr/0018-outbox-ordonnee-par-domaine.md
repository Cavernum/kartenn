---
status: proposed
date: 2026-10-14
---

# 0018 — Outbox transactionnelle, ordonnée par domaine, au format CloudEvents

## Context

Chaque changeset appliqué doit produire un événement fiable (au moins une fois, sans
trou, dans l'ordre d'application) pour les connecteurs, notifications et caches
(spec 004). Une séquence PostgreSQL n'est pas attribuée dans l'ordre des validations de
transaction : lire l'outbox par numéro croissant peut sauter une transaction lente.

## Decision

L'événement est écrit dans une table `outbox` dans la transaction du changeset, avec
l'identifiant de transaction (`xid8`). Un relais unique ne livre que les lignes de
transactions terminées (`xid < pg_snapshot_xmin(pg_current_snapshot())`), par numéro
croissant, et n'avance qu'après accusé. L'ordre est garanti **par domaine d'adressage**
(les changesets d'un domaine sont sérialisés par ADR 0015). Format CloudEvents 1.0 :
`id` = UUID du changeset, dédoublonnage par `source` + `id`.

## Alternatives

- Lecture par numéro seul : risque de trou.
- Ordre global : imposerait de sérialiser toutes les écritures.
- Décodage logique (réplication) : plus puissant, mais plus complexe à exploiter.

## Consequences

- Une transaction très longue retarde la livraison des événements suivants.
- Les consommateurs doivent être idempotents (dédoublonnage par `id`).

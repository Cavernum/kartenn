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
l'identifiant de transaction (`xid8`). Un relais par consommateur ne livre que les lignes
de transactions terminées (`xid < pg_snapshot_xmin(pg_current_snapshot())`), jamais encore
livrées à ce consommateur, par numéro croissant, et **enregistre chaque livraison** après
accusé (table `outbox_delivery`).

**Amendement (implémentation, 2026-10-14)** : la première version avançait un curseur
unique (dernier numéro livré). Elle pouvait sauter un événement : une transaction encore en
cours détient le numéro 5, une transaction d'un autre domaine, commencée avant elle, valide
le numéro 6 ; le curseur passe à 6 et le 5 n'est jamais livré. L'enregistrement des
livraisons supprime ce trou (test `a_running_transaction_is_never_skipped`). L'ordre est garanti **par domaine d'adressage**
(les changesets d'un domaine sont sérialisés par ADR 0015). Format CloudEvents 1.0 :
`id` = UUID du changeset, dédoublonnage par `source` + `id`.

## Alternatives

- Lecture par numéro seul : risque de trou.
- Ordre global : imposerait de sérialiser toutes les écritures.
- Décodage logique (réplication) : plus puissant, mais plus complexe à exploiter.

## Consequences

- Une transaction très longue retarde la livraison des événements suivants.
- Les consommateurs doivent être idempotents (dédoublonnage par `id`).

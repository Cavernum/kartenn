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

**Second amendement (implémentation, 2026-10-14)** : la barrière
`xid < pg_snapshot_xmin(pg_current_snapshot())` est retirée. Elle imposait un ordre
global que la décision ne promet pas, et `xmin` vaut pour **toute l'instance** PostgreSQL :
une transaction longue dans n'importe quelle base (sauvegarde, rapport, test) bloquait
tous les consommateurs. Sans elle, le relais lit les événements validés et non livrés par
numéro croissant ; un événement d'une transaction en cours, invisible, est livré au
passage suivant (l'enregistrement des livraisons empêche tout trou). Il ne peut être
dépassé que par des événements d'autres domaines : deux changesets d'un même domaine sont
sérialisés par son verrou (ADR 0015) et numérotent donc leurs événements dans l'ordre de
validation. La colonne `xid` est conservée pour le diagnostic.

## Alternatives

- Lecture par numéro seul : risque de trou.
- Ordre global : imposerait de sérialiser toutes les écritures.
- Décodage logique (réplication) : plus puissant, mais plus complexe à exploiter.

## Consequences

- Une transaction longue ne retarde que ses propres événements ; entre domaines
  différents, l'ordre de livraison peut différer de l'ordre de numérotation.
- La garantie d'ordre par domaine repose sur les verrous de domaine : toute écriture
  dans un domaine doit prendre son verrou.
- Les consommateurs doivent être idempotents (dédoublonnage par `id`).

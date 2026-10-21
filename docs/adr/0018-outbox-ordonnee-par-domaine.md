---
status: accepted
date: 2026-10-14
---

# 0018 — Outbox transactionnelle, ordonnée par domaine, au format CloudEvents

## Context

Chaque changeset appliqué doit produire un événement fiable (au moins une fois, sans
trou, dans l'ordre d'application) pour les connecteurs, notifications et caches
(spec 004). Une séquence PostgreSQL n'est pas attribuée dans l'ordre des validations de
transaction : lire l'outbox par numéro croissant peut sauter une transaction lente.

## Decision

*Décision en vigueur, après les deux amendements ci-dessous.* L'événement est écrit dans
une table `outbox` dans la transaction du changeset, avec l'identifiant de transaction
(`xid8`, pour le diagnostic). Un relais par consommateur livre les lignes validées et
jamais encore livrées à ce consommateur, par numéro croissant, et **enregistre chaque
livraison** après accusé (table `outbox_delivery`). L'ordre est garanti par domaine
d'adressage, par les verrous de l'ADR 0015 (test
`events_of_a_domain_follow_the_order_of_its_changesets`). Format CloudEvents 1.0 :
`id` = UUID du changeset pour `org.kartenn.changeset.applied`, `<uuid>/<état>` pour les
événements d'état d'un changeset en attente ; `source` = `urn:kartenn:instance`.

*Décision initiale (2026-10-14), remplacée* : le relais ne livrait que les lignes de
transactions terminées (`xid < pg_snapshot_xmin(pg_current_snapshot())`).

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

**Troisième amendement (spec 012, tâche 21, 2026-10-17)** : le relais relisait toute
l'outbox à chaque passe (anti-jointure sur `outbox_delivery`), un coût qui croît avec
l'histoire. Il lit désormais à partir d'un **plancher** par consommateur (table
`pass_floor`, partagée avec le scellement et les relais d'audit), sous lequel tout est
livré. Le plancher ne monte qu'une fois terminées toutes les transactions qui auraient
pu détenir un numéro inférieur (`pg_snapshot_xmin` au-delà du `pg_snapshot_xmax` noté à
la passe précédente) : une transaction longue retarde sa montée, jamais une livraison.
Une passe par heure relit depuis zéro, pour le cas d'un numéro tiré avant que sa
transaction ait un identifiant. Les garanties ci-dessus sont inchangées (tests
`a_running_transaction_is_never_skipped`, `pass_floor`).

**Quatrième amendement (spec 016, tâche 6, 2026-10-21)** : l'ordre est garanti par **clé
d'ordre** et non plus seulement par domaine (REQ-016-14). Les clés d'un changeset sont ses
domaines (`domain:<uuid>`) et les clés de verrou de ses opérations de module (ADR 0026,
clause 14), que le store prend comme un verrou de domaine (classe de verrou nommé) : deux
changesets qui partagent une clé sont sérialisés et numérotent leurs événements dans
l'ordre de validation, comme deux changesets d'un même domaine. Colonne
`outbox.order_keys text[]` (migration 0050, remplie par lots depuis `domain_ids` pour
l'existant), exposée au relais (`Event::order_keys`) ; la forme CloudEvents de
l'événement ne change pas, et `domain_ids` reste lue par les consommateurs de la v0.1
jusqu'à son retrait. Le relais lui-même ne change pas : il livre par numéro croissant
(test `two_changesets_under_one_module_lock_are_delivered_in_order`). Les événements
des modules (tâche 016-8) passent par la même outbox, sous les clés de leur changeset ;
ceux d'une tâche de fond, sans changeset (`changeset_id` nul, migration 0052), sous la clé
`<module>:<tâche>`.

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

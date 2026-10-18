---
status: proposed
date: 2026-10-18
---

# 0023 — Tâches de fond sous bail dans PostgreSQL

## Context

Plusieurs travaux ne répondent à aucune requête : imports (004/REQ-004-28 à 30),
expiration des changesets en attente (004), réévaluation des valeurs attendues (003),
réécriture de l'indicateur `public` après une mise à jour des registres (002), purge des
clés d'idempotence (006). Chacun doit tourner sur une seule instance à la fois et
survivre à l'arrêt de celle qui l'exécute. Les instances sont sans état et PostgreSQL est
le seul composant à état (D9, D15). Décisions 006/H9, H16.

## Decision

Une table `job` (nom, échéance, période, bail, détenteur, battement). Chaque instance
réclame les tâches échues par `SELECT … FOR UPDATE SKIP LOCKED`, prend un bail de 30 s,
le renouvelle toutes les 10 s pendant l'exécution, puis fixe l'échéance suivante. Une
instance arrêtée laisse expirer son bail ; une autre reprend. Un import prêt est une
tâche ; son exécution reprend au premier lot non appliqué (REQ-004-29). Les travaux
propres à une instance (purges de caches, rechargement des JWKS) restent des minuteries
locales ; la purge de l'audit garde sa sous-commande pour une minuterie externe (012).

## Alternatives

- File de messages ou magasin partagé : un composant à état de plus, contraire à D9.
- Instance « chef » par verrou consultatif : une connexion monopolisée, bascule lente.
- Minuterie externe pour tout : dépendance d'exploitation pour des tâches dont la
  correction dépend (l'expiration des changesets en attente).

## Consequences

- Une tâche peut s'exécuter deux fois si un bail expire pendant un battement manqué :
  chaque tâche doit être idempotente (les lots d'import le sont, les autres aussi par
  construction).
- Latence de reprise bornée par la durée du bail plus l'intervalle de scrutation (5 s).

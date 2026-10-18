---
status: accepted
date: 2026-10-18
---

# 0021 — RBAC évalué en SQL : périmètres hérités et confinement des émetteurs

## Context

D36 et l'ADR 0010 fixent rôles, liaisons et périmètres hérités, et exigent que le
filtrage des listes de 10⁶ objets tienne les budgets de D14. Des prestataires
s'authentifient chez leur propre IdP (D13) : un IdP tiers ne doit jamais s'accorder plus
que les entités qui lui sont confiées. Décisions 005/K4, K5, K7, K13.

## Decision

Une décision part des liaisons valides du sujet (identité, groupes de son émetteur,
émetteur entier) pour la permission demandée et calcule une **visibilité** : périmètre
global, ensembles d'entités et de sites étendus à leurs descendants, domaines,
sous-arbres de préfixes, et **confinement** de l'émetteur (descendants des entités
rattachées, ou aucun pour un rattachement global). Un objet est autorisé si son
propriétaire résolu (`core.owner`, ADR 0014), son site, son domaine ou sa place dans
l'arbre d'adressage tombe dans la visibilité, et si son propriétaire est dans le
confinement. Le même prédicat, paramétré par tableaux, sert à `authorize_many` et au
filtrage des listes. Refus par défaut ; pas de règle de refus explicite.

## Alternatives

- Moteur externe (OPA, Cedar) : filtrage des listes coûteux (ADR 0010).
- Visibilité matérialisée par sujet : coût d'écriture à chaque changement de liaison ou
  d'arbre.
- Cache des liaisons par instance : incohérence après un retrait de droits.

## Consequences

- La résolution matérialisée des propriétés (ADR 0016) porte aussi le RBAC ; son coût
  sur les listes est mesuré au banc `bench-rbac` (005-16) : pour 1 000 liaisons et 50
  groupes, les budgets de D14 tiennent ; la résolution des liaisons à chaque requête
  coûte environ 10 ms, accepté sans cache (005/K15).
- Un objet sans propriétaire résolu n'est visible que d'un sujet non confiné.
- Les droits de l'auteur d'un changeset en attente sont revalidés avec un instantané de
  ses groupes.

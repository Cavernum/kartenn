---
status: accepted
date: 2026-10-14
---

# 0008 — Noyau minimal et propriétés typées

## Context

Le « vrai » nom d'un objet n'existe pas (nom mémorisable, FQDN, autre) ; la sécurité
exige des classifications héritées (environnements). Décisions D19, D21–D27 ; 002/A10 ;
003/P1–P8.

## Decision

- Un attribut n'entre dans le noyau que si un invariant du noyau ou une ancre RBAC en
  dépend ; tout le reste est une **propriété**.
- Les propriétés sont déclarées par les modules (code) ou les administrateurs (types
  intégrés), **typées** et validées strictement, en valeur unique ou en ensemble,
  héritables ou non le long d'**un** arbre (adressage, entités ou sites), avec
  remplacement, effacement explicite et obligation portée.
- Chaque propriété a sa permission ; elle s'applique aussi aux **effets indirects**
  (différentiel des valeurs résolues).
- Les déclarations sont bitemporelles ; une évolution incompatible est refusée.
- La 003 définit le contrat des sources de valeurs attendues ; le moteur de règles
  arrive en v0.2.

## Alternatives

- Attributs descriptifs dans le noyau : noyau qui grossit, interprétation figée.
- Champs libres non typés (JSON) : contraire à la validation stricte.
- Droit de l'objet suffisant : un prestataire pourrait se reclasser.

## Consequences

- La résolution des propriétés et le différentiel doivent être matérialisés et
  incrémentaux pour tenir les budgets.
- Le module Politique (v0.2) repose entièrement sur ce mécanisme.

---
status: accepted
date: 2026-10-22
---

# 0031 — Évaluations de flux matérialisées, maintenues dans la transaction ; état et validité dérivés

## Context

Le registre de flux (spec 011, D30) doit répondre vite à « quels flux sont non conformes
à telle date ? », montrer dans le différentiel d'un changeset les flux qu'il casse ou
guérit (REQ-011-13), et ouvrir ou fermer des anomalies au moment où une politique, une
classification ou un rattachement change (REQ-011-11, 12). Le verdict d'un flux dépend
de quatre sources qui changent indépendamment : les extrémités et leurs adresses (002,
009), les valeurs résolues des axes (003), la politique (010, ADR 0030) et la date. La
décision F7 de la 011 retient des verdicts matérialisés ; F4 et F10 (amendée D30) veulent
un état et une validité dérivés, sans date de fin obligatoire pour un flux conforme.

## Decision

1. **Évaluations matérialisées** : `flow_evaluation` (verdict par paire d'adresses et par
   segment de temps applicatif) et `flow_summary` (synthèse et issue par segment), tables
   versionnées (ADR 0017), comme `property_origin` (ADR 0016) est un état matérialisé.
2. **Maintenues dans la transaction** : la contribution du module de flux au différentiel
   (ADR 0026, clause 13) s'exécute après celles du noyau, de la 009 et de la 010 (ordre
   du DAG des modules) et réévalue les seuls flux touchés : écrits par le changeset ;
   dont une extrémité, une adresse développée, un rattachement ou l'origine d'une valeur
   d'axe est touché ; tous les flux vivants si la politique (hors règles d'inférence) est
   écrite. Le recalcul va de maintenant à la fin du flux, coupé aux bornes des versions
   lues ; seuls les segments qui diffèrent de l'état stocké sont réécrits. La section
   `flow.flows` nomme les flux qui basculent (`breaks`, `heals`, issue).
3. **État et validité dérivés** : l'état (demandé, programmé, actif, non conforme, dérogé,
   expiré, retiré, rejeté) se calcule de la date, de la synthèse et de la dérogation ;
   rien n'est stocké. Un flux conforme sans fin reste valide tant que la politique
   l'autorise (D30 amendée).
4. **Figé à l'enregistrement** : `flow_registration` garde, en ajout seul, l'évaluation
   au moment où le flux est enregistré ; elle n'est jamais recalculée.
5. **Concurrence** : verrou de module `flow` pour les opérations sur les flux (après
   domaines et `policy`) ; la réévaluation lancée par un changeset de domaine ne prend que
   les lignes des flux touchés (F13).

## Alternatives

- **Calcul à la lecture** : simple, mais aucun différentiel possible sans tout recalculer,
  et « non conformes au 1er mars » coûte une évaluation complète du registre.
- **Recalcul par tâche de fond** : découple l'écriture, mais le différentiel d'un changeset
  ne pourrait pas nommer les flux cassés avant l'application, et l'approbation `flow.breaks`
  (F8) perdrait son objet.
- **État stocké** (colonne `state`) : il dériverait de la date et demanderait une tâche
  pour chaque bascule ; refusé par F4.

## Consequences

- Une écriture de la politique réévalue tous les flux vivants : le coût est borné par le
  banc `bench-flow` (011-10, 10⁵ flux) ; le filtrage par service et valeurs d'axes reste
  une optimisation à mesurer.
- L'oracle de la tâche 011-5 (recalcul complet après des suites aléatoires) est la garde
  de cohérence ; il a montré que le différentiel des propriétés du noyau omet les héritiers
  d'une valeur datée dans le futur, que la réévaluation rattrape par l'index des origines.
- La restauration d'un flux ne rétablit que sa ligne ; ses tables dérivées sont recalculées
  par la réévaluation.
- Le budget de la réévaluation est « à mesurer » jusqu'au banc 011-10.

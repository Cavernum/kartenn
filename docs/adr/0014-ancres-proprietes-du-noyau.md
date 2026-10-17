---
status: accepted
date: 2026-10-14
---

# 0014 — L'entité propriétaire et le site sont des propriétés du noyau

## Context

L'entité propriétaire d'un préfixe est optionnelle et héritée de son parent puis du
domaine, avec une résolution expliquée (002/REQ-002-38) ; le site se rattache aux
réseaux (REQ-002-20). C'est l'ancre du RBAC. L'arbre d'adressage étant déduit de
l'inclusion CIDR (ADR 0007), insérer ou clore un préfixe change le propriétaire résolu
d'objets existants. La spec 003 fournit déjà héritage, explication, différentiel et
droit sur l'effet pour les propriétés.

## Decision

Le noyau déclare deux propriétés : `core.owner` (référence vers une entité, portée par
les domaines et les préfixes, héritable sur l'arbre d'adressage, obligatoire sur les
domaines) et `core.site` (référence vers un site, portée par les préfixes, non
héritable, interdite sur les conteneurs par une règle du noyau).

## Alternatives

- Colonnes du noyau avec une résolution propre : double mécanisme d'héritage à
  maintenir, et pas de contrôle des effets indirects : un prestataire pourrait
  s'approprier un sous-arbre en clôturant un préfixe intermédiaire.

## Consequences

- Une seule mécanique d'héritage et d'explication pour tout le système.
- Les manœuvres de structure qui changent un propriétaire exigent
  `prop.core.owner.write` sur les objets touchés.
- Le RBAC dépend des performances de la résolution matérialisée (spec 003).
- La spec système (`data.md`) devra être resynchronisée : propriétaire et site ne sont
  plus des attributs du préfixe.

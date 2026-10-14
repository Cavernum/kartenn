---
status: proposed
date: 2026-10-14
---

# 0016 — Résolution matérialisée et versionnée des propriétés

## Context

Les propriétés héritables (spec 003) doivent être filtrables sur 10⁶ objets en moins de
150 ms, et chaque changeset doit produire le différentiel des valeurs résolues et en
contrôler les droits (< 1 s jusqu'à 10⁴ objets touchés), y compris à une date passée
(`as_of`, ×2 au plus). L'arbre d'adressage est déduit (ADR 0007) ; le modèle est
bitemporel (ADR 0005) ; le RBAC lit le propriétaire résolu (ADR 0014).

Le [prototype 003](../spikes/003-resolution.md) a mesuré deux variantes sur 1,2×10⁶
objets : matérialiser la **valeur** résolue dépasse les budgets (3,2 s pour 1,6×10⁴
objets, 16 s pour 7,6×10⁵), parce que chaque changement réécrit la valeur sur tous les
héritiers ; matérialiser l'**origine** les tient.

## Decision

Nous matérialisons, pour chaque objet et chaque propriété **héritable**, l'**objet
d'origine** : celui qui porte l'affectation applicable (amendement 003/P9). La valeur se
lit par jointure sur l'affectation de l'origine. Les propriétés non héritables ne sont pas
matérialisées : l'affectation est la valeur.

La table est versionnée en temps applicatif et en temps système, et maintenue
incrémentalement par la couche service dans la transaction de chaque changeset, selon
quatre règles :

1. changement de valeur d'une affectation existante : aucune écriture ; le différentiel
   porte sur les objets dont l'origine est cet objet ;
2. nouvelle affectation sur P (ou insertion d'un préfixe P qui en porte) : seules les
   propriétés affectées sur P bougent ; cibles = objets de la portée de P dont l'origine
   est strictement plus haut que P ;
3. effacement, suppression d'affectation, clôture ou redimensionnement de P : les objets
   dont l'origine est P reçoivent l'ancêtre le plus proche portant une affectation ;
4. insertion d'un préfixe sans affectation : aucun changement.

Le filtrage par valeur part des origines du domaine qui portent la valeur, puis des
objets résolus vers elles (index `(property_id, origin_id)`).

## Alternatives

- Matérialiser la valeur : réécriture massive à chaque changement, hors budget, doublée
  par l'historique (prototype 003, variante V).
- Résolution à la lecture : hors budget pour le filtrage et le différentiel.
- Déclencheurs PostgreSQL : logique dupliquée en PL/pgSQL, différentiel peu accessible.
- Matérialisation sans temps système : `as_of` passé hors budget.

## Consequences

- Un changement de valeur ne coûte qu'un décompte (0,65 s pour 7,6×10⁵ objets mesurés).
- Le travail est proportionnel au nombre d'objets dont l'origine change, et non à la
  taille de la portée structurelle.
- La lecture d'une valeur coûte une jointure indexée (≈ 22 µs par objet en résolution
  expliquée).
- La justesse repose sur l'algorithme incrémental : un résolveur naïf sert d'oracle dans
  les tests de propriétés.
- Un changement d'origine très haut dans l'arbre (plus de 10⁵ objets) reste coûteux
  (≈ 30 µs par objet) : budget assoupli à < 60 s avec décompte préalable (003/P10).

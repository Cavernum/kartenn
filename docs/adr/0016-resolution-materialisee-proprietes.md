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

## Decision

Nous matérialisons les valeurs résolues dans une table versionnée en temps applicatif
et en temps système, maintenue incrémentalement par la couche service dans la
transaction de chaque changeset : détermination des objets touchés (directement et par
portée structurelle), segmentation temporelle, calcul ensembliste de l'ancêtre le plus
proche portant une affectation, comparaison avec l'état matérialisé (différentiel),
contrôles, écriture des segments modifiés.

## Alternatives

- Résolution à la lecture : hors budget pour le filtrage et le différentiel.
- Déclencheurs PostgreSQL : logique dupliquée en PL/pgSQL, différentiel peu accessible.
- Matérialisation sans temps système : `as_of` passé hors budget.

## Consequences

- Une donnée dérivée de volume important, à partitionner si besoin.
- La justesse repose sur l'algorithme incrémental : un résolveur naïf sert d'oracle dans
  les tests de propriétés.
- Un changement structurel très haut dans l'arbre coûte cher ; le budget de 10 s pour
  10⁶ objets en fixe la limite.

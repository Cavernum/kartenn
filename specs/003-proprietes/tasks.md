---
status: approved
updated: 2026-10-15
---

# 003 — Propriétés : tâches

Chaque tâche est un commit, laisse le build vert et nomme ce qu'elle implémente. Phases :
voir [`../README.md`](../README.md).

## P1 — Prototype de risque

- [x] 1. **Prototype de résolution matérialisée** (`docs/spikes/003-resolution.md`) sur le
  jeu de 002-5 avec 10 propriétés dont 3 héritables : volume de `property_resolved`
  (avec historique), résolution expliquée d'un objet, page filtrée par valeur résolue,
  différentiel d'une insertion de conteneur touchant 10⁴ puis 10⁶ objets ; requête
  ensembliste `LATERAL` — REQ-003-26 — après 002-6 et 004-1 — verify : rapport avec
  mesures ; conception amendée si un budget est intenable

## P2 — Domaine pur

- [x] 2. Trait `ValueType`, types intégrés `enum`, `text` (NFC, contrôle, regex),
  `int`, `bool`, `date` (RFC 3339), `ref` ; ensembles triés et dédoublonnés —
  REQ-003-7, 8, 9 — verify : tests unitaires et de propriétés (normalisation idempotente)
- [x] 3. Type `fqdn` (crate `idna`, UTS #46 non transitional, STD3, CheckHyphens,
  longueurs, Fake A-labels, forme IPv4) avec les cas de la
  [note](../../docs/reference/noms-dates-texte.md) — REQ-003-8 — verify : tests
  (`straße.example`, `café.example`, `xn--` invalide, `10.1.2.3`)
- [x] 4. Déclarations : descripteurs de module, clés et espaces, porteurs, héritage,
  obligations ; vérificateur de compatibilité (un cas par ligne du tableau) —
  REQ-003-1 à 6 — verify : tests unitaires
- [x] 5. Trait `InheritanceTree` et arbres `entity`, `site`, `addressing` (ancêtres en
  pur) ; résolveur naïf de référence (oracle) — REQ-003-11 à 14 — verify : tests
  unitaires de l'oracle sur des arbres construits à la main

## P3 — Stockage

- [x] 6. Migrations : `property_declaration`, `property_obligation`, `property_enum_value`,
  `property_assignment` (colonnes typées, exclusion par objet et propriété),
  `property_origin` (propriétés héritables seules, P9) ; tables courantes + historique
  (004) ; index de filtrage dont `(property_id, origin_object_id)` —
  REQ-003-5, 12, 13, 25 — après 004-5 — verify : tests d'intégration des contraintes
- [x] 7. Enregistrement des déclarations de module par migration ; déclarations
  d'administrateur par changeset (opérations dédiées) — REQ-003-1, 2 — après 004-7 —
  verify : tests d'intégration (`kartenn-service/tests/declarations.rs`) — synchronisation
  depuis le code (P12) ; obligations reportées à 003-10

## P4 — Pipeline

- [x] 8. Résolution matérialisée incrémentale de l'origine : les quatre règles de
  maintenance (design), cibles dont l'origine change, segmentation temporelle, calcul
  ensembliste, écriture des segments modifiés —
  REQ-003-11, 12, 13, 15 — verify : **tests de propriétés contre l'oracle** sur des suites
  aléatoires de changesets, y compris dans le temps (`kartenn-service/tests/resolution.rs`)
  — arbre d'adressage ; entités et sites avec 002-22 ; ciblage fin d'une affectation au
  banc 003-16
- [x] 9. Différentiel (décompte, détail paginé) et simulation — REQ-003-19, 20 — verify :
  tests d'intégration (insertion d'un conteneur au-dessus de 4 096 adresses)
  (`kartenn-service/tests/diff.rs`)
- [x] 10. Obligations portées et effacement explicite — REQ-003-13, 16 — verify : tests
  (`kartenn-service/tests/obligations.rs`) — portée « entité » avec 002-22
- [x] 11. Droits : permission directe et droit sur l'effet via `Authorizer::authorize_many`
  (implémentation de test), bascule en proposition si `…propose` (004) — REQ-003-17, 18 —
  verify : chaque ligne de la table des cas limites (`kartenn-service/tests/rights.rs`)
  — bascule en proposition avec 004-14
- [x] 12. Références : intégrité à la clôture d'une cible — REQ-003-10 — verify : tests
  (`kartenn-service/tests/references.rs`) — cibles du noyau d'adressage ; entités et
  sites avec 002-22
- [x] 13. Contrat `ExpectedValueSource`, modes `suggest`, `default`, `check` (source de
  test), anomalie d'écart — REQ-003-21 à 24 — verify : tests d'intégration
  (`kartenn-service/tests/expected.rs`) — job de réévaluation au changement de version
  avec 003-15

## P5 — Lectures et exploitation

- [x] 14. Explication de résolution (origine, chaîne, provenance, valeur remplacée) —
  REQ-003-14 — verify : tests (`kartenn-service/tests/explain.rs`, deux axes du temps)
- [x] 15. Filtrage par valeur résolue en partant des origines qui portent la valeur
  (égalité, appartenance, intervalle, présence/absence) à une date ; job de réévaluation des écarts quand une source change
  de version — REQ-003-24, 25 — verify : tests d'intégration ; plans d'exécution vérifiés
  (`kartenn-service/tests/filter.rs`, `expected.rs`) — date d'application seule ; date
  système avec l'API (006)

## P6 — Budgets et synchronisation

- [x] 16. Banc CI : résolution expliquée, page filtrée, différentiel de valeur sur 10⁶
  objets, changement d'origine sur 10⁴ et plus de 10⁵ objets (P10) — REQ-003-26 —
  verify : seuils bloquants (`kartenn-bench props`, script `bench-properties`) — seuil
  du cas ≤ 10⁴ porté à 2,5 s (P14) ; branchement CI avec 002-27
- [ ] 17. Synchroniser `docs/spec/`, ADR 0016 → accepté, statut `implemented` — verify :
  rapport spec-keeper sans écart

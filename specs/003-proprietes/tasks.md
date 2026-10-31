---
status: implemented
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
  — arbre d'adressage ; entités et sites avec la tâche 18 ; ciblage fin d'une affectation au
  banc 003-16
- [x] 9. Différentiel (décompte, détail paginé) et simulation — REQ-003-19, 20 — verify :
  tests d'intégration (insertion d'un conteneur au-dessus de 4 096 adresses)
  (`kartenn-service/tests/diff.rs`)
- [x] 10. Obligations portées et effacement explicite — REQ-003-13, 16 — verify : tests
  (`kartenn-service/tests/obligations.rs`) — portée « entité » avec la tâche 18
- [x] 11. Droits : permission directe et droit sur l'effet via `Authorizer::authorize_many`
  (implémentation de test), bascule en proposition si `…propose` (004) — REQ-003-17, 18 —
  verify : chaque ligne de la table des cas limites (`kartenn-service/tests/rights.rs`)
  — bascule en proposition faite en 004-14
- [x] 12. Références : intégrité à la clôture d'une cible — REQ-003-10 — verify : tests
  (`kartenn-service/tests/references.rs`) — cibles d'adressage, entités et sites
  (`anchors.rs`, 002-22)
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
- [x] 17. Synchroniser `docs/spec/`, ADR 0016 → accepté, statut `implemented` — verify :
  rapport spec-keeper sans écart (aucun écart bloquant ; les écarts mineurs relevés, dont
  un test manquant de REQ-003-5, sont corrigés) — catalogue des permissions avec la 005

## P7 — Arbres d'entités et de sites (P15)

- [x] 18. Origines le long des arbres `entity` et `site` (parents versionnés, cibles :
  descendants d'un objet déplacé, segmentation sur les versions des parents) ; portée
  « entité » des obligations (objets dont le propriétaire résolu est l'entité ou une
  descendante) — REQ-003-11, 16 — verify : oracle de la tâche 5 sur des suites
  aléatoires de déplacements (`kartenn-service/tests/anchor_trees.rs`, entités et sites)
- [x] 19. Propriétés sensibles : colonne versionnée, opération `SetSensitivity`, clés
  sensibles dans la lecture d'un objet — REQ-003-27 — verify : test de bout en bout avec
  la tâche 012-7 (amendement du 2026-10-17, P17)

## P8 — Régression des budgets

- [ ] 20. Budgets de REQ-003-26 repassés sous leurs seuils. Au banc pleine échelle du
  2026-10-30 (graine 42, main 5d2da650), trois mesures dépassent :
  - retrait des racines de 3 domaines (P10) : 1 197,8 s pour 60 s, contre 13,8 s le
    2026-10-15 ;
  - changement de valeur de toutes les racines : 10,72 s pour 10 s, contre 9,1 s ;
  - résolution groupée 512 × 6 (N9, 016-4) : p95 de 38,2 ms pour 30 ms, contre 16,3 ms
    au banc réduit du 2026-10-21.

  Trouver la cause de chaque écart (régression de code, plan de requête, ou budget
  jamais tenu à cette échelle), la corriger, puis écrire la cause et les nouvelles
  mesures dans le design (§ « Banc REQ-003-26 », tableau des mesures) — REQ-003-26 —
  verify : `K_BENCH_SCALE=full bench-properties` sans dépassement, lancé par la session
  principale sur une machine sans autre agent ; si la cause est un plan de requête, un
  test qui le fixe (correctifs voisins dc8d52f4, 9b3e83ca : plan générique après cinq
  exécutions préparées)
  - où : mesures de `crates/kartenn-bench/src/props.rs` (l. 298, 393, 465, 496) ;
    écriture des origines et des valeurs dans `crates/kartenn-store-pg/src/write.rs`
    (`objects_without_value` l. 2224 et voisins) ; `crates/kartenn-service/src/resolution.rs`
    (l. 652), `declarations.rs` (l. 278) ; lecture groupée N9 de la 016 ; bissection sur
    `git log --since=2026-10-15 -- crates/kartenn-store-pg crates/kartenn-service`
    (suspects : 003-18 arbres `entity` et `site`, 016, 008-12, 024 « undo »)
  - méthode : itérer au banc réduit (`bench-properties`, échelle par défaut, un seul à
    la fois, `K_BENCH_STALL` laissé à 20 min) et par `EXPLAIN (ANALYZE, BUFFERS)` sur la
    base du banc ; vérifier d'abord que le banc réduit reproduit l'écart en proportion.
    Aucun banc pleine échelle par l'agent.
  - dépend de : —

---
status: approved
updated: 2026-10-14
---

# 003 — Propriétés : tâches

Chaque tâche est un commit, laisse le build vert et nomme ce qu'elle implémente. Phases :
voir [`../README.md`](../README.md).

## P1 — Prototype de risque

- [ ] 1. **Prototype de résolution matérialisée** (`docs/spikes/003-resolution.md`) sur le
  jeu de 002-5 avec 10 propriétés dont 3 héritables : volume de `property_resolved`
  (avec historique), résolution expliquée d'un objet, page filtrée par valeur résolue,
  différentiel d'une insertion de conteneur touchant 10⁴ puis 10⁶ objets ; requête
  ensembliste `LATERAL` — REQ-003-26 — après 002-6 et 004-1 — verify : rapport avec
  mesures ; conception amendée si un budget est intenable

## P2 — Domaine pur

- [ ] 2. Trait `ValueType`, types intégrés `enum`, `text` (NFC, contrôle, regex),
  `int`, `bool`, `date` (RFC 3339), `ref` ; ensembles triés et dédoublonnés —
  REQ-003-7, 8, 9 — verify : tests unitaires et de propriétés (normalisation idempotente)
- [ ] 3. Type `fqdn` (crate `idna`, UTS #46 non transitional, STD3, CheckHyphens,
  longueurs, Fake A-labels, forme IPv4) avec les cas de la
  [note](../../docs/reference/noms-dates-texte.md) — REQ-003-8 — verify : tests
  (`straße.example`, `café.example`, `xn--` invalide, `10.1.2.3`)
- [ ] 4. Déclarations : descripteurs de module, clés et espaces, porteurs, héritage,
  obligations ; vérificateur de compatibilité (un cas par ligne du tableau) —
  REQ-003-1 à 6 — verify : tests unitaires
- [ ] 5. Trait `InheritanceTree` et arbres `entity`, `site`, `addressing` (ancêtres en
  pur) ; résolveur naïf de référence (oracle) — REQ-003-11 à 14 — verify : tests
  unitaires de l'oracle sur des arbres construits à la main

## P3 — Stockage

- [ ] 6. Migrations : `property_declaration`, `property_obligation`, `property_enum_value`,
  `property_assignment` (colonnes typées, exclusion par objet et propriété),
  `property_resolved` ; tables courantes + historique (004) ; index de filtrage —
  REQ-003-5, 12, 13, 25 — après 004-5 — verify : tests d'intégration des contraintes
- [ ] 7. Enregistrement des déclarations de module par migration ; déclarations
  d'administrateur par changeset (opérations dédiées) — REQ-003-1, 2 — après 004-7 —
  verify : tests d'intégration

## P4 — Pipeline

- [ ] 8. Résolution matérialisée incrémentale : objets touchés, portée structurelle,
  segmentation temporelle, calcul ensembliste, écriture des segments modifiés —
  REQ-003-11, 12, 13, 15 — verify : **tests de propriétés contre l'oracle** sur des suites
  aléatoires de changesets, y compris dans le temps
- [ ] 9. Différentiel (décompte, détail paginé) et simulation — REQ-003-19, 20 — verify :
  tests d'intégration (insertion d'un conteneur au-dessus de 4 096 adresses)
- [ ] 10. Obligations portées et effacement explicite — REQ-003-13, 16 — verify : tests
- [ ] 11. Droits : permission directe et droit sur l'effet via `Authorizer::authorize_many`
  (implémentation de test), bascule en proposition si `…propose` (004) — REQ-003-17, 18 —
  verify : chaque ligne de la table des cas limites
- [ ] 12. Références : intégrité à la clôture d'une cible — REQ-003-10 — verify : tests
- [ ] 13. Contrat `ExpectedValueSource`, modes `suggest`, `default`, `check` (source de
  test), anomalie d'écart — REQ-003-21 à 24 — verify : tests d'intégration

## P5 — Lectures et exploitation

- [ ] 14. Explication de résolution (origine, chaîne, provenance, valeur remplacée) —
  REQ-003-14 — verify : tests
- [ ] 15. Filtrage par valeur résolue (égalité, appartenance, intervalle,
  présence/absence) à une date ; job de réévaluation des écarts quand une source change
  de version — REQ-003-24, 25 — verify : tests d'intégration ; plans d'exécution vérifiés

## P6 — Budgets et synchronisation

- [ ] 16. Banc CI : résolution expliquée, page filtrée, différentiel 10⁴ et 10⁶ — REQ-003-26
  — verify : seuils bloquants
- [ ] 17. Synchroniser `docs/spec/`, ADR 0016 → accepté, statut `implemented` — verify :
  rapport spec-keeper sans écart

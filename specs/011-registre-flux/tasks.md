---
status: draft
updated: 2026-10-21
---

# 011 — Registre de flux : tâches

Chaque tâche est un commit, laisse le build vert et nomme ce qu'elle implémente. La 011
suit la 010 (v0.2, D31) : elle commence quand les tâches 010-1 à 010-13 sont faites ;
elle s'appuie sur N21, N22 (ADR 0026, fournis) et M7 (REQ-009-23), et ajoute N38 et N39
(design, § Exigences). Chaque tâche porte `dépend de :`.

## P1 — Domaine et stockage

- [x] 1. Crate `kartenn-flow` : type `Flow` (sorte, dossier, références externes),
  extrémités, dérogation, validation ; état dérivé et validité dérivée — REQ-011-1, 2, 4,
  5, 9, 20, 24 — verify : un test par état de REQ-011-5 ; cas refusés de REQ-011-1, 9,
  20
  - où : modèle de `kartenn-policy` (010-3) ; crate à l'image de `crates/kartenn-vlan`
  - dépend de : —
  - fait : `crates/kartenn-flow` (`model.rs` : `Flow`, `Endpoint`, `Facts`, `check`,
    `check_update`, `Waiver`, `check_waiver`, `check_review`, `add_months` ;
    `state.rs` : `summarize`, `state_at`, `holds` ; `rules.rs` : les règles du design et
    `flow.field.invalid`, `flow.update.identity`) ; 20 tests unitaires (un par état de
    REQ-011-5, refus de REQ-011-1, 2, 4, 9, 20, séparation et fin d'instruction de
    REQ-011-22).
  - choix : mois calendaires en UTC, jour borné à la fin du mois ; un flux sans paire
    d'adresses est `denied` et `noncompliant` ; `allowed` sous réserve compte comme
    autorisant dans la synthèse (l'issue porte la réserve) ; deux règles ajoutées au
    tableau du design : `flow.field.invalid` (justification, identifiant externe) et
    `flow.update.identity` (REQ-011-4).
- [ ] 2. Migrations du module : `flow`, `flow_waiver`, `flow_endpoint_address`,
  `flow_evaluation`, `flow_summary`, `flow_registration`, `flow_rejection`,
  `flow_notice` versionnées — REQ-011-16, 23 — verify : tests de contraintes ; lecture
  `as_of` passé
  - où : migrations de `crates/kartenn-vlan` (forme), forme de `property_origin`
    (`crates/kartenn-store-pg/migrations/0011_properties.sql`)
  - dépend de : 1
  - fait : `crates/kartenn-flow/migrations/0001_flow.sql` (schéma `flow`), module `Flows`
    (`src/module.rs` : type `flow.flow` ancré sur l'entité, permissions de REQ-011-17,
    règles, migration) ; tests `crates/kartenn-flow/tests/it/schema.rs` (cible `flow_it`) :
    contraintes de `flow` (référence, justification, sorte, plage), de `flow_waiver` (fin,
    motif, risque), de `flow_summary` (accord synthèse et décomptes) et de
    `flow_evaluation` (réserve), lecture `as_of` passé dans `flow_summary_history`,
    `flow_registration` jamais réécrite par le rôle de l'application.
  - choix : extrémités et service sans clé étrangère (polymorphes, contrôlés par les
    règles du module) ; `flow_waiver`, `flow_endpoint_address`, `flow_evaluation`,
    `flow_summary` versionnées ; `flow_registration`, `flow_rejection`, `flow_notice`
    en ajout seul (`SELECT, INSERT`) ; `flow_notice.kind` distingue les échéances du flux
    et celles de la dérogation ; le module n'est pas encore monté par `kartenn-server`
    (opérations et gestionnaire en 011-3).

- [ ] 3. Opérations `flow.flow.*`, verrou `flow`, règles du design ; triage (proposition
  forcée et permission de validation, N39, amendement additif de la 004 et du contrat) ;
  instruction (`approve_review`, séparation des tâches) ; dérogation — REQ-011-1 à 4,
  8 à 10, 17, 21, 22 — verify : cas d'acceptation de REQ-011-8, 9, 21, 22 ; demande
  par `flow.flow.request` vers un objet illisible acceptée ; écriture directe par
  `flow.flow.write` vers un objet illisible → 404
  - où : opérations de `crates/kartenn-vlan`, `crates/kartenn-service/src/review.rs`
    (propositions, REQ-004-41, 42), `rights.rs`
  - dépend de : 2
- [ ] 4. Références des extrémités : clôture refusée ou cascade — REQ-011-3 — verify :
  clôture d'une adresse citée refusée ; cascade clôt le flux
  - où : références de module (ADR 0026 clause 12, tâche 008-7)
  - dépend de : 3
- [ ] 5. Réévaluation dans la transaction (flux touchés par les sections 003, 009, 010,
  recalcul segmenté avec dossier, écriture des seuls segments changés), évaluation figée
  à l'enregistrement, différentiel des flux — REQ-011-11, 12, 13, 23 — verify : cas
  d'acceptation de REQ-011-11 et 13 ; oracle contre recalcul complet sur suites
  aléatoires ; changement de règle d'inférence sans réévaluation
  - où : `Module::diff` (tâche 008-19), `kartenn-policy/src/evaluate.rs`
  - dépend de : 3
- [ ] 6. Déclencheurs `flow.created`, `flow.breaks` (politique par défaut à
  l'installation), `flow.waiver` ; anomalies `flow.verdict.denied` ouvertes et fermées ;
  crochet de rejet (N38) et `flow_rejection` — REQ-011-6, 12, 25 — verify : politique
  `flow.breaks ≥ 1` installée et active ; rejet motivé enregistré
  - où : déclencheurs de module (tâche 010-9), anomalies (ADR 0026 clause 7)
  - dépend de : 5

## P3 — Lectures, échéances, événements, API

- [ ] 7. Filtres, flux d'un objet, évaluations datées, non-conformités — REQ-011-14, 15,
  16, 25 — verify : un test par filtre ; sous-arbre d'un préfixe ; un demandeur voit ses
  non-conformités et pas celles d'un autre
  - où : `crates/kartenn-store-pg/src/filter.rs`
  - dépend de : 5, 6
- [ ] 8. Tâche `flow.expiry` (flux datés, dérogations) et événements `expiring`,
  `expired`, `outcome_changed`, `closed` — REQ-011-7 — verify : J-30, J-7, terme ;
  prolongation sans doublon ; flux sans fin jamais notifié ; événement d'issue émis à la
  bascule
  - où : tâches de fond et événements de module (tâches 010-8, 010-14)
  - dépend de : 5
- [ ] 9. Routes `/v1/flows`, `/v1/flows/{id}/evaluations`, `/v1/flow-nonconformities`,
  `/v1/{collection}/{id}/flows`, contrat OpenAPI — REQ-011-18 — verify : contrat
  comparé ; tests de routes ; `POST /v1/flows` rend l'évaluation et l'état
  - où : routeur de module (tâche 008-8), `crates/kartenn-rest/src/contract.rs`
  - dépend de : 7

## P4 — Budgets et synchronisation

- [ ] 10. Banc `bench-flow` (10⁵ flux) bloquant en CI — REQ-011-19 — verify : budgets
  tenus
  - où : `crates/kartenn-bench`
  - dépend de : 9
- [ ] 11. Synchronisation de `docs/spec/`, ADR proposée → acceptée, amendement de l'ADR
  0026 (N38, N39)
  - où : skill `tech-specs`, mode sync
  - dépend de : 10

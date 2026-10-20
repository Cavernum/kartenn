---
status: draft
updated: 2026-10-20
---

# 011 — Registre de flux : tâches

Chaque tâche est un commit, laisse le build vert et nomme ce qu'elle implémente. La 011
suit la 010 (v0.2, D31) : elle commence quand les tâches 010-1 à 010-12 sont faites et
que le contrat de module offre N21, N22 et la 009 M7 (design, § Exigences).

## P1 — Domaine et stockage

- [ ] 1. Crate `kartenn-flow` : type `Flow`, extrémités, dérogation, validation ; état
  dérivé — REQ-011-1, 2, 4, 5, 9 — verify : un test par état de REQ-011-5 ; cas refusés
  de REQ-011-1
  - où : modèle de `kartenn-policy` (010-3)
- [ ] 2. Migrations : `flow`, `flow_endpoint_address`, `flow_verdict`, `flow_summary`,
  `flow_notice`, `flow_changeset_diff` versionnées — REQ-011-16 — verify : tests de
  contraintes ; lecture `as_of` passé
  - où : `crates/kartenn-store-pg/migrations/0011_properties.sql`, forme de
    `property_origin`

## P2 — Pipeline

- [ ] 3. Opérations `flow.flow.*`, verrou `flow`, règles du design, conformité à
  l'enregistrement et dérogation — REQ-011-1 à 4, 8 à 10, 17 — verify : cas
  d'acceptation de REQ-011-8, 9 ; création vers un objet illisible → 404
  - où : `crates/kartenn-service/src/operations.rs`, `rights.rs`
- [ ] 4. `dependents()` des extrémités : clôture refusée ou cascade — REQ-011-3 —
  verify : clôture d'une adresse citée refusée ; cascade clôt le flux
  - où : `crates/kartenn-service/src/cascade.rs`
- [ ] 5. Réévaluation dans la transaction (flux touchés, recalcul segmenté, écriture
  des seuls segments changés) et différentiel des flux — REQ-011-11, 12, 13 — verify :
  cas d'acceptation de REQ-011-11 et 13 ; oracle contre recalcul complet sur suites
  aléatoires
  - où : contribution au différentiel (N4, N21), `kartenn-policy/src/evaluate.rs`
- [ ] 6. Déclencheurs `flow.created`, `flow.breaks` ; anomalies `flow.verdict.denied`
  ouvertes et fermées — REQ-011-6, 12 — verify : politique `flow.breaks ≥ 1` met en
  attente
  - où : `crates/kartenn-service/src/review.rs`, anomalies (N11)

## P3 — Lectures, échéances, API

- [ ] 7. Filtres, flux d'un objet, verdicts datés — REQ-011-14, 15, 16 — verify : un
  test par filtre ; sous-arbre d'un préfixe
  - où : `crates/kartenn-store-pg/src/filter.rs`
- [ ] 8. Tâche `flow.expiry` et événements — REQ-011-7 — verify : J-30, J-7, terme ;
  prolongation sans doublon
  - où : `crates/kartenn-service/src/import.rs` (tâche sous bail)
- [ ] 9. Routes `/v1/flows`, `/v1/flows/{id}/verdicts`, `/v1/{collection}/{id}/flows`,
  contrat OpenAPI — REQ-011-18 — verify : contrat comparé ; tests de routes
  - où : `crates/kartenn-rest/src/resources.rs`, `contract.rs`

## P4 — Budgets et synchronisation

- [ ] 10. Banc `bench-flow` (10⁵ flux) bloquant en CI — REQ-011-19 — verify : budgets
  tenus
  - où : `crates/kartenn-bench`
- [ ] 11. Synchronisation de `docs/spec/`, ADR proposée → acceptée
  - où : skill `tech-specs`, mode sync

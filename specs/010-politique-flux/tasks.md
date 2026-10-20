---
status: draft
updated: 2026-10-20
---

# 010 — Politique de flux : tâches

Chaque tâche est un commit, laisse le build vert et nomme ce qu'elle implémente. La 010
est en v0.2 (D31) : elle ne commence qu'après la validation de cette spec et la
livraison, dans le contrat de module (ADR 0026), des points N1 à N16 du design
(§ Exigences envers le noyau). Les tâches de l'inférence s'ajouteront après la session
dédiée ([`inference-questions.md`](inference-questions.md)).

## P0 — Prérequis

- [ ] 1. **Prototype** des budgets (REQ-010-28) sur le jeu de 10⁶ objets : instantané
  de 10⁴ cellules, lecture groupée de 512 environnements résolus (N9), machine × machine
  16 × 16, différentiel d'un garde-fou sans environnement ; résultats dans
  `docs/spikes/010-verdicts.md` — verify : rapport avec mesures ; conception amendée si
  un budget est intenable
  - où : jeu de `kartenn-bench` (`crates/kartenn-bench`), modèle de
    `docs/spikes/003-resolution.md`
- [ ] 2. Lecture groupée des valeurs résolues avec explication (N9) — verify : 512
  objets < 30 ms au banc ; mêmes résultats que la lecture unitaire
  - où : `crates/kartenn-store-pg/src/explain.rs`, `crates/kartenn-service/src/reads.rs`

## P1 — Domaine pur

- [ ] 3. Crate `kartenn-policy` : types `Environment`, `FlowType`, `Service`,
  `ServiceEntry` (transports, ports, ICMP, protocole), `Guardrail`, `MatrixCell` avec
  validation — REQ-010-4, 5, 6, 7, 8, 9 — verify : un test par cas de REQ-010-5
  - où : modèle de `crates/kartenn-core/src/values.rs` (validation), note
    `docs/reference/services-transports.md`
- [ ] 4. `PolicySnapshot` et `evaluate` (non classifié, garde-fous, matrice, refus par
  défaut, élément décisif) — REQ-010-10, 11, 12 — verify : cas d'acceptation de
  REQ-010-11 ; proptest contre l'oracle (le refus l'emporte ; monotonie des cellules et
  des garde-fous)
  - où : `kartenn-policy/src/snapshot.rs`, `src/evaluate.rs` ; tests à l'image de
    `crates/kartenn-core/src/inheritance.rs`
- [ ] 5. Différentiel de politique pur (candidats, verdicts de principe avant et après)
  — REQ-010-18 — verify : égal aux basculements du produit cartésien sur politiques
  aléatoires
  - où : `kartenn-policy/src/diff.rs`

## P2 — Stockage et pipeline

- [ ] 6. Migrations du module : tables `policy_*` versionnées, contraintes, index GiST
  des ports, `policy_revision` — REQ-010-26 — verify : tests de contraintes (doublon de
  cellule, sélecteur, `CHECK` par transport)
  - où : `crates/kartenn-store-pg/migrations/0011_properties.sql` (forme versionnée),
    mécanisme de migrations de module (ADR 0026)
- [ ] 7. Déclarations `policy.environment` et `policy.declared_environment` ; opération
  `policy.environment.create` (valeur de liste + attributs) — REQ-010-1, 2, 3 — verify :
  cas d'acceptation de REQ-010-1 et 2
  - où : descripteurs de `crates/kartenn-service/src/declarations.rs`
- [ ] 8. Opérations `policy.*` (types, services, garde-fous, cellules, rôles exemptés),
  règles du tableau du design, verrou `policy`, événements, révision — REQ-010-4 à 9,
  26 — verify : chaque règle a un test ; annulation d'un changeset de matrice
  - où : `crates/kartenn-service/src/operations.rs`, `pipeline.rs`, `undo.rs`
- [ ] 9. Contribution au différentiel et déclencheurs `policy.opens`, `policy.closes` —
  REQ-010-18, 19, 20 — verify : cas d'acceptation de REQ-010-18 en simulation et à
  l'application ; politique d'approbation `policy.opens ≥ 1` met en attente
  - où : `crates/kartenn-store-pg/src/diff.rs` (forme de `changeset_diff`),
    `crates/kartenn-service/src/review.rs`
- [ ] 10. Obligation par domaine modifiable à l'exécution — REQ-010-21, 22 — verify :
  cas d'acceptation de REQ-010-21
  - où : `crates/kartenn-core/src/properties.rs` (`ObligationScope`, `check_evolution`)

## P3 — Lectures et requêtes

- [ ] 11. Lectures datées des objets de politique et cache de l'instantané (révision,
  LRU des instantanés passés) — REQ-010-16, 26 — verify : cas d'acceptation de
  REQ-010-16
  - où : `crates/kartenn-service/src/reads.rs`
- [ ] 12. Requête de conformité : extrémités (adresse, préfixe, interface, machine),
  objet (service, port, type), synthèse, bornes, visibilité — REQ-010-13, 14, 15, 25 —
  verify : cas d'acceptation de REQ-010-13, 14 ; extrémité invisible → 404
  - où : port de lecture de la 009 (M4), `crates/kartenn-service/src/rights.rs`
- [ ] 13. Cohérence des machines — REQ-010-17 — verify : cas d'acceptation ; anomalie
  fermée quand la divergence disparaît
  - où : règles de module (N3), anomalies (N11)
- [ ] 14. Rapport d'audit (tâche de fond, rubriques, pagination, purge) — REQ-010-23 —
  verify : un test par rubrique
  - où : `crates/kartenn-service/src/import.rs` (tâche sous bail),
    `crates/kartenn-store-pg/migrations/0039_job.sql`

## P4 — API, budgets, synchronisation

- [ ] 15. Routes `/v1/policy/*`, `QUERY` et `GET` des verdicts, rapports, contrat
  OpenAPI — REQ-010-24, 27 — verify : contrat comparé ; tests de routes
  - où : `crates/kartenn-rest/src/resources.rs`, `filter.rs`, `contract.rs`
- [ ] 16. Banc `bench-policy` bloquant en CI — REQ-010-28 — verify : budgets tenus
  - où : `crates/kartenn-bench`
- [ ] 17. Synchronisation de `docs/spec/` (spec-keeper), ADR proposées → acceptées
  - où : skill `tech-specs`, mode sync

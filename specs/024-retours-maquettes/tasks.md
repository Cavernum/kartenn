---
status: draft
updated: 2026-10-24
---

# 024 — Retours sur les maquettes : tâches

Brouillon du 2026-10-24, à valider avec les exigences et la conception. Phases : P0
(permissions) avant tout ; P1 (brouillons, statistiques, simulation) en parallèle ; P2
(révision des specs 020, 021) puis P3 (maquettes) ; P4 clôture.

## P0 — Deux chemins de droits

- [ ] 1. Catalogue `edit`/`apply`/`apply_own`, migration des rôles par changeset système,
  `write`, `propose` et `core.changeset.approve` retirés — REQ-024-9 — verify :
  `k-check` ; aucune occurrence des anciens noms hors migrations et audit (test du type
  018/REQ-018-6)
  - où : `docs/spec/permissions.md` ; catalogues des modules (`crates/kartenn-*/src/`,
    `registry.rs`) ; migration `crates/kartenn-store-pg/migrations/0063_permission_verbs.sql`
    sur le modèle de `0058_permission_names.sql` ; test voisin : celui de 018-1 à 4
- [ ] 2. Soumission toujours en attente sous `edit` ; validation par `apply` (autre) ou
  `apply_own` (auteur) ; politique qui retient → `changeset.approval.self` —
  REQ-024-10, 11, 12 — verify : matrice auteur/autre × apply/apply_own × politique
  - où : `crates/kartenn-service/src/pipeline.rs`, `rights.rs`,
    `review.rs::validate_pending_with` ; design « B »
- [ ] 3. `?apply=own` sur `POST /v1/changesets` (une transaction, deux audits, 202 en
  repli) ; imports et annulation suivent la règle ; aides de test — REQ-024-13 — verify :
  test de repli 202, audit des deux événements
  - où : `crates/kartenn-rest/src/changesets.rs` ; `crates/kartenn-service/src/import.rs`,
    `undo.rs` ; aides de `crates/kartenn-testing`
- [ ] 4. Amendements de texte : 004 (C1, REQ-004-21, 41, 42 renvoient à la 024), 005,
  018, `docs/spec/` (règles, permissions) ; client Python et CLI (noms, `apply=own`) —
  verify : `k-py-check`, `spec-keeper` sur la 004
  - où : `specs/004-changesets-bitemporel/requirements.md` § D ;
    `clients/python/kartenn/`, `clients/python/kartenn-cli/`

## P1 — Serveur (en parallèle, après P0)

- [ ] 5. Table `draft`, routes, 404 d'un autre, If-Match, limites, rétention annoncée,
  reprise par `core.draft.admin` (lecture, suppression, attribution, toutes auditées) —
  REQ-024-1, 2, 3, 7, 7a — verify : tests d'intégration par route ; audit d'une lecture
  par l'administrateur
  - où : `crates/kartenn-store-pg/migrations/0062_drafts.sql` ;
    `crates/kartenn-rest/src/drafts.rs` (modèle : `changesets.rs`) ; job dans
    `crates/kartenn-server` (jobs) ; design « A »
- [ ] 6. Simulation et soumission d'un brouillon (`?apply=own`), audit, SSE `draft`
  filtré par auteur — REQ-024-4, 5, 6 — verify : brouillon gardé en 422, supprimé en
  201/202 ; flux d'un autre sans l'événement
  - où : `drafts.rs` ; distributeur de la 022 (`crates/kartenn-rest/src/events/hub.rs`,
    `stream.rs` : contrôle des droits à la remise)
- [ ] 7. `GET /v1/stats`, port `StatsContributor` des modules, cache par empreinte des
  droits, SSE `stats` regroupé, scénario de banc — REQ-024-14, 15, 16 — verify : tests
  de filtrage par les droits ; `bench-core` scénario `stats` p95 ≤ 200 ms (lancé par
  l'agent principal)
  - où : `crates/kartenn-rest/src/stats.rs` ; `crates/kartenn-service/src/reads.rs`
    (prédicats de lecture) ; `crates/kartenn-bench`
- [ ] 8. `?simulate=true` sur toute route d'écriture, `x-kartenn.simulate`,
  `simulate.unsupported` ; xfail `--dry-run` de la CLI levé — REQ-024-17 — verify :
  test générique sur toutes les routes d'écriture de l'OpenAPI ; `k-py-e2e` sans xfail
  - où : `crates/kartenn-rest/src/kit.rs`, `resources.rs`, `cycle.rs` ;
    `clients/python/kartenn-cli/` (dry-run) ; `clients/python/e2e/`

## P2 — Révision des specs de l'interface

- [ ] 9. Report de REQ-024-18 à 28 dans les designs et exigences des specs 020 et 021
  (cadre, inventaire des composants, régression visuelle, écrans) ; SDK : brouillons
  serveur, statistiques ; section « Structure avant style » du skill `kartenn-ui` —
  verify : `spec-keeper` sur 020 et 021
  - où : `specs/020-systeme-de-design/design.md` (« Inventaire », « Architecture CSS »,
    « Régression visuelle ») ; `specs/021-shell-v2/design.md` (« Cadre », « Écrans ») ;
    `.claude/skills/kartenn-ui/SKILL.md`

## P3 — Maquettes révisées

- [ ] 10. Composants : `SidePanel`, `CommitForm`, `SplitButton`, `Toolbar` repliable,
  `Tooltip` des actions grisées, `MatrixGrid` ; `DataGrid` en grille unique (subgrid),
  tri, filtre, squelettes animés — REQ-024-19, 20, 23, 25, 26 — verify : catalogue,
  captures aux quatre largeurs, axe
  - où : `web/v2/ui/src/components/` ; prototype de référence
    `/tmp/…/scratchpad/proto-panneaux/PanneauxProto.vue` (hors dépôt, à reprendre
    proprement) ; design « E »
- [ ] 11. Maquettes E1 à E10 reprises, maquette de l'accueil ajoutée, captures 1440,
  1280, 1024, 768 px — REQ-024-21, 22, 24, 27, 28, 29 — verify : `k-web2-check`,
  revues `ui-reviewer` et `a11y-auditor`
  - où : `web/v2/shell/src/_ui/ecrans/` ; `registry.ts` ; `e2e/visual/ecrans.spec.ts`
- [ ] 12. Validation des maquettes révisées par l'utilisateur (reprend la tâche 15 de la
  020) — REQ-024-29

## P4 — Clôture

- [ ] 13. Synchronisation `docs/spec/` (API : brouillons, statistiques, simulation ;
  permissions ; invariants), index `specs/README.md` — verify : `spec-keeper`

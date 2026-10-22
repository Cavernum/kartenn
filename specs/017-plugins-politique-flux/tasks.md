---
status: draft
updated: 2026-10-22
---

# 017 — Plugins Politique et Flux : tâches

Chaque tâche est un commit, laisse le build vert (`k-web-check`, et `k-check` pour la
tâche 1) et nomme ce qu'elle implémente. « verify » = test qui prouve la tâche ; « où » =
fichiers et symboles à toucher ou à imiter ; « dépend de » = tâches à finir avant. Rien
ne commence avant l'arbitrage des questions Q-017-1 à 7 ([`decisions.md`](decisions.md)) ;
les tâches qui dépendent d'une option le disent.

Parallélisme : 1 et 2 d'abord ; puis P2 (politique) et P3 (flux) avancent en parallèle ;
dans P2, 3, 4 et 8 sont indépendantes, 6 suit 5 ; dans P3, 11 suit 10, 12 et 13 suivent
10. Pas plus de trois sous-agents d'implémentation à la fois (CLAUDE.md).

## P1 — Squelettes

- [ ] 1. Plugin `policy` : paquet `@kartenn/plugin-policy`, manifeste (vues et action du
  design, permissions), catalogues FR/EN, éléments vides qui s'enregistrent, projet
  Vitest, construction par `serve.sh`, test serveur de publication — REQ-017-1, 3 —
  verify : `test/policy.test.ts` (clés FR = EN, éléments définis) ;
  `the_policy_plugin_is_published_only_with_its_module`
  où : `web/plugins/machines/{package.json,public/manifest.json,vite.config.ts}` ;
  `web/vitest.config.ts` ; `web/e2e/serve.sh` (filtre `--filter @kartenn/plugin-…`) ;
  `crates/kartenn-rest/tests/it/plugins.rs` (test du plugin VLAN à copier) ; design
  « Plugin `policy` »
  dépend de : arbitrage Q-017-1 à 7

- [ ] 2. Plugin `flow` : même squelette (`@kartenn/plugin-flow`, vues, onglet, action) —
  REQ-017-2, 3 — verify : `test/flow.test.ts` ;
  `the_flow_plugin_is_published_only_with_its_module` ; le bundle de `flow` n'importe
  rien de `web/plugins/policy/` (test sur le graphe de `vite build`)
  où : comme 1 ; design « Plugin `flow` »
  dépend de : 1

## P2 — Politique

- [ ] 3. Règles et motifs : liste filtrée, fiche, historique, formulaire (sélecteurs par
  axe, même valeur, flux, effet, motifs requis, textes), création, modification,
  clôture — REQ-017-4, 5 — verify : Vitest (texte exigé pour `deny`, violations du 422
  placées, 412 garde la saisie, membres envoyés = `SPECS`)
  où : `crates/kartenn-policy/src/objects.rs:SPECS` (membres acceptés) ; formulaires et
  `If-Match` de `web/plugins/machines/src/index.ts` (`MachinesView`, `attach`) ; design
  « Règles et motifs »
  dépend de : 1

- [ ] 4. Catalogue : services, types de flux, environnements en écriture ; axes, valeurs,
  conditions, signaux, rôles exemptés, règles d'inférence en lecture — REQ-017-10 —
  verify : Vitest (avertissement `policy.service.wide_ports` affiché, refus de clôture
  listé)
  où : design « Catalogue » ; `crates/kartenn-policy/src/rest.rs` (table des collections :
  filtres et membres écrits) ; périmètre selon Q-017-5
  dépend de : 1

- [ ] 5. Matrice en lecture : choix du type de flux, table datée, cellules, `outside` et
  lien vers les règles hors vue, navigation au clavier (grille APG) — REQ-017-6, 27 —
  verify : Vitest (cellule absente = défaut, multi-axes hors vue, flèches/Origine/Fin,
  état lisible sans couleur, rendu de 2 500 cellules < 100 ms indicatif, REQ-017-29)
  où : `crates/kartenn-policy/src/matrix.rs` (sémantique des cellules) ; schéma
  `PolicyMatrix` ; design « Matrice »
  dépend de : 1

- [ ] 6. Matrice en écriture et ensemble en préparation : panneau de cellule, ensemble
  local, simulation (`?simulate=true`) et rendu des sections, contrôle de révision,
  soumission (201, 202) — REQ-017-7, 8 — verify : Vitest (corps `policy.matrix.set`,
  `conditional` sans motif refusé, section `policy.combinations` rendue, révision changée
  signalée, 202 → lien `changes/<id>`)
  où : design « Ensemble en préparation et différentiel » ; simulation du brouillon du
  shell (`web/shell/src/draft/index.ts`, délai et forme `ChangesetSimulated`) ; options
  de Q-017-1 et Q-017-7
  dépend de : 5

- [ ] 7. Vue des changements d'un changeset : en-tête, sections paginées, lien vers la
  fiche shell — REQ-017-9 — verify : Vitest (pages suivies par `links.next`, section
  `flow.flows` demandée seulement si `flow` est actif)
  où : `GET /v1/changesets/{id}/diff?section=` (schéma `SectionPage`) ; design
  « Ensemble en préparation et différentiel »
  dépend de : 6

- [ ] 8. Requête de conformité et action « Vérifier un flux » : formulaire, `QUERY
  /v1/verdicts`, synthèse, explication complète et réduite, textes pédagogiques, 422 —
  REQ-017-11, 12, 13 — verify : Vitest (6 évaluations machine × machine,
  `unknown_service`, conditions `not_evaluated` sans dossier, aucun appel sortant en
  réduit, `hidden` affiché)
  où : schémas `PolicyVerdictQuery`, `PolicyVerdict` ; `crates/kartenn-policy/src/verdict.rs`
  (droits, réduction) ; action `attach` du plugin machines (pré-remplissage depuis une
  fiche) ; design « Requête de conformité »
  dépend de : 1

- [ ] 9. Rapport d'audit : demande, 409, suivi d'état, décomptes, détail paginé, export
  JSON — REQ-017-14 — verify : Vitest (409 → rapport en cours, rubrique `null` =
  « indisponible », export = concaténation des pages)
  où : `crates/kartenn-policy/src/rest.rs` (section « reports ») ; design « Rapport
  d'audit »
  dépend de : 1

## P3 — Flux

- [ ] 10. Liste, fiche et onglet « Flux » ; modifier, avancer la fin, clore — REQ-017-17,
  18, 19, 25 — verify : Vitest (filtres dans l'URL, `machine=` et `endpoint=interface:`,
  évaluations réduites rendues sans complément, `If-Match` sur `PATCH`/`DELETE`)
  où : schémas `Flow`, `FlowEvaluation`, `FlowPatch` ; `crates/kartenn-flow/src/rest.rs`
  (filtres de `list_flows`) ; onglet `machines-tab` du plugin machines ; design « Flux,
  fiche, onglet »
  dépend de : 2

- [ ] 11. Demande d'un flux : formulaire, pré-vérification, envoi idempotent, 201 et 202,
  action « Demander un flux » depuis une fiche, renouvellement pré-rempli — REQ-017-15,
  16 — verify : Vitest (référence exigée, plage refusée, pré-vérification sans écriture,
  202 → issue de la pré-vérification marquée, une seule soumission après coupure)
  où : schéma `FlowRequest` ; `crates/kartenn-flow/src/handler.rs` (`request`,
  `complete`, triage) ; `core.external_system` (`GET /v1/property-declarations/…/values`) ;
  design « Demande » ; option de Q-017-2 et Q-017-4
  dépend de : 10

- [ ] 12. File d'instruction et décision : file, fiche de la demande, approbation
  (`approve_review`, `waive` si refusé), rejet motivé, boutons absents pour le demandeur
  — REQ-017-20, 21 — verify : Vitest (corps de `validation`, plafonds 12 et 6 mois côté
  client, `flow.separation` affichée)
  où : `crates/kartenn-flow/src/handler.rs:ReviewIn`, `module.rs:review_operations` ;
  REQ-016-11 ; design « Instruction » ; option de Q-017-3 (B : amendement de la 011
  livré avant ; A : propositions + `QUERY /v1/verdicts`)
  dépend de : 10 ; si Q-017-3 = B, la tâche de la 011 qui sert `state=requested`

- [ ] 13. Dérogations et échéances : octroi, recertification, liste J-30/J-7 — REQ-017-22,
  23 — verify : Vitest (J-7 marqué, flux sans fin absent, 202 de la dérogation)
  où : schéma `FlowWaiver` ; `crates/kartenn-flow/src/read.rs` (`ending_before` : fin du
  flux ou de la dérogation) ; design « Dérogations et échéances »
  dépend de : 10

- [ ] 14. Non-conformités et acquittement — REQ-017-24 — verify : Vitest (texte en repli
  `fr` avec la langue indiquée, page pleine signalée, acquittement)
  où : schéma `FlowNonconformity` ; `crates/kartenn-flow/src/read.rs` (non-conformités) ;
  `web/shell/src/views/AnomaliesView.vue` (acquittement) ; design « Non-conformités »
  dépend de : 2

## P4 — Parcours et synchronisation

- [ ] 15. Parcours du responsable sécurité : règle `deny` avec texte, matrice au clavier,
  simulation et différentiel, soumission, requête de conformité, rapport — REQ-017-26,
  27, 28 — verify : `web/e2e/journeys/policy.spec.ts` (Chromium, Firefox, axe-core à
  chaque étape) dans `k-web-e2e`
  où : `web/e2e/journeys/machines.spec.ts` ; `web/e2e/global-setup.ts` (rôles et liaisons
  par l'API)
  dépend de : 3 à 9

- [ ] 16. Parcours demandeur et instructeur : demande conforme, demande à instruire,
  approbation avec dérogation, rejet, non-conformité vue par le demandeur, échéance —
  REQ-017-26, 27, 28 — verify : `web/e2e/journeys/flow.spec.ts` dans `k-web-e2e`
  où : `web/e2e/users.toml` (deux identités : demandeur, instructeur) ;
  `web/e2e/global-setup.ts` ; parcours de la tâche 15
  dépend de : 10 à 14

- [ ] 17. Synchronisation : `docs/spec/interfaces.md` (§ Shell Web et plugins UI : plugins
  livrés `policy`, `flow`), `specs/README.md`, renvois des « Non-objectifs » de la 010 et
  de la 011 vers la 017, statuts `implemented` — verify : rapport `spec-keeper` sans écart
  bloquant
  où : skill `tech-specs` (mode sync) ; 009 tâche 14
  dépend de : 1 à 16

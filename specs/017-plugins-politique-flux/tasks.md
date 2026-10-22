---
status: approved
updated: 2026-10-22
---

# 017 — Plugins Politique et Flux : tâches

Chaque tâche est un commit, laisse le build vert (`k-web-check`, et `k-check` pour les
tâches qui touchent le serveur) et nomme ce qu'elle implémente. « verify » = test qui
prouve la tâche ; « où » = fichiers et symboles à toucher ou à imiter ; « dépend de » =
tâches à finir avant (`010-21`, `011-12`, `011-13` : tâches d'amendement de la 010 et de
la 011). Questions Q-017-1 à 7 arbitrées le 2026-10-22 ([`decisions.md`](decisions.md)).

Ordre : la tâche 1 (maquettes validées, U14) précède **toute** tâche web ; les tâches
d'amendement 010-21, 011-12 et 011-13 n'en dépendent pas et avancent en parallèle. Puis
2 et 3 ; puis P2 (politique) et P3 (flux) en parallèle : dans P2, 4, 5, 8, 11, 12 et 14
sont indépendantes, 6, 7 et 13 suivent 5, 9 suit 8 et 010-21, 10 suit 9 ; dans P3, 17,
20 et 21 suivent 16, 18 suit 17 et 011-13, 19 suit 16 et 011-12. Pas plus de trois
sous-agents d'implémentation à la fois (CLAUDE.md).

## P0 — Maquettes

- [ ] 1. Maquettes des écrans (policy, flow) validées par l'utilisateur : une maquette par
  vue, onglet et action des deux manifestes (design, tableaux « Plugin `policy` » et
  « Plugin `flow` »), états vides, d'erreur (412, 422, 404) et réduits compris ; matrice
  au clavier, ensemble en préparation et différentiel, formulaires du catalogue et de
  l'inférence, demande et pré-vérification, file d'instruction, prolongation,
  échéances, non-conformités — U14, REQ-017-4 à 34 — verify : validation de
  l'utilisateur consignée dans `decisions.md` (U14, date) ; écarts au design reportés
  dans le design avant la tâche 2 ; aucun code de production
  où : `specs/017-plugins-politique-flux/maquettes/` (pages statiques, hors du build) ;
  design « Plugin `policy` », « Plugin `flow` » ; écrans existants des plugins
  `web/plugins/vlan`, `web/plugins/machines` et du shell (`web/shell/src/views/`) pour le
  style
  dépend de : —

## P1 — Squelettes

- [ ] 2. Plugin `policy` : paquet `@kartenn/plugin-policy`, manifeste (vues et actions du
  design, permissions), catalogues FR/EN, éléments vides qui s'enregistrent, projet
  Vitest, construction par `serve.sh`, test serveur de publication — REQ-017-1, 3 —
  verify : `test/policy.test.ts` (clés FR = EN, éléments définis) ;
  `the_policy_plugin_is_published_only_with_its_module`
  où : `web/plugins/machines/{package.json,public/manifest.json,vite.config.ts}` ;
  `web/vitest.config.ts` ; `web/e2e/serve.sh` (filtre `--filter @kartenn/plugin-…`) ;
  `crates/kartenn-rest/tests/it/plugins.rs` (test du plugin VLAN à copier) ; design
  « Plugin `policy` »
  dépend de : 1

- [ ] 3. Plugin `flow` : même squelette (`@kartenn/plugin-flow`, vues, onglet, action) —
  REQ-017-2, 3 — verify : `test/flow.test.ts` ;
  `the_flow_plugin_is_published_only_with_its_module` ; le bundle de `flow` n'importe
  rien de `web/plugins/policy/` (test sur le graphe de `vite build`)
  où : comme 2 ; design « Plugin `flow` »
  dépend de : 1, 2

## P2 — Politique

- [ ] 4. Règles et motifs : liste filtrée, fiche, historique, formulaire (sélecteurs par
  axe, même valeur, flux, effet, motifs requis, textes), création, modification,
  clôture — REQ-017-4, 5 — verify : Vitest (texte exigé pour `deny`, violations du 422
  placées, 412 garde la saisie, membres envoyés = `SPECS`)
  où : `crates/kartenn-policy/src/objects.rs:SPECS` (membres acceptés) ; formulaires et
  `If-Match` de `web/plugins/machines/src/index.ts` (`MachinesView`, `attach`) ; design
  « Règles et motifs »
  dépend de : 1, 2

- [ ] 5. Catalogue : formulaire générique piloté par une table de membres par
  collection ; services, types de flux, environnements (création, description,
  clôture) — REQ-017-10 — verify : Vitest (table des membres = `COLLECTIONS` du
  serveur, avertissement `policy.service.wide_ports` affiché, refus de clôture listé,
  description d'un environnement par `policy-values`)
  où : design « Catalogue » ; `crates/kartenn-policy/src/rest.rs:COLLECTIONS` (filtres,
  membres de création et de modification, clôture) ; formulaire de la tâche 4 à
  généraliser
  dépend de : 1, 2

- [ ] 6. Axes et valeurs : désignation, modification, retrait d'un axe ; description des
  valeurs (libellé, description, transverse, rang) — REQ-017-30 — verify : Vitest
  (membres `designate` et `update` envoyés, valeurs triées par rang, retrait refusé
  avec les objets qui citent l'axe, aucune création ni clôture de valeur offerte)
  où : `COLLECTIONS` (`policy-axes`, `policy-values`) ; `crates/kartenn-policy/src/handler.rs`
  (`plan_describe`) ; formulaire générique de la tâche 5 ; design « Catalogue »
  dépend de : 1, 5

- [ ] 7. Conditions d'admissibilité, signaux de risque, rôles exemptés : listes, fiches,
  historique, création, modification (sauf rôles exemptés), clôture ; passage par
  l'ensemble en préparation pour simuler — REQ-017-31 — verify : Vitest (texte exigé
  pour une condition, `policy.text.missing` placé, poids d'un signal envoyé, rôle
  exempté sans modification offerte)
  où : `COLLECTIONS` (`admissibility-conditions`, `risk-signals`, `policy-exempt-roles`) ;
  `crates/kartenn-policy/src/objects.rs:SPECS` ; formulaire générique de la tâche 5 ;
  design « Catalogue »
  dépend de : 1, 5

- [ ] 8. Matrice en lecture : choix du type de flux, table datée, cellules, `outside` et
  lien vers les règles hors vue, navigation au clavier (grille APG) — REQ-017-6, 27 —
  verify : Vitest (cellule absente = défaut, multi-axes hors vue, flèches/Origine/Fin,
  état lisible sans couleur, rendu de 2 500 cellules < 100 ms indicatif, REQ-017-29)
  où : `crates/kartenn-policy/src/matrix.rs` (sémantique des cellules) ; schéma
  `PolicyMatrix` ; design « Matrice »
  dépend de : 1, 2

- [ ] 9. Matrice en écriture et ensemble en préparation : panneau de cellule, ensemble
  local avec l'ETag de chaque matrice lue, simulation (`?simulate=true`) et rendu des
  sections, soumission (`if_match` par opération, `PUT` sous `If-Match`), 201, 202,
  412 et `policy.matrix.stale` (relecture, ensemble gardé) — REQ-017-7, 8 — verify :
  Vitest (corps `policy.matrix.set` avec `if_match`, `conditional` sans motif refusé,
  section `policy.combinations` rendue, 412 → matrice relue et ensemble intact, 202 →
  lien `changes/<id>`)
  où : design « Matrice » et « Ensemble en préparation et différentiel » ; simulation du
  brouillon du shell (`web/shell/src/draft/index.ts`, délai et forme
  `ChangesetSimulated`) ; contrat du `PUT` après 010-21 (`web/sdk/src/api/schema.d.ts`)
  dépend de : 1, 8, 010-21

- [ ] 10. Vue des changements d'un changeset : en-tête, sections paginées, lien vers la
  fiche shell — REQ-017-9 — verify : Vitest (pages suivies par `links.next`, section
  `flow.flows` demandée seulement si `flow` est actif)
  où : `GET /v1/changesets/{id}/diff?section=` (schéma `SectionPage`) ; design
  « Ensemble en préparation et différentiel »
  dépend de : 1, 9

- [ ] 11. Requête de conformité et action « Vérifier un flux » : formulaire (recherche
  dans le lisible et identifiant collé, Q-017-2), `QUERY /v1/verdicts`, synthèse,
  explication complète et réduite, textes pédagogiques, 422 — REQ-017-11, 12, 13 —
  verify : Vitest (6 évaluations machine × machine, `unknown_service`, conditions
  `not_evaluated` sans dossier, aucun appel sortant en réduit, `hidden` affiché,
  identifiant collé accepté)
  où : schémas `PolicyVerdictQuery`, `PolicyVerdict` ; `crates/kartenn-policy/src/verdict.rs`
  (droits, réduction) ; action `attach` du plugin machines (pré-remplissage depuis une
  fiche) ; design « Requête de conformité »
  dépend de : 1, 2

- [ ] 12. Rapport d'audit : demande, 409, suivi d'état, décomptes, détail paginé, export
  JSON — REQ-017-14 — verify : Vitest (409 → rapport en cours, rubrique `null` =
  « indisponible », export = concaténation des pages)
  où : `crates/kartenn-policy/src/rest.rs` (section « reports ») ; design « Rapport
  d'audit »
  dépend de : 1, 2

- [ ] 13. Règles d'inférence : liste, fiche, historique et révision du jeu, formulaire
  (axe visé, attendus, critères de la liste fermée, exceptions en arbre ≤ 4, priorité,
  domaine), création, modification, clôture — REQ-017-32 — verify : Vitest (profondeur 5
  refusée côté client, `policy.inference.depth`, `regex_invalid` et `criterion_invalid`
  placés sur leur critère, `policy.inference.conflict` affiché, 403 d'un domaine hors
  périmètre sans perte de saisie)
  où : `COLLECTIONS` (`inference-rules`) ; `crates/kartenn-policy/src/inference.rs` et
  `rules.rs` (critères, règles `policy.inference.*`) ; formulaire générique de la tâche
  5 ; design « Inférence »
  dépend de : 1, 5

- [ ] 14. Attendus et écarts d'inférence : action `expected` sur les fiches `address` et
  `prefix` (attendus, candidats, explication), vue `inference/mismatches` (anomalies
  `policy.inference.mismatch`, décompte des héritiers, acquittement) — REQ-017-33 —
  verify : Vitest (deux candidats à égalité tous affichés, explication rendue critère
  par critère, acquittement retire l'écart de la liste ouverte)
  où : routes `…/properties/{key}/suggestion` et `…/explanation` du noyau
  (`web/sdk/src/api/schema.d.ts`) ; `GET /v1/anomalies?rule=` et acquittement
  (`web/shell/src/views/AnomaliesView.vue`) ; design « Inférence »
  dépend de : 1, 2

- [ ] 15. Aperçu d'une règle d'inférence avant écriture : nœuds qui correspondraient,
  écarts ouverts ou fermés — REQ-017-34 — verify : Vitest (aperçu de 12 nœuds et 3
  écarts sans écriture), selon la route que l'amendement fixera
  où : design « Inférence » (ligne « Aperçu ») ; formulaire de la tâche 13
  dépend de : 1, 13 ; **amendement de la 010 à rédiger** (section de différentiel ou
  simulation de l'inférence, non spécifié au 2026-10-22) et sa tâche serveur

## P3 — Flux

- [ ] 16. Liste, fiche et onglet « Flux » ; modifier, avancer la fin, clore —
  REQ-017-17, 18, 19, 25 — verify : Vitest (filtres dans l'URL, `machine=` et
  `endpoint=interface:`, évaluations réduites rendues sans complément, `If-Match` sur
  `PATCH`/`DELETE`, ligne `requested` sans lien de fiche dans « Mes demandes »)
  où : schémas `Flow`, `FlowEvaluation`, `FlowPatch` ; `crates/kartenn-flow/src/rest.rs`
  (filtres de `list`) ; onglet `machines-tab` du plugin machines ; design « Flux,
  fiche, onglet »
  dépend de : 1, 3 ; 011-12 pour les lignes `requested` de « Mes demandes »

- [ ] 17. Demande d'un flux : formulaire (recherche dans le lisible et identifiant collé,
  Q-017-2), pré-vérification, envoi idempotent, 201 et 202, action « Demander un flux »
  depuis une fiche, nouvelle demande pré-remplie depuis un flux — REQ-017-15, 16 —
  verify : Vitest (référence exigée, plage refusée, pré-vérification sans écriture,
  202 → issue de la pré-vérification marquée, une seule soumission après coupure)
  où : schéma `FlowRequest` ; `crates/kartenn-flow/src/handler.rs` (`request`,
  `complete`, `triage`) ; `core.external_system` (`GET /v1/property-declarations/…/values`) ;
  design « Demande »
  dépend de : 1, 16

- [ ] 18. Prolongation d'un flux : action « Prolonger » sur un flux daté non terminé,
  nouvelle fin, pré-vérification de la portion ajoutée, `PATCH`, 200 ou 202 (fiche qui
  garde l'ancienne fin et signale la prolongation en attente), absente d'un flux
  terminé — REQ-017-25, 23 — verify : Vitest (corps `FlowPatch` à fin plus tardive,
  202 → lien vers le changeset, « Prolonger » absent d'un flux `expired`,
  `flow.update.ended` affiché si forcé)
  où : design « Flux, fiche, onglet » (paragraphe « Prolonger ») ; contrat du `PATCH`
  après 011-13 ; pré-vérification de la tâche 17
  dépend de : 1, 17, 011-13

- [ ] 19. File d'instruction et décision : file `GET /v1/flows?review=pending` (demandes
  et prolongations), fiche `review/<changeset>`, approbation (`approve_review`, `waive`
  si refusé), rejet motivé, boutons absents pour le demandeur ou l'auteur — REQ-017-20,
  21 — verify : Vitest (une requête par page, aucune lecture par changeset, corps de
  `validation`, plafonds 12 mois depuis la décision et 6 mois côté client,
  `flow.separation` affichée, ligne `extension` rendue)
  où : `crates/kartenn-flow/src/handler.rs:ReviewIn`, `module.rs:review_operations` ;
  REQ-016-11 ; membre `review` de la ligne (011-12) ; design « Instruction »
  dépend de : 1, 16, 011-12 ; 011-13 pour les prolongations

- [ ] 20. Dérogations et échéances : octroi, recertification, liste J-30/J-7, prolongation
  depuis la liste — REQ-017-22, 23 — verify : Vitest (J-7 marqué, flux sans fin absent,
  202 de la dérogation, « Prolonger » mène au parcours de la tâche 18)
  où : schéma `FlowWaiver` ; `crates/kartenn-flow/src/read.rs` (`ending_before` : fin du
  flux ou de la dérogation) ; design « Dérogations et échéances »
  dépend de : 1, 16 ; 18 pour l'action « Prolonger »

- [ ] 21. Non-conformités et acquittement — REQ-017-24 — verify : Vitest (texte en repli
  `fr` avec la langue indiquée, page pleine signalée, acquittement)
  où : schéma `FlowNonconformity` ; `crates/kartenn-flow/src/read.rs` (`nonconformities`) ;
  `web/shell/src/views/AnomaliesView.vue` (acquittement) ; design « Non-conformités »
  dépend de : 1, 3

## P4 — Parcours et synchronisation

- [ ] 22. Parcours du responsable sécurité : règle `deny` avec texte, matrice au clavier,
  simulation et différentiel, soumission (412 rejoué), catalogue (service, signal),
  règle d'inférence et écart, requête de conformité, rapport — REQ-017-26, 27, 28 —
  verify : `web/e2e/journeys/policy.spec.ts` (Chromium, Firefox, axe-core à chaque
  étape) dans `k-web-e2e`
  où : `web/e2e/journeys/machines.spec.ts` ; `web/e2e/global-setup.ts` (rôles et liaisons
  par l'API)
  dépend de : 1, 4 à 14

- [ ] 23. Parcours demandeur et instructeur : demande conforme, demande à instruire vue
  dans la file, approbation avec dérogation, rejet, non-conformité vue par le
  demandeur, prolongation instruite, échéance — REQ-017-26, 27, 28 — verify :
  `web/e2e/journeys/flow.spec.ts` dans `k-web-e2e`
  où : `web/e2e/users.toml` (deux identités : demandeur, instructeur) ;
  `web/e2e/global-setup.ts` ; parcours de la tâche 22
  dépend de : 1, 16 à 21

- [ ] 24. Synchronisation : `docs/spec/interfaces.md` (§ Shell Web et plugins UI : plugins
  livrés `policy`, `flow`), `specs/README.md`, renvois des « Non-objectifs » de la 010 et
  de la 011 vers la 017, statuts `implemented` — verify : rapport `spec-keeper` sans écart
  bloquant
  où : skill `tech-specs` (mode sync) ; 009 tâche 14
  dépend de : 1 à 14, 16 à 23 ; 15 si l'amendement de la 010 est livré, sinon 15 reste
  ouverte et la 017 reste `in-progress` sur REQ-017-34

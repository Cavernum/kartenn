---
status: proposed
updated: 2026-10-22
---

# 018 — Retours d'usage de la v0.2 : tâches

Statut : **proposée, en attente de validation** ; aucune tâche ne commence avant la
validation de la spec et l'arbitrage des questions dont elle dépend (colonne « dépend
de » : `Q-018-n`).

Chaque tâche est un commit, laisse le build vert (`k-check` pour le serveur,
`k-web-check` pour le web) et nomme ce qu'elle implémente. « verify » = test qui prouve
la tâche ; « où » = fichiers et symboles à toucher ou à imiter ; « dépend de » = tâches
ou arbitrages à finir avant.

Ordre : P1 (permissions) d'abord, avant les tâches 017-2 et 017-3 (R18) ; P2 (libellés,
serveur) en parallèle de P1 une fois Q-018-4 à 7 arbitrées ; la tâche 1 (maquettes,
avec la 017-1) précède toute tâche web de P3 à P5 ; P6 en dernier. Pas plus de trois
sous-agents d'implémentation à la fois (CLAUDE.md).

## P0 — Maquettes

- [ ] 1. Maquettes validées par l'utilisateur : `<k-code>`, `<k-label>` (repli, clé
  visible), formulaires de libellés par langue, cellule éditable (lecture, édition,
  modifiée dans le brouillon, erreur), héritage, action de nœud de l'arbre, constructeur
  de filtres (chaque type) ; faites avec celles de la 017 (tâche 017-1) — R12, R14, R17 ;
  REQ-018-9, 15, 16, 18 à 22, 24, 25 — verify : validation consignée dans
  `decisions.md` (R17, date) ; écarts reportés au design ; aucun code de production
  où : `specs/018-retours-interface-v0-2/maquettes/` (pages statiques) ; design, sections
  C, D, E ; maquettes de la 017 (`specs/017-plugins-politique-flux/maquettes/`) ; style de
  `web/shell/src/views/`
  dépend de : validation de la spec

## P1 — Noms de permissions

- [ ] 2. Grammaire et table : `RENAMED_PERMISSIONS`, test des catalogues (pas de `m.m.a`,
  aucune ressource nommée comme une action, cibles connues), `type_read_permission` et
  `type_write_permission` selon R2, migration SQL de la contrainte de
  `role_permission.permission` — REQ-018-1, R3 à R5 — verify :
  `permission_names_omit_the_module_resource`, `renamed_permissions_target_the_catalogue`,
  `a_module_kind_reads_under_the_short_permission` ; test de migration (insérer
  `flow.read`)
  où : `crates/kartenn-core/src/authz.rs` (`PERMISSIONS`, `permission`,
  `valid_role_entry`, `type_permissions`, l. 133-195, tests l. ~369) ;
  `crates/kartenn-store-pg/migrations/0028_rbac.sql` (contrainte, l. 29) pour la nouvelle
  migration ; design « A. Permissions »
  dépend de : Q-018-1

- [ ] 3. Renommage dans les modules et le service : catalogues `vlan`, `machines`,
  `flow`, rôles livrés, appels de contrôle, tests et bancs ; `docs/spec/permissions.md`
  régénéré avec « Anciens noms » — REQ-018-2 — verify : `k-check` complet ;
  `the_published_catalogue_matches_the_code` (`kartenn-server/src/modules.rs`) ; test
  d'absence des anciens noms hors des exceptions
  où : `crates/kartenn-{vlan,machines,flow}/src/module.rs` (l. 124, 168, 92) ;
  `crates/kartenn-core/src/authz.rs` (`BUILTIN_ROLES`, `catalogue_markdown`) ;
  `crates/kartenn-{service,rest,flow,policy,vlan,machines}/src/**` ; tests
  `crates/*/tests/it/**` ; `crates/kartenn-bench/src/*_budgets.rs`, `rbac.rs`
  dépend de : 2

- [ ] 4. Migration des rôles personnalisés — REQ-018-3, R6 — verify :
  `custom_roles_take_the_new_names_once` (idempotence, liaisons inchangées, droits
  identiques), `a_role_read_before_the_rename_keeps_the_old_name` (si Q-018-3 = A)
  où : job de migration et `sync_modules` (`crates/kartenn-service/src/declarations.rs`,
  `kartenn-server` : séquence des migrations) ; écriture de rôles
  `crates/kartenn-store-pg/src/rbac_admin.rs` (l. 85, 148-170) ; tests à imiter
  `crates/kartenn-service/tests/it/rbac.rs`
  dépend de : 3, Q-018-3

- [ ] 5. Compatibilité selon Q-018-2 (B : alias à l'entrée, `core.permission.renamed`,
  retrait daté) — REQ-018-5 — verify : `an_old_permission_name_is_normalised_with_a_warning`
  ; test qui échoue quand la version atteint 0.3 ; (A : `an_old_permission_name_is_refused`)
  où : `crates/kartenn-service/src/rbac.rs` (`check`) ; `crates/kartenn-core/src/rules.rs`
  (nouvelle règle, macro l. ~97) ; `docs/spec/regles-noyau.md`
  dépend de : 3, Q-018-2

- [ ] 6. Web : manifestes `vlan`, `machines`, `example`, `web/e2e/plugins/notes`, shell
  (`admin.ts`, `navigation.ts`), parcours E2E ; table générée pour le SDK
  (`web/sdk/src/permissions.ts`) ; affichage « nouveau (anciennement ancien) » des rôles
  et de l'audit ; normalisation des manifestes si Q-018-2 = B — REQ-018-4, 6 — verify :
  `k-web-check` ; Vitest `permissions.test.ts` (catalogue généré), `admin.test.ts` ;
  `k-web-e2e`
  où : `web/plugins/{vlan,machines,example}/public/manifest.json` ;
  `web/shell/src/{admin/admin.ts,navigation.ts,rights/permissions.ts,plugins/registry.ts}` ;
  `web/shell/src/admin/{RolesView,AuditView}.vue` ; `web/e2e/{global-setup.ts,fixtures/data.ts}`
  dépend de : 3, 5

- [ ] 7. Docs du renommage : `docs/spec/{interfaces,invariants}.md`, descriptions de
  `specs/006-api-rest/openapi-v1.json` (entrées de rôle), README du SDK, ADR 0010
  (amendement), 005/K12 et 011/F12 amendés — REQ-018-6 — verify : test de contrat
  (`openapi.rs` = fichier de référence) ; `grep` des anciens noms vide hors exceptions
  où : `crates/kartenn-rest/src/openapi.rs` ; `docs/adr/0010-rbac.md` ;
  `specs/005-authn-rbac/decisions.md` (K12) ; `specs/011-registre-flux/decisions.md` (F12)
  dépend de : 3

## P2 — Libellés (serveur)

- [ ] 8. ADR 0033 « Libellés de données par langue » (proposée) et note de référence
  BCP 47 (RFC 5646 §2.1, RFC 4647 §3.4, normatif / choix de kartenn) — R9, Q-018-4 à 7 —
  verify : relecture de l'utilisateur
  où : `docs/adr/0026-contrat-de-module.md` (forme) ; `docs/reference/` (forme des notes,
  `noms-dates-texte.md`)
  dépend de : Q-018-4, 5, 6, 7

- [ ] 9. Schéma et règle : `labels jsonb` (Q-018-6), reprise des `label` existants,
  `policy_value.label` vers le noyau (Q-018-7), `core.label.invalid`, `[i18n]
  default_locale`, `Locale` au noyau — REQ-018-8, 10, 13, 14 — verify :
  `labels_are_checked`, `existing_labels_become_default_language_labels`,
  `the_default_locale_is_configured` ; tests de la politique verts
  où : `crates/kartenn-store-pg/migrations/{0011_properties.sql,0028_rbac.sql}` ;
  `crates/kartenn-policy/migrations/0001_policy.sql` ; `crates/kartenn-policy/src/{codec.rs,model.rs:484}` ;
  configuration `crates/kartenn-server/src/` (section `[web]` à imiter)
  dépend de : 8

- [ ] 10. Opérations : `labels` sur `declare_property`, `update_property`,
  `add_enum_value`, rôles et opérations de la politique ; `update_enum_value` ;
  différentiel et historique des libellés — REQ-018-10 — verify :
  `relabelling_a_value_is_a_changeset`, `a_value_key_never_changes`, différentiel
  où : `crates/kartenn-rest/src/operation.rs` (`Operation`, `samples`, l. ~1340) ;
  `crates/kartenn-core` (opérations de propriétés) ; `crates/kartenn-policy/src/handler.rs`
  (l. 172, 318-353)
  dépend de : 9

- [ ] 11. Libellés des modules (R10) : descripteurs, `sync_modules`, rôles livrés FR/EN —
  REQ-018-11, 14 — verify : `an_administrator_label_survives_the_module_sync`
  où : `crates/kartenn-service/src/declarations.rs` (`sync_modules`) ; descripteurs des
  modules (`crates/kartenn-policy/src/module.rs`, propriétés) ; `BuiltinRole`
  (`authz.rs`, l. ~200)
  dépend de : 10

- [ ] 12. API : `labels` et `label` (langue par défaut) sur les représentations ;
  `/values` (`labels`, période, `valid_at`, `q`) ; `capabilities` (`labels`,
  `default_locale`) ; contrat et comparateur — REQ-018-7, 8 ; Q-018-4 — verify : tests
  REST, contrat de référence à jour
  où : `crates/kartenn-rest/src/{properties.rs (l. 215, 240, 280, 286),capabilities.rs,rights.rs:265}` ;
  `crates/kartenn-policy/src/rest.rs` (l. 111-280) ; `specs/006-api-rest/openapi-v1.json`
  dépend de : 10, Q-018-4

## P3 — Libellés et codes (web)

- [ ] 13. SDK : `labelOf`, `<k-label>`, `<k-code>`, `moduleLabel`, `ApiProblem` avec
  `instance` et `violations` — REQ-018-9, 12, 15 à 17 — verify : Vitest (repli de
  langue, `lang`, copie et annonce, violations gardées)
  où : `web/sdk/src/{i18n.ts,api/problem.ts,extensions.ts}` ; tests `web/sdk/test/`
  dépend de : 1, 12

- [ ] 14. Shell, codes visibles : `problemOf`, `problemText` → message + code ; avis,
  dialogue 412, `QuickEdit`, brouillon, changeset, anomalies, imports, audit —
  REQ-018-15 à 17 — verify : Vitest par vue ; axe-core
  où : `web/shell/src/{data/api.ts,i18n/index.ts}` ;
  `web/shell/src/{edit/QuickEdit.vue,draft/DraftDrawer.vue,changesets/ChangesetView.vue,record/AnomaliesPanel.vue,admin/ImportView.vue,admin/AuditView.vue}`
  dépend de : 13

- [ ] 15. Shell, libellés : propriétés et valeurs, sortes de module, natures et usages,
  rôles des liaisons ; catalogues FR/EN des codes du noyau ; test de parité des codes —
  REQ-018-9, 12 — verify : Vitest ; parité (chaque code a une clé FR et EN)
  où : `web/shell/src/{record/PropertiesPanel.vue,views/PropertiesView.vue,views/ObjectView.vue:128,data/collections.ts,edit/fields.ts,admin/BindingsView.vue}` ;
  `web/shell/src/i18n/{fr,en}.json`, `i18n.test.ts`
  dépend de : 13

- [ ] 16. Formulaires de libellés : déclarations et valeurs (`PropertiesView`), rôles
  (`RolesView`) — REQ-018-10 — verify : Vitest (une ligne par langue, langue par défaut
  exigée, 422 placé) ; E2E court
  où : `web/shell/src/views/PropertiesView.vue` ; `web/shell/src/admin/{RolesView.vue,write.ts}`
  dépend de : 12, 15

- [ ] 17. Plugins `vlan` et `machines` : nouveaux noms (fait en 6), libellés par
  `moduleLabel`, codes par `<k-code>`, clés `kinds.*`, `states.*`, `rules.*` dans leurs
  catalogues — REQ-018-12, 15 — verify : Vitest des plugins ; parité
  où : `web/plugins/{vlan,machines}/src/index.ts` ; `web/plugins/*/public/i18n/{fr,en}.json`
  dépend de : 13

## P4 — Édition en place

- [ ] 18. Grille éditable : `DataTable` au motif `grid` (tabindex itinérant, F2, Échap),
  cellule éditable typée, opération ajoutée au brouillon, marque « modifié », droits et
  temps — REQ-018-18, 19, 22 — verify : Vitest (clavier, lecture seule sans droit ou en
  `as_of`, opération ajoutée et retirée) ; mesure de rendu de 100 lignes
  où : `web/shell/src/data/{DataTable.vue,collections.ts}` ; `web/shell/src/draft/index.ts`
  (`add`) ; `web/shell/src/rights/permissions.ts` ; `web/shell/src/edit/fields.ts`
  dépend de : 1, 15

- [ ] 19. Colonnes de propriétés et édition des propriétés (listes et panneau de la
  fiche) : `assign_property`, `clear_property`, `unassign_property`, héritage (D26) —
  REQ-018-18, 20 — verify : Vitest (héritage nommé, trois opérations distinctes) ;
  banc ou mesure de la page avec colonnes (REQ-018-32)
  où : `web/shell/src/record/PropertiesPanel.vue` ; `web/shell/src/views/ListView.vue` ;
  lecture des propriétés `crates/kartenn-rest/src/properties.rs` (l. ~349)
  dépend de : 18

- [ ] 20. Arbre : action « Changer <propriété> » sur le nœud (menu, Maj+F10) —
  REQ-018-21, 22 — verify : Vitest (clavier seul) ; axe-core
  où : `web/shell/src/tree/{PrefixTree.vue,model.ts}`
  dépend de : 19

- [ ] 21. SDK : `<k-editable-value>` ; utilisation dans `machines` (sorte) et `vlan`
  (état) — REQ-018-23 — verify : Vitest des plugins (droit absent → lecture seule ;
  `draft.add` appelé)
  où : `web/sdk/src/` (`draft`, contexte) ; `web/plugins/{machines,vlan}/src/index.ts`
  dépend de : 18

## P5 — Filtres typés

- [ ] 22. Serveur, suggestions selon Q-018-8 (A : `q` sur les collections citables,
  `GET /v1/rules`) et banc — REQ-018-26, 29 — verify : tests REST (RBAC : un objet non
  lisible n'est jamais suggéré) ; `bench-core` p95 < 50 ms
  où : `crates/kartenn-rest/src/{filter.rs,anomalies.rs}` ; routes des collections des
  modules (`crates/kartenn-{machines,vlan}/src/rest.rs`) ; `crates/kartenn-core/src/rules.rs`
  (`catalogue`, l. 290-320) ; `crates/kartenn-bench/src/`
  dépend de : 12, Q-018-8

- [ ] 23. Constructeur de filtres (shell, SDK) : champs typés, opérateurs, contrôles de
  valeur, aller-retour avec l'URL, filtres illisibles signalés ; anomalies — REQ-018-24,
  25, 27, 28, 29 — verify : Vitest (chaque type ; URL ↔ constructeur ; valeur hors liste
  impossible) ; axe-core
  où : `web/shell/src/views/{ListView.vue,AnomaliesView.vue}` ;
  `web/shell/src/data/collections.ts` (`FILTERS`) ; syntaxe `crates/kartenn-rest/src/filter.rs`
  dépend de : 1, 15, 22

## P6 — Clôture

- [ ] 24. Parcours de bout en bout — REQ-018-31 — verify : `k-web-e2e` (Chromium,
  Firefox, axe-core à chaque étape)
  où : `web/e2e/journeys/` (parcours existants à imiter) ; `web/e2e/users.toml`
  dépend de : 14, 16, 19, 20, 23

- [ ] 25. Synchronisation : `docs/spec/` (`data.md`, `interfaces.md`, `regles-noyau.md`),
  amendements des specs 003, 006, 007, 008, 009, 010, 012, 017 (design, « Amendements
  d'autres specs »), ADR 0033 acceptée, `specs/README.md` — REQ-018-6, 32 — verify :
  `spec-keeper` sur la 018 sans écart
  où : design, « Amendements d'autres specs » et « Impact sur la 017 »
  dépend de : 24

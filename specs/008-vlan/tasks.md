---
status: draft
updated: 2026-10-20
---

# 008 — VLAN : tâches

Chaque tâche est un commit, laisse le build vert (`devenv test`, `k-check`) et nomme ce
qu'elle implémente. « verify » = test qui prouve la tâche ; « dépend de » = tâches à
finir avant (les autres peuvent avancer en parallèle, chacune dans son arbre de
travail). Les tests d'intégration PostgreSQL imitent `kartenn-store-pg/tests/` ; les
tests HTTP passent par `tower::ServiceExt::oneshot` comme
`kartenn-rest/tests/resources.rs`.

Parallélisme : P1 (contrat, dans le noyau) et la tâche 9 (domaine pur du VLAN)
démarrent ensemble ; dans P1, 6 et 20 ne dépendent que de 1 ; 4, 5 et 19 avancent en
parallèle après 3 ; P2 suit P1 ; la tâche 17 attend en plus la 007 (SDK et chargement
des plugins). Les tâches 19 et 20 (besoins N4, N8, N21 de la 010 et de la 011) sont
numérotées après coup.

## P1 — Contrat de module (ADR 0026)

- [x] 1. Registre de modules : `kartenn-service/src/module.rs` (traits `Module`,
  `ModuleHandler`, `ModuleTx`, types `ObjectType`, `Reference`, `EdgeType`,
  `ModuleMigration`, `ModuleRegistry::new` avec tri topologique) ; refus au démarrage
  dans `kartenn-server::app` ; `modules` des capacités tiré du registre — REQ-008-1 —
  verify : tests unitaires `registry_refuses_duplicate`, `_core_name`,
  `_missing_dependency`, `_cycle` ; `capabilities_list_registered_modules`
  (`kartenn-rest/tests/`)
  où : design « Registre de modules » ; `declarations::ModuleDeclarations` (forme des
  déclarations), `capabilities.rs::Module` ; composition dans
  `kartenn-server/src/app.rs`
  dépend de : —
  — Service : `module.rs` (`Module` aux méthodes par défaut, `ObjectType`, `OnClose`,
  `Reference`, `EdgeType`, `ModuleMigration`, `RegistryError`, `ModuleRegistry::new` :
  noms, `core` réservé, dépendances, Kahn stable, types hors espace de noms, collections
  en double ou du noyau `CORE_COLLECTIONS`). Rest : `Api::modules`, `modules` des
  capacités = étiquettes du contrat ∪ modules du registre (version de la crate).
  Server : `modules.rs::registry()` construit avant tout enregistrement, erreur =
  arrêt. Tests : `registry_*` (service), `capabilities_list_registered_modules`,
  `core_collections_are_those_of_the_contract` (rest).
  choix : `ModuleHandler` vide ici, ses méthodes et `ModuleTx` arrivent avec la tâche 3
  qui les branche ; `CORE_COLLECTIONS` vérifiée contre le contrat par un test.
- [x] 2. Migrations : migration du noyau `0041_module_object.sql` (`module_object`
  versionnée, `module_migration`, index par parent), migrateur de modules dans
  `kartenn-store-pg` (schéma par module, somme SHA-256, `search_path` local) appelé par
  le job de migration après `MIGRATOR` — REQ-008-2, 3 (table) — verify :
  `module_migrations_apply_once`, `a_changed_migration_stops_the_job`,
  `a_build_without_the_module_leaves_its_schema`
  où : design « Données du noyau » ; `kartenn-store-pg/src/lib.rs` (`MIGRATOR`),
  migrations `0002`, `0019` (`kartenn_make_versioned`), `0037` (`object_version`) ;
  job de migration dans `kartenn-server`
  dépend de : 1
  — Migration `0041_module_object.sql` (`module_object` versionnée, nom et type
  qualifiés contrôlés, `module_object_by_parent`, `module_migration`). Store :
  `modules.rs::migrate` (une transaction par migration, verrou consultatif, `CREATE
  SCHEMA`, `GRANT USAGE` à `kartenn_app`, `SET LOCAL search_path`, SHA-256, versions
  croissantes), `maintenance::deploy(pool, modules)` l'appelle après `MIGRATOR`. Server :
  `migrate` passe le registre. Tests `module_migrations.rs` (les trois du verify et
  `module_object_is_versioned`).
- [x] 3. Enveloppe d'opération : `Operation::Module`, désérialisation par étiquette
  qualifiée, `ObjectKind::Module` ; pipeline (`plan`, `lock_targets`, `apply`,
  validation globale, droits `<type>.write|propose`), opérations du noyau planifiées
  par une opération de module, clés de verrou nommées après les domaines (amendement de
  l'ADR 0015) ; `ModuleTx` pour la transaction de `kartenn-store-pg` (connexion,
  `register` dans `module_object`) ; module factice de test
  (`kartenn-service/tests/support/dummy_module.rs`, une table, une opération) —
  REQ-008-4, 27 — verify : `a_mixed_changeset_is_atomic`,
  `an_unknown_module_operation_is_refused_before_locks`,
  `approval_policy_counts_module_operations`, `module_write_needs_its_permission`,
  `a_module_operation_plans_core_operations`, `named_lock_keys_do_not_deadlock`
  où : design « Enveloppe d'opération » ; `operations.rs::Operation`,
  `pipeline.rs::{plan, lock_targets, apply, stages}`, `port.rs::WriteTx` ; test voisin
  `kartenn-service/tests/` (pipeline)
  dépend de : 1, 2
  — Service : `operations.rs` (`ModuleKind` en ligne, `Copy` ; `ObjectKind::Module`,
  (dé)sérialisé par son nom ; `ModuleOperation` et `Operation::Module`, étiquette
  qualifiée routée avant la désérialisation dérivée `remote = "Self"`), `module.rs`
  (`OperationType`, `ModulePlan`, `ModuleObjectRow`, `ModuleTx`, `ModuleHandler::{plan,
  apply, check}`, `Module::operations`, `ModuleRegistry::operation`), `port.rs`
  (`Store::modules`, `WriteTx::{lock_keys, module_tx}`), `pipeline.rs`
  (`RequestError::UnknownOperation` dans `check_request`, `Planned::Module`, file des
  opérations du noyau ajoutées par un module dans `plan`, périodes par opération planifiée,
  domaines et clés dans `lock_targets`, `apply`, `check_modules` après la validation
  globale), `rights.rs` (`<type>.write|propose`). Store : `PgStore::with_modules`,
  `ModuleTx` pour `PgTx` (`register` dans `module_object`), `lock_keys` (verrous à deux
  clés), `domain_of` d'un objet de module. Rest : `write_store` avec le registre,
  `UnknownOperation` → 400. Server : store des tâches de fond avec le registre. ADR 0015
  amendée (clés nommées). Tests : `module_pipeline.rs` (les six du verify), module
  factice `tests/support/dummy_module.rs`, `module_operations_round_trip_through_json`.
  choix : `ModuleKind` en tampon fixe de 64 octets pour garder `ObjectKind` `Copy` ;
  verrous et domaines portés par le `ModulePlan` (pas de lecture en base pour les
  calculer) ; l'enveloppe REST (`OperationDto`) reste à la tâche 8 ; `Close` et les
  opérations génériques sur objets de module à la tâche 4.
- [x] 4. Opérations génériques et mécanismes transverses sur objets de module : `Close`,
  `Expect` et `object_version`, `BindExternal`, `Within`, historique, `Restore` et
  annulation — REQ-008-3, 5 — verify : `module_object_has_etag_and_history`,
  `undo_closes_a_created_module_object`, `undo_restores_a_modified_module_object`,
  `bind_external_on_module_object`
  où : design « Enveloppe d'opération » (annulation) ; `pipeline.rs::{close, restore,
  conflict}`, `undo.rs`, store `undo.rs`, `temporal.rs`, `keys.rs` (external_id)
  dépend de : 3
  — Service : `module.rs` (`ModuleTx::{close_versioned, restore_versioned}`,
  `ModuleHandler::{close, restore}` sans effet par défaut, `ModuleRegistry::handler_of`),
  `pipeline.rs` (`close` et `restore` atteignent les lignes du module après
  l'enregistrement du noyau, `close` retire les affectations ; `check_modules` vérifie
  aussi les objets de module clos ou restaurés ; le registre passé à `apply`). Store :
  `register` réécrit l'enregistrement d'un objet déjà inscrit sur la période (nouvelle
  version, trace pour l'annulation) ; `close_in`/`restore_in` génériques pour le noyau
  et les tables versionnées d'un module (`module_table` vérifie `<module>.<table>`) ;
  `undo.rs` lit `module_object` (`module_objects`), `kind_of` et `object_kind`
  acceptent les types qualifiés. Tests : `module_pipeline.rs` (les quatre du verify,
  `Within` et `Expect` dans le premier), module factice versionné avec
  `dummy.update_thing`.
  choix : une modification d'objet de module réécrit tout l'enregistrement qui recouvre
  sa période (pas de découpe dans `module_object`, le module découpe ses propres lignes) ;
  l'affectation de propriétés à un objet de module reste à la tâche 5 (porteurs).
- [ ] 5. Ancres et RBAC des objets de module : porteurs et cibles de référence étendus
  par le registre, `core.owner`/`core.site` portés par les types de module, arbre
  `InheritTree::Module` le long de `parent_id` dans la résolution matérialisée,
  `carrier_object`, `kartenn_site`, `kartenn_visible`/`kartenn_readable` (migration
  `0042`) — REQ-008-6, 7 — verify : `a_reader_on_an_entity_sees_its_module_objects`,
  `a_site_binding_covers_module_objects_of_child_sites`,
  `a_domain_anchored_module_object_is_in_the_domain_scope`,
  `a_module_object_inherits_owner_from_its_parent`, `ref_to_a_module_type`
  où : design « Données du noyau » ; `kartenn-core/src/properties.rs` (`REF_TARGETS`,
  `CORE_CARRIERS`, `InheritTree`), `declarations.rs::core_declarations`,
  `resolution.rs`, store `origins.rs`, migrations `0022`, `0028` à `0030`, `0033` ;
  test voisin `kartenn-store-pg/tests/` (rbac)
  dépend de : 3
- [ ] 6. Catalogues fusionnés : permissions (`valid_role_entry`, `grants`, rôles livrés
  de lecture), règles, génération de `docs/spec/permissions.md` avec les modules
  livrés — REQ-008-8 — verify : `a_role_with_a_module_permission`,
  `unknown_module_permission_refused`, test de génération existant étendu
  où : design « Registre de modules » ; `kartenn-core/src/authz.rs` (`PERMISSIONS`,
  `BuiltinRole`), `rbac.rs::check`, test de `docs/spec/permissions.md`
  dépend de : 1
- [x] 7. Références et suppression : `Reference` des modules interrogées à la clôture de
  tout objet, refus listant les dépendants, cascade (clôture ou mise à nul) sous les
  droits de chaque dépendant — REQ-008-10 — verify (module factice) :
  `closing_a_referenced_core_object_is_refused_with_dependents`,
  `cascade_closes_module_dependents`, `nullify_policy_unassigns`
  où : design « Références » ; `cascade.rs::dependents`, `pipeline.rs::close`
  dépend de : 4
  — Service : `module.rs` (`ModuleHandler::{referrers, nullify}`, sans effet par
  défaut), `cascade.rs` (`referrers` interroge les références déclarées vers la sorte
  de l'objet clos ; la cascade ajoute la clôture des sources sous `Cascade`,
  récursivement, et le `ModulePlan` de mise à nul sous `Nullify` ; `check_referrers` sur
  l'état final), `pipeline.rs` (registre passé à `expand`, contrôle après la validation
  globale) ; règle `core.object.dependents` (catalogue régénéré). Tests :
  `module_pipeline.rs` (les trois du verify ; la mise à nul exige `dummy.thing.write`),
  module factice : référence au préfixe (`Cascade`) et au domaine (`Nullify`).
  choix : identifiant `core.object.dependents` (le design écrit `core.dependents`, le
  catalogue impose trois segments) ; sans cascade, toute référence vivante refuse ; avec
  cascade, `Refuse` refuse encore, `Cascade` clôt, `Nullify` détache par une
  modification que le module construit (`nullify` sans plan : refus) ; le refus est
  vérifié sur l'état final, après les écritures.
- [ ] 8. REST du contrat : montage des routeurs de modules sous `/v1`, routes
  transverses par type de module (`history`, `properties`, `explanation`,
  `suggestion`), enveloppe d'opération dans le contrat OpenAPI, `GET /v1/graph/edges`,
  contrat de référence régénéré — REQ-008-9, 11 — verify : `module_routes_are_mounted`,
  `transversal_routes_for_module_types`, `graph_edges_filtered_by_rights`, comparaison
  au contrat de référence (additive)
  où : design « Lectures et routes » ; `kartenn-rest/src/lib.rs` (`router`, `v1`),
  `resources.rs`, `properties.rs`, `versions.rs`, `contract.rs`, `openapi.rs`
  dépend de : 4, 5
- [x] 19. Contributions au différentiel : `Module::diff`, `DiffContext` (différentiel des
  propriétés, objets touchés typés, sections précédentes), ordre du registre, table
  `changeset_diff_section` (migration), sections dans la réponse de simulation et
  `GET /v1/changesets/{id}/diff?section=`, `CheckContext` des règles — REQ-008-26 —
  verify (deux modules factices A et B) : `b_reads_the_section_of_a`,
  `sections_in_simulation_and_after_application`, `a_rule_of_b_reads_a_section`,
  `sections_are_paginated`
  où : design « Enveloppe d'opération » (sections) ; `pipeline.rs::stages` (étape du
  différentiel des propriétés), store `diff.rs`, migration `0003` (`changeset_diff`),
  `0015` ; rest `cycle.rs::diff`
  dépend de : 3
  — Service : `module.rs` (`DiffRow`, `DiffSection`, `DiffContext`, `CheckContext`,
  `ModuleHandler::diff`, `check` sur `CheckContext`), `sections.rs` (objets touchés
  typés, contributions dans l'ordre du registre, nom `<module>.<nom>` vérifié),
  `pipeline.rs` (sections puis règles des modules après le différentiel des propriétés ;
  `Outcome::Simulated::sections`, `SectionSample`), `port.rs::record_section` ; store
  `sections.rs` (table temporaire, copie au journal, `section_counts`, `section_page`),
  migration `0043_changeset_diff_section.sql` ; REST : `sections` dans la simulation et
  le différentiel, `?section=` paginé, contrat de référence et types du SDK régénérés.
  Tests : `kartenn-service/tests/module_diff.rs` (les quatre du verify, modules
  `alpha` et `beta` de `support/section_modules.rs`), `kartenn-rest/tests/diff_sections.rs`
  (simulation, comptes, pages par liens).
  choix : `diff` et `check` sur `ModuleHandler` (asynchrones, comme les autres étapes)
  plutôt que sur `Module` ; les règles des modules passent après le différentiel des
  propriétés (elles lisent les sections), chaque module une fois dans l'ordre du registre,
  même sans objet écrit ; sections stockées pour un changeset appliqué seulement, comme
  `changeset_diff` ; réponse du différentiel en `oneOf` (`DiffAnswer`) selon `section=` ;
  migration `0043` (la `0042` est prévue par la tâche 5).
- [x] 20. Obligations d'administrateur sur une propriété de module : colonne `origin` de
  `property_obligation`, opérations `AddObligation`/`RemoveObligation` (sous
  `core.property.admin`, contrôle de REQ-003-6), `sync_operations` limité aux
  obligations du code — REQ-008-28 — verify :
  `admin_obligation_survives_module_sync`, `incompatible_obligation_refused`
  où : design « Erreurs et cas limites » ; `declarations.rs::{sync_operations,
  sync_modules}`, migration `0011` (`property_obligation`) ; test voisin des
  déclarations dans `kartenn-service/tests/`
  dépend de : 1
  — Migration `0044_property_obligation_origin.sql` (`origin`, `module` par défaut,
  ajoutée aussi à l'historique) ; store `obligations.rs` (`obligations_at` par origine,
  `set_obligations` limité à `module` et sans doublon d'une obligation d'administrateur,
  `admin_obligation`), `write.rs` (`declaration` lit toutes les obligations,
  `declarations_in` celles du module, `modify` de `Change::Obligation`) ; service :
  `Operation::{AddObligation, RemoveObligation}` planifiées en `Planned::Modify` d'une
  propriété (droit `core.property.admin`), `Change::Obligation`, contrôles de
  `declarations.rs` (`check_admin_obligation`, `admin_obligation_held`) ; REST :
  `add_obligation`, `remove_obligation`, contrat de référence et types du SDK régénérés.
  Tests : `declarations.rs` (les deux du verify).
  choix : l'opération d'administrateur porte une seule portée (forme de `scope` des
  déclarations) ; ajouter une portée déjà requise, ou retirer une portée qu'aucun
  administrateur n'a posée, est refusé (`prop.declaration.incompatible`) ; une obligation
  d'administrateur s'applique comme celle d'une déclaration.

## P2 — Module VLAN

- [x] 9. Crate `kartenn-vlan` et domaine pur : VID (1–4094), nom (NFC, 1–64, sans
  contrôle), plage d'un groupe, occupation en tranches et prochain libre, règles
  (`Rule` statiques) ; `workspace-hack` à jour — REQ-008-14, 20 (calcul) — verify :
  tests unitaires `vid_bounds`, `name_rules`, `occupancy_slices`,
  `next_free_from`, `exhausted_group`
  où : design « Données du module », « Règles » ; modèle `kartenn-core/src/occupancy.rs`,
  `rules.rs` ; `docs/practices/rust-build.md`
  dépend de : —
  — `vlan.rs` (`Vid`, `VlanName`, `VlanStatus`), `group.rs` (`VidRange` et `excluded`
  pour REQ-008-13, `occupancy`, `next_free`), `rules.rs` (`VLAN_RULES`, les neuf règles
  du design) ; tests unitaires dans chaque module, cas d'acceptation de REQ-008-20
  compris.
  choix : `deprecated` compte comme utilisé dans l'occupation ; le nom est normalisé
  NFC avant d'être compté ; macro `rules!` recopiée (privée dans `kartenn-core`).
- [ ] 10. Groupes : migration `0001_vlan.sql` (schéma `vlan`, `group`), adaptateur `pg`,
  gestionnaire `vlan.create_group`/`vlan.update_group` (ancres, `core.owner` et
  `core.site` ajoutés au plan, déplacement d'ancre), feature `vlan` de
  `kartenn-server` — REQ-008-12, 13 — verify : `group_requires_an_owner`,
  `domain_anchor_defaults_owner`, `slug_unique_over_time`,
  `range_cannot_exclude_existing_vlans`, `moving_the_anchor_reclassifies`
  où : design « Données du module », « Opérations » ; tâche 3 (module factice) comme
  modèle de gestionnaire
  dépend de : 2, 3, 5, 9
- [ ] 11. VLAN : table `vlan.vlan`, `vlan.create_vlan`/`vlan.update_vlan`, unicités
  (verrou consultatif par groupe, exclusion en dernier recours), clôture — REQ-008-14,
  15, 16 — verify : `concurrent_same_vid_one_wins`, `same_vid_two_groups`,
  `vid_reused_after_closure`, `move_to_group_with_taken_vid_refused`,
  `name_duplicate_refused`
  où : design « Opérations », « Erreurs et cas limites »
  dépend de : 10
- [ ] 12. Lien aux réseaux : déclaration `vlan.vlan`, règle `vlan.prefix.not_network`,
  avertissements `vlan.site.mismatch`/`vlan.domain.mismatch` (anomalies) — REQ-008-17,
  18 — verify : `vlan_on_container_refused`, `network_to_container_with_vlan_refused`,
  `site_mismatch_is_an_anomaly`, `v4_and_v6_networks_on_one_vlan`
  où : design « Données du module » (propriété), « Règles » ; `declarations.rs`
  (`sync_modules`) ; anomalies `kartenn-store-pg/src/anomalies.rs`
  dépend de : 11
- [ ] 13. Références du module : groupe → VLAN, `vlan.vlan` → VLAN, ancre → objets du
  noyau ; cascade — REQ-008-10 — verify : `closing_a_vlan_on_a_network_lists_it`,
  `cascade_unassigns_vlan_property`, `closing_a_group_cascades_its_vlans`,
  `closing_an_anchor_entity_refused`
  où : design « Références »
  dépend de : 7, 12
- [ ] 14. Routes REST du module : groupes et VLAN (liste filtrée, `QUERY`, création,
  Merge Patch, clôture), occupation, prochain libre, réseaux d'un VLAN, permissions —
  REQ-008-19, 20, 21, 22 — verify : `vlans_filtered_by_vid_range`,
  `occupancy_over_http`, `next_free_404_when_exhausted`,
  `prefixes_of_a_vlan_hide_unreadable`, `writer_on_other_entity_gets_403`,
  `patch_needs_if_match`
  où : design « Lectures et routes » ; `kartenn-rest/src/objects.rs`, `tree.rs`
  (occupation, prochain libre), `filter.rs`, `dated.rs`, `cursor.rs`
  dépend de : 8, 11, 12
- [ ] 15. Imports : opérations du module dans les imports par morceaux — REQ-008-23 —
  verify : `import_ten_thousand_vlans_resumes_without_duplicates`
  où : `kartenn-rest/src/imports.rs`, `kartenn-service/src/import.rs` ; test voisin
  `kartenn-rest/tests/imports.rs`
  dépend de : 14

## P3 — Budgets, interface, synchronisation

- [ ] 16. Banc : jeu de référence augmenté (2·10⁴ groupes, 10⁵ VLAN, moitié avec
  réseaux), mesures lecture, page filtrée, occupation, création ; seuils bloquants —
  REQ-008-25 — verify : `bench-core` vert en CI (sous-agent `bench-runner`)
  où : `kartenn-bench` (jeu, `core`), 002/A14
  dépend de : 14
- [ ] 17. Plugin `web/plugins/vlan` : vues groupes et VLAN, onglet et colonne sur
  `prefix`, action « créer un VLAN », catalogues FR/EN — REQ-008-24 — verify :
  parcours Playwright `web/e2e/journeys/vlan.spec.ts` avec axe-core ; plugin absent
  d'une build sans module
  où : design « Interface » ; plugin d'exemple `web/plugins/example` (007, tâche 23),
  007/design « Plugins côté serveur »
  dépend de : 14 ; 007 : 10, 12, 15, 23
- [ ] 18. Synchronisation : `docs/spec/` (architecture : contrat de module ; data :
  `module_object`, schéma `vlan` ; interfaces : routes et opérations ; permissions),
  ADR 0026 acceptée, matrice CI serveur sans module et avec, spec passée à
  `implemented` — verify : rapport `spec-keeper` sans écart bloquant
  où : skill `tech-specs` (mode sync)
  dépend de : 1 à 17, 19, 20

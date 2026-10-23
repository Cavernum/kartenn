---
status: approved
updated: 2026-10-23
---

# 018 — Retours d'usage de la v0.2 : tâches

Spec validée le 2026-10-23 ([`decisions.md`](decisions.md), Q-018-1 à 8 arbitrées).

Chaque tâche est un commit, laisse le build vert (`k-check` ; `k-web-check` quand elle
touche `web/` ; `k-py-check` quand elle touche `clients/python/`) et nomme ce qu'elle
implémente. « verify » = test qui prouve la tâche ; « où » = fichiers et symboles à
toucher ou à imiter ; « dépend de » = tâches à finir avant (`022-n` : tâche de la 022).

**Trois flux parallèles** (R25), un sous-agent chacun au plus :

| Flux | Tâches | Fichiers dominants |
|---|---|---|
| A — Permissions | 1 → 2 → 3 → 4 | `kartenn-core/src/authz.rs`, catalogues de module, tests de droits, manifestes |
| B — Libellés | 5 → 6 → 7 → 8 ; 9 (après 2) ; 10 → 11, 12, 13 | migrations, `operation.rs`, `properties.rs` (écritures), route `labels`, `clients/python` |
| C — Suggestions | 14 (après 6) → 15 → 16 | `properties.rs` (`/values`), `filter.rs`, routes des collections, `rules.rs` |

Points de rencontre : 9 attend 2 (mêmes catalogues) ; 14 attend 6 (colonne `labels`) ;
11 attend 022-3 et 022-8 (sortes et répartiteur de la 022). La tâche 17 clôt. Le flux A
et la tâche 10 sont à livrer avant les tâches web de la 021 et avant 017-2, 017-3 (R18).

## Flux A — Permissions

- [x] 1. Grammaire et table : `RENAMED_PERMISSIONS`, refus des anciens noms par
  `valid_role_entry` (`core.role.invalid`), `type_read_permission` et
  `type_write_permission` selon R2, test des catalogues (pas de `m.m.a`, aucune ressource
  nommée comme une action, cibles connues), migration SQL de la contrainte de
  `role_permission.permission` — REQ-018-1, 3 ; R3 à R5 — verify :
  `permission_names_omit_the_module_resource`, `renamed_permissions_target_the_catalogue`,
  `an_old_permission_name_is_refused`, `a_module_kind_reads_under_the_short_permission` ;
  test de migration (insérer `flow.read`)
  où : `crates/kartenn-core/src/authz.rs` (`PERMISSIONS`, `permission`,
  `valid_role_entry`, `type_permissions`, l. 133-195, tests l. ~369) ;
  `crates/kartenn-store-pg/migrations/0028_rbac.sql` (contrainte, l. 29) pour la
  nouvelle migration ; design « A. Permissions »
  dépend de : —
  fait : `RENAMED_PERMISSIONS`, `catalogue_problems` (grammaire : deux ou trois
  segments, pas de `m.m.a`, aucune ressource nommée comme une action, doublons) et
  migration `0058_permission_names.sql` (la contrainte n'existe que sur la table :
  l'historique, créé par `LIKE`, n'en porte pas ; test
  `a_role_entry_may_omit_the_module_resource`). Pour garder chaque commit vert, le refus
  explicite des anciens noms, `type_*_permission` selon R2 et les tests qui supposent le
  catalogue renommé (`renamed_permissions_target_the_catalogue`,
  `an_old_permission_name_is_refused`, `a_module_kind_reads_under_the_short_permission`)
  passent avec la tâche 2.

- [ ] 2. Renommage dans les modules et le service : catalogues `vlan`, `machines`,
  `flow` (dont `flow.waiver.grant`), rôles livrés, appels de contrôle, tests et bancs ;
  `docs/spec/permissions.md` régénéré avec « Anciens noms » — REQ-018-2, 6 — verify :
  `k-check` complet ; `the_published_catalogue_matches_the_code`
  (`kartenn-server/src/modules.rs`) ; test d'absence des anciens noms hors exceptions
  où : `crates/kartenn-{vlan,machines,flow}/src/module.rs` (l. 124, 168, 92) ;
  `crates/kartenn-core/src/authz.rs` (`BUILTIN_ROLES`, `catalogue_markdown`) ;
  `crates/kartenn-{service,rest,flow,policy,vlan,machines}/src/**` ;
  `crates/*/tests/it/**` ; `crates/kartenn-bench/src/{*_budgets.rs,rbac.rs}`
  dépend de : 1

- [ ] 3. Migration des rôles personnalisés : étape `rename_permissions` du job de
  migration, changeset `système`, motif `permission rename 018`, idempotente —
  REQ-018-4, 5 ; Q-018-3 — verify : `custom_roles_take_the_new_names_once` (liaisons et
  droits identiques, second passage sans changeset),
  `a_role_read_before_the_rename_keeps_the_old_name`
  où : `crates/kartenn-service/src/declarations.rs:607` (`sync_modules`, forme du
  changeset `système`) ; séquence des migrations dans `crates/kartenn-server/src/` ;
  écriture des rôles `crates/kartenn-store-pg/src/rbac_admin.rs` (l. 85, 148-170) ;
  tests à imiter `crates/kartenn-service/tests/it/rbac.rs`
  dépend de : 2

- [ ] 4. Clients et docs du renommage : manifestes `web/plugins/{vlan,machines}`,
  description de `waive_flow` (`openapi.rs`, `openapi-v1.json`, `schema.d.ts` régénéré),
  `docs/spec/{interfaces,invariants}.md`, ADR 0026 (exemples), amendement de l'ADR 0010,
  005/K12, 011/F12 — REQ-018-6 — verify : `k-web-check` ; test de contrat ; test
  d'absence des anciens noms étendu à `web/`, `clients/`, `docs/spec/`
  où : `web/plugins/{vlan,machines}/public/manifest.json` ;
  `crates/kartenn-rest/src/openapi.rs` ; `specs/006-api-rest/openapi-v1.json` ;
  `web/sdk/src/api/schema.d.ts` ; `docs/adr/{0010-rbac.md,0026-contrat-de-module.md}` ;
  `specs/005-authn-rbac/decisions.md` (K12) ; `specs/011-registre-flux/decisions.md`
  (F12)
  dépend de : 2

## Flux B — Libellés

- [ ] 5. ADR 0040 « Dictionnaire de libellés » (acceptée avec la spec) et note de
  référence (RFC 5646 §2.1, RFC 4647 §3.4, RFC 9110 §8.5, §12.5.4, §12.5.5 ; normatif /
  choix de kartenn) — R19 à R24 — verify : relecture de l'utilisateur
  où : `docs/adr/0035-clients-decouverts-depuis-le-contrat.md` (forme) ;
  `docs/reference/noms-dates-texte.md` (forme des notes)
  dépend de : —

- [ ] 6. Schéma : `labels jsonb` sur les tables propriétaires, reprise des `label` puis
  suppression des colonnes, `policy_value.label` vers `property_enum_value.labels`,
  `label_revision`, règle `core.label.invalid`, `[i18n] default_locale` publiée aux
  capacités, `Locale` au noyau — REQ-018-8, 12, 13 ; Q-018-5 à 7 — verify :
  `labels_are_checked`, `existing_labels_become_default_language_labels`,
  `environment_labels_live_in_the_core`, `the_default_locale_is_configured` ; tests de
  la politique verts
  où : `crates/kartenn-store-pg/migrations/{0011_properties.sql,0028_rbac.sql}` ;
  `crates/kartenn-policy/migrations/0001_policy.sql` ;
  `crates/kartenn-policy/src/{codec.rs:22,model.rs:484,handler.rs:172,318-353}` ;
  `crates/kartenn-rest/src/capabilities.rs` ; configuration du serveur
  (`crates/kartenn-server/src/`, section `[web]` à imiter)
  dépend de : 5

- [ ] 7. Écritures : `labels` sur `declare_property`, `update_property`,
  `add_enum_value`, rôles (`PUT /v1/roles/{key}` : `{labels, entries}`) et opérations de
  la politique ; `update_enum_value` ; corps neutres (plus de `label`) ; incrément de
  `label_revision` et événement d'outbox `org.kartenn.labels.changed` ; shell actuel
  (`RolesView`) adapté pour rester vert — REQ-018-7, 9 — verify :
  `relabelling_a_value_is_a_changeset`, `a_value_key_never_changes`,
  `a_body_does_not_vary_with_the_language`, `a_label_write_bumps_the_revision` ;
  `k-web-check`
  où : `crates/kartenn-rest/src/operation.rs` (`Operation`, `samples`, l. ~1340) ;
  `crates/kartenn-rest/src/{properties.rs,rights.rs:265}` ;
  `crates/kartenn-policy/src/{handler.rs,rest.rs:111-280}` ; outbox (ADR 0018) dans
  `crates/kartenn-store-pg/src/` ; `web/shell/src/admin/{RolesView.vue,write.ts}`
  dépend de : 6

- [ ] 8. Libellés des modules et des rôles livrés : descripteurs avec `labels`,
  `sync_modules` sans écraser un libellé d'administrateur, `BuiltinRole` FR/EN —
  REQ-018-11, 13 ; R10 — verify : `an_administrator_label_survives_the_module_sync`,
  `builtin_roles_have_french_and_english_labels`
  où : `crates/kartenn-service/src/declarations.rs:607` ; descripteurs
  (`crates/kartenn-policy/src/module.rs`, propriétés déclarées) ;
  `crates/kartenn-core/src/authz.rs` (`BuiltinRole`, l. ~200)
  dépend de : 7

- [ ] 9. Libellés compilés FR/EN : descriptions des permissions, résumés des règles,
  titres des problèmes, sortes, états, natures et usages ; fonction `labels()` du
  contrat de module ; empreinte calculée au démarrage — REQ-018-10 ; R8 — verify :
  `every_code_has_french_and_english_labels` (permissions, règles, problèmes, sortes,
  états)
  où : `crates/kartenn-core/src/{authz.rs,rules.rs:31,97}` ;
  `crates/kartenn-rest/src/problem.rs` (`ProblemKind::title`) ;
  `crates/kartenn-{vlan,machines,policy,flow}/src/{module.rs,rules.rs}` ;
  `docs/adr/0026-contrat-de-module.md` (clause additive)
  dépend de : 2

- [ ] 10. `GET /v1/labels` : espaces, négociation (`lang`, `Accept-Language`, Lookup),
  `Content-Language`, `Vary`, repli signalé, ETag `"<révision>.<empreinte>"`, 304,
  lecture datée et `Cache-Control`, filtrage par droits (R24), mise en mémoire par
  instance, contrat — REQ-018-14 à 17, 19 — verify : `labels_are_negotiated`,
  `a_missing_label_falls_back_to_the_default_language`, `the_labels_etag_changes_with_the_revision`,
  `labels_read_at_a_date`, `a_space_is_served_only_to_its_readers`
  où : `crates/kartenn-rest/src/` (route nouvelle `labels.rs` ; `properties.rs` et
  `capabilities.rs` à imiter ; `openapi.rs`) ; ETag de 006/H19 (`crates/kartenn-rest/src/`,
  fonction d'étiquette composite) ; `specs/006-api-rest/openapi-v1.json`
  dépend de : 8, 9

- [ ] 11. Événement SSE `labels` : ligne de la table de traduction outbox → SSE, remise à
  toute connexion, contrat OpenAPI de `/v1/events` ; amendement de 022/R2 —
  REQ-018-18 ; R23 — verify : `a_label_change_is_pushed_as_a_labels_event` (vrai
  serveur, flux ouvert)
  où : `kartenn-service::events` (022-3, sortes) et le répartiteur (022-8) ;
  `specs/022-evenements-sse/{decisions.md (R2),design.md (« Format »)}`
  dépend de : 7, 022-3, 022-8

- [ ] 12. Bibliothèque et CLI Python : `client.labels(lang)`, cache par révision dans le
  cache XDG, `If-None-Match`, invalidation par `labels` et `ready`, `resolve` avec repli ;
  CLI : libellés en table, clés avec `--keys` et en JSON ; amendement de 023/T15 —
  REQ-018-21 — verify : pytest `test_labels_cache_by_revision`,
  `test_labels_event_invalidates`, `test_cli_shows_labels_in_tables` ; `k-py-check`
  où : `clients/python/kartenn/src/kartenn/{catalog/cache.py,client.py,events.py}` ;
  `clients/python/kartenn-cli/src/kartenn_cli/main.py` ;
  `specs/023-client-python-cli/decisions.md` (T15)
  dépend de : 10 (11 pour l'invalidation par événement)

- [ ] 13. Banc du dictionnaire : complet par langue p95 < 50 ms, 304 p95 < 10 ms, taille
  < 1 Mo sur le jeu des bancs — REQ-018-20 — verify : `bench-core` dans les budgets
  où : `crates/kartenn-bench/src/{core_budgets.rs,http.rs}`
  dépend de : 10

## Flux C — Suggestions

- [ ] 14. `/values` : `{key, valid_from, valid_to}`, `valid_at`, `q` (clé et libellés,
  préfixe insensible à la casse), `cursor`, `page_size` (20) — REQ-018-22 — verify :
  `values_are_suggested_by_prefix_at_a_date`
  où : `crates/kartenn-rest/src/properties.rs` (`EnumValueItem`, route l. ~280-286) ;
  lecture des valeurs dans `crates/kartenn-store-pg/src/`
  dépend de : 6

- [ ] 15. `q` sur les collections citables (entités, sites, domaines, machines,
  interfaces, clusters, `vlan-groups`, VLAN, préfixes, adresses) sous RBAC, index
  manquants ; `GET /v1/rules` — REQ-018-23 — verify :
  `a_suggestion_never_shows_an_unreadable_object`, `the_rule_catalogue_is_served` ;
  test de contrat
  où : `crates/kartenn-rest/src/filter.rs` ; routes des collections
  (`crates/kartenn-rest/src/`, `crates/kartenn-{machines,vlan}/src/rest.rs`) ;
  `crates/kartenn-core/src/rules.rs` (`catalogue`, l. 290-320) ;
  `crates/kartenn-rest/src/openapi.rs`
  dépend de : 14

- [ ] 16. Banc des suggestions : préfixe sur le jeu complet, p95 < 50 ms par route —
  REQ-018-23 — verify : `bench-core` dans les budgets
  où : `crates/kartenn-bench/src/{core_budgets.rs,machine_budgets.rs,http.rs}`
  dépend de : 15

## Clôture

- [ ] 17. Synchronisation : `docs/spec/` (`data.md`, `interfaces.md`, `invariants.md`,
  `regles-noyau.md`, `evenements-audit.md`), amendements des specs 003, 006, 007 (W11),
  008, 009, 010, 012, 017, 022, 023 (design, « Amendements d'autres specs »),
  `specs/README.md` — REQ-018-6 — verify : `spec-keeper` sur la 018 sans écart
  où : design, « Amendements d'autres specs » et « Impact sur la 017 »
  dépend de : 4, 8, 10, 11, 12, 13, 16

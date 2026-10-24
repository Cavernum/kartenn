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

- [x] 2. Renommage dans les modules et le service : catalogues `vlan`, `machines`,
  `flow` (dont `flow.waiver.grant`), rôles livrés, appels de contrôle, tests et bancs ;
  `docs/spec/permissions.md` régénéré avec « Anciens noms » — REQ-018-2, 6 — verify :
  `k-check` complet ; `the_published_catalogue_matches_the_code`
  (`kartenn-server/src/modules.rs`) ; test d'absence des anciens noms hors exceptions
  où : `crates/kartenn-{vlan,machines,flow}/src/module.rs` (l. 124, 168, 92) ;
  `crates/kartenn-core/src/authz.rs` (`BUILTIN_ROLES`, `catalogue_markdown`) ;
  `crates/kartenn-{service,rest,flow,policy,vlan,machines}/src/**` ;
  `crates/*/tests/it/**` ; `crates/kartenn-bench/src/{*_budgets.rs,rbac.rs}`
  dépend de : 1
  fait : catalogues renommés (`WAIVER_GRANT` = `flow.waiver.grant`, opération `waive`
  inchangée), `REVIEW` du gestionnaire de flux, lecture du VLAN par `machines` ; refus
  explicite des anciens noms dans `valid_role_entry`, le message de `core.role.invalid`
  nomme le nouveau nom (pas d'alias) ; `type_*_permission` selon R2 (`m.m` → `m.<action>`).
  Choix : `machines.machine` n'est pas `m.m` au sens strict (pluriel) ; plutôt qu'une
  règle de pluriel, le module `machines` déclare ses `TypePermissions`
  (`machines.read`, `machines.write`). Tests du noyau
  (`permission_names_omit_the_module_resource`, `an_old_permission_name_is_refused`,
  `a_module_kind_reads_under_the_short_permission`), du serveur sur le catalogue du
  build (mêmes noms, `renamed_permissions_target_the_catalogue`,
  `no_old_permission_name_is_left` sur `crates/`) et du service (anciens noms refusés
  par `a_role_names_known_permissions_only`). `docs/spec/permissions.md` régénéré avec
  « Anciens noms » ; la description de `waive_flow` (dans `kartenn-flow/src/rest.rs`)
  change ici, avec `openapi-v1.json` et `schema.d.ts` régénérés, pour garder le test de
  contrat vert.

- [x] 3. Migration des rôles personnalisés : étape `rename_permissions` du job de
  migration, changeset `système`, motif `permission rename 018`, idempotente —
  REQ-018-4, 5 ; Q-018-3 — verify : `custom_roles_take_the_new_names_once` (liaisons et
  droits identiques, second passage sans changeset),
  `a_role_read_before_the_rename_keeps_the_old_name`
  où : `crates/kartenn-service/src/declarations.rs:607` (`sync_modules`, forme du
  changeset `système`) ; séquence des migrations dans `crates/kartenn-server/src/` ;
  écriture des rôles `crates/kartenn-store-pg/src/rbac_admin.rs` (l. 85, 148-170) ;
  tests à imiter `crates/kartenn-service/tests/it/rbac.rs`
  dépend de : 2
  fait : `kartenn_service::rbac::rename_permissions` (modelée sur `sync_roles`, motif
  `RENAME_MOTIVE`), appelée par `maintenance::deploy` après `sync_roles` ; lecture par
  le nouveau port `WriteTx::custom_roles_citing` (rôles non livrés en vigueur qui citent
  un ancien nom, `rbac_admin.rs`). Un seul changeset pour tous les rôles concernés,
  borné par `MAX_OPERATIONS` comme `sync_roles`. Tests dans
  `kartenn-service/tests/it/rbac.rs` : droits identiques sous le nouveau nom, ancien nom
  plus accordé, audit par rôle, second passage nul ; lecture datée et historique gardent
  l'ancien nom.

- [x] 4. Clients et docs du renommage : manifestes `web/plugins/{vlan,machines}`,
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
  fait : manifestes `vlan` et `machines` ; `interfaces.md`, `invariants.md` ; ADR 0026
  (exemples, amendement 5) et ADR 0010 (amendement) ; amendements datés de 005/K12,
  008/V13 et 011/F12. Noms remplacés aussi dans les exigences, conceptions et tâches
  de 008, 009, 011, 016 et 017 ; les journaux de décision gardent les anciens noms
  (historique), sauf les amendements ajoutés. La description de `waive_flow` et le
  contrat régénéré sont passés avec la tâche 2. `no_old_permission_name_is_left`
  couvre `crates/`, `web/`, `clients/`, `docs/spec/` (exceptions : `authz.rs`,
  `docs/spec/permissions.md`). Rien à changer dans `clients/python` (aucun nom codé ;
  l'instantané du catalogue ne porte pas les descriptions).

## Flux B — Libellés

- [x] 5. ADR 0040 « Dictionnaire de libellés » (acceptée avec la spec) et note de
  référence (RFC 5646 §2.1, RFC 4647 §3.4, RFC 9110 §8.5, §12.5.4, §12.5.5 ; normatif /
  choix de kartenn) — R19 à R24 — verify : relecture de l'utilisateur
  où : `docs/adr/0035-clients-decouverts-depuis-le-contrat.md` (forme) ;
  `docs/reference/noms-dates-texte.md` (forme des notes)
  dépend de : —
  fait : `docs/adr/0040-dictionnaire-de-libelles.md` (acceptée, indexée dans
  `docs/spec/README.md`) et `docs/reference/langues-negociation.md` (indexée). Choix
  notés dans la note : `?lang=` sans liste ni poids, `q=0` écarté, étiquettes
  `grandfathered` refusées, casse normalisée en minuscules, `Accept-Language` mal formé
  ignoré (seul `lang` mal formé vaut 400). Citations relues par un outil de résumé :
  points à revérifier listés en fin de note.

- [x] 6. Schéma : `labels jsonb` sur les tables propriétaires, reprise des `label` puis
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
  fait : migrations `0060_labels.sql` (noyau : `labels` sur déclarations, valeurs,
  rôles ; `label_revision`) et `policy/0004_labels.sql` (six tables de la politique ;
  `policy_value.label` → `property_enum_value.labels`), lignes et historique relabellisés
  sans nouvelle version (déclencheur suspendu). `kartenn_core::labels` : `Locale`
  (grammaire de la RFC 5646 §2.1, minuscules, 35 caractères au plus, `truncations` pour
  Lookup), `Labels` (`parse`, `parse_required`, `stored`), `default_locale` ;
  `core.label.invalid` dans `CHANGESET_RULES`. `[i18n] default_locale` (`fr` par défaut)
  installé au démarrage (`I18n::install`, `run` et sous-commandes), publié par
  `GET /v1/capabilities` (`default_locale`), documenté dans
  `docs/exploitation/configuration.md`. Choix : (1) la suppression des colonnes `label`
  passe avec les écritures (tâche 7, seconde migration) pour garder ce commit vert ;
  d'ici là la politique écrit aussi `labels` ; (2) la reprise en SQL prend `fr`, seule
  langue des données de la v0.2 (la migration ne lit pas la configuration) ; (3) la
  langue par défaut est un réglage du processus (`OnceLock`, posé une fois au
  démarrage) plutôt qu'un paramètre de chaque appel ; (4) une table vide est admise
  (valeurs, déclarations), `parse_required` sert aux objets qui exigent un libellé.
  Outillage : `kartenn_testing::create_empty` (base vide, migrations jouées en partie
  par le test).

- [x] 7. Écritures : `labels` sur `declare_property`, `update_property`,
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
  fait : `labels` (table brute `RawLabels`, vérifiée en `Labels` par le pipeline) sur
  `DeclarationInput` (facultative : absente, aucune à la déclaration et celles en vigueur
  à la mise à jour), `add_enum_value`, `set_role` (`PUT /v1/roles/{key}` `{labels,
  entries}`) ; opération `update_enum_value` (`Planned::EnumLabels`, `Change::Labels` par
  `kartenn_portion`, droit `core.property.admin`). Lectures : `labels` sur les rôles et
  les déclarations. Politique : `labels` requis sur types de flux, services, motifs et
  règles, facultatif sur conditions et signaux ; `value.describe` et
  `environment.create` écrivent les libellés au noyau (`update_enum_value`,
  `add_enum_value`) ; cellule de matrice `labels` ; `policy.label.invalid` et le type
  `Label` disparaissent du modèle (les verdicts ne portent que des clés) ; la non-
  conformité perd `rule_label` (clé seule, R20). Migrations `0061_role_label_dropped.sql`
  et `policy/0005_label_dropped.sql` (dernière recopie, suppression des colonnes
  `label` et de `policy_value.label`). Révision : `PgTx` note toute écriture qui change
  `labels` (insertion non vide, `Change::Labels` différent, rôle, objet de module par
  `ModuleTx::labels_written`, clôture ou restauration d'une ligne d'une table à
  libellés) ; `journal_applied` incrémente `label_revision` une fois et écrit
  `org.kartenn.labels.changed` `{revision}` (entier de la base ; l'empreinte des libellés
  compilés s'y joint au dictionnaire, tâche 10). Shell : `RolesView` lit `labels`
  (langue de l'interface, sinon la première, sinon la clé) et écrit celle de
  l'interface ; données des parcours e2e en `labels`. Les migrations du noyau prennent
  0060 et 0061 (0059 est pris sur `main`).

- [x] 8. Libellés des modules et des rôles livrés : descripteurs avec `labels`,
  `sync_modules` sans écraser un libellé d'administrateur, `BuiltinRole` FR/EN —
  REQ-018-11, 13 ; R10 — verify : `an_administrator_label_survives_the_module_sync`,
  `builtin_roles_have_french_and_english_labels`
  où : `crates/kartenn-service/src/declarations.rs:607` ; descripteurs
  (`crates/kartenn-policy/src/module.rs`, propriétés déclarées) ;
  `crates/kartenn-core/src/authz.rs` (`BuiltinRole`, l. ~200)
  dépend de : 7
  fait : `kartenn_core::labels::Compiled { fr, en }` (avec la langue par défaut de
  l'instance, en anglais, quand ce n'est ni `fr` ni `en`, R9) ; `BuiltinRole.labels`,
  `sync_roles` compare aussi les libellés (port `WriteTx::role_labels`) ;
  `ModuleDeclarations.labels` (`<propriété>` ou `<propriété>/<valeur>`), renseigné pour
  le noyau (`core.owner`, `core.external_system`, `core.site`), `vlan`, `machines` (dont
  les rôles d'interface livrés) et `policy` (axes d'exemple et leurs valeurs) ;
  `sync_modules` lit les libellés en vigueur et leur auteur (port `WriteTx::labels_in`,
  auteur `système` ou non) : il écrit ceux du code s'ils manquent ou venaient du
  système, jamais ceux d'un administrateur. Choix : un libellé d'administrateur n'est pas
  complété des langues qui lui manquent (le compléter en ferait une version du système,
  que la synchronisation suivante remplacerait).

- [x] 9. Libellés compilés FR/EN : descriptions des permissions, résumés des règles,
  titres des problèmes, sortes, états, natures et usages ; fonction `labels()` du
  contrat de module ; empreinte calculée au démarrage — REQ-018-10 ; R8 — verify :
  `every_code_has_french_and_english_labels` (permissions, règles, problèmes, sortes,
  états)
  où : `crates/kartenn-core/src/{authz.rs,rules.rs:31,97}` ;
  `crates/kartenn-rest/src/problem.rs` (`ProblemKind::title`) ;
  `crates/kartenn-{vlan,machines,policy,flow}/src/{module.rs,rules.rs}` ;
  `docs/adr/0026-contrat-de-module.md` (clause additive)
  dépend de : 2
  fait : `kartenn_core::labels::CodeLabel` (espace, clé, `fr`, `en`) ;
  `kartenn_core::code_labels` : anglais des permissions et des règles du noyau en tables
  à côté de leurs définitions (le français reste le résumé de la définition, pas de
  réécriture des macros), sortes (`core.prefix`…), états (`changeset.*`,
  `changeset.kind.*`, `import.*`), natures et usages ; `English` et `of_module` pour les
  modules. `Module::labels()` (défaut vide, ADR 0026 amendement 7), rempli par `vlan`,
  `machines`, `policy` et `flow` (fichier `labels.rs` de chaque crate : permissions,
  règles, sortes, états `flow.*`, verdicts et issues) ; `ModuleRegistry::labels()`.
  `ProblemKind::labels()` : français, et l'anglais du `title`. `kartenn_rest::labels::
  CodeCatalogue::of(registry)` assemble le tout, ajoute les anciens noms des permissions
  renommées (« … (ancien nom de flow.read) », R6) et calcule l'empreinte (SHA-256 du
  rendu trié, 16 caractères hexadécimaux), une fois, au démarrage du routeur (tâche 10).
  Tests : `every_code_of_the_core_has_french_and_english_labels` (noyau, et aucune
  entrée anglaise orpheline), `every_code_has_french_and_english_labels`
  (`kartenn-server`, build complet), `the_problems_and_former_names_are_labelled`.

- [x] 10. `GET /v1/labels` : espaces, négociation (`lang`, `Accept-Language`, Lookup),
  `Content-Language`, `Vary`, repli signalé, ETag `"<révision>.<empreinte>"`, 304,
  lecture datée et `Cache-Control`, filtrage par droits (R24), mise en mémoire par
  instance, contrat — REQ-018-14 à 17, 19 — verify : `labels_are_negotiated`,
  `a_missing_label_falls_back_to_the_default_language`, `the_labels_etag_changes_with_the_revision`,
  `labels_read_at_a_date`, `a_space_is_served_only_to_its_readers`
  où : `crates/kartenn-rest/src/` (route nouvelle `labels.rs` ; `properties.rs` et
  `capabilities.rs` à imiter ; `openapi.rs`) ; ETag de 006/H19 (`crates/kartenn-rest/src/`,
  fonction d'étiquette composite) ; `specs/006-api-rest/openapi-v1.json`
  dépend de : 8, 9
  fait : `kartenn_rest::labels` (route `labels`, schéma `Labels`) ; lectures
  `kartenn_store_pg::labels_read` (`revision`, `stored` aux dates de la lecture,
  `held` : entrées et rôles détenus sur une portée quelconque) ; `Module::label_tables()`
  (ADR 0026, amendement 7 complété), rempli par `policy` (six espaces `policy.*` sous
  `policy.catalogue.read`) ; `temporal::snapshot` accepte `<schéma>.<table>`.
  Négociation : `lang` (400 si mal formée) sinon `Accept-Language` (poids, `q=0` et `*`
  écartés, plage mal formée ignorée), Lookup parmi les langues présentes (`fr`, `en`, la
  langue par défaut et celles des tables), puis la langue par défaut. Repli signalé dans
  `fallback` (`<espace>/<clé>`, `value/<propriété>/<valeur>`) ; entrée sans libellé
  absente. Révision `<compteur>-<empreinte des libellés compilés>`, ETag fort
  `"<révision>.<empreinte du corps>"` (`ETag::labels`), 304 ; `Content-Language`,
  `Vary: Accept-Language` ; `Cache-Control` `private, max-age=86400` à un `as_of` figé,
  `private, no-cache` sinon. Droits (R24) : `policy.*` et espaces de module sous leur
  permission, valeurs d'une propriété sensible sous `prop.<clé>.read`, rôles sous
  `core.rbac.read` sinon ceux de ses liaisons, codes à tous. Mise en mémoire par
  instance des lectures au présent (clé : base, révision, langue, espaces, droits ;
  60 s au plus, une valeur dont la validité commence plus tard apparaissant sans
  écriture). `valid_at` et `as_of` ne figurent dans le corps que s'ils sont demandés
  (un corps au présent reste identique d'une lecture à l'autre). `labels` ajouté à
  `CORE_COLLECTIONS`. Contrat et `schema.d.ts` régénérés.

- [x] 11. Événement SSE `labels` : ligne de la table de traduction outbox → SSE, remise à
  toute connexion, contrat OpenAPI de `/v1/events` ; amendement de 022/R2 —
  REQ-018-18 ; R23 — verify : `a_label_change_is_pushed_as_a_labels_event` (vrai
  serveur, flux ouvert)
  où : `kartenn-service::events` (022-3, sortes) et le répartiteur (022-8) ;
  `specs/022-evenements-sse/{decisions.md (R2),design.md (« Format »)}`
  dépend de : 7, 022-3, 022-8
  fait : la 022 (fusionnée) traduisait déjà `org.kartenn.labels.changed` en sorte
  `labels` (`kartenn_service::events`, `Kind::Labels`, remise sans filtre `types`,
  contrat de `/v1/events` et `REQ-022-3` amendés). Ajouts : le lecteur
  (`kartenn_rest::events::reader`) joint au compteur de l'outbox l'empreinte des
  libellés compilés du build, pour que la révision poussée soit celle que sert
  `GET /v1/labels` ; amendement daté de 022/R2 et ligne de la table de traduction du
  design de la 022 ; test `a_label_change_is_pushed_as_a_labels_event`
  (`kartenn-rest/tests/it/events.rs` : flux ouvert par une identité sans droit, rôle
  relibellé, `labels` reçu avec `revision` seule, égale à celle du dictionnaire).

- [x] 12. Bibliothèque et CLI Python : `client.labels(lang)`, cache par révision dans le
  cache XDG, `If-None-Match`, invalidation par `labels` et `ready`, `resolve` avec repli ;
  CLI : libellés en table, clés avec `--keys` et en JSON ; amendement de 023/T15 —
  REQ-018-21 — verify : pytest `test_labels_cache_by_revision`,
  `test_labels_event_invalidates`, `test_cli_shows_labels_in_tables` ; `k-py-check`
  où : `clients/python/kartenn/src/kartenn/{catalog/cache.py,client.py,events.py}` ;
  `clients/python/kartenn-cli/src/kartenn_cli/main.py` ;
  `specs/023-client-python-cli/decisions.md` (T15)
  dépend de : 10 (11 pour l'invalidation par événement)
  fait : `kartenn.labels` (`Dictionary` : `label`, `resolve` avec repli sur la clé ;
  `LabelCache` ; plan `fetch` sous `If-None-Match`) ; `Context.labels` (mémoire jusqu'à
  `forget_labels`, disque dans `<catalogue>/labels/` par serveur, profil et langue ; lu
  à une date : ni gardé ni conditionnel) ; `Client.labels(lang)` et
  `AsyncClient.labels(lang)` ; `events()`/`aevents()` rendent obsolètes les
  dictionnaires en mémoire à `labels`, `ready` et `reset` ; paramètre `profile` des
  clients (posé par `from_profile` et la CLI). CLI : `Output.labels` en table seulement
  (et sans `--keys`), `Runtime.label_of` : valeur de liste fermée d'une colonne de
  propriété par le dictionnaire (lu seulement pour une telle colonne ; sans lui, la
  clé), table `labels` dans la langue de la CLI ; `/v1/labels` hors des ressources du
  catalogue et exclu de la couverture de la CLI (`kartenn api labels`). Amendement daté
  de 023/T15. Tests : `test_labels_cache_by_revision`, `test_labels_event_invalidates`
  (`clients/python/kartenn/tests/test_labels.py`), `test_cli_shows_labels_in_tables`
  (`kartenn-cli/tests/test_reads.py`).

- [x] 13. Banc du dictionnaire : complet par langue p95 < 50 ms, 304 p95 < 10 ms, taille
  < 1 Mo sur le jeu des bancs — REQ-018-20 — verify : `bench-core` dans les budgets
  où : `crates/kartenn-bench/src/{core_budgets.rs,http.rs}`
  dépend de : 10
  fait : mesures dans `http.rs` (`bench-http`, qui sert le routeur réel avec
  l'authentification ; `bench-core` mesure le service sans HTTP) : dictionnaire complet
  d'une langue (`fr`, `en` en alternance, budget 50 ms), même lecture sous son `ETag`
  (304, budget 10 ms), taille non compressée (< 1 Mo) ; la lecture des libellés stockés
  sert de mesure « service ». Jeu des bancs : valeurs des propriétés de `props.rs`
  libellées en français et en anglais. `bench-http 1 small` (2026-10-24) : 12 060
  octets, p95 2,4 ms (complet) et 2,3 ms (304), tous les budgets tenus ; mesure à
  l'échelle de test, la pleine échelle reste à passer.

## Flux C — Suggestions

- [x] 14. `/values` : `{key, valid_from, valid_to}`, `valid_at`, `q` (clé et libellés,
  préfixe insensible à la casse), `cursor`, `page_size` (20) — REQ-018-22 — verify :
  `values_are_suggested_by_prefix_at_a_date`
  où : `crates/kartenn-rest/src/properties.rs` (`EnumValueItem`, route l. ~280-286) ;
  lecture des valeurs dans `crates/kartenn-store-pg/src/`
  dépend de : 6
  fait : `property_read::enum_values` rend `EnumValueRead` ; la période d'une valeur
  couvre ses versions connues à `as_of` (un libellé écrit sur une portion la découpe en
  versions, pas en périodes). `q` : `filter::prefix` (une fois, non vide, 100 caractères
  au plus, mis en minuscules), motif `LIKE` échappé (`list::like_prefix`), comparé à la
  clé et à `lower()` de chaque libellé de `labels` ; gardé dans le curseur avec les
  autres filtres. 20 par défaut : `Page::suggesting` (constante `SUGGESTION_SIZE`), qui
  ne touche pas une taille donnée ni celle d'un curseur ; `Page::only_prefix` pour une
  liste dont `q` est le seul filtre. Paramètre `q` déclaré au contrat ; `filters.ts` des
  deux SDK régénérés. CLI : `complete.prefixed_values` (complétion des valeurs de liste
  fermée) lit une page de 100 ; une liste entière se garde et se filtre localement, une
  liste plus longue se redemande par `q` pour chaque préfixe (cache `nom?q=…`). Écart
  hors tâche : `events::a_client_that_no_longer_reads_loses_its_socket` échoue sur cette
  machine chargée, sans lien avec la tâche (tampons TCP).

- [x] 15. `q` sur les collections citables (entités, sites, domaines, machines,
  interfaces, clusters, `vlan-groups`, VLAN, préfixes, adresses) sous RBAC, index
  manquants ; `GET /v1/rules` — REQ-018-23 — verify :
  `a_suggestion_never_shows_an_unreadable_object`, `the_rule_catalogue_is_served` ;
  test de contrat
  où : `crates/kartenn-rest/src/filter.rs` ; routes des collections
  (`crates/kartenn-rest/src/`, `crates/kartenn-{machines,vlan}/src/rest.rs`) ;
  `crates/kartenn-core/src/rules.rs` (`catalogue`, l. 290-320) ;
  `crates/kartenn-rest/src/openapi.rs`
  dépend de : 14
  fait : `q` comparé en `LIKE` (motif échappé, minuscules) au slug (entités, sites,
  domaines, clusters, `vlan-groups`), à `text(prefix)`, à `host(address)`, au nom
  (machines, déjà en minuscules ; interfaces par `lower(name)`), au nom ou au VID des
  VLAN ; sous le prédicat de lecture existant de chaque liste ; 20 par défaut
  (`kit::suggestion`, `Page::suggesting`), gardé par le curseur. Refusé (400) sur les
  plages (citées par identifiant) et avec un filtre `prop.…` (lu par identifiant).
  Contrat : `Op::suggestion` déclare `q` sur chaque liste citable (H20). `GET /v1/rules`
  (`kartenn-rest/src/rules.rs`, étiquette `operations`, `rules` ajouté à
  `CORE_COLLECTIONS`) : `{id, severity, module}` du noyau (`module` = `core`), des
  imports puis des modules du build, une page, `q` sur l'identifiant. Les codes du tri
  d'un import (`import.*`, 006/H18) n'étaient pas des règles du catalogue : groupe
  `IMPORT_RULES` de `kartenn-core` hors de `all_rules` (leur forme précède la grammaire
  à trois segments), section ajoutée à `regles-noyau.md`, libellés FR/EN dans l'espace
  `rule` du dictionnaire. Index : reportés à la mesure de la tâche 16 (« à mesurer »).
  CLI : `--domain <TAB>` et les autres références passent par `q` au-delà d'une page,
  repli sur la première page pour une liste sans `q` au contrat (services) ; la
  ressource `rules` apparaît dans l'arbre de la CLI.

- [x] 16. Banc des suggestions : préfixe sur le jeu complet, p95 < 50 ms par route —
  REQ-018-23 — verify : `bench-core` dans les budgets
  où : `crates/kartenn-bench/src/{core_budgets.rs,machine_budgets.rs,http.rs}`
  dépend de : 15
  fait : `machine_budgets::suggestions` (dans `bench-core`, après les modules, à travers
  le routeur avec le noyau, `vlan` et `machines`) : par route, 200 préfixes de 1 à 6
  caractères de clés existantes ; entités, sites, domaines, préfixes, adresses,
  machines, interfaces, clusters, groupes de VLAN, VLAN, valeurs d'une liste fermée,
  règles, puis les machines lues par le lecteur d'un seul propriétaire. Mesures et
  correctifs : (1) l'union avec l'historique, même vidée par `$2`, interdit au
  planificateur l'ordre d'un index : une liste courante (`as_of` absent) lit la table
  courante seule (`temporal::current_rows`, `listed` des modules ; le noyau passe
  désormais `moment.as_of` et non `Some(moment.as_of())`) ; (2) `q` est planifié avec sa
  valeur (instruction non nommée, `persistent(false)`), sans quoi `LIKE $n` n'atteint
  aucun index ; (3) préfixes et adresses suggérés dans l'ordre de leur texte puis du
  domaine (index `C` de la migration 0062) : dans l'ordre de la liste, un préfixe rare
  parcourait tous les domaines (39 ms pour « 3 ») ; (4) index `interface_keyset`,
  `interface_name_pattern` (machines 0004) et `vlan_group_keyset` (vlan 0002).
  `bench-core` réduit (2026-10-24 ; 1,1·10⁵ adresses, 1,1·10⁴ machines, 3,3·10⁴
  interfaces, 2 200 groupes, 11 000 VLAN) : p95 entités 5,6 ms, sites 5,2, domaines
  4,5, préfixes 6,5, adresses 6,4, machines 8,5, interfaces 3,8, clusters 7,4, groupes
  4,8, VLAN 7,9, valeurs 1,5, règles 0,02, machines d'un propriétaire 22,6 ; tous les
  autres budgets tenus. La pleine échelle reste à passer.

## Clôture

- [ ] 17. Synchronisation : `docs/spec/` (`data.md`, `interfaces.md`, `invariants.md`,
  `regles-noyau.md`, `evenements-audit.md`), amendements des specs 003, 006, 007 (W11),
  008, 009, 010, 012, 017, 022, 023 (design, « Amendements d'autres specs »),
  `specs/README.md` — REQ-018-6 — verify : `spec-keeper` sur la 018 sans écart
  où : design, « Amendements d'autres specs » et « Impact sur la 017 »
  dépend de : 4, 8, 10, 11, 12, 13, 16

---
status: in-progress
updated: 2026-10-21
---

# 014 — Bases de test clonées d'un gabarit : tâches

Mise en œuvre de l'[ADR 0028](../../docs/adr/0028-base-de-test-gabarit.md). Chaque tâche est
un commit et laisse le build vert. Les tâches 2 à 6 remplacent `#[sqlx::test(migrator =
"kartenn_store_pg::MIGRATOR")]` par `#[kartenn_testing::pg_test]` dans les tests d'une
crate. Elles touchent les mêmes fichiers que les tâches de la
[013](../013-binaires-de-tests/tasks.md) (ADR 0027). Chaque tâche de crate passe donc
**après** la tâche de la 013 sur cette crate, jamais en parallèle d'elle. La tâche 1 ne
touche aucun `tests/` existant.

Verify commun aux tâches 2 à 6 : même nombre de tests dans `cargo nextest list
--workspace --all-features` avant et après, `k-check` complet vert.

- [x] 1. Moteur et attribut. Crates `kartenn-testing` (moteur `run(test_path, variante,
  corps)`) et `kartenn-testing-macros` (attribut `#[kartenn_testing::pg_test]`),
  `publish = false`, dans le `workspace-hack` (hakari). Gabarit `_kt_tpl_<empreinte>`
  (SHA-256 des couples version et somme de contrôle de `MIGRATOR`) construit sous
  `pg_advisory_lock`, renommé, puis `ALLOW_CONNECTIONS false` et `IS_TEMPLATE true` ;
  clone `STRATEGY = WAL_LOG` ; suppression de la base sur succès ; table
  `_kartenn_test.templates(name, last_used)` ; nettoyage avant nextest dans `k-check`
  (bases `_kt_test_%`, `_sqlx_test_%`, gabarits périmés depuis 24 h) — ADR 0028
  décisions 1 à 7 — verify : construction concurrente de 8 processus, un seul gabarit ;
  empreinte changée par une migration ajoutée ; base d'un test en échec gardée ;
  `SET ROLE kartenn_app` sur un clone
  où : `crates/kartenn-testing/`, `crates/kartenn-testing-macros/` ; `devenv.nix`
  (`scripts.k-check`) ; modèle : sqlx-postgres 0.9.0 `src/testing/mod.rs`,
  sqlx-macros-core 0.9.0 `src/test_attr.rs` ; `kartenn-store-pg/src/lib.rs` (`MIGRATOR`)
  dépend de : —
  — fait (2026-10-21), sauf le câblage dans `k-check` : `kartenn-testing` (`run`,
  `ensure_template`, `build_template`, `clone`, `clean`, `fingerprint`, `database_name`,
  binaire `kartenn-testing-clean`) et `kartenn-testing-macros` (`pg_test`, argument
  `migrator = "…"`, `kartenn_store_pg::MIGRATOR` par défaut) ; tests
  `crates/kartenn-testing/tests/it/` (cible `testing_it`, ADR 0027) :
  `eight_concurrent_builders_make_one_template`,
  `a_migration_added_changes_the_fingerprint`,
  `a_failed_test_keeps_its_database_a_passed_one_drops_it`,
  `the_application_role_keeps_its_grants_on_a_clone`, plus l'attribut sur `()` et
  `Result`, et `#[ignore]` conservé. `kartenn-testing-clean` essayé sous `k-heavy --pg`
  (bases `_sqlx_test_` restantes supprimées, gabarit récent gardé). Câblé ensuite dans
  `scripts.k-check` (devenv.nix) : binaire construit sous un créneau, lancé sous
  `k-heavy --pg` avant nextest ; un échec du nettoyage est signalé sans arrêter `k-check`.
  choix : le migrateur est passé par l'attribut (aucune dépendance de `kartenn-testing`
  sur `kartenn-store-pg`) ; la construction concurrente est éprouvée par 8 fils, chacun
  avec son runtime et sa connexion, plutôt que 8 processus ; `clean` n'a pas de test
  automatique (il supprimerait les bases des suites qui tournent en même temps) ; le nom
  d'une base de test est `_kt_test_` et 54 caractères hexadécimaux du SHA-256 du chemin.
- [ ] 2. kartenn-store-pg : remplacement de l'attribut (46 tests), mesure de la suite de
  la crate avant et après — ADR 0028 décision 1 — verify : commun, plus la durée de la
  suite avant et après
  où : `crates/kartenn-store-pg/tests/it/`, `Cargo.toml` (dev-dependency)
  dépend de : 1 ; 013 : 4
- [ ] 3. kartenn-service (157 tests) — verify : commun
  où : `crates/kartenn-service/tests/it/`, `Cargo.toml`
  dépend de : 1 ; 013 : 2
- [ ] 4. kartenn-rest (97 tests) — verify : commun
  où : `crates/kartenn-rest/tests/it/`, `tests/it/common/`, `Cargo.toml`
  dépend de : 1 ; 013 : 3
- [ ] 5. kartenn-server, kartenn-auth, kartenn-telemetry (27 tests) — verify : commun
  où : `crates/kartenn-{server,auth,telemetry}/tests/`, `Cargo.toml`
  dépend de : 1 ; 013 : 5, 6, 8
- [ ] 6. kartenn-vlan, kartenn-machines (71 tests) — verify : commun
  où : `crates/kartenn-vlan/tests/it/support/env.rs`, tests des deux modules, `Cargo.toml`
  dépend de : 1 ; 013 : 7
- [ ] 7. Règle et mesure finale : règle des futurs tests (`docs/practices/rust-build.md`
  §2.5 après la règle de l'ADR 0027, étape 3 de `.claude/commands/tache.md`) ; `k-check`
  complet à froid puis après une modification de kartenn-core, selon le protocole de
  l'ADR 0027, charge relevée — ADR 0028 décisions 10 — verify : gain mesuré reporté dans
  l'ADR 0028
  où : `docs/practices/rust-build.md`, `.claude/commands/tache.md`,
  `docs/adr/0028-base-de-test-gabarit.md`
  dépend de : 2 à 6 ; 013 : 9

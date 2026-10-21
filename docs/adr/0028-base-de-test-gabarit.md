---
status: proposed
date: 2026-10-21
---

# 0028 — Bases de test clonées d'un gabarit migré

## Context

Les tests qui touchent PostgreSQL utilisent tous `#[sqlx::test(migrator =
"kartenn_store_pg::MIGRATOR")]` : **398 tests** au commit `aa3d303` (kartenn-service
157, kartenn-rest 97, kartenn-store-pg 46, kartenn-machines 45, kartenn-vlan 26,
kartenn-server 14, kartenn-auth 7, kartenn-telemetry 6). Pour chacun, sqlx crée une base
vide puis y rejoue les 44 migrations du noyau :

- le nom de la base dérive du chemin du test : `_sqlx_test_` suivi d'un SHA-512 tronqué de
  `module_path!()::nom` (sqlx-core 0.9.0, `src/testing/mod.rs`, l. 49-57 ;
  sqlx-macros-core 0.9.0, `src/test_attr.rs`, l. 177) ;
- la base est créée par `create database "<nom>"`, **sans `TEMPLATE`**, donc copiée de
  `template1`. Aucun paramètre ne permet d'en choisir un autre (sqlx-postgres 0.9.0,
  `src/testing/mod.rs`, l. 167-169 ; `TestArgs` ne porte que `migrator` et `fixtures`,
  sqlx-core, l. 65-69) ;
- puis `migrator.run_direct` applique toutes les migrations (sqlx-core, l. 246-263). La
  base est supprimée si le test réussit (l. 233-240) et, avant sa création, si elle existe
  déjà sous ce nom (sqlx-postgres, l. 154-155 et 189-198).

Les tests des modules (kartenn-vlan, kartenn-machines) ajoutent, dans le test lui-même,
les migrations du module et la synchronisation de ses déclarations
(`kartenn_store_pg::modules::migrate`, `sync_modules`, par exemple
`kartenn-vlan/tests/support/env.rs`).

Selon l'ADR 0027, un `k-check` complet après une modification du noyau passe environ
90 s à exécuter les tests, contre environ 40 s de compilation et de lien.

### Mesures

Mesures prises le 21/10 sur PostgreSQL 18.6 (devenv), données sur btrfs, sous le
verrou PostgreSQL exclusif de `k-heavy`. La charge de la machine (`uptime`) était de 5 à 7
pendant la première mesure et de 4 à 5 pendant la seconde. Chaque temps compte aussi le
démarrage d'un processus `psql` ou `sqlx` (environ 20 ms). Scripts `adr0028-measure.sh`
et `adr0028-measure2.sh`, joints à la tâche 1.

| Opération | n | Moyenne | p90 |
|---|---|---|---|
| `CREATE DATABASE` (copie de `template1`) | 10 | 65 ms | 72 ms |
| 44 migrations du noyau (`sqlx migrate run`) | 10 | 490 ms | 567 ms |
| **Création et migration, aujourd'hui** | 10 | **555 ms** | 635 ms |
| Clone d'un gabarit migré, `STRATEGY = WAL_LOG` | 20 | **98 ms** | 127 ms |
| Clone, `STRATEGY = FILE_COPY` (`file_copy_method = copy`) | 20 | 889 ms | 961 ms |
| Clone, `FILE_COPY` avec `file_copy_method = clone` (btrfs) | 10 | 671 ms | 790 ms |
| Migrateur rejoué sur un clone (rien à appliquer) | 5 | 23 ms | 25 ms |
| `DROP DATABASE` | 1 | 61 ms | — |

Autres résultats :
- le gabarit migré pèse 12 Mo ;
- 12 clones lancés en parallèle depuis le même gabarit réussissent tous, en 728 ms au
  total ;
- le propriétaire de la base (`kartenn`, `CREATEDB`, pas super-utilisateur) peut lui
  poser `IS_TEMPLATE true` et `ALLOW_CONNECTIONS false`, puis la cloner.

**Gain estimé** : environ 0,45 s par test, soit environ 180 s de temps cumulé sur les 398
tests. Les tests tournent sur 12 fils (`NEXTEST_TEST_THREADS` sous `k-heavy`) : le gain
réel attendu se situe entre 15 s (parallélisme parfait) et 30 s (PostgreSQL saturé par
les migrations concurrentes), sur les environ 90 s de l'exécution des tests. Ce n'est
qu'une estimation : la tâche 2 la vérifie sur la suite réelle. Le coût propre aux modules
(migrations et synchronisation dans le test) n'est pas mesuré ici ; il relève de la
variante différée (point 5).

### Exigences normatives (PostgreSQL 18)

- « no other sessions can be connected to the source database while it is being copied.
  `CREATE DATABASE` will fail if any other connection exists when it starts »
  ([Managing Databases, 22.3 Template Databases][pg-tpl] ; [CREATE DATABASE, Notes][pg-cdb]).
- `datistemplate` : la base peut être clonée par tout rôle `CREATEDB` ; sinon, seuls les
  super-utilisateurs et son propriétaire le peuvent. `datallowconn = false` refuse les
  nouvelles connexions ([22.3][pg-tpl]).
- `STRATEGY` : `WAL_LOG` copie bloc par bloc ; c'est le défaut, « the most efficient
  strategy in cases where the template database is small ». `FILE_COPY` force un point
  de contrôle avant et après la création ([CREATE DATABASE][pg-cdb]).
- `file_copy_method` (`COPY` ou `CLONE`, contexte `user`) ne concerne que `FILE_COPY`
  ([19.4 Resource Consumption][pg-res]).

Ce qui suit est un choix propre à kartenn.

## Decision

1. **Un attribut de test propre au projet, sur un gabarit.**
   - Deux crates de développement, `publish = false` :
     - `kartenn-testing` : le moteur, exposé par `kartenn_testing::run(test_path, variante,
       corps)` ;
     - `kartenn-testing-macros` : l'attribut `#[kartenn_testing::pg_test]`.
   - L'attribut remplace `#[sqlx::test(migrator = "kartenn_store_pg::MIGRATOR")]` sans
     toucher au corps des tests. La signature reste `async fn x(pool: PgPool)`, avec
     retour `()` ou `Result`.
   - L'attribut produit un test sur le même environnement d'exécution que celui de sqlx
     (tokio, fil courant). Le test path reste `concat!(module_path!(), "::", nom)`.
   - Le moteur clone la base du test depuis le gabarit, ouvre le pool (5 connexions,
     comme sqlx), exécute le corps, puis supprime la base si le test réussit.
   - Le remplacement est mécanique : une ligne par test.
   - Dépendances : `syn`, `quote` et `proc-macro2`, déjà dans `Cargo.lock` par
     transitivité. Elles deviennent des dépendances directes d'une crate de développement.
     C'est un choix de dépendance, soumis à cette validation.
2. **Gabarit nommé par une empreinte.**
   - Nom : `_kt_tpl_<empreinte>`, où l'empreinte est le SHA-256 (16 premiers caractères
     hexadécimaux) de la liste triée des couples (version, somme de contrôle) de
     `kartenn_store_pg::MIGRATOR`.
   - Une migration ajoutée ou modifiée change donc le nom : un gabarit périmé n'est
     jamais réutilisé. Deux worktrees sur des branches différentes ont chacune le leur.
3. **Construction sûre en concurrence.** Plusieurs binaires nextest démarrent en même
   temps. Pour chaque processus :
   1. prendre `pg_advisory_lock(<clé de l'empreinte>)` sur une connexion à la base de
      `DATABASE_URL` ;
   2. si `_kt_tpl_<empreinte>` existe dans `pg_database`, relâcher le verrou ;
   3. sinon :
      1. supprimer une construction abandonnée (`_kt_build_<empreinte>`) ;
      2. créer `_kt_build_<empreinte>` et y appliquer `MIGRATOR` ;
      3. fermer toutes ses connexions ;
      4. renommer en `_kt_tpl_<empreinte>` ;
      5. poser `ALLOW_CONNECTIONS false` puis `IS_TEMPLATE true` ;
      6. relâcher le verrou.

   Le renommage est atomique, et un clone ne vise que le nom final : aucun clone ne voit
   de gabarit à moitié migré. `ALLOW_CONNECTIONS false` empêche qu'une connexion égarée
   fasse échouer les clones (exigence de 22.3). Les clones concurrents n'ont pas besoin
   de verrou (12 en parallèle, mesuré). Le contrôle d'existence est mémorisé par
   processus (`OnceCell`).
4. **`STRATEGY = WAL_LOG`**, écrit explicitement : 98 ms contre 671 à 889 ms pour
   `FILE_COPY`, pour un gabarit de 12 Mo.
5. **Une seule variante en v0.1 : le noyau.**
   - Les tests des modules gardent leurs migrations et leur synchronisation dans le test.
     Le noyau est déjà cloné.
   - Une variante « noyau et modules déployés et synchronisés » est différée. Il faudrait
     d'abord mesurer `Env::new` des tests VLAN et machines, et que la synchronisation
     tolère des déclarations déjà appliquées : `env.rs` affirme aujourd'hui
     `Outcome::Applied`.
   - L'empreinte inclurait alors les sommes de contrôle des migrations des modules
     (`kartenn_store_pg::modules`).
6. **Noms des bases clonées et nettoyage.**
   - Une base clonée s'appelle `_kt_test_` suivi d'un hachage du test path, 63 octets au
     plus comme chez sqlx.
   - Avec l'ADR 0027, le test path commence par le nom du binaire de la crate
     (`service_it::rbac::…`) : il est unique dans le dépôt.
   - Comme sqlx, le moteur supprime d'abord une base du même nom, puis supprime la base
     d'un test réussi. Celle d'un test en échec est gardée pour l'examen.
   - Nettoyage global : `k-check` tient le verrou PostgreSQL exclusif pendant toute la
     phase de tests, donc aucune autre suite ne tourne. Avant nextest, il supprime :
     - les bases `_kt_test_%` et `_sqlx_test_%` restantes ;
     - les gabarits `_kt_tpl_%` autres que celui de l'empreinte courante qu'aucune
       worktree n'a utilisés depuis 24 h. Cette date est tenue dans la table
       `_kartenn_test.templates(name, last_used)` de la base de `DATABASE_URL`, mise à jour
       à chaque construction ou premier clone d'un processus.
7. **Rôles et droits inchangés (INV-11).**
   - `kartenn_app` est un rôle du cluster.
   - Les `GRANT`, propriétaires et extensions (`btree_gist`) sont des objets de la base :
     un clone les copie à l'identique. Le `SET ROLE kartenn_app` des tests ne change pas.
   - Le gabarit appartient au rôle de `DATABASE_URL`, sans super-utilisateur (vérifié).
8. **Données de référence.**
   - Les migrations ne chargent pas les registres IANA : c'est la tâche de déploiement
     (`maintenance::deploy`). Les tests qui l'appellent (`special_registry`,
     `audit_events::the_migration_job_and_module_sync_are_the_system`) le font sur la base
     clonée comme aujourd'hui sur la base migrée. Le migrateur y est rejoué sans effet
     (23 ms).
   - Les lignes datées par `now()` dans les migrations (premier passage des tâches de
     fond en 0039, lignes initiales de 0013, 0014, 0021, 0023, 0037) portent l'heure de
     construction du gabarit et non celle du test. Ces lignes sont « dues » dans les deux
     cas.
   - La suite complète, verte à nombre de tests égal, valide l'absence d'effet. Un test
     qui en dépendrait serait corrigé, pas exclu.
9. **Aucun test ne garde une base migrée pas à pas.**
   - Aucun test ne vérifie une migration intermédiaire.
   - `harness::throwaway_database_is_migrated` vérifie l'état final (extension
     `btree_gist`), copié par le clone.
   - `kartenn-rest/src/health.rs` lit `_sqlx_migrations`, copiée elle aussi.
   - Le seul `#[sqlx::test]` sans migrateur est un commentaire de documentation
     (`kartenn-store-pg/tests/harness.rs`, l. 1).
   - L'attribut sqlx reste disponible pour un futur test qui voudrait une base vide.
10. **Règle pour les futurs tests**, à reporter dans `docs/practices/rust-build.md` §2.5
    (après la règle de l'ADR 0027) et dans l'étape 3 de `.claude/commands/tache.md` :
    - un test qui a besoin de la base utilise `#[kartenn_testing::pg_test]` ;
    - `#[sqlx::test]` est réservé au cas, à justifier en commentaire, d'une base vide ou
      migrée autrement ;
    - aucun test ne crée lui-même de base dans le cluster partagé.

## Alternatives

- **Migrer `template1`.** Toute base créée sans `TEMPLATE`, y compris celles de sqlx,
  naîtrait migrée. Rejeté :
  - le cluster de devenv sert aussi aux bases `kartenn`, `kartenn_bench` et
    `kartenn_e2e` ;
  - un seul `template1` ne peut pas suivre plusieurs branches ;
  - rien n'invalide `template1` quand une migration change.
- **Garder `#[sqlx::test]` et recréer la base dans le test** (signature
  `fn(PgPoolOptions, PgConnectOptions)` : sqlx crée la base vide, le test la supprime et
  la reclone). Rejeté : chaque test crée deux bases, et le contournement dépend de la
  séquence interne de sqlx.
- **Une fonction à appeler dans chaque test** (`#[tokio::test] async fn x() { let pool =
  kartenn_testing::pool!().await; … }`), sans macro procédurale. Rejeté : les 398 corps
  de test changent de signature. L'attribut ne change qu'une ligne et garde la forme
  actuelle.
- **`STRATEGY = FILE_COPY`**, avec ou sans `file_copy_method = clone` : 7 à 9 fois plus
  lent ici, à cause des points de contrôle imposés.
- **Contribuer un paramètre `template` à sqlx.** Possible à terme. Le projet n'en dépend
  pas : le moteur propre au projet suffit, et l'attribut pourra déléguer à sqlx si le
  paramètre apparaît.

## Consequences

- Chaque test PostgreSQL gagne environ 0,45 s de préparation ; gain attendu de 15 à 30 s
  sur les environ 90 s de tests d'un `k-check`, à confirmer par la tâche 2.
- La première exécution après un changement de migration paie une construction du
  gabarit (environ 0,6 s), une fois par empreinte.
- Deux crates de développement s'ajoutent, ainsi que trois dépendances directes déjà
  présentes dans l'arbre.
- Le nombre de tests de nextest ne change pas. Les noms des tests ne changent pas non
  plus ; seuls les noms des bases (`_kt_test_…`) changent.
- Une base de test en échec reste visible sous un préfixe distinct de sqlx.
  `k-check` nettoie avant chaque exécution des tests.

### Découpage proposé (après validation)

Les tâches 2 et suivantes remplacent l'attribut de chaque test d'une crate. Elles
touchent les mêmes fichiers `tests/` que l'ADR 0027 :
- chaque crate passe **après** sa tâche de l'ADR 0027 (fichiers sous `tests/it/`) ;
- deux tâches sur la même crate ne tournent jamais en parallèle ;
- la tâche 1 ne touche aucun `tests/` existant et peut commencer dès la validation.

1. **Moteur et attribut.**
   - Crates `kartenn-testing` et `kartenn-testing-macros`, hakari, `publish = false`.
   - Gabarit : empreinte, verrou, construction, renommage, drapeaux.
   - Clone `WAL_LOG`, suppression sur succès, table `_kartenn_test.templates`.
   - Nettoyage avant nextest dans `k-check` (devenv.nix).
   - Scripts de mesure joints.
   - Où : `crates/kartenn-testing*/` ; `devenv.nix` (`scripts.k-check`) ; modèle :
     sqlx-postgres 0.9.0 `src/testing/mod.rs` ; `kartenn-store-pg/src/lib.rs`
     (`MIGRATOR`).
   - Verify :
     - construction concurrente de 8 processus : un seul gabarit ;
     - empreinte changée par une migration ajoutée ;
     - base d'un test en échec gardée ;
     - `SET ROLE kartenn_app` sur un clone.
2. **kartenn-store-pg** : remplacement de l'attribut, 46 tests, suivi de la mesure de
   référence de la suite de la crate avant et après.
   - Où : `crates/kartenn-store-pg/tests/it/` (après la tâche correspondante de 0027).
   - Verify : même nombre de tests, durée de la suite avant et après.
3. **kartenn-service** (157 tests). Où : `crates/kartenn-service/tests/it/`.
4. **kartenn-rest** (97). Où : `crates/kartenn-rest/tests/it/` et `tests/it/common/`.
5. **kartenn-server, kartenn-auth et kartenn-telemetry** (27).
6. **kartenn-vlan et kartenn-machines** (71). Où : `tests/it/support/env.rs` et les
   tests des modules.
7. **Règle** (`rust-build.md` §2.5, `tache.md`) et **mesure finale** : `k-check` complet
   à froid et après une modification de kartenn-core, selon le protocole de l'ADR 0027,
   avec la charge relevée.

[pg-tpl]: https://www.postgresql.org/docs/18/manage-ag-templatedbs.html
[pg-cdb]: https://www.postgresql.org/docs/18/sql-createdatabase.html
[pg-res]: https://www.postgresql.org/docs/18/runtime-config-resource.html

---
status: accepted
date: 2026-10-21
---

# 0027 — Un binaire de tests d'intégration par crate

## Context

Cargo fait de chaque fichier `tests/*.rs` une crate distincte, compilée et liée en un
exécutable propre ; un sous-répertoire de `tests/` n'est une cible que s'il contient
`main.rs` ([Cargo, « Integration tests »][cargo-it] ; [« Target auto-discovery »][cargo-auto]).
Les helpers partagés (`tests/support/`, `tests/common/`, `tests/wire/`) sont donc
recompilés dans chaque binaire qui les inclut par `mod` ou `#[path]`.

Constat sur la session du 20-21/10 (mesures de l'utilisateur) : les sous-agents ont passé
88 % de leur temps dans des commandes ; `k-check` médian 114 s, p90 297 s ; `target/` de
50 à 190 Go par worktree. Inventaire au commit `76fa6d4` : **110 fichiers**
`crates/*/tests/*.rs`, donc 110 binaires d'intégration, plus les binaires de tests
unitaires des crates.

| Crate | Fichiers | Helpers partagés |
|---|---|---|
| kartenn-service | 40 | `tests/support/` (`kit_module`, `section_modules`, `dummy_module`), par `#[path]` |
| kartenn-rest | 23 | `tests/common/`, `tests/wire/`, et `#[path]` vers `kartenn-service/tests/support/section_modules.rs` |
| kartenn-store-pg | 12 | — |
| kartenn-telemetry | 9 | — |
| kartenn-server | 8 | — |
| kartenn-vlan | 6 | `tests/support/env.rs`, par `#[path]` |
| kartenn-auth | 5 | — |
| kartenn-machines | 3 | — |
| kartenn-core | 2 | — |
| syslog-sender | 2 | — |

Chaque binaire d'intégration lie toutes les dépendances de la crate (sqlx, axum, utoipa,
rustls…) : le coût du lien et la taille sur disque se répètent autant de fois qu'il y a
de fichiers, et une modification de `kartenn-core` les relie tous.

### Mesure de référence (avant)

Worktree neuve au commit `76fa6d4` avec `devenv.local.nix` (linker wild, infos de
débogage `line-tables-only`, `split-debuginfo = unpacked`), `target/` vide, sccache
neutralisé (`SCCACHE_DISABLE=1` et `RUSTC_WRAPPER=` vide, que Cargo traite comme
« pas d'enveloppe ») ; `k-check` complet lancé sous un créneau `k-heavy` déjà pris
(`CARGO_BUILD_JOBS=12`, `NEXTEST_TEST_THREADS=12`), donc sans temps d'attente compté.
Script : `k-heavy --pg bash adr27-measure.sh <worktree> <label>`.

| Passe (21/10, machine partagée) | Durée de `k-check` | dont tests (nextest) | Binaires de test (nextest) | `target/debug` |
|---|---|---|---|---|
| À froid | 341 s | 88 s | 126 | 12 Go |
| Après une modification de `kartenn-core` | 131 s | 90 s | 126 | 15 Go |

« Modification de `kartenn-core` » : une instruction neutre ajoutée au corps d'une
fonction publique (`std::hint::black_box(())` dans `Violation::at`), ce qui recompile
la crate et tous ses dépendants, puis annulée. 731 tests passent dans les deux passes.

Lecture : avec `devenv.local.nix`, `target/debug` reste à 12-15 Go ; les 50 à 190 Go
observés venaient en partie des worktrees des agents, créées sans ce fichier non
versionné (infos de débogage complètes, linker par défaut). Après une modification du
noyau, l'exécution des tests (environ 90 s, une base PostgreSQL par test) pèse plus que
la compilation et le lien (environ 40 s) : le regroupement ne vise que cette seconde
part et l'espace disque.

## Decision

Chaque crate a **un seul binaire de tests d'intégration** : `tests/it/main.rs` déclare
un module par ancien fichier (`mod rbac;` pour l'ancien `tests/rbac.rs`, devenu
`tests/it/rbac.rs`) et les helpers deviennent des modules du même binaire
(`tests/it/support/…`, `tests/it/common/…`), compilés une seule fois.

1. **Cible explicite, nom propre à la crate.** Le `Cargo.toml` de chaque crate déclare
   `autotests = false` et la cible :

   ```toml
   [[test]]
   name = "service_it"          # <crate sans préfixe>_it
   path = "tests/it/main.rs"
   ```

   Le nom n'est pas `it` partout parce qu'il entre dans le nom des bases
   `#[sqlx::test]` (point 4) : deux crates aux modules homonymes (`imports`, `rest`,
   `audit`, `changesets`, `filter`, `references`, `rights`, `transport` existent déjà
   dans plusieurs crates) partageraient une base. Les binaires restent distincts pour
   nextest (identifiant `kartenn-service::service_it`, [nextest, « Binary IDs »][nx-bin]).

2. **Crates à `required-features`** (kartenn-vlan, kartenn-machines : `pg` pour
   les tests du pipeline, `rest` pour les routes). Options :
   - (a) `#[cfg(feature = "…")]` sur chaque module de `tests/it/main.rs`, un seul
     binaire avec `required-features = ["pg"]` ;
   - (b) un binaire par jeu de features (`…_it` et `…_rest_it`) ;
   - (c) un seul binaire avec `required-features = ["rest"]`, `rest` impliquant `pg`.

   **Choix : (c).** Les tests de ces crates ne tournent qu'avec `--all-features`
   (`k-check`, `devenv test`) ; `networks.rs`, `references.rs` et `vlans.rs` n'ont
   d'ailleurs aujourd'hui ni `[[test]]` ni garde de feature. Vérifier qu'une crate
   compile avec un sous-ensemble de features relève de `cargo check`, pas d'un second
   binaire de tests. `required-features` porte sur la cible entière, jamais sur un
   module ([Cargo, « The `required-features` field »][cargo-rf]) ; (a) multiplierait les
   gardes sans rien tester de plus, (b) garderait deux liens.

3. **Tests qui restent à part.** nextest exécute chaque test dans son propre
   processus ([nextest, « How nextest works »][nx-how]) : l'état global d'un test
   (variables d'environnement, crochet de panique, abonné `tracing` global, signaux) ne
   fuit pas vers un autre, même dans un binaire commun. Le risque ne vient que de
   `cargo test`, qui exécute tous les tests d'un binaire dans un seul processus
   (`enterTest` de `devenv.nix`). Recensement au commit `76fa6d4` :

   | Motif cherché | Occurrences |
   |---|---|
   | `std::env::set_var` / `remove_var` | aucune |
   | `#[ctor]` | aucune |
   | `harness = false` | aucune |
   | abonné ou fournisseur global (`set_global_default`, `opentelemetry::global`) | aucun (les tests utilisent `tracing::subscriber::set_default`, local au fil) |
   | sous-processus du binaire de test lui-même | `kartenn-telemetry/tests/panic.rs` (`current_exe()` avec `--exact panicking_child`, crochet de panique installé dans l'enfant seulement) |
   | sous-processus d'un autre exécutable | `kartenn-server/tests/dev_idp.rs` (`CARGO_BIN_EXE_kartenn-server`, `SSL_CERT_FILE` passé à l'enfant), `kartenn-telemetry/tests/architecture.rs` (`cargo metadata`) |
   | signal au processus du test | `kartenn-server/tests/http.rs` (`kill -HUP <pid du test>`, deux tests) |

   **Choix :** un seul test reste dans son propre binaire, **`kartenn-server/tests/http.rs`**
   (cible `[[test]] name = "http"`, avec un commentaire qui en donne la raison) : sous
   `cargo test`, son `SIGHUP` atteindrait tous les tests du binaire commun.
   `panic.rs` rejoint le binaire commun : seul l'argument `--exact` change
   (`panic::panicking_child`). `dev_idp.rs` et `architecture.rs` rejoignent le binaire
   commun sans changement (`CARGO_BIN_EXE_*` est fourni à toute cible de test du paquet,
   [Cargo, « Environment variables »][cargo-env]).

4. **Effets de bord.**
   - **Noms des tests.** nextest nomme un test par son chemin dans le binaire : `rbac::a_test`
     au lieu de `a_test` dans le binaire `rbac`. Un filtre par sous-chaîne
     (`k-check -p kartenn-service a_test`) trouve toujours le test ; un filtre
     `binary(rbac)` devient `binary(service_it) & test(/^rbac::/)` ([nextest,
     « Filtersets »][nx-filter]). Aucun profil nextest n'existe (`.config/` ne contient que
     `hakari.toml`) et aucun filtre `binary(…)` n'est écrit dans le dépôt.
   - **Lignes `où :` et `verify`.** Elles citent 152 fois un `tests/<fichier>.rs` dans
     `specs/` et `docs/`. Les tâches cochées sont de l'histoire et ne changent pas ; les
     tâches ouvertes sont corrigées par la tâche de chaque crate (chemin
     `tests/it/<module>.rs`). Les `verify` nomment des fonctions de test, inchangées.
   - **Bases `#[sqlx::test]`.** Le nom de la base est `_sqlx_test_` suivi du SHA-512,
     tronqué et encodé, de `concat!(module_path!(), "::", nom_du_test)`
     ([sqlx-macros-core 0.9.0, `src/test_attr.rs`, l. 177][sqlx-attr] ;
     [sqlx-core 0.9.0, `src/testing/mod.rs`, `db_name`][sqlx-core]). `module_path!()`
     commence par le nom de la crate de test : `rbac::a_test` devient
     `service_it::rbac::a_test`, donc un autre nom de base. Sans conséquence pour les
     tests : la base est créée à chaque test et supprimée s'il réussit
     ([sqlx-postgres 0.9.0, `src/testing/mod.rs`, `test_context` et `do_cleanup`][sqlx-pg]).
     Seules les bases laissées par des tests échoués sous l'ancien nom restent
     orphelines ; elles sont listées dans `_sqlx_test.databases` et se suppriment par
     `DROP DATABASE`. Le nom par crate (point 1) supprime en outre une collision latente :
     `do_cleanup` supprime, avant de la recréer, une base du même nom, qu'un test homonyme
     d'une autre crate pourrait être en train d'utiliser (erreur 55006 « being accessed by
     other users » vue pendant la session).
   - **Visibilité.** Les helpers deviennent des modules d'un même binaire : `pub(crate)`
     suffit, et un helper inutilisé dans un module déclenche `dead_code` à l'échelle du
     binaire seulement (moins d'`#[allow(dead_code)]` qu'aujourd'hui).
   - **Parallélisme de compilation.** Un binaire par crate se compile en une seule unité
     au lieu de 40 en parallèle : le temps de compilation des tests d'une crate peut
     croître, celui du lien et l'espace disque baissent. La mesure finale le tranche.

5. **Règle pour les futurs tests**, à reporter dans `docs/practices/rust-build.md`
   (nouvelle sous-section de §2.5) et dans `.claude/commands/tache.md` (étape 3) :
   - un test d'intégration est un module de `tests/it/` (déclaré dans
     `tests/it/main.rs`), jamais un nouveau fichier `tests/*.rs` ;
   - un helper partagé vit dans `tests/it/support/` ; un `#[path]` vers les tests d'une
     autre crate est évité (préférer une feature `test-support` de la crate qui fournit
     le helper) ;
   - un binaire à part (`[[test]]` explicite) n'est admis que pour un test qui agit sur
     son propre processus de façon visible des autres tests sous `cargo test` (signal à
     son PID, `set_var`, état global non local au fil), avec un commentaire qui le dit ;
   - les lignes `où :` des tâches citent `tests/it/<module>.rs`, et les `verify` le nom
     de la fonction de test.

## Alternatives

- **Statu quo.** Simple, mais coût de lien et disque proportionnel au nombre de
  fichiers ; il ne fait que croître avec les modules métier.
- **Tests d'intégration déplacés en tests unitaires** (`#[cfg(test)]` dans `src/`). Un
  seul binaire par crate aussi, mais les tests n'éprouvent plus l'API publique de la
  crate, et les helpers de test entrent dans la crate.
- **Un binaire de tests pour tout le workspace** (crate `kartenn-tests`). Un seul lien,
  mais toute modification relie un binaire qui dépend de tout, et la parallélisation
  entre crates disparaît.
- **Partager seulement les helpers dans une crate de support**, en gardant un binaire par
  fichier. Réduit la recompilation des helpers, pas le nombre de liens ni la taille.

## Consequences

- Un binaire d'intégration par crate (onze), plus `kartenn-server::http` : de 126 à
  environ 28 binaires de test (intégration et unitaires). Gain attendu sur le lien et
  l'espace disque ; la durée d'exécution des tests (une base par test) n'en dépend pas.
- `devenv.local.nix` n'existe pas dans les worktrees créées pour les sous-agents : à
  copier à leur création, faute de quoi le gain de §1.1 et §1.2 de
  `docs/practices/rust-build.md` est perdu (hors de cette ADR, à traiter à part).
- Les chemins de test changent sous un préfixe de module ; aucun test n'est supprimé ni
  renommé hors de ce préfixe : `cargo nextest list` donne le même nombre de tests avant
  et après chaque tâche.
- Les tâches d'implémentation touchent tous les `tests/` d'une crate : elles ne tournent
  pas en parallèle d'une tâche qui modifie les tests de la même crate, dont les branches
  sont fusionnées d'abord.

### Découpage proposé (une tâche par crate, après validation)

| # | Tâche | Où |
|---|---|---|
| 1 | Règle des futurs tests | `docs/practices/rust-build.md` §2.5, `.claude/commands/tache.md` étape 3 |
| 2 | kartenn-service | `crates/kartenn-service/Cargo.toml`, `tests/*.rs` (40), `tests/support/` → `tests/it/` ; `#[path]` remplacés par `mod support` |
| 3 | kartenn-rest | `crates/kartenn-rest/Cargo.toml`, `tests/*.rs` (23), `tests/common/`, `tests/wire/` ; `diff_sections.rs` : le `#[path]` vers kartenn-service devient une copie locale ou une feature `test-support` |
| 4 | kartenn-store-pg | `crates/kartenn-store-pg/Cargo.toml`, `tests/*.rs` (12) |
| 5 | kartenn-telemetry | `crates/kartenn-telemetry/Cargo.toml`, `tests/*.rs` (9) ; `panic.rs` : `--exact panic::panicking_child` |
| 6 | kartenn-server | `crates/kartenn-server/Cargo.toml`, `tests/*.rs` (8) sauf `http.rs` (cible `http` gardée, commentaire) |
| 7 | kartenn-vlan, kartenn-machines | `Cargo.toml` (`required-features = ["rest"]` sur `…_it`, les `[[test]]` actuels retirés), `tests/*.rs` (6 + 3), `kartenn-vlan/tests/support/env.rs` |
| 8 | kartenn-auth, kartenn-core, syslog-sender | `Cargo.toml` et `tests/*.rs` (5 + 2 + 2) |
| 9 | Mesure finale et synchronisation | même protocole que la mesure de référence ; tâches ouvertes de `specs/*/tasks.md` (chemins `où :`) ; cette ADR passe à `accepted` avec le gain |

Chaque tâche vérifie : même nombre de tests dans `cargo nextest list --workspace
--all-features` avant et après, `k-check` complet vert.

[cargo-it]: https://doc.rust-lang.org/cargo/reference/cargo-targets.html#integration-tests
[cargo-auto]: https://doc.rust-lang.org/cargo/reference/cargo-targets.html#target-auto-discovery
[cargo-rf]: https://doc.rust-lang.org/cargo/reference/cargo-targets.html#the-required-features-field
[cargo-env]: https://doc.rust-lang.org/cargo/reference/environment-variables.html#environment-variables-cargo-sets-for-crates
[nx-how]: https://nexte.st/docs/design/how-it-works/
[nx-filter]: https://nexte.st/docs/filtersets/
[nx-bin]: https://nexte.st/docs/running/#binary-ids
[sqlx-attr]: https://github.com/launchbadge/sqlx/blob/v0.9.0/sqlx-macros-core/src/test_attr.rs
[sqlx-core]: https://github.com/launchbadge/sqlx/blob/v0.9.0/sqlx-core/src/testing/mod.rs
[sqlx-pg]: https://github.com/launchbadge/sqlx/blob/v0.9.0/sqlx-postgres/src/testing/mod.rs

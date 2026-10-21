---
status: in-progress
updated: 2026-10-21
---

# 013 — Un binaire de tests d'intégration par crate : tâches

Outillage décidé par l'[ADR 0027](../../docs/adr/0027-un-binaire-de-tests-par-crate.md)
(acceptée le 2026-10-21) : chaque crate a un seul binaire d'intégration, `tests/it/main.rs`,
qui déclare un module par ancien fichier `tests/*.rs` ; les helpers sont des modules du même
binaire. Chaque tâche est un commit, laisse le build vert, et ne supprime ni ne renomme
aucun test hors du préfixe de module. « verify » = preuve de la tâche ; « dépend de » =
tâches à finir avant.

Commun à toutes les tâches de crate — verify : nombre de tests de
`cargo nextest list -p <crate> --all-features` identique avant et après (noté dans la
tâche) ; `k-check -p <crate>` vert ; lignes `où :` et filtres des `verify` des tâches **non
cochées** de `specs/*/tasks.md` qui citent un fichier de test de la crate corrigés
(`tests/it/<module>.rs`), tâches cochées inchangées. Ces tâches touchent tous les `tests/`
d'une crate : elles ne tournent pas en parallèle d'une tâche qui modifie les tests de la
même crate.

## P1 — Règle

- [x] 1. Règle des futurs tests : nouvelle sous-section de §2.5 de
  `docs/practices/rust-build.md` (module de `tests/it/`, helpers dans `tests/it/support/`,
  binaire à part seulement pour un test qui agit sur son propre processus, avec un
  commentaire ; `où :` cite `tests/it/<module>.rs`) et étape 3 de
  `.claude/commands/tache.md` — ADR 0027 point 5 — verify : relecture
  où : ADR 0027 « Decision » point 5 ; `docs/practices/rust-build.md` §2.5 ;
  `.claude/commands/tache.md` étape 3
  dépend de : —

## P2 — Crates

- [x] 2. kartenn-service : `autotests = false`, cible `service_it` (`tests/it/main.rs`),
  `tests/*.rs` → `tests/it/`, `tests/support/` → `tests/it/support/`, `#[path]` remplacés
  par `mod support` — ADR 0027 points 1, 4 — verify : nombre de tests identique
  où : `crates/kartenn-service/Cargo.toml`, `crates/kartenn-service/tests/`
  dépend de : 1
  — 177 tests avant et après (mêmes noms hors préfixe de module), 41 binaires → 2 (lib et
  `service_it`) ; `tests/it/support/mod.rs` déclare les trois modules de test, appelés par
  `use crate::support::… as …` ; le `#[path]` de `kartenn-rest/tests/diff_sections.rs`
  suit le déplacement jusqu'à la tâche 3. Aucune ligne `où :` de tâche ouverte ne citait
  un test de la crate.
- [x] 3. kartenn-rest : cible `rest_it`, `tests/*.rs`, `tests/common/`, `tests/wire/` →
  `tests/it/` ; `diff_sections.rs` : le `#[path]` vers les helpers de kartenn-service
  devient une copie locale ou une feature `test-support` — verify : nombre de tests
  identique
  où : `crates/kartenn-rest/Cargo.toml`, `crates/kartenn-rest/tests/`
  dépend de : 2
  — 198 tests avant et après (mêmes noms hors préfixe), 24 binaires → 2 ; `mod common;`
  et `mod wire;` des fichiers deviennent `use crate::common;` et `use crate::wire;`,
  modules déclarés une fois dans `tests/it/main.rs`.
  choix : copie locale `tests/it/support/section_modules.rs` (avec une ligne qui en
  nomme l'original) plutôt qu'une feature `test-support` de kartenn-service : un seul
  utilisateur, aucune dépendance de la crate vers ses helpers de test.
- [x] 4. kartenn-store-pg : cible `store_pg_it`, `tests/*.rs` → `tests/it/` — verify :
  nombre de tests identique
  où : `crates/kartenn-store-pg/Cargo.toml`, `crates/kartenn-store-pg/tests/`
  dépend de : 1
  — 50 tests avant et après (mêmes noms hors préfixe), 13 binaires → 2 ; aucun helper
  partagé, aucune ligne `où :` de tâche ouverte à corriger.
- [x] 5. kartenn-telemetry : cible `telemetry_it`, `tests/*.rs` → `tests/it/` ; `panic.rs`
  relance son enfant par `--exact panic::panicking_child` — verify : nombre de tests
  identique ; le test de panique trouve toujours son enfant
  où : `crates/kartenn-telemetry/Cargo.toml`, `crates/kartenn-telemetry/tests/`
  dépend de : 1
  — 20 tests avant et après (mêmes noms hors préfixe), 10 binaires → 2 ; `panic.rs` relance
  `--exact panic::panicking_child` et attend `tests/it/panic.rs` dans le lieu de la
  panique ; `architecture.rs` inchangé.
- [x] 6. kartenn-server : cible `server_it` pour tous les fichiers sauf `http.rs`, gardé en
  cible `http` avec un commentaire (il envoie SIGHUP à son propre processus) — ADR 0027
  point 3 — verify : nombre de tests identique
  où : `crates/kartenn-server/Cargo.toml`, `crates/kartenn-server/tests/`
  dépend de : 1
  — 32 tests avant et après (mêmes noms hors préfixe), 9 binaires → 3 (lib, `server_it`,
  `http`) ; `dev_idp` lance toujours le binaire par `CARGO_BIN_EXE_kartenn-server` ; le
  commentaire de `secrets.rs` suit le déplacement de `panic.rs`. Aucune ligne `où :` de
  tâche ouverte ne citait un test de la crate.
- [x] 7. kartenn-vlan, kartenn-machines : cibles `vlan_it` et `machines_it` avec
  `required-features = ["rest"]`, `[[test]]` actuels retirés, `kartenn-vlan/tests/support/`
  → `tests/it/support/` — ADR 0027 point 2 — verify : nombre de tests identique
  où : `crates/kartenn-vlan/`, `crates/kartenn-machines/` (`Cargo.toml`, `tests/`)
  dépend de : 1
  — kartenn-vlan : 35 tests avant et après (mêmes noms hors préfixe), 7 binaires → 2 ;
  kartenn-machines : 54 tests avant et après, 4 binaires → 2. Le `#[path = "support/env.rs"]
  mod env;` de chaque fichier devient `use crate::support::env;`, module déclaré une fois.
  choix : dans un seul binaire, les blocs `impl Env` des fichiers se rejoignent ; l'aide
  `group` de `rest.rs` (autre signature que celle de `vlans.rs`) devient `seed_group`, nom
  d'aide et non de test. Aucune ligne `où :` de tâche ouverte ne citait ces tests.
- [x] 8. kartenn-auth, kartenn-core, syslog-sender : cibles `auth_it`, `core_it`,
  `syslog_sender_it` — verify : nombre de tests identique
  où : `Cargo.toml` et `tests/` de chaque crate
  dépend de : 1
  — kartenn-auth : 30 tests avant et après (mêmes noms hors préfixe), 6 binaires → 2 ;
  kartenn-core : 107, 3 → 2 ; syslog-sender : 20, 3 → 2. `roundtrip.proptest-regressions`
  suit `roundtrip.rs` (proptest lit ses régressions à côté du fichier source) ;
  `tests/it/main.rs` de syslog-sender porte l'en-tête MPL-2.0. Aucune ligne `où :` de
  tâche ouverte ne citait ces tests.

## P3 — Mesure

- [x] 9. Mesure finale et synchronisation : même protocole que la mesure de référence de
  l'ADR (worktree neuve avec `devenv.local.nix`, sccache neutralisé, `k-check` complet à
  froid puis après une modification de kartenn-core : temps, nombre de binaires, taille
  de `target/debug`) ; gain reporté dans l'ADR ; restes des lignes `où :` des tâches
  ouvertes — verify : nombre total de tests de `cargo nextest list --workspace
  --all-features` égal à celui d'avant la tâche 2
  où : ADR 0027 « Mesure de référence (avant) » ; `specs/*/tasks.md`
  dépend de : 2 à 8
  — 732 tests avant et après (731 à la référence, plus `an_unrelated_changeset_raises_no_warning`
  ajouté par le correctif de 009-7) ; ADR 0027 § « Mesure après » : 126 → 27 binaires,
  `target/debug` 12 → 6,1 Go à froid et 15 → 8,1 Go après une modification du noyau,
  compilation et lien d'environ 41 s à 25-36 s ; durée totale non concluante (charge de
  la machine partagée, tests de 61 à 122 s pour le même code). Aucune ligne `où :` de
  tâche ouverte ne citait un fichier de test déplacé.
  choix : un `CARGO_TARGET_DIR` vide dans la worktree de la tâche tient lieu de worktree
  neuve (mêmes sources, même `devenv.local.nix`) ; deux passes après la modification du
  noyau pour montrer la variance.

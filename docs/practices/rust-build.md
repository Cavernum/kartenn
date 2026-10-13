# Compilation Rust : vitesse et profils

> Statut : **proposé**. Ces recommandations seront reprises (ou amendées) par la spec de
> création du workspace Cargo. Elles s'appuient sur D6 (modules compilés), D13
> (Argon2id), D14 (budgets de performance) et D16 (Rust, sqlx, utoipa).

Objectif : garder une boucle édition → compilation → test courte sur un workspace
multi-crates qui grossira module par module, sans sacrifier les performances des
binaires livrés.

Les réglages se répartissent en trois niveaux :

| Niveau | Où | Partagé ? |
|---|---|---|
| Poste local | `devenv.local.nix` (non versionné), configuration de l'éditeur | non |
| Projet | `Cargo.toml` du workspace, `.config/hakari.toml`, `.sqlx/` | oui |
| CI / livraison | pipeline, images OCI | oui |

## 1. Poste local

### 1.1 Linker wild

`devenv.local.nix` active `languages.rust.wild.enable = true`. devenv utilise Clang
comme pilote d'édition de liens (`CARGO_TARGET_<triple>_LINKER`) et ajoute à `RUSTFLAGS` :

```
-C linker-features=-lld -C link-arg=-B<wild>/bin
```

- `-C linker-features=-lld` : sur `x86_64-unknown-linux-gnu`, rustc utilise désormais
  LLD par défaut ; ce drapeau (seul opt-out stable) le désactive pour laisser la place à
  wild. Source : [rustc codegen options, `linker-features`](https://doc.rust-lang.org/rustc/codegen-options/index.html#linker-features).
- `-B<dir>` fait trouver au pilote un `ld` qui est wild (méthode documentée par wild
  pour les compilateurs sans `-fuse-ld=wild`). Source :
  [wild, USAGE.md](https://github.com/davidlattimore/wild/blob/main/USAGE.md).
- wild : Linux uniquement (x86-64, AArch64, RISC-V…), pas encore d'édition de liens
  incrémentale. Source : [wild, README](https://github.com/davidlattimore/wild).
- Vérification : `readelf -p .comment target/debug/<bin>` doit contenir
  `Linker: Wild`.

**Piège** : une variable `RUSTFLAGS` présente dans l'environnement fait ignorer à cargo
les `[build] rustflags` de `.cargo/config.toml`. Les drapeaux supplémentaires passent donc
par `languages.rust.rustflags` (devenv), pas par `.cargo/config.toml`. Source :
[Cargo configuration, `build.rustflags`](https://doc.rust-lang.org/cargo/reference/config.html#buildrustflags)
(sources mutuellement exclusives, la première trouvée l'emporte).

wild reste un choix **local** : la CI et les images OCI utilisent le linker par défaut.

### 1.2 Informations de débogage

Dans `devenv.local.nix` :

| Variable | Effet |
|---|---|
| `CARGO_PROFILE_DEV_SPLIT_DEBUGINFO=unpacked` (et `TEST`) | DWARF laissé dans des `*.dwo` par unité de compilation au lieu d'être recopié dans le binaire (le défaut ELF est `off`) : moins de travail pour le linker. |
| `CARGO_PROFILE_DEV_DEBUG=line-tables-only` | Seulement les tables de lignes : backtraces lisibles, pas de variables ni de paramètres dans le débogueur. À commenter pour une session de débogage. |

Sources : [Cargo profiles, `debug` et `split-debuginfo`](https://doc.rust-lang.org/cargo/reference/profiles.html) ;
[rustc, `split-debuginfo`](https://doc.rust-lang.org/rustc/codegen-options/index.html#split-debuginfo).

### 1.3 rust-analyzer : même environnement ou dossier séparé

Si rust-analyzer ne tourne pas dans l'environnement devenv, il compile avec d'autres
`RUSTFLAGS` que cargo : les deux invalident mutuellement le cache de `target/` et tout
recompile à chaque alternance. Deux parades, cumulables :

1. lancer l'éditeur depuis l'environnement devenv (hook zsh ou `devenv shell`) ;
2. `rust-analyzer.cargo.targetDir = true` : rust-analyzer compile dans un
   sous-dossier de `target/`, ce qui évite aussi la contention sur le verrou, au prix
   d'artefacts dupliqués. Source :
   [rust-analyzer, configuration](https://rust-analyzer.github.io/book/configuration.html).

### 1.4 Mesurer

`cargo build --timings` produit un rapport HTML (crates sur le chemin critique,
parallélisme effectif). On mesure avant d'ajouter une optimisation.

## 2. Projet (`Cargo.toml` du workspace)

### 2.1 Dépendances optimisées en dev

```toml
[profile.dev.package."*"]
opt-level = 2
```

`"*"` vise toutes les dépendances **mais aucun membre du workspace** : notre code reste
en `opt-level = 0` (compilation incrémentale rapide), les dépendances sont optimisées
une fois puis mises en cache. Source :
[Cargo profiles, overrides](https://doc.rust-lang.org/cargo/reference/profiles.html#overrides).

Raison propre à kartenn : Argon2id (D13) est volontairement coûteux en calcul ; non
optimisé, il ralentit fortement les tests d'authentification. À confirmer par mesure
lors de la spec 005.

### 2.2 Macros de sqlx optimisées

```toml
[profile.dev.package.sqlx-macros]
opt-level = 3
```

Recommandation explicite de sqlx : les requêtes vérifiées à la compilation font
« beaucoup de travail à la compilation », et `cargo check` / `cargo build` sont
nettement plus rapides avec des macros optimisées. Source :
[sqlx, README](https://github.com/launchbadge/sqlx/blob/main/README.md).

Par défaut, Cargo n'optimise ni les scripts de build ni les macros procédurales
(`[profile.dev.build-override] opt-level = 0`). Une surcharge globale
`build-override` n'est à envisager que si `--timings` montre d'autres macros lentes
(utoipa, serde) ; elle allonge la première compilation.

### 2.3 sqlx hors ligne

- `cargo sqlx prepare --workspace -- --all-targets --all-features` écrit les
  métadonnées des requêtes dans `.sqlx/` à la racine, **versionné**.
- `SQLX_OFFLINE=true` force l'usage de `.sqlx/` même si `DATABASE_URL` est défini
  (sinon `DATABASE_URL` l'emporte).
- CI : `cargo sqlx prepare --workspace --check` échoue si `.sqlx/` est périmé.

Effet : compiler n'exige ni PostgreSQL ni réseau. Source :
[sqlx-cli, README](https://github.com/launchbadge/sqlx/blob/main/sqlx-cli/README.md).

### 2.4 Unification des features : cargo-hakari

Avec des modules sélectionnés par features (D6, D16), compiler une crate seule
(`cargo build -p kartenn-vlan`) puis le workspace peut activer des jeux de features
différents sur une même dépendance, donc la recompiler. `cargo-hakari` maintient une
crate *workspace-hack* qui fige l'union des features.

```sh
cargo hakari init crates/kartenn-workspace-hack
cargo hakari generate
cargo hakari manage-deps
```

- `Cargo.lock` **doit** être versionné.
- CI : `cargo hakari generate --diff` et `cargo hakari manage-deps --dry-run`.
- Diagnostic : `cargo hakari verify`.

Source : [cargo-hakari](https://docs.rs/cargo-hakari/latest/cargo_hakari/).
Recommandation : l'adopter **dès la création du workspace**, plus coûteux à introduire tard.

### 2.5 Découpage en crates

rustc parallélise surtout entre crates. Règles :

- crates de taille moyenne, graphe de dépendances large plutôt que profond ;
- le noyau du domaine (`kartenn-core`) ne dépend pas des crates lourdes en macros
  (utoipa, axum, sqlx) : elles vivent dans les adaptateurs (REST, PostgreSQL),
  conformément à l'architecture hexagonale ;
- un changement dans un module métier ne doit recompiler que ce module et ses dépendants.

### 2.6 Profils de livraison et de mesure

```toml
[profile.release]
lto = "thin"
codegen-units = 1

[profile.profiling]
inherits = "release"
debug = "line-tables-only"
```

- `lto = "thin"` : gains proches du LTO complet pour un temps bien moindre.
- `profile.profiling` : binaires optimisés avec symboles pour les bancs d'essai de D14
  et le profilage (`cargo build --profile profiling`, sortie dans `target/profiling`).

Source : [Cargo profiles](https://doc.rust-lang.org/cargo/reference/profiles.html).

## 3. Plus tard

| Sujet | Quand | Remarque |
|---|---|---|
| `cargo-nextest` | premiers tests d'intégration PostgreSQL | un processus par test : isolation, délais par test, rapports JUnit ; les doctests restent sur `cargo test --doc`. [nexte.st](https://nexte.st/docs/design/why-process-per-test/) |
| Cache CI (rust-cache, sccache) | mise en place de la CI | peu utile en local avec une seule copie du dépôt et l'incrémental |
| Backend Cranelift, `-Zthreads` | si la boucle de dev devient lente | exige une chaîne nightly (rust-overlay) ; à réévaluer |
| `chattr +C` sur `target/` (btrfs) | si les E/S dominent `--timings` | gain marginal attendu |

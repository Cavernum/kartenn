---
status: draft
updated: 2026-10-21
---

# 015 — Paquetage et déploiement : tâches

Chaque tâche est un commit, laisse le build vert et nomme ce qu'elle implémente.
« verify » = test qui prouve la tâche ; « dépend de » = tâches à finir avant. Les
constructions Nix lourdes (`nix build`, `nix flake check`) passent par `k-heavy` ; le
test NixOS (VM) par `k-heavy --pg` sur la machine partagée.

Parallélisme : P1 (1 à 5) en parallèle ; P2 (6, 7) en parallèle ; 8 et 9 en parallèle,
puis 10. Les tâches 3 et 4 touchent `kartenn-server/src/main.rs` : à fusionner l'une
après l'autre.

## P1 — Briques

- [x] 1. Flake et binaire : `flake.nix`, `flake.lock` (nixpkgs de devenv),
  `nix/package/server.nix`, version lue dans `Cargo.toml`, `checks.lock-sync` —
  REQ-015-1, 2 — verify : `nix build .#kartenn-server` deux fois, même chemin ;
  `result/bin/kartenn-server --version` ; `nix build .#checks.x86_64-linux.lock-sync`
  où : design « Flake » ; `devenv.yaml`, `devenv.lock` ; `Cargo.toml`
  (`[workspace.package]`) ; `.sqlx/`
  dépend de : —
  — `flake.nix` (seule entrée `nixpkgs`, `x86_64-linux` et `aarch64-linux`), `flake.lock`
  recopié des nœuds `nixpkgs` et `nixpkgs-src` de `devenv.lock`, `nix/package/server.nix`
  (`buildRustPackage`, source filtrée à `Cargo.*`, `crates/`, `.sqlx/`, `SQLX_OFFLINE`,
  `doCheck = false`), `nix/checks/lock-sync.nix` (compare les `rev` des deux verrous).
  Vérifié : deux `nix build .#kartenn-server` donnent le même chemin ; le binaire démarre
  (usage des sous-commandes) ; `checks.x86_64-linux.lock-sync` construit. Sortie : binaire
  38 Mo, clôture 84 Mo (glibc, libgcc). `--version` arrive avec la tâche 3.
  choix : version lue dans `crates/kartenn-server/Cargo.toml`, ou dans
  `[workspace.package]` dès que le paquet en hérite ; pas de `cargoHash` (verrou lu
  directement) ; le test de dérive des verrous échoue au build, pas à l'évaluation, pour
  ne pas masquer les autres `checks`.
- [x] 2. Interface : `nix/package/web.nix` (shell, plugins `vlan` et `machines`),
  `scripts/update-pnpm-hash.sh`, paquet joint `kartenn` — REQ-015-1 — verify : `nix
  build .#kartenn` contient `share/kartenn/web/shell/index.html` avec son import map et
  `share/kartenn/plugins/{vlan,machines}/<version>/manifest.json`, sans `example`
  où : design « Flake » ; `web/e2e/serve.sh` (commandes de build des plugins) ;
  `web/plugins/*/package.json` ; `crates/kartenn-rest/src/web/plugins.rs` (structure
  attendue du répertoire)
  dépend de : —
  — `nix/package/web.nix` (`fetchPnpmDeps` avec `fetcherVersion = 4` et `pnpm_12`, la
  version de `packageManager` ; `pnpmConfigHook` ; build filtré au shell et aux plugins
  `vlan` et `machines` ; copie de `web/dist/shell` et `web/dist/plugins/{vlan,machines}`),
  `nix/package/default.nix` (`symlinkJoin` nommé `kartenn`, paquet par défaut du flake),
  `scripts/update-pnpm-hash.sh`. Vérifié : `nix build .#kartenn` donne `bin/kartenn-server`,
  `share/kartenn/web/shell/index.html` avec son import map (`/sdk/index.js`,
  `/sdk/host.js`) et `share/kartenn/plugins/{vlan,machines}/1.0.0/manifest.json`, sans
  `example` ; shell 596 Ko, plugins 60 Ko, clôture du paquet 84,6 Mo.
  choix : `fetchPnpmDeps` et `pnpmConfigHook` (noms actuels de nixpkgs) au lieu de
  `pnpm.fetchDeps` et `pnpm.configHook` cités par le design ; le manifeste de build
  `.vite/` (banc de taille) est retiré du paquet ; source = `web/` hors `node_modules`,
  `dist` et état des parcours.
- [x] 3. URL de la base par fichier et `--version` : section `[database] url_file`,
  argument facultatif, `--config` pour `migrate` et `breakglass create` — REQ-015-4 —
  verify : tests de `config.rs` (champ inconnu, fichier vide, illisible, ambiguïté
  argument et fichier) ; test dans `kartenn-server/tests/it/` : `run` démarre sur
  `url_file`, l'URL absente des logs capturés
  où : design « Configuration de la base » ; `crates/kartenn-server/src/config.rs`
  (`Config`, tests en fin de fichier), `src/main.rs` (dispatch des sous-commandes)
  dépend de : —
  — `config.rs` : section `[database]` (`Database { url_file }`, champs inconnus refusés),
  `Config::database_url` (argument ou fichier, exactement un ; fin de ligne retirée ;
  fichier vide, illisible ou double source refusés, le message nomme le fichier, jamais
  l'URL) ; `main.rs` : `--version`/`-V`, `<database-url>` facultatif pour `run`,
  `audit purge`, `migrate` et `breakglass create`, `--config <fichier>` pour les deux
  derniers. Tests : `config::tests::the_database_comes_from_the_argument_or_its_file_once`
  (champ inconnu, argument seul, fichier seul, aucune, deux, vide, illisible, le secret
  jamais dans l'erreur) ; `server_it database_file::{the_server_tells_its_version,
  an_instance_starts_on_the_url_of_its_file}` (`migrate --config`, double source refusée
  sans l'URL, `run` sans URL en argument démarre, l'URL absente du fichier de logs et de
  la sortie d'erreur).
  choix : pour `run` et `audit purge`, le premier argument est l'URL s'il commence par
  `postgres://` ou `postgresql://`, sinon le fichier de configuration (pas de nouvelle
  option sur ces deux sous-commandes) ; `audit verify` garde son URL en argument (hors de
  REQ-015-4) ; la variable `KARTENN_DATABASE__URL_FILE` marche aussi (figment).
- [x] 4. Premier administrateur : réalisé par la tâche **005-19** (même travail, une seule
  implémentation ; cette ligne se coche avec elle). `breakglass create` lie `administrator`
  global jusqu'à `valid_until`, mention dans l'audit ; le mode d'écriture de la liaison
  (adaptateur direct ou changeset sous identité système) est celui du design de la 005
  (K17) — REQ-015-10 — verify : test `kartenn-server/tests/it/` : base neuve,
  `migrate`, `breakglass create`, connexion du compte, `POST /v1/bindings` accepté ;
  après `valid_until`, refusé ; `web/e2e/serve.sh` cesse de poser le rôle en SQL
  où : design « Premier administrateur » ; `crates/kartenn-server/src/main.rs`
  (`create_breakglass`) ; `kartenn-store-pg/src/breakglass.rs` ; spec 005
  (`requirements.md` REQ-005-14, `decisions.md` K9)
  dépend de : —
- [x] 5. Versionnement : version `0.1.0` du workspace et des paquets web,
  `CHANGELOG.md`, test de cohérence des versions — REQ-015-12 — verify :
  `version.rs` compare Cargo, `web/*/package.json` et la version du flake
  où : design « Versionnement » ; `Cargo.toml`, `crates/*/Cargo.toml`, `web/*/package.json`
  dépend de : —
  — `[workspace.package] version = "0.1.0"` ; les crates `kartenn-*` en
  `version.workspace = true`, leurs dépendances de chemin entre elles en `"0.1.0"` ;
  `Cargo.lock` ; contrat de référence régénéré (`info.version`) ; `web/sdk`, `web/shell`,
  `web/e2e` déjà en `0.1.0` ; `CHANGELOG.md` (entrée `[0.1.0] — à publier`) ; test
  `kartenn-server/tests/it/version.rs` (crates, paquets web, journal des changements).
  Version du flake (2026-10-21) : `nix/checks/version.nix` (`checks.version`) exige
  la même version de `kartenn`, `kartenn-server` (lue dans Cargo) et `kartenn-web` (lue
  dans `web/shell/package.json`) ; `version.rs`
  (`the_flake_takes_its_versions_from_cargo_and_the_web_packages`) vérifie que les
  paquets Nix n'écrivent aucune version littérale et que le flake déclare ce check.
  choix : `syslog-sender` (bibliothèque à part, MPL-2.0, 012/L24) et
  `kartenn-workspace-hack` (géré par hakari) gardent leur propre version ; les plugins de
  `web/plugins/*` gardent la leur (version publiée dans leur manifeste, servie sous
  `/plugins/<nom>/<version>/`).

## P2 — Livrables

- [ ] 6. Image OCI : `nix/oci.nix`, `checks.oci` — REQ-015-3 — verify : `nix build
  .#oci` ; `checks.oci` (utilisateur, point d'entrée, absence de `bin/sh`) ; taille
  relevée dans la note de la tâche
  où : design « Image OCI »
  dépend de : 1, 2, 3
- [ ] 7. Module NixOS et test VM : `nix/module.nix`, `nixosModules.default`,
  `checks.vm` — REQ-015-5, 6, 7, 8, 9, 11 — verify : `checks.vm` (sondes de deux
  instances, migration en échec bloquante, secrets absents du magasin, de `systemctl
  show` et de l'environnement, note `systemd-analyze security` ≤ 2,0, refus d'`UPDATE`
  d'historique au rôle d'instance, plugin ajouté publié, base en retard → 503 puis 200
  après migration, migration interrompue puis rejouée)
  où : design « Module NixOS », « Mise à jour » ; `crates/kartenn-rest/src/health.rs` ;
  `kartenn-store-pg/migrations/0001_*.sql` (rôle `kartenn_app`)
  dépend de : 1, 2, 3, 4

## P3 — Exploitation

- [ ] 8. Documentation de l'exploitant : `docs/exploitation/` et `compose.yaml`
  d'essai — REQ-015-13 — verify : procédures d'amorçage, de mise à jour et de
  sauvegarde-restauration jouées (VM de la tâche 7 ou essai consigné dans la note) ;
  `compose.yaml` démarré une fois avec l'image de la tâche 6
  où : design « Documentation » ; ADR 0019 (audit exporté) ; spec 007 W4
  dépend de : 6, 7
- [ ] 9. CI proposée : `scripts/ci.sh` — E14 — verify : le script passe sur `main`
  où : design « CI » ; `devenv.nix` (`enterTest`)
  dépend de : 6, 7
- [ ] 10. Synchronisation : `docs/spec/architecture.md` § Déploiement (une image,
  module), `interfaces.md` (CLI, `[database]`), D35 amendée dans
  `specs/001-cadrage/decisions.md`, ADR 0029 acceptée, statuts de la 015 — verify :
  audit `spec-keeper` de la 015 sans écart bloquant
  où : `docs/spec/` ; `docs/adr/0029-paquetage-nix-oci.md`
  dépend de : 1 à 9

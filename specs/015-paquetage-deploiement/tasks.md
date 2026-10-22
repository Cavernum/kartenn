---
status: implemented
updated: 2026-10-22
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
  correctif (tâche 6, 2026-10-21) : le paquet `kartenn` était un `symlinkJoin` dont
  chaque fichier du shell et des plugins était un lien vers `kartenn-web` ; le registre
  des plugins refuse les liens et le shell ne sert que sous sa racine canonique. Le
  paquet copie désormais `bin/` et `share/` (fichiers réels, `passthru` vers les deux
  paquets) ; `checks.oci` refuse tout lien dans `share/kartenn/{web,plugins}`.
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
  (K17) — REQ-015-10 — verify : `kartenn-auth/tests/it/breakglass.rs` :
  `a_new_instance_is_bootstrapped_by_a_break_glass_account` (rôles livrés, compte créé,
  audit avec rôle et portée, connexion de secours, liaison d'un groupe OIDC appliquée) et
  `the_bootstrap_binding_ends_with_the_account` (la liaison finit avec le compte ; après
  `valid_until`, la même liaison est refusée) ; de bout en bout dans `nix/checks/vm.nix`
  (base neuve, migration, `kartenn-breakglass`, connexion TOTP, `/v1/me` administrateur) ;
  `web/e2e/serve.sh` cesse de poser le rôle en SQL
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

- [x] 6. Image OCI : `nix/oci.nix`, `checks.oci` — REQ-015-3 — verify : `nix build
  .#oci` ; `checks.oci` (utilisateur, point d'entrée, absence de `bin/sh`) ; taille
  relevée dans la note de la tâche
  où : design « Image OCI »
  dépend de : 1, 2, 3
  — `nix/oci.nix` (`dockerTools.buildLayeredImage`, `packages.oci`) : clôture du paquet
  seule, sans `/bin` ; `User = "65532:65532"`, `Entrypoint` = `kartenn-server` du paquet,
  `Cmd = ["run"]`, `SSL_CERT_FILE` vers `cacert`, port 8443, étiquettes OCI (titre,
  version, licence) ; `/etc/kartenn/` vide, `/tmp` en 1777, `/opt/kartenn/shell` et
  `/opt/kartenn/plugins` liés au shell et aux plugins livrés. `nix/checks/oci.nix`
  (`checks.oci`) : lit `manifest.json` et la configuration de l'image (utilisateur,
  point d'entrée, commande, certificats) et toutes les couches (aucun `bin/sh`, `bash`,
  `dash`, `ash`, `zsh` ni `busybox` ; certificats, shell et plugins présents). Image
  compressée : **32 Mo** (31 936 Kio), 1 387 entrées.
  choix : aucune variable `KARTENN_*` par défaut dans l'image (elles écraseraient le
  fichier monté) : `[web] root` et `plugins_dir` se posent dans la configuration de
  l'exploitant, vers les chemins fixes `/opt/kartenn/…` ; image non lancée par le check
  (configuration et contenu seulement, comme le prévoit le design).
- [x] 7. Module NixOS et test VM : `nix/module.nix`, `nixosModules.default`,
  `checks.vm` — REQ-015-5, 6, 7, 8, 9, 11 — verify : `checks.vm` (sondes de deux
  instances, migration en échec bloquante, secrets absents du magasin, de `systemctl
  show` et de l'environnement, note `systemd-analyze security` ≤ 2,0, refus d'`UPDATE`
  d'historique au rôle d'instance, plugin ajouté publié, base en retard → 503 puis 200
  après migration, migration interrompue puis rejouée)
  où : design « Module NixOS », « Mise à jour » ; `crates/kartenn-rest/src/health.rs` ;
  `kartenn-store-pg/migrations/0001_*.sql` (rôle `kartenn_app`)
  dépend de : 1, 2, 3, 4
  — fait (2026-10-21) : `nix/module.nix` (`nixosModules.default`,
  `services.kartenn` : `settings`, `instances`, `databaseUrlFile`,
  `migrationDatabaseUrlFile`, `credentials` en `LoadCredential=` désignés par `%d/<nom>`,
  `plugins` joints aux livrés en fichiers réels, `openFirewall`,
  `database.createLocally`) ; unités `kartenn-migrate` (oneshot, temporisateur mensuel),
  `kartenn@<nom>` (`Requires`/`After` la migration, `ExecReload` = SIGHUP),
  `kartenn-audit-purge` (temporisateur quotidien), `kartenn-db-setup` (rôle `kartenn_app`
  avant les migrations, rôles propriétaire et d'instance membres) ; durcissement commun ;
  commande `kartenn-breakglass <nom> <heures>` (005-19, sous le propriétaire, trousseau
  par crédential). `nix/checks/vm.nix` (`checks.vm`, 29 s de script) : migration oneshot
  avant deux instances, `/health/ready` 200 sur les deux, `kartenn-breakglass` puis
  connexion TOTP et `/v1/me` administrateur, plugins `vlan`, `machines` et `notes`
  (exploitant) publiés, aucune URL ni clé dans la ligne de commande, l'environnement,
  `systemctl show` ni les fichiers de configuration, `UPDATE` d'historique refusé au rôle
  d'instance par manque de droit (lecture permise), note `systemd-analyze security
  kartenn@main` = **1,1**, migration en échec (rôle propriétaire sans `LOGIN`) qui
  empêche le redémarrage d'une instance, puis reprise ; base en retard d'une migration
  (dernière ligne de `_sqlx_migrations` retirée) → 503 ; migration bloquée sur un verrou
  `SHARE` de `_sqlx_migrations` (SQL exécuté, version pas encore inscrite), tuée par
  SIGKILL, rien d'inscrit, puis rejouée → succès et 200 sur les deux instances.
  Défaut trouvé : sqlx crée `_sqlx_migrations` pour le seul propriétaire ; le rôle
  d'instance ne pouvait pas la lire et `/health/ready` restait à 503 en production
  (question ouverte d'E7). Corrigé par la migration `0054_migrations_readable.sql`
  (`SELECT` seul à `kartenn_app`, test `store_pg_it harness::the_application_reads_the_applied_migrations_and_nothing_more`) ;
  le contournement `ExecStartPost` du module est retiré.
  choix : le cas « base en retard » rejoue la dernière migration, qui doit pouvoir
  repasser sur une base migrée (0054 : un `GRANT`) ; une migration future qui ne le peut
  pas devra être accompagnée ici de ce qui l'annule.
  choix : rôle propriétaire `kartenn` et rôle d'instance `kartenn_server`, utilisateurs
  dynamiques de même nom (authentification `peer`) ; URL locale
  `postgresql:///kartenn?host=/run/postgresql&user=…` (sqlx refuse un hôte vide avant
  `@`) ; `PrivateUsers`, `ProtectProc=invisible`, `SystemCallFilter=@system-service
  ~@privileged ~@resources` en plus de E6 ; `/run/postgresql` en `ReadWritePaths`
  (connect(2) sur le socket) ; PostgreSQL 18 posé en `mkOverride 900` (au-dessus du défaut
  du module nixpkgs, sous le choix de l'exploitant).

## P3 — Exploitation

- [x] 8. Documentation de l'exploitant : `docs/exploitation/` et `compose.yaml`
  d'essai — REQ-015-13 — verify : procédures d'amorçage, de mise à jour et de
  sauvegarde-restauration jouées (VM de la tâche 7 ou essai consigné dans la note) ;
  `compose.yaml` démarré une fois avec l'image de la tâche 6
  où : design « Documentation » ; ADR 0019 (audit exporté) ; spec 007 W4
  dépend de : 6, 7
  — fait (2026-10-22) : `docs/exploitation/` (`README.md`,
  `nixos.md`, `oci.md`, `configuration.md`, `amorcage.md`, `plugins.md` avec
  l'avertissement W4, `mise-a-jour.md`, `sauvegarde.md`, `observabilite.md`,
  `versions.md`), `compose.yaml` et `initdb.sql` d'essai. `configuration.md` est tenue à
  la main et vérifiée par `config::tests::the_operator_reference_covers_every_field`
  (chaque champ public de `config.rs` cité, exemple complet chargé). Procédures jouées par
  `checks.vm` : amorçage (`kartenn-breakglass`, connexion, `/v1/me`), mise à jour (base en
  retard, migration interrompue puis rejouée) et sauvegarde-restauration (`pg_dump`,
  `dropdb`, `pg_restore --no-owner --role=kartenn`, migration, instances prêtes, mêmes
  personnes, `audit verify` : 7 sceaux, 37 événements). Essai `compose.yaml` joué le
  2026-10-22 avec Podman 5.8 (`podman compose`, délégué à `docker-compose` 5.6 ;
  `podman-compose` absent) et l'image de `nix build .#oci` chargée par `podman load`
  (`localhost/kartenn:0.1.0`) : `up -d` → migration (`deployed`) puis `/health/ready`
  200 ; `compose run --rm migrate breakglass create … secours 4 /run/secrets/keyring`,
  `POST /v1/auth/breakglass` (TOTP) 204, `GET /v1/me` : `administrator`, portée globale ;
  dernière ligne de `_sqlx_migrations` retirée → 503, `stop kartenn`, `run --rm migrate`,
  `start kartenn` → 200 ; `pg_dump` (rôle propriétaire), `dropdb`, `createdb --owner`,
  `pg_restore --no-owner --role=kartenn`, migration, 200, même compte reconnecté,
  `audit verify` : 7 sceaux, 39 événements ; enfin `down --volumes`, nouvel `up -d` sur
  une base neuve → 200, et `down --volumes`. Défaut trouvé : la connexion est refusée en HTTP clair (`TLS
  required`) ; l'essai publiait l'instance en HTTP sur 8080, il la publie désormais en
  HTTPS sur `127.0.0.1:8443` (`[http.tls]`, certificat autosigné généré par `openssl`,
  `oci.md`).
  choix : secrets de l'essai en `chmod 0644` : les secrets de Compose sont des montages
  du fichier, avec ses droits sur l'hôte, et l'image tourne sous 65532 ; port publié sur
  la seule boucle locale (mots de passe d'exemple) ; `secrets/` et `kartenn.dump` ignorés
  par git.
  écart : la « base en retard » ne se simule plus en retirant la dernière ligne de
  `_sqlx_migrations` : la migration 0057 (`ADD COLUMN error`, tâche 006-27) ne repasse
  pas sur une base migrée (essai : colonne défaite à la main avant le rejeu). `checks.vm`
  (tâche 7) fait ce rejeu et doit être revu, comme sa note le prévoyait.
  choix : `compose.yaml` est volontairement sous `docs/exploitation/` et non à la racine :
  c'est un essai documenté de l'image, pas un déploiement ni un outil de développement ;
  son service `migrate` monte aussi le trousseau, pour l'amorçage
  (`compose run --rm migrate breakglass create …`, `oci.md`).
- [x] 9. CI proposée : `scripts/ci.sh` — E14 — verify : le script passe sur `main`
  où : design « CI » ; `devenv.nix` (`enterTest`)
  dépend de : 6, 7
  — `scripts/ci.sh` (`--no-nix` pour sauter la VM et l'image) : vérifie que le
  PostgreSQL de développement répond, puis une ligne par étape, le détail dans un journal :
  hakari, métadonnées sqlx hors ligne (`k-heavy --pg`), `k-check`, `k-web-check`,
  `k-web-e2e`, `nix flake check` (`k-heavy --all`). Passé sur la branche (2026-10-21) :
  hakari 1 s, sqlx 11 s, Rust 142 s, web 32 s, E2E 316 s, Nix 550 s. Une première passe
  a révélé un test instable de `kartenn-testing` (suppression forcée refusée par un
  autovacuum, 42501), corrigé dans la bibliothèque (`drop_database` réessayé 10 s).
  choix : les étapes de `devenv test` (`enterTest`) sont reprises une à une sous le
  sémaphore plutôt que `devenv test` lui-même, qui relance tous les tests hors `k-heavy`
  et sans le verrou PostgreSQL.
- [x] 10. Synchronisation : `docs/spec/architecture.md` § Déploiement (une image,
  module), `interfaces.md` (CLI, `[database]`), D35 amendée dans
  `specs/001-cadrage/decisions.md`, ADR 0029 acceptée, statuts de la 015 — verify :
  audit `spec-keeper` de la 015 sans écart bloquant
  où : `docs/spec/` ; `docs/adr/0029-paquetage-nix-oci.md`
  dépend de : 1 à 9
  — fait (2026-10-21), **non cochée** : `docs/spec/architecture.md` § Déploiement
  (livrables : flake, une image, module, VM ; `_sqlx_migrations` lisible par le rôle
  d'instance, rejeu des migrations), `interfaces.md` (`[database] url_file`, `--version`,
  sous-commandes à URL facultative, `breakglass create` lié à `administrator`,
  `openapi`) ; D35 amendée et ADR 0029 acceptée (déjà sur `main`) ; statuts : exigences
  et conception `implemented`, tâches `in-progress` ; ligne et point d'étape de la 015
  dans `specs/README.md`. Audit `spec-keeper` de la 015 (2026-10-21, sur 8cdd6ba) : aucun
  écart bloquant, les 13 REQ ont code et vérification ; ses points mineurs repris
  (chemins `/opt` de l'image suivis par `checks.oci` et testés côté serveur ; verify de la
  tâche 4 aligné ; note de durcissement datée). Seule la tâche 8 reste ouverte (essai
  `compose.yaml`).

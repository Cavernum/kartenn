---
status: implemented
updated: 2026-10-21
---

# 015 — Paquetage et déploiement : conception

## Vue d'ensemble

```mermaid
flowchart LR
  subgraph flake.nix
    srv[kartenn-server<br/>buildRustPackage] --> pkg[kartenn<br/>fichiers réels]
    web[kartenn-web<br/>pnpm.fetchDeps] --> pkg
    pkg --> oci[oci<br/>buildLayeredImage]
    pkg --> mod[nixosModules.default]
    mod --> vm[checks: test NixOS]
    oci --> chk[checks: configuration de l'image]
  end
  mod --> mig[kartenn-migrate.service] --> inst[kartenn@nom.service]
  mod --> purge[kartenn-audit-purge.timer]
```

Une seule source de nixpkgs pour devenv et le flake (E1). Le flake n'ajoute aucune
entrée en dehors de nixpkgs ; ADR [0029](../../docs/adr/0029-paquetage-nix-oci.md)
(proposée) consigne le choix.

## Flake (REQ-015-1, 2)

- `flake.nix` à la racine ; `inputs.nixpkgs.url =
  "github:cachix/devenv-nixpkgs/rolling"` ; `flake.lock` figé au commit de
  `devenv.lock`. Systèmes : `x86_64-linux`, `aarch64-linux`.
- `nix/package/server.nix` : `rustPlatform.buildRustPackage { pname = "kartenn-server";
  version = <workspace>; src = <filtre : Cargo.*, crates/, .sqlx/, .cargo/>;
  cargoLock.lockFile = ../../Cargo.lock; buildAndTestSubdir = "crates/kartenn-server";
  env.SQLX_OFFLINE = "true"; doCheck = false; }`. La version est lue dans
  `Cargo.toml` (`builtins.fromTOML`) pour n'avoir qu'une source.
- `nix/package/web.nix` : `stdenv.mkDerivation { pnpmDeps = pnpm.fetchDeps { hash =
  …; }; nativeBuildInputs = [ nodejs_24 pnpm.configHook ]; buildPhase = pnpm -C web
  --filter @kartenn/shell --filter @kartenn/plugin-vlan --filter
  @kartenn/plugin-machines build; }` puis copie vers `share/kartenn/…`, chaque plugin
  sous `<nom>/<version du manifeste>/`. Le shell est construit avec la même import map
  que pour le développement (ADR 0025).
- `scripts/update-pnpm-hash.sh` : recalcule `hash` après un changement de
  `web/pnpm-lock.yaml` (construction avec `lib.fakeHash`, lecture de l'empreinte
  attendue).
- `nix/package/default.nix` : `symlinkJoin` des deux, nommé `kartenn`.
- Un test (`checks.lock-sync`) compare le commit de nixpkgs des deux verrous.

Sources : manuel nixpkgs, « Rust » (`buildRustPackage`, `cargoLock`) et « Javascript »
(`pnpm.fetchDeps`, `pnpm.configHook`) — <https://nixos.org/manual/nixpkgs/stable/>.

## Image OCI (REQ-015-3)

- `nix/oci.nix` : `dockerTools.buildLayeredImage { name = "kartenn"; tag = version;
  contents = [ kartenn cacert ]; config = { User = "65532:65532"; Entrypoint = [
  "${kartenn}/bin/kartenn-server" ]; Cmd = [ "run" ]; Env = [
  "SSL_CERT_FILE=…/ca-bundle.crt" ]; ExposedPorts."8443/tcp" = {}; WorkingDir =
  "/"; }; extraCommands` : `/etc/kartenn/` vide, `/opt/kartenn/plugins` → plugins
  livrés, `/tmp` en 1777. }`. Pas de `fakeRootCommands` : rien n'appartient à root
  hors du magasin.
- Configuration par défaut de l'image : `/etc/kartenn/kartenn.toml` monté par
  l'exploitant ; sans fichier, `run` lit les variables `KARTENN_*` (D35, 12-factor).
- Usages : `run /etc/kartenn/kartenn.toml` (instance), `migrate` (job),
  `breakglass create …` (amorçage). La migration lit son URL dans
  `[database] url_file` d'un fichier de configuration minimal monté.
- `checks.oci` : lit le JSON de configuration de l'image et la liste des couches ;
  vérifie `User`, `Entrypoint`, `Cmd`, l'absence de `bin/sh`.

Sources : spécification OCI Image, « Image Configuration »
(<https://github.com/opencontainers/image-spec/blob/main/config.md>) ; manuel nixpkgs,
`dockerTools.buildLayeredImage`.

## Configuration de la base (REQ-015-4)

- `crates/kartenn-server/src/config.rs` : section `[database]` (`url_file:
  Option<PathBuf>`), champs inconnus refusés comme partout ; lecture du fichier au
  démarrage, espaces de fin retirés, contenu jamais journalisé ni affiché dans les
  erreurs (la règle de 012 sur l'URL de connexion s'applique).
- `crates/kartenn-server/src/main.rs` : `<database-url>` devient facultatif pour `run`,
  `migrate`, `audit purge`, `breakglass create` ; exactement une source (argument ou
  `url_file`), sinon refus. `migrate` et `breakglass create` acceptent `--config
  <fichier>` pour lire `[database]`.
- `--version` sur `kartenn-server`.

## Module NixOS (REQ-015-5 à 9)

`nix/module.nix`, exporté en `nixosModules.default`.

| Option | Type | Rôle |
|---|---|---|
| `enable` | bool | |
| `package` | package | défaut : `kartenn` du flake |
| `settings` | TOML libre (`pkgs.formats.toml`) | configuration de `config.rs`, fusionnée avec ce que pose le module (`web.root`, `web.plugins_dir`, `database.url_file`) |
| `instances.<nom>.listen` | chaîne | adresse de `[http] listen` propre à l'instance ; défaut : une instance `main` |
| `databaseUrlFile` | chemin | URL du rôle d'instance (E7) |
| `migrationDatabaseUrlFile` | chemin | URL du propriétaire du schéma |
| `credentials` | attrset nom → chemin | autres secrets, passés par `LoadCredential=` ; `settings` les désigne par `%d/<nom>` |
| `plugins` | liste de paquets | plugins de l'exploitant, joints aux plugins livrés |
| `database.createLocally` | bool | PostgreSQL 18 local, rôles et authentification `peer` |
| `openFirewall` | bool | ouvre les ports des instances |

Unités :
- `kartenn-migrate.service` : `Type=oneshot`, `RemainAfterExit=yes`,
  `LoadCredential=database-url:<migrationDatabaseUrlFile>`, `ExecStart=kartenn-server
  migrate --config <fichier généré>` ; `kartenn-migrate.timer` : `OnCalendar=monthly`,
  `Persistent=true`.
- `kartenn@<nom>.service` : `Requires=` et `After=kartenn-migrate.service`,
  `Restart=on-failure`, `ExecStart=kartenn-server run <fichier généré pour nom>`,
  `ExecReload=kill -HUP $MAINPID` (certificat TLS et plugins relus).
- `kartenn-audit-purge.service` et `.timer` : `OnCalendar=daily`.
- Durcissement commun (E6) ; `DynamicUser=yes` : les secrets ne sont lisibles que par
  l'unité via `$CREDENTIALS_DIRECTORY`.
- Le fichier de configuration généré ne contient que des chemins, jamais un secret
  (REQ-015-5) ; il est dans le magasin.
- `database.createLocally` : `services.postgresql.package = postgresql_18`,
  `ensureDatabases = [ "kartenn" ]`, rôle propriétaire `kartenn` (utilisateur système de
  la migration), rôle d'instance `kartenn_server` membre de `kartenn_app` (créé par la
  migration 0001 ; l'appartenance est posée par un `postStart`) ; URL par socket Unix,
  sans secret.

Sources : manuel NixOS, « Writing NixOS Modules » ; `systemd.exec(5)` (`DynamicUser`,
`LoadCredential`, options de protection) ; `systemd.unit(5)`, `systemd.timer(5)` ;
`systemd-analyze(1)` (`security`).

## Premier administrateur (REQ-015-10)

`crates/kartenn-server/src/main.rs::create_breakglass` crée le compte, puis, dans la même
transaction, une liaison du rôle livré `administrator`, portée globale, sujet le compte de
secours, valide jusqu'à `valid_until` du compte ; l'événement
`account.breakglass.created` reçoit `role: administrator`. La liaison passe par le
pipeline de changesets sous l'identité `system` (D11 : toute écriture est un changeset),
motif « amorçage par compte de secours ». Spec 005 : REQ-005-14 et K9 amendés.

## Mise à jour (REQ-015-11)

`/health/ready` compare la dernière version de migration appliquée (`_sqlx_migrations`)
à la plus haute que le binaire embarque (`MIGRATOR`) ; vérifier l'existant (spec 012) et
compléter si besoin. Rejeu : sqlx applique chaque fichier dans sa transaction et
n'inscrit la version qu'au commit ; les migrations sans transaction (`-- no-transaction`)
sont recensées et rendues idempotentes.

## Versionnement (REQ-015-12)

`Cargo.toml` `[workspace.package] version = "0.1.0"` ; toutes les crates en
`version.workspace = true` ; `web/*/package.json` à la même version ; test
`crates/kartenn-server/tests/it/version.rs` qui compare les versions. `CHANGELOG.md` à la
racine. Procédure de version dans `docs/exploitation/versions.md`.

## Documentation (REQ-015-13)

`docs/exploitation/` : `README.md` (index), `nixos.md`, `oci.md` (avec
`compose.yaml` d'essai : PostgreSQL 18, migration, une instance), `configuration.md`
(référence générée depuis `config.rs` si possible, sinon tenue à la main avec un test de
couverture des champs), `amorcage.md`, `plugins.md` (W4), `mise-a-jour.md`,
`sauvegarde.md`, `observabilite.md`.

## CI (E14)

`scripts/ci.sh` : `devenv test`, `k-web-e2e`, `nix flake check` ; sortie courte.
Amendé par l'ADR 0033 : `.github/workflows/ci.yml` et `release.yml` appellent
`scripts/ci.sh`, `scripts/ci-lint.sh` et `scripts/release.sh` ; procédure dans
`docs/developpement/publication.md`.

## Stratégie de test

- `checks.<sys>.vm` : `nixosTest` sur une machine virtuelle : PostgreSQL local, migration, deux instances, sondes, compte de secours, connexion,
  liaison, plugin ajouté, refus `UPDATE` sur l'historique, secrets absents, note de
  durcissement, migration en retard → 503.
- `checks.<sys>.oci` : configuration de l'image (sans la lancer).
- Rust : tests de `config.rs` et `main.rs` (REQ-015-4), test de version (REQ-015-12),
  test d'amorçage (REQ-015-10) dans `kartenn-server/tests/it/`, sous
  `#[kartenn_testing::pg_test]` (ADR 0028).
- Les `checks` Nix ne tournent pas dans `k-check` (durée, VM) : ils passent par
  `nix flake check` et `scripts/ci.sh`.

## Risques

- Empreinte pnpm à tenir à jour : script dédié et `checks` qui échoue en clair.
- `nix flake check` lourd (VM) : sous `k-heavy --pg` si lancé sur la machine partagée.
- Différence de chaîne Rust si `devenv.lock` avance sans `flake.lock` : test de
  synchronisation (REQ-015-2).

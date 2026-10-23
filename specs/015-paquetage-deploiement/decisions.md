# 015 — Paquetage et déploiement : journal des décisions

Entretien mené le 2026-10-21 par délégation : l'utilisateur a fixé l'ordre « paquetage
puis v0.2 » et laissé les choix réversibles à la recommandation la plus standard. Les
choix **structurants** (point d'entrée Nix, format de configuration, publication) sont
marqués « à valider ». Cadre : D9 (N instances sans état), D15 (PostgreSQL ≥ 18), D35
(OCI, module NixOS, configuration 12-factor, OpenTelemetry), D37 (AGPL-3.0) ; ADR 0012
(instances sans état), 0020 (session BFF), 0023 (tâches sous bail), 0025 (plugins web) ;
W2 et W3 (spec 007) ; K9 (spec 005). Existant : aucun `flake.nix`, aucune CI, version
`0.0.0` ; devenv épingle nixpkgs par `devenv.yaml` (`devenv-nixpkgs/rolling`, Rust
1.98) ; `kartenn-server` reçoit l'URL de la base en argument.

## Synthèse
- E1 point d'entrée : `flake.nix` à la racine, même nixpkgs que devenv (à valider).
- E2 binaire : `rustPlatform.buildRustPackage`, sqlx hors ligne, modules par défaut.
- E3 interface : `pnpm.fetchDeps` ; shell et plugins de première partie construits par Nix.
- E4 image OCI : une seule image, `dockerTools.buildLayeredImage`, non root, sans shell (amende D35, à valider).
- E5 base de données : secret par fichier, section `[database]` (à valider).
- E6 module NixOS : `services.kartenn`, unité de migration, instances nommées, durcissement systemd.
- E7 rôles PostgreSQL : migration par le propriétaire, instances par un rôle de connexion membre de `kartenn_app`.
- E8 plugins : vlan et machines livrés ; plugins de l'exploitant ajoutés par une option.
- E9 premier administrateur : `breakglass create` lie le rôle `administrator`.
- E10 mise à jour : arrêt, migration, redémarrage ; mise à jour roulante pour les seules migrations additives.
- E11 sauvegarde : documentée (sauvegarde physique et archivage des WAL), hors du module.
- E12 versionnement : SemVer, étiquettes `vX.Y.Z` signées ; journal des changements rendu par git-cliff, faits marquants à la main (amendé par l'ADR 0033).
- E13 publication de l'image : GHCR par défaut, réversible ; signature des artefacts plus tard (ADR 0033).
- E14 CI : scripts neutres, appelés par des workflows GitHub Actions (amendé par l'ADR 0033).
- E15 documentation de l'exploitant : `docs/exploitation/`.

## E1 — Point d'entrée Nix
- Décision : un **`flake.nix` à la racine**, dont l'entrée `nixpkgs` suit la même source
  que devenv (`github:cachix/devenv-nixpkgs/rolling`), figée dans `flake.lock` au même
  commit que `devenv.lock` ; sorties `packages.<système>` (`kartenn-server`,
  `kartenn-web`, `kartenn`, `oci`), `nixosModules.default`, `checks.<système>` (paquets,
  test NixOS). devenv reste l'environnement de développement ; le flake ne fournit pas de
  `devShell`.
- Pourquoi : le module NixOS et l'image se consomment par un flake ; un seul nixpkgs pour
  le développement et la livraison garde la même chaîne Rust (1.98, `rust-version` du
  workspace) et les mêmes navigateurs.
- Rejeté : `default.nix` sans flake (pas de verrou d'entrées, consommation moins
  standard) ; nixpkgs `nixos-unstable` distinct (deux chaînes à garder alignées).
- Structurant : nouveau point d'entrée et nouvelle entrée Nix. **À valider.**
- Décidé par : Claude (délégation de l'utilisateur, 2026-10-21)

## E2 — Construction du binaire
- Décision : `rustPlatform.buildRustPackage` de nixpkgs, `cargoLock.lockFile =
  ./Cargo.lock`, `SQLX_OFFLINE=true` (métadonnées `.sqlx/` versionnées), features par
  défaut de `kartenn-server` (modules `vlan` et `machines`), seul le binaire
  `kartenn-server` installé, `doCheck = false` (les tests exigent PostgreSQL : ils
  tournent dans `k-check` et le test NixOS, pas dans le bac à sable de construction).
- Pourquoi : aucune entrée Nix de plus ; sqlx hors ligne ne demande pas de base au
  build ; les migrations sont embarquées (`sqlx::migrate!`).
- Rejeté : crane (entrée supplémentaire ; son cache des dépendances sert surtout une CI
  qui n'existe pas) ; naersk (moins maintenu) ; construire avec `cargo` hors Nix pour
  l'image (non reproductible).
- Décidé par : Claude (délégation de l'utilisateur, 2026-10-21)

## E3 — Construction de l'interface
- Décision : dérivation `kartenn-web` par `stdenv.mkDerivation` avec `pnpm.fetchDeps`
  (empreinte fixe des dépendances) et `pnpm.configHook` ; sortie
  `share/kartenn/web/shell/` (build du shell) et
  `share/kartenn/plugins/<nom>/<version>/` (plugins `vlan` et `machines`, manifeste
  compris). Le plugin d'exemple n'est pas livré. Un script régénère l'empreinte pnpm
  quand `pnpm-lock.yaml` change.
- Pourquoi : W2 (le serveur sert le build) et W6 (plugins de première partie livrés) ;
  pnpm seul (ADR 0024).
- Rejeté : construire l'interface dans l'image par un Dockerfile (non reproductible) ;
  livrer le plugin d'exemple (code de démonstration chez l'exploitant).
- Décidé par : Claude (délégation de l'utilisateur, 2026-10-21)

## E4 — Image OCI
- Décision : **une seule image** `kartenn`, construite par
  `dockerTools.buildLayeredImage` (sans démon, couches par dépendance) : `kartenn-server`,
  `kartenn-web`, certificats racine (`cacert`) ; utilisateur numérique non root
  (65532:65532), aucun shell ni gestionnaire de paquets ; `Entrypoint =
  ["kartenn-server"]`, `Cmd = ["run"]` ; configuration par fichier monté et variables
  `KARTENN_*`. Le job de migration est la même image avec `migrate`. Pas d'image « web
  statique » : le serveur sert le shell (W2).
- Pourquoi : une image, trois usages (instance, migration, compte de secours) : une seule
  version à publier ; surface d'attaque minimale.
- Rejeté : trois images (D35 d'origine : serveur, migration, web statique ; la troisième
  contredit W2, les deux premières ont le même contenu) ; image sur base de distribution
  (shell et paquets inutiles).
- Conséquence : D35 est amendée (une image au lieu de trois). **À valider.**
- Décidé par : Claude (délégation de l'utilisateur, 2026-10-21)

## E5 — URL de la base et secrets
- Décision : l'URL de la base peut venir d'un **fichier** : nouvelle section `[database]`
  (`url_file`), lue par `run`, `migrate`, `audit purge` et `breakglass create` ;
  l'argument `<database-url>` reste accepté (compatibilité, développement). Tous les
  autres secrets sont déjà des fichiers (`key_file`, `client_secret_file`,
  `signing_key_file`, `tls.key_file`). L'URL n'apparaît plus dans la ligne de commande.
- Pourquoi : un argument se lit dans `/proc/<pid>/cmdline` par tout utilisateur de la
  machine ; un fichier se transmet par les identifiants systemd (`LoadCredential=`) ou un
  secret monté.
- Rejeté : variable d'environnement `KARTENN_DATABASE_URL` (le préfixe `KARTENN_` est lu
  comme configuration et une variable d'environnement fuit vers les processus fils et les
  rapports de plantage) ; garder l'argument seul.
- Structurant : format de configuration (section nouvelle, additive). **À valider.**
- Décidé par : Claude (délégation de l'utilisateur, 2026-10-21)

## E6 — Module NixOS
- Décision : `services.kartenn` :
  - `enable`, `package`, `settings` (TOML libre, `pkgs.formats.toml`, fusionné avec les
    valeurs du module), `plugins` (liste de dérivations ajoutées aux plugins livrés),
    `databaseUrlFile`, `migrationDatabaseUrlFile`, `credentials` (fichiers transmis par
    `LoadCredential=`), `instances.<nom>` (au moins `main` ; chacune son `listen`),
    `openFirewall`, `database.createLocally` ;
  - unités : `kartenn-migrate.service` (oneshot, avant les instances, relancée par un
    temporisateur mensuel pour les partitions d'historique, 004-25),
    `kartenn@<nom>.service` (instances, `Requires`/`After` la migration),
    `kartenn-audit-purge.timer` (quotidien, REQ-012-16) ;
  - durcissement systemd : `DynamicUser`, `ProtectSystem=strict`, `ProtectHome`,
    `PrivateTmp`, `PrivateDevices`, `NoNewPrivileges`, `CapabilityBoundingSet=` vide
    (sauf `CAP_NET_BIND_SERVICE` pour un port < 1024), `RestrictAddressFamilies=AF_INET
    AF_INET6 AF_UNIX`, `SystemCallFilter=@system-service`, `MemoryDenyWriteExecute`,
    `LockPersonality`, `ProtectKernel*`, `ProtectControlGroups`, `RestrictNamespaces`,
    `RestrictRealtime`, `UMask=0077` ; cible : `systemd-analyze security` sous 2,0 ;
  - `database.createLocally` active PostgreSQL 18 local avec le rôle propriétaire et le
    rôle d'instance (E7), par socket et authentification `peer`.
- Pourquoi : D35 ; les instances sont sans état (ADR 0012), donc `DynamicUser` sans
  répertoire d'état ; migration séparée (D9 : job unique avant les instances).
- Rejeté : migration au démarrage de chaque instance (N instances concurrentes, et le
  rôle d'instance n'a pas les droits du propriétaire) ; options typées pour toute la
  configuration (double maintenance avec `config.rs`).
- Décidé par : Claude (délégation de l'utilisateur, 2026-10-21)

## E7 — Rôles PostgreSQL en production
- Décision : la migration se connecte avec le **propriétaire du schéma** ; les instances
  avec un **rôle de connexion membre de `kartenn_app`**, sans droit d'administration
  (INV-11 : pas d'`UPDATE` ni de `DELETE` sur l'historique). Deux URL, donc deux
  fichiers de secret (E5).
- Pourquoi : moindre privilège ; c'est déjà le modèle des tests (`SET ROLE kartenn_app`).
- Question ouverte : vérifier que `run` fonctionne entièrement sous ce rôle (tâche 6,
  test NixOS).
- Décidé par : Claude (délégation de l'utilisateur, 2026-10-21)

## E8 — Plugins livrés et plugins de l'exploitant
- Décision : le paquet livre `vlan` et `machines` sous `share/kartenn/plugins/`. Le module
  NixOS assemble ce répertoire et les dérivations de `services.kartenn.plugins` par
  `symlinkJoin` et pose `[web] plugins_dir` dessus ; l'image OCI pose
  `plugins_dir=/opt/kartenn/plugins`, que l'exploitant peut remplacer par un volume. La
  documentation répète l'avertissement W4 : installer un plugin revient à lui confier la
  session de chaque utilisateur.
- Pourquoi : W3 (répertoire déclaré), W6 (première partie livrée et activée selon les
  modules du build : un plugin de module absent n'est pas publié).
- Décidé par : Claude (délégation de l'utilisateur, 2026-10-21)

## E9 — Premier administrateur
- Décision : `kartenn-server breakglass create` **lie le rôle livré `administrator`, de
  portée globale**, au compte de secours créé, pour sa durée de validité (24 h au plus) ;
  l'événement d'audit de création le mentionne. Parcours d'amorçage : migration →
  compte de secours → connexion → rattachement des groupes OIDC. Amende REQ-005-14.
- Pourquoi : sur une instance neuve, rien d'autre ne donne ce rôle (les parcours de bout
  en bout le posaient en SQL).
- Décidé par : l'utilisateur (2026-10-21)

## E10 — Mise à jour
- Décision : procédure par défaut : arrêt des instances, migration, redémarrage. Mise à
  jour roulante (instances anciennes et nouvelles en même temps) permise seulement quand
  les notes de version l'indiquent (migrations additives). `/health/ready` refuse le
  trafic tant que la version des migrations attendue par le binaire n'est pas atteinte.
  Une migration interrompue se rejoue : chaque fichier est appliqué dans sa transaction.
- Pourquoi : simple et sûr pour une v0.1 ; la roulante exige une discipline
  d'expansion puis contraction qu'on n'impose pas encore.
- Décidé par : Claude (délégation de l'utilisateur, 2026-10-21)

## E11 — Sauvegarde et restauration
- Décision : documentée, pas fournie par le module : sauvegarde physique
  (`pg_basebackup`) et archivage continu des WAL pour une restauration à un instant
  donné ; `pg_dump` accepté pour une petite instance. Après une restauration :
  `kartenn-server audit verify` (chaîne des sceaux) et relance des instances. Le journal
  d'audit exporté (ADR 0019) reste la preuve hors base.
- Pourquoi : la sauvegarde relève de l'exploitation de PostgreSQL, déjà outillée ; une
  restauration doit vérifier l'intégrité de l'audit.
- Décidé par : Claude (délégation de l'utilisateur, 2026-10-21)

## E12 — Versionnement
- Décision : SemVer ; version unique du workspace (`[workspace.package] version`), des
  paquets web et du paquet Nix ; étiquette annotée `vX.Y.Z` ; `CHANGELOG.md` au format
  « Keep a Changelog », tenu à la main à partir des Conventional Commits ;
  `kartenn-server --version`. Première version : `0.1.0`.
- Rejeté : générateur de journal des changements (dépendance de plus pour un gain faible
  à ce rythme).
- Décidé par : Claude (délégation de l'utilisateur, 2026-10-21)
- **Amendement (2026-10-23, [ADR 0033](../../docs/adr/0033-publication-des-versions.md))** :
  à la demande de l'utilisateur, git-cliff (`cliff.toml`) rend chaque section de
  `CHANGELOG.md` depuis les Conventional Commits ; les faits marquants restent écrits à
  la main sous `[Non publié]`. L'étiquette `vX.Y.Z` est annotée **et signée** par le
  mainteneur (`k-release`), son message est la section de la version, en markdown. Le
  rejet du générateur est levé.

## E13 — Publication
- Décision : en v0.1, `nix build .#oci` produit l'archive de l'image ; sa publication dans
  un registre et la signature des artefacts sont **à décider par l'utilisateur**
  (registre, compte, clés).
- Structurant et externe. **À valider.**
- Décidé par : Claude (délégation de l'utilisateur, 2026-10-21), question ouverte
- **Amendement (2026-10-23, ADR 0033)** : registre GHCR par défaut
  (`ghcr.io/<dépôt>:X.Y.Z`, empreinte dans les notes de la release), choix réversible ;
  artefacts et `SHA256SUMS` attachés à la release GitHub. Signature et provenance des
  artefacts : toujours à décider (Sigstore sans clé ou clé de projet), place réservée
  dans le workflow.

## E14 — Intégration continue
- Décision : un script `scripts/ci.sh` neutre vis-à-vis de la forge : `devenv test`,
  `k-web-e2e`, `nix flake check`. Le brancher sur une forge reste un choix de
  l'utilisateur.
- Décidé par : Claude (délégation de l'utilisateur, 2026-10-21)
- **Amendement (2026-10-23, ADR 0033)** : l'utilisateur choisit GitHub Actions. Les
  workflows `.github/workflows/ci.yml` et `release.yml` n'appellent que les scripts
  neutres (`scripts/ci.sh`, `scripts/ci-lint.sh`, `scripts/release.sh`) et le flake :
  une autre forge les réutiliserait tels quels.

## E15 — Documentation de l'exploitant
- Décision : `docs/exploitation/` : installation NixOS, installation OCI (exemple Compose
  pour essai), référence de configuration, secrets, amorçage, plugins (avertissement W4),
  mise à jour, sauvegarde et restauration, observabilité. Manifestes Kubernetes (D35 :
  Helm ou Kustomize) reportés.
- Décidé par : Claude (délégation de l'utilisateur, 2026-10-21)

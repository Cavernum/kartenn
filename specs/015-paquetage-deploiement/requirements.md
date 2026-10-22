---
status: implemented
updated: 2026-10-22
---

# 015 — Paquetage et déploiement : exigences

> Implémentées et vérifiées : les procédures de l'exploitant sont jouées dans la VM du
> module et, pour l'image OCI, par l'essai `docs/exploitation/compose.yaml` (tâche 8).

## Résumé

Rendre la v0.1 installable : un paquet Nix, une image OCI et un module NixOS, construits
de façon reproductible depuis le dépôt ; des secrets passés par fichiers ; un amorçage du
premier administrateur ; une procédure de mise à jour et de sauvegarde ; une
documentation de l'exploitant. Décisions : [`decisions.md`](decisions.md) (E1–E15) ; D9,
D15, D35, D37 ([`../001-cadrage/decisions.md`](../001-cadrage/decisions.md)) ; ADR 0012,
0019, 0025 ; W2–W4, W6 ([007](../007-web-shell-plugins/decisions.md)) ; K9
([005](../005-authn-rbac/decisions.md)).

### Vocabulaire

- **Paquet** : dérivation Nix `kartenn` (binaire `kartenn-server`, build du shell,
  plugins de première partie).
- **Image** : image OCI `kartenn` construite depuis le paquet.
- **Module** : module NixOS `services.kartenn`.
- **Instance** : un processus `kartenn-server run` (ADR 0012).
- **Rôle d'instance** : rôle PostgreSQL de connexion, membre de `kartenn_app`.

## Exigences

### A. Construction

#### REQ-015-1 — Paquet reproductible
kartenn doit fournir un `flake.nix` dont `nix build .#kartenn` produit, sans accès au
réseau pendant la construction, `bin/kartenn-server` (modules `vlan` et `machines`),
`share/kartenn/web/shell/` et `share/kartenn/plugins/{vlan,machines}/<version>/`. [E1–E3]

Acceptation :
- deux constructions du même commit donnent le même chemin de sortie ;
- `kartenn-server --version` affiche la version du workspace ;
- le répertoire des plugins ne contient pas le plugin d'exemple.

#### REQ-015-2 — Même chaîne que le développement
Le flake doit suivre le nixpkgs de devenv, et la construction doit utiliser une chaîne
Rust qui satisfait `rust-version` du workspace. [E1]

Acceptation : `flake.lock` et `devenv.lock` désignent le même commit de nixpkgs ; un
test le vérifie.

#### REQ-015-3 — Image OCI minimale
`nix build .#oci` doit produire une image OCI dont l'utilisateur est non root
(65532:65532), sans shell ni gestionnaire de paquets, avec `Entrypoint =
["kartenn-server"]` et `Cmd = ["run"]`, contenant le paquet et les certificats racine.
[E4]

Acceptation :
- la configuration de l'image (`User`, `Entrypoint`, `Cmd`) est vérifiée par un test ;
- aucun fichier `bin/sh` dans les couches ;
- la taille compressée est relevée dans les notes de version (pas de budget en v0.1).

### B. Configuration et secrets

#### REQ-015-4 — URL de la base par fichier
Where `[database] url_file` est configuré, `kartenn-server` (`run`, `migrate`,
`audit purge`, `breakglass create`) doit lire l'URL de la base dans ce fichier ;
l'argument `<database-url>` reste accepté. [E5]

Acceptation :
- avec `url_file` et sans argument, `run` démarre ;
- un fichier illisible ou vide refuse le démarrage, sans écrire son contenu dans les
  logs ;
- l'URL n'apparaît ni dans `/proc/<pid>/cmdline` ni dans les logs ;
- avec à la fois l'argument et `url_file`, le démarrage est refusé (ambiguïté).

#### REQ-015-5 — Secrets par fichiers seulement
Le module et l'image doivent transmettre tous les secrets (URL de base, trousseau,
secrets clients, clé de signature de l'audit, clé TLS) par fichiers ; aucun secret dans
le magasin Nix, dans la ligne de commande ou dans l'environnement. [E5, E6]

Acceptation : le test NixOS vérifie qu'aucun secret de test n'apparaît dans
`/nix/store`, dans `systemctl show` ni dans `/proc/<pid>/environ`.

### C. Module NixOS

#### REQ-015-6 — Service
When `services.kartenn.enable` est vrai, le module doit installer une unité de migration
oneshot, une unité par instance de `services.kartenn.instances`, démarrée après la
migration réussie, et un temporisateur quotidien de purge de l'audit (REQ-012-16) et un
temporisateur mensuel de migration (partitions d'historique, 004-25). [E6, E10]

Acceptation (test NixOS) : après démarrage, `/health/ready` répond 200 sur chaque
instance ; une migration en échec empêche le démarrage des instances.

#### REQ-015-7 — Durcissement
Les unités du module doivent tourner sans privilège (`DynamicUser`), et
`systemd-analyze security` doit noter chaque unité d'instance à 2,0 ou moins. [E6]

Acceptation : le test NixOS lit la note de `systemd-analyze security kartenn@main`.

#### REQ-015-8 — Base locale
Where `services.kartenn.database.createLocally` est vrai, le module doit configurer
PostgreSQL 18 avec le rôle propriétaire du schéma (migration) et un rôle d'instance
membre de `kartenn_app`, sans mot de passe, par socket local. [E6, E7]

Acceptation : le test NixOS crée un changeset et vérifie qu'un `UPDATE` sur une table
d'historique est refusé au rôle d'instance (INV-11).

#### REQ-015-9 — Plugins
Le module doit servir les plugins livrés et ceux de `services.kartenn.plugins` depuis un
seul répertoire `plugins_dir` en lecture seule ; la documentation doit avertir
qu'installer un plugin lui confie la session de chaque utilisateur (W4). [E8]

Acceptation : un plugin de test ajouté par l'option apparaît dans `ui_plugins` de
`/v1/capabilities` ; le plugin `vlan` est publié.

### D. Amorçage et exploitation

#### REQ-015-10 — Premier administrateur
`kartenn-server breakglass create` doit lier au compte créé le rôle livré
`administrator`, de portée globale, pour la durée de validité du compte, et l'événement
d'audit de création doit le mentionner. Amende REQ-005-14. [E9]

Acceptation : sur une base neuve, après `migrate` puis `breakglass create`, le compte se
connecte et crée une liaison de groupe OIDC (`POST /v1/bindings`) ; après expiration du
compte, la liaison du compte ne donne plus aucun droit.

#### REQ-015-11 — Mise à jour
`/health/ready` doit répondre 503 tant que la base n'a pas toutes les migrations que le
binaire embarque ; une migration interrompue doit pouvoir être rejouée sans
intervention manuelle. [E10]

Acceptation : instance démarrée sur une base en retard d'une migration → 503 ;
`migrate` → 200 ; migration tuée au milieu puis relancée → succès.

#### REQ-015-12 — Versionnement
Chaque version publiée doit avoir une étiquette annotée `vX.Y.Z`, une entrée dans
`CHANGELOG.md` et la même version dans le workspace Cargo, les paquets web et le paquet
Nix. [E12]

Acceptation : un test compare ces versions entre elles.

#### REQ-015-13 — Documentation de l'exploitant
kartenn doit fournir `docs/exploitation/` : installation NixOS et OCI, configuration et
secrets, amorçage, plugins (W4), mise à jour, sauvegarde et restauration (avec
`audit verify` après restauration), observabilité. [E11, E15]

Acceptation : chaque procédure est jouée telle qu'écrite dans le test NixOS ou un essai
consigné (amorçage, mise à jour, sauvegarde et restauration).

## Non-objectifs

- Manifestes Kubernetes (Helm, Kustomize) : reportés (D35).
- Publication dans un registre et signature des artefacts : en attente de décision (E13).
- Sauvegarde automatisée par le module (E11).
- Réplicas en lecture (D9) : configuration non couverte en v0.1.
- Paquets d'autres distributions.

## Questions ouvertes

- E13 : registre de l'image, signature (Sigstore ou clé de projet). Responsable :
  utilisateur.
- E7 : `run` entièrement fonctionnel sous le rôle d'instance (vérifié par la tâche 7).
- Budget de taille de l'image : relevé en v0.1, fixé ensuite.

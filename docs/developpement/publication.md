# Publier une version

Procédure du mainteneur ([ADR 0033](../adr/0033-publication-des-versions.md)). En bref :
le mainteneur crée et signe l'étiquette `vX.Y.Z` sur sa machine ; la poussée de
l'étiquette déclenche `.github/workflows/release.yml`, qui vérifie, construit, attend une
approbation puis publie l'image et la release GitHub. Aucune clé privée n'est confiée à
la CI.

| Commande (devenv) | Rôle |
|---|---|
| `k-release-notes [X.Y.Z]` | Aperçu du message de l'étiquette (et des notes de la release) ; sans argument, version calculée depuis les Conventional Commits |
| `k-release X.Y.Z` | Version, `CHANGELOG.md`, commit, étiquette signée, poussée après confirmation |
| `k-release-verify vX.Y.Z` | Les contrôles du job `verify` : signature, `main`, versions, `CHANGELOG.md` |
| `k-ci-lint` | actionlint, zizmor et shellcheck sur les workflows et les scripts |

Toutes appellent `scripts/release.sh` ou `scripts/ci-lint.sh`, utilisables hors de devenv
et d'une autre forge.

Les scripts du dépôt (`scripts/*.sh`, `web/e2e/serve.sh`, les scripts de `devenv.nix`)
sont du sh POSIX exécuté par le dash de Nix (`#!/usr/bin/env dash`) : dash 0.5.13 ou
plus, pour `set -o pipefail`, que le dash de Debian 12 ou d'Ubuntu 24.04 n'a pas. Hors de
devenv : `nix shell nixpkgs#dash --command scripts/release.sh …`. `k-ci-lint` les passe à
`shellcheck --shell=dash`, ceux de `devenv.nix` compris depuis le shell devenv.

## Préparer

1. Rédiger les faits marquants sous `## [Non publié]` de `CHANGELOG.md`, au fil de l'eau
   ou juste avant : quelques paragraphes ou une liste, en français, pour un lecteur
   exploitant. Ils ouvrent la section de la version ; la liste des commits rendue par
   git-cliff se replie en dessous. Commit `docs(changelog): …` (écarté des notes).
2. `k-release-notes` : relire le rendu. Un commit mal typé se corrige avant l'étiquette,
   pas après (`cliff.toml` dit quels types sont retenus).
3. Pousser `main` et attendre la CI verte sur ce commit (`gh run watch`).

## Publier

```sh
k-release 0.2.0
```

Le script refuse si : la branche n'est pas `main` ; l'arbre n'est pas propre ; `main`
diffère de `main` de `upstream` ou d'`origin` (après `git fetch`) ; `v0.2.0` existe
déjà ; 0.2.0 n'est pas supérieure à la dernière étiquette ; la dernière exécution du
workflow CI sur ce commit n'est pas terminée en succès (`gh` connecté requis).
`--skip-ci-check` lève ce dernier contrôle, avec un avertissement : réservé au cas où la
CI est indisponible et où `scripts/ci.sh` a passé en local.

Il porte ensuite la version (workspace, dépendances internes, `web/{sdk,shell,e2e}`,
`Cargo.lock`), écrit la section dans `CHANGELOG.md`, commite
`chore(release): v0.2.0` (crochets pre-commit compris), crée l'étiquette signée et la
vérifie contre `.github/release-keys`. Il affiche alors les commandes de poussée et
demande de taper `v0.2.0` pour les lancer :

```sh
git push --atomic upstream refs/heads/main refs/tags/v0.2.0
git push --atomic origin refs/heads/main refs/tags/v0.2.0
```

Toute autre réponse arrête sans rien pousser. Pour tout défaire tant que rien n'est
poussé :

```sh
git tag -d v0.2.0
git reset --keep HEAD~1
```

Variables utiles : `K_RELEASE_REMOTES` (dépôts poussés, défaut `upstream origin`),
`K_RELEASE_SIGNING_KEY` (clé de `git tag -u`), `KARTENN_REPO_URL` (URL des liens des
notes, défaut : `upstream`).

## Ce que fait le workflow de release

| Job | Permissions | Contenu |
|---|---|---|
| `verify` | `contents: read` | `scripts/release.sh verify` : étiquette annotée, sujet `vX.Y.Z`, signature par une clé de `.github/release-keys`, commit sur `origin/main`, versions et section de `CHANGELOG.md` égale au message |
| `ci` | `contents: read` | `ci.yml` entier sur le commit étiqueté, sans cache |
| `build` | `contents: read` | `scripts/release.sh artefacts` : archive de l'image (`nix build .#oci`), archive des sources, `SHA256SUMS` |
| `publish` | `contents: write`, `packages: write` ; environnement `release` | contrôle de `SHA256SUMS`, image vers `ghcr.io/<propriétaire>/<dépôt>:X.Y.Z`, release GitHub liée à l'étiquette (`--verify-tag`), notes = corps du message de l'étiquette + référence et empreinte de l'image |

La release GitHub a pour titre `vX.Y.Z` et pour notes le markdown du message de
l'étiquette, sans la signature (rangée à part dans l'objet étiquette).

## Réglages GitHub (une fois, par dépôt)

À faire dans l'interface, par un administrateur du dépôt ; rien de cela n'est dans le
dépôt.

1. **Actions › General** : « Workflow permissions » en lecture seule (les workflows
   demandent leurs droits job par job) ; ne pas autoriser Actions à créer ou approuver
   des pull requests ; exiger l'approbation des workflows des contributeurs externes.
   Optionnel : n'autoriser que les actions épinglées par SHA complet.
2. **Environments › `release`** : relecteurs obligatoires (le mainteneur, et idéalement
   une seconde personne ; « Prevent self-review » si deux personnes) ; « Deployment
   branches and tags » limité à la règle d'étiquette `v*`. Aucun secret d'environnement :
   le job utilise le `GITHUB_TOKEN` éphémère.
3. **Rules › Rulesets** :
   - branche `main` : pull request obligatoire, statuts requis (jobs `lint`, `devenv`,
     `nix` de CI), pas de force-push ni de suppression, commits signés si souhaité ;
   - étiquettes `v*` : création réservée aux mainteneurs (contournement par rôle), ni
     mise à jour ni suppression ; signatures requises.
4. **Releases** : activer les releases immuables (« Immutable releases ») : une release
   publiée ne change plus d'artefacts, et son étiquette est verrouillée.
5. **Packages** (après la première publication) : paquet `ghcr.io/<propriétaire>/<dépôt>`
   rendu public, lié au dépôt ; dans « Manage Actions access », seul ce dépôt en
   écriture.
6. **Code security** : alertes et mises à jour de sécurité Dependabot activées (Cargo et
   pnpm n'ont pas de mises à jour de version automatiques, voir `.github/dependabot.yml`).

## Vérifier une étiquette et une image

```sh
git fetch --tags upstream
k-release-verify v0.2.0                       # ou : scripts/release.sh verify v0.2.0
git tag -v v0.2.0                             # contre son propre trousseau
scripts/release.sh tag-notes v0.2.0           # le markdown du message
```

Artefacts d'une release :

```sh
gh release download v0.2.0 --dir v0.2.0 && cd v0.2.0
sha256sum --check SHA256SUMS
```

L'archive de l'image se reconstruit depuis l'étiquette (`git checkout v0.2.0 &&
nix build .#oci`) et se compare à celle de la release. L'image du registre se désigne par
l'empreinte donnée dans les notes :

```sh
skopeo inspect docker://ghcr.io/<propriétaire>/<dépôt>@sha256:…
```

## Rotation d'une clé de publication

Les clés publiques autorisées sont `.github/release-keys/<empreinte>.asc` (export
minimal : `gpg --armor --export-options export-minimal --export <empreinte>`). Ajouter
une clé, ou retirer une clé compromise ou expirée, est un commit sur `main` (pull request
revue). Une signature par une clé expirée ou révoquée est refusée. Les étiquettes déjà
publiées ne sont pas revérifiées. Ne jamais signer une étiquette par une sous-clé d'une
clé absente de ce répertoire : seule l'empreinte de la clé primaire est comparée.

## Reprendre après un échec

- **`k-release` s'arrête avant le commit** : il affiche la commande `git restore` qui
  rend l'arbre initial.
- **Arrêt après l'étiquette, rien de poussé** : `git tag -d vX.Y.Z` puis
  `git reset --keep HEAD~1`, corriger, relancer.
- **Poussée partielle** (un dépôt l'a reçue, l'autre non) : relancer la commande
  `git push --atomic` affichée pour le dépôt manquant.
- **`verify` échoue** (signature, version, `CHANGELOG.md`) : rien n'est publié.
  Supprimer l'étiquette distante (`git push <dépôt> :refs/tags/vX.Y.Z`, si les règles
  le permettent à l'administrateur) et locale, corriger par un commit sur `main`,
  publier une nouvelle étiquette. Ne jamais réutiliser une étiquette déjà vue par
  d'autres : préférer la version de correctif suivante.
- **`ci` ou `build` échoue** : relancer les jobs échoués (« Re-run failed jobs ») si
  l'échec est passager ; sinon corriger et publier la version suivante.
- **`publish` échoue après l'image** : relancer le job ; l'image est repoussée à la même
  étiquette, la release est créée, ou complétée si elle existe déjà (artefacts
  remplacés, notes inchangées).
- **Release publiée fautive** : ne pas la modifier ; publier `X.Y.Z+1` et marquer la
  précédente dans ses notes.

## Plus tard

Signature et attestation de provenance des artefacts et de l'image : la place est
marquée dans le job `publish` ; décision à prendre (spec 015, E13).

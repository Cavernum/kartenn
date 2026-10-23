---
status: accepted
date: 2026-10-23
---

# 0033 — Publication des versions : git-cliff, étiquettes signées, GitHub Actions

## Context

La spec 015 a fixé le versionnement (E12 : SemVer, étiquette annotée `vX.Y.Z`,
`CHANGELOG.md` tenu à la main, générateur rejeté), laissé ouverte la publication de
l'image (E13) et proposé un script de CI neutre sans forge imposée (E14,
`scripts/ci.sh`). Rien ne publiait encore une version.

Demande de l'utilisateur (2026-10-23) : générer `CHANGELOG.md` et les messages
d'étiquette avec git-cliff ; des étiquettes signées dont le message, en markdown, s'affiche
correctement sur GitHub ; une release GitHub liée à chaque étiquette ; des scripts devenv
pour tout cela et une CI GitHub Actions complète ; un processus stable et sûr. La
signature des artefacts de release viendra plus tard.

Contraintes : le mainteneur signe localement par sa clé OpenPGP (`tag.gpgSign`) ; aucune
clé privée ne doit atteindre la CI ; deux dépôts distants (`origin`, `upstream`) ; les
workflows ne doivent nommer aucun dépôt ; l'esprit d'E14 (logique hors de la forge)
reste.

## Decision

1. **La logique dans des scripts neutres, la forge les appelle.** `scripts/release.sh`
   (sous-commandes `notes`, `prepare`, `verify`, `tag-notes`, `artefacts`, `push-image`)
   et `scripts/ci-lint.sh`, exposés par devenv : `k-release-notes`, `k-release`,
   `k-release-verify`, `k-ci-lint`. Les workflows de `.github/workflows/` ne font
   qu'installer Nix et appeler ces scripts, `scripts/ci.sh` et le flake. Une autre
   forge réutilise les scripts tels quels.

2. **Notes rendues par git-cliff, faits marquants écrits à la main.** `cliff.toml` rend,
   depuis les Conventional Commits, une section par version : titres en français
   (Ruptures de compatibilité, Ajouté, Modifié, Corrigé, Performances, Sécurité,
   Annulé, Documentation, Construction et intégration continue), portée en gras, liens
   vers le commit, la pull request `(#N)` et la comparaison avec la version précédente.
   Sont écartés : les commits `merge:`, `chore(release)`, `chore`, `style`, `test`, et
   la rédaction des specs (`docs(spec)`, `docs(specs)`, `docs(claude)`,
   `docs(changelog)`). Le texte rédigé sous `## [Non publié]` (et une section
   `## [X.Y.Z] — à publier` préparée d'avance, cas de la 0.1.0) devient l'introduction
   de la section ; la liste des commits se replie alors dans un `<details>`, que GitHub
   affiche. L'URL des liens vient de `KARTENN_REPO_URL`, par défaut du dépôt distant
   `upstream`, sinon `origin` : `cliff.toml` ne nomme aucun dépôt.

3. **`CHANGELOG.md` complété, jamais régénéré.** `k-release` remplace `[Non publié]`
   (et la section « à publier ») par une section `[Non publié]` vide suivie de la section
   rendue ; les sections publiées ne changent plus. Rejeté : régénérer tout le fichier à
   chaque version (le texte rédigé de la 0.1.0, meilleur qu'une liste de commits, serait
   perdu ou à maintenir dans un fichier de faits marquants par version) ; `git cliff
   --prepend` (il n'enlève pas la section `[Non publié]` consommée).

4. **Une seule source pour l'étiquette et la release.** Le message de l'étiquette est le
   sujet `vX.Y.Z`, une ligne vide, puis le corps de la section de `CHANGELOG.md` (sans
   son titre). L'étiquette est créée par `git tag --sign --cleanup=verbatim --file` :
   le nettoyage par défaut supprimerait les lignes commençant par `#`, donc les titres
   markdown. La signature OpenPGP est rangée à part dans l'objet étiquette :
   `%(contents:body)` (et `scripts/release.sh tag-notes`) rend le markdown seul. La
   release GitHub prend ce corps pour notes (`gh release create --verify-tag
   --notes-file`), plus une ligne qui donne la référence et l'empreinte de l'image.
   `--notes-from-tag` n'est pas employé : il reprendrait le sujet `vX.Y.Z` en tête des
   notes et dépend de la version de `gh` pour écarter la signature.

5. **`k-release X.Y.Z` (mainteneur, en local).** Refuse sauf : branche `main`, arbre
   propre, `main` égal à `main` de chaque dépôt distant après `git fetch`, étiquette
   absente partout, version supérieure à la dernière étiquette, et dernière exécution
   du workflow CI terminée en succès sur ce commit exact (`gh run list --commit`).
   `--skip-ci-check` lève ce dernier contrôle et l'annonce en gros. Puis : version
   portée dans le workspace, les dépendances internes, les paquets web et `Cargo.lock`
   (le paquet Nix la lit de ces fichiers) ; `CHANGELOG.md` ; commit
   `chore(release): vX.Y.Z` ordinaire (crochets exécutés, jamais `--no-verify`) ;
   étiquette signée ; `git tag --verify` puis la même vérification que la CI, contre les
   clés de `.github/release-keys`. La poussée (`git push --atomic <dépôt> main vX.Y.Z`,
   pour chaque dépôt) n'a lieu qu'après avoir tapé le nom de l'étiquette ; sinon les
   commandes sont affichées. Un arrêt laisse tout en local et affiche comment défaire
   (`git tag -d`, `git reset --keep HEAD~1`) ; le script ne détruit rien lui-même.

6. **Clés de publication versionnées.** Les clés publiques autorisées à signer une
   étiquette de version sont dans `.github/release-keys/<empreinte>.asc`. La vérification
   (`k-release-verify`, job `verify`) importe ces seules clés dans un trousseau jetable et
   exige `GOODSIG` et une empreinte de clé primaire présente dans ce répertoire. Ajouter
   ou retirer une clé est un commit sur `main`, donc revu.

7. **CI GitHub Actions** (`ci.yml`, sur `push` de `main`, `pull_request`, et appelée par
   la release) : `lint` (`scripts/ci-lint.sh`), `devenv` (`devenv up -d postgres`, puis
   `scripts/ci.sh --no-nix` : hakari, métadonnées sqlx, `k-check`, `k-web-check`,
   `k-web-e2e`), `nix` (`nix flake check`, test NixOS sous KVM compris, puis
   `nix build .#kartenn .#oci`).

8. **Release GitHub Actions** (`release.yml`, sur une étiquette `v*.*.*`) : `verify`
   (étiquette annotée, signée par une clé autorisée, sujet `vX.Y.Z`, commit sur `main`,
   versions et section de `CHANGELOG.md` égale au message), `ci` (le workflow CI entier
   sur ce commit, sans cache), `build` (sans privilège : archive de l'image, archive des
   sources par `git archive`, `SHA256SUMS`), puis `publish`, derrière l'environnement
   GitHub `release` à relecteurs obligatoires : l'image vers
   `ghcr.io/<dépôt en minuscules>:X.Y.Z`, puis la release GitHub liée à l'étiquette avec
   les artefacts. Le job qui détient les droits d'écriture n'exécute ni Nix ni code
   construit : `skopeo` et `gh` de l'image du runner.

9. **Durcissement des workflows.** Actions tierces épinglées par commit complet avec la
   version en commentaire ; `permissions: {}` au niveau du workflow et permissions
   minimales par job ; `persist-credentials: false` ; ni `pull_request_target` ni
   `workflow_run` ; aucune expression `${{ }}` dans un `run:`, les valeurs passent par
   l'environnement ; groupes de concurrence (une pull request annule sa propre exécution
   précédente, `main` jamais) ; délais par job ; aucun jeton GitHub dans la
   configuration de Nix des jobs qui exécutent le code d'une pull request ; aucun cache
   dans la release. `actionlint`, `zizmor` (profil courant, sans constat ; profil
   pédant également propre) et `shellcheck` passent en crochet pre-commit, en
   `k-ci-lint` et dans le job `lint`. Dependabot met à jour les actions chaque semaine,
   avec un délai de 7 jours après leur publication.

10. **Cache Nix : les caches binaires publics seulement.** cache.nixos.org et
    devenv.cachix.org (nixpkgs de devenv), en lecture, vérifiés par leur clé publique ;
    plus un cache `sccache` (actions/cache) pour les compilations de `k-check`, clé sur
    `Cargo.lock` et `devenv.lock`, jamais restauré pour une étiquette. Rejetés : Cachix
    en écriture (compte et secret à gérer), magic-nix-cache (dépend d'un service tiers
    dont les conditions ont déjà changé), cache-nix-action (gain faible : les dérivations du projet
    changent à chaque commit, et un cache du store restauré sans signature dans la
    release serait une voie d'empoisonnement). Réversible.

11. **Registre de l'image : GHCR par défaut** (E13). `ghcr.io/${{ github.repository }}`
    en minuscules, étiquette `X.Y.Z`, empreinte publiée dans les notes. Choix par
    défaut, réversible : `scripts/release.sh push-image` accepte toute destination
    `docker://`.

## Plus tard

- **Signature et provenance des artefacts et de l'image** : la place est marquée dans le
  job `publish`, avant la création de la release (attestation de provenance par
  `id-token: write` et `attestations: write`, ou signature `cosign` de l'empreinte).
  À décider : Sigstore sans clé ou clé de projet.
- Étiquette `latest` de l'image, images multi-architectures (`aarch64-linux`).
- Versions préliminaires (`vX.Y.Z-rc.N`) : le motif d'étiquette ne les accepte pas.

## Alternatives

- **E12 tel quel** (journal à la main, pas de générateur) : écarté par la demande de
  l'utilisateur ; la rédaction à la main reste pour les faits marquants.
- **Étiquettes créées et signées par la CI** (clé en secret, ou signature Sigstore de
  l'étiquette) : écarté, la clé du mainteneur ne quitte pas sa machine et l'étiquette
  atteste une décision humaine.
- **Logique dans les workflows** : écarté (E14) ; les workflows deviendraient la seule
  définition du processus et ne se testeraient pas en local.
- **CI de la release par attente du workflow de `main`** : plus économe, mais une course
  (exécution annulée, relancée) déciderait de la publication. Le workflow CI est appelé
  sur le commit étiqueté, au prix d'une seconde exécution.

## Consequences

- Amende la spec 015 : E12 (générateur), E13 (GHCR par défaut), E14 (workflows GitHub
  qui appellent les scripts neutres).
- Réglages GitHub à faire une fois, hors du dépôt : environnement `release`, règles de
  protection de `main` et des étiquettes `v*`, visibilité du paquet GHCR, permissions
  d'Actions ; voir `docs/developpement/publication.md`.
- Une release coûte une seconde exécution de la CI (plus d'une heure sur un runner
  standard) ; les runners hébergés sont gratuits pour un dépôt public.
- Rotation d'une clé de publication : ajouter la nouvelle clé publique, retirer
  l'ancienne, par commit sur `main` ; les étiquettes déjà publiées ne sont pas
  revérifiées.

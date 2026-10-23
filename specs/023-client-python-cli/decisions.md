# 023 — Bibliothèque cliente Python et CLI : journal des décisions

Brouillon du 2026-10-23, rédigé sans entretien, puis **arbitré par l'utilisateur le
même jour** (A1 à A8, choix des dépendances et des tests). Les questions Q-023-1 à 14
suivent la recommandation, sauf là où un arbitrage les remplace. La spec est **validée**
et son implémentation commence tout de suite (A6 : D31 amendée).

Demande d'origine (2026-10-23) : « Tu peux aussi commencer le design de la CLI, je veux
aussi un genre de maquette et la possibilité de réaliser la plupart des appels de l'API
depuis la CLI. Enfin, et je pense que tu peux l'utiliser pour créer la CLI, une
librairie/bindings python pour interagir avec Kartenn. N'oublie pas de supporter les
plugins type politique de sécurité etc automatiquement selon ce qui est disponible sur
le serveur. »

Cadre : D7, D10, D13, D16, D31, D32, D33, D37, Q-ouv-1, Q-ouv-6
([`../001-cadrage/decisions.md`](../001-cadrage/decisions.md)). 004 (changesets).
005/K1 et K8 (Bearer des clients non navigateur, secrets des comptes de service). 006/H8,
H12 et ADR 0022 (contrat HTTP). ADR 0026, clause 9 (conventions REST des modules). 017/U1.
022 (événements SSE, validée le 2026-10-23 : `GET /v1/events`, § « Contrat consommé par
les clients »). Retour de l'utilisateur sur les maquettes du 2026-10-22 (filtres typés,
clé et libellé, codes visibles).

Surface REST de référence : `specs/006-api-rest/openapi-v1.json` au 2026-10-23. Elle
compte 279 opérations, dont 25 collections à `QUERY` (extension `x-query`), et les tags
`core`, `properties`, `changesets`, `imports`, `rights`, `audit`, `authentication`,
`operations`, `graph`, `policy`, `flow`, `vlan` et `machines`.

## Synthèse
- T1 deux livrables, la bibliothèque `kartenn` et la CLI `kartenn-cli` (commande
  `kartenn`), implémentés maintenant (D31 amendée).
- T2 bibliothèque en Python pur : un cœur écrit à la main et la découverte à
  l'exécution ; pas de bindings (D7 amendée).
- T3 modules découverts depuis les capacités et le contrat ; extension `x-kartenn`
  facultative.
- T4 accès dynamique, stubs de types générés à la demande.
- T5 HTTP : **httpx2** derrière un port de transport (envoi, flux, SSE) ; adaptateur
  HTTP/3 futur et facultatif.
- T6 Python ≥ 3.11.
- T7 deux paquets distincts (pyproject, version, tests, publication) dans
  `clients/python/`.
- T8 licences : bibliothèque MPL-2.0, CLI AGPL-3.0.
- T9 compatibilité par `api_versions` et présence des opérations.
- T10 humains : OIDC natif (bouclage et PKCE, `--device`).
- T11 machines : Basic ; secours par session cookie.
- T12 CLI : **click 8.5 et rich-click** (aide seulement), groupe paresseux construit
  depuis le catalogue en cache.
- T13 arbre dérivé du contrat, un groupe par module, préfixe retiré ; commandes nommées ;
  `kartenn api`.
- T14 sorties, dates, filtres, complétion.
- T15 clé et libellé.
- T16 problèmes et codes de sortie.
- T17 écritures par changeset, `--dry-run`, édition sous `If-Match`.
- T18 imports NDJSON, et CSV ou JSON par ressource.
- T19 profils XDG, secrets au trousseau (CLI), aucun secret en argument.
- T20 cache du catalogue par révision (ETag du contrat).
- T21 i18n FR et EN.
- T22 `watch` sur `GET /v1/events` (022).
- T23 tests : pytest 9.1, outillage complet, contre un vrai serveur en bout de chaîne.
- T24 budgets de démarrage et de complétion.
- T25 intégration devenv (`k-py-check` en sh POSIX) et tâche de CI.

## T1 — Périmètre et calendrier
- Décision : deux paquets clients de l'API HTTP `/v1` (D7) : la bibliothèque `kartenn`
  et la CLI `kartenn-cli`, qui installe la commande `kartenn`. La CLI est le premier
  client de la bibliothèque et l'éprouve.
- Calendrier : implémentation **maintenant**, en parallèle de la v0.2, et non
  « ensuite » comme le disait D31 (amendée le 2026-10-23).
- Décidé par : l'utilisateur (A6)

## T2 — Bibliothèque, pas de bindings
- Constat : une **bibliothèque** est un paquet Python qui parle HTTP au serveur. Des
  **bindings** enveloppent une bibliothèque native (ici une crate Rust, exposée par PyO3
  et maturin). Dans les deux cas, le serveur reste atteint par HTTP (D7).
- Options étudiées :
  - (a) client généré au build depuis l'OpenAPI : figé à une build du serveur ; ignore
    `QUERY`, les curseurs, les changesets et les imports ; générateur en 0.x qui
    épingle httpx `<0.29` ;
  - (b) cœur écrit à la main (HTTP, authentification, reprise, pagination, problèmes,
    changesets, imports, dates, événements) et découverte des ressources à l'exécution ;
  - (c) bindings PyO3 : matrice de wheels (manylinux, musllinux, macOS, abi3 et abi3t),
    aucun gain pour un client limité par le réseau, et la même découverte à écrire.
- Décision : **(b)**, la seule option qui satisfait « automatiquement selon ce qui est
  disponible sur le serveur » sans nouvelle version du client.
- D7 amendée : « SDK client **dérivé** de l'OpenAPI » (à l'exécution) au lieu de
  « généré ».
- D16 rejette Python **pour le serveur** seulement ; un client n'est pas concerné.
- Décidé par : l'utilisateur (A1)

## T3 — Découverte des modules
- Décision : au premier contact avec un serveur, la bibliothèque lit
  `GET /v1/capabilities` (version, `api_versions`, `modules`, propriétés déclarées,
  limites, `events`) et `GET /v1/openapi.json`. Elle en dérive un **catalogue** compact :
  - ressources (premier segment après `/v1`) ;
  - module de chaque ressource (tag de l'opération) ;
  - opérations (méthode et forme du chemin), paramètres, énumérations et schémas de
    corps ;
  - collections à `QUERY` (`x-query`).
- La dérivation suit les conventions que l'ADR 0026 (clause 9) impose à tout module :
  - collections au pluriel en kebab-case, objet désigné par UUID ;
  - `GET` et `POST` sur la collection ; `GET`, `PATCH` et `DELETE` sur l'objet ;
  - `history` et `properties` fournis par le noyau ;
  - actions en `POST` de sous-ressource.
- Extension `x-kartenn` (ajout au serveur, T3/A5) : clé naturelle, nom de commande,
  colonnes par défaut. Le kit REST (`kartenn_rest::kit`) la produit, donc tout module la
  reçoit. Sans elle, la CLI fonctionne par convention.
- Rejeté :
  - une liste de modules connue du client ;
  - un endpoint de description propre à la CLI, qui serait une seconde surface à tenir.
- Décidé par : l'utilisateur (A1, A5)

## T4 — Typage
- Décision : les objets rendus sont des `Record`, des `dict` qui gardent l'ETag, les liens
  et l'`as_of` effectif. `kartenn stubs generate` (bibliothèque, sans dépendance de la
  CLI) écrit pour un serveur donné un répertoire de types dans `typings/` (`.pyi`,
  `TypedDict`, `Literal`, `Protocol`).
- Rejeté :
  - pydantic à l'exécution : import lent, et validation en double de celle du serveur ;
  - modèles générés au build.
- Décidé par : recommandation acceptée (Q-023-6)

## T5 — Transport HTTP
- Décision : **httpx2**, avec les caractéristiques suivantes :
  - version ≥ 2.16, BSD-3 ;
  - fork maintenu de httpx par l'équipe de Pydantic ;
  - synchrone et asynchrone par la même API ;
  - HTTP/2 en option ;
  - SSE natif (`client.sse()`) ;
  - magasin de confiance du système par truststore.
- Il est placé derrière un **port de transport** interne à trois opérations : `send`
  (requête et réponse), `stream` (corps lu en continu : imports, `ndjson`) et `sse`
  (événements de la 022).
- Constat (2026-10-23) :
  - httpx 0.28 est figé (aucune version stable depuis décembre 2024) ;
  - niquests est rejeté : son socle urllib3-future remplace le module `urllib3` de tout
    l'environnement de l'utilisateur par un fichier `.pth` ;
  - respx ne gère pas encore httpx2 : les tests utilisent `httpx2.MockTransport`.
- **Autorités de certification** : truststore change le comportement par défaut (magasin
  du système au lieu de certifi). Une CA propre au profil (`ca_bundle`) passe donc par un
  `ssl.SSLContext` explicite, construit par la bibliothèque, avec un test dédié : CA
  du profil acceptée, CA inconnue refusée, et aucun repli silencieux vers le magasin du
  système quand `ca_bundle` est donné.
- **HTTP/3** : le port admet un futur adaptateur facultatif `kartenn[http3]` sur
  curl_cffi (libcurl et ngtcp2). Il ne sera écrit que quand le serveur ou son
  mandataire inverse parlera HTTP/3 **et** qu'un banc montrera un gain, avec un ADR à ce
  moment.
- Décidé par : l'utilisateur

## T6 — Version de Python
- Décision : **3.11** au minimum. Python 3.10 est en fin de vie depuis octobre 2026 ; 3.11
  est maintenu jusqu'en octobre 2027 et fournit `tomllib`. Le plancher passera à 3.12
  dans la première version publiée après octobre 2027.
- Source : https://devguide.python.org/versions/ (lu le 2026-10-23).
- Décidé par : recommandation acceptée (Q-023-4)

## T7 — Deux paquets distincts
- Décision : la bibliothèque et la CLI sont **deux paquets distincts** : un pyproject, un
  numéro de version, des tests et une publication PyPI chacun. Les publications sont
  faites par l'utilisateur.
  - `clients/python/kartenn/` : distribution `kartenn`, import `kartenn` ; dépendances
    d'exécution : `httpx2` et `platformdirs` (cache du catalogue). Rien de la CLI.
  - `clients/python/kartenn-cli/` : distribution `kartenn-cli`, import `kartenn_cli`,
    commande `kartenn`. Elle dépend de `kartenn` par une plage de versions
    (`kartenn>=X.Y,<X+1`), de `click`, `rich-click`, `rich`, `keyring`, `PyYAML` et
    `jmespath`.
  - Un espace de travail uv (`clients/python/pyproject.toml`) sert au développement
    seulement : un verrou commun, et la CLI résolue sur la bibliothèque locale
    (`tool.uv.sources`).
- Construction : `uv_build` épinglé à une version mineure ; wheels `py3-none-any`. Nix :
  deux paquets du flake.
- Un test vérifie que l'installation seule de `kartenn` n'importe ni click, ni rich, ni
  keyring, ni yaml, ni jmespath.
- Emplacement : `clients/` à côté de `crates/` et `web/` (D32 complétée).
- Décidé par : l'utilisateur (A7, et « deux paquets » explicitement)

## T8 — Licences
- Décision : bibliothèque **MPL-2.0** (Q-ouv-1 : les clients générés ou dérivés sont en
  MPL-2.0, pour que des scripts et connecteurs propriétaires puissent l'importer) ; CLI
  **AGPL-3.0**, comme le serveur (D37).
- Compatibilité : la MPL-2.0 permet l'usage depuis une œuvre sous GPL ou AGPL (licences
  secondaires, MPL-2.0 §3.3). Un fichier MPL reste MPL.
- Décidé par : l'utilisateur (A3)

## T9 — Versions et compatibilité
- Décision : chaque paquet a ses versions sémantiques, indépendantes de celles du
  serveur. À la connexion, `v1` doit figurer dans `api_versions` (sinon
  `client.api-unsupported`). Chaque fonction du socle s'active selon la présence de son
  opération dans le contrat, jamais selon un numéro de version. Ce qui manque rend
  `client.operation-unavailable`. Les en-têtes `Deprecation` et `Sunset` (006/H12) sont
  signalés une fois par jour et par opération.
- Décidé par : recommandation acceptée

## T10 — Authentification des humains
- Décision : **OIDC natif**. La CLI est un client public de l'émetteur.
  - Bouclage : redirection `http://127.0.0.1:<port>/callback`, port choisi à l'exécution
    (RFC 8252 §7.3 ; §8.3 : `localhost` « NOT RECOMMENDED »).
  - PKCE `S256` (RFC 8252 §6, RFC 7636 §4.2, RFC 9700 §2.1.1).
  - Navigateur externe (RFC 8252 §8.12).
  - `--device` : flux d'autorisation d'appareil (RFC 8628 §3.1 à 3.5), avec
    l'avertissement de §5.4.
  - Jetons de rafraîchissement **tournants** exigés de l'émetteur (RFC 9700 §2.2.2).
- kartenn n'émet aucun jeton (005/K1). L'émetteur et son client public sont publiés par
  `GET /v1/auth/native-clients` (T3/A5, ajout au serveur).
- Rejeté :
  - jeton personnel émis par le web : kartenn deviendrait émetteur de jetons ;
  - flux d'appareil seul.
- Note `docs/reference/` (RFC 8252, 8628, 9700, 7636) à rédiger. Citations à recouper mot
  pour mot ; RFC 8628 §5.2 et §5.3 et RFC 9700 §4.14 à confirmer.
- Décidé par : l'utilisateur (A4)

## T11 — Machines et secours
- Décision :
  - compte de service en Basic (RFC 7617, 005/K8), secret lu par le trousseau,
    `KARTENN_SECRET_FILE` ou l'entrée standard, jamais en argument ;
  - secours (`kartenn login --breakglass`) : phrase de passe et TOTP saisis sans écho ;
  - session cookie gardée en mémoire, ou avec `--keep` dans `$XDG_RUNTIME_DIR` (0600) ;
  - `Origin` sur chaque écriture (CSRF, 006).
- Décidé par : recommandation acceptée (Q-023-12)

## T12 — Cadre de la CLI
- Décision : **click 8.5** et **rich-click**.
  - rich-click n'est importé que pour rendre `--help`.
  - Un `click.Group` paresseux construit les commandes depuis le catalogue en cache, dont
    la révision repose sur l'ETag du contrat : `--help` ne demande pas de réseau.
  - `shell_complete` interroge le serveur avec un délai court (une seconde).
- Rejeté :
  - typer : il embarque sa copie de click depuis 0.26, sans interopérabilité avec un
    `click.Group` ; un arbre construit à l'exécution y est malcommode ;
  - cyclopts : même modèle par annotations, démarrage plus lent.
- Décidé par : l'utilisateur

## T13 — Arbre de commandes
- Décision :
  - premier niveau : les ressources du noyau et du socle ;
  - un **groupe par module** servi (`policy`, `flow`, `vlan`, `machines`). Ses
    collections perdent le préfixe du module (`policy-rules` → `policy rules`). La
    collection qui porte le nom du module rattache ses verbes au groupe
    (`/v1/flows` → `kartenn flow list`) ;
  - verbes dérivés : `list`, `get`, `create`, `update`, `close`, `edit`, `history`,
    `properties` ;
  - commandes nommées, par une table de la CLI liée à des `operationId` ;
  - `kartenn api` pour tout le reste.
  `close` et non `delete` (D33).
- Rejeté : un niveau plat calqué sur l'API (40 collections au premier niveau).
- Décidé par : l'utilisateur (A8)

## T14 — Lectures : dates, filtres, sorties, complétion
- Dates : `--at` (`valid_at`) et `--as-of` acceptent RFC 3339, une date seule (minuit
  dans le fuseau du profil) ou une durée relative. L'`as_of` effectif est affiché en
  pied ; `--freeze` et `KARTENN_AS_OF` le figent.
- Filtres : options typées tirées du contrat et `--prop clé[:op]=valeur` ; bascule
  automatique sur `QUERY` au-delà de 8 Kio.
- Sorties :
  - formats `table` (rich, paresseux), `json`, `ndjson` (en flux), `yaml`
    (`safe_dump`) et `csv` ;
  - `--fields` choisit les colonnes ;
  - `--query` projette en JMESPath (Q-023-10).
- Complétion :
  - sources locales : catalogue et capacités ;
  - valeurs à liste fermée et slugs lus au serveur, gardés 5 min en cache, sans valeur
    masquée ;
  - délai d'une seconde au plus.
- Décidé par : recommandation acceptée

## T15 — Clé et libellé
- Décision : en table, la CLI montre le libellé que l'API rend, et sinon la clé ;
  `--keys` force les clés. Les formats de données rendent toujours les clés. Les codes ne
  sont jamais traduits ni masqués.
- Les libellés par langue servis par l'API sont à traiter avec la réécriture du web
  (Q-023-3).
- Décidé par : recommandation acceptée

## T16 — Problèmes et codes de sortie
- Décision : un problème est rendu dans l'ordre code, titre, détail, identifiant de
  requête, violations. Codes de sortie (design, § Codes de sortie) :

  | Code | Sens |
  |---|---|
  | 0 | succès ; écriture en attente sans `--require-applied` (Q-023-8) |
  | 1 | erreur inattendue |
  | 2 | usage, module ou opération absents |
  | 3 | introuvable |
  | 4 | authentification ou droit |
  | 5 | conflit ou version |
  | 6 | refus de validation |
  | 7 | indisponible après reprises, réseau ou TLS |
  | 8 | simulation qui échouerait |
  | 9 | verdict refusé avec `--exit-status` |
  | 10 | en attente avec `--require-applied` |

- Décidé par : recommandation acceptée

## T17 — Écritures
- Décision :
  - une commande d'écriture produit un changeset ;
  - options communes : `--motive`, `--valid-from` et `--on-behalf-of` ;
  - `--dry-run` = `?simulate=true` ;
  - `edit` passe par l'éditeur sous `If-Match` ;
  - `--yes` est requis hors terminal pour une cascade ;
  - `Idempotency-Key` est générée automatiquement ;
  - `changeset new|add|simulate|apply` compose dans un fichier local.
- Décidé par : recommandation acceptée

## T18 — Imports
- Décision : NDJSON d'`ImportElement` tel quel ; CSV ou JSON d'objets d'une ressource
  traduits en opérations de création par le schéma du contrat. Comportement :
  - lecture en flux ;
  - morceaux d'au plus `chunk_elements` éléments ;
  - reprise vérifiée par l'empreinte du fichier ;
  - violations rapportées à la ligne source.
- Décidé par : recommandation acceptée

## T19 — Profils, secrets, environnement
- Décision :
  - `$XDG_CONFIG_HOME/kartenn/config.toml` (profils) ;
  - `$XDG_CACHE_HOME/kartenn/` (catalogues, complétion) ;
  - `$XDG_STATE_HOME/kartenn/` (imports à reprendre) ;
  - jetons et secrets au trousseau (keyring, dans la CLI). La bibliothèque définit un port
    `TokenStore`, en mémoire par défaut, que la CLI implémente par keyring.
  Sans trousseau utilisable, rien n'est stocké : la CLI propose `KARTENN_TOKEN`, ou
  `--token-file` explicite (Q-023-7).
- Décidé par : recommandation acceptée

## T20 — Cache du catalogue
- Décision : la révision du catalogue dépend de l'URL, de la `version`, des versions des
  modules et de l'ETag de `/v1/openapi.json` (ajout au serveur, A5). Le cache garde le
  catalogue dérivé en JSON compact, en mode 0600, écrit de façon atomique. Les capacités
  sont relues au plus une fois par minute et par profil.
- Décidé par : l'utilisateur (A5)

## T21 — Langue
- Décision : catalogues de messages TOML, en français et en anglais, avec la même
  résolution de langue que le web (Q-ouv-6). `title` et `detail` des problèmes viennent du
  serveur.
- Décidé par : recommandation acceptée

## T22 — Suivi d'événements
- Décision : `kartenn watch` et les méthodes `events()` et `aevents()` de la bibliothèque
  lisent `GET /v1/events` (022) en flux, par le port `sse` (httpx2 `client.sse()`).
  - Elles envoient l'en-tête `Authorization` (Basic ou Bearer) et posent elles-mêmes
    `Last-Event-ID`.
  - Avant ouverture, elles lisent le statut et `Retry-After`, puis reprennent avec une
    attente croissante, plafonnée et aléatoire.
  - Elles ignorent les identités déjà vues : des doublons sont possibles, rien n'est
    perdu.
  - Sur `reset`, elles signalent l'événement et continuent sur le même flux. Le curseur
    (`id`) est chiffré et opaque : le client le stocke sans l'interpréter.
  La commande n'existe que si `capabilities.events` n'est pas `null`. `--follow` d'un
  import ou d'un changeset en attente utilise le flux s'il existe, sinon il interroge le
  serveur.
- Décidé par : l'utilisateur (022 validée)

## T23 — Tests
- Décision : **pytest ≥ 9.1**, configuré par la table TOML native `[tool.pytest]` en mode
  strict, dans chacun des deux paquets. Plugins et réglages :
  - **pytest-xdist** (`-n auto`) ;
  - **pytest-cov 7** et **coverage 7.17** : `branch = true`, `parallel = true`,
    `patch = ["subprocess"]` (pour les tests de la CLI en sous-processus), `fail_under` ;
  - plugin **anyio** pour l'asynchrone (pas pytest-asyncio) ;
  - **pytest-randomly** (ordre aléatoire, graine affichée) ;
  - **pytest-timeout**, méthode `signal` ;
  - **hypothesis** (dates, filtres, découpage en morceaux, dérivation du catalogue) ;
  - **syrupy** pour les instantanés des sorties de la CLI, mis à jour sans `-n`
    (`--snapshot-update` en série) ;
  - `httpx2.MockTransport` pour simuler le serveur.
- Substitution à la demande « clarity » : pytest-clarity est abandonné (dernière version
  en 2021). Ses diffs lisibles sont fournis par les diffs natifs de pytest 9.1
  (`assertion_text_diff_style`) et, en option, par pytest-icdiff.
- pytest-sugar : greffon facultatif pour le confort local, désactivé en CI
  (`-p no:sugar`). Sa compatibilité avec xdist est à vérifier à la tâche 2 ; à défaut, il
  reste hors des dépendances de développement.
- Bout en bout contre un vrai `kartenn-server` (base `kartenn_cli`, comme
  `web/e2e/serve.sh`, sous `k-heavy --pg`), avec `kartenn-dev-idp` étendu.
- Décidé par : l'utilisateur

## T24 — Budgets
- Décision : budgets de REQ-023-39, mesurés par le banc `bench-cli`. Un test vérifie que
  `kartenn --version` n'importe ni httpx2 ni rich.
- Décidé par : recommandation acceptée

## T25 — Intégration au dépôt
- Décision :
  - Python et uv dans `devenv.nix` ;
  - script `k-py-check`, **en sh POSIX** (les scripts du projet tournent sous dash), qui
    lance ruff (lint et format), pyright, puis `pytest -n auto` sur les deux paquets, avec
    un verdict sur une ligne comme `k-check` ;
  - `k-py-e2e` pour le bout en bout ;
  - un job de CI dans `scripts/ci.sh`.
- Décidé par : l'utilisateur

## Arbitrages rendus (2026-10-23, utilisateur)

| # | Sujet | Décision |
|---|---|---|
| A1 | Approche de la bibliothèque | cœur écrit à la main et découverte à l'exécution ; D7 amendée (« dérivé de l'OpenAPI ») |
| A2 | Dépendances | bibliothèque : httpx2, platformdirs. CLI : click 8.5, rich-click, rich, keyring, PyYAML, jmespath. Outillage : uv, ruff, pyright, pytest 9.1 et ses greffons (T23) |
| A3 | Licences | bibliothèque MPL-2.0, CLI AGPL-3.0 |
| A4 | Humains | OIDC natif, bouclage et `--device` |
| A5 | Ajouts au serveur | ETag de `/v1/openapi.json`, extension `x-kartenn` par le kit REST, `GET /v1/auth/native-clients` : tâches de la 023 au sens de la 006 (amendements additifs) |
| A6 | Calendrier | implémentation maintenant ; D31 amendée |
| A7 | Emplacement | `clients/python/`, deux paquets distincts ; D32 complétée |
| A8 | Arbre | un groupe par module, préfixe retiré |

## Questions arbitrées

Toutes suivent la recommandation du brouillon, sauf mention.

- [x] Q-023-1 — Nom de la commande : `kartenn` seul. `kn` et `kt` sont déjà pris par
  d'autres outils répandus.
- [x] Q-023-2 — PyPI : `kartenn` et `kartenn-cli`, publiés par l'utilisateur lui-même,
  chacun à son rythme (T7).
- [x] Q-023-3 — Libellés par langue servis par l'API : avec la réécriture du web ; la
  CLI montre les clés d'ici là.
- [x] Q-023-4 — Plancher Python 3.11, puis 3.12 après octobre 2027.
- [x] Q-023-5 — Client asynchrone dès la première version.
- [x] Q-023-6 — Stubs dans un répertoire `typings/` du projet.
- [x] Q-023-7 — Sans trousseau : rien n'est stocké ; `--token-file` explicite.
- [x] Q-023-8 — Écriture en attente : code 0 ; code 10 avec `--require-applied`.
- [x] Q-023-9 — Références par clé naturelle déclarée par `x-kartenn` ; UUID sinon.
- [x] Q-023-10 — Projection en JMESPath (`--query`).
- [x] Q-023-11 — Télémétrie : `traceparent` seul.
- [x] Q-023-12 — Secours depuis la CLI : oui, session en mémoire par défaut.
- [x] Q-023-13 — La bibliothèque sera la base du futur MCP (à confirmer dans sa spec).
- [x] Q-023-14 — Image de conteneur de la CLI, dérivée du paquet Nix, après la première
  version publiée.

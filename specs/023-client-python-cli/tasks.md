---
status: approved
updated: 2026-10-23
---

# 023 — Bibliothèque cliente Python et CLI : tâches

Spec validée le 2026-10-23 ([`decisions.md`](decisions.md)). Règles communes :
- chaque tâche est un commit qui laisse le build vert : `k-py-check` à partir de la
  tâche 2, `k-check` en plus pour les tâches du flux S ;
- chaque tâche porte `où :`, `verify :` et `dépend de :` ;
- les chemins `clients/python/…` sont ceux du design (§ Disposition).

**Découpage.** Après le socle (1 à 3, séquentiel), trois flux avancent en parallèle,
avec au plus trois agents à la fois (CLAUDE.md) :

| Flux | Tâches | Touche | Commence après |
|---|---|---|---|
| **S** — ajouts au serveur | 4 à 7 | `crates/kartenn-rest`, `kartenn-auth`, `kartenn-dev-idp`, specs 005 et 006 | rien (indépendant du socle Python) |
| **B** — bibliothèque | 8 à 15 | `clients/python/kartenn/` | 1, 2 |
| **C** — CLI | 16 à 25 | `clients/python/kartenn-cli/` | 1, 2 ; puis les tâches B nommées |

La fin (26 à 28) réunit les flux. Les flux ne partagent aucun fichier, sauf
`clients/python/uv.lock`, que seules les tâches 1 et 2 modifient. Une dépendance ajoutée
plus tard dans un flux régénère le verrou à son commit, après rebase.

## Socle

- [x] 1. Squelette des deux paquets distincts. Contenu :
  - `clients/python/pyproject.toml` (espace de travail uv de développement, ruff,
    pyright strict) ;
  - `kartenn/pyproject.toml` : MPL-2.0, version 0.1.0.dev0, `requires-python >=3.11`,
    deps `httpx2>=2.16`, `platformdirs` ;
  - `kartenn-cli/pyproject.toml` : AGPL-3.0, version propre, deps `kartenn`, `click>=8.5`,
    `rich-click`, `rich`, `keyring`, `PyYAML`, `jmespath`, script `kartenn` ;
  - `uv_build` épinglé ;
  - groupes de développement de T23 ; `[tool.pytest]` strict et `[tool.coverage]` du
    design ;
  - un test par paquet, dont le test d'indépendance de REQ-023-44.
  Vérifier aussi la compatibilité de pytest-sugar avec xdist et consigner le résultat. —
  T7, T8, T23 ; REQ-023-42, 44, 45 — verify : `uv build` dans chaque paquet produit une
  wheel `py3-none-any` ; `uv run pytest -n auto` vert dans chaque paquet ; métadonnées
  des wheels : licences MPL-2.0 et AGPL-3.0
  - où : design § Disposition, § Stratégie de test (bloc TOML) ; licence MPL de
    `web/sdk/package.json` ; texte AGPL de la racine (`LICENSE`)
  - dépend de : —
  - choix : clés de pytest 9.1.1 confirmées (`strict`, `minversion`, `testpaths`,
    `addopts`, `assertion_text_diff_style = "block"`) ; `timeout = "30"` en chaîne, car
    pytest-timeout déclare une option texte et la table TOML native refuse un entier.
    Licences en expressions SPDX (`MPL-2.0`, `AGPL-3.0-only` comme le serveur), textes
    copiés de `crates/syslog-sender/LICENSE` et de la racine. `uv_build>=0.12.22,<0.13`
    (version de l'uv de devenv). CLI : `kartenn>=0.1.0.dev0,<1` (X = 0) ; squelette
    click minimal (`--version`, `--help`, instantané syrupy). Ruff : `select = ["ALL"]`
    sauf `D`, `CPY`, `COM812`, `ISC001` ; pyright strict, `pythonVersion = "3.11"`, sur
    l'environnement actif (`uv run pyright`). Test d'indépendance : `Requires-Dist`
    d'exécution = httpx2 et platformdirs, puis import de tous les modules de `kartenn`
    dans un interpréteur neuf sans qu'aucun module de la CLI n'entre dans
    `sys.modules` (l'environnement de travail les contient tous). `trio` et `packaging`
    ajoutés aux groupes `dev` (moteur trio d'anyio, lecture des exigences).
    pytest-sugar 1.1.1 fonctionne avec pytest-xdist 3.8.0 (`-n 4 --force-sugar`) : il
    entre, avec pytest-icdiff, dans un groupe facultatif `confort`, hors du groupe `dev`.
- [x] 2. devenv : Python ≥ 3.11 et uv dans `devenv.nix`. Script `k-py-check` **en sh
  POSIX** (dash : pas de `pipefail`, de tableaux ni de `[[`) qui lance, pour chaque paquet,
  `ruff check`, `ruff format --check`, `pyright` et `pytest -n auto -p no:sugar`. Les
  journaux vont dans un répertoire temporaire. Le verdict tient en une ligne
  (`OK — kartenn N tests, kartenn-cli M tests`) ou en `KO <étape> (<journal>)` suivi des
  seules lignes d'échec. La commande passe par `k-heavy`. — T25 ; REQ-023-45 —
  verify : `k-py-check` vert ; un test cassé exprès donne `KO pytest` et sa ligne ; un
  fichier mal formaté donne `KO ruff-format` ; `dash -n` sur le script extrait
  - où : `devenv.nix`, `scripts.k-web-check` (l. ~456, forme du verdict) et `scripts.k-check`
    (k-heavy) ; `languages.python` et `uv` de devenv
  - dépend de : 1
  - choix : Python 3.11 de Nix (plancher des paquets, Q-023-4) ; uv ne télécharge
    aucun interpréteur (`UV_PYTHON`, `UV_PYTHON_DOWNLOADS=never`) ; l'environnement uv
    est celui de devenv (`.devenv/state/venv`). ruff vient de nixpkgs (0.16.10), pyright
    du serveur de langage de devenv (1.1.414) : pas de pyright de PyPI, qui télécharge
    Node. Ruff et pyright tournent une fois sur l'espace de travail (les deux paquets),
    pytest dans chaque paquet. Une étape `uv-sync` (`--locked`) précède : un verrou
    périmé échoue là. `-n` prend les cœurs que `k-heavy` donne au créneau
    (`NEXTEST_TEST_THREADS`), `auto` hors de lui. `k-py-check` refuse les arguments.
    `FORCE_COLOR` reste non défini : ruff le lit comme une demande de couleurs, quelle
    que soit sa valeur. Vérifié : test cassé → `KO pytest` et sa ligne `FAILED` ;
    couverture à 31 % → `KO pytest` et `FAIL Required test coverage … not reached` ;
    fichier mal formaté → `KO ruff-format` ; `dash -n` et `k-ci-lint` propres.
- [x] 3. CI : étape `k-py-check` dans `scripts/ci.sh`, et construction des deux wheels
  (`uv build`) avec vérification des métadonnées de licence — T25 ; REQ-023-45 —
  verify : `scripts/ci.sh` local vert ; l'étape échoue si `k-py-check` échoue
  - où : `scripts/ci.sh` (étapes existantes de `k-check` et `k-web-check`)
  - dépend de : 2
  - choix : deux étapes de `scripts/ci.sh` après `web` : `python` (`k-py-check`) et
    `python-wheels`, script `scripts/py-wheels.sh` (dash, contrôlé par `k-ci-lint`) qui
    construit les deux roues (`uv build --wheel`) et vérifie qu'elles sont
    `py3-none-any`, que `License-Expression` vaut `MPL-2.0` et `AGPL-3.0-only`, et que
    le texte de licence est dans le dist-info. Le workflow lance déjà `scripts/ci.sh` :
    seul le nom du job change (« Rust, web, Python et parcours de bout en bout »).
    Vérifié localement : `k-ci-lint`, `k-py-check`, `scripts/py-wheels.sh` et `k-check`
    verts ; `scripts/ci.sh` complet (bout en bout du web, `nix flake check`) non relancé.

## Flux S — ajouts au serveur (amendements additifs de la 006 et de la 005)

- [x] 4. ETag fort de `GET /v1/openapi.json` (empreinte SHA-256 du document de la build,
  calculée une fois au démarrage), `Cache-Control: no-cache`, 304 sur `If-None-Match`.
  Amendement écrit dans `specs/006-api-rest/` (REQ-006-34 complétée). — T20 ;
  REQ-023-2 — verify : test REST (200 avec ETag, 304, ETag stable entre deux instances
  de même build)
  - où : route `openapi` de `crates/kartenn-rest` (`GET /v1/openapi.json`, 006/design
    § Versions et contrat) ; tests voisins de `crates/kartenn-rest/tests/it/` (cache et
    304 des lectures)
  - dépend de : —
  - choix : `openapi::Served` porte les octets et l'ETag (SHA-256 complet en hexadécimal,
    par `ETag::fingerprint`, comparaison de `If-None-Match` de `dated`). Le document est
    sérialisé par `serde_json::Value` (membres triés) : les extensions d'utoipa sont des
    `HashMap`, dont l'ordre changerait d'un processus à l'autre et romprait l'égalité
    des ETag entre instances. Le 304 est documenté dans le contrat (réponse ajoutée à
    l'opération `openapi`, contrat et `schema.d.ts` régénérés). Tests :
    `contract::the_contract_is_revalidated_by_its_tag`,
    `contract::the_tag_of_the_contract_is_that_of_the_build`. La route reste
    authentifiée comme avant (006) ; seul `native-clients` est public (tâche 6).
- [x] 5. Extension `x-kartenn`, produite par `kartenn_rest::kit` :
  - `key` (clé naturelle) et `columns` sur les collections du noyau, de `vlan`,
    `machines`, `policy` et `flow` ;
  - `command` facultatif sur une opération ;
  - contrat de référence régénéré (`cargo run -p kartenn-server --all-features --
    openapi`) ;
  - le comparateur l'accepte comme un ajout ;
  - amendement de la 006 et note à la clause 9 de l'ADR 0026.
  — T3 ; REQ-023-27 — verify : le contrat régénéré porte `x-kartenn` sur chaque
  collection ; comparateur vert ; test du kit pour un module de test
  - où : `crates/kartenn-rest` (`kit`, construction de l'`OpenApiRouter`, extension
    `x-query` à imiter, 006/design § Versions et contrat) ; `docs/adr/0026-contrat-de-module.md`
    clause 9
  - dépend de : —
  - choix : `kit::presented` et `kit::commanded` (dans `openapi.rs`, réexportés par le
    kit). `key` nomme des **filtres** de la liste (ce que la CLI envoie pour résoudre une
    référence), pas des membres : le kit refuse une clé qui n'en est pas un. Sans clé
    (UUID seul) : `machines` (nom non unique), `flows`, `flow-nonconformities`,
    `anomalies`, `policy-values` (aucun filtre de `value_key` ; à ajouter si la CLI le
    demande). Entités et sites : `slug` seul (unique à une date). Commandes : `check`
    sur `GET /v1/verdicts`, `report` sur `POST /v1/policy-reports`. 25 collections
    décrites (test `contract::every_collection_tells_the_clients_its_key_and_its_columns`),
    test du kit `contract::the_kit_describes_the_collection_of_a_module` (comparateur
    vert dans les deux sens). Les collections du socle (`rights`, `changesets`, `imports`,
    `audit`, `properties`) n'en portent pas : hors du périmètre de la tâche. Instantané
    du catalogue de la bibliothèque régénéré (`check`, `report`). `schema.d.ts`
    inchangé : openapi-typescript ignore les extensions.
- [x] 6. `GET /v1/auth/native-clients`, publique :
  `[{id, discovery_url, client_id, audience, scopes, device}]`, seulement pour les
  émetteurs configurés avec `native_client_id`. Nouveaux champs `native_client_id`,
  `native_scopes` et `native_device` de `[[auth.issuers]]`. Amendements de la 005 et de
  la 006. — T10 ; REQ-023-7 — verify : test REST sans session ; un émetteur sans
  `native_client_id` est absent ; aucun secret dans la réponse ; `issuers` inchangée
  (REQ-007-41)
  - où : route `auth/issuers` de `crates/kartenn-rest` (à imiter) ; configuration des
    émetteurs dans `crates/kartenn-auth` (005/design, bloc `[auth]`)
  - dépend de : —
  - choix : `IssuerConfig.native: Option<NativeClient>` (`kartenn-auth`), méthode
    `Authenticator::native_clients` (vide par défaut, comme `issuers`) ; route ajoutée à
    `authn::PUBLIC` et aux opérations publiques du contrat (`security: []`),
    `operationId` `native_clients`. Défauts : `native_scopes = ["openid"]`,
    `native_device = false`. Démarrage refusé si `native_client_id` égale `client_id`
    (un client confidentiel n'est pas public), si une portée n'est pas un jeton de
    RFC 6749 §3.3 ou si `openid` manque, et si `native_scopes`/`native_device` sont
    posés sans `native_client_id`. `audience` publiée = celle de l'émetteur.
    Tests : `sessions::the_native_clients_are_public`,
    `config::tests::an_issuer_may_publish_a_native_client` et cas d'erreur.
    `docs/exploitation/configuration.md` complété.
- [x] 7. `kartenn-dev-idp` :
  - client public natif (sans secret) ;
  - redirection de bouclage à port libre (RFC 8252 §7.3) ;
  - PKCE `S256` exigé pour ce client ;
  - `device_authorization_endpoint` (RFC 8628 §3.1 à 3.5, `slow_down` simulable) ;
  - rotation des jetons de rafraîchissement ;
  - jetons d'accès `at+jwt` avec l'audience de kartenn.
  — T10, T23 — verify : tests du crate (code et PKCE, appareil, rotation : l'ancien jeton
  de rafraîchissement est refusé)
  - où : `crates/kartenn-dev-idp/src/server.rs` (`grant_types_supported` l. ~183,
    échange l. ~394) ; `web/e2e/users.toml` (forme de la configuration)
  - dépend de : —
  - choix : table facultative `[native]` (`client_id` autre que celui du BFF,
    `audience`, `redirect_uris` de bouclage sans port, `["http://127.0.0.1/callback",
    "http://[::1]/callback"]` par défaut, `localhost` refusé (RFC 8252 §8.3), `device`,
    `device_interval` (5 s), `device_expires_in` (600 s), `slow_down`). Client public
    authentifié par `client_id` du formulaire sans secret (`token_endpoint_auth_methods`
    gagne `none`) ; codes et jetons de rafraîchissement liés à leur client ; `resource`
    ou `audience` autre que l'audience → `invalid_target`. Flux d'appareil :
    `POST /device_authorization`, page `GET /device` (code tapé sans tiret ni casse) et
    `POST /device` (choix de l'utilisateur ; un utilisateur à `error` refuse) ;
    `slow_down` sur les `slow_down` premières interrogations d'un code et sur toute
    interrogation avant l'intervalle, qui croît de 5 s ; `authorization_pending`,
    `access_denied`, `expired_token`. Jeton d'accès du client public : JWT `at+jwt`
    (`aud`, `client_id`, `scope`, `groups`, `jti`) ; le BFF garde son jeton opaque.
    `dev/idp.toml`, `web/e2e/users.toml` (intervalle 1 s, un `slow_down`),
    `dev/kartenn.toml` et `web/e2e/kartenn.toml` (`native_client_id = "kartenn-cli"`)
    configurés pour la CLI. Tests de routeur sans TLS (`server/tests.rs`, `tower` et
    `http-body-util` en dépendances de développement). Parcours web de bout en bout
    (`k-web-e2e`) non relancé : le flux du BFF est couvert par
    `the_refresh_tokens_rotate`.

## Flux B — bibliothèque `kartenn`

- [x] 8. Cœur sans entrées-sorties (`_core/`) :
  - dates : RFC 3339 avec décalage, date seule dans un fuseau, relatives ;
  - requêtes : filtres de la 006, `--prop` et opérateurs, bascule `QUERY` au-delà de
    8 Kio, en-têtes ;
  - problèmes → exceptions typées avec violations, membres d'extension conservés ;
  - pages et curseurs ;
  - suivi SSE : `Last-Event-ID`, doublons, `reset`, attente de reprise.
  — REQ-023-9, 10, 11, 22 (analyse), 38 (logique) — verify : tests unitaires et
  hypothesis (aller-retour des dates, filtres → URL ou corps `QUERY` équivalents)
  - où : `clients/python/kartenn/src/kartenn/_core/` ; 006/design § Curseurs, § Filtres,
    § Erreurs ; 022/design § Format, § Doublons
  - dépend de : 1, 2
  - choix : messages HTTP du cœur (`Headers`, `Request`, `Response`) dans
    `_core/messages.py`, hors de `requests.py`. Toute erreur est un `KartennError` à
    code : slug du problème (`http-<statut>` sans corps de problème) ou `client.*`.
    Classes par statut (`NotFound`, `Conflict`, `PreconditionFailed` avec
    `current_etag` pris de l'extension `current_etag` puis de l'en-tête `ETag`,
    `Refused` pour tout 422…) ; ruff ignore `N818` (noms de REQ-023-11) et `EM101` (le
    code est un littéral). Dates relatives `[+-]N[smhdwMy]` (`M` mois, `m` minute) et
    `now` ; mois ajoutés au jour borné. Filtres : valeurs Python gardées telles quelles
    dans le corps `QUERY`, mises en texte dans l'URL (`true`/`false`) ; `:` littéral dans
    l'URL ; bascule `QUERY` quand `base + chemin + requête` dépasse 8 Kio ;
    `traceparent` marqué échantillonné (`-01`). `page.as_of` : membre `as_of` du corps
    s'il existe, sinon la date demandée — le serveur ne rend pas encore l'`as_of`
    effectif d'une liste (point ouvert). Doublons d'événements : identité
    `changes`/changeset (ou `event` d'une tâche) et `changeset`/changeset/état, 10 000
    retenues (023) ; `ready` et `reset` jamais écartés. Profil hypothesis sans délai ni
    `too_slow` (machine partagée).
- [x] 9. Port de transport (`send`, `stream`, `sse`, synchrone et asynchrone) et
  adaptateur httpx2 :
  - délais ;
  - HTTP/2 facultatif ;
  - `ssl.SSLContext` explicite pour `ca_bundle` ;
  - reprise sur 429 et 503 (`Retry-After`) ;
  - `Idempotency-Key` ;
  - `traceparent` ;
  - `User-Agent`.
  — REQ-023-4, 5, 14, 41 — verify :
  - `httpx2.MockTransport` : reprise, `POST` sans clé jamais rejoué, flux et SSE ;
  - serveur TLS local (CA de test) : `ca_bundle` accepté, CA du système seule refusée
    quand `ca_bundle` est donné ;
  - test anyio sur asyncio et trio.
  - où : `clients/python/kartenn/src/kartenn/transport/`, `retry.py` ; design
    § Transport
  - dépend de : 8
  - choix : un statut d'erreur garde la réponse ; `stream` et `sse` rendent alors son
    corps dans `content` et rien en flux ; une réponse 200 qui n'est pas
    `text/event-stream` est traitée de même. `sleep` fait partie du port (attente du
    moteur d'événements, tests sans attente) ; l'adaptateur asynchrone attend par anyio,
    dépendance de httpx2, importé seulement dans `transport/httpx2.py`. Délai de lecture
    de 45 s pour `sse` (trois `: ping` manqués) plutôt qu'aucun : la coupure est alors
    une `NetworkError` que le lecteur reprend. HTTP/2 automatique si `h2` est installé
    (pas d'extra déclaré : le verrou n'en dépend pas), refus explicite si demandé sans
    lui. Reprise : 3 rejeux au plus, 60 s au total, `Retry-After` sinon 1, 2, 4 s ; une
    coupure réseau (jamais TLS) se rejoue pareil ; méthodes rejouables : `GET`, `HEAD`,
    `OPTIONS`, `QUERY`, `PUT` (morceaux d'import), ou toute requête à
    `Idempotency-Key` ; `PATCH` et `DELETE` ne le sont pas. Un même `traceparent` pour
    les rejeux d'une requête. Journal `kartenn` en DEBUG : méthode, chemin sans requête,
    statut, durée. Test TLS : deux CA générées par `cryptography` (ajoutée au groupe
    `dev`, verrou régénéré hors ligne), « magasin du système » simulé par
    `SSL_CERT_FILE`. Moteurs anyio asyncio et trio par la fixture `anyio_backend`.
- [x] 10. Catalogue :
  - dérivation (tableau du design) ;
  - modèle gelé ;
  - cache XDG par révision (version, modules, ETag du contrat s'il est servi), écriture
    atomique en 0600 ;
  - relecture des capacités au plus une fois par minute.
  Rédiger l'ADR 0035 (proposée). — REQ-023-1, 2, 3 — verify :
  - instantané du catalogue de `specs/006-api-rest/openapi-v1.json` ;
  - contrat sans le tag `policy` → aucune ressource `policy` ;
  - module `demo` dérivé sans code ;
  - même révision → aucun téléchargement.
  - où : `clients/python/kartenn/src/kartenn/catalog/` ; design § Catalogue ; ADR 0026
    clause 9 ; forme des ADR dans `docs/adr/0031-…`
  - dépend de : 8 (9 pour le chargement réseau)
  - choix : la découverte est un *plan* (`_core/plans.py` : générateur qui émet des
    requêtes et reçoit des réponses, exécuté par `run` ou `arun`), pour écrire une fois
    la logique des deux clients. Toutes les collections de `/v1` deviennent des
    ressources, changesets et imports compris, sauf `auth`, `capabilities`, `me` et
    `openapi.json` ; chaque opération du contrat (sondes comprises) est dans l'index par
    `operationId`. Verbes : `list`, `create`, `query`, `replace` (collection) ; `get`,
    `update`, `close`, `set`, `post` (objet) ; sous-ressources nommées par leurs segments
    fixes joints par `.`, suffixées de la méthode quand le chemin finit par un gabarit
    hors `GET` (`attachments.delete`) ou en cas de collision ; `x-kartenn.command`
    l'emporte. Schémas gardés bruts (`components.schemas`), références suivies à la
    demande (profondeur 4) plutôt qu'aplaties dans le cache. Révision tronquée à 32
    chiffres hexadécimaux. Le catalogue en cache ne garde pas les capacités : le
    pointeur du serveur les porte (relues ≤ 1/min sans changer la révision). Sans ETag du
    contrat (le serveur actuel n'en sert pas, tâche 4), même version et mêmes modules →
    aucun téléchargement ; avec ETag → `If-None-Match` et 304. Instantané du catalogue de
    référence en JSON (`tests/snapshots/`, `KARTENN_SNAPSHOT_UPDATE=1` pour le refaire) :
    syrupy n'est que dans les dépendances de la CLI. `x-kartenn` codé contre le contrat
    de la tâche 5 (pas encore servi), testé sur un contrat simulé (`demo`). ADR 0035
    rédigée (proposée) et indexée.
- [x] 11. Ressources dynamiques et `Record` (ETag, liens, `as_of`) :
  - `list`, `get`, `create`, `update` (Merge Patch sous `If-Match`), `close` (motif) ;
  - `kn.call(operationId)`, `kn.at(...)`, `kn.modules`, `kn.has()` ;
  - `Client.from_profile` (tomllib) ;
  - port `TokenStore` (mémoire ; fichier 0600 explicite).
  — REQ-023-12, 16 — verify : 412 → `PreconditionFailed` avec l'ETag courant ;
  `close` sans motif → refus local ; profil lu sans secret
  - où : `clients/python/kartenn/src/kartenn/resources.py`, `profiles.py`,
    `auth/store.py` ; 006/design § Écritures (arbitrage 006-16)
  - dépend de : 9, 10
  - choix : clients dans `client.py` (`Client`, `AsyncClient`), exportés par
    `kartenn`. Chaque appel est un plan de `resources.Context` (catalogue chargé dans le
    plan) : l'`AsyncClient` atteint une ressource sans catalogue, le lit à l'appel ou
    par `await kn.load()` / `async with`. `kn.<attribut>` : `_` → `-`
    (`kn.policy_rules`) ; `kn.resource()` du client synchrone vérifie aussitôt et lève
    `UnknownResource` (aussi `AttributeError`). Verbes par nom du catalogue
    (`kn.prefixes.verb("properties.explain", id, key=…)`) ou par attribut (`.` et `-`
    → `_`) ; `pages_of` suit les pages d'une sous-ressource. Écritures : `Applied`
    (avec `Location`), `Pending` (avec le lien `urn:kartenn:rel:changeset`), l'objet
    rendu sinon, `None` sans corps ; `Idempotency-Key` sur tout `POST` dont
    l'opération l'accepte et toujours sur `create` ; `If-Match` pris du `Record` (ETag
    opaque, H19), refus local `client.etag-required` sans lui ; `close` sans motif →
    `client.motive-required` sans requête. Paramètre inconnu d'une opération →
    `client.unknown-parameter`. `kn.at()` rend une vue (transport partagé, non fermé
    par la vue). Profils : `KARTENN_PROFILE` puis `default` ; clés de secret refusées
    (`client.secret-in-profile`) ; modes `none` et `token` (`KARTENN_TOKEN`) ici, les
    autres à la tâche 13. `FileStore` refuse un fichier lisible par d'autres
    (`client.store-permissions`). Auth par plans (`authorize`, `unauthorized`) : une
    seule nouvelle tentative après un 401 si l'auth se renouvelle.
- [x] 12. Changesets et imports :
  - `ChangesetBuilder`, simulation, soumission (`Applied`, `Pending`, `Refused`),
    actions sous `If-Match`, différentiel paginé par section ;
  - `ImportSession` : morceaux, état local XDG, reprise par empreinte, rappel
    d'avancement.
  — REQ-023-13, 15 — verify : 202 → `Pending` ; reprise au morceau 41 ; 10⁶ éléments
  synthétiques en mémoire ≤ 100 Mio (transport simulé)
  - où : `clients/python/kartenn/src/kartenn/changesets.py`, `imports.py` ; 006/design
    § Imports et tâches de fond ; 017/decisions Q-017-1
  - dépend de : 11
  - choix : `kn.changeset(motive=…)` rend le constructeur ; `add(op)` rend l'indice
    (référence locale `{"local": n}`), `create(ressource, …)` prend pour `op`
    l'`operationId` de la création de la ressource, résolu à l'envoi. Discriminants lus
    dans le schéma `Operation` du contrat (références et `oneOf`/`allOf` suivis) ; une
    clé d'idempotence par constructeur, absente de la simulation ;
    `Kartenn-On-Behalf-Of` pour « pour le compte de ». `kn.changeset_of(réf)` :
    `read`, `approve([opérations])`, `reject(motif)`, `withdraw`, `undo(simulate=)`
    sous l'`If-Match` de la lecture (relue si absente, oubliée après chaque action) ;
    `diff(section=, prop=)` rend un `Listing` de lignes génériques. Imports :
    `kn.import_session(source, digest=…)` ; `run` crée toujours un nouvel import,
    `resume` exige l'état local de (serveur, source) et la même empreinte ; état effacé
    après `start`. Morceaux numérotés depuis 0, `chunk_elements` des capacités par
    défaut (borné par elles), découpage par nombre d'éléments seulement (pas par
    `body_bytes`). Lignes des éléments gardées dans un `array` (8 octets par élément) pour
    traduire l'indice d'une violation (dernier entier du `pointer`, ou membre `element`)
    en ligne source. `wait` suit au rythme de `Retry-After` (5 s sans lui). Test de
    10⁶ éléments sous `tracemalloc` sur un transport muet : pic sous 100 Mio (environ
    17 s).
- [x] 13. Authentification : Basic, Bearer fourni, OIDC natif (bouclage `127.0.0.1` à
  port libre, PKCE `S256`, `state`, `nonce`), flux d'appareil, rafraîchissement, secours
  avec `Origin`. Note `docs/reference/oauth-clients-natifs.md` (RFC 8252, 8628, 9700,
  7636, citations recoupées). — REQ-023-6, 7, 8, 40 — verify :
  - `MockTransport` : `state` falsifié refusé, `slow_down` +5 s, `expired_token` ;
  - aucun secret dans les journaux (capture) ;
  - Basic vers `http://` hors bouclage refusé ;
  - contre `kartenn-dev-idp` (si la tâche 7 est faite) : connexion complète.
  - où : `clients/python/kartenn/src/kartenn/auth/` ; design § Authentification ;
    005/design § Jetons Bearer ; forme des notes de `docs/reference/`
  - dépend de : 9 (7 pour le test réel)
  - choix : chaque mode est un `Auth` à plans (`authorize`, `unauthorized`,
    `check_server`). `Secret` masqué dans toute représentation, lu d'un `TokenStore`
    (`kartenn:<profil>:secret`) ou de `KARTENN_SECRET_FILE` (première ligne) ; Basic
    refusé hors `https`, bouclage permis seulement par `allow_insecure_loopback`.
    OIDC : émetteur complet dans le profil (`discovery_url`, `client_id`…) ou identifiant
    résolu par `GET /v1/auth/native-clients` (contrat de la tâche 6, pas encore servi :
    testé sur réponses simulées) ; document de découverte refusé s'il annonce des
    méthodes PKCE sans `S256` ; audience demandée par `resource` (RFC 8707) quand c'est
    une URI absolue, par l'extension `audience` sinon ; jetons dans le `TokenStore`
    (`access_token`, `refresh_token`, `expires_at`), le jeton d'identité jamais gardé ni
    envoyé ; rafraîchissement 60 s avant l'expiration ou une fois sur un 401, échec →
    jetons effacés et `client.login-required`. `login` (bouclage) et `device_login`
    sont synchrones et prennent `Client.run_unauthenticated` (ajouté, avec
    `Client.sleep`) : la connexion interactive est l'affaire de la CLI ; l'`AsyncClient`
    rafraîchit par les mêmes plans. Flux d'appareil : intervalle de 5 s par défaut,
    +5 s cumulés par `slow_down`, arrêt sur `access_denied`, `expired_token` ou
    `expires_in` dépassé ; l'avertissement de RFC 8628 §5.4 est rendu avec le code.
    Secours : cookie de la première ligne `Set-Cookie`, phrase de passe et code oubliés
    après usage, `Origin` sur l'ouverture et sur toute écriture, cookie oublié sur un
    401. Pas de verrou entre processus autour du rafraîchissement (proposition de la
    note, laissée à la CLI). Note `docs/reference/oauth-clients-natifs.md` rédigée par
    un sous-agent sur les sources primaires (citations marquées « à vérifier » quand
    non recoupées) et indexée. Test contre `kartenn-dev-idp` non fait : la tâche 7
    n'est pas sur `main`.
- [x] 14. Événements : `events()` et `aevents()` sur `GET /v1/events`, filtres, reprise,
  doublons, `reset`, absence quand `capabilities.events` vaut `null`. — REQ-023-38 —
  verify : `MockTransport` en flux SSE (coupure puis reprise avec `Last-Event-ID`,
  doublon écarté, `reset` rendu, 429 puis `Retry-After`)
  - où : `clients/python/kartenn/src/kartenn/events.py` ; design § Événements ;
    022/design § Contrat consommé par les clients
  - dépend de : 9, 11
  - choix : `kn.events(...)` (synchrone) vérifie aussitôt `capabilities.events` et rend
    un itérateur sans fin ; `kn.aevents(...)` (asynchrone) est un générateur qui vérifie à
    la première itération. Filtres en listes séparées par des virgules ;
    `last_event_id=` reprend après un curseur connu ; `reconnect=False` s'arrête à la
    fin du flux. Avant ouverture : un 401 renouvelle une fois par l'`Auth` du client puis
    rouvre, 429 et 503 attendent `Retry-After` (sinon l'attente croissante), toute autre
    erreur lève son problème. Coupure (fin du flux, `NetworkError`, 45 s de silence) →
    reprise après l'attente du suivi (tâche 8) ; un échec TLS n'est jamais repris.
    Chaque ouverture porte `User-Agent` et un `traceparent` neuf, pas de nouvelle
    tentative du transport : le suivi gère ses propres attentes. Codé contre le
    contrat de la 022 (capacité `events`, `GET /v1/events`), pas encore servi : testé sur
    flux simulés.
- [x] 15. Stubs : `python -m kartenn stubs generate` (`TypedDict`, `Literal`,
  `Protocol`, surcharge de `Client.__getattr__`) — REQ-023-17 — verify : pyright signale
  un membre inconnu dans un script d'exemple ; les stubs ne sont jamais importés à
  l'exécution
  - où : `clients/python/kartenn/src/kartenn/stubs.py`, `__main__.py`
  - dépend de : 10
  - choix : source du catalogue au choix (`--profile`, `--url`, ou `--contract`
    hors ligne avec `--capabilities` facultatif) ; sortie `--out typings` par défaut,
    paquet `kartenn_server/__init__.pyi`. `TypedDict` en syntaxe fonctionnelle (membres
    qui ne sont pas des identifiants, `allOf` fusionné), `NotRequired` hors `required` ;
    énumérations et autres schémas en `TypeAlias` ; `typing`, `kartenn` et
    `kartenn.resources` importés sous alias (`_t`, `_k`, `_r`) pour ne heurter aucun nom
    de schéma ; noms invalides préfixés ou suffixés de `_`. Protocole par ressource :
    `list` aux filtres typés (énumérations en `Literal`), `create` et `update` aux
    membres en `**members: Unpack[…]` (membres réservés exclus), sous-ressources et
    actions par leur nom en `snake_case`. `Client` du stub : une propriété par
    ressource, `resource()` surchargé par nom littéral, `__getattr__(Never)` pour qu'un
    nom inconnu soit une erreur ; ces surcharges plus étroites que `kartenn.Client` sont
    voulues (`# pyright: ignore[reportIncompatibleMethodOverride]`). Usage :
    `cast("kartenn_server.Client", kartenn.Client(...))` sous `TYPE_CHECKING`. Test :
    pyright de devenv (sauté sans lui) signale `nmae` et `prefixess` dans un script
    d'exemple, rien dans le bon ; le script tourne sans que `kartenn_server` soit
    importé.

## Flux C — CLI `kartenn-cli`

- [x] 16. Racine paresseuse :
  - `click.Group` dont `list_commands` lit le catalogue en cache sans réseau ;
  - rich-click importé seulement pour `--help` ;
  - profils et précédence ; `config` ; i18n `fr.toml` et `en.toml` ;
  - `TokenStore` keyring (rien stocké sans trousseau ; `--token-file`) ;
  - rendus `table`, `json`, `ndjson`, `yaml`, `csv`, rendu des problèmes, codes de
    sortie ;
  - garde d'import : `--version` n'importe ni httpx2 ni rich.
  — REQ-023-25, 26 (rendu), 33, 34, 35, 36, 37, 39 — verify : instantanés syrupy des
  rendus sur des réponses figées ; un test par code de sortie ; `NO_COLOR` ;
  `--no-input` sans terminal
  - où : `clients/python/kartenn-cli/src/kartenn_cli/main.py`, `config.py`, `render/`,
    `i18n/` ; design § Sorties, § Codes de sortie, § Rendu d'un problème ;
    `web/sdk/src/i18n.ts` (résolution de langue)
  - dépend de : 1, 2, 8 (exceptions)
  - choix : `--help` en texte par click (sections d'options et de commandes, traduites
    à l'affichage : une aide ou une option porte une clé de catalogue), rendu par
    rich-click seulement sur un terminal en couleurs, sur un miroir de la commande
    (`base.py`) ; les instantanés et la maquette sont donc le rendu texte. Langue :
    `--lang`, puis `KARTENN_LANG`, puis la langue du profil, puis la première variable
    de locale posée (`LC_ALL`, `LC_MESSAGES`, `LANG`, comme POSIX), repli en anglais ;
    lue avant l'analyse des options (la ligne d'usage s'écrit d'abord). Les options de
    la racine s'appliquent dès leur analyse (`-p` et `--lang` en premier) ; celles
    écrites après `--help` sont ignorées. `--version` lit `kartenn_cli.__version__`
    (un test le garde égal à la métadonnée) : 53 ms en médiane, 61 ms au p95 sur la
    machine partagée, au lieu de 67/96 ms avec `importlib.metadata`. `config` :
    `add-profile` (le premier profil devient le défaut), `list`, `show`, `use`,
    `remove` (efface aussi le secret du trousseau) ; écriture du TOML à la main (aucune
    dépendance), commentaires non gardés, 0600. `KARTENN_URL` remplace l'URL du profil
    ou tient lieu de profil (`auth = token` si `KARTENN_TOKEN`). Trousseau : `keyring`
    importé au premier secret ; un dorsal de priorité nulle (`fail`, `null`) ou en
    erreur vaut absence (`client.keyring-unavailable`, code 4, qui propose
    `KARTENN_TOKEN` et `--token-file`). Rendus : un JSON de liste est un tableau d'un
    élément par ligne (maquette § 4) et non la page entière ; table et CSV prennent les
    colonnes de la table ; en-têtes de table = noms des membres en capitales (non
    traduits : clés) ; pied de table sur la sortie standard ; valeur masquée
    `‹masquée›` en table, vide en CSV. Codes de sortie : 400, 413, 415 → 2 ; erreurs
    `client.*` → 2 sauf connexion (4), `reference-not-found` (3), réseau (7),
    `unexpected-answer` (1). Problème en `-o json` : RFC 9457 tel quel ; erreur du
    client : `{"code", "detail"}`. Messages de la bibliothèque laissés en anglais ;
    messages propres à la CLI traduits. `--verbose` : méthode, chemin, statut, durée,
    `Request-Id` (006) sur la sortie d'erreur.
- [x] 17. Arbre dérivé :
  - groupes de modules, préfixe retiré, verbes, sous-ressources ;
  - aide tirée du contrat (module, opération HTTP) ;
  - module absent → code 2 et message de la maquette § 12.
  — REQ-023-18, 21 — verify : instantané de `--help` de chaque groupe sur le contrat de
  référence ; contrat sans `policy` ; module `demo`
  - où : `clients/python/kartenn-cli/src/kartenn_cli/tree.py` ; design § Nommage ;
    maquette § 2, 3, 12
  - dépend de : 10, 16
  - choix : `changesets`, `imports`, `proposals` ne sont pas dérivés (commandes
    nommées `changeset` et `import`) ; `audit-events` devient `audit` (seul alias du
    premier niveau). Verbes : noms d'action du contrat mis au verbe
    (`acknowledgement` → `acknowledge`, `validation` → `approve`, `waiver` → `waive`,
    `attachments` → `attach`/`detach`, `secrets` → `secrets add|revoke`…) ; `query`
    caché (`list` bascule seul) ; noms pointés en sous-groupes, `properties REF` vaut
    `properties show REF`. Sections de l'aide de la racine par tag (`core`,
    `properties`, `graph` → noyau ; `rights`, `audit` → droits ; `operations` →
    outils) ; description d'un module dans les catalogues de messages
    (`module.about.<nom>`, le contrat ne décrit pas ses tags), sinon « Module <nom> ».
    Un nom inconnu relit les capacités (au plus une fois par minute) avant de répondre ;
    `client.module-unavailable` pour les modules connus (`policy`, `flow`, `vlan`,
    `machines`) et pour une sous-ressource servie par un module absent (`flows`,
    `evaluations` → `flow`), sinon « no such command » de click (code 2). La découverte
    est anonyme (routes publiques), par un client sans authentification, et dit sa ligne
    sur la sortie d'erreur (« Découverte du serveur… »), aussi pendant `--help`. Sans
    serveur ni cache, `--help` liste les commandes locales et le dit en une ligne.
    Budgets mesurés à cache chaud (contrat de référence, machine partagée, 20 à 40
    tirages) : `--help` 100 ms en médiane, 119 ms au p95 ; `policy --help` 90/106 ms ;
    `policy rules list --help` 93/113 ms.
- [x] 18. Lectures :
  - `--at`, `--as-of`, `--freeze` ;
  - filtres typés et `--prop` ;
  - `--fields`, `--query` (JMESPath), `--keys`, `--ids`, `--all`, `ndjson` en flux ;
  - références par clé naturelle (`x-kartenn.key`, UUID sinon).
  — REQ-023-22, 23, 25, 26, 27 — verify : maquette § 4 en instantané ; filtre inconnu
  refusé sans requête ; `ndjson --all` en mémoire constante
  - où : `clients/python/kartenn-cli/src/kartenn_cli/tree.py`, `render/` ; maquette § 4
  - dépend de : 11, 17
  - choix : lectures dans `reads.py` (`list`, `get`, autres lectures d'objet et de
    sous-ressource) ; dates résolues par le cœur de la bibliothèque
    (`kartenn._core.dates`), dans le fuseau du profil, sinon le fuseau local ; les dates
    de la commande priment sur celles de la racine (`KARTENN_AS_OF` compris).
    `--freeze` envoie `as_of` = maintenant (UTC, à la seconde) et l'écrit sous la table
    (`export KARTENN_AS_OF=…`). Pied de table : nombre d'éléments, dates (l'`as_of` du
    corps s'il est rendu, sinon celui demandé ; rien sinon), puis « suite : --all ou
    --cursor <curseur> » ; hors table, cette suite va sur la sortie d'erreur. Options de
    filtre toutes répétables (« un parmi ») ; une option inconnue de `list` est
    `client.unknown-filter` avec la liste des filtres, avant toute requête. Une
    sous-ressource paginée (`history`, `children`…) prend `--all`. Références : UUID,
    sinon clé naturelle de `x-kartenn.key` découpée par `/` (la dernière partie garde ses
    `/`) et cherchée par les filtres de `list` (`page_size=2`) : aucune → code 3,
    plusieurs → code 2 ; sans clé déclarée, l'UUID est exigé (`client.key-unknown`),
    conformément à Q-023-9. Le flux S (`x-kartenn`) n'est pas encore sur main : tests sur
    un contrat simulé qui le porte. Une sortie `ndjson --all` écrit la page 1 avant de
    demander la page 2 (testé).
- [x] 19. Complétion bash, zsh et fish (`shell_complete`) : catalogue, énumérations, clés
  et valeurs de propriétés, slugs ; cache de 5 min sans valeur masquée ; délai d'une
  seconde au plus — REQ-023-24 — verify : scripts générés ; complétions testées dans les
  trois shells (pexpect) ; serveur muet → propositions locales en moins d'une seconde
  - où : `clients/python/kartenn-cli/src/kartenn_cli/complete.py` ; maquette § 5
  - dépend de : 18
  - choix : `kartenn completion bash|zsh|fish` écrit le script de click
    (`_KARTENN_COMPLETE`). Commandes, options et énumérations : catalogue en cache
    seulement (jamais de découverte pendant une complétion) ; clés de propriétés :
    `capabilities.properties` en cache ; valeurs d'une liste fermée :
    `GET /v1/property-declarations/{clé}/values`, proposées `clé=valeur` avec le
    libellé en aide ; slugs : un filtre `x` nomme la collection `xs`, `xes` ou `xies`
    si elle existe (`domain`, `entity`, `flow_type`…), valeurs `slug` ou `key` de sa
    première page de 200. Lecture au serveur dans un fil démon attendu 0,8 s ; tout
    échec ou retard ne propose rien et n'écrit rien. Cache des valeurs : un fichier
    0600 par serveur sous `$XDG_CACHE_HOME/kartenn/complete/`, 5 min, sans élément
    `masked`. Tests : protocole de click pour les trois shells ; bash et zsh réels sous
    pexpect (nouvelle dépendance de développement, avec `types-pexpect`) ; fish absent
    de la machine, son test réel est sauté (protocole testé). Budget : l'import de la
    bibliothèque coûtait l'essentiel (180 ms en médiane pour `kartenn vlan <TAB>`) ;
    `kartenn/__init__.py` charge désormais ses noms au premier usage (commit à part) :
    complétion locale 78 à 83 ms en médiane, 110 à 124 ms au p95 sur la machine
    partagée chargée (budget 100 ms au p95, à confirmer par le banc de la tâche 27).
- [x] 20. Écritures :
  - `create`, `update`, `close`, `edit` (fichier 0600, 412 → réouverture) ;
  - `--dry-run`, `--into-changeset`, `--require-applied` ;
  - `changeset new|add|simulate|apply|show|diff|list|approve|reject|withdraw|undo|proposals`
    et rendu des sections de module.
  — REQ-023-28 à 31 — verify : maquette § 8 et 9 en instantané ; fichier temporaire
  supprimé même sur erreur
  - où : `clients/python/kartenn-cli/src/kartenn_cli/edit.py`, `tree.py` ; design
    § Codes de sortie
  - dépend de : 12, 18
  - choix : écritures dans `writes.py`, `edit.py`, `changeset.py`. Membres en options
    typées tirées du schéma du corps (scalaires, énumérations, références `{id}` résolues
    par clé naturelle), puis `--set chemin=valeur` (JSON si lisible), sur un document
    `--from-file` (YAML ou JSON, `-` : entrée standard) ; envoi par `client.call`
    (`Idempotency-Key`, `If-Match` d'une lecture préalable, `Kartenn-On-Behalf-Of`).
    Motif exigé quand le schéma l'exige (`client.motive-required`, code 2, demandé sur
    un terminal) ; sans motif, `--dry-run` simule sous le motif « simulation ». Hors
    `-o` demandé (option, environnement, profil), une écriture répond par une phrase et
    non en JSON. **Point ouvert** : le contrat ne dit pas quelle opération de changeset
    correspond à une écriture REST. `--dry-run` et `--into-changeset` traduisent donc
    `create_<x>` (membres tels quels), `update_<x>` si le schéma `Operation` le connaît,
    `close` d'un `kind` de `TargetKind`, et `<module>.<operationId>` pour un module ;
    sinon `client.dry-run-unavailable`. Les membres REST et ceux des opérations
    diffèrent parfois (`value` d'un préfixe contre `prefix` de `create_prefix`) : à
    trancher côté serveur (accepter `?simulate=true` sur toute écriture, ou décrire
    l'opération dans `x-kartenn`). Bibliothèque corrigée à part : une opération
    `<module>.<op>` d'un module servi n'est plus refusée localement. Changeset local
    `./.kartenn-changeset.yaml` (ou `KARTENN_CHANGESET`), 0600, avec sa clé
    d'idempotence, supprimé après soumission ; `changeset list` et `proposals`
    reprennent `list` du catalogue. Simulation : comptes, violations, différentiel
    compté par propriété, sections de module en table générique. `edit` : YAML des
    membres modifiables (corps de `update`), le reste en commentaire ; fichier 0600 dans
    un répertoire privé supprimé dans tous les cas ; 412 → proposition de rouvrir sur la
    version courante, modifications de l'utilisateur en commentaire. Découverte du
    catalogue désormais authentifiée (flux S : `capabilities` et `openapi.json` ne sont
    pas publics) ; sans connexion, `--help` dit « kartenn login ».
- [x] 21. `import` : NDJSON, CSV et JSON par ressource (`--resource`, `--id-column`),
  avancement (rich sur terminal, lignes sinon), `--resume`, `--follow`, violations à la
  ligne source — REQ-023-32 — verify : maquette § 10 sur transport simulé ;
  `import.unknown-reference` avec `fichier:ligne`
  - où : `clients/python/kartenn-cli/src/kartenn_cli/importing.py` ; schéma
    `ImportElement` du contrat
  - dépend de : 20
  - choix : `importing.py`. NDJSON (`.ndjson`) tel quel ; sinon CSV (ligne = ligne du
    fichier, en-tête en 1) ou JSON (tableau, rang à partir de 1) avec `--resource`
    exigé (`client.resource-required`). Une ligne devient l'opération de création de la
    tâche 20 (même point ouvert sur la correspondance REST → changeset) : colonnes
    typées par le schéma, références résolues par clé naturelle une seule fois par
    valeur, `prop.<clé>` dans `properties`, cellules vides omises, `--id-column` en
    `external_id`. Source par défaut : le nom du fichier. Suivi jusqu'à l'état final par
    défaut (`--no-follow` pour rendre la main après le démarrage), par la
    bibliothèque (`wait`) ; avancement rich sur la sortie d'erreur d'un terminal (morceaux
    et éléments ; le total n'est pas connu en flux), une ligne par étape sinon. Refus du
    tri : problème de l'import, violation à `fichier:ligne`, consigne de reprise, code 6.
    `--resume ID` reprend l'import local de la source (empreinte vérifiée), et dit si
    l'identifiant diffère.
- [x] 22. Commandes nommées : `policy check` (`--exit-status`), `policy matrix show|set`,
  `policy report`, `flow request|review|waive`. Table `named.py` liée aux `operationId`,
  masquée si une opération manque. — REQ-023-20 — verify : maquette § 6 et 7 ; contrat
  sans `flow` → commandes absentes
  - où : `clients/python/kartenn-cli/src/kartenn_cli/named.py` ; design § Commandes
    nommées ; 017/design § Requête de conformité
  - dépend de : 20
  - choix : table `NAMED` de `named.py` (module, nom, `operationId` requis, fabrique,
    commandes dérivées remplacées) : `policy check` remplace `verdicts`, `policy matrix`
    la ressource `matrix`, `policy report` `reports`, `flow request` le verbe `create` de
    `flows` ; `flow waive` reste le verbe dérivé (action `waiver`). Une entrée dont une
    opération manque n'est pas offerte et ne remplace rien. `check` : SOURCE et
    DESTINATION passées telles quelles (`machine:bastion-1`), rendu par la synthèse
    (verdict, issue, comptes), la règle décisive, les conditions non remplies, la
    réduction et les adresses cachées ; `--exit-status` → 9 si `denied`. `matrix show` :
    grille ✓ ✗ ◐ source × destination ; `matrix set` : `If-Match` de la matrice lue, corps
    `{state, motive}` (forme du corps de `PolicyObject` non précisée par le contrat : à
    confirmer au bout en bout). `report` : demande, `--wait` (relecture toutes les 2 s),
    comptes par rubrique, `--rubric` pour ses lignes, `--id` pour un rapport existant.
    `flow request` : extrémités `TYPE:RÉF` (référence résolue par la collection du type)
    ou JSON, service et entité par clé, `--ref SYSTÈME:ID`. `flow review` :
    approbation ou rejet du changeset en attente du flux (lu par `show_flow`).
- [x] 23. `kartenn api` : méthodes dont `QUERY`, `-f`, `--data @fichier|-`, `-H`,
  `--paginate` — REQ-023-19 — verify : maquette § 11
  - où : `clients/python/kartenn-cli/src/kartenn_cli/main.py`
  - dépend de : 18
  - choix : dans `api.py` (et non `main.py`, gardé léger). Méthode insensible à la
    casse ; chemin absolu (`/…`) ; `-f` en paramètres de requête même pour `QUERY` ;
    `--at`/`--as-of` ajoutent `valid_at`/`as_of` ; `--data` pose `Content-Type:
    application/json` sauf `-H` contraire ; un `POST` reçoit une `Idempotency-Key`
    si `-H` n'en donne pas. Réponse JSON rendue (JSON d'une ligne par défaut), texte
    tel quel, rien sans corps ; erreur rendue comme un problème avec son code de
    sortie. `--paginate` sur une page `{items}` suit `links.next` et écrit un élément
    par ligne (`ndjson` par défaut), une réponse non paginée est rendue telle quelle.
    Outils de la racine ajoutés ici : `api spec` (le contrat), `capabilities`,
    `health [live|ready]`, `stubs generate --out` (même génération que
    `python -m kartenn`).
- [x] 24. `login` (bouclage, `--device`, `--breakglass`), `logout`, `me`, et `watch`
  (filtres, `--since`, `ndjson`). `--follow` d'un import ou d'un changeset sur le flux
  d'événements quand il existe. — REQ-023-7, 8, 38 — verify : maquette § 1 et 15 sur
  transport simulé ; `watch` absent si `events` vaut `null`
  - où : `clients/python/kartenn-cli/src/kartenn_cli/main.py`, `render/` ; design § Suivi
  - dépend de : 13, 14, 16
  - choix : `session.py` et `watch.py`. `login` selon le profil : `oidc` par le
    navigateur (URL écrite sur la sortie d'erreur en cas d'échec d'ouverture) ou
    `--device` (avertissement de RFC 8628 §5.4, URI et code) ; `basic` demande le
    secret sans écho et le range au trousseau ; `token`/`none` : rien à faire.
    `--breakglass` : nom, phrase de passe et TOTP saisis sans écho, session en mémoire ;
    `--keep` la garde (nom et cookie, jamais la phrase) dans
    `$XDG_RUNTIME_DIR/kartenn/<profil>.session` en 0600, que les commandes suivantes
    reprennent et que `logout` ferme (`POST /v1/auth/logout`). Après connexion :
    `GET /v1/me` puis découverte du catalogue. `me` : identité, groupes, rôles (aucun
    nom de permission codé). `watch` : `--types`, `--domains`, `--objects`, `--kinds`
    (listes séparées par des virgules), `--since` ; ligne horodatée à la réception dans
    le fuseau du profil ; Ctrl-C écrit la commande de reprise avec le dernier curseur.
    `changeset show --follow` suit le changeset sur le flux (événement `changeset` à
    état final) ou relit toutes les 2 s ; `import` suit par relecture (la bibliothèque
    attend au rythme de `Retry-After`), non par le flux.
- [x] 25. Couverture : `coverage.py` (exclusions motivées) et test qui classe chaque
  opération du contrat de référence — REQ-023-43 — verify : une opération ajoutée à un
  contrat de test et non classée fait échouer le test
  - où : design § Couverture de l'API
  - dépend de : 22, 23, 24
  - choix : `coverage.py` : `EXCLUDED` (raison par opération) et `BASE` (commandes
    écrites à la main et leurs opérations) ; `reached(catalogue)` dérive l'arbre
    (verbes, sous-groupes, `query` compté pour `list`) et ajoute la table `NAMED` et
    `BASE` ; `unclassified` nomme le reste. Contrat de référence au 2026-10-23 (après le
    flux S) : 305 opérations, 301 avec une commande, 4 exclues : `login`,
    `login_callback`, `login_issuers` (BFF du navigateur) et `query_verdicts`
    (`policy check` passe par `GET`). `PUT …/chunks/{n}` est compté pour `import`, qui
    l'emploie, et non exclu. Pour couvrir les imports, `import` sans fichier prend
    `--list`, `--status ID`, `--cancel ID` et `--resume ID` (reprise côté serveur d'un
    import en pause ; avec un fichier, `--resume` renvoie les morceaux manquants).
    `native_clients` (flux S) est compté pour `login`.

## Fin — réunion des flux

- [ ] 26. Bout en bout : script `k-py-e2e` (sh POSIX, `k-heavy --pg`) qui :
  - construit et lance `kartenn-server` sur la base `kartenn_cli` recréée et
    `kartenn-dev-idp` ;
  - crée le compte de secours et charge les données de la maquette par la CLI ;
  - rejoue les sessions de la maquette (UUID et dates normalisés) ;
  - lance une build sans la feature `policy` pour la maquette § 12 ;
  - teste `watch` si la 022 est servie.
  Étape ajoutée à la CI. — REQ-023-7, 18, 32, 38 — verify : `k-py-e2e` vert
  - où : `web/e2e/serve.sh`, `web/e2e/global-setup.ts` (à imiter) ; `devenv.nix`
    (`scripts.k-web-e2e`, l. ~492)
  - dépend de : 4, 5, 6, 7, 25
- [ ] 27. Banc `bench-cli` : budgets de REQ-023-39 (hyperfine, ou boucle chronométrée
  sous `k-heavy`, cache chaud et froid), résultats consignés dans le design —
  REQ-023-39 — verify : budgets tenus, ou écart documenté et arbitré
  - où : forme de consignation des bancs (`crates/kartenn-bench`, 006 § Budgets)
  - dépend de : 26
- [ ] 28. Livraison :
  - deux paquets Nix du flake (`kartenn-py`, `kartenn-cli`) ; complétions et page de
    manuel installées ;
  - `docs/spec/` synchronisée (interfaces : clients) ;
  - ADR 0035 acceptée ;
  - README de chaque paquet (installation, licence) pour la publication PyPI, faite par
    l'utilisateur.
  — REQ-023-42 — verify : `nix build .#kartenn-cli` puis `result/bin/kartenn --version` ;
  `devenv test` vert
  - où : paquets de la 015 (`specs/015-paquetage-deploiement/`, ADR 0029)
  - dépend de : 27

## Différé

- Adaptateur HTTP/3 `kartenn[http3]` (T5) : quand le serveur ou son mandataire parlera
  HTTP/3 et qu'un banc montrera un gain ; ADR à ce moment.
- Image de conteneur de la CLI (Q-023-14) : après la première version publiée.

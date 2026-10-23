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

- [ ] 4. ETag fort de `GET /v1/openapi.json` (empreinte SHA-256 du document de la build,
  calculée une fois au démarrage), `Cache-Control: no-cache`, 304 sur `If-None-Match`.
  Amendement écrit dans `specs/006-api-rest/` (REQ-006-34 complétée). — T20 ;
  REQ-023-2 — verify : test REST (200 avec ETag, 304, ETag stable entre deux instances
  de même build)
  - où : route `openapi` de `crates/kartenn-rest` (`GET /v1/openapi.json`, 006/design
    § Versions et contrat) ; tests voisins de `crates/kartenn-rest/tests/it/` (cache et
    304 des lectures)
  - dépend de : —
- [ ] 5. Extension `x-kartenn`, produite par `kartenn_rest::kit` :
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
- [ ] 6. `GET /v1/auth/native-clients`, publique :
  `[{id, discovery_url, client_id, audience, scopes, device}]`, seulement pour les
  émetteurs configurés avec `native_client_id`. Nouveaux champs `native_client_id`,
  `native_scopes` et `native_device` de `[[auth.issuers]]`. Amendements de la 005 et de
  la 006. — T10 ; REQ-023-7 — verify : test REST sans session ; un émetteur sans
  `native_client_id` est absent ; aucun secret dans la réponse ; `issuers` inchangée
  (REQ-007-41)
  - où : route `auth/issuers` de `crates/kartenn-rest` (à imiter) ; configuration des
    émetteurs dans `crates/kartenn-auth` (005/design, bloc `[auth]`)
  - dépend de : —
- [ ] 7. `kartenn-dev-idp` :
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
- [ ] 12. Changesets et imports :
  - `ChangesetBuilder`, simulation, soumission (`Applied`, `Pending`, `Refused`),
    actions sous `If-Match`, différentiel paginé par section ;
  - `ImportSession` : morceaux, état local XDG, reprise par empreinte, rappel
    d'avancement.
  — REQ-023-13, 15 — verify : 202 → `Pending` ; reprise au morceau 41 ; 10⁶ éléments
  synthétiques en mémoire ≤ 100 Mio (transport simulé)
  - où : `clients/python/kartenn/src/kartenn/changesets.py`, `imports.py` ; 006/design
    § Imports et tâches de fond ; 017/decisions Q-017-1
  - dépend de : 11
- [ ] 13. Authentification : Basic, Bearer fourni, OIDC natif (bouclage `127.0.0.1` à
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
- [ ] 14. Événements : `events()` et `aevents()` sur `GET /v1/events`, filtres, reprise,
  doublons, `reset`, absence quand `capabilities.events` vaut `null`. — REQ-023-38 —
  verify : `MockTransport` en flux SSE (coupure puis reprise avec `Last-Event-ID`,
  doublon écarté, `reset` rendu, 429 puis `Retry-After`)
  - où : `clients/python/kartenn/src/kartenn/events.py` ; design § Événements ;
    022/design § Contrat consommé par les clients
  - dépend de : 9, 11
- [ ] 15. Stubs : `python -m kartenn stubs generate` (`TypedDict`, `Literal`,
  `Protocol`, surcharge de `Client.__getattr__`) — REQ-023-17 — verify : pyright signale
  un membre inconnu dans un script d'exemple ; les stubs ne sont jamais importés à
  l'exécution
  - où : `clients/python/kartenn/src/kartenn/stubs.py`, `__main__.py`
  - dépend de : 10

## Flux C — CLI `kartenn-cli`

- [ ] 16. Racine paresseuse :
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
- [ ] 17. Arbre dérivé :
  - groupes de modules, préfixe retiré, verbes, sous-ressources ;
  - aide tirée du contrat (module, opération HTTP) ;
  - module absent → code 2 et message de la maquette § 12.
  — REQ-023-18, 21 — verify : instantané de `--help` de chaque groupe sur le contrat de
  référence ; contrat sans `policy` ; module `demo`
  - où : `clients/python/kartenn-cli/src/kartenn_cli/tree.py` ; design § Nommage ;
    maquette § 2, 3, 12
  - dépend de : 10, 16
- [ ] 18. Lectures :
  - `--at`, `--as-of`, `--freeze` ;
  - filtres typés et `--prop` ;
  - `--fields`, `--query` (JMESPath), `--keys`, `--ids`, `--all`, `ndjson` en flux ;
  - références par clé naturelle (`x-kartenn.key`, UUID sinon).
  — REQ-023-22, 23, 25, 26, 27 — verify : maquette § 4 en instantané ; filtre inconnu
  refusé sans requête ; `ndjson --all` en mémoire constante
  - où : `clients/python/kartenn-cli/src/kartenn_cli/tree.py`, `render/` ; maquette § 4
  - dépend de : 11, 17
- [ ] 19. Complétion bash, zsh et fish (`shell_complete`) : catalogue, énumérations, clés
  et valeurs de propriétés, slugs ; cache de 5 min sans valeur masquée ; délai d'une
  seconde au plus — REQ-023-24 — verify : scripts générés ; complétions testées dans les
  trois shells (pexpect) ; serveur muet → propositions locales en moins d'une seconde
  - où : `clients/python/kartenn-cli/src/kartenn_cli/complete.py` ; maquette § 5
  - dépend de : 18
- [ ] 20. Écritures :
  - `create`, `update`, `close`, `edit` (fichier 0600, 412 → réouverture) ;
  - `--dry-run`, `--into-changeset`, `--require-applied` ;
  - `changeset new|add|simulate|apply|show|diff|list|approve|reject|withdraw|undo|proposals`
    et rendu des sections de module.
  — REQ-023-28 à 31 — verify : maquette § 8 et 9 en instantané ; fichier temporaire
  supprimé même sur erreur
  - où : `clients/python/kartenn-cli/src/kartenn_cli/edit.py`, `tree.py` ; design
    § Codes de sortie
  - dépend de : 12, 18
- [ ] 21. `import` : NDJSON, CSV et JSON par ressource (`--resource`, `--id-column`),
  avancement (rich sur terminal, lignes sinon), `--resume`, `--follow`, violations à la
  ligne source — REQ-023-32 — verify : maquette § 10 sur transport simulé ;
  `import.unknown-reference` avec `fichier:ligne`
  - où : `clients/python/kartenn-cli/src/kartenn_cli/importing.py` ; schéma
    `ImportElement` du contrat
  - dépend de : 20
- [ ] 22. Commandes nommées : `policy check` (`--exit-status`), `policy matrix show|set`,
  `policy report`, `flow request|review|waive`. Table `named.py` liée aux `operationId`,
  masquée si une opération manque. — REQ-023-20 — verify : maquette § 6 et 7 ; contrat
  sans `flow` → commandes absentes
  - où : `clients/python/kartenn-cli/src/kartenn_cli/named.py` ; design § Commandes
    nommées ; 017/design § Requête de conformité
  - dépend de : 20
- [ ] 23. `kartenn api` : méthodes dont `QUERY`, `-f`, `--data @fichier|-`, `-H`,
  `--paginate` — REQ-023-19 — verify : maquette § 11
  - où : `clients/python/kartenn-cli/src/kartenn_cli/main.py`
  - dépend de : 18
- [ ] 24. `login` (bouclage, `--device`, `--breakglass`), `logout`, `me`, et `watch`
  (filtres, `--since`, `ndjson`). `--follow` d'un import ou d'un changeset sur le flux
  d'événements quand il existe. — REQ-023-7, 8, 38 — verify : maquette § 1 et 15 sur
  transport simulé ; `watch` absent si `events` vaut `null`
  - où : `clients/python/kartenn-cli/src/kartenn_cli/main.py`, `render/` ; design § Suivi
  - dépend de : 13, 14, 16
- [ ] 25. Couverture : `coverage.py` (exclusions motivées) et test qui classe chaque
  opération du contrat de référence — REQ-023-43 — verify : une opération ajoutée à un
  contrat de test et non classée fait échouer le test
  - où : design § Couverture de l'API
  - dépend de : 22, 23, 24

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

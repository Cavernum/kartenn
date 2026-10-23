# 023 — Bibliothèque cliente Python et CLI : journal des décisions

Brouillon du 2026-10-23, **sans entretien**. Ce journal suit la demande de l'utilisateur
du même jour : « Tu peux aussi commencer le design de la CLI, je veux aussi un genre de
maquette et la possibilité de réaliser la plupart des appels de l'API depuis la CLI.
Enfin, et je pense que tu peux l'utiliser pour créer la CLI, une librairie/bindings python
pour interagir avec Kartenn. N'oublie pas de supporter les plugins type politique de
sécurité etc automatiquement selon ce qui est disponible sur le serveur. »

Chaque décision ci-dessous est une **proposition** qui retient la recommandation. Les
choix structurants (approche, dépendances, licence, authentification, amendements du
serveur, écart à D31) attendent l'arbitrage de l'utilisateur, en fin de journal
(A1 à A8). Les choix peu coûteux à défaire sont des questions avec une recommandation
(Q-023-1 à 14).

Cadre : D7 (REST, frontière unique ; endpoint de capacités ; « Le CLI et le MCP peuvent
partager un SDK client généré depuis l'OpenAPI »), D10, D13, D16, D31, D33, D37, Q-ouv-1,
Q-ouv-6 ([`../001-cadrage/decisions.md`](../001-cadrage/decisions.md)). 004 (changesets).
005/K1, K8 (Bearer des clients non navigateur, secrets des comptes de service). 006/H8,
H12 et ADR 0022 (contrat HTTP). ADR 0026, clause 9 (conventions REST des modules). 017/U1
(un module = un plugin, publié selon les capacités). Retour de l'utilisateur sur les
maquettes du 2026-10-22 (filtres typés, clé et libellé, codes visibles).

Surface REST de référence : `specs/006-api-rest/openapi-v1.json` au 2026-10-23. Il
contient 279 opérations, dont 25 collections à `QUERY` (extension `x-query`). Les tags
sont `core`, `properties`, `changesets`, `imports`, `rights`, `audit`, `authentication`,
`operations`, `graph`, `policy`, `flow`, `vlan` et `machines`.

## Synthèse
- T1 périmètre : bibliothèque `kartenn` et CLI `kartenn`, clientes REST seulement ;
  démarrage anticipé sur D31 (A6).
- T2 bibliothèque en Python pur, pas de bindings (A1).
- T3 modules découverts à l'exécution (capacités et contrat), sans liste figée.
- T4 accès dynamique, stubs de types générés à la demande ; pas de modèles générés à la
  construction.
- T5 HTTP : httpx derrière un port de transport ; client synchrone et asynchrone (A2).
- T6 Python ≥ 3.11.
- T7 paquetage : `clients/python/`, espace de travail uv, wheels purs, paquet Nix.
- T8 licence MPL-2.0 pour les deux (A3).
- T9 compatibilité par `api_versions` et présence des opérations, versions
  indépendantes.
- T10 humains : OIDC natif (code et PKCE, bouclage ; appareil en option), sans jeton émis
  par kartenn (A4).
- T11 machines : Basic de compte de service ; secours par session cookie.
- T12 CLI sur Click 8, groupes paresseux (A2).
- T13 arbre dérivé du contrat ; un groupe par module ; commandes nommées par table ;
  commande `api` générique (A8).
- T14 sorties, dates, filtres, complétion.
- T15 clé et libellé.
- T16 problèmes et codes de sortie.
- T17 écritures par changeset ; `--dry-run` = simulation ; édition sous `If-Match`.
- T18 imports NDJSON natifs et CSV/JSON par ressource.
- T19 profils XDG, secrets au trousseau, pas de secret en argument.
- T20 cache du catalogue par serveur et révision (A5 : ETag du contrat).
- T21 i18n par catalogues FR et EN.
- T22 `watch` dépend de la 022.
- T23 tests contre un vrai serveur (gabarit 013/014), émetteur de développement étendu.
- T24 budgets chiffrés de démarrage et de complétion.

## T1 — Périmètre et calendrier
- Décision proposée : deux paquets clients, la bibliothèque `kartenn` et la CLI
  `kartenn-cli` (commande `kartenn`), qui ne parlent qu'à l'API REST (D7).
- Pourquoi : la demande de l'utilisateur ; une seule surface publique ; la CLI est le
  premier client de la bibliothèque, ce qui l'éprouve.
- Écart : D31 place la CLI « ensuite » (après la v0.3). L'utilisateur choisit d'en
  commencer la conception maintenant ; l'implémentation peut suivre la v0.2 sans la
  bloquer, puisqu'elle ne touche le serveur que par trois amendements additifs (A5).
  À confirmer (A6).
- Décidé par : à arbitrer (A6)

## T2 — Bibliothèque ou bindings
- Constat : « librairie » et « bindings » désignent deux choses.
  - Une **bibliothèque** est un paquet Python qui parle HTTP au serveur. Elle est écrite
    en Python, ou produite par un générateur depuis l'OpenAPI.
  - Des **bindings** enveloppent une bibliothèque native : ici une crate Rust cliente,
    exposée à Python par PyO3 et construite par maturin.
  Dans les deux cas, le serveur reste atteint par REST (D7). Les bindings ne donnent
  aucun accès plus direct.
- Options :
  - (a) **client généré à la construction** depuis l'OpenAPI (openapi-python-client,
    attrs et httpx). Il est typé, mais figé à la build du serveur qui l'a produit : un
    module activé plus tard demande un nouveau client. Il ne connaît pas `QUERY`
    (`x-query`), les curseurs, les changesets ni les imports, qu'il faut écrire à côté.
    Le générateur reste en 0.x et épingle `httpx<0.29` ;
  - (b) **cœur écrit à la main** (HTTP, authentification, reprise, pagination, problèmes,
    changesets, imports, dates) et **découverte à l'exécution** des ressources par les
    capacités et le contrat, plus des stubs de types générés à la demande pour un serveur
    donné (T4) ;
  - (c) **bindings PyO3** sur une crate cliente Rust. Ils demandent une matrice de wheels
    (manylinux, musllinux, macOS, deux architectures ; abi3, plus abi3t pour Python
    3.15 et les builds sans GIL). Ils n'apportent aucun gain de performance pour un client
    limité par le réseau, et la crate devrait faire la même découverte qu'en (b). Un
    scripteur Python ne pourrait plus lire ni corriger le client.
- Recommandation : **(b)**. C'est la seule option qui satisfait la demande
  (« automatiquement selon ce qui est disponible sur le serveur ») sans nouvelle version
  du client. Le cœur fait une dizaine de modules ; les ressources, de loin la plus grande
  surface, ne demandent aucun code.
- Écart de lettre avec D7 : « un SDK client **généré** depuis l'OpenAPI ». En (b), le
  client est dérivé du contrat **à l'exécution** plutôt qu'au build. L'intention de D7
  (un seul contrat, pas de surface parallèle) est tenue, et D7 dit « peuvent ». À
  confirmer (A1).
- Écart apparent avec D16 : Python y est rejeté **pour le serveur** (« marge de
  performance plus faible »). Un client n'est pas concerné. Python est choisi ici pour
  l'écosystème des scripts d'exploitation et de sécurité, public premier de la
  bibliothèque.
- Décidé par : à arbitrer (A1)

## T3 — Découverte des modules
- Décision proposée : au premier contact avec un serveur, la bibliothèque lit
  `GET /v1/capabilities` (version, `api_versions`, `modules`, propriétés déclarées,
  limites) et `GET /v1/openapi.json`. Elle en dérive un **catalogue** compact :
  - ressources : premier segment après `/v1`, module = tag de l'opération ;
  - opérations : méthode et forme du chemin ;
  - paramètres, énumérations, schémas de corps ;
  - `QUERY` par `x-query`.
  La dérivation repose sur les conventions que l'ADR 0026 (clause 9) impose à tout
  module : collections au pluriel en kebab-case, objet par UUID, `GET`/`POST` sur la
  collection, `GET`/`PATCH`/`DELETE` sur l'objet, `history` et `properties` fournis par
  le noyau, actions en `POST` de sous-ressource.
- Extension facultative du contrat (A5) : `x-kartenn` sur une opération ou une
  collection. Elle porte la clé naturelle (`["domain", "value"]`), un nom de commande
  quand la convention ne suffit pas, et les colonnes par défaut d'une table. Elle est
  fournie par le kit REST (`kartenn_rest::kit`), donc reçue par tout module sans effort.
  Sans elle, la CLI marche par convention, avec des UUID et toutes les colonnes de
  premier niveau.
- Pourquoi : un module livré ou activé côté serveur (build par features, D6, ADR 0026)
  apparaît dans le client sans version nouvelle.
- Rejeté : une liste de modules connue du client (contraire à la demande) ; un endpoint
  de description propre à la CLI (seconde surface à tenir à jour, alors que le contrat
  existe et est testé, REQ-006-34).

## T4 — Typage
- Décision proposée : les objets rendus sont des `dict` enveloppés (`Record`) qui gardent
  l'ETag, les liens et l'`as_of` effectif. `kartenn stubs generate` écrit, pour le serveur
  du profil, un paquet de types (`.pyi`, `TypedDict`, `Literal` des énumérations,
  `Protocol` des ressources). Avec ces types, `client.vlans.create(...)` se complète et se
  vérifie dans l'éditeur (pyright, mypy).
- Pourquoi : un typage réel sans figer le client ; aucun coût à l'exécution.
- Rejeté : pydantic à l'exécution (import lent, environ 100 ms, et validation en double
  de celle du serveur) ; modèles générés à la construction (option a de T2).

## T5 — Transport HTTP
- Décision proposée : httpx 0.28 (BSD-3), derrière un **port de transport** interne (une
  classe, deux méthodes, synchrone et asynchrone). Un client synchrone `Client` et un
  client asynchrone `AsyncClient` partagent le cœur, sans entrées-sorties : construction
  des requêtes, lecture des problèmes, pagination.
- Constat (2026-10-23) : httpx n'a pas publié de version stable depuis décembre 2024 ;
  1.0 est en pré-version, annoncée sans rupture. niquests (Apache-2.0, actif) est
  l'alternative ; son import est plus lent (environ 89 ms contre 65 ms).
- Pourquoi : httpx offre synchrone et asynchrone par la même API, HTTP/2 et les
  mandataires standard. Le port borne le coût d'un changement de client HTTP à un seul
  fichier.
- Décidé par : à arbitrer (A2, dépendance)

## T6 — Version de Python
- Décision proposée : **3.11** au minimum. Python 3.10 est en fin de vie depuis octobre
  2026 ; 3.11 est maintenu (correctifs de sécurité) jusqu'en octobre 2027. La version 3.11
  apporte `tomllib` (lecture de la configuration sans dépendance) et un démarrage plus
  rapide.
- Source : https://devguide.python.org/versions/ (lu le 2026-10-23).
- Voir Q-023-4 (relever le plancher à 3.12).

## T7 — Paquetage
- Décision proposée :
  - emplacement : `clients/python/`, un espace de travail uv qui contient deux membres,
    `kartenn` (bibliothèque) et `kartenn-cli` (CLI, commande `kartenn`) ;
  - construction : `uv_build`, épinglé à une version mineure (uv est en 0.x) ; wheels
    `py3-none-any` ;
  - Nix : paquet du flake (`buildPythonApplication`) à côté de ceux de la 015 ;
  - devenv : Python et uv ajoutés, avec un script `k-py-check` (ruff, pyright, pytest) ;
  - PyPI : publication à la première version stable (Q-023-2).
- Pourquoi : la CLI tire ses dépendances d'interface ; un script qui n'importe que la
  bibliothèque n'en hérite pas. `clients/` est un répertoire de premier niveau nouveau,
  à côté de `crates/` et `web/` (D32).

## T8 — Licence
- Décision proposée : **MPL-2.0** pour la bibliothèque et pour la CLI.
- Pourquoi : Q-ouv-1 (close le 2026-10-19) retient MPL-2.0 pour le SDK de plugins et
  « les clients générés ». La bibliothèque est un tel client : des scripts et connecteurs
  propriétaires doivent pouvoir l'importer, comme pour le SDK web
  (`web/sdk/package.json`). La CLI pourrait rester en AGPL-3.0 (D37), mais elle ne sert
  rien sur un réseau. L'AGPL n'y ajouterait que de la friction pour les distributions
  internes.
- Décidé par : à arbitrer (A3)

## T9 — Versions et compatibilité
- Décision proposée : versions sémantiques indépendantes de celles du serveur. À la
  connexion, la bibliothèque exige `v1` dans `api_versions` (sinon
  `client.api-unsupported`). Elle active chaque fonction du socle selon la présence de son
  opération dans le contrat (`QUERY` d'une collection, `diff` paginé, imports), jamais
  selon un numéro de version. L'évolution additive de `/v1` (006/H12) garantit qu'un
  client ancien marche contre un serveur récent. Un client récent contre un serveur
  ancien rend `client.operation-unavailable` pour ce qui manque.
- Pourquoi : pas de matrice de compatibilité à tenir ; la même logique sert aux modules.
- Avertissement de dépréciation : les en-têtes `Deprecation` et `Sunset` (006/H12) sont
  affichés une fois par jour et par opération sur la sortie d'erreur.

## T10 — Authentification des humains
- Constat : le navigateur passe par le BFF (005/K1), une session cookie dont aucun jeton
  n'est lisible. Les clients non navigateur présentent un jeton d'accès Bearer d'un
  émetteur configuré (REQ-005-9, `at+jwt`, audience de kartenn). kartenn n'émet aucun
  jeton.
- Options :
  - (A) **OIDC natif** : la CLI est un client public de l'émetteur et obtient elle-même
    le jeton par le flux par code avec PKCE, redirigé vers une adresse de bouclage
    (RFC 8252). Dans un environnement sans navigateur, elle utilise le flux
    d'autorisation d'appareil (RFC 8628) quand l'émetteur l'offre ;
  - (B) **jeton émis par l'interface web** : l'utilisateur crée dans le web un jeton
    personnel à durée bornée, que kartenn émet et vérifie. Cela fait de kartenn un
    émetteur de jetons, avec une nouvelle table de secrets, une révocation et un audit,
    et contourne la MFA de l'émetteur au moment de l'usage ;
  - (C) le flux d'appareil seul.
- Recommandation : **(A)**. kartenn reste sans émission de jetons (005), et la MFA comme
  les groupes restent ceux de l'émetteur.
  - Bouclage par défaut : redirection `http://127.0.0.1:<port>/callback`, avec un port
    choisi à l'exécution (RFC 8252 §7.3 : le serveur « MUST allow any port » ; §8.3 :
    `localhost` « NOT RECOMMENDED »).
  - PKCE `S256` obligatoire (RFC 8252 §6 ; RFC 7636 §4.2 ; RFC 9700 §2.1.1 : « Public
    clients MUST use PKCE »).
  - Navigateur externe seulement (RFC 8252 §8.12 : « MUST NOT use embedded
    user-agents »).
  - `--device` pour un hôte sans navigateur (RFC 8628 §3.1 à 3.5). Avant la saisie du
    code, la CLI rappelle que l'on autorise un appareil (RFC 8628 §5.4, phishing à
    distance : « RECOMMENDED to inform the user »).
  - Rafraîchissement : un jeton de rafraîchissement de client public doit être lié à
    l'expéditeur ou tourner (RFC 9700 §2.2.2) ; on exige la **rotation** côté émetteur,
    documentée dans la note d'exploitation.
- Conséquence pour le serveur (A5) : la CLI a besoin, par émetteur, de l'URL de
  découverte et de l'identifiant du client public natif. `GET /v1/auth/issuers` ne publie
  que l'identifiant local, sans URL ni client (REQ-007-41, pour la page de connexion).
  Deux voies sont possibles :
  - le profil porte ces valeurs, distribuées par l'administrateur ;
  - une route publique additive `GET /v1/auth/native-clients` les sert, seulement pour
    les émetteurs où l'administrateur a configuré un `native_client_id` (valeurs
    publiques par nature).
  Recommandation : la route, qui évite de distribuer la configuration à la main.
- Note de référence à rédiger dans `docs/reference/` (RFC 8252, 8628, 9700, 7636), à
  partir du brouillon de recherche du 2026-10-23 : citations à recouper mot pour mot,
  sections 5.2 et 5.3 de RFC 8628 et 4.14 de RFC 9700 à confirmer.
- Décidé par : à arbitrer (A4, A5)

## T11 — Machines et secours
- Décision proposée : compte de service en Basic (RFC 7617, 005/K8). Le secret vient du
  trousseau, de `KARTENN_SECRET_FILE` ou de l'entrée standard (`--secret-stdin`), jamais
  d'un argument (visible dans `ps`). Le secours (`kartenn login --breakglass`) demande
  la phrase de passe et le code TOTP sans écho. La session cookie d'au plus 1 h reste en
  mémoire du processus, ou dans `$XDG_RUNTIME_DIR` (0600) si l'utilisateur le demande
  (`--keep`). Chaque écriture porte `Origin` (contrôle CSRF de la 006).
- Pourquoi : le secours sert quand l'émetteur est indisponible ; un administrateur doit
  pouvoir l'utiliser sans navigateur.

## T12 — Cadre de la CLI
- Options :
  - **Click 8.5** (BSD-3) : groupes paresseux (`list_commands`, `get_command`),
    complétion personnalisée par paramètre (`shell_complete`) pour bash, zsh et fish ;
    import environ 13 ms ;
  - **Typer** (MIT, 0.x) : construit sur des annotations de fonctions, pensé pour des
    commandes écrites à la main ; il embarque désormais sa copie de Click et tire
    toujours rich ; import environ 29 ms ;
  - **cyclopts** (Apache-2.0) : moderne, même modèle par annotations, rich obligatoire ;
    import environ 35 ms.
- Recommandation : **Click**. Les commandes sont créées à l'exécution depuis le catalogue,
  ce que Click fait nativement (objets `Command` construits par programme) et que Typer
  ou cyclopts font mal. C'est aussi le plus léger au démarrage. rich (MIT) est importé
  seulement au rendu d'une table ou d'une barre d'avancement.
- Mesures : `python -X importtime`, Python 3.14, 2026-10-23, cache chaud, meilleur de
  5 essais.
- Décidé par : à arbitrer (A2, dépendance)

## T13 — Arbre de commandes
- Décision proposée :
  - premier niveau : les ressources du noyau et du socle (`domains`, `entities`, `sites`,
    `prefixes`, `ranges`, `addresses`, `anomalies`, `changeset`, `import`, `roles`,
    `bindings`, `identities`, `service-accounts`, `audit`, `me`, `capabilities`, `api`,
    `login`, `logout`, `config`, `stubs`, `completion`) ;
  - un groupe par module servi, nommé comme le module (`policy`, `flow`, `vlan`,
    `machines`). Ses collections perdent le préfixe du module (`policy-rules` →
    `policy rules`, `vlan-groups` → `vlan groups`). La collection qui porte le nom du
    module rattache ses verbes au groupe lui-même (`/v1/flows` → `kartenn flow list`,
    `/v1/vlans` → `kartenn vlan list`) ;
  - verbes dérivés : `list`, `get`, `create`, `update`, `close`, `edit`, `history`,
    `properties [explain|suggest] <clé>` ; sous-ressources en sous-commandes
    (`prefixes occupancy`, `prefixes next-free`) ; une action `POST` de sous-ressource
    devient un verbe (`/v1/anomalies/{id}/acknowledgement` → `anomalies acknowledge`) ;
  - les sous-ressources qu'un module ajoute à une ressource du noyau
    (`/v1/addresses/{id}/flows`) n'apparaissent que si le module est servi ;
  - commandes nommées : une table **dans la CLI** lie un nom à un `operationId`
    (`policy check` → `show_verdicts`, `flow request` → `request_flow`, `flow waive` →
    `waive_flow`). La commande est masquée si l'opération manque. Elles ajoutent un
    rendu propre (verdict, explication) ;
  - `kartenn api` atteint tout le reste.
- Pourquoi : les modules sont visibles comme tels (la demande parle de « plugins ») et
  l'aide reste lisible avec 279 opérations. `close` et non `delete`, car une clôture ne
  supprime rien (D33, D10).
- Rejeté : un niveau plat calqué sur l'API (`kartenn policy-rules list`), plus simple mais
  qui mélange 40 collections au premier niveau.
- Décidé par : à arbitrer (A8, contrat des scripts)

## T14 — Lectures : dates, filtres, sorties, complétion
- Dates : `--at` (`valid_at`) et `--as-of`. Formes acceptées : RFC 3339, date seule
  (minuit dans le fuseau du profil, envoyé avec son décalage) ou durée relative
  (`-7d`, `+1M`). La CLI affiche l'`as_of` effectif en pied de table ; `--freeze` réutilise
  le dernier pour une suite de commandes (variable `KARTENN_AS_OF`).
- Filtres : options typées tirées des paramètres du contrat, `--prop clé[:op]=valeur`
  (`:between=60..3600`, `:exists=true`) ; `QUERY` automatique au-delà de 8 Kio.
- Sorties : `table` (rich, paresseux), `json`, `ndjson` (une ligne par élément, en flux),
  `yaml` (PyYAML, `safe_dump`), `csv` ; `--fields a,b.c` ; `--query` en JMESPath
  (jmespath, MIT, pur Python).
- Complétion : scripts générés par `kartenn completion bash|zsh|fish`. Sources des
  propositions :
  - catalogue local (commandes, options, énumérations) ;
  - capacités (clés de propriétés) ;
  - serveur, pour les valeurs à liste fermée et les slugs, gardées 5 min en cache.
  Une interrogation du serveur pour une complétion a un délai d'une seconde au plus, et
  aucune valeur masquée n'est mise en cache.

## T15 — Clé et libellé
- Décision proposée : en table, la CLI montre le libellé que l'API rend (`label`,
  `rule_label`…) et sinon la clé ; `--keys` force les clés ; les formats de données
  rendent toujours les clés. Les codes ne sont jamais traduits ni masqués.
- Constat : l'API rend aujourd'hui peu de libellés (rôles, règle d'une non-conformité).
  Le retour du 2026-10-22 demande un libellé par langue avec repli sur la clé. Ce
  libellé relève du serveur et sera partagé avec la refonte du web (Q-023-3).

## T16 — Problèmes et codes de sortie
- Décision proposée : rendu en tête du code, puis du titre, du détail, de l'identifiant
  de requête et des violations. Codes de sortie stables (design, § Codes de sortie) :
  - 0 succès ;
  - 1 erreur inattendue ;
  - 2 usage (Click), y compris un module ou une opération absents ;
  - 3 introuvable ;
  - 4 authentification ou droit ;
  - 5 conflit ou version périmée ;
  - 6 refus de validation (422) ;
  - 7 serveur indisponible après les reprises ;
  - 8 simulation qui aurait échoué ;
  - 9 verdict refusé (`policy check --exit-status`) ;
  - 10 en attente d'approbation (avec `--require-applied`).
- Pourquoi : les scripts distinguent « réessayer », « corriger », « demander un droit »
  et « attendre ».

## T17 — Écritures
- Décision proposée :
  - une commande d'écriture produit **un** changeset, comme la route par ressource de la
    006 ;
  - `--motive`, `--valid-from` et `--on-behalf-of` sont communs à toutes ;
  - `--dry-run` soumet avec `?simulate=true` et affiche le différentiel complet ;
  - `edit` passe par l'éditeur sous `If-Match` ;
  - `--yes` est requis hors terminal pour une clôture en cascade ;
  - `Idempotency-Key` est générée et affichée en `--verbose` pour un rejeu manuel ;
  - `kartenn changeset new/add/simulate/apply` compose un changeset de plusieurs
    opérations dans un fichier local (`.kartenn-changeset.yaml`), comme l'« ensemble en
    préparation » du plugin `policy` (017/Q-017-1).

## T18 — Imports
- Décision proposée : un import de la 006 reçoit des `ImportElement` (opération et
  `external_id`), pas des lignes de tableau. La CLI accepte donc :
  - du NDJSON d'éléments, tel quel ;
  - un CSV ou un JSON d'objets d'une ressource, traduit en opérations `Create…` par le
    schéma de création de la collection (en-têtes = membres ; `prop.<clé>` = propriétés ;
    `external_id` = colonne désignée par `--id-column`).
  La lecture se fait en flux. Les morceaux sont de `chunk_elements` éléments au plus, et
  un `PUT` déposé deux fois à l'identique reste idempotent (006/H9). L'avancement
  affiche les morceaux, les éléments, le débit et le temps restant. Chaque violation d'un
  import refusé est rapportée à la ligne du fichier source par l'index de l'élément.

## T19 — Profils, secrets, environnement
- Décision proposée :
  - `$XDG_CONFIG_HOME/kartenn/config.toml` (profils) ;
  - `$XDG_CACHE_HOME/kartenn/` (catalogues, complétion) ;
  - `$XDG_STATE_HOME/kartenn/` (historique des imports à reprendre) ;
  - défauts XDG : `~/.config`, `~/.cache`, `~/.local/state` ;
  - jetons et secrets au trousseau (keyring, MIT), clé `kartenn:<profil>`.
  Sans trousseau utilisable (hôte sans session D-Bus), la CLI ne stocke rien : elle
  demande `KARTENN_TOKEN`, `KARTENN_SECRET_FILE` ou une reconnexion (Q-023-7). Le paquet
  d'autorités vient de `ca_bundle` dans le profil ; les mandataires suivent les variables
  standard.
- Source XDG : https://specifications.freedesktop.org/basedir/latest/ (v0.8).

## T20 — Cache du catalogue
- Décision proposée : la clé du cache est l'URL, la `version` des capacités, les versions
  des modules et l'ETag de `/v1/openapi.json` (A5, amendement additif de la 006 : ETag
  fort = empreinte du document de la build). Le cache stocke le catalogue **dérivé**, en
  JSON compact (une fraction du contrat), pas le contrat brut. Une invocation le lit en
  quelques millisecondes ; elle relit les capacités au plus une fois par minute et par
  profil (`If-None-Match`).
- Sans ETag servi, la clé est la version et les modules : deux builds de même version et
  mêmes modules servent le même contrat.

## T21 — Langue
- Décision proposée : catalogues de messages par clés (TOML, un par langue, français et
  anglais), chargés à la demande ; même résolution de langue que le web (Q-ouv-6). Les
  `title` et `detail` des problèmes viennent du serveur ; la CLI ne traduit que ses
  propres textes et les noms de classes de problèmes qu'elle connaît.

## T22 — Suivi d'événements
- Décision proposée : `kartenn watch [changesets|imports|flows]` s'appuie sur la spec
  022 (SSE, brouillon en cours) et n'existe que si son opération figure au contrat. En
  attendant, `--follow` interroge l'état (import : selon `Retry-After`, 5 s en tri ;
  changeset en attente : 10 s, plafonné).

## T23 — Tests
- Décision proposée :
  - tests unitaires pytest, transport simulé (`httpx.MockTransport`, sans dépendance) ;
  - test de dérivation sur le contrat de référence (`specs/006-api-rest/openapi-v1.json`) :
    arbre de commandes en instantané, couverture (REQ-023-43) ;
  - parcours de bout en bout contre un vrai `kartenn-server`, base `kartenn_cli` recréée
    comme `web/e2e/serve.sh` (sous `k-heavy --pg`), émetteur `kartenn-dev-idp` étendu d'un
    client public natif, du bouclage et du flux d'appareil.

## T24 — Budgets
- Décision proposée : budgets de REQ-023-39. Banc `bench-cli` (hyperfine, ou une boucle
  `time` sous `k-heavy`) ; import des modules contrôlé par un test qui échoue si
  `kartenn --version` importe httpx ou rich.

## Arbitrages demandés

| # | Sujet | Recommandation | Contredit ou complète |
|---|---|---|---|
| A1 | Approche de la bibliothèque : (a) générée au build, (b) cœur et découverte à l'exécution, (c) bindings PyO3 | **(b)** | D7 dit « SDK généré depuis l'OpenAPI » : dérivé à l'exécution plutôt qu'au build (T2) |
| A2 | Dépendances : httpx (derrière un port), click, rich (paresseux), keyring, platformdirs, PyYAML, jmespath ; outillage uv, ruff, pyright, pytest ; Python et uv dans devenv | les retenir | nouvelles dépendances et nouvel outillage (CLAUDE.md : choix structurant) |
| A3 | Licence de la bibliothèque et de la CLI | **MPL-2.0** pour les deux | complète Q-ouv-1 ; écart à D37 (AGPL) pour la CLI |
| A4 | Authentification des humains : (A) OIDC natif, bouclage et appareil, (B) jeton émis par le web, (C) appareil seul | **(A)** | complète D13 et 005/K1 ; demande un client public par émetteur |
| A5 | Amendements additifs du serveur : ETag de `/v1/openapi.json` ; extension `x-kartenn` (clé naturelle, nom, colonnes) par le kit REST ; `GET /v1/auth/native-clients` | les trois, dans une tâche de la 006 | REQ-007-41 (identifiants seuls) reste vrai pour `issuers` |
| A6 | Démarrer la conception (et l'implémentation ?) de la CLI avant la v0.3 | conception maintenant ; implémentation après la v0.2 | D31 (CLI « ensuite ») |
| A7 | Emplacement `clients/python/` dans le monorepo | oui | complète D32 |
| A8 | Forme de l'arbre : un groupe par module, préfixe retiré, commandes nommées | oui | contrat des scripts : coûteux à changer après publication |

## Questions ouvertes

- [ ] Q-023-1 — Nom de la commande. `kn` est déjà la CLI d'un projet d'orchestration
  répandu, `kt` celle d'un outil de messagerie. Recommandation : `kartenn` seul ;
  l'utilisateur crée l'alias qu'il veut.
- [ ] Q-023-2 — Nom sur PyPI et date de publication. Recommandation : vérifier et
  réserver `kartenn` et `kartenn-cli` dès la validation ; publier à la première version
  stable, pas avant.
- [ ] Q-023-3 — Libellés par langue servis par l'API (catalogues de valeurs, règles,
  classes de problèmes). Recommandation : à traiter avec la refonte du web (v0.2) ; la CLI
  montre les clés en attendant.
- [ ] Q-023-4 — Plancher Python 3.11 ou 3.12. Recommandation : 3.11 (support jusqu'en
  octobre 2027) ; relever à 3.12 à la première version après cette date.
- [ ] Q-023-5 — Client asynchrone dès la première version. Recommandation : oui (le MCP
  et les connecteurs en auront besoin) ; le cœur sans entrées-sorties le rend peu
  coûteux.
- [ ] Q-023-6 — Forme des stubs : paquet de types installé dans l'environnement
  (`kartenn-stubs-<profil>`) ou répertoire `typings/` du projet. Recommandation : le
  répertoire, plus simple, sans installation.
- [ ] Q-023-7 — Sans trousseau (hôte sans session D-Bus) : refuser de stocker, ou
  fichier 0600. Recommandation : refuser par défaut, avec `--token-file <chemin>`
  explicite pour qui l'assume.
- [ ] Q-023-8 — Code de sortie d'une écriture mise en attente (202). Recommandation : 0
  avec un message « en attente d'approbation » ; 10 avec `--require-applied`.
- [ ] Q-023-9 — Clé naturelle dans les références. Recommandation : oui, déclarée par
  `x-kartenn` (A5) ; UUID seul pour une ressource qui n'en déclare pas.
- [ ] Q-023-10 — Projection : JMESPath (`--query`, pur Python) ou jq (binding natif ou
  binaire externe). Recommandation : JMESPath intégré ; `--output json | jq` reste
  possible.
- [ ] Q-023-11 — Télémétrie : `traceparent` seul, aucune remontée. Recommandation : oui.
- [ ] Q-023-12 — Secours depuis la CLI. Recommandation : oui, avec avertissement et
  session gardée en mémoire par défaut.
- [ ] Q-023-13 — La bibliothèque comme base du futur MCP. Recommandation : oui, à
  confirmer dans la spec MCP.
- [ ] Q-023-14 — Image de conteneur de la CLI (pour la CI des utilisateurs).
  Recommandation : oui, dérivée du paquet Nix comme l'image du serveur (015), après la
  première version stable.

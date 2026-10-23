---
status: draft
updated: 2026-10-23
---

# 023 — Bibliothèque cliente Python et CLI : conception

## Vue d'ensemble

Deux paquets Python dans `clients/python/` (espace de travail uv) :

- `kartenn` (bibliothèque, MPL-2.0) : un cœur sans entrées-sorties, deux clients
  (synchrone et asynchrone) sur un port de transport httpx, et un **catalogue** dérivé des
  capacités et du contrat du serveur ;
- `kartenn-cli` (commande `kartenn`, MPL-2.0) : un arbre Click construit paresseusement
  depuis le catalogue, des commandes nommées par une table, des rendus et une
  configuration.

> Décision centrale : aucune ressource de module n'est connue du client. Le catalogue est
> **dérivé à l'exécution** du contrat OpenAPI servi (`/v1/openapi.json`) et des
> capacités (`/v1/capabilities`), selon les conventions REST imposées à tout module
> (ADR 0026, clause 9). Il est gardé en cache par révision du serveur. Un module livré
> côté serveur apparaît dans la bibliothèque et dans la CLI sans nouvelle version
> (T2, T3).

```mermaid
flowchart LR
  subgraph cli[kartenn-cli]
    R[racine Click paresseuse] --> T[table des commandes nommées]
    R --> D[commandes dérivées]
    R --> A[kartenn api]
    D --> O[rendus : table, json, yaml, csv, ndjson]
  end
  subgraph lib[kartenn]
    C[Client / AsyncClient] --> K[Catalogue]
    C --> CS[Changesets, imports, pagination, problèmes]
    C --> AU[Auth : Basic, Bearer OIDC, secours]
    C --> P[Port de transport httpx]
    K --> CA[(cache XDG : catalogue dérivé)]
  end
  cli --> lib
  P -->|HTTPS /v1| S[kartenn-server]
  AU -->|OIDC natif| I[Émetteur OIDC]
```

## Changements

### Disposition (T7)

```
clients/python/
  pyproject.toml            # espace de travail uv, outils (ruff, pyright, pytest)
  uv.lock
  kartenn/                  # bibliothèque
    src/kartenn/
      __init__.py           # Client, AsyncClient, exceptions publiques
      _core/                # sans entrées-sorties
        requests.py         # construction des requêtes (dates, filtres, QUERY, en-têtes)
        problems.py         # Problem → exceptions, violations
        pages.py            # curseurs, links.next
        dates.py            # RFC 3339, dates seules, relatives
      transport.py          # port Transport/AsyncTransport ; adaptateur httpx
      retry.py              # 429/503 et Retry-After, idempotence
      catalog/
        derive.py           # contrat + capacités → Catalogue
        model.py            # Resource, Operation, Param, Schema (dataclasses gelées)
        cache.py            # XDG, clé de révision, écriture atomique 0600
      resources.py          # Resource dynamique, Record (dict + etag + links)
      changesets.py         # ChangesetBuilder, simulate, submit, actions
      imports.py            # ImportSession : morceaux, reprise, avancement
      auth/
        basic.py  bearer.py  oidc_native.py  device.py  breakglass.py  store.py
      stubs.py              # génération des .pyi
    tests/
  kartenn-cli/
    src/kartenn_cli/
      main.py               # racine Click, chargement paresseux
      tree.py               # catalogue → commandes Click
      named.py              # table des commandes nommées (policy check, flow request…)
      render/               # table (rich), json, ndjson, yaml, csv, problèmes
      edit.py               # $EDITOR, Merge Patch, 412
      importing.py          # CSV/JSON → ImportElement, avancement
      complete.py           # complétion, cache de valeurs
      config.py             # profils TOML, trousseau
      i18n/  fr.toml  en.toml
    tests/
```

### Bibliothèque

#### Clients (REQ-023-4, 5, 9 à 14)

```python
from kartenn import Client

with Client.from_profile("prod") as kn:          # ou Client(url, auth=…, ca_bundle=…)
    at = kn.at(valid_at="2026-11-01T00:00:00+01:00")   # vue datée, immuable
    for p in at.prefixes.list(domain="interne", within="192.0.2.0/24"):
        print(p["value"], p.etag)
    vlan = kn.vlans.get("7c1e…")                   # Record : dict + etag + links
    kn.vlans.update(vlan, {"name": "admin"}, motive="renommage")   # If-Match = vlan.etag
    cs = kn.changeset(motive="ouverture du lot 3", valid_from="2026-11-01")
    cs.create("prefixes", domain={"id": d}, value="198.51.100.0/24", nature="subnet")
    report = cs.simulate()                         # diff, warnings, violations, sections
    result = cs.submit()                           # Applied | Pending | lève Refused
```

- `kn.call(operation_id, **paramètres, body=None)` atteint toute opération du catalogue par
  son `operationId` (pendant de `kartenn api`). `kn.modules` liste les modules servis ;
  `kn.has("policy")` teste la présence d'un module.
- `Client` et `AsyncClient` partagent `_core`. Les fonctions du cœur prennent et rendent
  des descriptions de requêtes et de réponses ; le client exécute le transport.
- `kn.at(valid_at=, as_of=)` rend une vue qui ajoute les dates à toute lecture. Une
  liste rend l'`as_of` effectif (`page.as_of`), et `kn.at(as_of=page.as_of)` fige les
  lectures suivantes.
- **Reprise** : 429 et 503, `Retry-After` (secondes ou date HTTP), au plus 3 essais, 60 s
  au total. Un `POST` n'est rejoué que s'il porte `Idempotency-Key`, ce qui est toujours
  le cas pour les créations et les changesets émis par la bibliothèque (REQ-023-14). Un
  409 `idempotency-in-progress` est attendu puis rejoué.
- **Limites** : lues dans `capabilities.limits`. Un changeset au-delà de
  `changeset_operations` est refusé localement (`client.changeset-too-large`) avec la
  suggestion d'un import.

#### Catalogue (REQ-023-1 à 3, 16)

Dérivation, en une passe sur le contrat :

| Contrat | Catalogue |
|---|---|
| `/v1/<coll>` avec `get`, `post`, `x-query` | `Resource(coll)` : `list` (paramètres de filtre, `QUERY` si `x-query`), `create` (schéma du corps) |
| `/v1/<coll>/{id}` avec `get`, `patch`, `delete` | `get`, `update` (Merge Patch, `If-Match`), `close` |
| `/v1/<coll>/{id}/<sous>` en `get` | lecture de sous-ressource (`history`, `occupancy`, `children`…) |
| `/v1/<coll>/{id}/<sous>` en `post` ou `put` | action (`acknowledgement`, `validation`, `waiver`, `attachments`) |
| `/v1/<coll>/{id}/properties/{key}/<x>` | `properties.explain`, `properties.suggest` |
| `tags[0]` de l'opération | module de l'opération ; ressource « de module » si le tag figure dans `capabilities.modules` |
| `x-kartenn` (A5, facultatif) | clé naturelle, nom de commande, colonnes par défaut |
| `components.schemas` | schémas résolus (références aplaties à une profondeur bornée), énumérations |

- Le module d'une ressource est celui de son opération de collection. Une sous-ressource
  dont le tag diffère (`/v1/addresses/{id}/flows`, tag `flow`) est rattachée à la
  ressource **et** marquée du module qui la sert.
- Opérations du socle codées à la main (REQ-023-1) : `capabilities`, `openapi`, `me`,
  changesets, imports, `auth/*`. Elles sont vérifiées au chargement par leur
  `operationId` ; une absence lève `client.operation-unavailable` à l'usage.
- Le catalogue est un ensemble de dataclasses gelées, sérialisées en JSON compact dans
  `$XDG_CACHE_HOME/kartenn/catalog/<sha256(url)>-<révision>.json`.
- **Révision** : `sha256(version, modules triés, ETag du contrat)`. Le fichier est écrit
  dans un fichier temporaire, puis renommé (atomique), en mode 0600.

#### Authentification (REQ-023-6 à 8)

| Mode | Usage | Matériel | Stockage |
|---|---|---|---|
| `basic` | compte de service | nom ; secret par trousseau, `KARTENN_SECRET_FILE` ou entrée standard | trousseau (`kartenn:<profil>:secret`) |
| `oidc` | humain | émetteur (URL de découverte), `client_id` public, audience, scopes | jeton d'accès et jeton de rafraîchissement au trousseau |
| `token` | jeton fourni (CI) | `KARTENN_TOKEN` | aucun |
| `breakglass` | secours | nom, phrase de passe et TOTP saisis sans écho | cookie en mémoire ; `--keep` : `$XDG_RUNTIME_DIR/kartenn/<profil>.session` (0600) |

Flux `oidc` (T10) :
1. Découverte de l'émetteur (`/.well-known/openid-configuration`), par le profil ou par
   `GET /v1/auth/native-clients` (A5).
2. Par défaut, bouclage :
   - écoute sur `127.0.0.1:0`, puis `[::1]:0` si IPv4 manque ;
   - `code_verifier` de 32 octets aléatoires, `S256`, `state` et `nonce` aléatoires ;
   - ouverture du navigateur système (`webbrowser`), délai de 5 min ;
   - échange du code, puis contrôle de `state` ; le jeton d'identité n'est pas utilisé
     comme jeton d'accès.
3. `--device` : `device_authorization_endpoint`.
   - La CLI affiche `verification_uri_complete`, ou à défaut l'URI et le code, avec
     l'avertissement de RFC 8628 §5.4.
   - Elle interroge l'émetteur avec `grant_type=urn:ietf:params:oauth:grant-type:device_code`
     au rythme de `interval` (5 s par défaut), qu'elle allonge de 5 s à chaque
     `slow_down`.
   - Elle s'arrête sur `access_denied` ou `expired_token`.
4. Rafraîchissement : à 60 s de l'expiration, ou sur un 401 `invalid_token`, une seule
   fois. Un échec efface les jetons et demande `kartenn login`.

#### Changesets et imports (REQ-023-13 à 15)

- `ChangesetBuilder` accumule des `Operation` (schéma `Operation` du contrat, 38
  variantes au 2026-10-23, validées localement par leur discriminant seulement). Il les
  soumet par `POST /v1/changesets` (avec `?simulate=true` pour la simulation).
  - Le résultat est `Applied`, `Pending` (202, `Link`) ou l'exception `Refused`
    (violations).
  - Les actions `approve`, `reject`, `withdraw` et `undo` envoient l'`If-Match` de l'état
    lu.
  - `diff(section=…)` pagine les sections, y compris celles de module, rendues comme des
    lignes génériques (`{section, kind, …}`).
- `ImportSession` : `POST /v1/imports {source}`, puis `PUT …/chunks/{n}` par morceaux de
  `chunk_elements` au plus, puis `POST …/start`. Le suivi se fait par
  `GET /v1/imports/{id}`, au rythme de `Retry-After`.
  - L'état local (identifiant, dernier morceau confirmé, empreinte du fichier source) est
    gardé dans `$XDG_STATE_HOME/kartenn/imports/`.
  - `resume` relit le fichier source jusqu'au morceau suivant, après avoir vérifié son
    empreinte.
  - En cas de refus du tri, les violations de `error` portent l'index de l'élément ; la
    session le traduit en ligne du fichier source.

#### Stubs (REQ-023-17)

`kartenn stubs generate [--profile p] --out typings/` écrit `typings/kartenn_server/`
(`__init__.pyi`) :
- un `TypedDict` par schéma ;
- un `Literal` par énumération ;
- un `Protocol` par ressource, avec ses verbes et leurs paramètres nommés ;
- une surcharge de `Client.__getattr__` qui rend ces protocoles.
Un en-tête rappelle le serveur et sa révision. Cette sortie sert seulement aux outils
de vérification de types.

### CLI

#### Racine et chargement (REQ-023-18, 39)

- `main.py` n'importe que Click et la configuration. `--version`, `config` et
  `completion` n'importent ni httpx ni rich.
- `KartennGroup.list_commands` lit le catalogue en cache du profil courant (JSON compact)
  sans réseau. Sans cache, la première commande qui en a besoin le construit, avec un
  message d'une ligne sur la sortie d'erreur (« découverte du serveur … »).
- `get_command(name)` construit la commande Click demandée et seulement elle : le groupe
  du module, puis la ressource, puis le verbe.
- Contre un serveur sans le module demandé, `get_command("policy")` rend une commande
  d'erreur. Elle sort avec le code 2 et un message qui cite le module, le profil, la
  version du serveur et les modules servis (maquette, § 9).

#### Nommage (T13)

| Collection | Module | Commande |
|---|---|---|
| `prefixes`, `domains`, `addresses`… | noyau | `kartenn prefixes …` |
| `policy-rules`, `policy-axes`, `policy-values`, `policy-patterns`, `policy-exempt-roles` | policy | `kartenn policy rules|axes|values|patterns|exempt-roles …` |
| `services`, `environments`, `flow-types`, `inference-rules`, `risk-signals`, `admissibility-conditions` | policy | `kartenn policy services|environments|flow-types|…` |
| `policy-matrix`, `verdicts`, `policy-reports` | policy | `kartenn policy matrix`, `kartenn policy check`, `kartenn policy report` (nommées) |
| `flows`, `flow-nonconformities` | flow | `kartenn flow list|get|request|review|waive|close`, `kartenn flow nonconformities` |
| `vlans`, `vlan-groups` | vlan | `kartenn vlan list|get|…`, `kartenn vlan groups …` |
| `machines`, `interfaces`, `clusters` | machines | `kartenn machines list|…`, `kartenn machines interfaces …`, `kartenn machines clusters …` |
| collection d'un module futur `<m>` | `<m>` | `kartenn <m> <collection sans "m-">` |

Règle : on retire `<module>-` en tête du nom de collection ; la collection égale au
module, ou à son pluriel, porte ses verbes sur le groupe. Les noms des groupes et des
collections ne se traduisent pas.

#### Commandes nommées (REQ-023-20)

| Commande | Opération(s) | Rendu propre |
|---|---|---|
| `policy check SRC DST --service s` | `show_verdicts` (`GET`/`QUERY /v1/verdicts`) | verdict, issue, règle décisive, conditions non remplies, explication réduite signalée |
| `policy matrix show [--environment e]` | `show_policy_matrix` | grille source × destination par type de flux |
| `policy matrix set SRC TYPE DST VALEUR` | `set_policy_matrix_cell` (`If-Match`, 010/S35) | différentiel de la simulation si `--dry-run` |
| `policy report [--wait]` | `request_policy_report`, `show_policy_report`, `policy_report_items` | avancement, puis lignes |
| `flow request` | `request_flow` | triage : enregistré, ou à instruire avec le lien |
| `flow review ID --approve/--reject` | actions de proposition (004) sur le changeset du flux | — |
| `flow waive ID` | `waive_flow` | dérogation datée |
| `changeset …` | `submit_changeset`, `show_changeset`, `changeset_diff`, `validate_changeset`, `reject_changeset`, `withdraw_changeset`, `undo_changeset`, `list_proposals` | différentiel, sections de module |
| `import …` | `create_import`, `deposit_import_chunk`, `start_import`, `resume_import`, `cancel_import`, `show_import`, `list_imports` | avancement |
| `login`, `logout`, `me` | `me`, `breakglass`, `logout` (session de secours) | — |

Chaque ligne est une entrée `NamedCommand(path, operation_ids, render)` ; une entrée dont
une opération manque est masquée de l'aide et répond comme un module absent.

#### Sorties (REQ-023-25, 26)

- `table` : colonnes `x-kartenn.columns`, sinon les membres scalaires de premier niveau
  sauf `id`, `href` et `links`. Une référence `{id, href, slug?}` est rendue par sa clé.
  Libellé si présent, sinon clé ; `--keys`, `--ids`. Pied : nombre de lignes,
  `as_of` effectif, et « suite : `--all` ou `--cursor …` » s'il reste des pages.
- `json` : la page entière (ou l'objet) ; `--all` concatène les `items`. `ndjson` : un
  élément par ligne, en flux, sans tenir la liste en mémoire.
- `csv` : en-têtes = `--fields`, ou les colonnes de la table ; les membres imbriqués sont
  aplatis en `a.b`.
- `--query` (JMESPath) s'applique avant le rendu, à l'objet JSON.

#### Codes de sortie (REQ-023-34)

| Code | Cas |
|---|---|
| 0 | succès ; écriture appliquée ; écriture en attente sans `--require-applied` |
| 1 | erreur inattendue (500, bogue du client) |
| 2 | usage : option invalide, module ou opération non servis, référence ambiguë |
| 3 | 404 `not-found` |
| 4 | 401 et 403 (`unauthenticated`, `forbidden`, `cross-site-request`) |
| 5 | 409, 412, 428 (conflit, version périmée) |
| 6 | 422 (`changeset-refused`, `invalid-member`…), import refusé |
| 7 | 429 ou 503 après reprises ; réseau, TLS |
| 8 | `--dry-run` : la simulation rend des violations bloquantes |
| 9 | `policy check --exit-status` : verdict `denied` ; `allowed` sous réserve = 0 |
| 10 | `--require-applied` : écriture en attente d'approbation |

#### Rendu d'un problème (REQ-023-33)

```
erreur [changeset-refused] Changeset refusé (422)
  2 violations — requête urn:uuid:5b0e…  changeset 0c41…
  ✗ [core.prefix.overlap] /operations/0  Le préfixe 198.51.100.0/25 chevauche 198.51.100.0/24 (interne).
  ! [core.prefix.unused]  —              Aucun préfixe enfant (avertissement)
```

Le code est toujours en tête, entre crochets, sans couleur requise ; en
`--output json`, le problème RFC 9457 est écrit tel quel sur la sortie d'erreur.

#### Configuration (REQ-023-36)

```toml
# $XDG_CONFIG_HOME/kartenn/config.toml
default = "prod"

[profiles.prod]
url = "https://kartenn.example.org"
auth = "oidc"
issuer = "interne"            # identifiant local ; détails par /v1/auth/native-clients
lang = "fr"
timezone = "Europe/Paris"
output = "table"

[profiles.ci]
url = "https://kartenn.example.org"
auth = "basic"
account = "sync-dns"          # secret : trousseau ou KARTENN_SECRET_FILE
ca_bundle = "/etc/ssl/interne-ca.pem"
```

Précédence : option, puis variable d'environnement (`KARTENN_URL`, `KARTENN_PROFILE`,
`KARTENN_OUTPUT`, `KARTENN_LANG`), puis profil, puis défaut.

### Amendements du serveur (A5)

Trois amendements additifs à la 006, sans changement de `/v1` existant :
1. `GET /v1/openapi.json` : ETag fort (empreinte du document), `Cache-Control: no-cache`,
   304 sur `If-None-Match`.
2. Extension `x-kartenn` produite par `kartenn_rest::kit` :
   - sur le `PathItem` de collection : `{"key": ["domain", "value"], "columns": [...]}` ;
   - sur une opération : `{"command": "check"}` facultatif.
   Les modules existants la reçoivent par le kit ; le comparateur de contrat l'ignore
   (additive).
3. `GET /v1/auth/native-clients` publique : `[{id, discovery_url, client_id, audience,
   scopes, device: bool}]` pour les émetteurs de configuration `native_client_id`
   (nouveau champ de `[[auth.issuers]]`, 005).

## Couverture de l'API (REQ-023-43)

Contrat de référence au 2026-10-23 : 279 opérations.

| Tag (opérations) | Atteint par | Commandes |
|---|---|---|
| `core` (43) | dérivé | `entities`, `sites`, `domains`, `prefixes`, `ranges`, `addresses` : `list/get/create/update/close/edit/history` ; `prefixes children/ancestors/occupancy/next-free`, `domains roots` ; `anomalies list/acknowledge` |
| `properties` (69) | dérivé | `<ressource> properties <réf>`, `properties explain|suggest <réf> <clé>` sur toute ressource ; `property-declarations list|values` ; `property-gaps list` |
| `changesets` (9) | nommé | `changeset list/show/diff/simulate/apply/approve/reject/withdraw/undo/proposals` |
| `imports` (7) | nommé | `import` (création, morceaux, démarrage, reprise, annulation, suivi, liste) |
| `rights` (19) | dérivé | `roles`, `bindings`, `issuer-attachments`, `service-accounts` (dont `secrets add/revoke`), `identities` (liste, état) |
| `audit` (2) | dérivé | `audit list` (`GET`/`QUERY`), `identities pseudonymise` |
| `operations` (5) | nommé et dérivé | `capabilities`, `api spec`, `log-filters set`, `health` (`/health/live`, `/health/ready`) |
| `graph` (1) | dérivé | `graph edges` |
| `policy` (67) | dérivé et nommé | `policy rules|axes|values|patterns|exempt-roles|services|environments|flow-types|inference-rules|risk-signals|admissibility-conditions …`, `policy matrix`, `policy check`, `policy report` |
| `flow` (13) | dérivé et nommé | `flow list/get/request/review/waive/close/history`, `flow evaluations`, `flow nonconformities`, `<ressource> flows <réf>` |
| `vlan` (15) | dérivé | `vlan …`, `vlan groups … (occupancy, next-free)`, `vlan prefixes` |
| `machines` (23) | dérivé | `machines …`, `machines interfaces … (attach, detach)`, `machines clusters …`, `machines addresses|hosted` |
| `authentication` (6) | nommé, voir exclusions | `me`, `login --breakglass` (`breakglass`), `logout` (session de secours) |

Exclusions motivées (atteignables seulement par `kartenn api`) :

| Opération | Raison |
|---|---|
| `GET /v1/auth/login`, `GET /v1/auth/callback` | flux du BFF pour le navigateur (005/K1) ; la CLI obtient son jeton de l'émetteur (T10) |
| `GET /v1/auth/issuers` | page de connexion web ; la CLI lit `native-clients` (A5) |
| `PUT /v1/imports/{id}/chunks/{n}` en direct | encapsulé par `import` (découpage, reprise) ; pas de commande unitaire |

Ainsi 275 opérations sur 279 ont une commande dérivée ou nommée, et toutes sont
atteignables par `kartenn api`. Le test de couverture lit ces deux tableaux depuis
`kartenn_cli/coverage.py` (la liste des exclusions) et échoue sur toute opération
nouvelle non classée.

## Erreurs et cas limites

| Cas | Comportement | REQ |
|---|---|---|
| Serveur injoignable au premier contact, pas de cache | code 7, message avec l'URL et la cause (DNS, TLS, refus) | 4 |
| Catalogue en cache, serveur mis à jour | 1ʳᵉ commande : capacités relues (≤ 1/min), révision changée → reconstruction, une ligne d'information | 2 |
| Commande d'un module retiré du serveur depuis le dernier cache | reconstruction, puis code 2 « module non servi » | 18 |
| `--at` sans décalage dans une valeur RFC 3339 | refusé localement (le serveur refuserait aussi, 400) | 22 |
| Curseur reçu, puis changement des filtres | refus local : un curseur fige les filtres (006) | 10 |
| 412 sur `update` non interactif | code 5, ETag courant affiché ; `edit` propose de rouvrir | 12, 29 |
| 202 (en attente) | message et lien du changeset, code 0 ou 10 | 34 |
| Import : fichier source modifié avant `--resume` | refus (`client.import-source-changed`) : l'empreinte ne correspond plus | 15 |
| Jeton expiré et rafraîchissement refusé | code 4, « reconnectez-vous : `kartenn login` » | 7 |
| Trousseau absent (hôte sans D-Bus) | rien n'est stocké ; message qui propose `KARTENN_TOKEN` ou `--token-file` | 36 |
| Valeur masquée (`masked`) | rendue `‹masquée›` en table, absente en JSON comme côté serveur ; jamais en cache | 40 |
| Complétion sans réseau | propositions locales seulement, aucune erreur affichée | 24 |
| `QUERY` refusé par un mandataire (405 ou 501) | un seul repli en `GET` si l'URL tient en 8 Kio, sinon code 7 avec l'explication | 10 |

## Alternatives considérées

- Client généré au build (openapi-python-client) : rejeté (T2, a). Il est figé par build,
  ne couvre ni `QUERY`, ni les changesets, ni les imports, et épingle httpx.
- Bindings PyO3 sur une crate Rust (T2, c) : matrice de wheels, aucun gain pour un client
  limité par le réseau.
- CLI en Rust (binaire unique, démarrage de quelques ms) : elle ne réutiliserait pas la
  bibliothèque Python que l'utilisateur demande aussi. Deux clients à maintenir.
- Typer ou cyclopts (T12) : modèle par annotations, peu adapté à des commandes créées à
  l'exécution ; démarrage plus lent.
- Jetons personnels émis par kartenn (T10, B) : kartenn deviendrait émetteur.

## Risques

- **Démarrage de Python** : un interpréteur nu démarre en 15 à 25 ms ; httpx coûte environ
  65 ms à l'import et rich environ 17 ms. Les budgets de REQ-023-39 ne tiennent qu'avec
  un import paresseux strict, gardé par un test (T24).
- **httpx figé** : aucune version stable depuis décembre 2024. Le port de transport
  (T5) borne le changement.
- **Contrat volumineux** : 830 Kio aujourd'hui, et il croît avec les modules. Le
  catalogue dérivé et le cache par révision évitent de le relire.
- **Conventions non tenues par un module futur** : la commande dérivée serait mal nommée
  ou absente. Le test de couverture le voit sur le contrat de référence, et `x-kartenn`
  corrige sans code client.
- **Émetteurs sans client public ou sans flux d'appareil** : la CLI ne connecte alors un
  humain qu'avec un jeton fourni (`KARTENN_TOKEN`). À documenter par émetteur.
- **Jetons de rafraîchissement d'un client public** : RFC 9700 §2.2.2 exige la rotation
  ou un lien à l'expéditeur. Un émetteur sans rotation fait du trousseau une cible.
  Documenter l'exigence, et proposer une durée de session courte.

## Stratégie de test

| Tâches | Tests |
|---|---|
| 2 à 7 | unitaires pytest, `httpx.MockTransport` : problèmes, reprise, pagination, dates, idempotence, Merge Patch, If-Match |
| 4 | dérivation sur `specs/006-api-rest/openapi-v1.json` : catalogue en instantané ; contrat amputé du tag `policy` ; module fictif `demo` ajouté |
| 13 à 18 | arbre de commandes en instantané (`--help` de chaque groupe), rendus en instantané, codes de sortie par statut, couverture (REQ-023-43) |
| 19 | bout en bout contre `kartenn-server` (base `kartenn_cli`, comme `web/e2e/serve.sh`) : les sessions de la [maquette](maquette.md) rejouées et comparées (dates et UUID normalisés) |
| 8, 19 | `kartenn-dev-idp` : client public, bouclage, flux d'appareil, rotation des jetons de rafraîchissement |
| 20 | banc `bench-cli` : budgets de REQ-023-39 |

## ADR

- À proposer à la validation : **ADR 0033 — Clients découverts depuis le contrat**. Il
  couvrirait l'approche (b) de T2, la découverte (T3) et le cache par révision (T20), et
  préciserait D7 (« SDK généré depuis l'OpenAPI »).

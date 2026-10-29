---
status: implemented
updated: 2026-10-29
---

# 023 — Bibliothèque cliente Python et CLI : exigences

## Résumé

Deux livrables clients de l'API REST (D7), qui n'ont aucun autre accès au serveur :

- **`kartenn`, bibliothèque Python** : connexion, authentification, découverte, lectures
  datées, pagination, problèmes, changesets, imports. Elle ne contient **aucune liste
  figée des modules**. Elle lit les capacités (`GET /v1/capabilities`) et le contrat
  (`GET /v1/openapi.json`) du serveur. Les ressources et actions d'un module (Politique,
  Flux, VLAN, Machines, et ceux à venir) apparaissent donc dès que le serveur les sert,
  sans nouvelle version du client.
- **`kartenn`, la CLI**, construite sur la bibliothèque. Son arbre de commandes se dérive
  du contrat du serveur (ressource → verbe), plus quelques commandes propres à un module
  (`policy check`, `flow request`). La commande générique `kartenn api` atteint toute
  opération. Le reste de la CLI couvre les lectures datées, les filtres typés avec
  complétion, plusieurs formats de sortie, le rendu des problèmes avec leur code, les
  écritures par changeset avec `--dry-run`, les imports par morceaux et les profils.

Les deux livrables sont des **paquets distincts** (T7) : la bibliothèque, sous
MPL-2.0, ne dépend d'aucune dépendance de la CLI ; la CLI est sous AGPL-3.0 (T8).
Spec **validée le 2026-10-23** : l'utilisateur a rendu les arbitrages A1 à A8 et les
questions Q-023-1 à 14 sont tranchées ([`decisions.md`](decisions.md)). L'implémentation
commence tout de suite, avant la v0.3 (D31 amendée, A6).

Sources : D7, D13, D31, D37, Q-ouv-1, Q-ouv-6
([`../001-cadrage/decisions.md`](../001-cadrage/decisions.md)). Spec 004 (changesets,
simulation, propositions). Spec 005 (jetons Bearer RFC 9068, comptes de service,
secours). Spec 006 (contrat HTTP, ADR 0022). ADR 0026, clause 9 (conventions REST des
modules). Specs 008 à 011 (routes des modules). Spec 022 (événements SSE, `GET /v1/events`, validée le
2026-10-23). Maquette : [`maquette.md`](maquette.md).

### Vocabulaire

- **Bibliothèque** : paquet Python qui parle HTTP au serveur. On l'oppose aux
  **bindings**, une enveloppe Python autour d'une bibliothèque native (par exemple une
  crate Rust exposée par PyO3), voir T2.
- **Catalogue** : modèle des ressources, opérations, paramètres et schémas qu'un serveur
  sert. La bibliothèque le dérive de ses capacités et de son contrat, puis le garde en
  cache.
- **Ressource** : collection REST (`/v1/prefixes`) et ses sous-ressources (`/{id}/history`,
  `/{id}/occupancy`…).
- **Module** : un module de la build du serveur, tel que `modules` des capacités le liste.
  Les opérations d'un module portent son nom en étiquette (`tags`) dans le contrat.
- **Profil** : un serveur, une identité et des préférences, nommés dans la configuration
  de l'utilisateur.
- **Clé / libellé** : la clé interne (`prod`) et le texte affiché (« Production »).
- **Code** : le slug d'un problème (`changeset-refused`, tiré de
  `urn:kartenn:problem:<slug>`) ou la règle d'une violation (`import.unknown-reference`).

## Exigences

### A. Bibliothèque : connexion et contrat

#### REQ-023-1 — Découverte des modules
La bibliothèque doit construire son catalogue à partir de `GET /v1/capabilities` et de
`GET /v1/openapi.json` du serveur. Elle ne doit coder en dur aucune ressource ni aucun
module, à l'exception des opérations transverses du socle : capacités, contrat, `me`,
changesets, imports et authentification. [T3, D7, ADR 0026 clause 9]

Acceptation :
- Contre un serveur construit sans le module `policy`, `client.resource("policy-rules")`
  lève `UnknownResource`, dont le message cite le module absent.
- Contre un serveur qui sert un module de test `demo` inconnu de la bibliothèque, ses
  collections sont listables, lisibles et modifiables par la même version de la
  bibliothèque, sans aucun changement.

#### REQ-023-2 — Cache du catalogue
La bibliothèque doit garder le catalogue en cache par serveur. La clé du cache est
l'URL de base, la `version` et les versions des modules annoncées par les capacités,
et l'ETag du contrat quand le serveur le sert. Elle ne doit relire le contrat que si
cette clé change. [T20]

Acceptation :
- Deux invocations successives contre le même serveur ne téléchargent le contrat
  (environ 830 Kio pour la build toutes fonctionnalités au 2026-10-23) qu'une fois.
- Après la mise à jour du serveur (nouvelle `version`), la première invocation reconstruit
  le catalogue.

#### REQ-023-3 — Compatibilité
La bibliothèque doit refuser un serveur dont `api_versions` ne contient pas `v1`, avec
l'erreur `client.api-unsupported` qui nomme les versions servies. Elle doit détecter les
fonctions du socle **par la présence de leurs opérations** dans le contrat, et non par un
numéro de version du serveur. [T9]

Acceptation :
- Contre un serveur qui ne sert pas `QUERY` sur une collection, un filtre trop long pour
  une URL échoue avec `client.query-unsupported`, et non avec un 405 brut.

#### REQ-023-4 — Transport
La bibliothèque doit offrir un client synchrone et un client asynchrone qui partagent le
même cœur. Ils passent par un port de transport (`send`, `stream`, `sse`) dont
l'adaptateur par défaut repose sur httpx2. La bibliothèque doit :
- vérifier TLS par défaut ;
- accepter un paquet d'autorités (`ca_bundle`) par un contexte SSL explicite, sans repli
  vers le magasin du système quand il est donné ;
- respecter les variables de mandataire standard (`HTTPS_PROXY`, `NO_PROXY`) ;
- envoyer un `User-Agent` qui porte sa version. [T5]

Acceptation :
- Contre un serveur à certificat auto-signé, sans `ca_bundle`, la connexion échoue
  (`client.tls`) ; avec le paquet de la CA, elle réussit.
- Avec un `ca_bundle` qui ne contient pas la CA du serveur, la connexion échoue même si
  cette CA figure dans le magasin du système.

#### REQ-023-5 — Reprise et limites
La bibliothèque doit rejouer une requête refusée par 429 ou 503 après le délai de
`Retry-After`, au plus N fois (3 par défaut) et dans un délai total borné. Elle ne doit
rejouer automatiquement un `POST` que s'il porte une `Idempotency-Key`. Elle doit lire
les limites des capacités (taille de page, d'un changeset, d'un morceau, d'un corps) et
découper ses envois en conséquence. [T5, 006/H8]

Acceptation :
- Un 503 `Retry-After: 1` suivi d'un 200 rend le résultat, après une attente d'au moins
  1 s.
- Un `POST /v1/changesets` sans clé n'est jamais rejoué.

### B. Bibliothèque : authentification

#### REQ-023-6 — Comptes de service
La bibliothèque doit authentifier un compte de service en Basic (RFC 7617). Elle doit le
refuser hors de `https`, sauf vers une adresse de bouclage avec un réglage explicite
pour les tests. Le secret est lu dans un objet secret fourni par l'appelant, jamais dans
un argument de ligne de commande. [T11, D13, 005/K8]

#### REQ-023-7 — Humains
La bibliothèque doit obtenir un jeton d'accès Bearer de l'émetteur OIDC de l'utilisateur.
Elle mène le flux par code avec PKCE `S256` et redirection vers une adresse de bouclage
(RFC 8252 §7.3, RFC 7636 §4.2) dans un navigateur externe. Le flux d'autorisation
d'appareil (RFC 8628) est disponible quand l'émetteur l'offre et qu'aucun navigateur n'est
utilisable. La bibliothèque renouvelle le jeton par son jeton de rafraîchissement. Elle
n'envoie jamais un jeton d'identité comme jeton d'accès. [T10, D13, 005/REQ-005-9]

Acceptation :
- Contre l'émetteur de développement (`kartenn-dev-idp`), `login` puis `me` rend
  l'identité ; après l'expiration du jeton d'accès, l'appel suivant le renouvelle sans
  interaction.
- Avec `--device`, la CLI affiche l'URL et le code, puis interroge l'émetteur au rythme
  de `interval`, et de `slow_down` quand il le demande (RFC 8628 §3.5).

#### REQ-023-8 — Secours
La bibliothèque doit permettre une connexion de secours (`POST /v1/auth/breakglass`). La
session est un cookie d'au plus 1 h, gardé en mémoire seulement. Les écritures sous cette
session portent l'en-tête `Origin` de l'instance (contrôle CSRF, 006). [T11, 005]

### C. Bibliothèque : lectures, écritures, erreurs

#### REQ-023-9 — Lectures datées
Toute lecture doit accepter `valid_at` et `as_of`, envoyés en RFC 3339 avec décalage. La
bibliothèque doit exposer l'`as_of` effectif que le serveur rend, pour que l'appelant
puisse figer ses lectures suivantes. [D10, 006]

#### REQ-023-10 — Pagination
Une liste doit se parcourir par un itérateur qui suit `links.next` (curseur signé) sans
jamais réécrire le curseur. Elle accepte des filtres. Quand l'URL dépasserait la limite
(8 Kio), elle bascule sur `QUERY`. [006/REQ-006-7 à 10]

Acceptation :
- Itérer une liste de 2 500 éléments avec `page_size=1000` fait 3 requêtes et rend 2 500
  éléments distincts.

#### REQ-023-11 — Problèmes
Une réponse d'erreur doit lever une exception typée par classe (`NotFound`,
`Conflict`, `PreconditionFailed`, `Refused`…). L'exception porte le `type`, le code (slug),
`title`, `detail`, `status`, l'identifiant de requête (`instance`) et les `violations`
(règle, sévérité, détail, pointeur, sujets). Les membres d'extension inconnus restent
lisibles, sans erreur (RFC 9457 §3.2). [006/REQ-006-20, 21]

#### REQ-023-12 — Version attendue
Un objet lu doit garder son ETag. `update` et `close` doivent l'envoyer en `If-Match`
sans que l'appelant ait à le recopier. Un 412 lève `PreconditionFailed` avec l'ETag
courant quand le serveur le rend. [006, REQ-004-11]

#### REQ-023-13 — Changesets
La bibliothèque doit permettre de composer un changeset de plusieurs opérations (le
schéma `Operation` du contrat), avec un motif, `valid_from` et un « pour le compte de ».
On peut le **simuler** (`?simulate=true`), le soumettre, puis suivre son état (appliqué,
en attente, refusé). Elle doit aussi permettre de valider, rejeter, retirer ou annuler un
changeset, et d'en lire le différentiel, y compris les sections de module
(`policy.combinations`, `flow.flows`…) rendues de façon générique. [004, 006, 017/Q-017-1]

#### REQ-023-14 — Idempotence
Tout `POST` de création ou de changeset émis par la bibliothèque doit porter une
`Idempotency-Key` générée (UUID v4), sauf si l'appelant en fournit une. Un rejeu après
une coupure réseau réutilise la même clé. [006/H8]

#### REQ-023-15 — Imports
La bibliothèque doit importer un flux d'éléments (`ImportElement`) de taille non bornée
en mémoire constante. Elle crée l'import, dépose des morceaux numérotés (au plus la
limite `chunk_elements`), démarre l'import et rapporte l'avancement par un rappel.
Elle doit reprendre un import interrompu à partir des morceaux déjà reçus. [004/REQ-004-28, 006]

Acceptation :
- Un import de 10⁶ éléments n'occupe pas plus de 100 Mio dans le processus client.
- Après l'arrêt du client au morceau 40 sur 100, `resume` ne renvoie que les morceaux 41
  à 100.

### D. Bibliothèque : ergonomie typée

#### REQ-023-16 — Accès dynamique
Les ressources du catalogue doivent être atteignables par nom
(`client.resource("vlans")`) et par attribut (`client.vlans`). Chacune offre les verbes
que le contrat lui donne : `list`, `get`, `create`, `update`, `close`, ses sous-ressources
et ses actions. [T3, T4]

#### REQ-023-17 — Stubs générés
La bibliothèque doit pouvoir générer, à partir du contrat d'un serveur donné, des fichiers
de types (`.pyi` et `TypedDict`). Ils donnent la complétion et la vérification statique
des ressources et des corps dans un éditeur. Ces types restent facultatifs : la
bibliothèque fonctionne sans eux. [T4]

Acceptation :
- `kartenn stubs generate --out typings/` puis `pyright` sur un script qui appelle
  `client.vlans.create(...)` avec un membre inconnu : erreur signalée par l'outil.

### E. CLI : commandes

#### REQ-023-18 — Arbre dérivé du contrat
La CLI doit dériver ses commandes du catalogue :
- le **noyau** et le **socle** au premier niveau (`kartenn prefixes list`) ;
- un **groupe par module** (`kartenn policy …`, `kartenn flow …`, `kartenn vlan …`,
  `kartenn machines …`), dont les ressources perdent le préfixe du module
  (`/v1/policy-rules` → `kartenn policy rules`).

Verbes : `list` (GET collection), `get` (GET objet), `create` (POST collection), `update`
(PATCH), `close` (DELETE, au sens de D33), `history`, `properties`, et les
sous-ressources en sous-commandes. Une commande de module n'existe que si le serveur sert
le module. [T13]

Acceptation :
- Contre un serveur sans `policy`, `kartenn policy check …` répond (code de sortie 2) :
  « le module `policy` n'est pas servi par ce serveur (profil `prod`, version 0.1.4) ;
  modules servis : vlan, machines ».
- `kartenn --help` liste le groupe `policy` seulement contre un serveur qui le sert.

#### REQ-023-19 — Commande générique
`kartenn api <MÉTHODE> <chemin>` doit atteindre toute opération du contrat, y compris
`QUERY`. La commande gère l'authentification, les dates, les filtres (`-f clé=valeur`),
le corps (`--data`, `@fichier`, `-` pour l'entrée standard), la pagination
(`--paginate`) et les en-têtes (`-H`). [T13]

#### REQ-023-20 — Commandes propres aux modules
La CLI doit offrir, au-dessus des verbes dérivés, des commandes nommées pour les parcours
d'un module : `policy check` (verdicts), `policy matrix show/set`, `policy report`,
`flow request`, `flow review`, `flow waive`. Elles sont déclarées par une table de
correspondance **dans la CLI**, liée à des `operationId` du contrat. Elles n'apparaissent
que si ces opérations existent. [T13]

#### REQ-023-21 — Aide
`--help` doit montrer, pour une commande dérivée, le résumé et les paramètres tirés du
contrat, le module qui la sert et l'opération HTTP correspondante. [T13]

### F. CLI : lectures

#### REQ-023-22 — Dates
Toute lecture doit accepter `--at` (temps applicatif, `valid_at`) et `--as-of` (temps
système). Elles prennent une date RFC 3339, une date seule (minuit dans le fuseau du
profil) ou une durée relative (`-7d`, `+1M`). La CLI affiche les dates effectives quand
elles diffèrent de « maintenant ». [D10, T14]

#### REQ-023-23 — Filtres
`list` doit accepter les filtres du contrat sous forme d'options typées (`--domain`,
`--within`…), et les propriétés par `--prop clé[:op]=valeur` (opérateurs de la 006 :
égal, `between`, `exists`). Un filtre inconnu est refusé localement, avant toute
requête, avec la liste des filtres possibles. [006/REQ-006-9, 10]

#### REQ-023-24 — Complétion
La CLI doit offrir la complétion pour bash, zsh et fish : commandes, options, valeurs des
énumérations du contrat, clés de propriétés et leurs valeurs à liste fermée
(`/v1/property-declarations/{key}/values`), slugs des références (domaines, entités,
sites). Les valeurs lues au serveur sont gardées en cache peu de temps. Une complétion ne
doit jamais bloquer plus d'une seconde : passé ce délai, elle rend les seules
propositions locales. [T14, retour UI du 2026-10-22 : filtres typés]

#### REQ-023-25 — Formats de sortie
La CLI doit sortir en `table` (par défaut sur un terminal), `json` (par défaut hors
terminal), `ndjson`, `yaml` et `csv`. Elle permet de choisir des colonnes (`--fields`) et
de projeter le résultat par une expression (`--query`). [T14]

#### REQ-023-26 — Clés et libellés
En `table`, la CLI doit afficher le libellé quand l'API en rend un, et sinon la clé.
`--keys` force les clés. `json`, `yaml`, `csv` et `ndjson` rendent toujours les clés.
Les identifiants (UUID) sont masqués en `table`, sauf avec `--ids`. [T15, retour UI du
2026-10-22]

#### REQ-023-27 — Résolution des références
Les commandes qui désignent un objet doivent accepter son UUID ou sa clé naturelle quand
la ressource en déclare une (`kartenn domains get interne`,
`kartenn prefixes get interne/192.0.2.0/24`). Une clé ambiguë ou introuvable est un
refus local, avec code. [T13, Q-023-9]

### G. CLI : écritures

#### REQ-023-28 — Création et modification
`create` et `update` doivent accepter les membres en options typées (dérivées du schéma),
en `--set chemin=valeur`, ou en document (`--from-file`, `-` pour l'entrée standard).
`update` envoie un Merge Patch (RFC 7396) sous `If-Match` (ETag de la lecture). `close`
exige `--motive`. [006, D33]

#### REQ-023-29 — Édition
`kartenn <ressource> edit <réf>` doit ouvrir l'objet dans `$VISUAL` ou `$EDITOR` (YAML
par défaut, champs non modifiables en commentaire). La commande calcule le Merge Patch
de la différence et l'envoie sous l'`If-Match` de la lecture. En cas de 412, elle propose
de rouvrir l'éditeur sur la version courante, avec les modifications de l'utilisateur
en commentaire. Le fichier temporaire est créé en mode 0600 dans un répertoire privé,
puis supprimé. [T17]

#### REQ-023-30 — Simulation
`--dry-run` doit être accepté par toute commande d'écriture. Il soumet le même changeset
avec `?simulate=true` et affiche le différentiel (sections de module comprises), les
avertissements et les violations, sans rien écrire. [004, T17]

#### REQ-023-31 — Changesets
`kartenn changeset` doit offrir : `new` (fichier de changeset local), `add` (ajoute une
opération), `simulate`, `apply` (soumet), `show`, `diff`, `list`, `approve`, `reject`,
`withdraw`, `undo`, et `proposals` (changesets en attente que l'appelant peut
instruire). [004, T17]

#### REQ-023-32 — Imports
`kartenn import` doit accepter :
- un fichier NDJSON d'`ImportElement` ;
- un CSV ou un JSON d'objets d'**une** ressource (`--resource prefixes`), traduit en
  opérations de création par les schémas du contrat (colonnes = membres, `prop.<clé>` =
  propriétés).

La commande affiche l'avancement (morceaux, éléments, débit, temps restant) sur un
terminal, une ligne par étape hors terminal. Elle permet `--resume <id>` et
`--follow` (suivi jusqu'à l'état final). [T18]

Acceptation :
- Un import dont un élément référence un objet inconnu affiche le code
  `import.unknown-reference`, la ligne du fichier source et le pointeur.

### H. CLI : erreurs, sorties, environnement

#### REQ-023-33 — Rendu des problèmes
Toute erreur doit afficher, sur la sortie d'erreur, le **code** (toujours, en clair et
copiable), le titre, le détail, l'identifiant de requête et chaque violation (code de
règle, sévérité, message, pointeur ou opération). En `--output json`, le problème est
rendu tel quel sur la sortie d'erreur. [T16, retour UI du 2026-10-22 : codes visibles]

#### REQ-023-34 — Codes de sortie
La CLI doit sortir avec des codes stables et documentés, un par classe d'échec
(design, § Codes de sortie). [T16]

#### REQ-023-35 — Non interactif
Hors terminal, ou avec `--no-input`, la CLI ne doit jamais poser de question ni ouvrir de
navigateur ou d'éditeur. Une commande qui en aurait besoin échoue avec un code et un
message qui disent quoi fournir. Les couleurs suivent `NO_COLOR` et la détection du
terminal. [T19]

#### REQ-023-36 — Profils et configuration
La configuration doit vivre dans `$XDG_CONFIG_HOME/kartenn/config.toml` : profils
(URL, émetteur, compte, fuseau, langue, format), avec `KARTENN_PROFILE` et `--profile`.
Les jetons et secrets ne doivent jamais y être écrits. Ils sont gardés dans le trousseau
du système, ou fournis par l'environnement (`KARTENN_TOKEN`,
`KARTENN_SECRET_FILE`). [T19]

#### REQ-023-37 — Langue
Les textes de la CLI doivent exister en français et en anglais (catalogues par clés). La
langue suit `--lang`, puis le profil, puis `LC_ALL`, `LC_MESSAGES` et `LANG` ; l'anglais
sert de repli. Les codes, clés et noms de commandes ne se traduisent pas. [Q-ouv-6, T21]

#### REQ-023-38 — Suivi d'événements
La bibliothèque (`events()`, `aevents()`) et `kartenn watch` doivent lire
`GET /v1/events` (022) en flux. Ils envoient `Authorization` et posent `Last-Event-ID`
eux-mêmes. Ils acceptent les filtres de la 022 (`--types`, `--domains`, `--objects`,
`--kinds`). Avant ouverture, ils respectent le statut et `Retry-After` ; ils reprennent
avec une attente croissante, plafonnée et aléatoire. Ils écartent les doublons par
identité d'événement, et signalent `reset` sans fermer le flux. La commande est absente
quand `capabilities.events` vaut `null`. `--follow` d'un import ou d'un changeset en
attente utilise le flux s'il existe, sinon il interroge le serveur. [T22, 022]

Acceptation :
- Coupure du flux après l'événement `k1.X` : la reprise envoie `Last-Event-ID: k1.X` ;
  un événement reçu deux fois n'est affiché qu'une fois.
- `kartenn watch --types core.prefix -o ndjson` écrit un événement `changes` par ligne
  dès sa réception.

### I. Transverse

#### REQ-023-39 — Budgets
La CLI doit tenir les budgets suivants, mesurés à cache chaud du catalogue, au p95 sur la
machine de banc :
- `kartenn --version` < 80 ms ;
- `kartenn --help` et l'aide d'une commande de module < 200 ms ;
- une complétion locale < 100 ms, une complétion qui interroge le serveur < 400 ms ;
- surcoût du client sur `prefixes list` (une page de 100) < 120 ms au-delà du temps de
  réponse du serveur ;
- construction du catalogue au premier contact (contrat d'environ 1 Mio) < 1,5 s ;
- import de 10⁶ éléments : le client n'est pas le goulot (débit d'envoi ≥ 2× celui
  du serveur au budget D14).

[T24, D14]

#### REQ-023-40 — Sécurité du client
Le client doit respecter les règles suivantes :
- il ne journalise jamais `Authorization`, cookie, secret ni jeton ;
- il n'accepte aucun secret en argument ;
- il crée ses fichiers de cache et d'état en 0600, dans des répertoires 0700 ;
- il ne met jamais en cache une valeur masquée (`masked`) ni une valeur de propriété
  sensible ;
- il ne désactive jamais la vérification TLS (pas d'option `--insecure`).

[T19, 005]

#### REQ-023-41 — Corrélation
Chaque requête doit porter un en-tête `traceparent` (W3C Trace Context). En `--verbose`,
la CLI affiche la méthode, le chemin, le statut, la durée et l'identifiant de requête
rendu par le serveur. La CLI n'émet aucune autre télémétrie. [012/REQ-012-6, Q-023-11]

#### REQ-023-42 — Paquetage et licence
La bibliothèque et la CLI doivent se livrer comme deux distributions distinctes
(`kartenn`, `kartenn-cli`), chacune avec son pyproject, sa version, ses tests et sa
publication. Elles sont produites en wheels `py3-none-any` (uv) et en paquets Nix du
flake. Licences : bibliothèque MPL-2.0 (Q-ouv-1), CLI AGPL-3.0 (D37). [T7, T8]

#### REQ-023-44 — Indépendance de la bibliothèque
La bibliothèque ne doit dépendre, à l'exécution, que de `httpx2` et `platformdirs`. Elle
ne doit importer aucune dépendance de la CLI (click, rich-click, rich, keyring, PyYAML,
jmespath). [T7]

Acceptation :
- Dans un environnement où seule `kartenn` est installée, `import kartenn` et un
  parcours de lecture et d'écriture sur transport simulé réussissent.

#### REQ-023-45 — Outillage de test et de contrôle
Chaque paquet doit se tester par pytest ≥ 9.1, avec les greffons de T23. `k-py-check`
(sh POSIX) doit lancer ruff, pyright et `pytest -n auto` sur les deux paquets, et rendre
un verdict sur une ligne. La CI doit lancer `k-py-check`. [T23, T25]

Acceptation :
- Une couverture de branches sous le seuil `fail_under` fait échouer `k-py-check`.
- `k-py-check` sort en `OK — …` sur une ligne en cas de succès, et ne liste que les
  échecs sinon.

#### REQ-023-43 — Couverture
Un test doit dériver la CLI du contrat de référence
(`specs/006-api-rest/openapi-v1.json`) et échouer si une opération n'est ni atteinte par
une commande dérivée ou nommée, ni listée dans les exclusions motivées du design
(§ Couverture). [T13]

## Non-objectifs

- TUI et serveur MCP : specs ultérieures. Le MCP réutilisera la bibliothèque (Q-023-13).
- Bindings natifs (PyO3) et crate cliente Rust (T2).
- HTTP/3 : le port de transport l'admet (`kartenn[http3]`, curl_cffi), à écrire seulement
  quand le serveur ou son mandataire le parlera et qu'un banc montrera un gain (T5).
- Autre client que Python. Le SDK TypeScript du web reste celui de la 007.
- Windows : au mieux, sans test en CI. PowerShell n'a pas de complétion.
- Mise en forme de rapports (PDF, tableurs) : la CLI sort des données, pas des documents.
- Mode hors ligne : toute commande exige le serveur, sauf `--help`, la complétion locale
  et `config`.

## Questions

Toutes arbitrées le 2026-10-23 : [`decisions.md`](decisions.md), § Arbitrages rendus
(A1 à A8) et § Questions arbitrées (Q-023-1 à 14).

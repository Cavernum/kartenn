---
status: approved
updated: 2026-10-18
---

# 006 — API REST : exigences

## Résumé

L'API REST est la seule frontière publique de kartenn (D7) : le Web, le CLI, le MCP et
les connecteurs ne passent que par elle. Cette spec fixe ses conventions transverses
(représentations, dates, pagination, filtres, erreurs, concurrence, idempotence,
versions) et expose toute la surface que fournissent les specs 002 à 005 et 012 :
objets du noyau, lectures datées et arbre, propriétés, changesets et leur cycle,
imports, administration des droits, audit, authentification et exploitation. Elle porte
aussi ce que ces specs laissent à la couche HTTP : cookies et CSRF, limitation de débit,
transport, tâches de fond, et les budgets D14 mesurés à travers HTTP.

Décisions : [`decisions.md`](decisions.md) (H1–H16) ; D7, D9, D10, D11, D13, D14, D15
([`../001-cadrage/decisions.md`](../001-cadrage/decisions.md)) ; ADR 0002, 0020 ;
fin de [`../005-authn-rbac/design.md`](../005-authn-rbac/design.md) (« Laissé à la couche
HTTP »). Sources :
[`docs/reference/api-http.md`](../../docs/reference/api-http.md).

### Vocabulaire

- **Ressource** : collection ou objet désigné par une URL sous `/v1`.
- **Route par ressource** : `POST`, `PATCH` ou `DELETE` sur une ressource, qui enveloppe
  un changeset d'une opération (H2).
- **Lecture datée** : lecture au couple (`valid_at`, `as_of`) ; l'un ou l'autre absent
  vaut « maintenant » (D10).
- **Passé figé** : `as_of` antérieur à maintenant d'au moins la durée maximale d'une
  transaction d'écriture (H4) ; rien ne peut plus y être écrit.
- **Curseur** : jeton opaque et signé qui désigne la page suivante d'une liste (H5).
- **Problème** : corps d'erreur `application/problem+json` (RFC 9457).
- **Origine publique** : schéma, hôte et port configurés par lesquels le navigateur
  atteint kartenn.
- **Proxy de confiance** : adresse configurée dont kartenn croit l'en-tête `Forwarded`.
- **Tâche de fond** : travail périodique ou long exécuté par une seule instance sous
  bail (H9, H16).

### Conventions

- Forme EARS en français, comme dans
  [`002/requirements.md`](../002-noyau-adressage/requirements.md#conventions).
- Les références entre crochets renvoient aux décisions (H*) et aux sources :
  [RFC 9110 §x] sémantique HTTP, [RFC 9111 §x] cache, [RFC 9457 §x] problèmes,
  [RFC 10008 §x] QUERY, [RFC 7396] Merge Patch, [RFC 6585 §x], [RFC 6750 §x],
  [RFC 7617 §x], [RFC 8246], [RFC 8288], [RFC 8594], [RFC 9745], [RFC 7239],
  [Fetch Metadata §x], [Idempotency-Key -07], [RateLimit -11].
- Les codes d'état cités sont ceux de RFC 9110 §15, sauf mention.

## Exigences

### A. Ressources et représentations

#### REQ-006-1 — Collections à plat, UUID canonique
kartenn doit exposer chaque type d'objet comme une collection à plat sous `/v1`, chaque
objet à une seule URL `/v1/<collection>/{uuid}` ; la clé naturelle (domaine et valeur,
slug) doit être un filtre de la collection, pas un chemin. [H3, RFC 9111 §4.1]

Acceptation :
- `GET /v1/prefixes?domain=lab&value=10.1.2.0/24` renvoie une liste d'au plus un
  élément dont le lien `self` est `/v1/prefixes/{uuid}`.
- Un renommage de domaine ne change pas l'URL du préfixe.

#### REQ-006-2 — Représentation complète, résumés liés
kartenn doit renvoyer chaque objet en entier, avec pour chaque objet lié un résumé
(identifiant, clé naturelle, `href`) et des liens (RFC 8288) ; les propriétés héritées
doivent se lire par leur propre route. Ni `fields=` ni `expand=` en v1. [H13b]

Acceptation :
- Un préfixe porte `domain: {id, slug, href}` ; la lecture de l'objet lié par `href`
  donne le même identifiant.

#### REQ-006-3 — JSON et types de média
kartenn doit produire du JSON (RFC 8259), `application/json` pour les représentations et
`application/problem+json` pour les erreurs ; si une requête porte un corps d'un type
non accepté par la route, alors kartenn doit répondre 415. [RFC 9110 §15.5.16]

#### REQ-006-4 — Surface couverte
kartenn doit exposer, sous `/v1`, au moins (H1) :
- les objets du noyau : entités, sites, domaines, préfixes, plages, adresses, avec leur
  historique (004/REQ-004-18), la navigation dans l'arbre, l'occupation et le prochain
  libre (002/REQ-002-42 à 44), les anomalies et leur acquittement (002/REQ-002-36) ;
- les propriétés : déclarations, listes de valeurs, affectations, valeurs résolues,
  explication, suggestion et écarts (003/REQ-003-14, 23, 24) ;
- les changesets : soumission, simulation, lecture, différentiel, validation, rejet,
  retrait, propositions et annulation (004) ;
- les imports (H9) ;
- l'administration des droits : rôles, liaisons, rattachements, comptes de service,
  secrets, état des identités (005) ;
- l'audit : consultation, export, pseudonymisation (004/REQ-004-32, 33 ; 012/REQ-012-17) ;
- l'exploitation : capacités, contrat OpenAPI, niveau de log à chaud (012/REQ-012-27),
  sondes.

Acceptation :
- Le contrat OpenAPI de la build toutes features contient une opération pour chaque
  élément de la liste ; un test le vérifie.

### B. Lectures

#### REQ-006-5 — Lectures datées
kartenn doit accepter `valid_at` et `as_of` sur toute lecture et toute liste, en RFC 3339
avec décalage obligatoire (`Z` ou `±hh:mm`) ; une date sans décalage doit être refusée
(400). [H4, D10]

Acceptation :
- `?as_of=2026-01-01T00:00:00` → 400 ; `?as_of=2026-01-01T00:00:00Z` → l'état connu à
  cette date.

#### REQ-006-6 — Cache des lectures
kartenn doit marquer toute réponse de lecture `private` ; une lecture au passé figé doit
porter un `max-age` long et `immutable` ; toute autre lecture `no-cache` et un ETag,
et une requête conditionnelle dont l'ETag n'a pas changé doit recevoir 304. [H4,
RFC 9111 §5.2.2, RFC 8246, RFC 9110 §13]

Acceptation :
- Deux lectures au passé figé renvoient le même corps et `immutable`.
- Une lecture courante relue avec `If-None-Match` → 304 tant que l'objet n'a pas changé,
  200 après un changeset qui le modifie.
- Un `as_of` plus récent que la borne du passé figé n'est pas `immutable`.

#### REQ-006-7 — Pagination par curseur
kartenn doit paginer toute liste par curseur opaque, 100 éléments par défaut et 1 000 au
plus (`page_size`), sans total, le lien suivant dans `Link` (`rel="next"`) et dans le
corps, et aucun lien suivant en fin de liste ; le curseur doit figer `as_of` à la
première page. [H5, RFC 8288] Exception : la chaîne des ancêtres d'un préfixe, bornée
par la profondeur de l'arbre (128 au plus), est rendue en entier sans curseur (006-12e).
Exception : une liste filtrée par propriétés (`prop.<clé>`) relit l'état courant à
chaque page ; son curseur porte l'UUID du dernier élément et ne fige que `valid_at`. Un
objet dont la valeur change entre deux pages peut y entrer ou en sortir ; aucun n'est
répété (arbitrage du 2026-10-19, 006-13b).

Acceptation :
- `page_size=1001` → 400.
- Un changeset appliqué entre deux pages ne fait ni sauter ni répéter d'élément.

#### REQ-006-8 — Curseur signé, droits réévalués
Si un curseur est altéré, forgé ou signé par une clé retirée du trousseau, alors kartenn
doit le refuser (400) sans exécuter la requête ; un curseur ne doit accorder aucun droit :
le contrôle d'accès est réévalué à chaque page. [H5, 005/REQ-005-24]

Acceptation :
- Un octet modifié dans le curseur → 400.
- Une liaison retirée entre deux pages : la page suivante ne contient plus les objets
  qu'elle ouvrait.

#### REQ-006-9 — Filtres par paramètres
kartenn doit accepter les filtres des listes en paramètres de requête, combinés en
conjonction : champs du noyau (`domain`, `within` inclusion CIDR, `nature`, `usage`…)
et propriétés (`prop.<clé>`), avec les conditions du moteur : égal, un parmi (valeurs
répétées), entre bornes, présent, absent. [H6, 003/REQ-003-25]

Acceptation :
- `?domain=lab&prop.env=prod&prop.env=test` : objets du domaine `lab` dont `env` vaut
  `prod` ou `test`.
- Un paramètre inconnu → 400 qui le nomme.

#### REQ-006-10 — Recherche par QUERY
kartenn doit accepter la méthode `QUERY` sur les collections, avec un corps JSON portant
les mêmes conditions que REQ-006-9, et l'annoncer par `Accept-Query`. Si l'URL d'un
`GET` dépasse 8 Kio, alors kartenn doit répondre 414. [H6, RFC 10008 §2, §2.6, §3]

Acceptation :
- La même recherche par `GET` et par `QUERY` renvoie les mêmes éléments.
- `QUERY` sans `Content-Type` → 415 ; `QUERY` rejoué donne le même résultat sans clé
  d'idempotence.

### C. Écritures

#### REQ-006-11 — Changesets
kartenn doit accepter `POST /v1/changesets` : lot atomique d'opérations (004), avec
motif, mode simulation et références internes ; la réponse doit désigner le changeset
journalisé par un lien (`Location` ou `Link`, et corps), 201 s'il est appliqué, 202 s'il
est en attente d'approbation, 200 en simulation. [H2, D11, 004/REQ-004-1 à 9]

Acceptation :
- Un changeset de 3 opérations dont la troisième viole une règle : rien n'est appliqué,
  422 (REQ-006-21).
- Une simulation ne change aucun état et renvoie validation, anomalies et différentiel.

#### REQ-006-12 — Routes par ressource
kartenn doit accepter `POST` sur une collection, `PATCH` et `DELETE` (clôture) sur un
objet, chacun enveloppant un changeset d'une opération par le même pipeline ; le motif
et la période de validité doivent voyager dans le corps (membres réservés) ; la réponse
doit désigner le changeset par un lien. [H2, RFC 9205 §3.2]

Acceptation :
- Un `POST /v1/prefixes` produit un changeset visible par `GET /v1/changesets/{id}`, de
  même auteur et même motif.
- Une politique d'approbation qui s'applique → 202 et lien vers le changeset en attente.

#### REQ-006-13 — PATCH en JSON Merge Patch
kartenn doit accepter `PATCH` en `application/merge-patch+json` uniquement : les champs
envoyés sont modifiés, `null` retire un champ facultatif, un tableau est remplacé entier,
une table de propriétés se modifie clé par clé. [H13a, RFC 7396]

Acceptation :
- `{"description": null}` retire la description ; `{"properties": {"a": "x", "b": null}}`
  affecte `a` et efface `b` sans toucher aux autres clés.
- `application/json-patch+json` → 415.

#### REQ-006-14 — Concurrence par ETag
kartenn doit donner à chaque objet un ETag fort égal à sa version (dernier changeset qui
l'a modifié) et exiger `If-Match` sur `PATCH` et `DELETE` : absent → 428 avec un
problème qui explique comment resoumettre, périmé → 412. Dans `POST /v1/changesets`,
`expected` reste facultatif, par opération. [H8, RFC 9110 §13.1.1, RFC 6585 §3,
004/REQ-004-11]

Acceptation :
- Deux clients lisent le même objet, le premier le modifie ; le `PATCH` du second avec
  l'ancien ETag → 412 et l'objet porte la modification du premier.
- `PATCH` sans `If-Match` → 428.

#### REQ-006-15 — Idempotence
kartenn doit accepter l'en-tête `Idempotency-Key` sur `POST /v1/changesets` et sur les
créations, avec la sémantique de REQ-004-10 (24 h, par identité) : rejeu → la première
réponse inchangée ; même clé, contenu différent → 422 ; même clé encore en traitement →
409. La durée de rétention doit figurer dans le contrat OpenAPI. [H8, Idempotency-Key
-07]

Acceptation :
- Deux `POST` identiques avec la même clé : un seul objet créé, deux réponses égales.
- La même clé par une autre identité crée un second objet.

#### REQ-006-16 — Cycle d'un changeset
kartenn doit exposer la lecture d'un changeset (état, opérations, résultat,
différentiel), la liste des changesets filtrable par état et auteur, les propositions à
valider par l'appelant, et les actions valider, rejeter, retirer et annuler ; une action
doit exiger `If-Match` sur le changeset en attente. [004/REQ-004-19 à 24, 34, 35, 41 à
43]

Acceptation :
- La validation d'un changeset en attente par un approbateur l'applique (200) ; par son
  auteur → 403.
- L'annulation d'un changeset dont un objet a été modifié depuis → 409 qui liste les
  conflits.

### D. Imports

#### REQ-006-17 — Dépôt par morceaux
kartenn doit permettre de créer un import (`POST /v1/imports`, 201), d'y déposer des
morceaux numérotés d'au plus 10 000 éléments (un morceau renvoyé sous un numéro déjà
reçu est ignoré s'il est identique, refusé en 409 sinon), puis de le démarrer (`start`,
202 et `Location` vers l'import). [H9, 004/REQ-004-28]

Acceptation :
- Le dépôt de 10⁶ éléments en 100 morceaux, dont un renvoyé deux fois, donne un import
  de 10⁶ éléments.
- Un morceau de 10 001 éléments → 413.

#### REQ-006-18 — Exécution reprise après panne
kartenn doit exécuter un import démarré sur une seule instance à la fois, sous bail en
base ; si l'instance s'arrête, alors une autre doit reprendre au premier lot non
appliqué, sans doublon. [H9, D9, 004/REQ-004-29]

Acceptation :
- Instance arrêtée pendant un import : l'import se termine sur une autre instance, sans
  objet en double, dans le délai du bail plus le reste du travail.

#### REQ-006-19 — Suivi et contrôle
kartenn doit exposer l'état d'un import (lots appliqués, restants, rapport, échec), avec
`Retry-After` indicatif pendant l'exécution, et les actions reprendre et annuler ;
l'annulation arrête après le lot en cours et laisse les lots appliqués. [H9,
004/REQ-004-30]

### E. Erreurs

#### REQ-006-20 — Problem Details
Si une requête échoue, alors kartenn doit répondre par un problème RFC 9457 : un `type`
(URI propre à kartenn) par classe d'erreur, documenté dans le contrat OpenAPI, `title`,
`status`, `detail`, et l'identifiant de requête (012/REQ-012-6) ; aucune trace interne,
requête SQL ou valeur sensible. [H7, RFC 9457 §3, §3.1]

Acceptation :
- Une panique pendant une requête → 500, problème générique avec l'identifiant de
  requête, sans message de panique.

#### REQ-006-21 — Changeset refusé
Si un changeset est refusé par les règles, alors kartenn doit répondre 422 avec
l'extension `violations` listant toutes ses violations (règle, sévérité, détail, sujets,
indice d'opération, pointeur JSON dans la requête) et un lien vers le changeset
journalisé. [H7, RFC 9457 §3, 004/REQ-004-7]

Acceptation :
- Un changeset à deux violations de règles différentes → une seule réponse 422 avec les
  deux violations.

#### REQ-006-22 — Un problème par réponse sinon
Si une requête présente des problèmes de classes différentes, alors kartenn doit
renvoyer le plus pertinent dans l'ordre de traitement HTTP (authentification, droits,
syntaxe, préconditions, règles). [H7, RFC 9457 §3]

Acceptation :
- Une requête non authentifiée et malformée → 401.

#### REQ-006-23 — Pas d'oracle
Si l'appelant n'a pas le droit de lire un objet, alors kartenn doit répondre 404, comme
pour un objet inexistant ; 403 est réservé à une action refusée sur un objet visible
hors changeset (un droit d'écriture manquant est une violation du changeset, 422).
[H7, 005/REQ-005-19]

Acceptation :
- Même réponse (statut, type, corps hors identifiant de requête) pour un UUID invisible
  et un UUID inexistant.

### F. Authentification et sessions

#### REQ-006-24 — Routes de connexion
kartenn doit exposer la connexion OIDC (début vers l'émetteur choisi, retour), la
déconnexion, la connexion de secours (phrase de passe et code TOTP, session d'au plus
1 h) et la lecture de la session courante ; elles appellent les briques de la 005.
[005/REQ-005-4 à 8, 15]

Acceptation :
- Retour OIDC avec un `state` rejoué → refus et `authn.login.failed`.
- Une session de secours est refusée 1 h après son ouverture.

#### REQ-006-25 — Cookies
kartenn doit émettre les cookies de transaction et de session avec `__Host-`, `Secure`,
`HttpOnly`, `SameSite=Strict`, `Path=/`, sans `Domain`, les supprimer à la déconnexion
et à la fin de la connexion, et réémettre le cookie de session au plus une fois par
minute. [005/REQ-005-6, 8]

#### REQ-006-26 — Bearer et Basic
kartenn doit accepter `Authorization: Bearer` (jeton d'accès, 005/REQ-005-9) et `Basic`
(comptes de service, 005/REQ-005-12) ; un jeton refusé doit recevoir 401 avec
`WWW-Authenticate: Bearer error="invalid_token"`, une requête sans authentification 401
sans code d'erreur ; une identité juste-à-temps ne doit pas produire
`authn.login.succeeded` à chaque requête. [RFC 6750 §3, §3.1, RFC 7617 §2]

Acceptation :
- Jeton expiré → 401, `error="invalid_token"` ; aucune authentification → 401 sans
  `error`.

#### REQ-006-27 — TLS obligatoire
Si une requête n'est pas arrivée par TLS (directement, ou selon `Forwarded: proto` d'un
proxy de confiance), alors kartenn doit refuser le Basic et ne poser aucun cookie ;
l'en-tête `Forwarded` d'une autre adresse doit être ignoré. [H14, RFC 7239, RFC 7617 §4,
005/REQ-005-12]

Acceptation :
- Basic avec `Forwarded: proto=http` d'un proxy de confiance → 401 ; `proto=https`
  d'une adresse non configurée → traité comme sans TLS.

#### REQ-006-28 — CSRF
Quand une méthode non sûre est authentifiée par cookie, kartenn doit exiger
`Sec-Fetch-Site: same-origin`, ou, en son absence, un `Origin` égal à l'origine
publique ; sinon 403. Les méthodes sûres (`GET`, `HEAD`, `QUERY`) et les requêtes
Bearer ou Basic ne sont pas concernées. [H10, Fetch Metadata §2.3]

Acceptation :
- Matrice `Sec-Fetch-Site` (`same-origin`, `same-site`, `cross-site`, `none`, absent) ×
  `Origin` (égal, différent, absent) : seuls `same-origin`, et absent avec `Origin` égal,
  passent.

### G. Protection et transport

#### REQ-006-29 — Quota par identité
kartenn doit limiter le débit par identité (par adresse source avant authentification),
compartiments lecture et écriture distincts, morceaux d'import pondérés par leur taille ;
au-delà → 429, `Retry-After`, `RateLimit-Policy` et `RateLimit`, et l'événement d'audit
d'abus (012). Les limites sont par instance et configurables. [H11, RateLimit -11,
RFC 6585 §4]

Acceptation :
- Défauts : lecture 50/s rafale 200, écriture 10/s rafale 20, avant authentification
  20/s par adresse ; la 21ᵉ écriture d'une rafale immédiate → 429.

#### REQ-006-30 — Délestage
Tant que le nombre de requêtes en cours d'une instance atteint son plafond (2 × la
taille du pool par défaut), kartenn doit répondre aussitôt 503 avec `Retry-After`, sans
solliciter la base ; aucune file d'attente dans kartenn (amendé le 2026-10-18 : la
file d'attente bornée est retirée, voir H11). [H11, D14]

#### REQ-006-31 — Corps bornés
Si un corps de requête dépasse la limite configurée (16 Mio par défaut) ou porte un
`Content-Encoding`, alors kartenn doit le refuser (413, 415) sans le lire en entier.
[H14, RFC 9110 §15.5.14]

#### REQ-006-32 — Transport
kartenn doit servir HTTP/1.1 et HTTP/2, terminer TLS lui-même (rustls) ou derrière un
proxy selon la configuration, et compresser les réponses de plus de 1 Kio selon
`Accept-Encoding` (`zstd`, `br`, `gzip`, `Vary: Accept-Encoding`), sauf celles qui
montrent un secret, non compressées et `no-store`. [H14]

Acceptation :
- La création d'un secret de compte de service n'est jamais compressée et porte
  `Cache-Control: no-store`.

### H. Contrat et exploitation

#### REQ-006-33 — Versions
kartenn doit préfixer l'API par sa version majeure (`/v1`) et n'y faire que des ajouts ;
une version retirée doit annoncer `Deprecation`, `Sunset` et un lien
`rel="successor-version"` pendant son préavis. [H12, RFC 9745, RFC 8594]

#### REQ-006-34 — Contrat OpenAPI
kartenn doit servir le contrat OpenAPI 3.1 de sa build à `GET /v1/openapi.json`,
généré depuis le code ; un test doit comparer le contrat de la build toutes features au
contrat de référence du dépôt et échouer sur tout retrait ou changement de type. [H12,
D7]

#### REQ-006-35 — Capacités
kartenn doit exposer `GET /v1/capabilities` : modules actifs et versions, propriétés
déclarées, plugins UI, limites (taille de page, de changeset, de morceau, de corps,
rétention des clés d'idempotence). [D7, D17]

#### REQ-006-36 — Sondes
kartenn doit exposer `/health/live` (le processus répond) et `/health/ready` (base
joignable, migrations au niveau attendu, journal d'audit disponible), hors `/v1`, sans
authentification, sans détail interne. [D35, 012]

Acceptation :
- Base arrêtée → `ready` 503, `live` 200.

#### REQ-006-37 — Corrélation
kartenn doit attribuer un identifiant à chaque requête, le renvoyer dans un en-tête et
dans tout problème, et appliquer les règles de contexte de trace entrant de la 012.
[012/REQ-012-6, 7]

#### REQ-006-38 — Tâches de fond
kartenn doit exécuter chaque tâche de fond (imports, expiration des changesets en
attente, réévaluation des valeurs attendues, réécriture de l'indicateur `public` après
une mise à jour des registres, purge des clés d'idempotence) sur une seule instance à la
fois, sous bail en base, et la reprendre sur une autre instance si celle-ci s'arrête.
[H16, D9]

Acceptation :
- Trois instances, une tâche périodique : une seule exécution par échéance ; l'instance
  qui l'exécute est arrêtée, la tâche s'exécute ailleurs après expiration du bail.

### I. Budgets

#### REQ-006-39 — Budgets à travers HTTP
kartenn doit tenir INV-30 à INV-34 mesurés à travers HTTP (authentification cookie et
Bearer, contrôle d'accès, JSON, compression), et INV-36 (200 utilisateurs concurrents,
50 écritures/s soutenues) sans 5xx hors délestage. [H15, D14]

Acceptation :
- Banc HTTP en processus, jeu de `bench-core` : chaque p95 sous son budget ; le surcoût
  par rapport au banc du service est rapporté (cible < 10 ms au p95).

## Non-objectifs

- `fields=`, `expand=`, JSON Patch (H13) ; total des listes (H5) ; versions datées par
  requête ou dans le type de média (H12).
- HTTP/3 dans kartenn (H14) ; quotas globaux exacts entre instances (H11).
- Notifications de fin d'import et webhooks : specs des événements sortants.
- Ressource générique d'opération longue : seuls les imports sont longs (H9).
- Banc du déploiement complet en CI (H15).

## Questions ouvertes

- [ ] Séparateur des opérateurs de filtre, les clés de propriétés contenant des points
  (H6). (Claude, design)
- [ ] Borne du passé figé : durée maximale d'une transaction d'écriture et son
  application (H4). (Claude, design)
- [ ] Champs où `null` et absent différeraient, incompatibles avec Merge Patch (H13a).
  (Claude, design)
- [ ] Prise en charge de `QUERY` par axum, hyper et les proxys inverses courants (H6).
  (Claude, design, `crate-scout`)
- [ ] Crate de limitation GCRA (H11). (Claude, design, `crate-scout`)
- [x] H16 (tâches de fond sous bail) décidé par délégation. (utilisateur, à la
  relecture) — approuvé avec les exigences le 2026-10-18.

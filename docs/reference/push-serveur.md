# Canal push serveur → client : SSE, MCP, mandataires et alternatives

Note de lecture pour la spec [022-evenements-sse](../../specs/022-evenements-sse/).
Étude documentaire du 2026-10-23, faite à la demande de l'utilisateur pour l'arbitrage
022-A1 (le protocole MCP a déprécié son transport « HTTP+SSE »). Ce qui n'a pas pu être
relu à la source est marqué **à vérifier**. Errata des RFC citées : **non vérifiés**.

Le WHATWG écrit ses exigences en minuscules (« must », « should ») : elles sont
normatives quand elles décrivent un algorithme, informatives dans les notes et les
« authoring notes ». Les RFC suivent RFC 2119 / RFC 8174 (MUST, SHOULD en capitales).

## Sources

| Document | Statut | Consulté |
|---|---|---|
| [WHATWG HTML, § 9.2 Server-sent events](https://html.spec.whatwg.org/multipage/server-sent-events.html) | Living Standard | 2026-10-23 |
| [Fetch Standard](https://fetch.spec.whatwg.org/) | Living Standard | 2026-10-23 |
| [CSP Level 3](https://www.w3.org/TR/CSP3/) | Working Draft | 2026-10-23 |
| [RFC 9110](https://www.rfc-editor.org/rfc/rfc9110.html) — HTTP Semantics | STD 97 (2022) | 2026-10-23 |
| [RFC 9112](https://www.rfc-editor.org/rfc/rfc9112.html) — HTTP/1.1 | STD 99 (2022) | 2026-10-23 |
| [RFC 9113](https://www.rfc-editor.org/rfc/rfc9113.html) — HTTP/2 | Proposed Standard (2022) | 2026-10-23 |
| [RFC 6585](https://www.rfc-editor.org/rfc/rfc6585.html) — 428, 429 | Proposed Standard (2012) | 2026-10-23 |
| [RFC 10036](https://www.rfc-editor.org/rfc/rfc10036.html) — Incremental Forwarding of HTTP Messages | Proposed Standard (août 2026) | 2026-10-23 |
| [RFC 6455](https://www.rfc-editor.org/rfc/rfc6455.html) (WebSocket), [RFC 8441](https://www.rfc-editor.org/rfc/rfc8441.html) (sur HTTP/2), [RFC 9220](https://www.rfc-editor.org/rfc/rfc9220.html) (sur HTTP/3) | Proposed Standard | 2026-10-23 |
| [WHATWG WebSockets](https://websockets.spec.whatwg.org/) | Living Standard | 2026-10-23 |
| [RFC 8030](https://www.rfc-editor.org/rfc/rfc8030.html) — Web Push | Proposed Standard (2016) | 2026-10-23 |
| [RFC 8895](https://www.rfc-editor.org/rfc/rfc8895.html) — ALTO, mises à jour incrémentales par SSE | Proposed Standard (2020) | 2026-10-23 |
| [draft-dunglas-mercure-08](https://datatracker.ietf.org/doc/draft-dunglas-mercure/) | draft individuel | 2026-10-23 |
| WebTransport : [W3C](https://www.w3.org/TR/webtransport/) (Working Draft, 2026-07-06), [draft-ietf-webtrans-http3-16](https://datatracker.ietf.org/doc/draft-ietf-webtrans-http3/) (dernier appel du groupe) | brouillons | 2026-10-23 |
| [MCP, versions](https://modelcontextprotocol.io/specification/versioning) ; transports des révisions 2025-03-26, 2025-11-25, 2026-07-28 ; [SEP-2575](https://modelcontextprotocol.io/seps/2575) | spécification du protocole | 2026-10-23 |
| [PostgreSQL 18, `NOTIFY`](https://www.postgresql.org/docs/18/sql-notify.html), [`LISTEN`](https://www.postgresql.org/docs/18/sql-listen.html) | documentation officielle | 2026-10-23 |
| [Module mandataire, `proxy_buffering`](https://nginx.org/en/docs/http/ngx_http_proxy_module.html#proxy_buffering) | documentation officielle du logiciel | 2026-10-23 |
| [OpenAPI 3.2.0](https://spec.openapis.org/oas/v3.2.0) | spécification | **à vérifier** |

## Ce que disent les sources

### SSE (WHATWG HTML § 9.2)

- Format : `text/event-stream`, « must always be encoded as UTF-8 » (§ 9.2.5) ; champs
  `event`, `data`, `id`, `retry`, comparés littéralement ; ligne commençant par `:`
  ignorée (commentaire) (§ 9.2.6).
- `id` : pris seulement s'il ne contient pas U+0000 (§ 9.2.6) ; l'en-tête
  `Last-Event-ID` n'est envoyé à la reconnexion que si l'identifiant n'est pas vide,
  sans NUL, LF ni CR (§ 9.2.3, étape 5.3 de « reestablish the connection » ; § 9.2.4).
  L'espace des valeurs reste en discussion (whatwg/html #7363).
- `retry` : appliqué seulement s'il ne contient que des chiffres ASCII, en
  millisecondes (§ 9.2.6) ; délai initial « implementation-defined », attente
  supplémentaire permise (§ 9.2.3).
- Échec : « if res's status is not 200, or if res's "Content-Type" is not
  "text/event-stream", then fail the connection » (§ 9.2.2, étape 15.3) ; « Once the
  user agent has failed the connection, it does not attempt to reconnect » (§ 9.2.3).
  L'introduction (§ 9.2.1, informative) cite 204 pour arrêter les reconnexions.
- Requête : constructeur sans en-têtes ni méthode ; `withCredentials` faux par défaut,
  donc cookies envoyés en même origine seulement (§ 9.2.2) ; mode de cache `no-store`
  (étape 11) ; « potential-CORS request » (étape 8).
- Mise à jour de l'identifiant par un bloc `id:` sans `data:` : l'algorithme « dispatch
  the event » met à jour le dernier identifiant avant de tester le tampon de données
  vide (§ 9.2.6) — **à vérifier** mot à mot.
- Notes d'auteur (§ 9.2.7, informatives) : commentaire de maintien « every 15 seconds or
  so » contre les mandataires qui coupent les connexions muettes ; désactiver le
  découpage ; plusieurs pages d'un même site peuvent épuiser les connexions (piste :
  worker partagé).
- Pas de contre-pression applicative : seul le contrôle de flux du transport joue
  (RFC 9113 § 5.2).

### Limites de connexions

- RFC 9112 § 9.4 : « A client ought to limit the number of simultaneous open
  connections that it maintains to a given server » ; aucun maximum imposé.
- La limite de six connexions par origine en HTTP/1.1 est une pratique des navigateurs,
  absente du WHATWG et des RFC ; documentée par la documentation de référence des
  développeurs web (page « Using server-sent events », § Connection limit, non
  normative).
- RFC 9113 § 6.5.2 : `SETTINGS_MAX_CONCURRENT_STREAMS`, sans limite initiale, « It is
  recommended that this value be no smaller than 100 » ; dépassement → `PROTOCOL_ERROR`
  ou `REFUSED_STREAM` (§ 5.1.2).

### Mandataires

- RFC 10036 (§ 3-4) : en-tête `Incremental: ?1` ; un intermédiaire SHOULD NOT mettre
  le message en tampon et, s'il refuse, MUST répondre en erreur plutôt que de le
  retenir. Déploiement réel : **à vérifier**.
- `X-Accel-Buffering: no` : en-tête non standard d'un mandataire répandu, qui désactive
  sa mise en tampon (`proxy_buffering`, désactivable par `proxy_ignore_headers`).

### Statuts et quotas

- RFC 9110 § 15.6.4 : 503, « The server MAY send a Retry-After header field » ;
  § 10.2.3 : `Retry-After` en date HTTP ou en secondes.
- RFC 6585 § 4 : 429, détails SHOULD, `Retry-After` MAY.

### Fetch et CSP

- `Last-Event-ID` n'est ni « no-CORS-safelisted » ni interdit (Fetch, listes
  correspondantes) : posé à la main en inter-origines, il déclencherait une requête
  préalable. Section « CORS-safelisted request-header » : **à vérifier**.
- En-tête `Origin` : envoyé sur une requête de mode CORS ou de méthode non sûre ; pour
  un `GET` de même origine par `EventSource`, sa présence est **à vérifier** (Fetch,
  « append a request Origin header »).
- CSP3 § 6.1.2 `connect-src` : « This includes APIs like fetch(), XHR, EventSource,
  Beacon, and a's ping » — couverture d'`EventSource` **à vérifier** sur la page
  rendue.

### MCP

- 2025-03-26 (transports, § Streamable HTTP) : « This replaces the HTTP+SSE transport
  from protocol version 2024-11-05 » ; « The server MUST provide a single HTTP endpoint
  path (…) that supports both POST and GET methods » ; sessions MAY ; reprise MAY par
  `id` et `Last-Event-ID`, par flux, « MUST NOT replay messages that would have been
  delivered on a different stream ». Motif officiel de cette dépréciation : **à
  vérifier**.
- 2025-11-25 (transports) : « SHOULD immediately send an SSE event consisting of an
  event ID and an empty data field » ; le client « MUST respect the retry field » ;
  « Servers MUST validate the Origin header on all incoming connections to prevent DNS
  rebinding attacks », sinon 403 ; « MUST NOT broadcast the same message across
  multiple streams ».
- 2026-07-28 (révision courante ; `basic/transports/streamable-http`, changelog § Major
  changes 1, 4, 9 ; `patterns/subscriptions` § Acknowledgment) : un seul point d'accès
  en `POST` ; réponse JSON ou `text/event-stream` liée à la requête ;
  `subscriptions/listen` ouvre un flux dont le premier message MUST être
  `notifications/subscriptions/acknowledged` ; « Resumable SSE streams via
  Last-Event-ID are not supported » ; `GET`/`DELETE` d'anciens clients SHOULD 405 ;
  `Origin` MUST (403) ; `X-Accel-Buffering: no` SHOULD.
- SEP-2575 (§ Motivation, § Resumable Streams Are Removed) : « difficult to run MCP at
  scale (…) sticky sessions » ; « resuming would require the server to retain
  per-request state ». C'est un rejet de l'**état par session** côté serveur, pas de SSE
  comme format : les réponses restent en `text/event-stream`.

### Autres candidats

- WebSocket : RFC 6455 (§ 10.2 : vérifier `Origin`, 403 ; § 10.5 : aucune
  authentification prescrite) ; API WHATWG (§ 3.1) sans en-têtes, cookie envoyé (risque
  inter-sites), pas de contre-pression (fermeture si le tampon déborde). Bidirectionnel :
  surdimensionné pour du push. « Updated by RFC 9220 » sur RFC 6455 : **à vérifier** ;
  statut de `WebSocketStream` : **à vérifier**.
- WebTransport : brouillons, HTTP/3 de bout en bout requis : écarté.
- Interrogation longue : motif sans norme ; une requête par événement ; repli possible.
- HTTP/2 Server Push : alimente le cache, pas le JavaScript ; désactivable (RFC 9113
  § 8.4) et retiré des principaux navigateurs : inutilisable.
- Web Push (RFC 8030) : réveil d'un client absent par un service tiers ; hors sujet pour
  une page ouverte.
- Précédents de SSE normalisé : RFC 8895 (ALTO, mises à jour incrémentales) ;
  draft-dunglas-mercure-08 (individuel) : `Authorization` recommandé, `last_event_id`
  en paramètre, refus d'un jeton d'accès en paramètre d'URL, droits revérifiés à la
  relecture.
- Aucune norme IETF ne définit `text/event-stream` : le WHATWG fait foi.

### PostgreSQL 18 : `LISTEN` / `NOTIFY`

- Charge « shorter than 8000 bytes » ; remise à la validation seulement ; notifications
  identiques d'une transaction fusionnées ; file de 8 Go, `NOTIFY` échoue à la
  validation si elle est pleine ; garder les transactions courtes (`NOTIFY`).
- `LISTEN` prend effet à la validation ; inscriptions effacées en fin de session ;
  ordre sûr : `LISTEN` validé, puis lecture de l'état, puis notifications (`LISTEN`).

## Pièges

- Un statut autre que 200 (401, 429, 503, 400) arrête le client natif sans reconnexion
  ni lecture de `Retry-After` : la reprise d'un client natif doit passer par 200 suivi
  d'un événement (`reset`), et les clients qui veulent honorer `Retry-After` lisent le
  flux par `fetch`.
- Un `id:` vide remet à zéro le dernier identifiant : la reconnexion suivante part sans
  `Last-Event-ID`.
- Un mandataire ou un compresseur qui met en tampon retient les événements sans erreur
  visible ; le maintien ne passe plus, la connexion est coupée au délai de lecture.
- `NOTIFY` est perdu si la connexion d'écoute tombe : il faut une relecture de secours.
- MCP 2026-07-28 n'a plus de reprise par `Last-Event-ID` : un point d'accès MCP ne peut
  pas être le flux `/v1/events`, dont la reprise fait partie du contrat.

## Choix de kartenn (arbitrage 022-A1, 2026-10-23)

Ce qui suit n'est pas exigé par les sources ; ce sont des choix, consignés dans
[`../../specs/022-evenements-sse/decisions.md`](../../specs/022-evenements-sse/decisions.md).

1. Un seul point d'accès `GET /v1/events` (`text/event-stream`), partie de l'API HTTP
   `/v1` (D7 précisé).
2. Web : `EventSource` avec le cookie BFF ; `Origin` vérifié (403), `Sec-Fetch-Site` ;
   CSP `connect-src 'self'`.
3. CLI et bibliothèque Python : lecture du même flux par une réponse HTTP en continu,
   en-tête `Authorization`.
4. Reprise : curseur opaque chiffré et `reset` ; droits revérifiés à la relecture
   (comme le draft Mercure). L'état de reprise est l'outbox, déjà en base : pas d'état
   par session, pas d'affinité (le reproche de SEP-2575 ne s'applique pas).
5. Maintien : commentaire `:` toutes les 15 s, `retry:` ; en-têtes `Cache-Control:
   no-store`, `X-Accel-Buffering: no`, `Incremental: ?1` en option.
6. HTTP/2 recommandé jusqu'au mandataire ; une connexion par navigateur, partagée entre
   onglets.
7. File bornée par abonné ; retard → `reset`, puis fermeture.
8. MCP futur : adaptateur séparé (`POST /mcp`, Streamable HTTP 2026-07-28,
   `subscriptions/listen`) branché sur la même source d'événements ; perte du flux →
   réabonnement et relecture (équivalent du `reset`). `/v1/events` n'est jamais le point
   d'accès MCP.

## À vérifier

Motif officiel de la dépréciation MCP du 2025-03-26 ; couverture d'`EventSource` par
`connect-src` (CSP3) ; `Origin` sur un `GET` de même origine par `EventSource` ;
comportement d'`EventSource` face à un 401 et en CORS avec cookie ; mise à jour de
l'identifiant par un bloc sans donnée ; « Updated by RFC 9220 » sur RFC 6455 ; statut de
`WebSocketStream` ; déploiement de RFC 10036 ; OpenAPI 3.2 (`itemSchema`) ; capacité
serveur (banc `bench-events`).

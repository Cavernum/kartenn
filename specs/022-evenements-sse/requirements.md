---
status: draft
updated: 2026-10-23
---

# 022 — Événements poussés (SSE) : exigences

## Résumé

kartenn pousse à ses clients, par un flux **server-sent events** (WHATWG HTML § 9.2),
les changements de son référentiel : le futur shell web (specs 020 et 021, à venir), la
future bibliothèque cliente et ses commandes `watch`, les connecteurs. Un client tient
ainsi ses caches à jour **sans interroger l'API en boucle** : l'événement ne transporte
que des **clés et des révisions**, jamais les objets ; le client relit ce qui l'intéresse
par l'API REST, avec ses ETag (REQ-006-6, 14). Le flux est alimenté par l'**outbox
ordonnée** de la 004 (ADR 0018), lue **une fois par instance** et distribuée aux
connexions ouvertes, filtrée par les droits de chaque abonné sans révéler l'existence
d'un objet caché.

Sources : besoin exprimé le 2026-10-23 (invalidation des caches en temps réel) ;
décisions [`decisions.md`](decisions.md) (R1 à R22) ; D3, D7, D9, D11, D13, D14, D17,
D35, D36 ([`../001-cadrage/decisions.md`](../001-cadrage/decisions.md)) ; specs 004
(outbox, REQ-004-36, 37), 005 (sessions, jetons, RBAC), 006 (contrat HTTP, quotas,
délestage), 012 (audit, télémétrie), 016 (clés d'ordre, REQ-016-14, 15) ; ADR 0018,
0020, 0022. Sources primaires : § Références de [`design.md`](design.md).

**Brouillon.** Rien n'est implémenté ; les décisions R1 à R22 sont des recommandations
à valider, et la section « Arbitrages demandés » de [`decisions.md`](decisions.md)
liste ce qui contredit ou amende une décision ou une spec existante.

### Vocabulaire

- **Flux d'événements** : réponse `text/event-stream` de `GET /v1/events`, tenue
  ouverte ; une **connexion** en est une instance.
- **Abonné** : identité authentifiée qui tient une connexion (personne par le cookie de
  session, compte de service par Basic, client par jeton Bearer).
- **Lecteur partagé** : composant d'une instance qui lit l'outbox et distribue chaque
  événement à toutes les connexions de l'instance.
- **Anneau** : mémoire bornée, par instance, des derniers événements lus, qui sert les
  reprises.
- **Curseur** : valeur opaque du champ `id:` d'un événement SSE ; il dit jusqu'où
  l'abonné a reçu le flux et sert de `Last-Event-ID` à la reconnexion.
- **Plancher** : numéro d'outbox sous lequel le lecteur partagé a tout vu (même règle
  que `pass_floor`, migration 0027, ADR 0018 amendement 3).
- **Remise à zéro** (`reset`) : événement qui dit à l'abonné que sa vue n'est plus
  continue et qu'il doit invalider tous ses caches.
- **Révision** : ETag fort d'un objet, c'est-à-dire l'identifiant du dernier changeset
  qui a écrit l'objet ou ses propriétés (006, `object_version`).

## Exigences

### A. Flux et format

#### REQ-022-1 — Point d'accès
kartenn doit servir `GET /v1/events`, qui répond `200` avec `Content-Type:
text/event-stream` et tient la réponse ouverte ; la requête porte `Accept:
text/event-stream` (ou `*/*`), sinon 406. [R1, WHATWG HTML § 9.2.2, D7]

Acceptation :
- `GET /v1/events` authentifié → 200, `text/event-stream`, premier événement `ready`
  reçu en moins de 1 s.
- `Accept: application/json` → 406 (Problem Details, REQ-006-20).

#### REQ-022-2 — Filtres
kartenn doit accepter des filtres facultatifs, combinés par ET : `types` (types d'objet
qualifiés, `core.prefix`, `vlan.vlan`…, 32 au plus), `domains` (domaines d'adressage,
32 au plus), `objects` (identifiants d'objets surveillés, 100 au plus), `kinds` (sortes
d'événement, REQ-022-3). Un filtre inconnu ou hors bornes doit être refusé (400) à
l'ouverture ; un type d'un module absent aussi. Un filtre ne doit jamais élargir ce que
les droits laissent voir. [R4, REQ-006-9]

Acceptation :
- `types=vlan.vlan` : un changeset qui ne touche que des préfixes ne produit rien sur
  ce flux.
- `objects=<id>` : seul un changement de cet objet produit un événement `changes`.
- `types=inconnu.truc` → 400 (`events.filter.invalid`).

#### REQ-022-3 — Sortes d'événement
kartenn doit émettre les sortes (champ `event:`) suivantes, et aucune autre sans
amender cette spec : `ready` (ouverture), `changes` (objets écrits par un changeset ou
une tâche de fond), `changeset` (état d'un changeset en attente, pour son auteur et ses
valideurs possibles), `reset` (remise à zéro), et des **points de reprise** sans donnée
(REQ-022-9). [R2, REQ-004-36, 43]

#### REQ-022-4 — Charge utile minimale
La donnée (`data:`) d'un événement doit être un objet JSON sur **une seule ligne** qui ne
porte que des clés et des révisions : pour `changes`, l'identifiant du changeset (la
révision des objets qu'il a écrits), la liste des objets visibles (type, identifiant) et
les domaines visibles ; pour `changeset`, l'identifiant et le nouvel état. Elle ne doit
porter ni valeur d'attribut ou de propriété, ni différentiel, ni auteur, ni motif.
[R3, WHATWG HTML § 9.2.5, § 9.2.6]

Acceptation :
- Un changeset qui renomme le préfixe P produit `{"changeset":"<C>","objects":[{"type":
  "core.prefix","id":"<P>"}],"domains":["<D>"]}` ; `GET /v1/prefixes/<P>` répond alors
  avec `ETag: "<C>"`.
- Aucune donnée d'événement ne contient de saut de ligne.

#### REQ-022-5 — Taille bornée et invalidation large
Si un changeset écrit plus d'objets visibles que la borne d'un événement (1 000 par
défaut, réglable), alors kartenn doit remplacer la liste par une **invalidation large**
(types et domaines touchés, `"truncated": true`) ; les effets dérivés que la liste ne
nomme pas (valeurs héritées, reclassification, réévaluations de flux) doivent être
signalés de la même façon, par type. [R5, D34, 003, 011]

Acceptation :
- Un import de 10⁴ adresses en un changeset produit un seul événement `changes` avec
  `"truncated": true` et `types: ["core.address"]`.
- L'affectation d'une propriété héritable sur un préfixe signale `core.address` (valeurs
  héritées) sans lister les adresses du sous-arbre.

### B. Curseur et reprise

#### REQ-022-6 — Curseur
Chaque événement porteur de donnée et chaque point de reprise doit porter un champ
`id:` : un curseur opaque, authentifié, sans `U+0000`, `LF` ni `CR`, qui encode la
position du flux dans l'outbox (plancher du lecteur et dernier numéro remis). Un
curseur altéré ou émis pour une autre identité doit être traité comme trop ancien
(REQ-022-8). [R6, WHATWG HTML § 9.2.4, § 9.2.6, ADR 0018]

#### REQ-022-7 — Reprise par `Last-Event-ID`
Quand une requête porte `Last-Event-ID` (en-tête) ou, à défaut, le paramètre
`last_event_id`, kartenn doit reprendre le flux **sans trou** à partir de ce curseur, sur
n'importe quelle instance : tout événement validé après la position du curseur et
visible de l'abonné doit lui être remis, au moins une fois, dans l'ordre d'application
des changesets d'une même clé d'ordre (REQ-004-37, REQ-016-14). Un événement peut être
remis deux fois ; la révision permet au client de l'ignorer. [R6, R7, D9, WHATWG HTML
§ 9.2.3]

Acceptation :
- Coupure réseau pendant trois changesets, reconnexion sur une autre instance avec le
  dernier curseur : les trois sont remis, dans l'ordre pour un même domaine.
- Une transaction longue détient le numéro 5 pendant que 6 est remis ; à sa validation,
  5 est remis, y compris à un abonné reconnecté entre-temps avec un curseur postérieur
  à 6.

#### REQ-022-8 — Position trop ancienne
Si le curseur présenté est antérieur à ce que l'anneau de l'instance peut rejouer, ou
invalide, alors kartenn doit envoyer, avant tout autre événement, un événement `reset`
(raison `too_old` ou `invalid_cursor`) portant un curseur courant, puis continuer le flux
à partir de là. [R7, R8]

Acceptation :
- Curseur d'il y a deux jours avec un anneau de 15 min → `reset` `too_old`, puis les
  événements suivants.
- Curseur dont un octet est modifié → `reset` `invalid_cursor`, jamais 400 (le client
  natif `EventSource` ne se reconnecterait plus).

#### REQ-022-9 — Points de reprise
kartenn doit faire avancer le curseur d'un abonné même quand aucun événement ne lui est
visible : au plus toutes les 30 s, et seulement si sa position a avancé, il émet un
point de reprise (champ `id:` seul, sans `data:`), qui met à jour le dernier
identifiant du client sans lui remettre d'événement. [R9, WHATWG HTML § 9.2.6]

Acceptation :
- Abonné filtré sur `vlan.vlan`, 2 h sans changement de VLAN mais 10⁴ changesets
  d'adresses : sa reconnexion reprend sans `reset`.

### C. Vie de la connexion

#### REQ-022-10 — Maintien
kartenn doit écrire une ligne de commentaire (`:`) sur chaque connexion restée muette
15 s (réglable, de 5 à 60 s). [R10, WHATWG HTML § 9.2.7]

#### REQ-022-11 — Délai de reconnexion
kartenn doit envoyer un champ `retry:` dans l'événement `ready`, tiré au hasard dans un
intervalle réglable (1 000 à 3 000 ms par défaut), pour étaler les reconnexions.
[R10, WHATWG HTML § 9.2.3, § 9.2.6]

#### REQ-022-12 — Arrêt propre
Quand une instance s'arrête, kartenn doit refuser les nouveaux flux (503,
`Retry-After`), envoyer à chaque connexion un `retry:` tiré au hasard (0,5 à 5 s), puis
la fermer, le tout dans le délai d'arrêt de l'instance, sans attendre que les clients
ferment. [R11, D9, RFC 9110 § 10.2.3]

Acceptation :
- 1 000 connexions ouvertes, arrêt demandé : toutes fermées en moins de 2 s ; les
  clients se reconnectent sur l'autre instance, étalés, sans trou (REQ-022-7).

#### REQ-022-13 — Intermédiaires
kartenn doit servir le flux avec `Cache-Control: no-store`, `X-Accel-Buffering: no`,
sans compression ni `Content-Length`, et écrire chaque événement sans tampon ; le guide
de déploiement doit dire comment désactiver la mise en tampon et allonger le délai de
lecture d'un mandataire inverse. [R12, REQ-006-32, D35]

Acceptation :
- `Accept-Encoding: gzip, br, zstd` : la réponse n'a pas de `Content-Encoding`.

#### REQ-022-14 — HTTP/2
kartenn doit servir le flux en HTTP/1.1 et en HTTP/2 et annoncer en HTTP/2 au moins 100
flux concurrents par connexion. Le guide de déploiement doit recommander HTTP/2 de bout
en bout pour les navigateurs. [R13, RFC 9113 § 6.5.2, RFC 9112 § 9.4]

### D. Autorisation

#### REQ-022-15 — Authentification
kartenn doit authentifier l'ouverture du flux comme toute requête `/v1` : cookie de
session (navigateur), Basic (compte de service), Bearer (jeton d'accès) ; sans
authentification valide → 401, sans jamais ouvrir le flux. Le flux n'exige pas la
protection CSRF des méthodes non sûres (`GET`), mais une requête authentifiée par cookie
dont `Sec-Fetch-Site` n'est ni `same-origin` ni absent doit être refusée (403). [R14,
REQ-005-6, 9, 12, REQ-006-26, 28]

Acceptation :
- `Sec-Fetch-Site: cross-site` avec un cookie valide → 403 ; avec Bearer → 200.

#### REQ-022-16 — Filtrage par les droits, sans oracle
kartenn doit remettre à un abonné un objet d'un événement si et seulement si l'abonné a,
**au moment de la remise**, la permission de lecture de ce type sur cet objet (D36,
REQ-005-24) ; un événement dont aucun objet n'est visible ne doit laisser aucune trace
(ni événement, ni curseur qui trahisse son existence, ni délai mesurable propre à lui).
Les domaines d'un événement suivent la même règle. [R15, R16, REQ-006-23, INV-63]

Acceptation :
- Un opérateur externe lié à l'entité E ne reçoit rien d'un changeset qui ne touche que
  l'entité F ; son curseur n'avance qu'aux points de reprise.
- Un changeset qui touche un objet visible et un objet caché : l'événement ne nomme que
  le visible.
- Test de propriété : pour un échantillon de changesets et d'abonnés, les objets remis
  coïncident avec la décision unitaire de lecture.

#### REQ-022-17 — Changesets en attente
kartenn doit remettre l'événement `changeset` d'un changeset en attente à son auteur et
aux abonnés qui pourraient le valider (REQ-004-41 à 43), et à eux seuls. [R2, R15]

#### REQ-022-18 — Révocation pendant la connexion
kartenn doit fermer, dans les 10 s, toute connexion dont l'identité est désactivée, la
session révoquée (REQ-005-3), le jeton expiré ou le compte de service révoqué ; et
envoyer `reset` (raison `rights_changed`) aux connexions d'un abonné dont les liaisons
ou les rôles changent. Une connexion par cookie doit être fermée au plus tard à
l'échéance de sa session (inactivité ou durée absolue, REQ-005-7) ; le flux ne compte
pas comme une activité. [R17, REQ-005-3, 7, D13]

Acceptation :
- Révocation de la session d'une personne connectée : son flux est fermé en moins de
  10 s ; la reconnexion reçoit 401 ; `authn.session.closed` (cause `revoked`).
- Retrait d'une liaison : `reset` puis plus aucun objet de l'ancien périmètre.

#### REQ-022-19 — Même origine
Le flux du navigateur doit être servi par l'origine publique de kartenn ; kartenn ne doit
pas répondre aux requêtes CORS sur `/v1/events` (aucun `Access-Control-Allow-Origin`),
et la CSP du shell (`connect-src 'self'`) doit suffire à l'atteindre. [R14, INV-23,
CSP3 § 6.1.2]

### E. Ressources du serveur

#### REQ-022-20 — Un lecteur par instance
kartenn doit lire l'outbox **une fois par instance**, quel que soit le nombre de
connexions : aucune requête à la base par connexion ni par événement remis, hors
l'authentification à l'ouverture et les contrôles groupés de REQ-022-16 et 18. La
détection d'un nouvel événement doit s'appuyer sur `LISTEN`/`NOTIFY` avec une
interrogation de secours bornée (1 s par défaut). [R18, R19, D9, ADR 0018]

Acceptation :
- 1 000 connexions et 50 changesets/s : le nombre de requêtes du lecteur par seconde est
  le même qu'avec une connexion (à 10 % près).
- `NOTIFY` perdu (connexion d'écoute coupée) : l'événement est remis au plus tard après
  l'interrogation de secours.

#### REQ-022-21 — Tampon borné, abonné lent
kartenn doit donner à chaque connexion un tampon borné (256 événements par défaut) ;
s'il est plein, kartenn doit abandonner les événements en attente de cette connexion,
lui envoyer `reset` (raison `overflow`) dès qu'elle peut écrire, et la fermer si elle ne
peut toujours pas écrire après 30 s. Un abonné lent ne doit jamais retarder le lecteur
ni les autres abonnés. [R20, INV-56]

Acceptation :
- Un client qui ne lit plus : les autres connexions reçoivent leurs événements dans le
  budget de latence ; le client lent reçoit `reset` `overflow` en reprenant sa lecture,
  ou est fermé à 30 s.

#### REQ-022-22 — Plafonds et quotas
kartenn doit plafonner les flux ouverts par instance (5 000 par défaut, réglable) et par
identité (16 par défaut) ; au-delà : 503 avec `Retry-After` pour l'instance, 429 avec
`Retry-After` pour l'identité, sans solliciter la base. Les flux ouverts ne doivent pas
compter dans le plafond des requêtes en cours du délestage (REQ-006-30) ; leur ouverture
compte une fois dans le compartiment de lecture du quota (REQ-006-29). [R21, H11,
RFC 6585 § 4, RFC 9110 § 15.6.4]

Acceptation :
- 5 000 flux ouverts : le 5 001ᵉ → 503 ; une requête de lecture ordinaire passe.
- 17ᵉ flux d'une même identité → 429.

### F. Contrat, audit, télémétrie

#### REQ-022-23 — Capacités et OpenAPI
kartenn doit annoncer dans `GET /v1/capabilities` si le flux est actif, ses bornes
(filtres, taille d'un événement, plafond par identité) et la durée de l'anneau ; et
décrire `GET /v1/events` dans le contrat OpenAPI (REQ-006-34) : paramètres, réponses
d'erreur, type de média `text/event-stream` et schéma JSON de la donnée de chaque sorte
d'événement. [R22, REQ-006-34, 35]

#### REQ-022-24 — Audit
kartenn doit enregistrer `events.stream.opened` (filtres) et `events.stream.closed`
(cause, durée, nombre d'événements remis, de `reset`, d'identifiants remis) pour chaque
connexion, en criticité `low` ; les refus suivent REQ-012-8 (`authz.access.denied`,
abus). Il ne doit pas produire d'événement d'audit par événement remis. [R16, REQ-012-8,
18 — voir l'arbitrage A4]

#### REQ-022-25 — Télémétrie
kartenn doit exposer les métriques `kartenn.events.streams` (connexions ouvertes),
`kartenn.events.delivered`, `kartenn.events.resets{reason}`,
`kartenn.events.dropped`, `kartenn.events.lag` (histogramme, de la validation du
changeset à l'écriture sur la connexion) et `kartenn.events.reader.duration`, et une
span par passe du lecteur, jamais une par connexion ni par événement. [D35, REQ-012-2]

### G. Performance

#### REQ-022-26 — Budgets
kartenn doit tenir, sur le jeu de référence, à 50 changesets unitaires par seconde et
2 000 flux ouverts par instance (dont 200 identités distinctes) :
- latence de la validation du changeset à l'écriture de l'événement sur la connexion :
  p95 < 500 ms, p99 < 1 s ;
- 5 000 flux ouverts par instance, mémoire ajoutée < 64 Kio par flux au repos ;
- reprise de 5 000 abonnés en 10 s (redémarrage d'une instance) servie par l'anneau,
  sans dépasser INV-34 pour les écritures concurrentes ;
- surcoût du lecteur sur le p95 du changeset unitaire < 5 % (`NOTIFY` compris).
[R18, D14, INV-34, INV-36]

## Non-objectifs

- WebSocket, flux bidirectionnel, abonnement à une requête arbitraire (filtre `QUERY`).
- Notifications aux personnes (courriel, messagerie) : consommateurs de l'outbox à part.
- Webhooks sortants vers des tiers : relais d'outbox par consommateur (ADR 0018), autre
  spec.
- Contenu des objets dans les événements, différentiel, valeurs anciennes et nouvelles.
- Lecture « à la date X » d'un flux d'événements ; relecture de l'histoire au-delà de
  l'anneau (l'audit et les changesets l'offrent déjà).
- Le client : SDK TypeScript (specs 020 et 021, à venir), bibliothèque cliente et
  commandes `watch` (spec à venir) ; seul leur **contrat** est décrit
  ([`design.md`](design.md), § Contrat consommé par les clients).

## Questions ouvertes

Voir [`decisions.md`](decisions.md), § Questions ouvertes (Q-022-1 à Q-022-12) et
§ Arbitrages demandés (A1 à A8).

---
status: approved
updated: 2026-10-29
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
décisions [`decisions.md`](decisions.md) (R1 à R25) ; D3, D7 et D31 (amendées le
2026-10-23), D9, D11, D13, D14, D17, D35, D36
([`../001-cadrage/decisions.md`](../001-cadrage/decisions.md)) ; note de référence
[`push-serveur.md`](../../docs/reference/push-serveur.md) ; specs 004
(outbox, REQ-004-36, 37), 005 (sessions, jetons, RBAC), 006 (contrat HTTP, quotas,
délestage), 012 (audit, télémétrie), 016 (clés d'ordre, REQ-016-14, 15) ; ADR 0018,
0020, 0022. Sources primaires : § Références de [`design.md`](design.md).

**Validée le 2026-10-23** : l'utilisateur a arbitré A1 à A8 et retenu les réponses
recommandées aux questions Q-022-1 à 12 ([`decisions.md`](decisions.md)). Implémentée le
2026-10-23 (tâches 2 à 13, 15 à 18), budgets mesurés à l'échelle réduite seulement.
Rattrapage JSON (Q-022-4, tâche 14) retenu par l'utilisateur le 2026-10-29 : REQ-022-28,
qui amende REQ-022-1, implémentée le même jour.

**Lecteurs du flux.** Un seul flux, deux façons de le lire : le shell web par
`EventSource` (cookie de session BFF, `Origin` vérifié) ; la CLI, la bibliothèque Python
et les connecteurs par une réponse HTTP lue en continu, avec l'en-tête `Authorization`.
Un futur serveur MCP est un adaptateur distinct (R23), jamais ce point d'accès.

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
*Amendée le 2026-10-29 (REQ-022-28, Q-022-4) : `Accept: application/json` demande une
page de rattrapage.*
kartenn doit servir `GET /v1/events`, qui répond `200` avec `Content-Type:
text/event-stream` et tient la réponse ouverte ; la requête porte `Accept:
text/event-stream` (ou `*/*`), ou `application/json` pour une page de rattrapage
(REQ-022-28), sinon 406. [R1, WHATWG HTML § 9.2.2, D7]

Acceptation :
- `GET /v1/events` authentifié → 200, `text/event-stream`, premier événement `ready`
  reçu en moins de 1 s.
- `Accept: text/html` → 406 (Problem Details, REQ-006-20).

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
*Amendée par la 018 (REQ-018-18, 2026-10-23) : sorte `labels`.*
kartenn doit émettre les sortes (champ `event:`) suivantes, et aucune autre sans
amender cette spec : `ready` (ouverture), `changes` (objets écrits par un changeset ou
une tâche de fond), `changeset` (état d'un changeset en attente, pour son auteur et ses
valideurs possibles), `reset` (remise à zéro), et des **points de reprise** sans donnée
(REQ-022-9). [R2, REQ-004-36, 43]

#### REQ-022-4 — Charge utile minimale
La donnée (`data:`) d'un événement doit être un objet JSON sur **une seule ligne** qui ne
porte que des clés et des révisions : pour `changes`, l'identifiant du changeset (la
révision des objets qu'il a écrits), la liste des objets visibles (type, identifiant) et
les domaines visibles ; pour un événement de tâche de fond, l'identifiant CloudEvents
de l'événement ; pour `changeset`, l'identifiant et le nouvel état. Elle ne doit
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
des changesets d'une même clé d'ordre (REQ-004-37, REQ-016-14). Garantie : **aucun
événement perdu, quelques doublons possibles** ; les cas de doublon et leur
reconnaissance sont ceux de REQ-022-27. [R6, R7, D9, WHATWG HTML § 9.2.3]

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
kartenn doit servir le flux avec `Cache-Control: no-store`, `X-Accel-Buffering: no`, et
`Incremental: ?1` si la configuration le demande (`[events] incremental`, faux par
défaut ; RFC 10036), sans compression ni `Content-Length`, et écrire chaque événement sans tampon ; le guide
de déploiement doit dire comment désactiver la mise en tampon et allonger le délai de
lecture d'un mandataire inverse. [R12, REQ-006-32, D35, RFC 10036 § 3-4]

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
authentification valide → 401, sans jamais ouvrir le flux. Une requête authentifiée par
cookie doit être refusée (403) si elle porte un `Origin` différent de l'origine publique,
ou un `Sec-Fetch-Site` autre que `same-origin` ; un `Origin` absent n'est accepté
qu'avec `Sec-Fetch-Site: same-origin` ou sans en-tête de métadonnées (présence d'`Origin`
sur un `GET` de même origine : à vérifier, note de référence). Les requêtes Basic et
Bearer ne sont pas soumises à ce contrôle. [R14, REQ-005-6, 9, 12, REQ-006-26, 28,
MCP 2025-11-25 transports (même règle d'`Origin`)]

Acceptation :
- Cookie valide avec `Origin` d'un autre site → 403 ; `Sec-Fetch-Site: cross-site` → 403 ;
  avec Bearer → 200.

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
kartenn doit enregistrer `events.stream.opened` (acteur, filtres) et
`events.stream.closed` (cause, durée, nombre d'événements remis, de `reset`, et les
identifiants des objets remis, bornés par `audit.read_ids_max`, ceux au-delà comptés
dans `targets_omitted`) pour chaque connexion, en criticité `low` ; les refus suivent
REQ-012-8 (`authz.access.denied`, abus). Il ne doit pas produire d'événement d'audit par
événement remis : le flux ne porte que des notifications, et toute lecture de contenu
ou écriture passe par l'API REST, auditée par requête (REQ-012-18, amendée par A4).
Quand les identifiants d'une connexion dépassent la borne, chaque remise au-delà doit
être journalisée dans les **logs techniques**, au niveau DEBUG, sous le nom
`events.delivery` (connexion, acteur, sorte, identifiants remis), hors du journal
d'audit scellé, avec une rétention courte réglée sur la sortie (7 jours recommandés).
[R16, R24, REQ-012-8, 18, L25]

Acceptation :
- `audit.read_ids_max = 2`, trois événements de deux objets remis : `events.stream.closed`
  liste 2 identifiants et `targets_omitted = 4` ; deux enregistrements DEBUG
  `events.delivery` nomment les 4 autres ; aucun n'entre dans le journal d'audit.

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

### H. Doublons

#### REQ-022-27 — Doublons reconnus
kartenn peut remettre deux fois un même événement dans ces seuls cas : (a) reprise par
un curseur dont le plancher est sous le dernier numéro remis (événements entre les
deux, déjà reçus) ; (b) reprise sur une autre instance (même effet, son anneau ne sait
pas ce que la première a remis). Il ne doit jamais remettre deux fois un événement sur
une même connexion sans reprise (le balayage de secours du lecteur est dédoublonné par
l'instance). Un `reset` ne rejoue rien : il remplace les événements abandonnés par une
invalidation complète, ce qui ne perd aucune information. Chaque événement `changes` ou `changeset` doit porter une
identité stable (`changeset`, ou l'identifiant CloudEvents d'un événement de tâche) qui
permet au client de reconnaître un doublon exact, et chaque objet sa révision, qui
permet de reconnaître un changement déjà appliqué (ETag connu égal à la révision). Un
doublon ne doit coûter au client au plus qu'une relecture conditionnelle (304). [R6,
R25, ADR 0018]

Acceptation :
- Deux instances, transaction longue qui détient le numéro le plus bas, coupures et
  reprises aléatoires sur l'une ou l'autre : l'ensemble des événements reçus contient
  tous les événements validés visibles (aucun perdu) ; chaque doublon reçu porte un
  `changeset` déjà vu ; sur une même connexion sans reprise, aucun doublon (test de
  propriété).
- Un client qui ignore les `changeset` déjà vus et compare les révisions n'émet aucune
  relecture pour un doublon exact.

### I. Rattrapage JSON

#### REQ-022-28 — Page d'événements après un curseur
*Ajoutée le 2026-10-29 (Q-022-4, tâche 14).*
Quand une requête `GET /v1/events` admet `application/json` sans nommer
`text/event-stream`, kartenn doit répondre `200`, `application/json`, par une **page
d'événements** après le curseur présenté (`Last-Event-ID` ou `last_event_id`), lue dans
le même anneau que le flux, puis rendre la réponse : `{"items": [{"event", "id",
"data"}, …], "cursor": "k1.…", "links": {"next": …}}`. `event` est la sorte, `data` la
donnée du flux (même schéma, en objet), `id` le curseur de reprise juste après cet
élément. La page doit tenir les règles du flux :

- mêmes filtres (REQ-022-2, `page_size` en plus), même authentification et même
  contrôle d'origine du cookie (REQ-022-15, 19), mêmes sortes et mêmes données
  (REQ-022-3 à 5), sans `ready`, `stats` ni point de reprise ;
- droits décidés **au moment de la réponse**, portée de lecture relue pour chaque page
  (REQ-022-16, 17) : aucun objet que l'appelant ne peut lire, et un événement sans objet
  visible ne laisse aucune trace (ni élément, ni curseur propre, la taille de la page
  ne compte que les éléments remis) (INV-67) ;
- même curseur que le flux (REQ-022-6, 7, 27) : un `id:` du flux reprend une page, un
  `cursor` ou un `id` de page reprend le flux ; une reprise ne perd aucun événement
  validé et visible, sur toute instance (INV-68) ; doublons seulement ceux de
  REQ-022-27 ;
- curseur plus ancien que l'anneau, altéré ou d'une autre identité : la page ne porte
  qu'un élément `reset` (`too_old`, `invalid_cursor`) au curseur courant, jamais 400
  (REQ-022-8) ; sans curseur, une page vide et le curseur courant (le point de départ) ;
- taille bornée : `page_size` de 1 à 1 000, 100 par défaut (REQ-006-7) ; une page
  pleine porte `links.next` (et `Link`, `rel="next"`), la suite immédiate ; sinon
  `links.next` est `null` : l'appelant est à jour et repasse plus tard avec `cursor` ;
- la page ne sert que les événements **sous le plancher** du lecteur de l'instance :
  un événement validé au-dessus d'une transaction longue attend qu'elle se termine, pour
  qu'une page bornée avance toujours sans rien sauter.

Une page est une lecture ordinaire, pas un flux : quota de lecture et délestage
(REQ-006-29, 30), audit `data.objects.read` par page (REQ-012-18 : critères = filtres,
taille, reprise ; cibles = objets remis, bornés par `audit.read_ids_max`), ni plafond de
flux ni `events.stream.*` ; `Cache-Control: no-store`, `Vary: Accept` ; elle n'est pas
une activité de session (A6, Q-022-11). [Q-022-4, R6, R7, R8, R15, REQ-006-7, 8]

Acceptation :
- Trois changesets visibles après le curseur C, `page_size=2` : la page donne les deux
  premiers et `links.next` ; la suivante, le troisième et `links.next` `null` ; une
  autre depuis son `cursor`, rien.
- Un abonné confiné au domaine D : aucun élément d'un changeset de l'autre domaine ; son
  `cursor` le passe sans le révéler ; la reprise sur une autre instance ne perd rien.
- Curseur altéré ou d'une autre identité → un seul élément `reset` `invalid_cursor` ;
  plus ancien que l'anneau → `reset` `too_old` ; puis la page suivante continue.
- Une transaction longue détient le plus petit numéro : les changesets validés après
  elle n'apparaissent qu'à sa validation, avec elle ; les pages de taille 1 avancent
  jusqu'au bout ; aucun perdu.
- Cookie d'une autre origine → 403 ; `page_size=0` → 400 ; chaque page enregistre un
  `data.objects.read` qui nomme les objets remis, aucun `events.stream.opened`.

## Non-objectifs

- WebSocket, flux bidirectionnel, abonnement à une requête arbitraire (filtre `QUERY`).
- Point d'accès MCP : un futur serveur MCP est un adaptateur séparé (`POST /mcp`,
  Streamable HTTP révision 2026-07-28, `subscriptions/listen`) sur la même source
  d'événements ; `/v1/events` n'est jamais réutilisé comme point d'accès MCP (R23).
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

Toutes closes le 2026-10-23 : voir [`decisions.md`](decisions.md), § Arbitrages (A1 à
A8) et § Questions (Q-022-1 à 12).

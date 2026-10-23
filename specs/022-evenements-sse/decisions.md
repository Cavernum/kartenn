# 022 — Événements poussés (SSE) : journal des décisions

Rédigé le 2026-10-23 sans entretien, chaque décision retenant la recommandation la plus
standard. **Validé le même jour** : l'utilisateur a tranché les arbitrages A1 à A8
(A1 après une étude des normes, [`push-serveur.md`](../../docs/reference/push-serveur.md))
et retenu les réponses recommandées aux questions Q-022-1 à 12 ; R23 à R25 sont issues
de ces arbitrages.

Cadre : D3, D7, D9, D11, D13, D14, D17, D35, D36 de
[`../001-cadrage/decisions.md`](../001-cadrage/decisions.md) ; ADR 0018 (outbox), 0020
(session BFF), 0021 (prédicats RBAC), 0022 (contrat HTTP) ; specs 004, 005, 006, 012,
016.

## Synthèse
- R1 SSE sur `GET /v1/events`, pas de WebSocket ni d'interrogation.
- R2 cinq sortes : `ready`, `changes`, `changeset`, `reset`, point de reprise.
- R3 charge utile minimale (clés, révision), relecture par REST et ETag.
- R4 filtres `types`, `domains`, `objects`, `kinds`.
- R5 1 000 objets par événement au plus, puis invalidation large par type.
- R6 curseur opaque chiffré qui porte le plancher du lecteur, pas le numéro brut.
- R7 anneau par instance (15 min, 10⁴ événements), reprise sur toute instance, doublons
  permis.
- R8 `reset` pour un curseur trop ancien ou invalide, jamais une erreur HTTP.
- R9 points de reprise toutes les 30 s au plus.
- R10 maintien toutes les 15 s ; `retry:` aléatoire.
- R11 arrêt propre : 503, `retry:` étalé, fermeture.
- R12 pas de tampon ni de compression ; `X-Accel-Buffering: no`.
- R13 HTTP/2 recommandé ; un flux par navigateur.
- R14 authentification de `/v1` ; `Origin` et `Sec-Fetch-Site` pour le cookie ; pas de
  CORS.
- R15 droits évalués à la remise, par lot et par instance.
- R16 aucune trace d'un événement caché ; audit par connexion.
- R17 révocation en 10 s au plus ; le flux n'est pas une activité de session.
- R18 un lecteur partagé par instance, vue volatile de l'outbox.
- R19 `LISTEN`/`NOTIFY` et interrogation de secours à 1 s.
- R20 tampon de 256 par connexion ; débordement → `reset`, fermeture à 30 s.
- R21 plafonds 5 000 par instance, 16 par identité ; hors délestage.
- R22 OpenAPI 3.1 avec extension ; capacités.
- R23 un futur serveur MCP est un adaptateur séparé, jamais `/v1/events`.
- R24 au-delà de la borne d'audit, chaque remise en log technique DEBUG.
- R25 aucun événement perdu, quelques doublons possibles, reconnus par le client.

## R1 — SSE sur `GET /v1/events`
- Décision : un flux `text/event-stream` (WHATWG HTML § 9.2) sur `GET /v1/events`, dans
  l'API versionnée, servi par les mêmes instances.
- Pourquoi : unidirectionnel, ce qui suffit (le client écrit par REST) ; reprise
  standard (`Last-Event-ID`) ; client natif dans les navigateurs, trivial ailleurs ;
  traverse les mandataires comme une réponse HTTP.
- Rejeté : WebSocket (bidirectionnel inutile, reprise à inventer) ; interrogation
  périodique (ce qu'on remplace) ; webhooks vers les navigateurs (impossible) ;
  bibliothèque de publication externe (dépendance d'exploitation, contredit D9 qui veut
  la base pour seul point de cohérence).
- Deux lecteurs du même flux : le shell web par `EventSource` (cookie BFF) ; la CLI, la
  bibliothèque Python et les connecteurs par une réponse HTTP lue en continu, avec
  `Authorization`.
- Étude (A1) : MCP a déprécié son transport « HTTP+SSE » à deux points d'accès
  (2025-03-26), puis retiré la reprise et l'écoute par `GET` (2026-07-28) pour ne pas
  garder d'état par session (SEP-2575) ; ses réponses restent en `text/event-stream`.
  Ce n'est pas un rejet du format, et l'état de reprise de kartenn est l'outbox, déjà en
  base : la raison ne s'applique pas.
- Conséquences : un point d'accès à longue durée à intégrer au délestage, à la
  compression, à l'arrêt propre (R11, R12, R21) ; D7 amendée (A1).

## R2 — Sortes d'événement
- Décision : `ready` (ouverture, curseur courant, `retry:`), `changes` (objets écrits),
  `changeset` (état d'un changeset en attente), `reset`, point de reprise sans donnée.
  Une sorte nouvelle amende la spec.
- Pourquoi : le client n'a que trois réactions (invalider des clés, tout invalider,
  rien) ; peu de sortes, un contrat stable.
- Rejeté : une sorte par type CloudEvents (couple le client aux noms internes de
  l'outbox) ; une sorte par type d'objet (le filtre `types` suffit).
- Conséquences : la traduction outbox → SSE est une table fermée (design).

## R3 — Charge utile minimale
- Décision : clés (type, identifiant), domaines, révision (= changeset, l'ETag fort de
  006) ; jamais de valeur, de différentiel, d'auteur ni de motif.
- Pourquoi : les droits de lecture fins (valeurs sensibles REQ-005-21, masquage) restent
  appliqués par une seule voie, l'API REST ; l'événement n'est pas un second contrat de
  représentation ; la révision évite une relecture inutile.
- Rejeté : objets complets (sérialisation par abonné et par droits, doublon du REST) ;
  différentiel résumé (révèle des valeurs).
- Conséquences : chaque événement coûte au plus une relecture conditionnelle (304
  souvent) ; la charge de lecture est absorbée par les budgets de D14.

## R4 — Filtres
- Décision : `types`, `domains`, `objects` (≤ 100), `kinds`, combinés par ET, bornés,
  validés à l'ouverture.
- Pourquoi : ce sont les clés que porte l'outbox (domaines, objets) ou qu'on en déduit
  sans requête (types) ; `objects` sert `watch <objet>`.
- Rejeté : filtre par entité, site ou sous-arbre (l'outbox ne les porte pas, il faudrait
  une lecture par événement ; le RBAC confine déjà, Q-022-2) ; filtre `QUERY` arbitraire
  (non-objectif).
- Conséquences : un client par entité reçoit les événements de tout ce qu'il peut lire.

## R5 — Taille bornée, invalidation large
- Décision : au-delà de 1 000 objets visibles, ou pour les effets dérivés (héritage,
  reclassification, réévaluations), l'événement nomme des **types** (et domaines) et
  porte `truncated: true`.
- Pourquoi : un import de 10⁶ objets ne doit pas produire un événement de 40 Mo ; un
  changement de propriété héritée touche jusqu'à 10⁶ valeurs résolues sans les écrire.
- Rejeté : découper en plusieurs événements (inonde les tampons, déclenche des
  débordements) ; ignorer les effets dérivés (caches faux).
- Conséquences : invalidation de listes entières côté client après un gros changeset ;
  voir le risque de l'ETag des valeurs héritées (Q-022-12).

## R6 — Curseur opaque qui porte le plancher
- Décision : `id:` = `k1.` + chiffrement authentifié de (plancher du lecteur, dernier
  numéro remis, identité, émission), clé du trousseau serveur.
- Pourquoi : l'outbox ne numérote pas dans l'ordre de validation entre clés d'ordre
  (ADR 0018) : reprendre « après le numéro N » sauterait l'événement d'une transaction
  lente. Reprendre « après le plancher » ne saute rien, au prix de doublons ignorés par
  la révision. Chiffré : un opérateur externe (D3) ne lit pas le volume d'écriture de
  l'instance dans les sauts de numéros ; lié à l'identité : un curseur volé ne sert pas.
- Rejeté : numéro d'outbox brut (trou possible, fuite de volume ; voir A2) ; numéro
  signé mais lisible (fuite de volume) ; barrière `xmin` avant remise (retirée par
  l'ADR 0018, amendement 2 : une transaction longue n'importe où bloque tout).
- Conséquences : une transaction longue fait rejouer plus à la reprise ; trousseau
  partagé déjà requis (ADR 0020).

## R7 — Anneau par instance
- Décision : chaque instance garde les événements traduits des 15 dernières minutes
  (10⁴ au plus), rechargé de la base au démarrage ; la reprise est servie par l'anneau
  sur n'importe quelle instance.
- Pourquoi : toutes les instances lisent la même outbox, donc ont le même anneau ; une
  reprise ne coûte aucune requête par connexion ; pas d'affinité au répartiteur (D9).
- Rejeté : relire la base à chaque reprise (tempête de reconnexions = tempête de
  requêtes) ; affinité de session (contredit D9).
- Conséquences : déconnecté plus de 15 min → `reset` ; mémoire bornée (≈ 10 Mo).

## R8 — `reset` plutôt qu'une erreur
- Décision : curseur trop ancien, invalide ou d'une autre identité → 200 puis `reset`.
- Pourquoi : un statut autre que 200 fait échouer le client natif sans reconnexion
  (WHATWG § 9.2.2, « fail the connection ») ; le client sait toujours quoi faire d'un
  `reset`.
- Rejeté : 400 ou 410 (client bloqué) ; rejeu complet de l'état (c'est le rôle du REST).
- Conséquences : un `reset` coûte au client la relecture de ce qu'il affiche.

## R9 — Points de reprise
- Décision : `id:` seul, toutes les 30 s au plus, si la position a avancé.
- Pourquoi : sans eux, un abonné filtré ou peu habilité garde un vieux curseur et
  retombe sur `reset` à chaque reconnexion ; périodiques, ils masquent le rythme des
  événements cachés (R16).
- Rejeté : point de reprise à chaque événement caché (oracle temporel) ; curseur dans
  le commentaire de maintien (le client ne le lit pas).
- Conséquences : comportement du client natif à vérifier sur le texte du § 9.2.6
  (tâche 1).

## R10 — Maintien et délai de reconnexion
- Décision : commentaire `: ping` après 15 s de silence (réglable 5 à 60 s) ; `retry:`
  aléatoire 1 à 3 s à l'ouverture.
- Pourquoi : conseil du § 9.2.7 contre les mandataires qui coupent les connexions
  muettes ; l'aléa étale les reconnexions.
- Rejeté : maintien par événement (déclenche le client à vide).
- Conséquences : 1 octet/s environ par connexion.

## R11 — Arrêt propre
- Décision : à l'arrêt, refuser (503), écrire `retry:` aléatoire 0,5 à 5 s, fermer.
- Pourquoi : l'arrêt propre de `hyper` attend la fin des réponses, qui ne finissent
  jamais ; l'étalement évite que 5 000 clients frappent l'autre instance à la même
  milliseconde.
- Rejeté : attendre le délai d'arrêt puis tuer (clients reconnectés en rafale).
- Conséquences : un déploiement progressif ne perd aucun événement (reprise R7).

## R12 — Intermédiaires
- Décision : `Cache-Control: no-store`, `X-Accel-Buffering: no`, `Incremental: ?1` en
  option (RFC 10036, déploiement réel à vérifier), pas de compression ni de
  `Content-Length`, écriture sans tampon ; guide de déploiement.
- Pourquoi : un compresseur ou un mandataire qui met en tampon retient les événements
  (latence non tenue, maintien inopérant).
- Rejeté : compression avec vidage par événement (gain faible sur des lignes courtes,
  complexité, risque de canal auxiliaire par compression sur un flux chiffré).
- Conséquences : amende REQ-006-32 pour cette route (A5).

## R13 — HTTP/2 et limite des navigateurs
- Décision : servir HTTP/1.1 et HTTP/2 ; ≥ 100 flux concurrents en HTTP/2 ; le contrat
  du SDK impose un seul flux par navigateur.
- Pourquoi : en HTTP/1.1, la limite de connexions par origine des navigateurs (pratique
  d'implémentation, RFC 9112 § 9.4) est épuisée par quelques onglets ; en HTTP/2, un
  flux est un stream parmi d'autres (RFC 9113 § 6.5.2).
- Rejeté : exiger HTTP/2 (certains mandataires terminent en HTTP/1.1 vers l'amont).
- Conséquences : partage entre onglets à concevoir dans le SDK (spec à venir).

## R14 — Authentification et origine
- Décision : mêmes moyens que `/v1` (cookie, Basic, Bearer) ; pour le cookie, `Origin`
  vérifié (égal à l'origine publique s'il est présent) et `Sec-Fetch-Site` égal à
  `same-origin` ou absent, 403 sinon ; aucune réponse CORS ; droits revérifiés à la
  relecture (R15).
- Pourquoi : `GET` échappe à la protection CSRF (REQ-006-28), mais un flux ouvert depuis
  un autre site garderait une connexion authentifiée (lecture impossible pour l'attaquant
  sans CORS, mais coût et quotas consommés) ; le shell est de même origine
  (`connect-src 'self'`).
- Rejeté : jeton dans l'URL (fuite dans les journaux) ; CORS avec `withCredentials`
  (inutile, élargit la surface).
- Conséquences : un client tiers d'une autre origine passe par Bearer, sans cookie.

## R15 — Droits à la remise, par lot
- Décision : visibilité calculée au moment de la remise, pour tous les sujets connectés
  d'une instance et tous les objets d'un lot, en une requête qui réutilise les prédicats
  RBAC (ADR 0021).
- Pourquoi : une requête par connexion et par événement ne tient pas (200 sujets ×
  50 événements/s) ; les droits d'aujourd'hui priment (D36).
- Rejeté : droits figés à l'ouverture (révocation ignorée) ; droits à la validation du
  changeset (les rejouer à la reprise exigerait de les stocker).
- Conséquences : coût borné par lot ; mesuré au banc (REQ-022-26).

## R16 — Aucune trace d'un événement caché ; audit par connexion
- Décision : un événement sans objet visible n'est pas remis et n'avance pas le curseur
  hors des points de reprise ; l'audit enregistre l'ouverture et la fermeture, avec le
  nombre et (bornés) les identifiants remis, pas un événement par remise.
- Pourquoi : pas d'oracle d'existence (REQ-006-23, INV-63) ; un événement d'audit par
  remise multiplierait le volume (5 000 connexions × 50 événements/s) alors que la
  lecture effective, par REST, est déjà auditée.
- Rejeté : événement vide « quelque chose a changé » (oracle) ; audit par remise.
- Conséquences : amende REQ-012-18 (A4) ; au-delà de la borne, R24.

## R17 — Révocation et session
- Décision : fermeture en 10 s au plus d'une connexion révoquée (changesets
  d'administration vus par le lecteur, revérification groupée toutes les 10 s) ;
  `reset` sur changement de liaison ou de rôle ; fermeture à l'échéance du jeton ou de
  la session ; le flux ne rafraîchit pas l'inactivité.
- Pourquoi : REQ-005-3 refuse « la requête suivante », qui n'existe pas pour un flux ;
  un onglet oublié ne doit pas garder une session vivante.
- Rejeté : revérifier à chaque événement (coût) ; ignorer la révocation jusqu'à la
  reconnexion.
- Conséquences : un tableau de bord regardé sans interaction se ferme à 30 min (A6).

## R18 — Lecteur partagé, vue volatile
- Décision : un lecteur par instance, hors de `outbox_delivery`, avec un plancher en
  mémoire (logique de `pass_floor`).
- Pourquoi : le coût en base ne dépend pas du nombre de connexions ; les abonnés sont
  éphémères, la base reste l'autorité (D9).
- Rejeté : consommateur d'outbox par connexion ou par instance (écritures inutiles) ;
  lecteur unique sous bail qui republierait aux autres instances (bus à ajouter).
- Conséquences : N instances = N lecteurs, N lectures par passe (négligeable).

## R19 — `LISTEN`/`NOTIFY` et interrogation de secours
- Décision : `pg_notify('kartenn_outbox','')` dans la transaction qui écrit l'outbox ;
  connexion d'écoute dédiée par instance ; interrogation toutes les 1 s en secours.
- Pourquoi : latence de l'ordre de la dizaine de millisecondes sans interroger en
  boucle ; notifications remises à la validation seulement et fusionnées ; perdues si
  l'écoute tombe, d'où le secours.
- Rejeté : interrogation seule à 100 ms (charge permanente) ; notification portant le
  numéro (inutile, la lecture suit) ; déclencheur SQL sur `outbox` (caché du code).
- Conséquences : verrou de la file de notification à la validation, à mesurer ; une
  connexion de base hors pool par instance ; incompatible avec un regroupeur de
  connexions en mode transaction (guide).

## R20 — Tampons bornés
- Décision : 256 événements par connexion ; plein → abandon, `reset` `overflow`,
  fermeture après 30 s de blocage.
- Pourquoi : mémoire bornée, un abonné lent n'en retarde aucun autre (comme INV-56 pour
  les sorties techniques).
- Rejeté : tampon illimité ; contre-pression vers le lecteur (un client bloque tous les
  autres).
- Conséquences : le client lent relit tout, ce qui est correct.

## R21 — Plafonds et quotas
- Décision : 5 000 flux par instance (503), 16 par identité (429), réglables ; hors du
  compteur de délestage ; l'ouverture compte une lecture du quota.
- Pourquoi : un flux occupe une tâche et un descripteur, pas une connexion de base ; le
  compter dans le délestage (2 × pool) bloquerait l'API à la 41ᵉ connexion.
- Rejeté : aucun plafond (épuisement des descripteurs) ; quota par événement remis.
- Conséquences : amende REQ-006-30 (A5) ; dimensionnement : 200 utilisateurs (D14),
  quelques navigateurs chacun, plus les machines, tiennent sur une instance.

## R22 — Contrat
- Décision : OpenAPI 3.1, réponse `text/event-stream` et extension
  `x-kartenn-events` vers les schémas des données ; capacités (`events`).
- Pourquoi : le contrat de référence (REQ-006-34) doit protéger aussi le flux ; 3.1 ne
  décrit pas les éléments d'un flux.
- Rejeté : flux hors contrat ; passage immédiat à OpenAPI 3.2 (dépend du générateur).
- Conséquences : un SDK généré lit l'extension (Q-022-9).

## R23 — MCP : adaptateur séparé
- Décision : un futur serveur MCP est un adaptateur distinct, `POST /mcp`, transport
  Streamable HTTP de la révision 2026-07-28 (`subscriptions/listen`), branché sur la même
  source d'événements (lecteur partagé, visibilité par lot) ; `/v1/events` n'est jamais
  réutilisé comme point d'accès MCP.
- Pourquoi : les contrats diffèrent (méthode, enveloppe JSON-RPC, accusé d'abonnement,
  pas de reprise par `Last-Event-ID` côté MCP) ; les mêler figerait `/v1/events` sur les
  révisions d'un protocole tiers.
- Rejeté : servir MCP sur `/v1/events` ; transport « HTTP+SSE » de 2024-11-05 (déprécié).
- Conséquences : spec à venir ; perte du flux MCP → réabonnement et relecture.
- Décidé par : l'utilisateur (A1, 2026-10-23)

## R24 — Remises au-delà de la borne d'audit
- Décision : quand les identifiants remis à une connexion dépassent
  `audit.read_ids_max`, chaque remise suivante est écrite dans les logs techniques
  (`events.delivery`, DEBUG : connexion, acteur, sorte, identifiants), hors du journal
  d'audit scellé, avec une rétention courte réglée sur la sortie (7 jours recommandés).
- Pourquoi : condition de l'utilisateur à l'audit par connexion (A4) ; la trace fine
  reste disponible pour une enquête récente sans gonfler l'audit.
- Rejeté : tout dans l'audit (volume) ; rien au-delà de la borne (trace perdue).
- Conséquences : filtre de sortie et rétention à documenter (`observabilite.md`).
- Décidé par : l'utilisateur (A4, 2026-10-23)

## R25 — Doublons
- Décision : garantie « aucun événement perdu, quelques doublons possibles ». Doublons
  seulement à la reprise (même instance ou autre), jamais sur une connexion sans
  reprise ; un `reset` ne rejoue rien. Le client reconnaît un doublon exact par
  l'identité de l'événement (`changeset` ou `event`) et un changement déjà appliqué par
  la révision (égalité avec l'ETag) ; au pire, une relecture conditionnelle (304).
- Pourquoi : conséquence de R6 (le curseur porte un plancher, pas l'ensemble des
  numéros remis) ; une garantie « exactement une fois » exigerait un état par abonné.
- Rejeté : curseur portant l'ensemble des numéros remis (taille non bornée) ; état de
  livraison par abonné en base (contredit R18).
- Conséquences : test de propriété dédié (REQ-022-27, tâche 10).
- Décidé par : l'utilisateur (A2, 2026-10-23)

## Arbitrages (tranchés le 2026-10-23 par l'utilisateur)

- **A1 — D7.** SSE retenu après étude ; un seul point d'accès `GET /v1/events`, partie de
  l'API HTTP `/v1`. D7 amendée : « API HTTP (REST et flux d'événements) ». Deux lecteurs
  (`EventSource` avec cookie et `Origin` vérifié ; réponse lue en continu avec
  `Authorization`) ; droits revérifiés à la relecture ; `Cache-Control: no-store`,
  `X-Accel-Buffering: no`, `Incremental: ?1` en option ; MCP séparé (R23).
- **A2 — Curseur.** Curseur opaque chiffré qui porte le plancher (R6) ; doublons
  documentés et testés (R25, REQ-022-27).
- **A3 — D9.** L'anneau et l'état des connexions ne font pas autorité ; D9 inchangée.
- **A4 — REQ-012-18 et INV-50.** Audit par connexion, identifiants remis bornés par
  `audit.read_ids_max`, car chaque action reste rattachée à son acteur (le flux ne porte
  que des notifications ; lectures de contenu et écritures passent par REST, auditées
  par requête). Condition : au-delà de la borne, log DEBUG à rétention courte (R24).
  REQ-012-18 à amender (tâche 2).
- **A5 — REQ-006-30 et REQ-006-32.** `/v1/events` hors délestage et hors compression,
  plafonds propres (5 000 par instance → 503, 16 par identité → 429). À amender
  (tâche 2).
- **A6 — REQ-005-3 et REQ-005-7.** Révocation en 10 s au plus pour un flux ouvert ; le
  flux n'est pas une activité de session. À amender (tâche 2).
- **A7 — D31.** La 022 précède les specs web 020 et 021 ; D31 amendée.
- **A8 — D17.** Inchangée ; dépendance notée pour 020 et 021 et pour le débat web D-1.

## Questions (closes le 2026-10-23 : réponse recommandée retenue)

- **Q-022-1** — Taille de l'anneau ? Retenu : 15 min et 10⁴ événements par
  défaut, réglables ; à revoir après le banc.
- **Q-022-2** — Filtres par entité, site ou sous-arbre de préfixes ? Retenu :
  non dans cette spec ; l'outbox ne porte pas ces ancres, le RBAC confine déjà ; à
  rouvrir si un connecteur le demande.
- **Q-022-3** — Pousser les événements de module sans `subject` typé ? Retenu :
  non ; le module déclare le type de son sujet (E3) pour être poussé.
- **Q-022-4** — Variante JSON de rattrapage (`Accept: application/json`, page
  d'événements après un curseur) pour les clients qui ne tiennent pas un flux ?
  Retenu : oui, plus tard, tâche facultative (P4) ; même anneau, mêmes droits.
- **Q-022-5** — Identifiants remis dans `events.stream.closed` ? Retenu : oui,
  bornés par `audit.read_ids_max` (L25), le reste compté.
- **Q-022-6** — Droits évalués à la remise ou à la validation du changeset ?
  Retenu : à la remise (R15).
- **Q-022-7** — Plafond par identité ? Retenu : 16 par défaut ; plafond propre
  aux comptes de service plus tard si un connecteur en a besoin.
- **Q-022-8** — Charge utile de `NOTIFY` ? Retenu : vide, un seul canal
  `kartenn_outbox`.
- **Q-022-9** — OpenAPI 3.2 (`itemSchema`) ? Retenu : extension en 3.1
  maintenant, passage à 3.2 quand le générateur le permet (à vérifier dans la note).
- **Q-022-10** — Curseur chiffré ou seulement signé ? Retenu : chiffré (R6),
  coût négligeable, aucun volume lisible.
- **Q-022-11** — Le flux doit-il compter comme activité de session ? Retenu :
  non (A6) ; le SDK affiche l'approche de l'échéance.
- **Q-022-12** — La révision (ETag = dernier changeset qui a écrit l'objet ou ses
  propriétés) ne change pas quand une **valeur héritée** change : un client qui compare
  l'ETag garderait une valeur résolue périmée. Retenu : l'invalidation large par
  type (R5) force la relecture sans condition pour ces cas ; vérifier à la 006 si la
  représentation REST inclut des valeurs résolues sous un ETag fort (sinon, rien à
  faire).

# 022 — Événements poussés (SSE) : journal des décisions

Brouillon rédigé le 2026-10-23 sans entretien. Chaque décision retient la
recommandation la plus standard ; toutes restent à valider par l'utilisateur, ainsi que
les arbitrages A1 à A8 et les questions Q-022-1 à Q-022-12 en fin de journal.

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
- R14 authentification de `/v1` ; `Sec-Fetch-Site` pour le cookie ; pas de CORS.
- R15 droits évalués à la remise, par lot et par instance.
- R16 aucune trace d'un événement caché ; audit par connexion.
- R17 révocation en 10 s au plus ; le flux n'est pas une activité de session.
- R18 un lecteur partagé par instance, vue volatile de l'outbox.
- R19 `LISTEN`/`NOTIFY` et interrogation de secours à 1 s.
- R20 tampon de 256 par connexion ; débordement → `reset`, fermeture à 30 s.
- R21 plafonds 5 000 par instance, 16 par identité ; hors délestage.
- R22 OpenAPI 3.1 avec extension ; capacités.

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
- Conséquences : un point d'accès à longue durée à intégrer au délestage, à la
  compression, à l'arrêt propre (R11, R12, R21) ; voir l'arbitrage A1 (D7).

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
- Décision : `Cache-Control: no-store`, `X-Accel-Buffering: no`, pas de compression ni
  de `Content-Length`, écriture sans tampon ; guide de déploiement.
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
- Décision : mêmes moyens que `/v1` (cookie, Basic, Bearer) ; pour le cookie,
  `Sec-Fetch-Site` doit être `same-origin` ou absent ; aucune réponse CORS.
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
- Conséquences : amende REQ-012-18 (A4).

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

## Arbitrages demandés

Contradictions ou amendements de décisions et de specs existantes : rien n'est appliqué
avant l'arbitrage.

- **A1 — D7 (REST, frontière publique unique).** Le flux n'est pas une ressource REST au
  sens strict. Recommandation : le considérer comme partie de l'API HTTP `/v1` (même
  authentification, RBAC, audit, quotas, contrat OpenAPI) et préciser D7 : « API HTTP
  (REST et flux d'événements) ». Aucune autre frontière n'est ouverte.
- **A2 — Hypothèse « `id` = position dans l'outbox » contre l'ADR 0018.** Les numéros ne
  suivent pas l'ordre de validation entre clés d'ordre ; un `id` brut saute des
  événements. Recommandation : R6 (curseur opaque qui porte le plancher), qui reste
  dérivé de la position.
- **A3 — D9 (aucun cache local faisant autorité).** L'anneau et l'état des connexions
  sont locaux à une instance. Recommandation : confirmer qu'ils ne font pas autorité
  (la base et l'outbox restent la vérité ; reprise sur toute instance ; pas
  d'affinité).
- **A4 — REQ-012-18 et INV-50 (toute lecture de l'API auditée avec ses identifiants).**
  Recommandation : audit par connexion (R16), amender REQ-012-18 pour le flux.
- **A5 — REQ-006-30 (délestage) et REQ-006-32 (compression).** Recommandation :
  exclure `/v1/events` des deux, avec son propre plafond (R12, R21).
- **A6 — REQ-005-3 et REQ-005-7 (révocation « à la requête suivante », inactivité).**
  Recommandation : révocation en 10 s au plus pour un flux ouvert ; le flux ne compte
  pas comme activité (un écran passif se ferme à 30 min d'inactivité).
- **A7 — D31 (feuille de route).** Ni le flux, ni une bibliothèque cliente en Python,
  ni des commandes `watch` n'y figurent (CLI « ensuite » ; D7 parle d'un SDK généré
  depuis l'OpenAPI). Recommandation : placer la 022 avant les specs 020 et 021 (à
  venir), comme brique transverse, et ouvrir une décision sur la bibliothèque cliente.
- **A8 — D17 (shell Vue 3) et specs 020, 021 à venir.** Le contrat client cite le cœur
  de TanStack Query, indépendant du framework ; la 022 ne tranche rien sur la réécriture
  du shell. Recommandation : aucune modification de D17 par cette spec ; dépendance à
  noter dans 020 et 021.

## Questions ouvertes

- **Q-022-1** — Taille de l'anneau ? Recommandation : 15 min et 10⁴ événements par
  défaut, réglables ; à revoir après le banc.
- **Q-022-2** — Filtres par entité, site ou sous-arbre de préfixes ? Recommandation :
  non dans cette spec ; l'outbox ne porte pas ces ancres, le RBAC confine déjà ; à
  rouvrir si un connecteur le demande.
- **Q-022-3** — Pousser les événements de module sans `subject` typé ? Recommandation :
  non ; le module déclare le type de son sujet (E3) pour être poussé.
- **Q-022-4** — Variante JSON de rattrapage (`Accept: application/json`, page
  d'événements après un curseur) pour les clients qui ne tiennent pas un flux ?
  Recommandation : oui, plus tard, tâche facultative (P4) ; même anneau, mêmes droits.
- **Q-022-5** — Identifiants remis dans `events.stream.closed` ? Recommandation : oui,
  bornés par `audit.read_ids_max` (L25), le reste compté.
- **Q-022-6** — Droits évalués à la remise ou à la validation du changeset ?
  Recommandation : à la remise (R15).
- **Q-022-7** — Plafond par identité ? Recommandation : 16 par défaut ; plafond propre
  aux comptes de service plus tard si un connecteur en a besoin.
- **Q-022-8** — Charge utile de `NOTIFY` ? Recommandation : vide, un seul canal
  `kartenn_outbox`.
- **Q-022-9** — OpenAPI 3.2 (`itemSchema`) ? Recommandation : extension en 3.1
  maintenant, passage à 3.2 quand le générateur le permet (à vérifier dans la note).
- **Q-022-10** — Curseur chiffré ou seulement signé ? Recommandation : chiffré (R6),
  coût négligeable, aucun volume lisible.
- **Q-022-11** — Le flux doit-il compter comme activité de session ? Recommandation :
  non (A6) ; le SDK affiche l'approche de l'échéance.
- **Q-022-12** — La révision (ETag = dernier changeset qui a écrit l'objet ou ses
  propriétés) ne change pas quand une **valeur héritée** change : un client qui compare
  l'ETag garderait une valeur résolue périmée. Recommandation : l'invalidation large par
  type (R5) force la relecture sans condition pour ces cas ; vérifier à la 006 si la
  représentation REST inclut des valeurs résolues sous un ETag fort (sinon, rien à
  faire).

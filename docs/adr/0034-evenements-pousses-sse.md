---
status: proposed
date: 2026-10-23
---

# 0034 — Événements poussés par SSE, vue volatile de l'outbox

## Context

Les clients de kartenn (shell web des specs 020 et 021, bibliothèque Python et CLI de la
023, connecteurs) tiennent des caches des objets du référentiel. Sans signal du serveur,
ils interrogent l'API en boucle, ce qui coûte en lecture et laisse des vues périmées. La
spec 022 demande un flux d'invalidation en temps réel, filtré par les droits de chaque
abonné, sans oracle d'existence (INV-63).

L'outbox transactionnelle de l'ADR 0018 porte déjà, dans la transaction de chaque
changeset, un événement CloudEvents qui nomme le changeset, ses domaines et ses objets.
Elle ne numérote pas ses lignes dans l'ordre de validation entre clés d'ordre (ADR 0018,
amendements 1 et 2) : un lecteur qui reprend « après le numéro N » peut sauter une
transaction lente. Les consommateurs de l'ADR 0018 enregistrent donc chaque livraison
(`outbox_delivery`), ce qui ne convient pas à des abonnés éphémères (une écriture par
événement et par connexion).

Étude des normes : [`push-serveur.md`](../reference/push-serveur.md) (WHATWG HTML § 9.2,
RFC 9110, 9112, 9113, 6585, 10036, MCP, `LISTEN`/`NOTIFY`).

## Decision

1. **SSE** (`text/event-stream`, WHATWG HTML § 9.2) sur `GET /v1/events`, dans l'API
   `/v1` (D7 amendée, A1). Cinq sortes : `ready`, `changes`, `changeset`, `reset`, point
   de reprise (`id:` seul) ; la 018 ajoute `labels` (REQ-018-18). La donnée ne porte que
   des clés et des révisions ; le client relit par REST avec ses ETag (R3).
2. **Vue volatile de l'outbox.** Le flux n'est pas un consommateur au sens de l'ADR 0018 :
   aucune ligne dans `outbox_delivery`. Un **lecteur partagé par instance** lit l'outbox
   au-dessus d'un **plancher en mémoire** (règle de `pass_floor`, migration 0027, sans
   écriture), traduit chaque événement et le range dans un **anneau** (15 min, 10⁴
   événements) rechargé au démarrage (R7, R18).
3. **Réveil par `LISTEN`/`NOTIFY`** : `pg_notify('kartenn_outbox', '')` dans toute
   transaction qui écrit l'outbox, connexion d'écoute dédiée hors du pool, interrogation
   de secours toutes les secondes (R19).
4. **Curseur opaque qui porte le plancher** : `id:` = `k1.` + AES-256-GCM de (plancher,
   dernier numéro remis, identité, émission), clé dérivée du trousseau serveur. La
   reprise rejoue l'anneau au-dessus du plancher : aucun événement perdu, doublons
   possibles à la reprise seulement, reconnus par l'identité de l'événement et la
   révision (R6, R25). Curseur trop ancien, altéré ou d'une autre identité : `reset`,
   jamais une erreur HTTP (R8).
5. **Visibilité par lot** : un répartiteur calcule, pour un lot d'événements, la
   lecture de chaque objet par chaque sujet connecté en une requête qui réutilise les
   prédicats RBAC (ADR 0021), au moment de la remise (R15) ; un événement sans objet
   visible ne laisse aucune trace (R16).
6. **Tampon borné par connexion** (256) ; débordement → `reset` `overflow`, fermeture
   après 30 s de blocage ; un abonné lent ne retarde ni le lecteur ni les autres (R20).
7. **Hors délestage et hors compression** (amende REQ-006-30, 32 : A5) ; plafonds
   propres (5 000 par instance → 503, 16 par identité → 429) ; arrêt propre (`retry:`
   étalé puis fermeture, R11, R21).
8. **Audit par connexion** (`events.stream.opened`, `events.stream.closed`), identifiants
   remis bornés par `audit.read_ids_max` ; au-delà, log technique DEBUG
   `events.delivery` hors du journal scellé (amende REQ-012-18, INV-50 : A4, R24).
9. Un futur serveur MCP est un **adaptateur séparé** (`POST /mcp`) branché sur la même
   source ; `/v1/events` n'est jamais un point d'accès MCP (R23).

## Alternatives

- WebSocket : bidirectionnel inutile, pas de reprise standard (R1).
- Interrogation périodique ou longue : ce que le flux remplace.
- Consommateur d'outbox par connexion (`outbox_delivery`) : une écriture par événement
  et par connexion.
- Décodage logique (réplication) : rejeté par l'ADR 0018.
- `id:` = numéro d'outbox brut : saute une transaction lente et révèle le volume
  d'écriture (R6).
- `tokio::sync::broadcast` : un récepteur lent perd des messages sans signal
  exploitable, et le filtrage par droits doit précéder la remise.

## Consequences

- N instances = N lecteurs, chacun une lecture par passe, indépendante du nombre de
  connexions (INV-69).
- Une connexion PostgreSQL hors pool par instance pour `LISTEN` ; incompatible avec un
  regroupeur de connexions en mode transaction (guide d'exploitation).
- `NOTIFY` prend un verrou de la file de notification à la validation : surcoût mesuré
  par le banc `bench-events` (REQ-022-26).
- Une transaction longue retient le plancher : plus de doublons à la reprise, puis
  `reset` au-delà de l'anneau.
- Le client doit être idempotent (identité d'événement, révision) et traiter `reset`
  comme une invalidation complète.
- Amendements : D7 et D31 (A1, A7) ; REQ-004-36 (`NOTIFY`, E1) ; REQ-005-3 et 7 (A6) ;
  REQ-006-30 et 32 (A5) ; REQ-012-18 et INV-50 (A4, R24).

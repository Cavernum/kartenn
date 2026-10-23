---
status: draft
updated: 2026-10-23
---

# 022 — Événements poussés (SSE) : tâches

Brouillon : aucune tâche ne commence avant la validation de la spec, des arbitrages A1 à
A8 et des questions Q-022-1 à 12 ([`decisions.md`](decisions.md)). Chaque tâche est un
commit, laisse le build vert et nomme ce qu'elle implémente ; chacune porte `où :` et
`dépend de :`. Commandes lourdes par `k-heavy` (`k-check`, bancs).

## P0 — Références et décision

- [ ] 1. Note de référence `docs/reference/evenements-sse.md` : WHATWG HTML § 9.2
  (format, `Last-Event-ID`, échec sur statut ≠ 200, mise à jour de l'identifiant par un
  bloc `id:` sans donnée, § 9.2.7 informatif), RFC 9110 § 10.2.3, § 15.6.4, RFC 6585
  § 4, RFC 9112 § 9.4, RFC 9113 § 6.5.2, CSP3 § 6.1.2, Fetch (en-têtes), PostgreSQL 18
  `LISTEN`/`NOTIFY`, `proxy_buffering`, OpenAPI 3.2 (`itemSchema`) ; normatif et
  informatif séparés — REQ-022-6, 9, 13, 14, 23 — verify : chaque citation a son URL et
  sa section ; les deux points « à vérifier » du design sont tranchés
  - où : forme de `docs/reference/api-http.md`, `docs/reference/web-navigateur.md` ;
    sous-agent `ref-researcher` ; design § Références
  - dépend de : —
- [ ] 2. ADR 0033 « Événements poussés par SSE, vue volatile de l'outbox » (proposée) ;
  amendements arbitrés : D7 (A1), REQ-006-30, 32 (A5), REQ-012-18 (A4), REQ-005-3, 7
  (A6), REQ-004-36 (`NOTIFY`, E1) — verify : liens croisés ; `specs/README.md` à jour
  - où : `docs/adr/0018-outbox-ordonnee-par-domaine.md` (forme), decisions § Arbitrages
  - dépend de : 1, validation de la spec

## P1 — Domaine pur

- [ ] 3. Module `kartenn-service::events` : sortes (`Ready`, `Changes`, `Changeset`,
  `Reset`, point de reprise), traduction d'un `Event` d'outbox en événement SSE
  (table du design § Format), troncature à 1 000 objets et invalidation large,
  sérialisation WHATWG (une ligne `data:`, pas de LF/CR) — REQ-022-3, 4, 5 — verify :
  tests unitaires par type CloudEvents ; propriété : toute sortie est analysée par un
  analyseur WHATWG de test en l'événement d'origine
  - où : `crates/kartenn-store-pg/src/outbox.rs` (`Event`) ; payloads de
    `crates/kartenn-store-pg/src/write.rs` (`org.kartenn.changeset.applied`, événements
    de module) et `pending.rs` (`event`)
  - dépend de : 2
- [ ] 4. Curseur : `(plancher, dernier numéro, identité, émission)`, chiffré AES-256-GCM
  avec une clé dérivée du trousseau, préfixe `k1.` ; refus d'un curseur altéré, d'une
  autre identité ou d'une autre clé — REQ-022-6, 8 — verify : aller-retour ; octet
  modifié → invalide ; rotation de clé : l'ancienne reste lisible
  - où : `crates/kartenn-auth/src/seal.rs` (`seal`, `open` : chiffrement du cookie) et
    `signed.rs` (`Purpose`, `KeyRing`) ; `crates/kartenn-rest/src/cursor.rs` (curseur
    signé de REQ-006-8, forme)
  - dépend de : 3

## P2 — Lecteur partagé

- [ ] 5. `NOTIFY` : `pg_notify('kartenn_outbox','')` dans les transactions qui écrivent
  l'outbox — E1, REQ-022-20 — verify : test d'intégration, une notification par
  transaction validée, aucune pour une transaction annulée
  - où : `crates/kartenn-store-pg/src/write.rs` (`write_events`, insertion de
    `org.kartenn.changeset.applied`), `crates/kartenn-store-pg/src/pending.rs` (`event`) ;
    test voisin `crates/kartenn-service/tests/it/outbox.rs`
  - dépend de : 2
- [ ] 6. Plancher en mémoire : variante sans écriture de `floor.rs` (`pending_max`,
  `pending_xmax`, montée sur `pg_snapshot_xmin`) et balayage de la fenêtre — E6,
  REQ-022-7 — verify : transaction longue qui détient un numéro inférieur, jamais sautée
  - où : `crates/kartenn-store-pg/src/floor.rs`, migration 0027 (commentaire), tests
    `a_running_transaction_is_never_skipped`, `pass_floor`
  - dépend de : 2
- [ ] 7. Lecteur : connexion d'écoute dédiée (reconnexion avec attente croissante),
  interrogation de secours à 1 s, passe par lot de 1 000, typage groupé par
  `changeset_operation`, anneau (15 min, 10⁴) rechargé au démarrage — REQ-022-7, 20 —
  verify : requêtes du lecteur indépendantes du nombre d'abonnés ; écoute coupée →
  remise ≤ 1 s ; deux instances, même anneau
  - où : nouveau `crates/kartenn-store-pg/src/events.rs` (lecture) ; tâche dans
    `crates/kartenn-server/src/app.rs` à côté des relais d'audit (`relay_once`)
  - dépend de : 3, 5, 6
- [ ] 8. Répartiteur et visibilité par lot : regroupement par sujet, requête sujets ×
  objets avec les prédicats RBAC, court-circuit de lecture globale, `changeset` à
  l'auteur et aux valideurs possibles, `mpsc` borné par connexion, débordement —
  REQ-022-16, 17, 21 — verify : test de propriété (remis = décision unitaire) ;
  opérateur externe ; client qui ne lit plus → `reset` `overflow`, fermeture à 30 s,
  autres abonnés à l'heure
  - où : `crates/kartenn-store-pg/src/rbac.rs` (`readable`, `authorize_many`),
    `crates/kartenn-service/src/authz.rs` ; propriété voisine du filtrage des listes
    (REQ-005-24, `crates/kartenn-store-pg/tests/it/`)
  - dépend de : 7

## P3 — Point d'accès

- [ ] 9. `GET /v1/events` : négociation, filtres et bornes, authentification cookie,
  Basic, Bearer, `Sec-Fetch-Site`, pas de CORS, `ready` avec `retry:` aléatoire,
  maintien 15 s, points de reprise 30 s, en-têtes (`no-store`, `X-Accel-Buffering: no`)
  — REQ-022-1, 2, 9 à 11, 13, 15, 19 — verify : client SSE de test ; matrice cookie ×
  `Sec-Fetch-Site` ; `Accept-Encoding` ignoré
  - où : nouveau `crates/kartenn-rest/src/events.rs` ; montage dans
    `crates/kartenn-rest/src/lib.rs` (pile de middlewares, l. 122-180) ; `csrf.rs`,
    `authn.rs`, `compression.rs` ; tests à l'image de `crates/kartenn-rest/tests/it/csrf.rs`
  - dépend de : 4, 8
- [ ] 10. Reprise : `Last-Event-ID` et `last_event_id`, rejeu de l'anneau, `reset`
  `too_old` et `invalid_cursor` — REQ-022-7, 8 — verify : coupure pendant trois
  changesets, reprise sur la seconde instance ; transaction longue ; curseur d'une
  autre identité
  - où : `crates/kartenn-rest/src/events.rs` ; deux instances comme
    `crates/kartenn-server/tests` (`an_import_ends_on_another_instance_without_duplicates`)
  - dépend de : 9
- [ ] 11. Révocation et échéances : sujets marqués par les changesets d'administration,
  revérification groupée toutes les 10 s, fermeture à `exp` et à l'échéance de session,
  `reset` `rights_changed` — REQ-022-18 — verify : cas d'acceptation de REQ-022-18 ;
  `authn.session.closed` (cause `revoked`)
  - où : `crates/kartenn-rest/src/authn.rs` (état des identités, cache 5 s, ADR 0020),
    `crates/kartenn-rest/src/cookies.rs` (échéances) ; test voisin
    `crates/kartenn-rest/tests/it/sessions.rs`
  - dépend de : 9

## P4 — Ressources et exploitation

- [ ] 12. Plafonds (instance, identité), hors délestage, quota d'ouverture ; arrêt
  propre (503, `retry:` étalé, fermeture) ; HTTP/2 ≥ 100 flux concurrents — REQ-022-12,
  14, 22 — verify : 5 001ᵉ flux → 503, lecture ordinaire servie ; 17ᵉ → 429 ; 1 000
  connexions fermées en < 2 s à l'arrêt
  - où : `crates/kartenn-rest/src/quota.rs` (`shed`, `InFlight`),
    `crates/kartenn-rest/src/transport.rs` (`max_in_flight`),
    `crates/kartenn-server/src/http.rs` (`GracefulShutdown`, l. 109-)
  - dépend de : 9
- [ ] 13. Guide de déploiement : mandataire inverse (tampon, délai de lecture,
  HTTP/2), regroupeur de connexions et `LISTEN` — REQ-022-13, 14 — verify : relecture
  - où : `docs/exploitation/configuration.md` (section `[events]`), `oci.md`,
    `nixos.md`, `observabilite.md` (métriques)
  - dépend de : 12
- [ ] 14. (Facultative, Q-022-4) Rattrapage JSON : `GET /v1/events` avec `Accept:
  application/json`, page après un curseur — verify : mêmes droits que le flux
  - où : `crates/kartenn-rest/src/events.rs`, pagination de `cursor.rs`
  - dépend de : 10

## P5 — Contrat, audit, télémétrie

- [ ] 15. OpenAPI (réponse `text/event-stream`, `x-kartenn-events`, schémas des
  données), capacités (`events`) — REQ-022-23 — verify : contrat de référence mis à
  jour, test de REQ-006-34 vert
  - où : `crates/kartenn-rest/src/openapi.rs`, `contract.rs`, `capabilities.rs` ; test
    `crates/kartenn-rest/tests/it/contract.rs`
  - dépend de : 9
- [ ] 16. Audit `events.stream.opened` / `closed` ; métriques et spans — REQ-022-24, 25
  — verify : événements au catalogue et au schéma ; métriques exposées ; aucune span
  par connexion
  - où : `docs/spec/evenements-audit.md`, `evenements-audit.schema.json`,
    `crates/kartenn-rest/src/audit.rs`, `crates/kartenn-telemetry` ; test voisin
    `crates/kartenn-rest/tests/it/audit.rs`
  - dépend de : 11, 12

## P6 — Budgets et synchronisation

- [ ] 17. Banc `bench-events` : 2 000 puis 5 000 flux en processus, 50 changesets/s ;
  latence validation → écriture (p95, p99), mémoire par flux, tempête de 5 000 reprises
  en 10 s, surcoût de `NOTIFY` sur le changeset unitaire, coût de la requête de
  visibilité (500 sujets × 1 000 objets) — REQ-022-26 — verify : chaque mesure sous son
  budget, rapportée dans `docs/spec/invariants.md`
  - où : `crates/kartenn-bench/src/http.rs` (banc HTTP en processus),
    `crates/kartenn-bench/src/core_budgets.rs` (forme) ; sous-agent `bench-runner`
  - dépend de : 12
- [ ] 18. Synchronisation : `docs/spec/` (interfaces : flux d'événements ; invariants
  proposés ci-dessous ; architecture : lecteur partagé), ADR 0033 acceptée,
  `specs/README.md` — verify : sous-agent `spec-keeper` sans écart
  - où : `docs/spec/interfaces.md`, `docs/spec/invariants.md`, `docs/spec/architecture.md`
  - dépend de : 15, 16, 17

### Invariants proposés (numéros à attribuer à la synchronisation)

| ID | Invariant | Garantie | Test |
|---|---|---|---|
| INV-67 | Un abonné ne reçoit jamais un objet qu'il ne peut lire au moment de la remise, ni la trace d'un événement caché | App (visibilité par lot) | Propriété, tâche 8 |
| INV-68 | Une reprise depuis un curseur ne saute aucun événement validé après lui et visible, même après une transaction longue ; sinon `reset` | Plancher porté par le curseur | Tâche 10 |
| INV-69 | Le coût du flux en base ne dépend pas du nombre de connexions | Lecteur partagé | Tâche 7 |
| INV-70 | Un abonné lent ne retarde ni le lecteur ni les autres abonnés | Tampon borné, `reset` | Tâche 8 |
| INV-71 | Latence validation → client p95 < 500 ms à 50 écritures/s et 2 000 flux ; 5 000 flux par instance, < 64 Kio par flux | Banc `bench-events` | Tâche 17 |

---
status: approved
updated: 2026-10-23
---

# 022 — Événements poussés (SSE) : tâches

Spec validée le 2026-10-23 (arbitrages A1 à A8, questions Q-022-1 à 12,
[`decisions.md`](decisions.md)) ; la 022 précède les specs web 020 et 021 (D31 amendée). Chaque tâche est un
commit, laisse le build vert et nomme ce qu'elle implémente ; chacune porte `où :` et
`dépend de :`. Commandes lourdes par `k-heavy` (`k-check`, bancs).

## P0 — Références et décision

- [x] 1. Note de référence `docs/reference/push-serveur.md` : SSE (WHATWG HTML § 9.2),
  limites de connexions (RFC 9112 § 9.4, RFC 9113 § 6.5.2), mandataires (RFC 10036,
  `X-Accel-Buffering`), statuts (RFC 9110, RFC 6585), Fetch et CSP3, MCP (révisions
  2025-03-26, 2025-11-25, 2026-07-28, SEP-2575), alternatives (WebSocket,
  WebTransport, interrogation longue, Server Push, Web Push), `LISTEN`/`NOTIFY` ;
  normatif et choix de kartenn séparés — REQ-022-6, 9, 13 à 15, 23 — verify : chaque
  affirmation a son URL et sa section ; points non relus marqués « à vérifier »
  - où : forme de `docs/reference/api-http.md` ; étude du 2026-10-23 (arbitrage A1)
  - dépend de : —
  - fait : note écrite avec la validation de la spec ; restent « à vérifier » (motif
    MCP 2025-03-26, `connect-src` et `EventSource`, `Origin` sur un `GET` de même
    origine, bloc `id:` sans donnée, RFC 9220, `WebSocketStream`, déploiement de
    RFC 10036, OpenAPI 3.2), à lever avant les tâches 9 et 15.
- [x] 2. ADR 0034 « Événements poussés par SSE, vue volatile de l'outbox » (proposée) ;
  amendements arbitrés : REQ-006-30, 32 (A5), REQ-012-18 et INV-50 (A4, R24),
  REQ-005-3, 7 (A6), REQ-004-36 (`NOTIFY`, E1) ; D7 et D31 déjà amendées (A1, A7) —
  verify : liens croisés ; `specs/README.md` à jour
  - où : `docs/adr/0018-outbox-ordonnee-par-domaine.md` (forme), decisions § Arbitrages ;
    marqueur « *Amendée par la 022 …* » comme REQ-004-37
  - dépend de : 1
  - fait : `docs/adr/0034-evenements-pousses-sse.md` (proposée), indexée dans
    `docs/spec/README.md` ; marqueurs sur REQ-004-36, REQ-005-3, 7, REQ-006-30, 32,
    REQ-012-18 ; INV-50 précisé (audit par connexion pour le flux).

## P1 — Domaine pur

- [x] 3. Module `kartenn-service::events` : sortes (`Ready`, `Changes`, `Changeset`,
  `Reset`, point de reprise), traduction d'un `Event` d'outbox en événement SSE
  (table du design § Format), troncature à 1 000 objets et invalidation large,
  sérialisation WHATWG (une ligne `data:`, pas de LF/CR) — REQ-022-3, 4, 5 — verify :
  tests unitaires par type CloudEvents ; propriété : toute sortie est analysée par un
  analyseur WHATWG de test en l'événement d'origine
  - où : `crates/kartenn-store-pg/src/outbox.rs` (`Event`) ; payloads de
    `crates/kartenn-store-pg/src/write.rs` (`org.kartenn.changeset.applied`, événements
    de module) et `pending.rs` (`event`)
  - dépend de : 2
  - fait : `crates/kartenn-service/src/events.rs` (`translate`, `render`, `Filters`,
    `Frame`, `Parser` WHATWG de test). Choix : objets typés par leur sorte dans
    `carrier_object` (fonction fournie par le lecteur) plutôt que par les opérations ;
    un objet sans type d'objet (politique, liaison, déclaration) n'est pas poussé ;
    effets dérivés = sortes héritières des objets écrits quand le différentiel compte
    plus d'objets que l'événement (domaine, préfixe → préfixe, plage, adresse ; plage →
    adresse ; module → son type) ; un changeset approuvé donne aussi `changeset`
    `applied` ; un événement de module dans un changeset porte `event` (son identité) ;
    sorte `labels` (018) traduite depuis `org.kartenn.labels.changed`. E2 vérifiée :
    `Planned::object` couvre les affectations.
- [x] 4. Curseur : `(plancher, dernier numéro, identité, émission)`, chiffré AES-256-GCM
  avec une clé dérivée du trousseau, préfixe `k1.` ; refus d'un curseur altéré, d'une
  autre identité ou d'une autre clé — REQ-022-6, 8 — verify : aller-retour ; octet
  modifié → invalide ; rotation de clé : l'ancienne reste lisible
  - où : `crates/kartenn-auth/src/seal.rs` (`seal`, `open` : chiffrement du cookie) et
    `signed.rs` (`Purpose`, `KeyRing`) ; `crates/kartenn-rest/src/cursor.rs` (curseur
    signé de REQ-006-8, forme)
  - dépend de : 3
  - fait : `crates/kartenn-rest/src/events/cursor.rs` sur `seal`/`open`, usage
    `Purpose::EventCursor` (`kartenn/event-cursor/v1`) ; forme `k1.<clé>.<base64url>`
    (l'identifiant de clé reste lisible pour la rotation) ; 40 octets en clair.

## P2 — Lecteur partagé

- [x] 5. `NOTIFY` : `pg_notify('kartenn_outbox','')` dans les transactions qui écrivent
  l'outbox — E1, REQ-022-20 — verify : test d'intégration, une notification par
  transaction validée, aucune pour une transaction annulée
  - où : `crates/kartenn-store-pg/src/write.rs` (`write_events`, insertion de
    `org.kartenn.changeset.applied`), `crates/kartenn-store-pg/src/pending.rs` (`event`) ;
    test voisin `crates/kartenn-service/tests/it/outbox.rs`
  - dépend de : 2
  - fait : `outbox::notify` (canal `outbox::CHANNEL`) appelé après chaque insertion
    (une instruction de plus par transaction, mesurée par le banc) ; test
    `a_committed_changeset_notifies_the_outbox_once`.
- [x] 6. Plancher en mémoire : variante sans écriture de `floor.rs` (`pending_max`,
  `pending_xmax`, montée sur `pg_snapshot_xmin`) et balayage de la fenêtre — E6,
  REQ-022-7 — verify : transaction longue qui détient un numéro inférieur, jamais sautée
  - où : `crates/kartenn-store-pg/src/floor.rs`, migration 0027 (commentaire), tests
    `a_running_transaction_is_never_skipped`, `pass_floor`
  - dépend de : 2
  - fait : `crates/kartenn-store-pg/src/events.rs` (`Floor`, `raise`, `read`, `sweep`,
    `recent`, `kinds`, et pour les tâches 8 et 11 `visible`, `states`,
    `administrative`) ; test `a_long_transaction_holding_a_lower_number_is_never_skipped`
    (`kartenn-service/tests/it/events_floor.rs`). Choix : plancher de départ = juste sous
    le premier événement rechargé dans l'anneau (sinon le dernier numéro) ; le balayage
    horaire relit la fenêtre de l'anneau.
- [x] 7. Lecteur : connexion d'écoute dédiée (reconnexion avec attente croissante),
  interrogation de secours à 1 s, passe par lot de 1 000, typage groupé par
  `changeset_operation`, anneau (15 min, 10⁴) rechargé au démarrage — REQ-022-7, 20 —
  verify : requêtes du lecteur indépendantes du nombre d'abonnés ; écoute coupée →
  remise ≤ 1 s ; deux instances, même anneau
  - où : nouveau `crates/kartenn-store-pg/src/events.rs` (lecture) ; tâche dans
    `crates/kartenn-server/src/app.rs` à côté des relais d'audit (`relay_once`)
  - dépend de : 3, 5, 6
  - fait : `crates/kartenn-rest/src/events/reader.rs` (`Reader::start`, `pass`, `run` ;
    écoute `PgListener` hors pool, attente croissante 0,1 → 30 s ; secours
    `[events] poll_ms`), lancé par `kartenn-server/src/app.rs` (`Api::start`). Écart
    tranché sans demander : typage par `carrier_object` (une requête par passe), pas par
    `changeset_operation`. Vérifié : deux instances sur la même outbox
    (`a_resumption_on_another_instance_loses_nothing`) ; requêtes du lecteur comptées
    (`Counters`), indépendantes des connexions par construction, rapportées par le banc.
    Coupure forcée de l'écoute : `a_cut_of_the_listening_connection_loses_nothing`
    (`pg_terminate_backend`, remise en moins de 1,5 s, nouvelle écoute).
- [x] 8. Répartiteur et visibilité par lot : regroupement par sujet, requête sujets ×
  objets avec les prédicats RBAC, court-circuit de lecture globale, `changeset` à
  l'auteur et aux valideurs possibles, `mpsc` borné par connexion, débordement —
  REQ-022-16, 17, 21 — verify : test de propriété (remis = décision unitaire) ;
  opérateur externe ; client qui ne lit plus → `reset` `overflow`, fermeture à 30 s,
  autres abonnés à l'heure
  - où : `crates/kartenn-store-pg/src/rbac.rs` (`readable`, `authorize_many`),
    `crates/kartenn-service/src/authz.rs` ; propriété voisine du filtrage des listes
    (REQ-005-24, `crates/kartenn-store-pg/tests/it/`)
  - dépend de : 7
  - fait : `crates/kartenn-rest/src/events/hub.rs` (`Hub::publish`, `sights`,
    `events_for`, `send`) : sujets regroupés (`SubjectKey`), portée de lecture en cache
    10 s, types lus partout sans confinement décidés sans la base, une requête
    `events::visible` par lot (sujets × objets, `kartenn_scopes` + `kartenn_visible` ;
    objet fermé jugé sur sa dernière version) ; `changeset` à l'auteur et à qui
    `review::may_validate` (nouvelle fonction publique) ; `mpsc` borné, débordement →
    `reset` `overflow`, fermeture après 30 s : la connexion quitte les plafonds et son
    socket est fermé (`transport::ConnectionKill`, posé par `kartenn-server/src/http.rs`),
    test `a_client_that_no_longer_reads_loses_its_socket` sur un vrai port). Tests
    `the_objects_delivered_are_those_each_subscriber_may_read` (six portées dont un
    opérateur externe confiné, tirages déterministes),
    `a_slow_subscriber_delays_no_one_and_starts_over`.

## P3 — Point d'accès

- [x] 9. `GET /v1/events` : négociation, filtres et bornes, authentification cookie,
  Basic, Bearer, `Sec-Fetch-Site`, pas de CORS, `ready` avec `retry:` aléatoire,
  maintien 15 s, points de reprise 30 s, en-têtes (`no-store`, `X-Accel-Buffering: no`,
  `Incremental: ?1` en option), `Origin` et `Sec-Fetch-Site` pour le cookie —
  REQ-022-1, 2, 9 à 11, 13, 15, 19 — verify : client SSE de test ; matrice cookie ×
  `Origin` × `Sec-Fetch-Site` ; Basic et Bearer non soumis ; `Accept-Encoding` ignoré
  - où : nouveau `crates/kartenn-rest/src/events.rs` ; montage dans
    `crates/kartenn-rest/src/lib.rs` (pile de middlewares, l. 122-180) ; `csrf.rs`,
    `authn.rs`, `compression.rs` ; tests à l'image de `crates/kartenn-rest/tests/it/csrf.rs`
  - dépend de : 4, 8
  - fait : `crates/kartenn-rest/src/events/mod.rs` (route), `stream.rs` (corps) ; route
    hors `quota::shed` et hors compression (`NotForContentType::SSE`) ; cookie non
    réémis (`NotActivity`) ; 406 `not-acceptable` (`problem.rs`) ; section `[events]`
    (`kartenn-server/src/config.rs`). Règle d'origine propre au flux
    (`events::same_origin` : `Origin` et `Sec-Fetch-Site` vérifiés s'ils sont présents).
    Ordre retenu : `ready` toujours premier, puis `reset` le cas échéant. Tests
    `crates/kartenn-rest/tests/it/events.rs`.
- [x] 10. Reprise : `Last-Event-ID` et `last_event_id`, rejeu de l'anneau, `reset`
  `too_old` et `invalid_cursor` ; identité d'événement (`changeset`, `event`) —
  REQ-022-7, 8, 27 — verify : coupure pendant trois changesets, reprise sur la seconde
  instance ; curseur d'une autre identité ; **test de propriété des doublons** (deux
  instances, transaction longue qui retient le plancher, coupures et reprises au
  hasard : reçus ⊇ validés visibles, tout doublon porte une identité déjà vue, aucun
  doublon sans reprise)
  - où : `crates/kartenn-rest/src/events.rs` ; deux instances comme
    `crates/kartenn-server/tests` (`an_import_ends_on_another_instance_without_duplicates`)
  - dépend de : 9
  - fait : rejeu de l'anneau (`Hub::open`, `Hub::replay`), plancher de chaque curseur
    abaissé sous l'événement suivant du lot ; `reset` `too_old` / `invalid_cursor` ;
    tests `a_resumption_on_another_instance_loses_nothing`,
    `resumptions_lose_nothing_and_duplicate_only_on_resumption` (deux instances,
    transaction longue, coupures tirées). Un événement de module porte `event` sans
    `changeset` (identité distincte de l'`applied`, lue ainsi par le client Python).
- [x] 11. Révocation et échéances : sujets marqués par les changesets d'administration,
  revérification groupée toutes les 10 s, fermeture à `exp` et à l'échéance de session,
  `reset` `rights_changed` — REQ-022-18 — verify : cas d'acceptation de REQ-022-18 ;
  `authn.session.closed` (cause `revoked`)
  - où : `crates/kartenn-rest/src/authn.rs` (état des identités, cache 5 s, ADR 0020),
    `crates/kartenn-rest/src/cookies.rs` (échéances) ; test voisin
    `crates/kartenn-rest/tests/it/sessions.rs`
  - dépend de : 9
  - fait : `Hub::recheck` toutes les 10 s et dès qu'un changeset d'administration est lu
    (`RBAC_OBJECT`, liaison ou identité) : états en une requête (désactivée, sessions
    révoquées après l'ouverture → fermeture), portée relue et comparée → `reset`
    `rights_changed` ; échéance = `exp` du jeton (`VerifiedToken::expires`) ou fin de
    session sans activité (`QuietEnd`). Test
    `a_revocation_ends_the_stream_and_a_binding_removed_starts_it_over`. Ajouts après revue :
    `authn.session.closed` (cause `revoked`) enregistré quand la revérification ferme un
    flux par cookie (`a_revoked_session_records_its_end`) ; secret de compte de service
    présenté suivi (`authenticate_service_secret`, `events::revoked_secrets`) et sa
    révocation ferme le flux (`a_revoked_secret_ends_its_stream`).

## P4 — Ressources et exploitation

- [x] 12. Plafonds (instance, identité), hors délestage, quota d'ouverture ; arrêt
  propre (503, `retry:` étalé, fermeture) ; HTTP/2 ≥ 100 flux concurrents — REQ-022-12,
  14, 22 — verify : 5 001ᵉ flux → 503, lecture ordinaire servie ; 17ᵉ → 429 ; 1 000
  connexions fermées en < 2 s à l'arrêt
  - où : `crates/kartenn-rest/src/quota.rs` (`shed`, `InFlight`),
    `crates/kartenn-rest/src/transport.rs` (`max_in_flight`),
    `crates/kartenn-server/src/http.rs` (`GracefulShutdown`, l. 109-)
  - dépend de : 9
  - fait : plafonds dans `Hub::open` (503 / 429, `Retry-After: 5`), avant la base ;
    `Hub::shutdown` avant le drain (`Api::stop`) ; HTTP/2 `MAX_CONCURRENT_STREAMS = 200`.
    Tests `the_ceilings_refuse_before_the_base`,
    `the_stop_spreads_the_reconnections_and_ends_the_streams` (10 flux ; l'échelle est
    au banc).
- [x] 13. Guide de déploiement : mandataire inverse (tampon, `Incremental`, délai de
  lecture, HTTP/2), regroupeur de connexions et `LISTEN`, rétention courte des logs
  `events.delivery` (7 jours recommandés) — REQ-022-13, 14, 24 — verify : relecture
  - où : `docs/exploitation/configuration.md` (section `[events]`), `oci.md`,
    `nixos.md`, `observabilite.md` (métriques)
  - dépend de : 12
  - fait : `configuration.md` (section `[events]`, mandataire, regroupeur, sonde
    `curl -N`), `observabilite.md` (métriques, `events.delivery`, 7 jours), `oci.md`,
    `nixos.md` (connexion `LISTEN` en plus). Aucun nom de produit : le réglage du tampon
    est décrit par l'en-tête `X-Accel-Buffering` que « des mandataires courants »
    respectent.
- [x] 14. (Q-022-4, retenue le 2026-10-29) Rattrapage JSON : `GET /v1/events` avec
  `Accept: application/json`, page après un curseur — REQ-022-28 — verify : mêmes droits
  que le flux
  - où : `crates/kartenn-rest/src/events/mod.rs`, `events/hub.rs`, pagination de
    `cursor.rs`
  - dépend de : 10
  - fait : REQ-022-28 écrite d'abord (amende l'acceptation de REQ-022-1), design
    « Rattrapage JSON ». `Hub::page` (avis de l'anneau `> f` et `≤` plancher publié,
    triés par numéro, visibilité par tranches de 256, portée relue hors cache),
    `events::page` (`EventPage`, `EventItem`, `links.next` + `Link` si pleine,
    `data.objects.read`, `NotActivity`, `Vary: Accept`) ; le quota compte les pages
    (`events::wants_page`) ; contrat de référence et `web/sdk/src/api/schema.d.ts`
    régénérés. Tests `a_page_after_a_cursor_gives_the_events_by_bounded_pages`,
    `a_page_holds_only_what_the_caller_may_read` (deux instances),
    `a_page_starts_over_on_a_cursor_it_cannot_follow`,
    `pages_wait_for_a_long_transaction_and_lose_nothing`,
    `accept_asks_the_stream_a_page_or_neither`.
  - choix : `application/json` nommé sans `text/event-stream` → page (`*/*` ou absent :
    le flux) ; réponse alignée sur les listes (`items`, `links.next`) plus `cursor`, et
    `next` seulement quand la page est pleine, pour qu'un paginateur générique s'arrête à
    jour ; `id` de chaque élément = curseur `(seq, seq)`, compatible avec le flux ;
    sans curseur → page vide au point de départ ; `reset` seul, sans
    `links.next` ; pas de métriques `kartenn.events.*` pour les pages (lecture
    ordinaire) ; critères d'audit = filtres, `page_size`, `resumed`.

## P5 — Contrat, audit, télémétrie

- [x] 15. OpenAPI (réponse `text/event-stream`, `x-kartenn-events`, schémas des
  données), capacités (`events`) — REQ-022-23 — verify : contrat de référence mis à
  jour, test de REQ-006-34 vert
  - où : `crates/kartenn-rest/src/openapi.rs`, `contract.rs`, `capabilities.rs` ; test
    `crates/kartenn-rest/tests/it/contract.rs`
  - dépend de : 9
  - fait : `utoipa::path` de `/v1/events` (paramètres, 400 à 503), schémas
    `EventReady`, `EventChanges`, `EventChangeset`, `EventLabels`, `EventReset`,
    `EventObject`, extension `x-kartenn-events` (`openapi::events`) ; capacités `events`
    (ou `null`) ; contrat de référence régénéré ; `events` réservé au noyau
    (`CORE_COLLECTIONS`) ; instantané du catalogue Python mis à jour.
- [x] 16. Audit `events.stream.opened` / `closed` (identifiants bornés par
  `audit.read_ids_max`, `targets_omitted`) ; au-delà de la borne, log technique DEBUG
  `events.delivery` hors audit ; métriques et spans — REQ-022-24, 25 — verify :
  acceptation de REQ-022-24 ; événements au catalogue et au schéma ; aucun
  `events.delivery` dans le journal d'audit ; aucune span par connexion
  - où : `docs/spec/evenements-audit.md`, `evenements-audit.schema.json`,
    `crates/kartenn-rest/src/audit.rs`, `crates/kartenn-telemetry` ; test voisin
    `crates/kartenn-rest/tests/it/audit.rs`
  - dépend de : 11, 12
  - fait : `STREAM_OPENED`, `STREAM_CLOSED` au catalogue (doc et schéma régénérés) ;
    `events/audit.rs` (ouverture enregistrée avant le flux, en échec fermé ; fermeture
    à la chute du corps ; `events.delivery` DEBUG au-delà de `read_ids_max`) ;
    métriques `kartenn.events.*` (`kartenn-telemetry/src/otel.rs`, copiées de
    `Hub::stats` à chaque tour du relais) ; span `events.reader.pass`. Test
    `the_audit_records_the_stream_by_connection` (borne 2, 4 omis ; ajouté à la tâche
    18) ; l'écriture DEBUG `events.delivery` n'est pas vérifiée par un test.

## P6 — Budgets et synchronisation

- [x] 17. Banc `bench-events` : 2 000 puis 5 000 flux en processus, 50 changesets/s ;
  latence validation → écriture (p95, p99), mémoire par flux, tempête de 5 000 reprises
  en 10 s, surcoût de `NOTIFY` sur le changeset unitaire, coût de la requête de
  visibilité (500 sujets × 1 000 objets) — REQ-022-26 — verify : chaque mesure sous son
  budget, rapportée dans `docs/spec/invariants.md`
  - où : `crates/kartenn-bench/src/http.rs` (banc HTTP en processus),
    `crates/kartenn-bench/src/core_budgets.rs` (forme) ; sous-agent `bench-runner`
  - dépend de : 12
  - fait : `crates/kartenn-bench/src/events_budgets.rs`, `bench-events` (devenv) ; flux
    ouverts par le `Hub` en processus (l'authentification de l'ouverture est celle de
    `bench-http`). Échelle réduite (2026-10-23, 200 puis 500 flux, 20 identités) :
    validation → écriture p95 3,7 ms, p99 4,5 ms, aucun événement perdu ; 5,4 Kio par
    flux au repos ; 500 reprises servies en 81 ms, changeset unitaire p95 8,0 ms pendant
    la tempête ; surcoût du lecteur (`NOTIFY` compris, 5 tours alternés) 0,05 ms sur un
    p95 de 6,8 ms ; visibilité d'un lot 9,9 ms (33 sujets × 1 000 objets). La requête de
    visibilité regroupe les sujets de même portée (`events::visible`) : sans ce
    regroupement, 173 ms à l'échelle réduite, à surveiller à l'échelle complète si les
    portées sont toutes distinctes. Mesure complète : à lancer par l'orchestrateur.
- [x] 18. Synchronisation : `docs/spec/` (interfaces : flux d'événements ; invariants
  proposés ci-dessous ; architecture : lecteur partagé), ADR 0034 acceptée,
  `specs/README.md` — verify : sous-agent `spec-keeper` sans écart
  - où : `docs/spec/interfaces.md`, `docs/spec/invariants.md`, `docs/spec/architecture.md`
  - dépend de : 15, 16, 17
  - fait : `docs/spec/interfaces.md` (section « Flux d'événements »), `invariants.md`
    (INV-67 à 71, mesures réduites), `architecture.md` (composant, déploiement) ;
    ADR 0034 acceptée ; REQ-022-3 marquée amendée par la 018 (`labels`) ; tests ajoutés
    après la passe `spec-keeper` : `the_audit_records_the_stream_by_connection`,
    `a_cursor_older_than_the_ring_starts_over`. Écarts relevés : fermés ensuite
    (`authn.session.closed`, secret révoqué, coupure de `LISTEN`, socket d'un client
    bloqué, tâches 7, 8, 11) ; restent sans test de bout en bout `truncated`,
    `Incremental` et l'échéance (`QuietEnd`, `exp`) ; budgets INV-71 mesurés à
    l'échelle réduite seulement.

## P6 — Écart relevé (2026-10-29)

- [ ] 19. Ouverture d'un flux comptée dans le quota de lecture — REQ-022-22 (« leur
  ouverture compte une fois dans le compartiment de lecture du quota (REQ-006-29) ») —
  verify : test d'acceptation (un flux ouvert débite une fois le compartiment de lecture
  de l'identité ; compartiment vide → 429 `Retry-After` avant toute ouverture, sans
  compter dans les plafonds de flux ni dans le délestage ; la connexion ouverte ne
  débite plus rien ensuite) ; les plafonds par instance et par identité inchangés ;
  `k-check`
  - constat : la couche de quota laisse passer `/v1/events` sans débit dès que la requête
    n'est pas une page de rattrapage (`quota.rs`, `layer`, test `wants_page`), donc
    l'ouverture d'un flux ne compte pas, contrairement à REQ-022-22 ; relevé à la tâche
    14.
  - où : `crates/kartenn-rest/src/quota.rs` (`layer`, `Charge`, compartiments) ;
    `crates/kartenn-rest/src/events/mod.rs` (`wants_page`, ouverture du flux, plafonds) ;
    ordre des couches (quota avant délestage, REQ-006-29, 30) dans
    `crates/kartenn-rest/src/lib.rs` ou le routeur du serveur ; tests à imiter :
    `crates/kartenn-rest/tests/it/quota.rs` (`behind`), `tests/it/events.rs` (plafonds par
    identité) ; `docs/spec/interfaces.md` (quotas du flux) si la description change
  - dépend de : 14

### Invariants proposés (numéros à attribuer à la synchronisation)

| ID | Invariant | Garantie | Test |
|---|---|---|---|
| INV-67 | Un abonné ne reçoit jamais un objet qu'il ne peut lire au moment de la remise, ni la trace d'un événement caché | App (visibilité par lot) | Propriété, tâche 8 |
| INV-68 | Une reprise depuis un curseur ne saute aucun événement validé après lui et visible, même après une transaction longue ; sinon `reset`. Doublons seulement à la reprise, reconnaissables (identité, révision) | Plancher porté par le curseur | Tâche 10 (propriété des doublons) |
| INV-69 | Le coût du flux en base ne dépend pas du nombre de connexions | Lecteur partagé | Tâche 7 |
| INV-70 | Un abonné lent ne retarde ni le lecteur ni les autres abonnés | Tampon borné, `reset` | Tâche 8 |
| INV-71 | Latence validation → client p95 < 500 ms à 50 écritures/s et 2 000 flux ; 5 000 flux par instance, < 64 Kio par flux | Banc `bench-events` | Tâche 17 |

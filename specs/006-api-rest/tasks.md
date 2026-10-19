---
status: approved
updated: 2026-10-18
---

# 006 — API REST : tâches

Chaque tâche est un commit, laisse le build vert (`devenv test`) et nomme ce qu'elle
implémente. Les tests d'intégration HTTP passent par `tower::ServiceExt::oneshot` sur le
routeur complet, sauf mention « vrai TCP ».

## P1 — Socle

- [x] 1. Crate `kartenn-rest` (dépendances du design, `workspace-hack` à jour), routeur
  `/v1`, type `Problem` et table de correspondance des erreurs du service, couche
  d'identifiant de requête (UUID v7 toujours généré, `Request-Id`), sondes
  `/health/live` et `/health/ready` — REQ-006-3, 20, 36, 37 — verify : panique → 500
  générique avec l'identifiant ; base arrêtée → `ready` 503, `live` 200 ; même
  identifiant dans la réponse, le log et l'audit — `problem.rs` (`ProblemKind`, URN),
  `error.rs` (`RequestError`, `WriteError`, `ImportError` ; erreurs de structure du
  changeset → 400), `request_id.rs` (contexte à acteur `Unknown`, `instance` réécrit
  au retour, log `http.request`), `health.rs` (`PgReadiness` : migrations +
  `AuditStatus` partagé, nouveau dans `kartenn-service::audit`) ; `tests/socle.rs`.
  Dettes : `AuditStatus::update` depuis `relay_all` et `traceparent` de confiance
  (toujours `false`) à câbler en 2 et 3
- [x] 2. Serveur : configuration `[http]` (refus des champs inconnus), boucle
  d'acceptation `hyper-util` (HTTP/1.1, h2, h2c), TLS `tokio-rustls` + `ring`, ALPN,
  délai de poignée de main, rechargement du certificat sur `SIGHUP`, arrêt propre dans
  `app::run` — REQ-006-32 — verify (vrai TCP) : h2 par ALPN, HTTP/1.1, rechargement sans
  coupure, arrêt dans les 10 s de REQ-012-34 —
  `config.rs` (`Http`, `Tls`, `Rates`, défauts du design, contrôles), `http.rs`
  (`Certificates` sur `ArcSwap`, clé vérifiée contre le certificat ; `serve` : poignée de
  main 10 s, `ConnectInfo`, vidange 5 s), `app.rs` (`Api` arrêtée en premier, `SIGHUP`,
  `AuditStatus` mis à jour par `relay_all`) ; `tests/http.rs` (5 tests vrai TCP, dont
  `run` complet : `ready`, `SIGHUP`, arrêt < 10 s)
- [x] 3. Couches de transport : proxys de confiance et `Forwarded` (RFC 7239), délai,
  limite de corps, refus de `Content-Encoding`, 414 au-delà de 8 Kio d'URL —
  REQ-006-10 (414), 27 (transport), 31 — verify : `Forwarded` d'une adresse non
  configurée ignoré ; 413 annoncé et en cours de lecture ; 415 ; 414 —
  `kartenn-rest/src/transport.rs` (`TrustedProxy` CIDR, `Settings`, `Client` en
  extension, `ConnectionTls` posé par le serveur ; couches délai → 503 `unavailable`
  en problème, `Forwarded`, limites : 414 sans corps, 415 avec `Accept-Encoding:
  identity`, 413 sur `Content-Length` puis `Limited` en lecture),
  `RequestBodyTimeoutLayer`, `traceparent` suivi des proxys de confiance (REQ-012-7),
  `router(…, Settings)` ; `[http] trusted_proxies` typé et contrôlé ;
  `tests/transport.rs` (5 tests) et tests unitaires du parcours `Forwarded`
- [x] 4. Délestage (`LoadShed` + `ConcurrencyLimit` → 503) et quotas GCRA par identité
  (couche propre sur `governor` : poids, `Retry-After`, `RateLimit-Policy`,
  `RateLimit`, purge et plafond de clés, événement d'abus agrégé) — REQ-006-29, 30 —
  verify : 21ᵉ écriture d'une rafale → 429 et en-têtes ; plafond atteint → 503 sans
  requête SQL ; la table des clés ne dépasse pas son plafond sous clés variées —
  `kartenn-rest/src/quota.rs` : délestage par sémaphore à `try_acquire` (couche 2,
  même effet que `LoadShed` + `ConcurrencyLimit`, qu'axum aurait posés route par
  route), 503 `Retry-After: 1` ; `Quotas` (`r`, `w` par identité du `RequestContext`,
  `a` par adresse, IPv6 en /64), `Charge::weigh` et `chunk_weight` pour les imports,
  plafond de clés (100 000, au-delà une clé de débordement partagée), `tick` : purge
  et refus agrégés par minute ; `abuse.rate.exceeded` gagne `count` et
  `window_seconds`, acteur `system` sans identité (REQ-012-9) ; serveur : plafond
  2 × le pool, minuterie d'une minute, refus restants écrits à l'arrêt ;
  `tests/quota.rs` (4 tests) et tests unitaires (poids, /64, agrégation)
- [x] 5. Compression (`zstd`, `br`, `gzip`, > 1 Kio) sauf extension `Secret` —
  REQ-006-32 — verify : liste compressée selon `Accept-Encoding` ; réponse marquée
  `Secret` jamais compressée, `no-store`
  — `compression.rs` : `CompressionLayer` (sans `deflate`), `SizeAbove(1025)` et
  absence de `Secret` ; `Secret` est une partie de réponse qui pose l'extension et
  `Cache-Control: no-store` ; couche 10, la plus intérieure ; `tests/compression.rs`
  (3 tests)

## P2 — Authentification

- [x] 6. Couche d'authentification : cookie, Bearer, Basic → `Principal` ; deux preuves
  → 400 ; `authn::seen` (juste-à-temps sans événement) ; 401 et `WWW-Authenticate`
  (RFC 6750) ; Basic et cookies refusés hors TLS ; `FailureLimiter` branché —
  REQ-006-26, 27 — verify : jeton expiré → `invalid_token` ; sans preuve → 401 sans
  `error` ; aucun `authn.login.succeeded` par requête Bearer ; matrice TLS × `Forwarded`
  — `authn.rs` : port `Authenticator` (`Authn` sur les briques de la 005, `Nobody` sans
  `[auth]`), couche 6 entre les limites et le quota ; publiques : hors `/v1` et
  `auth/login|callback|breakglass` ; défi `Bearer` seul (pas de `Basic` : dialogue du
  navigateur) ; clés d'émetteur injoignables → 503 ; `FailureLimiter` 20/min par source
  → 429, échecs agrégés vidés par la minuterie des quotas ; `authn::seen` dans le
  service ; `tests/authn.rs` (5 tests), `tests/common` (authentificateur des autres tests)
- [x] 7. Module `cookies` et routes `auth/login`, `auth/callback`, `auth/logout`,
  `auth/breakglass`, `me` ; réémission au plus une fois par minute ; session de secours
  d'1 h ; `session_closed` — REQ-006-24, 25 — verify : flux OIDC contre l'émetteur de
  test de la 005 ; `state` rejoué refusé ; attributs des cookies ; secours refusé après
  1 h
  — `cookies.rs` (pose, lecture par morceaux, suppression `Max-Age=0`), `auth.rs`
  (routes, 303 vers l'émetteur, `/` ou sa déconnexion ; `breakglass` → 204 ; `me` avec
  les rôles) ; port `Authenticator` étendu (`begin`, `complete`, `breakglass`, `logout`,
  `roles`), `Authn` porte l'`Oidc` ; couche 6 : session réémise après 60 s ou après
  rafraîchissement des groupes (REQ-005-7, refus de l'émetteur → `refresh_failed`),
  cookie supprimé à toute fin de session, `session_closed` (`idle`, `absolute`,
  `revoked`, `logout`) ; 005 : `Session::breakglass` (1 h), `unseal`/`check`,
  `Identities::roles` ; hors TLS → 400 sans cookie ; `tests/sessions.rs` (5 tests,
  émetteur de test sur la boucle locale)
- [x] 8. Couche CSRF (`Sec-Fetch-Site`, repli `Origin` = origine publique) —
  REQ-006-28 — verify : matrice complète `Sec-Fetch-Site` × `Origin` ; Bearer et `QUERY`
  non concernés
  — `csrf.rs` : couche 8 entre le quota et la route, sur les principaux par cookie
  (`OidcSession`, `Breakglass`) et toute méthode hors `GET`, `HEAD`, `OPTIONS`, `QUERY`
  (`quota::reads`) ; `Origin` comparé entier, sans casse ; sans `public_origin`, seul
  `same-origin` passe ; `transport::Settings::public_origin` alimenté par `[http]` ;
  `tests/csrf.rs` (matrice × `POST`/`PUT`/`PATCH`/`DELETE`, méthodes sûres et Bearer) ;
  `tests/sessions.rs` envoie `Sec-Fetch-Site: same-origin`

## P3 — Lectures

- [x] 9. Lectures datées et cache : analyse stricte RFC 3339, `as_of` futur refusé,
  `transaction_timeout` sur le pool d'écriture, passé figé `immutable`, ETag fort
  (objet) et faible (page), 304 — REQ-006-5, 6 — verify : date sans décalage → 400 ;
  `immutable` seulement au-delà de `T` ; 304 puis 200 après un changeset
  — `dated.rs` : extracteur `Dates` (`valid_at`, `as_of` en doublon → 400), `Moment`
  (`now()` de la base lu avant les données, figé si `now − as_of ≥ T + 1 s`), `ETag`
  (`version` fort, `page` faible SHA-256), `respond` (304 par comparaison faible,
  `Cache-Control` et `ETag` sur 200 comme 304) ; `transport::Settings::
  write_transaction_timeout` ; `read::version` et `read::now` (store) ;
  `app::bounded` : `transaction_timeout` sur le pool unique de `run` (arbitrage : pas de
  second pool, design corrigé) ; `tests/dated.rs` (route d'essai), `tests/pool.rs`
  (serveur, transaction coupée au-delà de la borne)
- [x] 10. Curseur signé (`Purpose::Cursor`), pagination par clé sur les préfixes,
  `Link: rel="next"`, `page_size` 1 à 1 000, curseur seul avec `page_size` — REQ-006-7,
  8 — verify : octet modifié → 400 ; écritures entre deux pages sans saut ni répétition ;
  liaison retirée entre deux pages
  — `kartenn-auth::signed` (`v1.<clé>.<contenu>.<HMAC>`, base64url, étiquette vérifiée
  avant décodage) ; `cursor.rs` : `Page::from_query` (dates figées par le curseur,
  paramètres restants triés pour 006-11), `Page::next`, `next_href`, `respond_page`
  (`{items, links.next}`, `Link`, ETag faible) ; `list::prefixes` (store, clé domaine,
  valeur, UUID, portée de lecture) et migration 0034 (index de clé, table et historique) ;
  `tests/cursor.rs` (route d'essai). Écarts au design : le curseur porte aussi la
  collection (un curseur d'une autre liste → `invalid-cursor`) ; pas de « sens », la
  pagination n'allant qu'en avant
- [x] 11. Filtres par paramètres (opérateurs après `:`), traduction vers
  `kartenn_store_pg::filter`, route `QUERY` (`any()`, contrôle de méthode, `Allow` écrit,
  `Accept-Query`) — REQ-006-9, 10 — verify : mêmes éléments par `GET` et `QUERY` ;
  paramètre inconnu → 400 nommé ; `QUERY` sans `Content-Type` → 415 ; 405 et `Allow`
  — `filter.rs` : `parse` (champs du noyau de la collection, `prop.<clé>`, `:between`
  `a..b`, `:exists`, valeurs répétées → `OneOf`, 400 nommant le paramètre) vers
  `Condition` du store ; corps `QUERY` (`filters[{field, op: eq|between|exists,
  value}]`, bornes `[bas, haut]`) réécrit en paramètres canoniques, ajoutés à ceux de
  l'URL, lus par l'extracteur `ListQuery` : même code et même curseur que `GET` ;
  `collection(list)` ; `ProblemKind::MethodNotAllowed` ; `tests/filter.rs` (route
  d'essai sur `filter_in`). Écarts : `get(…).fallback(…)` plutôt que `any()` (axum ne
  connaît pas `QUERY` ; laisse place au `.post` de 006-14), 405 en `method-not-allowed`
  (classe absente de la table « Erreurs ») ; clé non déclarée → 400 à la charge des
  routes (006-12, 13)
- [x] 12a. État de `v1`, DTO et lectures des objets du noyau (entités, sites, domaines,
  préfixes, plages, adresses) : listes paginées et filtrées par clé naturelle, lecture
  par UUID avec résumés liés, historique — REQ-006-1, 2, 4, 23 — verify : clé naturelle
  en filtre ; renommage sans changement d'URL ; 404 identique pour invisible et absent
  — `objects.rs` : `Api` (pool, anneau des curseurs, borne, journal, `ReadAudit`),
  `v1(api)`, DTO par type (résumés `{id, slug, href}`, `links.self/history/properties`,
  valeurs canoniques par le noyau), filtres de clé naturelle (égalité ou « l'une de »,
  valeurs vérifiées), lecture par `reads::read_object` (droits, audit, 404 si autre
  type), historique (droit de lecture actuel, sinon 404) ; listes et historique audités
  (`data.objects.read`) ; `list::objects`, `list::link`, `list::history` (store,
  `list::prefixes` réécrit dessus) et migration 0035 (index de clé) ; câblage dans
  `app::Api::start` (anneau de `[auth]`, sinon tiré au départ) ; `tests/objects.rs`.
  Dettes : historique non paginé ; `prop.<clé>` → 400 jusqu'à 006-13
- [x] 12b. Arbre : enfants, ancêtres, occupation, prochain libre (`kind=prefix&length=…`,
  `kind=address`), liens de navigation des préfixes — REQ-006-4, 23 — verify : enfants
  invisibles absents ; préfixe invisible → 404
  — `tree.rs` (rest) : `/v1/prefixes/{id}/children|ancestors|occupancy|next-free`, garde
  `objects::gate` (extraite de l'historique : 404 + `authz.access.denied`), enfants et
  ancêtres filtrés par la portée de lecture, nœuds `{kind, id, href, …}`, lectures
  auditées (`data.objects.read`) ; occupation en chaînes décimales (compte IPv6 > 2⁵³) ;
  prochain libre `null` si rien ne reste, longueur hors du conteneur → 400, mauvaise
  nature → 409 ; `links.children/ancestors/occupancy` des préfixes. Store : `tree.rs`
  bitemporel (`*_as_of`, relations `snapshot` avec `as_of`, tables courantes sans),
  `prefix_as_of` ; anciennes signatures conservées (bench-core : sous-arbre et
  occupation p95 12 ms). `tests/objects.rs` (`the_tree_shows_only_what_the_caller_may_read`).
  Écarts : occupation et prochain libre comptent le contenu invisible (mesure du
  préfixe ; ne pas proposer un pris). Dettes : enfants non paginés (comme
  l'historique) ; arbre à un `as_of` passé en parcours complet de `prefix` (l'`UNION ALL`
  avec l'historique partitionné n'est pas aplati, l'index par domaine est perdu)
- [x] 12c. Anomalies : liste filtrée par la visibilité de l'objet, acquittement
  (`core.anomaly.acknowledge` sur l'objet, `motive` obligatoire, événement d'audit, sans
  `If-Match`) — REQ-006-4, 23 — verify : anomalie d'un objet invisible absente et 404 à
  l'acquittement ; motif manquant → 422 ; acquittement audité
  — `anomalies.rs` (rest) : `GET /v1/anomalies?rule|domain|entity` (lecture auditée
  `data.objects.read`, lien vers l'objet), `POST /v1/anomalies/{id}/acknowledgement` →
  204 ; ordre : anomalie ouverte → `gate` de lecture (404 + `authz.access.denied`) →
  permission (403 audité) → corps (415, 400, 422 `motive-required`), pour qu'aucune
  erreur de corps ne serve d'oracle. Store : `anomalies::open_within` (portée de lecture,
  objet courant), `find`. Catalogues : permission `core.anomaly.acknowledge` (aucun rôle
  livré hors `*`), événement `anomaly.state.acknowledged` (détails `rule`, `object`) ;
  docs générées. Titre de `motive-required` rendu générique. `tests/objects.rs`
  (`anomalies_follow_the_visibility_of_their_object`). Écarts : liste non paginée ;
  écriture et audit hors d'une même transaction (comme les lectures) ; une anomalie
  dont l'objet est clos n'est plus listée ni acquittable
- [x] 12d. Dettes de lecture (12a, 12b) : enfants et historique paginés par clé et curseur
  signé (enfants : adresse, rang, id ; historique : validité, temps système) ; arbre à
  un `as_of` passé indexé (préfixe lu d'abord, domaine et valeur passés en paramètres
  aux requêtes des enfants et ancêtres ; index des partitions de `prefix_history` à
  vérifier) ; mesure « sous-arbre au passé » dans `bench-core` — REQ-006-4, 5, 7, 8 —
  verify : page suivante des enfants et de l'historique par curseur ; sous-arbre au passé
  < 200 ms (p95) au `bench-core`
  — Store : `tree::children_of` (préfixe lu d'abord, domaine et valeur en paramètres,
  clé `ChildKey` = première adresse, rang, id ; `LIMIT`), `ancestors_of` et
  `ancestors_as_of` sur le même principe ; occupation et prochain libre réutilisent le
  préfixe déjà lu ; `list::history_page` (`as_of` : versions assertées alors, une
  version retirée plus tard rendue courante ; clé validité, temps système). Rest :
  `children` et `history` par `Page` (collection propre à l'objet, filtre inconnu → 400,
  `valid_at` sans effet sur l'historique), `Page::unfiltered`. `tests/objects.rs`
  (`children_and_history_page_by_cursor` : enfant créé entre deux pages absent, curseur
  altéré ou d'une autre liste → 400). `bench-core` : « subtree and occupancy in the
  past » p95 13 ms (courant 4 ms) ; plan : index `*_keyset` par domaine sur les
  partitions peuplées, parcours séquentiels sur les seules partitions vides.
  Écart : ancêtres non paginés (profondeur ≤ 128)
- [x] 12e. Écarts de 12d à arbitrer puis traiter : ancêtres non paginés (au plus 128,
  à paginer comme les autres listes ou exception écrite à REQ-006-7) ; `valid_at`
  accepté sans effet sur l'historique (REQ-006-5 : le refuser par 400, ou ne garder
  que les versions valides à cette date) ; historique à un `as_of` passé : une version
  retirée après `as_of` est rendue courante (`asserted_to` nul), choix à écrire dans
  `design.md` ; planification des lectures au passé (~9 ms au `bench-core`, une
  partition d'historique par mois) à surveiller — REQ-006-5, 7 — verify : selon
  l'arbitrage
  — Arbitré : ancêtres en entier, exception écrite à REQ-006-7 ; `valid_at` filtre
  l'historique (store `list::history_page`, `valid_period @> valid_at`), gardé par la
  clé du curseur `[valid_from, asserted_from, valid_at]` ; `asserted_to` nul au passé,
  écrit dans `design.md` (Historique). `tests/objects.rs`
  (`history_keeps_the_versions_valid_at_and_known_as_of`). Planification au passé :
  à surveiller au `bench-core`, sans changement.
- [x] 13a. Lectures des propriétés : déclarations et listes de valeurs (tout authentifié),
  valeurs résolues, explication, suggestion (registre `Sources` dans l'`Api`), écarts ;
  lecture à un temps système passé ; message d'écart masqué pour une propriété sensible
  (aussi dans `/v1/anomalies`) — REQ-006-4 — verify : valeur sensible masquée sans
  droit ; explication au passé ; nœud invisible de la chaîne à `null`
  — `properties.rs` (rest) : `property-declarations` et `…/{key}/values` paginés par
  clé (404 clé inconnue, 409 hors `enum`) ; `…/{id}/properties` par `read_object`
  (`masked`) ; `explanation` (objet du bon type à la date, `gate`, nœuds visibles sinon
  `null`, valeur et valeur remplacée masquées) ; `suggestion` (`as_of` → 400, `Sources`
  dans `objects::Api`, vide au serveur) ; `property-gaps` et `mask_gaps` (aussi dans
  `anomalies.rs`). Service : `reads::sensitive_disclosed` (`data.sensitive.read` par
  divulgation). Store : `property_read` (déclarations, valeurs, clés sensibles, natures
  des objets à une date). `tests/objects.rs` (`properties_resolve_explain_and_mask`,
  `a_deviation_of_a_sensitive_property_hides_its_value`). Écarts : une seule anomalie
  ouverte par règle et objet (`anomaly_one_open`), donc un seul écart par objet, le
  dernier message l'emportant (dette de la 003) ; propriété d'un écart lue dans le
  préfixe du message ; listes des écarts non paginées (comme les anomalies)
- [x] 13b. Filtres `prop.<clé>` des listes de 12a par le moteur de la 003 : un seul
  `domain=` exigé, conditions intersectées, tri et curseur par UUID, 400 sans domaine,
  sur entités et sites ou au passé — REQ-006-9 — verify : `?domain=lab&prop.env=prod
  &prop.env=test` ; page suivante par curseur ; 400 qui nomme le paramètre
  — Store : `filter::filter_all_in` (conditions intersectées par `id IN`, sorte d'objet
  `$11`, valeurs de clé naturelle `$12`, paramètres par condition), `filter_in` y délègue ;
  `list::Criteria.ids`. Rest : `objects::filtered` (déclaration à `valid_at`, valeurs
  vérifiées selon le type, domaine par slug, inconnu → page vide, `value`/`first`
  intersectés), objets relus par identifiants et triés par UUID ; curseur sans `as_of`
  (`Page::next_current`, exception de REQ-006-7). `tests/objects.rs`
  (`properties_filter_the_objects_of_one_domain`)

## P4 — Écritures

- [x] 14. `POST /v1/changesets` : simulation, 201/202/200, lien `rel` du changeset,
  422 et `violations` avec pointeurs JSON, priorité des problèmes — REQ-006-11, 21, 22
  — verify : changeset à deux violations de règles différentes → une réponse avec les
  deux ; non authentifié et malformé → 401
  — `changesets.rs` (rest) : corps `{motive, valid_from, valid_to, operations}` (JSON
  seul → 415, sinon 400), `valid_from` absent → heure de la base, `?simulate=` ; 201
  `Location` et `Link` `urn:kartenn:rel:changeset`, 202, 200 sans lien, 422
  `violations` `{rule, severity, detail, subjects, operation, pointer}` et `changeset` ;
  message d'écart d'une propriété sensible masqué (`properties::masked_gap`). Noyau :
  `Violation.operation` et `at`. Service : `plan` rend l'opération de chaque écriture,
  `apply`, `cascade::expand` et `check_declared_sources` l'attribuent ; journalisée
  (`violations_json`). `PgAuditJournal::instance`. `tests/changesets.rs` (201 et
  simulation, 202 d'une proposition, 422 à deux règles, `valid_to` seul, ordre
  401/400/415/422) ; `planning_collects_every_local_violation` (indices)
- [x] 15. `Idempotency-Key` : verrou consultatif (409 en cours), rejeu reconstruit du
  journal, contenu différent → 422, purge par tâche (branchée en 20) — REQ-006-15 —
  verify : deux requêtes simultanées de même clé → une 201, une 409 ; rejeu égal octet
  pour octet ; autre identité → second objet
  — Store : `replay` par `pg_try_advisory_xact_lock` (`Replay::InProgress`), migration
  0036 `changeset.cascaded` (`Journal.cascaded`), `idempotency::journaled` et
  `purge_expired`. Service : `RequestError::IdempotencyInProgress` → 409. Rest :
  en-tête (1 à 255 caractères, une fois, sinon 400), réponses 201/202/422 partagées par
  la soumission et le rejeu (`replayed`), 422 erreurs puis avertissements.
  `tests/changesets.rs` (`a_replay_answers_the_first_response_byte_for_byte`,
  `a_replay_of_a_refusal_answers_the_same_problem`, `a_replay_of_a_proposal_answers_202`,
  `a_key_in_progress_answers_409`) ; `idempotency_keys_replay_or_conflict` (pipeline)
- [x] 16. Routes par ressource : `POST`, `PATCH` (Merge Patch, `{"clear": true}`),
  `DELETE` ; membres réservés `motive`, `valid_from`, `valid_to` ; `If-Match` → 428, 412 —
  REQ-006-12, 13, 14 — verify : mise à jour perdue empêchée ; JSON Patch → 415 ; retrait
  et effacement de propriété distincts
  — Store : migration 0037 `object_version` (`WriteTx::version` sous `FOR UPDATE`,
  `touch`), ETag des lectures sans `as_of`, `list::created`. Service :
  `Operation::Expect` (garde, `core.<kind>.read`), `conflict()` sur la version, `touch`
  après chaque écriture d'`apply`. Rest : `resources.rs` (corps du DTO, références
  `{id}`, `If-Match`, garde puis champs puis propriétés, `invalid-member`), réponses de
  `changesets.rs` partagées (`Answer`, pointeurs de membre, 412 sur conflit,
  `Location` de l'objet créé, rejeu). `tests/resources.rs` (`a_lost_update_is_prevented`,
  `a_patch_is_a_merge_patch`, `removing_and_clearing_a_property_differ`,
  `a_creation_links_to_its_changeset`, `a_creation_replays_its_first_response`,
  `a_deletion_closes_the_object`, `an_approval_policy_answers_202`,
  `a_patch_renames_and_moves`)
- [x] 17. Cycle d'un changeset : lecture, liste filtrée, différentiel paginé,
  validation, rejet, retrait, annulation (409 et conflits), propositions ; `If-Match`
  sur les actions — REQ-006-16 — verify : auteur qui valide → 403 ; annulation en
  conflit → 409 avec la liste
  — Core : permission globale `core.changeset.read` (`docs/spec/permissions.md`).
  Service : `reads::changesets_readable` (objet `CHANGESETS`, refus audité). Store :
  `changesets.rs` (`find`, `page` en keyset, `summaries`, `diff_objects`), migration
  0038 `changeset_keyset`, `property_read::sensitive_ids`. Rest : `cycle.rs` (lecture,
  liste, différentiel, propositions, actions sous `If-Match` = état, annulation qui
  soumet l'inverse ou 409 `undo-conflict`), masquage des valeurs sensibles (opérations,
  détail et décompte) et du condensat d'un secret ; `answered` partagé (revalidation
  échouée → 422), `entity_tag`, `motive` partagé, refus de revue → 403 / 409.
  `tests/cycle.rs` (`an_author_who_validates_gets_403`,
  `an_undo_in_conflict_answers_409_with_the_list`, `reading_needs_core_changeset_read`,
  `the_list_filters_by_state_and_author_by_pages`,
  `rejection_and_withdrawal_close_a_pending_changeset`,
  `the_proposals_are_those_the_caller_may_validate`,
  `a_sensitive_value_is_masked_without_its_right`)
- [x] 18. Administration des droits : rôles, liaisons, rattachements, comptes de
  service et secrets (réponse `Secret`, `no-store`), état des identités — REQ-006-4,
  32 — verify : délégué sans escalade (005) par HTTP ; secret jamais compressé
  où : design « Ressources et routes » (ligne Droits) ; store `rbac_admin.rs` (`bind`,
  `add_secret`, `binding_scope`) ; réponse `compression::Secret` ; modèle de route
  `rest/src/resources.rs`, branchement `rest/src/lib.rs::v1` ; test à imiter
  `tests/resources.rs`
  — Arbitrage au design (« Droits », 2026-10-19). Core : permission globale
  `core.rbac.read` (`docs/spec/permissions.md`). Service : `reads::rbac_readable`
  (contrôle global partagé avec `changesets_readable`), port `Identities::rights`
  (`Rights`, `HeldBinding`) à la place de `roles`. Store : `rbac_read.rs` (rôles,
  liaisons, rattachements, comptes et secrets sans condensat, identités, liaisons d'un
  sujet, permissions globales), `request_hash` sans l'identifiant ni le condensat d'un
  secret. Auth : `Session::ends_at`. Rest : `rights.rs` (lectures paginées et
  auditées, écritures par changeset d'une opération, secret généré et montré une fois
  sous `Secret`), `Answer.shown` et `Location::{Fixed, Binding, Account}` dans
  `changesets.rs`, `SessionEnds` et `GET /v1/me` étendu (`auth.rs`). `tests/rights.rs`
  (`a_delegate_binds_without_escalation`, `a_secret_is_never_compressed`,
  `a_replay_does_not_show_the_secret_again`, `reading_the_rights_needs_core_rbac_read`,
  `attachments_and_identities_are_administered`), `tests/sessions.rs` (liaisons et fin
  de session dans `/v1/me`)
  choix : pas de vérification d'existence avant une écriture (rôle, liaison ou secret
  inconnu → 422 du pipeline, pas de 404) ; `PUT` d'un rôle → 201 ou 200 selon sa
  présence lue avant ; `Location` d'un ajout de secret → le compte
- [ ] 19. Audit et exploitation : recherche et export (`GET`, `QUERY`),
  pseudonymisation, filtres de log à chaud — REQ-006-4 — verify : consultation sans
  permission refusée et auditée ; filtre qui revient à la configuration
  où : design « Ressources et routes » (ligne Audit, Exploitation) ; service
  `reads::read_audit`, `observability::set_log_filter` ; store `audit.rs` (`search`,
  `pseudonymise`) ; `QUERY` comme `rest/src/filter.rs` ; test à imiter `tests/filter.rs`

## P5 — Tâches de fond et imports

- [ ] 20. Migration `job`, planificateur sous bail (scrutation 5 s, bail 30 s, battement
  10 s), tâches `expire-pending`, `reevaluate-expected`, `public-drift`,
  `idempotency-purge` ; minuteries locales (caches, limiteurs, JWKS) — REQ-006-38 —
  verify : trois planificateurs sur une base, une exécution par échéance ; détenteur
  arrêté → reprise après le bail
  où : design « Imports et tâches de fond » ; `review::expire_pending`,
  `expected::reevaluate_outdated`, store `registry::public_drift` ; migration suivante
  `0039_job.sql` ; boucle lancée depuis `kartenn-server/src/app.rs`
- [ ] 21. Imports : migration `import_chunk` et état `uploading`, `PUT` des morceaux
  (identique ignoré, différent → 409, > 10 000 → 413), `start` (202), `resume`,
  `cancel`, état et `Retry-After`, exécution comme tâche — REQ-006-17, 18, 19 — verify :
  10⁶ éléments en 100 morceaux dont un renvoyé ; arrêt d'une instance pendant
  l'exécution, fin sur une autre sans doublon
  où : design « Imports et tâches de fond », « Transport » (limites des morceaux) ;
  service `import.rs` (`create_import`, `run_import`, `import_status`) ; migration
  `import_chunk` après celle de la tâche 20 ; tâche `import` du planificateur de la tâche
  20

## P6 — Contrat, bout en bout, budgets

- [ ] 22. Contrat OpenAPI 3.1 (utoipa, `x-query` construit à la main), `GET
  /v1/openapi.json`, `GET /v1/capabilities`, contrat de référence
  `specs/006-api-rest/openapi-v1.json` et comparateur — REQ-006-4, 33, 34, 35 — verify :
  toutes les opérations de REQ-006-4 présentes ; un champ retiré fait échouer le test ;
  `Deprecation` et `Sunset` sur une route de test retirée
  où : design « Versions et contrat » ; dépendances `utoipa`/`utoipa-axum` (design «
  Crates et dépendances ») ; routes existantes de `rest/src/*.rs` à annoter ; binaire
  `openapi` de `kartenn-rest`, à créer
- [ ] 23. Matrice des erreurs : un test par ligne du tableau du design — REQ-006-20 à
  23 — verify : statut, `type`, `Request-Id`, aucune valeur sensible
  où : design « Erreurs » (tableau) et « Erreurs et cas limites » ;
  `rest/src/problem.rs`, `rest/src/error.rs` ; un test par ligne, à la manière de
  `tests/socle.rs`
- [ ] 24. Vrai TCP et TLS : `QUERY` en HTTP/1.1 et h2, cookies et `Forwarded` de bout en
  bout — REQ-006-10, 27, 32 — verify : tests d'intégration sur un port local
  où : design « Transport » ; `rest/src/transport.rs`, `kartenn-server/src/http.rs` ;
  tests à étendre `tests/transport.rs`, `tests/sessions.rs`, `tests/csrf.rs`
- [ ] 25. Banc `bench-http` : scénarios de `bench-core` à travers HTTP, charge de
  200 utilisateurs et 50 écritures/s — REQ-006-39 — verify : p95 sous INV-30 à 34 et
  INV-36, surcoût rapporté par rapport au banc du service
  où : design « Stratégie de test » (ligne 39) ; `kartenn-bench/src/core_budgets.rs`
  (scénarios à rejouer), `main.rs` (sous-commandes) ; agent `bench-runner`
- [ ] 26. Synchronisation de la spec système : `docs/spec/interfaces.md` (section API
  REST, Q-ouv-7 close), `invariants.md`, ADR 0022 et 0023 acceptés, `specs/README.md`
  — verify : `spec-keeper` sur `specs/006-api-rest`
  où : `docs/spec/interfaces.md`, `docs/spec/invariants.md`, `docs/adr/0022-*`,
  `docs/adr/0023-*`, `specs/README.md` ; agent `spec-keeper`

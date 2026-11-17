---
status: implemented
updated: 2026-10-19
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
- [x] 19. Audit et exploitation : recherche et export (`GET`, `QUERY`),
  pseudonymisation, filtres de log à chaud — REQ-006-4 — verify : consultation sans
  permission refusée et auditée ; filtre qui revient à la configuration
  où : design « Ressources et routes » (ligne Audit, Exploitation) ; service
  `reads::read_audit`, `observability::set_log_filter` ; store `audit.rs` (`search`,
  `pseudonymise`) ; `QUERY` comme `rest/src/filter.rs` ; test à imiter `tests/filter.rs`
  — Arbitrage au design (« Audit et exploitation », 2026-10-19) : curseur sur `seq`,
  export NDJSON par `Accept`, pseudonymisation immédiate sous
  `core.identity.pseudonymise`, filtres `object`, `from`, `to`, `retroactive` sur
  `GET /v1/changesets` (REQ-004-32). Core : permission `core.identity.pseudonymise`,
  détail `motive` de `person.identity.pseudonymised` (catalogues régénérés). Service :
  `AuditQuery.before`, `Reader::audit_extent`, `read_audit` (page et suite),
  `export_audit` (audité une fois, borné à la sélection), `reads::pseudonymisable`.
  Store : `read.rs::audit_criteria`, `audit.rs` (`pseudonymise_identity`, `written_by`
  partagé avec `changesets::page`). Telemetry : `capture::valid_filter`. Rest :
  `audit.rs` (journal par pages ou `QUERY`, export NDJSON en flux, pseudonymisation,
  `PUT /v1/log-filters/{output}`), `Api.outputs`, problème 422 `invalid-override`,
  critères de `cycle.rs::filter`. `tests/audit.rs`
  (`a_consultation_without_permission_is_refused_and_audited`,
  `the_journal_reads_by_pages_and_by_query`, `an_export_is_whole_and_audited_once`,
  `a_person_is_pseudonymised_at_once`, `a_log_filter_returns_to_the_configuration`),
  `tests/cycle.rs` (`the_list_searches_by_object_period_and_retroactivity`)
  choix : sortie inconnue → 404 avant le contrôle de permission (les noms de sortie ne
  sont pas sensibles) ; compte de secours et identité système → 409 comme un compte de
  service ; refus de l'export audité sur la même cible que la consultation ; export en
  `Cache-Control: no-store`, lots internes de 1 000 ; `futures-util` passe des
  dépendances de test aux dépendances (déjà dans l'arbre)

## P5 — Tâches de fond et imports

- [x] 20. Migration `job`, planificateur sous bail (scrutation 5 s, bail 30 s, battement
  10 s), tâches `expire-pending`, `reevaluate-expected`, `public-drift`,
  `idempotency-purge` ; minuteries locales (caches, limiteurs, JWKS) — REQ-006-38 —
  verify : trois planificateurs sur une base, une exécution par échéance ; détenteur
  arrêté → reprise après le bail
  où : design « Imports et tâches de fond » ; `review::expire_pending`,
  `expected::reevaluate_outdated`, store `registry::public_drift` ; migration suivante
  `0039_job.sql` ; boucle lancée depuis `kartenn-server/src/app.rs`
  — Migration `0039_job.sql` (table et quatre tâches semées). Store : `jobs.rs` (`claim`
  par `FOR UPDATE SKIP LOCKED`, `heartbeat`, `finish`, `due_now`), `registry.rs`
  (`rewrite_public` par changesets système sous les verrous des domaines et de l'espace
  public ; `public_drift` borné aux versions valides maintenant ou plus tard, une ligne
  par objet, `PublicDrift.domain`). Server : `jobs.rs` (`Scheduler`, `Runner`, `Tasks`),
  `app.rs` (planificateur en tâche à part, arrêté avant l'API ; purge des états et des
  échecs d'authentification à la minute ; rechargement des émetteurs toutes les 5 min).
  Auth : `TokenVerifier::reload`, `Oidc::reload`, `FailureLimiter::purge` ; service
  `StateCache::purge` ; rest `Authn::purge`, `Authn::reload`. Tests `tests/jobs.rs`
  (`three_schedulers_run_each_due_task_once`, `a_stopped_holder_is_relieved_after_its_lease`,
  `the_tasks_of_kartenn_run`), `special_registry.rs`
  (`drifting_flags_are_rewritten_by_system_changesets`), `secrets.rs`
  (`the_purge_forgets_the_windows_over`)
  choix : `public-drift` rendu dû à chaque démarrage d'instance (donc à celui d'une
  version ; sans écart, aucun changeset) ; tâche en échec replanifiée une période plus
  tard (journalisée) ; bail perdu → tâche abandonnée, sans replanification ; tâche en
  cours abandonnée à l'arrêt (transaction annulée, bail laissé à expirer) ; lots de 1 000
  objets par changeset pour la réévaluation et les drapeaux ; un émetteur muet au
  rechargement garde ses clés et sa découverte (WARN `authn.issuer.reload_failed`)
- [x] 21. Imports : migration `import_chunk` et état `uploading`, `PUT` des morceaux
  (identique ignoré, différent → 409, > 10 000 → 413), `start` (202), `resume`,
  `cancel`, état et `Retry-After`, exécution comme tâche — REQ-006-17, 18, 19 — verify :
  10⁶ éléments en 100 morceaux dont un renvoyé ; arrêt d'une instance pendant
  l'exécution, fin sur une autre sans doublon
  où : design « Imports et tâches de fond », « Transport » (limites des morceaux) ;
  service `import.rs` (`create_import`, `run_import`, `import_status`) ; migration
  `import_chunk` après celle de la tâche 20 ; tâche `import` du planificateur de la tâche
  20
  — Migration `0040_import_chunk.sql` (`queued` → `ready`, `uploading`, table
  `import_chunk` avec `size`, droits de `kartenn_app` sur `import_chunk` et `job`).
  Service : `import.rs` (`open_import`, `batches` extrait de `create_import`,
  `run_import` limité à `ready`/`running` et relisant l'état avant chaque lot,
  `import_status` par `Store::import_progress`) ; port `set_import` gardé par l'état de
  départ. Store : `imports.rs` (`header`, `deposit`, `start` en une transaction sous
  `FOR UPDATE`, `transition`, `unschedule`, tâche `import:<id>`). Rest : `imports.rs`
  (six routes, auteur seul, poids du morceau au quota), `ProblemKind::InvalidImport`.
  Server : `Tasks::import`. Tests `rest/tests/imports.rs`
  (`a_million_elements_in_a_hundred_chunks`, `an_import_is_its_authors`,
  `the_state_guards_the_actions`, `a_chunk_weighs_its_thousands_on_the_quota`),
  `server/tests/jobs.rs` (`an_import_ends_on_another_instance_without_duplicates`),
  `service/tests/import.rs` (`a_cancelled_import_stops_after_the_batch_in_progress`)
  choix : colonne `size` du morceau (décompte sans relire le JSON) ; empreinte sur le
  tableau `elements` resérialisé ; morceaux gardés après le démarrage ; tâche d'import
  de période 10 s (rejouée ainsi après un échec d'infrastructure), retirée par le
  planificateur quand l'import est fini, en pause ou annulé ; réponse d'un dépôt = la
  représentation de l'import ; `source` vide → 400

## P6 — Contrat, bout en bout, budgets

- [x] 22. Contrat OpenAPI 3.1 (utoipa, `x-query` construit à la main), `GET
  /v1/openapi.json`, `GET /v1/capabilities`, contrat de référence
  `specs/006-api-rest/openapi-v1.json` et comparateur — REQ-006-4, 33, 34, 35 — verify :
  toutes les opérations de REQ-006-4 présentes ; un champ retiré fait échouer le test ;
  `Deprecation` et `Sunset` sur une route de test retirée
  où : design « Versions et contrat » ; dépendances `utoipa`/`utoipa-axum` (design «
  Crates et dépendances ») ; routes existantes de `rest/src/*.rs` à annoter ; binaire
  `openapi` de `kartenn-rest`, à créer
  — Dépendances `utoipa` 6 (`macros`, `uuid`) et `utoipa-axum` 0.3, workspace-hack
  régénéré. Rest : `openapi.rs` (`document`, `json`, `finish` : problème par défaut,
  schémas de sécurité, routes publiques ; `queryable` → `x-query` ; `op`, `documented`,
  `listing_of` pour les routes par collection ; miroirs `enums`), `operation.rs`
  (`OperationDto`, `ImportItemDto`), `contract.rs` (`breaches`), `capabilities.rs`,
  `versions.rs` (`Retirement`, `retire`), binaire `openapi` ; routes de chaque module en
  `OpenApiRouter` annotées, réponses `json!` remplacées par des DTO typés
  (`cursor::Listing`, `changesets::ChangesetApplied`…). Store :
  `IDEMPOTENCY_RETENTION_HOURS`. Contrat de référence `openapi-v1.json` (arbitrage du
  design « Schémas du contrat »). Tests `rest/tests/contract.rs`
  (`the_contract_keeps_its_reference`, `every_operation_of_the_surface_is_in_the_contract`,
  `a_member_removed_fails_the_comparison`,
  `the_contract_is_openapi_3_1_its_query_in_an_extension`,
  `a_retired_route_announces_its_successor`,
  `the_contract_and_the_capabilities_are_served`), unitaires `operation`
  (`each_operation_reads_alike_through_the_mirror`), `contract`, `resources`
  (`the_contract_of_a_patch_names_what_it_writes`), `openapi`
  (`the_mirrors_serialise_as_the_core`)
  choix : modules des capacités = étiquettes du contrat ; plugins UI `{name, version}`,
  liste vide ; propriétés déclarées `{key, type}` ; événements d'audit en objet libre
  (format de la 012) ; corps des écritures par ressource décrits par composition
  `Reserved` + membres (`allOf`), leur analyse membre à membre inchangée ; corps des
  écritures des droits lus par structures (`deny_unknown_fields`, membre inconnu
  toujours 400, requis toujours 422) ; schémas nommés par `#[schema(as)]` (`Submission`,
  `TreeNode`, `CoreObject`…) ; réponse `default` `Problem` sur toute opération ;
  `OpenAPI` au vocabulaire de clippy
- [x] 23. Matrice des erreurs : un test par ligne du tableau du design — REQ-006-20 à
  23 — verify : statut, `type`, `Request-Id`, aucune valeur sensible
  où : design « Erreurs » (tableau) et « Erreurs et cas limites » ;
  `rest/src/problem.rs`, `rest/src/error.rs` ; un test par ligne, à la manière de
  `tests/socle.rs`
  Tests `rest/tests/errors.rs` : `line_400_a_malformed_request` à
  `line_500_a_panic` (une par ligne du tableau, à travers le routeur complet, statut,
  `type`, `title`, `Request-Id` en en-tête et en `instance`, ni valeur sensible, ni jeton,
  ni requête, ni message de panique dans le corps),
  `the_first_problem_in_the_order_of_http_wins` ; design « Erreurs » et REQ-006-23
  amendés (arbitrage : un droit d'écriture manquant est la violation
  `core.object.forbidden`, 422 ; 403 aux actions hors changeset)
  choix : authentificateur de test par jeton fixe (`Who`) ; journal d'audit coupé par un
  déclencheur sur `audit.event` ; valeur sensible posée en SQL comme dans `objects.rs`
- [x] 24. Vrai TCP et TLS : `QUERY` en HTTP/1.1 et h2, cookies et `Forwarded` de bout en
  bout — REQ-006-10, 27, 32 — verify : tests d'intégration sur un port local
  où : design « Transport » ; `rest/src/transport.rs`, `kartenn-server/src/http.rs` ;
  tests à étendre `tests/transport.rs`, `tests/sessions.rs`, `tests/csrf.rs`
  Module de test `rest/tests/wire` (routeur servi par `kartenn_server::http::serve`, en
  clair ou en TLS auto-signé, client HTTP/1.1 ou h2 par ALPN, h2c en clair). Tests
  `rest/tests/transport.rs` (`query_crosses_a_local_port_in_http1_and_h2`,
  `forwarded_crosses_a_local_port`), `sessions.rs`
  (`a_login_crosses_a_local_port_over_tls`, `the_cookies_follow_forwarded_on_a_local_port`),
  `csrf.rs` (`the_check_holds_on_a_local_port`)
  choix : `kartenn-server` en dépendance de développement de `kartenn-rest` (cycle de
  dev-dépendance admis par cargo) pour servir par le vrai `serve` ; `hyper` client,
  `hyper-util`, `tokio-rustls`, `rcgen` en dépendances de développement (déjà dans
  l'espace de travail) ; le proxy de confiance des tests est le bouclage `127.0.0.0/8`
- [x] 25. Banc `bench-http` : scénarios de `bench-core` à travers HTTP, charge de
  200 utilisateurs et 50 écritures/s — REQ-006-39 — verify : p95 sous INV-30 à 34 et
  INV-36, surcoût rapporté par rapport au banc du service
  où : design « Stratégie de test » (ligne 39) ; `kartenn-bench/src/core_budgets.rs`
  (scénarios à rejouer), `main.rs` (sous-commandes) ; agent `bench-runner`
  `kartenn-bench/src/http.rs` (sous-commande `http`, script devenv `bench-http`) : routeur
  servi par `kartenn_server::http::serve`, authentification réelle (jeton ES256 pour la
  moitié des 200 personnes, cookie de session pour l'autre), personne de `bench-rbac`.
  Référence (graine 42) : objet 9,5 ms (+2,3), passé 9,8 ms, page filtrée 52 ms (+9,5),
  sous-arbre 39 ms (+24), au passé 49 ms (+24), changeset 18 ms ; charge : 51,5
  écritures/s, aucun 5xx hors délestage (168 délestées sur 7 527). Le banc a révélé une
  page filtrée à 3 s par HTTP : `kartenn-store-pg/src/filter.rs` (`filter_sql`) réunit
  type, valeurs et portée en une sous-requête scalaire (52 ms)
  choix : HTTP/1.1 en clair derrière le bouclage en proxy de confiance (`Forwarded`
  `proto=https`) ; référence du surcoût = le service avec le contrôle d'accès de la
  même personne, mesures alternées ; sous-arbre = deux requêtes (`children`, page de 100,
  puis `occupancy`), d'où un surcoût au-dessus de la cible de 10 ms ; charge de 30 s,
  lecteurs avec une pause de 0,5 à 1,5 s, 55 rédacteurs à 1/s pour en tenir 50 ;
  reconnexion du client hors mesure quand hyper ferme une connexion inactive (30 s)
- [x] 26. Synchronisation de la spec système : `docs/spec/interfaces.md` (section API
  REST, Q-ouv-7 close), `invariants.md`, ADR 0022 et 0023 acceptés, `specs/README.md`
  — verify : `spec-keeper` sur `specs/006-api-rest`
  où : `docs/spec/interfaces.md`, `docs/spec/invariants.md`, `docs/adr/0022-*`,
  `docs/adr/0023-*`, `specs/README.md` ; agent `spec-keeper`
  `docs/spec/interfaces.md` (API REST, couche HTTP de l'authentification, configuration
  `[http]`, sondes, tâches de fond), `invariants.md` (INV-29 nouveau, tests d'INV-20,
  25, 40, 41, banc `bench-http`), `architecture.md`, `README.md` ; ADR 0022 et 0023
  acceptées ; Q-ouv-5 et Q-ouv-7 closes dans `001-cadrage/decisions.md` ; reports de la
  002 (`public-drift`) et de la 004 (worker d'import) notés faits ; questions ouvertes
  de la 006 cochées (tranchées dans le design). `spec-keeper` : chaque REQ-006 a son
  code, aucun écart bloquant ; la `source` des CloudEvents reste `urn:kartenn:instance`
  (noté dans `specs/README.md`)
  choix : le report de la `source` CloudEvents (004 → 006) est noté comme rejoignant le
  transport des événements sortants plutôt que tranché ici ; Q-ouv-5, déjà close par
  005/K2, barrée en même temps que Q-ouv-7

## P7 — Amendements

- [x] 27. Tri d'un import en tâche de fond (H18) : état `sorting`, colonne `error`, `start`
  sans tri (202, efface `error`, pose `import:<id>`), la tâche trie puis passe à `ready`
  ou revient à `uploading` avec `error` ; `cancel` admis en `sorting` ; `Retry-After` en
  `sorting` ; contrat OpenAPI, SDK et client web (état et erreur affichés) — REQ-006-17,
  19 — verify : `start` de 10⁶ éléments répond 202 sous le délai ; import à cycle →
  202, puis `uploading` et `error.type` `invalid-import` ; instance arrêtée pendant le
  tri, reprise par une autre ; `docs/spec/interfaces.md` à jour
  où : design « Imports et tâches de fond » (puces `start`, états, `GET`) ; migration
  après la dernière de `crates/kartenn-store-pg/migrations/` (`import.state`, colonne
  `error jsonb`) ; store `imports.rs` (`start`, `transition`) ; service `import.rs`
  (`create_import`, `run_import`, `import_status`) ; rest `imports.rs` (`start`,
  représentation, `ProblemKind::InvalidImport`) ; tâche `Tasks::import` du serveur ;
  tests `rest/tests/it/imports.rs` (le test du million et celui qui fixe le 503 actuel
  sont à réécrire), `server/tests/it/jobs.rs` ; écran d'import web s'il existe
  — Migration `0057_import_sorting.sql` (état `sorting`, colonne `error jsonb`). Store :
  `imports.rs` (`start` : `uploading` → `sorting`, `error` effacée, tâche posée ;
  `sorting` lit les morceaux ; `sorted` écrit les lots puis passe à `ready` en une
  transaction, rien si l'import n'est plus `sorting` ; `unsorted` : retour à
  `uploading` avec `error`, tâche retirée ; `ImportHeader.error`). Rest : `imports.rs`
  (`start` sans tri, `sort_error`, `error` dans la représentation, `Retry-After` et
  `cancel` en `sorting`). Server : `Tasks::sort` avant `run_import`, tri par
  `spawn_blocking`. Contrat `openapi-v1.json` et `web/sdk` régénérés ; web
  `ImportView.vue` (relecture en `sorting`, section « Tri impossible » et violations),
  `admin.ts` (`IMPORT_BUSY`, `ImportError`), i18n `fr`/`en`. Tests
  `rest/tests/it/imports.rs` (`a_million_elements_in_a_hundred_chunks` : 202 en moins de
  5 s au délai par défaut, `the_state_guards_the_actions` : 202 puis `uploading` et
  `error`, `a_sorting_import_cancels_and_its_sort_records_nothing` ; le test du 503
  retiré), `server/tests/it/jobs.rs` (`a_sort_stopped_is_taken_over_by_another_instance`,
  `an_unsorted_import_is_open_again_with_its_error`), parcours e2e `admin.spec.ts`
  (`an import whose sort fails is open again with its error`) ; `docs/spec/interfaces.md`,
  `data.md`
  choix : orchestration du tri dans la tâche du serveur et le store (pas de méthode de
  port nouvelle) ; tri hors des fils du runtime (`spawn_blocking`) pour que le battement
  tienne le bail ; lots insérés avant le passage à `ready` (une annulation n'attend que
  le commit) ; `error` = problème RFC 9457 complet (`type`, `title`, `status`,
  `detail`) avec une violation par erreur, règles `import.missing-external-id`,
  `import.unknown-reference`, `import.order`, membre `element` = rang de l'élément dans
  les morceaux lus par `n` croissant ; un tri en échec d'infrastructure laisse l'import
  en `sorting`, rejoué une période plus tard ; parcours d'import e2e en `test.slow()`,
  attente de la tâche portée à 60 s (une tâche due par scrutation de 5 s, après celles du
  démarrage)
- [x] 28. ETag composite d'un objet (H19) : `"<version>.<empreinte du corps>"` pour toute
  lecture d'un objet du noyau et des modules, `If-Match` sur la version seule (forme nue
  acceptée) ; descriptions OpenAPI, contrat et SDK régénérés — REQ-006-14 — verify :
  parent renommé → `If-None-Match` 200, `PATCH` sous l'ancien ETag 200, version périmée
  412 ; fiche d'une machine revalidée après le renommage d'une interface → 200
  où : `rest/src/dated.rs` (`ETag::record`, `ETag::own_version`, `respond_object`),
  `rest/src/kit.rs` (`if_match`, `OBJECT_READ`, `IF_MATCH_OBJECT`),
  `rest/src/resources.rs` (`if_match`), `rest/src/objects.rs` (`object`), `show`/`answer`
  de `vlan`, `machines`, `policy`, `flow` ; tests `rest/tests/it/resources.rs`
  (`renaming_the_parent_changes_the_etag_of_the_child`,
  `if_match_compares_the_own_version_only`), `machines/tests/it/rest.rs`
  (`a_renamed_interface_changes_the_etag_of_the_card`,
  `a_new_interface_changes_the_etag_of_the_card`)
  choix : empreinte du corps plutôt que maximum des versions des liés (couvre héritage,
  visibilité et collections, aucune requête) ; forme nue gardée sans échéance ; banc non
  relancé (un SHA-256 de quelques centaines d'octets par lecture, sérialisation
  inchangée) ; l'ETag d'un changeset (son état) n'est pas concerné : sa lecture ne
  répond jamais 304
- [x] 29. La version d'un objet dans son corps (H21) : membre `version` (part propre de
  l'ETag) dans toute lecture par identifiant et dans les pages des collections d'objets,
  noyau et modules, déclaré au contrat par le kit ; contrat, `web/sdk`, `web/v2/sdk`
  régénérés — REQ-006-14 — verify : `version` = part propre de l'ETag = changeset de la
  dernière écriture, le même dans la page ; `PATCH` qui l'écrit → 422
  où : `rest/src/dated.rs` (`VERSION`, `with_version`, `respond_object`),
  `rest/src/kit.rs` (`versioned`), `store-pg/src/read.rs` (`versions`),
  `rest/src/openapi.rs` (`object_versions`), `rest/src/resources.rs` (`read_only`),
  listes de `objects.rs`, `vlan`, `machines`, `flow`, `policy` ; test
  `rest/tests/it/resources.rs` (`the_body_states_the_own_version`)
  choix : membre non obligatoire au schéma (le même schéma montre l'objet sans version
  dans l'historique et les explications) ; un flux en revue d'une page ne porte pas de
  version (il montre l'état proposé) ; une requête de plus par page (`object_version` par
  clé primaire au présent) ; banc non relancé
- [x] 30. Clé d'idempotence libérée dès l'échec d'une écriture (REQ-006-15 ; relevé par la
  revue de la 012-23, à vérifier) : si une écriture échoue pendant que sa clé est tenue
  par `pg_try_advisory_xact_lock` (`replay`, `crates/kartenn-store-pg/src/write.rs` vers
  la ligne 1187) et que la transaction est lâchée sans annulation explicite, sqlx ne fait
  que mettre le `ROLLBACK` en file : un client qui rejoue aussitôt la même clé peut lire
  un 409 « en cours » à tort. `pipeline.rs` annule explicitement sur certaines branches
  (vers les lignes 2024 à 2049) ; vérifier toutes les sorties en erreur après la prise de
  la clé, annuler explicitement là où il le faut, et le prouver — verify :
  `k-check -p kartenn-store-pg -p kartenn-rest` (un rejeu immédiat après une écriture en
  erreur n'est pas un 409, sous un pool dont `after_release` retient les connexions
  rendues)
  - où : `crates/kartenn-store-pg/src/{write,pipeline}.rs` ; imiter `audit_relay.rs`
    (`relay_once`, 012-23) et `crates/kartenn-telemetry/tests/it/audit_export.rs`
    (`a_pass_right_after_a_rejection_relays`)
  - dépend de : —
  constat : défaut réel, pour les seules erreurs levées hors de la base après la prise de
  la clé (décodage, modules, construction de l'audit) ; une instruction en échec abandonne
  la transaction dans PostgreSQL et libère ses verrous aussitôt (vérifié avec `pg_locks`).
  Les sorties de `stages` (`kartenn-service/src/pipeline.rs`, et non `store-pg`) sans
  annulation passent par `or_roll_back!` (`rolled_back`) : `replay`, `lock_changeset`,
  périodes, `cascade::expand`, `lock_targets`, `lock`, `lock_keys`,
  `approval_policies`, `holding_policy`, `authorize_in`, `changeset_events`,
  `journal_applied` ; test `rest/tests/it/errors.rs`
  (`a_replay_right_after_a_failed_write_is_not_in_progress` : 409 avant le correctif,
  201 après)
  choix : erreur provoquée par une politique d'approbation indécodable (un `NULL` dans
  `property_keys`), lue après la prise de la clé ; macro locale plutôt qu'un découpage de
  `stages` (diff minimal, les branches déjà annulées restent telles quelles) ; échec
  d'annulation journalisé en `warn`, l'erreur d'origine rendue ; les sorties propres à la
  validation (`lock_changeset`, `authorize_in`) couvertes aussi (même verrou tardif, sur
  la ligne du changeset) ; `commit` laissé tel quel (un `COMMIT` en échec clôt la
  transaction)
  reste (revue) : deux conduites sur un échec d'annulation dans `stages` (`rolled_back`
  le journalise et rend l'erreur d'origine, les `tx.rollback().await?` voisins rendent
  l'erreur d'annulation) ; rien n'empêche un `?` ajouté plus tard après `begin` de
  réintroduire le défaut (une ligne au commentaire de tête de `stages` le dirait) ;
  l'attente `while slow.num_idle() < size` du test n'a pas de borne ; le test s'insère
  entre `line_409_…` et `line_412_…` de `errors.rs` au lieu de suivre leur suite ; le
  pool qui retient les connexions rendues est copié une troisième fois (012-23, 012-24),
  à mettre en commun dans `kartenn-testing` ; hors du pipeline, `expected.rs`,
  `module.rs`, `declarations.rs` et `rbac.rs` lâchent aussi leurs transactions par `?`,
  sans clé d'idempotence ni verrou consultatif non bloquant à libérer

## Lectures par fenêtre (H23, amendement du 2026-11-14)

- [x] 31. Propriétés résolues par lots : `GET /v1/<collection>/properties` (`id` répété,
  200 au plus ; `key` répété ; `valid_at`, `as_of`), servi par `reads::resolved_many` ;
  identifiant inconnu, d'une autre collection ou illisible omis ; un événement d'audit
  pour le lot ; poids au quota d'une unité par centaine d'identifiants entamée ; route
  au contrat OpenAPI et types du SDK régénérés — REQ-006-40 — verify :
  `k-check -p kartenn-rest` (chaque élément égal à la lecture unitaire, aux mêmes dates,
  valeur sensible et masquage compris ; illisible omis sans différence avec inconnu ;
  201 identifiants → 400 ; un seul événement d'audit ; poids au quota) ; banc HTTP
  réduit, 200 objets × 6 clés sous le budget d'une page
  - où : `crates/kartenn-rest/src/properties.rs` (lecture unitaire, vers les lignes
    452-510, à imiter), `crates/kartenn-service/src/reads.rs` (`resolved_many`, vers la
    ligne 232), `crates/kartenn-rest/src/quota.rs` (poids d'une requête), contrat
    OpenAPI et `web/v2/sdk/src/api/schema.d.ts` ; `kartenn-bench` (`props.rs`, banc HTTP)
  - dépend de : —
  fait : `crates/kartenn-rest/src/properties.rs` (`batch_route` dans `object_routes`, donc
  pour toute collection du noyau et des modules ; `batch_query`, `resolved_batch`, `batch`,
  `items_of`), `quota.rs` (`ids_weight`), `openapi.rs` (`Op::repeated`) ;
  `kartenn-service/src/reads.rs` (`resolved_many` masque une propriété sensible qui
  s'applique même en « sans valeur », comme `read_object`) ; contrat et types des deux
  SDK régénérés (`schema.d.ts`, `dated.ts`, par `scripts/gen.mjs`) ; banc HTTP
  `http.resolved-batch` (200 adresses × 6 clés, budget 150 ms d'une page filtrée,
  cinq clés `custom.b1…b5` ajoutées à la préparation ; échelle réduite, 2026-11-14,
  charge 6,7 au départ et ≈ 20 pendant : p95 96 ms par HTTP sous 150 ms, 47 ms par le
  service, surcoût 49 ms au-delà de la cible de 10 ms, à remesurer en pleine échelle sur
  une machine calme ; pistes si le surcoût tient : la seconde autorisation de
  `reads::resolved_many` sur les objets déjà autorisés, `kind_names` à part, la
  compression d'un corps de 200 éléments). Tests `rest/tests/it/objects.rs` :
  `a_batch_reads_each_object_as_its_own_read` (deux appelants, trois dates, toutes les
  clés et un sous-ensemble : chaque élément égal à la lecture unitaire sans ses liens ;
  héritage d'un porteur invisible, sensible masqué, « sans valeur » sensible ; pas
  d'ETag), `a_batch_leaves_out_alike_the_unknown_the_foreign_and_the_unreadable` (statut,
  en-têtes hors `request-id` et octets identiques ; un `data.objects.read` par lot, mêmes
  critères ; aucun refus journalisé), `a_batch_names_its_parameter_and_weighs_per_hundred`
  (0 et 201 `id` → 400, 200 → 200 ; 100 `id` passent une rafale d'un, 101 non, 200 une
  rafale de deux) ; `contract.rs` (route à la surface) ; `quota.rs` (poids).
  choix : élément `{id, properties, sources, masked}` (H23), les `links` de la lecture
  unitaire omis ; ordre des éléments celui des `id`, doublons d'`id` ou de `key` lus une
  fois ; `key` inconnue ignorée comme une clé non déclarée ; paramètre inconnu → 400 qui
  le nomme, comme les listes ; borne de 200 et poids comptés sur les `id` donnés,
  doublons compris ; `Cache-Control` de la lecture unitaire, sans `ETag` ; opération
  `<kind>_properties_batch` ; un seul `data.objects.read` (cible `object`, critères
  `collection`, `keys`, `valid_at`, `as_of`, sans les `id` demandés) plus un
  `data.sensitive.read` par valeur sensible montrée, comme la lecture unitaire ; un objet
  illisible ne journalise pas de refus (005 : invisible égale absent) ; le chemin statique
  `/<collection>/properties` passe avant `/<collection>/{id}` (matchit), vérifié par le
  test.
- [x] 32. Filtre `id` répété sur toute collection, `address` répété sur la liste des
  interfaces ; déclarés au contrat — REQ-006-41. Découpée en 32a–32d (2026-11-14) : une
  trentaine de collections, onze listes à SQL propre dans sept crates. Commun aux quatre :
  borne `cursor::MAX_SIZE` (1000), refus dès le 1001ᵉ `id` lu ; poids au quota 1, la
  taille de page bornant déjà la lecture ; illisible = inconnu = absent de la page ;
  contrat et types des deux SDK régénérés (`cargo run -p kartenn-server -- openapi`,
  `scripts/gen.mjs`).
- [x] 32a. Brique commune, listes du noyau et des VLAN : `filter::ids` (retire `id` des
  paramètres comme `filter::prefix` ; UUID mal formé ou au-delà de la borne → 400 qui
  nomme `id`), `Op::ids` (sur `Op::repeated`) ; six listes du noyau (`Criteria.ids` sur
  la lecture par clé naturelle, candidats du moteur sur la lecture par propriétés) ; une
  page sans suite (REQ-006-41) ; groupes de
  VLAN et VLAN — verify : `k-check -p kartenn-rest` (deux identifiants dont un
  illisible : un seul rendu, page identique à celle d'un inconnu ; conjonction avec
  `domain` ; 1001 `id` → 400, 1000 → 200 ; `id` aux `parameters` de ces listes)
  - où : `crates/kartenn-rest/src/filter.rs` (`prefix`, à imiter), `openapi.rs`
    (`Op::repeated`), `objects.rs` (`declare_filters`, `wanted`, `list` vers les lignes
    1053-1130), `vlan.rs` (`list_groups`, `list_vlans`, vers les lignes 409 et 478),
    `crates/kartenn-store-pg/src/list.rs` (`Criteria.ids`, déjà en conjonction) ; tests
    `rest/tests/it/objects.rs` (lots de la 31, à imiter)
  - dépend de : —
  fait : `filter.rs` (`ids` : retire `id` des paramètres, dédoublonne et trie ;
  `push_id` refuse dès le 1001ᵉ ; `window` : filtrée par `id`, une liste lit autant de
  lignes que d'identifiants et n'offre pas de suite, `page_size` ou un curseur avec `id` →
  400 qui le nomme), `cursor.rs` (`Page::sized`), `openapi.rs` (`Op::ids`, tableau d'UUID
  facultatif), `objects.rs` (`id` déclaré en tête des filtres de chaque liste du noyau ;
  `Criteria.ids` sur la lecture par clé naturelle, la conjonction étant celle du magasin ;
  sur la lecture par propriétés, les candidats du moteur sont les `id` demandés),
  `kartenn-store-pg/src/filter.rs` (`filter_all_in` prend `ids`, un prédicat
  `id = ANY(…)` de plus après les conditions), `vlan.rs` (`filters` lit `id` dans le
  même parcours ; `cardinality(…) = 0 OR x.id = ANY(…)` dans les deux listes, sous le
  prédicat de lecture ; `window`) ; contrat et types des deux SDK régénérés
  (`schema.d.ts`, `filters.ts`).
  Côté web, conséquence du contrat : `id` tenu hors de la barre de filtres du shell
  (`views/filters.ts`, `LEFT_OUT`, comme `q`) et attentes de noms mises à jour
  (`sdk/test/client.test.ts` des deux SDK, `web/v2/sdk/test/filters.test.ts`). Tests :
  `rest/tests/it/objects.rs` `the_id_filter_leaves_out_alike_the_unknown_the_foreign_and_the_unreadable`
  (statut, en-têtes hors `request-id` et octets identiques ; événement de lecture limité à
  l'objet lu), `the_id_filter_goes_in_conjunction_with_the_others` (clé naturelle,
  domaine, `q`, propriétés ; ordre de la liste),
  `the_id_filter_answers_in_one_page_without_a_next` (60 objets, au-delà de la page par
  défaut, sur les deux lectures : une page, ni `links.next` ni `Link` ; `page_size` ou un
  curseur avec `id` → 400 qui le nomme),
  `the_id_filter_names_its_parameter_past_the_largest_page` (`QUERY` : 1000 → 200, 1001
  → 400 ; UUID mal formé ou vide → 400 qui nomme `id`) ; `vlan/rest.rs`
  `groups_and_vlans_filtered_by_identifier` (VLAN et groupe illisibles : réponse
  identique à celle d'un inconnu, en-têtes compris ; une page ; `page_size` → 400) ;
  `contract.rs` `the_lists_of_the_core_declare_the_id_filter`.
  choix : borne `cursor::MAX_SIZE` (1000), comptée sur les `id` lus, doublons compris ;
  poids au quota 1 (la taille de page borne la lecture) ; doublons lus une fois ; une
  liste filtrée par `id` répond en une page, sans suite (arbitrage de la revue), donc
  aucun curseur ne porte d'`id` ; les critères de l'événement de lecture gardent les `id`
  demandés parmi les filtres, comme tout filtre d'une liste : c'est ce que l'appelant a
  demandé (le lot de la 31 les omet de ses critères) ; `id` reste dans `FILTERS` du SDK
  (l'URL d'une liste qui le porte n'est pas rejetée) mais hors de la barre.
  reste : 1000 `id` ne tiennent pas dans les 8 Kio d'une URL (≈ 200 au plus) : au-delà,
  `QUERY` ; `prefetchFilters` (`web/v2/shell/src/views/routes.ts`, vers la ligne 66) lit
  le modèle du SDK, sans `LEFT_OUT` : une URL portant `id` (ou `q`) serait préchargée
  sous une autre clé que celle que la vue lit (le shell n'écrit pas `id` dans une URL).
- [x] 32b. Listes des machines : `id` sur les grappes, machines et interfaces ; `address`
  répété sur les interfaces (un parmi) — verify : `k-check -p kartenn-machines` (deux
  identifiants dont un illisible : un seul rendu ; conjonction ; deux adresses : leurs
  deux interfaces ; filtrée par `id`, une page sans suite, `page_size` ou curseur → 400,
  par `filter::window`)
  - où : `crates/kartenn-machines/src/rest.rs` (`list_clusters`, `list_machines`,
    `list_interfaces` vers les lignes 644, 887, 1071 ; `optional_uuid` de `address` →
    `uuid[]`)
  - dépend de : 32a
  fait : `crates/kartenn-machines/src/rest.rs` (`filters` lit `id` par
  `filter::push_id` (1000 au plus) et `address` répété (`MAX_ADDRESSES`, 64 au plus)
  dans le même parcours, chacun dédoublonné, rendus dans une struct `Wanted` ;
  `filter::window` pour `id`, `address` paginé comme tout filtre, son curseur portant les
  adresses ; `cardinality($n::uuid[]) = 0 OR x.id = ANY($n)` sous le prédicat de lecture
  dans les trois listes, `a.address_id = ANY($7)` dans la jointure des rattachements ;
  `MachineFilter.ids`, vide pour les machines hébergées et celles d'une grappe ; contrat :
  `Op::ids` en tête de chaque liste, `address` par `Op::repeated`, tableau d'UUID) ;
  contrat et types des deux SDK régénérés (`schema.d.ts`, `filters.ts`). Tests :
  `machines/tests/it/rest.rs` `clusters_machines_and_interfaces_filtered_by_identifier`
  (illisible = inconnu, statut, en-têtes hors `request-id` et octets ; conjonction avec
  `slug`, `name`, `machine` ; 101 machines par `QUERY`, au-delà de la page par défaut :
  une page sans suite ; `page_size` ou curseur avec `id` → 400 qui le nomme ; UUID mal
  formé → 400), `interfaces_filtered_by_several_addresses` (deux adresses : leurs deux
  interfaces ; une adresse partagée `vip` : ses deux interfaces ; conjonction avec
  `machine` et `id` ; l'adresse partagée portée par plus d'interfaces qu'une page, lue
  en entier page après page ; 64 adresses sur deux pages suivies jusqu'au bout, le `Link`
  de suite sous 6 Ko ; 65 → 400 qui nomme `address` ; valeur qui n'est pas un UUID → 400).
  Côté web : `address` passe de `string` à `array` dans `FILTERS`, et reste dans la barre
  du shell (le type `array` y tombe sur le texte, comme avant) ; le greffon machines de
  `web/v2` l'envoie en tableau d'un élément (`MachineColumn.vue`, `InterfacesTab.vue`,
  `InterfaceSearch.vue`, clé attendue de `test/machines.test.ts`), sans `page_size`.
  choix (après revue) : `address` ne suit pas la règle « une page » d'`id` : le nombre
  d'adresses ne borne pas les lignes (une adresse partagée, `vip` ou `anycast`, tient
  plusieurs interfaces, au-delà d'une page). Il reste un filtre paginé ordinaire, curseur
  et `page_size` admis ; répété 64 fois au plus, 400 qui nomme `address` au-delà. La
  borne est basse à dessein : le curseur recopie les filtres en clair (`cursor.rs`,
  `Content.filters`) et repart dans `Link` puis dans l'URL de la page suivante ;
  100 adresses donnaient environ 7 Ko, au ras des 8 Kio d'une ligne de requête. Doublons
  lus une fois, comptés dans la borne.
  reste : `address=<uuid>` répond sur une adresse que l'appelant ne lit pas (oracle,
  reporté à la 35) ; le filtre sélectif (`address` ou `id` de quelques valeurs) garde le
  plan de la liste : les interfaces parcourues dans l'ordre de la page, le rattachement
  ou l'identifiant sondé à chaque ligne, sans partir de `attachment_by_address` ni de la
  clé primaire — un parcours complet des interfaces quand peu d'entre elles répondent, à
  mesurer au banc HTTP avant d'écrire une branche propre au filtre sélectif.
- [x] 32c. Listes de kube et des flux — verify : `k-check -p kartenn-kube`,
  `k-check -p kartenn-flow` (deux identifiants dont un illisible : un seul rendu ;
  conjonction ; filtrée par `id`, une page sans suite, `page_size` ou curseur → 400, par
  `filter::window`)
  - où : `crates/kartenn-kube/src/rest/clusters.rs` (`list`), `rest/mirrors.rs`
    (`list<M>`), listes des correspondances de libellés, valeurs admises et libellés
    suivis (à repérer) ; `crates/kartenn-flow/src/rest.rs` (`page_of`, `list`, vers les
    lignes 901 et 1062)
  - dépend de : 32a
  inventaire : kube sert sept listes d'objets, les grappes (`clusters.rs`) et les cinq
  collections miroirs (`mirrors.rs`, `list<M>` : nœuds, namespaces, workloads, services,
  ingresses) ; les correspondances de labels, valeurs admises et labels suivis
  (`kube-label-mappings`, `kube-admitted-values`, `kube-tracked-labels`) n'ont pas de
  liste, seulement les routes génériques de propriétés et d'historique, rien à filtrer ;
  les observations (`kube-clusters/{id}/observations`, `workloads/{id}/observations`) ne
  sont pas des objets. Côté flux, seule `/v1/flows` est une collection d'objets : les
  non-conformités n'ont pas d'identifiant, `flow-checks` est une question, `flow-types`
  revient à la politique (32d), les sous-listes `/{collection}/{id}/flows` gardent leurs
  seuls filtres (aucun : `id` y est refusé, 400, comme tout paramètre inconnu).
  fait : `clusters.rs` (`list` lit `id` par `filter::push_id` dans le même parcours que
  `slug`, dédoublonné ; `filter::window` ; `cardinality($9::uuid[]) = 0 OR x.id = ANY($9)`
  sous le prédicat de lecture ; `Op::ids` en tête), `mirrors.rs` (`Filter.ids` lu par
  `filter_of`, `$14` dans `page_sql` sous le prédicat de lecture, `filter::window` dans
  `list<M>`, `Op::ids` en tête des cinq listes), `kartenn-flow/src/rest.rs` (`ids_of` et
  `filter::window` dans `page_of`, qui passe les `id` à `fetch` ; `list` les met dans
  `FlowFilter.ids` par `among`, le prédicat `f.id = ANY(…)` déjà écrit dans la requête
  des flux appliqués ; `wanted` saute `id` quand la route le nomme, `page_of` l'ayant lu ;
  `of_object` le refuse par son nom ; `Op::ids` et la description de la liste) ; contrat
  et types des deux SDK régénérés (`schema.d.ts`, `filters.ts`). Tests :
  `kube/tests/it/observations/route/mirrors.rs`
  `clusters_and_mirrors_filtered_by_identifier` (grappe `stage` d'une autre entité et ses
  objets : illisibles = inconnus sur les grappes, nœuds et workloads, statut, en-têtes hors
  `request-id` et octets ; conjonction avec `slug`, `cluster`, `kind`, `label`, `type` sur
  les six listes ; 101 namespaces par `QUERY` : une page sans suite ; `page_size` ou
  curseur avec `id` → 400 qui le nomme ; UUID mal formé → 400),
  `flow/tests/it/rest.rs` `flows_filtered_by_identifier` (flux d'une autre entité :
  illisible = inconnu, octets compris ; conjonction avec `entity`, `state=active` ;
  101 flux par `QUERY` : une page sans suite ; `page_size`, curseur, `state=requested`,
  `state=active,requested`, `review=pending`, `changeset`, UUID mal formé, `id` sur
  `/v1/addresses/{id}/flows` → 400 qui les nomme).
  choix (après revue) : filtrée par `id`, `/v1/flows` lit des flux appliqués ; un filtre
  qui lit les demandes en attente de revue (`review`, `changeset`, l'état `requested`, seul
  ou parmi d'autres) ne va pas avec `id` : 400 qui le nomme (`among`), plutôt qu'une page
  qui tairait la révision en attente d'un flux demandé — une demande nouvelle n'a pas
  encore d'identifiant, une révision ne se lit pas par celui du flux ; aucun écran ni
  greffon ne fait cette lecture. `/{collection}/{id}/flows` refuse `id` par son nom. Le
  prédicat `id` est dans la requête, comme les autres filtres, et la page lit autant de
  lignes que d'identifiants ; le filtre de lecture des flux reste celui du module, après
  la requête (`flows_seen`, REQ-027-1), ce qui est sans effet sous `id` (une page, sans
  curseur) mais pas sans lui (tâche 36).
  reste : l'idiome `cardinality($n::uuid[]) = 0 OR x.id = ANY($n)` (32a, 32b, 32c) garde le
  plan de la liste sous le plan générique des requêtes préparées : sans index sondé par
  l'identifiant, une liste filtrée par quelques `id` parcourt la collection dans l'ordre
  de la page ; à mesurer au banc HTTP sur les charges de travail (les workloads, la plus
  grande des collections de kube) avant d'écrire une branche propre au filtre `id`. La
  coupe de la page des flux avant leur filtre de lecture, antérieure : tâche 36.
- [x] 32d. Listes de la politique et garde du contrat : `id` sur la liste générique de la
  politique ; test du contrat : `id` aux `parameters` de chaque liste d'une collection
  d'objets, kube compris — verify : `k-check -p kartenn-policy` (filtrée par `id`, une
  page sans suite, `page_size` ou curseur → 400, par `filter::window`),
  `k-check -p kartenn-rest contract`
  - où : `crates/kartenn-policy/src/rest.rs` (`list`, vers la ligne 449),
    `crates/kartenn-rest/tests/it/contract.rs`
    (`every_collection_tells_the_clients_its_key_and_its_columns`, à imiter) ; le test
    énumère les listes réelles (le `GET` d'une collection dans l'OpenAPI), pas les types
    déclarés : les correspondances de labels, valeurs admises et labels suivis de kube
    (`kube-label-mappings`, `kube-admitted-values`, `kube-tracked-labels`) sont des types
    globaux sans route de liste (inventaire de la 32c)
  - dépend de : 32a, 32b, 32c
  fait : `crates/kartenn-policy/src/rest.rs` (`wanted` lit `id` par `filter::push_id` dans
  le même parcours que les filtres de la collection, dédoublonné ; `filter::window` dans
  `list` ; `cardinality(…::uuid[]) = 0 OR x.id = ANY(…)` dans la requête, sous le prédicat
  de lecture `visible` ; `Op::ids` en tête de la liste générique, donc des onze
  collections du module) ; contrat et types des deux SDK régénérés (`schema.d.ts`,
  `filters.ts`). Tests : `policy/tests/it/rest.rs`
  `the_collections_filtered_by_identifier` (ordre de la liste ; conjonction avec `key`,
  `effect`, `flow_type` ; un objet d'une autre collection et un objet que l'appelant ne
  lit pas : réponse identique à celle d'un inconnu, statut, en-têtes hors `request-id` et
  octets ; 103 environnements par `QUERY`, au-delà de la page par défaut : une page sans
  suite ; `page_size`, curseur, UUID mal formé → 400 qui le nomme) ;
  `rest/tests/it/contract.rs` `every_list_of_a_collection_of_objects_declares_the_id_filter`
  (remplace `the_lists_of_the_core_declare_the_id_filter`).
  choix : la garde du contrat reconnaît une liste d'objets à son `GET /v1/<collection>`
  qui répond une page (`Listing_*`) et dont l'objet `GET /v1/<collection>/{id}` prend
  `valid_at` (un objet daté) : 29 listes, noyau, VLAN, machines, kube, flux et politique ;
  l'historique ne suffisait pas (`environments` n'en a pas de route). Les types sans liste
  (`kube-label-mappings`, `kube-admitted-values`, `kube-tracked-labels`) et les pages de ce
  qui n'est pas un objet daté (anomalies, non-conformités, rapports, identités, groupes,
  jeux de modifications…) en sortent ; le test les nomme en partie et compte les 29. Aucune
  liste oubliée par les 32a–32c. La liste de la politique filtre la lecture dans la
  requête : rien à reporter à la tâche 36.
  reste : `id` n'est pas servi sur les pages qui ne sont pas des objets datés mais ont un
  identifiant et une lecture par identifiant (`policy-reports`, `changesets`, `imports`,
  `identities`, `groups`, `service-accounts`, `bindings`) : hors de « collection
  d'objets » au sens de la 32, à rouvrir si un client en a besoin.
- [x] 32e. Adresses demandées rendues sous le filtre `address` (constat de la 021-11p10-e,
  arbitrage du 2026-11-15) : `GET /v1/interfaces?address=a&address=b` ne dit pas quelle
  adresse demandée chaque interface porte, le client ne peut pas répartir la réponse par
  ligne. Sous `address`, chaque interface porte `addresses: MachineCardAddress[]`, celles
  des adresses demandées qu'elle tient et que l'appelant lit (même champ, même forme que
  sous `network`, 021/A28) ; une adresse illisible en est omise ; pas de requête par ligne,
  les adresses lues dans la requête de la page ; contrat et types des deux SDK régénérés —
  REQ-006-41 — verify : `k-check -p kartenn-machines` (deux adresses demandées, trois
  interfaces, l'une tenant les deux : chacune rend exactement les demandées qu'elle tient ;
  une adresse tenue non demandée n'est pas rendue ; une demandée illisible est omise ;
  `network` et `address` ensemble ; pagination inchangée)
  - où : `crates/kartenn-machines/src/rest.rs` (`network_addresses`, `list_interfaces`
    vers les lignes 1094 et 1127, journal des adresses lues ; `InterfaceDto.addresses`,
    sa description au contrat) ; test `machines/tests/it/rest.rs`
    `interfaces_filtered_by_several_addresses` (à étendre),
    `interfaces_of_a_network_with_their_addresses` (`held_values`, à imiter)
  - dépend de : 32b
  fait : `crates/kartenn-machines/src/rest.rs` (`network_addresses` devient
  `held_addresses` : sous `network`, la sous-requête du réseau inchangée, plus
  `t.address_id = ANY($7)` quand `address` est donné ; sous `address` seul, les
  rattachements de l'interface parmi `$7` (`interface_id = x.id AND address_id = ANY($7)`)
  puis l'adresse, sous son prédicat de lecture ; évaluée sur les lignes de la page, dans
  la requête de la page ; les adresses rendues journalisées comme lues sous `address`
  aussi ; description de `Interface.addresses` au contrat) ; contrat et `schema.d.ts` des
  deux SDK régénérés. Test : `machines/tests/it/rest.rs`
  `interfaces_filtered_by_address_carry_the_requested_ones` (deux adresses partagées
  demandées, trois interfaces : chacune rend exactement les demandées qu'elle tient, en
  une page puis page à page ; une adresse tenue non demandée absente ; journal de la
  dernière page ; une demandée illisible omise ; `network` et `address` ensemble).
  choix : avec `network` et `address` ensemble, `addresses` rend l'intersection
  (demandées et dans le réseau) : la réponse reste répartissable par adresse demandée ;
  les filtres de ligne restent indépendants, une interface gardée peut donc rendre
  `addresses: []` (une demandée hors du réseau, une autre du réseau non demandée).
  après revue : le filtre de lignes `address` (`holds_requested`) joint l'adresse, datée,
  sous le prédicat de lecture : une interface n'est appariée que par une adresse demandée
  lisible, et une adresse illisible répond comme une inconnue (statut, en-têtes, octets ;
  seule, à côté d'une lisible, parmi 64). C'est le même code à une valeur : l'oracle de
  la 32b (`address=<uuid>`) est fermé pour les interfaces, la 35 garde `domain=<slug>`
  et `parent=<slug>`. `addresses` trié par valeur puis identifiant (deux adresses de
  même valeur dans deux domaines). Tests ajoutés au même test : réponses identiques pour
  le lecteur restreint, l'administrateur voyant les interfaces qui la tiennent ; à une
  date passée (`valid_at`), les demandées tenues alors.
- [ ] 33. Résumé de toute référence rendue : recensement des corps qui portent une
  référence nue (objet d'une anomalie et d'un écart, ancre et groupe d'un VLAN, valeur
  d'une propriété de type référence), puis `{id, clé naturelle, href}`, réduit à `{id}`
  pour un objet illisible par l'appelant ; à découper par famille de corps avant d'être
  lancée si le recensement dépasse une dizaine de fichiers — REQ-006-42 — verify :
  `k-check -p kartenn-rest` (résumé présent ; `{id}` seul et identique pour un objet
  illisible et pour un objet inconnu ; ETag changé quand la clé naturelle du lié change)
  - où : `crates/kartenn-rest/src/` (résumé `domain` d'un préfixe comme modèle, H19),
    contrat OpenAPI ; 005 (prédicat de lecture)
  - dépend de : 31, 32 (le shell peut déjà grouper ses lectures sans elle)
  Découpée en 33a–33f (2026-11-15) ; recensement des corps à référence incomplète :
  noyau (`objects.rs` : `domain`, `parent`, en lecture, liste et historique, clé montrée
  quel que soit le droit sur le lié) ; anomalies et écarts (`anomalies.rs`,
  `properties.rs` : `ObjectRef {kind, id, href}` sans clé) et valeur d'une propriété de
  type référence ; VLAN (`vlan.rs` : ancre, groupe, `IdRef`) ; machines
  (`machines/src/rest.rs` : propriétaire, site, grappe, VLAN d'accès, natif et marqués,
  machine d'une interface, interface et adresse d'un rattachement, fiche) ; kube
  (`kube/src/rest.rs`, `rest/clusters.rs`, `rest/mirrors.rs`, `rest/chain.rs`) ; flux et
  politique (`flow/src/rest.rs` : extrémités ; `policy/src/rest.rs` : extrémités, points,
  résolution d'axe, sujet d'un rapport). Hors champ : changeset et auteur d'une version
  (`{id, href}`, `{id, href, name}`), qui ne sont pas des objets du réseau. Commun :
  lisibilité des liés d'une page par une autorisation groupée par type visé
  (`summary::readable`, sur `authorize_many`), sans journal de refus ; clés naturelles
  lues avec la page ou en une requête par type visé au plus ; illisible = absent aux
  dates = `{id}` seul.
- [x] 33a. Brique commune et résumés du noyau : `summary::readable` (par `Target`, noyau
  et modules) ; `Summary` à `slug` et `href` facultatifs, omis ensemble ; `domain` et
  `parent` des entités, sites, préfixes, plages et adresses, en lecture, liste et
  historique — verify : `k-check -p kartenn-rest` (résumé complet pour un lié lisible ;
  `{id}` seul et octets identiques pour un lié illisible et pour un lié absent aux
  dates ; une page à liés lisibles et non ; ETag changé au renommage du lié)
  - où : `crates/kartenn-rest/src/objects.rs` (`Summary`, `represent`, `list`, `object`,
    `history`), `properties.rs` (`visible_nodes`, à imiter) ; tests
    `rest/tests/it/objects.rs`
  - dépend de : —
  fait : `crates/kartenn-rest/src/summary.rs` (nouveau : `Summary` déplacé d'`objects.rs`
  et réexporté, `Summary::of` complet si le lié existe aux dates et est lisible,
  `Summary::bare` sinon ; `Readable`, ensemble des refus d'`authorize_many` cumulable par
  `Target`, sans journal de refus ; `readable`) ; `objects.rs` (`link_ids`,
  `linked_readable` : une autorisation par page, par objet ou par page d'historique,
  pour le type lié ; `represent` et `version_of` prennent `Readable`) ; contrat et
  `schema.d.ts` des deux SDK régénérés (`href` n'est plus requis ; `dated.ts` inchangé).
  Tests : `summary.rs` `an_unreadable_object_and_an_absent_one_leave_the_same_bytes` ;
  `rest/tests/it/objects.rs` `a_summary_leaves_alike_the_unreadable_and_the_unknown`
  (lecture, liste et historique d'un préfixe par deux appelants, l'un sans
  `core.domain.read` : même corps au résumé près ; domaine supprimé : statut, en-têtes
  hors `request-id` et octets identiques pour les deux appelants à ceux de l'illisible),
  `a_page_summarises_each_linked_object_as_its_caller_may_read_it` (préfixes de deux
  domaines dont un seul lisible ; entités dont le parent racine est hors portée) ; ETag
  au renommage du lié : `rest/tests/it/resources.rs`
  `renaming_the_parent_changes_the_etag_of_the_child` (28), inchangé.
  choix : `slug` et `href` omis (et non `null`) ensemble, la forme de H23 ; un lié absent
  aux dates rend `{id}`, comme un illisible (il rendait `slug: null` avec `href`) ;
  lisibilité « maintenant », comme le lot de la 31 ; requête sans sujet : tout lisible,
  comme le lot ; la clé naturelle reste lue par la jointure de la page (aucune requête de
  plus), seule l'autorisation s'ajoute ; helper de test `grant` (un rôle de plus pour une
  même personne) et `journal_of` (historique d'une autre table).
  reste : le filtre `domain=<slug>` (et `parent=<slug>`) d'une liste compare la clé d'un
  lié que l'appelant ne lit peut-être pas : un oracle sur la clé d'un domaine illisible,
  antérieur à cette tâche, à trancher (REQ-005, invisible égale absent).
- [x] 33b. Anomalies, écarts et valeurs de type référence : `object` d'une anomalie
  (`anomalies.rs`) et d'un écart (`properties.rs`, `/v1/property-gaps`) avec sa clé
  naturelle ; valeur `ref` d'une propriété résolue, unitaire et par lots (31) — verify :
  `k-check -p kartenn-rest`
  - où : `crates/kartenn-rest/src/anomalies.rs` (`ObjectRef`), `properties.rs`
  - dépend de : 33a ; tranché (l'utilisateur, 2026-11-15, H23 et REQ-006-42) : un
    membre `label`, une chaîne, sur tout résumé et pour tous les types, `slug` restant
    là où il existe ; `label` = slug (domaine, entité, site, groupe de VLAN), valeur
    canonique (préfixe, adresse), `<première> – <dernière>` (plage), `<vid> · <nom>`
    (VLAN), clé donnée par le module (type d'un module), sans le domaine ni le groupe ;
    lié illisible ou absent : `{id}` seul
  fait : `summary.rs` (`Summary` à `label` ; `Summary::of` pour un type à slug, le slug
  étant aussi le `label`, `Summary::labelled` pour les autres, `slug`, `label` et `href`
  omis ensemble ; `Summaries::read`, résumés d'objets de tous types en une lecture des
  clés aux dates, une par module visé et une autorisation par type ; `key_of`, la
  composition par type) ; `kartenn-store-pg/src/keys.rs` (`natural_keys` : genre et clé
  stockée de chaque objet aux dates, une requête, noyau et VLAN ; `None` pour un objet de
  module) ; `kartenn-service/src/module.rs` (`ModuleHandler::keys`, clés des objets d'un
  module, aucune par défaut) ; `objects.rs` (`address_text`, `prefix_text`,
  `range_bounds` partagés avec `represent`) ; `anomalies.rs` (`ObjectRef {kind, id,
  label, href}`, le genre d'un objet de module désormais donné) ; `properties.rs`
  (`object` d'un écart ; valeurs `ref`, seules et en ensemble, de la lecture unitaire, du
  lot, et `resolved`/`expected` d'un écart d'une propriété `ref` ; `declared`,
  `ref_keys`, `references`, `summarised`, `with_summaries`) ; contrat et `schema.d.ts`
  des deux SDK régénérés. Tests : `summary.rs`
  `an_unreadable_object_and_an_absent_one_leave_the_same_bytes` (étendu : slug, clé
  composée, `Summaries`), `each_kind_reads_its_key_as_a_human_does` ;
  `rest/tests/it/objects.rs` `the_object_of_an_anomaly_and_of_a_deviation_carries_its_key`,
  `a_value_of_type_reference_leaves_alike_the_unreadable_and_the_unknown` (lecture
  unitaire et lot, valeur seule et ensemble, deux appelants, puis entités supprimées :
  statut, en-têtes hors `request-id` et octets identiques) ; 33a mis à jour (`label`).
  choix : `label` aligné sur `nameOf` du shell (`<vid> · <nom>`, `<première> –
  <dernière>`) ; le groupe de VLAN, qui a un slug, porte `slug` et `label` ; un objet de
  module dont le module ne donne pas de clé (aucun avant 33d, 33e) rend `{id}` (son
  `ObjectRef` garde `kind`), comme aujourd'hui où son `href` était `null` ; `ObjectRef`
  sans `slug` (il n'en avait pas) ; `href` d'`ObjectRef` omis au lieu de `null` ; les
  valeurs d'un écart d'une propriété `ref` résumées aussi (même mécanisme, REQ-006-42
  « tout corps ») ; une lecture des déclarations aux dates par lecture unitaire (le lot la
  faisait déjà sans `key`) pour savoir quelles clés sont `ref` ; une valeur mal formée
  reste brute ; lisibilité « maintenant » (33a) ; les clés lues aux dates de la route
  (anomalies et écarts : l'état présent, qu'ils lisent seul) ; le `kind` d'un
  `ObjectRef` dit l'existence et non la lisibilité, ce qui ne fuit rien : une anomalie
  ou un écart d'un objet illisible n'est pas listé (`open_and_readable`), comme celui
  d'un objet supprimé (revue : `an_anomaly_of_an_unreadable_object_and_of_a_gone_one_leave_the_same_bytes`).
  reste : le shell montre encore l'identifiant d'une valeur `ref` (et le JSON d'un
  ensemble) : 11p10-d ; l'explication d'une valeur (`/explanation`, `raw_value`) rend
  encore une référence brute : 33f ; `kartenn-store-pg/src/keys.rs` `natural_keys`
  redit l'union de `property_read::kind_names` (même remarque que la revue de la
  017-26), non factorisée.
- [x] 33c. VLAN : ancre et groupe d'un VLAN et d'un groupe — verify :
  `k-check -p kartenn-rest vlan`
  - où : `crates/kartenn-rest/src/vlan.rs` (`IdRef`, `Anchor`) ; `summary.rs`
    (`Summary::of` pour un groupe, à slug ; `Summary::labelled` pour une ancre de genre
    variable, ou `Summaries::read`), `label` d'un VLAN `<vid> · <nom>`
  - dépend de : 33a, 33b (forme de la clé)
  fait : `vlan.rs` (`AnchorSummary`, schéma `VlanGroupAnchorSummary` : l'ancre d'un groupe
  lu, résumé de son entité, site ou domaine ; `group` d'un VLAN, d'une occupation et d'un
  prochain VID libre en `Summary` ; `group_dto`, `vlan_dto` prennent les `Summaries` de la
  page, lues par `Summaries::read` en un lot pour la page ou l'objet ; `itself`, le
  groupe que sert sa propre route) ; contrat et `schema.d.ts` des deux SDK régénérés.
  Test : `rest/tests/it/vlan/rest.rs`
  `the_anchor_and_the_group_leave_alike_the_unreadable_and_the_unknown` (lecture unitaire
  et liste des groupes et des VLAN, deux appelants, l'un sans `core.domain.read` ou sans
  `core.vlan_group.read` : même corps au résumé près ; domaine puis groupe supprimés :
  statut, en-têtes hors `request-id` et octets identiques ; occupation et prochain VID
  libre résumant leur groupe).
  choix : les corps d'écriture gardent `VlanObjectRef` et `VlanGroupAnchor` (`{id}`), une
  référence qu'un client donne et non qu'il lit ; l'ancre lue prend un schéma à part,
  `VlanGroupAnchorSummary` ; les clés par `Summaries::read` (une lecture des clés et une
  autorisation par genre visé) plutôt qu'une jointure par genre d'ancre dans la requête
  de la page ; le groupe d'une occupation ou d'un prochain VID libre est résumé sans
  autorisation de plus (la route l'a déjà ouvert, `dated_read`, et lu aux dates,
  `group_at`) ; aucune référence à un VLAN dans `vlan.rs` (son `label` `<vid> · <nom>`
  vient de la 33b, `key_of`).
  reste : l'historique d'un groupe et d'un VLAN (`modules.rs`, `ModuleObjectDto`, commun
  à tous les types de module) rend `parent` et `domain` en identifiants nus, ni résumés
  ni masqués selon la lisibilité du lié : hors du recensement 33b–33f, à rattacher à la
  33f (garde du contrat) ou à une tâche à part.
- [x] 33d. Machines : références des grappes, machines, interfaces, rattachements et
  fiche — verify : `k-check -p kartenn-machines`
  - où : `crates/kartenn-machines/src/rest.rs` (`IdRef`, `HostRef`) ; implémenter
    `ModuleHandler::keys` pour les types du module (sinon leurs objets restent `{id}`
    dans les anomalies et les valeurs `ref`) ; `Summary::labelled` avec la clé lue par
    la jointure de la page et `summary::readable`
  - dépend de : 33a, 33b
  fait : `crates/kartenn-machines/src/read.rs` (`keys_of` : nom d'une machine, slug d'un
  cluster, nom seul d'une interface, aux dates, une requête) et `handler.rs`
  (`ModuleHandler::keys` par le port de lecture) ; `rest.rs` : `owner`, `site`,
  `member_of` d'un cluster et d'une machine (liste, lecture, fiche, `hosted`,
  `clusters/{id}/machines`), `machine` et `vlan.access`/`native`/`tagged` d'une
  interface (liste, lecture) en `Summary` par `Summaries::read` (une lecture des clés
  du noyau et une du module par page, une autorisation par type) ; l'hôte en
  `MachineHostSummary {id, kind, label, href}`, `kind` omis avec `label` (le genre d'un
  hôte illisible ne se dit pas), `HostRef` gardé pour l'écriture ; `interface` et
  `address` d'un rattachement (`MachineAddress`) par `Summaries::read` aussi (clé
  d'adresse du noyau, lisibilité d'`authorize_many`, revue) ; contrat et `schema.d.ts`
  des deux SDK régénérés ; `web/v2/plugins/machines` (`MachineList`, `MachineRecord`,
  `ClusterRecord`, `HostSection`) et leurs tests, `sdk/test/dependents.test.ts`, le
  parcours `moves.spec.ts` lisent `host.kind`/`host.id` ; design 009 « Routes ». Test :
  `machines/tests/it/rest.rs` `a_reference_leaves_alike_the_unreadable_and_the_absent`
  (cluster, machine, fiche, interface, en lecture et en liste par `id`, deux
  appelants : corps identique au résumé coupé près, l'hôte illisible sans `kind` ; puis
  entités, cluster, machine hôte et VLAN supprimés : statut, en-têtes hors
  `request-id` et octets du lecteur identiques à ceux de l'illisible) ;
  `the_addresses_of_a_machine_are_summarised_as_the_core_reads_them` (IPv4, IPv6 à deux
  suites de zéros ; trois appelants : tout, les adresses d'un réseau, sans interfaces ;
  `label` égal à la `value` de `/v1/addresses/{id}`) ;
  `a_host_as_read_is_refused_as_a_write` (Merge Patch de `host` tel que lu : 422,
  `machines.operation.invalid`, détail `host:…`) ;
  `renaming_the_host_changes_the_etag_of_its_guest`. Design 006 « Représentations » (un
  résumé se lit, une référence s'écrit par son identifiant) et 009 « Routes ».
  choix : `Summaries::read` plutôt qu'une jointure par requête de page (moins de SQL
  touché, nombre de lectures constant par page, VLAN et entités compris) ; cluster en
  `label` seul, sans `slug` (le crochet `keys` ne rend qu'une chaîne ; même forme dans
  les anomalies) ; `kind` de l'hôte à côté du résumé, et non l'enveloppe
  `{machine: …}` qui dirait le genre ; `interface_name`, `value` et `href` d'un
  rattachement gardés à côté des résumés ; `addresses` d'une interface (REQ-006-41) et
  de la fiche inchangées : objets rendus, déjà filtrés par la lisibilité ; dans le
  shell, un hôte sans `kind` se montre par son identifiant, sans type ni lien
  (`CitedName` à `type` facultatif) ; pas d'adresse IPv4-mapped à tester, aucune n'est
  stockée (REQ-002-31).
  reste : les résumés lus par `Summaries::read` hors de l'instantané de la page (même
  remarque que la 33b) ; `interface_name`/`value`/`href` redondants d'un rattachement,
  à retirer avec 11p10-d ; un cluster n'a pas `slug` dans ses résumés (le crochet
  `keys` ne dit pas de slug) : à trancher avec 33h ; le refus d'un `host` tel que lu
  dit le message de serde (« invalid value: map, expected map with a single key ») avec
  un `pointer` vide, au lieu de `/host` et de la forme attendue (dans `handler.rs`,
  hors de cette tâche).
- [x] 33e. Kube : grappes, miroirs, chaîne d'exécution, observations — verify :
  `k-check -p kartenn-kube`
  - où : `crates/kartenn-kube/src/rest.rs` (`IdRef`), `rest/clusters.rs`,
    `rest/mirrors.rs`, `rest/chain.rs` (`HostRef`) ; `ModuleHandler::keys` et
    `Summary::labelled` comme la 33d
  - dépend de : 33a, 33b
  fait : `crates/kartenn-kube/src/keys.rs` (nouveau : clés des objets du module aux dates,
  une requête pour tous) et `handler.rs` (`ModuleHandler::keys`) ; `rest.rs` (`Refers`,
  `summarised` : les références d'une page lues `{id}` puis résumées ensemble par
  `Summaries::read`, une lecture des clés et une autorisation par type pour la page ;
  `journal_summaries` : `ObservationDto` résumé par `keys::at_instants`, une requête
  pour la page quel que soit le nombre d'instants, et `summary::readable` par type) ;
  `rest/clusters.rs`, `rest/mirrors.rs` (lecture et liste de la grappe et des cinq
  miroirs), `rest/chain.rs` (`HostRef` retiré, `LinkDto` en résumés) ; `IdRef`
  (`KubeObjectRef`) ne reste qu'aux corps écrits ; `connector_group` d'une grappe en
  `RightsGroupRef` ; contrat et `schema.d.ts` des deux SDK régénérés. Tests :
  `tests/it/observations/route/summaries.rs` `a_mirror_summarises_what_its_caller_may_read`
  (charge, nœud, service, espace de noms), `a_cluster_leaves_alike_an_unreadable_reference_and_an_absent_one`,
  `a_journal_names_its_objects_as_they_were_at_the_instant_asked`,
  `a_chain_leaves_alike_an_unreadable_node_and_a_gone_one` (statut, en-têtes hors
  `request-id` et octets identiques), `an_anomaly_of_a_setting_carries_the_key_of_the_module`
  (clé d'un réglage par `/v1/anomalies`) ; `route/clusters.rs`, `route/mirrors.rs`,
  `route/chain.rs` mis à jour.
  choix : clé d'un objet du module = son seul nom (nœud, espace de noms, charge, service,
  ingress), sans grappe ni espace de noms, la charge sans sa sorte (son `href` la dit) ;
  slug pour une grappe, rendu en `label` seul (le crochet `keys` ne donne qu'un `label`,
  pas de `slug` pour un type de module) ; réglages : `label_key` (étiquette suivie),
  `axis_key` (valeurs admises, sans l'espace de noms), `<clé>=<valeur> → <axe>`
  (correspondance) ; `Summaries::read` plutôt qu'une jointure par lié (les liés couvrent
  noyau, kube et machines) ; clés lues aux dates de la route, sauf le journal (revue,
  décision de la session principale) : ses clés à l'instant `at` de la question (route
  de la grappe), à défaut à l'instant où chaque pod fut vu (`from`, route d'une charge),
  par les assertions courantes, la lisibilité jugée maintenant comme ailleurs (33a) ; un
  hôte illisible de la chaîne reste `null` (Q-019-31), son type ne se dit pas ;
  `connector_group`, un groupe des droits et non un objet du référentiel, reste `{id}`
  sous un schéma distinct, `RightsGroupRef` (décision provisoire, voir 33f).
  reste : les machines d'un nœud et de la chaîne restent `{id}` jusqu'à la fusion de la
  33d (`keys` de `machines`).
- [x] 33f. Flux et politique (découpée le 2026-11-15 : 33f1, 33f2, 33f3, chacune dans un
  contexte d'agent) :
  - [x] 33f1. Flux : les extrémités d'un flux, d'une demande et d'une non-conformité en
    résumés ; `ModuleHandler::keys` pour `kartenn-flow` — verify : `k-check -p kartenn-flow`
    (illisible et absent laissent les mêmes octets)
    - où : `crates/kartenn-flow/src/rest.rs` (`FlowEndpoint`), `handler.rs` (`keys`, à
      imiter : `crates/kartenn-kube/src/keys.rs`, 33e) ; `summary.rs` (`Summaries::read`)
    - dépend de : 33a–33e
    - arbitrage de l'utilisateur (2026-11-16) : option B. REQ-027-2 n'est pas amendée :
      les extrémités d'un flux restent rendues selon elle et sont exemptées des résumés
      (l'option A, amender REQ-027-2 pour les couper comme les autres références, est
      écartée). La tâche devient : l'exemption écrite et motivée dans la garde
      (`UNSUMMARISED`, sans renvoi à cette tâche) et au design 006 « Représentations »,
      et un test que ce que REQ-027-2 laisse voir d'un flux ne dépasse pas ce qu'elle
      dit. *Ajusté à la clôture (2026-11-17), REQ-027-2 primant sur la note reconstruite* :
      la note prévoyait de résumer les autres références des corps de flux (entité,
      service, demande) ; REQ-027-2 range le service parmi les données du flux servies
      sans réponse réduite selon le lecteur, et un résumé de l'entité y porterait son
      slug, une donnée de l'objet que seule sa route sert. Aucune référence d'un corps
      de flux n'est donc résumée : toutes sont des données du flux, par identifiant.
    - la garde de la 33f3 exempte en renvoi à cette tâche `FlowEvaluation`,
      `FlowNonconformity` et `FlowTriage` (`UNSUMMARISED`, `rest/tests/it/contract.rs`) :
      retirer ces exemptions, ne garder que ce qui n'est pas un objet du référentiel
      (identité, changeset) et, motivé par REQ-027-2, ce qui est une donnée du flux.
    fait : `rest/tests/it/contract.rs` : les exemptions globales en renvoi remplacées par
    une exemption par membre, motivée : `FlowEvaluation` `source`, `destination`,
    `decisive`, `unmet_conditions`, `FlowTriage` `unmet_conditions`, `FlowNonconformity`
    `flow`, `entity`, `rule` (données du flux, REQ-027-2), `requester` (identité),
    `changeset` ; design 006 « Représentations » (garde : les corps d'un flux exemptés
    des résumés, REQ-027-2 primant sur REQ-006-42). Test :
    `flow/tests/it/by_endpoint.rs` `a_flow_carries_nothing_of_an_object_beyond_req_027_2`
    (un flux, la liste, les flux d'une interface, les évaluations : même corps à Alice,
    qui ne lit ni `eth1` ni `198.51.100.0/24`, au lecteur de l'entité et à un lecteur de
    tout ; ni nom de machine ou d'interface, ni slug d'entité, aucun membre `label`,
    `labels`, `slug`, `name` ni `href` à aucune profondeur ; chaque page non vide).
    choix : exemption par membre plutôt que par schéma entier (`*`), pour qu'un membre
    ajouté à un corps de flux soit relevé et motivé ; la non-conformité tenue pour un
    corps de flux (« quelle que soit la voie de lecture »), son `rule_key` et ses textes
    restant servis ; contrat inchangé (aucun schéma de réponse ne bouge), SDK non
    régénérés ; le test omet les non-conformités, vides dans ce monde (la garde les
    couvre par leur schéma) ; `ModuleHandler::keys` pour `kartenn-flow` non écrit : un
    flux n'a pas de clé naturelle et aucun corps de flux ne résume plus rien.
    reste : un flux nommé hors de ses propres corps (différentiel d'un changeset, 33g)
    serait résumé `{id}` faute de clé ; à décider avec la 33g s'il lui faut un `label`
    (ses extrémités ?). Le constat de la 002-29d3 (un demandeur nomme une extrémité qu'il
    ne lit pas et relit sa valeur dans le flux créé) reste à la tâche 31, inchangé ici.
  - [x] 33f2. Politique : extrémités, points, résolutions d'axe et lignes de rapport en
    résumés ; `ModuleHandler::keys` pour `kartenn-policy` — verify :
    `k-check -p kartenn-policy` (illisible et absent laissent les mêmes octets)
    - où : `crates/kartenn-policy/src/rest.rs` (`PolicyEndpoint`, `PolicyPoint`,
      `PolicyAxisResolution`, `PolicyReportItem`), `handler.rs` (`keys`)
    - dépend de : 33a–33e
    fait : `crates/kartenn-policy/src/keys.rs` (nouveau : clés des objets du module aux
    dates, une requête pour les dix tables) et `handler.rs` (`ModuleHandler::keys`) ;
    `rest.rs` : `PolicyPoint.object` (au lieu d'`id`), `PolicyAxisResolution.origin`,
    `chain`, `machines` en `Summary`, lus ensemble pour la réponse par `Summaries::read`
    aux dates de la requête (`named` dédoublonne : une lecture des clés par module visé
    et une autorisation par type, aucune par point) ; `verdict_dto` devient asynchrone
    (`api`, `context`, `moment`), son appel du test d'une connexion
    (`kartenn-flow/src/rest.rs`, une ligne) suivi ; `PolicyReportItem.subject` en
    `Summary` (toujours `null` sans sujet), lus pour la page aux dates du rapport ;
    contrat et `schema.d.ts` des deux SDK régénérés ; design 010 « Droits ». Tests :
    `tests/it/rest.rs` `a_verdict_summarises_what_its_caller_may_read` (deux appelants,
    l'un sans `core.prefix.read` : réponse identique aux résumés des préfixes coupés à
    `{id}` près), `a_report_item_leaves_alike_an_unreadable_subject_and_an_absent_one`
    (préfixe, inconnu, règle, valeur, service, type de flux par leur clé ; préfixe
    illisible et préfixe absent à la date du rapport : statut, en-têtes hors
    `request-id` et octets identiques).
    choix : `PolicyEndpoint` inchangé, corps écrit d'une requête (un résumé se lit, une
    référence s'écrit) ; le point prend le membre `object` (comme l'`ObjectRef` d'une
    anomalie) plutôt qu'un `id` devenu objet ou des membres aplatis hors `Summary`, que
    la garde de la 33f3 relèverait ; clé d'un objet de politique : `property_key` d'un
    axe, `value_key` d'une valeur (sans son axe), `key` des types de flux, services,
    motifs, règles, conditions, signaux et règles d'inférence, `role_key` d'un rôle
    exempté ; sujet d'un rapport lu à sa date `valid_at` par les assertions courantes,
    la lisibilité jugée maintenant (33a) ; explication réduite : rien à résumer, aucune
    lecture ; REQ-027-2 (flux servi tel quel) ne vise que la réponse d'un flux, pas le
    verdict (REQ-010-25 y règle déjà ce que voit chaque appelant), ni le test d'une
    connexion qui l'embarque (REQ-011-26).
    reste : `detail` d'une ligne de rapport peut porter des identifiants bruts (selon la
    rubrique), hors de cette tâche (33f3, garde) ; l'export JSON d'un rapport
    (`report::export`, non servi par REST) garde `subject` en identifiant.
  - [x] 33f3. Restes communs et garde du contrat : l'explication d'une valeur
    (`explanation`, `raw_value`) dont une référence est encore brute (reste de la 33b) ;
    l'historique d'un objet de module (`ModuleObjectDto`, commun à tous les types : `parent`
    et `domain` en identifiants nus, ni résumés ni masqués selon la lisibilité du lié,
    reste de la 33c, que la garde ne verrait pas puisque ce sont des chaînes) ; le refus
    d'un `host` renvoyé tel que lu, qui montre le texte de la désérialisation sans
    pointeur `/host` (reste de la 33d) ; puis la garde : aucun schéma de réponse ne porte
    une référence `{id}` seule, ni un identifiant d'objet nu, hors résumé — verify :
    `k-check -p kartenn-rest contract`, `k-check -p kartenn-rest`
    - où : `crates/kartenn-rest/src/properties.rs` (`explanation`, `raw_value`),
      `crates/kartenn-rest/src/modules.rs` (`ModuleObjectDto`),
      `crates/kartenn-machines/src/handler.rs` (désérialisation de `host`),
      `crates/kartenn-rest/tests/it/contract.rs` (garde, à côté de celle des filtres `id`)
    - dépend de : 33f1, 33f2
    fait (avant la 33f1, ses corps exemptés par renvoi) : `properties.rs` : la valeur
    d'une explication et celle qu'elle remplace (`replaces.value`), pour une propriété
    `ref`, résumées par `Summaries::read` aux dates de la lecture ; les valeurs d'une
    suggestion d'une propriété `ref` en `Summary` (schéma `ExpectedValue`, `Summary` ou
    `RawValue`, au lieu de `RawValue`) ; `modules.rs` : `parent` et `domain` de
    `ModuleObject` (historique d'un objet de module et d'un VLAN ou d'un groupe) en
    `Summary`, lus pour la page ; `summary.rs` : `Summaries::read` sans requête pour aucun
    identifiant ; `kartenn-machines` : `handler.rs` (`host_input`, `HOST_FORM` : la
    forme d'un hôte dite au lieu du texte de serde, en création comme en Merge Patch et
    par `POST /v1/changesets`), `rest.rs` (`Writes::check` : un `host` d'une autre forme
    refusé avant le changeset, 422 `invalid-member`, `pointer` `/host`) ; contrat et
    `schema.d.ts` des deux SDK régénérés ; design 006 « Représentations » (garde, refus
    d'un membre tel que lu) et 009. Tests : `rest/tests/it/contract.rs`
    `no_response_names_an_object_by_its_identifier_alone` (tout schéma atteint depuis
    une réponse : un objet au seul membre `id` UUID, ou un membre UUID autre que `id`,
    hors exemptions motivées d'`UNSUMMARISED` ; une exemption inutile échoue) ;
    `rest/tests/it/objects.rs`
    `the_explanation_and_the_suggestion_of_a_reference_summarise_it` (explication d'une
    valeur seule et d'un ensemble, suggestion d'une source de test, deux appelants, puis
    entités supprimées : statut, en-têtes hors `request-id` et octets identiques) ;
    `rest/tests/it/vlan/rest.rs`
    `the_history_leaves_alike_an_unreadable_parent_and_an_unknown_one` (domaine d'un
    groupe, groupe d'un VLAN) ; `machines/tests/it/rest.rs`
    `a_host_as_read_is_refused_as_a_write` (Merge Patch et création : `invalid-member`,
    `/host`, forme dite ; changeset : `/operations/0`, même détail).
    choix : la garde lit le contrat de référence (modules compris) et nomme ses
    exemptions par schéma et membre, une page `Listing_<item>` par son item ; ne sont
    pas des objets du référentiel (exemptés) : `version`, `uid`, `pod_uid`, les lots
    d'observation, les identités (`author`, `on_behalf_of`, `validator`, `identity`,
    `requested_by`), brouillon, politique d'approbation, changeset d'une revue de flux et
    `RightsGroupRef` ; renvois : `FlowEvaluation`, `FlowNonconformity`, `FlowTriage`
    (33f1) ; `DiffObject`, `DiffItemDto.object`, `Ref`, `Operation`, `Placement`,
    `SectionRowDto.object`, `ViolationDto.subjects`, `PendingReason.objects`,
    `Required.objects`, `Cascaded`, `Unsupported` (33g) ; `AttachmentItem.entity`,
    `Scope`, `HeldScope` (42) ; un hôte mal
    formé refusé par la route (`invalid-member` et `pointer`, comme un membre non
    modifiable, design 006 « Routes ») plutôt que par la violation de l'opération, dont le
    pointeur est le corps entier pour un `Update…` de module et qu'il aurait fallu
    porter dans `Violation` et son journal ; liés de l'historique d'un objet de module
    lus aux dates de la route (comme 33d, 33e), non au début de chaque version comme la
    jointure de l'historique du noyau ; la suggestion résume une valeur texte qui est un
    UUID, pour une propriété `ref` seulement ; `Summaries::read` sans identifiant rend
    « tout lisible » sans requête.
    revue (une passe) : la garde relève aussi la référence sans libellé (`id` UUID et
    rien d'autre que `kind`, `type`, `href`, `{ref}`) et lit la donnée des événements
    (`x-kartenn-events`) ; recensement : `NodeRef` (explication, sources d'une valeur)
    et `GraphNode` (extrémités du graphe) portent désormais `label` (`Summaries::labels`,
    les clés sans jugement de lisibilité, ces nœuds étant déjà réduits à ce que
    l'appelant lit ; l'objet lui-même compris parmi les détenteurs pour sa clé) ;
    exemptés : `ChangesetRef`, `GroupMember` (identité ou groupe d'émetteur),
    `EventChangeset.changeset`, `EventReady.instance`, `EventChanges`, `EventObject`
    (clés d'invalidation des seuls objets visibles, REQ-022-4) ; renvois `Cascaded`,
    `Unsupported` (33g), `Scope`, `HeldScope` (42) ; `PolicyEndpoint` n'est qu'un corps de
    requête (hors de la garde) ; `Writes::check` lit le corps par `kit::body` (415 et 400
    du kit d'abord, puis l'hôte, avant le 428 d'`If-Match`, l'ordre du kit) ; description
    d'`ExpectedValue` : résumé pour une propriété `ref` seulement. Tests :
    `properties_resolve_explain_and_mask` (nœuds à `label`),
    `machines/tests/it/rest.rs` `graph_edge_address_to_interface` (extrémités à
    `label`), `a_host_as_read_is_refused_as_a_write` (415 avant, 422 sans `If-Match`).
    reste : les liés de l'historique d'un objet de module lus aux dates de la route
    (33i) ; `Cascaded` d'un changeset lu (`GET /v1/changesets/{id}`) dit le genre et
    l'identifiant d'un objet fermé par cascade à tout lecteur du changeset, qu'il lise
    l'objet ou non : fuite relevée par la revue, touchant le contrat du changeset, à
    arbitrer dans la 33g ; la garde ne voit pas les valeurs `Object` (explication,
    différentiel, détail d'une ligne de rapport, 33f2) ; à la fusion de la 33f1, de la
    33g et de la 42, retirer leurs exemptions (la garde échoue sur une exemption
    inutile).
  - exemption de la garde : `RightsGroupRef` (le groupe de connecteur d'une grappe kube,
    33e), un groupe des droits qui n'est pas un objet du référentiel et n'a pas de
    résumé ; la garde l'exempte avec cette raison. Question laissée ouverte (antérieure
    à la 33e) : un lecteur de la grappe sans droit sur les droits voit l'identifiant du
    groupe.
  - dépend de : 33a–33e
- [ ] 33g. Changesets (découpée le 2026-11-16 : 33g1 à 33g5) : le différentiel (`DiffObject {id}`), les opérations rendues
  (`RefDto`) et les valeurs de type référence dans `before`/`after`, démasquées
  aujourd'hui selon la lisibilité de l'objet et non de la cible (revue de la 33a : hors
  du recensement 33b–33f, et la garde du contrat de la 33f échouerait sur eux) — verify :
  `k-check -p kartenn-rest` (différentiel lu par deux appelants : cible illisible et
  cible inconnue rendues à l'identique, octets compris)
  - où : `crates/kartenn-rest/src/cycle.rs` (≈ 530, `DiffObject`),
    `crates/kartenn-rest/src/operation.rs` (`RefDto`), `summary.rs` (`Readable`)
  - dépend de : 33a, 33b
  - arbitrage de l'utilisateur (2026-11-16), objets clos par cascade (`Cascaded`,
    `GET /v1/changesets/{id}`) : option 1, un résumé, sans `kind` ni `label` pour un
    objet que l'appelant ne lit pas, comme l'hôte d'une machine (33d) ; l'option 2
    (les seuls objets lisibles et un compte des autres) est écartée, le compte restant
    un signal.
  - la garde de la 33f3 exempte aussi, en renvoi à cette tâche : `DiffItemDto.object`,
    `Ref`, `Operation`, `Placement`, `SectionRowDto.object`, `ViolationDto.subjects`,
    `PendingReason.objects`, `Required.objects`, `Cascaded`, `Unsupported`
    (`changesets.rs`, `cycle.rs`) ; les résumer ou motiver leur exemption, et retirer de
    `UNSUMMARISED` (`rest/tests/it/contract.rs`) celles qui ne servent plus.
  - à arbitrer (contrat du changeset, revue de la 33f3) : `cascaded` d'un changeset lu
    (`GET /v1/changesets/{id}`, `cycle.rs` `represent`) nomme `{kind, id}` de chaque objet
    fermé par cascade pour tout lecteur du changeset, qu'il lise l'objet ou non (la
    cascade exige `core.<kind>.edit` de l'auteur, pas la lecture du lecteur) : le genre
    d'un objet illisible se dit. Options : résumer (`kind` omis avec `label` pour un
    illisible, comme l'hôte d'une machine, 33d) ; ou ne garder que les objets lisibles
    avec un compte des autres.
  - reste pour 33g2 à 33g5 (établi par la 33g1) : un objet que le changeset ferme
    (différentiel, opérations, placements, sujets) n'est plus situé maintenant ; pour un
    appelant à portée, l'autorisateur le juge hors de tout placement et `Summaries::read`
    le rend `{id}` même s'il le lisait : le lire par `Summaries::read_closed` à l'instant
    qui précède sa fermeture, et tester avec un appelant à portée.
  - découpée le 2026-11-16 (33g1 à 33g5, chacune dans un contexte d'agent) ; chacune
    retire de `UNSUMMARISED` les exemptions qu'elle rend inutiles, régénère le contrat
    (`cargo run -p kartenn-server --all-features -- openapi`) et les types des SDK
    (`node scripts/gen.mjs` sous `web/sdk` et `web/v2/sdk`), et lit ses cibles par
    `Summaries::read` (`summary.rs`) :
  - [x] 33g1. Objets clos par cascade (`Cascaded`, arbitrage du 2026-11-16, option 1) :
    `{id, kind, slug, label, href}` pour un objet que l'appelant lit, `{id}` seul sinon,
    dans le changeset lu, appliqué et simulé — verify : `k-check -p kartenn-rest cascad`
    (deux appelants, objet clos par cascade illisible pour l'un : `{id}`, mêmes octets
    qu'un absent hors l'identifiant ; le genre ne se dit qu'avec la clé)
    - où : `crates/kartenn-rest/src/changesets.rs` (`Cascaded`, `cascaded`, `applied`,
      `answered` `Outcome::Simulated`), `cycle.rs` (`represent`), `summary.rs`
      (`Summaries`) ; modèle : `kartenn-machines/src/rest.rs` (`HostSummary`, `host`) ;
      test voisin `rest/tests/it/simulated.rs`, cascade de `rest/tests/it/vlan/references.rs`
      `closing_a_group_cascades_its_vlans`
    fait : `changesets.rs` (`Cascaded` : `id`, puis `kind`, `slug`, `label`, `href`
    omis ensemble pour un objet illisible ou absent ; `cascaded` lit les clés par
    `Summaries::read` à l'instant qui précède la fermeture, `cascaded_by` pour un
    changeset appliqué ou rejoué ; `applied` et `replayed` prennent le contexte de la
    requête) ; `cycle.rs` (`represent` : à `valid_from` du changeset) ;
    `kartenn-store-pg` `changesets.rs` (`valid_from`, lue seulement s'il y a des
    fermetures) ; `drafts.rs` (rejeu d'une soumission) ; `UNSUMMARISED` sans `Cascaded` ;
    contrat et `schema.d.ts` des deux SDK régénérés ; `web/v2/sdk/src/data/write.ts`
    (`afterApplied` n'invalide que les fermetures qui disent leur genre) ;
    `web/v2/shell/src/views/edit/close.ts` et `ClosePanel.vue` (les fermetures sans genre
    comptées à part et en dernier, « Objets que vous ne pouvez pas lire · n »,
    `edit.close.unreadable`,
    test `shell/test/close.test.ts`) ; design 006
    « Idempotence », rejeu. Test : `rest/tests/it/vlan/rest.rs`
    `the_closures_of_a_cascade_say_their_kind_with_their_key_only` (simulation,
    application et lecture d'un groupe fermé avec ses deux VLAN : `kind`, `label`,
    `href` pour un lecteur des VLAN, `{id}` pour un lecteur des seuls changesets ; les
    VLAN effacés, le lecteur des VLAN reçoit les octets du second, en-têtes compris).
    choix : clés lues à `valid_from` du changeset moins une microseconde, telles
    qu'affirmées maintenant (une fermeture sous `within` d'une autre période peut donc
    laisser `{id}` à un lecteur : perte d'information, pas de fuite) ; simulation lue à
    l'instant de la requête, quel que soit le `valid_from` demandé, même à venir (rien
    n'est fermé) ; le rejeu d'une clé d'idempotence sert les résumés du moment, non ceux
    de la première réponse (REQ-006-15 amendé le 2026-11-17, changement de contrat porté
    à la connaissance de l'utilisateur).
    corrections de la revue (une passe) : un appelant à portée (droits limités à une
    entité) recevait `{id}` pour toute clôture appliquée ou lue, l'autorisateur jugeant
    l'objet maintenant, donc fermé, hors de tout placement (test rouge
    `a_scoped_author_reads_the_closures_it_could_read` : simulation résumée, 201 en
    `{id}`) ; `summary.rs` `Summaries::read_closed` juge chaque objet là où il était à
    l'instant lu (`PgAuthorizer::visibility` des droits tenus maintenant, puis `outside`
    à cette date), `Readable::placed`, `readable_placed` ; `Summaries::read` inchangé.
    Rejeu testé (même appelant : même corps ; `core.vlan.read` retiré : `{id}`),
    simulation par un rédacteur qui ne lit pas les VLAN (`{id}`) ; SDK : une clôture
    sans genre invalide son identifiant et toutes les listes (`write.test.ts`) ; libellé
    de la 021-11p10-d1 au pluriel (« Objets illisibles pour vous, ou absents »), testé
    dans `ClosePanel.vue` (`close.test.ts`).
  - [x] 33g2. Différentiel : `DiffObject {id}` (`GET /v1/changesets/{id}/diff`),
    `DiffItemDto.object` (échantillon d'une simulation) et les valeurs de type référence
    de `before`/`after`, démasquées selon la lisibilité de l'objet et non de la cible —
    verify : `k-check -p kartenn-rest diff` (deux appelants : cible illisible et cible
    inconnue rendues à l'identique, octets compris)
    - où : `crates/kartenn-rest/src/cycle.rs` (≈ 530, `DiffObject`, `diff_entry`),
      `changesets.rs` (≈ 607, `DiffItemDto`) ; valeurs de référence comme la 33b
      (`properties.rs`) ; test voisin `rest/tests/it/cycle.rs`
    fait : `cycle.rs` (`DiffEntry.object` en `Summary`, `DiffObject` retiré ;
    `diff_entries`, `diff_summaries` : objets et cibles des valeurs `ref` montrées lus
    aux dates de la route, puis les absents par `Summaries::or_closed` au début de la
    période du changeset, puis à l'instant d'avant) ; `changesets.rs` (`DiffItemDto.object`
    en `Summary`, `sampled` : échantillon résumé à l'instant de la requête, valeurs `ref`
    résumées et toujours en texte JSON) ; `summary.rs` (`Summaries::or_closed`, sur lequel
    repose `read_closed`) ; `properties.rs` (`declared_refs`, `references` et
    `summarised` partagés) ; `UNSUMMARISED` sans `DiffObject` ni `DiffItemDto.object` ;
    contrat et `schema.d.ts` des deux SDK régénérés ; design 006 « Routes »
    (différentiel) ; `web/v2/shell/src/changesets/model.ts` (`diffGroups` : titre d'un
    groupe par le `label` servi, une valeur résumée lue par son libellé, sinon son
    identifiant, test `shell/test/changesets.test.ts`). Tests : `rest/tests/it/cycle.rs`
    `the_diff_leaves_alike_an_unreadable_target_and_an_unknown_one` (lecteur, lecteur
    sans les entités, lecteur sans les préfixes ; la cible effacée, le lecteur reçoit les
    octets et en-têtes de l'aveugle ; échantillon d'une simulation par l'administrateur
    et par un rédacteur qui ne lit pas les entités),
    `the_diff_of_an_object_closed_since_is_read_where_it_lived` (appelants à portée d'un
    domaine : celui du préfixe fermé depuis lit sa clé, l'autre `{id}` ; le préfixe
    effacé, mêmes octets).
    choix : une fermeture ne laisse aucune ligne au différentiel (seuls ses effets sur
    d'autres objets en laissent), d'où un objet absent lu d'abord tel que le changeset
    l'a laissé (`valid_from`), puis tel qu'il l'a trouvé (`valid_from` moins une
    microseconde, une cible qu'il ferme) ; la période n'est lue que si une page nomme un
    absent ; les déclarations `ref` lues aux dates de la route (une propriété déclarée
    depuis ou par le changeset même reste en identifiants nus) ; `DiffEntry.object`
    prend le schéma commun `Summary`, sans `kind` (`href` le dit).
    reste : `counts` (décompte, route et changeset lu, simulation) garde des
    identifiants nus pour une propriété `ref`, aucun libellé n'y fuit ; l'échantillon
    d'une simulation ne masque pas une propriété sensible (antérieur à la 33g2, à
    arbitrer : l'auteur voit-il les effets hérités d'une valeur sensible ?).
  - [ ] 33g3. Opérations rendues et inverse : `Ref` et `Operation` des opérations d'un
    changeset lu (telles qu'écrites), `Unsupported` de l'inverse — verify :
    `k-check -p kartenn-rest operations` (deux appelants, cible illisible et inconnue
    à l'identique)
    - où : `crates/kartenn-rest/src/operation.rs` (`RefDto`, `schema(as = Ref)`),
      `cycle.rs` (`operations` ≈ 353, `Unsupported` ≈ 1068) ; test voisin
      `rest/tests/it/changesets.rs`
  - [ ] 33g4. Placements et sections : `Placement` (simulation, objet et parents avant et
    après) et `SectionRowDto.object` (sections des modules, simulation et
    `?section=`) — verify : `k-check -p kartenn-rest placements diff_sections`
    - où : `crates/kartenn-rest/src/changesets.rs` (`Placement`, `SectionRowDto` ≈ 457,
      `mask_rows`), `cycle.rs` (`section_diff`) ; tests voisins
      `rest/tests/it/placements.rs`, `diff_sections.rs`
  - [ ] 33g5. Violations et décisions : `ViolationDto.subjects`, `PendingReason.objects`,
    `Required.objects` — verify : `k-check -p kartenn-rest changesets` (sujet illisible
    et inconnu à l'identique)
    - où : `crates/kartenn-rest/src/changesets.rs` (`ViolationDto` ≈ 268, `violations`,
      `PendingReason` ≈ 699), `cycle.rs` (`Required` ≈ 801) ; test voisin
      `rest/tests/it/changesets.rs`
- [x] 33i. Historique d'un objet de module : `parent` et `domain` de `ModuleObject` lus au
  début de chaque version, comme la jointure de l'historique du noyau
  (`versions_page`, `l.valid_period @> lower(h.valid_period)`), et non aux dates de la
  route (33f3) : un lié supprimé depuis garde sa clé dans les versions où il vivait —
  verify : `k-check -p kartenn-rest vlan` (historique d'un VLAN dont le groupe est
  renommé puis supprimé : chaque version résume le groupe tel qu'il était)
  - où : `crates/kartenn-rest/src/modules.rs` (`history`, `links_of`),
    `crates/kartenn-rest/src/summary.rs` (`keys`, une lecture par date de début ou une
    clé lue par la requête de la page), `crates/kartenn-store-pg/src/list.rs`
    (`versions_page`, `vlan_history_page`, `module_history_page`) ; test voisin
    `rest/tests/it/vlan/rest.rs`
    `the_history_leaves_alike_an_unreadable_parent_and_an_unknown_one`
  - dépend de : 33f3
  fait : `kartenn-store-pg` `keys.rs` (`natural_keys_at` : les clés du noyau de toutes
  les paires (identifiant, date) en une requête, `unnest … WITH ORDINALITY` et jointure
  latérale par genre ; `natural_keys` inchangée, ses genres en `KEYED` partagés),
  `temporal.rs` (`snapshot_at`, `snapshot` à une date donnée par une expression) ;
  `kartenn-rest` `summary.rs` (`DatedSummaries` : les paires lues par `natural_keys_at`,
  les clés des modules par `module_keys`, une lecture par date qui lie un objet de
  module, la lisibilité, maintenant, demandée une fois par genre pour toutes les dates et
  tenue une fois ; `of_module`, `module_keys`, `summary_of`, `readable_among` partagés
  avec `Summaries`) ; `modules.rs` (`history` lit les liés de chaque version à son
  `valid_from`) ; design 006 « Représentations », Historique : les liés au début de chaque
  version, et l'écart avec le noyau sous `as_of`. Tests : store
  `keys_at_their_own_dates_in_one_query` (une requête pour des paires de dates
  différentes, renommage suivi, paire absente omise, `as_of` avant l'assertion : rien) ;
  `rest/tests/it/vlan/rest.rs` `the_history_summarises_the_group_as_it_was` (groupe
  renommé, VLAN déplacé, premier groupe clos : `edge`, `edge`, `border`, `border`,
  `core` sur les deux axes pour un lecteur des groupes, `{id}` pour un lecteur des seuls
  VLAN ; correction rétroactive du second slug : `border` à `as_of` d'avant, `frontier`
  maintenant ; lecteur à portée sur l'entité de `lab`, le premier groupe étant de `far` :
  `{id}`, mêmes octets une fois le groupe effacé) ; `rest/tests/it/modules.rs`
  `the_history_keys_a_parent_of_a_module_at_each_version` (le parent, objet de module,
  résumé par la clé de son module lue au début de chaque version ; `Kit::keyed`) ;
  `the_history_leaves_alike_an_unreadable_parent_and_an_unknown_one` inchangé et vert.
  choix : l'axe des assertions est celui de la route (`moment.as_of`) ; le groupe
  « supprimé » du test est clos par SQL en temps de validité, aucune opération ne fermant
  un groupe de VLAN ; les paires rendues par leur rang (`WITH ORDINALITY`) plutôt que par
  la date relue de la base.
  reste : un lié de genre module coûte une lecture par date de début distincte qui en
  lie un (au plus une par version de la page, `page_size` jusqu'à 1 000), faute d'une
  lecture des clés d'un module à plusieurs dates (`ModuleHandler::keys` prend une
  `ModuleRead` à une date) ; aligner l'historique du noyau (`versions_page`, jointure sur
  la table courante) sur la lecture bitemporelle des modules sous `as_of` (design 006,
  Historique) ; un objet clos est jugé lisible d'après son dernier placement (un lecteur
  à portée sur l'entité de son domaine voit sa clé), comme la porte de son propre
  historique.
- [ ] 33h. Restes de la revue de la 33a — le schéma `Summary` du contrat dit
  `["string","null"]` alors que `slug` et `href` sont omis et jamais `null` ; H23 ne dit
  pas qu'un lié masqué n'est pas un refus de droit au sens de REQ-012 (« tout refus de
  droit est audité ») : l'écrire dans H23, ou journaliser — verify : `k-check -p
  kartenn-rest contract`
  - où : `crates/kartenn-rest/src/summary.rs` (27-31, 83), `specs/006-api-rest/decisions.md`
    (H23)
- [ ] 34. Cache des réponses qui dépendent des droits (sécurité, antérieur à la 33a,
  relevé par sa revue) : `FROZEN` (`private, immutable`, un an, `dated.rs:30`) s'applique
  à des corps qui dépendent des droits de l'appelant « maintenant » (porte de l'objet,
  lot de la 31, résumés de la 33a), et aucune réponse ne porte `Vary` sur
  l'authentification. Après un retrait de droit, le navigateur garde l'ancien corps un
  an ; sur un navigateur partagé, un second utilisateur peut recevoir du cache le corps
  calculé pour le premier. À arbitrer avant le code (contrat de cache, H-décision) :
  borner ou retirer `immutable` pour tout corps dépendant des droits, clé de cache liée
  au sujet, purge à la déconnexion côté shell — verify : `k-check -p kartenn-rest` (deux
  appelants, même URL datée : aucun en-tête ne permet à un cache de servir à l'un le
  corps de l'autre ; un droit retiré ne laisse pas de corps servi sans revalidation)
  - où : `crates/kartenn-rest/src/dated.rs` (30-34, 340), `specs/006-api-rest/decisions.md`
    (décision de cache, ligne ≈ 442), `web/v2/sdk/src/data/cache.ts` (déconnexion)
- [x] 35. Oracle des filtres par clé d'un objet lié (antérieur, relevé par la 33a) :
  `domain=<slug>` et `parent=<slug>` répondent sur la clé d'un lié que l'appelant ne peut
  pas lire — un filtre sur un lié illisible doit répondre comme sur un lié inconnu.
  L'oracle de `GET /v1/interfaces?address=<uuid>` (relevé par la 32b) est fermé par la
  32e : une interface n'est appariée que par une adresse demandée lisible, à une valeur
  comme à 64 — verify : `k-check -p kartenn-rest` (deux appelants, liste filtrée par la
  clé d'un domaine illisible et par une clé inconnue : mêmes octets)
  - où : `crates/kartenn-rest/src/objects.rs` (`declare_filters`, résolution de `domain`
    et `parent`), `crates/kartenn-rest/src/filter.rs` ;
    `crates/kartenn-machines/src/rest.rs` (`holds_requested`, sous `address_visible`, à
    imiter)
  fait : `crates/kartenn-store-pg/src/list.rs` (`objects_sql` : le filtre `$9` sur le
  slug du lié `l` porte le prédicat de lecture de `l` sous la portée `s` à `$1` ;
  `Kind::linked`, `Kind::readable(row)` remplace `Shape::readable`, partagé par la page,
  le lien et `count`) ; `crates/kartenn-rest/src/objects.rs` (`filtered` : le domaine
  d'un filtre de propriétés se résout sous la portée de l'appelant, plus sous
  `ReadScope::all()`) ; design « Objets du noyau » complété. Routes couvertes : `GET` et
  `QUERY` de `/v1/prefixes`, `/v1/ranges`, `/v1/addresses` (`domain`, avec ou sans
  `prop.*` ou `q`), `/v1/entities`, `/v1/sites` (`parent`). Test :
  `rest/tests/it/objects.rs` `a_filter_on_an_unreadable_link_answers_as_on_an_unknown_one`
  (rouge avant : `domain=lab` illisible rendait le préfixe et un curseur) — statut,
  en-têtes, corps et curseur identiques pour un lié illisible et une clé inconnue, un lié
  lisible filtrant toujours.
  choix : lisibilité du lié évaluée aux dates de la lecture (`$1`, comme les lignes),
  non « maintenant » comme le résumé (`Readable`) ; prédicat posé sur la jointure
  existante plutôt qu'un CTE des liés résolus, pour ne pas changer le plan (aucun banc).
  reste : un appelant qui lit des préfixes sans lire leur domaine ne peut plus les
  filtrer par ce domaine (voulu) ; les filtres par identifiant d'un lié des modules
  (`/v1/inference-rules?domain=<uuid>`, `parent=<uuid>`) ne sont pas touchés : l'identifiant
  y figure déjà dans les lignes lisibles ; coût du prédicat non mesuré.
- [x] 36. Page des flux coupée avant le filtre de lecture (sécurité, antérieur à la 32c,
  relevé par sa revue) : sans `id`, `/v1/flows` (et `/{collection}/{id}/flows`) lit la
  page et une ligne de plus, coupe la page et calcule son curseur sur les lignes brutes,
  puis seulement écarte les flux que l'appelant ne lit pas (`flows_seen`,
  `reviews_seen`). Le dernier rang brut peut être un flux illisible : le curseur, signé
  mais lisible (`cursor.rs`, en tête : « readable, not forgeable »), en livre l'`f.id` ;
  les pages sont courtes ou vides avec une suite, ce qui trahit le nombre de flux
  illisibles entre deux flux lisibles. À faire : le prédicat de lecture dans la requête
  (sous `flow.read` par le prédicat du noyau sur l'entité, comme demandeur, ou par une
  extrémité lue, REQ-027-1), la page et son curseur calculés sur les seules lignes
  lisibles, comme les autres listes à SQL propre — verify : `k-check -p kartenn-flow`
  (deux appelants, l'un lisant tous les flux, l'autre une partie, page après page : les
  pages du second sont celles du premier privées des flux qu'il ne lit pas ; aucun
  curseur ne porte l'identifiant d'un flux illisible ; aucune page courte avec une suite,
  ni page vide avec une suite)
  - où : `crates/kartenn-flow/src/rest.rs` (`page_of`, la coupe et le curseur avant
    `flows_seen` et `reviews_seen`, vers les lignes 946-1000 ; `visible`,
    `through_endpoints`, `endpoint_readable`, à porter dans la requête),
    `crates/kartenn-flow/src/read.rs` (`flows`, `applied`, `pending`) ;
    `crates/kartenn-kube/src/rest/mirrors.rs` (`page_sql`, prédicat `readable` sous les
    filtres, à imiter) ; tests `flow/tests/it/rest.rs`
    (`pending_requests_are_paged_after_the_applied_flows`, à imiter)
  - dépend de : —
  fait : `crates/kartenn-flow/src/read.rs` (`Reader` : l'appelant, sa portée en JSON de
  `kartenn_scopes`, la condition SQL de lecture d'un flux `f` à `$1` sous `s`, les
  entités de ses demandes en attente ; `FlowFilter::reader` ; `applied` matérialise `s`
  et garde, avant `ORDER BY … LIMIT`, ses propres demandes ou la condition ; `pending`
  écarte les demandes illisibles dans le même passage que `passes`, avant la coupe ;
  `of_object` prend le lecteur) ; `crates/kartenn-flow/src/rest.rs` (`reader_of` : la
  condition du noyau sur l'entité (`flow_readable`, partagée avec `visible`) ou, `flow.read`
  tenu ailleurs et tout objet filtré lu, une extrémité lue (`endpoint_readable`) ;
  `page_of` coupe la page et calcule son curseur sur les seules lignes lues, plus de
  filtrage après coup ; `reviews_seen` retiré). Test : `flow/tests/it/rest.rs`
  `a_page_of_flows_is_cut_on_the_rows_the_caller_reads` (rouge avant : page vide avec une
  suite) — deux auteurs en alternance, flux appliqués et demandes en attente, un lecteur
  de tout : les pages de l'auteur sont celles du lecteur privées des lignes de l'autre,
  aucune page courte avec une suite, aucun curseur ne nomme une ligne de l'autre, sur
  `/v1/flows` (avec et sans `requested`) et `/v1/addresses/{id}/flows`.
  choix : le lecteur porte une condition SQL construite par les routes (`readable` vit
  dans `kartenn-store-pg`, dépendance optionnelle de la crate, absente de `read.rs`) ; la
  requête lit le demandeur dans `flow.flow` courant, comme `visible` ; les demandes en
  attente gardent leur filtrage en mémoire, déjà fait avant la coupe sur toutes les lignes
  lues, désormais avec le lecteur (mêmes règles que `reviews_seen`) ; `flow.review` lu
  seulement si la liste lit les demandes en attente ; alias du résumé `s` renommé `sm`
  (le CTE des portées s'appelle `s`) ; anneau de clés partagé par les instances de test
  pour lire les curseurs servis.
  reste : `GET /v1/flow-nonconformities` coupe encore sa page avant le filtre des droits
  (« The key of the last row read, before the filter of the rights (design 011) ») : même
  défaut (pages courtes, curseur portant la clé d'une non-conformité illisible), à
  traiter par sa propre tâche (37) ; coût du prédicat dans la requête non mesuré (aucun
  banc).
- [x] 37. Page des non-conformités coupée avant le filtre des droits (sécurité, antérieur,
  relevé par la 36) : `GET /v1/flow-nonconformities` lit sa page, la coupe et calcule son
  curseur sur les lignes brutes (« The key of the last row read, before the filter of the
  rights (design 011) »), puis écarte ce que l'appelant ne lit pas. Les pages sont courtes
  ou vides avec une suite, et le curseur, lisible, peut porter la clé d'une non-conformité
  illisible. À faire : le prédicat de lecture dans la requête, avant la coupe, comme la 36
  (`read::Reader`), la page et son curseur calculés sur les seules lignes lues ; le design
  011 corrigé — verify : `k-check -p kartenn-flow` (deux appelants, l'un lisant tout,
  l'autre une partie, page après page : les pages du second sont celles du premier
  privées de ce qu'il ne lit pas ; aucune page courte avec une suite ; aucun curseur ne
  nomme une ligne illisible)
  - où : `crates/kartenn-flow/src/rest.rs` (route `flow-nonconformities`, vers la ligne
    1351, sa coupe et son curseur) ; `crates/kartenn-flow/src/read.rs` (`Reader`, à
    réemployer) ; test voisin `flow/tests/it/rest.rs`
    `a_page_of_flows_is_cut_on_the_rows_the_caller_reads` ;
    `specs/011-registre-flux/design.md` (pagination des non-conformités)
  - dépend de : 36
  fait : `crates/kartenn-flow/src/read.rs` (`NonconformityFilter::reader` ; la requête
  matérialise les portées `s` et garde, avant `ORDER BY … LIMIT`, les lignes de ses
  demandes ou la condition du lecteur ; alias `x` → `n`, `s` du résumé → `sm`) ;
  `crates/kartenn-flow/src/rest.rs` (`nonconformity_reader` : `flow.read` sur l'entité de
  la ligne, comme `readable_entities`, ou, `flow.read` tenu ailleurs, une extrémité lue du
  flux à l'une de ses versions, comme `through_endpoints` ; la route coupe la page et son
  curseur sur les seules lignes lues ; `readable_entities` et `through_endpoints`
  retirés) ; `specs/011-registre-flux/design.md` (décisions, pagination des
  non-conformités). Test : `flow/tests/it/rest.rs`
  `a_page_of_nonconformities_is_cut_on_the_rows_the_caller_reads` (rouge avant : page vide
  avec une suite) — demandes rejetées de deux demandeurs sur deux entités en alternance ;
  un lecteur d'une entité, et un demandeur lecteur de l'autre, en pages de 1 à 3 : leurs
  pages sont celles d'un lecteur de tout privées de ce qu'ils ne lisent pas, aucune page
  courte avec une suite, aucun curseur ne nomme une ligne illisible.
  choix : `read::Reader` réemployé tel quel, sa condition nommant la ligne `n` (doc
  élargie ; `every_review` et `review_entities` sans objet ici) ; mêmes règles qu'avant,
  déplacées dans la requête (l'entité par les portées, sans le prédicat du noyau sur le
  domaine) ; aide `Env::grant` des tests pour donner un droit à une personne existante.
  reste : coût du prédicat dans la requête non mesuré (`bench-flow`, non lancé) ; le
  design 011 (« Non-conformités ») dit encore « demandeur de la ligne avec
  `flow.request` », alors que la route ne vérifie pas `flow.request` (antérieur, inchangé ;
  tâche 38).
- [x] 38. Lecture de ses propres demandes sans `flow.request` (droits,
  antérieur, relevé par la revue de la 37) : REQ-011-17 et le design 011 (« Demandes »,
  « Non-conformités ») rangent la lecture de ses propres demandes sous `flow.request` ;
  deux routes acceptent le demandeur sans le vérifier, `GET /v1/flows` (`read.rs`,
  `Reader`, condition « as its requester ») et `GET /v1/flow-nonconformities`. Scénario :
  une personne qui a perdu `flow.request` lit encore ses demandes rejetées (motif, texte
  de la règle) et ses flux. Portée faible : ses seules lignes. Options : A, le code suit
  la spec (le demandeur ne lit ses lignes que s'il tient encore `flow.request`) ; B, la
  spec suit le code (l'auteur d'une demande en garde la lecture, REQ-011-17 et le design
  amendés) — verify : `k-check -p kartenn-flow` (une personne sans `flow.request` : ses
  lignes servies ou non selon l'option, réponses égales à « absent » si non servies)
  - où : `crates/kartenn-flow/src/read.rs` (`Reader::reads`, `Reader::condition`),
    `crates/kartenn-flow/src/rest.rs` (`reader_of`, `nonconformity_reader`) ;
    `specs/011-registre-flux/requirements.md` (REQ-011-17),
    `specs/011-registre-flux/design.md` (lignes 140 et 189) ; tests
    `flow/tests/it/requests.rs`
  - dépend de : arbitrage de l'utilisateur (droits)
  arbitrage (2026-11-15) : option A, le code suit la spec (REQ-011-17, design 011) : le
  demandeur ne lit ses propres lignes que s'il tient encore `flow.request`.
  fait : `crates/kartenn-flow/src/read.rs` (`Reader::requested_on`, `Reader::push_own`,
  `read::requested_on` ; `Reader::reads` pour les demandes en attente, la requête des flux
  appliqués et celle des non-conformités) ; `crates/kartenn-flow/src/rest.rs`
  (`requested_on`, servi à `reader_of`, `nonconformity_reader`, `visible` — donc
  `flows_seen` et le test d'une connexion — et `access` — `GET /v1/flows/{id}` et
  `/evaluations`) ; `crates/kartenn-flow/src/stats.rs` (compteur des flux de l'accueil,
  `GET /v1/stats`) et `kartenn-service` `StatsScope::subject` (au lieu de `identity`),
  rempli par `kartenn-rest/src/stats.rs` ; `handler::REQUEST` ; design 011 (« Demandes »,
  « Non-conformités »). Test : `flow/tests/it/rest.rs`
  `a_requester_without_flow_request_reads_its_rows_as_absent` (rouge avant : le demandeur
  privé du droit lisait son flux) — flux appliqués, demandes en attente et rejetées d'un
  demandeur sur `acme` et d'un autre sur `beta`, en alternance ; le premier perd
  `flow.request` : sur `/v1/flows` (avec et sans `requested`), `/v1/addresses/{id}/flows`
  et `/v1/flow-nonconformities`, en pages de 1 et 2, ses corps (lignes, curseurs, nombre)
  égalent ceux d'une personne aux mêmes autres droits ; par identifiant, 404 comme elle ;
  `GET /v1/stats` : mêmes compteurs ; `flow.read` sur `acme` lui rend ses lignes. Le test
  de la 37 donne désormais `flow.request` au demandeur.
  choix : portée du droit sur l'entité de la ligne (REQ-011-17 : « portées sur l'entité du
  flux ») — l'entité de la demande en attente, de la version du flux dont il est demandeur,
  de la ligne de non-conformité ; demande en attente sans entité lue par son demandeur
  seulement s'il tient `flow.request` partout ; droit lu à l'instant de la requête (comme
  la portée de lecture), borné par le confinement de l'émetteur ; constante
  `handler::REQUEST` réemployée par `POST /v1/flows`.
  reste : aucun parcours du shell ne lit ses demandes sans le droit (maquettes seules) ; le
  compteur de l'accueil se met en cache par portée de lecture et par sujet : un retrait de
  `flow.request` s'y voit au recalcul suivant (révision), non mesuré (tâche 39).
- [x] 39. Compteur de l'accueil après un droit retiré, et tests manquants de la 38 (relevés
  par sa revue) : la clé `Own` du cache de `GET /v1/stats` ne porte pas les entités où le
  sujet tient `flow.request` ; un retrait ne se voit qu'au recalcul suivant (au moins
  30 s, et seulement quand la révision de l'outbox avance), et une liaison qui arrive à
  son terme par la seule date, sans écriture, pourrait laisser le compteur faux jusqu'à
  la prochaine écriture ou au jour UTC suivant (à vérifier). La fuite se limite à un
  nombre. À faire : `requested_on` dans la clé `Own` ; tests de la 38 qui manquent : droit
  tenu sur une autre entité que celle de la ligne (refus partiel), confinement de
  l'émetteur, demande en attente sans entité (lue seulement avec `flow.request`
  partout) ; les deux commentaires de `rest.rs` (module, `flows_seen`) qui disent encore
  « as their requester » sans le droit — verify : `k-check -p kartenn-rest stats`,
  `k-check -p kartenn-flow` (un sujet dont le droit est retiré, ou dont la liaison est
  échue par la date : le compteur servi juste après égale celui d'un sujet sans le droit)
  - où : `crates/kartenn-rest/src/stats.rs` (clé `Own`, vers 584),
    `crates/kartenn-flow/src/stats.rs`, `crates/kartenn-flow/src/rest.rs`
    (`requested_on`, doc du module ligne 10, `flows_seen` vers 837) ; test voisin
    `flow/tests/it/rest.rs` `a_requester_without_flow_request_reads_its_rows_as_absent`
  - dépend de : 38
  fait : `kartenn-service` `StatsContributor::permissions` (méthode par défaut, vide) ;
  `kartenn-flow/src/stats.rs` la rend (`flow.request`) ; `kartenn-rest/src/stats.rs`
  `held` (là où le sujet tient ces droits à la requête, `rbac::visibilities_on`, en
  empreinte) dans `subject_key`, la clé `Own` ; commentaires de `kartenn-flow/src/rest.rs`
  (module, `flows_seen`) ; design 024 (« Instantanés »). Tests :
  `flow/tests/it/requester.rs` — `the_home_counter_follows_a_withdrawn_flow_request` et
  `the_home_counter_follows_a_binding_ended_by_its_date` (rouges avant : le compteur
  servi juste après le retrait, ou après la fin de la liaison par sa seule date, restait
  celui de l'instantané fait avec le droit ; le cas « à vérifier » est donc réel, et la clé
  le couvre : le droit se lit à la requête) ; `a_right_on_another_entity_reads_none_of_the_rows`
  (refus partiel), `the_confinement_of_the_issuer_bounds_the_rows_of_the_requester`,
  `a_request_naming_no_entity_is_read_with_the_right_everywhere` (verts d'emblée : le code
  de la 38 tenait).
  choix : un port générique (les modules déclarent les droits dont leurs compteurs
  dépendent) plutôt que `flow.request` codé dans `kartenn-rest` ; empreinte de la
  visibilité entière (portées et confinement, triés) plutôt que des seules entités ; une
  requête de plus par appel de `GET /v1/stats`, une seule pour tous les droits ; demande
  sans entité simulée par une référence locale (`{"local": 1}`) dans la proposition gardée,
  `POST /v1/flows` exigeant l'entité ; aucun corps ni contrat changé.
  reste : un recalcul en arrière-plan d'une clé relit les droits à son heure, pas ceux de
  la clé : un droit retiré puis rendu puis retiré dans les 10 min où la clé reste active
  peut resservir une fois le compteur fait avec le droit (fuite d'un nombre, fenêtre
  étroite ; fermer : porter l'empreinte dans l'origine et jeter un recalcul dont
  l'empreinte diffère) ; horloge du cache, hors de cette tâche : un compteur que la seule
  date change (flux échu, dérogation qui entre dans les 30 jours) attend une écriture ou
  le jour UTC suivant (proposition : un âge maximal d'instantané, quelques minutes, dans
  `Snapshots::due`). Les deux vont à la tâche 40.
- [x] 40. Instantanés des compteurs de l'accueil : recalcul lié aux droits de sa clé, âge
  maximal, droits du compteur `to_apply` (restes de la 39, relevés par sa revue) :
  (a) le recalcul d'arrière-plan d'une clé `Own` (`Own::compute`, par `contributions`)
  relit `flow.request` en direct, sans lien avec l'empreinte de la clé (`Origin` =
  `(Subject, ReadScope)`) : droit retiré (la clé « sans » naît), rendu dans les dix
  minutes, l'outbox avance, la clé « sans » est recalculée avec le droit, droit retiré à
  nouveau : elle sert le nombre fait avec le droit, une fois si l'outbox avance, jusqu'à
  l'écriture suivante ou au jour UTC suivant si la liaison finit par sa seule date — à
  faire : l'empreinte dans l'origine de la clé, un recalcul dont l'empreinte diffère est
  abandonné ; (b) un compteur que la seule date change (flux échu, dérogation qui entre
  dans les trente jours, liaison échue) attend une écriture ou le jour UTC suivant — à
  faire : un âge maximal d'instantané de quelques minutes dans `Snapshots::due` ;
  (c) à vérifier : la clé `Own` ne porte pas les droits dont dépend `proposals_for`
  (compteur `to_apply`) ; le retrait d'un droit d'application laisserait ce compteur
  périmé de la même façon — à faire : test rouge, puis ces droits déclarés par
  `StatsContributor::permissions()`. La fuite se limite à des nombres — verify :
  `k-check -p kartenn-rest stats`, `k-check -p kartenn-flow requester` (la séquence
  retiré, rendu, retiré sert le nombre sans le droit ; un flux échu par la date sort du
  compteur dans l'âge maximal, sans écriture ; un droit d'application retiré sort
  `to_apply` à la requête suivante)
  - où : `crates/kartenn-rest/src/stats.rs` (`Origin` vers 240, `Own::compute`, `held`,
    `subject_key`, `Snapshots::due`, `make_room` vers 323),
    `crates/kartenn-service/src/module.rs` (`StatsContributor::permissions`),
    `crates/kartenn-flow/src/stats.rs` (`proposals_for`) ; tests voisins
    `crates/kartenn-flow/tests/it/requester.rs` ; design 024 « Instantanés »
  - dépend de : 39
  fait : `kartenn-rest/src/stats.rs` — (a) l'origin d'une clé `Own` porte l'empreinte
  `held` de sa requête ; `Part::holds` dit si ces droits sont encore ceux du sujet :
  relu avant le calcul (à son `now`) et après (à un `now` relu) ; une empreinte autre que
  celle de la clé retire la clé — un recalcul ne lance pas `own()`, un premier calcul
  répond à sa requête sans être gardé ; (b) `Freshness::max_age` (5 min par défaut) dans
  `Snapshots::due` ; (c) `held` ajoute l'application (`verb_permission`, `Verb::Apply`)
  de ce qu'exigent les propositions en attente, `kartenn-store-pg/src/changesets.rs`
  `required_by_proposals` (une exigence qui n'est pas une liste n'en nomme aucune, garde
  `jsonb_typeof` dans l'appel) ; design 024 (« Instantanés »). Tests, rouges avant :
  `flow/tests/it/requester.rs` — `a_right_withdrawn_given_back_and_withdrawn_again_counts_without_it`,
  `a_key_whose_rights_moved_is_dropped_by_the_sweep`,
  `a_flow_ended_by_its_date_leaves_the_counter_within_the_age` ;
  `rest/tests/it/stats.rs` — `to_apply_follows_a_withdrawn_right_of_application` (le cas
  « à vérifier » de (c) était réel : `to_apply` restait à 1 après le retrait de
  `core.prefix.apply`) ; `store-pg/tests/it/changesets.rs` —
  `the_requirements_of_the_proposals_skip_what_is_not_a_list`.
  choix : (c) les droits de `to_apply` ne sont pas une liste fixe (une application par
  sorte d'objet, plus `prop.<clé>.apply` pour toute clé) ni ceux d'un module : lus des
  propositions en attente à chaque requête plutôt que déclarés par
  `StatsContributor::permissions()`, qui garde les droits des modules (validé à la
  revue) ; propositions de l'appelant comprises (en trop, sans effet) ; une clé retirée
  plutôt que marquée non due (sa prochaine lecture la recalcule en ligne) ; un recalcul
  non lancé ne compte pas dans `computations` ; âge maximal de 5 min, pour les deux
  niveaux ; aucun corps ni contrat changé.
  reste : séquence A-B-A dans un seul calcul — droit donné puis retiré pendant le calcul
  d'une clé « sans » : `held` avant et après vaut « sans », le compteur fait « avec » est
  gardé sous la clé « sans » jusqu'au recalcul suivant (fuite d'un nombre, fenêtre d'un
  calcul, INV-63). Aucun marqueur monotone ne couvre toutes les écritures de droits sans
  migration : l'outbox ne reçoit ni l'adhésion d'un compte de secours
  (`breakglass::create`) ni l'écriture directe en base, et sa révision avance à toute
  écriture (abandonner à chacune affamerait les recalculs sous charge). Pistes : un
  compteur d'écritures de droits (déclencheur sur `role`, `role_permission`, `binding`,
  `group_member`, `rbac_group` — migration) lu avant et après ; ou le calcul entier dans
  un seul instantané PostgreSQL (`pg_export_snapshot` partagé par les connexions de
  `own()`, `proposals_for` et des contributeurs — refonte de leurs lectures) ; à
  arbitrer. Test de la branche « premier calcul non gardé, clé retirée » : non écrit, le
  changement de droits entre la clé et le calcul ne se provoque pas sans point
  d'injection. `held` coûte 2 + P requêtes par appel de `GET /v1/stats` (propositions en
  attente, droits, et une lecture `sensitive` par permission `prop.*`), autant deux fois
  de plus par calcul d'une clé propre, non mesuré au banc (`bench-http`,
  REQ-024-16) ; une proposition qui arrive ou part change l'empreinte, et
  donc la clé, de chaque appelant (calcul en ligne au prochain appel) ; la description
  OpenAPI de `GET /v1/stats` (« at most every 30 s once something changed ») à compléter
  de l'âge maximal au prochain changement du contrat.
- [x] 41. (à arbitrer : migration ou refonte des lectures) La séquence A-B-A des
  instantanés des compteurs : un droit donné puis retiré pendant le calcul d'une clé
  laisse sous cette clé un compteur fait avec le droit (reste de la tâche 40, INV-63 :
  fuite d'un nombre, fenêtre d'un calcul, servi jusqu'au recalcul suivant, 5 min au
  plus). Deux voies : (A) un compteur des écritures de droits tenu par un déclencheur
  sur `role`, `role_permission`, `binding`, `group_member`, `rbac_group`, lu avant et
  après le calcul, toute écriture entre les deux abandonne — une migration, un coût
  d'une lecture par calcul, couvre aussi l'écriture directe en base et le compte de
  secours ; (B) le calcul entier dans un seul instantané PostgreSQL — pas de migration,
  mais refonte des lectures de `own()`, `proposals_for` et des contributeurs de modules
  — verify : test rouge d'abord, un droit donné puis retiré entre deux points
  d'injection du calcul ne laisse aucun compteur fait avec lui
  - où : `crates/kartenn-rest/src/stats.rs` (`Part::holds`, `Part::compute`, `held`),
    `crates/kartenn-store-pg/migrations/` (voie A), `crates/kartenn-service/src/breakglass.rs`
    (`create`) ; tests voisins `crates/kartenn-flow/tests/it/requester.rs`
    (`a_key_whose_rights_moved_is_dropped_by_the_sweep`)
  - dépend de : 40 ; arbitrage de l'utilisateur (schéma)
  - arbitrage de l'utilisateur (2026-11-16) : voie A, le compteur des écritures de
    droits tenu par déclencheur (une migration nouvelle).
  - fait (2026-11-17) : migration `0079_rights_writes.sql` — séquence
    `kartenn_rights_writes`, déclencheur par instruction (`AFTER INSERT OR UPDATE OR
    DELETE OR TRUNCATE … FOR EACH STATEMENT`) sur `role`, `role_permission`, `binding`,
    `rbac_group`, `group_member`, `issuer_attachment`, `property_declaration`, lu par
    `kartenn_rights_written()` (`rbac::rights_written`) ; `Snapshots::compute` le lit
    avant la première lecture des droits et après la dernière, pour une partie qui lit
    des droits à son moment (`Part::READS_RIGHTS` : `Own` ; `Shared` lit les siens de son
    origine) ; valeurs différentes ou `None` : corps non gardé, la clé garde son
    instantané (`Computed::Unsettled`, soumis comme avant à `holds` et à l'âge maximal) ;
    `None` à la première lecture : un recalcul ne tourne pas, un premier calcul répond
    sans être gardé mais prend une place de `Freshness::concurrent` comme les recalculs ;
    chaque cas compté (`Snapshots::unsettled`) et tracé (`stats.snapshot.unsettled`).
    Tests, rouges avant : `rest/tests/it/stats.rs` —
    `a_right_given_and_withdrawn_during_a_computation_is_not_kept` (module de test
    `probe` : son `StatsContributor`, trait déjà présent, donne le droit puis le retire au
    milieu du recalcul par le balayage ; `held` valait 1 sous la clé « sans »),
    `a_write_of_rights_in_flight_keeps_the_snapshot_of_the_key` (seconde lecture puis
    première refusées, premier calcul non gardé) ; `store-pg/tests/it/rbac_schema.rs` —
    `the_writes_of_rights_are_counted_in_flight_too`, `two_readers_of_the_count_read_it_both`
    (revue : la prise exclusive sans attente faisait se refuser deux lecteurs).
    choix : une séquence plutôt qu'une ligne compteur (une ligne mise à jour par
    déclencheur sérialise les transactions qui écrivent des droits jusqu'à leur
    validation ; `nextval` n'attend personne). `CACHE 1` : chaque `nextval` déplace la
    valeur stockée, lue hors instantané (une séquence n'est pas versionnée), donc vue dès
    le déclencheur, avant la validation. Écriture non validée à la seconde lecture : son
    déclencheur a tourné après la première lecture (valeur différente, non gardé, à tort
    peut-être mais sans fuite) ou n'a pas encore tourné (elle valide après la seconde
    lecture, donc après toutes les lectures du calcul : invisible de lui). Le cas que la
    séquence seule manque est l'écriture en cours à la *première* lecture (déclencheur
    passé, valeur déjà comptée, validation pendant le calcul) : deux écritures de droits
    en vol (une liaison ajoutée, une adhésion retirée) ou une seule dont la période
    finit entre `now` et `later` referaient l'A-B-A. D'où un verrou consultatif partagé
    pris par le déclencheur jusqu'à la fin de sa transaction (partagé entre écrivains :
    aucune attente entre eux), pris avant son `nextval` ; la lecture ne prend aucun
    verrou : elle lit la valeur, *puis* cherche ce verrou accordé dans `pg_locks` (même
    base). Une écriture comptée dans la valeur lue a pris son verrou avant : encore en
    vol, son verrou est vu et la lecture rend `NULL` (non gardé) ; finie, elle est validée
    avant la lecture des droits. Les lecteurs ne s'attendent ni ne se refusent entre eux,
    et ne font jamais attendre un écrivain (une prise exclusive, même sans attente, les
    faisait se refuser l'un l'autre et rendait `NULL` sans écriture ; une prise de session
    sérialisée par une seconde clé les aurait fait s'attendre). Coût : `pg_locks` lit la
    table des verrous de toute l'instance, deux fois par calcul d'une clé propre. Un
    corps non gardé pour un compteur qui a bougé garde l'instantané de la clé plutôt que
    de la retirer (ses droits sont ceux de la clé aux deux bouts : l'ancien corps reste
    juste, et des écritures de droits fréquentes n'évincent pas les clés). Déclencheur par instruction plutôt que par ligne (un `nextval` par
    instruction, une instruction sans ligne compte aussi : prudent). Tables : en plus des
    cinq de la tâche, `issuer_attachment` (le confinement) et `property_declaration` (la
    sensibilité d'un droit `prop.*`), lues par `visibilities_on` ; `breakglass::create`
    écrit `group_member`, compté. Les arbres d'entités, de sites et les préfixes portés
    par une liaison, objets écrits par les imports, ne sont pas comptés : ils font avancer
    l'outbox (instantané de nouveau dû) et les compter laisserait non gardé tout calcul
    pendant un import. Pas de lecture du compteur pour `Shared`.
    reste : une transaction longue qui écrit des droits (un import de liaisons) gèle les
    instantanés propres pendant sa durée (servis tels quels jusqu'à l'âge maximal) et
    fait calculer en ligne, dans la limite des places, chaque premier appel ; coût non
    mesuré au banc (`bench-http`, REQ-024-16) ; l'A-B-A d'un arbre d'entités ou de sites
    (une entité déplacée puis remise pendant un calcul) reste couvert par la seule
    révision de l'outbox (au plus 30 s). Revue : (a) une session qui tient la clé
    consultative en exclusif (`pg_advisory_lock(7737591478723635048)`, fonctions de
    verrou consultatif exécutables par PUBLIC, donc par `kartenn_app`) bloque toute
    écriture de droits, compte de secours compris, jusqu'à sa fin — piste : un
    `lock_timeout` local au déclencheur (`SET lock_timeout` de la fonction), qui ferait
    échouer l'écriture plutôt que l'attendre sans fin ; (b) une écriture directe dans les
    tables `*_history` n'est pas comptée : `kartenn_app` n'y a que `SELECT`
    (`kartenn_make_versioned`), seul leur propriétaire peut y écrire, et les vues
    `*_asserted` n'en lisent que les lignes de `sys_end = now()` ; (c) le module de test
    `Probe` lit le droit à `now_utc()` et non au `now` du calcul (`read.valid_at()`).
- [ ] 42. Références des corps des droits, hors du recensement de la 33 (relevées par la
  garde de la 33f3) : l'entité d'un rattachement d'émetteur (`AttachmentItem.entity`,
  `GET /v1/issuer-attachments`, identifiant nu) et la portée d'une liaison (`Scope`,
  `HeldScope` : `{kind, id}` d'une entité, d'un site, d'un domaine ou d'un préfixe, sans
  libellé) en résumés (`{id, label, href}`, `{id}` seul pour un objet illisible ou
  absent), ou une raison écrite de les laisser (lus par l'administration des droits,
  REQ-005) ; puis retirer leurs exemptions d'`UNSUMMARISED` — verify :
  `k-check -p kartenn-rest rights` (rattachement et liaison lus par deux appelants, l'un
  sans `core.entity.read` : corps identique au résumé coupé près ; entité supprimée :
  octets identiques), `k-check -p kartenn-rest contract`
  - où : `crates/kartenn-rest/src/rights.rs` (`AttachmentItem`, `attachment_item`,
    `Scope`), `crates/kartenn-rest/src/auth.rs` (`Scope`, schéma `HeldScope`) ;
    `summary.rs` (`Summaries::read`, à imiter : `vlan.rs` `AnchorSummary`, 33c) ;
    `rest/tests/it/contract.rs` (`UNSUMMARISED`) ; tests voisins `rest/tests/it/rights.rs`
  - dépend de : 33f3 ; la forme d'une portée lue (`kind` gardé à côté du résumé, ou omis
    avec `label` comme l'hôte d'une machine, 33d) touche le contrat des droits : à
    trancher avec l'utilisateur
- [ ] 43. (sécurité, avant la v0.2) La simulation et les réponses d'écriture masquent
  les propriétés sensibles comme le différentiel lu — REQ-005-21. Constat de la 33g2 et
  de sa revue (2026-11-17, par lecture, antérieur) : `POST /v1/changesets?simulate=true`
  et toute écriture simulée servent `diff` par `DiffCountDto::from` (`masked: None`,
  `changesets.rs` vers 1111) et `sample` depuis `diff_first`
  (`kartenn-store-pg/src/diff.rs` vers 298), sans `sensitive_keys` ni
  `sensitive_disclosed` ; les réponses appliquée et en attente de même (`counted`, vers
  993). Un auteur qui édite un objet sans `prop.<clé>.read` simule une opération qui
  change la valeur effective d'une propriété sensible et lit l'ancienne valeur dans
  `before` (de `counts` comme de `sample`), sans événement `data.sensitive.read`. Le
  différentiel lu (`GET /v1/changesets/{id}/diff`) masque, lui (`disclosed`,
  `cycle.rs` vers 762 ; `counts()`, vers 310), avec audit : s'en servir. À établir
  d'abord par un test rouge : quelle opération y donne accès sans droit `prop.<clé>`
  (héritage par déplacement ou rattachement, écriture sans lecture) — verify : test
  rouge par route (simulation, appliquée, en attente, rejouée) : valeur sensible
  `null` et marquée masquée sans `prop.<clé>.read`, servie et auditée avec ;
  `k-check -p kartenn-rest`
  - où : `crates/kartenn-rest/src/changesets.rs` (`sampled`, `counted`,
    `DiffCountDto`), `cycle.rs` (`disclosed`, `counts`), `kartenn-store-pg/src/diff.rs`
    (`diff_first`) ; tests voisins `crates/kartenn-rest/tests/it/cycle.rs`
  - versé par la revue de la 33g2 (à arbitrer ou écrire, pas des fuites) : le temps de
    réponse du différentiel distingue un objet illisible présent d'un absent (une
    lecture contre trois à cinq, `cycle.rs` 578 `diff_summaries`) — lire la période sans
    condition, ou l'écrire comme limite de REQ-006-42 ; les références de `counts` et
    de l'échantillon (texte JSON) échappent à la garde `UNSUMMARISED` — exemption à
    écrire dans la garde ; `declared_refs` (`properties.rs` 593) lit toutes les
    déclarations à chaque page ; `summaryText` (`web/v2/shell/src/changesets/model.ts`
    183) prend une valeur JSON `{id}` ordinaire pour un résumé

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
- [ ] 32. Filtre `id` répété sur toute collection, `address` répété sur la liste des
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
- [ ] 32c. Listes de kube et des flux — verify : `k-check -p kartenn-kube`,
  `k-check -p kartenn-flow` (deux identifiants dont un illisible : un seul rendu ;
  conjonction ; filtrée par `id`, une page sans suite, `page_size` ou curseur → 400, par
  `filter::window`)
  - où : `crates/kartenn-kube/src/rest/clusters.rs` (`list`), `rest/mirrors.rs`
    (`list<M>`), listes des correspondances de libellés, valeurs admises et libellés
    suivis (à repérer) ; `crates/kartenn-flow/src/rest.rs` (`page_of`, `list`, vers les
    lignes 901 et 1062)
  - dépend de : 32a
- [ ] 32d. Listes de la politique et garde du contrat : `id` sur la liste générique de la
  politique ; test du contrat : `id` aux `parameters` de chaque liste d'une collection
  d'objets, kube compris — verify : `k-check -p kartenn-policy` (filtrée par `id`, une
  page sans suite, `page_size` ou curseur → 400, par `filter::window`),
  `k-check -p kartenn-rest contract`
  - où : `crates/kartenn-policy/src/rest.rs` (`list`, vers la ligne 449),
    `crates/kartenn-rest/tests/it/contract.rs`
    (`every_collection_tells_the_clients_its_key_and_its_columns`, à imiter)
  - dépend de : 32a, 32b, 32c
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
- [ ] 33b. Anomalies, écarts et valeurs de type référence : `object` d'une anomalie
  (`anomalies.rs`) et d'un écart (`properties.rs`, `/v1/property-gaps`) avec sa clé
  naturelle ; valeur `ref` d'une propriété résolue, unitaire et par lots (31) — verify :
  `k-check -p kartenn-rest`
  - où : `crates/kartenn-rest/src/anomalies.rs` (`ObjectRef`), `properties.rs`
  - dépend de : 33a ; à trancher avant : nom du membre de la clé naturelle pour une
    clé composée (préfixe, plage, adresse : `domain` et `value` ; VLAN : groupe et
    `vid`) et pour un type d'un module
- [ ] 33c. VLAN : ancre et groupe d'un VLAN et d'un groupe — verify :
  `k-check -p kartenn-rest vlan`
  - où : `crates/kartenn-rest/src/vlan.rs` (`IdRef`, `Anchor`)
  - dépend de : 33a, 33b (forme de la clé)
- [ ] 33d. Machines : références des grappes, machines, interfaces, rattachements et
  fiche — verify : `k-check -p kartenn-machines`
  - où : `crates/kartenn-machines/src/rest.rs` (`IdRef`, `HostRef`)
  - dépend de : 33a, 33b
- [ ] 33e. Kube : grappes, miroirs, chaîne d'exécution, observations — verify :
  `k-check -p kartenn-kube`
  - où : `crates/kartenn-kube/src/rest.rs` (`IdRef`), `rest/clusters.rs`,
    `rest/mirrors.rs`, `rest/chain.rs` (`HostRef`)
  - dépend de : 33a, 33b
- [ ] 33f. Flux et politique, et garde du contrat (aucun schéma de réponse ne porte une
  référence `{id}` seule hors résumé) — verify : `k-check -p kartenn-flow`,
  `k-check -p kartenn-policy`, `k-check -p kartenn-rest contract`
  - où : `crates/kartenn-flow/src/rest.rs` (`FlowEndpoint`),
    `crates/kartenn-policy/src/rest.rs` (`PolicyEndpoint`, `PolicyPoint`,
    `PolicyAxisResolution`, `PolicyReportItem`)
  - dépend de : 33a–33e
- [ ] 33g. Changesets : le différentiel (`DiffObject {id}`), les opérations rendues
  (`RefDto`) et les valeurs de type référence dans `before`/`after`, démasquées
  aujourd'hui selon la lisibilité de l'objet et non de la cible (revue de la 33a : hors
  du recensement 33b–33f, et la garde du contrat de la 33f échouerait sur eux) — verify :
  `k-check -p kartenn-rest` (différentiel lu par deux appelants : cible illisible et
  cible inconnue rendues à l'identique, octets compris)
  - où : `crates/kartenn-rest/src/cycle.rs` (≈ 530, `DiffObject`),
    `crates/kartenn-rest/src/operation.rs` (`RefDto`), `summary.rs` (`Readable`)
  - dépend de : 33a, 33b
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
- [ ] 35. Oracle des filtres par clé d'un objet lié (antérieur, relevé par la 33a) :
  `domain=<slug>` et `parent=<slug>` répondent sur la clé d'un lié que l'appelant ne peut
  pas lire — un filtre sur un lié illisible doit répondre comme sur un lié inconnu. De
  même `GET /v1/interfaces?address=<uuid>` (relevé par la 32b) : sur une adresse que
  l'appelant ne lit pas, l'interface lisible qui la tient est rendue, ce qui révèle que
  l'adresse existe et à quelle interface elle est rattachée ; la répétition d'`address`
  permet d'en sonder 64 par requête, et une dichotomie sur les réponses non vides
  retrouve laquelle — verify : `k-check -p kartenn-rest` (deux appelants, liste filtrée
  par la clé d'un domaine illisible et par une clé inconnue : mêmes octets),
  `k-check -p kartenn-machines` (interfaces filtrées par une adresse illisible et par une
  adresse inconnue : mêmes octets)
  - où : `crates/kartenn-rest/src/objects.rs` (`declare_filters`, résolution de `domain`
    et `parent`), `crates/kartenn-rest/src/filter.rs`,
    `crates/kartenn-machines/src/rest.rs` (`list_interfaces`, condition sur `$7` ;
    `address_visible`, à imiter)

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
- [ ] 8. Couche CSRF (`Sec-Fetch-Site`, repli `Origin` = origine publique) —
  REQ-006-28 — verify : matrice complète `Sec-Fetch-Site` × `Origin` ; Bearer et `QUERY`
  non concernés

## P3 — Lectures

- [ ] 9. Lectures datées et cache : analyse stricte RFC 3339, `as_of` futur refusé,
  `transaction_timeout` sur le pool d'écriture, passé figé `immutable`, ETag fort
  (objet) et faible (page), 304 — REQ-006-5, 6 — verify : date sans décalage → 400 ;
  `immutable` seulement au-delà de `T` ; 304 puis 200 après un changeset
- [ ] 10. Curseur signé (`Purpose::Cursor`), pagination par clé sur les préfixes,
  `Link: rel="next"`, `page_size` 1 à 1 000, curseur seul avec `page_size` — REQ-006-7,
  8 — verify : octet modifié → 400 ; écritures entre deux pages sans saut ni répétition ;
  liaison retirée entre deux pages
- [ ] 11. Filtres par paramètres (opérateurs après `:`), traduction vers
  `kartenn_store_pg::filter`, route `QUERY` (`any()`, contrôle de méthode, `Allow` écrit,
  `Accept-Query`) — REQ-006-9, 10 — verify : mêmes éléments par `GET` et `QUERY` ;
  paramètre inconnu → 400 nommé ; `QUERY` sans `Content-Type` → 415 ; 405 et `Allow`
- [ ] 12. DTO et lectures des objets du noyau (entités, sites, domaines, préfixes,
  plages, adresses) avec résumés liés, historique, arbre, occupation, prochain libre,
  anomalies et acquittement — REQ-006-1, 2, 4, 23 — verify : clé naturelle en filtre ;
  renommage sans changement d'URL ; 404 identique pour invisible et absent
- [ ] 13. Lectures des propriétés : déclarations, valeurs, valeurs résolues,
  explication, suggestion, écarts ; lecture à un temps système passé — REQ-006-4 —
  verify : valeur sensible masquée sans droit ; explication au passé

## P4 — Écritures

- [ ] 14. `POST /v1/changesets` : simulation, 201/202/200, lien `rel` du changeset,
  422 et `violations` avec pointeurs JSON, priorité des problèmes — REQ-006-11, 21, 22
  — verify : changeset à deux violations de règles différentes → une réponse avec les
  deux ; non authentifié et malformé → 401
- [ ] 15. `Idempotency-Key` : verrou consultatif (409 en cours), rejeu reconstruit du
  journal, contenu différent → 422, purge par tâche (branchée en 20) — REQ-006-15 —
  verify : deux requêtes simultanées de même clé → une 201, une 409 ; rejeu égal octet
  pour octet ; autre identité → second objet
- [ ] 16. Routes par ressource : `POST`, `PATCH` (Merge Patch, `{"clear": true}`),
  `DELETE` ; membres réservés `motive`, `valid_from`, `valid_to` ; `If-Match` → 428, 412 —
  REQ-006-12, 13, 14 — verify : mise à jour perdue empêchée ; JSON Patch → 415 ; retrait
  et effacement de propriété distincts
- [ ] 17. Cycle d'un changeset : lecture, liste filtrée, différentiel paginé,
  validation, rejet, retrait, annulation (409 et conflits), propositions ; `If-Match`
  sur les actions — REQ-006-16 — verify : auteur qui valide → 403 ; annulation en
  conflit → 409 avec la liste
- [ ] 18. Administration des droits : rôles, liaisons, rattachements, comptes de
  service et secrets (réponse `Secret`, `no-store`), état des identités — REQ-006-4,
  32 — verify : délégué sans escalade (005) par HTTP ; secret jamais compressé
- [ ] 19. Audit et exploitation : recherche et export (`GET`, `QUERY`),
  pseudonymisation, filtres de log à chaud — REQ-006-4 — verify : consultation sans
  permission refusée et auditée ; filtre qui revient à la configuration

## P5 — Tâches de fond et imports

- [ ] 20. Migration `job`, planificateur sous bail (scrutation 5 s, bail 30 s, battement
  10 s), tâches `expire-pending`, `reevaluate-expected`, `public-drift`,
  `idempotency-purge` ; minuteries locales (caches, limiteurs, JWKS) — REQ-006-38 —
  verify : trois planificateurs sur une base, une exécution par échéance ; détenteur
  arrêté → reprise après le bail
- [ ] 21. Imports : migration `import_chunk` et état `uploading`, `PUT` des morceaux
  (identique ignoré, différent → 409, > 10 000 → 413), `start` (202), `resume`,
  `cancel`, état et `Retry-After`, exécution comme tâche — REQ-006-17, 18, 19 — verify :
  10⁶ éléments en 100 morceaux dont un renvoyé ; arrêt d'une instance pendant
  l'exécution, fin sur une autre sans doublon

## P6 — Contrat, bout en bout, budgets

- [ ] 22. Contrat OpenAPI 3.1 (utoipa, `x-query` construit à la main), `GET
  /v1/openapi.json`, `GET /v1/capabilities`, contrat de référence
  `specs/006-api-rest/openapi-v1.json` et comparateur — REQ-006-4, 33, 34, 35 — verify :
  toutes les opérations de REQ-006-4 présentes ; un champ retiré fait échouer le test ;
  `Deprecation` et `Sunset` sur une route de test retirée
- [ ] 23. Matrice des erreurs : un test par ligne du tableau du design — REQ-006-20 à
  23 — verify : statut, `type`, `Request-Id`, aucune valeur sensible
- [ ] 24. Vrai TCP et TLS : `QUERY` en HTTP/1.1 et h2, cookies et `Forwarded` de bout en
  bout — REQ-006-10, 27, 32 — verify : tests d'intégration sur un port local
- [ ] 25. Banc `bench-http` : scénarios de `bench-core` à travers HTTP, charge de
  200 utilisateurs et 50 écritures/s — REQ-006-39 — verify : p95 sous INV-30 à 34 et
  INV-36, surcoût rapporté par rapport au banc du service
- [ ] 26. Synchronisation de la spec système : `docs/spec/interfaces.md` (section API
  REST, Q-ouv-7 close), `invariants.md`, ADR 0022 et 0023 acceptés, `specs/README.md`
  — verify : `spec-keeper` sur `specs/006-api-rest`

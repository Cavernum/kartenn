---
status: implemented
updated: 2026-10-18
---

# 005 — Authentification et RBAC : tâches

Chaque tâche est un commit, laisse le build vert et nomme ce qu'elle implémente. Les
routes HTTP (connexion, rappel, déconnexion, protection CSRF) sont la 006 : la 005 fournit
les fonctions qu'elles appelleront et les teste par des appels directs.

## P1 — Modèle de contrôle d'accès

- [x] 1. Catalogue des permissions et rôles livrés (`kartenn_core::authz`), publié dans
  `docs/spec/permissions.md` — REQ-005-16, 17 — verify : test de fraîcheur
  (`KARTENN_BLESS`), noms uniques et bien formés, rôles ⊆ catalogue — motifs de rôle
  `prop.*.<action>` (propriétés non sensibles seulement) et `*` ; `core.property.admin`
  ajoutée (déclarations et sensibilité)
- [x] 2. `Subject` (identité, émetteur, groupes) dans le port `Authorizer` ; `Grants`,
  `Unrestricted` et tous les appelants ; `core.changeset.approve` ; instantané des
  groupes de l'auteur — REQ-005-18, 22 — verify : tests existants verts, test de
  revalidation avec les groupes de l'auteur — l'instantané est le champ `author` du
  `Pending` (JSON, sans migration) ; `RequestContext` porte les groupes ;
  `validate_pending`, `reject`, `proposals_for` reçoivent un `Subject` ; `Grants`
  accorde aussi par groupe (`review.rs::an_approval_checks_the_author_with_the_groups_it_had`)
- [x] 3. Migration `0028_rbac.sql` : identités (`kind`, `disabled_at`, `revoked_after`),
  `role`, `role_permission`, `binding`, `issuer_attachment` versionnées, `service_secret`,
  `breakglass_account` ; fonctions `kartenn_owner`, `kartenn_site` — REQ-005-3, 11, 14,
  18, 20 — verify : tests de schéma (contraintes, droits du rôle applicatif) —
  `kartenn-store-pg/tests/rbac_schema.rs`, `kartenn-service/tests/rbac.rs`
  (propriétaire et site de chaque sorte d'objet) ; identité `system` pour l'auteur
  système
- [x] 4. `PgAuthorizer` : visibilité d'un sujet (liaisons d'identité, de groupe,
  d'émetteur ; descendants ; confinement) et `authorize_many` par le prédicat — REQ-005-18
  à 20, 23 — verify : tests sur un petit arbre (chaque périmètre, confinement, émetteur
  sans rattachement, objet sans propriétaire, liaison échue) — `kartenn_store_pg::rbac`,
  prédicat `kartenn_visible` (migration 0029) ; le test a trouvé un défaut qui laissait
  passer : pour un objet sans propriétaire (un site), le prédicat valait `NULL`, lu comme
  « non refusé » ; il ne vaut plus jamais `NULL` (`kartenn-service/tests/rbac.rs`)
- [x] 5. Filtrage des listes et masquage des valeurs sensibles — REQ-005-21, 24 —
  verify : test de propriété décision unitaire = filtrage ; lecteur sans
  `prop.<clé>.read` — `ReadScope` et `kartenn_readable` (migration 0030), `filter_in`,
  `children_in` ; `read_object` reçoit l'autorisateur, répond absent sans droit (refus
  audité) et liste les valeurs masquées (`ObjectRead::masked`) ; vérification exhaustive
  sur les six périmètres du petit arbre plutôt qu'aléatoire (`rbac.rs`)
- [x] 6. Droits d'écriture par type d'objet dans le pipeline (objet ou parent),
  propositions — REQ-005-22 — verify : éditeur d'une entité, proposition (004) — la
  décision se prend **dans la transaction du changeset** (`Authorizer::authorize_in`,
  `WriteTx::rbac_outside`) : par le pool, un objet créé par le changeset n'existait pas et
  aurait été refusé ; objets existants contrôlés avant les écritures (clos, ils ne sont
  plus valides), objets créés après (avec leur propriétaire hérité) ; règle
  `core.object.forbidden` ; les droits de propriété (003) passent aussi par la
  transaction (`rbac.rs`, tests 003 ajustés : `rights.rs`, `anchors.rs`)
- [x] 7. Opérations d'administration (rôles, liaisons, rattachements, comptes de service,
  état des identités) et règle anti-escalade ; synchronisation des rôles livrés —
  REQ-005-3, 17, 25, 26 — verify : délégation bornée (test de propriété), lecture datée
  des droits, rôles livrés non modifiables — `SetRole`, `CloseRole`, `Bind`, `Unbind`,
  `AttachIssuer`, `DetachIssuer`, `SetIdentityState` (`kartenn_service::rbac`,
  `Planned::Rbac`, `WriteTx::rbac_write`) ; règles `core.role.invalid`,
  `core.rbac.forbidden`, `core.binding.escalation`, `core.role.builtin`,
  `core.role.unknown` ; les entrées-motifs (`*`, `prop.*.write`) se vérifient comme
  telles ; `sync_roles` au job de migration ; matrice de délégation exhaustive plutôt
  qu'aléatoire (`rbac.rs`) ; les comptes de service passent à la tâche 10 (avec leurs
  secrets)
- [x] 8. Politiques d'approbation par périmètre et auteurs par rôle ou groupe —
  REQ-005-27 — verify : tests (`review.rs`) — `SetApprovalPolicy` reçoit `scopes`
  (entité, site, sous-arbre ; un domaine rejoint `domains`), `author_roles`,
  `author_groups` ; un seul déclencheur de périmètre (domaines ou portées) et un seul
  d'auteur (personne, rôle ou groupe) ; stockés dans les listes JSON existantes, sans
  migration ; évalués dans la transaction (`WriteTx::scope_touched`, `roles_of`)
  (`review.rs` unitaire, `rbac.rs::policies_hold_by_scope_and_by_role`)

## P2 — Authentification

- [x] 9. Crate `kartenn-auth` : trousseau, HKDF, cookie chiffré (AES-256-GCM, découpage) ;
  vérification des dépendances (sans aws-lc) — REQ-005-6 — verify : aller-retour, octet
  modifié refusé, clé retirée, autre instance — `keys::KeyRing` (`<id> <base64>` par
  ligne, la première écrit), `seal::{seal, open}` (données associées : nom du cookie et
  identifiant de clé ; nom ou usage différent refusé) ; RustCrypto (sha2 0.10, hmac,
  hkdf, aes-gcm), aucun aws-lc dans l'arbre
- [x] 10. Comptes de service : secrets générés, HMAC, Basic, rotation, expiration,
  limitation de débit — REQ-005-11 à 13 — verify : tests, coût < 1 ms — opérations
  `CreateServiceAccount`, `AddServiceSecret`, `RevokeServiceSecret` (venues de la tâche 7) ;
  `kartenn_auth::secrets::{generate, parse_basic, FailureLimiter}`, `KeyRing:
  SecretHasher` ; `kartenn_service::authn::authenticate_service` (une seule réponse de
  refus, la raison dans l'audit, l'identifiant revendiqué en empreinte HMAC sous la clé
  d'instance) ; un succès Basic n'est pas audité à part (la requête l'est, avec son
  acteur) ; le refus hors TLS et le branchement du limiteur reviennent à la couche HTTP
  (006), qui connaît le transport (`kartenn-auth/tests/service_accounts.rs`)
- [x] 11. Jetons d'accès RFC 9068 et clés des émetteurs (cache, rechargement borné) —
  REQ-005-9, 10 — verify : IdP de test en processus, chaque refus —
  `kartenn_auth::tokens::TokenVerifier` (`jsonwebtoken` 11, moteur RustCrypto, sans
  aws-lc) ; `kid` exigé ; l'émetteur nommé par le jeton choisit la configuration, la
  validation le revérifie signé ; seuls les rechargements causés par un `kid` inconnu
  sont bornés à un par minute (une rotation juste après le démarrage passe) ;
  `HttpClient` sur hyper-rustls (`ring`), HTTP seulement vers la boucle locale ; source
  de clés en mémoire dans les tests (`kartenn-auth/tests/access_tokens.rs`)
- [x] 12. Flux OIDC du BFF : début, rappel (`state`, `iss`, `nonce`, PKCE), identité
  juste-à-temps, session, rafraîchissement des groupes, déconnexion — REQ-005-1, 2, 4 à 8
  — verify : IdP de test ; refus de REQ-005-4, 5 — `kartenn_auth::oidc` écrit sur le client
  HTTP et `jsonwebtoken` plutôt que la crate `openidconnect` (le flux est court, ses
  contrôles listés par RFC 9700 et OIDC Core, une dépendance lourde de moins) ;
  `TokenVerifier::verify_id_token` (audience = le seul client, `azp`, `nonce` ; sans
  `nonce` pour le jeton d'un rafraîchissement, OIDC Core §12.2) ; `Session` scellée ;
  `authn::logged_in` crée ou met à jour l'identité et audite la connexion ; IdP HTTP dans
  le processus de test (`kartenn-auth/tests/oidc.rs`, `service_accounts.rs`)
- [x] 13. État des identités : désactivation, révocation des sessions, cache de 5 s —
  REQ-005-3 — verify : session coupée à la requête suivante — `authn::StateCache` ;
  l'instance qui applique le changement oublie l'entrée (coupure à la requête suivante),
  les autres au plus 5 s après ; refus audité `authz.access.denied`
  (`identity.disabled`, `session.revoked`) (`service_accounts.rs`)
- [x] 14. Comptes de secours : sous-commande, Argon2id, TOTP, session d'1 h, 100 échecs —
  REQ-005-14, 15 — verify : code rejoué refusé, rôle applicatif sans écriture —
  `kartenn-server breakglass create` (1 à 24 h, phrase et URI `otpauth://` affichées une
  fois), `kartenn_auth::breakglass` (Argon2id RFC 9106, TOTP RFC 6238 avec `last_step`),
  `authn::authenticate_breakglass` ; un compte inconnu calcule quand même un hachage
  (`burn`, pas d'oracle temporel) ; événements `authn.breakglass.used` et
  `account.breakglass.created` (criticité haute) ; la durée d'1 h de la session est
  appliquée par la couche HTTP (spec 006) (`tests/breakglass.rs`, `rbac_schema.rs`)
- [x] 15. Configuration `[auth]` et événements d'authentification — REQ-005-28 — verify :
  configurations invalides refusées ; événements de chaque refus — `config::Auth`
  (durées bornées, URL en https, identifiants uniques, algorithmes à clé publique
  seulement), `kartenn_server::auth::load` (trousseau et secrets lus au démarrage) ;
  `authn::Failure` porte l'émetteur ; raisons codées par `LoginError::reason` et
  `TokenError::reason`, un refus de jeton garde sa cause (`signature`, `expired`,
  `audience`, `nonce`) (`config.rs`, `auth.rs`, `tests/refusals.rs`)

## P3 — Budgets et synchronisation

- [x] 16. Banc : `bench-core` avec 1 000 liaisons et 50 groupes — REQ-005-29 — verify :
  ~~surcoût sous 10 %~~ budgets tenus avec contrôle d'accès (K15) — banc `bench-rbac`
  (jeu de `bench-core`, une personne, 50 groupes, 1 000 liaisons). Jeu de référence
  (2026-10-18), p95 avec contrôle (sans) / budget : lecture d'un objet 11,9 ms (1,3) /
  50 ms ; lecture datée 12,8 ms (3,6) / 23,7 ms (deux fois la lecture courante) ; page
  filtrée 47,6 ms (73,0) / 150 ms ; sous-arbre 13,7 ms (6,3) / 200 ms ; changeset
  unitaire 10,1 ms (3,4) / 200 ms. Le surcoût restant est la résolution des liaisons à
  chaque requête (≈ 8 à 10 ms, surtout les 980 préfixes des périmètres) ; les prédicats
  sont devenus négligeables (migration 0033)
- [x] 17. Synchroniser `docs/spec/`, ADR 0020 et 0021 acceptés, statut `implemented`,
  rapport spec-keeper — `interfaces.md`, `data.md`, `architecture.md`, `invariants.md`
  (INV-20, 21 corrigés ; INV-25 à 28), `README.md` ; ADR 0020 et 0021 acceptés, ADR 0009
  amendée (K8). Le rapport spec-keeper a mené à ces corrections : pas TOTP rejoué par
  deux requêtes simultanées, `SetIdentityState` sur un compte de secours, propriété non
  déclarée dans un rôle, événements `rbac.binding.changed`, `account.service.changed`,
  `authn.session.closed`, annulation muette des changesets d'administration, politique
  `bound_roles` (second administrateur) ; ce qui revient à la 006 est listé à la fin de
  `design.md`. Reste un arbitrage : REQ-005-25 « annulable » (tranché : K16)

## P4 — Annulation de l'administration (K16)

- [x] 18. Inverses des rôles, liaisons et rattachements d'émetteurs — REQ-005-25 —
  verify : annuler la création d'un rôle, d'une liaison et d'un rattachement rend l'état
  d'avant ; annuler une clôture de rôle rend le rôle et ses liaisons ; annuler la
  suppression d'une liaison future la recrée ; une liaison écrite depuis est un
  conflit ; l'inverse d'une liaison passe le contrôle d'escalade ; comptes de service
  toujours listés non annulables — `UndoFacts::rbac`
  (`kartenn-store-pg/src/undo.rs`, lignes supprimées retrouvées par la fin de leur
  historique à l'instant d'application), `undo::rbac_inverse` (`tests/undo.rs`,
  `tests/rbac.rs::an_undo_cannot_escalate`)

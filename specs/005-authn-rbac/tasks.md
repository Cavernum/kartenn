---
status: approved
updated: 2026-10-17
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
- [ ] 11. Jetons d'accès RFC 9068 et clés des émetteurs (cache, rechargement borné) —
  REQ-005-9, 10 — verify : IdP de test en processus, chaque refus
- [ ] 12. Flux OIDC du BFF : début, rappel (`state`, `iss`, `nonce`, PKCE), identité
  juste-à-temps, session, rafraîchissement des groupes, déconnexion — REQ-005-1, 2, 4 à 8
  — verify : IdP de test ; refus de REQ-005-4, 5
- [ ] 13. État des identités : désactivation, révocation des sessions, cache de 5 s —
  REQ-005-3 — verify : session coupée à la requête suivante
- [ ] 14. Comptes de secours : sous-commande, Argon2id, TOTP, session d'1 h, 100 échecs —
  REQ-005-14, 15 — verify : code rejoué refusé, rôle applicatif sans écriture
- [ ] 15. Configuration `[auth]` et événements d'authentification — REQ-005-28 — verify :
  configurations invalides refusées ; événements de chaque refus

## P3 — Budgets et synchronisation

- [ ] 16. Banc : `bench-core` avec 1 000 liaisons et 50 groupes — REQ-005-29 — verify :
  surcoût sous 10 %
- [ ] 17. Synchroniser `docs/spec/`, ADR 0020 et 0021 acceptés, statut `implemented`,
  rapport spec-keeper

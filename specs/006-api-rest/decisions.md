# 006 — API REST : journal des décisions

Entretien (grill-me) du 2026-10-18. Cadre : D7 (REST, seule frontière publique), D9
(instances sans état), D13 (OIDC et Basic sur TLS), D14 (budgets), ADR 0002, 0020 ;
briques des specs 002 à 005 et 012 ; ce que la 005 laisse à la couche HTTP (fin de
`specs/005-authn-rbac/design.md`). Crate cible : `kartenn-rest` (axum, utoipa ;
`docs/spec/architecture.md`).

## H1 — Périmètre
- Décision : **conventions transverses et toute la surface existante** : erreurs,
  pagination, dates, concurrence, idempotence, versions, et les routes de ce que
  fournissent 002 à 005 et 012 (objets, lectures datées, filtres, arbre, changesets et
  leur cycle, imports, propriétés, administration du RBAC, audit, authentification,
  sondes). Découpage en phases dans `tasks.md`.
- Pourquoi : v0.1 livre le Web, qui ne passe que par l'API (D7) ; les routes des specs
  précédentes n'ont pas d'autre propriétaire.
- Rejeté : routes du noyau seulement (le shell 007 buterait sur les trous) ; conventions
  seules (rien d'utilisable, routes sans propriétaire).
- Décidé par : recommandation acceptée

## H2 — Modèle d'écriture
- Décision : **changesets et routes par ressource**. `POST /changesets` (lot atomique
  d'opérations, simulation, idempotence) est la voie complète ; `POST`, `PATCH` et
  `DELETE` sur les ressources (`/prefixes`, `/addresses`…) enveloppent chacun un
  changeset d'une opération, par le même pipeline : motif obligatoire, réponse 201 ou
  200 avec l'identifiant du changeset, 202 si une politique d'approbation le met en
  attente.
- Pourquoi : le changeset reste la seule voie d'écriture (D11), avec ses lots
  hétérogènes atomiques ; un changement unitaire garde une forme REST simple pour le Web
  et le CLI.
- Rejeté : changesets seuls (chaque client construit des opérations pour un changement
  unitaire) ; CRUD et lots d'import séparés (perd les lots atomiques hétérogènes, le
  changeset devient un détail).
- Précision après la note [`api-http.md`](../../docs/reference/api-http.md) : conforme
  aux guides (lot atomique borné ; méthodes au sens enregistré, RFC 9205 §4.5).
  Le motif voyage dans le corps et le changeset est désigné par un lien (`Link` et
  corps), plutôt que par des en-têtes propres (`Kartenn-Motive`, `Kartenn-Changeset`) :
  RFC 9205 §3.2 recommande les liens, §4.7 réserve les nouveaux champs aux champs
  enregistrés. Un 202 sur PATCH désigne le changeset en attente, pas une opération
  longue (un PATCH n'en est jamais une, consensus des guides).
- Décidé par : recommandation acceptée ; précision : Claude, après recherche

## H3 — Identification dans les URL
- Décision : **UUID canonique, clé naturelle en filtre**. Collections à plat sous `/v1`
  (`/v1/prefixes/{uuid}`) ; la clé naturelle (domaine et valeur, slug) se cherche par un
  filtre indexé (`GET /v1/prefixes?domain=lab&value=10.1.2.0/24`), paginé comme toute
  liste ; les objets liés sont désignés par des liens.
- Pourquoi : une seule URL par objet, stable à travers renumérotations et déplacements,
  donc une seule clé de cache et un seul ETag (RFC 9111 §4.1) ; pas de `/` encodé dans
  un chemin. RFC 9205 §4.4 ne contraint pas ce choix pour une API applicative.
- Rejeté : chemins imbriqués par clé naturelle (`%2F` mal géré par certains proxys, URL
  qui change avec l'objet, deux URL par objet) ; alias redirigés en 303 (aller-retour en
  plus, double surface à tester et à contrôler).
- Décidé par : recommandation acceptée

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

## H4 — Lectures datées et cache
- Décision : **paramètres `valid_at` et `as_of`** (REQ-004, couple bitemporel) sur toute
  lecture et toute liste, en RFC 3339 avec décalage obligatoire (`Z` ou `±hh:mm`) ;
  absents, ils valent maintenant. Cache selon le temps système :
  - `as_of` passé : `Cache-Control: private, max-age` long, `immutable` (RFC 8246) ;
  - sans `as_of` : `private, no-cache` et ETag (revalidation, 304).
  Le curseur de pagination fige `as_of` à la première page : les pages d'une même liste
  sont cohérentes entre elles.
- Réserve (Claude) : `sys_start` vaut l'heure de *début* de la transaction (`now()`) ;
  une transaction commencée avant `as_of` et validée après écrit encore dans ce passé.
  Une réponse n'est donc `immutable` que si `as_of` précède maintenant d'au moins la
  durée maximale d'une transaction d'écriture (délai borné par la configuration) ; en
  deçà, elle est traitée comme une lecture courante. Même borne pour figer `as_of` dans
  un curseur. À reprendre dans le design.
- Pourquoi : le temps système est immuable (D8, ADR 0017) ; l'audit et les comparaisons
  de dates relisent souvent le même passé ; `private` parce que la réponse dépend des
  droits de l'appelant (RFC 9111 §5.2.2). Paramètres plutôt que ressource : rien à
  stocker, instances sans état (D9).
- Rejeté : pas de cache propre au passé (relectures historiques recalculées) ; ressource
  d'instantané `/snapshots/{id}` (état à stocker et à expirer, contraire à D9).
- Décidé par : recommandation acceptée ; réserve : Claude

## H5 — Pagination
- Décision : **curseur opaque signé, 100 par défaut, 1000 au plus, sans total**. Le
  curseur porte la clé de tri, les filtres et l'`as_of` figé (H4) ; il est signé (HMAC
  du trousseau, usage dédié) : un curseur altéré est refusé (400), jamais exécuté. Il
  n'accorde aucun droit : le RBAC est réévalué à chaque page. `page_size` est
  facultatif ; fin de liste = pas de `next`. Le lien suivant est donné dans `Link`
  (`rel="next"`) et dans le corps.
- Pourquoi : un curseur sur clé triée se lit par index à coût constant et ne glisse pas
  sous les écritures (consensus des guides) ; 100 est la page du budget D14 (150 ms) ;
  un total exact coûte un parcours complet sous RBAC et peut révéler ce qu'on ne voit
  pas.
- Rejeté : total estimé sur demande (l'estimation du planificateur ignore le RBAC :
  trompeuse et révélatrice) ; offset/limit (coût linéaire en profondeur, pages qui
  glissent).
- Décidé par : recommandation acceptée

## H6 — Filtres
- Décision : **paramètres de requête en conjonction**, un par condition, ET entre eux ;
  les valeurs d'un « un parmi » sont répétées. Champs du noyau (`domain`, `within` =
  inclusion CIDR, `nature`, `usage`…) et propriétés par clé (`prop.<clé>`), avec les
  conditions du moteur : égal, un parmi, entre bornes, présent, absent. Au-delà de
  8 Kio d'URL : 414.
- Réserve (Claude) : une clé de propriété contient des points ; l'opérateur ne peut donc
  pas être un suffixe pointé (`.between`) sans ambiguïté. Le séparateur exact est à fixer
  dans le design.
- Pourquoi : correspond exactement au moteur de la 003 (pas de OU entre champs) ; un GET
  reste sûr et cacheable.
- Rejeté : expression `?filter=` (grammaire à spécifier et à borner, disjonctions hors
  budget, moteur sans OU) ; POST de recherche en JSON (non cacheable ; la méthode QUERY
  n'est qu'un draft).
- Décidé par : recommandation acceptée

## H7 — Erreurs
- Décision : **Problem Details, liste complète pour un même type, le plus pertinent
  sinon** (RFC 9457 §3, à la lettre). Toujours `application/problem+json`, un `type`
  (URI propre à kartenn) par classe d'erreur, documenté dans l'OpenAPI. Un changeset
  refusé : 422, extension `violations` avec toutes ses violations (règle, sévérité,
  détail, sujets, indice d'opération, pointeur dans la requête) et lien vers le
  changeset journalisé (REQ-004-7 tenue). Des problèmes de classes différentes : seul le
  plus pertinent, selon l'ordre de HTTP (une requête non authentifiée et malformée →
  401). Codes : 400 syntaxe, 401, 403, 404 (aussi pour un objet invisible, pas
  d'oracle), 409 conflit d'état, 412 précondition, 413, 414, 415, 422 règle métier,
  428, 429, 503.
- Pourquoi : RFC 9457 §3 donne en exemple une erreur de validation en 422 avec un
  tableau d'erreurs du même type, et ne recommande un problème unique que pour des types
  différents, parce qu'une réponse n'a qu'un code de statut (« batch » types « do not
  map well into HTTP semantics »).
- Rejeté : un seul problème toujours (amender REQ-004-7, corrections en boucle) ; liste
  mêlant les types (contraire au RECOMMENDED de la RFC) ; format JSON propre.
- Décidé par : utilisateur (après relecture de RFC 9457 §3, 2026-10-18)

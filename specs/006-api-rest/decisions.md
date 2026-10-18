# 006 — API REST : journal des décisions

Entretien (grill-me) du 2026-10-18. Cadre : D7 (REST, seule frontière publique), D9
(instances sans état), D13 (OIDC et Basic sur TLS), D14 (budgets), ADR 0002, 0020 ;
briques des specs 002 à 005 et 012 ; ce que la 005 laisse à la couche HTTP (fin de
`specs/005-authn-rbac/design.md`). Crate cible : `kartenn-rest` (axum, utoipa ;
`docs/spec/architecture.md`).

## Synthèse
- H1 périmètre : conventions et toute la surface existante (002 à 005, 012).
- H2 écritures : `POST /changesets` et routes par ressource enveloppant un changeset.
- H3 URL : `/v1/<collection>/{uuid}`, clé naturelle en filtre, liens.
- H4 dates : `valid_at`, `as_of` en RFC 3339 ; passé `immutable`, présent `no-cache` + ETag.
- H5 pagination : curseur signé, 100 / 1000, sans total.
- H6 filtres : paramètres en conjonction ; `QUERY` (RFC 10008) pour les filtres longs.
- H7 erreurs : RFC 9457, liste complète pour un même type, le plus pertinent sinon.
- H8 concurrence : `If-Match` obligatoire (428, 412) ; `Idempotency-Key` (24 h).
- H9 imports : dépôt par morceaux, worker à bail dans PostgreSQL, suivi par interrogation.
- H10 CSRF : `SameSite=Strict`, `Sec-Fetch-Site`, `Origin` en repli.
- H11 débit : délestage par instance (503) et quota GCRA par identité (429).
- H12 versions : `/v1`, évolution additive, OpenAPI tiré du code et vérifié.
- H13 représentations : JSON Merge Patch ; objets complets avec résumé des liés.
- H14 transport : TLS configurable, HTTP/1.1 et 2, réponses compressées.
- H15 budgets : D14 mesurés à travers HTTP ; matrices de sécurité de bout en bout.

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
  budget, moteur sans OU) ; POST de recherche en JSON (non cacheable).
- Décidé par : recommandation acceptée
- Amendement (2026-10-18, utilisateur) : la méthode QUERY est normalisée
  ([RFC 10008](https://www.rfc-editor.org/rfc/rfc10008)) ; le rejet « seulement un
  draft » était faux. Les collections acceptent aussi **`QUERY` avec un corps JSON**
  portant les mêmes conditions, pour les filtres longs ou structurés ; c'est l'issue
  quand l'URL dépasserait 8 Kio. Sûre et idempotente (§2) : rejouable sans
  `Idempotency-Key`. Clé de cache incluant le corps (§2.7), `private` comme `GET` ;
  conditionnelles comme `GET` (§2.6) ; `Accept-Query: "application/json"` annonce la
  prise en charge (§3) ; sans `Content-Type` → 415 (§2.1 impose un 4xx). Hors liste
  CORS « safelisted » : sans effet pour le BFF de même origine. Prise en charge par
  axum et les proxys inverses à vérifier dans le design.

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

## H8 — Concurrence et idempotence
- Décision : **`If-Match` obligatoire sur les ressources, `Idempotency-Key` en
  en-tête**.
  - Concurrence : `PATCH` et `DELETE` d'une ressource exigent `If-Match` ; absent →
    428 (RFC 6585 §3, la réponse dit comment resoumettre), périmé → 412. L'ETag est la
    version de l'objet et devient l'`expected` de l'opération (REQ-004-11) : un seul
    mécanisme. Dans `POST /changesets`, `expected` reste dans le corps, par opération,
    facultatif comme en 004.
  - Idempotence : en-tête `Idempotency-Key` sur `POST /changesets` et sur les créations
    de ressources, porté sur l'`idempotency_key` existante avec les règles de
    REQ-004-10 (24 h, par identité). Même clé, contenu différent → 422 ; même clé en
    cours de traitement → 409 ; rejeu après la fin → la première réponse, inchangée. La
    durée de rétention est publiée dans l'OpenAPI.
- Réserve : le draft `Idempotency-Key` a expiré (-07) ; kartenn suit une proposition
  de la voie normative, pas un standard. La sémantique est celle de la 004 dans tous les
  cas.
- Pourquoi : un référentiel édité à plusieurs (Web, CLI, connecteurs) perd sinon des
  modifications en silence ; la précondition ne coûte qu'un GET que le client fait déjà.
  L'en-tête est reconnu par les intergiciels de rejeu génériques, la clé dans le corps
  non.
- Rejeté : `If-Match` facultatif (mises à jour perdues dès qu'il manque) ; clé
  d'idempotence dans le corps seulement (invisible des clients HTTP génériques).
- Décidé par : recommandation acceptée

## H9 — Imports et opérations longues
- Décision : **ressource d'import remplie par morceaux, exécutée par un worker sur
  PostgreSQL, suivie par interrogation**.
  - Dépôt : `POST /v1/imports` (source) → 201 et `Location` ;
    `POST /v1/imports/{id}/chunks` avec un numéro de séquence et 10 000 éléments au
    plus (un morceau renvoyé sous le même numéro est ignoré) ;
    `POST /v1/imports/{id}/start` clôt le dépôt, trie et découpe (004), répond 202 et
    `Location` vers l'import.
  - Exécution : chaque instance porte un worker qui réclame un import prêt
    (`FOR UPDATE SKIP LOCKED`, bail et battement de cœur) ; une instance morte laisse
    expirer le bail, une autre reprend au premier lot non appliqué (REQ-004-29). Pas de
    file externe : PostgreSQL reste le seul composant à état (D9, D15).
  - Suivi : `GET /v1/imports/{id}` (état, lots appliqués et restants, rapport ;
    `Retry-After` indicatif pendant l'exécution) ; `POST …/resume` relance un import en
    pause ; `POST …/cancel` l'arrête après le lot en cours, les lots appliqués restent
    (changesets ordinaires, annulables).
  - Les imports sont la seule opération longue : les changesets restent synchrones, pas
    de ressource générique `/operations`. Pas de notification de fin : les événements
    sortants relèvent de leurs specs.
- Pourquoi : corps bornés (une seule limite 413 pour l'API), dépôt reprenable après une
  coupure, aucune requête n'occupe une instance pendant les minutes d'envoi de 10⁶
  éléments ; la reprise par bail découle de REQ-004-29 sans nouvel état.
- Rejeté : corps unique en flux JSON Lines (une coupure fait tout recommencer, limite
  de taille propre à la route, requête longue) ; file de messages externe (composant à
  état de plus, contraire à D9).
- Décidé par : recommandation acceptée

## H10 — CSRF des sessions par cookie
- Décision : **`SameSite=Strict`, Fetch Metadata, `Origin` en repli**. Sur une méthode
  non sûre authentifiée par cookie (`POST`, `PATCH`, `DELETE`) : `Sec-Fetch-Site` doit
  valoir `same-origin` (`same-site`, `cross-site`, `none` → 403) ; en son absence,
  `Origin` doit égaler l'origine publique configurée ; ni l'un ni l'autre → 403. Les
  méthodes sûres (`GET`, `HEAD`, `QUERY`) ne sont pas contrôlées. Bearer et Basic ne sont
  pas concernés : le navigateur ne les envoie pas de lui-même.
- Pourquoi : aucun jeton à stocker (D9) ; `SameSite=Strict` seul laisse passer un
  sous-domaine (même site) et les vieux navigateurs, Fetch Metadata et `Origin` les
  couvrent.
- Rejeté : jeton synchroniseur en plus (en-tête propre et travail du client Web pour une
  défense déjà couverte) ; `SameSite=Strict` seul.
- Décidé par : recommandation acceptée

## H11 — Limitation de débit
- Décision : **deux couches en mémoire, par instance, configurables**.
  - Délestage (protège PostgreSQL) : plafond de requêtes en cours par instance (par
    défaut 2 × la taille du pool), courte file bornée devant ; au-delà → 503 et
    `Retry-After`. C'est lui qui tient les budgets D14 sous charge, quel que soit le
    nombre d'identités.
  - Quota par identité (équité) : GCRA, compartiments lecture et écriture distincts ;
    avant authentification, clé = adresse source (en-tête transféré lu seulement
    derrière des proxys de confiance configurés). Dépassement → 429, `Retry-After`,
    et `RateLimit-Policy` / `RateLimit` (draft-11, indicatifs). Les morceaux d'import
    comptent comme écritures pondérées par leur taille. Défauts : lecture 50/s rafale
    200, écriture 10/s rafale 20, avant authentification 20/s par adresse.
  - Documenté comme par instance : plafond réel N × la limite. But : équité et
    protection, pas facturation. Des quotas globaux exacts pourront s'ajouter plus tard
    (seaux locaux, répartition périodique par PostgreSQL) sans changer l'API.
  - La force brute sur les connexions reste au `FailureLimiter` (005).
- Pourquoi : coût d'environ 100 ns sans entrée-sortie, aucun état nouveau (D9) ; GCRA
  donne un débit régulier et une rafale bornée en un entier par clé, sans le double
  débit des fenêtres fixes. Crate à évaluer au design (`crate-scout`).
- Rejeté : compteurs dans PostgreSQL (une écriture par requête sur le seul primaire,
  qu'on veut protéger) ; magasin clé-valeur partagé (composant à état de plus, contraire
  à D9 et D15) ; proxy inverse seul (ne connaît pas l'identité, compte par adresse
  derrière NAT et BFF ; il peut rester une barrière grossière en amont).
- Décidé par : recommandation acceptée

## H12 — Versions et contrat OpenAPI
- Décision : **version majeure dans le chemin, évolution additive, OpenAPI tiré du
  code**.
  - `/v1` : dans une version, seulement des ajouts ; les clients tolèrent les champs et
    les valeurs d'énumération inconnus. Un changement cassant ouvre `/v2`, servi à côté
    de `/v1` pendant un préavis, avec `Deprecation` (RFC 9745), `Sunset` (RFC 8594) et
    un lien `rel="successor-version"`.
  - OpenAPI 3.1 généré par utoipa, servi à `GET /v1/openapi.json` pour les modules de
    cette build (la liste est aussi dans `GET /v1/capabilities`). Le contrat de
    référence (build toutes features) est versionné dans le dépôt ; un test le compare
    au contrat produit et refuse tout changement cassant dans `/v1` (ajout : contrat mis
    à jour ; retrait ou changement de type : échec).
- Pourquoi : un lien simple se teste et se cache sans `Vary` ; les SDK (CLI, MCP,
  TypeScript) se génèrent d'un contrat qui ne peut pas dériver du code ; le test rend
  la promesse additive vérifiable.
- Rejeté : version dans le type de média (`Vary: Accept`, outils et générateurs mal
  à l'aise) ; versions datées par requête (couches de traduction à maintenir, trop lourd
  pour la taille du projet) ; contrat écrit à la main (dérive possible du code).
- Décidé par : recommandation acceptée

## H13a — Format du PATCH
- Décision : **JSON Merge Patch seul** (RFC 7396, `application/merge-patch+json`).
  Les champs envoyés deviennent une opération `Set…` ; `null` retire un champ
  facultatif ; les tableaux sont remplacés entiers ; les tables de propriétés se
  modifient clé par clé (`{"properties": {"k": "v", "old": null}}`). Le motif et la
  période de validité (H2) sont des membres réservés de premier niveau, documentés.
  Concurrence par `If-Match` (H8).
- Réserve (Claude) : `null` vaut retrait ; le design vérifie, champ par champ, qu'aucun
  champ ne distingue `null` d'absent.
- Pourquoi : correspond directement aux opérations ; idempotent ; schéma OpenAPI précis
  (la ressource, champs facultatifs) ; facile pour un formulaire ou le CLI. Les
  modifications fines ou multi-objets passent par `POST /changesets`.
- Rejeté : JSON Patch (RFC 6902) : second langage d'opérations à côté des changesets,
  non idempotent, `value` non typé, chemins liés à la forme JSON et aux indices,
  `move`/`copy` sans sens, aucune place pour le motif (contraire à H2) ; les deux formats
  (double surface pour un gain limité aux tableaux, courts chez nous).
- Décidé par : utilisateur, après comparaison

## H13b — Réponses partielles et expansion
- Décision : **représentations complètes et liens, avec un résumé des objets liés**
  (`domain: {id, slug, href}` : identifiant, clé naturelle, lien). Ni `fields=` ni
  `expand=` en v1 ; les propriétés héritées (003) restent hors de la représentation et
  se lisent par leur route.
- Pourquoi : coût de requête prévisible (budgets D14), une variante de cache par URL et
  par appelant, ETag = version de l'objet (H8 reste simple), RBAC vérifié une fois,
  types OpenAPI exacts. Le besoin réel du Web (listes affichant domaine et site) est
  couvert par le résumé, jointure fixe et bornée. Les objets sont petits : une réponse
  partielle n'économiserait presque rien. Les deux mécanismes restent ajoutables sans
  rupture (H12) si une mesure le justifie.
- Rejeté : `fields=` (variantes de cache, ETag partiel inutilisable en `If-Match`, types
  partiels) ; `fields=` et `expand=` (coût non borné, RBAC et lecture datée à chaque
  expansion, ETag composite, oracle possible sur un objet invisible).
- Décidé par : utilisateur, après comparaison

## H14 — Transport : TLS, versions HTTP, compression
- Décision :
  - TLS : **terminaison configurable**, par le répartiteur par défaut ou par kartenn
    (rustls). Derrière un proxy, `Forwarded: proto=https` (RFC 7239) n'est cru que des
    adresses de proxys configurées (même liste que H11) ; une requête arrivée sans TLS
    voit le Basic refusé et ne reçoit aucun cookie (REQ-005-12, `__Host-`, `Secure`).
  - HTTP/1.1 et HTTP/2 (ALPN `h2` en TLS, `h2c` derrière un proxy) ; pas de HTTP/3
    dans kartenn, il relève du proxy de bordure.
  - Réponses compressées (`zstd`, `br`, `gzip` selon `Accept-Encoding`, au-delà de
    1 Kio, `Vary: Accept-Encoding`), sauf celles qui montrent un secret une fois
    (secret de compte de service, 005) : non compressées et `Cache-Control: no-store`
    (BREACH). Corps de requête compressés refusés en v1 (415) : pas de bombe de
    décompression, limite 413 exacte.
- Pourquoi : la topologie d'architecture met TLS au répartiteur, mais une petite
  installation doit pouvoir s'en passer ; les exigences TLS de la 005 restent tenues
  dans les deux cas. HTTP/1.1 et 2 sont fournis par hyper ; les listes JSON se
  compressent de 5 à 10 fois ; nos corps ne portent pas de secret, sauf l'exception
  traitée.
- Rejeté : TLS au répartiteur seulement (pas d'installation sans proxy) ; TLS dans
  kartenn seulement (rotation de certificats dans chaque instance imposée) ; HTTP/3 dans
  kartenn (pile QUIC séparée sans gain derrière un répartiteur) ; décompression des
  requêtes.
- Décidé par : recommandation acceptée

## H15 — Budgets et tests
- Décision :
  - Budgets D14 mesurés **à travers HTTP**, serveur en processus : mêmes scénarios que
    les bancs actuels, en passant par axum (authentification cookie et Bearer, RBAC,
    JSON, compression). Un test de charge vérifie 200 utilisateurs concurrents et
    50 écritures/s soutenues, sans 5xx hors délestage volontaire (H11). Les bancs du
    service restent, pour localiser le surcoût HTTP (cible : < 10 ms au p95).
  - Tests fonctionnels en processus (`tower`) : une route et un code d'erreur H7 par
    test ; test de contrat OpenAPI (H12) ; matrices de sécurité de bout en bout (CSRF
    H10, 404 pour un objet invisible, Basic refusé hors TLS via l'en-tête transféré H14,
    428 et 412, curseur altéré, idempotence : rejeu, contenu différent, en cours, 429 et
    503, attributs des cookies de la 005).
  - Quelques tests sur vrai TCP et TLS pour ce que `tower` ne voit pas : ALPN, cookies,
    `Forwarded`.
  - Le déploiement complet (répartiteur, TLS, plusieurs instances) est hors CI v1 : banc
    d'exploitation documenté dans le runbook.
- Pourquoi : la couche HTTP peut régresser seule (sérialisation, filtrage RBAC,
  compression) ; la mesure en processus reste reproductible en CI.
- Rejeté : service seul (ne voit pas la couche HTTP) ; déploiement complet en CI
  (infrastructure dédiée, mesures bruitées).
- Décidé par : recommandation acceptée

## Fin de l'entretien
Plus aucune question ne change les exigences. Restes de la 005 (attributs des cookies,
routes de connexion, déconnexion et secours, variante de `logged_in` sans événement,
rechargement des JWKS) : déjà décidés par la 005, repris tels quels dans les exigences.

Questions ouvertes, toutes de design :
- séparateur des opérateurs de filtre, les clés de propriétés contenant des points
  (H6) ;
- borne de l'`immutable` et du gel de l'`as_of` : durée maximale d'une transaction
  d'écriture (H4) ;
- champs où `null` et absent différeraient, pour JSON Merge Patch (H13a) ;
- prise en charge de QUERY par axum et les proxys inverses (H6) ;
- crate de limitation GCRA (H11, `crate-scout`).

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
- H19 ETag d'un objet : `"<version>.<empreinte du corps>"` ; `If-Match` ne compare que la version.
- H20 filtres déclarés au contrat : chaque filtre servi est un `parameter` ; `prop.{key}`
  et ses opérateurs en gabarit décrit par `x-kartenn` (021/X41).
- H21 version dans le corps : tout objet lu (par identifiant ou dans la page de sa
  collection) porte sa version en `version`, la part propre de son ETag.
- H22 libellés et suggestions (spec 018) : corps neutres en langue (`labels`), seul
  `GET /v1/labels` varie selon la langue ; `q` sur les listes citables, `/values`
  datées, `GET /v1/rules`.

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
    428 (RFC 6585 §3, la réponse dit comment resoumettre), périmé → 412. L'ETag porte la
    version de l'objet, qui devient l'`expected` de l'opération (REQ-004-11) : un seul
    mécanisme (étiquette composite depuis H19). Dans `POST /changesets`, `expected` reste dans le corps, par opération,
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
    défaut 2 × la taille du pool) ; au-delà → 503 et `Retry-After`, aussitôt.
    Amendé au design (2026-10-18, utilisateur) : pas de file d'attente dans kartenn
    (tower n'en fournit pas ; une requête qui attend sort déjà de son budget) ; la file
    est celle des connexions en attente du système. C'est lui qui tient les budgets D14 sous charge, quel que soit le
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
- Amendement (2026-10-23, utilisateur, H19) : le résumé rend l'ETag « version de
  l'objet » faux pour le cache (un lié renommé change le corps). L'ETag d'un objet
  devient composite, `"<version>.<empreinte>"`, et `If-Match` n'en compare que la
  version : le rejet de l'« ETag composite » ci-dessus visait l'expansion, pas ce cas.

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

## H16 — Tâches de fond
- Constat (Claude, en écrivant les exigences) : plusieurs specs renvoient à la 006
  l'ordonnanceur des tâches de fond : expiration des changesets en attente (004),
  réévaluation des sources de valeurs attendues (003), réécriture de l'indicateur
  `public` après une mise à jour des registres (002), purge des clés d'idempotence
  (H8), et le worker d'import (H9).
- Décision : **le mécanisme de bail de H9, généralisé**. Une table de tâches (nom,
  prochaine échéance, période, bail, battement) ; chaque instance réclame les tâches
  échues (`FOR UPDATE SKIP LOCKED`), une seule les exécute ; une instance morte laisse
  expirer le bail. Les tâches propres à une instance (purge de `StateCache` et du
  `FailureLimiter`, rechargement des JWKS et de la découverte) restent des minuteries
  locales. La purge de l'audit garde sa sous-commande pour une minuterie externe (012).
- Pourquoi : pas de composant à état de plus (D9, D15) ; un seul mécanisme pour les
  imports et les tâches périodiques.
- Rejeté : verrou consultatif PostgreSQL tenu par une instance « chef » (une connexion
  monopolisée, bascule plus lente) ; minuterie externe pour tout (dépendance
  d'exploitation pour des tâches dont la correction dépend, comme l'expiration).
- Décidé par : Claude (délégation, « continue au maximum ») ; approuvé par l'utilisateur
  avec les exigences (2026-10-18)

## H17 — Le délestage ne compte que `/v1` (précise H11, REQ-006-30)
- Constat (2026-10-21) : le délestage comptait toutes les requêtes en cours sous un
  plafond de 2 × la taille du pool ; le shell et ses plugins chargent des dizaines de
  fichiers à la fois en HTTP/2, et un module de plugin pouvait recevoir 503
  `application/problem+json`, que le navigateur refuse d'exécuter.
- Décision : seules les requêtes sous `/v1` comptent pour le délestage, comme pour les
  quotas (007/W18) ; les fichiers du shell et des plugins ne sollicitent pas la base.
  REQ-006-30 est précisée en ce sens.
- Décidé par : l'utilisateur (2026-10-21), sur la proposition de Claude

## H18 — Tri d'un import en tâche de fond (amende H9, REQ-006-17 et 19)
- Constat (2026-10-21) : `start` trie le million d'éléments dans la requête (environ
  11 s au calme, plus de 30 s sous charge) ; au-delà du délai de transport (30 s), le
  client reçoit 503 alors que le tri se poursuit ou échoue sans être vu.
- Options : (A) délai plus long pour cette seule route ; (B) tri dans la tâche de fond
  de l'import, l'erreur rendue dans l'état de l'import.
- Décision : **B**. `start` vérifie l'état, passe l'import à `sorting` et pose la tâche
  `import:<id>` ; la tâche trie et découpe en lots dans une transaction, puis passe à
  `ready` et applique comme avant. Une erreur de tri ramène l'import à `uploading` et
  s'expose dans sa représentation (`error`, problème `invalid-import` avec ses
  violations) ; `start` ne répond plus 422. Toute requête reste bornée par le délai de
  transport.
- Décidé par : l'utilisateur (2026-10-22), sur la proposition de Claude

## H19 — ETag composite d'un objet (amende H8, H13b et REQ-006-14)
- Constat (2026-10-23) : l'ETag fort d'un objet était sa version (dernier changeset qui
  l'a écrit, ou ses propriétés). Or sa représentation montre aussi des données d'autres
  objets : le résumé d'un lié porte sa `slug` (H13b), une interface montre le
  propriétaire et le site résolus de sa machine, la fiche d'une machine embarque ses
  interfaces et leurs adresses. Renommer le domaine d'un préfixe change le corps du
  préfixe sans changer son ETag : `If-None-Match` répond 304 à tort. C'est contraire à
  RFC 9110 §8.8.1 : un validateur fort doit changer dès que les données de la
  représentation changent d'une façon observable dans un 200.
- Options : (a) ETag composite, la part propre seule comparée par `If-Match` ; (b) ETag
  faible (empreinte du corps) pour le cache, la version dans un autre en-tête pour les
  écritures ; (c) relever la version de tous les dépendants à chaque écriture (cascade
  d'écritures non bornée : un domaine renommé toucherait tous ses préfixes).
- Décision : **(a)**. L'ETag fort d'un objet est `"<version>.<empreinte>"` :
  - `<version>` : la version de l'objet (REQ-006-14, design « Version d'un objet »),
    celle que compare `If-Match` et que porte `expected` (REQ-004-11) ;
  - `<empreinte>` : 16 chiffres hexadécimaux (64 bits de SHA-256) des octets du corps
    servi. Elle couvre tout ce que le corps montre d'autre que l'objet : résumés des
    liés, valeurs héritées, collections embarquées, et même ce qui dépend des droits de
    l'appelant (interfaces visibles d'une fiche). Choix de Claude, peu coûteux à défaire
    : la « version des liés » demandée est réalisée par l'empreinte plutôt que par le
    maximum des versions lues, qui manquerait les valeurs calculées (héritage,
    visibilité), obligerait chaque module à énumérer ses dépendances et dépendrait de
    l'ordre des UUID. Coût : un SHA-256 sur quelques centaines d'octets, aucune requête.
  - `If-None-Match` compare l'étiquette entière (comparaison faible, RFC 9110 §13.1.2) :
    un parent renommé donne 200.
  - `If-Match` ne compare que la part propre : l'étiquette entière ou la version seule
    (`"<version>"`, l'ancienne forme, celle d'`expected`) sont acceptées ; toute autre
    forme → 412. La forme nue est gardée sans échéance : elle ne coûte qu'une branche et
    protège exactement autant (voir ci-dessous).
- Écart à la norme, propre à kartenn : RFC 9110 §13.1.1 évalue `If-Match` par la
  comparaison forte de l'étiquette entière contre l'ETag courant de la représentation ;
  kartenn n'en compare que la version. Pourquoi c'est sûr : `If-Match` sert ici à
  empêcher la mise à jour perdue (RFC 9110 §13.1.1, « prevent accidental overwrites »).
  `PATCH`, `DELETE` et les actions d'un objet n'écrivent que son propre état (champs et
  affectations), que la version couvre exactement : un client ne peut jamais écraser une
  modification de l'objet qu'il n'a pas lue. Les données des liés ne sont ni réécrites
  ni recopiées par l'écriture ; leurs propres écritures ont leur propre ETag, et les
  règles qui en dépendent sont revalidées dans la transaction du changeset (004).
  Comparer l'étiquette entière refuserait au contraire (412) toute écriture en attente
  dès qu'un parent change : renommer un domaine invaliderait les éditions de tous ses
  préfixes, sans rien protéger de plus.
- Rejeté : (b) deux validateurs pour une même représentation (un client générique
  renvoie l'ETag, pas un en-tête propre) ; (c) écritures en cascade non bornées.
- Décidé par : l'utilisateur (2026-10-23) pour (a) ; empreinte et forme nue : Claude.

## H20 — Filtres déclarés au contrat (précise H6, H12 et REQ-006-9 ; 021/X41)
- Constat (2026-10-23, spec 021, REQ-021-35) : les filtres typés du nouveau shell et les
  options de la CLI (023) se tirent des `parameters` du contrat. Or le contrat ne
  déclarait que la clé naturelle des listes du noyau : `prop.<clé>` et ses opérateurs n'y
  figuraient pas (la CLI ajoutait `--prop` à toute liste, par convention), et `within`,
  `nature`, `usage`, promis par REQ-006-9, n'étaient pas servis.
- Décision : **tout filtre servi par une liste est déclaré dans ses `parameters`, typé** ;
  un test le vérifie sur le contrat de la build (chaque liste est sondée avec tous les
  noms de filtre du contrat ; un nom qu'elle ne refuse pas comme inconnu doit y être
  déclaré). Les filtres de propriétés, une famille de noms qu'OpenAPI 3.1 ne sait pas
  décrire (un paramètre a un nom fixe ; `style: deepObject` donnerait `prop[clé]`), sont
  déclarés comme trois **gabarits** : `prop.{key}`, `prop.{key}:between` (chaîne
  contenant `..`), `prop.{key}:exists` (booléen) ; `{key}` s'y lit comme dans un gabarit
  de chemin, et l'extension `x-kartenn` du paramètre le dit :
  `{"template": {"key": {"pattern": "^[a-z][a-z0-9_]*\\.[a-z][a-z0-9_]*$", "collection": "/v1/property-declarations"}}}`.
  Un filtre dont les valeurs viennent d'une liste porte `x-kartenn.collection`
  (`domain` → `/v1/domains`) ; une valeur fermée, le schéma de son énumération
  (`nature` → `Nature`). Les gabarits ne figurent que sur les listes qui filtrent par
  propriété (préfixes, plages, adresses) ; une autre liste refuse `prop.…` comme
  paramètre inconnu.
- Pourquoi : un client générique voit des paramètres valides, de nom inoffensif ; un
  client de kartenn reconnaît le gabarit par l'extension, sans convention cachée. Les
  types restent ceux des autres filtres répétables (`string`, « répété : un parmi »),
  pour ne pas changer le type d'un paramètre existant (H12).
- Rejeté : `style: deepObject` (autre syntaxe que celle servie, H6) ; un paramètre
  unique `in: querystring` (OpenAPI 3.2, pas encore la version du contrat, H12) ; la
  convention implicite des clients (ce qui manquait).
- Décidé par : Claude (forme du contrat déléguée par la tâche 021-28, 2026-10-23) ;
  `within`, `nature`, `usage` : REQ-006-9, déjà validée.

## H21 — La version d'un objet dans son corps (précise H19 et REQ-006-14 ; 022, 021)
- Constat (2026-10-24) : le contrat client de la 022, amendé par la 021 (A3), compare la
  `version` lue dans le corps d'un objet à la révision d'un événement (`changeset`) pour
  ne pas relire un objet déjà à jour. Aucun corps d'objet ne la portait : la version
  n'était que la part propre de l'ETag (H19), opaque pour le client, qui relisait donc
  tout objet nommé par un événement (relecture conditionnelle, 304 au mieux). Aucun
  membre existant ne la portait sous un autre nom (`vid` est le numéro d'un VLAN,
  `changeset` d'une version d'historique est une autre notion).
- Décision : **tout objet servi porte `version`**, l'UUID du dernier changeset qui l'a
  écrit, lui ou ses propriétés (design « Version d'un objet ») : la même valeur que la
  part propre de son ETag et qu'`expected` (REQ-004-11). Le kit l'ajoute :
  `respond_object` l'insère dans le corps de toute lecture par identifiant (noyau et
  modules, avant l'empreinte de l'ETag) ; `kit::versioned` dans les objets d'une page de
  la collection, en une requête par page (`read::versions`) ; l'assemblage du contrat
  déclare le membre (`string`, `uuid`, `readOnly`) sur le schéma de la lecture par
  identifiant et sur celui des éléments de la page, pour tout type d'objet du noyau et
  des modules. Un `PATCH` du noyau qui le porte → 422 (membre en lecture seule, comme
  `id`) ; un module le refuse comme membre inconnu (400).
- Non obligatoire au schéma : un même schéma montre aussi l'objet ailleurs, sans
  version (version d'historique, origine d'une valeur héritée dans une explication, flux
  en revue dont la page montre l'état proposé). Le client compare quand le membre est
  là, relit sinon.
- Additif (H12) : un membre de plus dans des corps ouverts ; aucun alias.
- Décidé par : orchestrateur de la session (suite de la 022/021, 2026-10-24) ; forme
  (non obligatoire, pages incluses, refus en écriture) : Claude, peu coûteuse à défaire.

## H22 — Libellés, suggestions et règles (amende REQ-006-4, 9, 35 ; spec 018)
- Constat (2026-10-24) : la 018 (retours d'usage de la v0.2) ajoute au contrat ce que
  les filtres typés et l'édition en place de la 021 consomment.
- Décision :
  - **Libellés** (ADR 0040, 018/R19 à R24) : un corps porte des clés et des `labels`
    `{langue: texte}`, jamais un libellé résolu ni de champ `label` ; `GET /v1/labels`
    est la seule route qui varie selon la langue (`lang`, sinon `Accept-Language` ;
    `Content-Language`, `Vary`) ; `default_locale` aux capacités ; opération
    `update_enum_value`.
  - **Suggestions** (Q-018-8 : extension des routes existantes) : `q` (préfixe insensible
    à la casse, 1 à 100 caractères) déclaré sur les listes citables par une référence,
    sous leur prédicat de lecture, 20 éléments par défaut ; refusé sur `ranges` et avec
    `prop.…` ; `/values` rend `{key, valid_from, valid_to}` à `valid_at`, `q` sur la clé
    et les libellés.
  - **Règles** : `GET /v1/rules` (`{id, severity, module}`), collection réservée au
    noyau (`CORE_COLLECTIONS`).
- Pourquoi : ajouts seulement (H12) ; un client lit les paramètres au contrat (H20).
- Décidé par : l'utilisateur (spec 018, 2026-10-23) ; forme du contrat : Claude

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

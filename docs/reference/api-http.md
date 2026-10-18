# API HTTP : sémantique, conventions et montée en charge

Note de lecture pour la spec [006-api-rest](../../specs/006-api-rest/). Recherche
documentaire du 2026-10-18 ; ce qui n'a pas pu être relu à la source est marqué
**non vérifié**. Errata des RFC citées : **non vérifiés**.

## Sources

| Document | Statut | Consulté |
|---|---|---|
| [RFC 9110](https://www.rfc-editor.org/rfc/rfc9110.html) — HTTP Semantics | STD 97 (2022) | 2026-10-18 |
| [RFC 9111](https://www.rfc-editor.org/rfc/rfc9111.html) — HTTP Caching | STD 98 (2022) | 2026-10-18 |
| [RFC 9205](https://www.rfc-editor.org/rfc/rfc9205.html) — Building Protocols with HTTP | BCP 56 (2022), remplace RFC 3205 | 2026-10-18 |
| [RFC 6585](https://www.rfc-editor.org/rfc/rfc6585.html) — codes 428, 429 | Proposed Standard (2012) | 2026-10-18 |
| [RFC 9457](https://www.rfc-editor.org/rfc/rfc9457.html) — Problem Details for HTTP APIs | Proposed Standard (2023), remplace RFC 7807 | 2026-10-18 |
| [RFC 8288](https://www.rfc-editor.org/rfc/rfc8288.html) — Web Linking | Proposed Standard (2017) | 2026-10-18 |
| [RFC 8246](https://www.rfc-editor.org/rfc/rfc8246.html) — `immutable` | Proposed Standard (2017) | 2026-10-18 |
| [RFC 6750](https://www.rfc-editor.org/rfc/rfc6750.html) (Bearer), [RFC 7617](https://www.rfc-editor.org/rfc/rfc7617.html) (Basic) | Proposed Standard (2012, 2015) | 2026-10-18 |
| [RFC 7396](https://www.rfc-editor.org/rfc/rfc7396.html) (Merge Patch), [RFC 6902](https://www.rfc-editor.org/rfc/rfc6902.html) (JSON Patch), [RFC 9535](https://www.rfc-editor.org/rfc/rfc9535.html) (JSONPath) | Proposed Standard | 2026-10-18 |
| [RFC 3339](https://www.rfc-editor.org/rfc/rfc3339.html) — dates | Proposed Standard (2002) | 2026-10-18 |
| [RFC 9113](https://www.rfc-editor.org/rfc/rfc9113.html) (HTTP/2), [RFC 9114](https://www.rfc-editor.org/rfc/rfc9114.html) (HTTP/3) | Proposed Standard (2022) | 2026-10-18 |
| [RFC 7932](https://www.rfc-editor.org/rfc/rfc7932.html) (brotli), [RFC 8878](https://www.rfc-editor.org/rfc/rfc8878.html) (zstd) | Informational | 2026-10-18 |
| [draft-ietf-httpapi-idempotency-key-header-07](https://datatracker.ietf.org/doc/draft-ietf-httpapi-idempotency-key-header/) | draft de groupe de travail, **expiré** (2025-10-15) | 2026-10-18 |
| [draft-ietf-httpapi-ratelimit-headers-11](https://datatracker.ietf.org/doc/draft-ietf-httpapi-ratelimit-headers/) | draft actif (2026-05-23, expire le 2026-11-24) | 2026-10-18 |
| [draft-ietf-httpbis-rfc6265bis-22](https://datatracker.ietf.org/doc/draft-ietf-httpbis-rfc6265bis/) — cookies, `SameSite` | draft en dernière relecture (2025-12-01) | 2026-10-18 |
| [W3C Fetch Metadata Request Headers](https://www.w3.org/TR/fetch-metadata/) | Working Draft (2026-09-21) | 2026-10-18 |
| [OpenAPI 3.1.2](https://spec.openapis.org/oas/v3.1.2.html) ; [`latest`](https://spec.openapis.org/oas/latest.html) annonce 3.2.1 | spécification (2025-09-19 ; 2026-09-10, **non vérifié**) | 2026-10-18 |
| R. Fielding, [thèse, chapitre 5](https://ics.uci.edu/~fielding/pubs/dissertation/rest_arch_style.htm) — style REST | thèse (2000), non normative | 2026-10-18 |
| Guides publics de conception d'API d'éditeurs de logiciels (pagination, champs partiels, lots, opérations longues, idempotence, compatibilité) | guides, non normatifs ; volontairement non nommés | 2026-10-18 |

Les guides d'éditeurs ne sont pas des normes, et la documentation de kartenn ne nomme
pas d'entreprise : leurs recommandations sont reprises comme **consensus des guides**,
seulement là où aucune RFC ne parle (pagination, champs partiels, lots, opérations
longues, idempotence, compatibilité), et seulement quand plusieurs guides concordent.

## Ce que disent les sources

### Le style REST et la montée en charge (Fielding, chap. 5)

- §5.1.3, sans état : chaque requête porte tout son contexte. Cela apporte la
  visibilité (une requête s'examine seule), la fiabilité (reprise après panne partielle)
  et la montée en charge (le serveur libère ses ressources entre deux requêtes). Le prix
  est la répétition de données à chaque requête.
- §5.1.4, cache : il supprime tout ou partie des interactions, au risque de données
  périmées.
- §5.1.5, interface uniforme : visibilité et évolution indépendante, au prix de
  l'efficacité. §5.1.6, couches : permet la répartition de charge, au prix de la
  latence.
- HATEOAS : **non relu** dans cette recherche.

### Bonnes pratiques d'une application sur HTTP (RFC 9205, BCP 56)

- §2 : le document vise les *spécifications* d'applications bâties sur HTTP. Pour une
  API d'un seul produit, c'est une analogie, pas une obligation.
- §3.1 : MUST NOT redéfinir ni surcharger la sémantique des éléments génériques de HTTP
  (méthodes, codes, champs). §3.2 : des liens entre ressources sont RECOMMENDED.
- §4.1 : MUST NOT imposer une version maximale de HTTP ; une version minimale est NOT
  RECOMMENDED.
- §4.4 et §4.4.1 : décourage de figer des chemins d'URL dans une spécification
  d'application, **sans mot normatif**. Le point vise surtout les specs IETF qui
  imposeraient des chemins à des serveurs tiers.
- §4.5 et §4.6 : méthodes et codes enregistrés seulement (MUST), sans en changer le
  sens (MUST NOT). Le détail d'une erreur va dans le corps.
- §4.7 : un nouveau champ d'en-tête MUST être enregistré, et les Structured Fields sont
  RECOMMENDED.
- §4.8 : un media type distinct par format. §4.9 : conseils de cache (`must-revalidate`
  §4.9.2, `Vary` §4.9.4).
- §4.11 : MUST NOT supposer de lien entre les requêtes d'une même connexion.
- §4.16 : versionnage possible par relation de lien, par media type ou par en-tête, sans
  préférence normative.

### Méthodes, codes, validateurs (RFC 9110, RFC 6585)

- Méthodes (§9.2.1, §9.2.2) : GET et HEAD sont sûres ; PUT, DELETE et les méthodes sûres
  sont idempotentes ; POST et PATCH ne le sont pas.
- POST (§9.3.3) : 201 avec `Location` (SHOULD). PUT (§9.3.4) : 201 s'il crée (MUST),
  sinon 200 ou 204. DELETE (§9.3.5) : 202 si l'action n'est pas encore faite, 204 si
  elle l'est.
- 202 (§15.3.3) est « intentionally noncommittal » : la RFC ne définit ni ressource de
  statut ni format.
- 401 (§15.5.2) : MUST porter `WWW-Authenticate`. 405 (§15.5.6) : MUST porter `Allow`.
- 422 (§15.5.21) s'appelle « Unprocessable Content ». 413 (§15.5.14) : si la condition
  est temporaire, `Retry-After` SHOULD. 414 (§15.5.15) : URI trop longue. 503 (§15.6.4)
  et `Retry-After` (§10.2.3).
- 428 et 429 viennent de la RFC 6585 (§3, §4) : 428 SHOULD expliquer comment resoumettre
  la requête ; 429 SHOULD détailler la condition, `Retry-After` MAY ; aucun des deux ne
  doit être mis en cache (MUST NOT).
- ETag (§8.8.3) : une chaîne entre guillemets, `W/` pour un validateur faible (§8.8.1).
  Un contenu encodé (`Content-Encoding`) a son propre ETag fort ; un codage de transfert
  ne change pas l'ETag.
- `If-Match` (§13.1.1) : comparaison forte (MUST). `If-None-Match` (§13.1.2) :
  comparaison faible (MUST), 304 sur GET et HEAD, 412 sinon. L'ordre d'évaluation des
  préconditions (§13.2.2) est un MUST.
- Codage de contenu (§8.4) : l'émetteur MUST envoyer `Content-Encoding`. §12.5.3
  (`Accept-Encoding`) et §12.5.5 (`Vary`) : **non relus**.

### Cache (RFC 9111, RFC 8246)

- §5.2.2 : `private` réserve la réponse au seul utilisateur. `no-store` interdit de la
  stocker. `no-cache` impose une revalidation avant toute réutilisation. `max-age` donne
  la fraîcheur, `must-revalidate` interdit de servir une réponse périmée.
- §3.5 : un cache partagé ne réutilise une réponse à une requête portant `Authorization`
  que si elle porte `must-revalidate`, `public` ou `s-maxage`.
- §4.1 : `Vary` liste les en-têtes de la requête qui entrent dans la clé de cache.
  §4.3 : revalidation par une requête conditionnelle et une réponse 304.
- RFC 8246 §2 : `immutable` dit que la réponse ne changera pas pendant sa fraîcheur ; le
  client SHOULD NOT envoyer de requête conditionnelle pendant ce temps.

### Erreurs (RFC 9457)

- §3.1 : membres `type` (URI, par défaut `about:blank`), `status`, `title`, `detail`,
  `instance`. Un membre de type incorrect MUST être ignoré.
- §3.2 : extensions permises ; un client MUST ignorer celles qu'il ne connaît pas.
- Media type `application/problem+json` (§3, §6). §3 : pour plusieurs problèmes, il est
  RECOMMENDED de ne représenter que le plus pertinent ; une liste d'erreurs est donc une
  extension propre à l'API.

### Liens et pagination

- RFC 8288 §3 : syntaxe de `Link` ; `rel` est MUST, une seule fois (§3.3). Les relations
  `next` et `prev` viennent du registre IANA « Link Relations » (**non relu**), pas de la
  RFC.
- **Aucune norme** sur les curseurs, la taille des pages ou les totaux.
- Consensus des guides :
  - Jeton de page opaque, sûr dans une URL, non interprétable par le client.
    Il n'accorde aucun droit : l'autorisation est revérifiée à chaque page. La taille
    de page n'est jamais obligatoire, elle a un plafond, et une valeur négative est
    refusée. Un jeton vide marque la fin. Un total est permis, éventuellement estimé ;
    un jeton peut expirer (environ 3 jours en règle empirique).
  - Paginer toute collection, préférer le curseur, éviter le total.
  - Lien de page suivante opaque ; documenter les doublons ou manques
    possibles entre pages ; mêmes filtres et même tri d'une page à l'autre. Ajouter la
    pagination plus tard casse les clients.
- Raison de montée en charge (déduite, non démontrée par ces guides) : un décalage
  (`offset`) fait lire puis écarter les lignes précédentes et glisse quand une écriture
  passe entre deux pages. Un curseur sur une clé triée se lit par un index, à coût
  constant.

### Champs partiels, lots, opérations longues, idempotence, compatibilité

- Champs partiels : un masque de champs facultatif (sans masque : la vue
  par défaut) et des vues nommées ; un champ ne sort jamais d'une vue qui ne le contient
  pas ; sélection (`fields`) et expansion (`embed`) recommandées. Raison : moins d'octets sérialisés et de colonnes lues.
- Lots : un lot atomique, tout ou rien (MUST), une réponse dans l'ordre de
  la demande, un maximum documenté et pas de pagination. Raison : un aller-retour au
  lieu de N, et la borne protège la mémoire.
- Opérations longues :
  - Renvoyer une ressource d'opération plutôt que le résultat ; expiration
    facultative.
  - 202 si le p99 dépasse 1 s, `Retry-After` tant que
    l'opération n'est pas finie, statut gardé au moins 24 h, jamais de PATCH en
    opération longue.
  - Raison : la connexion et le répartiteur sont libérés, et le travail survit au délai
    d'attente du client.
- Idempotence :
  - Draft `Idempotency-Key` -07 (**expiré**) :
    - la valeur MUST être une String Structured Field ;
    - une clé MUST NOT être réutilisée avec une charge différente ;
    - le serveur MUST publier sa politique (durée de rétention) ;
    - erreurs SHOULD : clé absente → 400, même clé avec une autre charge → 422, doublon
      encore en cours → 409 ;
    - un rejeu après la fin renvoie le résultat précédent.
  - Guides : un identifiant de requête garantit l'idempotence et un doublon renvoie la
    réponse précédente, sans durée imposée.
- Compatibilité : les ajouts sont permis dans une même version majeure. Sont cassants :
  rendre un champ obligatoire, le retirer, le renommer, en changer le type ou la valeur
  par défaut. Ne pas casser les clients, et les préparer aux extensions. Avec RFC 9457 §3.2, le client tolérant devient la
  règle.

### Limitation de débit

- 429 (RFC 6585 §4), avec `Retry-After` ; `Retry-After` prime sur tout autre champ
  (draft ratelimit §7, MUST).
- Draft ratelimit-headers -11 :
  - champs `RateLimit-Policy` (paramètres `q`, `qu`, `w`, `pk`) et `RateLimit` (`r`,
    `t`, `pk`), en Structured Fields (RFC 9651), par exemple
    `RateLimit-Policy: "burst";q=100;w=60` et `RateLimit: "default";r=50;t=30` ;
  - les anciens `RateLimit-Limit`, `RateLimit-Remaining` et `RateLimit-Reset` ne sont
    plus définis, et `X-RateLimit-*` est hérité ;
  - un champ malformé MUST être ignoré, et une valeur annoncée n'est pas une garantie.
- Guides : 429 avec en-têtes de limite.

### Authentification et CSRF

- RFC 6750 :
  - §2.1 : `Authorization: Bearer` (MUST côté serveur) ; jeton hors de l'URL (§2.3,
    §5.3) et TLS (MUST).
  - §3 : `WWW-Authenticate: Bearer`, avec `error` SHOULD quand un jeton a échoué ; sans
    tentative d'authentification, le serveur SHOULD NOT mettre de code d'erreur.
  - §3.1 : `invalid_request` → 400, `invalid_token` → 401, `insufficient_scope` → 403.
- RFC 7617 : `realm` REQUIRED, `charset="UTF-8"` seule valeur permise ; deux-points
  interdit dans l'identifiant ; sans TLS, SHOULD NOT (§4).
- Fetch Metadata §2.3 : `Sec-Fetch-Site` vaut `cross-site`, `same-site`, `same-origin`
  ou `none`. Le document ne définit pas de politique d'isolation.
- rfc6265bis-22 : un cookie sans `SameSite` valide se comporte comme `Lax`.

### Mises à jour partielles, dates, flux

- RFC 7396 (`application/merge-patch+json`) : `null` supprime un membre, un tableau se
  remplace en entier ; ne convient pas à un document qui porte des `null` significatifs
  (§1).
- RFC 6902 (`application/json-patch+json`) : six opérations, dont `test` ; une erreur
  arrête l'évaluation, et un PATCH est atomique.
- RFC 9535 (JSONPath) sélectionne seulement, sans rien modifier.
- RFC 3339 §1 : les intervalles ne sont pas couverts. L'ABNF `duration` et `period` de
  l'annexe A est informative et renvoie à ISO 8601, qui est payante et sans source
  libre. Voir aussi [noms-dates-texte.md](noms-dates-texte.md).
- NDJSON : **aucune RFC** ; le dépôt communautaire propose `application/x-ndjson`, un
  JSON par ligne. JSON Lines : **non consulté**.

### Transport : HTTP/2, HTTP/3, compression

- RFC 9113 : plusieurs flux sur une connexion (§5), plafonnés par
  `SETTINGS_MAX_CONCURRENT_STREAMS` (§6.5.2), en-têtes compressés (HPACK, §4.3). Le
  blocage en tête de ligne de TCP demeure (§1).
- RFC 9114 : un flux QUIC par échange ; un flux bloqué n'arrête pas les autres.
- Conséquence : plusieurs petites requêtes coûtent moins cher en transport, mais chacune
  repasse par l'authentification, l'autorisation et la base.
- Compression : `gzip` ; `br` (RFC 7932) et `zstd` (RFC 8878), toutes deux
  informatives.

## Pièges

- La RFC 9205 ne contient **aucun** « SHOULD NOT » sur des chemins d'URL figés pour une
  API applicative (§4.4) : c'est un conseil aux auteurs de specs de protocoles.
- **Cache et RBAC** : la même URL donne des contenus différents selon l'utilisateur.
  `Cache-Control: private` est nécessaire, faute de quoi un cache partagé peut servir à
  un utilisateur la réponse d'un autre (RFC 9111 §3.5, §5.2.2). Un ETag calculé sur
  l'objet seul fait de même fuir, par un 304, l'égalité d'un contenu qu'on ne voit
  pas : il doit tenir compte de ce que l'appelant voit.
- `Idempotency-Key` et les en-têtes `RateLimit` ne sont pas des RFC : adopter le premier
  revient à suivre un draft expiré, le second un draft qui change encore.
- Le 422 de la RFC 9110 s'appelle « Unprocessable Content », plus « Entity ».
- Le 202 n'implique ni `Location` ni ressource de statut : c'est un choix de l'API.
- Un total exact sur une liste filtrée par RBAC coûte un parcours complet, et peut
  révéler le nombre d'objets invisibles s'il est mal calculé.
- Un PATCH en Merge Patch ne sait pas dire « mettre ce champ à `null` » si `null`
  signifie aussi « supprimer ».

## Implications pour kartenn (propositions, à trancher dans la spec 006)

Légende : **[N]** normatif (RFC, BCP) ; **[G]** consensus des guides publics ; **[T]**
choix à trancher.

1. [N] Sans état (D9) : aucune affinité de session, toute requête autonome ; le cookie
   BFF chiffré (ADR 0020) le permet déjà.
2. [N] Codes et méthodes avec leur sens enregistré (RFC 9205 §4.5, §4.6). Erreurs en
   `application/problem+json`, `type` propre à kartenn, extension listant les
   violations (le pipeline en produit plusieurs, REQ-004-7).
3. [N] Cache : toute réponse authentifiée porte `private`. [T] `private, no-cache` avec
   ETag (revalidation, 304) pour les lectures courantes ; ETag dérivé de la version de
   l'objet **et** de ce que l'appelant en voit.
4. [T] Lecture « à la date X » : une réponse sur un instant système passé ne change plus
   (le temps système est immuable, D8, ADR 0017) et pourrait porter `max-age` long et
   `immutable` (RFC 8246), toujours `private`. Pour une date d'application seule, une
   correction rétroactive reste possible : pas d'`immutable`.
5. [N] Concurrence optimiste : `If-Match` sur PATCH et DELETE (412 si l'ETag a changé).
   [T] Obligatoire (428 sinon) ou facultatif ; lien avec les `expected` des opérations
   (REQ-004).
6. [G] Pagination par curseur opaque sur clé triée, taille plafonnée, jeton absent en
   fin de liste, droits revérifiés à chaque page, sans total par défaut. Liens `next`
   dans `Link` et dans le corps. [T] Plafond, durée de vie du jeton, date de lecture
   figée dans le curseur (lecture stable d'une page à l'autre).
7. [G] Lots atomiques bornés : le changeset l'est déjà (D11, `MAX_OPERATIONS`).
   [T] Plafond publié dans l'API.
8. [G] Opérations longues : 202, `Location` vers une ressource d'opération,
   `Retry-After` jusqu'à la fin. Raison : l'import de 10⁶ objets (< 15 min, D14) ne
   tient pas une connexion. [T] Rétention du statut, annulation.
9. [G] Idempotence des POST de changesets : la clé existe déjà dans la requête
   (`idempotency_key`, 004). [T] La porter dans l'en-tête `Idempotency-Key` (draft
   expiré) ou dans le corps ; durée de rétention publiée.
10. [G] Champs partiels et expansion bornée, pour éviter les N+1 sous le budget du
    sous-arbre (200 ms). [T] Syntaxe, profondeur.
11. [N] 429 et `Retry-After` ; 413, 414 et 503 avec `Retry-After`. [T] `RateLimit`
    (draft -11) en plus ; clé de partition (identité, source).
12. [N] Pas de version de HTTP imposée (RFC 9205 §4.1) : HTTP/2 et HTTP/3 se négocient
    au répartiteur. [T] Compression : `gzip`, plus `br` ou `zstd`.
13. [N] Évolution additive, clients tolérants (RFC 9457 §3.2) ; [G] changement cassant
    = nouvelle version majeure (guides). [T] Emplacement de la version : chemin
    (`/v1`), media type ou en-tête.
14. [N] CSRF du BFF : `SameSite=Strict` (ADR 0020), plus `Sec-Fetch-Site` sur les
    méthodes non sûres. [T] Repli quand l'en-tête manque (vieux navigateurs).
15. [T] Réplicas de lecture : hors sujet tant que PostgreSQL n'a qu'un primaire (D15) ;
    la « lecture de ses écritures » se posera avec eux.

## Non trouvé ou non relu

- Aucune norme sur : curseurs, totaux, champs partiels, lots, ressource d'opération,
  lecture sur réplica, webhooks comparés à la scrutation, délais et annulation.
- Non relus : RFC 9110 §12.5.3, §12.5.5, §15.4.5 (304) ; RFC 9113 §8.4 (poussée) ;
  RFC 5861 (`stale-while-revalidate`) ; RFC 9535 §4 ; registre IANA des relations de
  lien ; OWASP (CSRF) ; JSON Lines.
- La note [authentification-autorisation.md](authentification-autorisation.md) donne la
  RFC 7617 comme non relue : elle l'est ici.

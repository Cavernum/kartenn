---
status: approved
updated: 2026-10-23
---

# Interfaces

Les détails normatifs de l'API sont dans la spec 006 (conventions REST, contrat
`specs/006-api-rest/openapi-v1.json`), ceux du shell et des plugins dans la 007 ;
l'authentification et le RBAC sont fixés par la spec 005. Ce document fixe les principes
décidés.

## API REST (D7, ADR 0002, ADR 0022 ; spec 006)

Seule frontière publique : Web, CLI, TUI, MCP et connecteurs passent tous par elle.
Crate `kartenn-rest` (routeur axum, DTO distincts des types du noyau, problèmes,
curseurs, filtres, OpenAPI) ; aucune logique métier : chaque route appelle le service.

| Sujet | Principe | Source |
|---|---|---|
| Transport | HTTPS (TLS terminé par kartenn ou par un proxy de confiance) ; HTTP/1.1 et HTTP/2 ; JSON (RFC 8259) ; compression `zstd`, `br`, `gzip` sauf réponses portant un secret | D13, RFC 9110, REQ-006-31, 32 |
| Versionnement | Version majeure dans le chemin (`/v1`), évolution additive seulement ; retrait annoncé par `Deprecation` (RFC 9745) et `Sunset` (RFC 8594) | 006/H12, Q-ouv-7 close |
| Contrat | OpenAPI 3.1 tiré du code (utoipa), servi à `GET /v1/openapi.json` ; contrat de référence versionné, comparé par un test (retrait ou changement de type → échec) ; `QUERY` en extension `x-query` jusqu'à OpenAPI 3.2 ; SDK clients générés (TypeScript, Rust) ; clients Python dérivés à l'exécution (ADR 0035) ; ETag fort et 304 du contrat, extension `x-kartenn` (spec 023) | D7, REQ-006-34 |
| Ressources | Collections à plat (`entities`, `sites`, `domains`, `prefixes`, `ranges`, `addresses`…), un objet par UUID canonique ; clé naturelle en filtre ; objets complets avec un résumé des objets liés ; ni `fields=` ni `expand=` | 002/A9, REQ-006-1, 2 |
| Écriture | `POST /v1/changesets` (opérations multiples, atomiques) ; routes de ressource (`POST`, `PATCH` en JSON Merge Patch RFC 7396, `DELETE`) qui enveloppent un changeset d'une opération ; tout changeset soumis est en attente (202) ; `?apply=own` sur toute écriture l'applique dans la même transaction quand l'auteur détient `apply_own` sur tout ce qu'il touche et qu'aucune politique ne le retient (201 ou 200, audits `changeset.state.pending` puis `changeset.state.applied`), sinon 202 avec `reason.code` `changeset.approval.self` ; imports et annulations l'appliquent ainsi d'office | D11, REQ-006-11 à 13 ; 024/REQ-024-10, 13 |
| Simulation | `POST /v1/changesets?simulate=true` → validation, anomalies, différentiel, sans écriture | 003/REQ-003-20 |
| Cycle d'un changeset | Actions en sous-ressources `POST` (`validation`, `rejection`, `withdrawal`, `undo`) ; lecture sous `core.changeset.read` ; `GET /v1/proposals`. Une opération dont le module exige une revue devient une proposition même si l'auteur peut écrire ; `validation` accepte un corps facultatif d'opérations « de revue » (`Module::review_operations`) appliquées avec la proposition, toute autre opération jointe → 422 `core.review.operation` ; le valideur détient `….apply` de tout ce que le changeset touche (son auteur `….apply_own`, refusé quand une politique le retient ou qu'un module exige une revue : 403 `changeset.approval.self`), dont la permission que nomme `ModuleHandler::review` (vérifiés dans la transaction) ; les droits d'édition de l'auteur sont revérifiés | REQ-006-16, REQ-016-10, 11 ; 024/REQ-024-11, 12 |
| Pour le compte d'un demandeur | En-tête `Kartenn-On-Behalf-Of`, dictionnaire RFC 9651 (`sub` exigé, `iss` et `name` facultatifs) : accepté d'un compte de service détenteur de `core.identity.act_on_behalf` sur une écriture ; en-tête mal formé ou sur une lecture → 400 ; tout autre appelant → 403. Le demandeur inconnu devient une identité `requester` (audit `authn.requester.created`) ; les droits restent ceux du seul compte de service ; `on_behalf_of` est porté par le changeset et par chaque événement d'audit de la requête | REQ-016-12, 13 |
| Temps | Paramètres `valid_at` (temps applicatif) et `as_of` (temps système), RFC 3339 avec décalage, « maintenant » par défaut ; passé figé `immutable`, le reste `no-cache` et ETag | D10, REQ-006-5, 6 |
| Concurrence | ETag / `If-Match` (RFC 9110 §13) obligatoire sur `PATCH`, `DELETE`, les actions d'un changeset et le `PUT` d'une cellule de la matrice (ETag fort de la matrice du type de flux) : absent → 428, périmé → 412. ETag fort d'un objet `"<version>.<empreinte du corps>"`, opaque pour le client ; `If-Match` n'en compare que la version (forme nue `"<version>"` acceptée), `If-None-Match` l'étiquette entière. Le corps d'un objet (lecture par identifiant, page de sa collection) porte cette version en `version` (lecture seule), ce que le client compare au `changeset` d'un événement | REQ-006-14, H19, H21 ; 010/REQ-010-34 |
| Idempotence | `Idempotency-Key` sur les `POST` d'écriture : rejeu → même réponse ; contenu différent → 422 ; requête en cours → 409 | REQ-006-15 |
| Erreurs | `application/problem+json` (RFC 9457), `type` en URN `urn:kartenn:problem:<slug>` ; changeset refusé → 422 avec toutes ses violations (`rule`, `severity`, `subjects`, `pointer` RFC 6901) ; objet invisible ou absent → même 404 | 002/REQ-002-37, REQ-006-20 à 23 |
| Pagination | Curseur opaque signé (HMAC, clé du trousseau `Purpose::Cursor`), droits réévalués à chaque page, sans total ; tri stable | REQ-006-7, 8 |
| Filtrage | Paramètres conjonctifs (`champ=…`, `prop.<clé>:<op>=…`), et méthode `QUERY` (RFC 10008) au corps JSON pour les filtres longs ; URL au-delà de 8 Kio → 414 | 003/REQ-003-25, REQ-006-9, 10 |
| Imports | `POST /v1/imports`, morceaux `PUT …/chunks/{n}` (10 000 éléments au plus, renvoi identique idempotent), `start` (202 sans trier : `uploading` → `sorting`, sinon 409 ; efface l'`error` précédente), `resume`, `cancel` (admis jusqu'à `paused`, `sorting` compris) ; tri, découpage en lots et exécution par la tâche de fond `import:<id>` sous bail, reprise au premier lot non appliqué (un tri interrompu reprend sur une autre instance) ; un tri impossible ramène l'import à `uploading` avec `error` (problème `invalid-import` et ses violations), lu par `GET` ; `Retry-After: 5` en `sorting`, `ready`, `running` | 004/REQ-004-28 à 30, REQ-006-17 à 19, 006/H18 |
| Capacités | `GET /v1/capabilities` : modules actifs, versions, propriétés, plugins UI (`ui_plugins` : nom, version, plage de SDK, `base`, `entry`, empreintes SRI par fichier, catalogues, points d'extension), limites (page, changeset, morceau, corps), langue par défaut de l'instance (`default_locale`) | D7, D17, REQ-006-35, REQ-007-7, 018/REQ-018-12 |
| Libellés (ADR 0040) | Corps neutres en langue : un objet porte ses clés et ses `labels` `{langue: texte}` (déclarations de propriétés, valeurs de listes fermées, rôles, objets de la politique), jamais un libellé résolu ; aucun champ `label`. `GET /v1/labels` : table clé → libellé d'une langue, rangée par espace (`value`, `property`, `role`, `permission` — anciens noms compris, « anciennement » —, `rule`, `problem`, `kind`, `state`, `nature`, `usage`, espaces des modules), seule route qui varie selon la langue : `lang` (400 si mal formée) sinon `Accept-Language` (Lookup RFC 4647), repli sur la langue par défaut puis sur la clé (`fallback`), `Content-Language`, `Vary: Accept-Language` ; `spaces` restreint ; ETag de la révision des libellés, de l'empreinte des libellés compilés, de la langue et des espaces lisibles ; `valid_at`, `as_of` (passé : `private, max-age=86400`) ; un espace de données n'est servi qu'aux lecteurs de sa collection, les valeurs d'une propriété sensible à `prop.<clé>.read`, les codes à tout appelant authentifié | 018/REQ-018-7, 14 à 19, R19 à R24 |
| Suggestions | `GET /v1/property-declarations/{key}/values` rend `{key, valid_from, valid_to}` des valeurs valides à `valid_at` (période couvrant leurs versions connues à `as_of`), par clé. `q` (préfixe, insensible à la casse, une fois, 1 à 100 caractères, gardé par le curseur) : clé ou libellé d'une langue quelconque pour les valeurs ; slug (`entities`, `sites`, `domains`, `clusters`, `vlan-groups`), nom (`machines`, `interfaces`), nom ou VID (`vlans`), texte de la valeur (`prefixes`, `addresses` : alors dans l'ordre de ce texte, puis du domaine), sous le prédicat de lecture de chaque liste ; 20 éléments par page quand `q` est donné (sauf `page_size`) ; refusé (400) sur `ranges` (citées par identifiant) et avec un filtre `prop.…`. Les libellés des suggestions viennent du dictionnaire | 018/REQ-018-22, 23, Q-018-8 |
| Règles | `GET /v1/rules` : `{id, severity, module}` de chaque règle du build — noyau (`module` `core`), tri d'un import (`import.*`, 006/H18), modules —, en une page ; `q` sur l'identifiant ; tout appelant authentifié ; libellés par l'espace `rule` du dictionnaire | 018/REQ-018-23 |
| Modules (ADR 0026) | Chaque module monte ses collections sous `/v1` (VLAN : `vlan-groups` avec `occupancy` et `next-free`, `vlans` avec `prefixes` ; machines : `clusters` avec `machines` (membres), `machines` avec `addresses` (adresses par
interface) et `hosted` (machines hébergées), filtre `network` (machines dont une adresse est
dans un réseau), `interfaces` (filtre `address` : interfaces qui portent une adresse) avec `attachments` (rattacher : `POST`, détacher :
`DELETE …/attachments/{address}`), écriture de l'interface et lecture de l'adresse exigées,
009/M16 ; politique, spec 010 : `policy-axes`, `policy-values`, `environments`, `flow-types`, `services`, `policy-patterns`, `policy-rules`, `admissibility-conditions`, `risk-signals`, `policy-exempt-roles`, `inference-rules` rendus par un schéma générique `PolicyObject`, `GET /v1/policy-matrix?flow_type=` (ETag fort : empreinte de la vue) et `PUT /v1/policy-matrix/{source}/{flow_type}/{destination}` sous `If-Match` de cette empreinte (428, 412 ; dans un changeset composé, membre facultatif `if_match` de `policy.matrix.set`, écart → `policy.matrix.stale`), `QUERY` et `GET /v1/verdicts` (extrémité invisible → 404, explication réduite sous `policy.flow.request`, plus de 64 adresses par extrémité ou de 10⁴ évaluations → `policy-query-too-large` 422), `POST /v1/policy-reports` (409 si un rapport tourne), `GET …/{id}` et `…/items` ; flux, spec 011 : `GET` et `QUERY /v1/flows` (filtres de REQ-011-14, état dérivé, datés ; `state` accepte une liste séparée par des virgules ; avec `requested` cité, `review=pending` ou `changeset=<id>`, les demandes et prolongations en attente à `as_of` suivent les flux appliqués, F19 : ligne `id` nul pour une demande, `id` du flux pour une prolongation, membre `review` — changeset, `kind` `request` | `extension`, triage lu dans l'opération complétée, évaluations par paire comprises —, lue sous `flow.read` ou `flow.review` sur l'entité ou par son demandeur, triage réduit pour ce dernier ; filtres de préfixe, machine et axe : exclues), `POST /v1/flows` (demande : 201 avec le flux et ses évaluations si elle s'applique, 202 avec le seul changeset si elle part en instruction), `GET`, `PATCH` (raccourcir : 200 ; prolonger, fin plus tardive ou `end: null` d'un flux daté non terminé : triée sur la portion ajoutée, 200 si automatique, 202 `ChangesetPending` à instruire ; flux terminé → `flow.update.ended`, F20) et `DELETE /v1/flows/{id}`, `GET /v1/flows/{id}/evaluations`, `POST /v1/flows/{id}/waiver`, `GET /v1/flow-nonconformities` (une page, sans curseur), `GET /v1/{addresses,prefixes,interfaces,machines}/{id}/flows` ; un flux se lit sous `flow.read` ou par son demandeur, avec des évaluations réduites (adresses et élément décisif masqués) ; l'écriture directe de maintenance (`flow.flow.register`) passe par `POST /v1/changesets`) aux conventions ci-dessus ; le noyau fournit à chaque type de module `history`, `properties`, `explanation` et `suggestion` ; `GET /v1/graph/edges` sert les arêtes des modules filtrées par les droits sur les deux extrémités ; `GET /v1/changesets/{id}/diff?section=<module>.<nom>` pagine une section de module ; contrat de référence écrit par `kartenn-server openapi` | 008/REQ-008-11, 19 à 22, 26 ; 009/REQ-009-15 à 18 ; 010/REQ-010-27 ; 011/REQ-011-18 |
| Ajouts pour l'interface | `GET /v1/me` (identité, liaisons, permissions globales : masquer ou griser, le serveur reste seul juge) ; `GET /v1/domains/{id}/roots` (racines de l'arbre, mêmes curseurs que `prefixes/{id}/children`) ; `GET /v1/imports` (imports de l'appelant, du plus récent) ; `GET /v1/auth/issuers` publique (identifiants des émetteurs, ordre de la configuration) | REQ-007-39 à 41, 007/W17, W18 |
| Protection | Quotas GCRA par identité (par adresse avant authentification) → 429, `Retry-After`, `RateLimit` ; délestage → 503 ; corps borné → 413 ; délai de requête → 503 | REQ-006-29 à 31 |
| Corrélation | `Request-Id` (UUID v7 généré, jamais repris) en réponse, dans le problème, les logs et l'audit ; `traceparent` des proxys de confiance | REQ-006-37, REQ-012-7 |

Ordre des couches (REQ-006-22) : identifiant, délestage, délai, transport (`Forwarded`
des proxys de confiance seulement), limite de corps, authentification, quota, CSRF,
route, compression. Une requête non authentifiée est refusée (401) avant la lecture de
son corps.

## Authentification (D13, ADR 0009 amendée par 005/K8, ADR 0020 ; spec 005)

- **Personnes, navigateur** : BFF (ADR 0020). kartenn est client confidentiel OIDC de
  chaque émetteur (code d'autorisation, PKCE S256 RFC 7636, `state`, `nonce`, paramètre
  `iss` RFC 9207) ; le navigateur ne reçoit qu'un cookie de session chiffré (AES-256-GCM,
  trousseau partagé par les instances), sans état côté serveur. Inactivité 30 min,
  durée absolue 8 h (réglables, au plus 60 min et 24 h) ; groupes rafraîchis par le
  jeton de rafraîchissement de l'émetteur.
- **Personnes, clients non navigateurs** : jetons d'accès JWT de l'émetteur (RFC 9068 :
  `typ` `at+jwt`, `iss` exact, audience de kartenn, algorithme de la liste fermée de
  l'émetteur, `exp` à 60 s près) ; clés JWKS en cache, rechargées sur `kid` inconnu au
  plus une fois par minute.
- **Identités** : une par (émetteur, `sub`), créée à la première connexion avec le seul
  nom affiché (pas d'adresse électronique). Désactivation et révocation des sessions
  par changeset, effectives à la requête suivante (cache d'état de 5 s par instance au
  plus).
- **Machines** (CLI, connecteurs, scripts) : comptes de service en **Basic** (RFC 7617)
  sur TLS ; secret de 256 bits généré par kartenn, montré une fois, stocké en empreinte
  HMAC-SHA-256 sous le trousseau (K8, pas d'Argon2id : le secret est aléatoire) ; deux
  secrets actifs au plus (rotation), expiration, révocation immédiate.
- **Secours** : comptes locaux créés hors de l'API par le propriétaire du schéma
  (`kartenn-server breakglass create`), valables 1 à 24 h ; phrase de passe (Argon2id,
  RFC 9106) **et** code TOTP (RFC 6238), un code jamais accepté deux fois ; 100 échecs
  consécutifs désactivent le compte ; chaque usage audité en criticité haute.
- **Refus** : une seule réponse à l'appelant ; la raison codée (`state`, `iss`, `nonce`,
  `signature`, `expired`, `audience`, `secret`, `totp`, `disabled`, `revoked`…) et
  l'émetteur vont à l'audit (`authn.login.failed`), l'identifiant revendiqué en empreinte.
- Couche HTTP (spec 006) : `GET /v1/auth/login?issuer=…`, `GET /v1/auth/callback`,
  `POST /v1/auth/logout`, `POST /v1/auth/breakglass` (session de secours d'une heure
  au plus) ; cookies `__Host-kartenn-N`, `Secure`, `HttpOnly`, `SameSite=Strict`,
  `Path=/`, sans `Domain`, réémis après une minute d'activité ; `Authorization: Bearer`
  (jeton d'accès, identité juste-à-temps sans événement) ou `Basic` (compte de
  service) ; deux preuves à la fois → 400 ; sans TLS, Basic → 401 et aucun cookie émis ;
  jeton refusé → 401 `WWW-Authenticate: Bearer error="invalid_token"` (RFC 6750 §3.1) ;
  échecs limités par adresse ; CSRF sur les méthodes non sûres par cookie
  (`Sec-Fetch-Site`, puis `Origin`) → 403 ; pas de CORS (BFF de même origine).

## Autorisation (D36, 003/P6–P7, ADR 0010, ADR 0021 ; spec 005)

- Catalogue des permissions généré : [`permissions.md`](permissions.md)
  (`core.<objet>.<action>`, `prop.<clé>.<action>`, `prop.*.<action>` hors propriétés
  sensibles, `*`) ; rôles livrés `reader`, `editor`, `approver`, `administrator`,
  synchronisés par le job de migration et non modifiables.
- Rôle = ensemble de permissions ; liaison = sujet (identité, groupe d'un émetteur,
  émetteur entier) × rôle × périmètre (global, entité, site, domaine, sous-arbre de
  préfixes), héritée vers le bas. **Refus par défaut**, pas de règle de refus.
- **Confinement** : un émetteur rattaché à des entités n'accorde rien hors de leurs
  sous-arbres, quelles que soient les liaisons ; rattachement global possible. Les
  rattachements vivent en base, pas dans la configuration.
- Une lecture sans droit répond l'objet absent (`authz.access.denied` audité) ; les
  listes (`filter_in`, `children_in`) sont filtrées par le même prédicat SQL que la
  décision ; une valeur sensible sans `prop.<clé>.read` est retirée et sa clé listée.
- Une écriture exige `core.<sorte>.edit` sur chaque objet visé, décidé dans la
  transaction du changeset, **et** le droit sur les effets indirects (différentiel des
  propriétés, `prop.<clé>.edit`) ; le changeset soumis est en attente. L'appliquer
  exige, sur les mêmes objets et effets, `….apply` (changeset d'un autre) ou
  `….apply_own` (le sien) ; une permission d'une autre action (`core.role.admin`,
  `flow.request`) vaut les deux (spec 024).
- Administration par changesets : rôles, liaisons, rattachements, comptes de service,
  état des identités ; un administrateur délégué n'accorde que ce qu'il détient sur le
  périmètre (`core.binding.escalation`). L'annulation inverse les rôles, liaisons et
  rattachements par des opérations ordinaires (`CloseRole`/`SetRole`,
  `Unbind`/`Bind`, `DetachIssuer`/`AttachIssuer`), soumises aux mêmes contrôles ; une
  liaison rendue reçoit un nouvel identifiant ; ce qu'un changeset plus récent a écrit
  est un conflit laissé de côté (005/K16). Comptes de service, secrets et état des
  identités ne s'annulent pas : listés comme non pris en charge.
- Politiques d'approbation dans le même modèle : périmètre (domaine, entité, site,
  sous-arbre), auteur (personne, rôle, groupe), rôle lié (`bound_roles`, par exemple un
  second administrateur pour toute liaison du rôle administrateur) ; les modifier exige
  `core.approval_policy.admin`.
- Budgets : tenus avec 1 000 liaisons et 50 groupes (banc `bench-rbac`, 005/K15).

## Shell Web et plugins UI (D17, ADR 0011, ADR 0025 ; spec 007)

**Service** (REQ-007-1 à 3) : kartenn-server sert le shell depuis `[web] root` sur la
même origine que l'API (cookie `__Host-`, CSRF par `Sec-Fetch-Site`). Ordre : `/v1` et
`/health` inchangés ; `/plugins/<nom>/<version>/…` (fichiers relevés seulement) ;
fichier du shell (chemins canoniques sous `root`, `..`, fichiers cachés et liens
sortants refusés) ; repli sur `index.html` pour un chemin sans extension. `GET` et
`HEAD` seulement. Cache : `/assets/*` `immutable`, le reste `no-cache` avec `ETag`.
En-têtes hors `/v1` : CSP `default-src 'none'; script-src 'self' 'sha256-<import map>';
style-src 'self'; img-src 'self' data:; font-src 'self'; connect-src 'self';
manifest-src 'self'; base-uri 'none'; frame-src 'none'; frame-ancestors 'none'`,
`nosniff`, `Referrer-Policy: same-origin`. Les quotas ne comptent que `/v1`.

**Plugins** (REQ-007-4 à 11) : répertoire de l'exploitant `[web] plugins_dir`,
`<nom>/<version>/manifest.json` + modules ES ; manifeste validé (nom, version semver
égaux au répertoire, plage `sdk`, `entry`, catalogues dont `en`, extensions, `module`
facultatif) ; empreintes `sha384` ; version publiée = la plus haute valide ; relecture
toutes les 10 s et sur `SIGHUP`, journaux `web.plugin.loaded`, `web.plugin.rejected`,
`web.plugins.unreadable`. Un plugin dont le module manque au build n'est pas publié.

**Confiance** (W4) : un plugin est du code de confiance de l'exploitant, dans le même
document que le shell, avec tous les droits de l'utilisateur et jamais plus (INV-23).
Installer un plugin revient à lui confier la session de chaque utilisateur ; une
extension non fiable passe par l'API, comme un connecteur.

**Chargement** (REQ-007-9, 10, 13) : compatibilité semver de la plage `sdk` ;
catalogues lus avec leur empreinte ; un `modulepreload` avec `integrity` par module
JavaScript (`.js` seuls ; styles et catalogues sont lus par `fetch` avec leur
`integrity`), puis le script d'entrée ; plugin écarté (`incompatible`, `load-failed`, `exception`,
`timeout` à 5 s) sans bloquer le shell, motif affiché sur `/about`.

**Points d'extension** (W5), déclarés au manifeste et filtrés par type d'objet et
permission : `view` (route `/x/<plugin>/<id>`, entrée de navigation), `tab` (onglet de
fiche), `panel` (colonne latérale de fiche), `column` (colonne des listes), `action`
(menu d'actions, ouverte dans un dialogue).

**SDK** `@kartenn/sdk` (MPL-2.0, Q-ouv-1 close ; W7, W8) : instance unique désignée par
l'import map du shell, `external` au build des plugins ; `SDK_VERSION`, `context`
(utilisateur, `valid_at`/`as_of`, langue), `definePlugin`, `api` (types tirés du contrat
par openapi-typescript, couche `fetch` propre : `QUERY`, `If-Match` exigé par le type,
`Idempotency-Key`, Problem Details, pages), `draft` (opérations du brouillon local),
`tokens` (jetons CSS) ; `@kartenn/sdk/host` réservé au shell ; bac à sable
`@kartenn/sdk/testing`.

**Interface** : contexte temporel dans l'URL avec bandeau hors présent, écritures coupées
sous un `as_of` passé (W9) ; modification rapide par la route de la ressource et
brouillon local (IndexedDB) simulé en continu puis soumis en un changeset (W10) ; i18n
FR et EN, erreurs traduites par leur `type` (W11) ; WCAG 2.2 AA, axe-core bloquant dans
chaque parcours (W14).

## Lectures pour les modules dépendants (spec 009)

Le module `machines` offre aux modules qui en dépendent (la politique de flux, 010) un
port de lecture daté, `kartenn_machines::read`, utilisable dans la transaction d'un
changeset comme en lecture, à (`valid_at`, `as_of`) (REQ-009-22, 25 ; besoins M1 à M10 de
la 010 et de la 011) : adresses d'une machine ou d'une interface avec l'interface et son
rôle résolu (`machines.interface_role`) ; rattachements d'une adresse (interfaces et
machines) ; machines dont une adresse est dans un réseau ; machines hébergées et membres
d'un cluster ; valeurs de propriétés des machines d'un lot d'adresses, avec leur source ;
arêtes du graphe (`machines.has_address`, `machines.hosted_on`, `machines.hosted_in`,
`machines.member_of`). Le port ne filtre pas par les droits : ce sont les routes, ou le
module appelant, qui le font. Le changeset publie la section de différentiel
`machines.attachments` (rattachements ajoutés et retirés, REQ-009-23).

## Opérations d'un changeset (interface du service)

Le service expose `submit` et le type `Operation`
(`crates/kartenn-service/src/operations.rs`) : création, modification, clôture et
restauration des objets du noyau ; déplacement et renommage (`Rename`, l'UUID est
conservé) des entités, des sites et des domaines ; déclarations, valeurs de listes et
affectations de propriétés ; liaison d'identifiants externes ; politiques d'approbation ;
administration du RBAC (`SetRole`, `CloseRole`, `Bind`, `Unbind`, `AttachIssuer`,
`DetachIssuer`, `CreateServiceAccount`, `AddServiceSecret`, `RevokeServiceSecret`,
`SetIdentityState`) ; obligations d'administrateur (`AddObligation`,
`RemoveObligation`) ; `Within` pour donner sa propre période à une opération ; et
`Operation::Module`, l'enveloppe des opérations d'un module (`{"op":
"<module>.<op>", …}`, `vlan.create_vlan`, `machines.attach_address`), décodée et
planifiée par le gestionnaire du module (ADR 0026, clause 6). L'API en
lit un miroir (`OperationDto` de `kartenn-rest`, lié par une conversion exhaustive) :
les déclarations, les valeurs et les rôles portent `labels` (`core.label.invalid` :
1 à 200 caractères, étiquette BCP 47, langue par défaut présente) ; `UpdateEnumValue`
remplace les libellés d'une valeur sur la période du changeset, sa clé ne change jamais
(018/REQ-018-9) ;
le DTO est le contrat public, le type du service reste libre d'évoluer.

## Événements sortants (D11, ADR 0018)

CloudEvents 1.0 en JSON, relayés depuis l'outbox au moins une fois, dans l'ordre par
clé d'ordre (domaines d'adressage et clés de verrou des opérations de module ; ADR 0018,
amendement 4) : `org.kartenn.changeset.applied` (changeset, domaines, objets,
auteur, valideur, résumé du différentiel), `org.kartenn.changeset.<état>` pour les
changesets en attente, et `org.kartenn.<module>.<nom>` pour les événements que déclare
un module (REQ-016-15). Le transport (webhook, file de messages) et une `source` propre
à chaque déploiement (aujourd'hui `urn:kartenn:instance`) restent à spécifier : la 006
les a exclus (événements sortants).

## Clients Python et CLI (D7 amendée, ADR 0035 ; spec 023)

- Deux paquets de `clients/python/`, versions et publications PyPI propres : la
  bibliothèque `kartenn` (MPL-2.0 ; `httpx2`, `platformdirs`) et la CLI `kartenn-cli`
  (commande `kartenn`, AGPL-3.0). Paquets Nix `kartenn-py` et `kartenn-cli` du flake
  (complétions bash, zsh, fish et page de manuel installées).
- **Découverte** : aucune ressource de module n'est connue du client. Le catalogue est
  dérivé à l'exécution de `GET /v1/capabilities` et `GET /v1/openapi.json` (conventions
  REST de l'ADR 0026, clause 9 ; extension `x-kartenn` : clé naturelle et colonnes d'une
  collection, commande nommée d'une opération, gabarit `prop.{key}` et liste d'origine
  des valeurs d'un filtre, 006/H20), en cache XDG par révision (version,
  modules, ETag du contrat ; capacités relues après une minute, contrat revalidé par
  `If-None-Match`). Les étiquettes du socle que les capacités listent parmi les modules
  (`core`, `rights`, `audit`…) restent à la racine de la CLI.
- **Authentification** : client public natif par émetteur (`GET /v1/auth/native-clients`,
  champs `native_*` de `[[auth.issuers]]`) : navigateur et redirection de bouclage
  (RFC 8252, PKCE S256) ou flux d'appareil (RFC 8628) ; jetons au trousseau ou dans un
  fichier explicite (`--token-file`, 0600). Comptes de service en Basic, jeton porteur
  (`KARTENN_TOKEN`), compte de secours (`--breakglass`).
- **Écritures** : `Idempotency-Key`, `If-Match` d'une lecture préalable, motif ;
  `--dry-run` et `--into-changeset` traduisent une écriture REST en opération de
  changeset (`create_<x>`, `update_<x>`, `close`, `<module>.<operationId>`) faute de
  correspondance dans le contrat (point ouvert). Imports par morceaux avec reprise.
- **Codes de sortie** : 0 succès, 2 usage, 3 introuvable, 4 authentification,
  5 conflit, 6 refus, 7 indisponible, 8 simulation refusée, 9 verdict refusé
  (`--exit-status`), 10 en attente (`--require-applied`).
- Vérifié bout en bout par `k-py-e2e` (maquette rejouée contre kartenn-server,
  PostgreSQL et kartenn-dev-idp) ; budgets de démarrage par `bench-cli`.

## Flux d'événements (D7 amendée, ADR 0034 ; spec 022)
`GET /v1/events`, `text/event-stream` (WHATWG HTML § 9.2), dans l'API `/v1` :
- Sortes : `ready` (ouverture, `retry:` tiré entre 1 et 3 s), `changes` (objets écrits :
  `changeset` = révision, `objects` typés, `domains` ; au-delà de 1 000 objets ou pour
  les valeurs héritées, `truncated` et `types` ; une tâche de fond donne `task` et
  `event`), `changeset` (état d'un changeset en attente, pour son auteur et ses
  valideurs possibles), `labels` (révision des libellés, 018), `reset` (`too_old`,
  `invalid_cursor`, `overflow`, `rights_changed`), points de reprise (`id:` seul, au
  plus toutes les 30 s). Maintien `: ping` après 15 s de silence. Données : clés et
  révisions seulement, JSON sur une ligne ; schémas dans le contrat
  (`x-kartenn-events`).
- Filtres `types`, `domains`, `objects`, `kinds` (ET, bornés, 400
  `events.filter.invalid`) ; reprise par `Last-Event-ID` ou `last_event_id`, curseur
  opaque `k1.` chiffré (AES-256-GCM, trousseau) qui porte le plancher du lecteur et
  l'identité ; reprise sur toute instance par l'anneau (15 min, 10⁴ événements) ;
  doublons possibles à la reprise seulement, reconnus par l'identité et la révision.
- Authentification de `/v1` ; pour le cookie, `Origin` égal à l'origine publique et
  `Sec-Fetch-Site: same-origin` quand ils sont présents (403 sinon) ; pas de CORS.
- Droits évalués à la remise, par lot (une requête sujets × objets) ; révocation en
  10 s au plus ; le flux n'est pas une activité de session.
- Hors délestage et hors compression ; plafonds 5 000 flux par instance (503) et 16 par
  identité (429) ; arrêt : `retry:` étalé puis fermeture. `Cache-Control: no-store`,
  `X-Accel-Buffering: no`, `Incremental: ?1` en option.
- Capacités : `events` (bornes des filtres, taille d'un événement, plafond par
  identité, durée de l'anneau, maintien) ou `null`.
- Audit par connexion (`events.stream.opened`, `events.stream.closed`) ; au-delà de
  `audit.read_ids_max`, log technique DEBUG `events.delivery`.
## Connecteurs (D1, D7)

- Processus externes, clients REST, authentifiés par compte de service.
- Importent par changesets (lots) ; référencent les objets par identifiant externe
  (source, id externe → UUID, REQ-002-41).

## Configuration et exploitation (D35)

- Configuration 12-factor : fichier TOML, puis variables `KARTENN_*` (imbrication par
  `__`, par exemple `KARTENN_AUDIT__SEAL_INTERVAL_MS`). Une sortie inconnue, un champ
  manquant ou inconnu, un filtre invalide refusent le démarrage
  (`kartenn_server::config`, REQ-012-31).
- Sondes (hors `/v1`, sans authentification) : `/health/live` (200 si l'instance
  tourne), `/health/ready` (version des migrations lue, sorties d'audit saines ; sinon
  503 sans détail).
- Tâches de fond sous bail dans PostgreSQL (ADR 0023) : table `job`, réclamée par
  `FOR UPDATE SKIP LOCKED`, bail de 30 s renouvelé toutes les 10 s, scrutation toutes
  les 5 s ; tâches `expire-pending` (1 min), `reevaluate-expected` (5 min),
  `public-drift` (au démarrage d'une version, puis chaque jour), `idempotency-purge`
  (1 h), et une tâche `import:<id>` par import démarré (tri, puis lots). Minuteries locales à chaque
  instance : purge des caches d'état et des limiteurs, rechargement des JWKS (5 min).
- Base de données (spec 015, REQ-015-4) : section `[database] url_file`, un fichier qui
  contient l'URL `postgres://…`, lu au démarrage ; l'URL est alors absente de la ligne de
  commande (lisible par tout utilisateur de la machine), des logs et des erreurs. Les
  sous-commandes prennent l'URL en argument **ou** depuis ce fichier, jamais les deux.
- Binaire `kartenn-server` (`--version` donne la version) :
  - `migrate [<url>] [--config <fichier>]` : job de migration (D9) ;
  - `run [<url>] [<config>]` : une instance, jusqu'à `SIGTERM` ou Ctrl-C ; à l'arrêt,
    l'événement d'arrêt est scellé et exporté, les files des sorties sont vidées et
    OpenTelemetry est arrêté, dans 10 s (REQ-012-34) ;
  - `audit verify <url> [<clé-publique-hex>]` : vérifie la chaîne des sceaux et leurs
    signatures ; code de sortie 2 si le journal est altéré (REQ-012-14) ;
  - `audit purge [<url>] [<config>]` : rétention par criticité (REQ-012-16), pour une
    minuterie quotidienne (cron, systemd).
  - `breakglass create [<url>] <nom> <heures> <fichier-de-clés> [--config <fichier>]` :
    compte de secours valable de 1 à 24 h, avec la connexion du propriétaire du schéma,
    lié au rôle `administrator` de portée globale jusqu'à sa fin de validité (amorçage du
    premier administrateur, REQ-005-14, K17) ; affiche une fois la phrase de passe et
    l'URI `otpauth://` du second facteur.
  - `openapi [<chemin>]` : contrat OpenAPI des routes du build (modules compris).

## Configuration de l'API (spec 006)

**Réglages** (`[http]`, facultatif : sans lui, l'instance ne sert pas l'API et n'exécute
que les tâches de fond) : `listen` (adresse et port), `public_origin` (origine
`https`, ou `http` sur la boucle locale, sans chemin : CSRF, liens absolus,
`redirect_uri` par défaut), `trusted_proxies` (adresses ou blocs CIDR dont `Forwarded`
et `traceparent` sont crus), `max_body_bytes` (16 Mio), `request_timeout_ms` (30 000),
`write_transaction_timeout_s` (60 : `transaction_timeout` du pool, sur lequel repose le
passé figé des lectures), `max_in_flight` (0 = deux fois la taille du pool) ;
`[http.tls]` `cert_file`, `key_file` (PEM, relus sur `SIGHUP` ; absent : HTTP clair
derrière un proxy) ; `[http.rate]` `read`, `write`, `anonymous` en
`{ per_second, burst }` (50/200, 10/20, 20/20).

- Limites et délais nuls, quota nul ou origine invalide refusent le démarrage
  (`kartenn_server::config`).
- Les limites sont par instance : avec N instances, le plafond réel est N fois la
  limite (006/H11).

## Configuration du shell (spec 007)

**Réglages** (`[web]`, facultatif) : `root` (build du shell ; sans lui, pas de shell) et
`plugins_dir` (répertoire des plugins ; sans lui, `ui_plugins` est vide).

- Champ inconnu, `root` sans `index.html` ou sans import map, `plugins_dir` inexistant
  ou illisible refusent le démarrage ; un répertoire de plugins devenu illisible garde
  l'instantané précédent.
- **Avertissement d'exploitation** : déposer un plugin dans `plugins_dir` revient à lui
  confier la session de chaque utilisateur (W4) ; seul l'exploitant y écrit.
- Développement : le processus devenv `web` (Vite) relaie `/v1` et `/plugins` vers le
  processus `kartenn` (`dev/kartenn.toml`), qui fait confiance à l'émetteur de
  développement `kartenn-dev-idp` (`dev/idp.toml`) ; ni l'un ni l'autre n'est livré.

## Configuration de l'authentification (spec 005)

**Réglages** (`[auth]`, facultatif : sans lui, personne ne peut se connecter) :
`key_file` (trousseau, une clé de 256 bits par ligne, la première courante),
`session_idle_minutes` (30, de 1 à 60), `session_absolute_hours` (8, de 1 à 24),
`redirect_uri` (requis avec des émetteurs) et `[[auth.issuers]]` : `id` (minuscules,
chiffres, tirets ; unique), `issuer`, `client_id`, `client_secret_file`, `audience`,
`groups_claim` (`groups`), `algorithms` (`["RS256", "ES256"]`).

- `redirect_uri` et `issuer` sont en `https`, ou en `http` sur la boucle locale pour le
  développement (RFC 8252 §7.3), sans fragment (RFC 6749 §3.1.2) ; un `issuer` sans
  requête (OpenID Connect Discovery 1.0 §3).
- `algorithms` ne prend que des signatures à clé publique (RS, PS, ES, EdDSA) : `none` et
  les HMAC sont refusés (RFC 8725 §3.1 ; choix de kartenn).
- Le trousseau et les secrets des clients sont lus au démarrage : un fichier absent, un
  trousseau mal formé ou un secret vide refusent le démarrage (`kartenn_server::auth`).
- Le rattachement d'un émetteur à une entité vit en base (K11), pas dans la
  configuration.

## Journalisation et audit (spec 012, ADR 0019)

Deux flux distincts : le **journal d'audit**, en base, qui fait foi (qui a fait quoi, avec
quel résultat) ; les **logs techniques**, pour l'exploitation. Le code métier n'émet que
par le port `AuditJournal` et la façade `tracing` ; aucune bibliothèque de sortie
n'atteint `kartenn-core` ni `kartenn-service` (REQ-012-2, test d'architecture).

**Événements d'audit** : catalogue généré
[`evenements-audit.md`](evenements-audit.md) (nom en trois parties, sévérité,
criticité, champs, correspondances OCSF et ECS) et son schéma JSON
[`evenements-audit.schema.json`](evenements-audit.schema.json) (forme exportée, une
branche par événement ; REQ-012-10). Chaque événement est attribué
(personne, compte de service, `system`, ou empreinte HMAC de l'identifiant revendiqué).

**Sorties de l'audit** (`[audit] sinks`, plusieurs à la fois, chacune reçoit chaque
événement au moins une fois, dans l'ordre de `seq`) :

| `kind` | Format | Transport |
|---|---|---|
| `file` | une ligne JSON par événement (la ligne stockée, que les sceaux couvrent) | fichier, synchronisé après chaque lot |
| `syslog` | RFC 5424, JSON dans MSG | TCP ou TLS, comptage d'octets (RFC 6587 §3.4.1, RFC 5425) |
| `otlp` | log OTLP, l'événement en corps structuré | OTLP/gRPC, un `Export` acquitté par lot |

**Réglages de l'audit** (`[audit]`) : `seal_interval_ms` (15 000), `relay_interval_ms`
(1 000), `signing_key_file` (facultatif), `retention_days = { low = 90, normal = 365,
high = 1095 }` (au moins un jour chacune), `read_ids_max` (100 : identifiants enregistrés
par événement de lecture, les autres comptés dans `targets_omitted` ; 0 = critères
seuls ; L25).

**Sorties techniques** (`[telemetry] outputs`, plusieurs à la fois, chacune avec son
filtre `EnvFilter` et sa file bornée de `queue` enregistrements ; une file pleine compte
et abandonne, sans ralentir la requête) :

| `kind` | Format |
|---|---|
| `stdout` | texte ou JSON (`format`) |
| `file` | lignes JSON |
| `journald` | protocole natif (`MESSAGE_ID` dérivé du nom) |
| `syslog` | RFC 5424 ; transport `local`, `udp`, `tcp` ou `tls` |
| `otlp` | logs, traces et métriques OTLP/gRPC vers `endpoint`, ou `OTEL_EXPORTER_OTLP_ENDPOINT` |

- Chaque enregistrement porte un horodatage RFC 3339 en UTC à la microseconde, une
  sévérité (table unique : ERROR 3/17, WARN 4/13, INFO 6/9, DEBUG 7/5, TRACE 7/1, en
  syslog/OpenTelemetry) et un nom stable (`event.name`).
- Événements techniques du pipeline : `pipeline.changeset.<état>` (`applied`, `refused`,
  `pending`, `stale`, `replayed`, `simulated`, `rejected`, `failed`), avec `state`,
  `operations`, `violations`, `duration_ms` ; `audit.write.failed` ; `sys.*` (démarrage,
  arrêt, horloge, panique, filtres).
- Traces : span `pipeline.changeset`, spans `db.*` du store (`db.system.name`,
  `db.operation.name`, `db.query.summary`, jamais de paramètre) ; échantillonnage
  `ParentBased(TraceIdRatio(sampling_ratio))`.
- Métriques : `kartenn.changeset.count{state}`, `kartenn.pipeline.duration`,
  `kartenn.log.dropped{output}`, `kartenn.audit.export.lag{sink}`,
  `kartenn.audit.unsealed`, `kartenn.audit.write.failures`, `kartenn.clock.skew`,
  `http.server.request.duration` (avec l'API).
- Filtre d'une sortie changé à chaud par un administrateur
  (`core.observability.admin`), audité, appliqué par toutes les instances, borné à 24 h
  (REQ-012-27).
- Masquage : valeurs des champs nommés comme un secret (`password`, `token`,
  `authorization`, `cookie`, `secret`, `dsn`, `connection_string`…) et mot de passe des
  URL, sur toutes les sorties, OTLP comprises, et dans le rapport de panique
  (REQ-012-28) ; valeurs bornées à 2 Kio, caractères de contrôle neutralisés
  (REQ-012-29).

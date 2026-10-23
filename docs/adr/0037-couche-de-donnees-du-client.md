---
status: proposed
date: 2026-10-23
---

# 0037 — Couche de données du client web : cache partagé dans le SDK

## Context

Le client du SDK (`web/sdk/src/api/client.ts`, 007/W8) est un `fetch` typé sans cache :
chaque navigation relit ses données, d'où l'attente et les écrans vides. Shell et
plugins lisent les mêmes objets sans rien partager. Le serveur offre déjà de quoi
revalider à bas coût : ETag fort composite d'un objet (`"<version>.<empreinte>"`, 006/H19),
ETag faible d'une page, 304 sur `If-None-Match`, lectures datées immuables quand `as_of`
est passé (`Cache-Control: immutable`, 006). La spec 022 (validée) pousse les changements
par SSE (`GET /v1/events`) et décrit le contrat d'invalidation des clients.

Arbitrages de l'utilisateur du 2026-10-23 : D-5 (`@tanstack/query-core` dans le SDK,
adaptateur fin selon D-1, invalidation par SSE), D-6 (tranché par la 022), D-7 (optimisme
au présent seulement ; 202 « proposé, en revue » ; 412 relit sans perdre la saisie).
Décisions D7 (amendée), D8, D10, D11 ; ADR 0005, 0018, 0022 ; 007/W8 à W10, W17.

## Decision

1. **Un cache unique, dans le SDK.** Le SDK crée **un** `QueryClient` de
   `@tanstack/query-core` (cœur sans framework) et l'expose aux plugins ; shell et
   plugins partagent cache, déduplication et invalidations. La couche HTTP actuelle
   (QUERY, ETag, If-Match, Idempotency-Key, Problem Details, curseurs, lectures datées)
   devient la fonction de requête du cache.
2. **Clés de cache normalisées et datées** :
   `[ 'v1', ressource, paramètres de chemin, filtres normalisés, { valid_at, as_of } ]`,
   le présent noté `null`. Une fabrique de clés par ressource dans le SDK ; aucune clé
   écrite à la main dans un écran.
3. **Fraîcheur par famille** :
   - `as_of` passé : immuable (bitemporel, ADR 0005), `staleTime: Infinity`, `gcTime`
     long borné par la mémoire ;
   - `valid_at` seul ou présent : revalidation normale ; l'ETag est gardé avec la donnée,
     la relecture part en `If-None-Match`, un 304 réutilise la donnée gardée ;
   - listes : pages plafonnées en mémoire, `placeholderData` qui garde la page
     précédente (jamais d'écran blanc).
   L'ETag est **opaque** pour le client : il ne l'analyse jamais (006/H19).
4. **Préchargement à l'intention** : les routes déclarent leurs requêtes ; un lien les
   précharge au survol (délai court) ou au focus, la garde de navigation attend au plus
   environ 100 ms puis affiche la donnée en cache ou un squelette. Au défilement, la page
   suivante d'une liste et les enfants d'un nœud d'arbre visible sont préchargés. Les
   modules de plugins sont préchargés au survol de leur entrée de navigation
   (`modulepreload` avec empreinte, mécanisme existant).
5. **Invalidation** :
   - *ses propres écritures* : la réponse du changeset nomme les objets touchés ; le SDK
     invalide leurs clés, leurs collections et leurs dépendants connus ;
   - *celles des autres* : le SDK tient **un** flux SSE par navigateur, partagé entre
     onglets, et applique le contrat de la 022 (§ « Contrat consommé par les clients ») :
     `ready` invalide tout une fois, `changes` invalide les objets nommés et les listes
     de leurs types (relecture en `If-None-Match`), `truncated` invalide par type et
     domaine, `reset` invalide tout, `changeset` rafraîchit le changeset et les
     propositions ;
   - *repli sans flux* : revalidation au retour sur l'onglet, à la reconnexion et au
     retour du bfcache.
   La comparaison « révision connue = révision de l'événement » se fait sur la `version`
   lue dans le corps de l'objet, jamais sur l'ETag (composite depuis 006/H19).
6. **Optimisme au présent seulement** (D-7) : une modification rapide au présent met à
   jour le cache avant la réponse, annule les lectures en cours de la clé et revient en
   arrière sur erreur. Une réponse **202** (revue requise) remplace l'état optimiste par
   « proposé, en revue », jamais « appliqué ». Un **412** relit l'objet et rend la saisie
   intacte, à fusionner par l'utilisateur. Aucune écriture optimiste hors présent ni sous
   `as_of` (W9). Le brouillon reste local (IndexedDB, W10) ; sa simulation est une requête
   à clé `['simulate', empreinte des opérations]`, débrayée par la saisie.
7. **Mémoire** : `gcTime` par famille, pages de liste plafonnées, base IndexedDB fermée
   sur `pagehide` (bfcache) et revalidation au `pageshow` persistant.

## Résolution de D-1 : Vue 3 (2026-10-23)

- **Adaptateur** : `@tanstack/vue-query`, branché sur le `QueryClient` du SDK
  (`VueQueryPlugin` avec `queryClient` fourni) ; il vit dans `@kartenn/ui`, jamais dans
  `@kartenn/sdk`, qui reste sans framework (REQ-007-12).
- **Routeur** : vue-router ; chaque route déclare `meta.queries(params, time)` ; une garde
  `beforeResolve` lance les préchargements et attend au plus 100 ms.
- **Données volumineuses** : les objets lus pour un arbre ou une grille ne sont pas rendus
  réactifs en profondeur (`shallowRef`, `markRaw`) ; c'est le cache qui porte la
  réactivité (leçon du prototype : enveloppes profondes coûteuses et identité perdue).

## Alternatives

- **Cache idiomatique d'un seul framework** (magasin de requêtes propre à Vue, par
  exemple) : plus naturel dans ce framework, mais un plugin écrit autrement n'en
  profiterait pas.
- **Cache maison** : réécrire déduplication, ramasse-miettes, annulation et
  revalidation.
- **Aucun optimisme** : plus simple, moins réactif. **Optimisme partout** : faux sous
  revue obligatoire et au futur.
- **Interrogation périodique** au lieu du flux SSE : écartée par la 022.
- **Comparer l'ETag à la révision de l'événement** (première rédaction de la 022) :
  incompatible avec l'ETag composite opaque de 006/H19.

## Consequences

- Le SDK passe en **version majeure 2.0** (contrat public, ADR 0025) : `QueryClient`
  partagé, fabriques de clés, flux d'événements, contexte réactif ; seuls des plugins de
  première partie existent, c'est le moment.
- La dépendance `@tanstack/query-core` entre dans le SDK (MIT) ; elle est singleton par
  l'import map, comme le SDK.
- Les tests du cache (Vitest) couvrent clés, fraîcheur, optimisme, 202, 412 et
  invalidation ; un parcours de fraîcheur (deux navigateurs) vérifie l'invalidation par
  le flux.
- Le contrat client de la 022 est précisé sur un point (comparaison sur `version` et non
  sur l'ETag) : amendement à porter dans le design de la 022 (spec 021, arbitrage).

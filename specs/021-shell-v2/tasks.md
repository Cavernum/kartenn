---
status: approved
updated: 2026-10-23
---

# 021 — Shell v2 : tâches

Validée le 2026-10-23 ([`decisions.md`](decisions.md)). Chaque tâche est un commit, laisse le build vert
(`k-web-check`, `k-web-e2e` pour les écrans, `k-check` pour les tâches qui touchent le
serveur) et nomme ce qu'elle implémente ; chacune porte `où :` et `dépend de :`.
D-1 est tranché (Vue 3, tâche 1 faite) : plus aucune tâche ne l'attend. Bascule en un bloc
(design, § « Migration ») : tout le code nouveau va sous `web/v2/` ; les chemins `web/shell`,
`web/sdk`, `web/plugins`, `web/e2e` des lignes `où :` désignent l'existant à reprendre ou à
imiter, dont la copie adaptée vit sous `web/v2/` (même arborescence) ; l'ancien shell ne
reçoit que des correctifs. Commandes lourdes
par `k-heavy`.

Ordre (design, § « Migration ») : la 020 jusqu'à sa tâche 15 (maquettes validées) ; P1
(SDK 1.0) et P1 bis (côté serveur) en parallèle de la 020 ; P2 (écrans) par zones, chacune après les composants qu'elle emploie (020, P4) ;
P3 (plugins) ; P4 (budgets, liste de parité, commit de bascule 27, puis synchronisation 23). Trois sous-agents d'implémentation au plus, et
aucun en parallèle sur `@kartenn/ui` (020).

## P0 — Décision du framework

- [x] 1. Prototype D-1 : la même grille éditable (10⁴ lignes virtualisées, édition au
  clavier, filtres typés, erreur codée) et le même arbre éditable (10⁵ nœuds, 1 000
  enfants sous un nœud, déplacement au clavier) en Vue 3 + Reka UI et en React 19 + React
  Aria ; mesures : latences, trames perdues, tas, poids gzip ; violations CSP ; axe ;
  scripts APG — D-1, REQ-021-30 — verify : tableau comparatif, chiffres reproductibles
  - où : [`docs/spikes/005-framework-web.md`](../../docs/spikes/005-framework-web.md)
  - dépend de : —
  - fait le 2026-10-23 : **Vue 3** retenu par l'utilisateur ; D17 et ADR 0011 inchangées ;
    leçons reprises dans l'ADR 0036 et la 020 (REQ-020-33) ; budgets fixés à partir des
    mesures (REQ-020-29, REQ-021-30 : tas ≤ 64 Mo, Q-021-7 close).

## P1 — SDK 1.0 (phase 3)

- [x] 25. Espace `web/v2/` : `pnpm-workspace.yaml` et lockfile propres, paquets `sdk`
  (copie de `web/sdk` passée en 1.0.0), `ui`, `shell`, `plugins`, `e2e` vides qui
  compilent ; `stylelint.config.js` ; étape `web/v2` de `scripts/ci.sh` ; commandes devenv
  `k-web2-check` et `k-web2-e2e` (sous `k-heavy`) ; configuration de test de
  kartenn-server avec `[web] root` et `plugins_dir` sur les builds de `web/v2/` — REQ-021-31
  — verify : `scripts/ci.sh` vert avec l'étape `web/v2` ; `[web] root` par défaut sert
  toujours l'ancien shell
  - où : `web/pnpm-workspace.yaml`, `web/package.json` (forme) ; `scripts/ci.sh` ;
    `devenv.nix` `scripts.k-web-check`, `scripts.k-web-e2e` (vers les lignes 494 et 533) ;
    `web/e2e/kartenn.toml`, `web/e2e/serve.sh`
  - dépend de : —
  - fait : espace, lockfile, `ui`, `shell`, `e2e`, lint, étape de `ci.sh` et commandes
    devenv avec la 020 (2026-10-23) ; paquet `sdk` copié le 2026-10-24 (1.0.0, MPL-2.0,
    `LICENSE`, contrat généré depuis `web/v2/sdk/scripts/gen.mjs`, test du paquet sur
    `tools/notices.ts`) ; `tokens` retiré du SDK (les jetons sont ceux de `@kartenn/ui`).
    Le 2026-10-29 : `plugins/*` dans l'espace, premier paquet `web/v2/plugins/example`
    (`@kartenn/plugin-example` 1.0.0, MPL-2.0, manifeste `sdk: "^1.0"` sans extension,
    `definePlugin` vide, build vers `web/v2/dist/plugins/example/1.0.0/`, test du paquet) ;
    `web/v2/e2e/kartenn.toml` (`[web] root = "web/v2/dist/shell"`,
    `plugins_dir = "web/v2/dist/plugins"`), `web/v2/e2e/serve.sh` (`idp`, `kartenn`),
    `journeys.config.ts` et parcours `journeys/served.spec.ts` (shell servi sous la CSP du
    serveur, carte d'import à trois entrées, aucune violation ; manifeste du plugin publié) ;
    `k-web2-e2e` lance ces parcours, étape `e2e-v2` de `ci.sh`. Vérifié : `k-web2-check`
    (318 tests, 1308 captures), `k-web2-e2e` (4 passés, Chromium et Firefox), `k-web-check`
    (170) et `k-ci-lint` verts ; `[web] root` par défaut (`web/e2e/kartenn.toml`,
    `nix/module.nix`, OCI) inchangé, sur l'ancien shell.
  - choix : `plugins` est un dossier de paquets (`plugins/*`, comme `web/` et la 017 :
    `web/v2/plugins/<nom>`, règle ESLint `plugins/*/src/**` déjà posée), pas un paquet
    unique ; le paquet vide est l'amorce de l'exemple, que la tâche 19 remplit. Le build
    externalise les trois modules de la carte d'import (`@kartenn/sdk`, `@kartenn/ui`,
    `vue`). `plugins_dir` pointe directement sur `web/v2/dist/plugins` (pas de
    rassemblement sous `.state` tant qu'aucun plugin de test n'est porté). Le matériel
    (certificat, trousseau, émetteur de `web/e2e/users.toml`) reste celui de
    `web/e2e/.state` jusqu'au portage des parcours (tâche 26) ; base `kartenn_e2e_v2`,
    preuves de secours dans `web/e2e/.state/breakglass-v2.txt` ; mêmes ports que `web/e2e`,
    sérialisés par `k-heavy --pg --all`. Les parcours ont leur propre configuration
    Playwright (`journeys.config.ts`) : les `webServer` sont globaux à une configuration,
    et les captures de `k-web2-check` ne doivent ni compiler kartenn-server ni attendre
    PostgreSQL ; `playwright.config.ts` reste celle des captures.

- [x] 2. Cache : `@tanstack/query-core` dans le SDK, `queryClient` unique, fabriques de
  clés (`keys.object`, `keys.list`, `keys.sub`, `timeKey`), fonction de requête sur le
  client existant avec `If-None-Match` et réutilisation sur 304, fraîcheur par famille ;
  `SDK_VERSION` 1.0.0 — REQ-021-21, 25 — verify : Vitest : clés normalisées (ordre des
  filtres, présent = `null`), `as_of` passé jamais relu, 304 → donnée gardée sans
  nouveau rendu
  - où : `web/sdk/src/api/client.ts` (`createClient`, réponse `{data, etag}`),
    `dated.ts` ; `web/sdk/src/index.ts` (exports) ; `web/sdk/package.json` ; design
    § « Clés et fraîcheur »
  - dépend de : 25
  - fait le 2026-10-24 : `web/v2/sdk/src/data/` (`keys.ts`, `cache.ts`, `types.ts`,
    `filters.ts`) ; `@tanstack/query-core` épinglé en 5.102.4 (version de deux mois).
    Choix notés : les fabriques de requêtes s'appellent `reads` (`object`, `list`, `read`,
    `sub`, `subPages`) pour ne pas heurter le type `queries` du contrat ; une liste se lit
    page à page seulement (`read` la refuse : mêmes formes de clé) ; une clé d'objet porte le
    type qualifié (`core.prefix`, celui du flux), une liste le chemin de sa collection, et la
    table type → collection (cœur et modules de première partie, `registerType` pour un
    tiers) vit dans le SDK ; les dates d'une clé sont celles du contexte sauf sur un chemin
    non daté (présent) ; `client.request` lit par URL sans date du contexte, 304 compris ;
    `ApiProblem.code` rend le slug. Modèle de filtres (`filterModel`, `parseFilters`,
    `filterSearch`) et type `FiltersOf<P>` tirés du contrat généré. Vitest :
    `test/cache.test.ts`, `test/filters.test.ts`.
- [x] 3. Dépendants et version : extraction des références `{type, id}` d'un corps
  (schémas de résumé du contrat), index `id → clés`, comparaison sur `version` ; relevé,
  par ressource, de la présence de `version` dans le corps — REQ-021-23 — verify : Vitest
  sur des corps du contrat (préfixe avec résumé de domaine, fiche de machine) ; tableau
  des ressources sans `version` consigné dans le design
  - où : `web/sdk/src/api/schema.d.ts` (`components['schemas']`) ;
    `crates/kartenn-rest/src/dated.rs` (`ETag::record`, `own_version`) pour la règle
    serveur ; 006/H19
  - dépend de : 2
  - fait le 2026-10-24 : `web/v2/sdk/src/data/dependents.ts` (`refsOf`, `versionOf`, index
    `Dependents` tenu par les notifications du cache, oubli à l'éviction) et
    `invalidate.ts` (objets, listes de leur type, dépendants ; `truncated` par type, relu
    sans `If-None-Match` ; jamais `as_of`). Relevé consigné dans le design (§ « Clés et
    fraîcheur ») : aucun corps d'objet du contrat ne porte `version` aujourd'hui, la
    relecture conditionnelle fait donc tout le travail. Choix noté : l'index est par
    identifiant seul (UUID uniques entre types). Vitest : `test/dependents.test.ts`.
- [x] 4. Écritures : `write.quick` (annulation des lectures de la clé, optimisme au
  présent, retour arrière, issues `applied`/`proposed`/`conflict`/`invalid`/`forbidden`),
  invalidation de ses propres écritures (objets de la réponse, collections, dépendants) ;
  `draft.add` inchangé — REQ-021-16, 24 — verify : Vitest : 200, 202 (jamais
  « appliqué »), 412 (relecture, saisie rendue), 422, 403 ; aucun optimisme sous
  `valid_at` ou `as_of`
  - où : `web/shell/src/edit/submit.ts` (logique actuelle de la modification rapide) ;
    `web/sdk/src/draft.ts`, `host.ts` ; `web/sdk/src/api/problem.ts`
  - dépend de : 3
  - fait le 2026-10-24 : `web/v2/sdk/src/data/write.ts`, singletons dans
    `src/instance.ts`. `write.quick` : annulation des lectures de la clé, optimisme par
    *merge patch* (RFC 7396) au présent seulement, retour arrière sur 202, 403, 422 et
    échec ; 412 relit la clé et rend `current` avec la saisie ; refus sous `as_of`
    (exception), `If-Match` pris de l'entrée du cache. Après 200 : objet, cascades, listes
    du type et dépendants invalidés. Choix notés : une sixième issue `failed` (réseau ou
    autre problème) ; 422 dont une règle refuse pour droits (`*.forbidden`) rendu
    `forbidden` comme dans l'ancien shell ; violations groupées par membre (`fieldOf`).
    Client du changeset d'un brouillon : `changesets.simulate` (requête `['simulate',
    empreinte]`, `staleTime` 0, `gcTime` 1 min, 422 rendu comme rapport) et
    `changesets.submit` (clé d'idempotence fournie, 409 retenté cinq fois, objets nommés et
    toutes les listes invalidés) ; le stockage IndexedDB du brouillon reste au shell
    (tâche 14). `draft.add` inchangé. Vitest : `test/write.test.ts`.
  - amendé par la 024 (2026-10-24) : les brouillons passent au serveur ; `sdk.drafts`,
    `sdk.stats` et la fin du stockage IndexedDB sont décrits au design § « Écritures » et
    se font avec la tâche 14 ; `write.quick` envoie `?approval=self`.
  - fait le 2026-10-24 (partie SDK de la 024) : `web/v2/sdk/src/data/drafts.ts`, singleton
    `drafts` de `src/instance.ts`, API `sdk.drafts` et `sdk.stats`. Lectures : liste et
    brouillon par chemin (`keys.list('/v1/drafts')`, `keys.list('/v1/drafts/{id}', …,
    {id})`), familles `drafts` et `stats` (30 s, 10 min) ; `stats.query()` lit
    `GET /v1/stats`. Écritures : `create`, `update`, `addOperations`, `remove` sous
    `If-Match` pris de l'entrée du cache (lue d'abord s'il n'y en a pas), réponse remise
    dans le cache avec son `ETag`, liste invalidée ; 412 sur un ajout → relecture puis un
    seul nouvel essai, sinon issue `conflict` avec la relecture ; 404 → le brouillon quitte
    l'onglet. `simulate(brouillon, période)` : requête `['simulate', 'draft', id, version,
    empreinte]` dont la fonction attend 400 ms sous le signal d'annulation — la version
    suivante (autre clé) abandonne l'attente de la précédente, d'où le débrayage sans
    minuterie du shell. `submit(id, { approvalSelf, period, idempotencyKey })` :
    `?approval=self` si demandé, `If-Match` si l'`ETag` est connu, 409 retenté cinq fois ;
    201 et 202 → brouillon oublié (le serveur l'a supprimé), 422 → il reste. Brouillon
    actif : identifiant sous `kartenn.draft.active` en `sessionStorage` (`active`,
    `setActive`, `onActive`), rien sans `sessionStorage`. `draft.add` (API des plugins)
    ajoute au brouillon actif, en crée un s'il n'y en a pas ou s'il a disparu (404) ;
    `draft.operations`, `remove`, `subscribe` et `draftHost.set` retirés (pas de
    compatibilité avant le gel) ; il n'y avait pas de stockage IndexedDB dans `web/v2`.
    Événements : `draft` et `stats` ajoutés au flux (identité `draft:<id>:<version>`,
    `stats` sans identité) ; `draft` → brouillon relu si sa version diffère, liste relue ;
    `deleted` → brouillon oublié et signal `onGone({ id, active })` ; `stats` → `/v1/stats`
    invalidé. `write.quick` envoie `?approval=self` ; l'issue `proposed` porte `reason`
    (`changeset.approval.self.missing`, `changeset.approval.self`). Choix notés : une issue
    `saved` (le brouillon et son `ETag`) et `removed` pour les écritures de brouillon ;
    titre d'un brouillon créé par `draft.add` = minute de création, remplaçable par le
    shell (`setDraftTitle` de `@kartenn/sdk/host`) ; un brouillon que l'onglet supprime
    ou soumet lui-même ne lève pas `onGone` (son événement `deleted` est attendu) ; le
    brouillon actif est remis à zéro par le SDK quand il disparaît, le shell n'a qu'à
    quitter la vue sur `active: true`. `changesets.simulate` et `changesets.submit`
    (changeset sans brouillon) restent. Vitest : `test/drafts.test.ts`, ajouts à
    `test/write.test.ts`, `test/events.test.ts`, `test/plugins.test.ts`.
- [x] 5. Flux d'événements : `events` (ouverture avant les lectures, contrat de la 022,
  doublons, reconnexion avec attente croissante et `last_event_id`), un flux par
  navigateur (`navigator.locks` + `BroadcastChannel`, repli par onglet), repli sans flux
  (focus, reconnexion, `pageshow` persistant) — REQ-021-23 — verify : Vitest avec un
  `EventSource` simulé (`ready`, `changes`, `truncated`, `reset`, `changeset`, doublons,
  401 puis réouverture) ; élection entre deux contextes simulés ; parcours à deux
  navigateurs (renommage visible en < 2 s) ; trois onglets → une connexion
  - où : 022 design § « Contrat consommé par les clients », § « Doublons » ;
    `web/shell/src/draft/tab.ts` (`BroadcastChannel` existant) ; `web/sdk/src/context.ts`
  - dépend de : 3 ; 022-9, 022-10 (point d'accès servi)
  - SDK fait le 2026-10-24 — `web/v2/sdk/src/events/stream.ts` (`EventStream` :
    verrou `kartenn-events`, relais `BroadcastChannel`, repli par onglet, curseur repris en
    `last_event_id`, coupure laissée au navigateur, fermeture → `GET /v1/capabilities`
    puis réouverture à attente doublée plafonnée à 60 s et tirée au hasard, rien sur 401
    avant `restartEvents()`, doublons par identité sur une fenêtre de 1 000) et
    `events/apply.ts` (contrat de la 022 amendé) ; `startData()` de `@kartenn/sdk/host`
    (montage du cache, ouverture, flux rendu sur `pagehide` persistant, tout revalidé sur
    `pageshow` persistant) ; `refetchOnWindowFocus` et `refetchOnReconnect` actifs
    seulement sans flux ouvert ; API `events.subscribe`, `state`, `onState`. Choix
    notés : chaque `ready` invalide tout (pas seulement le premier) ; identité d'un
    `changeset` = changeset et état ; l'onglet détenteur relaie aussi l'état du flux.
    Vitest (`test/events.test.ts`) : `EventSource` simulé, élection entre trois contextes
    simulés, reprise par un autre onglet, doublons, 401 puis réouverture, attente
    croissante, effets sur le cache, cycle bfcache.
  - fait le 2026-11-11 : parcours `web/v2/e2e/journeys/events.spec.ts` contre
    kartenn-server (Chromium et Firefox) : un réseau renuméroté par Alice (appel de l'API,
    hors du cache) se voit dans la liste des préfixes de Bob, dans un autre contexte, en
    moins de 2 s et sans rechargement ; trois onglets d'un contexte n'ouvrent qu'un
    `EventSource` sur `/v1/events`, le changement s'affiche dans les trois ; l'onglet
    détenteur fermé, un autre rouvre le flux (toujours un seul) et les deux restants
    suivent un second changement. Rien de plus dans le SDK : `events/apply.ts` borne déjà
    les arbres relus aux domaines nommés de l'événement (22c, REQ-022-16 amendée).
  - choix : « renommage » pris comme la renumérotation d'un préfixe (`update_prefix`),
    visible dans la cellule de valeur de la liste ; les 2 s comptent depuis la réponse du
    changeset ; connexions comptées dans la page (sous-classe d'`EventSource` posée par
    `addInitScript`, flux `OPEN` sur `/v1/events`) plutôt qu'au réseau : elle dit combien
    sont ouverts à un instant, le réseau combien ont été ouverts (rechargements
    compris) ; reprise par un autre onglet ajoutée au parcours, déjà couverte par Vitest.
  - reste (revue, mineurs) : `openStreams` ne compte que les flux `OPEN` (compter
    `readyState !== CLOSED` verrait un flux parasite en `CONNECTING`) ; l'attente porte sur
    `OPEN`, pas sur le traitement de `ready`, si bien qu'un changement tombé avant `ready`
    est montré par l'invalidation totale et non par `changes` ; après la reprise, l'unicité
    du flux n'est pas revérifiée une fois le second changement suivi ; une relecture au
    focus sous `fallback.active` pourrait en théorie montrer la valeur.
- [x] 6. Amendement du contrat client de la 022 (A3) : comparaison sur `version`,
  invalidation des vues qui montrent l'objet, relecture en `If-None-Match` sans `version`
  — verify : paragraphe amendé, marqué « *amendé par la 021* »
  - où : `specs/022-evenements-sse/design.md` § « Contrat consommé par les clients »
  - dépend de : —
  - fait le 2026-10-23 avec la validation de la spec
- [x] 7. Préchargement : `meta.queries` des routes, `KLink` (survol 60 ms, focus), garde
  de navigation (attente ≤ 100 ms), page suivante et enfants préchargés, modules de
  plugins au survol — REQ-021-22 — verify : parcours : navigation vers une fiche survolée
  sans squelette ; marque `k:view` ≤ 100 ms (banc, tâche 22)
  - où : `web/v2/shell/src/prefetch/` (`intent.ts`, `KLink.vue`, `nextPage.ts`,
    `tree.ts`), `web/v2/shell/src/plugins/preload.ts`, `web/v2/sdk/src/data/prefetch.ts` ;
    test `web/v2/shell/test/prefetch.test.ts` ; design § « Routes », « Préchargement »
  - dépend de : 2
  - en cours : partie SDK faite le 2026-10-24 — `web/v2/sdk/src/data/prefetch.ts` :
    `prefetch.intent(cible, requêtes)` (survol tenu 60 ms, focus immédiat, sortie avant le
    délai sans lecture), `prefetch.guard(requêtes)` (≤ 100 ms, `ready` ou `timeout`),
    `prefetch.prefetch` (lectures et listes). Vitest : `test/prefetch.test.ts`. Reste au
    shell : `KLink`, `meta.queries` des routes et garde `beforeResolve`, page suivante à
    l'approche de la fin, enfants survolés, `modulepreload` des plugins, parcours et banc.
  - en cours : partie shell faite le 2026-10-30 — `web/v2/shell/src/prefetch/` :
    `intent.ts` (`RouteMeta.queries(params, time, query)` et `RouteMeta.modules(params)`
    déclarés ; `timeOf` : dates de la cible, sinon celles du contexte au survol (comme
    `carryTime`), le présent dans la garde (retour explicite au présent) ; `queriesOf`,
    `preloadModulesOf`, `installPrefetch` : garde `beforeResolve` sur `prefetch.guard`,
    posée dans `main.ts`, sans attente pour une route sans lectures), `KLink.vue`
    (`useLink`, `aria-current="page"`, `prefetch.intent` sur son `<a>`), `nextPage.ts`
    (`useNextPage(liste, fenêtre)` : page suivante lue à moins de deux écrans de la fin
    des lignes chargées, `end` étant un index parmi les lignes chargées ; une à la fois,
    rien pendant une lecture, après la dernière page ni après un échec de la page suivante
    tant que la liste n'est pas relue), `tree.ts` (`childrenIntent(racine, enfants)` : un
    écouteur délégué sur la racine d'un `TreeGrid`, enfants d'une ligne fermée lus après
    60 ms de pointeur ou de focus) ; `web/v2/shell/src/plugins/preload.ts`
    (`preloadModules` : un `modulepreload` avec empreinte par module `.js` de
    `integrity`, une fois par URL, oublié s'il échoue). Vitest
    (`shell/test/prefetch.test.ts`, 19 tests) : survol de 60 ms et passage sans lecture,
    focus immédiat, dates de la cible et du contexte, clic et page courante, modules d'un
    plugin une fois chacun, module en échec inséré de nouveau ; garde qui attend la
    lecture, plafond de 100 ms, route sans lectures, 403, retour au présent, lectures non
    déclarables (avertissement) ; page suivante (seuil, une à la fois, lien `next` d'une
    liste du cache, arrêt après 403 et après 503 et ses deux reprises) ; enfants d'un nœud
    fermé sur un vrai `TreeGrid`, au pointeur et au focus (chacun son délai), rien pour un
    nœud ouvert ni une ligne passée. Choix notés : `meta.queries` reçoit aussi la
    requête de la cible (filtres d'une liste) ; `meta.modules` porte les modules d'un
    plugin (la route `/x/:plugin/…` le déclarera) et `KLink` les insère au même moment que
    ses lectures ; le focus d'un nœud de l'arbre attend aussi 60 ms (les flèches passent
    sur beaucoup de lignes), celui d'un lien lit tout de suite ; `meta.breadcrumb` laissé
    au fil d'Ariane de la tâche 9. Fait par la tâche 9 : `provideLink(KLink)` à la racine du
    cadre, `meta.breadcrumb`, garde du temps en `beforeEach`. Reste, faute d'écran :
    `meta.queries` des routes de la fiche, des listes et de la Topographie, `useNextPage`
    branché sur la fenêtre de la `DataGrid` virtualisée et `childrenIntent` sur la
    Topographie (tâche 11) ; `meta.modules` de `/x/:plugin/…`, chargeur qui attend les
    mêmes `preloadModules` et entrées des plugins préchargées au repos
    (`requestIdleCallback`, tâche 16) ; parcours « fiche survolée puis ouverte sans
    squelette » après la tâche 11 ; marque `k:view` ≤ 100 ms au banc de la tâche 22.
  - fait le 2026-11-11 (relevé contre le code, main 1b6f0972) : le reste est livré par les
    tâches qui l'ont repris, sans code de plus. `meta.queries` de chaque écran et de chaque
    fiche (`views/routes.ts` : listes par `listOf` et leurs filtres, fiche `object` et
    `properties`, Topographie `/v1/domains`, Accueil `stats` ; `admin/routes.ts`,
    `changesets/routes.ts`), `useNextPage` sur la fenêtre de la `DataGrid`
    (`views/list.ts`, donc listes, anomalies, journal, imports, changesets ; onglets
    `HistoryTab`, `GroupVlansTab`, `VlanNetworksTab`), `childrenIntent` sur la Topographie
    (`TopologyView.vue`) : tâche 11 ; `meta.modules` de `/x/:plugin/…`
    (`frame/routes.ts`), chargeur sur les mêmes `preloadModules` et entrées préchargées
    au repos (`plugins/loader.ts`, `requestIdleCallback`) : tâche 16 ; marque `k:view`
    (`prefetch/viewMark.ts`) et sa mesure (`e2e/budgets/budgets.spec.ts`, navigation en
    cache) : tâche 22a. Verify : parcours `records` « a record hovered then opened shows
    without a skeleton » ; Vitest `shell/test/prefetch.test.ts`, `sdk/test/
    prefetch.test.ts`, `shell/test/plugins.test.ts`.
  - reste (banc) : marque `k:view` ≤ 100 ms (acceptation de REQ-021-22) : 131 ms au p75
    à la passe pleine échelle de la 22b, avec des fiches non préchargées au survol ;
    portée par la 22g. Les routes des vues de plugins ne déclarent pas de lectures (la vue
    les fait dans son élément) : leur navigation ne gagne que les modules.
- [x] 8. Partage et manifeste : import map étendue (framework, `@kartenn/ui`, feuille liée
  avec empreinte) ; manifeste `ui`, `framework`, `space`, `go` validés côté serveur et
  publiés dans `ui_plugins` (ajout additif) ; plages vérifiées au chargement ; adaptateur
  du cache dans `@kartenn/ui` ; API `commands`, `shortcuts`, `label` — REQ-021-25, 26 —
  verify : plugin `ui: ^3` sur `ui` 1.x → `incompatible` ; CSP inchangée hors empreinte de
  l'import map ; `k-check` (manifeste) ; `vue` servi en build *runtime* (aucun
  `new Function` sous la CSP)
  - où : `web/shell/vite.config.ts` (`sharedSdk`) ; `crates/kartenn-rest/src/web/plugins.rs`
    (`Manifest`, `sdk_range`), `capabilities.rs` ; `crates/kartenn-rest/src/web.rs`
    (`import_map`, `csp`) ; `web/sdk/src/semver.ts` ; `web/shell/src/plugins/loader.ts`
  - dépend de : 2 ; 020-4
  - en cours : adaptateur Vue écrit le 2026-10-24, provisoirement dans
    `web/v2/shell/src/data/query.ts` (`@kartenn/ui` étant en chantier par la 020) :
    `installData(app)` (`VueQueryPlugin` sur le `QueryClient` du SDK), `useLabel`,
    `useLocale`, `useEventsState` ; `@tanstack/vue-query` épinglé en 5.102.4 comme
    `query-core`, script d'installation de `vue-demi` refusé (`allowBuilds`, entrée Vue 3
    par défaut). Constat : passé tel quel, le client du cœur n'est jamais observé par
    vue-query (ses composables ne s'abonnent que si le client porte la ref
    `isRestoring`) ; l'adaptateur la pose, test `shell/test/data.test.ts` en garde.
    Remplacé le 2026-10-24 (ADR 0037 amendée) par un adaptateur propre sur les
    observateurs de `@tanstack/query-core` : `installData` (fourniture du client),
    `useQueryClient`, `useQuery`, `useInfiniteQuery`, `useMutation` (désabonnement à la
    fin de la portée) ; `@tanstack/vue-query`, `vue-demi`, `match-sorter-utils` et
    `remove-accents` retirés ; tests d'abonnement et de fin de portée dans
    `shell/test/data.test.ts`.
    Pas encore branché dans `main.ts` (le cadre, tâche 9, appellera `installData` et
    `startData`).
  - fait le 2026-10-25 : serveur — `Manifest` accepte `ui`, `framework` (mêmes formes que
    `sdk`, `version_range` remplace `sdk_range`), `space` (liste fermée `UiSpace` :
    `referential` … `admin`) et `go` sur une extension (vue seulement, `[a-z]`, unique dans
    le plugin) ; publiés dans `ui_plugins` s'ils sont déclarés (absents sinon), contrat et
    `schema.d.ts` (web/sdk, web/v2/sdk) régénérés, 007 (design, règles de validation) et
    `docs/spec/interfaces.md` amendés ; tests : manifestes fautifs, publication,
    `capabilities`, CSP identique hors empreinte d'une import map étendue avec feuille liée
    (`tests/it/web.rs`). SDK — `commands.register`, `shortcuts.register` (une touche,
    `scope: "view"`, refus d'une touche du shell réservée par `reserveKeys` ou déjà prise ;
    chacun rend son retrait), registres lus par l'hôte (`commandRegistry`,
    `shortcutRegistry`), `unmetRange(plages, versions)` dans l'hôte (`ui: ^3` sur `ui`
    1.4.0 → `ui`, donc `incompatible`) ; `label` existait déjà. Build de `web/v2/shell`
    (`sharedModules`) — `vue` (build *runtime*), `@kartenn/sdk` et `@kartenn/ui`
    (`src/shared/ui.ts`, qui porte la feuille) en entrées à signature stricte : le shell et
    les plugins chargent les mêmes instances ; l'import map écrite après l'écriture des
    fichiers nomme les trois et donne l'empreinte de **tous** les modules du build
    (`integrity`) ; la feuille `@kartenn/ui` émise à part, liée avec `integrity` ;
    l'aperçu envoie la CSP du serveur avec l'empreinte de la carte (parcours `csp` du
    catalogue vert) ; `shell/test/build.test.ts` vérifie carte, empreintes, feuille et
    l'absence de `new Function`/`eval`. Choix notés : pas d'import map sous le serveur de
    développement (aucun plugin n'y charge encore) ; Command et Shortcut typés ici
    (`label` en fonction, lue à l'affichage, `permission` facultative).
  - reste : déplacement de l'adaptateur (`web/v2/shell/src/data/query.ts`) dans
    `@kartenn/ui` quand la 020 libère `web/v2/ui` ; appel de `unmetRange` par le chargeur
    du nouveau shell (versions : `SDK_VERSION`, celle de `@kartenn/ui` — 0.1.0 à ce jour,
    à passer en 1.x —, `version` de `vue`) et `handle.styles()` pour la feuille de
    `@kartenn/ui` en racine d'ombre (REQ-021-27), avec la tâche 16.

## P1 bis — Côté serveur (avant les écrans)

Contrat complété (X41, REQ-021-35) ; chaque tâche amende la spec du module concerné et
régénère le contrat de référence et les types du SDK. `k-check` pour chacune.

- [x] 28. Paramètres de filtre déclarés : `prop.<clé>`, `prop.<clé>:between`,
  `prop.<clé>:exists`, `within`, `usage`, `nature`, `value`, `first`, `parent`, `slug` et
  tout autre filtre servi, décrits dans les `parameters` de l'OpenAPI des routes de liste
  (cœur et modules) ; test de contrat : tout paramètre accepté est déclaré — amendement de
  la 006 (« Filtres de propriétés ») et de la 016 — REQ-021-35 — verify : test de contrat
  vert ; `filters.ts` généré les contient ; un paramètre servi non déclaré fait échouer
  le test
  - où : `crates/kartenn-rest/src/filter.rs`, `openapi.rs`, `contract.rs` ;
    `specs/006-api-rest/openapi-v1.json` ; `web/sdk/scripts/gen.mjs`
  - dépend de : —
  - fait le 2026-10-23 : `within`, `nature`, `usage` n'étaient pas servis (REQ-006-9 les
    promettait) : servis en égalité (`list::objects`, `$12` CIDR, `$13` valeurs) puis
    déclarés ; filtres de propriétés en trois gabarits `prop.{key}`, `prop.{key}:between`,
    `prop.{key}:exists` marqués `x-kartenn.template`, sur les seules listes qui filtrent
    par propriété (006/H20, forme du contrat choisie par Claude) ; `x-kartenn.collection`
    sur `domain` et `parent` ; une liste sans propriétés refuse `prop.…` comme paramètre
    inconnu ; test de contrat `declared.rs` (sonde de chaque liste par tous les noms de
    filtre du contrat) ; `web/sdk/src/api/filters.ts` généré ; client Python : filtres
    sans gabarit, `--prop` seulement sur une liste qui le déclare, `--within` de la
    maquette rejoué dans `k-py-e2e`. Rien d'amendé dans la 016 (aucun filtre de liste
    n'en vient).
- [x] 29. Curseurs sur `GET /v1/anomalies`, `/v1/property-gaps` (006, 016) et
  `/v1/flow-nonconformities` (011) — REQ-021-35 — verify : parcours par curseur jusqu'à
  la dernière page sur un jeu de plus d'une page ; curseur signé refusé s'il est altéré
  - où : `crates/kartenn-rest/src/cursor.rs` (forme, REQ-006-8), `anomalies.rs`,
    `properties.rs` ; `crates/kartenn-flow/src/rest.rs`
  - dépend de : —
  - fait le 2026-10-23 : anomalies et écarts par `(opened_at, id)` croissants, lus à
    l'état courant à chaque page (`next_current`, sans `valid_at` ni `as_of`, refusés à
    la première page) ; `anomalies::open_page` n'écrit la condition de clé qu'après une
    première page (pas de `$n IS NULL OR …` qui rendrait le plan générique non indexable)
    ; le filtre `property` des écarts passe en SQL (préfixe `<clé>:` du message) pour
    couper la page après lui ; non-conformités par `(at, flow ou changeset)` décroissants,
    `as_of` figé, la clé de la dernière ligne lue avant le filtre des droits (page parfois
    plus courte). Pas d'index nouveau sur `anomaly` (pas de migration, tri des anomalies
    ouvertes à chaque page) : à reprendre si un banc le demande. Bancs réduits du
    2026-10-24 tenus : `bench-flow` page de non-conformités p95 4,5 ms, `bench-core`
    page filtrée p95 22,8 ms.
- [x] 30. `GET /v1/policy-reports` : liste paginée des rapports d'audit de la politique
  (état, date, lanceur), sous `policy.rule.admin` — amendement de la 010 —
  REQ-021-35 — verify : test REST (liste, droits, curseur)
  - où : `crates/kartenn-policy/src/rest.rs` (rapport existant, 409 si en cours)
  - dépend de : —
  - fait le 2026-10-23 : `report::reports` (date de demande puis identifiant
    décroissants, index `policy_report_by_date`), lue à l'état courant à chaque page ;
    filtres `state` (répété : un parmi), `requested_by`, `from`, `to` (date de demande) ;
    `policy.rule.admin` global sinon 404, comme un rapport ; élément de la forme de
    `GET /v1/policy-reports/{id}` ; `x-kartenn` (colonnes) sur la collection. CLI :
    commande nommée `kartenn policy reports` (un `list` dérivé de l'opération : filtres
    du contrat, pages, sorties ; 023 design « Commandes nommées »).
- [x] 31. Aperçu d'une règle d'inférence (010, amendement S36, accepté) : route décrite par
  l'amendement, sans écriture ; débloque la tâche 017-15 — REQ-021-35 — verify : tests
  de l'amendement S36
  - où : `specs/010-politique-flux/` (amendement S36) ; `crates/kartenn-policy/src/`
    (moteur d'inférence)
  - dépend de : —
  - fait le 2026-10-24 : amendement S36 repris (cherry-pick de 7d7255e, conflit résolu
    sur le résumé et les questions ouvertes de REQ-010), marqué accepté, puis implémenté
    (tâche 010-22, cochée avec ses notes). S36 n'ajoute pas de route : l'aperçu se lit
    par `POST /v1/changesets?simulate=true` et `GET /v1/changesets/{id}/diff?section=
    policy.combinations` (lignes `policy.inference.summary` et `policy.inference.node`) ;
    la tâche 017-15 est débloquée.
- Dépendance serveur hors de la 021 : `GET /v1/labels` et l'événement `labels` (spec 018,
  validée) avant la tâche 13.

### Sous-tâches serveur des arbitrages du 2026-11-01 (A11 à A14)

- [x] 11a. `TreeNode` porte `version` et, pour un préfixe, `occupancy` (A11) dans
  `GET /v1/domains/{id}/roots` et `GET /v1/prefixes/{id}/children`, lus en lot au même
  instant que la page (aucune requête par ligne) ; contrat OpenAPI régénéré — verify :
  `k-check -p kartenn-rest` (test d'intégration : `version` égale à l'ETag de l'objet,
  `occupancy` égale à `…/occupancy` au même `valid_at`) ; `bench-affected` réduit si un
  banc lit l'arbre
  - où : `crates/kartenn-rest/src/tree.rs` (`NodeDto`, `roots`, `children`, `occupancy`) ;
    la lecture d'occupation du store qu'appelle `occupancy` ; test voisin
    `crates/kartenn-rest/tests/it/objects.rs` (arbre) ; 006 design § tableau « Arbre »
  - dépend de : —
  - fait le 2026-11-01 — REST (`crates/kartenn-rest/src/tree.rs`) : `NodeDto` (schéma
    `TreeNode`, de `roots` et `children`) gagne `version` (requis, les trois genres) et
    `occupancy` (requis pour un préfixe, schéma `Occupancy` de `…/occupancy`, construit
    par le même `Occupancy::of`) ;
    la page, ses versions (`read::versions`, celle des pages d'objets, 006/H19) et
    l'occupation de ses préfixes (`tree::occupancies_as_of`) se lisent au `Moment` de la
    page dans un seul instantané, une transaction `REPEATABLE READ, READ ONLY`
    (`read::consistent`) : une `version` nomme les valeurs servies. `…/ancestors` garde
    la forme d'avant sous le schéma `AncestorNode`, sans `version` ni `occupancy` (ni
    calcul d'occupation). Store (`crates/kartenn-store-pg/src/tree.rs`) :
    `occupancies_as_of(executor, domain, ids, valid_at, as_of)` en une requête par page,
    quel que soit le nombre de préfixes (chaque préfixe avec ses drapeaux, puis le contenu
    que compte son occupation : marche des enfants directs d'un conteneur, une sonde
    d'index chacun comme `tops`, plages et adresses d'un réseau, présent ou passé par
    `probe`), mesures par les fonctions de `kartenn_core::occupancy` ; `recent` extrait de
    `tops` et partagé ; `children_of`, `roots_of`, `read::versions` prennent un
    exécuteur. Contrat `specs/006-api-rest/openapi-v1.json` et
    `web/v2/sdk/src/api/schema.d.ts` régénérés. Test :
    `objects::tree_nodes_carry_their_version_and_occupancy` (racines, enfants de deux
    préfixes ; présent, `valid_at` passé, `as_of` : `version` = membre `version` de l'objet
    et tête de son ETag, `occupancy` = `…/occupancy` ; ancêtres sans l'un ni l'autre ;
    mesures d'un réseau avec plage DHCP, d'un conteneur, d'un /32 IPv6 ; adresses ajoutées
    après `as_of` comptées au présent seulement, adresse supprimée après `as_of` encore
    comptée à `as_of`) ; `k-check` vert (1214 tests, puis `-p kartenn-rest -p
    kartenn-store-pg` après la passe de corrections). Shell : seul le typage de
    `web/v2/shell/src/views/TopologyView.vue` suit (`Shown`, un nœud sans `version` ni
    `occupancy` pour le préfixe lu seul par l'URL), sans changement de rendu ;
    `k-web2-check` vert (711 tests, 2706 captures, aucune réécrite). Banc (session
    principale, `bench-http` réduit, sur 37498177) : « subtree and occupancy » 43,7 ms au
    p95, 84,8 ms dans le passé (budget < 200 ms) ; surcoût HTTP +26,6 ms (cible 10 ms,
    manquée).
  - choix : `occupancy` reprend tout l'objet de `…/occupancy` (`prefix`, `nature`,
    `used`, `total`), à la lettre de A11 ; l'occupation d'un réseau compte ses plages et
    adresses sans chercher de sous-préfixe (`core.prefix.network_not_leaf`) ; un nœud sans
    version ni occupation dans l'instantané (impossible : même instantané, même moment)
    serait omis de la page, dit au niveau `debug`.
  - reste : avant la 12a, `kit::versioned` (pages d'objets) lit les versions hors de
    l'instantané de la page — même trou que celui corrigé ici (une valeur ancienne sous
    une `version` nouvelle, un `If-Match` qui passerait sans 412). Agrégation de
    l'occupation en SQL plutôt qu'en Rust (une page de réseaux lit toutes leurs adresses,
    ~25 000 lignes pour 100 réseaux pleins) non faite : elle quitterait les fonctions de
    `kartenn_core::occupancy` ; à reprendre si le surcoût HTTP (+26,6 ms, cible 10 ms)
    doit tomber.
- [x] 16a. `SessionState.absolute_expires_at` et en-tête `Kartenn-Background: ?1` traité
  en `NotActivity` (A14) : ni `seen` prolongé ni cookie réémis ; valeur autre que `?1` ou
  `?0` ignorée (requête ordinaire) — verify : `k-check -p kartenn-rest` (sessions : une
  requête de fond ne repousse pas `expires_at`, une requête ordinaire le repousse ;
  `absolute_expires_at` constant sur toute la session)
  - où : `crates/kartenn-rest/src/auth.rs` (`SessionState`, `Me`),
    `crates/kartenn-rest/src/authn.rs` (`NotActivity`, comme
    `crates/kartenn-rest/src/events/mod.rs`) ; test voisin
    `crates/kartenn-rest/tests/it/sessions.rs` ; 005 design § « Cookie de session »,
    006 design § « Cookies »
  - dépend de : —
  - fait le 2026-11-01 : `Session::absolute_end` (`kartenn-auth`), `SessionEnds { at,
    absolute }`, `Authenticated.absolute_end` et `authn::background` ; la couche 6 retire
    le cookie réémis et donne `quiet_end` à `GET /v1/me` pour une requête de fond ;
    contrat et `schema.d.ts` (`web/sdk`, `web/v2/sdk`) régénérés ; tests
    `only_the_boolean_true_marks_a_request_in_the_background` (unitaire),
    `a_request_in_the_background_is_no_activity` et la fin absolue d'une heure du secours
    (`sessions.rs`) ; `docs/spec/interfaces.md` (Authentification).
  - choix : analyse stricte du champ structuré (RFC 9651 §4.2) : une seule ligne, valeur
    `?1` à espaces près, sans paramètre ; `?0`, plusieurs lignes (même `?1, ?1`), des
    paramètres (`?1;a=b`) ou une valeur qui ne s'analyse pas → requête ordinaire, sans
    400 (l'en-tête ne peut que priver une requête de son effet d'activité). Les
    paramètres, que RFC 9651 permettrait d'ignorer, sont refusés : l'en-tête n'en définit
    aucun. Une requête de fond refusée (session close) retire le cookie comme une autre.
  - reste : une requête de fond qui rafraîchit les groupes (jeton d'accès de l'émetteur
    expiré) ne réémet pas le cookie, comme le flux d'événements déjà : si l'émetteur fait
    tourner ses jetons de rafraîchissement (RFC 9700 §4.14.2), le jeton gardé par le
    cookie est périmé et le rafraîchissement suivant échoue (fermeture
    `refresh_failed`). Recommandation : réémettre le cookie après un rafraîchissement,
    `seen` laissé à sa valeur lue (ne prolonge rien) ; à trancher (A14 dit « ne réémet
    pas »).
- [x] 11c. Pages d'objets lues dans un seul instantané : `kit::versioned` lit la page et
  ses versions dans la même transaction `REPEATABLE READ, READ ONLY` (`read::consistent`
  de la 11a), comme l'arbre ; sans cela, une écriture validée entre les deux lectures sert
  une valeur ancienne sous une `version` nouvelle, et un `If-Match` de l'édition en place
  (12, 12a) écraserait sans 412 un changement non vu (relevé par la revue de la 11a) —
  verify : `k-check -p kartenn-rest` (test : chaque objet d'une page porte la `version`
  de la valeur servie) ; `bench-affected` réduit sur `bench-http` (pages filtrées) par la
  session principale
  - où : `crates/kartenn-rest/src/kit.rs` (`versioned`, vers la ligne 170) et ses
    appelants ; `read::consistent` et `children` de `crates/kartenn-rest/src/tree.rs`
    (forme) ; test voisin `crates/kartenn-rest/tests/it/objects.rs`
  - dépend de : 11a
  - fait le 2026-11-01 — `kit` (`crates/kartenn-rest/src/kit.rs`) : `snapshot` (la
    transaction de `read::consistent`, type `Snapshot`), `versions(executor, moment, ids)`
    qui lit les versions dans l'instantané, `versioned(items, &versions)` qui les pose
    (sans lecture), `release` qui finit l'instantané avant l'événement d'audit. Chaque
    page d'objets lit ses lignes et leurs versions dans un instantané : objets du cœur
    par clé naturelle et par propriétés (`objects::list`, l'instantané ouvert après la
    sélection des identifiants par le moteur de filtres, aucun s'il n'y en a pas),
    groupes de VLAN et VLAN (`vlan.rs`), grappes, machines et interfaces
    (`kartenn-machines`), collections de politique (`kartenn-policy`), flux et flux d'un
    objet (`kartenn-flow`, `fetch` ouvre l'instantané une fois ses filtres valides et le
    rend à `page_of` ; versions
    des flux en vigueur lues avant les lectures de visibilité et de valeurs des
    extrémités, une seule connexion tenue à la fois) ; l'arbre (`tree.rs`) passe aux
    mêmes `snapshot`/`release`. `list::objects` prend un exécuteur. Coût : un aller-retour
    de plus par page (le `BEGIN`) ; la fin de l'instantané n'en coûte aucun à la réponse.
    Tests, par les routes, une écriture validée entre la lecture des lignes et celle des
    versions (`common::served_across_a_write`) : `resources::a_page_carries_the_version_of_the_value_served`
    (`GET /v1/prefixes`, sans filtre et par domaine : `version` de la valeur lue,
    l'écriture vue ensuite, `If-Match` de la version servie → 412) et
    `vlan::rest::a_page_of_vlans_carries_the_version_of_the_value_served` (groupes de
    VLAN, VLAN) ; tous deux échouent quand les versions se relisent sur le pool ;
    `objects::properties_filter_the_objects_of_one_domain` (page filtrée : `version` de
    chaque objet = celle de sa lecture et tête de son ETag) ; `k-check -p kartenn-rest -p
    kartenn-flow` vert (410 tests) ; `k-check` complet vert hors deux tests du flux
    d'événements (`settled`, attente du lecteur de l'outbox dépassée sous charge), verts
    seuls. Banc
    `bench-http` (pages filtrées) laissé à la session principale.
  - choix : l'instantané se finit en le lâchant (`ROLLBACK` mis en file ; la connexion
    revient au pool dans une tâche à part, dont le ping l'envoie pendant que la requête
    continue, audit et réponse) plutôt que par un `COMMIT` attendu, qui coûtait un second
    aller-retour ; pour une transaction en lecture seule les deux se valent (seul
    `pg_stat_database.xact_rollback` les distingue) ; l'arbre de la 11a suit, pour une
    forme unique. `versioned` séparé en `versions` (lecture) et `versioned` (pose), pour
    que les flux lisent leurs versions avec leurs lignes. L'écriture des tests passe par
    un verrou `ACCESS EXCLUSIVE` sur `object_version`, que seules les lectures de
    versions lisent, tenu par le test qui y écrit puis valide quand la requête l'attend :
    rien dans le code servi, aucun coût en production.
  - reste : (1) page filtrée par propriétés : les identifiants sont choisis par le moteur
    de filtres hors de l'instantané, seules les lignes et leurs versions y sont lues (un
    objet qui ne vérifie plus le filtre peut encore paraître) ; limite admise. (2) Lecture
    d'un objet par identifiant : la valeur et la version (`kit::version`, l'ETag) se
    lisent en deux lectures sur le pool, dans `vlan.rs` (`answer`), `kartenn-machines`
    (`answer`), `kartenn-policy` (`show`, et la relecture avant écriture),
    `kartenn-flow` (`show`) et le `GET` d'un objet du noyau (`objects::object`, valeur
    par `read_object` du service sur un `PgStore` du pool, puis `read::version`) : une
    écriture entre les deux sert une valeur ancienne sous une version nouvelle, et le
    panneau d'édition (12a) en tire son `If-Match`. À traiter dans une tâche à part (le
    noyau demande que `read_object` lise dans l'instantané) ; en attendant, lire la
    version avant la valeur suffirait à rendre la course sûre (412 de trop, jamais
    d'écrasement).
- [x] 16c. Rafraîchissement pendant une requête de fond (A15) : après un rafraîchissement
  des groupes sur une requête `Kartenn-Background: ?1` ou à l'ouverture du flux
  d'événements, cookie réémis avec `seen` remis à la valeur lue (ni fin par inactivité
  ni fin absolue repoussée) ; sans rafraîchissement, aucun cookie (A14) ; l'en-tête
  `Kartenn-Background` déclaré au contrat OpenAPI (paramètre d'en-tête des routes
  authentifiées, ou composant commun) — verify : `k-check -p kartenn-rest` (sessions :
  requête de fond avec rafraîchissement dû → cookie réémis, nouveau jeton de
  rafraîchissement, `expires_at` inchangé ; même cas à l'ouverture du flux ; sans
  rafraîchissement, pas de cookie)
  - où : `crates/kartenn-rest/src/authn.rs` (couche 6, rafraîchissement vers la ligne
    487, filtre du cookie vers la ligne 943), `crates/kartenn-rest/src/events/mod.rs`
    (`NotActivity`) ; test voisin `crates/kartenn-rest/tests/it/sessions.rs`
    (`a_request_in_the_background_is_no_activity`) ; 005 design § « Cookie de session »
  - dépend de : 16a
  - fait le 2026-11-01 : `Authenticated.quiet_cookie` (`authn.rs`), scellé seulement
    après un rafraîchissement, avec `seen` remis à la valeur lue ; la couche 6 l'envoie
    pour une requête de fond ou une réponse `NotActivity` (flux et rattrapage), le cookie
    ordinaire sinon (`reissue`) ; contrat : paramètre d'en-tête commun
    `components.parameters.Kartenn-Background`, référencé par toute opération non
    publique ; contrat et `schema.d.ts` (`web/sdk`, `web/v2/sdk`) régénérés ; tests
    `a_refresh_in_the_background_keeps_the_new_tokens` et
    `a_refresh_at_the_opening_of_the_stream_keeps_the_new_tokens` (`sessions.rs`, issuer
    de test qui tourne son jeton de rafraîchissement, `Fixture::streamed` sur un hub sans
    lecteur) ; `docs/spec/interfaces.md` (Authentification).
  - choix : composant commun plutôt que paramètre recopié sur 298 opérations ; les
    opérations `QUERY` de l'extension `x-query` ne le déclarent pas (construites à part,
    comme `approval` et `simulate`) ; la réponse paginée du rattrapage, `NotActivity`
    elle aussi, suit la même règle que l'ouverture du flux.
  - reste : `contract::breaches` compare les paramètres par nom et emplacement sans suivre
    les `$ref` : un changement du composant `Kartenn-Background` (le rendre requis) ne
    serait pas vu comme une rupture.
- [x] 11d. Lecture d'un objet par identifiant en un seul instantané : la valeur et sa
  version se lisent dans la même transaction `REPEATABLE READ, READ ONLY` (`kit::snapshot`
  de la 11c), pour le `GET` d'un objet du noyau (`objects::object` : `read_object` du
  service sur un `PgStore` du pool, puis `read::version`) et des modules (`vlan.rs`
  `answer`, `kartenn-machines` `answer`, `kartenn-policy` `show` et sa relecture avant
  écriture, `kartenn-flow` `show`) ; sans cela, le panneau d'édition (12a) prend un
  `If-Match` d'une version plus récente que la valeur montrée (relevé par la 11c) —
  verify : `k-check -p kartenn-rest` et les crates des modules (test par une route, à
  la forme de `served_across_a_write` de la 11c : la version suit la valeur servie, et
  le test échoue sur l'ancien code)
  - où : `crates/kartenn-rest/src/objects.rs` (`object`), `kit.rs` (`version`,
    `snapshot`, `release`), `read_object` de `crates/kartenn-service/` (lecture sur un
    exécuteur), `crates/kartenn-rest/src/vlan.rs`, `crates/kartenn-machines/src/rest.rs`,
    `crates/kartenn-policy/src/rest.rs`, `crates/kartenn-flow/src/rest.rs` ; aide de test
    `crates/kartenn-rest/tests/it/common/mod.rs` (`served_across_a_write`)
  - dépend de : 11c
  - fait le 2026-11-02 — `read_object` du service se coupe en deux : la lecture sur un
    `Reader`, puis `reads::shown_object` (droits, masques, événements d'audit) sur une
    valeur lue ailleurs ; `kartenn_store_pg::read::object_in` lit l'objet sur une
    connexion (`object` la prend du pool), `read::version` et `kit::version` sur un
    exécuteur ; `kit::read_one` lit la ligne d'un objet et sa version dans un
    instantané, le libère et rend 404 si l'un manque. `objects::object` lit valeur et
    version dans l'instantané, le libère, puis droits et audit (`shown_object`) ; VLAN et
    groupes (`vlan.rs`), grappes, machines et interfaces (`kartenn-machines`, la fiche
    libère l'instantané avant ses interfaces et adresses), objets des collections de
    politique et relecture de `DELETE /v1/environments/{id}` (`kartenn-policy`
    `row_at`) passent par `read_one` ; `kartenn-flow` `show` lit le flux et sa version
    dans un instantané (`flow_at` sur une connexion), libéré avant les extrémités. Une
    connexion tenue à la fois. Tests, par les routes, une écriture validée entre la
    lecture de la valeur et celle de la version (`kartenn_testing::across_a_write`, que
    `common::served_across_a_write` reprend) : `resources::a_read_by_identifier_carries_the_version_of_the_value_served`
    (`GET /v1/prefixes/{id}` : `version` et `ETag` de la valeur, écriture vue ensuite,
    `If-Match` servi → 412), `vlan::rest::a_page_of_vlans_carries_the_version_of_the_value_served`
    (étendu aux lectures d'un groupe et d'un VLAN) et
    `a_read_by_identifier_carries_the_version_of_the_value_served` de `kartenn-machines`
    (machine, interface), `kartenn-policy` (environnement) et `kartenn-flow` (flux) ;
    les cinq échouent sur l'ancien code (version écrite servie avec l'ancienne valeur).
  - choix : coupe de `read_object` plutôt qu'un `Reader` sur une transaction (le port
    a d'autres méthodes et prend `&self`) ; `across_a_write` dans `kartenn-testing`
    (identifiants en texte, sans nouvelle fonctionnalité de sqlx) pour servir les crates
    des modules ; `list::link` (libellé de l'objet lié) reste lu sur le pool, hors de la
    version.
  - reste : la relecture de `DELETE /v1/environments/{id}` compare l'`If-Match` à la
    version lue puis soumet la fermeture par clé sans version attendue : une écriture
    validée entre cette lecture et le changeset n'est pas vue (fenêtre antérieure à la
    11d, que l'instantané ne ferme pas) ; la fermer demande une version attendue dans
    l'opération `policy.environment.close`.
- [x] 11e. Fermeture d'un environnement sous `If-Match` sans écriture écrasée :
  `DELETE /v1/environments/{id}` compare l'`If-Match` à la version lue, puis soumet
  `policy.environment.close` par clé sans version attendue ; une écriture validée entre
  les deux passe sans 412 (REQ-004-11, REQ-006-14 ; relevé par la revue de la 11d). La
  version attendue est portée par l'opération (`expected`, comme les opérations du
  noyau) et vérifiée dans le changeset ; même examen pour les autres routes de modules
  qui vérifient un `If-Match` avant de soumettre — verify : `k-check -p kartenn-policy`
  (test à la forme de `kartenn_testing::across_a_write` : écriture validée entre la
  comparaison et le changeset → 412, et l'environnement reste ouvert)
  - où : `crates/kartenn-policy/src/rest.rs` (`close_environment`, vers les lignes
    587-606), l'opération `policy.environment.close` du module ; `expected` des
    opérations du noyau (004 design § « Concurrence optimiste ») ;
    `crates/kartenn-testing/src/lib.rs` (`across_a_write`)
  - dépend de : 11d
  - fait : `policy.environment.close` déclare `expected` (`CloseEnvironment`, version
    facultative, à la forme du noyau) ; le plan le porte dans ses données et l'écriture
    du module (`Handler::check_expected`, `pg::version`) relit la version de
    l'environnement sous verrou de sa ligne `object_version` (`FOR UPDATE`, comme
    l'`expect` du noyau) : une autre version refuse le changeset par
    `changeset.operation.conflict`. `close_environment` garde sa comparaison, passe la
    version dans `expected` et soumet par `kit::submit_matched` (ce conflit → 412).
    Routes de modules examinées : `kit::modify`/`modify_nullable` et `kit::act`
    (machines, flux, politique) soumettent un `expect` dans le même changeset,
    `kit::close` porte `expected` dans la clôture, `PUT` d'une cellule de matrice
    (`submit_matched`) revérifie la révision sous la clé `policy` (`check_set`) : aucune
    autre ne compare puis soumet sans garde. Tests :
    `rest::a_closure_under_if_match_never_passes_over_a_write` (`across_a_write` : 412,
    l'environnement reste ouvert, puis fermé sous la version écrite ; 200 sans
    vérification sur l'ancien code) et `axes::acceptance_of_req_010_2` (`expected`
    périmé → conflit, juste → fermé).
  - choix : vérification dans l'écriture du module plutôt qu'un `expect` du noyau ajouté
    au plan, que le pipeline planifie après l'opération du module (ADR 0026, clause 6),
    donc après que l'objet du plan a été marqué écrit par ce changeset ; la version lue
    ne compte pas les écritures du même changeset encore en attente de `touch` ;
    `object_version` lu par le module (`public.`), comme ses autres lectures du noyau.
- [x] 16d. Jeton tourné jamais perdu (A15, relevé par la revue de la 16c) : les retours
  anticipés de la couche 6 (refus `on_behalf`, `approval` ou `simulate` invalides)
  partent après un rafraîchissement réussi sans aucun cookie : ils réémettent le cookie
  qui convient (ordinaire, ou de fond `seen` inchangé) ; deux requêtes concurrentes dont
  le rafraîchissement est dû ne présentent pas deux fois le même jeton à l'émetteur
  (rafraîchissement unique par session en cours, RFC 9700 § 4.14.2) ; tests : l'émetteur
  de test refuse `r1` après la rotation, et une fin absolue comparée au `started` du
  cookie d'entrée — verify : `k-check -p kartenn-rest` (sessions : refus anticipé après
  rafraîchissement → cookie réémis ; deux requêtes concurrentes → un seul appel à
  l'émetteur ; cookie resté sur `r1` → `refresh_failed`)
  - où : `crates/kartenn-rest/src/authn.rs` (couche 6, retours anticipés vers les lignes
    966-989, `reissue`, `quiet_cookie`) ; test voisin
    `crates/kartenn-rest/tests/it/sessions.rs` (`a_refresh_in_the_background_keeps_the_new_tokens`)
  - dépend de : 16c
  - fait le 2026-11-02 : couche 6, `context_of` (`authn.rs`) rend la réponse de refus
    (`on_behalf`, `approval`, `simulate`) et la couche y réémet `quiet_cookie` pour une
    requête marquée, `cookie` sinon ; `Oidc::refresh` (`kartenn-auth/src/oidc.rs`)
    partage l'échange en cours par jeton présenté (`Turn`, carte
    émetteur × sujet × jeton → `OnceCell` du résultat, oubliée dès l'échange fini ou
    sans plus aucune requête) ; `LoginError` et `TokenError` deviennent `Clone` ; tests
    `a_refusal_after_a_refresh_keeps_the_new_tokens`,
    `concurrent_refreshes_present_the_token_once` et fin absolue comparée au `started` du
    cookie d'entrée (`sessions.rs`, émetteur de test qui ne prend que le dernier jeton
    donné, compte les échanges et peut les retenir) ; 005 design § « Rafraîchissement
    des groupes ».
  - choix : état par instance, en mémoire (le plus simple qui tienne une instance) ;
    clé émetteur × sujet × jeton plutôt qu'un identifiant de session, que le cookie
    n'a pas ; un refus anticipé d'une requête ordinaire réémet le cookie ordinaire
    (`seen` à l'instant), comme la tâche le dit ; un échec de l'échange (émetteur
    injoignable, refus) est partagé de même.
  - reste : (1) plusieurs instances : deux requêtes concurrentes servies par deux
    instances présentent encore deux fois le jeton (la seconde ferme la session en
    `refresh_failed`, et un émetteur qui détecte la réutilisation peut révoquer toute
    la famille, RFC 9700 § 4.14.2) ; options : affinité de session au répartiteur, ou
    verrou partagé dans PostgreSQL (`pg_advisory_xact_lock` sur une empreinte du jeton,
    plus un résultat gardé le temps de l'échange) — arbitrage à prendre avant le
    déploiement multi-instance. (2) Une requête partie avec l'ancien cookie mais
    arrivée après la fin de l'échange (réponse du premier pas encore reçue par le
    navigateur) présente encore l'ancien jeton : seul un résultat gardé quelques
    secondes après l'échange la couvrirait (arbitrage : un ancien cookie y vaut le
    nouveau jeton pendant la fenêtre). (3) Une requête abandonnée (client parti)
    pendant l'échange laisse la suivante le refaire avec le même jeton.
  - portée, après la revue (2026-11-02) : la clause tient pendant l'échange chez
    l'émetteur et sur une seule instance ; l'entrée partagée est retirée dès le résultat,
    avant la route de la première requête, et une requête partie avec l'ancien cookie
    pendant toute la durée de cette route présente encore `r1`. Ce cas et le partage entre
    instances (D9) vont à la 16f (A24, A25)
- [x] 16f. Échange du jeton de rafraîchissement partagé entre instances, avec fenêtre de
  grâce (A24, A25) : un verrou en base (`pg_advisory_xact_lock` sur une empreinte du
  jeton présenté) sérialise l'échange entre instances (D9) ; le résultat (nouveaux jetons,
  groupes) est gardé chiffré 30 s après l'échange (ou le délai maximal d'une requête, s'il
  est plus long), sous la clé émetteur × sujet × empreinte du jeton présenté, puis purgé :
  une requête partie avec l'ancien cookie et arrivée pendant ce délai reçoit le même
  résultat au lieu de présenter de nouveau `r1` ; un échec partagé ne se garde pas ; la
  carte en mémoire de la 16d devient un cache devant la base ou disparaît — verify :
  `k-check -p kartenn-rest -p kartenn-auth` (deux instances sur la même base : deux
  requêtes simultanées → un seul appel à l'émetteur ; requête tardive dans les 30 s →
  même cookie neuf ; après 30 s → `refresh_failed` ; table purgée ; aucun jeton en
  clair en base) ; migration et revue de sécurité attentive
  - où : `crates/kartenn-auth/src/oidc.rs` (`exchanges`, `Turn`, `refresh`, `exchange`),
    `crates/kartenn-store-pg/migrations/` (table courte des échanges, chiffrée par la
    clé des cookies ou une clé dérivée), purge à la forme des jobs distribués (D9) ; test
    voisin `crates/kartenn-rest/tests/it/sessions.rs`
    (`concurrent_refreshes_present_the_token_once`) ; 005 design § « Rafraîchissement
    des groupes »
  - dépend de : 16d
  - fait le 2026-11-02 : port `RefreshExchanges` et `Claim` (`kartenn-service/src/authn.rs`),
    adaptateur `PgRefreshExchanges` et `purge` (`kartenn-store-pg/src/refresh_exchanges.rs`),
    migration 0074 (`refresh_exchange` : `key` empreinte de 32 octets, `holder` ou
    `sealed`, jamais les deux, `ends_at` ; tâche `refresh-exchange-purge`, 1 min) ;
    `Oidc::shared` et `exchange_once` (`kartenn-auth/src/oidc.rs`) : la carte de la 16d
    reste un cache devant la base (une requête par instance et par jeton l'interroge) ;
    sous `pg_advisory_xact_lock` sur l'empreinte, une transaction courte lit le résultat
    gardé ou réclame l'échange sous un bail de 40 s, puis se termine : l'émetteur est
    appelé hors transaction (30 s au plus), sans connexion tenue, et les autres instances
    redemandent toutes les 100 ms, sans connexion tenue entre deux questions ; le
    résultat est gardé la plus longue de 30 s et du délai maximal d'une requête
    (`authn` du serveur, `request_timeout_ms`) ; un échec rend la réclamation ; tâche
    `refresh-exchange-purge` (`kartenn-server/src/jobs.rs`) ; tests
    `two_instances_present_the_token_once` et
    `a_late_request_within_the_window_gets_the_same_tokens` (`sessions.rs`, deux `Oidc`
    sur la même base et le même trousseau, cartes distinctes) ; le test de la 16d sur le
    cookie resté sur `r1` le présente après la fenêtre ; 005 design § « Rafraîchissement
    des groupes », `docs/spec/data.md`, `docs/spec/interfaces.md`.
  - choix : chiffrement par une clé dérivée du trousseau des cookies (HKDF, but
    `kartenn/refresh-exchange/v1`, nouvelle variante `Purpose::RefreshExchange`) et, en
    plus, données associées liant le nom `refresh-exchange/<empreinte>` (le `seal` des
    cookies) : un résultat ne s'ouvre ni comme cookie ni sous une autre ligne ; empreinte
    SHA-256 étiquetée, chaque partie préfixée de sa longueur, plutôt qu'un HMAC sous le
    trousseau (un jeton de rafraîchissement a l'entropie d'un secret, et l'empreinte ne
    change pas pendant une rotation du trousseau) ; le verrou sérialise la lecture et la
    réclamation, la réclamation (bail en base) sérialise l'échange : aucune connexion
    pendant l'appel à l'émetteur, au prix d'un bail (40 s, au-delà des 30 s laissées à
    l'émetteur) qui fait attendre les autres instances si le porteur s'arrête ; une ligne
    échue se lit comme absente (un émetteur qui ne tourne pas le jeton le reprend au
    rafraîchissement suivant) ; un résultat qui ne se garde pas ou une réclamation qui ne
    se rend pas (base indisponible) est laissé à son bail ; base indisponible à la
    réclamation → 503, la session reste ; le port est un trait objet (`BoxFuture`)
    installé par `Oidc::shared`, pour ne pas ajouter de paramètre de type à `Oidc` ni à
    `Authn` ; sans lui (tests de `kartenn-auth`, banc), le partage reste par instance.
  - passe de corrections (revue, 2026-11-02) : l'échange réclamé tourne dans une tâche à
    part (`tokio::spawn`, bornée par `EXCHANGE_WITHIN`), qui garde son résultat ou rend
    la réclamation même si la requête est partie (16d (3)) ; `Oidc` tient donc son
    vérificateur et ses découvertes sous `Arc`, et `refresh` demande `S: 'static` ; la
    réclamation encore en cours de sa propre instance n'est plus reprise mais attendue
    (`Busy`), puisque sa tâche tourne encore ; l'attente sur `Busy` est bornée (bail plus
    un quart, puis 503) ; bail, fenêtre et purge à `clock_timestamp()` ; échecs de garde,
    de rendu ou de scellement journalisés par `tracing` (déjà au workspace, ajouté à
    `kartenn-auth`), la tâche détachée n'ayant plus forcément d'appelant ; `EXCHANGE_WITHIN`
    reste à 30 s : détaché, l'échange n'est plus coupé par le délai de la requête, et sa
    borne propre libère toujours la réclamation ; tests
    `a_lease_over_is_taken_over_and_a_lost_claim_keeps_nothing`,
    `a_row_is_a_claim_or_a_result` (`kartenn-store-pg/tests/it/refresh_exchanges.rs`),
    `a_request_dropped_during_the_exchange_leaves_its_result_kept` (`sessions.rs`),
    `the_purge_task_deletes_the_exchanges_past_their_end` (`kartenn-server/tests/it/jobs.rs`).
  - reste : rien de la 16f.
- [x] 24c. Indice d'opération dans un placement (A26) : `Placement.operation` (indice de
  l'opération source, `null` pour un objet placé par effet), au contrat et dans la
  simulation d'un changeset et d'un brouillon, y compris sous `within` et pour une
  opération de module ; le shell (`previewOf`, `web/v2/shell/src/views/topology.ts`)
  rapproche une création par cet indice et non plus par l'ordre — verify :
  `k-check -p kartenn-rest -p kartenn-service` (indice juste pour une création du noyau,
  une création de module et une création sous `within` ; `null` pour un enfant déplacé
  par effet) ; Vitest de `previewOf` (brouillon `[machines.create_machine,
  create_prefix]` et `[within(create_prefix A), create_prefix B]` : chaque ligne porte
  le bon libellé) ; contrat et schémas du SDK régénérés
  - où : `crates/kartenn-service/src/placements.rs`, DTO `Placement` de
    `crates/kartenn-rest/src/changesets.rs` ; `web/v2/shell/src/views/topology.ts`
    (`targetOf`, `previewOf`), `web/v2/shell/test/topology.test.ts`
  - dépend de : 24a, 24b
  - choix : l'indice d'un objet des opérations est celui de la première opération qui
    l'écrit (`sections::touched_at`, rapporté à l'opération de la requête par les
    origines du plan, cascade comprise) : pour une création, la création ; un objet écrit
    par une opération mais déplacé par une autre (un préfixe modifié sans changer de
    valeur, sous un préfixe inséré) garde l'indice de l'opération qui l'écrit, `null`
    restant réservé aux objets qu'aucune opération n'écrit ; côté shell, `targetOf`
    disparaît : `creationOf` déballe `within` et une création se rapproche par
    indice × genre × `before: null` ; test de module dans `kartenn-rest`
    (`placements.rs`, routeur de la construction, modules synchronisés)
- [x] 16e. Comparaison du contrat à travers les `$ref` : `contract::breaches` résout les
  références locales vers `components.parameters` avant de comparer (aujourd'hui une
  `$ref` reçoit la clé `("", "")` : rendre `Kartenn-Background` requis passerait
  inaperçu) ; l'en-tête déclaré aussi sur les opérations `x-query` — verify :
  `k-check -p kartenn-rest` (test : composant rendu requis → rupture signalée)
  - où : `crates/kartenn-rest/src/contract.rs` (`breaches`, vers les lignes 145-195),
    `crates/kartenn-rest/src/openapi.rs` (`finish`, vers les lignes 339-385)
  - dépend de : 16c ; avant le gel du contrat
  - fait : `components.parameters` comparés un à un (disparu, nom ou place changés,
    rendu requis, schéma) ; un paramètre d'opération identifié par son nom et sa place
    une fois sa référence résolue (`resolve`, pointeur JSON local, références en chaîne) ;
    les 26 opérations `x-query` prennent `Kartenn-Background` ; contrat régénéré, types
    du SDK inchangés (`gen.mjs` ne lit pas les paramètres des `x-query`) ; 4 tests
    unitaires dans `contract.rs`.
  - choix : une même `$ref` des deux côtés n'est pas comparée sur l'opération, son
    composant l'est une fois sous `components.parameters` (une rupture, pas une par
    opération) ; une référence qui ne se résout pas garde la référence pour identité
    (place `$ref`), si bien que deux références ne se confondent jamais ; un composant
    présent mais dont la référence ne se résout plus : « reference not resolved » ;
    un `x-query` d'un chemin public prendrait `security: []` comme les autres opérations.
  - reste : les schémas restent comparés par nom (`$ref` égales, puis
    `components.schemas` un à un, dans les deux sens) : sûr mais plus strict que
    nécessaire pour un schéma qui ne voyage que dans un sens ; aucune autre section de
    `components` (réponses, corps) n'est utilisée par le contrat aujourd'hui, et
    `breaches` ne les lit pas.
- [x] 14a. `PATCH /v1/drafts/{id}` en `application/merge-patch+json` seulement (A17,
  REQ-006-13), 415 pour `application/json` ; client du SDK en `merge: true` ; contrat
  régénéré — verify : `k-check -p kartenn-rest` (drafts : 415 en `application/json`,
  200 en `merge-patch+json`) ; Vitest du SDK des brouillons ; parcours `draft`
  - où : `crates/kartenn-rest/src/drafts.rs` (le `PATCH`), forme des autres `PATCH`
    du contrat ; `web/v2/sdk/src/data/drafts.ts` (vers la ligne 213, `merge: false`) ;
    test voisin `crates/kartenn-rest/tests/it/drafts.rs`
  - dépend de : —
  - fait le 2026-11-02 — `crates/kartenn-rest/src/drafts.rs` (`change` : corps lu par
    `resources::members` en `MERGE_PATCH`, 415 sinon ; `ChangeBody` en fusion RFC 7396,
    `ReasonPatch`, `given`), `crates/kartenn-store-pg/src/drafts.rs` (`update` : le motif
    fusionné membre à membre, `jsonb_strip_nulls(reason || $3)`), contrat
    `specs/006-api-rest/openapi-v1.json` et `web/v2/sdk/src/api/schema.d.ts` régénérés,
    `web/v2/sdk/src/data/drafts.ts` (`update` sans `merge: false`) ; tests
    `tests/it/drafts.rs` (`a_change_is_a_json_merge_patch` : 415 en `application/json`,
    membre absent laissé, motif fusionné, `null` qui efface un membre puis le motif,
    tableau remplacé, 422 pour `title` ou `operations` à `null`, 400 pour un membre
    inconnu du motif), `web/v2/sdk/test/drafts.test.ts` (type de contenu) ; l'ancien
    shell n'appelle pas la route (rien à aligner)
  - choix : `null` sur `title` ou `operations` → 422 « cannot be removed » (pointeur),
    comme `kit::modify_nullable` ; `reason: null` vide le motif (`{}`) ; la fusion du
    motif se fait dans l'`UPDATE` verrouillé (pas de lecture préalable) ; `title` et
    `operations` non nullables dans `DraftChange` ; le SDK prend le défaut d'un `PATCH`
    (fusion) comme `write.ts`, plutôt qu'un `merge: true` explicite ; l'ordre reste
    428 avant 415 (`every_route_under_if_match_requires_it`)
- [x] 24a. `placements` dans `ChangesetSimulated` (A12) : `[{object, kind, before,
  after}]` pour chaque objet dont le parent change (déduit : préfixes, plages, adresses ;
  déclaré : modules), au présent, sur la simulation d'un changeset et d'un brouillon —
  verify : `k-check -p kartenn-rest` (préfixe inséré entre un parent et ses enfants : les
  enfants changent de parent ; préfixe retiré : ses enfants remontent ; rien pour un
  changeset sans effet sur l'arbre)
  - où : `crates/kartenn-rest/src/changesets.rs` (`ChangesetSimulated`, `simulate`) ;
    la simulation du service qui produit `diff` et `cascaded` ; test voisin
    `crates/kartenn-rest/tests/it/simulated.rs` ; 006 design § « `?simulate=true` »
  - dépend de : —
  - fait le 2026-11-02 : service `crates/kartenn-service/src/placements.rs` (`prior`
    avant les écritures, `placements` après, avant le rollback de la simulation ;
    `Outcome::Simulated.placements`) ; port `WriteTx::tree_places` (parent déduit ou
    déclaré au `now()` de la transaction, avec des préfixes supposés : `Supposed`) et
    `WriteTx::tree_children` (enfants qu'aurait une valeur, la marche `tops` de l'arbre),
    `crates/kartenn-store-pg/src/tree.rs` (`places`, `children_within`) ; crochet
    `ModuleHandler::parents` (parent déclaré hors de la fiche), servi par `machines` (hôte
    d'une machine, `pg::hosts`) ; REST `Placement`, `placements` et
    `placements_truncated` dans `ChangesetSimulated` ; contrat et `schema.d.ts`
    (`web/sdk`, `web/v2/sdk`) régénérés, `write.ts` (simulation refusée) complété ;
    006 design et `docs/spec/interfaces.md` ; tests `crates/kartenn-rest/tests/it/placements.rs`
    (insertion, aussi par un brouillon ; fermeture et redimensionnement ; rien sans effet ;
    troncature à 1000) et `a_simulated_move_places_the_machine_under_its_new_host`
    (`kartenn-machines`).
  - choix : candidats lus sans relire l'arbre : les objets des opérations, plus les
    enfants (après écriture) des anciennes et nouvelles valeurs des seuls préfixes dont la
    valeur change, apparaît ou disparaît ; leur parent d'avant déduit de l'état d'après
    où les préfixes du changeset reprennent leur valeur d'avant. Objet créé listé avec
    `before: null` (y compris à la racine, `after: null`) ; objet fermé non listé (il
    quitte l'arbre ; `cascaded` et les opérations le nomment). Borne : au plus 1000
    placements et 1000 objets lisibles examinés hors des opérations, au-delà
    `placements_truncated: true` (coût borné à une marche de 1001 enfants et deux
    lectures en lot ; simulation entière d'une insertion sur 1001 enfants ≈ 130 ms en
    test, profil dev). Filtre de lecture de l'arbre (correction de revue) : sous les
    droits de l'auteur (`Authorizer::authorize_in`, `core.<genre>.read` ou la lecture du
    type de module), un objet illisible n'est pas placé et ne consomme pas le budget
    (enfants lus page par page, `tree_children` avec curseur), un parent illisible se
    sert `null` (le haut de l'arbre que l'auteur lit, sans marqueur distinct qui
    trahirait son existence), un placement resté sans changement de parent est retiré ;
    `placements_truncated` se compte sur le lisible ; lisibilité d'avant lue avant les
    écritures, d'après sur l'état simulé. Objet de module placé seulement si son type a
    un parent de fiche ou si son module le place (`parents`) : ni flux ni politique.
    Parent d'un groupe de VLAN : son ancrage ; d'un VLAN : son groupe.
  - reste : le budget compte aussi les enfants lisibles dont le parent ne bouge pas (une
    valeur redimensionnée qui garde ses enfants) ; des enfants illisibles nombreux se
    parcourent sans borne de pages ; le test de lecture retire la lecture des adresses
    partout, pas sur les adresses d'une seule entité.
- [x] 17a. Référence locale dans une valeur de propriété (A13) : `RawValue`
  `{"local": n}` (`RawDto`, `RawInput`), résolue à l'application comme les `Ref` des
  opérations, prise dans le tri topologique ; référence vers un indice absent ou vers un
  objet d'un autre type que la déclaration → violation codée ; brouillon et simulation
  compris — verify : `k-check -p kartenn-rest -p kartenn-service` (VLAN créé et posé en
  `core.vlan` sur un réseau dans un seul changeset, appliqué puis au brouillon ; indice
  absent → 422 codé) ; revue attentive du contrat du noyau
  - où : `crates/kartenn-rest/src/operation.rs` (`RawDto`),
    `crates/kartenn-service/src/operations.rs` (`Ref`, résolution des références
    locales), lecture des valeurs de référence dans `kartenn-core` (`RawInput`) ; test
    voisin `crates/kartenn-rest/tests/it/vlan/` ; 004 design § « Références internes »
  - dépend de : —
  - fait le 2026-11-02 — `RawInput::Local { local }` (`{"local": n}`, variante non
    étiquetée, après `External`) et `RawDto::Local` au contrat (`RawValue`, contrat et
    schémas des deux SDK régénérés) ; les formes objet refusent un champ inconnu
    (`RawWire`, `deny_unknown_fields`, aussi par le contrat). `RawInput::to_raw` rend
    `Err(nom)` pour un nom local ; une suggestion de source qui en porterait un ne
    propose rien d'utilisable (`expected.rs`). Résolution au planifié
    (`pipeline::plan` → `resolution::plan`, qui rend désormais un `Result`) : le nom
    désigne l'UUID généré pour l'objet d'une opération antérieure, posé en `Raw::Ref`
    (nouvelle forme du cœur), que seul `RefType` accepte : une propriété texte ou nom
    d'hôte la refuse (`prop.assignment.invalid_value`). Les noms des créations
    d'objets sont suivis à part (`creates_object` : opérations du cœur qui créent,
    `Operation::created_kind`, déplacée d'`import.rs` ; opérations de module dont le
    type déclare `defines`). Import : `uses` compte les noms des valeurs dans le tri
    topologique, `resolve_known` remplace un nom d'un autre lot par le texte de l'UUID
    (et non `{"id": …}`, qui n'est pas une valeur). Brouillon : rien à changer à
    `drafts.rs`, qui stocke et relit des `OperationDto`. Tests (`vlan::networks`,
    `vlan::rest`, `vlan::imports`) : VLAN créé et posé sur un réseau en un changeset,
    simulé (rien d'écrit) puis appliqué ; par les routes, appliqué, puis en brouillon
    relu tel quel, simulé sans violation et soumis ; nom absent → 422 codé, rapporté à
    son opération ; import où l'affectation vient en premier et le VLAN dans un autre lot.
  - choix : un seul code nouveau, `prop.assignment.unknown_local` (REQ-004-4, 021/A13),
    pour un nom qu'aucune opération antérieure ne définit (absent ou défini plus tard)
    et pour celui d'une opération qui ne crée pas d'objet (liaison, appartenance,
    compte de service), messages distincts ; un objet d'un autre type que la déclaration
    donne `prop.assignment.invalid_value`, comme un UUID du mauvais type (même contrôle
    `RefType`). Violations du planifié (422), non des `RequestError` (400) comme les
    `Ref` des opérations : la déclaration visée n'est connue qu'en base, et le shell
    lit les violations par opération. Un nom défini après son usage est refusé, comme
    pour les `Ref` (« plus tôt », REQ-004-4) ; seul l'import réordonne. Le schéma
    `web/sdk` (ancien shell), en retard depuis la 11a, est régénéré en entier.

## P2 — Écrans du nouveau shell (phase 4)

Chaque zone : écrans du nouveau shell dans `web/v2/shell`, parcours de l'ancien portés dans
`web/v2/e2e` et verts, captures comparées aux maquettes validées, revues `ui-reviewer` et
`a11y-auditor`, lignes de la liste de parité cochées. L'ancien shell n'est pas modifié.

- [x] 9. Cadre : squelette statique de `index.html` et feuille critique, barre latérale
  (espaces, repli), barre d'état (fil d'Ariane, pastille de date, « Brouillon (n) », menu
  de l'utilisateur : langue, thème, densité, raccourcis), bande hors présent, lien
  d'évitement — REQ-021-1, 2, 4, 5, 6 — verify : parcours `shell`, `time` ; CLS ≤ 0,05 ;
  2.4.11 (focus non masqué par la barre et la bande)
  - où : `web/shell/src/app/AppShell.vue`, `navigation.ts`, `UserMenu.vue`,
    `time/TimeBanner.vue`, `time/index.ts` ; maquettes E1, E6 ; `web/shell/index.html` ;
    repris de la tâche 7 : `provideLink(KLink)` (`web/v2/shell/src/prefetch/KLink.vue`) à
    la racine du cadre, `meta.breadcrumb` à côté de `meta.queries` (`prefetch/intent.ts`) ;
    garde du temps en `beforeEach` (dates reportées écrites dans la cible avant la garde de
    préchargement, qui lit une cible sans dates au présent)
  - dépend de : 7, 8 ; 020-23
  - fait le 2026-10-31 : `web/v2/shell/index.html` (squelette du cadre, statut
    « Chargement de kartenn… », `noscript`) et `public/boot.css` (feuille critique liée en
    premier : géométrie du cadre en jetons et ordre des couches entier) ;
    `src/frame/ShellFrame.vue` (cadre de la maquette `EcranCadre.vue` : `SidebarNav`
    repliable gardée en `kartenn.sidebar`, tiroir en classe compacte, en-tête `banner`
    avec `Breadcrumb`, `DatePill`, `DraftCounter`, `Menu` de l'utilisateur, puis
    `TimeBand` ; surface `main`, `SidePanel` Détail/Brouillon gardé en `kartenn.panel`,
    `CommandPalette` ouverte par « Rechercher » sur la navigation, `ToastRegion`, couche
    `#k-layers` ; `provideLink(KLink)`, titre du document et page atteinte dite par une
    région `status`) ; `frame/spaces.ts` (espaces du cœur dans l'ordre fermé, entrées
    par droits), `routes.ts` (une route par entrée, `meta.breadcrumb`, vue d'attente
    `PendingView.vue`, page introuvable), `breadcrumb.ts`, `settings.ts` (thème,
    densité, mouvement, raccourcis à une touche `kartenn.shortcuts.single`, panneau),
    `session.ts` (`GET /v1/me` avant la première page du cadre, puis `startData`) ;
    `src/rights/` (repris de `web/shell`, lecture par le client du SDK) ; `src/time/`
    (repris, `carryTime` en `beforeEach` avant la garde de préchargement, `goToPeriod`,
    `checkPeriod` : refus de l'API rendu sur le champ de la pastille, un objet neuf par
    refus) ; `src/i18n/` (vue-i18n, catalogues FR/EN, langue gardée en
    `kartenn.locale`) ; `main.ts` (gardes session, temps, préchargement ; montage après
    `router.isReady()`, échec du démarrage dit sur le squelette) ; `meta.breadcrumb`
    déclaré dans `prefetch/intent.ts`. Vitest : `shell/test/frame.test.ts` (espaces par
    droits, `locate`, fil d'Ariane, ordre des gardes et dates reportées lues par la garde
    de préchargement, retour au présent, session lue une fois et jamais pour `/_ui`,
    `startData` seulement connecté, `checkPeriod`, réglages gardés), `build.test.ts`
    (feuille critique liée en premier, squelette dans `index.html`). Parcours
    (`web/v2/e2e/journeys/`, `global-setup.ts` : émetteur rattaché, groupes de
    `users.toml` liés) : `shell.spec.ts` (lien d'évitement premier arrêt, espaces d'un
    lecteur et d'un administrateur, fil d'Ariane, barre repliée gardée au rechargement,
    langue, thème et densité gardés, compteur du brouillon bascule du panneau, CLS ≤ 0,05
    sous Chromium, 2.4.11 : chaque lien d'une surface longue, sous la bande, est le point
    au centre de sa boîte), `time.spec.ts` (lien daté identique pour un second
    navigateur, navigation qui garde les dates, retour au présent et focus sur la
    pastille, `valid_at` seul, période appliquée par la pastille, `as_of` refusé par
    l'API rendu sur son champ), `served.spec.ts` mis à jour.
  - choix : menu de l'utilisateur en groupes radio (langue, thème, densité) et cases
    (« Toujours réduire le mouvement », raccourcis à une touche) plutôt que les entrées
    uniques de la maquette, qui n'agissaient pas ; VLAN et groupes de VLAN dans l'espace
    `network` (REQ-021-2), la maquette les plaçant sous Référentiel ; entrées masquées
    par droits (accueil, anomalies et propriétés : l'une des lectures du référentiel ;
    brouillons et imports : une édition du cœur, `prop.<clé>.edit` ou
    `core.draft.admin` ; rôles aussi `core.role.admin`, identités aussi
    `core.account.admin`), l'élément d'espace du fil d'Ariane et `/admin` mènent à la
    première entrée visible ; `meta.breadcrumb(params, t)` (libellés traduits, design
    § « Routes » amendé) ; vérification d'une période par une ligne de la première liste
    datée que l'utilisateur lit (`probePath`, ordre : domaines, entités, sites, groupes de
    VLAN, VLAN, préfixes, plages, adresses), sans lecture permise rien n'est demandé, seul
    un 400 la refuse, la réponse d'une demande dépassée est écartée ; un déplacement dans
    l'historique (Retour, Avancer) ne reporte aucune date ; vue d'attente et cadre dans le
    premier fichier ; `boot.css` dans `public/` (copiée telle quelle, liée en premier) ;
    squelette à la géométrie du cadre par un script de démarrage en fichier
    (`public/boot.js`, CSP `'self'`, aucun script en ligne) qui lit avant la peinture
    l'état gardé (barre repliée, panneau ouvert et sa largeur, thème, densité,
    mouvement) et les dates de l'URL (rang de la bande), et marque la racine
    (`data-boot-*`) ; parcours : boîtes du squelette et du cadre égales emplacement par
    emplacement à 1280 et 1100 px, au départ, repliée, datée, repliée datée panneau
    ouvert ; build du shell (`vite.config.ts`) : `@kartenn/ui` en
    un seul fichier (groupe `ui` de `codeSplitting`, `vue` dans son groupe propre), car
    un écran paresseux scindait des composants de la bibliothèque, dont les feuilles
    passaient alors avant la sienne ou manquaient (`Dialog` absent des pages d'`AlertDialog`,
    `Drawer`, `CommandPalette` du catalogue : captures de référence réécrites, voile et
    dialogue désormais stylés) ; brouillons lus
    seulement par qui détient une édition ; sans session, le cadre montre ce qui ne
    demande aucun droit (connexion : tâche 16).
  - reste : registre, `[`, `]`, Ctrl+K, aide et palette complète (tâche 10 ; le réglage
    à une touche est gardé, rien ne le lit encore) ; contenu de l'onglet Brouillon
    (tâche 14) ; compteurs de la barre latérale (tâches 11, 14) ; entrées des plugins par
    espace, connexion, « À propos », déconnexion (tâche 16) ; à la bibliothèque (020) :
    groupes radio d'un `Menu` nommés par leur étiquette, `lang` d'une entrée (noms des
    langues), relevés par la relecture d'accessibilité ; à arbitrer : catalogues de
    vue-i18n précompilés au build et compilateur de messages retiré (design « Pile
    retenue ») — le compilateur reste, faute d'outil de précompilation dans le dépôt, et
    la 007 (« vue-i18n et CSP ») le garde pour les catalogues des plugins ; squelette à
    la classe compacte, où la bande peut aller à la ligne (non mesuré).
- [x] 10. Raccourcis et palette : registre (portée, conflits, réglage à une touche), un
  écouteur, séquences `g`, aide `?` et `/help/shortcuts`, palette Ctrl+K (navigation,
  actions, recherche par valeur, récents, commandes des plugins) — REQ-021-3, 7, 8 —
  verify : Vitest du registre ; parcours : `g c`, `?`, saisie « gc » dans un champ sans
  effet, réglage désactivé ; palette au clavier seul
  - où : `web/shell/src/app/SearchBox.vue`, `views/SearchView.vue` (recherche actuelle) ;
    design § « Raccourcis » ; maquette E8 ; l'aide dit l'Échap en deux temps des
    filtres (020, décision A8)
  - dépend de : 9
  - fait le 2026-10-31 : `web/v2/shell/src/shortcuts/` — `registry.ts` (registre : `id`,
    touches en combinaison ou séquence, portée `global`/`view`, lieu dit par l'aide,
    `shown` par droits ; refus des mêmes touches et d'une touche seule qui ouvre une
    séquence ; écouteur unique `keydown` sur `document`, bouillonnement : ignore
    `defaultPrevented`, composition, touche tenue, Ctrl/Alt/Méta sauf AltGr, champ
    éditable, `data-k-typeahead`, rôles `menu`/`menubar`/`listbox`/`tree`, dialogue ;
    séquence 1,5 s ; Ctrl+K et ⌘K partout ; réglage coupé : combinaisons seules),
    `shell.ts` (jeu du shell dans l'ordre de l'aide, `g` + lettre par droits,
    `SHELL_KEYS` réservées aux plugins par `reserveKeys`, raccourcis des vues des plugins
    lus au registre du SDK), `view.ts` (`useViewAction` : `/`, `c`, `e` d'une vue, `c` et
    `e` au présent et avec `allowed`), `search.ts` (recherche par valeur, `q` sur
    préfixes et adresses, et par slug ou nom sur domaines, entités, sites, VLAN, groupes,
    par droit de lecture, par le cache du SDK aux dates de la page, 150 ms après la
    dernière touche, réponse dépassée écartée), `recent.ts` (objets récents,
    `kartenn.recent.objects.<identité>`, filtrés par droits de lecture, `forgetRecent` pour
    la déconnexion), `help.ts` (lignes de l'aide), `palette.ts` (groupes :
    objets trouvés ou récents, commandes, pages ; commandes des plugins hors droits
    absentes), `ShortcutHelp.vue` (table de l'aide, réglage dit), `HelpView.vue`
    (`/help/shortcuts`), `context.ts` ; `frame/ShellFrame.vue` (registre, écouteur, aide
    `?` en dialogue, palette complète : créer et modifier de la vue, ouvrir le brouillon,
    revenir au présent, barre latérale, panneau, aide, commandes des plugins ; entrée
    « Raccourcis clavier » du menu de l'utilisateur ; `[` et `]` montrés sur leurs
    boutons tant que le réglage les permet) ; `frame/routes.ts` (`/help/shortcuts`,
    fiches `/:collection/:id` en vue d'attente pour les collections cherchées) ;
    catalogues FR/EN ; `@kartenn/ui` : `SidebarNav` `singleKeys`, `SidePanel` `shortcut` ;
    maquette E8 sur `ShortcutHelp` et le jeu réel. Vitest : `shell/test/shortcuts.test.ts`
    (conflits, clés du shell refusées aux plugins, séquence et délai, champ, consommateur,
    menu, dialogue, modificateurs, Ctrl+K dans un champ, réglage coupé, raccourci d'un
    plugin, navigation par droits, actions de la vue au présent, groupes de la palette,
    commande d'un plugin absente sans `core.vlan_group.edit`, recherche par valeur et par
    nom, récents). Parcours `e2e/journeys/shortcuts.spec.ts` (`g c`, `?` et
    `/help/shortcuts`, lecteur sans `g x`, « gc » dans le champ de la palette, réglage
    coupé gardé au rechargement avec Ctrl+K actif, `[` et `]`, palette au clavier seul :
    « 192.0.2. » Entrée → fiche du préfixe, récent, page) ; `global-setup.ts` charge
    une entité, un domaine, un préfixe et une adresse (RFC 5737).
  - choix : `g v` au cœur (VLAN, écrans du cœur depuis la 026), `m t q f` laissées aux
    manifestes (tâche 16) ; `g x` mène à `/admin` ; une navigation hors des droits n'est ni
    montrée ni jouée ; `/`, `c`, `e` passent par `useViewAction` (aucune vue ne les donne
    encore : tâches 11 et 12) ; les touches seules s'arrêtent aussi dans tout dialogue et
    dans les rôles `menu`, `menubar`, `listbox`, `tree` (recherche à la saisie de l'APG) ;
    AltGr compte comme une modification (REQ-021-8 : aucune autre que Maj), donc `[` et `]`
    tapés par AltGr (AZERTY, dont Mac) ne jouent rien : la palette (« Replier la barre
    latérale », « Ouvrir le panneau droit ») et les boutons restent le recours ; une lettre
    tapée avec Verr. Maj sans Maj compte comme sans ; Ctrl+Maj+K n'est pas
    Ctrl+K ; une touche nommée (Tab, Échap) clôt une séquence ; Ctrl+K ne s'ouvre pas
    par-dessus une autre couche modale (le tiroir de la barre lui cède la place), et une
    commande de la palette ne joue qu'une fois le focus rendu à son ouvreur (l'aide ouverte
    de la palette rend le focus au bouton « Rechercher » ; un focus ailleurs, une nouvelle
    ouverture ou 1 s sans retour du focus abandonnent la commande) ; registre : mêmes touches
    toujours refusées, préfixe de séquence refusé entre raccourcis joués seulement (Échap
    et Échap Échap de l'A8 sont documentaires) ; aide : réglage dit en tête, réglage coupé
    chaque touche seule jouée marquée « (désactivé) » dans son action, colonne
    « Portée » de priorité 2, libellés courts de la maquette ; `reserveKeys` posé dans
    `main.ts` avant le montage ; objets de la palette en un
    groupe « Objets » comme la maquette, nom = type en mots et lieu (« Préfixe ·
    production »), valeur en chasse fixe en détail, 5 par collection et 8 en tout ;
    recherche par `q` (début de valeur, de slug ou de nom ; filtre typé du contrat), par
    valeur seulement si le texte en a l'air, annulée (`AbortController`, annulation de la
    requête du cache) à chaque saisie et à la fermeture, les objets trouvés gardés jusqu'à
    la réponse suivante ; page de 20 (la suggestion du contrat) : les filtres de liste du SDK
    écartent `page_size` (une clé de cache par filtre), la palette en prend les premiers ;
    une collection en échec est omise sans message ; récents gardés en `localStorage` comme
    les recherches de la 007 (REQ-007-28), sous l'identité de l'utilisateur
    (`kartenn.recent.objects.<identité>`, un UUID), montrés seulement pour les collections
    qu'il lit encore, effacés à la déconnexion (tâche 16) ; fiches `/:collection/:id` en vue d'attente pour que la palette y mène ;
    indices des touches seules masqués quand le réglage les coupe (palette, menus,
    `SidebarNav`, `SidePanel`) ; `CommandPalette` dit « Chargement » tant que l'hôte
    cherche, même avec des entrées ; `ShortcutHelp` sans feuille propre (partagé avec E8,
    il passerait dans un morceau commun refusé par `build.test.ts`).
  - reste : aucun raccourci ni touche réservée pour les messages passagers (décision
    ouverte) ; F8 et les touches de fonction restent libres au registre.
- [x] 11. Référentiel (dépend aussi de 28 et 29) : accueil en tableau de bord
  (`GET /v1/stats`, tuiles `widget`), Topographie (arbre des préfixes en `TreeGrid`,
  colonnes de plugins, « + » sur un nœud ; 024), listes
  (`DataGrid`, filtres typés tirés du contrat : `filters.ts` généré comme `dated.ts`,
  un seul modèle pour la barre et les filtres de colonne, 020 décision A7),
  fiche en panneaux, anomalies, propriétés — REQ-021-9, 18 — verify : parcours `records`,
  `tree` réécrits ; lien filtré copié rouvert à l'identique ; captures E0 à E3
  - où : `web/shell/src/views/{HomeView,ListView,ObjectView,AnomaliesView,PropertiesView}.vue`,
    `record/*.vue`, `tree/model.ts`, `data/collections.ts` ; `web/sdk/scripts/gen.mjs`
    (génération de `dated.ts`, à étendre) ; repris de la tâche 7
    (`web/v2/shell/src/prefetch/`) : `meta.queries` des routes, `useNextPage` sur la
    fenêtre de la `DataGrid`, `childrenIntent` sur la Topographie, parcours « fiche
    survolée puis ouverte sans squelette » ; repris de la tâche 16
    (`web/v2/shell/src/plugins/slots.ts`, `ExtensionElement.vue`) : tuiles des plugins
    après celles du cœur (`useWidgets`, une `ExtensionElement` par tuile avec son
    `status`), onglets et panneaux de la fiche (`useExtensions("tab" | "panel", type)`,
    `tabItems`, `object` par `objectRef`), colonnes des listes (`gridColumns`,
    `elementProps` de la `DataGrid`), menu d'actions de l'objet (`actionItems`,
    `PluginAction`) ; parcours `plugins` étendu avec le plugin de test `notes`
    (`web/v2/e2e/plugins/notes`, onglet sur `prefix` et `vlan`, panneau, colonne, tuile)
  - dépend de : 10 ; 020-19, 020-25, 020-26
  - en cours : fait le 2026-11-01 — `web/v2/shell/src/views/` : `routes.ts` (écran et
    `meta.queries` de chaque entrée du référentiel et de chaque fiche, chacun dans son
    morceau ; `frame/routes.ts` les prend, fiches aussi pour les plages et `/changesets/:id`
    en attente, nom de l'objet dans le fil d'Ariane par `names.ts`), `collections.ts`
    (colonnes, filtre de chaque colonne, nom d'une ligne), `filters.ts` (modèle de
    `FilterPicker` tiré de `filterModel` du SDK — `filters.ts` généré par la 28 — et des
    déclarations : `within` opérateur de la valeur, listes fermées du contrat, slug d'une
    collection, identifiant pour une référence ou pour `domain`/`entity` des anomalies et
    lacunes, propriétés typées par leur déclaration ; URL ⇄ pastilles, filtres envoyés =
    ceux que `checkFilter` accepte), `list.ts` (`useFilteredList` : filtres de l'URL,
    brouillons à part, liste lue après les déclarations quand l'URL porte un `prop.`,
    `useNextPage` sur la fenêtre de la `DataGrid`, curseur refusé → liste relue du début et
    dite), `ListView.vue` (E3 : `FilterPicker` + `DataGrid`, filtre de colonne =
    `openFor` de la barre, A7 ; `/` par `useViewAction` ; colonnes des plugins par
    `gridColumns`), `RecordView.vue` et `record/` (E2 : onglets dans l'URL — propriétés
    résolues et `Popover` d'explication, contenu d'un domaine ou d'un préfixe, historique,
    anomalies de l'objet et leur acquittement —, attributs, occupation et prochain libre,
    onglets, panneaux et actions des plugins), `HomeView.vue` (E0 : tuiles de
    `GET /v1/stats`, histogramme, tuiles `widget` des plugins), `TopologyView.vue` (E1 :
    domaine dans l'URL, `TreeGrid` des racines et enfants paginés par la cache,
    `childrenIntent`, contenu du nœud `?node=`, Entrée → fiche), `AnomaliesView.vue`
    (anomalies et lacunes, `?tab=gaps`, acquittement avec motif), `PropertiesView.vue`
    (déclarations, valeurs d'une liste fermée, lien aux lacunes) ; `parts/ViewPanel.vue`,
    `parts/AttributeList.vue` (repris d'`EcranPanneau`, `EcranAttributs`, qui les
    enveloppent ; déplacé dans `@kartenn/ui` par la 020-29) ; catalogues `views/i18n/{fr,en}.json`. `@kartenn/ui` : `FilterPicker`
    expose `openFor(clé, origine)` et `openAdd()`, état de catalogue `columns`. Vitest
    `shell/test/views.test.ts` (modèle, URL aller-retour, valeur hors type gardée et non
    envoyée, lien copié = même clé de cache, colonnes filtrables seulement si servies,
    routes et lectures, fil d'Ariane nommé, liste lue avec ses filtres) ; script
    `e2e/components/filter-picker.spec.ts` (filtres de colonne) ; parcours
    `e2e/journeys/records.spec.ts` (domaine → préfixe → adresse, explication, historique ;
    liste longue virtualisée et lue d'avance, ligne ouverte au clavier ; curseur refusé ;
    filtre de colonne puis lien copié rouvert à l'identique dans une autre session ; fiche
    survolée puis ouverte sans squelette), `tree.spec.ts`, `plugins.spec.ts` étendu (tuile,
    colonne, onglet, panneau de `notes`) ; jeu de données en RFC 3849 (`global-setup.ts`).
  - choix : panneau et liste d'attributs du shell, non de la bibliothèque (pièces des
    maquettes laissées « à trancher » par la 020-15) ; `q` hors de la barre (recherche de la
    palette) ; liste et fiche lisent les filtres de l'URL, la barre en garde une copie avec
    ses brouillons ; propriétés déclarées lues au plus 10 pages, compte dit partiel en
    lecture et au plafond ; un filtre `prop.` de l'URL attend toutes les pages utiles des
    déclarations, et resté sans type (plafond, clé inconnue, lecture refusée) il est une
    pastille « non reconnue » et un état d'erreur, jamais une liste lue sans lui (ni
    préchargée sous une autre clé) ; `q` envoyé aux seules listes qui le déclarent, sinon la
    première page filtrée par le texte ; anomalies d'un domaine ou d'une entité lues page à
    page à l'approche de la fin de la grille, celles d'un objet d'un domaine dans celles du
    domaine, 10 pages au plus, « liste peut-être incomplète » au-delà, celles d'un objet
    hors domaine (site) « lecture non disponible » ; nœud de `?node=` non encore lu relu
    comme préfixe ; domaine de `?domain=` cherché dans les pages suivantes (10 au plus),
    sinon le premier et un avis ; vue de plugin `/x/…` en morceau paresseux (son hôte
    `ExtensionElement`, partagé avec les écrans, n'a plus sa feuille liée par la page) ;
    `c` et `e` laissés à la 12 (panneau d'édition) ; titre de fiche = sorte (dictionnaire)
    puis nom ; comptes au format de la langue ; « il y a … » de l'accueil sur l'horloge du
    serveur. Session (décision de la session principale, option b, côté client) : seules
    les lectures nées d'un geste de l'utilisateur (touche, pointeur, molette, navigation,
    dans les 10 s avant la requête) sont une activité (`session/expiry.ts`,
    `installGestures`, `markGesture`) ; les relectures sur événement du flux, à la
    revalidation ou au chargement des plugins ne le sont plus.
  - reste : Topographie — colonne d'occupation (`TreeNode` de `roots`/`children` ne la sert
    pas : l'ajouter au contrat 006, ou une lecture par ligne rendue), colonnes des plugins
    sur le `TreeGrid` (il n'a pas de colonne d'élément, comme la `DataGrid`), « + » sur un
    nœud (panneau d'édition, tâche 12) ; fiche — colonne « Source » des propriétés et
    encart « propriété attendue manque » (`Resolved` ne sert ni la source ni les lacunes de
    l'objet), motif et auteur de l'historique (absents de `Version`), comptes des onglets ;
    tri des listes (aucun paramètre de tri au contrat) ; tuile VLAN sans l'occupation des
    groupes (`BarList`) ; acceptation de REQ-021-18 sur `site = <site>` (le jeu de données
    n'a ni site ni `core.site`) ; actions du type `changeset` (tâche 14) ; onglet `notes`
    sur `vlan` (écrans VLAN, tâche 17) ; captures E0 à E3 des vrais écrans : non prises —
    ils ne s'affichent qu'avec kartenn-server (projet `visual` sans API ; les parcours
    tournent sur deux navigateurs avec des identifiants et des instants qui changent), il
    faudrait un banc de captures à API simulée, tâche à part ; les captures des maquettes
    E0 à E3 restent les mêmes. Manques du contrat 006, à arbitrer (contournés ici) : pas de
    filtre `object` sur `GET /v1/anomalies` (l'onglet lit les anomalies du domaine et les
    trie, plafonné ; rien pour un objet hors domaine), pas de filtre par porteur ni de
    recherche par clé sur `GET /v1/property-declarations` (toutes les pages lues pour
    typer un filtre).
  - fait le 2026-11-13 — relevé contre le code (main `94430d28`) : le livré du 2026-11-01
    tient, et le reste ci-dessus est levé ou réparti. Levés : occupation de la Topographie
    (11a, 11b : `TopologyView.vue`, `views/occupancy.ts`) ; « + » d'un nœud (12, 11b) ;
    filtre `object` des anomalies (11k, `record/AnomaliesTab.vue`) ; actions des plugins
    sur `changeset` (14 : `changesets/ChangesetView.vue`, `useExtensions("action",
    "changeset")`) ; onglet `notes` sur un VLAN (17 : `plugins.spec.ts`). Répartis : colonnes
    des plugins sur le `TreeGrid` → 11o ; tuile VLAN et occupation des groupes → reste de
    la 17 (« groupes les plus remplis ») ; captures E0 à E3 des vrais écrans → 12l
    (banc de captures à API simulée, transverse) ; manques du contrat (source des valeurs et
    lacunes d'un objet, motif et auteur de l'historique, tri, comptes des onglets,
    déclarations par porteur ou par clé) et acceptation de REQ-021-18 → 11p, à
    l'utilisateur. `verify` tenu : parcours `records` et `tree` réécrits (lien filtré
    copié rouvert à l'identique dans une autre session, « a filtered link copied… ») ;
    ajouté ici, l'acceptation de REQ-021-18 telle que le modèle la permet : parcours
    `records` « prefixes filtered by their site and their owner… » (site et entité
    propriétaire choisis parmi les suggestions de l'API, dans un domaine ; lien rouvert
    dans une autre session : mêmes lignes, mêmes pastilles). Ce parcours a levé un écart :
    une pastille de référence (`core.owner`, `core.site`, `domain` et `entity` des
    anomalies et lacunes) rouverte par un lien disait l'identifiant de l'objet, son nom
    n'étant connu que des suggestions de la session ; `views/filters.ts` lit désormais
    l'objet une fois par son identifiant (`cacheObject`, `GET /v1/<collection>/{id}` par la
    cache) et la pastille suit (noms réactifs) ; Vitest `shell/test/views.test.ts` (« names
    a reference of a link opened afresh… »).
  - choix : nom d'une pastille de référence lu par l'objet, une fois par identifiant et par
    modèle, seulement pour une valeur en forme d'UUID et une collection du cœur
    (`COLLECTIONS`) ; lecture refusée ou objet inconnu : l'identifiant reste montré ;
    l'acteur du journal d'audit, filtré en texte (`AuditView.vue`), inchangé.
  - reste (revue de la clôture, mineurs) : doc de `cacheObject` qui cite les seules
    entités ; `reads.object` lit sans l'instant de la liste (puce au nom actuel sur une liste
    datée) ; `model` recalculé à chaque page de déclarations (noms appris perdus, identifiant
    montré un instant) ; `referenceLabel` refait `useScopeNames`/`nameOf` ; le cas refusé du
    test passerait sans la lecture, `cacheObject` non testé seul.
- [x] 11o. Colonnes des plugins sur l'arbre de la Topographie (reste de la 11) : le
  `TreeGrid` prend les colonnes d'élément comme la `DataGrid` (`elementProps`, l'élément
  du plugin par cellule ; amendée le 2026-11-14 après la revue : l'`ExtensionElement` par
  cellule et son `status`, clause de REQ-021-13, passent à la 11o2, pour les deux
  grilles), `TopologyView.vue` y met
  `gridColumns("prefix")` après les colonnes du cœur — REQ-021-13, REQ-021-9 — verify :
  Vitest du `TreeGrid` (colonne d'élément, ligne virtualisée), script
  `e2e/components/tree-grid.spec.ts` (cellule d'élément atteinte au clavier) ; parcours
  `plugins` (colonne de `notes` sur la Topographie)
  - où : `web/v2/ui/src/components/TreeGrid/TreeGrid.vue` (imiter la colonne d'élément de
    `DataGrid/DataGrid.vue`, `elementProps`), `TreeGrid.catalogue.ts` ;
    `web/v2/shell/src/views/TopologyView.vue`, `web/v2/shell/src/plugins/slots.ts`
    (`gridColumns`) ; plugin de test `web/v2/e2e/plugins/notes` (une colonne sur `prefix`) ;
    `e2e/journeys/plugins.spec.ts` ; inventaire de `specs/020-systeme-de-design/design.md`
    et de la skill `kartenn-ui`
  - dépend de : 11
  - fait le 2026-11-14 — `TreeGrid.vue` prend les colonnes d'élément (`element`,
    `elementProps`) par `ExtensionCell`, sortie de `DataGrid.vue` dans
    `internal/extensionCell.ts` avec le sélecteur des contrôles (`CELL_WIDGETS`) que les
    deux grilles partagent ; cellule jamais éditée ni dite sans éditeur, ses contrôles hors
    tabulation (au rendu et, seulement s'il existe une colonne d'élément, par un
    `MutationObserver` comme la `DataGrid`), atteints par Entrée ou F2, Échap et F2 au
    retour ; l'élément clé par sa ligne : un élément de ligne recyclé (22e) le recrée pour le
    nœud qu'il vient montrer, sans rien changer au recyclage des lignes. État de catalogue
    `extension`. `TopologyView.vue` met `gridColumns(useExtensions("column", "prefix"))`
    après les colonnes du cœur, `elementProps` donnant l'`object` et le `row` (le nœud tel
    que sa page le sert) aux seules lignes de préfixe lues ; plugin de test `notes` : sa
    colonne aussi sur `prefix`. Vitest `ui/test/trees.test.ts` (« an extension column… »,
    « …virtualised… »), `ui/test/trees-mounted.test.ts` (« gives each row its own element,
    created again when a recycled row shows another node », en échec sans la clé) ; script
    `e2e/components/tree-grid.spec.ts` (« an extension column… ») ; parcours `plugins`
    (« the topography places the column of the prefixes… »). `k-web2-check --base main` :
    1074 tests, ciblé 2323 captures sur 2738 ; captures `TreeGrid-extension-*` ajoutées,
    `TreeGrid-*-confortable` réécrites (un état de plus).
  - choix : `elementProps` peut rendre `undefined` (aucun élément sur la ligne : un nœud
    d'un autre type, une ligne d'aperçu), dans le `TreeGrid` et, pour un même contrat, dans
    la `DataGrid` (dont l'hôte, `ListView`, rend toujours des propriétés) ; F2 depuis un
    contrôle que la cellule ne liste pas (racine fantôme) revient à la cellule ; `row` vaut
    le nœud par défaut, l'hôte en donne un autre par `elementProps` ; la cellule d'une extension héberge
    l'élément brut comme la `DataGrid` (la bibliothèque ignore les plugins), pas
    l'`ExtensionElement` du shell : un élément qui lève est rapporté par le navigateur et
    laisse la cellule vide, sans encart ; clip de la valeur à marge (`overflow-clip-margin`)
    pour l'anneau de focus d'un contrôle de l'élément, comme la `DataGrid` ; colonne de
    priorité 3 comme dans les listes (`gridColumns`) : masquée dans l'arbre plus étroit que
    `lg`, à un choix de « Colonnes ».
  - reste : pas d'encart d'erreur par cellule pour l'élément d'une colonne (ni dans la
    `DataGrid`) ; un contrôle en racine fantôme n'est ni sorti de la tabulation ni listé
    (ADR 0038 § 4 : DOM léger) ; anneau de focus d'un contrôle de cellule jamais capturé ;
    effet sur le défilement à mesurer par la passe `bench-web` (élément recréé par ligne
    recyclée, seulement avec une colonne d'extension).
- [x] 11o2. Erreur d'un plugin confinée à sa cellule (relevé de la revue de la 11o, le
  2026-11-14) : l'élément d'une colonne d'extension qui lève laisse aujourd'hui une
  cellule vide et muette, dans le `TreeGrid` comme dans la `DataGrid`, alors que la même
  extension en onglet de fiche montre son encart ; les deux grilles offrent une fente de
  cellule où le shell pose son `ExtensionElement` (encart avec le code `plugin.element`,
  `status`, liens de l'élément suivis par le routeur), la bibliothèque continuant
  d'ignorer les plugins ; `tame`, `onMutations` et `observeExtensions`, recopiés d'une
  grille à l'autre, sortis dans `internal/extensionCell.ts` ; `objectRef` stable d'un
  rendu à l'autre (`.object` n'est plus réaffecté à chaque déplacement du focus) —
  REQ-021-13 — verify : Vitest des deux grilles (un élément qui lève à la construction
  et à `connectedCallback` : encart et code dans sa cellule, les autres cellules et
  lignes intactes ; `elementProps` rendant `undefined` dans la `DataGrid` ; ligne
  recyclée : l'encart ne suit pas la ligne) ; parcours `plugins` (une colonne de `notes`
  qui lève, sur une liste et sur la Topographie) ; `k-web2-check --base main`
  - où : `web/v2/ui/src/components/internal/extensionCell.ts`, `TreeGrid/TreeGrid.vue`,
    `DataGrid/DataGrid.vue` et `DataGrid/grid.ts` (doc de `GridColumn.element`) ;
    `web/v2/shell/src/plugins/ExtensionElement.vue` et `slots.ts` (`gridColumns`),
    `views/ListView.vue`, `views/TopologyView.vue` (`elementProps`, `objectRef`) ;
    plugin de test `web/v2/e2e/plugins/notes`
  - dépend de : 11o ; avant la bascule (27) ; le recyclage des lignes de la 22e ne doit
    rien y perdre (passe réduite `bench-web` par la session principale)
  - fait le 2026-11-14 — emplacement `extension` des deux grilles (`ExtensionSlotProps` :
    la ligne telle que l'élément la recevrait, la colonne, les autres propriétés de
    `elementProps`), passé par `ExtensionCell` (son emplacement par défaut, sinon l'élément
    brut) : aucun appel sur une ligne que `elementProps` écarte, la clé de ligne du `TreeGrid`
    inchangée (un contenu recréé sur une ligne recyclée), rien de plus sans colonne
    d'extension ; `tameControl` et `useExtensionControls` (l'observateur) sortis dans
    `internal/extensionCell.ts`. Shell : `ExtensionElement` prend `cell` (élément en ligne ;
    erreur dite courte, « Extension en erreur » et `CodeBadge` `plugin.element`, comme
    `NetworkVlan`), `cellExtension` (slots.ts) lui donne balise, plugin, libellé, ligne et
    `object` ; `ListView.vue` et `TopologyView.vue` remplissent l'emplacement ;
    `stableObjectRefs` : un même `object` par ligne d'un rendu à l'autre, tenu par la ligne
    (`WeakMap`, revue : aucune référence gardée pour une ligne que l'écran ne tient plus).
    Plugin de test `notes` : sa colonne `flag` lève à la connexion dans la ligne qu'un
    parcours nomme (`localStorage`). Vitest `shell/test/plugin-cells.test.ts` (élément qui
    lève à la construction, à la connexion, par un setter `object` ou `row`, dans une liste
    et dans l'arbre ; `elementProps` sans rien dans la `DataGrid` ; ligne recyclée de
    l'arbre, ligne de la liste qui montre un autre objet ; même élément et même `object`
    d'un rendu à l'autre ; référence libérée avec sa ligne), `ui/test/components-data.test.ts` et
    `ui/test/trees.test.ts` (« …slot `extension`… »), `ui/test/trees-mounted.test.ts`
    (contenu recréé sur une ligne recyclée) ; parcours `plugins` (« a column of a plugin
    that throws in a row… », liste des préfixes et Topographie).
  - choix : erreur dans une cellule sans région vivante (un plugin qui lève lève sur toutes
    les lignes ; l'en-tête de colonne nomme l'extension), le motif long réservé aux encarts
    (onglet, vue), sur une ligne, le code rétréci le premier (sa bulle le dit entier) ;
    `status` non passé dans une cellule (les colonnes ne viennent que des plugins chargés,
    `extensionsOf`), un plugin en chargement y aurait un squelette de cellule ; l'élément d'une cellule est recréé quand sa ligne
    change d'objet (`ExtensionElement`, comme un onglet), donc aussi quand `shownRows` de la
    liste recopie ses lignes (colonnes de propriétés choisies, saisie en place) ; prop
    d'emplacement par l'emplacement par défaut d'`ExtensionCell` (typé, inféré par vue-tsc),
    pas une prop `slot` ; pas d'état de catalogue nouveau (l'emplacement est celui de l'hôte,
    l'état `extension` montre toujours l'élément brut).
  - reste : à mesurer par la passe `bench-web` (session principale) : avec une colonne
    d'extension, un composant `ExtensionElement` par cellule (montage, observateurs, un
    élément ajouté après le rendu) au lieu d'un nœud brut, recréé à chaque ligne recyclée ou
    qui entre dans la fenêtre ; rien sans colonne d'extension. Relectures (mineurs) : une
    annonce polie, une fois par grille, quand une cellule d'extension tombe en erreur après
    le chargement (4.1.3, l'événement `failed` existe) ; hauteur de la ligne en erreur
    jamais capturée ; une cellule en erreur a deux racines (`ExtensionElement` : la phrase
    et le conteneur caché par `v-show`) ; `status` non passé à l'emplacement (voir `choix`).
- [x] 11p. Manques du contrat du référentiel (reportés de la 11, cochée le 2026-11-13),
  **arbitrés le 2026-11-13 (Q-021-23 à 28)** : source par valeur et `object` des lacunes
  au contrat ; motif et auteur dans `Version` (auteur par référence, pseudonymisable) ;
  ordre de la clé seul ; comptes des onglets retirés ; `carrier` et `q` des déclarations ;
  REQ-021-18 amendée. À découper en sous-tâches serveur (contrat 006) puis shell avant de
  lancer. Texte d'origine : (1) fiche, colonne « Source » des propriétés et encart
  « propriété attendue manque » (maquette E2) : `Resolved` ne sert pas la source d'une
  valeur, `GET /v1/property-gaps` n'a pas de filtre `object` (seulement `domain`,
  `entity`) — ajout au contrat (source par valeur, `object` comme A37 pour les anomalies,
  recommandé) ou lectures côté client (explication par ligne, lacunes du domaine triées,
  plafonnées) ; (2) motif et auteur de l'historique : `Version.changeset` ne porte que
  `id` et `href` — les servir dans `Version` (recommandé) ou lire chaque changeset d'une
  page ; (3) tri des listes : aucun paramètre au contrat — un `sort` déclaré (index,
  curseur, banc) ou l'ordre de la clé seul et la maquette amendée (recommandé tant
  qu'aucun besoin ne le demande) ; (4) comptes des onglets de la fiche (maquette E2) :
  aucun total servi — compter ce qui est lu (propriétés, anomalies de l'objet), un total
  au contrat, ou les retirer de la maquette ; (5) déclarations sans filtre par porteur ni
  recherche par clé (`GET /v1/property-declarations`, dix pages lues au plus pour typer
  un filtre) — les ajouter ou garder le plafond ; (6) acceptation de REQ-021-18 :
  « Adresses filtrées par `site = <site>` » est impossible, `core.site` n'étant porté que
  par les préfixes et non hérité (REQ-002-20, `core_declarations`), et tout filtre de
  propriété demande un domaine (`kartenn-rest`, `filter::parse`) — l'amender en
  « Préfixes d'un domaine filtrés par `core.site` et `core.owner` » (recommandé, prouvé par
  le parcours `records`) ou rendre `core.site` hérité — verify : décisions consignées dans
  `decisions.md` de la 021, REQ-021-18 amendée, sous-tâches écrites avec `où :` et `verify`
  - où : tâche 11 (« reste » et relevé du 2026-11-13) ; contrat 006
    (`crates/kartenn-rest/src/properties.rs`, `versions.rs`, `anomalies.rs`) ;
    `web/v2/shell/src/views/record/{PropertiesTab,HistoryTab}.vue`, `RecordView.vue`,
    `views/data.ts` (`useDeclarations`)
  - dépend de : 11
  - fait le 2026-11-13 — arbitrages consignés (`decisions.md`, Q-021-23 à 28), REQ-021-18
    amendée (87f0b795) ; découpée en 11p1 à 11p3 (serveur, contrat 006, ajouts seuls)
    puis 11p4 à 11p7 (shell), ci-dessous.
- [x] 11p1. Source par valeur et lacunes d'un objet au contrat (Q-021-23, ajout seul) :
  `Resolved` (`GET /v1/<collection>/{id}/properties`) sert `sources`, par clé de
  `properties`, `{origin, inherited, provenance}` : `origin` le nœud `{kind, id, href}` de
  l'objet qui porte l'assignation, `null` s'il n'est pas lisible (son existence n'est pas
  divulguée, REQ-005-19) ; `inherited` vrai quand ce porteur est un autre objet ;
  `provenance` comme l'explication (`input`, `source:<id>@<version>`, `module:…`) ; une
  clé de `masked` n'a pas de source (REQ-005-21) ; `GET /v1/property-gaps` accepte
  `object=<uuid>` (au plus une fois, UUID, sinon 400), passé au filtre SQL des anomalies
  comme `/v1/anomalies` (021/A37) et cité dans l'audit de la lecture ; contrat et SDK
  régénérés, design de la 006 suivi — REQ-021-9, REQ-003-14, REQ-005-19, REQ-005-21 —
  verify : `k-check -p kartenn-rest` (it `objects` : source directe, héritée d'un ancêtre
  lisible, d'un ancêtre illisible `origin: null`, clé masquée sans source ; lacunes d'un
  seul objet, `object` invalide ou répété → 400) ; test du contrat (ajout seul) ;
  `k-check` complet
  - où : `crates/kartenn-store-pg/src/read.rs` (`properties_sql`, `object_in` : porteur
    et provenance) ; `crates/kartenn-service/src/port.rs` (`ObjectRead`), `reads.rs`
    (`shown_object` : la source d'une clé masquée retirée) ;
    `crates/kartenn-rest/src/properties.rs` (`Resolved`, `resolved`, `visible_nodes`,
    `gap_filter`, `gaps`), imiter `anomalies.rs` (`filter`, paramètre `object`) ; tests
    `crates/kartenn-rest/tests/it/objects.rs` (`properties_resolve_explain_and_mask`,
    `a_deviation_of_a_sensitive_property_hides_its_value`) ;
    `specs/006-api-rest/openapi-v1.json` (`cargo run -p kartenn-server --all-features --
    openapi`), `node web/sdk/scripts/gen.mjs`, `node web/v2/sdk/scripts/gen.mjs` ;
    `specs/006-api-rest/design.md` (valeurs résolues, écarts)
  - dépend de : 11p
  - fait le 2026-11-13 — `read::properties_sql` rend le porteur et la provenance de
    chaque valeur (`ValueSource` de `ObjectRead.sources`, retirée avec la valeur masquée
    dans `shown_object`) ; `properties::resolved` nomme les porteurs par
    `visible_nodes` (schéma `ValueSource`) ; `GapFilter` prend `object`, passé à
    `AnomalyFilter` et à l'audit de la lecture ; contrat (ajouts : `Resolved.sources`,
    `ValueSource`, paramètre `object`) et SDK régénérés ; design de la 006 suivi. Tests :
    it `objects::properties_resolve_explain_and_mask` (source directe, héritée d'un
    ancêtre lisible, d'un ancêtre illisible, clé masquée sans source, origines nommées
    avec tous les droits), `objects::the_deviations_of_one_object_read_by_its_filter`
    (objet seul, avec `property`, par pages, illisible comme inconnu, 400) ; unitaire
    `the_gaps_filter_takes_a_property_a_domain_an_entity_and_an_object`. Revue : le nœud
    `{kind, id, href}` a son propre schéma, `NodeRef` (il se confondait avec le `Node`
    d'une entité ou d'un site : `Explanation.origin`, `chain`, `replaces.origin` étaient
    typés faux), les transtypages du shell retirés (`ExplanationBody.vue`,
    `edit/properties.ts`) ; `origin`, `provenance` (et `replaces`) requis, servis à `null`
    au besoin, dans `ValueSource`, `Explanation` et `Replaced` ; testés en plus : `object`
    dans les critères d'audit des lacunes, provenance `source:` d'un porteur illisible
    servie sans le nommer.
  - choix : `inherited` servi plutôt que déduit de `origin` (un porteur illisible reste
    dit hérité) ; une clé masquée sans source du tout, plutôt qu'une origine sans valeur
    (la provenance `source:` pourrait trahir la valeur) ; l'objet lui-même nommé sans
    lecture de droits (il vient d'être lu sous les siens).
- [x] 11p2. Motif et auteur dans `Version` (Q-021-24, ajout seul) : chaque version servie
  (`GET /v1/<collection>/{id}/versions` et partout où le schéma `Version` est rendu)
  porte le motif et l'auteur de son changeset ; l'auteur est une référence d'identité
  résolue à la lecture (jointure sur la table des identités, sous la forme déjà servie
  pour une identité ailleurs dans `/v1`, à défaut `{id, name}`), jamais recopiée dans la
  version, un cache ou une table de lecture : une personne pseudonymisée (REQ-004-33) se
  lit « personne #… » dans tout l'historique dès sa pseudonymisation. La pseudonymisation
  automatique au terme d'une durée de conservation (RGPD) n'est pas de cette tâche : elle
  attend sa spec (Q-021-24) — REQ-021-9, REQ-004-18, REQ-004-33 — verify :
  `k-check -p kartenn-rest` (it : historique d'un objet avec motif et nom de l'auteur ;
  après `pseudonymise`, le même historique ne contient plus ni le nom ni le courriel) ;
  contrat (ajout seul) et SDK régénérés ; `k-check` complet
  - où : `crates/kartenn-rest/src/objects.rs` (`VersionDto`, `version_of`),
    `changesets.rs` (`ChangesetRef`, `summary`) ; `crates/kartenn-store-pg/src/list.rs`
    (`Version`, sa requête), `identities.rs`, `audit.rs` (`pseudonymise_identity`) ;
    imiter le rendu d'une identité de `crates/kartenn-rest/src/audit.rs` ; test voisin de
    l'historique dans `crates/kartenn-rest/tests/it/objects.rs` ; contrat et SDK comme 11p1
  - dépend de : 11p
  - fait le 2026-11-13 — `list::versions_page` joint `changeset` et `person` à chaque
    lecture (`Version.motive`, `Version.author`, `list::Author`) ; `VersionDto` sert
    `motive` et `author` (`IdentityRef {id, href, name}`, `rights.rs`), l'un et l'autre
    à qui lit l'objet (Q-021-29, 30, arbitrées après une première version qui gardait
    le nom sous `core.rbac.read`) ; l'historique jamais `immutable`, même à un `as_of`
    du passé figé (`Moment::live`, `dated.rs`), ses auteurs étant lus au présent ;
    contrat, SDK et design de la 006 (« Historique », REQ-006-6 amendée) mis à jour ; it
    `history_names_the_motive_and_the_author_until_pseudonymised` (motif et nom pour un
    lecteur du seul objet ; à un `as_of` figé, `private, no-cache` ; puis
    `POST …/pseudonymisation` : ni nom ni courriel, « personne #… », au présent comme à
    l'`as_of` figé, ETag changé, pas de 304).
  - choix : `IdentityRef` à trois membres plutôt qu'`IdentityItem` (son état — émetteur,
    désactivation, révocation — relève de `core.rbac.read`) ; `name` jamais `null` : une
    personne pseudonymisée se lit « personne #… » comme dans l'audit (REQ-004-33), et un
    auteur absent du journal ne peut se servir ; jointure externe, et une version dont le
    changeset manque au journal répond 500 (journalisé) plutôt que d'être tue ; les
    fixtures des tests d'historique journalisent leurs changesets (`journal`) ;
    `ModuleVersion` (objets des modules, VLAN) reste sans motif ni auteur (→ 11p8).
  - reste : une version dont le changeset manque au journal fait tomber toute la page en
    500 ; les tables d'objets du cœur n'ont pas de clé étrangère vers `changeset`
    (`0004_core_objects.sql`), l'invariant n'est tenu que par le code ; `journal()` des
    tests est codé en dur sur l'auteur système et les domaines.
- [x] 11p8. Motif et auteur dans `ModuleVersion` (Q-021-24, 29, 30, ajout seul) :
  l'historique d'un objet de module et d'un VLAN ou groupe de VLAN
  (`GET /v1/<collection>/{id}/history` des modules, route transversale des VLAN) sert
  `motive` et `author` (`IdentityRef`) comme `Version`, déjà lus par `list::versions_page`
  (`module_version_of` les ignore), jamais `immutable` (`Moment::live`) — REQ-004-18,
  REQ-004-33 — verify : `k-check -p kartenn-rest` (it : historique d'un VLAN et d'un
  objet de module avec motif et nom de l'auteur, puis après pseudonymisation
  « personne #… » ; `private, no-cache` à un `as_of` figé) ; contrat (ajout seul) et SDK
  régénérés ; `k-check` complet
  - où : `crates/kartenn-rest/src/modules.rs` (`ModuleVersionDto`, `module_version_of`,
    sa réponse) ; imiter `VersionDto`, `version_of` et la réponse de l'historique de
    `crates/kartenn-rest/src/objects.rs` ; tests voisins `tests/it/modules.rs`,
    `tests/it/vlan/rest.rs` (historique d'un groupe), fixtures journalisées comme
    `journal()` de `tests/it/objects.rs` ; contrat et SDK comme 11p1
  - dépend de : 11p2
  - fait le 2026-11-14 — `ModuleVersionDto` sert `motive` et `author` (`IdentityRef`),
    `module_version_of` faillible comme `version_of` ; l'historique des modules et des
    VLAN répond `moment.live()` ; contrat (ajout seul), SDK et design de la 006
    (« Historique ») mis à jour ; it `history_of_a_module_object_names_the_motive_and_the_author`
    (`tests/it/modules.rs`) et `history_of_a_vlan_names_the_motive_and_the_author`
    (`tests/it/vlan/rest.rs`, VLAN et groupe) : motif et nom pour un lecteur du seul objet,
    `private, no-cache` à un `as_of` figé, puis après pseudonymisation « personne #… »,
    au présent comme au passé figé, ETag changé.
  - choix : la vérification du journal de `version_of` extraite en
    `objects::journaled`, partagée par les deux rendus (même 500 journalisé
    `http.history.unjournaled`) ; les fixtures de `tests/it/modules.rs` journalisent leur
    changeset (`changeset`, `thing_by`, l'auteur système pour `thing`) ; `app_bounded`
    ajouté aux deux fichiers de test pour lire au passé figé sans attendre la borne.
- [x] 11p3. Déclarations filtrables (Q-021-27, ajout seul) : `GET /v1/property-declarations`
  accepte `carrier=<sorte>` (déclarations dont `carriers` contient la sorte du cœur ou le
  type qualifié d'un module) et `q=` (préfixe de la clé ou d'un libellé, sans casse, en
  toute langue, comme les autres `q` du contrat, Q-018-8 ; libellés du dictionnaire de la
  018), chacun au plus une fois, le
  curseur gardant le filtre ; contrat et SDK régénérés, design de la 006 suivi —
  REQ-021-18, REQ-003-* (schéma) — verify : `k-check -p kartenn-rest` (it : filtre par
  porteur, par clé, par libellé, combinés, page suivante sous le même filtre, paramètre
  inconnu ou répété → 400) ; contrat (ajout seul) ; `k-check` complet
  - où : `crates/kartenn-rest/src/properties.rs` (`declarations`, `page.unfiltered()` à
    remplacer par ses filtres), `crates/kartenn-store-pg/src/property_read.rs`
    (`declarations`), `crates/kartenn-rest/src/labels.rs` ; test
    `properties_resolve_explain_and_mask` (section « The schema ») ; contrat et SDK comme
    11p1
  - dépend de : 11p
  - choix : écart au texte d'origine (« sous-chaîne », « langue négociée »), tranché par
    la session principale : `q` est un préfixe de la clé ou d'un libellé d'une langue
    quelconque, par cohérence avec Q-018-8 et les autres `q` du contrat (`…/{key}/values`,
    `filter::prefix`, `like_prefix`), et la page passe à 20 éléments avec lui
    (`suggesting`) ; le dictionnaire des propriétés étant les `labels` de la déclaration
    (`labels_read::stored`), le store les lit sans arbitrage, et sans négociation de
    langue (ADR 0040 : `/v1/labels` seule route qui varie selon la langue). Limite connue,
    pour la 11p7 : par la clé seule, « site » ne trouve pas `core.site` (il faut taper
    `core.s…`) ; seul un libellé commençant par « Site » le fait sortir. `carrier` accepte une sorte de `CORE_CARRIERS` ou un nom
    qu'`is_module_kind` reconnaît (jamais `object_kind`, qui interne chaque nom reçu) ;
    `core.prefix` ou une majuscule → 400, un type de module bien formé que rien ne porte
    → liste vide. Une déclaration sans porteurs n'existe pas (`CHECK
    (cardinality(carriers) > 0)`, `prop.declaration.invalid`) : le filtre est
    `$carrier = ANY(carriers)`, sans cas « toutes sortes ». Le filtre vit dans
    `DeclarationFilter` du store ; test `declarations_filter_by_carrier_and_prefix` à côté
    de `properties_resolve_explain_and_mask`, dont la section « The schema » est inchangée.
- [x] 11p4. Fiche, colonne « Source » et encart des lacunes (maquette E2) : `PropertiesTab`
  lit `sources` (directe, héritée de l'origine nommée par sa référence, héritée d'un
  ancêtre illisible, provenance `source:`/`module:` dite selon la maquette, masquée) et,
  au-dessus de la grille, l'encart « propriété attendue manque » par
  `GET /v1/property-gaps?object=<id>` — REQ-021-9, REQ-021-20 — verify : Vitest des vues
  (source de chaque sorte, encart vide ou rempli), parcours `records` (fiche d'un préfixe
  héritant d'un ancêtre), `k-web2-check --base main`
  - où : `web/v2/shell/src/views/record/PropertiesTab.vue`, `provenance.ts`,
    `ExplanationBody.vue` (libellés de provenance à réutiliser), `views/data.ts` ; maquette
    `web/v2/shell/src/_ui/ecrans/E2Fiche.vue` (`SOURCE`) ; `views/anomalies/GapList.vue`
    (rendu d'une lacune) ; `web/v2/e2e/journeys/records.spec.ts`
  - dépend de : 11p1
  - fait le 2026-11-13 — `PropertiesTab` : colonne « Source » (`ValueSource.vue`, lue de
    `sources` par `provenance.ts`, `valueOriginOf` ; `provenanceOf` lit la source et la
    révision d'un `source:`) : « Directe », « Héritée de » l'origine nommée et liée
    (`ObjectName`), « Héritée d'un objet que vous ne lisez pas », « Inférée de la source »
    avec sa révision, les mots de l'explication pour `module:` et `label:`, « Droit de
    lecture manquant » pour une clé masquée ; au-dessus de la grille, `GapsNotice.vue`
    (`GET /v1/property-gaps?object=`, au présent seulement) : chaque écart composé de ses
    membres (`anomalies/gap.ts`, partagé avec `GapList`), erreur avec ses codes et
    « Réessayer », page suivante à la demande ; grille empilée sous un conteneur étroit
    (`stack`, reste de la 11p6 : Source lisible à 390 px). Maquette E2 alignée (encart
    au-dessus, mêmes mots, `stack`), amendement daté dans `decisions.md` de la 020,
    catalogue des écrans de la 021 suivi ; captures E2 réécrites et regardées. Tests :
    Vitest `properties-source.test.ts` (chaque sorte de source, encart vide, rempli, en
    erreur puis réessayé), `explanation.test.ts` (provenance `source:`), fake de
    `properties-edit.test.ts` avec `sources` ; parcours `records` (fiche d'un préfixe :
    `core.owner` hérité du domaine nommé dans la colonne, lacunes lues, aucun encart).
  - choix : la bulle d'explication classe la valeur comme la colonne
    (`explanationOriginOf`, mêmes règles que `valueOriginOf`) et la dit avec les mêmes mots
    (`ValueSource`) : le défaut d'une source posé sur l'objet est « Inférée de la source »
    dans les deux (la 13d le disait « posée directement » dans la bulle), un porteur
    illisible « Héritée d'un objet que vous ne lisez pas » dans les deux (la bulle disait
    « Inférée par la règle » ou « Aucune valeur ») ; pour une valeur héritée, nommée ou
    non, la cellule ne dit que l'origine et la bulle ajoute la provenance d'une source ou
    d'un module ; « Aucune valeur posée ni héritée » seulement sans origine, ni
    provenance, ni valeur, ni objet illisible sur le chemin ; encart absent pendant le chargement (pas de squelette pour une zone le plus
    souvent vide) et hors du présent (les lacunes sont celles du présent, la lecture n'a
    pas de dates) ; source d'un écart en chasse fixe et règle en `CodeBadge` nu, plutôt
    qu'en `KeyLabel` copiable comme `GapList` : deux cibles copiables sur une ligne
    repliée tombaient l'une sous l'autre (axe `target-size`, 1280 px) ; valeur en `md`
    et Source en `lg` à la ligne, comme la maquette. Restent ouverts (relecture
    d'accessibilité, hors de la tâche, bibliothèque) : le bouton de copie de `KeyLabel`
    n'a pas de verbe dans son nom (« TTL DNS dns.ttl » quand `CodeBadge` dit « Copier… »),
    et les régions `status` de copie de `KeyLabel` et `CodeBadge` sont imbriquées dans le
    `status` d'une `InlineAlert`.
  - reste : non testés, « Lire la suite » de l'encart, l'échec d'une page suivante
    (`NextPageAlert`) et l'absence d'encart hors du présent ; après une modification en
    place, `reread` ne relit que `resolved` : l'encart peut garder un écart corrigé
    jusqu'à une autre invalidation de `/v1/property-gaps` (à vérifier, l'évaluation des
    écarts côté serveur pouvant d'ailleurs suivre la validation).
- [x] 11p5. Historique, motif et auteur : `HistoryTab` montre le motif et l'auteur servis
  par `Version` (auteur pseudonymisé tel quel, sans repli sur un nom du cache) —
  REQ-021-9 — verify : Vitest (motif, auteur, auteur pseudonymisé), parcours `records`
  (historique d'un objet modifié), `k-web2-check --base main`
  - où : `web/v2/shell/src/views/record/HistoryTab.vue` ; maquette `E2Fiche.vue`
    (`Version`, colonnes `motive`, `author`)
  - dépend de : 11p2
  - fait le 2026-11-14 — `HistoryTab.vue` : colonnes Valeur, Valide, Connue du
    (priorité 3), Motif (`wrap`, lien vers le changeset ; « Changeset » et son
    identifiant en chasse fixe si le motif est vide), Auteur (`Version.author.name` tel que servi, priorité 2), Changeset (priorité
    3) ; grille `stack` (reste de la 11p6 : à 768 px, plus que Version et Valide) ; SDK :
    l'historique n'est jamais figé, même à un `as_of` du passé (`isRevalidated` dans
    `keys.ts`, exclu d'`isImmutable` ; `freshness` à `staleTime` 0, relu en
    `If-None-Match` à chaque lecture), d'accord avec le `no-cache` du serveur ; aucun nom
    gardé côté client hors du cache en mémoire. Maquette E2 alignée (même grille, auteur
    pseudonymisé en exemple), amendement daté dans `decisions.md` de la 020, design de la
    021 (« Catalogue des écrans » : Historique ; « Clés et fraîcheur » : ligne de
    l'historique) ; captures E2 historique réécrites et regardées. Tests : Vitest
    `history.test.ts` (motif, lien, auteur ; auteur pseudonymisé relu au remontage, sans
    nom dans le stockage du navigateur ; motif vide), `sdk/test/cache.test.ts`
    (historique au passé relu, `staleTime` 0, enfants au passé toujours figés) ; parcours
    `records` (réseau renuméroté : trois versions, motifs liés, auteur nommé).
  - choix : le motif porte le lien vers le changeset (nom du lien = ce qu'il dit du
    changeset), l'identifiant du changeset passe en colonne de priorité 3, la maquette
    suivant (« Version » seule en tête plutôt que « Version, changeset ») ; auteur en
    texte, sans lien (la fiche d'une identité relève de `core.rbac.read`, Q-021-29) ;
    l'historique relu à chaque lecture à toute date (`staleTime` 0) plutôt qu'à 30 s ;
    la première colonne de l'écran, qui montre la valeur de l'objet (l'API ne numérote
    pas les versions), titrée « Valeur » et non « Version » (relecture UI) ; deux motifs
    identiques gardent deux liens de même nom, la ligne les distinguant (relecture a11y).
  - reste : à 1024 px le conteneur de la fiche (≈ 465 px utiles, entre `sm` et `md`) est
    trop étroit pour les trois colonnes de priorité 1 (Version `md`, Valide `lg`, Motif
    `fill` au moins `md`) et pas assez pour s'empiler : le motif est coupé, la grille
    défile de côté (déjà le cas avant la tâche) ; à trancher dans la bibliothèque (seuil
    d'empilement par grille, ou minimum de `fill`). En `stack`, la bande d'en-tête
    répète les libellés des colonnes (bibliothèque). La maquette garde le filtre de
    « Motif », que l'écran n'a pas (l'API ne filtre pas l'historique). Pas de script
    clavier dédié au lien d'une cellule de l'historique (modèle de `DataGrid`). La
    maquette E2 numérote les versions (« v4 »…) sous « Version » quand l'écran montre la
    valeur sous « Valeur ». Dans la fiche, « Connue du » (second axe du bitemporel) est
    derrière « Colonnes » dès 1440 px (seuil `lg` du conteneur) : à revoir avec les seuils
    de la bibliothèque.
- [x] 11p6. Maquettes du référentiel amendées (Q-021-25, 26) : comptes des onglets de la
  fiche retirés (E2, `RecordView.vue` s'il en montre), tri des colonnes retiré des
  maquettes E1 à E3 et des vues du référentiel (ordre de la clé, `sortable` ôté) ; les
  autres écrans gardent leur tri jusqu'à leur propre tâche — REQ-021-9 — verify :
  `k-web2-check --base main` (captures E1 à E3 réécrites et regardées)
  - où : `web/v2/shell/src/_ui/ecrans/{E1Topographie,E2Fiche,E3Adresses}.vue`,
    `web/v2/shell/src/views/RecordView.vue`, `views/list.ts`, `propertyColumns.ts`
  - dépend de : 11p
  - fait le 2026-11-13 — `E1Topographie.vue` (table du nœud), `E2Fiche.vue` (propriétés,
    historique) et `E3Adresses.vue` sans colonne triable (`sortable`, `v-model:sort`
    ôtés ; filtres gardés) ; onglets d'E2 sans compte, cœur et plugins ; propriétés d'E2
    dans l'ordre de la clé, masquées comprises ; `RecordView.vue`, `list.ts`,
    `propertyColumns.ts` et les vues du référentiel n'avaient ni tri ni compte : inchangés ;
    amendements datés sous Q-021-25 et Q-021-26 (`decisions.md`), dans la trace des
    validations de la 020 (§ « Validations des maquettes ») et dans « Catalogue des
    écrans » (listes, fiche) ; captures E1 à E3 réécrites et regardées ;
    `k-web2-check --base main` vert.
  - choix : captures réécrites en `--update all` restreint à E1 à E3, car `changed`
    gardait des références sous la tolérance qui montraient encore la flèche (E1 à
    1280 px) ; les instantanés d'accessibilité réécrits par `changed` seulement (ceux d'E2,
    les autres inchangés). Reste ouvert (antérieur, relecture) : à 390 px la colonne
    Source d'E2 est coupée (« Masquée » illisible), et à 768 px l'historique d'E2 ne montre
    que « Version » et « Valide », le reste par défilement horizontal.
- [x] 11p7. Déclarations lues par filtre : `useDeclarations` lit
  `GET /v1/property-declarations?carrier=<sorte>` (filtres typés d'une liste) et `q=`
  (suggestions de clé), au lieu de dix pages au plus — REQ-021-18 — verify : Vitest des
  vues (une lecture filtrée, plus de plafond), parcours `records` : l'acceptation amendée
  de REQ-021-18 (Q-021-28), préfixes d'un domaine filtrés par `core.site` et
  `core.owner`, les déclarations lues par `carrier=prefix`, le lien copié rouvrant la même
  vue ; `k-web2-check --base main`
  - où : `web/v2/shell/src/views/data.ts` (`useDeclarations`), ses appelants
    (`list.ts`, `ListView.vue`, `PropertiesView.vue`, `anomalies/GapList.vue`,
    `record/PropertiesTab.vue`, `record/NetworkVlanTab.vue`)
  - dépend de : 11p3
  - fait le 2026-11-14 — `useDeclarations(carrier)` lit `carrier=<sorte>` (la sorte d'une
    collection du shell, `carrierOf`, `null` pour une liste sans sorte : rien lu), toutes
    les pages suivies et gardées (`maxPages: Infinity` sur cette lecture seule, le cache
    en garderait dix et lâcherait les premières), sans `placeholderData` (les déclarations
    d'une autre sorte ne tiennent jamais lieu) ; la liste, ses colonnes et l'onglet
    Propriétés partagent cette clé (`carriedRead`), que la route d'une liste précharge et
    que `prefetchFilters` lit. `useDeclarationsOf(keys)` lit une déclaration par sa clé
    (`q=<clé>`, la clé exacte gardée ; requête `["shell", "declaration", clé]` au-dessus de
    `cacheSuggest`) : lacunes (`GapList`, `GapsNotice`, qui ne reçoit plus les
    déclarations de l'onglet), `core.vlan` de l'onglet VLAN, valeurs héritées d'une autre
    sorte dans l'onglet Propriétés (demandées une fois celles de la sorte lues, pour les
    clés qui leur manquent). Suggestions du filtre `property` des lacunes par `q`
    (`keySuggest`). L'écran des propriétés déclarées lit le catalogue page à page à la
    fenêtre de la grille (`useNextPage`), `NextPageAlert` sur une page refusée, au lieu de
    l'encart « Liste incomplète » (clés `countPartial`, `partialTitle`, `partial` ôtées).
    Vitest des vues (lecture filtrée par `carrier=address`, douze pages suivies sans perte,
    clé inconnue après toutes les pages, suggestions `q`), parcours `records` (lectures
    des déclarations toutes en `?carrier=prefix`, dans les deux sessions).
  - choix : un texte sans point demande aussi `q=core.<texte>` (« site » → `core.site`),
    deux lectures fusionnées sans doublon ; une clé d'un autre espace (`ttl` pour
    `dns.ttl`) ne se trouve que par son espace ou un libellé. Le libellé d'une suggestion
    vient du dictionnaire (`property`), seul : il est fait des `labels` des déclarations
    (11p3), un repli sur les `labels` de l'item serait la même donnée (relecture). La
    requête par clé est hors des clés par chemin du SDK : un changeset ne l'invalide pas,
    elle se relit périmée après 30 s (`FRESHNESS.list`). La sorte se tire de `COLLECTIONS`
    du shell (le SDK n'exporte pas `typeOfCollection`, et `cache.ts`/l'index du SDK sont
    chargés sur toutes les pages). `:row-count` passé à la grille des propriétés une fois
    complète, comme les autres listes (relecture). Passe de revue : l'onglet Propriétés dit
    en encart (`InlineAlert`, codes, « Réessayer ») les déclarations non lues, de la sorte
    ou d'une clé héritée, les valeurs restant montrées ; Vitest de `useDeclarationsOf`
    (clé exacte seule gardée, clé absente, refus) et de cet encart ; fixtures des vues
    servant sous `carrier=address` ce que le serveur y sert (`dns.ttl`, et non
    `core.owner` à `carriers: []`).
  - reste :
    - à arbitrer : filtres et colonnes d'une liste sur une propriété héritée que la sorte
      ne porte pas (`core.owner` sur `/addresses` ou `/ranges`, porté par `domain`,
      `prefix` et les types de module) : le serveur filtre sur la valeur résolue, mais le
      shell ne l'a jamais offert, ni avant ni après la 11p7 (`propertyFields` du SDK,
      `sdk/src/data/filters.ts:55`, et `declaredFor`, `propertyColumns.ts:41`, ne gardent
      que les porteurs). Options : ajout au contrat (`carrier` étendu aux héritiers, ou
      `inherited=true`) ; arbres d'héritage connus du shell (lire aussi les sortes
      ancêtres, garder les déclarations héritables) ; clés nommées par l'URL seules,
      cherchées par `useDeclarationsOf` et acceptées si héritables (sans les proposer au
      sélecteur).
    - lecture groupée de plusieurs clés : une requête `q=` par clé distincte aujourd'hui
      (une quarantaine en parallèle sur une page de lacunes) ; il faudrait un paramètre au
      contrat, donc un arbitrage.
    - la requête par clé (`["shell", "declaration", clé]`) n'est pas invalidée par un
      changeset : relue périmée après 30 s.
    - `edit/properties.ts:524` offre un éditeur sans vérifier que la sorte porte la
      déclaration (une valeur héritée d'une propriété non portée).
- [x] 11p9. Parcours `records` instable sur main (hors numéro, inscrite le 2026-11-14) :
  « a filtered link copied… » (Chromium, `:304` puis `:317`) et « past ten pages… »
  (Firefox, « page 4 shown », `:211`) échouaient sur une machine calme, pas à chaque fois
  — établir produit ou test, preuves à l'appui (traces Playwright), corriger la cause —
  verify : Vitest des vues (la liste relue et refusée garde sa grille et ses lignes) ;
  parcours `records` : les deux tests vingt fois de suite sous les deux navigateurs
  (`--repeat-each 20`), le fichier entier trois fois (`--repeat-each 3`), verts ;
  `k-web2-check --base main`
  - où : `web/v2/e2e/journeys/records.spec.ts` (« past ten pages », `readOneMore`),
    `web/v2/e2e/kartenn.toml` (`[http.rate]`), `web/v2/shell/src/views/list.ts`
    (`useFilteredList` : `failed`, `refreshFailed`), `ListView.vue` et les vues sur
    `useFilteredList` (`AnomalyList`, `GapList`, `ProposalsView`, `ChangesetsView`,
    `ImportsView`, `AuditView`), `problems/NextPageAlert.vue` ; test voisin : « a next page
    refused » de `shell/test/views.test.ts`
  - fait le 2026-11-14 — trois causes, deux d'environnement ou de test, une du produit :
    - quota de lecture (test) : les deux workers de Playwright lisent sous la même identité
      (Alice) et partagent son quota de lecture (50/s, 200 d'un coup, REQ-006-29) ; « past
      ten pages » le dépense seul en quelques secondes (douze pages, une lecture de
      propriétés par ligne montrée). Traces : 429 `rate-limited` sur `GET /v1/addresses`
      et `property-declarations?carrier=address` dès la première lecture du lien filtré
      (état « Trop de requêtes » à la place des lignes, `:304`), sur la page suivante de
      « past ten pages » (« Expected > 7, received 7 »). Ce n'était ni la charge ni une
      course de la 11p7 : à a1d663bb, « past ten pages » (22f) n'existait pas. Correctif :
      `[http.rate] read` de l'instance des parcours à 1 000/s, 4 000 d'un coup ; les
      écritures gardent le leur (les parcours `moves` attendent `Retry-After`).
    - liste relue et refusée (produit) : `failed` valait aussi pour une relecture refusée
      (invalidation par un événement, données présentes) ; l'état d'erreur remplaçait la
      grille, qui revenait en haut une fois la relecture passée, les pages lues sous la
      vue hors de vue. C'est, d'après l'état qu'il a consigné (sa trace écrasée depuis),
      l'échec Firefox de la session principale (« pages 4 started 23 … top 0 … rows 0..26
      count 401 » : quatre pages chargées, la liste relue, la vue en haut). `failed` ne vaut plus qu'avant toute ligne lue ; `refreshFailed`
      (`isRefetchError`) montre au-dessus des lignes gardées `NextPageAlert` titré
      `problem.readAgain`, « Réessayer » relisant la liste (`refetch`), dans les sept vues
      sur `useFilteredList`. Test monté d'abord rouge (la grille disparaissait).
    - course du test : la garde « plus de défilement une fois une lecture partie » se
      jugeait dans le test (événements `request`), après coup ; sous Firefox, un second
      `toEnd` partait avant l'événement de la page 2 et s'exécutait après son rendu,
      lisant la page 3 sans défilement voulu (`expect(pages).toHaveLength(10)` reçu 11,
      trace : `toEnd` de 412 ms chevauchant l'arrivée de la page). Les lectures de la liste
      sont comptées dans la page (`fetch` enveloppé par `addInitScript`, avant que le shell
      garde le sien, `activityFetch`) et le défilement s'y juge dans le même tour
      (`toEndUnlessRead`) ; un premier défilement toujours, la liste relue entre-temps
      lisant la suite d'elle-même. Ce que le test prouve est inchangé : chaque page lue
      montrée, aucune lue sans défilement, la fin atteinte ; il vérifie en plus que le
      compte voit les lectures.
    Passages : les deux tests 80 sur 80 (20 × 2 tests × 2 navigateurs), le fichier entier
    66 sur 66 (3 passages × 11 tests × 2 navigateurs), `k-heavy` tenant `pg` et tous les
    créneaux pendant chaque série.
    Corrections après revue (même jour), chacune d'abord rouge : les lignes ne restent que
    sur un refus passager (429, 5xx, réseau : `passing`, `list.ts`) ; refusée pour de bon
    (401, 403, 404…), la relecture les retire pour l'état d'erreur, comme avant (test monté
    au 403) ; tant que la relecture est refusée, `useNextPage` ne lit aucune page suivante
    (`isRefetchError`, `prefetch/nextPage.ts`), qui effacerait l'erreur et l'alerte alors
    que les premières pages n'ont pas été relues (test de `prefetch.test.ts`).
  - choix : quota de lecture relevé dans la configuration des parcours plutôt qu'une
    identité par worker (tous les fichiers lisent sous Alice) ; `NextPageAlert` réutilisé
    avec un titre plutôt qu'un composant de plus ; le correctif de `failed` porté aux sept
    vues de `useFilteredList`, qui partageaient le défaut.
  - reste :
    - le défaut que le quota relevé cache : la 11p10.
    - mineurs de la revue : un curseur refusé sur une relecture (`invalid-cursor` hors de
      `fetchNextPage`) n'est pas traité comme celui d'une page suivante ; les deux avis
      (page suivante, relecture) ne se montrent jamais ensemble, le second masqué par le
      `v-else-if` ; le nom de `NextPageAlert`.
    - vu une fois sous `--repeat-each 3` (avant le correctif du quota) : toutes les
      connexions Firefox refusées pendant quatre minutes, `GET /v1/me` en 503 « too many
      requests in flight » (plafond de 20, `quota.rs:483`) ; non reproduit ensuite, journaux
      du serveur non gardés (`stdout: "ignore"`).
    - les lectures hors `useFilteredList` remplacent encore leurs lignes par l'état
      d'erreur sur une relecture refusée (relecture `ui-reviewer`) : `record/AnomaliesTab.vue:86`,
      `record/GapsNotice.vue:39`, `PropertiesView.vue:47`, `record/HistoryTab.vue:84`,
      `changesets/ChangesetView.vue:504`, `admin/reads.ts:28`, niveaux des arbres.
    - `NextPageAlert` (page suivante comme relecture, défaut antérieur, relecture
      `a11y-auditor`) : après un « Réessayer » réussi, l'alerte disparaît et le focus tombe
      sur `body` (WCAG 2.4.3) ; pendant la relecture, ni `aria-busy` ni bouton désactivé,
      et un second refus n'est pas réannoncé (4.1.3). Le nom `NextPageAlert` couvre mal la
      relecture (renommer si d'autres usages viennent).
- [ ] 11p10. Un seul utilisateur qui défile atteint le quota de lecture par défaut
  (REQ-006-29, 50/s, 200 d'un coup) — **arbitrage de l'utilisateur** : une longue liste
  défilée vite lit environ 50 fois par seconde, une lecture de propriétés par ligne montrée
  (relevé de la 11p9) ; au quota, l'écran montre « Trop de requêtes » (première lecture) ou
  l'avis de la page suivante. Les parcours ne le voient plus : la 11p9 a relevé le quota de
  lecture de leur instance (`web/v2/e2e/kartenn.toml`, `[http.rate] read`). Options : lecture
  des propriétés par lots (un paramètre au contrat, donc un arbitrage) ; réessai d'un 429
  après `Retry-After` dans le cache du SDK (`sdk/src/data/cache.ts:236`, aujourd'hui aucun
  4xx relu) — verify : un parcours au quota par défaut (une instance à `[http.rate]` par
  défaut, une liste de dix pages défilée jusqu'au bout sans état d'erreur ni avis)
  - où : `web/v2/sdk/src/data/cache.ts` (`retry`), les lectures
    `/v1/<sorte>/{id}/properties` par ligne d'une liste (`web/v2/shell/src/views/
    propertyColumns.ts`, `views/edit/`), `crates/kartenn-rest/src/quota.rs` (`Rates`),
    `web/v2/e2e/kartenn.toml`
  - dépend de : 11p9
- [x] 11p11. Parcours `vlan` et `tree` rouges sous Firefox sur main (hors numéro, inscrite
  le 2026-11-14) : `k-web2-e2e` complet à fb651314, machine calme, Chromium vert, sept
  échecs Firefox — `vlan.spec.ts:102` (description du VID vide au lieu de
  `core.vlan.vid_out_of_range`, `:194`), `tree.spec.ts:28` (rang 200 au plus, `:69`), et
  `vlan.spec.ts:273`, `tree.spec.ts:88, 116, 182, 226` dont la connexion n'aboutit pas
  (`loginIn`) — établir produit ou test pour chacun, preuves à l'appui (traces Playwright,
  journal du serveur), corriger la cause, test rouge d'abord pour un défaut du produit ;
  ni `retries`, ni délai allongé, ni plafond relevé — verify : `vlan.spec.ts` et
  `tree.spec.ts` dix passages verts de suite sous Firefox ; `k-web2-e2e` complet vert sous
  les deux navigateurs ; `k-web2-check --base main`
  - où : `web/v2/e2e/serve.sh` (sortie du serveur gardée), `web/v2/e2e/journeys.config.ts`,
    `e2e/fixtures/journeys.ts` (`loginIn`, `atRest`), `journeys/{vlan,tree}.spec.ts` ;
    `crates/kartenn-rest/src/quota.rs` (`shed`, plafond des requêtes en vol, REQ-006-30) ;
    suspects du jour : 22o-b (`fd7560b1`, `ui/src/components/DataGrid/internal/
    cellReasons.ts`), 22e-c (`f9ec0616`, `TreeGrid` : `factsOf`, `stopIn`), 11p9
    (`3259fc44`, `shell/src/prefetch/nextPage.ts`)
  - dépend de : 11p9
  - fait le 2026-11-14 — une cause du serveur pour les sept, aucune des suspectes du jour ;
    une course de test vue en route :
    - pool empoisonné (produit, `kartenn-store-pg`) : sqlx 0.9.0 oublie une transaction
      dont le `BEGIN` est lâché pendant qu'il attend la base (la garde de
      `PgTransactionManager::begin` ne prévoit le `ROLLBACK` qu'à une profondeur qu'elle
      n'a pas encore montée) ; le ping de la remise au pool envoie le `BEGIN`, et le pool
      prête une connexion dans une transaction. Une requête dont le client s'en va (hyper
      lâche le futur de son gestionnaire) suffit. Preuves, passage complet relancé à
      a1e2ca10 avec le journal du serveur gardé : à 11:08:21 `audit.write.failed` « cannot
      execute INSERT in a read-only transaction » (une connexion restée dans la
      transaction `REPEATABLE READ, READ ONLY` de `read::consistent`), puis 1 600 refus
      « current transaction is aborted » (`http.store.failed`, `events.reader.failed`,
      `job.*`, `authn.unavailable`) jusqu'à la fin ; `pg_stat_activity` : une session
      `idle in transaction (aborted)` dont la dernière requête est `BEGIN` ; 52 échecs des
      deux navigateurs à mi-course (connexions, lectures en 503). Le 1 (refus du VID rendu
      en 503 au lieu de 422) et le 2 (page suivante refusée) en sont des suites : non revus
      en 140 passages après le correctif. Correctif : `transaction::{begin, begin_with}`
      (`kartenn-store-pg/src/transaction.rs`), le `BEGIN` dans une tâche à part qu'un
      appelant lâché n'annule pas ; tous les `Pool::begin` du crate et celui de
      `kartenn-kube/src/rest.rs` y passent. Test d'abord rouge
      (`tests/it/transactions.rs`) : `read::consistent` et `Pool::begin` lâchés après trois
      sondages laissent leur connexion dans la transaction (`now()` figé entre deux
      requêtes).
    - contraste lu en plein fondu (test, Firefox, 2 sur 70 de `vlan.spec.ts:102`, une fois
      `machines.spec.ts:309`) : « Appliquer » du brouillon, désactivé tant que la simulation
      n'est pas venue (demandée une demi-seconde après un chargement, après les plugins),
      s'activait pendant qu'axe lisait ; contraste 1,2 et 1,34 lus à mi-chemin des 80 ms
      du fondu (`.k-button`, `transition`). Instrumentation dans la page (fetch, attributs
      du bouton, `transitionrun`) : la bascule et le fondu après la mise au repos de
      `expectAccessible`. Correctif : `expectAccessible` coupe les transitions le temps
      qu'axe lise (feuille construite, que la CSP admet, retirée après) ; un état qui
      change alors prend ses couleurs d'un coup.
    - `web/v2/e2e/serve.sh` garde la sortie du serveur des parcours
      (`web/e2e/.state/kartenn-v2.log`, jusqu'au passage suivant).
    Passages : `vlan.spec.ts` et `tree.spec.ts` dix fois de suite sous Firefox, 70 sur 70 ;
    `k-web2-e2e` complet 285 verts (3 ignorés), journal du serveur sans un refus ;
    `k-check` 1 420 ; `k-web2-check --base main` (tout, la fixture le demande).
  - choix : le `BEGIN` dans une tâche plutôt qu'un correctif de sqlx (`[patch]`, une
    dépendance) ou une vérification à chaque remise au pool (un aller-retour de plus) ;
    `kartenn-bench` et `kartenn-testing` gardent `Pool::begin` (aucun futur lâché) ;
    transitions coupées dans `expectAccessible` plutôt qu'une attente propre à chaque
    parcours (toute bascule tardive y passait).
  - reste :
    - **arbitrage** : le plafond des requêtes en vol (REQ-006-30, 20 par défaut, deux fois
      le pool) est franchi par un seul écran : la tournée de `accessibility.spec.ts:118`
      envoie 66 `GET /v1/addresses/{id}` à la fois après la lecture d'un préfixe (trace,
      `time` de chaque requête). Au départ à froid d'un passage complet, 65 refus en 3 s
      (`http.request` 503) ; une fois, le rappel `/v1/auth/callback` d'une connexion
      d'`admin.spec.ts:419` (Chromium) pris dedans, la connexion perdue. Même famille que
      la 11p10 (une lecture par ligne) : lectures par lots, plafond de concurrence dans le
      SDK, ou taille du plafond.
    - rapporter le défaut de `TransactionManager::begin` à sqlx ; une règle
      `clippy::disallowed_methods` sur `Pool::begin` attendrait que les tests aient leur
      exception.
- [x] 12. Édition en place et modification rapide : éditabilité (droits, présent,
  contrat), cellule → brouillon, Ctrl+Entrée → `write.quick` (`?approval=self`) avec motif
  en ligne, rendu de 202, 412 (fusion), 422, 403 ; création (`c`) et modification (`e`)
  par le panneau d'édition de la sorte (`EditPanel`, 024) ; droits manquants au survol —
  REQ-021-15, 16, 36, 37 — verify : parcours `edit` réécrit : lecteur sans éditeur ; brouillon à 1 ; 202 →
  « en revue » ; deux onglets → 412 sans perte
  - où : `web/v2/shell/src/views/edit/` (`fields.ts`, `inline.ts`, `QuickApply.vue`,
    `EditPanel.vue`, `panel.ts`), `views/{ListView,RecordView,TopologyView}.vue`,
    `frame/ShellFrame.vue` ; `web/v2/ui/src/components/DataGrid/DataGrid.vue`,
    `internal/cellEditor.ts` ; tests `shell/test/edit.test.ts`,
    `e2e/journeys/edit.spec.ts` ; référence de l'ancien shell
    `web/shell/src/edit/{QuickEdit.vue,fields.ts,submit.ts}`, `rights/index.ts`,
    `views/NewView.vue`
  - dépend de : 11 ; 4
  - en cours : fait le 2026-11-01 — `web/v2/shell/src/views/edit/` : `fields.ts` (champs de
    création et de patch de chaque sorte, nommés par les schémas `New*`, `*Patch` du contrat
    — un nom hors schéma ne compile pas —, listes fermées vérifiées exhaustives, opérations
    du brouillon `create_*`, `update_*`, `rename` avec `expected` = version lue,
    `draftedValues` qui relit le brouillon actif, `editability` : `X.edit` du type,
    `prop.<clé>.edit` d'une colonne `prop.<clé>`, présent, champ du patch, sinon la raison :
    permission, hors présent, `core.domain.immutable`, valeur calculée, fixée à la
    création, parent déplacé), `inline.ts` (`useInlineEdit` : Entrée → `draft.add`, cellule
    « en brouillon » tirée du brouillon actif, ligne marquée ; Ctrl+Entrée → motif en ligne
    prérempli puis `write.quick` sous `If-Match` de la version de la ligne, valeur montrée
    « en attente » avant la réponse ; 200 appliqué, 202 « en revue » sans jamais
    « appliqué », 412 saisie gardée et valeur relue à côté, nouvelle version prise, 422
    violation et code sur la cellule, 403 refus ; sans `X.approval.self`, Ctrl+Entrée va
    au brouillon et le dit), `QuickApply.vue` (formulaire du motif au pied de la liste),
    `RefusalAlert.vue`, `EditPanel.vue` + `panel.ts` (panneau d'édition de la sorte dans
    l'onglet Détail du panneau droit, mêmes champs d'où qu'on vienne, choix de la sorte pour
    le « + » d'un nœud, domaine prérempli ; bouton scindé « Ajouter au brouillon » /
    « Appliquer » (`changesets.submit` avec `approvalSelf` pour une création, `write.quick`
    pour une modification), action refusée visible avec sa permission (`forbidden`) ;
    Ctrl+Entrée lance l'action choisie ; focus au premier champ, rendu à l'ouvreur),
    `say.ts` (toasts : appliqué avec « Voir le changeset », proposé avec la raison et « Voir
    la proposition »). `ListView.vue` (grille éditable, pied : touches de la cellule ou
    raison de la lecture seule ; `c` créer, `e` modifier la ligne), `RecordView.vue`
    (« Modifier », `e`), `TopologyView.vue` (« + » d'un préfixe : adresses et plages dans un
    réseau, préfixes dans un conteneur ; `c`, `e`), `ShellFrame.vue` (panneau dans l'onglet
    Détail, morceau paresseux). `@kartenn/ui` : Ctrl+Entrée (⌘+Entrée) dans l'éditeur d'une
    cellule émet `commit` marqué `now` (`GridCommit.now`, `internal/cellEditor.ts`, les
    deux grilles) ; `DataGrid` prend `readonlyReason` (la raison d'une cellule sans
    éditeur, sa description `aria-describedby`, une phrase cachée par raison distincte, et
    son infobulle au survol) ; 020 design « Contrat d'édition », inventaire, skill. SDK :
    `changesets.submit(soumission, clé, { approvalSelf })`, `FORBIDDEN_RULE` exporté. Relu
    (`kartenn-revue`) : présent et droits revérifiés à l'écriture (ligne du motif et panneau),
    ligne du motif et panneau fermés hors présent ; 202 laisse la valeur lue, « en revue » ;
    violation d'une création (`/operations/0[/<membre>]`) sur son champ, focalisé ; un
    Ctrl+Entrée ne remplace jamais une ligne en vol ou un 412 ouvert (refus dit avec la
    saisie) ; entrées d'état purgées à la relecture de la ligne ; valeur de la ligne du
    motif vérifiée avant envoi. Vitest `shell/test/edit.test.ts` (éditabilité et raisons,
    opérations, valeurs du brouillon, vérification locale, champ d'une violation ; liste
    montée contre une API simulée : brouillon, lecteur, Ctrl+Entrée sans droit, 200 et son
    lien, 202 valeur lue gardée, 403, 412 puis nouvelle version, 422, hors présent, 412
    ouvert non remplacé, erreur purgée ; panneau fermé avec sa page et hors présent),
    `ui/test/components-data.test.ts`, `sdk/test/write.test.ts` ; scripts
    `e2e/components/data-grid.spec.ts` (Ctrl+Entrée, raison en description et au survol),
    `tree-grid.spec.ts` (Ctrl+Entrée) ; parcours `e2e/journeys/edit.spec.ts` (lecteur sans
    éditeur et raison ; préfixe modifié au clavier, brouillon à 1 ; politique d'approbation
    → « en revue », valeur lue gardée ; deux onglets → 412 sans perte, puis appliqué ; même
    panneau depuis un nœud et depuis la liste ; « Appliquer » refusé, « Il faut
    `core.address.approval.self` » atteint au clavier (Carol, rôle `redacteur` sans
    `approval.self`, `global-setup.ts`) ; création appliquée `?approval=self`, violation
    sur « Valeur », puis appliquée ; domaines nommés après `production`, que la Topographie
    ouvre d'abord ; base des parcours recréée à chaque passage, rien ne s'accumule).
  - choix : motif en ligne au pied de la liste (formulaire, Entrée applique, Échap annule,
    focus rendu à la cellule), non dans la cellule ; raison d'une cellule non éditable en
    description de la cellule et en infobulle au survol (décision de la session
    principale : REQ-021-37, « au survol et au focus »), touches ou raison montrées au pied
    pour la cellule active (pas d'encart permanent) ; violation `/operations/0` d'une
    création (le serveur ne nomme pas le membre) sur le champ qui nomme l'objet (valeur,
    première adresse, clé) ; « + » d'un nœud montré à qui peut ajouter seulement
    (le `TreeGrid` n'a pas de « + » refusé, et Entrée sur un nœud irait dans le « + » au
    lieu d'ouvrir sa fiche) ; colonne de propriété sans éditeur tant que la liste n'écrit
    pas de propriété ; panneau d'édition fermé au changement de page, focus rendu à
    l'ouvreur sinon à la surface ; « ajouté au brouillon » dit par le statut poli de la
    liste ou par un toast depuis le panneau, une issue appliquée ou proposée par son seul
    toast ; `Request-Id` d'un refus copiable dans son encart ; cellules à liste
    fermée (nature, usage) éditées en texte vérifié contre la liste (un `Select` dans
    l'emplacement `editor` perd le focus vers sa couche et validerait aussitôt) ; actions
    du panneau d'édition à la fin de son formulaire (dans le `form`, Entrée et Ctrl+Entrée),
    non dans le pied du `SidePanel` ; création appliquée par un changeset d'une opération
    (`POST /v1/changesets?approval=self`, la même opération que le brouillon) ; une
    référence (domaine, parent) choisie dans la première page de sa collection ; issue
    réussie dite par un toast, refus dit sur place ; entrée de session : les écritures
    suivent un geste (`session/expiry.ts` inchangé).
  - fait le 2026-11-11 (clôture) : le verify est tenu par le code ci-dessus, relevé contre
    `main` (ef46d41b) — `views/edit/inline.ts` (`useInlineEdit` : brouillon, Ctrl+Entrée
    → `write.quick` sous `If-Match`, 200, 202 « en revue », 412 saisie gardée, 422, 403),
    `fields.ts` (`editability`), `EditPanel.vue` + `action.ts` (12b), `say.ts`,
    `ListView.vue`, `TopologyView.vue`, `record/ContentTab.vue` — et par le parcours
    `e2e/journeys/edit.spec.ts` (lecteur sans éditeur et pourquoi ; brouillon à 1 ;
    politique d'approbation → « en revue », jamais « appliqué » ; deux onglets → 412 sans
    perte ; même panneau depuis un nœud et la liste ; « Appliquer » refusé avec
    `core.address.approval.self`), sous les deux navigateurs (12c). L'acceptation de
    REQ-021-15 « 202 : cellule en revue avec le motif de la réponse » manquait : 12e.
  - reste, réparti : édition en place dans l'arbre et la grille du contenu, `TreeNode.version`,
    « + » refusé d'un nœud → 12a (faits) ; motif de la 202 sur la cellule → 12e (faite) ;
    `TreeGrid` sans raison de cellule, 202, 422 et 403 non prouvés dans l'arbre, deux
    `useNodeEdit` pour un même nœud → 12f ; éditeur à liste dans une cellule → 12g ; onglet
    Propriétés édité en place → 12h ; colonnes de propriété d'une liste (à arbitrer) → 12i ;
    référence au-delà de la première page → 12j ; `c` et `e` refusés muets au clavier
    (12c, 12d) → 12k ; captures des vrais écrans (à arbitrer, transverse aux 11, 12, 14,
    17) → 12l ; relevés de la revue de la 12e → 12m, 12n ; valeur héritée d'une propriété
    (à arbitrer, avant la 12h) → 12o.
- [x] 24. Déplacements et invariant des arbres : arbre des préfixes en `derived` (aucun
  déplacement ; valeur modifiée par le brouillon, aperçu tiré de la simulation, arbre
  relu après application) ; `canMove` du shell (cycle, types tirés du contrat, droits sur
  les deux bouts, présent, codes des règles serveur) ; déplacement → brouillon ou
  changeset, sans optimisme ; test de propriété de bout en bout — REQ-021-17, 32, 33,
  34 — verify : Vitest de `canMove` (chaque refus, ordre) ; parcours : aucun moyen de
  déplacer un préfixe ; cluster sur sa propre machine refusé ; machine vers un hôte
  d'un autre site acceptée par le client : avertissement du serveur montré
  (`machines.host.site_mismatch`), déplacement au brouillon, le nœud à sa place
  (amendé le 2026-11-02, arbitrage de la 18b : la règle reste un avertissement, 009
  M11) ; réponse ralentie →
  nœud « en attente » à sa place ; test de propriété (graine fixée, 200 étapes de
  déplacements et de modifications de valeurs de préfixes et de plages, acceptées ou
  refusées) : chaque ligne contenue par sa ligne parente et parent égal à celui de l'API
  - où : design § « Déplacements dans les arbres » ; `web/shell/src/tree/model.ts` ;
    `web/sdk/scripts/gen.mjs` (table des parents admis, comme `dated.ts`) ;
    `crates/kartenn-machines/src` (règles `machines.host.*`) ; 020 design § « Contrat de
    déplacement »
  - dépend de : 12 ; 020-26 ; le cas machines après 18
  - en cours : fait le 2026-11-01 — SDK : `scripts/gen.mjs` génère `src/api/parents.ts`
    (`PARENTS` : pour chaque type à parent déclaré, l'écriture qui change son parent —
    `move_entity.parent`, `move_site.parent`, `update_vlan_group.anchor`,
    `MachinePatch.host` —, les types admis, lus des variantes du membre (`TargetKind`) ou du
    filtre `parent` de sa liste, et la racine permise si le membre du déplacement prend
    `null` et que la création ne l'exige pas ; build en échec si l'écriture ou son membre
    disparaît du contrat ; test de fraîcheur) ; `data/moves.ts`
    (`canMove(nœud, cible, { ancestors, can, present })` : cycle, parent incompatible, droit
    `X.edit` du nœud puis de la cible, lecture datée, dans cet ordre ; `MOVE_CODES` :
    `core.anchor.cycle`, `core.vlan.group_anchor`, `machines.host.invalid` ; exportés avec
    `parentRule`) ; `data/invalidate.ts` : un préfixe, une plage, une adresse changés
    invalident toutes les lectures d'enfants de l'arbre (`core.domain/roots`,
    `core.prefix/children` ; `machines.machine/hosted`, type de la clé qualifié : la
    Topographie lit par `prefix`), et `lists()` aussi (la liste du nouveau parent ne montre
    pas encore l'objet). Serveur (trouvé par le test de propriété) : `kartenn-rest`
    `request_id.rs` réécrivait le corps d'un problème (son `instance`) après la compression
    en gardant `Content-Encoding` — tout refus de plus de 1 Kio était illisible pour un
    navigateur (`ERR_CONTENT_DECODING_FAILED`) ; `instance` maintenant posée à l'intérieur
    de la compression (`request_id::instance`, `lib.rs`), le corps réécrit compressé selon
    `Accept-Encoding` (REQ-006-32), test
    `tests/it/compression.rs::a_problem_over_1_kib_is_compressed_with_its_instance`
    (zstd, br, gzip, décodé, `instance` présente). Shell : `views/edit/moves.ts` (`useMoves` :
    `canMove` en mots avec code, `moveState`, `move` → `draft.add` puis simulation du
    brouillon — valide : nœud marqué à sa place ; refusé par une violation de **son**
    opération (indice ou pointeur `/operations/<i>`, ou règle de l'état final sur le nœud),
    jamais par celle d'une autre opération ; simulation en échec : verdict inconnu, le
    problème rendu par `problems/` (`RefusalAlert`, `NetworkError` sans réponse) — ou
    `changesets.submit` `?approval=self` avec motif ; « en attente » jusqu'à la réponse,
    aucun `setQueryData`, refus 422/403/412 avec code (une violation : sa règle ; sinon la
    phrase du `type` par `problems/` et le `type` servi) ; un nouveau
    déplacement d'un nœud déjà déplacé au brouillon actif remplace son opération
    (`drafts.update`), jamais deux ; seconde intention en vol ignorée avant tout contrôle ;
    version lue, sinon (absente ou `null`) relue par identifiant, et sans elle rien n'est
    envoyé, refus dit) ; « Changer de parent… » d'une entité ou d'un site (menu de la fiche,
    `RecordView.vue` ; `EditPanel.vue` mode `move`, `Combobox` des seules cibles valides,
    « Aucun » pour la racine, simulation en échec rendue par `RefusalAlert`, panneau
    gardé) ; Topographie : nœud dont le brouillon actif change la valeur marqué « en
    brouillon » à sa place, et chaque nœud placé une fois pendant la relecture des niveaux
    (`views/topology.ts`, `nodesOf` : une liste encore périmée montrait un nœud sous son
    ancien enfant, un cycle, débordement de pile au rendu — trouvé par le test de
    propriété). Vitest `sdk/test/moves.test.ts` (table, contrat changé, membre
    disparu, racine par nullabilité, chaque refus et leur ordre), `sdk/test/dependents.test.ts`
    (arbre relu), `shell/test/moves.test.ts` (mots et codes, nœud que l'hôte ne déplace pas,
    hors présent, brouillon simulé valide et invalide, violation d'une autre opération,
    opération remplacée, simulation en échec, version illisible, réponse ralentie « en
    attente » sans écriture du cache, seconde intention, refus serveur après acceptation
    du client, 403, 412, déplacement sur son parent), `shell/test/moves-property.test.ts`
    (test de propriété monté : `TreeGrid` `declared` et `useMoves` contre un serveur simulé
    d'entités, graine `0x2124`, 200 dépôts au brouillon ou appliqués, refusés par le
    contrôle du client (cycle) ou par le serveur, nœud à sa place « en attente » tant que la
    réponse est retenue, puis chaque ligne rendue sous le parent du serveur) ; parcours
    `e2e/journeys/moves.spec.ts` (arbre des préfixes : ni glisser, ni Ctrl+X/Ctrl+V, ni
    « Déplacer vers… » ; valeur changée au brouillon, arbre inchangé et nœud marqué, puis
    brouillon soumis, arbre relu à la nouvelle place ; entité : cibles valides seules,
    réponse ralentie sans nouveau parent affiché, puis appliqué ; propriété : graine
    `0x2124`, 200 étapes de valeurs de préfixes et de remplacements de plages, au brouillon
    ou directement, acceptées et refusées, l'arbre rendu égal à celui de l'API et chaque
    ligne contenue par sa ligne parente).
  - choix : `canMove` et la table dans le SDK (un plugin de première partie ne peut pas
    importer le shell ; `context.user.has` et `context.time.present` lui suffisent), le
    déplacement et ses mots dans le shell ; une plage change de valeur par son
    remplacement (`close` puis `create_range` : le contrat n'a pas de bornes dans
    `update_range`) dans le test de propriété seulement ; un déplacement sur le parent
    actuel n'écrit rien ; une seconde intention pendant qu'une est en vol est ignorée ; un
    brouillon dont la simulation refuse le déplacement garde l'opération, le nœud marqué
    refusé avec la règle, le panneau le dit, et un nouveau choix de cible la remplace
    (retrait par la page des brouillons, tâche 14) ; `root` = membre du déplacement
    nullable **et** non exigé à la création (`update_vlan_group.anchor` prend `null` pour
    « inchangé » : un groupe n'est pas une racine) ; `expected` de l'opération : version de
    la ligne, sinon objet relu par identifiant (une lecture, du cache s'il est frais) ;
    `Combobox` des cibles valides (filtrées par la saisie), un texte sans cible choisie
    refusé au champ ; cibles et ancêtres d'une entité lus dans les dix
    premières pages de sa collection (un cycle au-delà : `core.anchor.cycle` du serveur) ;
    motif prérempli avec la destination (« … → <parent> ») ; panneau ouvert après la
    fermeture du menu, focus rendu au bouton du menu (l'entrée choisie disparaît avec
    lui, `openEdit`) ; option « Aucun »
    d'une référence facultative à valeur propre (`none`) : la chaîne vide est le « sans
    valeur » du `Select` (Reka refuse une option vide) ; test de propriété en Chromium
    seul, arbre comparé toutes les 10 étapes, 400 ms entre deux étapes et 429 attendu
    selon `Retry-After` (quotas d'écriture de l'instance, partagés avec les parcours
    voisins) ; sous Firefox, corps vide des modules préchargés pendant un `page.route`
    toléré dans le seul parcours de la réponse ralentie (empreinte SHA-384 du vide).
  - reste : le cas machines (tâche 18) — cluster sur sa propre machine refusé (cycle,
    `machines.host.invalid`, prêt dans `canMove`), machine vers un hôte d'un autre site
    (`machines.host.site_mismatch` : avertissement du serveur montré, déplacement au
    brouillon — amendé le 2026-11-02, fait par la 18b), réponse ralentie dans un
    `TreeGrid` `declared` (`useMoves` est prêt pour un arbre du shell ; une vue de plugin
    reprend `canMove` du SDK et ses écritures, ou `useMoves` passe au SDK) ; Ctrl+Entrée
    sur un dépôt (l'intention du `TreeGrid` n'a pas de `now`) ; **aperçu du brouillon** :
    à arbitrer, le contrat ne le permet pas — la simulation (`ChangesetSimulated`) ne rend
    que le différentiel des propriétés, ni valeur ni parent d'un préfixe ; options : (a)
    lecture de l'arbre sous un brouillon (`roots`/`children` `?draft=<id>`), (b)
    `placements` dans la simulation (objet, parent avant, après), (c) REQ-021-17 amendée
    sans aperçu ; états transitoires d'une relecture (parent relu avant ses enfants : nœud
    montré une fois, mais un instant sous un parent qui ne le contient plus) non couverts
    par le test de propriété, qui compare l'arbre une fois stable ; erreur d'un
    champ du panneau gardée jusqu'au prochain envoi quand la valeur change (commun à tous
    les champs du panneau) ; Ctrl+Entrée sur le chevron du `SplitButton`, juste après le
    choix « Appliquer », n'applique rien (le focus doit être dans un champ : composant
    020, à vérifier) ; déplacements à parent déclaré dans le test de propriété de bout en
    bout : aucun arbre à parent déclaré n'est encore à l'écran (tâche 18), le parcours ne
    peut en glisser — couverts par le test de propriété monté, le parcours à étendre avec
    l'arbre de l'hébergement ; le shell n'offre pas de changement des bornes d'une plage
    (le contrat n'en a pas : remplacement seulement, fait par le test), et la Topographie
    ne marque pas « en brouillon » une plage remplacée au brouillon (seules les opérations
    `update_*` le sont).
  - fait le 2026-11-11, relevé contre le code (main 50e5bd18) : la clause de `verify` tient
    toute. `canMove` et ses refus dans l'ordre : `web/v2/sdk/src/data/moves.ts`, Vitest
    `sdk/test/moves.test.ts` (codes `machines.host.*` compris, 18b). Cas machines : fait par
    la 18b puis la 18c — arbre de l'hébergement en `TreeGrid` `declared`
    (`plugins/machines/src/HostingTree.vue`, `canMove` et `draft.move` du SDK, sans
    optimisme), parcours `moves` « the host of a machine » (hôte sous son invité refusé
    par le client avec `machines.host.invalid`, rien envoyé ; vers un hôte d'un autre site
    accepté par le client, « en attente » à sa place pendant la simulation retenue, puis
    l'avertissement `machines.host.site_mismatch`, au brouillon, aucun changeset ; vers un
    cluster au clavier) et `machines` (physique vers un cluster refusé par la simulation, à
    sa place). « Cluster sur sa propre machine » est sans objet : un cluster n'a pas de
    parent dans le contrat (aucune entrée `machines.cluster` dans `PARENTS`), le refus par
    cycle est testé par son analogue « hôte sous son invité ». Aperçu du brouillon : tranché par l'option (b), fait par les 24a à 24d
    (`placements` et leur opération dans la simulation, aperçu et états `moved`/`preview`
    de la Topographie) ; enfants jusqu'à 10⁴ lignes par la 24e. Ajouté ici, seul morceau
    de `verify` manquant : le volet « déplacements dans un arbre à parent déclaré » du
    test de propriété de bout en bout — parcours `moves` « property: 100 moves of hosts,
    accepted or refused, and every row under the host the API gives » (Chromium, graine
    `0x2124`, deux clusters et neuf machines de chaque sorte sous une étiquette propre,
    100 `machines.update_machine` `host` vers une cible quelconque ou aucune, au brouillon
    ou directement, acceptés ou refusés par le serveur (009 M5) ; toutes les 10 étapes,
    chaque nœud de l'étiquette ouvert — en échec, « nœuds restés fermés » et leurs noms,
    jamais un arbre à moitié ouvert comparé —, l'arbre rendu égal à celui de l'API —
    niveau et parent de chaque ligne — et chaque ligne sous un hôte qui peut la porter) ;
    le test des valeurs ramené à 100 étapes : 200 au total, comme le demande `verify` ;
    `submit` des deux tests de propriété mis en commun.
  - choix : les déplacements du test de propriété passent par l'API, comme les valeurs du
    test voisin (le geste lui-même est couvert par les parcours de l'hébergement et par le
    test de propriété monté) ; arbre comparé sur les seules lignes de l'étiquette, triées
    par libellé (le premier niveau de l'hébergement est partagé avec les autres parcours ;
    l'ordre des frères n'est pas l'invariant) ; aucun site, donc aucun avertissement ni
    anomalie à chaque étape ; aucun cycle possible au-delà de soi-même (009 M5 : un hôte
    virtuel ne porte que des conteneurs, qui n'hébergent rien), les refus viennent des
    sortes.
  - reste, porté par des sous-tâches : Ctrl+Entrée sur un dépôt (REQ-021-32, changeset
    immédiat) dans l'arbre de l'hébergement, un plugin au brouillon seul (W10, 18b) —
    tâche **24f** (arbitrée par l'utilisateur le 2026-11-11, spec d'abord) ; plage
    remplacée au brouillon non marquée « en brouillon » par la Topographie
    (`draftedValues` ne lit que les `update_*`) — tâche **24g**.
  - reste (hors de `verify`, ouvert) : l'ancrage d'un groupe de VLAN par glisser n'est pas
    offert (REQ-021-32 le permet sans l'imposer) : il change par le panneau, sans arbre ;
    états transitoires d'une relecture non couverts (les deux tests comparent l'arbre
    stable) ; erreur d'un champ du panneau gardée jusqu'au prochain envoi
    (`EditPanel.vue`, `errors`) ; Ctrl+Entrée sur le chevron du `SplitButton` (020, à
    vérifier) ; changement des bornes d'une plage absent du contrat.
- [x] 13. Libellés et codes : `label` (dictionnaire `GET /v1/labels` de la 018 en cache par
  langue et révision, invalidé par l'événement `labels` ; catalogues ; clé),
  `KeyLabel` partout ; toute erreur par `ErrorState`, `InlineAlert`, `Toast` avec codes et
  `Request-Id` — REQ-021-19, 20 — verify : Vitest de `label` ; parcours : erreur 422
  montre le code de chaque violation, copiable
  - où : `web/shell/src/i18n/index.ts` (traduction des problèmes),
    `admin/WriteOutcome.vue`
  - dépend de : 11
  - en cours : partie SDK faite le 2026-10-24 (avec la P1) — `web/v2/sdk/src/data/labels.ts`,
    `label(space, key, lang?)`, `labelDictionary(lang)`, `lookup` (valeur d'une liste
    fermée en `<propriété>/<valeur>`), catalogue imbriqué `labels.<space>.<key>`, puis la
    clé ; `ApiProblem.code`. La route `GET /v1/labels` (018) n'étant pas encore au contrat
    de `main`, lecture par URL (`reads.byUrl`) et Vitest sur un serveur simulé
    (`test/labels.test.ts`). Reste : `KeyLabel`, rendu des erreurs, parcours.
  - en cours : fait le 2026-11-01 — SDK : `GET /v1/labels` étant au contrat de `main`,
    lecture typée `reads.keyed(clé, chemin, {query, family})` (chemin, paramètres et
    réponse du contrat ; `byUrl` retiré), `Dictionary` = schéma `Labels` aux espaces
    typés ; Vitest : un changeset (`lists()`) ne relit pas le dictionnaire. Shell : un seul
    chemin des problèmes, `web/v2/shell/src/problems/` — `useProblems()` (`sentence` : espace
    `problem` du dictionnaire, puis `labels.problem.<slug>` des catalogues, puis les mots de
    la réponse, sinon serveur injoignable ; `lines` : violations ou champs, puis le `type`
    tel que servi, puis `Request-Id`), `ProblemState.vue` (état d'erreur de toute vue :
    titre de la vue, pourquoi, codes, « Réessayer ») à la place de chaque `ErrorState` des
    vues ; `RefusalAlert`, la connexion et l'erreur d'une cellule (`cellErrorOf`) passent
    par lui ; `problemOf`, `problemSlug` déplacés de `views/data.ts`. `@kartenn/ui` :
    `ErrorState` met le texte du pourquoi dans sa région `alert` (relecture a11y : il
    n'était pas annoncé). Catalogues : `labels.problem.*` (29 types), `problem.*`,
    `views.properties.types.*` (type d'une propriété en mots, comme la maquette E13).
    Vitest `test/problems.test.ts` ; parcours `edit` : un 422 montre le code de chaque
    violation, du problème et le `Request-Id`, chacun un bouton qui copie.
  - fait aussi le 2026-11-01, après la revue : libellés du serveur, espace `problem`
    (`ProblemKind::labels()`, `crates/kartenn-rest/src/problem.rs`), réécrits en phrases
    courantes, fr et en, les mêmes que `labels.problem.*` (espace et clés inchangés ; le
    `title` reste le résumé neutre) ; SDK : `NetworkError` (pas de réponse) distinct d'un
    bogue du client, dit « erreur de l'interface » ; `keyed` refuse un chemin à paramètres
    et une liste ; `isDictionary` contrôle chaque réponse de `/v1/labels`. Shell :
    « Rester connecté » en échec (`session/warning.ts`) et le refus 400 d'une date
    (`time/index.ts`, `refusalOnField`) passent par `problems/` — phrase du `type`, `type`
    et `Request-Id` (avis ; sous les champs de `PeriodPicker` par sa nouvelle prop
    `problem`, relayée par `DatePill`) ; une page suivante refusée (hors curseur) est dite
    au-dessus des lignes gardées (`NextPageAlert`, listes, contenu et historique d'une
    fiche) ; « transmettez ces codes » seulement s'il y a des codes ; la clé du type d'une
    propriété reste à portée (`KeyLabel`, le mot montré). `KeyLabel` est fait là où la spec
    et les maquettes le montrent : clés des propriétés, valeurs des listes fermées (cellules,
    fiche, valeurs d'une liste déclarée), règles des anomalies, type d'une propriété.
  - choix : le code montré est le `type` entier (design « Erreurs », revue du 2026-10-25),
    plutôt que le slug (le design « Codes » est corrigé) ; la phrase d'un état d'erreur est
    prise une fois par erreur : l'arrivée du dictionnaire ne réécrit pas une région `alert`
    déjà dite (ses libellés et ceux des catalogues sont désormais les mêmes phrases) ; dictionnaire toujours lu au présent, sa
    clé reste `['v1', 'labels', langue]` (hors des clés par chemin qu'invalide un
    changeset) ; la page de connexion lit les seuls catalogues (pas de session pour le
    dictionnaire) ; dans un encart dont le titre dit déjà le problème (412, 422, 403,
    réseau), la ligne du `type` porte « Code du problème » plutôt que la phrase ;
    « Request-Id » en toutes lettres partout, comme `ErrorState` ; type d'une propriété en
    mots (maquette E13), sa clé dans la pastille ; les valeurs des listes fermées du cœur
    (`nature`, `usage`) restent libellées par les catalogues du shell (mêmes libellés que
    ceux compilés du serveur ; `usage` y est rangé sous `<sorte>.<usage>`).
  - reste : « À propos », capacités illisibles (`about.pluginsUnavailable`) : ni code ni
    `Request-Id`, le chargeur (`plugins/loader.ts`, `readPlugins`) ne garde pas le problème
    — le garder au registre et le rendre par `useProblems().lines()`. La page suivante
    refusée d'un arbre de la Topographie (`TopologyView.vue`, laissé à la 021-24 qui y
    travaille) reste muette. Questions ouvertes chez l'utilisateur, non tranchées : clés en
    texte simple ou `KeyLabel` dans les listes hors tableau (`DeclarationValues` les montre
    aujourd'hui en `KeyLabel`, à confirmer ou à revoir selon la décision) ; raccourci et
    position des messages passagers (`Toast` : seul l'échec de « Rester connecté » passe par
    un avis, avec son code et son `Request-Id`).
  - relevé le 2026-11-11 contre le code (main 2dc9ecc8) : le reste ci-dessus est seul
    ouvert, le reste de la tâche tient. SDK : `web/v2/sdk/src/data/labels.ts` (`label`,
    `lookup`, `isDictionary`, `changed`), l'événement `labels` par
    `sdk/src/events/apply.ts` ; Vitest `sdk/test/labels.test.ts` et `sdk/test/events.test.ts`
    (« labels: the dictionary »). Shell : toute erreur de l'API par `shell/src/problems/` —
    `ProblemState` dans chaque vue, celles de `admin/` comprises, `RefusalAlert`,
    `useProblems().lines` (`ImportDetail`, `AuditView`), `NextPageAlert`, `toastCodes`
    (`session/warning.ts`) ; les encarts des simulations montrent la règle de chaque
    violation (`ChangesetView`, `DraftOperations`, `ClosePanel`) et `ExtensionElement` son
    code `plugin.<raison>`. Le catalogue `admin` sorti de l'entrée par la 22h ne porte aucun
    mot des problèmes : `labels.problem.*` (les 29 types de `ProblemKind::slug`,
    `crates/kartenn-rest/src/problem.rs`) et `problem.*` restent dans `views/i18n`, chargé
    avec l'entrée. Libellés à clé par `useKeyLabel` (`views/data.ts`) : `kind`, `property`,
    `value`, `rule`, `state` (`changesets/words.ts`), `role` et `group` (`admin/RolesView`,
    `GroupsView`), `permission` (`admin/RoleDetail`). `verify` tenu : Vitest de `label`
    ci-dessus ; parcours `edit` (« a creation applied at once… ») : un 422 montre sous les
    champs « Copier core.prefix.unaligned », le `type` et le `Request-Id`, la copie
    elle-même prouvée par les scripts `e2e/components/code-badge.spec.ts` et
    `key-label.spec.ts` (les parcours tournent aussi sous Firefox, où Playwright n'accorde
    pas la lecture du presse-papiers). `web/v2/PARITE.md` : aucune ligne ouverte sur les
    libellés ou les codes. Découpée en 13a (« À propos ») et 13b (Topographie) ; les deux
    questions ci-dessus restent à l'utilisateur, et la tâche se coche avec 13b.
  - reste (relevé du 2026-11-11) : l'attribut `lang` en repli (018, « Repris par la 021 »,
    ligne R7, WCAG 3.1.2) n'est ni dans REQ-021-19 ni fait — un libellé servi dans la
    langue par défaut (`fallback.keys` du dictionnaire) est montré sans `lang` ; il faudrait
    que `label` du SDK dise la langue du libellé trouvé et que `KeyLabel` la porte : à
    amender dans REQ-021-19 avant une tâche.
- [x] 13a. Libellés et codes, « À propos » : capacités illisibles dites avec leur problème
  (phrase du `type`, `type` et `Request-Id`, copiables) — REQ-021-20 — verify : Vitest
  (`readPlugins` échoue avec le problème de la dernière réponse ; « À propos » le montre)
  - où : `web/v2/shell/src/plugins/loader.ts` (`readPlugins`, `startPlugins`),
    `plugins/registry.ts` (`unavailable`), `frame/AboutView.vue` ; imiter
    `session/LoginView.vue` (`useProblems({ dictionary: false })`)
  - dépend de : 13
  - fait le 2026-11-11 : `readPlugins` ne rend plus `null` : sa promesse échoue avec le
    pourquoi — `ApiProblem.from` de la dernière réponse (refus, ou 429 et 503 au bout de
    `CAPABILITY_ATTEMPTS`), `NetworkError` sans réponse, `TypeError` pour un corps sans
    liste de plugins ; `startPlugins` le garde au registre (`unavailable(true, error)`,
    `usePluginsProblem()`, remis à `null` par une lecture réussie et `resetPlugins`) ;
    « À propos » le rend sous l'encart (`InlineAlert` `items` = `useProblems().lines`).
    Vitest : `shell/test/plugins.test.ts` (dernière réponse 503 avec son `Request-Id`,
    réseau, corps sans liste ; 403 gardé puis effacé) et `shell/test/problems.test.ts`
    (« À propos » : la phrase, `type` et `Request-Id` en boutons de copie ; sans réponse,
    le réseau et aucun code). Relectures : « Réessayer » passe dans l'encart, en `size="sm"`,
    comme `NextPageAlert` (il le suivait, hors de l'encart) ; pendant la relecture, la page
    disait « aucun plugin » (`read` restait vrai) : `unavailable(false)` remet `read` à faux,
    « À propos » montre le chargement jusqu'à la réponse (Vitest).
  - choix : catalogues seuls (`dictionary: false`) — « À propos » s'ouvre sans session et
    le dictionnaire la demanderait ; leurs phrases sont celles de l'espace `problem` du
    serveur. Le titre de l'encart ne dit pas le problème : la ligne du `type` porte sa
    phrase (pas `said`). Pas de capture : aucune de « À propos » n'existe.
  - reste (revue, mineurs) : un corps 200 sans liste de plugins (`loader.ts`, `TypeError`)
    donne la phrase `problem.client` (« erreur de l'interface… rechargez la page ») au lieu
    d'une réponse fautive du serveur, et perd le `Request-Id` de la réponse (REQ-021-20) ;
    le test de « À propos » (`problems.test.ts`) ne vérifie ni « Réessayer » dans l'encart
    ni le chargement pendant la relecture (prouvé au registre seulement) ; le 403 de
    `plugins.test.ts` est servi sans `application/problem+json` (refus non transitoire
    avec un vrai `type` non couvert).
- [x] 13b. Libellés et codes, Topographie : la page suivante refusée d'un niveau de l'arbre
  dite avec son problème (phrase, `type`, `Request-Id`) et « Réessayer », au lieu de se taire
  — REQ-021-20 — verify : Vitest (`askNextPage` : un échec de page suivante exposé au
  niveau) ; parcours `tree` (page suivante refusée par `page.route`, dite, relue)
  - où : `web/v2/shell/src/views/topology.ts` (`askNextPage`, `levelOfPages`, ligne d'avis
    `capped:` à imiter), `views/TopologyView.vue` (`roots?.isError &&
    !roots.isFetchNextPageError` vers la ligne 686), `problems/NextPageAlert.vue` ;
    `shell/test/topology.test.ts`
  - dépend de : 13, 24e
  - fait le 2026-11-11 : `levelOfPages` dit au niveau sa page suivante refusée (`failed`,
    l'erreur de l'observateur, `isFetchNextPageError`) ; ses lignes lues restent, `more`
    aussi (« Afficher la suite » activée la redemande). `TopologyView` : `refused`, un
    `NextPageAlert` par niveau dans la bande au-dessus de l'arbre (avec l'aperçu du
    brouillon, `Stack`), titré par le niveau (`views.topology.refused.roots` / `.node`,
    le domaine ou la valeur du nœud), phrase du `type`, `type` et `Request-Id` copiables,
    « Réessayer » = `onLoad` du niveau ; la première page refusée d'un nœud ouvert
    (muette elle aussi) prend le même encart. `NextPageAlert` reçoit un `title` facultatif.
    Vitest : `shell/test/topology.test.ts` (« says at the level why its next page was
    refused… » : 403 avec `type` et `Request-Id` au niveau, lignes gardées, pas redemandée
    seule, relue par un nouvel appel). Parcours `tree` (« a next page of a level refused is
    said with its codes, and read again ») : les pages à curseur des enfants du grand
    conteneur servies 503 par `page.route`, l'encart nommé par le nœud, sa phrase, ses deux
    boutons de copie, puis « Réessayer » : l'encart part, le focus revient à l'arbre, les
    lignes d'après viennent. Relectures : le focus du bouton parti avec l'encart revient à
    la cellule de la focalisation de l'arbre (`retry`, 2.4.3) ; la bande des encarts
    plafonnée à douze lignes, défilante (plusieurs niveaux refusés ne chassent pas l'arbre) ;
    un nœud refermé cache l'encart de son niveau. Parcours `tree` sous Firefox : « the
    chosen node shows its content… » rechargeait avant que les plugins aient fini de charger
    (erreur d'intégrité du plugin `forged`, deux fois sur deux) : `atRest` avant `reload`.
    Avec 13a, la tâche 13 se coche ; ses deux questions restent à l'utilisateur.
  - choix : l'encart hors de l'arbre plutôt qu'une ligne d'avis dans `TreeGrid` — une ligne
    d'avis n'a qu'un libellé (ni codes copiables ni bouton), et hors de l'arbre l'encart
    reste hors des lignes que recycle la 22e (invariant de `virtualRows.ts` intact) ; la
    première page refusée d'un nœud ouvert comprise (même ligne « Afficher la suite », même
    silence) ; la première page refusée du premier niveau garde son `ProblemState` ; l'encart
    dans la bande au-dessus de l'arbre (le corps est sans marge), pas dans le corps comme
    les listes.
  - reste (relectures, mineurs) : pendant la relecture, ni `aria-busy` ni bouton désactivé,
    et le succès n'est pas dit (l'encart part) — comme les listes ; un second refus après
    « Réessayer » n'est pas couvert par le parcours ; le titre de repli (`problem.nextPage`)
    d'un niveau dont le nœud n'est plus lu ne le nomme pas ; la page suivante refusée des
    enfants s'affiche deux fois quand le nœud choisi est celui ouvert (arbre et
    `ContentTab`, même liste). Revue (mineurs) : `expanded.includes(parent)` ne regarde pas
    les ancêtres (encart possible sous un ancêtre refermé, à vérifier) ; une invalidation
    qui retire l'encart pendant que le focus est sur un de ses boutons de copie le laisse
    sur `body` ; si la cellule du focus est hors de la fenêtre virtualisée, le focus reste
    sur `body` (à vérifier) ; la première page refusée d'un nœud ouvert n'a ni Vitest ni
    parcours ; le correctif Firefox (`atRest` avant `reload`) aurait dû être un commit à
    part. Hors périmètre : une relecture refusée d'un niveau enfant qui a des données
    (`isRefetchError` après une invalidation) reste muette.
- [ ] 13c. Questions ouvertes des libellés (reportées de la 13, cochée avec la 13b le
  2026-11-11) : (1) clés en texte simple ou `KeyLabel` dans les listes hors tableau
  (`DeclarationValues` les montre aujourd'hui en `KeyLabel`) ; (2) raccourci et position des
  messages passagers (`Toast`) ; (3) attribut `lang` d'un libellé servi en repli dans la
  langue par défaut (018, ligne R7, WCAG 3.1.2), à amender dans REQ-021-19 avant une
  tâche (`label` du SDK dirait la langue trouvée, `KeyLabel` la porterait) — arbitrage
  de l'utilisateur, puis sous-tâches — verify : décisions consignées dans `decisions.md`
  de la 021, REQ-021-19 amendée, sous-tâches écrites avec `où :` et `verify`
  - où : tâche 13 (« reste » et relevé du 2026-11-11) ; `web/v2/shell/src/` (`KeyLabel`,
    `DeclarationValues`), `web/v2/sdk/src/data/labels.ts` ; spec 018 (« Repris par la 021 »)
  - dépend de : 13
- [x] 14. Changesets, revues, brouillon : listes, `DiffList`, actions selon les droits,
  « À instruire » et compteur, panneau droit et page des brouillons (brouillons du
  serveur, `CommitForm`, `SplitButton`, 024) — REQ-021-6, 10, 11, 37 — verify : parcours
  `changesets`, `draft` réécrits ; captures E4, E5
  - où : `web/shell/src/changesets/*`, `draft/{DraftDrawer.vue,store.ts,index.ts}`
    (le stockage IndexedDB disparaît) ; design 021 « Cadre » (panneau droit, pied du
    brouillon) ; `web/v2/sdk/src/draft.ts` ; repris de la tâche 16 : actions des plugins
    sur le type `changeset` dans le menu de la fiche du changeset
    (`useExtensions("action", "changeset")`, `actionItems`, `PluginAction`,
    `web/v2/shell/src/plugins/`), parcours `plugins` étendu (action `review` du plugin de
    test `notes`, son dialogue fermé par `kartenn-close`)
  - dépend de : 12
  - premier passage, fait le 2026-11-01 — SDK : `writes.decide` (validation, rejet avec motif,
    retrait, annulation `?approval=self`, sous `If-Match` = l'`ETag` du changeset lu ;
    décidé : changeset, listes et `/v1/proposals` relus, une validation ou une annulation
    appliquée invalide ce qu'elle écrit ; 412 : changeset relu, rien à rejouer), offert au
    shell seul par `@kartenn/sdk/host` (`review.decide`, hors contrat semver des plugins) ;
    `sdk/test/write.test.ts` ; correction : `drafts.update` envoie `application/json` (le
    client mettait tout `PATCH` en `merge-patch+json`, refusé 415 par `PATCH
    /v1/drafts/{id}` ; option `merge: false` du client), `sdk/test/drafts.test.ts`. Shell (`web/v2/shell/src/`) : `changesets/` — `model.ts`
    (états et leur badge, type d'une opération, permissions `<type>.apply`,
    `.approval.self`, `.edit` des opérations, actions de revue selon les droits : en
    attente, Valider et Rejeter par un autre (sans `<type>.apply`, refusés avec la
    permission), Valider par l'auteur (REQ-024-11, 12, comme `self_applicable` de
    `kartenn-service/src/review.rs` : avec `<type>.approval.self` sur tout ce qu'il touche
    et sans politique — `kind = approval` ou `policy` — ; sinon refusé avec la permission
    manquante ou `held`, l'avis `changeset.approval.self` et la politique ; pas de
    Rejeter pour lui), Retirer (l'auteur seul) ; appliqué, Annuler (`<type>.edit`) ; hors présent, toutes refusées avec leur
    motif ; différentiel groupé par objet pour `DiffList`, différentiel compté groupé par
    propriété pour un changeset pas encore appliqué), `words.ts` (état et nature par le
    dictionnaire, espace `state`, sinon les catalogues ; « vous »), `ChangesetsView.vue`
    (`/changesets` : `DataGrid` filtrée par l'URL — état en liste fermée, auteur, objet,
    dates de soumission, rétroactif —, « Mes changesets » = filtre de l'auteur),
    `ProposalsView.vue` (`/proposals`, droits qui manquent à l'auteur en `CodeBadge`),
    `ChangesetView.vue` (`/changesets/:id` : une barre, l'état en `StatusBadge` puis la
    `Toolbar` des actions de revue, refusées visibles avec leur permission ou leur motif,
    « … » avec « Copier l'identifiant » (issue dite, `copy.ts` partagé avec `/drafts`) et
    les actions des plugins sur le type `changeset` (`PluginAction`) ; avis à l'auteur
    d'un changeset retenu par une politique (`changeset.approval.self`, la politique) ;
    une décision refusée 412 dite comme telle (« a changé depuis votre lecture ; relu »,
    les actions de l'état relu, rien de « gardé ») ; attributs comme E4 (rétroactif compris) ; onglets
    Différentiel (anomalies rattachées en tête, `DiffList`, pages de `/diff` par
    « Afficher la suite », compte montré une fois tout lu ; avant application, le
    différentiel compté), Opérations, Violations (codes), Sections de modules (nom,
    lignes) ; rejet par un `Dialog` à motif obligatoire (focus au champ en erreur),
    annulation confirmée par un `AlertDialog` ; après une décision, focus à la première
    action restante ; refus dit par `RefusalAlert`), `routes.ts` (écrans de l'espace `changes`, `meta.queries`),
    `i18n/{fr,en}.json` (fusionnés en profondeur) ; `draft/` — `state.ts` (brouillon du
    serveur et sa simulation, requêtes partagées par la liste et le pied ; violations
    par opération ; actions du bouton scindé selon les droits et la simulation, refusées
    avec motif ou permission ; action par défaut), `DraftOperations.vue` (opérations avec
    la marque du brouillon, retrait sous `If-Match`, focus rendu au bouton suivant ; retrait
    refusé dit sur place par `problems/` — 404 « ce brouillon n'existe plus » avec code et
    `Request-Id`, puis « aucun brouillon » (le SDK distingue `Gone.cause` : `event` ou
    `refused`, le toast du cadre ne dit que le premier), 412 « a changé ; relu » — ;
    soumissions sans réponse dont le brouillon est parti (`LostSubmissions.vue`) ; violations par opération
    avec leur code, du résultat ; effet simulé, cascades ; suppression annoncée
    `expires_at`), `DraftSubmit.vue` (résultat de la simulation, `CommitForm` gardé dans
    le brouillon, `SplitButton` « Appliquer » / « Soumettre en revue » / « Enregistrer le
    brouillon », Ctrl+Entrée, résumé générique prérempli pour « Appliquer », clé
    d'idempotence d'une soumission sans réponse gardée pour la page au-delà du pied
    (`lostSubmissions`, `submitDraft`), « Réessayer » la rejoue, même si l'événement
    `draft` a fait partir le brouillon ; issue dite par un
    toast, refus sur place ; région d'état présente avant le brouillon lu),
    `DraftsView.vue` (`/drafts` : reprendre, nouveau, supprimer confirmé (focus rendu à
    la grille), actif marqué, suppression annoncée, tri, lignes empilées en conteneur
    étroit), `DraftView.vue` (`/drafts/:id`), `OperationText.vue` + `operations.ts` (une
    opération en ligne). `frame/ShellFrame.vue` (onglet Brouillon et pied du panneau,
    morceaux paresseux, renvoi à la page quand le brouillon actif y est ouvert ; titre
    des brouillons créés par `setDraftTitle` ; brouillon parti par l'événement dit par un
    toast ; `draftHost.onAdd` câblé : le `draft.add` d'un plugin ouvre le brouillon, le
    shell passant lui-même par `addToActiveDraft` de `@kartenn/sdk/host` (édition en
    place, panneau d'édition) ; compteur « À instruire », « n+ » quand une page suit,
    `pageCount`), `frame/spaces.ts` (`badges` de `sidebarGroups`),
    `frame/routes.ts` (fiche `/drafts/:id`), `views/routes.ts` (écrans de l'espace),
    `views/filters.ts` (état des changesets en liste fermée ; `from`, `to` en jours,
    envoyés comme premier et dernier instant du jour, `instantOfDay`), `i18n/index.ts` ;
    `@kartenn/ui` : `SidebarEntry.badge` prend aussi un texte (« 100+ »). Vitest
    `shell/test/changesets.test.ts` (23 : types et permissions, actions de revue selon les
    droits et le présent, l'auteur sous `approval.self`, sans, retenu par une politique,
    différentiel groupé, différentiel compté, opération en ligne, badge, « n+ », dates
    typées ; écrans montés contre une API simulée : validation sous `If-Match`, Valider
    refusé avec sa permission, 412 d'une décision, « Appliquer » ou « Soumettre en
    revue » selon `approval.self`, résumé prérempli, soumission `?approval=self` après le
    motif enregistré, réponse perdue puis même clé, et encore après le départ du
    brouillon, violation sur son opération, retrait, retrait 404 et 412, `draft.add`
    d'un plugin qui ouvre le brouillon et pas `addToActiveDraft`), `sdk/test/drafts.test.ts`
    (404 d'une écriture : `Gone.cause` `refused`). Parcours réécrits
    `web/v2/e2e/journeys/changesets.spec.ts` (proposition de Carol suivie dans « Mes
    changesets », Valider refusé à l'auteur avec `core.address.approval.self` qui lui
    manque, validée par Alice depuis « À instruire »
    comptée dans la barre, annulée ; rejet à motif obligatoire) et `draft.spec.ts`
    (modification en place → compteur → panneau, simulation valide, Ctrl+Entrée avec le
    résumé prérempli ; Carol sans `approval.self` : « Appliquer » refusé avec
    `core.prefix.approval.self`, soumis en revue ; violation `core.prefix.duplicate` sur
    son opération, retirée, valide ; brouillon repris depuis la page, repris dans un autre
    navigateur de la même identité avec ce qui a été écrit dans le premier, supprimé
    là) ; parcours `plugins` étendu
    (action `review` de `notes` dans « … » de la fiche, fermée par `kartenn-close`, focus
    rendu à « … ») ; rôle `redacteur` des parcours avec `core.changeset.read`.
  - choix : actions de revue dans la `Toolbar` de la fiche (Valider principal), « … »
    toujours présent (« Copier l'identifiant », puis les plugins) ; Retirer montré refusé
    à qui n'est pas l'auteur (« Seul l'auteur retire son changeset », maquette E4) ;
    décisions sans optimisme, l'état relu ; Valider et Retirer sans confirmation, Annuler
    confirmé (un nouveau changeset) ; droits d'un relecteur tirés des types que nomment
    les opérations (une opération sans type dit, droits ou module, laissée au serveur) ;
    titre d'un groupe du différentiel : le nom de l'objet si le cache le tient, sinon son
    identifiant (le contrat ne sert que `id`) ; « À instruire » compte la première page de
    `/v1/proposals`, « n+ » au-delà ; dates de soumission (`from`, `to`) choisies en jours
    dans la zone du navigateur ; une revue qu'un module exige n'est pas servie au client :
    le serveur seul la refuse alors à l'auteur ;
    action par défaut gardée pendant la simulation (« Appliquer » attend, maquette E5),
    puis selon droits et résultat, jusqu'au choix de l'utilisateur ; le motif est gardé
    dans le brouillon avant la soumission (le serveur le soumet) ; titre d'un brouillon
    créé : « Brouillon du <date> » ; page `/drafts` sans « Renommer » (non demandé par le
    design) ; état « en revue » : `StatusBadge` `review`, l'œil des maquettes E4/E5.
  - reste : « Instruire dans Flux » à la place de « Valider » sur une demande de flux
    (plugin Flux pas encore porté sur le SDK 1.0), et « Effet sur la politique » de E4
    (plugin Politique) ; « Revue requise » avec le nom de la politique avant la
    soumission (maquette E5) : la simulation (`ChangesetSimulated`) ne dit pas qu'une
    politique retient, seul le 202 le dit (manque de contrat, à arbitrer) ; « Renommer »
    un brouillon (menu de E5) ; résultat de simulation et ligne d'opération recopiés des
    maquettes (`EcranSoumission`, `EcranBrouillon`) : composants de `@kartenn/ui` à
    décider, comme une classe de texte atténué ; badge « À instruire » de `SidebarNav`
    sans unité dite (composant) ; un brouillon supprimé dans un autre navigateur dit dans
    l'onglet qui l'avait actif : par l'événement `draft` (SDK, Vitest) ou par le 404 de
    l'écriture suivante (Vitest), pas tenu par un parcours — sous Chromium l'événement
    n'est pas venu en 20 s (flux : tâche 5), et les parcours des deux navigateurs écrivent
    les brouillons d'Alice côte à côte ; dates d'effet de la soumission
    (période `valid_from`/`valid_to` du brouillon, pied) ; modifier une opération du
    brouillon (seul le retrait est là) ; « Tout afficher » d'une `DiffList` virtualisée
    (question ouverte : le différentiel se lit par « Afficher la suite ») ; captures des
    vrais écrans (comme les 11 et 12 : sans API dans le projet `visual` ; E4 et E5 restent
    les maquettes).
  - à arbitrer par l'utilisateur : l'icône de l'état « en revue » — œil des maquettes
    validées (gardé ici, `StatusBadge`) ou `git-pull-request` du design (« À instruire »
    de la barre latérale le porte déjà) ; `PATCH /v1/drafts/{id}` prend
    `application/json` alors que REQ-006-13 veut `merge-patch+json` « uniquement » pour un
    `PATCH` (amender la 006 ou aligner la route ; le client suit la route, `merge:
    false`) ; le contrat de `GET /v1/changesets` n'énumère pas `state` et ne dit pas le
    format `date-time` de `from`/`to` (le shell les type lui-même, `views/filters.ts`).
  - découpée le 2026-11-11, le reste relevé contre le code (main `2dc9ecc8`). Levés depuis :
    les dates d'effet de la soumission (14d : `draftPeriods`, `periodBody` de
    `draft/state.ts`, `DraftSubmit.vue`) ; `PATCH /v1/drafts/{id}` en `merge-patch+json`
    seulement (14a, l'arbitrage ci-dessus tranché) ; `CommitForm` en lecture seule avec sa
    raison (reste de la 14d : `ui/src/components/CommitForm/CommitForm.vue`, `readonly` et
    `reason`, posés par `DraftSubmit.vue` sous `as_of`) ; captures E4 et E5 : celles des
    maquettes, acceptées comme pour les 11, 12 et 15 (les vrais écrans demandent l'API) ;
    « Instruire dans Flux » et « Effet sur la politique » reviennent à la 017 (tâches 3 et
    19 : action `review-in-flow` ; 2 et 10 : action `effect` sur le type `changeset`). Sous-tâches du
    reste : « Renommer… » (14f, faite le 2026-11-11), opération masquée (14g, faite le 2026-11-11), pied en attente
    de reprise (14h, faite le 2026-11-11), badge de la barre et segments de date (14i, `@kartenn/ui`, faite le 2026-11-11), brouillon
    daté (14j), brouillon supprimé ailleurs tenu par un parcours (14k, faite le 2026-11-11), modifier une
    opération du brouillon (14l, faite le 2026-11-11). Restent à l'utilisateur, inchangés : « Revue requise »
    avant la soumission (contrat), composants du résultat de simulation et de la ligne
    d'opération, « Tout afficher » d'une `DiffList`, icône de l'état « en revue », contrat
    de `GET /v1/changesets` ; de la 14e, l'aperçu daté de la Topographie (une règle à
    poser) et la période par onglet (`sessionStorage`).
  - fait le 2026-11-14, relevé contre le code (main 83dd485f), sans code ajouté : chaque
    clause de `verify` tenue. Parcours réécrits : `web/v2/e2e/journeys/changesets.spec.ts`
    (six tests : proposition de Carol validée depuis « À instruire » puis annulée, rejet
    à motif, refus replié dans « … » en fenêtre étroite, retrait par l'autrice, valeur
    masquée, violations par règle) et `draft.spec.ts` (neuf tests : Ctrl+Entrée et
    résumé prérempli, « Appliquer » refusé sans `approval.self`, violation retirée puis
    brouillon repris et supprimé ailleurs, période sous `valid_at`, réponse perdue et
    même clé, lecture seule sous `as_of`, date d'ajout, « Renommer… », opération
    modifiée). Actions des plugins sur le type `changeset` : `useExtensions("action",
    "changeset")` de `shell/src/changesets/ChangesetView.vue`, parcours `plugins`
    « an action on the type changeset… ». Captures E4 et E5 : celles des maquettes
    (`E4Changeset.vue`, `E5Brouillon.vue`, `e2e/visual/__screenshots__/ecrans.spec.ts/E4-*`
    et `E5-*`), acceptées le 2026-11-11 ; celles des vrais écrans relèvent de la 12l (non
    placée). `web/v2/PARITE.md` : toutes les lignes « Changesets et brouillon » cochées ;
    la phrase des plugins Politique et Flux, qui disait encore « reste de la tâche 14 »,
    renvoie aux tâches 3, 19 et 2, 10 de la 017. Vitest `shell/test/changesets.test.ts`,
    `draft-replace.test.ts`, `sdk/test/drafts.test.ts`, `write.test.ts` : 80 sur 80 ;
    parcours `changesets`, `draft`, `plugins` : 52 sur 52 dans les deux navigateurs.
  - choix : aucun code (les sous-tâches 14a, 14c à 14l ont levé le reste de `verify`) ;
    l'icône de l'état « en revue » et `PATCH /v1/drafts/{id}`, listés plus haut à
    arbitrer, sont tranchés par A16 et A17 (`decisions.md`).
  - reste (hors de `verify`, à l'utilisateur, inchangé) : « Revue requise » avant la
    soumission (`ChangesetSimulated` ne dit pas qu'une politique retient), composants du
    résultat de simulation et de la ligne d'opération, « Tout afficher » d'une
    `DiffList`, contrat de `GET /v1/changesets` (`state` sans énumération, `from`/`to`
    sans `date-time`) ; de la 14e, l'aperçu daté de la Topographie et la période par
    onglet.
- [x] 15. Administration : groupes (membres, liaisons), rôles, émetteurs, comptes de
  service et secret, identités (groupes et droits effectifs en lecture), audit et
  export, imports ; permissions en nom servi et libellé — REQ-021-12, 025/REQ-025-12 —
  verify : parcours `admin` réécrit ; capture E7
  - où : `web/shell/src/admin/*`, `views/AdminView.vue` ; maquette
    `web/v2/shell/src/_ui/ecrans/E7Administration.vue` (vues `groupes`,
    `groupe-edition`, `groupe-refus`, `secours`, `identite`), design 021 § Administration.
  - dépend de : 13
  - découpée le 2026-11-01 en 15a, 15b et 15c (fin de P2)
  - faite le 2026-11-02 par ses trois sous-tâches, fusionnées
- [x] 16. Hôte des plugins et session : emplacements dans le nouveau cadre, point `widget`
  de l'Accueil, actions sur le type `changeset`, onglets sur les types de module, frontière
  d'erreur avec code, entrées par espace, « À propos » ; session (reconnexion, échéance
  signalée) — REQ-021-13, 14 — verify : parcours `plugins`, `session`, `example`
  - où : `web/shell/src/plugins/{ExtensionElement.vue,PluginView.vue,registry.ts}`,
    `session/*`, `views/{AboutView,LoginView}.vue` ; repris de la tâche 8 : le chargeur
    écarte `incompatible` par `unmetRange` (`@kartenn/sdk/host`), `handle.styles()` lie la
    feuille de `@kartenn/ui` ; repris de la tâche 7 : `meta.modules` de `/x/:plugin/…`, le
    chargeur attend les mêmes `preloadModules` (`web/v2/shell/src/plugins/preload.ts`),
    entrées des plugins préchargées au repos (`requestIdleCallback`) ; repris de la
    tâche 10 : la déconnexion appelle `forgetRecent()` (`web/v2/shell/src/shortcuts/recent.ts`,
    objets récents de tous les utilisateurs du navigateur) ; les lettres `g` des manifestes
    (`m`, `t`, `q`, `f` : machines, matrice, conformité, flux) s'enregistrent au registre du
    cadre (`createShortcutRegistry`, `ShellFrame.vue`, `navigate` vers `/x/<plugin>/…`,
    `shown` par droits), une lettre prise refusée et le plugin signalé ; `reserveKeys`
    (`SHELL_KEYS`, posé dans `main.ts` avant le montage) reste avant tout chargement de
    plugin
  - dépend de : 9
  - en cours : fait le 2026-11-01 — serveur : point `widget` (`Point::Widget`,
    `crates/kartenn-rest/src/web/plugins.rs` : sans `object_types` ni `go`, un au plus
    par plugin ; contrôles des extensions dans `check_extensions`), contrat et
    `schema.d.ts` (web/sdk, web/v2/sdk) régénérés, 007 (design, règles de validation) et
    `docs/spec/interfaces.md` amendés ; `tests/it/plugins.rs` (trois manifestes fautifs,
    tuile publiée). SDK : `ExtensionPoint` admet `widget`. Shell (`web/v2/shell/src/`) :
    `plugins/registry.ts` (état des plugins, `extensionsOf` par point, type — `changeset`
    et types de modules compris — et permission, `widgetsOf`, codes `plugin.<motif>`,
    version du serveur, lettres refusées), `plugins/loader.ts` (capacités relues sur
    429/503, `unmetRange` sur SDK, `@kartenn/ui` et `vue`, catalogues, modules par les
    `preloadModules` du préchargement, entrée, `definePlugin` en 5 s ; `beginPlugins` une
    fois par session, au repos par `requestIdleCallback`), `plugins/navigation.ts` (vues
    sous l'espace de leur manifeste, espaces `hosts`, `policy`, `flows` ajoutés à leur
    place ; lettres `g` au registre du cadre, une lettre prise refusée, le plugin signalé
    dans « À propos » et chargé), `plugins/{ExtensionElement,PluginView,PluginAction}.vue`
    (élément dans sa place, erreur confinée en encart avec code, squelette au
    chargement), `plugins/slots.ts` (onglets, panneaux, colonnes, actions, tuiles pour les
    écrans), route `/x/:plugin/:view/*` (`meta.modules`, fil d'Ariane sous l'espace),
    `frame/AboutView.vue`, `session/` (`index.ts` : retour, émetteur, `clearSensitive` qui
    appelle `forgetRecent`, `logout` ; `expiry.ts` ; `LoginView.vue` ; `SessionDialog.vue`),
    garde de session (`/login?return=`, `meta.public`), menu du compte (À propos, Se
    déconnecter), avis de fin de session ; `i18n/index.ts` (`mergePluginCatalog`,
    `shellKey`). Vitest : `shell/test/plugins.test.ts` (21), `session.test.ts` (21),
    `frame.test.ts` adapté ; parcours `plugins`, `session`, `example` verts (22, Chromium
    et Firefox). Parcours `plugins`, `session`, `example` réécrits sous
    `web/v2/e2e/journeys/` ; plugins de test sous `web/v2/e2e/plugins/` (`notes` : une
    extension de chaque point, `g m`, `g p` refusée, une vue qui lève, une vue
    d'administrateur ; `incompatible` en `ui ^3`, `throws`, `silent`, `forged`), réunis par
    `serve.sh` dans `web/e2e/.state/plugins-v2`.
  - reste, faute d'écran : tuiles sur l'Accueil, onglets, panneaux et colonnes (tâche 11),
    actions sur le type `changeset` (tâche 14), onglets et colonnes sur les types de module
    (écrans VLAN, tâche 17), avec leurs parcours (pointeurs écrits dans leurs lignes
    `où :`) ; parcours `example` complet avec le plugin d'exemple au SDK 1.0
    (tâche 19 : il ne déclare encore aucune extension, le parcours vérifie qu'il charge).
    Ouverts après relecture : connexion et « À propos » hors session gardent tout le cadre
    (barre latérale vide, pastille de date, compteur du brouillon) faute de maquette ; le
    style du titre de page est recopié dans cinq classes locales (`k-login__title`,
    `k-about__title`, `k-plugin-view__title`, `k-pending__title`, aide), une pièce de
    `@kartenn/ui` à décider ; focus rendu par `PluginAction` à vérifier au clavier quand la
    tâche 14 l'aura branché.
  - à arbitrer par l'utilisateur (limite, non corrigée) : la fin absolue d'une session
    active n'est pas annoncée d'avance. `SessionState` ne sert que `expires_at` = la plus
    proche des fins d'inactivité et absolue, et chaque requête repousse la première : la
    fin absolue n'apparaît qu'à une lecture où elle est devenue la plus proche (« Rester
    connecté », ou la lecture de `/v1/me` au chargement). Sonder `/v1/me` périodiquement
    annulerait la durée d'inactivité (refusé). Option : exposer la fin absolue dans
    `SessionState` (champ additif, par exemple `absolute_expires_at`, changement de contrat
    de la 005/006) ; le shell l'annoncerait alors 2 min avant, sans l'action.
  - à arbitrer par l'utilisateur (relevé par la 11, 2026-11-01) : côté client, seules les
    lectures nées d'un geste de l'utilisateur comptent comme activité (`activityFetch`,
    `installGestures`) ; côté serveur, toute requête sauf le flux prolonge encore `seen`
    (`oidc.rs`, seules les réponses du flux portent `NotActivity`) : un écran qui relit en
    fond (événement `stats`, `changes`, `labels`, revalidation au focus) garde la session
    ouverte sans geste, et la fin dite par le shell peut ne pas venir. Rejoint 022 R17/A6
    (le flux n'est pas une activité). Option : un marqueur de requête de fond posé par le
    client (en-tête), que `authn` traite comme `NotActivity` — changement de contrat et de
    sécurité de la 005/006.
  - choix : codes du shell `plugin.<motif>` (`incompatible`, `load-failed`, `exception`,
    `timeout`, `element`), jamais de l'API ; catalogues des plugins sous `plugin.<nom>`
    (un plugin nommé `app` ne réécrit pas les mots du shell), clés du `t` des plugins
    routées par `shellKey` ; vues d'un plugin sans `space` dans le référentiel, après
    celles du cœur, icône `plug` ; un plugin écarté ou en chargement ne place ni entrée ni
    lettre, une tuile oui (squelette, encart) ; lettres enregistrées dans l'ordre de
    `ui_plugins`, de nouveau à chaque changement ; plugins chargés une fois connecté
    (les capacités demandent une session) ; une erreur de la fenêtre pendant la création
    d'un élément est la sienne ; connexion et « À propos » dans le cadre, ouvertes sans
    session ; émetteurs et compte de secours lus hors du client du SDK, un 401 hors
    session n'ouvrant pas « Session expirée » ; la déconnexion garde langue, réglages
    d'affichage et de clavier (thème, densité, mouvement, touches seules, barre, panneau)
    et dernier émetteur, efface le reste de `kartenn.*` ; fin de session dite 2 min avant
    par un toast `warn` avec « Rester connecté » (relit `/v1/me`), activité = réponses de
    `/v1/*` hors `/v1/events`, partagée entre onglets (`kartenn.session.activity`), fin lue
    sur l'horloge du serveur (`Date`) ; « Rester connecté » relit la fin hors du client du
    SDK (sa réponse n'est pas une activité qui effacerait l'avertissement) et la donne au
    suivi : une fin qui a bougé efface l'avertissement, une fin inchangée est fixe et reste
    dite sans l'action (seulement quand une lecture la montre : voir la limite ci-dessus),
    une 401 ouvre « Session expirée », un autre échec garde la session et le dit avec son
    code ; activité d'un autre onglet suivie par l'événement `storage` ; le `return` de
    l'URL de connexion l'emporte sur un retour gardé plus ancien (`returnAfterLogin`) ;
    `shellKey` ne route vers `plugin.<nom>` que les clés traduites pour ce plugin (le
    traducteur du SDK reçoit le plugin), jamais `labels.*` ; aucune
    touche réservée aux toasts ; un libellé d'extension non encore traduit (plugin en
    chargement ou écarté) dit « plugin · id », jamais la clé ; premier émetteur en bouton
    principal, les autres et le compte de secours en secondaire ; « Réessayer » rend le
    focus au titre de sa section.
  - fait le 2026-11-11 (clôture relevée contre le code) : le reste « faute d'écran » est
    placé et prouvé — tuile de l'Accueil (`HomeView`, `useWidgets`), onglet et panneau de la
    fiche et menu d'actions (`RecordView`), colonnes des listes (`ListView`), actions sur
    `changeset` (`ChangesetView`), onglet sur `vlan` (fiche générique) : parcours `plugins`
    (« the screens of the referential place the tile, the tab, the panel and the column »,
    « an action on the type changeset… ») ; plugin d'exemple au SDK 1.0 placé partout
    (`example` : « the screens of the referential place its tile, column, tab, panel and
    action », tâche 19) ; les deux arbitrages ci-dessus tranchés et livrés par 16a à 16f
    (`absolute_expires_at`, `Kartenn-Background`), titres recopiés par 16g, « À propos » et
    action absente par 16h ; repris des tâches 7, 8 et 10 présents (`meta.modules`,
    `preloadModules`, `requestIdleCallback`, `unmetRange`, `forgetRecent`). Ajouté : focus
    de `PluginAction` au clavier seul (parcours `plugins`, « an action of a plugin at the
    keyboard alone… » : « … » à Entrée, l'entrée aux flèches, Tab jusqu'au bouton de
    l'élément, Entrée puis Échap ferment, le focus revient à « … »), sous Chromium et Firefox.
    Parcours `plugins`, `session`, `example` verts.
  - choix : la vérification au clavier passe par le dialogue d'une action sur `changeset`
    (le seul où une action de plugin est livrée avec un élément qui se ferme lui-même) ;
    Tab borné à six pressions, le bouton de l'élément visé dans l'élément (la croix du
    dialogue porte le même nom).
  - reste (faute de maquette, à trancher avec la 020) : connexion et « À propos » hors
    session gardent tout le cadre (barre latérale vide, pastille de date, compteur du
    brouillon).

### Sous-tâches shell des arbitrages du 2026-11-01

- [x] 11b. Topographie : colonne d'occupation tirée de `TreeNode.occupancy`, « + » sur un
  nœud ouvrant le panneau d'édition de la sorte (tâche 12) — REQ-021-9, 18 — verify :
  Vitest de la colonne ; parcours `tree` (occupation affichée sans lecture par ligne)
  - où : `web/v2/shell/src/views/TopologyView.vue`, `topology.ts` ; `views/edit/` ;
    parcours `web/v2/e2e/journeys/tree.spec.ts`
  - dépend de : 11a
  - fait le 2026-11-01 — `TopologyView.vue` : colonne « Occupation » (`sm`, alignée à la
    fin) juste après le préfixe, comme la maquette E1, tirée de `TreeNode.occupancy` de
    chaque page de `roots`/`children` (aucune lecture par ligne) ; vide pour une plage ou
    une adresse. Le pourcentage sort de `record/OccupancyPanel.vue` vers
    `views/occupancy.ts` (`shareOf`, `whole`), partagé par la fiche et l'arbre : même
    texte des deux côtés. Le « + » d'un préfixe (livré par la 12) ouvre le panneau de la
    sorte : adresses et plages pour un réseau, préfixes pour un conteneur, domaine
    prérempli, « Ajouter dans <préfixe> » en titre. Tests : `views.test.ts` (arbre monté,
    cellules d'occupation d'un réseau, d'un conteneur IPv6, d'une plage ; aucune requête
    `…/occupancy` ni lecture d'un préfixe ; `shareOf`) ; parcours `tree` (nouveau cas,
    alice : occupation de trois nœuds dont un enfant ouvert, aucune requête
    `…/occupancy` ; « + » du conteneur au clavier → panneau « Préfixe », domaine, contrôle axe,
    focus rendu au « + » par « Annuler ») vert, 6 passés.
  - choix : un nœud ouvert seul par l'URL (`Shown`, préfixe lu par `GET /v1/prefixes/{id}`,
    sans `occupancy`) n'affiche son occupation nulle part et ne la lit pas : il n'est dans
    l'arbre qu'une fois son niveau lu, et le panneau du contenu (maquette E1) ne montre pas
    d'occupation ; `…/ancestors` n'est pas lu pour elle. Colonne « Nature » de la 11
    gardée, absente de la maquette E1, en priorité 2 (masquée sous un conteneur `md`,
    sélecteur de colonnes). Format du pourcentage inchangé (« 71 % », arrondi vers le bas,
    « < 1 % »), celui de la fiche.
  - reste : Entrée sur un préfixe entre dans son « + » pour qui peut ajouter, la fiche
    n'est alors plus à Entrée (lien « Ouvrir » du panneau du contenu) — compromis de la 12,
    relevé par l'audit d'accessibilité, à trancher (autre touche pour la fiche ou pour le
    « + ») ; « < 1 % » lu selon le lecteur d'écran (« moins de 1 % » plus sûr, à changer
    avec la fiche) ; cellule vide d'une plage ou d'une adresse sans mot ; « + » refusé
    visible (12a ; le `TreeGrid` n'a ni état refusé pour le « + » ni colonne d'élément,
    non modifié).
- [x] 12a. Édition en place dans l'arbre de la Topographie et la grille du contenu d'un
  nœud, `expected` et `If-Match` tirés de `TreeNode.version` ; « + » refusé visible avec
  sa permission sur un nœud — REQ-021-15, 37 — verify : parcours `edit` (cellule de
  l'arbre → brouillon, Ctrl+Entrée → 201 ; deux onglets → 412 sans perte)
  - où : `web/v2/shell/src/views/TopologyView.vue`, `views/edit/` (`fields.ts`) ;
    parcours `web/v2/e2e/journeys/edit.spec.ts`
  - dépend de : 11a, 11b, 11c, 11d
  - amendée le 2026-11-02 (A20) : Entrée sur un nœud ouvre la fiche ; le « + » se joint
    par Tab dans la ligne ou par `c`
  - fait le 2026-11-02 — `views/edit/inline.ts` : `useInlineEdit` prend des lignes de
    plusieurs sortes (`collection(row)`, `field(row, colonne)`), les mots d'une liste
    fermée tapés valent sa clé (`words`), la relecture après un 412 est à l'hôte
    (`reread`), `whyWords` (le pourquoi en une phrase, repris par `ListView`) ;
    `views/edit/nodes.ts` (`useNodeEdit`, `useNodeWords`) : la colonne `detail` d'un nœud
    édite la nature d'un préfixe, l'usage d'une plage ; `TopologyView.vue` : cellules de
    l'arbre éditables (`editable`, `validate`, `cellState`, `commit`), `expected` et
    `If-Match` = la `version` servie par la page du nœud, après un 412 la version servie
    par la relecture de l'objet avec sa valeur (11d) et le niveau qui le tient relu ; pied
    de l'arbre : ligne du motif (`QuickApply`), ce que font Entrée et Ctrl+Entrée, ou
    pourquoi la cellule de la focalisation n'a pas d'éditeur avec sa permission ; Entrée sur
    un nœud ouvre la fiche, Tab entre dans son « + » (A20), `c` ajoute dans le nœud de la
    focalisation, sinon le choisi ; « Ajouter dans <préfixe> » du panneau du contenu
    (`Toolbar`, `c`) visible refusé avec sa permission (`core.address.edit` pour un réseau,
    `core.prefix.edit` pour un conteneur) ou hors présent ; `record/ContentTab.vue` : même
    édition dans la grille du contenu (fiche et Topographie), ligne du motif sous la
    grille, indice des touches ou pourquoi sous elle ; `edit/EditHint.vue` : indice d'une
    cellule (Entrée, Ctrl+Entrée ou la permission qui lui manque, Échap) ou pourquoi elle
    n'a pas d'éditeur, partagé par `ListView`, l'arbre et le contenu ; le pourquoi d'une
    cellule de l'arbre est aussi dit par le statut de la vue quand la focalisation y vient
    (le `TreeGrid` n'a pas de description de cellule). Tests : `test/edit.test.ts` (2 cas :
    nature et usage par `detail`, mots → clé,
    `expected` par sorte ; Ctrl+Entrée `If-Match` = version servie, 412 → niveau relu,
    version de la relecture) ; parcours `edit` (lecteur : pourquoi au pied, « Ajouter »
    refusé avec `core.address.edit` ; auteur : nature au clavier → brouillon ; Entrée sur
    le nœud → fiche ; deux onglets : Ctrl+Entrée → 201, second → 412, saisie gardée, puis
    appliquée ; panneau d'ajout par `c`) et `tree` (Tab → « + », Échap, Entrée → fiche).
  - choix : la valeur d'un nœud (colonne de l'arbre, valeur de la grille du contenu) n'est
    pas éditable en place : Entrée l'ouvre (A20), `e` ouvre le panneau d'édition ; une
    adresse n'a pas de détail dans l'arbre (`TreeNode` sans usage), donc rien d'éditable ;
    le mot affiché (« Réseau ») est le texte de départ de l'éditeur de l'arbre (le
    `TreeGrid` ne prend que des chaînes), d'où les mots acceptés pour la clé, et la liste
    des choix dite en mots ; pied de l'arbre sur la cellule de la focalisation (modèle
    `active` du `TreeGrid`, gardée à la sortie comme au pied d'une liste) ; une
    modification appliquée répond 200 (REQ-021-15 ; le « 201 » du verify est le code d'une
    création) ; la colonne « Nature, usage » étant de priorité 2 (A21), masquée dans la
    largeur de l'arbre, le parcours l'affiche par le sélecteur « Colonnes » ; le raccourci
    `c` n'est montré sur « Ajouter dans » du contenu que s'il agit sur ce nœud.
  - corrections de la revue (2026-11-02, une passe) : `TreeGrid` (web/v2/ui) porte A20 —
    Entrée sur la cellule du nœud active la ligne, Tab (ou F2) entre dans son « + », d'où
    Tab sort de l'arbre — et le « + » refusé visible (`addBlocked` : permission ou motif,
    `aria-disabled`, infobulle et description, REQ-021-37), la cellule de la focalisation
    en modèle `active` ; l'interception en capture de la vue est retirée, la Topographie
    passe `addBlocked` (« + » de chaque préfixe, refusé avec `core.address.edit` ou
    `core.prefix.edit`, ou hors présent) ; catalogue `add-refused`, Vitest
    `trees-mounted.test.ts` (Entrée, Tab, refus), script clavier `tree-grid.spec.ts`
    (Tab, Maj+Tab, « + » refusé) ; `nodeApplyRight` remplace le doublon des types dans
    `ContentTab` ; Vitest de la grille du contenu (`edit.test.ts` : pourquoi d'une cellule,
    ligne en brouillon, ligne du motif, 412 relu avec la saisie gardée) ; parcours `edit` :
    deux onglets, l'arbre et la grille du contenu, deux valeurs différentes, le second
    reçoit 412, garde sa saisie à côté de la valeur relue, l'applique, focus rendu à sa
    cellule ; lecteur : « + » de l'arbre et « Ajouter » du contenu refusés avec
    `core.address.edit`.
  - reste : le `TreeGrid` n'a pas encore de `readonlyReason` (description d'une cellule
    sans éditeur : la vue le dit par son statut et son pied) ni d'emplacement de cellule ou
    d'éditeur (texte de départ = texte affiché). L'arbre et la grille du contenu ont chacun
    leur `useNodeEdit` (états de cellule et ligne du motif séparés pour un même nœud) ;
    `kindOf` de `nodes.ts` lit `kind` sans garde de type ; 202, 422 et 403 ne sont prouvés
    que pour les listes (`edit.test.ts`), pas dans l'arbre. Relevé par les revues du
    `TreeGrid` : le « + » refusé ne se distingue de l'autre qu'au survol ou au focus
    (infobulle), et, comme lui, ne paraît que sur la ligne choisie, survolée ou focalisée ;
    rien ne dit sur la cellule du nœud que Tab mène au « + » ; le raccourci `c` du bouton
    « Ajouter dans » du contenu suit la cellule de la focalisation ; la maquette E1 n'a pas
    de « + » refusé. La seule colonne éditable de
    l'arbre est masquée par défaut (priorité 2, A21) : à revoir avec la maquette E1 (relevé par la revue). `c` refusé
    (droit, domaine absent) ne dit rien : `useViewAction` n'appelle pas l'action refusée,
    comportement du cadre pour toutes les vues (le bouton « Ajouter » visible dit la
    permission). Le pied de l'arbre apparaît à la première focalisation d'une cellule qui
    a quelque chose à dire (pas de pied permanent dans la maquette E1) ; focus rendu à la
    cellule après la ligne du motif sans repli si la ligne a disparu à la relecture.
- [x] 15a. Administration : groupes (membres, liaisons) et rôles ; permissions en nom servi
  et libellé — REQ-021-12, REQ-025-12 — verify : parcours `admin` (groupes, rôles)
  - où : `web/shell/src/admin/{BindingsView,RolesView}.vue`, `admin.ts`, `write.ts`,
    `WriteOutcome.vue` (comportement) ; maquette
    `web/v2/shell/src/_ui/ecrans/E7Administration.vue` (vues `groupes`,
    `groupe-edition`, `groupe-refus`) ; parcours `web/e2e/journeys/admin.spec.ts` porté
    sous `web/v2/e2e/journeys/admin.spec.ts` ; design 021 § Administration
  - dépend de : 13
  - fait le 2026-11-01 : `web/v2/shell/src/admin/` — `routes.ts` (`adminScreen`,
    `adminRecord`, `ADMIN_RECORDS` : la structure que reprennent 15b et 15c), `GroupsView`,
    `GroupDetail`, `MemberDialog`, `BindingDialog`, `GroupForm`, `RolesView`,
    `RoleDetail`, `RoleForm`, `AdminLines` (lignes alignées du panneau, marques du
    brouillon), `model.ts` (lignes marquées, refus, opérations), `draft.ts` (ajout,
    retrait, remplacement dans le brouillon actif), `reads.ts`, `i18n/` ; cadre :
    `meta.detail` d'une route (`views/routes.ts`, `frame/routes.ts`) montré dans l'onglet
    Détail (`ShellFrame.vue`), le panneau ouvert sur l'objet choisi ;
    `web/v2/PARITE.md` créée (section Administration) ; Vitest `test/admin.test.ts` ;
    parcours `web/v2/e2e/journeys/admin.spec.ts` (groupe : membre retiré puis rétabli,
    rôle ajouté, brouillon appliqué ; `secours` en lecture seule annoncée ; rôle et ses
    permissions servies ; lecteur refusé).
  - choix : l'objet choisi s'ouvre par l'URL (`/admin/groups/:id` par UUID,
    `/admin/roles/:key`), même écran que la liste, son détail dans le panneau droit ; le
    périmètre plus large que celui de l'utilisateur se lit dans des simulations
    (`?simulate=true`) séparées, qui ne se gênent pas : `close_group` seul
    (`core.group.escalation`), puis une par liaison, liée de nouveau telle quelle (`bind` ;
    `core.binding.escalation`, ou `core.rbac.forbidden` hors du périmètre administré :
    seuls ces codes comptent ; une par liaison parce que les règles des droits jugent
    l'état final et ne nomment aucune opération) ; une sonde par ouverture de groupe, en
    parallèle, gardée cinq minutes, jamais relue au focus ni à la reconnexion ; demandée
    seulement à qui n'a pas `core.rbac.admin` global : le
    client ne devine aucune inclusion de périmètres ; « Ajouter un rôle » refusé avec le
    groupe, comme la maquette ; les membres de `secours` à un administrateur global
    seulement ; « Modifier » une liaison = `unbind` puis `bind` ; une permission ajoutée
    ou retirée réécrit le `set_role` du brouillon (remplacé, ou retiré s'il revient à
    l'état servi) ; permissions proposées d'après l'espace `permission` du dictionnaire
    des libellés, un motif s'écrit tel quel ; objet d'un périmètre choisi parmi la
    première page de sa liste ; pas de colonne Description (le contrat n'en sert pas) ;
    clés validées comme le serveur (groupe `[a-z][a-z0-9_-]*`, rôle `[a-z][a-z0-9_]*`).
  - reste : un champ de texte multiligne manque à `@kartenn/ui` (permissions d'un rôle
    saisies séparées par des virgules) ; `AdminLines` candidat à la bibliothèque (lignes
    alignées avec actions et marques du brouillon) ; l'objet d'un périmètre se cherche
    dans la première page seulement (une recherche par le serveur viendra avec un filtre
    de nom) ; la portée affichée d'un rôle dans « Groupes liés » montre l'identifiant de
    l'objet, pas son nom ; `writeRole` (`draft.ts`) compare les permissions sans
    l'ordre mais les libellés par `JSON.stringify` (ordre des langues) ; classes de
    `AdminLines` (`__label`, `__value`…) posées par les vues, sans gabarit ; `id` fixes des
    titres de section (`k-admin-members`…), uniques tant qu'un seul détail est monté.
  - correctifs de revue (2026-11-01) : sonde de portée en simulations séparées (la première
    version, `close_group` puis `unbind`, finissait en `changeset.operation.no_version`
    avant les règles des droits) ; parcours `admin` : Dana, administratrice de la seule
    entité du jeu (`web/e2e/users.toml`, groupe `admins-entite` de `global-setup.ts` :
    `core.rbac.read` global, `administrator` sur l'entité), un groupe de son entité
    modifiable, un groupe lié au-delà en lecture seule annoncée ; même objet choisi de
    nouveau : le panneau se rouvre (`frame/detail.ts`, `revealDetail`).
- [x] 15b. Administration : émetteurs, comptes de service et secret (montré une fois),
  identités (groupes et droits effectifs en lecture), secours — REQ-021-12 — verify :
  parcours `admin` étendu (secret montré une fois, identité en lecture)
  - où : `web/shell/src/admin/{IssuersView,ServiceAccountsView,ServiceAccountView,
    SecretDialog,IdentitiesView,IdentityView}.vue` ; maquette E7 (vues `secours`,
    `identite`) ; parcours de la 15a
  - dépend de : 15a
  - fait le 2026-11-01 : `web/v2/shell/src/admin/` — `IdentitiesView`, `IdentityDetail`,
    `AccountsView`, `AccountDetail`, `SecretDialog`, `IssuersView`, `access.ts` (permissions
    globales, lignes marquées par le brouillon : état d'une identité, secrets, rattachements ;
    groupes de l'IdP de la dernière connexion ; refus du serveur), `secret.ts`
    (`useSecretWrite`), `reads.ts` (`useScopeNames`, repris par `GroupDetail`), `routes.ts`
    (`/admin/identities/:id`, `/admin/accounts/:id`, `/admin/issuers`), `i18n/` ;
    `web/v2/PARITE.md` ; Vitest `test/access.test.ts` ; parcours `admin` étendu (compte
    créé, secret montré une fois puis nulle part — page, stockages, brouillon —, nouveau
    secret, Échap demande avant de l'oublier, secret révoqué au brouillon puis rétabli ;
    compte de secours `e2e` membre direct de `secours`, ses droits en lecture ; émetteur
    détaché au brouillon puis gardé ; Dana lit son identité, ses groupes et droits, ne gère
    aucun compte).
  - choix : création d'un compte et nouveau secret écrits aussitôt (`POST` dédiés,
    `?approval=self`), hors brouillon : le serveur seul fait le secret, l'opération
    `create_service_account` porte son empreinte ; la réponse n'entre pas dans le cache
    (pas une requête : les listes sont relues sans elle), le secret vit dans le dialogue et
    s'oublie à sa fermeture ; Échap, la croix ou un clic dehors demandent confirmation ;
    le reste au brouillon (`set_identity_state`, `revoke_service_secret`, `attach_issuer`,
    `detach_issuer`) ; pseudonymisation aussitôt, confirmée, avec motif ; permissions de
    l'accès tenues globales (`me.global_permissions`, comme 005 les déclare) ; « secours »
    lu comme le compte de secours (sorte `breakglass`) et son groupe, le groupe livré
    étant fait par la 15a ; émetteurs en liste seule, « Détacher » agit sur la ligne
    choisie ; secret nommé par sa création à la minute (la clé de l'anneau est commune) ;
    l'état d'un compte se gère depuis son identité (même identifiant).
  - correctif de revue (2026-11-01) : pendant l'écriture d'un secret, son dialogue ne se
    ferme pas (Annuler inactif, Échap et la croix retenus : le secret serait venu à un
    dialogue fermé) ; le composant parti quand même (route quittée), la réponse est
    oubliée à son arrivée et un avis dit qu'un secret a été créé sans être montré
    (`createSecretWrite`, Vitest `test/access.test.ts`) ; exceptions au brouillon écrites
    au design ; `useQueryClient` du shell plutôt que celui du SDK.
  - reste : la pseudonymisation n'a ni parcours (elle consommerait une personne du jeu)
    ni Vitest ; `taken` (noms de comptes, émetteurs déjà rattachés) limité aux pages
    lues ; `useScopeNames` relit un nom dont la lecture a échoué à chaque changement des
    périmètres ; `@kartenn/ui` : `Dialog` sans mode qui retienne sa fermeture (garde faite par
    le shell, `AlertDialog` imbriqué), `CodeBadge` sans `aria-describedby` (l'avis « plus
    jamais affiché » n'est pas lu au focus : le nom du code le dit), `DataGrid` sans
    actions de ligne (d'où « Détacher » dans la barre, son nom ne dit pas l'émetteur
    choisi) ; `AdminLines` sans `role="list"` explicite (VoiceOver) ; l'objet d'un
    périmètre (entité d'un rattachement) choisi parmi les premières pages. Vérifié : la
    table d'idempotence ne garde aucun corps, un rejeu rend `shown: None`
    (`crates/kartenn-rest/src/changesets.rs`).
- [x] 15c. Administration : journal d'audit et export, imports ; capture E7 —
  REQ-021-12 — verify : parcours `admin` étendu (filtre du journal, export, import
  suivi) ; capture E7
  - où : `web/shell/src/admin/{AuditView,ImportsView,ImportView,PagedTable}.vue` ;
    maquette E7 ; parcours de la 15a
  - dépend de : 15a
  - fait le 2026-11-02 : `web/v2/shell/src/admin/` — `AuditView`, `AuditDetail`,
    `audit.ts` (événement, `cachedEvent`, `exportAudit` : corps NDJSON versé dans un
    fichier du stockage privé de l'origine au fil de sa réception, puis téléchargé),
    `auditWords.ts`, `ImportsView`, `ImportDetail`, `imports.ts`, `routes.ts`
    (`/admin/audit/:id`, `/imports`, `/imports/:id` dans `ADMIN_RECORDS`), `i18n/` ;
    `views/list.ts` (`useFilteredList` : options `dated`, `field`, filtres envoyés `sent`),
    `views/filters.ts` (période de l'audit en jours, acteur parmi les identités) ; design
    021 § Administration ; `web/v2/PARITE.md` ; Vitest `test/audit.test.ts` ; parcours
    `admin` étendu (journal filtré par l'en-tête de sa colonne, événement dans le panneau,
    même recherche exportée et relue ligne à ligne ; import en pause, repris, en pause de
    nouveau, annulé après confirmation ; import au tri impossible, sa violation par rang).
    Capture E7 : celle de la maquette, gardée, acceptée telle quelle (comme pour les 11, 12
    et 14 : les vrais écrans demandent l'API, absente du projet `visual`).
  - choix : l'export ne passe pas par le SDK (son client lit du JSON) : `fetch` dans
    `audit.ts`, `Accept: application/x-ndjson`, le corps versé par `pipeTo` dans un fichier
    d'OPFS (`kartenn-exports/`, vidé au début de chaque export), remis par un lien
    `blob:` sur ce fichier, effacé une minute après ; sans OPFS (fenêtre privée), le blob du
    navigateur (qu'il garde sur disque passé une taille) ; un corps coupé ne laisse aucun
    fichier ; pas de `showSaveFilePicker` (absent de Firefox). L'événement choisi est
    cherché dans les pages du journal que tient le cache (aucune route ne sert un
    événement seul) : hors d'elles, un état vide le dit. Le journal se lit au présent,
    quelles que soient les dates de la page ; période en jours (`from` au début du jour,
    `to` à sa fin, comme les changesets) ; acteur choisi parmi la première page des
    identités, son nom affiché quand le lecteur a `core.rbac.read` global, son identifiant
    sinon. Les imports, entrée de l'espace `changes`, suivent la structure de
    l'administration (liste, objet dans le panneau droit) ; état relu chaque seconde
    (`refetchInterval`) tant que la tâche y travaille, la liste relue quand l'état change ;
    « Reprendre » et « Annuler l'import » écrits aussitôt (`api.request`, sans corps),
    l'annulation confirmée ; ni création ni dépôt de morceaux dans le shell (l'ancien n'en
    avait pas : un import vient d'un outil, par l'API). Faits d'un objet (`k-admin-facts`,
    `AdminLines.vue`) : la valeur à gauche après son nom, coupée n'importe où (un agent
    utilisateur), au lieu de la valeur à droite des autres panneaux ; sévérité en mots
    (`INFO`, `WARN`, `ERROR`), son code sinon ; vingt objets d'un événement montrés, le
    reste dit (« dans l'export »).
  - relectures (2026-11-02) : `a11y-auditor` — focus remis au titre de l'import quand son
    action disparaît (reprise, annulation) ; une relecture en échec garde l'import affiché
    sous un avis au lieu de le remplacer ; sections sans nom en doublon de leur liste ;
    cellule vide retirée. `ui-reviewer` — colonnes de l'audit (acteur en priorité 2, objets
    en 3), « Éléments » des imports en priorité 2, JSON coupé à la ligne, sévérité
    traduite, mise en page des faits (les noms chevauchaient les valeurs longues).
  - correctifs de revue (2026-11-02) : sans `core.rbac.read` global, aucune lecture de
    `/v1/identities` (le refus serait journalisé `denied` dans le journal lu) : le filtre
    « Acteur » prend un identifiant collé (`useFilteredList`, option `refine`), le panneau
    montre l'acteur en `CodeBadge` copiable, sans lien vers l'identité ; parcours `admin` :
    Erwan, auditeur du jeu (`web/e2e/users.toml`, groupe `auditeurs` de
    `global-setup.ts` : `reader` et le rôle `auditeur`, `core.audit.read` seul), filtre par
    son identifiant collé, aucune réponse 403 ; export : un stockage qui refuse le fichier
    à son ouverture passe au blob, un stockage plein en cours d'écriture
    (`ExportStorage`) se distingue d'un corps coupé (`ExportCut`) ; design et
    `PARITE.md` : seulement ce que le parcours prouve, repli sans OPFS dit.
  - reste : instants de l'audit sans fuseau affiché (heure du navigateur ; à trancher :
    `timeZoneName` dans le panneau) ; objets d'un événement en identifiants, sans nom ni
    lien ; noms d'événements coupés dans la liste quand le panneau est ouvert (pas de
    point de coupure dans un nom pointé) ; pas de bouton de copie sur le JSON ; noms des
    acteurs limités aux premières pages des identités (dix pages, liste et suggestions),
    son nom perdu au rechargement (puce en UUID), un acteur au-delà montré par son
    identifiant ; le répertoire `kartenn-exports/` vidé au début d'un export peut couper
    celui d'un autre onglet en cours ; filtres de période et d'objet sans parcours ; « Exporter » sans icône (aucune icône de téléchargement
    dans la liste fermée de `@kartenn/ui`) ; le fichier d'OPFS d'un très gros export est
    effacé une minute après sa remise au navigateur, qui pourrait ne pas l'avoir copié.
- [x] 15d. Reste de l'administration prouvé par le parcours (relevé par la liste de parité,
  26) : pseudonymisation d'une identité (confirmation, motif, refus 403 et 409), sur une
  identité créée pour le parcours, afin de ne consommer aucune personne du jeu ; filtres de
  période et d'objet du journal d'audit — verify : parcours `admin` sous les deux
  navigateurs ; lignes de `web/v2/PARITE.md` cochées
  - où : `web/v2/shell/src/admin/{IdentityDetail,AuditView}.vue` ;
    `web/v2/e2e/journeys/admin.spec.ts`, `web/v2/e2e/journeys/global-setup.ts`
  - dépend de : 15b, 15c
  - fait le 2026-11-03 : le serveur ne crée une personne qu'à sa connexion par un émetteur
    (ni route, ni opération) : deux utilisateurs de l'émetteur de développement
    (`web/e2e/users.toml`, `oubli-chromium` et `oubli-firefox`, groupe `lecteurs`), un par
    navigateur puisque les deux tournent à la fois ; le parcours connecte le sien dans un
    contexte à part, lit son identité (`/v1/me`), et c'est elle qu'Alice pseudonymise ;
    pseudonymisée, elle perd son sujet, et la connexion suivante en crée une neuve.
    Parcours `admin`, « a person pseudonymised at once, confirmed, then found in the audit
    by object and period » : « Pseudonymiser… » (dans « Plus d'actions » à la largeur du
    panneau), motif vide refusé sans requête, puis une seule requête avec son motif ; nom
    remplacé par `personne #…`, aucun brouillon ; 409 du serveur sur un compte de service
    créé par le parcours (« only a person is pseudonymised ») ; l'événement
    `person.identity.pseudonymised` trouvé par « Ajouter un filtre » : objet
    (l'identifiant), « Depuis le » aujourd'hui (jours lus avant la pseudonymisation), lu
    dans le panneau ; « Jusqu'au » hier : recherche vide, filtres retirés, le focus sur
    « Ajouter un filtre ». Compte de secours : aucune de ses actions, barre et « Plus
    d'actions », n'est « Pseudonymiser… ». Dana (« an administrator of one entity ») :
    l'entrée refusée avec sa raison, 403 du serveur sur un identifiant inconnu (avant toute
    lecture). `IdentityDetail.vue` : « Pseudonymiser… » offert aussi à un demandeur
    (`requester`, 016), comme le serveur (`pseudonymisable`, `access.ts`), la sorte
    `requester` nommée. `AuditView.vue` : « Retirer les filtres » levait une exception
    (`$el` du `FilterPicker` n'est pas un élément : sa racine est celle du popover) ; le
    focus passe par le parent, en attendant la méthode exposée de la 11h. Vitest :
    `audit.test.ts` (période typée en jours, envoyée en instants ; objet ; un jour invalide
    hors de la requête), `access.test.ts` (409 dit tel quel ; personne et demandeur
    pseudonymisables, nulle autre sorte). `PARITE.md` : trois lignes cochées.
  - choix : une personne par navigateur plutôt qu'un `global-setup.ts` (une personne ne
    naît que d'une connexion, que le setup ne fait que par l'API) ; 403 et 409 prouvés
    sur le serveur par `apiCall` et, à l'écran, par l'entrée refusée et son absence hors
    d'une personne : l'interface n'envoie jamais ces requêtes, ni un faux serveur.
  - reste : relectures (mineurs) : l'avertissement d'irréversibilité n'est pas la description du
    dialogue (lu dans son corps, pas annoncé à l'ouverture) ; un motif vide laisse le
    focus sur « Pseudonymiser » (erreur dite par la région polie du champ).
- [x] 16b. Fin de session annoncée et requêtes de fond : avis deux minutes avant
  `absolute_expires_at` (sans « Rester connecté », qui ne la repousse pas) ; en-tête
  `Kartenn-Background: ?1` posé par le SDK sur les relectures sans geste (événements,
  revalidation au focus et à la reconnexion, préchargement) — REQ-021-14 — verify :
  Vitest (en-tête présent seulement hors geste ; avis à l'heure) ; parcours `session`
  - où : `web/v2/shell/src/session/{expiry,warning}.ts`, `SessionDialog.vue` ;
    `activityFetch` et `installGestures` du SDK (`web/v2/sdk/src/`) ; parcours
    `web/v2/e2e/journeys/session.spec.ts`
  - dépend de : 16a, 16c
  - fait le 2026-11-02 : `activityFetch` (`shell/src/session/expiry.ts`, branché sur le
    client du SDK par `setFetch`, donc aussi pour les plugins) pose
    `Kartenn-Background: ?1` sur toute lecture de l'API (GET, HEAD, QUERY) qu'aucun geste
    ne précède de `GESTURE_WINDOW_MS`, et sur elle seule : exactement les requêtes qu'il
    ne compte pas comme activité ; jugé à chaque requête (une lecture née d'un geste,
    relue en fond, le porte à sa relecture). Le flux d'événements (`EventSource`) ne peut
    poser d'en-tête : le serveur le traite déjà en `NotActivity` (022/A6), comme les pages
    d'événements. `createExpiry.read(expiresAt, absoluteAt)` : fin attendue
    `min(absolue, max(fin lue, dernière activité + fenêtre))`, la fenêtre étant la plus
    longue vue entre une lecture et sa fin (une lecture de fond la sous-estime : avis tôt,
    jamais tard) ; l'heuristique « fin qui n'a pas bougé » (`FIXED_AFTER_MS`) retirée.
    Avis de fin absolue : titre `session.endingAbsolute`, sans « Rester connecté ». La
    relecture de l'import chaque seconde (15c) le porte désormais. Tests : Vitest
    `session.test.ts` (en-tête absent après chaque geste, présent hors geste, lecture née
    d'un geste puis relue en fond, écritures et hors API jamais ; avis de fin absolue à
    l'heure malgré l'activité ; passage de l'avis d'inactivité à celui de fin absolue
    après « Rester connecté ») ; parcours `session` (revalidation au focus marquée,
    navigation cliquée non marquée ; fin absolue dite sans « Rester connecté », `/v1/me`
    réécrit par `page.route`, la durée absolue minimale d'une heure étant trop longue).
  - choix : en-tête posé dans `activityFetch` du shell plutôt que déplacé dans le SDK
    (c'est le `fetch` du client du SDK, plugins compris) ; écritures jamais marquées et
    toujours comptées comme activité côté client (accord avec le serveur, qui les compte) ;
    l'avis de fin absolue garde `endingFixed`, réécrit (« ne peut pas être prolongée »).
  - reste : relecture a11y — l'avis de fin absolue n'offre pas d'action « Se
    reconnecter » (le texte invite à terminer la saisie d'abord) ; un avis `warn` passe
    par la région polie (`role="status"`) et pourrait être lu tard par un lecteur d'écran,
    et sa perception sous une modale ouverte n'est pas vérifiée (`@kartenn/ui`, hors
    périmètre) ; la revalidation sur reconnexion et le préchargement au survol ne sont
    couverts qu'en Vitest (même règle), pas en parcours.
- [x] 17b. « Créer un VLAN pour ce réseau » (maquette E12) : création du VLAN et pose de
  `core.vlan` par une référence locale, au brouillon ou en Ctrl+Entrée ; poser un VLAN
  existant depuis l'onglet du réseau — REQ-026-6 — verify : parcours `vlan` (création
  pour un réseau au brouillon puis appliquée, un seul changeset)
  - où : écrans VLAN de la tâche 17 (`web/v2/shell/src/views/`, onglet Réseaux) ;
    parcours `web/v2/e2e/journeys/vlan.spec.ts`
  - dépend de : 17a
  - fait le 2026-11-02 — onglet VLAN d'un réseau qui n'en porte aucun
    (`record/NetworkVlanTab.vue`) : « Créer un VLAN pour ce réseau » ouvre le panneau
    d'édition sur un VLAN (`EditRequest.network`, `edit/panel.ts`) titré « Créer un VLAN
    pour <réseau> », champ « Réseau » en lecture seule, « Propriété posée » `core.vlan`, avis
    « Deux opérations, un seul changeset » ; les deux opérations (`vlanForNetwork`,
    `edit/vlan.ts`) : `create_vlan` sous un nom local, puis `assign_property` de
    `core.vlan` à `{"local": n}` (`networkVlanOperation`, `parts/vlan.ts`) — au brouillon
    (action par défaut, Ctrl+Entrée), le nom pris libre dans le brouillon actif relu
    (`vlanForNetworkInDraft`, `freeLocal` : les opérations données au SDK en fonction,
    `Additions` de `drafts.addOperations`/`addToActive`, faites du brouillon à la version
    envoyée, de nouveau après un 412), ou appliquées ensemble (`?approval=self`, motif
    prérempli « Création d'un VLAN pour <réseau> », nom 0). Droits de l'action : une seule
    liste, `core.vlan.edit` plus `NETWORK_VLAN_EDITS` (`core.prefix.edit`,
    `prop.core.vlan.edit`, aussi celle de l'onglet), et pour appliquer l'approbation de
    chacun (`approvalOf`, `lackingRight` : `core.prefix.approval.self`,
    `prop.core.vlan.approval.self`, comme le serveur), « Appliquer » refusé nomme celui qui
    manque ; une violation de la pose
    (`operation` 1 ou pointeur `/operations/1/…` : `prop.assignment.invalid_value`,
    `prop.assignment.unknown_local`) dite sur le champ « Réseau », avec son code
    (`ofAssignment`, `fieldErrors`). « Poser un VLAN existant » : `Combobox` des suggestions de
    `/v1/vlans` (`q`), « Poser le VLAN » au brouillon ; un texte sans VLAN choisi est
    refusé sur le champ. Les deux refusés avec leur raison si une pose de `core.vlan` sur
    ce réseau est déjà au brouillon (`isNetworkVlanAssignment`). Brouillon : une liste de
    valeurs se lit valeur par valeur (`#0` pour un nom local, `draft/operations.ts`).
    Tests : Vitest `test/edit.test.ts` (opérations, violation de la pose et
    `prop.assignment.invalid_value` sur le champ du réseau, nom local libre dans le
    brouillon, « Appliquer » refusé sans `core.prefix.approval.self`),
    `test/views.test.ts` (`freeLocal`, pose au brouillon), `test/changesets.test.ts`
    (valeurs d'une affectation, toute liste), `sdk/test/drafts.test.ts` (opérations faites
    du brouillon, de nouveau après un 412) ; parcours `vlan` (VLAN existant
    posé au brouillon puis appliqué ; créé pour un réseau au brouillon puis appliqué, un
    changeset `create_vlan` + `assign_property` ; appliqué d'emblée, refus de la pose
    réécrit par `page.route` dit sur le champ du réseau, puis appliqué, un changeset).
  - choix : les deux actions offertes seulement sur un réseau sans VLAN (remplacer passe
    par le retrait, au brouillon) ; pose d'un VLAN existant au brouillon seulement, comme
    le retrait ; groupes offerts sans filtre par l'ancre du réseau (le serveur juge,
    `core.vlan.site_mismatch`) ; le réseau en champ en lecture seule plutôt qu'en
    attribut (maquette), pour y dire le refus de la pose ; texte de l'avis fixe (une
    région `status` relue à chaque frappe sinon) ; au brouillon, pas de simulation dans le
    panneau (le panneau du brouillon la montre, comme pour les autres créations) ; champ
    du VLAN refusé (aucun choisi) : le focus y va, l'erreur part au choix (relectures UI et
    a11y du 2026-11-02).
  - reste : les suggestions de « Poser un VLAN existant » ne disent pas le groupe
    (`/v1/vlans` ne sert que son identifiant : deux VLAN de même VID et nom dans deux
    groupes se confondent) ; l'avis « Deux opérations » ne nomme ni le VLAN ni son groupe
    (maquette : « Créer le VLAN {vid} {nom} dans {groupe}… », texte fixe voulu) ;
    `values` d'une liste de valeurs vide reste lue en JSON (`[]`) ; après « Poser le
    VLAN », le focus reste sur le bouton, alors refusé (raison en description) : à
    vérifier au lecteur d'écran (revue du 2026-11-02).
- [x] 24b. Aperçu du brouillon dans l'arbre des préfixes, tiré de
  `ChangesetSimulated.placements` : nœuds déplacés montrés sous leur parent d'après,
  marqués « en brouillon » — REQ-021-17 — verify : Vitest de l'aperçu ; parcours `moves`
  (préfixe inséré au brouillon : ses futurs enfants montrés dessous avant application)
  - où : `web/v2/shell/src/views/TopologyView.vue`, `topology.ts` ; parcours
    `web/v2/e2e/journeys/moves.spec.ts`
  - dépend de : 24a
  - fait le 2026-11-02 : `topology.ts` (`previewOf` : placements du domaine, objet créé
    rapproché de sa création ; `overlay` : chaque déplacé sous sa ligne d'après, par-dessus
    l'arbre servi, jamais écrit dans le cache) ; `TopologyView.vue` (simulation du
    brouillon actif par `useDraftQueries`, la même requête que l'onglet Brouillon ; ligne
    d'aperçu `draft:…` marquée « en brouillon », l'objet marqué aussi à sa place présente ;
    avis au-dessus de l'arbre : lignes montrées, lignes en attente de leur parent, « Aperçu
    partiel » sur `placements_truncated`) ; chaînes `views.topology.preview.*` ; tests
    `web/v2/shell/test/topology.test.ts` (insertion et futurs enfants, rapprochement d'une
    création derrière des opérations sans placement, clé stable d'une simulation à
    l'autre, parent qui se déplace aussi, préfixe déplacé avec les enfants qu'il garde,
    autre domaine, troncature, parent illisible), parcours `moves` (préfixe inséré au
    brouillon par son « + », ses enfants dessous avant l'application, une ligne d'aperçu
    choisie reste sélectionnée, aperçu parti après ; valeur changée : la ligne d'aperçu sous le /24 ;
    `rendered` ne lit que l'arbre servi ; le motif du brouillon écrit en JSON Merge Patch,
    `applyActive`, le parcours envoyant encore `application/json` depuis la 14a).
  - choix : objet créé rapproché par l'ordre du contrat (objets des opérations d'abord,
    dans leur ordre ; création = même genre, `before` nul, identifiant hors des
    opérations) ; clé d'une ligne créée `draft:<indice de l'opération>`, stable d'une
    simulation à l'autre ; aperçu seulement d'une simulation valide, au présent, la
    dernière réponse gardée pendant la suivante ; `after` nul (racine ou parent illisible)
    au premier niveau ; un objet n'est de ce domaine que lu dans l'arbre ou créé avec ce
    domaine (les autres ne sont ni montrés ni comptés) ; un déplacé dont le parent d'après
    n'est pas lu attend (compté dans l'avis) ; lignes d'aperçu après les enfants servis,
    sans « + » ni éditeur, un objet déplacé avec son occupation lue et les enfants qu'il
    garde (copiés de l'arbre servi, jusqu'où il est lu ; ceux qui partent ailleurs non
    répétés) ; choisir une ligne d'aperçu la garde sélectionnée et met son objet dans
    l'URL, Entrée ouvre sa fiche (rien pour un objet créé) ; avis dans la bande d'outils du
    panneau (corps sans marge). Corrections de relecture (ui-reviewer, a11y-auditor) :
    sous-arbre gardé, sélection, occupation, place de l'avis, libellés.
  - reste : `TreeGrid` n'a que `draft` et `loading-children` : la ligne présente d'un
    objet déplacé et sa ligne d'aperçu portent la même marque et les mêmes mots « en
    brouillon », un lecteur d'écran ne les distingue que par le niveau (le design en veut
    deux : « déplacé dans le brouillon » et « aperçu » ; états à ajouter à `@kartenn/ui`,
    hors périmètre) ; lignes d'aperçu après les enfants servis, sans tri par valeur (pas de
    calcul CIDR du client) ; un enfant gardé non lu se montre sans ses enfants ; l'avis
    (`status`) se relit à chaque nouveau compte ; un objet créé choisi ne montre rien ; le
    rapprochement d'une création suit l'ordre du contrat (un `Placement` portant l'indice
    de son opération le rendrait exact).

- [x] 24d. États `moved` et `preview` dans la Topographie (020-29) : la ligne au présent
  d'un objet que le brouillon déplace en `moved`, sa ligne d'aperçu en `preview`, au lieu
  de `draft` sur les deux — verify : Vitest de l'aperçu (états par ligne) ; parcours
  `moves` (préfixe inséré au brouillon : ligne au présent et ligne d'aperçu dites
  différemment) sous les deux navigateurs
  - où : `web/v2/shell/src/views/TopologyView.vue`, `topology.ts` ; `TreeNodeState` de
    `web/v2/ui/src/components/TreeGrid/tree.ts` (en lecture)
  - dépend de : 24c, 020-29
  - fait : `draftStateOf` (`topology.ts`) donne `preview` à une ligne d'aperçu (création,
    objet déplacé, enfant qu'il garde), `moved` à la ligne servie d'un objet que le
    brouillon déplace, `draft` à un objet changé sans déplacement ; `nodeState` de
    `TopologyView.vue` s'en sert (au présent, `loading-children` d'abord). Vitest
    (`web/v2/shell/test/topology.test.ts` : états de chaque ligne d'un préfixe inséré, un
    déplacé dont le parent d'après n'est pas lu, un enfant emporté) ; parcours `moves`
    (`expectMoved` : `data-state` et mots de chaque ligne, ceux de l'autre et « en
    brouillon » absents), chromium et firefox. Relectures `ui-reviewer`, `a11y-auditor`.
  - choix : `moved` l'emporte sur `draft` (un préfixe dont la valeur change et qui change
    de place est dit « déplacé », sa valeur nouvelle se lit sur sa ligne d'aperçu) ; un
    déplacé dont le parent d'après n'est pas lu est dit `moved` sans ligne d'aperçu (il
    attend, compté dans l'avis, 24b) ; la copie d'un enfant emporté est `preview`, même si
    sa valeur est au brouillon, et sa ligne servie garde son état (il ne change pas de
    parent).
  - reste : une ligne `moved` qui lit ses enfants dit `loading-children` le temps de la
    lecture (comme `draft` avant).
- [x] 16g. Titres du shell par `Heading` (020-29) : les classes de titre recopiées
  (`k-login__title`, `k-about__title`, `k-plugin-view__title`, `k-pending__title`,
  `k-help__title`, `k-edit-panel__title`, `k-network-vlan__title`) remplacées par `Heading` ;
  `fieldText` passe une valeur absente à `AttributeList` au lieu d'un « — » lu « tiret »
  — verify : Vitest ; `k-web2-check --base main` (captures touchées listées) ; revue
  `ui-reviewer`
  - où : les fichiers des classes citées sous `web/v2/shell/src/` (grep), `fieldText`
    (`web/v2/shell/src/views/`) ; `Heading` et `AttributeList` de `web/v2/ui` (en lecture)
  - dépend de : 020-29
  - fait le 2026-11-03 : `Heading` (niveau 1 : `LoginView`, `AboutView`, `PluginView`,
    `PendingView`, `HelpView` ; niveau 2 : `EditPanel`, `ClosePanel`, `NetworkVlanTab`, son
    `id` gardé pour `aria-labelledby`), les huit classes et leurs règles retirées ;
    `fieldText` (`views/collections.ts`) rend `null` pour une valeur absente : la fiche
    (`RecordView`) la passe à `AttributeList`, la liste (`ListView`) rend `NoValue` ; tirets
    recopiés relevés par la 020-30 passés à `NoValue` : cellules d'`IdentitiesView`,
    `RolesView`, `DraftsView` (échéance absente), `AuditView` (`targets` de
    `auditWords.ts` rend `null`), `PropertiesView` (héritage et indicateurs absents) ;
    `person` et `when` de `changesets/words.ts` rendent `null` pour un absent (attributs de
    `ChangesetView` dits par `AttributeList`) ; `memberText` (`draft/operations.ts`) rend
    `null`, un membre absent dit par `OperationText`. Tests `test/views.test.ts`
    (`fieldText`), `test/changesets.test.ts` (`memberText`, `operationLine`). Corrigé après
    la relecture (ui-reviewer) : `person` et `when` typés par surcharge, un texte requis
    par le contrat reste un texte (pas de cellule blanche).
  - choix : aussi `k-close-panel__title` (même copie que `k-edit-panel__title`, venue avec
    la 11g) ; valeur absente sans mots propres à la vue : `NoValue` et « non renseigné »
    (pas de nouveau texte, comme la 020-30) ; lien de la première colonne d'une liste à
    valeur propre (`vid`) sans valeur : nommé par `nameOf` plutôt que par un tiret ; dans
    `memberText`, un élément absent d'une liste s'écrit `null` (son JSON), une plage dont une
    borne manque est nommée par sa première seule (le membre absent n'est pas écrit).
  - reste : titres de section recopiés (`k-about__heading`, `k-login__heading`, sous
    `ref` et `focus()` : `Heading` rendrait le composant, pas l'élément ;
    `k-admin-heading`, `k-view-panel__heading`, `k-draft-ops__subtitle`) ; tirets gardés
    hors de la liste : `shownText` d'`EditPanel` (`text || "—"`, valeurs relues après un
    412), `nameOf` (`String(item.id ?? "—")`) et `op.op ?? "—"` de `operationLine`
    (repli d'un nom ou d'un code manquant) ; « sans échéance », « aucun indicateur »
    seraient plus justes que « non renseigné » (textes de la vue à écrire).

- [x] 11g. « Clore » un objet depuis sa fiche (A31) : action de la fiche de chaque sorte du
  noyau (entité, site, domaine, préfixe, plage, adresse, VLAN, groupe de VLAN), au
  brouillon ou en Ctrl+Entrée avec motif ; `expected` de la version servie ; refus de
  cascade et de règle dits par leur code, sur l'action ; droit manquant visible avec sa
  permission (REQ-021-37) — verify : Vitest ; parcours `records` (clôture d'une adresse
  au brouillon puis appliquée ; clôture d'un préfixe refusée par une règle, code dit) sous
  les deux navigateurs ; ligne de la liste de parité cochée
  - où : `web/v2/shell/src/views/record/RecordView.vue` (barre d'actions), `views/edit/`
    (`write.quick`, brouillon), `Close` des opérations du noyau (contrat 006) ; ancien
    comportement : `web/shell/src/` (grep « Clore »)
  - dépend de : 11d
  - fait le 2026-11-03 — « Clore… », dernière entrée du menu d'actions de la fiche de
    chaque sorte (`RecordView.vue`, après un séparateur, `danger`), refusée avec
    `<sorte>.edit` ou hors du présent ; elle ouvre `views/edit/ClosePanel.vue` dans l'onglet
    Détail (requête `close` de `panel.ts`, montée par `ShellFrame.vue`). L'opération
    `close` (`views/edit/close.ts`, `closeOperation`) porte en `expected` la version servie
    avec la valeur lue (instantané de la 11d), relue seulement après un refus
    `changeset.operation.conflict`. Avant tout envoi, la simulation de la clôture seule
    (`POST /v1/changesets?simulate=true`, `closeVerdict`) dit les règles qui la refusent avec
    leur code, et, la cascade cochée (« Clore aussi ce qui en dépend », offerte pour domaine,
    préfixe, entité, site, groupe de VLAN, ou sur `core.object.dependents`), les clôtures
    ajoutées comptées par sorte. Bouton scindé « Clore au brouillon » / « Appliquer la
    clôture » (motif prérempli, `?approval=self`, sans `<sorte>.approval.self` refusé et
    nommé) ; Ctrl+Entrée lance l'action choisie, et dit la permission manquante par le statut.
    Un refus du serveur reste sur l'action avec son code (`RefusalAlert`). Tests :
    `shell/test/close.test.ts` (opération et verdict ; panneau monté : brouillon par
    Ctrl+Entrée à la version servie, refus de règle et cascade comptée, droits manquants) ;
    parcours `records` « an address closed from its record… » et « a network closed
    alone… » (`core.address.outside_network` dit avant l'envoi puis par le serveur), sous
    Chromium et Firefox ; ligne « Clore » de `web/v2/PARITE.md` cochée.
  - choix : un panneau de clôture à part plutôt qu'un mode du panneau d'édition (aucun
    champ, sa simulation) ; la simulation est celle de la clôture seule sur le référentiel,
    non du brouillon (le brouillon a la sienne) ; la soumission immédiate passe par
    `changesets.submit` (l'`expected` dans l'opération) plutôt que `DELETE` sous `If-Match`,
    comme le brouillon ; après une clôture appliquée, la fiche se relit et dit que l'objet
    n'est plus au présent (comme l'ancien shell) ; l'entrée du menu reprend l'icône `trash`
    de la maquette E2 ; le menu d'actions est désormais toujours montré.
  - corrections de revue : la clé d'idempotence se renouvelle dès que l'opération change
    (cascade, version relue après `changeset.operation.conflict`), sans quoi la seconde
    tentative recevait `IdempotencyConflict` (REQ-006-15) ; test monté du conflit (422, note,
    relecture, second envoi à la version relue sous une autre clé), d'un auteur sans
    `.approval.self` (« Appliquer » le nomme, le brouillon reste permis) ; un rejet de
    l'écriture (brouillon compris) garde son problème, code et `Request-Id` dits, et libère
    le bouton ; le séparateur de tête des actions des plugins n'est retiré que s'il y est.
  - reste : mécanique commune au panneau d'édition (action, motif, clé d'idempotence,
    `settle`) recopiée, à factoriser en un composable (du type `useWriteAction`) qui porte la
    règle « opération nouvelle, clé nouvelle » (revue `ui-reviewer`) ; dans `EditPanel.vue`,
    même cause : Ctrl+Entrée d'une action refusée reste muet (revue `a11y-auditor`), et un
    rejet de `addToActiveDraft` laisse `busy` vrai (pas de `.catch`) ; la fiche ne dit pas
    qu'une clôture de l'objet attend au brouillon ; les maquettes nomment encore
    `core.<sorte>.apply` sur « Clore… » (E2, E12, E27).

- [x] 12b. Action d'écriture partagée par les panneaux (relevé par la revue de la 11g) :
  un composable du type `useWriteAction` porte l'action scindée (brouillon ou
  application), le motif prérempli, la clé d'idempotence et `settle`, employé par
  `EditPanel` et `ClosePanel` ; la règle « opération nouvelle, clé nouvelle » y est écrite
  une fois ; au passage, les deux défauts d'`EditPanel` de même cause (Ctrl+Entrée muet sur
  une action refusée ; `addToActiveDraft` sans `.catch`, `busy` resté vrai) — verify :
  Vitest du composable (clé renouvelée quand l'opération change, rejet qui rend la main,
  refus dit) ; Vitest d'`EditPanel` et de `ClosePanel` inchangés ; parcours `edit` et
  `records` sous les deux navigateurs
  - où : `web/v2/shell/src/views/edit/{EditPanel,ClosePanel}.vue`, forme de
    `web/v2/shell/src/views/edit/` ; tests `web/v2/shell/test/{edit,close}.test.ts`
  - dépend de : 11g
  - fait le 2026-11-03 — `views/edit/action.ts` : `useWriteAction` (action du bouton scindé,
    `actions` refusées avec leur permission, `applying`, motif prérempli jusqu'à la saisie,
    `idempotencyKey()`, `busy`, `notice`, `refusal`, `begin` — droit, présent —, `write` —
    un rejet devient un refus `failed` qui garde son `ApiProblem` —, `settle`) ;
    `EditPanel.vue` et `ClosePanel.vue` l'emploient, chacun gardant ses refus propres
    (champs d'un 422, relecture d'un conflit) par `refused` ; Ctrl+Entrée d'une action
    refusée dit sa permission dans `EditPanel` aussi ; un rejet du brouillon, de la création
    appliquée ou d'un changement de parent rend le bouton ; `edit.close.forbidden` devenu
    `edit.unsent`, commun ; la même phrase redite (deux Ctrl+Entrée refusés) est vidée puis
    reposée, pour être annoncée de nouveau (revue `a11y-auditor`) ; test
    `web/v2/shell/test/action.test.ts` (clé gardée, renouvelée par une opération ou un motif
    nouveaux, rejet qui rend la main, refus et hors présent dits ; `EditPanel` monté :
    Ctrl+Entrée sans `core.domain.edit` dit la permission, un rejet de `addToActiveDraft`
    rend le bouton et dit le problème — les deux en échec sur l'`EditPanel` d'avant) ;
    `edit.test.ts` et `close.test.ts` inchangés et verts (conflit de la 11g compris).
  - choix : la clé se compare quand on la prend (l'envoi sérialisé avec son motif), au lieu
    d'un `watch` qui la renouvelait — aucun changement manqué entre deux ticks ;
    `write.quick` en échec garde son problème (avant : `problem: null`).
  - corrections de revue : `idempotencyKey(submission)` prend le corps même qui part (le
    changement de parent le demande à `useMoves`, `MoveOptions.idempotencyKey`, avec
    l'opération et son `expected`) ; un conflit d'un changeset (`changeset.operation.conflict`,
    `isConflict`, servi sans `current`) fait relire l'objet dans `EditPanel` comme dans
    `ClosePanel` ; `useMoves.move` rejeté ne laisse plus le nœud `pending` ; `edit.unchanged`
    dit par `tell`, qui compare à la phrase rendue ; l'erreur du motif de `ClosePanel`
    effacée avant `begin` ; tests montés : clôture (même clé après un 503, une autre et la
    version relue après un conflit), déplacement appliqué (conflit, relecture, second envoi
    à `V2` sous une autre clé — en échec sur le code d'avant la correction).
  - reste : une alerte `warn` (conflit 412) de `RefusalAlert` est un `role="status"` inséré
    avec son contenu, peu annoncé par les lecteurs d'écran (antérieur, `InlineAlert` de
    `web/v2/ui`, hors de cette tâche ; revue `a11y-auditor`).

- [x] 11h. « Retirer les filtres » sans exception (relevé par la 15d) : `FilterPicker`
  expose une méthode qui met le focus sur « Ajouter un filtre » (à la forme de `openAdd`) ;
  `ListView`, `ChangesetsView` et `AuditView` l'appellent au lieu de chercher le bouton
  dans le DOM depuis `$el` (un nœud texte, la racine du composant étant un fragment :
  `TypeError` aujourd'hui dans les deux premières vues, contournement fragile dans la
  troisième) — verify : Vitest des trois vues (filtres retirés, focus sur « Ajouter un
  filtre », aucune exception) ; Vitest du composant ; seul agent sur `web/v2/ui`
  - où : `web/v2/ui/src/components/FilterPicker/FilterPicker.vue` (`addButton`, vers la
    ligne 237 ; `defineExpose`, vers la ligne 299) ;
    `web/v2/shell/src/views/ListView.vue` (vers la ligne 225),
    `web/v2/shell/src/changesets/ChangesetsView.vue` (vers la ligne 102),
    `web/v2/shell/src/admin/AuditView.vue` (vers la ligne 153)
  - dépend de : 15d ; après la 020-30 (`web/v2/ui`)
  - fait le 2026-11-03 — `FilterPicker` expose `focusAdd()` (à côté d'`openAdd` et
    `openFor`), qui met le focus sur « Ajouter un filtre » par la référence qu'il tient déjà
    de sa barre (`addButton`). `ListView`, `ChangesetsView` et `AuditView` prennent la même
    forme : `clearFilters` retire les filtres appliqués (les brouillons gardés) puis
    `nextTick(() => picker.value?.focusAdd())` ; le contournement d'`AuditView`
    (`$el.parentElement`) et son commentaire ont disparu. Lignes d'inventaire de
    `FilterPicker` (020 `design.md`, skill `kartenn-ui`) complétées. Tests :
    `shell/test/clear-filters.test.ts` (les trois vues montées sur une liste filtrée vide :
    filtres retirés de l'URL, focus sur « Ajouter un filtre », ni erreur de Vue ni rejet
    non traité ; échoue sur l'ancien code de `ListView` et `ChangesetsView`),
    `ui/test/filter-picker-mounted.test.ts` (`focusAdd` avec et sans pastilles, `$el` qui
    n'est pas un élément, popover fermé).
  - choix : nom `focusAdd` ; méthode synchrone, le `nextTick` reste chez l'hôte (il attend
    le rendu des filtres retirés, comme avant) ; un fichier de test commun aux trois vues
    plutôt qu'un cas dans chacun de leurs fichiers (même forme, même vérification ;
    `audit.test.ts` tourne sous `node`) ; aucune entrée de catalogue ni capture : la
    méthode ne se montre pas.
  - reste (revue `a11y-auditor`, mineurs) : le retrait des filtres n'est dit que par le
    compte de la liste, aucune phrase « Filtres retirés » dans la région `polite` (à juger
    au lecteur d'écran) ; non couverts par un test : « Retirer les filtres » le popover
    ouvert (le clic au dehors le ferme sans rendre le focus, `focusAdd` l'emporte, lu dans
    le code) et un brouillon gardé après le retrait.

### Sous-tâches de la liste de parité (26, 2026-11-03)

Chacune porte les lignes `[ ]` de `web/v2/PARITE.md` qui la nomment, et les coche quand un
parcours les prouve ; `web/v2/shell/test/parity.test.ts` vérifie les titres cités.

- [x] 11i. Anomalies et déclarations prouvées par le parcours : onglet Anomalies de la
  fiche, `/anomalies` (ouvertes, filtres, `?tab=gaps`, acquittement avec motif),
  `/property-declarations` (cardinalité, anomalies ouvertes d'une clé) — verify : parcours
  `records` (ou un parcours `anomalies`) sous les deux navigateurs ; lignes 46 à 48 de la
  liste cochées
  - où : `web/v2/shell/src/views/{AnomaliesView,PropertiesView}.vue`, `views/record/` ;
    ancien parcours `web/e2e/journeys/records.spec.ts`
  - dépend de : 11
  - fait le 2026-11-03 — l'ancien parcours `records` n'avait aucun cas d'anomalie ni de
    propriété (l'ancien `accessibility` visitait seulement `/anomalies` et `/properties`) :
    parcours neuf `web/v2/e2e/journeys/anomalies.spec.ts`, trois tests, chacun dans un
    domaine à lui dont le réseau 198.51.100.0/24 (RFC 5737) ouvre l'avertissement
    `core.addr.documentation` : l'onglet Anomalies d'un domaine (l'anomalie et son objet,
    lien vers le réseau) puis du réseau (sans la colonne de l'objet) ; `/anomalies` filtré
    par domaine au clavier dans la barre (l'URL le garde), « Acquitter… » refusé sans motif
    (erreur sur le champ, focus rendu au champ), envoyé par Entrée, toast, état vide qui
    prend le focus, plus rien au serveur pour le domaine ; onglet Lacunes dans l'URL, les
    filtres des anomalies laissés ; `/property-declarations` : `core.owner` « Une valeur »,
    « Adressage », la clé attendue `policy.environment` et « Voir les lacunes de… » vers
    `/anomalies?tab=gaps&property=policy.environment`, sa pastille de filtre. Lignes 46 à
    48 de `web/v2/PARITE.md` cochées. Manques portés au plus petit : cardinalité et
    héritage en mots (`words("cardinality", …)` heurtait le titre de colonne, la valeur
    brute `single` s'affichait ; groupes `cardinalities` et `inheritance`), cellule vide
    dite par `NoValue` ; focus perdu après l'acquittement (l'invalidation attendue avant la
    fermeture désarmait la remise du focus de `AnomalyGrid` ; la dernière anomalie
    emportait la grille) : invalidation non attendue, événement `acknowledged`,
    `useFocusWhenEmptied` (`views/anomalies/anomaly.ts`) qui met le focus sur l'état vide
    de `AnomalyList` et de `AnomaliesTab` ; l'onglet dit l'échec d'une page suivante
    (`NextPageAlert`, au lieu de la taire) et ne la redemande plus en boucle ; vide
    partiel dit « dans les pages lues » ; lacunes : phrase de l'état vide, propriété
    absente par `NoValue`. Test `shell/test/anomalies.test.ts` (composable : armé et vide,
    non armé, désarmé par des lignes restantes, focus déjà ailleurs).
  - choix : un parcours `anomalies` à lui plutôt qu'un ajout à `records` (d'autres tâches
    y travaillent) ; l'état vide enveloppé d'un `div tabindex="-1"` (comme
    `DraftOperations`) plutôt que le focus sur « Ajouter un filtre » (il dit ce qui reste) ;
    mots « Une valeur », « Plusieurs valeurs », « Adressage », « Entités », « Sites » (un
    arbre de module reste sa clé).
  - reste : aucune lacune réelle dans le parcours — `prop.resolution.expected_mismatch`
    ne naît que d'une source attendue (l'inférence de la politique, ses règles et sa tâche
    de fond), trop lourde pour un parcours : l'onglet Lacunes y est prouvé vide ou listé,
    filtré, pas une ligne ; le filtre `object` absent de `GET /v1/anomalies` (relevé par
    la 11, contourné par l'onglet : anomalies du domaine, dix pages, triées ; rien hors
    domaine) attend toujours son arbitrage de contrat (006) ; relecture `a11y-auditor`,
    hors du shell : la raison d'un « Acquitter… » refusé seulement dans une infobulle
    (`Button`, `web/v2/ui`), une alerte du dialogue insérée remplie peut-être peu annoncée
    (à juger au lecteur d'écran).
- [x] 11j. Une ligne de lacune prouvée (relevé par la revue de la 11i) : l'onglet Lacunes
  de `/anomalies` rend une vraie lacune (clé en `KeyLabel`, objet, message, `NoValue` pour
  une propriété absente), produite par une source attendue dans le jeu des parcours
  (règle d'inférence de la politique et sa passe, ou une déclaration attendue posée par le
  `global-setup.ts`, au plus léger) ; au passage, l'onglet Anomalies d'une fiche traite un
  curseur refusé (`invalid-cursor`) comme `list.ts` (`nextFailed`, `resetQueries`) au lieu
  de le renvoyer à chaque « Réessayer », et montre l'état vide « dans les pages lues »
  quand la page suivante est refusée — verify : parcours `anomalies` (une ligne de lacune
  lue) sous les deux navigateurs ; Vitest du curseur refusé ; ligne 47 de la liste dite
  sans réserve
  - où : `web/v2/shell/src/views/anomalies/GapList.vue`,
    `web/v2/shell/src/views/record/AnomaliesTab.vue` (vers les lignes 83 et 94),
    `web/v2/shell/src/views/list.ts` (forme) ; `web/v2/e2e/journeys/anomalies.spec.ts`,
    `web/v2/e2e/journeys/global-setup.ts`
  - dépend de : 11i
  - fait : parcours `anomalies`, « a gap read: its key, its object and the deviation, from
    a source of the build » — dans le domaine du test, une clé à lui
    (`custom.gap_<navigateur>_<horodatage>`, liste fermée, portée par les préfixes,
    attendue de `policy.inference` en mode `check`), sa valeur `prod`, désignée axe
    d'inférence, une règle d'inférence restreinte au domaine (`criteria: [{domain}]`),
    puis un réseau écrit sans la valeur : la lacune s'ouvre à l'écriture (REQ-003-24),
    sans tâche de fond ; sa ligne lue dans `/anomalies?tab=gaps&property=<clé>` (libellé
    et clé, réseau en lien, message du serveur `<clé>: resolved no value, expected …`),
    axe sans violation. `global-setup.ts` intact. Onglet Anomalies d'une fiche : un
    curseur refusé relit les pages depuis la première (`resetQueries`), dit dans un
    `role="status"` masqué (comme `ContentTab`) ; refusé de nouveau, l'alerte de la page
    suivante, dont « Réessayer » repart du début au lieu de renvoyer le curseur ; page
    suivante refusée, l'état vide « dans les pages lues » sous l'alerte au lieu d'une
    grille vide. Vitest `shell/test/anomalies.test.ts` (curseur refusé relu une fois,
    refusé deux fois sans boucle, page suivante refusée). Ligne 47 de `PARITE.md` sans
    réserve.
  - choix : la source la plus légère du build est l'inférence de la politique en mode
    `check` sur une déclaration du test — la seule source d'attendus du build est
    `policy.inference` ; `policy.environment` y est en mode `default` (il n'ouvre aucune
    lacune) et le contrôle `policy.inference.mismatch` passe par une tâche de fond et ne
    figure pas parmi les lacunes ; la comparaison `check` se fait à l'écriture, quatre
    opérations dans un changeset puis un réseau. Clé, axe et règle propres à chaque
    navigateur et à chaque passage (la base est recréée à chaque lancement). Le
    redémarrage sur curseur refusé ne se fait qu'une fois de lui-même (les pages du
    domaine d'un objet se lisent sans le lecteur : un curseur toujours refusé bouclerait),
    là où `list.ts` repart à chaque refus (la page suivante y vient du défilement) ;
    relectures : l'état « reparti » remis à zéro au changement de la clé de lecture
    (filtre et dates, comme `sent` de `list.ts`), le statut vidé puis redit à chaque
    redémarrage pour être annoncé de nouveau.
  - reste : le parcours lit le message de l'écart tel que le serveur l'écrit, en anglais
    (`<clé>: resolved no value, expected [Text("prod")] by policy.inference@…`), libre et
    non traduit — à reprendre si la 006 le structure ; `a11y-auditor`, à juger au lecteur
    d'écran : le statut « la liste repart du début » suivi de près par l'alerte de la page
    suivante si le curseur est refusé deux fois.
- [x] 11k. Filtre `object` des anomalies (A37) : `GET /v1/anomalies?object=<uuid>` sous le
  filtre de lecture des anomalies (objet illisible : liste vide, comme un objet inconnu) ;
  l'onglet Anomalies d'une fiche le lit directement, sans lire les pages du domaine, et
  montre les anomalies d'un objet hors domaine — verify : `k-check -p kartenn-rest`
  (anomalies d'un objet, objet illisible, pagination) ; Vitest de l'onglet ; parcours
  `anomalies` sous les deux navigateurs ; contrat et schémas régénérés
  - où : route des anomalies de `crates/kartenn-rest/src/` (grep `anomalies`),
    `web/v2/shell/src/views/record/AnomaliesTab.vue`
  - dépend de : 11j
  - fait : `AnomalyFilter.object` (`store-pg/src/anomalies.rs`) : la condition `$9` de
    `open_page` n'est écrite qu'avec un objet, comme le keyset, sans migration — mesuré en
    plan générique (`plan_cache_mode = force_generic_plan`, 2·10⁵ anomalies ouvertes sur
    5·10⁴ objets, base de test) : `($9 IS NULL OR a.object_id = $9)` donnait un
    `Seq Scan on anomaly` (199 996 lignes écartées, 42 ms) ; écrite seule,
    `Index Scan using anomaly_by_object`, `Index Cond: (object_id = $9)`, 1,0 ms ; paramètre `object` de
    `GET /v1/anomalies` (`Wanted` nommé, UUID au plus une fois, sinon 400 ; journalisé dans
    les critères de lecture) ; test `the_anomalies_of_one_object_read_by_its_filter`
    (pages d'un objet, avec `rule`, site hors domaine, objet illisible = objet inconnu,
    400) ; l'onglet lit `{ object: id }` pour tout objet autre qu'un domaine ou une entité,
    pages suivantes par la fenêtre de la grille (`useNextPage`) ; retirés : la lecture des
    dix pages du domaine, l'état « Lecture non disponible » d'un site, l'alerte « Liste
    peut-être incomplète » et l'état vide « dans les pages lues » (clés `unreadable`,
    `unreadableText`, `partialTitle`, `partial`, `noneRead`), la prop `domain` de l'onglet ;
    Vitest (`anomalies.test.ts` : filtre `object`, site, vide, page suivante par la fenêtre,
    curseur refusé ; `views.test.ts` : un site lu par `object`, `object` filtre texte de la
    barre, gardé de l'URL à la requête, REQ-021-18) ; parcours `anomalies` (lecture par
    `object`, sans `domain`) sous les deux navigateurs ; contrat, `schema.d.ts` et
    `filters.ts` des deux SDK régénérés
  - choix : `object` filtre texte de la barre de `/anomalies`, comme celui de
    `/v1/audit-events` ; la relance d'un curseur refusé reste unique (puis l'alerte), pour
    un objet comme pour un domaine ; dans le Vitest de l'onglet, une mise en page simulée
    (`clientHeight`, `getBoundingClientRect`) fait demander la page suivante par la fenêtre
  - reste : le filtre `$2` (domaine) de `open_page` garde la forme `$2 IS NULL OR …` (déjà
    là avant la 11k), à mesurer en plan générique de même ; aucun test n'affirme qu'`object`
    est écrit dans les critères de l'événement de lecture ; la mise en page simulée de
    `anomalies.test.ts` dépend du balisage d'`AnomalyGrid` (`data-index` des lignes)
- [x] 11l. `Gap` structuré côté serveur (A36) : `rule`, `resolved`, `expected`, `source` et
  révision au schéma ; `message` lisible, sans `{:?}` (`crates/kartenn-service/src/expected.rs`
  vers la ligne 471) — verify : `k-check -p kartenn-rest -p kartenn-service` (lacune
  servie avec ses membres ; message sans représentation interne) ; contrat et schémas
  régénérés
  - où : `crates/kartenn-service/src/expected.rs`, DTO `Gap` de `crates/kartenn-rest/src/`
    (`properties.rs`, `gap_property`) ; d'après A39 : `crates/kartenn-core/src/rules.rs`
    (`Violation`), `crates/kartenn-store-pg/src/{anomalies,write,idempotency}.rs`, une
    migration (`anomaly.detail`) ; verify étendu à `-p kartenn-store-pg -p kartenn-core`
  - dépend de : —
  - fait : `Violation.detail` (`with_detail`), journalisé avec la violation et rejoué par
    l'idempotence (`write.rs`, test `a_replayed_violation_keeps_its_detail`) ; migration
    0075 (`anomaly.detail jsonb`, droit `UPDATE (detail)`), écrit et rafraîchi avec le
    message (`anomalies::record`), lu par `open_page` et `find` ; `expected::Gap` compose
    message et détail `{property, resolved, expected, source, revision}`, valeurs en
    texte JSON dans le message (test `a_deviation_reads_without_internal_representation`,
    `deviations_become_anomalies` : message exact et détail en base) ; DTO `Gap` avec
    `rule`, `resolved`, `expected`, `source`, `revision` (`Gap::of`), le masquage d'une
    propriété sensible retire aussi `resolved` et `expected` du détail
    (`a_deviation_of_a_sensitive_property_hides_its_value`) ; contrat, `schema.d.ts` de
    `web/sdk` et `web/v2/sdk` régénérés ; `docs/spec/data.md`
  - choix : `resolved` et `expected` sont servis en JSON libre (`unknown` côté client),
    non `Object` comme `Explanation.value`, qu'une chaîne contredirait ; `expected` prend
    la forme du résolu, une valeur seule ou un tableau pour un ensemble (d'après la valeur
    analysée) ; masquée, une lacune garde `source` et `revision`, sans valeurs ;
    l'explication de la source reste dans le message seul (A39 ne la range pas au détail)
  - reste (relevés par la revue) : `source` et `revision` hors de `required` dans le
    contrat (`?:` côté client) alors que le serveur les émet toujours, à l'inverse de
    `resolved` et `expected` ; `resolved: null` confond « aucune valeur », « masquée » et
    « antérieure à 0075 » (voir la 11m) ; aucun test REST ne relie une lacune évaluée à sa
    réponse (le `detail` y est posé par un `UPDATE`), ni ne sert les valeurs d'une
    propriété sensible à qui a le droit de la lire ; l'exemple de la 11j (vers la
    ligne 2478) cite encore l'ancien format `expected [Text("prod")]`
- [x] 11n. Une lacune par (objet, propriété) (A39, relevé par la 11l) : l'identité d'une
  anomalie `prop.resolution.expected_mismatch` porte la clé de la propriété, pour que deux
  propriétés en écart sur un même objet ouvrent deux lacunes (index `anomaly_one_open`,
  `BTreeMap` de `anomalies::record`) ; le filtre `property` lit `detail` et non plus le
  préfixe du message — verify : `k-check -p kartenn-store-pg -p kartenn-rest` (deux
  propriétés en écart, deux lacunes ; l'une corrigée, l'autre reste)
  - où : `crates/kartenn-store-pg/migrations/` (index), `crates/kartenn-store-pg/src/anomalies.rs`,
    `crates/kartenn-rest/src/properties.rs`
  - dépend de : 11l
  - fait : migration 0076 (`anomaly_one_open` sur `(rule_id, object_id,
    coalesce(detail ->> 'property', ''))`) ; `anomalies::record` clé son `BTreeMap` et sa
    fermeture par (objet, règle, propriété) (`property_of`, miroir de l'index), son
    `ON CONFLICT` nomme la même expression ; filtre `property` sur
    `detail ->> 'property'` ; `gap_property` lit le détail d'abord. Tests
    `two_properties_in_deviation_are_two_gaps` (`kartenn-store-pg`, nouveau module
    `tests/it/anomalies.rs` : deux lacunes, filtre, l'une corrigée close, l'autre reste
    rafraîchie), `a_deviation_of_a_sensitive_property_hides_its_value` (deux lacunes sur
    un même objet, l'une masquée), `a_deviation_names_its_property` ; `docs/spec/data.md`,
    A39
  - choix : une lacune ouverte avant 0075 (sans `detail`) reçoit dans la migration
    `{property}`, la clé que son message cite en tête (une clé n'a pas de `:`, REQ-003-2) :
    elle garde son identité, son acquittement et son ouverture à sa réévaluation, et le
    filtre `property` la trouve ; ses valeurs restent `null` jusque-là. Index d'expression
    plutôt qu'une colonne de plus ; la clé est générique (`property` du détail, `''` sans
    elle), une règle qui n'en nomme pas garde une anomalie par objet. `gap_property`
    retombe sur le préfixe du message sans détail, pour que le masquage d'une propriété
    sensible tienne en toute circonstance
  - reste (relevés par la revue) : le rattrapage de 0076 n'a pas de test (le harnais
    migre jusqu'au bout avant un test), et rien ne prouve qu'une lacune antérieure à 0075
    garde identité et acquittement à sa réévaluation ; `property_of` lit
    `as_str().unwrap_or_default()`, qui diverge de `->>` pour une `property` non textuelle
    (théorique, `expected.rs` écrit toujours une chaîne) ; le test du store passe la portée
    `"prop.resolution."` et non `PROP_EXPECTED_MISMATCH.id` comme le code réel ; une
    lacune antérieure à 0075 a désormais un `detail` (`{property}`), la 11m ne la
    reconnaît qu'à l'absence de `source`
- [x] 11m. Lacune composée par l'écran (A36) : `GapList` dit la règle par son code, les
  valeurs attendue et résolue par leur libellé (`KeyLabel`), la source et sa révision ; le
  parcours `anomalies` n'assertit plus le texte anglais du serveur — verify : Vitest de
  `GapList` ; parcours `anomalies` sous les deux navigateurs
  - où : `web/v2/shell/src/views/anomalies/GapList.vue`, `web/v2/e2e/journeys/anomalies.spec.ts`
  - attention (revue de la 11l) : `resolved: null` ne distingue pas une valeur absente
    d'une valeur masquée ou d'une lacune antérieure à 0075 ; ne pas composer « aucune
    valeur » sur ce seul membre. Si l'écran a besoin de les distinguer, c'est un ajout au
    contrat, à arbitrer avant le code
  - dépend de : 11l
  - fait : `GapList` compose chaque lacune de ses membres, sans le `message` du serveur :
    propriété, valeur attendue et valeur résolue (`PropertyValue`, libellé d'une valeur de
    liste fermée), source (`KeyLabel`) et sa révision, objet, règle par son code
    (`KeyLabel`), ouverture ; Vitest `web/v2/shell/test/gaps.test.ts` (évaluée, sans valeur
    résolue, masquée, antérieure à 0075) ; le parcours `anomalies` assertit la valeur
    attendue par son libellé, l'absence de valeur résolue, la source et sa révision, et
    plus le texte anglais
  - choix : aucun ajout au contrat ; les trois cas se lisent sur les membres servis, tels
    que le contrat les décrit : `source` nul, lacune antérieure à 0075 (« Non détaillée »
    dans les deux cellules de valeur ; depuis 0076 son `detail` vaut `{property}`, seule
    l'absence de `source` la reconnaît) ; `source` posé et `expected` nul, masquée
    (« Masquée », une lacune évaluée porte toujours une valeur attendue, `RawInput` n'a pas
    de forme nulle) ; seul un `resolved` nul (ou un ensemble vide) à côté d'une valeur
    attendue dit « non renseigné » (`NoValue`) ; règle en dernière colonne, priorité 3
    (la même sur toute la liste, contrairement à `AnomalyGrid`) ; révision en chasse fixe
    après le mot « révision » ; source sans libellé (aucun espace `source` au dictionnaire)
  - reste : la distinction « masquée » repose sur l'invariant « une lacune évaluée porte
    `expected` » décrit par le contrat sans être un membre ; un membre explicite (`masked`)
    s'ajouterait si un cas l'enfreignait ; `declarationOf`/`targetOf` doublés avec
    `record/PropertiesTab.vue`, à extraire dans `views/data.ts` à la troisième vue.
    Relevés par la revue : la seule colonne `fill` (`object`) porte `priority: 2` et
    disparaît sous `md` avec le lien vers l'objet (débordement ou largeur non remplie
    possibles) ; un `expected` à `[]` se lit « non renseigné », sans test (une source
    peut-elle attendre l'ensemble vide ?) ; le test du cas masqué ne prouve que le rendu,
    l'absence de fuite reste garantie par `mask_gaps` sans test qui relie les deux ;
    commentaire d'en-tête de `GapList.vue` dense. Hors périmètre : `record/PropertyValue.vue`
    (vers la ligne 29) recopie un tiret au lieu de `NoValue` pour ses autres appelants
- [x] 12c. Réponses d'une écriture et écritures coupées sous `as_of`, prouvées : 202 sous
  une politique d'approbation avec le lien « Voir le changeset » ; 403 du serveur qui garde
  la saisie et dit l'alerte ; éditeurs et actions de la fiche coupés sous `as_of`, le
  brouillon daté laissé possible sous `valid_at` seul — verify : parcours `edit` et `time`
  sous les deux navigateurs ; lignes 35, 58, 60, 176, 178 et 203 cochées
  - où : `web/v2/shell/src/views/edit/` (`useWriteAction`, 12b), `views/RecordView.vue`,
    contexte temporel ; anciens parcours `web/e2e/journeys/{edit,time}.spec.ts`
  - dépend de : 12, 12b
  - fait le 2026-11-03 — porté au plus petit, le brouillon daté de la fiche (REQ-021-5 ;
    jusque-là tout était coupé hors du présent, `valid_at` seul compris) : `action.ts`
    (`useWriteAction`) ne coupe tout que sous `as_of` ; sous `valid_at` seul, « Appliquer »
    est listé refusé avec sa raison (`edit.why.dated`), dite aussi par le statut au
    Ctrl+Entrée, le brouillon permis ; `RecordView.vue` : « Modifier », « Clore… » et la
    création d'un VLAN coupés sous `as_of` seulement (`blockedBy`), « Changer de parent… »
    toujours au présent seul (`edit.move.dated`, `moves.ts` ne déplace qu'au présent ;
    relecture `ui-reviewer`, qui a aussi aligné `edit.why.dated` sur « Lecture datée ») ;
    `shortcuts/view.ts` : `runnable(id, période)` et `ViewAction.dated` (l'action `e` de
    la fiche joue sous `valid_at` seul, jamais sous `as_of`), `ShellFrame.vue` lui passe
    la période ; `ClosePanel.vue` simule sous `valid_at` seul (au présent, la version que
    la clôture nomme) ; `panel.ts` : `closeEditWithPage` ferme aussi le panneau ouvert
    sous `valid_at` quand `as_of` arrive (relecture `a11y-auditor`). Parcours
    `web/v2/e2e/journeys/edit.spec.ts` : « under an approval policy, Ctrl+Enter with its
    motive: « en revue », never applied, its changeset linked » (le `Link` du 202 lu,
    « Voir la proposition » du toast mène à `/changesets/<id>`, son motif affiché) ; « a
    refusal of the server (403) keeps the input and says the alert » ;
    `web/v2/e2e/journeys/time.spec.ts` : « under as_of, the editors and the actions of the
    record are off, each saying why » (« Modifier », `e`, « Clore… », la cellule de la
    liste) ; « a validity date alone leaves the dated draft possible » (étendu : la fiche
    un mois après, panneau ouvert par `e`, « Appliquer » refusé, Ctrl+Entrée au brouillon,
    compteur à 1). Vitest : `action.test.ts` (`valid_at` seul), `shortcuts.test.ts`
    (action `dated`), `edit.test.ts` (fermeture quand `as_of` arrive).
  - choix : le 403 provoqué par `page.route` (réponse `urn:kartenn:problem:forbidden` avec
    son `Request-Id`, comme le serveur la rédige) plutôt qu'un droit retiré entre la
    lecture et l'envoi : les comptes des parcours sont partagés par les tests en
    parallèle, retirer un rôle d'un groupe en casserait d'autres ; le lien du 202 garde le
    mot « Voir la proposition » du shell (`edit.seeProposal`, un 202 est une proposition),
    la ligne 58 le dit ; sous `valid_at` seul, le panneau lit l'objet au présent (la
    version sur laquelle le brouillon daté écrit), pas à la date de la fiche ; un droit
    manquant l'emporte sur la raison datée d'« Appliquer » (comme les refus de la 12b) ;
    `as_of` d'il y a une seconde et `valid_at` d'un mois après dans le parcours (l'objet du
    jeu existe aux deux dates ; `VALID_AT`, en 2026-01-01, le précède).
  - passe de corrections (revue) : la règle de période lue une fois, `writesOf`
    (`time/index.ts`), par `action.ts`, `runnable` (qui prend le `Time`), `RecordView.vue`
    et `ClosePanel.vue` ; `moveEntryRefusal` (`moves.ts`) porte le refus de « Changer de
    parent… », son Vitest dans `moves.test.ts` (`edit.move.dated` sous `valid_at` seul et
    sous `as_of`, le droit manquant gardé) ; le 403 simulé a la forme du serveur
    (`instance` `urn:uuid:…`, `Request-Id` en UUID v7).
  - reste : la soumission du brouillon sous `valid_at` seul (`valid_from` prérempli, le
    bouton du brouillon encore coupé par `drafts.why.past`, `draft/state.ts`, le retrait
    d'une opération) revient à la 14e ; « Créer » d'une liste (`c`) et le « + » de la
    Topographie sous `valid_at` seul reviennent à la 12d ; la raison
    d'une cellule sous `valid_at` seul dit encore « revenez au présent pour modifier »
    alors que « Modifier » mène au brouillon daté ; `e`, `c` sans effet ni phrase sous
    `as_of` (la raison sur le bouton seulement ; relecture `a11y-auditor`, cadre).
- [x] 14c. Changesets, reste prouvé : filtres de `/changesets` (auteur choisi, période
  `from`/`to`, état) ; « Retirer » un changeset en attente, exercé ; différentiel aux valeurs
  masquées par les droits et violations d'un changeset ; retour à « Mes changesets » avec
  « Appliqué » après une proposition validée puis annulée — verify : parcours `changesets`
  sous les deux navigateurs ; lignes 67, 70, 71 et 171 cochées
  - où : `web/v2/shell/src/changesets/`, `DiffList` ; ancien parcours
    `web/e2e/journeys/changesets.spec.ts`
  - dépend de : 14
  - fait le 2026-11-03 : rien à porter à l'écran, le parcours seul
    (`web/v2/e2e/journeys/changesets.spec.ts`) : le premier test garde Carol jusqu'à son
    retour à « Mes changesets », sa proposition « Appliqué » après la validation et
    l'annulation d'Alice (renommé « …, then undone, applied in her list ») ; « a proposal
    withdrawn by its author, then found by its author, its state and its period » (Carol
    retire, Alice filtre par l'identifiant de l'autrice, l'état « Retiré » de la liste
    fermée, « Soumis depuis » aujourd'hui puis « Soumis jusqu'au » hier, l'état vide et
    « Retirer les filtres ») ; « a sensitive value masked in the differential to a reader
    without its right » (une propriété sensible propre au test, `declare_property` puis
    `set_sensitivity`, une valeur posée sur un réseau : Alice la lit, Carol lit « masqué ») ;
    « the violations of a refused changeset, each by its rule » (un réseau en double, 422 :
    « Refusé », l'onglet Violations compté, chaque règle servie) ; `web/v2/PARITE.md`,
    lignes 67, 70, 71 et 171 cochées, `parity.test.ts` vert
  - choix : le lecteur sans droit sur la valeur est Carol (`core.changeset.read` sans
    `prop.<clé>.read`), aucun compte ajouté ; la propriété sensible est déclarée par le
    test (une clé par navigateur), pas par `global-setup.ts` ; le typage de `state` par le
    shell (`CLOSED` de `views/filters.ts`) gardé
  - reste : l'onglet Opérations d'une opération masquée montre `masked : true` et tait
    `values` (`operationLine`, `draft/operations.ts`) au lieu de dire « masqué »
- [x] 14d. Brouillon, reste prouvé : période de la soumission (`valid_from`, `valid_to`,
  dates d'effet du pied) ; réponse perdue à la soumission, « Réessayer » qui renvoie la même
  clé et ne donne qu'un changeset ; brouillon en lecture seule sous `as_of` — verify :
  parcours `draft` sous les deux navigateurs ; lignes 76 à 78 et 173 cochées
  - où : `web/v2/shell/src/draft/` (`DraftSubmit`, `LostSubmissions`) ; ancien parcours
    `web/e2e/journeys/draft.spec.ts`
  - dépend de : 14, 12b
  - fait le 2026-11-03 — `draft/state.ts` : `draftPeriods` (période par brouillon, pour la
    vie de la page ; sans choix, `valid_from` suit `valid_at`), `periodBody`, simulation à
    cette période, `submitDraft` qui l'envoie et la garde dans `LostSubmission` (rejouée
    telle quelle avec la clé) ; `writable` (`writesOf`) remplace `present` : sous
    `valid_at` seul le brouillon se soumet (REQ-021-5), sous `as_of` tout est refusé,
    « Enregistrer le brouillon » compris ; `DraftSubmit.vue` : « Choisir les dates d'effet »
    (motif de « Ajouter une description »), deux `DateTimeField` (`readonly` sous `as_of`),
    erreur d'ordre sur la fin, phrase du résultat qui dit la période, `CommitForm` désactivé
    sous `as_of` ; `DraftOperations.vue` : encart « Lecture seule » (`warn`, cadenas) en
    tête ; `DraftsView.vue` : entrées du menu refusées avec `disabled` (« Reprendre » de
    l'actif, « Supprimer » sous `as_of`, qui n'étaient pas marquées) ; chaînes
    `drafts.period.*`, `drafts.readOnly` ; Vitest `changesets.test.ts` (période envoyée,
    simulée et rejouée sous sa clé ; `valid_from` par défaut ; lecture seule sous `as_of`) ;
    parcours `draft` : « under valid_at, submitted from that date… », « an answer lost at
    the submission, then « Réessayer »: the same key, one changeset » (`page.route` coupe la
    réponse après l'envoi ; deux envois, même clé, un changeset), « under as_of the draft is
    read only… » (liste, panneau, page) ; `PARITE.md` lignes 76 à 78 et 173 cochées.
  - choix : la clé gardée est celle de `submitDraft`/`lostSubmissions` (14), pas
    `useWriteAction` (12b), qui porte l'écriture d'un panneau d'édition, non la soumission
    d'un brouillon ; la période vit côté shell (le brouillon du serveur n'en a pas, elle
    n'est que le corps de la simulation et de la soumission) ; motif de `CommitForm`
    désactivé nativement sous `as_of`, faute de `readonly` dans la bibliothèque (reste
    ci-dessous).
  - reste : `CommitForm` sans `readonly` ni `reason` : sous `as_of` le résumé et la
    description sortent de l'ordre de tabulation sans motif (relecture `a11y-auditor`, M14),
    à ouvrir dans `web/v2/ui` ; maquette `EcranSoumission` sans dates d'effet ni lecture
    seule.
- [x] 14e. Brouillon daté sous `valid_at` seul, reste (REQ-021-5, X5 ; relevé par la revue
  de la 12c, réduit après la 14d, qui a levé le retrait et la soumission hors présent et
  préremplit `valid_from` au pied) : « Ajouter au brouillon » sous une date de validité
  simule à cette date (`web/v2/sdk/src/draft.ts` vers la ligne 100, aujourd'hui sans
  période) ; la date de l'ajout est gardée avec le brouillon, et non plus dans une carte
  de la page (perdue au rechargement, suivie par le `valid_at` courant) ; tant qu'une
  soumission attend sa reprise, les dates du pied sont figées ; la reprise de l'encart
  (`LostSubmissions.vue`) est refusée sous `as_of` comme celle du pied ; un retrait sous
  `valid_at` prouvé — verify : Vitest de l'état du brouillon par période ; parcours `draft`
  et `time` (une opération ajoutée sous `valid_at`, retirée, puis une autre soumise à la
  date de son ajout après un retour au présent) sous les deux navigateurs
  - où : `web/v2/shell/src/draft/` (`state.ts` : `draftPeriods`, `periodBody` ;
    `DraftSubmit.vue`, `LostSubmissions.vue`), `web/v2/sdk/src/draft.ts`, `writesOf`
  - dépend de : 12c, 14d
  - fait le 2026-11-08 — SDK (`data/drafts.ts`) : la période d'un brouillon gardée en
    `sessionStorage` sous `PERIOD_KEY` + son identifiant (`period`, `setPeriod`, `onPeriod`,
    `periodOf`) ; un ajout sous `valid_at` seul (`addOperations`, la création par
    `addToActive`) date le brouillon qui n'a pas encore de période (`dateAddition`) ; la
    période oubliée avec le brouillon (soumis, supprimé, parti) ; `instance.ts` lit le
    contexte daté ; `draft.ts` : `moveInDraft` simule à `periodOf` (la même clé de cache
    que le pied), un déplacement remplacé en place date aussi ; `host.ts` : `draftPeriod`
    (hors API des plugins). Shell : `draftPeriods` (`state.ts`) lit et écrit par
    `draftPeriod`, réactif par `onPeriod` ; `awaiting` (la soumission perdue du brouillon) :
    sa période est celle envoyée, les dates du pied figées avec leur raison
    (`drafts.period.frozen`) ; `LostSubmissions.vue` : « Réessayer » refusé sous `as_of`
    (`drafts.why.past`). Vitest : `drafts.test.ts` (daté par l'ajout sous `valid_at`, gardé
    au présent ; choix du pied gardé ; rien sous `as_of` ; oublié à la soumission et à la
    suppression), `draft-move.test.ts` (simulé à la date de l'ajout, remplacé au présent),
    `changesets.test.ts` (dates figées en attente de reprise ; période relue après un
    rechargement, au présent ; « Réessayer » de l'encart refusé sous `as_of`). Parcours
    `draft` : « under valid_at, an operation added then removed, another submitted at the
    date of its addition back at the present » (`valid_from` du changeset à cette date).
    `design.md` (`sdk.drafts`) dit la période gardée.
  - choix : la période vit en `sessionStorage`, à côté du brouillon actif (le brouillon du
    serveur n'en a pas, ajouter un champ serait un contrat ; maintenu par l'utilisateur
    le 2026-11-09 : la période reste côté shell, sans champ au brouillon de la 024) ; elle
    vaut pour tout le brouillon et date du premier ajout fait sous `valid_at` seul (redit
    par la 14j : un brouillon garni au présent puis complété sous `valid_at` prend cette
    date pour toutes ses opérations, celles d'avant comprises), puis ne suit plus le
    `valid_at` courant ; un brouillon vidé par un retrait la garde (le choix
    du pied aussi) ; l'accès passe par `@kartenn/sdk/host`, non par `drafts` public ; un
    ajout sous `as_of` ne date rien (refusé de toute façon) ; le scénario du parcours vit
    dans `draft` (domaine propre, écritures isolées), `time` rejoué sans changement ; la
    phrase des dates figées dit l'état puis la sortie (relectures).
  - reste : sous `valid_at`, l'aperçu et l'état de brouillon des nœuds de la Topographie
    (`TopologyView.vue`, `preview` et `nodeState`) restent réservés au présent : les ouvrir
    demande une règle (la période du brouillon égale à la date lue, sinon l'aperçu mêlerait
    deux dates), l'adresse ajoutée par « + » ne se voit qu'au compteur ; un brouillon repris
    d'un autre onglet n'a pas la période de celui qui l'a daté (`sessionStorage` est par
    onglet). Relevés par les relectures : en attente de reprise, le motif (`CommitForm`)
    reste modifiable alors que « Réessayer » ne l'envoie pas (`ui-reviewer`) ; les segments
    d'un `DateTimeField` en lecture seule ne portent pas l'aide (sa raison) dans leur
    `aria-describedby`, seul le groupe la porte (à ouvrir dans `web/v2/ui`) ; « Réessayer »
    disparu après une reprise réussie laisse le focus sur `body` (d'avant cette tâche) ;
    le passage aux dates figées n'est pas annoncé (`a11y-auditor`). Relevés par la revue :
    `replace` de `shell/src/admin/draft.ts` (vers la ligne 63) ne date pas le brouillon,
    alors que `moveInDraft` le fait (sans effet si l'administration n'est pas datée, à
    vérifier) ; un brouillon garni au présent puis complété sous `valid_at` prend cette
    date pour toutes ses opérations, conforme au design mais que la ligne `choix :` laisse
    lire à l'inverse ; `dateAddition` lit le contexte après la réponse du serveur, si bien
    qu'une date changée pendant l'écriture date le brouillon à la nouvelle ; une clé de
    période reste dans l'onglet si le brouillon est supprimé hors de ses événements (sans
    effet, l'identifiant n'étant pas réutilisé).
- [x] 14f. « Renommer… » un brouillon depuis « Mes brouillons » (menu de la maquette E5,
  `E5Brouillon.vue` vers la ligne 111) : dialogue au titre requis (1 à 200 caractères,
  comptés comme le serveur), `PATCH /v1/drafts/{id}` sous `If-Match`, refusé sous `as_of`
  avec sa raison, focus rendu au menu de la ligne — REQ-021-11, 024/REQ-024-21 — verify :
  Vitest `shell/test/changesets.test.ts` (titre requis, rogné, sous `If-Match` ; refusé
  sous `as_of`) ; parcours `draft` « renamed from « Mes brouillons »… » sous les deux
  navigateurs
  - où : `web/v2/shell/src/draft/DraftsView.vue` (`menu`, `onMenu` ; motif du dialogue de
    rejet de `changesets/ChangesetView.vue`), `drafts.update` (`web/v2/sdk/src/data/drafts.ts`),
    `changesets/i18n/{fr,en}.json` (`drafts.rename*`), `crates/kartenn-rest/src/drafts.rs`
    (`TITLE_MAX`, `title`)
  - dépend de : 14
  - fait le 2026-11-11 — `DraftsView.vue` : entrée « Renommer… » (`pencil`) entre
    « Reprendre » et « Copier l'identifiant », refusée sous `as_of` (`drafts.why.past`) ;
    `Dialog` « Renommer « titre » » au formulaire du rejet (`TextField` « Titre » prérempli,
    requis, aide « De 1 à 200 caractères. », erreur vide ou trop longue, focus au champ en
    erreur, effacée dès que le titre vérifié change ; Entrée l'envoie) ; `rename` :
    `drafts.update(id, { title })` rogné, sous `If-Match` (`etagOf`), un titre inchangé
    ferme sans écrire, un refus dit par `RefusalAlert` dans le dialogue ; succès : toast
    « Brouillon renommé : … », liste relue, focus rendu au menu de la ligne (`useOpener`).
    Chaînes `drafts.rename*`, `drafts.renamed`. Vitest `shell/test/changesets.test.ts`
    (« renames a draft from « Mes brouillons »… », « under as_of, « Renommer… » is
    refused… ») ; parcours `draft` « renamed from « Mes brouillons »: the title required,
    the new one listed, the focus back on its menu » (Chromium et Firefox).
  - choix : la longueur comptée par point de code, comme `title` du serveur (`chars()`),
    `TITLE_MAX` recopié côté shell (le contrat ne le publie pas) ; pas d'icône sur
    « Renommer » du dialogue (la confirmation voisine de suppression n'en a pas) ;
    l'entrée seulement dans « Mes brouillons », comme la maquette (ni la page
    `/drafts/:id` ni le panneau) ; description qui dit que la soumission porte le motif,
    pas le titre.
  - reste : l'erreur est dite deux fois (le focus au champ, qui la lit par
    `aria-describedby`, et la région vivante de `TextField`), comme le rejet : à juger
    avec un vrai lecteur d'écran (`a11y-auditor`). Revue (mineurs) : course entre deux
    renommages (A envoyé, dialogue fermé par Échap, « Renommer… » sur B : le `rename()` de
    B rend la main sans rien dire tant que `renameBusy` tient, et la réponse de A ferme le
    dialogue de B ou y affiche son refus) ; après un 412, `renaming` garde l'ancienne ligne
    (titre du dialogue et test « inchangé » comparés au titre d'avant la relecture) ; ni le
    412 dans le dialogue ni le focus rendu au menu ne sont testés en Vitest ; la borne de
    200 caractères n'est publiée qu'en prose (`DraftChange.title`, sans `maxLength`) ; le
    `choix :` de la 14 dit encore « sans « Renommer » » ; commentaire d'en-tête trop long
    dans `e2e/journeys/draft.spec.ts`.
- [x] 14g. Opération masquée dite « masqué » (reste de la 14c) : l'onglet Opérations d'un
  changeset, comme la ligne d'opération du brouillon, montre `masked : true` et tait
  `values` (`operationLine`, `draft/operations.ts`) ; dire « masqué » comme le différentiel
  (`changesets.masked`) — REQ-021-10 — verify : Vitest `shell/test/changesets.test.ts`
  (`operationLine` d'une opération masquée) ; parcours `changesets` « a sensitive value
  masked… » étendu à l'onglet Opérations sous les deux navigateurs
  - où : `web/v2/shell/src/draft/{operations.ts,OperationText.vue}`,
    `changesets/ChangesetView.vue` (onglet Opérations), `web/v2/e2e/journeys/changesets.spec.ts`
  - dépend de : 14c
  - fait le 2026-11-11 — `draft/operations.ts` : `masked` parmi les membres tus ; une
    opération servie masquée porte le membre `values` marqué `masked` (en dernier), ses
    valeurs jamais montrées, même servies à côté ; `OperationText.vue` le dit
    « values : masqué » (`changesets.masked`, le mot du différentiel), en texte simple,
    ni `k-mono` ni `NoValue` (qui dit une absence). Vitest `shell/test/changesets.test.ts`
    (« says the values of a masked operation masked, never `masked: true` nor absent ») ;
    parcours `changesets` « a sensitive value masked… » : Alice lit
    `values : <valeur>` dans l'onglet Opérations, Carol `values : masqué`, ni `masked`
    ni la valeur (Chromium et Firefox).
  - choix : le mot en texte simple, comme le différentiel (`diffGroups`) ; le membre
    masqué en fin de ligne plutôt qu'à la place de `values` dans l'objet servi.
  - reste : une valeur réelle « masqué » se lit comme le masque (en chasse fixe à l'écran,
    pas à l'oreille), comme dans le différentiel : un texte masqué visuellement
    (« valeur masquée, droit de lecture manquant ») pour les deux, à trancher (`a11y-auditor`).
    Revue (mineurs) : le Vitest prouve `operationLine` mais pas le rendu
    d'`OperationText.vue` (vu par le parcours seulement) ; l'onglet Opérations d'Alice,
    valeur en clair, n'est pas audité par axe (seul celui de Carol l'est) ; ligne d'en-tête
    trop longue dans `e2e/journeys/changesets.spec.ts`.
- [x] 14h. Pied du brouillon en attente de reprise (relevés des relectures de la 14e) : le
  motif (`CommitForm`) en lecture seule, avec la raison des dates figées, tant qu'une
  soumission attend sa reprise (« Réessayer » ne l'envoie pas) ; le passage aux dates
  figées annoncé (région d'état) ; après une reprise réussie, le focus rendu (« Réessayer »
  disparu le laisse sur `body`) — REQ-021-11, REQ-006-15 — verify : Vitest
  `shell/test/changesets.test.ts` (motif `readonly` et sa raison en attente ; focus après
  la reprise) ; relecture `a11y-auditor`
  - où : `web/v2/shell/src/draft/{DraftSubmit.vue,LostSubmissions.vue,state.ts}`
    (`awaiting`, `drafts.period.frozen`), `CommitForm` (`readonly`, `reason`)
  - dépend de : 14e
  - fait le 2026-11-11 — `DraftSubmit.vue` : `motiveWhy` (comme `periodWhy`) met le motif
    en lecture seule sous `as_of` et tant qu'une soumission attend sa reprise
    (`drafts.motiveFrozen` sous le résumé et dans la description des champs) ; le refus d'une
    réponse perdue (`lostAnswer` : `failed` sans problème, en attente) porte la note
    `drafts.frozen` (« Motif et dates figés : … »), dite par son `alert` ; les champs de
    date gardent `drafts.period.frozen`, raccourcie à la conséquence. Focus après la
    reprise (2.4.3), déplacé seulement s'il est perdu (`body` ou élément détaché) :
    pied resté (page `/drafts/:id`, refus) → bouton principal du `SplitButton` (premier
    bouton de la rangée d'actions) ; pied parti avec son brouillon (panneau) → paragraphe
    « Aucun brouillon actif… » (`DraftOperations.vue`, `focusNone`, observateur `post` de
    `id` qui passe à `null`, ce qui couvre aussi la première soumission) ; encart
    `LostSubmissions.vue` répondu (événement `answered`) → « Réessayer » de l'encart suivant
    (`data-k-retry`), sinon le même paragraphe. Vitest `shell/test/changesets.test.ts` :
    « awaiting its retry, the motive is read only, why said; answered, the focus to the
    panel » (hôte `FootHost` : opérations et pied), focus après la reprise de l'encart, de
    l'encart suivant, et sur la page (brouillon 404 après l'application). Parcours `draft`
    « an answer lost at the submission… » : le focus sur « Aucun brouillon actif… » ; en-tête
    du fichier remis à 100 colonnes (reste de la 14f). `k-web2-e2e draft.spec.ts
    accessibility.spec.ts` (réponse perdue, axe de tous les parcours) : 4 passés.
  - choix : le passage aux dates figées est annoncé par la note de l'alerte du refus (déjà
    dite à cet instant), non par une seconde région `status` qui doublerait l'annonce ; la
    phrase entière une fois (l'alerte), sous les champs la seule conséquence
    (`ui-reviewer`) ; aucun changement de `state.ts` ni de `@kartenn/ui` (le bouton
    principal trouvé dans la rangée d'actions, sans `focus()` exposé par `SplitButton`) ;
    `DraftOperations.vue` touché hors de la ligne `où :` (la cible du focus quand le pied
    part avec le brouillon y vit).
  - reste : sur la page `/drafts/:id`, après une soumission acceptée, le brouillon lu reste
    montré (la requête retirée du cache par `forget`, l'observateur garde sa donnée, même
    sous un 404 du serveur) avec son pied et une simulation relancée : d'avant cette tâche,
    à traiter avec la page (et le focus à y rendre si elle passe à « brouillon parti ») ;
    le retour à des champs modifiables après une reprise refusée (422, 403) n'est pas
    annoncé, l'alerte du refus dit `edit.kept` (`a11y-auditor`, mineur) ; un clic dans le
    vide pendant la reprise laisse le focus perdu, qui va alors au bouton principal ;
    lecture de la raison d'un champ en lecture seule par un vrai lecteur d'écran à juger.
    Revue (mineurs) : le bouton principal est pris comme premier `button` de la rangée
    (`DraftSubmit.vue`), fragile face à l'ordre interne de `SplitButton` (la classe
    `.k-split-button__main` ou un `focus()` exposé le serait moins) ; l'assert de
    `changesets.test.ts` sur le pied d'un brouillon supprimé côté serveur fige le défaut
    consigné ci-dessus ; la règle « déplacé seulement s'il est perdu » et le brouillon
    disparu après une action ailleurs n'ont pas de test ; la clause dit encore « région
    d'état » alors que l'annonce passe par l'alerte (noté en `choix :`).
- [x] 14i. Accessibilité de deux composants de `@kartenn/ui` (relevés des 14 et 14e) : le
  badge d'une entrée de `SidebarNav` dit son unité aux technologies d'assistance (« À
  instruire » suivi de « 3 changesets à instruire », pas d'un « 3 » nu) ; les segments d'un
  `DateTimeField` portent l'aide et la raison dans leur `aria-describedby`, pas le seul
  groupe (WCAG 1.3.1, 4.1.2) — verify : Vitest des deux composants ; leurs scripts clavier
  `web/v2/e2e/components/` ; `k-web2-check --base main` ; relecture `a11y-auditor`
  - où : `web/v2/ui/src/components/SidebarNav/{SidebarNav.vue,sidebar.ts}`
    (`SidebarEntry.badge`), `DateTimeField/DateTimeField.vue` (`describedBy`, vers la ligne
    329) ; `web/v2/shell/src/frame/spaces.ts` (`badges`) ; lignes d'inventaire de
    `specs/020-systeme-de-design/design.md` et de la skill `kartenn-ui`
  - dépend de : 14
  - fait le 2026-11-11 — `SidebarEntry.badgeLabel` (facultatif) : le compteur dit avec son
    unité après le libellé (span `k-visually-hidden`), le nombre visible alors
    `aria-hidden` ; sans lui, le nombre seul comme avant ; rendu inchangé. Shell :
    `sidebarGroups` prend des `EntryBadge` (`count`, `label`), `ShellFrame.vue` écrit
    `badge.proposals` (pluriel de vue-i18n) ou `badge.proposalsMore` (« plus de 100
    changesets à instruire ») sous une page suivante. `DateTimeField` : chaque segment prend
    la description de `FormField` (refus puis aide, la raison d'un champ en lecture seule),
    l'heure et les minutes leur fuseau en plus. Vitest `ui/test/components-frame.test.ts`
    (unité dite, nombre caché ; sans `badgeLabel`, le nombre nu), `ui/test/dates.test.ts`
    (segments décrits en lecture seule et sous un refus), `shell/test/changesets.test.ts`
    (`badgeLabel` passé, rien à zéro, pluriels 1 et 3, borne) ; scripts clavier
    `e2e/components/sidebar-nav.spec.ts` (noms entiers, `exact`) et `date-time-field.spec.ts`
    (Année et Heure décrites par l'aide, Jour fautif par le refus et l'aide) ; instantanés
    ARIA de `SidebarNav` réécrits (noms des liens comptés). `k-web2-check --base main` :
    969 tests, 2720 captures sur 2720 (tout, `ShellFrame.vue` chargé par chaque page).
  - choix : le groupe garde sa description et chaque segment la répète, dans l'ordre du
    groupe (erreur, aide, puis fuseau), comme l'erreur avant la tâche (`a11y-auditor` :
    bavard, pas nuisible) ; l'infobulle décorative du rail garde le nombre nu, comme le
    badge visible ; un `badgeLabel` sans `badge` n'est ni montré ni dit ; les maquettes
    (`shell/src/_ui/ecrans/sidebar.ts`, `donnees017.ts`) gardent leurs compteurs nus ; les
    instantanés ARIA « confortable » de `SidebarNav` gagnent aussi l'état
    `single-keys-off`, absent de la référence de main.
  - reste : la lecture réelle du nom d'un lien compté et de la double description des
    segments par NVDA, JAWS ou VoiceOver, à juger au lecteur d'écran ; les maquettes à
    doter de `badgeLabel` si leurs instantanés doivent montrer l'unité. Revue (mineurs) :
    « jamais nombre nu » du skill `kartenn-ui` et de l'inventaire de la 020 (ligne 18)
    promet trop — écrire « nombre nu sans `badgeLabel` » ; les maquettes `_ui`
    (`ecrans/sidebar.ts`, `donnees017.ts`) gardent des compteurs nus ; le choix de la clé
    `badge.proposals` / `badge.proposalsMore` (`ShellFrame.vue`) n'est pas testé (le
    parcours `changesets.spec.ts` pourrait vérifier « changesets à instruire » dans le nom
    du lien) ; la relecture `a11y-auditor` n'est notée qu'en parenthèse, sans son verdict.
- [x] 14j. Brouillon daté, reste de la revue de la 14e : `dateAddition`
  (`sdk/src/data/drafts.ts`) lit le contexte avant l'envoi, non après la réponse (une date
  changée pendant l'écriture date aujourd'hui le brouillon à la nouvelle) ; `replace` de
  `shell/src/admin/draft.ts` (vers la ligne 63) ne date pas le brouillon comme
  `moveInDraft` (vérifier : sans effet tant que l'administration n'est pas datée, sinon
  l'aligner) ; la ligne `choix :` de la 14e redite (un brouillon garni au présent puis
  complété sous `valid_at` prend cette date pour toutes ses opérations) — REQ-021-5 —
  verify : Vitest `sdk/test/drafts.test.ts` (la date lue avant l'envoi) ; Vitest de
  l'administration si `replace` change
  - où : `web/v2/sdk/src/data/drafts.ts` (`dateAddition`, `addOperations`, `addToActive`),
    `web/v2/shell/src/admin/draft.ts`
  - dépend de : 14e
  - fait le 2026-11-11 — SDK (`data/drafts.ts`) : `additionDate` (le `valid_at` du
    contexte sous `valid_at` seul, sinon rien) lu avant tout envoi, `dateAddition(id, at)`
    date le brouillon à cette date une fois l'ajout écrit ; `addOperations` passe par
    `addAt` (la date lue à l'appel, gardée pour le nouvel essai après un 412),
    `addToActive` lit la date une fois, pour l'ajout au brouillon actif comme pour le
    brouillon qu'il crée, ou la reçoit (`at`, second argument facultatif, hors API des
    plugins) ; `draft.ts` : `moveInDraft` lit la date à son appel, avant la lecture du
    brouillon actif, et la passe aux deux branches (remplacement en place, `drafts.update` ;
    ajout, `add(ops, at)`). `admin/draft.ts` : `replace` inchangé, son commentaire dit
    pourquoi (l'administration n'écrit qu'au présent : `present` faux sous `valid_at` seul
    aussi, refus `edit.why.past` de `access.ts`, `roleRefusals`, des listes). Vitest :
    `drafts.test.ts` (« dated at the date read before the addition is sent… » : date
    changée pendant la création puis pendant l'ajout, rien de daté pour un ajout envoyé au
    présent), `draft-move.test.ts` (remplacé en place ou ajouté, la date changée pendant la
    lecture du brouillon actif hors cache : daté et simulé à la date de l'appel). La ligne `choix :` de la 14e redite ; `design.md` (`sdk.drafts`) dit la
    date lue avant l'envoi.
  - choix : la date se lit à l'appel de l'ajout, non juste avant la requête (après la
    lecture de l'`ETag`) : c'est la date sous laquelle l'utilisateur a agi, et le nouvel
    essai après un 412 garde la même ; `dateAddition` prend la date en argument plutôt que
    de la relire (une seule lecture par ajout) ; pas de Vitest de l'administration,
    `replace` ne changeant pas de comportement.
  - reste : relevés par la revue (mineurs) : aucun test ne prouve que le second essai après
    un 412 garde la date lue à l'appel ; dans `drafts.test.ts`, un seul `it` tient trois cas
    (création, ajout, ajout envoyé au présent), à scinder.
- [x] 14k. Brouillon supprimé dans un autre navigateur, dit dans l'onglet qui l'avait actif,
  tenu par un parcours (reste de la 14 : sous Chromium l'événement `draft` n'était pas venu
  en 20 s, avant la clôture de la tâche 5) : le toast du cadre (`drafts.gone`), le compteur
  à 0, la page des brouillons sans lui — REQ-021-11, 024/REQ-024-21 — verify : parcours
  `draft` sous les deux navigateurs (deux contextes de la même identité)
  - où : `web/v2/e2e/journeys/draft.spec.ts` (« … resumed and deleted elsewhere »,
    `choose`), `web/v2/shell/src/frame/ShellFrame.vue` (brouillon parti par l'événement),
    `web/v2/sdk/src/data/drafts.ts` (`gone`)
  - dépend de : 14, 5
  - fait le 2026-11-11 — parcours `draft` « … resumed and deleted elsewhere » : le brouillon supprimé dans le second contexte d'Alice, le premier onglet, resté
    sur « Mes brouillons » avec ce brouillon actif et le panneau ouvert, montre sans
    rechargement le toast `drafts.gone` dans la région « Notifications », le compteur à
    « Brouillon 0 opération », la ligne retirée de la grille, puis passe axe ; en-tête du
    fichier complété. Aucun changement du shell ni du SDK : l'événement `draft` `deleted`
    arrive depuis la tâche 5. `k-web2-e2e draft.spec.ts accessibility.spec.ts`
    (« deleted elsewhere », « every journey runs axe ») : 4 passés ; le seul parcours
    répété trois fois par navigateur (`--repeat-each=3`) : 6 passés.
  - choix : l'attente du toast au délai par défaut d'`expect` (5 s), non au budget de 2 s
    de la tâche 5 (tenu par `events.spec.ts`), les parcours des deux navigateurs écrivant
    les brouillons d'Alice côte à côte ; les assertions dans le `try`, le second contexte
    encore ouvert ; titre du test inchangé (`web/v2/PARITE.md` le cite, vérifié par
    `shell/test/parity.test.ts`).
- [x] 14l. Modifier une opération du brouillon (design 021 « Écrans », Brouillons :
  « liste des opérations (retirer, modifier) » ; seul le retrait est là) : forme arbitrée
  par l'utilisateur le 2026-11-11 (Q-021-17) — « Modifier » sur la ligne
  rouvre le panneau d'édition de la sorte (`EditPanel`, REQ-021-36) prérempli de
  l'opération, qui la remplace en place sous `If-Match` (comme `replace` de
  `shell/src/admin/draft.ts`), la simulation relancée — REQ-021-11, 36 — verify : Vitest
  de l'état du brouillon (remplacement en place, `If-Match`) ; parcours `draft` sous les
  deux navigateurs
  - où : `web/v2/shell/src/draft/{DraftOperations.vue,operations.ts}`,
    `web/v2/shell/src/views/edit/` (`EditPanel`, `panel.ts`), `drafts.update`
  - dépend de : 14
  - fait le 2026-11-11 — SDK (`data/drafts.ts`) : `replaceOperation(read, index, op)`, offert
    au shell seul (`replaceInDraft` de `@kartenn/sdk/host`) : `PATCH` des opérations, celle
    de l'index remplacée, sous l'`If-Match` du brouillon tel que sa ligne était lu ; après
    un 412, relu, retenté une fois si la place tient encore la même opération, sinon le
    conflit ; la date lue à l'appel (`additionDate`) date le brouillon comme un ajout (règle
    de la 14j), gardée au second essai. Shell : `fields.ts` `editOfOperation` (la sorte, la
    création ou l'objet modifié, les valeurs du formulaire tirées des membres de
    l'opération : `prefix` → `value`, références, ancres, drapeaux ; `null` pour une
    clôture, un déplacement, une propriété, `within`) ; `panel.ts` : `replace` (`DraftSlot` :
    le brouillon lu et l'index) sur les demandes `create` et `edit`, `lineToReturn` ;
    `DraftOperations.vue` : bouton « Modifier l'opération n : … » (`pencil`, `data-k-modify`)
    avant « Retirer », refusé avec sa raison sous `as_of` (`drafts.why.past`) et pour une
    opération qu'aucun panneau n'écrit (`drafts.why.noPanel`) ; `EditPanel.vue` : titre de la
    ligne, phrase d'en-tête (`edit.replace.lead`), formulaire prérempli (les valeurs de
    l'opération sur l'objet lu), un seul bouton « Remplacer dans le brouillon » (rien
    d'appliqué tout de suite), « Rien n'a changé » sans envoi, l'objet déjà tel que saisi dit
    (`edit.replace.same`), refus 412 et 404 dans leurs mots ; succès « Opération n
    remplacée : … » (`words.saved` de `action.ts`), la simulation relancée par la nouvelle
    version du brouillon ; le focus rendu au « Modifier » de la ligne, l'onglet Brouillon
    remontré par le cadre (`ShellFrame.vue`) quand l'onglet Détail avait pris sa place.
    Vitest : `sdk/test/drafts.test.ts` (en place sous l'`If-Match` lu ; 412 retenté si
    l'opération est la même, sinon conflit ; daté à l'appel, gardé après le 412),
    `shell/test/draft-replace.test.ts` (`editOfOperation` ; une modification remplacée sous
    `If-Match`, simulée de nouveau, le focus sur sa ligne ; une création gardant son nom
    local ; un 412 dont la place a changé dit, rien de retenté). Parcours `draft` « an
    operation modified: its edit panel prefilled, replaced at its place, simulated again »
    (doublon corrigé, simulation valide, focus, brouillon relu) ; « … read only » vérifie
    aussi « Modifier » refusé sous `as_of`. `design.md` (`sdk.drafts`) dit le remplacement.
  - passe de correction (revue) : une modification remplacée garde l'`expected` d'origine
    (Q-021-20, arbitré le 2026-11-11 : ce qu'elle écrivait et ce qui est saisi en plus),
    l'objet changé depuis dit en tête du panneau (« L'objet a changé depuis cette
    opération », les champs qui ont bougé, lus dans la première page de l'historique de
    l'objet) ; « Reprendre sur la version actuelle » la réécrit sur la version lue, ce qui
    en diffère seulement, le focus au premier champ ; une référence locale (`{local: n}`,
    ancre comprise) gardée : `local:<n>` dans le formulaire (`textOf`, `ref`, `anchor` de
    `fields.ts`), l'objet de l'opération qu'elle désigne offert dans le champ avec ceux lus,
    nommé par ce que nomme son opération (« opération n du brouillon », `draftedCreations`).
    Vitest `draft-replace.test.ts` : `expected` d'origine gardé et champ bougé dit ;
    « Reprendre sur la version actuelle » ; création à référence locale remplacée ;
    `editOfOperation` et `create` d'une référence et d'une ancre locales. `design.md`
    (Brouillons) et `decisions.md` (Q-021-20). Maquette E5 amendée (validée par
    l'utilisateur, notée sous Q-021-17) : `EcranBrouillon.vue` porte le crayon avant
    « Retirer », refusé avec sa raison pour la propriété posée (`noPanel` de
    `DRAFT_OPERATIONS`) ; les 24 captures E5 (simulation, violations, soumission ; quatre
    largeurs, deux modes) réécrites et relues, seules à changer parmi celles des écrans.
  - choix : une création garde son `local`
    (021/A13) ; le bouton « Modifier » montré sur chaque ligne, refusé avec sa raison plutôt
    que caché ; un `Button` primaire au lieu d'un `SplitButton` à une entrée ; l'égalité de
    l'opération à sa place comparée par son JSON (servi par le même serveur) ;
    `editOfOperation` dans `fields.ts` (à côté de `OPERATION_MEMBERS`), non dans
    `draft/operations.ts` ; un clic qui laisse le focus sur `body` traité comme un
    déclencheur parti (le focus va à la ligne), et `closeEdit` ne rend plus le focus à
    `body` pour aucune demande d'édition (`#k-surface` alors, comme un déclencheur parti) ;
    seuls les objets créés par une opération d'avant la ligne remplacée sont offerts à ses
    références (un nom local désigne un objet créé plus tôt, A13) ; Vitest dans un fichier neuf plutôt que dans
    `changesets.test.ts` (déjà 1 000 lignes). Relectures : le
    retour du focus effacé aussi quand le brouillon est parti ou illisible (`a11y-auditor`) ;
    l'onglet Brouillon remontré seulement pour le brouillon actif hors de sa page (sur
    `/drafts/:id`, la ligne reprend le focus, le panneau laissé tel quel).
  - reste : revue (mineurs) : `returning` (`lineToReturn`)
    n'est pas effacé quand son brouillon n'est plus l'actif du cadre (`ShellFrame.vue`,
    vers la ligne 270 : le cadre l'ignore, aucune liste ne le consomme) ;
    `replaceOperation` (`drafts.ts`, vers la ligne 311) se replie sur l'`ETag` du cache
    quand `read.etag` est nul, qui peut être d'une autre version que la ligne lue ; une
    relecture en 404 après un 412 rend un conflit au lieu du brouillon parti (`gone`) ;
    l'objet bougé n'est cherché que dans la première page de son historique (sinon la
    phrase seule, sans champs) ; autres relevés : les lignes sont clées
    par leur index (une renumérotation entre l'ouverture et le retour mène le focus au
    « Modifier » voisin) ; brouillon illisible au retour, le focus reste perdu (l'état
    d'erreur n'a pas de cible) ; cibles de 24 px côte à côte à 4 px (`gap="1"`) à vérifier
    au rendu en densité compacte ; à juger au lecteur d'écran : l'annonce « Opération n
    remplacée » suivie du nom du bouton focalisé, le crayon refusé et sa raison, la phrase
    d'en-tête lue alors que le focus va au premier champ.
- [x] 12d. Créer sous `valid_at` seul (REQ-021-5, X5 ; relevé par la revue de la 12c) :
  « Créer » (`c`) des listes et « + » de la Topographie mènent au brouillon daté sous une
  date de validité, comme « Modifier » de la fiche ; « Appliquer » refusé avec sa raison ;
  rien sous `as_of` ; la raison d'une cellule sous `valid_at` dit le brouillon daté, et
  non « revenez au présent » — verify : Vitest des raccourcis par période ; parcours
  `time` (création au brouillon sous `valid_at`) sous les deux navigateurs
  - où : `web/v2/shell/src/views/{ListView,TopologyView}.vue`, `shortcuts/view.ts`
    (`dated`), `views/edit/` ; `writesOf`
  - dépend de : 12c
  - fait le 2026-11-08 — `ListView.vue` : `c` (« Créer ») et `e` (la ligne) marqués `dated`,
    joués sous `valid_at` seul (le panneau d'édition, `useWriteAction` de la 12c refuse
    « Appliquer » avec `edit.why.dated`), jamais sous `as_of` ; `TopologyView.vue` :
    `addRefusal` (« + » de l'arbre et « Ajouter dans » du contenu) ne refuse plus que sous
    `as_of` (`writesOf(...).draft`, raison `edit.why.past`), `c` et `e` marqués `dated` ;
    `fields.ts` : `editability` prend les `Writes` (`time/index.ts`, type exporté) et
    distingue `past` (`as_of`, avant les droits) de `dated` (`valid_at` seul, après les
    droits : un lecteur lit la permission qui lui manque, non un brouillon qu'il ne peut
    écrire) ; `whyWords` (`inline.ts`) et `EditHint.vue` disent `edit.why.inPlaceDated`
    (« Lecture datée : rien ne se modifie en place, « Modifier » mène au brouillon daté. »).
    Vitest : `dated-create.test.ts` (la liste et la Topographie montées sous `valid_at`,
    `as_of`, les deux, le présent : `runnable` de `c` et `e`, le « + » et « Ajouter dans »
    ouverts puis refusés avec leur raison, la demande `create` du panneau) ;
    `edit.test.ts` (`editability` par période, lecteur compris ; la cellule `dated` sous
    `valid_at`, `past` sous `as_of`). Parcours `time` : « under a validity date alone,
    « Créer » of a list and the « + » of the topography go to the dated draft » (la cellule
    qui dit le brouillon daté, `c` sur `/addresses` et « Appliquer » refusé, le « + » d'un
    réseau au clavier jusqu'à « Brouillon 1 opération », le « + » refusé sous `as_of`).
  - choix : `e` des lignes d'une liste et du nœud choisi de la Topographie marqués `dated`
    aussi (la raison de la cellule renvoie à « Modifier », qui doit alors jouer) ; la
    phrase de la cellule nomme « Modifier » sans la touche `e` (dans l'onglet Contenu d'une
    fiche, `e` modifie la fiche, non la ligne) ; sous `as_of`, `past` l'emporte sur un droit
    manquant (comme avant) ; aucun changement à `draft/` ni au SDK (14e).
  - reste : `c` et `e` refusés restent muets au clavier (sous `as_of`, ou sans le droit) :
    la liste n'a pas de bouton « Créer » qui porte la raison, et aucun statut ne la dit
    (relecture `a11y-auditor`, déjà notée par la 12c) ; dans l'onglet Contenu d'une fiche
    et le contenu d'un nœud, « Modifier » de la phrase désigne la fiche ou le nœud choisi,
    non la ligne (relecture `ui-reviewer`) ; la simulation d'un ajout sous `valid_at` reste
    au présent (14e). Relevés par la revue : `dated-create.test.ts` monte la liste vide, si
    bien que `e` d'une ligne n'y est jamais prouvé `runnable` sous `valid_at`, et le cas
    `valid_at` et `as_of` ensemble n'y est tenu que pour la liste, pas pour la Topographie ;
    sous `valid_at`, l'aperçu et l'état de brouillon des nœuds de la Topographie
    (`TopologyView.vue` vers les lignes 324 et 390) restent réservés au présent, l'adresse
    ajoutée par « + » ne se voit qu'au compteur du brouillon (avec la 14e) ;
    `record/NetworkVlanTab.vue` (vers la ligne 121) refuse encore sous `valid_at` par
    « revenez au présent ».
- [x] 12e. « En revue » avec son motif sur la cellule (acceptation de REQ-021-15, relevée à
  la clôture de la 12) : la cellule d'une écriture retenue (202) dit pourquoi, le motif de
  la réponse (`changeset.approval.self`, `changeset.approval.self.missing`), en
  description et au survol, non plus par le seul toast — verify : script clavier
  `data-grid.spec.ts` (Ctrl+Entrée → « en revue », sa raison en description et en
  infobulle) ; Vitest de la grille et de `useInlineEdit` ; parcours `edit` (politique
  d'approbation : la cellule décrite par le motif)
  - où : `web/v2/ui/src/components/DataGrid/DataGrid.vue` (comme `readonlyReason`),
    `DataGrid.catalogue.ts` ; `shell/src/views/edit/{inline.ts,say.ts}`, `ListView.vue`,
    `record/ContentTab.vue` ; tests `ui/test/`, `shell/test/edit.test.ts`,
    `e2e/components/data-grid.spec.ts`, `e2e/journeys/edit.spec.ts`
  - dépend de : 12
  - fait le 2026-11-11 — `DataGrid` prend `stateReason(row, column)` : pourquoi une cellule
    est dans son état, demandé pour une cellule à état autre qu'une erreur ; ses phrases
    rejoignent celles de `readonlyReason` (une phrase cachée par texte distinct,
    `aria-describedby` de la cellule, les deux jointes si une cellule a les deux) et
    l'infobulle au survol (Échap la ferme) ; une erreur garde sa seule bulle. Catalogue :
    `cell-states` décrit sa cellule en revue ; la démo `keyboard` retient en revue un
    `commit` marqué `now`. Shell : `heldWords` (`say.ts`, la phrase du catalogue d'après le
    code, sinon le message du serveur), partagé par le toast et par `useInlineEdit`, dont
    l'entrée « en revue » garde le motif (`held`) et que `stateReason` rend ; branché dans
    `ListView` et la grille du contenu, dont le pied (`EditHint`, `held`) dit aussi le motif
    de la cellule de la focalisation, avant ses touches (vu au clavier, pas seulement lu :
    relecture `a11y-auditor`). 020 design (contrat d'édition, inventaire), skill.
    Tests : `ui/test/state-reason-mounted.test.ts` (raison d'une cellule en revue, aucune
    sur une erreur, raison d'une cellule sans éditeur gardée), `shell/test/edit.test.ts`
    (202 : `stateReason` = `edit.reason.held` sur la cellule, rien sur une autre), script
    `data-grid.spec.ts`, parcours `edit` (description de la cellule en revue).
  - choix : propriété à part (`stateReason`) plutôt qu'une variante objet de `CellState`
    (`{ review, reason }`) : le type est partagé par `TreeGrid`, `Tree` et
    `ExpandableGrid`, qui lisent un objet comme une erreur ; la phrase du motif est celle
    du toast, sans « En revue : » devant (le mot caché « (en revue) » est déjà dans le nom
    de la cellule) ; pas de capture nouvelle (description et infobulle seulement, une
    infobulle à délai se capture mal) ; une cellule qui a les deux phrases (en revue, puis
    devenue sans éditeur) les montre jointes dans l'infobulle, chacune finie par son point ;
    la même phrase rendue par les deux n'est dite qu'une fois (un id, une ligne de
    l'infobulle : `notesOf`, corrections de la revue).
  - reste : le `TreeGrid` n'a pas `stateReason` (12f) ; infobulle de raison ni au focus,
    ni au toucher, ni survolable, Échap qui la ferme sans arrêter la touche → 12m ;
    `ExpandableGrid` sans `stateReason` ni `readonlyReason`, ligne en revue (`rowState`)
    muette sur son motif → 12n. Mineurs de la revue : l'infobulle reste-t-elle au-dessus
    de l'éditeur quand on entre à l'édition au clavier sous le pointeur (à vérifier) ;
    `activeHeld` de `ListView` parcourt `shownRows` (la grille du contenu lit
    `activeRow`) ; le motif `changeset.approval.self.missing` n'est pas éprouvé sur la
    cellule (seul `changeset.approval.self` l'est).
- [x] 12f. Édition en place dans l'arbre, reste de la 12a : `TreeGrid` prend
  `readonlyReason` et `stateReason` (description d'une cellule sans éditeur ou en revue,
  comme le `DataGrid`, 12e) ; 202, 422 et 403 prouvés dans l'arbre ; un seul `useNodeEdit`
  pour l'arbre et la grille du contenu d'un même nœud (états de cellule et ligne du motif
  partagés) ; `kindOf` de `nodes.ts` sous garde de type — REQ-021-15, 16, 37 — verify :
  Vitest `trees-mounted.test.ts` (raisons en description), `edit.test.ts` (202, 422, 403
  sur une cellule de l'arbre) ; script `tree-grid.spec.ts`
  - où : `web/v2/ui/src/components/TreeGrid/TreeGrid.vue` (imiter `reasonOf` du
    `DataGrid`), `TreeGrid.catalogue.ts` ; `shell/src/views/edit/nodes.ts`,
    `TopologyView.vue`, `record/ContentTab.vue`
  - dépend de : 12a, 12e
  - découpée le 2026-11-12 (un composant de `web/v2/ui`, puis un écran du shell) : le
    `TreeGrid` en 12f1, le branchement du shell et ses preuves en 12f2.
  - [x] 12f1. `TreeGrid` prend `readonlyReason` et `stateReason`, mêmes noms et même
    comportement que le `DataGrid` (12e, 12m) — verify : Vitest `trees-mounted.test.ts`
    (raisons en description) ; script `tree-grid.spec.ts`
    - où : `web/v2/ui/src/components/TreeGrid/TreeGrid.vue`, `TreeGrid.catalogue.ts`,
      `DataGrid/DataGrid.vue` (`reasonOf`, `reasonTip`)
    - fait le 2026-11-12 — `internal/cellReasons.ts` (`useCellReasons`, `notesOfTexts`) :
      les phrases distinctes des lignes rendues, cachées hors des lignes (`reasons`,
      `reasonOf` → `aria-describedby`), et l'infobulle unique de la grille (survol après
      le délai, focus clavier aussitôt, appui épinglé, survolable, Échap arrêté, fermée à
      l'ouverture d'un éditeur, texte qui suit sa cellule), tirées du `DataGrid`, qui s'en
      sert désormais (comportement inchangé, scripts `data-grid.spec.ts` et
      `touch.spec.ts` verts). `TreeGrid` : props `readonlyReason`, `stateReason` (sur la
      cellule du nœud, colonne `""`, comme sur les autres ; rien sur un avis ni sur la
      ligne « Afficher la suite »), description d'une cellule = son erreur, sinon ses
      raisons (`describedBy`), infobulle (`onCellEnter`, `onCellLeave`, épinglage dans
      `onRowClick`). Catalogue : `editing` (occupation sans éditeur et pourquoi ; la
      cellule en revue dit ses deux phrases), `keyboard` (nom de l'entité racine sans
      éditeur et pourquoi ; Ctrl+Entrée retenu en revue avec son motif). 020 design
      (inventaire 21, contrat d'édition), skill (ligne `TreeGrid`, `Tree`). Tests :
      `ui/test/trees-mounted.test.ts` (sans éditeur, en revue, erreur seule ; deux
      phrases jointes, la même une fois ; arbre en lecture seule sans `readonlyReason` ;
      phrases hors des lignes), `e2e/components/tree-grid.spec.ts` (Ctrl+Entrée → en
      revue, motif en description et en infobulle au focus ; cellule du nœud sans éditeur
      au focus ; Échap ferme sans autre effet ; survol ; autre cellule et sortie de
      l'arbre la ferment).
    - choix : composable partagé plutôt qu'une copie dans `TreeGrid` (un seul contrat,
      un seul comportement) ; `notesOf` reste à chaque grille (le `DataGrid` y ajoute
      `rowReason`, que le `TreeGrid` n'a pas faute de `rowState`) ; Échap annule aussi
      la bascule en cours vers une autre cellule (`switching`, mineur de la revue de la
      12m) ; les phrases restent hors des lignes recyclées (invariant de
      `virtualRows.ts` intact, l'éditeur ouvert reste une ligne gardée) ; pas de capture
      nouvelle (description et infobulle seulement) ; captures `TreeGrid` `editing` (clair,
      sombre) et confortables réécrites : la légende de l'état, plus longue, va à la
      ligne (rendu de l'arbre inchangé), et l'arbre ARIA des confortables gagne les états
      ajoutés depuis la 020-26 (`capped`, `moved-preview`, `drop-*-preview`).
    - corrections des relectures : l'infobulle de la cellule du nœud, large comme l'arbre,
      se place sous son libellé et non au milieu de la ligne (`Tooltip` prend `placeAt`,
      facultatif : ce contre quoi la bulle se place dans son ancre, qui garde focus,
      survol et appui ; `ui-reviewer`, éprouvé par `tree-grid.spec.ts`) ; le pointeur sur
      le « + » du nœud ferme l'infobulle de la cellule, le bouton ayant la sienne
      (`cellReasons.over`, `a11y-auditor` : jamais deux infobulles).
    - reste : la cellule du nœud ne rend pas d'état de cellule (`cellState` sur la
      colonne `""` : ni marque ni mot caché, seulement sa raison) — sans effet tant que
      l'arbre de la Topographie n'édite pas la valeur d'un nœud (12a). Relectures
      (mineurs) : `cellReasons.over` sans test (survol du « + » d'un nœud qui a une
      raison : aucun état de catalogue ne les réunit) et branché sur la cellule du nœud
      seule (une cellule de `DataGrid` à widget et à raison ferait voir deux infobulles) ;
      le pointeur revenu du « + » sur la cellule ne rouvre l'infobulle qu'en y rentrant ;
      les identifiants des phrases cachées suivent la fenêtre rendue (recalculés avec les
      cellules) ; restitution par un vrai lecteur d'écran non éprouvée. Revue (mineurs) :
      l'épinglage au toucher dans le `TreeGrid` n'est pas testé (`onRowClick`) ; un clic
      sur le chevron d'un nœud qui a une raison épingle l'infobulle en dépliant le nœud,
      ni dit ni testé ; ligne réécrite du design 020 (~992) non rejustifiée ; la cellule du
      nœud prend un `stateReason` sans montrer son état (« retenue » décrit, non nommé,
      à régler en 12f2) ; hors périmètre : une infobulle ouverte au survol suit l'élément
      de sa cellule, qui peut être recyclé pour une autre ligne pendant un défilement
      (`anchorTip` lit `dataset.row` sans réactivité).
  - [x] 12f2. Le shell branche les raisons de l'arbre et partage son édition : la
    Topographie passe `readonlyReason` et `stateReason` à son `TreeGrid` (le pourquoi
    d'une cellule de l'arbre en description, plus seulement par le statut de la vue) ;
    202, 422 et 403 prouvés sur une cellule de l'arbre ; un seul `useNodeEdit` pour
    l'arbre et la grille du contenu d'un même nœud (états de cellule et ligne du motif
    partagés) ; `kindOf` de `nodes.ts` sous garde de type — REQ-021-15, 16, 37 — verify :
    `shell/test/edit.test.ts` (202 : `stateReason` sur la cellule de l'arbre ; 422 et 403
    sur une cellule de l'arbre) ; parcours `tree` et `edit`
    (`accessibility.spec.ts -g "every journey runs axe"`)
    - où : `web/v2/shell/src/views/edit/nodes.ts` (`useNodeEdit`, `kindOf`),
      `views/edit/inline.ts` (`stateReason`, `whyWords`), `TopologyView.vue`,
      `record/ContentTab.vue` ; test voisin `shell/test/edit.test.ts`
    - dépend de : 12f1
    - fait le 2026-11-12 — `TopologyView.vue` : `readonlyReason` (`whyWords`) et
      `stateReason` passés au `TreeGrid`, pied de l'arbre qui montre aussi le motif d'une
      cellule en revue (`EditHint` `held`) ; `edit/nodes.ts` : `useNodeEdit` partagé par
      l'arbre et la grille du contenu du nœud choisi (`share` : la grille jointe donne ses
      lignes, son nom et sa relecture ; une cellule en brouillon, en attente, en revue ou
      refusée l'est dans les deux ; une seule ligne du motif, sous la grille où Ctrl+Entrée
      l'a demandée, `TREE_GRID`, `CONTENT_GRID` ; écrite à la version de la ligne de cette
      grille, `rowOf` ; un 412 relit les deux ; la grille partie, sa ligne se ferme, après
      la réponse d'une écriture en cours), `isKind` (garde de type : une ligne d'aucune sorte
      connue n'a pas d'éditeur) ; `edit/inline.ts` : `shownRow` (une ligne de n'importe
      quelle grille de ces objets), `heldOf`, `askOf`, `rowOf`, `QuickAsk.grid`,
      `rereadLists` ; `record/ContentTab.vue` : prend `nodeEdit` de son hôte (la fiche garde
      la sienne). 021 design (« Édition en place », Topographie). Tests :
      `shell/test/edit.test.ts` (Topographie montée : pourquoi d'une cellule sans éditeur en
      description, 202 : `stateReason` et « (en revue) » sur la cellule de l'arbre et dans le
      contenu, rien sur la cellule du nœud ; 422 : erreur sur la cellule des deux grilles,
      une seule ligne, sous l'arbre ; 403 : refus dans la ligne, saisie gardée, cellule
      intacte ; composable : version de la grille d'où vient la saisie, relecture des deux,
      ligne fermée avec sa grille, sorte inconnue sans éditeur), parcours `edit` (description
      de la cellule de l'arbre pour un lecteur).
    - choix : le statut de la vue ne dit plus le pourquoi à l'arrivée du focus (la
      description le dit, deux annonces sinon), seulement quand il change sous le focus (la
      date, les droits : une description changée en place est peu lue) ; la cellule du nœud
      n'a ni état ni raison (sa valeur ne s'édite pas en place, A20) : le reste de la 12f1
      (« retenue » décrite, non nommée) reste sans effet, le `TreeGrid` inchangé ; prop
      `nodeEdit` plutôt qu'un `provide` (un seul hôte, dit au gabarit) ; partagé, le contenu
      parle par le statut de la Topographie (sa région n'est plus rendue) ; un nœud dans les
      deux grilles vient d'ordinaire de la même page du cache (même clé
      `subPages("prefix", id, "children")`), mais la version écrite est celle de la grille de
      la saisie ; pas de capture nouvelle (description, pied, rien de dessiné en plus).
    - corrections des relectures : version d'`If-Match` tirée de la grille de la saisie et
      non de la première lue (`ui-reviewer`, `a11y-auditor`) ; ligne du motif d'un contenu
      démonté fermée (`a11y-auditor` : sinon ni montrée ni fermable, et toute autre
      Ctrl+Entrée refusée) ; noms de grilles et filtre de la ligne partagés (`askOf`),
      `heldOf` au lieu d'une colonne factice, région de statut du contenu partagé retirée,
      pourquoi changé sous le focus redit par le statut. Revue (une passe) : la durée de vie
      d'une cellule écrite (en revue, appliquée, refusée, en attente) se juge sur la ligne de
      la grille d'où elle a été écrite (`Shown.grid`, `rowIn`), plus sur la première lue de
      l'union (un 202 ou un 200 depuis le contenu, l'arbre tenant le nœud à une autre
      version, s'effaçait à la première relecture de l'arbre ; test `edit.test.ts` : 202 puis
      200 depuis le contenu, tenus jusqu'à la relecture du contenu) ; `shownRow` sans
      transtypage ; `shownById` de l'arbre sur ses seuls nœuds ; `useSay` du contenu créé
      seulement sans hôte ; commentaire de `focusWhy` corrigé. Vérifié : parcours `tree` et
      `edit` (34 réussis), `accessibility.spec.ts -g "every journey runs axe"` (2 réussis),
      `k-web2-check --base main` (1050 Vitest, ciblé : 0 capture sur 2734).
    - reste : branche « écriture en cours, puis grille partie » de `share` sans test (le
      faux `fetch` du test répond aussitôt), son `scope.run` sur une portée d'hôte déjà
      arrêtée et un contenu remonté avant la réponse non traités ; `said` du contenu
      partagé créé sans servir. Si un hôte édite un jour la valeur d'un nœud, le `TreeGrid`
      devra rendre l'état
      de la cellule du nœud (marque et mot caché) ; une Ctrl+Entrée d'une grille remplace la
      ligne ouverte non occupée de l'autre sans effacer l'erreur de sa cellule (comme dans
      une seule grille) ; focus après une écriture en cours dont la grille est partie : au
      corps ; restitution par un vrai lecteur d'écran non éprouvée (description des cellules
      de l'arbre, phrase redite par le statut).
- [x] 12g. Éditeur à liste dans une cellule de grille : l'emplacement `editor` du
  `DataGrid` (et du `TreeGrid`) accueille une liste fermée (`Select` ou listbox) dont la
  couche ne vaut pas sortie de l'éditeur (aujourd'hui le focus parti vers la couche
  valide aussitôt) ; nature et usage s'y choisissent au lieu du texte vérifié contre la
  liste — REQ-020-19, REQ-021-15 — verify : script clavier `data-grid.spec.ts` (Entrée
  ouvre, flèches, Entrée valide, Échap annule, focus rendu à la cellule) ; parcours
  `edit` (nature choisie dans la liste → brouillon à 1)
  - où : `web/v2/ui/src/components/internal/cellEditor.ts` (`onFocusout`),
    `DataGrid/DataGrid.vue`, `Select/` ; `shell/src/views/edit/inline.ts` (`words`),
    `ListView.vue`
  - dépend de : 12
  - fait le 2026-11-12 — `DataGrid` et `TreeGrid` prennent `options(row, column)`
    (facultative ; même contrat, `internal/cellEditor.ts` : `optionsOf`, lue à l'ouverture
    de l'éditeur, `choose`) : une cellule d'une liste fermée a pour éditeur
    `internal/CellSelect.vue` (Select-Only Combobox de Reka comme `Select`, bouton dans le
    cadre de l'éditeur, liste au calque contre ce cadre, aussi large que lui) : Entrée, F2
    ou un double clic ouvrent l'éditeur et sa liste sur la valeur tenue ; Entrée, Espace ou
    un clic choisissent et valident (Ctrl+Entrée : `now`) ; Tab et Maj+Tab choisissent,
    valident et passent le long de la ligne ; Échap ferme la liste (focus au bouton,
    éditeur ouvert), l'Échap suivant annule (focus à la cellule) ; dans un `SidePanel`,
    aucune ne ferme le panneau (la liste est hors du panneau, la seconde est arrêtée par
    l'éditeur), depuis la cellule Échap le ferme ; le focus parti dans la liste n'est plus
    une sortie (`inPopup` : la cible dans l'élément d'`aria-controls` d'un contrôle de
    l'éditeur) ; une valeur refusée garde l'éditeur ouvert, `aria-invalid` et l'alerte
    (l'effacement de l'erreur au changement de texte est devenu synchrone, sans quoi le
    choix refusé l'effaçait aussitôt). Shell : `useInlineEdit.choices` (nature, usage :
    clés tenues, mots montrés ; drapeau `true`/`false`), `propertyChoices` (`bool`,
    `enum` lu en entier ; « Aucune valeur (retirer) » pour une valeur propre), branchés
    dans `ListView` (et ses colonnes `prop.<clé>`, `propertyColumns.ts` ; `words` passé à
    `useInlineEdit`), la grille du contenu, l'arbre de la topographie et l'onglet
    Propriétés. Catalogue : `DataGrid` `choice`, `choice-open`, `TreeGrid` `choice`,
    `SidePanel` `cell-choice`. 020 design (contrat d'édition, inventaire 20 et 21), skill
    (lignes `DataGrid`, `TreeGrid`, piège du `Select` ouvert). Tests : scripts
    `data-grid.spec.ts` (Entrée ouvre, flèches, Entrée valide, Échap ferme puis annule,
    Tab, Ctrl+Entrée, refus, double clic et clic, « aucune valeur »), `tree-grid.spec.ts`,
    `side-panel.spec.ts` (deux Échap, le panneau reste ; la troisième le ferme) ; Vitest
    `components-data.test.ts` (liste lue à l'ouverture, choix validé, refus gardé),
    `edit.test.ts` (`choices` ; contenu et arbre choisis dans la liste),
    `properties-edit.test.ts` (`propertyChoices`) ; parcours `edit` (nature de l'arbre
    choisie dans la liste → brouillon à 1 ; Ctrl+Entrée de la liste ; usage par saisie dans
    la liste, deux onglets), `accessibility.spec.ts -g "every journey runs axe"`.
  - choix : composant interne (`CellSelect`) plutôt que `Select` dans l'emplacement
    `editor` (sans `FormField`, Tab et Échap au contrat de la grille) ; liste ouverte dès
    que l'éditeur prend le focus ; choix validé aussitôt (pas de second Entrée) ; options
    figées à l'ouverture ; « aucune valeur » sous une clé privée (U+E000), Reka refusant
    `""` ; drapeau et `bool` montrés `true`/`false` comme leur cellule ; la valeur tenue
    hors de la liste montrée telle quelle sur le bouton ; l'emplacement `editor` reste pour
    un éditeur de l'hôte (non utilisé) ; liste ouverte (`DataGrid` `choice-open`) montrée
    seule (`isolated`), passée à axe avec la seule exception `aria-hidden-focus` (020-11) :
    faux positif, Reka tient le focus dans la liste et bloque le pointeur dehors (skill,
    piège du `Select` ouvert).
  - passe de correction (revue, 2026-11-12) : `choice-open` isolé et vérifié par axe ;
    `draftNature` du parcours `draft` choisit « Conteneur » dans la liste (il tapait la
    clé, passée par la recherche à la saisie de Reka) ; un `enum` déclaré sans valeurs
    garde l'éditeur de texte (`propertyChoices`). Vérifié : `draft.spec.ts` et
    `edit.spec.ts` entiers sous les deux navigateurs (42 réussis),
    `accessibility.spec.ts -g "every journey runs axe"` (2 réussis), `k-web2-check --base
    main`.
  - reste : une liste d'`enum` lue au focus de la cellule : Entrée avant sa réponse ouvre
    l'éditeur de texte ; le parcours ne passe pas axe liste ouverte (`aria-hidden-focus`,
    vérifié au catalogue) ; `update:modelValue` de Reka réécrit `draft` après la fermeture
    de l'éditeur (sans effet : l'ouverture suivante le relit) ; après
    un refus, la liste reste fermée (Entrée la rouvre) ; Tab sur une option désactivée
    valide la valeur tenue ; restitution par un lecteur d'écran non éprouvée (nom du bouton
    par `aria-label`, valeur montrée ; alerte d'un refus pendant que la liste se démonte) ;
    anneau du cadre redéclaré dans `CellSelect.vue` au lieu de `shared.css` ; la logique
    Tab de la liste est en deux copies (`Select.vue`, `CellSelect.vue`).
- [x] 12h. Onglet Propriétés de la fiche édité en place : une valeur de l'objet modifiée
  dans la cellule (`prop.<clé>.edit`, présent), au brouillon (gardée par un `expect` de la
  version lue, hors compteur, Q-021-21) ou par Ctrl+Entrée (`properties` du patch de la
  sorte, sous `If-Match`), valeur vérifiée par le type de sa déclaration, jamais plus
  strictement que le serveur ; une valeur héritée s'édite et devient valeur propre qui la
  surcharge, dit avant l'envoi, vider une valeur propre rend l'héritage (Q-021-19) ; une
  valeur calculée ou masquée sans éditeur, avec pourquoi — REQ-021-15, 16, 37 ; 003
  (propriétés), 018 (permissions) — verify :
  Vitest (éditabilité d'une
  propriété, opération du brouillon, patch `properties`) ; parcours `edit` (une
  propriété modifiée au clavier → brouillon à 1)
  - où : `web/v2/shell/src/views/record/PropertiesTab.vue`, `views/edit/{fields.ts,inline.ts}`
    (`propertyRight`, colonne `prop.<clé>`) ; schémas `*Patch.properties` de
    `specs/006-api-rest/openapi-v1.json`
  - dépend de : 12 ; 12o (arbitrage de l'utilisateur)
  - fait le 2026-11-11 — la clause « héritée sans éditeur » lue selon Q-021-19 (spec 003
    concordante : REQ-003-12 remplacement, `null` du patch rend l'héritage) : une valeur
    héritée s'édite et devient valeur propre qui la surcharge ; le pied de la grille le dit
    avant l'envoi (« Une valeur saisie ici surcharge la valeur héritée de <source>. »,
    origine lue dans l'explication, source nommée), le statut poli à l'ouverture de
    l'éditeur, et la cellule de la focalisation dans son nom ; vider une valeur propre la
    retire (`unassign_property`, `properties: {<clé>: null}`), vider une héritée est refusé.
    `edit/properties.ts` (nouveau, `usePropertyEdit`) : brouillon `assign_property`,
    Ctrl+Entrée `write.quick` de `properties` sous `If-Match` (version de la fiche), 200,
    202 « en revue » et son motif (`stateReason`), 412 saisie gardée et valeur relue, 422,
    403 ; valeur vérifiée par le type (`text` longueur, motif, contrôle ; `int` bornes ;
    `bool` ; `date` ; `fqdn` ; `enum` contre la liste entière lue) ; sans éditeur, avec
    pourquoi : masquée, calculée (inférée), référence ou ensemble (`typed`), sorte dont le
    `PATCH` n'a pas `properties` (`PROPERTIES_PATCHED` vérifié contre les `*Patch` du
    contrat : VLAN, groupe), déclaration non lue, sans `prop.<clé>.edit`. `PropertiesTab.vue`
    (props `collection`, `version` depuis `RecordView.vue`), `EditHint` (`note`),
    `QuickApply` (`label`, `check`, `note` de la `QuickAsk`), `fields.ts` (`FixedReason`),
    catalogues fr/en, design 021 « Édition en place ». Tests : `shell/test/properties-edit.test.ts`
    (types, opérations, patch, brouillon, origine ; onglet monté : pourquoi, surcharge
    dite, brouillon, Ctrl+Entrée sous `If-Match` → en revue), `edit.test.ts`
    (`prop.<clé>.edit` seul), parcours `edit` (« a property inherited… » : surcharge dite,
    modifiée au clavier → brouillon à 1, axe), sous les deux navigateurs.
  - choix : une propriété demande `prop.<clé>.edit` seul, non plus aussi `X.edit` (design
    « X.edit du type ou prop.<clé>.edit », REQ-003-17 ; le serveur ne demande que celle de
    la propriété pour une affectation, `rights.rs` `written`) — vaut aussi pour les
    colonnes `prop.<clé>` de la 12i ; Ctrl+Entrée demande `prop.<clé>.approval.self` ;
    composable
    à part plutôt qu'un `useInlineEdit` généralisé (lignes = propriétés d'un seul objet) ;
    origine lue à la focalisation (explication, même cache que le popover), non pour
    toutes les lignes d'avance ; « calculée » = sans origine avec une provenance (ce que
    l'explication dit « inférée ») ; `bool` saisi `true`/`false` comme la cellule le montre.
  - reste : le pied et le nom de cellule ne disent la surcharge qu'à la focalisation (le
    `DataGrid` ne décrit une cellule éditable que par son état) ; la note de `QuickApply`
    n'est pas liée au champ du motif par `aria-describedby` ; une propriété déclarée sans
    valeur n'a pas de ligne (rien à poser depuis l'onglet) ; `confirm` double celui de
    `useInlineEdit` (~150 lignes), à factoriser si la 12i y touche ; `bool` et `enum` en
    texte, une liste viendra avec la 12g. Mineurs de la revue : le texte saisi est rogné
    de ses espaces sans le dire (le serveur ne rogne pas) ; l'origine inconnue si
    l'explication échoue (vider passe alors au serveur) ; « l'héritage reprend » dit même
    sans héritage déclaré ; aucune capture de l'onglet en édition, relectures UI et a11y
    sans rendu (12l).
  - corrections de la revue (2026-11-11) : garde `expect` au brouillon (Q-021-21 :
    `guarded` de `edit/properties.ts`, une par objet ; `operation_count` sans les gardes,
    `kartenn-store-pg` `drafts.rs` et contrat ; `draft/operations.ts` `listedOperations`,
    `withoutOperation`, `DraftOperations.vue` : gardes hors liste, leurs violations sur
    l'opération de leur objet, retirées avec la dernière) ; vérification locale jamais plus
    stricte que le serveur (`text` compté en NFC, motif laissé au serveur, `fqdn` plus lâche
    qu'UTS #46 avec ses points, années 0 à 99). Tests : `properties-edit.test.ts` (garde,
    liste, retrait ; `हिन्दी.example`, « café » décomposé sous `max_len: 4`,
    `0050-01-01`), `kartenn-rest` `drafts.rs` (garde gardée, non comptée).
- [x] 12i. Colonnes de propriété d'une liste (`prop.<clé>`), arbitrées par l'utilisateur le
  2026-11-11 (Q-021-18) : aucune par défaut ; le sélecteur « Colonnes » propose les
  propriétés déclarées pour le type, le choix gardé par vue et par utilisateur comme les
  autres réglages de vue — verify : Vitest du sélecteur (propriétés déclarées proposées,
  choix gardé) ; parcours d'une liste avec une colonne de propriété ajoutée puis éditée. La règle d'éditabilité est déjà là (`fields.ts`).
  - où : `web/v2/shell/src/views/ListView.vue`, `views/collections.ts`, `views/edit/fields.ts`
  - dépend de : 12h
  - fait le 2026-11-12 — `views/propertyColumns.ts` (nouveau, `usePropertyColumns`) :
    propriétés déclarées pour la sorte (`declaredFor`, porteuses vides ou la nommant), une
    colonne `prop.<clé>` chacune (libellé du filtre, clé sans libellé, « libellé (clé) »
    entre homonymes ; priorité 3), cachées tant que non choisies (`withPropertiesHidden`) ;
    valeurs lues par `/v1/<sorte>/{id}/properties` pour les lignes en vue et un écran autour
    (`rowsToRead`, `useQueries` nouveau de `data/query.ts`, `limited` de la colonne VLAN) ;
    grille fusionnée des champs (`useInlineEdit`) et des propriétés (`usePropertyEdit`).
    `views/viewSettings.ts` (nouveau) : colonnes gardées sous
    `kartenn.view.<identité>.<vue>.columns`. `edit/properties.ts` : `usePropertyEdit`
    généralisé à plusieurs objets (cellule = objet × clé, `recordGrid` pour l'onglet de la
    fiche), raisons `unread` et `unreadable` (`fields.ts`) ; `edit/inline.ts` : `quickWrite`,
    Ctrl+Entrée commun aux champs et aux propriétés (le `confirm` doublé de la 12h
    factorisé). `parts/PropertyCell.vue` (nouveau : lecture, illisible et son code,
    masquée, absente, valeur). `ui` `ColumnChooser` : le sélecteur reste dès qu'un choix est
    fait, une colonne montrée comprise (relevé de l'`a11y-auditor` : sinon, toutes les
    propriétés cochées, plus de retour au-delà de 960 px). Design 021 « Édition en place ».
    Tests : `shell/test/property-columns.test.ts` (propriétés proposées, décochées, aucune
    lue sans colonne ; choix gardé par utilisateur et par vue, effacé à la déconnexion ;
    valeur lue, éditeur avec `prop.<clé>.edit` seul), `ui/test/layout.test.ts` (sélecteur
    gardé sur un choix montré), parcours `edit` (« a column of a property added by
    « Colonnes »… » : ajoutée, gardée au rechargement, surcharge dite, modifiée au clavier →
    brouillon à 1, axe), sous les deux navigateurs.
  - choix : le choix gardé est celui de toute la grille de la vue (colonnes du cœur
    comprises, la vue = la collection), effacé à la déconnexion comme les objets récents
    (`clearSensitive`, aucune clé ajoutée à `KEPT_KEYS`) ; colonnes de propriétés après
    celles des plugins ; une lecture par ligne (pas de lecture groupée au contrat), la même
    clé de cache que l'onglet ; sans éditeur tant que la valeur n'est pas lue ; une seule
    ligne de motif à la fois, celle d'un champ avant celle d'une propriété.
  - reste : la surcharge n'est dite qu'à la focalisation (même reste que la 12h) ; un
    choix de colonnes perdu à la déconnexion (le garder demanderait un arbitrage) ; deux
    lignes de motif ouvertes à la fois (champ puis propriété) : la seconde attend la
    première ; la cellule illisible n'a pas de `Request-Id` sans code (comme la colonne
    VLAN) ; `reread` après un 412 relit toute la liste ; aucune capture (12l). Revue
    (mineurs) : `versions` part d'une `Map` vide, si bien que le premier rechargement de la
    liste efface les origines connues (une cellule calculée redevient éditable jusqu'à la
    relecture, l'écriture étant refusée quand même) ; le repli `getQueryData` hors fenêtre
    n'est pas réactif ; les deux lignes de motif (champ, propriété) ne se remplacent pas,
    à l'inverse de la 12h ; `shownRows` recalculé à chaque lecture (~60 fois au chargement
    d'une colonne) ; à vérifier : `aria-colindex`/`aria-colcount` comptent-ils les colonnes
    de propriétés cachées ?
- [x] 12j. Référence au-delà de la première page dans le panneau d'édition : domaine et
  parent choisis par un `Combobox` qui interroge `q` de leur collection (`/v1/domains`,
  `/v1/prefixes`), non plus dans la première page lue — REQ-021-36 — verify : Vitest
  (suggestions tirées de `q`, valeur prise au-delà de la première page) ; parcours `edit`
  (un domaine trouvé par sa saisie)
  - où : `web/v2/shell/src/views/edit/EditPanel.vue` (`optionsOf`, `suggestTargets` de
    « Changer de parent… »), `panel.ts`
  - dépend de : 12
  - fait le 2026-11-12 — `EditPanel.vue` seul (`panel.ts` inchangé) : toute référence
    (domaine, parent d'une entité ou d'un site, groupe d'un VLAN, ancre d'un groupe) est un
    `Combobox` (`suggestionsOf`) dont les suggestions sont demandées à sa collection par `q`
    à la saisie (`cacheSuggest` de `views/filters.ts` : la première page de la réponse ;
    l'ancre interroge entités, sites et domaines à la fois), Bas ou Alt+Bas sur un champ
    vide la première page ; à côté, « Aucun » d'une référence facultative et les objets
    créés plus tôt dans le brouillon (`draftedCreations`, références locales `local:<n>`
    de la 14l gardées, nommées « opération n du brouillon »). « Changer de parent… » prend
    le même chemin (cibles valides seules, `canMove` ; les ancêtres lus sur dix pages au
    plus, pour le cycle). Une valeur donnée d'avance (domaine de l'arbre, groupe d'un
    « Créer un VLAN ») ou lue sans ses mots (ancre, groupe) est nommée par la lecture de
    son objet (`readLabel`), le champ vide jusque-là (son identifiant seulement si
    l'objet ne se lit pas) ; un texte saisi sans suggestion prise est refusé sur le champ
    (`edit.check.pick`, celui d'un déplacement `edit.move.choose`), rien n'est deviné ;
    l'aide du champ le dit d'avance (`edit.referenceHint`). Listes fermées (nature, usage)
    toujours en `Select`. 021 design (« Panneau d'édition par sorte »). Tests : Vitest
    `shell/test/edit-reference.test.ts` (domaine trouvé par `q` au-delà de la première
    page, pris et écrit ; valeur donnée d'avance nommée par son objet, aucune liste lue ;
    texte sans suggestion refusé, rien d'envoyé ; « Aucun » et parents par `q`),
    `draft-replace.test.ts` (domaine local nommé dans le champ) ; parcours `edit` (domaine
    trouvé par sa saisie au clavier, `q` envoyé), `tree`, `vlan` (ancre et groupe saisis).
  - choix : toutes les références plutôt que domaine et parent seuls (un seul chemin, la
    même limite levée pour le groupe et l'ancre) ; la tâche nomme `/v1/prefixes`, mais
    aucun champ du panneau ne désigne un préfixe : les collections interrogées sont
    celles des références (`/v1/domains`, `/v1/entities`, `/v1/sites`,
    `/v1/vlan-groups`) ; un champ vidé vaut « Aucun » pour une référence facultative (à
    la création comme en modification, s'il en vient une), un déplacement exige une cible
    ou « Aucun » prise ; les mots d'une valeur
    connue gardés par valeur (`labels`) pour le motif d'un 412, l'aperçu et l'indice du
    VID.
  - passe de correction (revue) : les suggestions ont leur propre clé de cache
    (`cacheSuggest` de `views/filters.ts`, `page_size` écrit : la page par défaut du
    contrat), distincte de celle de la liste qu'une vue ou un panneau lit de la même collection (les
    ancêtres d'un déplacement, `nodesRead`) : une page seule, jamais les dix d'une liste
    périmée, et l'annulation par un texte plus récent ne coupe que leur lecture ; la barre
    de filtres en profite. Vitest `edit-reference.test.ts` (champ vidé pendant la lecture
    des ancêtres : leurs trois pages lues sans annulation, les suggestions d'une page sans
    curseur).
  - reste (relectures et revue) : Échap d'un combobox fermé dans un panneau → 12p ;
    l'erreur d'un champ reste jusqu'à l'envoi suivant, comme pour les autres champs ; une
    page de cibles toutes invalides d'un déplacement, champ vide, n'offre rien tant que
    rien n'est saisi ; un objet illisible dont l'identifiant remplit le champ, non testé ;
    « Aucun » filtré par le texte comme une suggestion (`filterOptions`) ; aucun Vitest de
    « Changer de parent… » par `q` (le parcours `moves` le tient) ; `labels` grossit sans
    purge le temps du panneau ; une cible au-delà des dix pages d'ancêtres lues, refusée
    par le serveur pour un cycle, non testée ; aucune capture (12l).
- [x] 12k. `c` et `e` refusés dits au clavier (relevés des 12c et 12d, `a11y-auditor`) :
  sous `as_of`, sans le droit ou sans domaine, la touche dit sa raison par le statut poli
  de la vue au lieu de rien ; dans l'onglet Contenu et le contenu d'un nœud, la phrase
  d'une cellule nomme ce que « Modifier » modifie — REQ-021-37 — verify : Vitest des
  raccourcis refusés ; parcours `time` (`e` sous `as_of` dit pourquoi)
  - où : `web/v2/shell/src/shortcuts/view.ts` (`useViewAction`), `views/edit/EditHint.vue`
  - dépend de : 12d
  - fait le 2026-11-12 — `shortcuts/view.ts` : `ViewAction.why` (la raison de la vue,
    `undefined` : rien à modifier) et `ViewAction.say` (son statut poli) ; `play(id, time,
    t)` joue l'action ou, refusée, fait dire au statut hors du présent d'abord
    (`edit.why.past`), puis le `why` de la vue, et prend la touche ; sans objet (aucune
    ligne, aucun nœud choisi), sans statut, elle se tait et passe ; `ShellFrame.vue`
    (`playView`) passe par `play`, la palette garde `runnable`. `ListView.vue` : `c` et `e`
    disent la permission (`whyWords`), « restarted » cède la région à la phrase ;
    `TopologyView.vue` : `c` dit « Ajout impossible : aucun domaine que vous puissiez lire »
    (domaines lus, aucun ; région de statut sœur de l'`EmptyState`), sinon ce que dit son
    « + » (`addRefusal`), `e` la permission du nœud choisi ; `RecordView.vue` : région de
    statut propre, `e` dit la permission, `blockedBy` met hors du présent avant le droit
    (comme les cellules et les touches). Onglet Contenu et contenu d'un nœud
    (`record/ContentTab.vue`) : `whyWords(why, t, of)` et `EditHint` `of` disent
    `edit.why.inPlaceDatedOf` (« « Modifier » mène au brouillon daté, pour {of} et non pour
    cette ligne »), `of` vide → la phrase sans nom. 021 design (« Raccourcis »), skill
    (§ 5). Tests : `shell/test/refused-keys.test.ts` (liste sous `as_of` et pour un
    lecteur, `e` sans ligne qui passe ; Topographie sans domaine, d'un lecteur, sous
    `as_of` ; fiche sous `as_of` ; cellule du contenu sous `valid_at`, description et
    pied), `shortcuts.test.ts` (`play` : ordre période puis droit, sans objet, sans statut,
    `dated`) ; parcours `time` (`e` sous `as_of` : la région de statut de la fiche dit
    « revenez au présent pour modifier »).
  - choix : hors du présent l'emporte sur un droit manquant, pour la touche comme pour les
    cellules (12d) et désormais le bouton « Modifier » de la fiche ; un refus dit prend la
    touche, un refus muet la laisse aller ; `say` donné par action (explicite) plutôt
    qu'une fois par vue ; les vues d'administration (sans région de statut, `allowed` sans
    raison) restent muettes ; `e` d'une liste sans ligne au focus ne dit rien (rien à
    modifier) ; le nom de la phrase datée est `name` de l'onglet (la fiche ou le nœud),
    sans sa sorte.
  - corrections des relectures : `c` de la Topographie sous `as_of`, domaine lu, restait
    muet (`why` ne rendait que le refus de droit ; `ui-reviewer`) ; ordre bouton/touche de
    la fiche aligné ; région « aucun domaine » hors du paragraphe de l'`EmptyState` et
    `restarted` d'une liste qui masquait la phrase (`a11y-auditor`) ; `of` vide ;
    formulations (`inPlaceDatedOf`, `addNoDomain`).
  - passe de corrections (revue) : `useStatus` (`edit/say.ts`) porte la région de statut
    des vues (liste, Topographie, fiche, onglets Contenu et Propriétés) : vidée puis dite,
    vidée à un autre période ou d'autres droits (une phrase encore à venir abandonnée),
    `clear` pour un autre objet (nœud choisi, fiche) ; la phrase datée du contenu suit
    ce qui y mène : « « Modifier » ouvre le brouillon daté de {of}, non de cette ligne »
    dans la fiche, la touche `e` ou « Ouvrir la fiche » puis « Modifier » à côté de l'arbre
    (`edit.why.inPlaceDatedNode`, le panneau du nœud n'a pas de « Modifier ») ;
    `whyWords(why, t, dated)` et `EditHint` `dated` reçoivent la phrase de l'hôte ; un nom
    pas encore lu (vide ou l'identifiant) la laisse sans nom. Tests :
    `refused-keys.test.ts` (refus sous `as_of` effacé au retour au présent, liste et
    fiche ; phrase du contenu à côté de l'arbre).
  - reste : sous `as_of`, « revenez au présent pour modifier » dit aussi le refus de `c`
    (créer) ; les vues d'administration sous `as_of` ou sans droit restent muettes ; dans
    l'arbre de la Topographie, la phrase datée d'une cellule renvoie à « Modifier » qui
    modifie le nœud choisi, non forcément la ligne ; la touche `e` nommée par la phrase du
    contenu à côté de l'arbre l'est aussi quand les raccourcis à une touche sont coupés ;
    deux régions de statut dans la fiche (la sienne et celle de `ContentTab`) ; le test de
    `e` sous `as_of` dans la Topographie ne prouve pas la phrase redite (une même phrase
    deux fois) ; restitution par un vrai lecteur d'écran non éprouvée.
- [ ] 12l. Captures des vrais écrans d'édition, à arbitrer : le projet `visual` n'a pas
  d'API ; il faudrait un banc de captures à API simulée, transverse (les 11, 14 et 17
  l'attendent aussi), plutôt qu'une tâche de la 12 — à la session principale ou à
  l'utilisateur de le placer.
- [x] 12m. Infobulle de raison d'une cellule accessible (relevés de l'`a11y-auditor` sur
  la 12e, communs à `readonlyReason` et `stateReason`) : ouverte aussi au focus de la
  cellule (WCAG 1.4.13 : au survol et au focus, REQ-021-37) et au toucher (un appui, ou
  une autre voie visible), survolable (le pointeur passé de la cellule à l'infobulle la
  garde ouverte), Échap qui la ferme arrête la touche (`closeTip` la laisse aller à la
  grille) ; ligne `Tooltip` du skill `kartenn-ui` — verify : script clavier
  `e2e/components/data-grid.spec.ts` (focus → infobulle, Échap la ferme sans autre effet,
  pointeur sur l'infobulle → gardée) ; script tactile (`e2e/visual/touch.spec.ts`)
  - où : `web/v2/ui/src/components/DataGrid/DataGrid.vue` (`onCellEnter`, `onCellLeave`,
    `closeTip`, `reasonTip`), `Tooltip/` (`useTooltip`), `DataGrid.catalogue.ts`
  - dépend de : 12e
  - fait le 2026-11-12 — `DataGrid` seul, sans changement d'API : l'infobulle de raison
    (`tipOf` : les phrases de `notesOf`, rien pendant l'édition de la cellule ni sur une
    erreur, qui a sa bulle) s'ouvre aussitôt au focus clavier de la cellule
    (`:focus-visible`, `focusTip` : aussi quand la raison vient sous le focus, une
    écriture retenue après son commit) et se ferme quand le focus part, sauf si le
    pointeur la garde ; un appui (ou un clic) l'épingle (`pinTip`, `useTooltip.pin`)
    jusqu'à un appui ailleurs ; survolable : ouverte sur une cellule, elle y reste
    (`hold`) et ne passe à la cellule entrée qu'après le délai, le pointeur traversant la
    ligne d'en dessous pour l'atteindre (`switching`) ; son texte suit la raison de sa
    cellule, et se ferme si elle part (`anchorTip`) ; Échap la ferme et s'arrête
    (`closeTip` : ni la grille ni une couche autour ne l'entendent, même sous le focus de
    la cellule) ; l'éditeur qui s'ouvre (Entrée, F2, double clic) la ferme, même sous le
    pointeur (point du reste de la 12e, vérifié : elle restait), et le pointeur revenu sur
    une cellule en édition ne la rouvre pas. 020 design (inventaire, contrat d'édition),
    skill (ligne `DataGrid`). Tests : `data-grid.spec.ts` (focus → infobulle, Échap la
    ferme sans autre effet puis va son chemin, autre cellule, sortie de la grille ;
    pointeur de la cellule à l'infobulle par la cellule d'en dessous → gardée, à la même
    place ; Entrée sous le pointeur → fermée, Échap atteint l'éditeur), `touch.spec.ts`
    (un appui sur la cellule en revue dit le motif, un appui sur la bulle la garde, un
    autre ailleurs la ferme).
  - choix : comportement local à la grille plutôt que `reasonTip.listeners()` (une
    infobulle pour toutes les cellules, dont les écouteurs de survol existaient déjà) ;
    Échap arrêté même sous le focus (la tâche le demande ; `useTooltip` le laisse aller à
    la couche, défaut empêché) : la grille n'a pas de `passEscape` ; un clic épingle aussi
    (comme une action refusée), le pointeur qui part la ferme ; une cellule en erreur ne
    montre plus l'infobulle de sa raison (sa seule bulle, comme dit la 12e) ; pas de
    capture nouvelle (rendu de la grille inchangé, infobulle à délai).
  - reste : placement sous la cellule, au-dessus de la ligne suivante (où va la flèche
    Bas, et que le pointeur doit traverser) : au-dessus pour les cellules du focus, à
    arbitrer avec une capture de référence (relevé `ui-reviewer`) ; l'infobulle ouverte
    par le focus sur la cellule A cède à celle du survol d'une cellule B, sans revenir à
    A quand le pointeur part ; ancre détachée (ligne virtualisée sortie) non surveillée,
    hormis le défilement qui la ferme (`Tooltip.vue`) ; restitution par un vrai lecteur
    d'écran non éprouvée. Revue (mineurs) : `closeTip` n'annule pas `switching` (Échap
    pendant la bascule vers B : l'infobulle se rouvre sur B 300 ms plus tard) ; la ligne
    `Tooltip` du skill ne dit pas l'exception de la grille, qui arrête Échap ; trois
    `waitForTimeout(500)` dans `data-grid.spec.ts` contre un délai de 300 ms, marge mince
    sous charge ; l'épinglage au clic n'est pas testé, et un clic de sélection
    (`selectable="single"`) ouvre l'infobulle à chaque fois.
- [x] 12n. Le pourquoi d'un état partout où une grille le porte (relevés de la revue de
  la 12e) : `ExpandableGrid` transmet `stateReason` et `readonlyReason` (avec `editable`)
  à ses deux `DataGrid` ; une ligne en revue (`rowState`) dit son motif (description de
  sa première cellule, comme son mot caché) — REQ-021-15 — verify : Vitest monté
  (`ExpandableGrid` : raisons dans la grille compacte et la grande ; ligne en revue
  décrite par son motif) ; `shell/test/edit.test.ts` (motif de la ligne)
  - où : `web/v2/ui/src/components/ExpandableGrid/ExpandableGrid.vue`,
    `DataGrid/DataGrid.vue` (`markOf`, `rowState`), `shell/src/views/edit/inline.ts`
    (`rowState`) ; test voisin `ui/test/no-value-mounted.test.ts`
  - dépend de : 12e
  - fait le 2026-11-12 — `DataGrid` prend `rowReason(row)` (facultative) : pourquoi une
    ligne est dans son état, une phrase ou une par écriture retenue, demandée pour une
    ligne à état seulement ; ses phrases rejoignent celles de sa première cellule rendue
    (celle de la marque et du mot caché : description, infobulle), avant celles de la
    cellule, chacune une fois (`notesOf`) ; `hasReasons` garde la phrase cachée et
    l'infobulle. `ExpandableGrid` passe à ses deux grilles, d'un seul objet (`shared`),
    `editable`, `validate`, `readonlyReason`, `stateReason`, `rowReason`, et réémet
    `commit` : la grille compacte qui omet la colonne retenue dit le motif par la première
    cellule de la ligne. Shell : `rowReason` dans `edit/inline.ts` (les motifs distincts des
    écritures retenues de la ligne, quelle que soit la colonne) et `edit/properties.ts`,
    branché dans `ListView`, `record/ContentTab.vue`, `record/PropertiesTab.vue`.
    Catalogue : `row-states` de `DataGrid` décrit sa ligne en revue. 020 design (contrat
    d'édition, inventaire 20 et 37), skill (lignes `DataGrid`, `ExpandableGrid`). Tests :
    `ui/test/expandable-reasons-mounted.test.ts` (raisons dans les deux grilles, ligne en
    revue décrite par son motif dans la compacte sans sa colonne retenue, ligne en
    brouillon, phrases de la ligne et de sa première cellule une fois chacune, édition :
    `commit` réémis, Échap dans l'éditeur de la grande grille l'annule sans fermer le
    dialogue), `shell/test/edit.test.ts` (202 : motif de la ligne ; brouillon : aucun),
    `properties-edit.test.ts` (première cellule de la ligne en revue décrite par le motif).
  - choix : propriété à part `rowReason` (avec `rowState`, comme `stateReason` avec
    `cellState`) plutôt que de déduire le motif des cellules (la grille compacte, ou une
    colonne masquée par l'utilisateur, omettent la cellule retenue) ; une liste de phrases
    admise, pour qu'une phrase que la cellule dit déjà ne se répète pas (relevé de la
    relecture : une jointure `"X Y"` doublait `X`) ; `editable` transmis avec `validate`
    et l'événement `commit` (ajout d'API de l'`ExpandableGrid`, rétrocompatible : sans
    lui, Entrée ouvrait un éditeur dont la saisie se perdait) ; chaque grille garde son
    focus et son éditeur ; la première cellule en erreur ne dit que son erreur (comme
    toute cellule en erreur, 12e) ; pas de capture nouvelle (description et infobulle
    seulement), pas d'état de catalogue d'`ExpandableGrid` éditable.
  - reste : le motif dit par la ligne ne nomme pas le champ retenu (`heldWords` : le motif
    de la politique), d'où, dans la grille compacte sans la colonne, le pourquoi sans le
    où ; le pied d'`EditHint` (`activeHeld` de `ListView`, `ContentTab`) ne dit que le
    motif de la cellule de la focalisation, pas celui de la ligne sur sa première
    cellule ; Échap dans l'éditeur d'une grille dans un `Dialog` éprouvé sous happy-dom
    seulement (pas de script clavier sous navigateur) ; restitution par un lecteur
    d'écran (mot caché puis longue description) non éprouvée. Revue (mineurs) : le test
    d'Échap d'`expandable-reasons-mounted.test.ts` n'a pas de témoin (un Échap hors
    éditeur qui ferme le dialogue sous happy-dom) ; `stateReason` et `readonlyReason` ne
    sont pas prouvés dans la grille compacte (seule la grande l'est) ; le branchement de
    `rowReason` dans `ListView` et `ContentTab` n'a pas de test monté ; une première
    colonne à `priority` masquée par le `@container` emporte le motif avec le mot caché ;
    aucun état éditable d'`ExpandableGrid` au catalogue.
- [x] 12o. Valeur héritée d'une propriété modifiée en place, à arbitrer par l'utilisateur
  avant la 12h — arbitrée le 2026-11-11 (Q-021-19) : une valeur propre de l'objet, qui
  surcharge l'héritée, la cellule le disant avant l'envoi ; le retrait rend l'héritage. Question d'origine : modifier dans l'onglet Propriétés une valeur héritée (résolue d'un
  parent ou d'une déclaration) pose-t-il une valeur propre de l'objet, ou la cellule
  reste-t-elle sans éditeur avec un renvoi vers le porteur ? La spec 003 (résolution des
  propriétés) est à relire d'abord ; si elle ne le dit pas, à l'utilisateur.
  - où : `specs/003-proprietes/`, `web/v2/shell/src/views/record/PropertiesTab.vue`
- [x] 12p. Échap d'un combobox fermé dans un panneau (relevé de l'`a11y-auditor` sur la
  12j) : `ownsEscape` donne Échap à tout `input` texte, si bien que dans `SidePanel` et
  `BottomPanel` la seconde Échap d'un champ de référence (`Combobox`, liste fermée) ne
  ferme pas le panneau, alors que le `Select` d'avant la laissait passer et que l'en-tête
  du `Combobox` promet « la suivante est à la couche autour » ; un combobox fermé
  (`role="combobox"`, `aria-expanded="false"`) ne la garde plus ; le sort d'un champ texte
  simple (`TextField`) à dire dans le même passage — REQ-021-36 — verify : script clavier
  `e2e/components/side-panel.spec.ts` (Combobox dans le panneau : Échap ferme la liste,
  la seconde ferme le panneau, focus rendu à l'ouvreur) ; Vitest de `ownsEscape`
  - où : `web/v2/ui/src/components/internal/panel.ts` (`ownsEscape`),
    `SidePanel/SidePanel.vue`, `BottomPanel/BottomPanel.vue`, `Combobox/Combobox.vue`
    (en-tête), `SidePanel.catalogue.ts` ; ligne `SidePanel` du skill `kartenn-ui`
  - dépend de : 12j
  - fait le 2026-11-12 — sans changement d'API publique (`ownsEscape` est interne ; il
    prend l'événement au lieu de sa cible) : un seul contrat d'Échap dans la bibliothèque.
    Un contrôle qui prend Échap pour lui l'arrête (`preventDefault` : éditeur de cellule,
    erreur, infobulle de la 12m) et les panneaux l'ignorent ; `ownsEscape` ne répond plus
    que pour ce qui ne l'arrête pas : une popup ouverte (`role="combobox"` ou
    `aria-haspopup` autre que `false`, avec `aria-expanded="true"` : Reka la ferme depuis
    le document, après le panneau), une composition en cours (`isComposing`, et
    `keyCode` 229 : sous WebKit la composition finit avant la touche) et un champ de
    saisie (`TextField`, `textarea`, segment de `DateTimeField`, `select` natif), dont la
    saisie partirait avec le panneau (Q-021-22, en attendant la 12q). Le reste laisse Échap
    au panneau, comme un dialogue : combobox fermé (sa seconde Échap, que promet son
    en-tête ; `Select` fermé compris), bouton, case ; le focus rendu à l'ouvreur, sinon à
    la languette. 020 design (« Adaptation à la largeur », inventaire 30 et 38), skill
    (ligne `SidePanel`), en-têtes de `SidePanel`, `BottomPanel`, `Combobox`. Catalogue :
    état `fields` du `SidePanel` (référence « Domaine » et texte « Libellé » dans l'onglet
    Détail). Tests : Vitest `ui/test/panel-escape.test.ts` (popup ouverte gardée, combobox
    fermé, menu fermé, divulgation, champs gardés, boutons, composition et WebKit) ;
    `e2e/components/side-panel.spec.ts` (Échap ferme la liste, la seconde le panneau,
    focus à la languette ; rouvert par la languette : le champ texte saisi garde Échap, le
    combobox fermé et saisi le laisse, focus rendu à l'ouvreur) ;
    `e2e/visual/clavier.spec.ts` (Échap dans « Résumé » : le panneau reste).
  - choix : un champ de recherche garde Échap comme les autres champs ; la composition
    IME gardée ; état de catalogue nouveau plutôt qu'un champ ajouté à `detail` (ses
    captures inchangées), champs espacés comme ceux du panneau d'édition
    (`Stack gap="3"`) ; tabulation du script clavier comptée (onglet, son panneau, champ).
  - passe de correction (revue ; arbitrage Q-021-22 de l'utilisateur, 2026-11-12) : la
    première version rendait Échap au panneau depuis un champ texte, ce qui fermait le
    panneau d'édition ou du brouillon et perdait la saisie sans rien demander ; la
    confirmation « Abandonner les modifications ? » touche les deux panneaux et leurs
    hôtes du shell, trop large pour cette passe : la règle « Échap appartient à un champ
    de saisie » est rétablie pour les champs texte (le combobox fermé la laisse toujours)
    et la confirmation part en 12q.
  - reste : le combobox fermé et saisi du panneau d'édition le ferme encore sans
    demander (12q) ; contrat d'un éditeur de cellule piloté par l'hôte (`driven()` faux :
    il n'arrête pas Échap, qui va alors au panneau) à dire ; aucun script clavier du
    `BottomPanel` ni d'un éditeur de cellule dans un panneau : le prédicat seul est testé ;
    la fermeture dite par un lecteur d'écran (NVDA, VoiceOver) non vérifiée ; IME
    simulée seulement (Vitest).
- [x] 12q. Panneau modifié : demander avant de fermer (Q-021-22) — Échap ou la croix
  d'un `SidePanel` ou d'un `BottomPanel` dont l'hôte dit un champ modifié demandent
  d'abord « Abandonner les modifications ? » (`AlertDialog` : Abandonner, Continuer,
  focus sur Continuer, rendu au champ ou à la croix) ; intact, le panneau se ferme tout
  de suite ; une fois la confirmation en place, Échap revient au panneau depuis un champ
  texte (`ownsEscape` ne garde plus que popup ouverte et composition) ; branché dans le
  panneau d'édition (`EditPanel`, `ClosePanel` : un champ changé depuis l'ouverture) et
  dans le panneau du brouillon (`DraftSubmit`, `CommitForm` : motif saisi) ; à dire
  aussi pour `]`, le compteur du brouillon et un changement de route qui ferment le
  panneau — REQ-021-36 — verify : script clavier `e2e/components/side-panel.spec.ts`
  (intact : Échap ferme ; modifié : Échap demande, Continuer garde la saisie et le focus,
  Abandonner ferme et rend le focus à l'ouvreur) ; Vitest d'`ownsEscape` ; parcours
  `edit` (un champ modifié, Échap demande ; abandonné, rien d'écrit)
  - où : `web/v2/ui/src/components/internal/panel.ts` (`ownsEscape`),
    `SidePanel/SidePanel.vue` et `BottomPanel/BottomPanel.vue` (`close`, `onKeydown`,
    nouvelle propriété « modifié » de l'hôte : API publique, à arbitrer dans la tâche),
    `AlertDialog/` (`cancel`, `confirm`), `SidePanel.catalogue.ts` (état `fields`) ;
    shell : `frame/ShellFrame.vue` (`SidePanel`), `views/edit/EditPanel.vue`,
    `views/edit/ClosePanel.vue`, `draft/DraftSubmit.vue` ; dialogue à imiter :
    `admin/SecretDialog.vue` et `e2e/journeys/admin.spec.ts` (« Escape asks first ») ;
    020 design (« Adaptation à la largeur », inventaire 30 et 38), ligne `SidePanel` du
    skill. Découpée en 12qa (bibliothèque : panneaux, catalogue, scripts clavier) et 12qb
    (shell : `EditPanel`, `ClosePanel`, `DraftSubmit`, parcours `edit`).
  - dépend de : 12p
- [x] 12qa. Panneau modifié, bibliothèque (Q-021-22 et son complément) : propriété
  `modified` (booléen, faux par défaut) de `SidePanel` et `BottomPanel` ; modifié, Échap,
  la croix, « Retour » et `close()` exposé demandent « Abandonner les modifications ? »
  (`PanelLeave` : Abandonner en danger, Continuer la saisie ; focus sur Continuer la
  saisie, rendu au champ ou au bouton, Abandonner ferme et rend le focus à l'ouvreur) ;
  intact, fermé tout de suite ; Échap revient au panneau depuis un champ (`ownsEscape` :
  popup ouverte et composition seules) ; `ask()` exposé pour une garde — verify : scripts clavier
  `side-panel.spec.ts` et `bottom-panel.spec.ts` ; Vitest d'`ownsEscape` et
  d'`usePanelClose`
  - où : `web/v2/ui/src/components/internal/panel.ts` (`ownsEscape`, `usePanelClose`),
    `SidePanel/SidePanel.vue`, `BottomPanel/BottomPanel.vue`, `AlertDialog/AlertDialog.vue`,
    `i18n.ts`, catalogues (`fields`) ; 020 design (« Adaptation à la largeur »,
    inventaire 30 et 38), ligne `SidePanel` du skill
  - dépend de : 12p
  - fait le 2026-11-12 — `usePanelClose` (`internal/panel.ts`) porte la fermeture des
    deux panneaux : ouvreur noté à l'ouverture, `request` (Échap, croix, « Retour »)
    qui demande si `modified` est vrai, `discard` (« Abandonner ») qui rend le panneau
    partant `inert` (le retour du focus de la question ne peut s'y poser) puis ferme et
    rend le focus à l'ouvreur, sinon à la languette ; une fermeture par le modèle de
    l'hôte pendant la question la retire et rend le focus de même ; un clic qui n'a pas
    pris le focus (Safari, Firefox sous macOS) le pose sur le bouton ; les répétitions
    d'un Échap tenu ne ferment ni ne répondent (`AlertDialog` les ignore aussi).
    `ownsEscape(event)` : popup ouverte et composition seules, un champ laisse Échap.
    `ask()` pose la question sans fermer et dit s'il faut abandonner (une promesse par
    question ; `modified` repassé à faux la retire) ; `close()` et `ask()` exposés par les
    deux panneaux. Question commune `internal/PanelLeave.vue` (au lieu du bloc
    `AlertDialog` copié). Chaînes `panelLeave*` (fr, en). Catalogue : `SidePanel` suivi
    par l'hôte (champs de `fields` et résumé du brouillon, remis à zéro sur `close`),
    état `fields` nouveau du `BottomPanel` (un `TextField` dans une colonne), son état
    `sheet` avec ce champ. Tests : Vitest `ui/test/panel-escape.test.ts` (`ownsEscape` ;
    `usePanelClose` : intact, modifié, Abandonner, Continuer la saisie, répétition sur un
    panneau modifié, fermeture par l'hôte pendant la question et focus à l'ouvreur,
    `modified` repassé à faux, `ask()`), `components-024.test.ts` (fragment du
    `BottomPanel`) ; `e2e/components/side-panel.spec.ts` (référence : la liste puis le
    panneau ; intact : Échap depuis le texte ferme ; modifié : question, Continuer la
    saisie et Échap gardent saisie et focus, Abandonner ferme, focus à la languette,
    saisie jetée ; Échap tenu ; référence saisie ; la croix ; « Retour » d'une feuille),
    `e2e/components/bottom-panel.spec.ts` (nouveau : intact et modifié, focus rendu au
    bouton qui l'a ouvert ; la croix ; « Retour » d'une feuille),
    `e2e/visual/clavier.spec.ts` (Échap dans « Résumé » saisi : la question).
  - choix : texte « Les modifications de ce panneau seront perdues. » (couvre une
    référence choisie) ; logique partagée dans `internal/panel.ts` plutôt que copiée ;
    champ de l'état `fields` du `BottomPanel` dans une `Grid` de trois colonnes ; le
    modèle de l'hôte mis à faux ferme sans demander (sa porte de sortie : déconnexion),
    les fermetures de l'hôte qui doivent demander passent par `close()` ; `shiftTabTo`
    dans `e2e/fixtures/index.ts`.
  - reste : ni capture de la question ouverte (couverte par les scripts clavier et les
    captures d'`AlertDialog`) ; lecteurs d'écran (NVDA, VoiceOver) non vérifiés ; la
    sortie animée (réduite dans les tests) non jouée.
- [x] 12qb. Panneau modifié, shell (Q-021-22 et son complément) : `modified` branché
  dans le panneau d'édition (`EditPanel`, `ClosePanel` : un champ changé depuis
  l'ouverture) et dans le panneau du brouillon (`DraftSubmit`, `CommitForm` : motif
  saisi), la saisie jetée sur `close` ; `]`, le compteur du brouillon et un changement de
  route qui perdrait l'édition passent par la même question — REQ-021-36 — verify :
  parcours `edit` (un champ modifié, Échap, le compteur et une navigation demandent ;
  abandonné, rien d'écrit) et `draft` (Échap dans « Résumé » saisi demande) ;
  `accessibility.spec.ts -g "every journey runs axe"`
  - où : `web/v2/shell/src/frame/ShellFrame.vue` (`SidePanel`),
    `views/edit/EditPanel.vue`, `views/edit/ClosePanel.vue`, `draft/DraftSubmit.vue` ;
    dialogue à imiter : `admin/SecretDialog.vue`, `e2e/journeys/admin.spec.ts` (« Escape
    asks first ») ; prop `modified` de `SidePanel` (12qa)
  - dépend de : 12qa
  - fait le 2026-11-12 — `frame/modified.ts` : chaque formulaire du panneau dit, tant
    qu'il est monté, s'il est modifié (`reportModified`) ; le cadre en tire le `modified`
    du `SidePanel` (`panelModified`) et, pour une édition que la page quittée perdrait
    (`withPage`), une garde de route (`router.beforeEach`, critère de
    `closeEditWithPage` : autre chemin, ou autre date hors du présent) qui pose la
    question par `ask()` : Continuer la saisie reste sur la page. `]` (`togglePanel`, et
    la commande de la palette) et le compteur (`toggleDraft`) ferment par `close()`
    exposé, jamais par le modèle. `EditPanel` : un champ dont le texte diffère de
    l'ouverture, ou un motif tapé ; `ClosePanel` : la cascade cochée ou un motif tapé
    (`motiveTyped` exposé par `useWriteAction`) ; `DraftSubmit` : résumé autre que celui
    du brouillon et que le résumé générique prérempli, ou description changée. Tests :
    parcours `edit` (nouveau : Échap, compteur, lien de la navigation ; Continuer la
    saisie garde la saisie et la page ; Abandonner ferme, rien d'écrit), `draft` (Échap
    dans le résumé tapé) ; `accessibility.spec.ts` (axe dans chaque parcours).
    La clause « `CommitForm` : motif saisi » tombe : `CommitForm` ne sert que dans les
    maquettes ; le motif du shell est celui de `DraftSubmit`.
  - passe de correction (revue, 2026-11-12 ; Q-021-22 : aucune perte de saisie sans
    question) :
    - une nouvelle demande d'édition (`c`, `e`, « Modifier », « Clore… ») qui
      remplacerait une édition modifiée passe par `ask()` (`guardEdit` de
      `views/edit/panel.ts`, posé par le cadre) ; « Continuer la saisie » l'écarte ;
    - les onglets des panneaux restent montés une fois montrés (`Tabs` `keepMounted`,
      `unmountOnHide` de Reka à faux, contenu monté à la première montre : rien n'est lu
      plus tôt) ; un ouvreur dans un onglet caché compte pour parti (`closeEdit`), et
      `DraftOperations` attend que son onglet se montre pour rendre le focus à sa ligne ;
    - le motif du brouillon est gardé hors du formulaire (`draft/motive.ts`), revu sur
      l'autre onglet comme sur la page `/drafts/:id` ; le cadre dit le panneau modifié
      tant qu'un motif tapé est gardé hors de la page du brouillon, et le jette sur
      `close` (« Abandonner ») ; une raison relue n'écrase plus un motif tapé ;
    - un VID proposé pour le groupe d'un VLAN ne compte pas pour une saisie ; une
      question de la garde retirée parce que `modified` est repassé à faux laisse la
      navigation se faire.
    Tests : Vitest `ui/test/tabs-keep.test.ts` ; parcours `edit` (onglet et retour, `c`
    qui demande, « Abandonner » sur une navigation), `draft` (motif revu après l'onglet
    Détail) ; parcours `admin` et `shortcuts` rejoués.
  - choix : un registre de sources (`frame/modified.ts`) plutôt qu'un événement par
    formulaire ; la garde de route ne demande que pour une édition (le motif du
    brouillon, gardé hors du formulaire, ne se perd pas d'une page à l'autre) ; onglets
    gardés montés plutôt qu'une question au changement d'onglet.
  - reste : `]` au clavier non joué dans un parcours (même chemin que le compteur) ;
    « Annuler » du formulaire d'édition ferme l'édition sans demander (hors de Q-021-22) ;
    ni changement de date ni `ClosePanel` modifié joués dans un parcours ; le motif
    gardé d'un brouillon abandonné ailleurs reste en mémoire jusqu'au rechargement.
- [x] 16h. « À propos » et actions des plugins, reste prouvé : versions (shell, SDK,
  `@kartenn/ui`, Vue, serveur) et fuseau horaire affichés ; une action de plugin absente
  sans sa permission — verify : parcours `shell` et `plugins` sous les deux navigateurs ;
  lignes 31, 92, 187 et 198 cochées
  - où : `web/v2/shell/src/frame/AboutView.vue`, `plugins/` (`PluginAction`) ; anciens
    parcours `web/e2e/journeys/{shell,plugins}.spec.ts`
  - dépend de : 16
  - fait le 2026-11-03 : parcours seulement, le code était livré par la 16 (`AboutView`,
    `extensionsOf` filtré par les droits). `shell` (« the about page names the versions of
    the build and of the server, and the time zone ») : Bob ouvre « À propos » par le menu
    du compte ; les six lignes de `Versions` comparées à ce que le build et le serveur
    servent, aucune écrite à la main — `package.json` du shell, du SDK et de `@kartenn/ui`,
    celui de `vue` résolu depuis le shell, `version` de `GET /v1/capabilities` — et le
    fuseau du contexte (`timezoneId`), un autre que celui de la machine. `plugins` (« an
    action is absent without its permission ») : Bob (lecteur) sur la fiche du préfixe des
    données, l'onglet « Exemple » du plugin d'exemple placé, le menu « Actions » ouvert avec
    « Clore… » (refusé) et sans « Ajouter une adresse au brouillon » (`core.address.edit`,
    celle d'Alice dans `example`). `PARITE.md` : lignes 31, 92, 187 et 198 cochées.
  - choix : l'action absente est celle du plugin d'exemple sur `prefix`, pas `review` du
    plugin `notes` (sur `changeset`, dont la fiche demande déjà `core.changeset.read`, la
    permission de l'action : aucun compte ne verrait la fiche sans elle) ; le serveur
    comparé au champ `version` des capacités, que le shell lit dans `modules[core]` (même
    `CARGO_PKG_VERSION` de `kartenn-rest`).

- [x] 16i. Cadre réduit hors session (arbitré le 2026-11-11 par l'utilisateur, relevé à
  la clôture de la 16) : sur la page de connexion et « À propos » sans session, le cadre
  ne rend ni la barre latérale, ni la pastille de date, ni le compteur du brouillon ;
  l'en-tête et le contenu restent ; une ligne au design de la 020 (cadre) — verify :
  capture de la page de connexion et d'« À propos » hors session, mise à jour
  (`k-web-shots --update` ciblé), Vitest du cadre (rien de monté sans session), parcours
  `session` inchangé ; `ui-reviewer` et `a11y-auditor` (repères et ordre du focus du
  cadre réduit)
  - où : `web/v2/shell/src/frame/ShellFrame.vue` (`DatePill`, `DraftCounter`, barre
    latérale), `specs/020-systeme-de-design/design.md` (cadre), `e2e/journeys/session.spec.ts`
  - dépend de : 16
  - fait le 2026-11-11 : `ShellFrame.vue` lit `signedIn` (`me !== null`) ; hors session,
    ni `SidebarNav`, ni son tiroir et son bouton, ni `DatePill`, ni `TimeBand`, ni
    `DraftCounter`, ni `SidePanel` ; la colonne prend la largeur (`data-sidebar="none"`),
    en classe compacte le fil d'Ariane au bord de la surface ; `[` et `]` ni listés ni
    entendus (`framed` de `shellShortcuts`, `shown`), commandes de la palette « barre
    latérale », « panneau » et « Revenir au présent » absentes. Design de la 020 :
    paragraphe « Cadre réduit hors session » sous les classes de largeur. Vitest :
    `frame.test.ts` (« the frame out of a session » : rien de monté à `/login` et
    `/about`, tout une fois connecté), `shortcuts.test.ts` (`[` et `]` hors de l'aide).
    Parcours `session` et `accessibility` verts aux deux navigateurs (hors l'échec connu
    d'`events.spec.ts` sans axe, déjà sur main ; un dépassement Firefox de « reads of the
    background », vert relancé seul).
  - choix : la bande du contexte daté et le panneau de droite suivent la pastille et le
    compteur (« l'en-tête et le contenu restent ») ; aucune capture de référence ne couvre
    les pages réelles du shell (la suite `visual` ne visite que `/_ui`) : captures prises
    hors suite (1440, 1024, 768, 390 px, clair et sombre, API simulée) et relues, rien à
    réécrire.
  - reste : Ctrl+K ouvre encore la palette hors session (aide seule, recherche sans
    droits, donc vide) ; le squelette d'`index.html` montre une barre latérale avant
    `/login` (session inconnue au démarrage) ; des captures de référence des pages réelles
    du shell demanderaient au graphe d'impact (029) de les relier à leurs sources.
  - verify, écarts actés (session principale, 2026-11-11) : aucune capture de référence
    créée, la suite `visual` ne visitant que `/_ui` ; à la place, captures hors suite de
    `/login` et `/about` (1440, 1024, 768 et 390 px, clair et sombre) relues par l'agent.
    Relectures faites : `ui-reviewer` (fil d'Ariane décalé de 4 px en classe compacte,
    `[` et `]` listés sans effet : corrigés) et `a11y-auditor` (repères et ordre du focus
    sains ; aide qui listait `[` et `]` : corrigée).
  - reste (revue, mineurs) : seule l'aide est testée pour `[` et `]` (ni l'écouteur hors
    session ni l'absence des commandes de la palette) ; le cas connecté de `frame.test.ts`
    suppose `sidebarCollapsed` à faux sans le remettre à zéro ; sans `TimeBand`, une URL
    `/about?valid_at=…` hors session garde un contexte daté que rien ne montre (sans
    effet aujourd'hui).

## P3 — Plugins (phase 5)

- [x] 17. Écrans VLAN du cœur (amendée par la 026 : plus un plugin, des écrans du shell)
  avec `@kartenn/ui`, espace `network`, filtres typés, édition en place — REQ-021-27,
  REQ-026-6 — verify : parcours `vlan` vert
  - où : `web/plugins/vlan/src/index.ts` (comportement actuel), `public/manifest.json`,
    `test/` ; repris de la tâche 16 : onglets et colonnes des plugins sur les types de
    module (`vlan`, `vlan_group`) dans la fiche et les listes VLAN
    (`useExtensions("tab" | "column", type)`, `tabItems`, `gridColumns`,
    `web/v2/shell/src/plugins/slots.ts`), parcours `plugins` étendu (onglet `notes` du
    plugin de test sur `vlan`)
  - dépend de : 11, 12, 16
  - fait le 2026-11-01 — le VLAN dans les vues génériques de la 11 : deux
    collections de plus (`vlan-groups`, `vlans`, `web/v2/shell/src/views/collections.ts`),
    donc listes filtrées par l'URL (modèle de `/v1/vlan-groups` et `/v1/vlans` : clé, VID,
    nom, statut, groupe), fiches, routes et préchargement, emplacements des plugins
    (`useExtensions` par la sorte `vlan`/`vlan_group`) sans code propre ; colonnes de
    références nommées (`reference` : groupe d'un VLAN, ancre d'un groupe, lus par
    `ObjectName`), statut en `StatusBadge` (`states`), colonne VLAN de la liste des
    préfixes (`networkVlan`, `views/parts/NetworkVlan.vue` : propriétés de la ligne lues
    une fois la cellule en vue, lecture abandonnée si elle en sort, quatre lectures à la fois
    avec le nom du VLAN, `views/parts/vlan.ts` ; valeur masquée dite « Masquée »). Fiche d'un groupe : ancre,
    plage, panneau Occupation (`record/VlanOccupancyPanel.vue` : utilisés, réservés, libres
    en `BarList`, prochain VID libre, à partir d'un VID saisi, tranches en `ExpandableGrid`),
    onglet VLAN (`record/GroupVlansTab.vue`, « Créer un VLAN » sur le groupe) ; fiche d'un
    VLAN : onglet Réseaux (`record/VlanNetworksTab.vue`, réseaux lisibles seulement) ;
    onglet VLAN d'un préfixe (`record/NetworkVlanTab.vue` : VLAN porté, groupe, statut ;
    valeur masquée dite avec `prop.core.vlan.read`, sans écriture ; « Retirer le VLAN » au
    brouillon, refusé avec sa raison tant que la déclaration `core.vlan` n'est pas lue ou
    si le retrait y est déjà ; un conteneur n'en porte pas, `core.vlan.prefix_not_network`).
    Édition (12) : `EDITS` des deux sortes (`views/edit/fields.ts`, sortes de champ
    `integer` et `anchor`, raison `reference` d'une cellule qui nomme un objet), panneau
    d'édition (cibles `vlan-groups` et ancres ; VID proposé = prochain libre du groupe
    choisi, son échec dit par `problems/`, `views/edit/vlan.ts`). Vitest `test/edit.test.ts`,
    `test/views.test.ts` (état masqué, retrait déjà au brouillon) ; parcours
    `e2e/journeys/vlan.spec.ts`, `plugins.spec.ts` (onglet `notes` sur un VLAN) ; jeu des
    parcours : un groupe `campus` et le VLAN 10 `serveurs` (`global-setup.ts`).
  - choix : un VLAN se nomme « VID · nom » (`nameOf`) ; statut d'un VLAN et état d'une
    tranche en `StatusBadge` (maquette E12), non en `KeyLabel` ; groupe d'un VLAN et ancre
    d'un groupe choisis dans le panneau d'édition seulement (raison `reference` sur la
    cellule), l'ancre par un seul `Select` (entités, sites puis domaines, sorte en détail,
    valeur `<sorte>:<id>`) ; plages de VID laissées vides à la création = celles du contrat ;
    colonne VLAN
    montrée avec `core.vlan.read` seulement, à toute largeur (maquette) ; lien de la liste
    des VLAN sur le VID seul (`ownValue`), le nom dans sa colonne ; « Retirer le VLAN » au
    brouillon ; VID proposé dit aussi par le statut du panneau, prochain libre « à partir
    de » dit par une région `status`, VID saisi hors type refusé sur le champ ; une cellule
    VLAN vide, en lecture ou illisible dite en mots (texte caché), illisible en court avec
    son code (relectures a11y et UI du 2026-11-01).
  - à arbitrer (tranché le 2026-11-01 : A13, option (a), livrée par 17a et 17b) :
    « Créer un VLAN pour ce réseau » (maquette E12 : « deux opérations au
    brouillon », création du VLAN puis pose de `core.vlan`) — une valeur de propriété
    (`RawValue`) ne nomme pas un objet créé plus tôt dans le même changeset. Options :
    (a) une référence locale dans `RawValue` (`{local: n}`, contrat 004/006) : un seul
    changeset, au brouillon ou appliqué, atomique ; (b) deux changesets appliqués (VLAN
    créé, relu par groupe et VID, puis posé) : non atomique (VLAN orphelin si la pose est
    refusée, un nouvel essai recrée le VLAN), `approval=self` imposé (aucun brouillon),
    deux motifs ou un motif répété. Retirée en attendant (revue du 2026-11-01) ; la
    création d'un VLAN dans son groupe et le retrait au brouillon restent. Aussi
    (contrat) : filtres `group` de `/v1/vlans` et `anchor` de `/v1/vlan-groups` typés texte
    (pas de `collection` au contrat : un identifiant à saisir, la pastille montrerait
    l'UUID ; `anchor` non offert sur sa colonne) ; `update_vlan` et `update_vlan_group`
    sans `expected` (un brouillon ne voit pas un VLAN changé depuis sa lecture). Place des
    VLAN : espace `network` (REQ-021-2, cette ligne), la maquette E12 les met sous
    Référentiel — écart signalé, non tranché.
  - reste (au 2026-11-01, réparti le 2026-11-14 ci-dessous) : tuile de l'accueil « groupes les plus remplis » (design § « VLAN, écrans du
    cœur ») ; ancre d'un groupe par glisser
    (panneau seulement) ; cohérence d'un réseau avec l'ancre du groupe
    (`core.vlan.site_mismatch`) dans l'onglet Réseaux ; onglet Interfaces (plugin
    Machines, tâche 18) ; filtre de la colonne VLAN des préfixes ; colonne d'un plugin sur
    les listes `vlan` et `vlan_group` prouvée par un parcours (le plugin `notes` n'en
    déclare que sur `address` ; l'onglet sur `vlan` l'est) ; refus `core.vlan.vid_duplicate`
    et `core.vlan.vid_out_of_range` montrés sur leur cellule ou champ, à tester au parcours
    (la règle `fieldOfPointer` l'est en Vitest) ; captures des vrais écrans (comme la 11 :
    sans API dans le projet `visual`). Hors périmètre, pour l'utilisateur : `prepare_group`
    (`crates/kartenn-store-pg/src/vlan.rs`, vers les lignes 697-707) ne complète pas
    `core.owner` d'un groupe ancré sur un domaine créé dans le même changeset (V3) : le
    serveur le refuse.
    Constaté : deux passages complets de `k-web2-e2e` ont échoué sous Firefox seulement
    (`session`, `time`, `edit`), des modules refusés par `integrity` dont le contenu reçu
    est vide (empreinte SHA-384 de la chaîne vide) ; les mêmes parcours passent seuls
    (14/14) ; `vlan` et `plugins` verts dans les deux navigateurs.
  - clos le 2026-11-14 — relevé clause par clause contre le code (main `337fdae9`,
    parcours repassés sur `5d8f076f`, après la 11o et la 11p7).
    Parcours `vlan` : vert (`e2e/journeys/vlan.spec.ts`, deux tests, Chromium et Firefox),
    étendu des refus du serveur sur le champ VID avec leur code (`core.vlan.vid_duplicate`
    pour un VID pris dans le groupe, `core.vlan.vid_out_of_range` hors de sa plage, pointeur
    `/operations/0` → champ clé `vid`, `fieldOfPointer`), saisie abandonnée sans écriture.
    Onglets et colonnes des plugins sur `vlan` et `vlan_group` : `ListView.vue`
    (`useExtensions("column", kind)`) et `RecordView.vue` (`useExtensions("tab" | "panel" |
    "action", kind)`), génériques par la sorte de la collection ; prouvés par le parcours
    `plugins` (« the screens of the referential… ») : colonne `notes-flag` sur `/vlans` et
    `/vlan-groups`, onglet `notes` sur la fiche d'un VLAN et d'un groupe, axe à chaque pas (le
    plugin de test `e2e/plugins/notes` déclare désormais l'onglet sur `vlan_group` et la
    colonne sur `vlan` et `vlan_group`). Lignes de `web/v2/PARITE.md` § « VLAN » : toutes
    cochées (vues `groups` et `vlans`, onglet `network` et colonne `vlan` des préfixes, action
    `create-vlan`, parcours réécrit), la dernière par la 17b. Manque trouvé au relevé et
    comblé : « Créer un VLAN » d'un groupe n'était offert que dans son état vide ; il est
    l'action d'une `Toolbar` au-dessus des VLAN (`record/GroupVlansTab.vue`, maquette E12
    « Actions du groupe »), là pendant la lecture, vide ou non, refusé avec sa permission ;
    le même bouton avant et après le premier VLAN, qui reprend le focus à la fermeture du
    panneau d'édition (parcours `vlan`) ; Vitest `test/group-vlans.test.ts` (3 : barre du
    groupe vide, même bouton une fois le VLAN relu, refus et sa raison). Le parcours `vlan`
    cherchait aussi le lien du groupe sur la fiche d'un VLAN sur toute la page, que
    l'onglet « Propriétés » nomme désormais aussi : cherché dans les attributs. Reste réparti : onglet Interfaces → fait par la 18 (`plugins/machines`, onglet
    `interfaces-of` sur `vlan`, parcours `machines`) ; captures des vrais écrans → 12l ;
    tuile « groupes les plus remplis » → 17c ; ancre par glisser → 17d ; cohérence
    `core.vlan.site_mismatch` dans l'onglet Réseaux → 17e ; filtre de la colonne VLAN des
    préfixes → 17f. Aucun n'est nommé par le verify de la 17 ni par la liste de parité : la
    27 ne les attend pas. Vérifié : parcours `vlan` et `plugins` (26, Chromium et Firefox) ;
    `k-web2-check --base main` : 1086 tests, 2738 captures sur 2738 (tout : les catalogues
    du shell changent), aucune réécrite ; `no_old_permission` vert.
  - choix (2026-11-14) : « Créer un VLAN » sans raccourci `c` sur la fiche, que la
    maquette lui donne (`useViewAction("create")` n'est posé que par les listes et la
    Topographie ; le poser sur toute fiche pour la seule sorte `vlan_group` ferait dire un
    refus à `c` partout ailleurs) ; une seule barre pour tous les états hors erreur, l'état
    vide sans bouton propre (son action est celle de la barre juste au-dessus) : un seul nœud,
    donc le focus rendu à l'ouvreur le trouve encore, et rien ne se décale quand la liste
    arrive (relectures UI et a11y du 2026-11-14) ; `Toolbar` à une action gardée, comme la
    maquette, pour la place des actions à venir (« Modifier le groupe », décompte).
- [ ] 17c. Tuile de l'accueil « groupes les plus remplis » (design § « VLAN, écrans du
  cœur », ligne Tuile ; maquette E0) — REQ-021-9, REQ-026-6 — verify : Vitest de
  `HomeView` ; parcours `shell` (tuile lue)
  - où : `web/v2/shell/src/views/HomeView.vue` (tuile VLAN : `reserved`, `deprecated`
    seulement), `VlanStats` du contrat (`/v1/stats`, `web/v2/sdk/src/api/schema.d.ts`),
    `record/VlanOccupancyPanel.vue` (`BarList`) à imiter
  - à arbitrer d'abord (contrat) : `VlanStats` ne sert aucune occupation par groupe ;
    l'ajouter à `/v1/stats` (filtrée par les droits, comme les autres compteurs) ou lire
    l'occupation groupe par groupe côté client (coût, plafond) — hors chemin de la bascule
- [ ] 17d. Ancre d'un groupe de VLAN par glisser (design § « VLAN, écrans du cœur » :
  « ancre par glisser ») ; aujourd'hui choisie dans le panneau d'édition seulement —
  verify : script clavier de l'alternative (WCAG 2.5.7) ; parcours `vlan`
  - où : liste `/vlan-groups` (`views/ListView.vue`, `collections.ts`), `draft.move` et
    `canMove` de la tâche 24 (`views/edit/moves.ts`) à imiter ; `TreeGrid` (attendre la
    fusion de la 11o, qui le modifie)
  - à arbitrer : sur quoi lâcher (arbre des ancres entités, sites, domaines ; aucune vue ne
    les montre ensemble) — hors chemin de la bascule
- [ ] 17e. Cohérence d'un réseau avec l'ancre du groupe de son VLAN
  (`core.vlan.site_mismatch`, avertissement 008/V9) dans l'onglet Réseaux d'un VLAN :
  colonne « Cohérence » en `StatusBadge` et `CodeBadge`, `InlineAlert` qui nomme les sites
  (maquette E12) — verify : Vitest de l'onglet ; parcours `vlan` (un réseau hors du site
  du groupe)
  - où : `web/v2/shell/src/views/record/VlanNetworksTab.vue` ; maquette
    `_ui/ecrans/E12Vlan.vue` (colonne `coherence`, vers la ligne 525) ; anomalies d'un objet
    (`record/AnomaliesTab.vue`, filtre `object` de la 11k) à imiter si l'avertissement est
    gardé en anomalie ; jeu de parcours sans site (`global-setup.ts`)
- [ ] 17f. Filtre de la colonne VLAN de la liste des préfixes (`networkVlan`) : un VLAN
  choisi par son nom, filtre `prop.core.vlan` — verify : Vitest des filtres ; parcours
  `vlan` (la liste filtrée par l'URL)
  - où : `web/v2/shell/src/views/collections.ts` (colonne `vlan` des préfixes, sans
    `filter`), `views/ListView.vue` (A7 : filtre offert si le modèle sert le paramètre),
    filtres typés des propriétés de la 11p3 à imiter
  - à vérifier d'abord : `prop.core.vlan` servi par le modèle de `/v1/prefixes`, et une
    pastille qui nomme le VLAN plutôt que son UUID (même écueil que `group` de `/v1/vlans`,
    noté à la 17) — hors chemin de la bascule
- [x] 18. Plugin Machines, même traitement, espace `hosts`, déplacement d'une machine vers
  un cluster (si Q-021-2 le retient) par le `canMove` de la tâche 24 (codes
  `machines.host.*`), sans optimisme — verify : parcours `machines` vert
  - où : `web/plugins/machines/`
  - dépend de : 17
  - découpée le 2026-11-01 en 18a et 18b (fin de P3)
  - faite le 2026-11-03 par ses sous-tâches 18a, 18b et 18ca à 18cd, fusionnées
- [x] 19. Plugin d'exemple passé au SDK 1.0, **toujours sans framework**, en racine
  d'ombre avec `handle.styles()` sur `@kartenn/ui/style.css`, lecture par le
  `QueryClient` — REQ-021-25, 27 — verify : parcours `example` vert ; invalidation reçue
  par le plugin
  - où : `web/v2/plugins/example/` (`src/index.ts`, `public/{manifest.json,styles.css,i18n}`,
    `test/example.test.ts`), à l'image de l'ancien `web/plugins/example/` (laissé tel quel) ;
    `handle.styles()` de `web/v2/sdk/src/extensions.ts` ; lien de la feuille dans
    `web/v2/shell/vite.config.ts` (`sharedModules`) ; parcours
    `web/v2/e2e/journeys/example.spec.ts` ; design 021 § « API des plugins »
  - dépend de : 16
  - fait le 2026-11-01 — plugin `web/v2/plugins/example` : une extension de chaque point
    (vue `overview` sous le référentiel, onglet et panneau du préfixe, colonne des adresses,
    action « Ajouter une adresse au brouillon » par `draft.add`, tuile `counts` de l'accueil
    par `stats.query()`), chaque élément en racine d'ombre, feuille de `@kartenn/ui` puis la
    sienne (`@layer plugins`, classes `k-example-…`) par `handle.styles()`, balisage des
    composants de la bibliothèque (`k-stack`, `k-skeleton`, `k-inline-alert`,
    `k-empty-state`, `k-button`, `k-form-field`) ; lectures par `QueryObserver` et
    `InfiniteQueryObserver` sur `queryClient` et `reads` (la vue : les préfixes IPv4,
    `within=0.0.0.0/0`, première page — les listes suivent l'ordre de création et le jeu des
    parcours compte plus de 1 000 préfixes IPv6), relues à l'invalidation ; la page de la
    vue reste, seule la place de la liste est redessinée quand ce qu'elle dit change ; le
    formulaire de l'action est relibellé en place dans une autre langue (saisie gardée).
    Premier dessin après une microtâche (un élément déjà dans la page, promu pendant
    `customElements.define`, reçoit ses feuilles et ses textes) ; une feuille refusée est
    dite par `console.warn`. SDK : `handle.styles(root, "@kartenn/ui/style.css")` reprend
    le lien du shell (`data-kartenn-sheet`, écrit par `sharedModules`) ; `QueryObserver`,
    `InfiniteQueryObserver` réexportés. Vitest : `plugins/example/test/example.test.ts`
    (15 : définitions, catalogues, feuilles, élément promu, feuille refusée, vue relue à
    l'invalidation et page gardée, date changée sur l'élément connecté, désabonnement à la
    déconnexion et avant les feuilles, erreurs de la vue, de l'onglet, de l'action — brouillon
    refusé, lecture en échec : alerte, `aria-invalid`, `aria-describedby`, code et
    Request-Id — et de la tuile, langue de l'action), `sdk/test/plugins.test.ts` (2 de
    `handle.styles`), `shell/test/build.test.ts` (lien nommé). Parcours `example` réécrit
    (3 : chargé, éléments définis, vue depuis la barre, feuilles dans l'ordre et
    appliquées, thème sombre, axe ; préfixe IPv4 créé par l'API → visible sans recharger
    dans le même élément, `h1` gardé ; tuile de l'accueil, colonne des adresses, panneau et
    onglet du préfixe, action dans le dialogue de la fiche → opération au brouillon, aucun
    changeset envoyé, fermée par son bouton), placés par les écrans de la tâche 11
    (fusionnée) : vert avec `plugins` (20, Chromium et Firefox) ; `k-web2-check` vert
    (579 tests, 2704 captures).
  - choix : observateurs de query-core (`QueryObserver`, `InfiniteQueryObserver`)
    réexportés par le SDK 1.0 plutôt qu'un abonnement au cache (`getQueryCache().subscribe`)
    — sans observateur, une requête sort du cache au bout de `gcTime` et le plugin ne suit
    plus rien ; export accepté par l'utilisateur le 2026-11-01, réversible avant le gel ;
    feuille partagée désignée par son
    spécifieur et trouvée par l'attribut `data-kartenn-sheet` du lien du shell (pas de crochet
    d'hôte) ; `ui: "^0.1"` déclaré dans le manifeste parce que le plugin lie la feuille (design
    amendé) ; contenu dans un conteneur à côté des `<link>` (un lien déplacé serait relu) ;
    rien n'est dessiné avant que les feuilles aient répondu ; `happy-dom` en dépendance de
    développement du plugin (déjà celle du SDK, du shell et de `@kartenn/ui`).
  - reste — ouverts après relecture : pas d'icône (données de `Icon` côté Vue) ni de bouton de copie du code (le
    code est un `<code>` sélectionnable) ; `data-motion="reduce"` du shell n'atteint pas le
    reflet du squelette en racine d'ombre (`:root[data-motion]` de `shared.css`, seule la
    préférence du système s'applique) ; `aria-busy` autour des squelettes de
    `ExtensionElement.vue` et `PluginView.vue`, dont le texte caché n'a pas de rôle
    `status` (hôte de la 16) ; niveau fixe `h2` du panneau, à confronter aux titres de la
    fiche qui le place.
- [x] 20. Amendement de la 017 : U2 et U14 remplacées (020/A1, 021/A1 closes), tâches web
  de la 017 en attente du socle de la 020 et de la 021, sous `web/v2/plugins/` — verify :
  amendement accepté consigné dans la 017 (decisions, requirements, tasks, design)
  - où : `specs/017-plugins-politique-flux/decisions.md` § « Amendement — socle 020 et
    021 »
  - dépend de : —
  - fait le 2026-10-23 (arbitrage de l'utilisateur)

- [x] 18a. Plugin Machines porté au SDK 1.0 sous `web/v2/plugins/machines` : listes et
  fiches des machines, clusters et interfaces, onglet Interfaces sur préfixe, adresse et
  VLAN, espace `hosts`, `@kartenn/ui` en racine d'ombre comme le plugin d'exemple —
  verify : parcours `machines` (lecture, création au brouillon) vert dans les deux
  navigateurs
  - où : `web/plugins/machines/src/index.ts` (comportement), `public/` ; à l'image de
    `web/v2/plugins/example/` (manifeste, `handle.styles()`, lectures par le
    `QueryClient`) ; parcours `web/e2e/journeys/machines.spec.ts` porté sous
    `web/v2/e2e/journeys/`
  - dépend de : 16, 19
  - fait le 2026-11-01 — plugin `web/v2/plugins/machines` (espace `hosts`, lettre `g m`),
    sans framework, chaque élément en racine d'ombre (feuille de `@kartenn/ui` puis la sienne,
    `@layer plugins`, classes `k-machines-…`), lectures par `QueryObserver` et
    `InfiniteQueryObserver` (`src/element.ts` : `follow`, `followList`, lectures suivies
    après coup et lâchées par `drop`), noms des objets cités lus une fois par le cache, quatre
    à la fois (`src/reads.ts`, `Names`). Vues : machines (liste : nature, propriétaire, site,
    hébergement ; fiche `/x/machines/machines/<id>` : attributs, interfaces avec MAC, mode
    802.1Q par VID et adresses, machines hébergées), clusters (liste ; fiche : membres et
    hébergées), interfaces (recherche par MAC, adresse et VID, critères dans l'adresse de la
    page ; fiche : machine, MAC, mode, adresses rattachées). Onglet `interfaces-of` sur
    adresse, réseau (adresses du réseau tenues par ses machines, `within`) et VLAN ; colonne
    `machine` des adresses ; action `attach` d'une adresse. Écritures au brouillon par
    `draft.add` (`src/forms.ts`) : `machines.create_machine`, `machines.create_interface`,
    `machines.update_interface` (mode aucun ou accès), `machines.attach_address` ; au présent
    seulement, bouton refusé avec la permission écrite sans elle, champs requis refusés avant
    envoi, violation du serveur dite sur son champ. Shell : un lien d'un élément de plugin
    vers une page du shell est suivi par le routeur (`ExtensionElement.vue`, clic composé ;
    modificateur, autre cible, autre origine, `/v1/…` laissés au navigateur) ; la vue d'un
    plugin recrée son élément à chaque chemin (`PluginView.vue`). Vitest
    `plugins/machines/test/machines.test.ts` (12 : éléments et ids uniques, catalogues,
    liste relue à l'invalidation, échec avec code et Request-Id, vide, création au brouillon
    et champ requis, droit absent et autre date, fiche de machine et ajout d'interface,
    recherche par adresse, onglets réseau et VLAN, action, `within`), `package.test.ts`,
    `shell/test/plugins.test.ts` (lien suivi par le routeur). Parcours
    `e2e/journeys/machines.spec.ts` (2 : lecture par la navigation, fiches de la machine, de
    l'interface et du cluster sans recharger la page, recherche par MAC, trois onglets,
    colonne, action au brouillon ; machine créée au brouillon, aucun changeset envoyé).
    `web/v2/PARITE.md` créée avec la section du plugin.
  - choix : identifiant de l'onglet `interfaces-of` (le serveur refuse deux extensions de
    même id, la vue s'appelle `interfaces`) ; lettre `g m` au plugin Machines (design
    « Raccourcis »), le plugin de test `notes` passé à `n` (parcours `plugins`) ; tables natives (`<table>`, `k-machines-table`,
    en-tête collant dans son conteneur de défilement) et listes fermées natives (`<select>`
    dans `k-field-frame`) : `DataGrid` et `Select` sont des composants Vue ; raison d'un
    bouton refusé écrite à côté de lui (pas d'infobulle sans framework) ; fiche en sections
    (attributs, interfaces, hébergées, ajout), sans onglets ni `?tab=interfaces` ; recherche
    des interfaces : adresse et VID résolus en objets (une liste par adresse et VLAN trouvés,
    fusionnées) ; critères posés par un lien suivi par le routeur du shell
    (`followLink`, nouvelle entrée de l'historique ; le retour arrière les rend par
    `popstate`, revue du 2026-11-01 : le SDK n'a ni navigation ni requête de la vue) ; mode
    802.1Q aucun ou accès comme l'ancien plugin ; machines, entités et VLAN proposés : la
    première page ; relectures a11y et UI du 2026-11-01 : focus sur le `h1` de la page atteinte
    par un lien du plugin, gardé sur « Afficher la suite » (sinon la dernière ligne), résultat
    de la recherche dit par une région `status` présente dès le départ (compte visible caché
    d'elle, sans « (s) »), « Ajout au brouillon… » dit pendant l'envoi, issue effacée à la
    saisie suivante, refus du serveur : focus sur le premier champ refusé, VLAN d'accès requis
    en mode accès seulement, VID en texte numérique (`inputmode`), `scroll-padding` sous
    l'en-tête collant, onglet sans `h2` (son panneau est nommé par l'onglet du shell).
  - reste : mode trunk à poser (lu seulement) ; déplacement vers un cluster (18b) ; listes
    sans tri ni filtre de colonne (pas de `DataGrid` sans framework) ; listes de choix
    (propriétaire, machine, VLAN) limitées à la première page, sans recherche ; codes
    `machines.name.duplicate`, `machines.mac.duplicate`, `machines.address.vlan_mismatch`
    montrés sur leur champ par la règle du pointeur, non prouvés au parcours (le brouillon ne
    les dit qu'à la simulation) ; captures des vues (projet `visual` sans API) ; ouverts après
    relecture : code et Request-Id d'un échec non copiables (`CodeBadge` est un composant
    Vue ; un `<code>` sélectionnable), état d'une interface en texte (pas de `StatusBadge`
    sans ses icônes), classes de titre, d'intertitre et de paires recopiées du plugin
    d'exemple (`k-machines-…` ≈ `k-example-…`) : une classe partagée de `@kartenn/ui` pour les
    plugins sans framework à décider, de même qu'une table et une liste fermée sans
    framework ; nombre de machines non dit dans la liste ; une seule cellule de nom en lecture
    dit « Chargement » par ligne.
- [x] 18b. Déplacement d'une machine vers un cluster par `canMove` du SDK (Q-021-2) :
  cycle refusé (`machines.host.invalid`), autre site : avertissement du serveur montré
  (`machines.host.site_mismatch`), déplacement au brouillon (amendé le 2026-11-02 : la
  règle reste un avertissement, 009 M11), sans optimisme ; reste
  « machines » de la tâche 24 (arbre `declared`, réponse ralentie) — REQ-021-17, 32 à
  34 — verify : Vitest de `canMove` sur les codes `machines.host.*` ; parcours
  `machines` et `moves` étendus (glisser au clavier)
  - où : `canMove` et `useMoves` (tâche 24, `web/v2/shell/src/`, SDK) ; plugin de la 18a
  - dépend de : 18a
  - en cours : fait le 2026-11-02 — SDK : `canMove` demande `machines.edit` pour une
    machine (018 R2 : la ressource qui porte le nom du module s'omet ; il demandait
    `machines.machine.edit`, qui n'existe pas, et refusait tout déplacement) ; `draft.move`
    (`src/draft.ts`, `moveInDraft`) : le changement de parent au brouillon actif, celui du
    même nœud remplacé (membre tiré de `PARENTS[type].via`), le brouillon simulé, verdict
    de sa seule opération (`valid` et ses avertissements, `refused` et sa règle, `unknown`
    et son problème) ; `useMoves` du shell le reprend par `moveInActiveDraft` de
    `@kartenn/sdk/host` (sans ouvrir le brouillon). Plugin : section « Hébergement » de la
    fiche d'une machine (`src/hosting.ts`, `HostMove`) — liste native des hébergeurs
    (aucun, clusters, machines, dix pages au plus), chacun jugé par `canMove` (ancêtres
    tirés des hôtes de la liste), refusés dits dans leur nom avec la raison et le code
    (`machines.host.invalid`), refusés de nouveau sur le champ s'ils sont envoyés ;
    `machines.update_machine` `host` au brouillon par `draft.move`, « en attente » jusqu'à
    la réponse, hébergeur lu gardé ; refus du serveur (alerte, règle, opération gardée),
    avertissements dans une région `status` présente dès le départ ; lecture des
    hébergeurs dite, échec par `failure`. Vitest `sdk/test/moves.test.ts` (codes
    `machines.host.*` : soi-même, hôte direct et indirect, type non admis, cluster, autre
    machine, racine, `machines.edit`), `plugins/machines/test/machines.test.ts` (4 :
    hébergeurs jugés et refus au champ, droit absent, attente puis avertissement
    `site_mismatch`, refus de la simulation et opération remplacée), `shell/test/moves*`
    inchangés et verts. Parcours `moves` (« the host of a machine » : hôte sous lui-même
    et sous son invité refusé par le client, rien envoyé ; vers un hôte d'un autre site,
    simulation retenue → « en attente », hébergeur inchangé, puis avertissement
    `machines.host.site_mismatch`, aucun changeset), `machines` (machine physique vers un
    cluster : offert par le client, refusé par la simulation `machines.host.invalid`, à sa
    place) ; verts dans les deux navigateurs.
  - choix : pas d'arbre de l'hébergement ni de glisser — le `TreeGrid` est un composant
    Vue et le plugin n'a pas de framework : la liste des hébergeurs est l'alternative au
    dépôt (2.5.7), au clavier ; brouillon seulement (W10 : un plugin n'envoie pas de
    changeset), donc ni « Appliquer » ni Ctrl+Entrée ; hébergeur actuel et hébergeurs
    refusés gardés dans la liste, actifs (une option désactivée est sautée au clavier et
    tue sa raison), refusés au champ à l'envoi ; liste non redessinée sous le focus ;
    section placée après les attributs ; `as Operation` sur l'opération (le type du
    contrat ne décrit pas les opérations des modules).
  - arbitré le 2026-11-02 (session principale) : `machines.host.site_mismatch` reste un
    avertissement (009 M11) — le serveur applique et ouvre une anomalie, il ne compare que
    les machines hôtes, pas les clusters (`pg::site_mismatches`) ; clauses de la 18b et de
    la 24 amendées. Corrections de relecture : un autre hôte choisi pendant l'envoi revient
    à l'hôte envoyé, l'attente reste dite ; test direct de `moveInDraft`
    (`sdk/test/draft-move.test.ts` : remplacement, violations d'autres opérations
    filtrées, refus de la sienne, `unknown`).
  - fait le 2026-11-14, relevé contre le code (main 15dd7ca1) : le reste de `verify` était
    tenu par la 18cd (plugin en Vue). Arbre `declared` : `HostingTree.vue` (`TreeGrid`
    `parentMode: "declared"`, glisser, Ctrl+X puis Ctrl+V, « Déplacer vers… », `canMove`
    et `draft.move` du SDK, sans `setQueryData`) — l'abandon du glisser, laissé à
    arbitrer, est sans objet. Parcours `moves` « the host of a machine » : dans l'arbre,
    vers un cluster au clavier (Ctrl+X, flèches, Ctrl+V), simulation retenue → « en
    attente » à sa place (`data-move="pending"`, niveau inchangé), puis au brouillon,
    aucun changeset (réponse ralentie dans un `TreeGrid` `declared`, reste de la 24) ;
    hôte sous son invité refusé par le client (`machines.host.invalid`, rien envoyé), puis
    vers un hôte d'un autre site, avertissement `machines.host.site_mismatch` ; mêmes cas
    depuis la fiche. Parcours `machines` : physique vers un cluster offert par le client,
    refusé par la simulation (`machines.host.invalid`), hébergeur inchangé. Vitest
    `sdk/test/moves.test.ts`, `sdk/test/draft-move.test.ts` (21),
    `plugins/machines/test/machines.test.ts` et `clusters-view.test.ts` (24) verts ;
    parcours `machines` et `moves` : 26 sur 26 dans les deux navigateurs. REQ-021-33 amendée selon A22 (acceptation : refus par `machines.host.invalid` de la
    simulation, avertissement `machines.host.site_mismatch` d'un autre site, au brouillon).
  - choix : aucun code ajouté (la clause tient) ; Ctrl+Entrée sur un dépôt de l'arbre de
    l'hébergement laissé à la 24f (spec à valider), hors de cette `verify`.
  - reste (hors de `verify`, ouvert) : `writeRight` de `sdk/src/data/moves.ts` omet la
    ressource qui porte le nom du module par une heuristique du pluriel (`machines` ↔
    `machine`), alors que `editRight` du shell (`views/edit/fields.ts`) écrit toujours
    `<type>.edit` — juste pour les types de chacun aujourd'hui (`core.*.edit`,
    `machines.edit`, `machines.cluster.edit`), une table de permissions tirée du contrat
    les accorderait (le contrat ne les porte pas : à arbitrer) ; les hébergeurs de la
    section « Hébergement » de la fiche au-delà de dix pages ne sont pas offerts
    (`HostSection.vue`, `MAX_PAGES` : la liste lue donne les ancêtres que `canMove`
    demande) ; sous Firefox, le parcours de la 24 « an entity changes parent by its
    menu » a perdu une fois le focus du champ pendant le parcours complet (vert relancé
    seul, deux fois).

- [x] 18c. Plugin Machines en Vue 3 et `@kartenn/ui` (A19) : réécriture de
  `web/v2/plugins/machines` en Vue, à la forme que prendront les plugins de la 017 ;
  listes en `DataGrid` (tri et filtres typés), choix en `Select`/`Combobox`, codes en
  `CodeBadge`, état en `StatusBadge` ; arbre de l'hébergement en `TreeGrid` à parent
  déclaré (`derived: false`), glisser et alternative au clavier par `canMove` et
  `draft.move` (18b), sans optimisme ; poids ≤ 50 Ko gzip (REQ-021-30) — verify :
  parcours `machines` et `moves` (déplacement glissé au clavier vers un cluster, refus
  par cycle, avertissement `machines.host.site_mismatch`) dans les deux navigateurs ;
  budget du plugin tenu ; revues `ui-reviewer` et `a11y-auditor`
  - où : `web/v2/plugins/machines/` (comportement de la 18a et de la 18b, tests à
    garder) ; montage d'un plugin Vue : la forme décidée pour la 017 (017-2) ou, à
    défaut, celle qu'en tire cette tâche, notée au design 021 § « API des plugins » ;
    `TreeGrid` de `web/v2/ui` ; `web/v2/shell/src/views/edit/moves.ts` (forme)
  - dépend de : 18b
  - découpée le 2026-11-02 en 18ca à 18cd (A23), chacune migrant des éléments entiers ;
    l'ancien élément et ses tests ne partent qu'une fois l'élément Vue et ses tests verts
  - faite le 2026-11-03 par ses sous-tâches 18ca à 18cd, fusionnées
- [x] 18ca. Montage d'un plugin de première partie en Vue (modèle de la 017) : une
  application par élément (`createApp`) montée dans son DOM léger à `connectedCallback`,
  démontée à `disconnectedCallback`, contexte du SDK, lien en `<a href>` suivi par
  `ExtensionElement`, portail ; lectures par composables sur `QueryObserver` et
  `InfiniteQueryObserver` (A23), `Names` ; textes par le handle ; manifeste
  `framework: "^3.5.0"` ; build `@vitejs/plugin-vue`, modules partagés externes ;
  contrôle du poids d'un plugin (gzip de `dist/plugins/machines/<v>/index.js` ≤ 50 Ko,
  modules partagés exclus) ; la forme écrite au design 021 § « API des plugins » ; la
  colonne `machines-column` et l'action `machines-attach` en Vue (`Select`/`Combobox`,
  `CodeBadge` sur un échec) — verify : Vitest du plugin (tests de la colonne et de
  l'action repris) ; parcours `machines` (colonne, action au brouillon) dans les deux
  navigateurs ; contrôle du poids ; `k-web2-check --base main` ; revues `ui-reviewer`
  et `a11y-auditor`
  - où : `web/v2/plugins/machines/{src/index.ts, src/host.ts (nouveau), src/data.ts
    (nouveau), src/extensions.ts, vite.config.ts, package.json, public/manifest.json,
    test/}` ; ADR 0038 (décisions 3 et 4) ; `web/v2/shell/vite.config.ts` (`SHARED`, en
    lecture) ; `web/v2/ui/src/context.ts` (lien, portail) ; `web/v2/shell/src/data/query.ts`
    (forme des composables) ; `web/v2/shell/scripts/size.ts` (forme du contrôle)
  - dépend de : 18b
  - fait le 2026-11-02 — hôte mince `src/host.ts` (`pluginHost()` : `element(composant)`,
    `define(définition)` ; `useHosted`, `useTexts`, `usePresent`, `useClose`) et
    composables `src/data.ts` (`useRead`, `useList`, `useNotified`), sans rien qui nomme
    le plugin, forme écrite au design 021 § « API des plugins » ; `MachineColumn.vue`
    (machines liées aux dates du contexte, lecture relancée à d'autres dates, échec dit par
    un mot et le code en `CodeBadge`) et `AttachAction.vue` (`Combobox` de machines par
    `q`, `Select` de ses interfaces, `Checkbox`, refus au champ puis focus, refus du
    serveur en `InlineAlert` avec règles, code et `Request-Id`, `forbidden` sur le bouton,
    message à une autre date) ; anciennes classes retirées de `extensions.ts` ; manifeste
    `framework: "^3.5.0"`, `vue` et `@kartenn/ui` en dépendances, `vue-tsc`, build
    `@vitejs/plugin-vue` (`index.js` : 16,2 Ko gzip -9, n'importe que `vue`,
    `@kartenn/sdk`, `@kartenn/ui`). Contrôle du poids : `web/v2/tools/plugin-size.ts`
    (build en mémoire, gzip -9, ≤ 50 Ko, modules partagés importés jamais embarqués), lu
    par `plugins/machines/test/package.test.ts`, donc par `k-web2-check` (Vitest), sans
    toucher `devenv.nix`. Vitest : 25 à la première passe (colonne : montage en DOM léger, déplacement gardé,
    démontage ; liens datés ; illisible avec code ; action : recherche puis ajout au
    brouillon, champs requis et texte non choisi, refus avec règles et codes, droit absent
    puis autre date). Parcours `machines` vert dans les deux navigateurs (envoi sans
    machine refusé au champ et focalisé, puis rattachement au brouillon).
  - choix : montage et démontage une microtâche après `connectedCallback` et
    `disconnectedCallback` (un montage pendant le rendu du shell viderait ses rappels au
    milieu de son correctif ; un élément déplacé garde son application), le contexte
    lâché aussitôt à la déconnexion ; `app.config.idPrefix` par application (`useId`
    compte par application) ; portail `#k-layers` du cadre, `body` sans lui ; aucun
    `provideLink` (liens `<a href>` suivis par `ExtensionElement`) ; erreurs d'une
    application dans la console (`errorHandler`) ; colonne : chargement en `Skeleton`
    `variant="cell"` (barre seule `aria-hidden`, sans région `status`, faite pour une
    cellule ; d'abord écarté sur une prémisse fausse, corrigé à la relecture), interfaces
    suivies par `useList` (relues à l'invalidation et à d'autres dates) depuis le tour de
    la cellule, la place parmi les lectures simultanées (`limited`) rendue à la première
    réponse ; le code seul en `CodeBadge` dans la cellule (pas de `Request-Id`, trop
    large) ; action : la
    machine cherchée par `q` (la liste entière n'était lue qu'en première page), la
    première interface prise, `Select` remonté à l'arrivée de ses options (piège de
    `SelectValue`) en gardant le focus, raison du champ vide en aide (`hint`) plutôt qu'en
    texte de remplacement, issue dans une région `status` à côté des boutons (comme
    l'ancien formulaire) ; trois clés de catalogue (`action.chooseMachine`,
    `action.noInterface`, `action.pick`) et `form.dated`.
  - relectures : `ui-reviewer` (aucun bloquant) et `a11y-auditor` (aucun bloquant) ;
    corrigés : erreur d'un champ effacée quand il change, focus gardé au remontage du
    `Select`, raison du champ des interfaces en aide, texte tapé sans suggestion dit,
    refus sans droit dit aussi par Entrée dans un champ, interface non exigée tant
    qu'aucune machine n'est choisie. Passe de corrections (revue de branche) : `Skeleton`
    de cellule ; colonne sur `useList` ; tests du modèle (abonnement au contexte rendu et
    observateur parti du cache au démontage, contexte suivi de nouveau après un
    déplacement, colonne relue à l'invalidation et à deux dates successives) ; alerte de
    refus effacée au changement d'un champ ; interfaces lues page après page jusqu'à
    `MAX_PAGES`, au-delà annoncées en aide (`action.firstInterfaces`) ;
    `@kartenn/ui/style.css` retiré des modules partagés (absent de la carte d'import) ;
    une feuille émise par le plugin fait échouer le contrôle du poids (A23). Vitest : 27.
  - reste : la liste des modules partagés est recopiée à trois endroits
    (`SHARED` de `web/v2/shell/vite.config.ts`, celui de
    `web/v2/plugins/machines/vite.config.ts`, `SHARED_MODULES` de
    `web/v2/tools/plugin-size.ts`) : une source unique à tirer ; les liens de la colonne dans la grille des adresses ne passent pas par
    `ExtensionElement` (rechargent la page, comme avant) ; un contexte qui passe à une
    autre date pendant que le dialogue est ouvert remplace le formulaire sans y mettre le
    focus ; un refus du serveur sans champ connu laisse le focus au bouton (l'alerte est
    dite) ; une erreur de rendu d'une application après sa création n'est pas confinée en
    encart ; `host.ts` et `data.ts` à déplacer dans `@kartenn/ui` (A23).
- [x] 18cb. Vue Machines en Vue : liste en `DataGrid` (sans tri : la route n'en sert pas ;
  filtres typés) ; fiche en
  sections (attributs, interfaces avec `StatusBadge`, machines hébergées, ajout d'une
  interface) ; section « Hébergement » de la 18b en `Combobox` d'hébergeurs jugés par
  `canMove`, envoyés par `draft.move`, sans optimisme ; anciens `MachinesView` et
  `HostMove` retirés — verify : Vitest ; parcours `machines` (lecture, création au
  brouillon, machine physique vers un cluster refusée par la simulation) et `moves`
  (« the host of a machine ») dans les deux navigateurs ; revues
  - où : `src/views.ts` (`MachinesView`, `MachineList`, `MachineRecord`),
    `src/hosting.ts`, `src/forms.ts` ; les tests de la 18b dans `test/machines.test.ts` ;
    `web/v2/shell/src/views/edit/moves.ts` (forme)
  - dépend de : 18ca
  - fait le 2026-11-02 — `machines-machines` = `host.element(MachinesView.vue)` : la liste
    (`MachineList.vue`) ou la fiche (`MachineRecord.vue`) selon le chemin, choisie à la
    création de l'élément ; `DataGrid` de `/v1/machines` (nom en lien, nature en
    `KeyLabel`, propriétaire, site, hébergeur cités par `CitedName.vue` sur `Names`), page
    suivante lue quand la fenêtre approche de la fin, au plus `MAX_PAGES` (au-delà, dit
    au-dessus de la grille) ; filtres typés en `FilterPicker`, tirés des paramètres que
    sert la route (`filterModel` du SDK : `name_prefix`, `kind`, `owner`, `site`, `host`,
    `cluster`), filtres de colonne (Alt+Bas), tenus dans la requête de la page par un lien
    que suit le shell, relus au retour arrière (`popstate`) ; **pas de tri** : la route n'a
    aucun paramètre de tri (ordre du serveur, par nom, dit dans l'introduction), et trier
    les seules pages chargées mentirait ; fiche en sections (attributs en `dl` dans `Grid`,
    « Hébergement », interfaces en `DataGrid` avec `StatusBadge`, machines hébergées, ajout
    d'une interface) ; `HostSection.vue` : `Combobox` des hébergeurs lus (aucun, clusters,
    machines, toutes les pages du cache), chacun jugé par `canMove`, refusé gardé avec sa
    raison et son code dans son nom, refusé de nouveau sur le champ (focus) s'il est
    envoyé ; `draft.move`, sans optimisme (« en attente », hébergeur lu inchangé),
    avertissements en `InlineAlert` ; `DraftForm.vue` (formulaire au brouillon commun aux
    deux créations) ; `problems.ts` (`linesOf`, repris par `AttachAction.vue`) ; états
    d'erreur avec « Réessayer » (`refetchQueries` sur la clé de la lecture). `host.ts`
    inchangé, `data.ts` aussi hormis `retry()` (passe de corrections). Retirés : `MachinesView`, `MachineList`,
    `MachineRecord`, `hostOf` de `views.ts`, `src/hosting.ts` (`HostMove`) et leurs tests ;
    `forms.ts` reste (vue Interfaces). Poids : `index.js` 20,0 Ko gzip -9 (16,2 avant),
    aucune feuille émise par Vue. Vitest : 30 (`test/machines-view.test.ts`, 11 : liste,
    invalidation, échec puis « Réessayer », vide, filtres de la requête et retour arrière,
    deux valeurs d'un filtre refusées, création et champ requis, droit absent et autre
    date ; fiche et ajout d'interface, hébergeurs jugés, droit absent, « en attente » puis
    avertissement, refus de la simulation). Parcours `machines` et `moves`
    (`--grep machine`) : 10 sur 10 dans les deux navigateurs (liste filtrée par l'en-tête
    « Nom », fiche, cluster refusé par la simulation, création au brouillon, cycle refusé,
    autre site avertie).
  - choix : filtres offerts `name_prefix` (« Début du nom ») plutôt que `name` (égalité
    exacte), `network` laissé (il faut choisir un préfixe : à ajouter avec une suggestion
    des réseaux) ; la route refuse un paramètre répété : deux valeurs d'un filtre sont
    refusées par un encart avant tout envoi (la liste n'est pas lue) ; propriétaire en
    `Select` de la première page des entités par slug, le premier pris, nature
    « physique » par défaut ; hébergeurs : nom et raison du refus dans le libellé de
    l'option (lus avec elle, gardés dans le champ une fois pris), type en détail ; un
    second envoi pendant un déplacement est ignoré (le champ n'est plus ramené à
    l'hébergeur envoyé, son attente reste dite) ; Entrée sur une cellule sans lien ouvre la
    fiche (`activate`), le focus au titre de la nouvelle page (`arrival`, clics sans
    modificateur seulement) ; message de droit vidé avant d'être redit.
  - relectures : `ui-reviewer` et `a11y-auditor` sur le code (sans rendu : serveur et
    base chargés par d'autres agents), aucun bloquant ; corrigés : « Réessayer » sur les
    quatre états d'erreur, `arrival` ignoré pour un clic avec modificateur, un autre bouton
    ou une autre cible, message de droit redit à chaque envoi, alias `Row` de `Cluster`
    renommé.
  - reste : titres `h1`/`h2` et `dl` des attributs sans style de la bibliothèque (pas de
    composant de titre ni de liste de propriétés dans `@kartenn/ui`, A23 : à demander à la
    bibliothèque) ; pas de pied de liste (décompte), ni de raccourci `/` sur la barre de
    filtres (le `ViewPanel` du shell n'est pas partagé) ; le tiret « aucun » est écrit à
    trois endroits ; avertissements du déplacement annoncés par l'`InlineAlert` créé avec
    eux (à vérifier au lecteur d'écran) ; le mode 802.1Q en texte, sans clé ; filtre
    `network` ; le parcours `moves` « the tree of the prefixes » a échoué ici
    (`If-Match is not an entity tag` à la soumission d'un brouillon, code que cette tâche
    ne touche pas ; non revérifié sur main) ; `AttachAction.vue` double encore une part de
    `DraftForm.vue` (envoi au brouillon, refus, issue dite) ; `lines()` d'`AttachAction`
    recopie l'appel de `linesOf` de `problems.ts`.
  - passe de corrections (revue de branche) : la lecture des pages suivantes s'arrête sur
    une page refusée (hébergeurs de `HostSection`, interfaces d'`AttachAction`, qui
    relançaient la page sans fin), dite avec « Réessayer » (`RetryAlert.vue`), aussi sur
    l'échec d'une page suivante de la liste et des deux grilles de la fiche ; `useList`
    de `data.ts` gagne `retry()` (première page relue, ou la page suivante refusée) ;
    « Hébergement » monté seulement une fois la machine lue. Vitest : 32 (page 2 refusée
    lue une seule fois, puis relue par « Réessayer », pour les hébergeurs et pour
    l'action ; second envoi ignoré pendant un déplacement).
- [x] 18cc. Vue Interfaces et onglet `interfaces-of` en Vue : recherche par MAC, adresse et
  VID, critères dans la requête de la page, `DataGrid`, fiche d'une interface ; onglet sur
  adresse, préfixe et VLAN ; anciens `InterfacesView` et `InterfacesTab` retirés —
  verify : Vitest ; parcours `machines` (recherche par MAC, retour arrière, trois onglets)
  dans les deux navigateurs ; revues
  - où : `src/views.ts` (`InterfacesView`, `criteriaOfPage`, `idsOf`, `interfaceTable`),
    `src/extensions.ts` (`InterfacesTab`)
  - dépend de : 18ca
  - fait le 2026-11-02 — `machines-interfaces` = `host.element(InterfacesView.vue)` :
    recherche (`InterfaceSearch.vue`) ou fiche (`InterfaceRecord.vue`) selon le chemin ;
    `machines-tab` = `host.element(InterfacesTab.vue)`. Recherche : formulaire `search`
    (MAC, adresse, VID en `TextField`), critères dans la requête de la page par un lien que
    suit le shell, relus au retour arrière (`popstate`), adresse et VID résolus en objets
    (première page de `/v1/addresses?value=`, `/v1/vlans?vid=`, relus à d'autres dates), une
    liste de `/v1/interfaces` par paire, interfaces fusionnées dans `InterfaceGrid.vue`
    (`DataGrid` : interface en lien, machine par `CitedName`, MAC, mode 802.1Q, état en
    `StatusBadge` ; Entrée sur une cellule sans lien ouvre la fiche), pages suivantes lues
    à l'approche de la fin, jamais après une page refusée (`RetryAlert`), au plus
    `MAX_PAGES` (au-delà, dit) ; ce qui est trouvé dit par une région `status` présente dès
    l'origine. Fiche : attributs en `dl`, adresses rattachées en `DataGrid` (lues dans la
    carte de la machine), mode 802.1Q au brouillon par `DraftForm.vue` (`Select` du mode,
    `Select` du VLAN d'accès en mode accès seulement, première page des VLAN, `RetryAlert`
    si elle est refusée). Onglet : interfaces de l'adresse ou du VLAN en `InterfaceGrid` ;
    d'un réseau, adresses du réseau tenues par ses machines (`?network=`, puis les adresses
    de chacune par `useReads`), interface, machine et adresse en liens. `PageRoot.vue` (tiré
    de `MachinesView.vue`) : focus au titre à l'arrivée par un lien du plugin, sans titre
    pour l'onglet. `data.ts` gagne `useReads` et `useLists` (un observateur par lecture,
    gardé d'un jeu d'options à l'autre par le hachage de sa clé ; `next()` ne relit pas une
    liste refusée), écrits au design 021 § « API des plugins ». Retirés : `InterfacesView`,
    `criteriaOfPage`, `idsOf`, `interfaceTable`, `InterfaceRow`, `src/extensions.ts`
    (`InterfacesTab`), `src/forms.ts` et leurs tests ; `views.ts` ne garde que
    `ClustersView`. Poids : `index.js` 20,8 Ko gzip -9 (20,0 avant), aucune feuille émise.
    Vitest : 40 (`test/interfaces-view.test.ts`, 11 : critères de la page puis recherche par
    MAC, adresse et VID, lien suivi, retour arrière ; refus avec codes puis « Réessayer »,
    rien trouvé ; page suivante refusée lue une fois puis relue par « Réessayer », dans la
    recherche et dans l'onglet ; Entrée sur une cellule ; fiche, attributs, adresses, mode
    au brouillon ; trunk refusé sur son champ, droit absent ; fiche illisible, autre date ;
    onglet d'un réseau, d'une adresse et d'un VLAN, refus et vide). Parcours `machines` :
    6 sur 6 dans les deux navigateurs (grilles `Interfaces trouvées` et `Adresses
    rattachées`, focus au titre de la fiche, retour arrière, trois onglets).
  - choix : une interface en trunk n'a aucun mode choisi dans le formulaire (l'ancien
    proposait « aucun » à tort), une aide dit que ses VLAN étiquetés se changent par l'API ;
    VLAN d'accès en `Select` de la première page, une aide quand il en reste ; erreurs des
    champs effacées quand le mode change ; recherche sans critère : toutes les interfaces
    (comme avant) ; la phrase de la région `status` vidée sur une erreur (dite par l'alerte
    de l'`ErrorState`) ; vide après recherche avec « Effacer » quand un critère est posé ;
    nom d'interface en `md` et machine en `fill` ; clés `search.first`, `tab.first`,
    `interfaces.firstVlans`, `interfaces.failedVlans`, `interfaces.trunkHint` (`tab.more`
    retirée).
  - relectures : `ui-reviewer` et `a11y-auditor` sur le code (sans rendu : serveur et base
    partagés), aucun bloquant ; corrigés : phrase de la région `status` en double avec
    l'alerte d'erreur, VLAN illisible dit avec « Réessayer », erreur du VLAN gardée après
    changement de mode, vide sans action, Entrée et lien de l'adresse dans la grille d'un
    réseau, aide du trunk, `queryClient` pris de `useHosted` partout.
  - reste : le compte des interfaces trouvées est redit à chaque page lue ; une recherche
    relancée avec les mêmes critères ne redit rien ; un MAC ou un VID mal formé est dit par
    l'`ErrorState` de la vue (code `machines.mac.invalid`), pas sur son champ, et le VID
    n'est pas vérifié avant l'envoi ; le VLAN d'accès actuel absent de la première page
    n'est pas proposé ; `arrival.byLink` reste levé si le lien suivi ne remonte pas la
    page (hérité de la 18cb) ; titre `h1` de la fiche générique jusqu'à la lecture ;
    « inconnue à cette date » (404) dite comme une erreur avec « Réessayer » ; le tiret
    « aucun » est écrit à cinq endroits ; `dom.ts` garde des fonctions mortes (`textField`,
    `selectField`, `option`, `fill`, `checkbox`, `wrapperOf`, `ruled`) et `styles.css`
    des règles de l'ancienne recherche, à retirer avec eux à la 18cd. À arbitrer côté API :
    l'onglet d'un réseau lit les adresses machine par machine (N+1 lectures : une route
    des adresses d'un réseau avec leur interface les ferait en une) ; la résolution de
    l'adresse et du VID est une lecture ponctuelle (`fetchInfiniteQuery`), pas observée :
    une écriture ou un événement ne la relance pas, seules les listes d'interfaces le sont
    (un filtre `address_value`/`vid` de `/v1/interfaces` la supprimerait).
  - passe de corrections (revue de branche) : onglet d'un réseau, adresses de chaque
    machine lues quatre à la fois (`limited`, `enabled` donné à l'observateur quand la
    machine a sa place, rendue à sa réponse ; places rendues au démontage et à d'autres
    dates), page suivante de machines seulement une fois les adresses des pages lues
    répondues ; recherche, adresse et VID résolus sur trois pages au plus
    (`RESOLVE_PAGES`), au plus douze listes de `/v1/interfaces` (`MAX_LISTS`), le reste
    dit par un avis « Résultat incomplet » et dans la phrase de la région `status`
    (`search.countPartial`), comme les listes plafonnées à `MAX_PAGES` ; `observeAll` de
    `data.ts` : états et observateurs alignés sur les options (doublons : le même
    observateur aux deux places), `setOptions` transmis à un observateur gardé, `Set`
    plutôt que `includes` ; `onPop` de la recherche ignore un autre chemin. Vitest : 42
    (adresses lues au plus quatre à la fois, page 2 après les six adresses de la page 1,
    chaque machine lue une fois, observateurs libérés au démontage ; objets au-delà des
    pages résolues et couples au-delà de la borne dits, compte « incomplet »).
- [x] 18cd. Vue « Hébergement et clusters » en `TreeGrid` à parent déclaré
  (`parentMode: "declared"`) : clusters et machines hôtes, machines hébergées chargées
  sous eux ; fiche d'un cluster ; déplacement par glisser, Ctrl+X puis Ctrl+V et
  « Déplacer vers… », par `canMove` et `draft.move` ; `moveState: "pending"` jusqu'au
  verdict, aucun `setQueryData` ; avertissements dans une région `status` présente dès
  l'origine ; refus dit par son code ; `dom.ts`, `element.ts`, `styles.css` retirés s'ils
  ne servent plus ; `web/v2/PARITE.md` et le catalogue du design à jour — verify :
  parcours `moves` (déplacement au clavier vers un cluster, cycle refusé, avertissement
  `machines.host.site_mismatch`, aucun changeset) et `machines` dans les deux
  navigateurs ; contrôle final du poids ; revues
  - où : `src/views.ts` (`ClustersView`) ; `web/v2/ui/src/components/TreeGrid/` (en
    lecture) ; design 021 § « Déplacements dans les arbres » et § « Catalogue » (plugin
    Machines, « Hébergement et clusters »)
  - dépend de : 18ca, 18cb
  - fait le 2026-11-02 — `machines-clusters` = `host.element(ClustersView.vue)` : arbre
    (`HostingTree.vue`) ou fiche (`ClusterRecord.vue`) selon le chemin. Arbre : `TreeGrid`
    à parent déclaré au présent (`derived` à une autre date, dit), au premier niveau les
    clusters puis les machines sans hébergeur (pages de `/v1/machines` lues jusqu'à ce
    qu'une racine nouvelle paraisse, au plus `MAX_PAGES`, le reste dit), sous un cluster
    ou une machine qui peut héberger (physique, virtuelle : M5) ses hébergées
    (`/v1/machines?host=`), lues à l'ouverture, quatre listes au plus à la fois
    (`limited`), jamais relues seules après un refus (`RetryAlert`) ; colonnes nature et
    site ; Entrée ouvre la fiche (focus au titre). Déplacement par glisser, Ctrl+X puis
    Ctrl+V et « Déplacer vers… » (`openMove` du nœud sélectionné ; refusé avec sa raison
    sans sélection, sur un cluster, sans `machines.edit`), chaque cible jugée par `canMove`
    du SDK (cycle par les hébergeurs lus, type, droits, présent ; hébergeur actuel refusé)
    avec le code de la règle ; au brouillon par `draft.move` (`machines.update_machine`) :
    `moveState: "pending"` jusqu'au verdict, puis nœud « en brouillon » à sa place tant
    que ses lectures l'y montrent, ou refusé avec la règle de la simulation ; aucun
    `setQueryData` des clés de l'arbre. Région `status` présente dès l'origine : issue du
    déplacement et avertissements avec leur code (`machines.host.site_mismatch`).
    Fiche : propriétaire et site en `dl`, membres et hébergées en `DataGrid` (lien,
    nature, lien au cluster). `data.ts` : `next(i)`, `retry(i)` de `useLists` (design 021
    § « API des plugins »). Retirés : `src/views.ts`, `src/element.ts`,
    `public/styles.css` ; `dom.ts` réduit à ce que servent les éléments Vue (`defined`, `t`,
    `hrefOf`, `arrival`, `followLink`, `PAGES`, `pageId`). Poids : `index.js` 22,7 Ko gzip
    -9 (20,8 avant), aucune feuille. Vitest : 49 (`test/clusters-view.test.ts`, 8 :
    premier niveau et hébergées à l'ouverture, Entrée ; premier niveau refusé et vide ;
    liste d'hébergées refusée lue une fois puis par « Réessayer » ; cycle refusé sans
    envoi ; vers un cluster, en attente puis au brouillon, avertissement dans la région,
    aucune clé de l'arbre écrite, aucun changeset ; refus de la simulation avec sa règle ;
    sans droit, autre date ; fiche et son refus). Parcours `moves` (deux tests de l'arbre :
    vers un cluster au clavier, aucun changeset ; cycle refusé puis avertissement d'un
    autre site) et `machines` (arbre depuis la navigation, Entrée vers la fiche, grille
    `Machines du cluster`) : 21 sur 21 dans les deux navigateurs.
  - choix : racines = clusters et machines sans hébergeur (un membre physique d'un cluster
    reste à la racine : il n'est pas hébergé) ; « en attente » dit par la ligne et le
    « demandé » de l'arbre, pas une troisième fois par la vue ; le refus annoncé par
    l'arbre, la vue ajoutant seulement que l'opération reste au brouillon ; site lu par
    `Names` dans une cellule texte (« Chargement… » le temps de la lecture) ; clés
    `hosting.*` ; clés mortes retirées (`list.more`, `error`, `required`,
    `clusters.title`, `intro`, `slug`, `failed`, `none`, `noneText`).
  - relectures : `ui-reviewer` et `a11y-auditor` sur le code (sans rendu : serveur et base
    partagés), aucun bloquant ; corrigés : messages d'un déplacement précédent effacés au
    suivant et à une autre date, échec d'envoi dit dans la région, région `status` sortie
    des états de la vue, phrase « en attente » en trop, troncature des hébergées dite,
    lecture datée dite, « Chargement… » au lieu de « … », Entrée et sélection dans
    l'introduction, raisons du cluster harmonisées.
  - passe de corrections (revue de branche) : places de `limited` rendues à la réponse,
    le nœud ouvert ou refermé (sa liste suivie jusque-là), nœud refermé avant son tour
    jamais admis ; cible d'une nature qui n'héberge rien refusée par le client
    (`MoveSubject.kind`, `PARENT_KINDS` du SDK : `container`, `appliance` →
    `machines.host.invalid`, aussi dans la section « Hébergement » de la fiche) ;
    `draft.onActive` au SDK (design 021 § « API des plugins »), les marques « en
    brouillon » effacées quand le brouillon actif change ou disparaît ; un seul `watch` de
    la date ; un `RetryAlert` par liste d'hébergées refusée, nommée par son hôte
    (`retry(i)`) ; `loading-children` sur un hôte ouvert pas encore admis. Vitest : 51 du
    plugin (refermé avant la réponse, cible `container`, brouillon quitté) ; SDK
    (`PARENT_KINDS`, `draft.onActive`).
  - reste : focus après « Déplacer vers… » rendu au bouton (comportement du composant), loin de la
    ligne « en attente » ; nature d'un cluster en texte (cellules de `TreeGrid` en
    chaînes, pas de `KeyLabel`) ; la grille de la fiche porte le nom de son titre. À
    arbitrer côté API : le premier niveau lit toutes les machines pour garder celles sans
    hébergeur (un filtre « sans hébergeur » de `/v1/machines` le ferait en une page ; fait
    par la 18f, A28).

- [x] 18e. `GET /v1/interfaces?network=<id>` (A28) : interfaces d'un réseau avec leurs
  adresses, paginées, sous le filtre de lecture ; l'onglet « Interfaces » d'un réseau
  du plugin la lit en une liste au lieu d'une lecture par machine — verify :
  `k-check -p kartenn-machines` (interfaces d'un réseau, pagination, lecture refusée
  hors droits) ; Vitest et parcours `machines` (onglet d'un réseau) ; contrat régénéré
  - où : `crates/kartenn-machines/src/rest.rs` (routes des interfaces, forme des
    filtres de `/v1/machines?network=`), `web/v2/plugins/machines/src/InterfacesTab.vue`
  - dépend de : 18cc
  - fait le 2026-11-03 — `list_interfaces` (`crates/kartenn-machines/src/rest.rs`) gagne le
    filtre `network`, de la forme de ceux de `/v1/machines` (inconnu ou répété refusé,
    identifiant) : les interfaces qui tiennent une adresse du réseau que l'appelant lit
    (`in_network`, jointures `LATERAL` depuis le réseau), chacune avec ces adresses seules,
    par valeur, dans `addresses` (`MachineCardAddress` : `id`, `value`, `primary`),
    calculées dans la requête de la page, sur ses seules lignes, après le `LIMIT`
    (`network_addresses`) ; page et versions dans un instantané ; les adresses servies
    journalisées en lecture. `addresses` absent des autres lectures d'interfaces. Contrat et
    schémas des deux SDK régénérés ; design 009 § Routes. Plugin : `InterfacesTab.vue` lit
    une liste `/v1/interfaces` pour les trois objets (`network`, `vlan`, `address`), états
    d'erreur, de vide et de reprise communs, machine citée par `CitedName` ; retirés : la
    lecture du préfixe, les lectures d'adresses par machine et leurs places au limiteur
    (`limited` reste à `MachineColumn` et `Names`), `within` et son test, le type
    `MachineAddress`. Tests : `rest::interfaces_of_a_network_with_their_addresses`
    (pagination par 2, adresse hors du réseau et interface sans adresse absentes, interface
    d'une autre entité lue du seul administrateur, sans lecture des interfaces ou des
    adresses : rien, interfaces lues mais adresses lues ailleurs : rien, filtre répété ou
    non identifiant : 400) ; Vitest 41 (une liste par le réseau, rien d'autre lu, page
    suivante, chaque machine nommée une fois, observateurs libérés) ; parcours `machines`
    6 sur 6 dans les deux navigateurs.
  - choix : seules les adresses du réseau sont servies, pas toutes celles de l'interface ;
    une interface dont aucune adresse du réseau n'est lisible n'est pas listée (chaque
    interface servie a donc au moins une ligne dans l'onglet) ; `useReads` reste dans `data.ts`
    (API du design 021 § « API des plugins »), sans usage dans le plugin ; nom de machine
    par `CitedName`, squelette de cellule pendant sa lecture comme dans `InterfaceGrid`.
  - corrections de la revue (ADR 0021) : un réseau que l'appelant ne lit pas répond comme un
    réseau inconnu, sur `/v1/interfaces?network=` comme sur `/v1/machines?network=`
    (`readable_network`, prédicat évalué une fois sur la ligne du réseau) ;
    `/v1/machines?network=` ne compte plus une machine que par les adresses du réseau que l'appelant lit
    (il les comptait toutes) : pour ses appelants (colonne et fiches du plugin, parcours),
    rien ne change sous les droits du jeu de données, un lecteur sans `core.address.read`
    ou `core.prefix.read` sur le réseau n'y trouve plus rien. `held_in_network` (commun aux
    deux) garde le prédicat des adresses au-dessus de la jointure des rattachements
    (`OFFSET 0`) : évalué sur les seules adresses rattachées. Tests :
    `rest::a_list_by_network_serves_only_what_the_caller_reads` (adresse lue et non lue
    d'une même interface par une portée de préfixe plus étroite que le réseau, écrite en
    base ; le test échoue sans le prédicat des adresses ; réseau illisible = réseau inconnu
    sur les deux listes), lecture journalisée des adresses servies ; Vitest du plugin 52
    (erreur, « Réessayer » et vide de l'onglet d'un réseau). `EXPLAIN (ANALYZE, BUFFERS)` de la
    requête de page (première page de 100, base jetable, test temporaire non gardé) : réseau
    10.1.0.0/16 de 20 000 adresses dont 10 000 rattachées à autant d'interfaces, lecteur
    d'entité (propriétaire résolu de chaque adresse) : plan générique 231 ms, 213 309
    tampons, personnalisé 1 164 ms (première exécution) ; lecteur global : 100 ms, 88 700
    tampons, dans les deux plans. Avant `OFFSET 0`, à 5 000 adresses dont 2 500 rattachées :
    605 ms (prédicat sur les 5 000), 318 ms personnalisé et 57 ms générique après. Chaque
    page relit l'appartenance de tout le réseau (sous-plan haché) : coût linéaire en
    adresses rattachées du réseau, à reprendre si un banc pleine échelle le demande.
- [x] 18f. Filtre « sans hôte » sur `/v1/machines` (A28) : un paramètre qui ne garde que
  les machines sans hôte ; le premier niveau de l'arbre de l'hébergement se lit en une
  liste paginée au lieu de toutes les machines — verify : `k-check -p kartenn-machines` ;
  Vitest et parcours `moves` (arbre de l'hébergement) ; contrat régénéré
  - où : `crates/kartenn-machines/src/rest.rs` (filtres de `/v1/machines`),
    `web/v2/plugins/machines/src/HostingTree.vue`
  - dépend de : 18cd
  - fait le 2026-11-03 — `GET`/`QUERY /v1/machines?host:exists=false` (sans hôte, un membre
    de cluster compris : l'appartenance n'est pas l'hébergement), `true` (hébergée par une
    machine ou un cluster), autre valeur → 400 ; en conjonction avec les autres filtres,
    gardé par le curseur ; un `QUERY` le dit `{"field": "host", "op": "exists", "value":
    false}`. Contrat : paramètre `host:exists` de schéma booléen (`filters.ts`,
    `schema.d.ts` régénérés). Index partiel `machine_unhosted (name, id) WHERE
    host_machine_id IS NULL AND host_cluster_id IS NULL` (migration `machines` 0005) ; la
    condition est écrite dans le texte de la requête (trois instructions selon la valeur),
    non liée : `$n IS NULL OR …` laisserait le prédicat de l'index non prouvé. `EXPLAIN`
    en réduit (100 000 machines, 5 475 sans hôte, base jetable) : plan de l'instruction
    préparée (personnalisé, gardé en mode `auto` après 7 exécutions, le générique estimé
    591 contre 15,6) `Index Scan using machine_unhosted`, 53 tampons, 0,06 ms ; sans
    l'index, `machine_by_name` et 11 460 lignes écartées (11 615 tampons, 20 ms) ; plan
    générique forcé : `BitmapAnd` de `machine_pkey` et `machine_unhosted`, tri des 5 475,
    pas de parcours de la table (547 blocs sur 1 799). Plugin : premier niveau lu par
    `{"host:exists": false}`, une page à la demande de l'arbre (`onLoad`) ; retirés la
    lecture des pages jusqu'à une racine nouvelle (`wanted`), le filtre client `!m.host`
    et les hôtes tirés de la liste de toutes les machines (`up` vient des hébergées
    lues). `k-check -p kartenn-machines` : 64 (`machines_filtered_by_their_host_or_none`) ;
    Vitest du plugin : 52 (premier niveau page à page par Entrée sur la ligne « plus »,
    aucune page lue d'avance) ; parcours `moves` : 15 sur 15 dans les deux navigateurs ;
    `k-web2-check --base main` : 837 Vitest, 2 707 captures sur 2 707 (tout : `filters.ts`
    régénéré est lu par l'entrée du shell) ; `k-check` : 1 265.
  - relectures : `ui-reviewer` et `a11y-auditor` sur le code, aucun bloquant ni important ;
    corrigé : le test passe la page suivante au clavier (Entrée) et non au clic.
  - reste : une page de machines qui n'ajoute aucune ligne mais garde un curseur n'est ni
    redemandée d'elle-même ni annoncée (`treePages.ts`, commun à tous les niveaux) ; rare
    avec `host:exists=false`.
  - choix : forme `host:exists=true|false`, la grammaire des listes du noyau (spec 006,
    design « Filtres » : `{champ}:exists`), plutôt que `host=none` (un identifiant
    surchargé d'une sentinelle) ou un booléen nouveau ; `true` servi aussi (la grammaire
    est à deux valeurs) ; `host=<id>&host:exists=false` rend une liste vide (conjonction),
    pas un 400 ; plafond `MAX_PAGES` du cache gardé pour toutes les listes de l'arbre (une
    page de plus ferait tomber la première), dit par `hosting.partial`.

- [x] 18g. Plugin Machines sur `Heading`, `AttributeList` et les états d'arbre (020-29) :
  fiches (`MachineRecord`, `InterfaceRecord`, `ClusterRecord`) en `AttributeList` et
  `Heading` au lieu de `Grid as="dl"` et de `h1`/`h2` nus ; `HostingTree` en
  `moved`/`preview` au lieu de `draft` — verify : Vitest du plugin ; parcours
  `machines` et `moves` sous les deux navigateurs ; poids du plugin
  - où : `web/v2/plugins/machines/src/{MachineRecord,InterfaceRecord,ClusterRecord,HostingTree}.vue`
  - dépend de : 18cd, 020-29
  - étendue le 2026-11-03 (A33) : l'ajout d'une interface et son mode 802.1Q depuis la
    fiche de l'interface prouvés par le parcours `machines` (lignes de la liste de parité)
  - fait le 2026-11-03 — Fiches `MachineRecord`, `InterfaceRecord`, `ClusterRecord` en
    `AttributeList` (valeur par l'emplacement nommé d'après sa clé ; absente : `CitedName`
    sous `v-if`, MAC `undefined`, la liste dit « non renseigné ») et `Heading` (`h1` à
    `tabindex="-1"`, `h2` des sections) ; `Heading` aussi pour `HostingTree`, `MachineList`
    et `InterfaceSearch` : plus aucun `h1`/`h2` nu ni `dl` brut dans le plugin. Cellules de
    grille sans valeur (MAC, adresses, relation au cluster, colonne des adresses, nom cité
    absent) : `NotSet.vue`, seul endroit du plugin qui écrit le tiret, clé `notSet` (« non
    renseigné », comme la liste) au lieu de `none` (« aucun ») ; les cinq copies relevées
    par la 18cc retirées. `HostingTree` : `draft` → `moved` sur la ligne à sa place ; ligne
    `preview:<id>` (`preview`, mêmes libellé et cellules, sans enfants) sous le nouvel hôte
    si ses hébergées sont lues (premier niveau, hôte ouvert), sinon `moved` seul et la
    région `status` le dit (« Son aperçu se montre sous {host} une fois ouvert. ») ; plus
    d'aperçu ni de marque dès que les listes relues la montrent ailleurs ou que le
    brouillon actif change ; sur l'aperçu, Entrée ouvre la fiche de la machine, le glisser
    et la cible refusés avec leur motif, « Déplacer vers… » refusé, la sélection effacée
    quand l'aperçu disparaît. Vitest du plugin : 53 (aperçu sous un hôte ouvert, deux
    lignes distinguées par leur état, Entrée, refus de « Déplacer vers… », brouillon
    quitté ; cluster fermé : `moved` seul, dit) ; parcours `machines` (nouveau test « an
    interface and its 802.1Q mode in the draft, from the records » : `eth1` et sa MAC
    depuis la fiche de la machine, nom requis ; `eth0` en aucun puis en accès sur le VLAN
    du jeu de données depuis sa fiche ; trois opérations au brouillon, aucun changeset) et
    `moves` (aperçu sous le cluster ouvert, sous l'hôte ouvert) : 23 sur 23 dans les deux
    navigateurs ; `k-web2-check --base main` : 852 Vitest, ciblé : 0 captures sur 2 715.
    Poids : `index.js` 22,9 Ko gzip -9 (22,7 avant), aucune feuille émise. `PARITE.md` :
    ajout d'une interface, mode 802.1Q, parcours `machines` (deux lignes) cochés.
  - relectures : `ui-reviewer` et `a11y-auditor` sur le code (sans rendu : serveur et base
    partagés), aucun bloquant ; corrigés : sélection d'un aperçu disparu effacée ; aperçu
    non montré dit dans la région `status`.
  - reste : le site absent d'une ligne de l'arbre s'écrit « non renseigné » en clair (les
    cellules de `TreeGrid` sont des chaînes, pas de tiret doublé) ; `NotSet.vue` et la clé
    `notSet` doublent `AttributeList` et `valueNone` de la bibliothèque, faute de cellule
    absente dans `DataGrid` (à exporter de `@kartenn/ui`, puis retirer la copie) ; la ligne
    `moved` ne nomme pas son nouvel hôte (dit seulement par la région `status` ; `detail`
    n'est pas rendu par `TreeGrid`) ; l'aperçu d'un hôte déplacé n'a pas ses hébergées ; la
    valeur absente d'un attribut dit « non renseigné » là où l'ancien disait « aucun »
    (hébergeur : « non hébergée » serait plus juste).
  - choix : valeur absente dans une grille par un composant du plugin (`NotSet`) plutôt
    que du texte nu ; identifiant `preview:<id>` de la ligne d'aperçu (celui du catalogue
    de `TreeGrid`) ; aperçu en fin des hébergées lues ; tests du plugin sur la marque
    `data-empty` de la liste (sa phrase suit la langue de la bibliothèque, que le shell
    règle) ; `Heading` étendu à `MachineList` et `InterfaceSearch` (hors `où :`, trois
    lignes) ; titre court du test du parcours, cité tel quel par la liste de parité.

## P4 — Budgets et clôture

- [x] 24e. Enfants d'un nœud jusqu'à 10⁴ lignes (A34) : les lectures `roots` et `children`
  gardent jusqu'à 100 pages dans le cache (les autres listes restent à `MAX_PAGES`) ; la
  lecture de l'arbre (`treePages.ts`) redemande la page suivante tant qu'il en reste et que
  le plafond n'est pas atteint (aujourd'hui elle s'arrête quand le compte d'enfants ne
  change plus, vers 10³) ; au-delà du plafond, un avis sous le nœud renvoie vers la liste
  filtrée « dans ce préfixe » — verify : Vitest (11ᵉ page puis 101ᵉ : lue, puis avis) ;
  parcours `tree` (un conteneur de plus de 10³ enfants défilé jusqu'au dernier lu) ; tas
  sous 64 Mo au banc (22a)
  - où : `web/v2/sdk/src/data/cache.ts` (`MAX_PAGES`, `maxPages`), `treePages.ts` du shell,
    `web/v2/shell/src/views/TopologyView.vue` (avis) ; jeu de données `core.wide-children`
  - dépend de : 22
  - fait : `CHILDREN_PAGES` = 100 (`web/v2/sdk/src/data/cache.ts`, exporté) : `subPages`
    de `children` et `roots` le prend pour `maxPages`, toute autre liste reste à
    `MAX_PAGES` ; l'appelant ne demande rien, le plafond suit la sous-ressource (l'arbre et
    l'onglet Contenu lisent la même requête, qui garderait sinon le plafond du dernier
    lecteur). Cause de l'arrêt vers 10³ : la onzième page faisait tomber la première du
    cache, le compte d'enfants restait égal et `treePages.ts` (inchangé, `web/v2/ui`) ne
    redemandait plus. `levelOfPages` (`topology.ts`) : page suivante tant qu'il en reste
    et que moins de `CHILDREN_PAGES` sont gardées, au-delà niveau `capped` ; `nodesOf`
    ajoute alors la ligne d'avis (`capped:<parent>`) sous le dernier enfant ;
    `TopologyView.vue` : `onLoad` ne lit que dans le plafond, l'avis (« L'arbre s'arrête à
    10 000 nœuds : Entrée ouvre la liste des préfixes dans … », adresses pour un réseau,
    les préfixes du domaine pour le premier niveau) ouvre au clic, par Espace ou Entrée la
    liste filtrée par `within` (`domain` au premier niveau) ; l'aperçu du brouillon ne le
    recopie pas sous un nœud déplacé (`overlay`). Vitest : `sdk/test/cache.test.ts`
    (100 pages pour `children`/`roots`, 10 ailleurs, onzième page lue, première gardée),
    `shell/test/topology.test.ts` (onzième page lue, 100ᵉ atteinte, 101ᵉ jamais
    demandée, avis sous le nœud et au premier niveau, avis laissé à la place au présent
    par l'aperçu) ; parcours `tree` : conteneur de
    1 100 enfants (`DATA.wide`) lu jusqu'au dernier, sans ligne « suite », le premier
    toujours là, sous les deux navigateurs ; `moves` inchangé et vert. Arrêt à 2–6 pages
    du banc non reproduit : la boucle du banc sur 5 000 enfants (essai jetable dans les
    parcours, deux navigateurs) lit les 50 pages ; `more: false` du banc n'en est pas un
    signe (la page arrive dans les 300 ms d'attente et repousse la ligne « suite » hors
    de la fenêtre) ; à reprendre au banc (22a).
  - choix : avis en ligne de l'arbre (`TreeGrid` n'a pas de fente, `web/v2/ui` hors
    périmètre), icône `info`, la consigne dans son libellé (`TreeGrid` ne rend pas
    `detail`) ; le clic et Espace y mènent aussi à la liste (comme le choix d'un nœud) ; `within` liste tous les descendants (pas seulement les
    enfants directs) et une seule sorte d'objets : pas de filtre « enfants directs » dans
    le contrat ; le défilement du parcours par `scrollTop` (à la molette, Firefox n'a pas
    atteint le dernier enfant en 60 s).
  - reste (banc) : tas sous 64 Mo sur 10⁴ lignes, à la passe de la 22a.
  - reste : l'onglet Contenu partage la requête des enfants : défilé au-delà de 10⁴, il
    lirait la 101ᵉ page et ferait tomber la première sous le nœud de l'arbre (rare) ;
    chaque ligne de réseau passée sous le pointeur immobile pendant le défilement lit ses
    enfants sur intention (≈ 100 lectures de `/children` vides pour 50 pages au banc) ;
    l'avis compte dans `aria-setsize` (10 001 au lieu d'inconnu) : il faudrait à `TreeGrid`
    une marque `capped` qui l'en sorte (020) ; sous un réseau plafonné, l'avis mène aux
    adresses, pas aux plages.
  - repris le 2026-11-03 par la 7a : les lectures d'enfants au survol pendant un défilement
- [x] 13d. Provenance de module dans l'explication d'une valeur (relevé de la 019-7d1) :
  `ExplanationBody.vue` dit « direct » pour toute valeur posée sur l'objet, quelle que soit
  sa provenance ; une valeur dérivée par un module (`module:kube#label:<clé>`, amendement
  14 de l'ADR 0026) se lit alors comme une saisie. L'explication la dit « tirée du label
  `<clé>` (module kube) », la forme `source:` gardée — REQ-003-14 — verify : Vitest de
  l'explication (trois formes : `input`, `source:`, `module:`) ; capture de la bulle mise
  à jour si le rendu change
  - où : `web/v2/shell/src/views/record/ExplanationBody.vue` (ou voisin), catalogues
    `views/i18n/{fr,en}.json` ; `specs/006-api-rest/openapi-v1.json` (`Explanation.provenance`)
  - dépend de : 019-7d1
  - fait le 2026-11-12 : `views/record/provenance.ts` (`provenanceOf`) lit les trois formes
    (`input`, `source:<…>`, `module:<module>#<motif>`, plus une forme inconnue gardée
    telle quelle) ; `ExplanationBody.vue` dit une valeur dérivée à la place de « Posée
    directement » : « Tirée par le module kube du label `<clé>` » (`fromLabel`), et pour
    un motif inconnu, de kube ou d'un autre module, « Dérivée par le module `<m>` :
    `<motif>` » (`byModule`), le motif brut ; héritée, la ligne « Héritée de » reste
    au-dessus ; l'origine retenue, la provenance d'un module n'est plus dite « Inférée
    par la règle ». `input` et `source:` gardent leur rendu. Clé et motif en `CodeBadge`
    copiable. Vitest : `shell/test/explanation.test.ts` (`provenanceOf` sur les trois
    formes et les formes mal faites ; l'explication montée : saisie, source, label de
    kube, motif d'un autre module, héritée et dérivée, origine retenue). Aucune capture ne
    montre la bulle réelle, et les rendus déjà capturés (maquette E2) ne changent pas.
  - choix : une phrase par clé, le `CodeBadge` en fin de phrase (relectures : ordre des
    mots traduisible, bouton « Copier » qui ne coupe pas la phrase) ; seul `kube` a un
    motif connu (`label:`), un `label:` d'un autre module reste brut ; le nom du module en
    texte courant.
- [ ] 24f. Ctrl+Entrée sur un dépôt dans un arbre de plugin (REQ-021-32, reste de la 24,
  arbitré par l'utilisateur le 2026-11-11) : un déplacement dans l'arbre de l'hébergement
  des machines peut devenir un changeset immédiat par Ctrl+Entrée, l'écriture passant par
  le shell pour le compte du plugin (motif demandé, `?approval=self`, sans optimisme : « en
  attente » à sa place, puis relu). Point nouveau du contrat du SDK des plugins et
  exception à W10 (un plugin n'envoie pas de changeset) : la tâche commence par une
  **spec** — amendement de la 021 (W10, REQ-021-32, contrat `draft` et écriture du SDK des
  plugins ; ADR 0038 si elle est touchée) — validée par l'utilisateur avant tout code
  — verify : spec validée consignée (amendement et date d'arbitrage dans la tâche) ; puis
  Vitest du geste (Ctrl+Entrée sur un dépôt accepté : une écriture par le shell, motif,
  « en attente » sans `setQueryData`, refus 422/403/412 avec code ; dépôt refusé par
  `canMove` : rien envoyé ; Entrée seule : brouillon, comme aujourd'hui) ; parcours
  `moves` (Ctrl+Entrée sur l'arbre de l'hébergement : changeset appliqué, aucun brouillon,
  nœud relu sous son nouvel hôte), `accessibility.spec.ts -g "every journey runs axe"`
  - où : SDK `web/v2/sdk/src/` (hôte des plugins, `draft.ts` : `moveInDraft`, et
    l'écriture qui le doublerait) ; plugin `web/v2/plugins/machines/src/HostingTree.vue`
    (`onMove`, intention `MoveIntent` du `TreeGrid`, qui n'a pas de `now`) ; shell
    `web/v2/shell/src/views/edit/moves.ts` (`useMoves`, branche `changesets.submit`, forme
    à reprendre) ; design 021 W10 (`specs/021-shell-v2/design.md`, tableau des amendements
    vers la ligne 61) ; ADR 0025 (plugins web), ADR 0038 (partage des composants avec les
    plugins)
  - dépend de : 24, 18c ; arbitrage de la spec par l'utilisateur
- [x] 24g. Plage remplacée au brouillon marquée « en brouillon » par la Topographie
  (REQ-021-17, clause « valeur modifiée par le brouillon » ; reste de la 24) : une plage
  dont le brouillon actif porte le remplacement (`close` de la plage, puis `create_range`
  de mêmes usage et domaine : le contrat n'a pas de bornes dans `update_range`) est
  marquée « en brouillon » à sa place, comme un `update_*` ; la nouvelle plage reste
  montrée par l'aperçu tiré des `placements` (24b) — verify : Vitest (`draftedValues` ou
  son voisin : un `close` de plage au brouillon marque la plage, un `close` d'un autre
  objet ou d'une autre plage ne la marque pas ; `inDraft` de la Topographie) ; parcours
  `moves` (plage remplacée au brouillon : ligne marquée « (en brouillon) » à sa place,
  arbre au présent inchangé, puis brouillon appliqué, arbre relu), `accessibility.spec.ts
  -g "every journey runs axe"`
  - où : `web/v2/shell/src/views/edit/fields.ts` (`draftedValues`, vers la ligne 349, ne
    lit que les `update_*`) ; `web/v2/shell/src/views/TopologyView.vue` (`inDraft` vers la
    ligne 367, `draftStateOf` de `views/topology.ts` vers la ligne 222) ;
    `web/v2/shell/test/edit.test.ts` (tests de `draftedValues` à imiter) ; parcours
    `web/v2/e2e/journeys/moves.spec.ts` (« a prefix inserted in the draft… », forme)
  - dépend de : 24, 24b
  - fait le 2026-11-11 : `draftedClosures` (`views/edit/fields.ts`, voisin de
    `draftedValues`) : les objets d'une collection qu'un brouillon clôt, par identifiant ;
    `inDraftOf` (`views/topology.ts`) : les objets de l'arbre que le brouillon change, les
    `update_*` des préfixes, plages et adresses plus les plages closes, sorti de
    `TopologyView.vue` (`inDraft` en est le `computed`) pour être testé ; une plage close
    n'est pas placée par la simulation (elle quitte l'arbre), elle reste donc `draft`, la
    plage créée `preview` sous son réseau (24b). Vitest : `edit.test.ts` (plage remplacée
    marquée, préfixe clos lu dans sa collection, plage close sous `within` non lue, une
    clôture ne change aucune valeur) ; `topology.test.ts` (plage remplacée `draft` à sa
    place, la créée `preview` sous P avec ses bornes, une autre plage et un préfixe clos sans
    état). Parcours `moves` « a range replaced in the draft is marked at its place, the new
    one shown by the preview » (chromium et firefox : brouillon des deux opérations écrit
    par l'API et rendu actif, faute de geste ; ligne servie « (en brouillon) », ligne
    d'aperçu « (aperçu du brouillon) » au niveau 2, arbre servi inchangé, axe ; appliqué,
    aperçu parti, la nouvelle plage servie à sa place, arbre égal à celui de l'API).
  - choix : toute clôture d'une plage au brouillon la marque, suivie ou non d'un
    `create_range` (le contrat n'apparie pas les deux ; une plage close est changée par le
    brouillon) ; une clôture sous `within` n'est pas lue (une autre période que le présent,
    comme `draftedValues` qui ne lit pas `within`) ; seules les plages : un préfixe ou une
    adresse clos au brouillon ne sont pas marqués (pas de remplacement à dire, et « en
    brouillon » n'est pas le mot d'un retrait) ; brouillon du parcours écrit par l'API (le
    shell n'a pas de geste de remplacement des bornes). Pas de relecture `ui-reviewer` ni
    `a11y-auditor` : aucun rendu nouveau, l'état `draft` du `TreeGrid` déjà relu (24b, 24d)
    s'applique à une ligne de plus.
  - reste : un préfixe ou une adresse clos au brouillon ne sont pas marqués dans la
    Topographie (`TreeGrid` n'a pas d'état « retiré dans le brouillon », que `DataGrid` a,
    `removed`) ; la plage close et sa remplaçante ne sont pas reliées à l'écran (deux lignes,
    deux états, sans lien entre elles). Revue (mineurs) : le `choix :` marque aussi une plage
    seulement retirée « (en brouillon) » tout en écartant les préfixes parce que ce n'est pas
    le mot d'un retrait — justification contradictoire, à reprendre avec l'état « retiré » ;
    un placement `after: null` d'un objet clos ferait passer `moved` avant `draft` (contrat
    de `placements` pour un objet clos non écrit) ; la clé `kartenn.draft.active` écrite en
    dur dans `moves.spec.ts` (aide partagée au prochain usage) ; `OTHER_RANGE` n'est jamais
    clos dans `topology.test.ts`.
- [x] 7a. Préchargement au survol sans lecture pendant un défilement (relevé par la revue
  de la 24e) : `childrenIntent` (`web/v2/shell/src/prefetch/tree.ts` vers la ligne 30) lit
  les enfants de chaque ligne qui passe sous le pointeur immobile pendant un défilement
  (une centaine de lectures `/children` vides pour 50 pages) ; l'intention ne naît que d'un
  mouvement du pointeur ou d'un focus, jamais d'une ligne qui défile sous lui. Le budget
  « Arbre de 10⁵ » exige aucune lecture de `/children` pendant la fenêtre — verify :
  Vitest (défilement sous un pointeur immobile : aucune lecture ; survol réel de 60 ms :
  une lecture) ; compte des lectures du banc à 0 (22a). Au passage : `restOf`
  (`TopologyView.vue` vers la ligne 282) ne renvoie jamais une liste sans filtre quand le
  parent manque, et la destination de l'avis du plafond est prouvée (Vitest : préfixes
  `within` sous un conteneur, adresses `within` sous un réseau, `domain` au premier niveau)
  - où : `web/v2/shell/src/prefetch/tree.ts`, `web/v2/shell/src/views/TopologyView.vue`,
    `web/v2/shell/test/prefetch.test.ts`
  - dépend de : 7, 24e
  - fait : Chromium et Firefox envoient `pointerover` et `pointerenter` à une ligne ou un
    lien qui défile sous un pointeur immobile, jamais `pointermove` (essai mesuré, à
    `scrollTop` et à la molette) ; la même faille tenait les liens. `childrenIntent`
    (`tree.ts`) arme le délai sur `pointermove`, et `pointerover` d'une autre ligne
    sans mouvement met fin au survol en cours ; `prefetch.intent` (`sdk/src/data/
    prefetch.ts`, derrière `KLink`) arme sur le premier `pointermove` du lien, les
    suivants gardent le délai, `pointerleave` le remet à zéro ; 60 ms et focus
    inchangés. `restOf` et `kindsIn` passent dans `topology.ts` (purs) : `null` sans le
    préfixe parent lu (ou sa valeur) ou sans domaine, l'avis dit alors « L'arbre s'arrête
    à N nœuds » sans destination et Entrée n'ouvre rien. Vitest :
    `shell/test/prefetch.test.ts` (lignes défilées sous le pointeur, chacune 120 ms :
    aucune lecture ; survol commencé puis remplacé par défilement : aucune ; mouvement
    sur la ligne venue : une lecture ; lien défilé sous le pointeur : aucune),
    `sdk/test/prefetch.test.ts` (entrée sans mouvement : rien, puis mouvement : lecture ;
    mouvements suivants gardent le délai), `shell/test/topology.test.ts` (destinations
    résolues par les routes du cadre et lectures déclarées : préfixes `within` sous un
    conteneur, adresses `within` sous un réseau, `domain` au premier niveau ; `null`
    sans parent, sans valeur ou sans domaine) ; parcours `tree` : molette sous le
    pointeur posé sur le nœud ouvert, aucune lecture de `/children` d'une ligne passée,
    sous les deux navigateurs. Banc (22a) non lancé.
  - choix : le mouvement se reconnaît à `pointermove` seul, sans comparer les
    coordonnées (aucun navigateur n'en envoie au défilement) ; clé `capped.nowhere` pour
    l'avis sans destination (état de course, parent relu ailleurs) plutôt que pas d'avis.
- [x] 22a. Mesures restantes du banc web : la page de 100 lignes rend son chiffre (lectures
  servies en 304 comptées à part, déjà codé, pas encore passé) ; l'arbre chargé jusqu'au
  bout avant le défilement, une fois la 24e faite, sur 10⁴ lignes ; navigation en cache au
  p75, le pire imprimé (A35) ; le compte des lectures `/children` pendant le défilement de
  l'arbre imprimé et tenu à 0 (7a) ; `.claude/agents/bench-runner.md` à jour (fait par la
  session principale à la fusion de la 22) — verify : une passe réduite où chaque mesure rend
  un chiffre ; la passe complète lancée par la session principale sur machine libre
  - où : `web/v2/e2e/budgets/budgets.spec.ts`, `web/v2/e2e/budgets/global-setup.ts`
  - dépend de : 22, 24e
  - fait le 2026-11-09 — trois causes trouvées au banc, sept passes réduites
    (`bench-web`, `K_BENCH_SCALE` par défaut) :
    - les tâches périodiques du serveur écrivaient pendant le banc : `public-drift`, due à
      chaque démarrage (`app.rs`), réécrit les drapeaux publics des adresses chargées par
      changesets de 10³ objets pendant des minutes ; chacun invalide toutes les lectures
      d'enfants des arbres (X24) et le conteneur ouvert relisait ses 100 pages (jusqu'à 325
      lectures au chargement, 33 à 41 pendant la fenêtre, 80 % de trames perdues).
      `serve.sh budgets` coupe et repousse les tâches (`UPDATE job SET enabled = false,
      due_at = now() + 1 jour`) après le chargement : instance calme, lectures `/children`
      pendant la fenêtre à 0 ;
    - la page de 100 lignes ne rendait aucun chiffre (11 à 19 pages « non dessinées ») :
      passé dix pages, le cache lâche la première (`MAX_PAGES`), les lignes montent d'une
      page sous la fenêtre et la première ligne de la nouvelle page reste au-dessus, jamais
      rendue ; la mesure prend la première de ses lignes dessinée (design, table des
      budgets) ; le défilement vers la fin s'arrête dès qu'une lecture est partie
      (événement `request`), la ressource n'arrivant qu'avec la fin de la réponse ;
    - Flèche bas dans l'arbre s'arrêtait sur l'avis du plafond (la fenêtre finit près de la
      dernière des 10⁴ lignes) : l'arbre remonte en haut avant les 50 appuis ; le tas, qui
      venait après, rend aussi son chiffre.
    Navigation en cache au p75 de dix, le pire imprimé (A35) ; arbre chargé jusqu'au bout
    vérifié (`children loaded before scrolling`, au moins 10⁴ et pas de ligne « suite ») ;
    lectures `/children` pendant la fenêtre comptées (du conteneur, de sa première page, de
    combien de nœuds) ; flux d'événements relevé pendant le chargement et la fenêtre (sonde
    sur `EventSource`). Dernière passe réduite (Chromium, CPU ×4) : shell initial 302,2 Ko
    (FAIL, 300), plugins 3,5 et 22,0 Ko ; premier écran 731 ms, CLS 0,002 ; page de 100
    lignes p95 212 ms (FAIL, 100 ; médiane 129, pire 319), 0 lecture sans lignes ; arbre
    10⁴ enfants en 100 pages, 0 lecture `/children` pendant la fenêtre, trames perdues
    77,3 % (FAIL, 2 %), 137 tâches longues (FAIL, 0 ; la plus longue 151 ms) ; Flèche bas
    arbre p75 42,7 ms, grille 100,0 ms (à la limite) ; tas 23,5 Mo ; navigation en cache
    p75 126,9 ms (FAIL, 100 ; médiane 117, pire 195), CLS 0,001, témoin vu (0,114) ; INP
    p75 : nœud 128, filtre 96, cellule 72, onglet 32, palette 72 ms. `k-web2-check --base
    main` : 942 Vitest ; tout, 2 719 captures sur 2 719 (`budgets.spec.ts` est un fichier du
    projet des captures).
  - choix : instance du banc sans tâches périodiques (elle mesure le navigateur ; le coût
    des invalidations d'arbre reste ouvert ci-dessous) ; page mesurée à sa première ligne
    dessinée plutôt que réduite à dix pages ; sonde du flux d'événements et décompte des
    lectures d'enfants gardés en `INFO` pour la passe complète ; `bench-runner.md` laissé à
    la session principale.
  - reste (passe complète) : la passe pleine échelle, par la session principale sur machine
    libre (`K_BENCH_SCALE=full bench-web`).
  - reste : arrêt du chargement de l'arbre à deux pages, une passe sur sept (après la
    deuxième page, ni ligne « suite » ni avis, 100 enfants seulement dessinés ; vu aussi
    par la 24e) : `treePages.ts` (`web/v2/ui`) ne redemande jamais à la même taille, et
    `onLoad` (`TopologyView.vue`) ne lit rien quand `levelOf(parent).more` est faux à ce
    moment ; le banc l'imprime en FAIL (`aria-setsize` à l'appui). Défilement de l'arbre à
    77 % de trames perdues, la page de 100 lignes à 212 ms et la navigation en cache à
    127 ms au p75 : budgets dépassés, à reprendre en tâches de performance. Passé dix
    pages, une liste saute d'une page sous la fenêtre à chaque page lue (pas de
    compensation du défilement quand le cache lâche la première, `DataGrid`). Deux
    navigations sur dix (enregistrements 6 et 10, à chaque passe) sans lecture de
    `/properties` au survol, malgré deux mouvements dans le lien : cause non trouvée. Le
    shell initial dépasse de 2,2 Ko. Toute modification d'un préfixe, d'une plage ou
    d'une adresse invalidait toutes les lectures d'enfants de tous les arbres (X24,
    `invalidate.ts`) : arbitré le 2026-11-09 (A40), tâche 22c. Ces restes sont découpés
    en 22b à 22h.
  - reste (revue) : commentaire de `web/v2/e2e/serve.sh` (vers la ligne 65) inexact :
    c'est `migrate` (`declare_module_jobs`, `kartenn-store-pg/src/maintenance.rs` vers la
    ligne 84) qui réactive les tâches des modules, et `run` remet `public-drift` à
    échéance (`app.rs` vers la ligne 515), si bien que seul `enabled = false` la tient ;
    `budgets.spec.ts` : `firsts` (vers la ligne 504) compte la lecture initiale (valeur
    normale 1, que l'`INFO` ne dit pas) ; `started` (vers la ligne 394) suit un événement
    côté Node, avec retard, le commentaire promet plus ; les navigations non préchargées
    entrent dans le p75 de la navigation en cache (vers la ligne 647), à exclure ou à
    noter (22g).

- [x] 22b. Passe pleine échelle du banc web, par la session principale, seule sur la
  machine (`k-heavy --status` tout libre) — verify : `K_BENCH_SCALE=full bench-web`,
  chaque mesure rend son chiffre ; chiffres reportés dans la note de la tâche
  - où : `web/v2/e2e/budgets/budgets.spec.ts`, `.claude/agents/bench-runner.md` (à
    remettre à jour si les lignes du verdict ont changé)
  - dépend de : 22a ; de préférence après 22c et 22d
  - fait le 2026-11-10 (session principale, main 34abb5a1, seule sur la machine, 8 min
    20 s) : chaque mesure rend son chiffre. Passent : plugins 3,5 et 22,1 Ko ; premier
    écran 768 ms, CLS 0,002 ; 10⁴ enfants chargés avant le défilement, 0 lecture
    `/children` pendant ; flèche bas dans l'arbre 41,6 ms et dans une grille 48,7 ms au
    p75 ; tas 23,4 Mo ; CLS d'une navigation 0,001 (témoin vu à 0,114) ; INP au p75 :
    nœud 128, filtre 96, cellule 64, onglet 32, palette 72 ms. Échouent : shell initial
    302,6 Ko (22h) ; page de 100 lignes 177 ms au p95, médiane 132, pire 251 (22f) ;
    défilement de l'arbre 76,8 % de trames perdues et 138 tâches longues, la plus longue
    131 ms (22e) ; navigation en cache 131 ms au p75, médiane 125, pire 199, et des
    enregistrements non préchargés au survol (6, 7 et 9) (22g). `bench-runner.md`
    inchangé (il ne cite pas les lignes du verdict).

- [x] 22c. Invalidation des arbres bornée aux domaines (A40) : l'événement `changes` lu
  avec ses `domains` ; `objects` de `createInvalidation` n'invalide plus que les lectures
  `core.domain/roots` et `core.prefix/children` des domaines nommés (clé de lecture à
  relier à son domaine) ; une écriture propre sans domaines dans sa réponse garde
  l'invalidation de tous les arbres ; `machines.machine/hosted` inchangé — verify :
  Vitest `a change in one domain leaves the trees of the others` et `a change without
  domains reads every tree again` ; le banc (passe réduite, tâches périodiques remises)
  tient 0 lecture `/children` pendant le défilement d'un arbre d'un autre domaine
  - où : `web/v2/sdk/src/data/invalidate.ts` (`TREES`, `objects` vers la ligne 70),
    `web/v2/sdk/src/events/apply.ts` (vers la ligne 44), `web/v2/sdk/src/data/write.ts`
    (vers la ligne 172) ; tests voisins `web/v2/sdk/test/events.test.ts`,
    `web/v2/sdk/test/write.test.ts` ; contrat de l'événement : `specs/022-evenements-sse/design.md`
    (vers la ligne 73)
  - dépend de : 22a
  - fait le 2026-11-09 — `invalidate.ts` : `objects(changed, revision, domains)` et
    `types(types, domains)` passent les lectures d'arbre par une même règle
    (`treeRule`) : `roots` d'un domaine nommé, `children` d'un préfixe dont le domaine
    est nommé ; le domaine se déduit du cache sans lecture (`domainsOf` : `roots` du
    domaine, puis enfants de parent en parent, ou `domain` du corps d'un préfixe, d'une
    plage, d'une adresse lus seuls) ; inconnu, la lecture est relue ; `apply.ts` lit
    `domains` pour `changes` et `truncated` ; `write.ts` passe `[]` (réponse sans
    domaines : tous les arbres). Design « Déplacements dans les arbres » mis à jour.
    Vitest `a change in one domain leaves the trees of the others` (avec `truncated`, relu
    sans `If-None-Match`, et une machine qui ne touche que `hosted`), `a change without
    domains reads every tree again` (`domains` vide, absent, écriture propre) et
    `200: every prefix tree read again, the answer naming no domain (A40)`.
    `k-web2-check --base main` : 945 Vitest ; tout, 2 719 captures sur 2 719
    (`invalidate.ts` est chargé par l'entrée du shell).
  - choix : `domains` vide traité comme absent (tous les arbres), un changement de préfixe
    nommant toujours son domaine côté serveur, un vide ne venant que d'un domaine
    invisible ; sous `truncated`, les lectures d'arbre de préfixes ne suivent plus que la
    règle des domaines (avant, seules celles lues par type qualifié l'étaient, toutes
    domaines confondus, et `roots` jamais) ; profondeur de remontée bornée à 129 (/0 à
    /128) contre un cycle du cache.
  - reste : la passe réduite du banc (tâches périodiques remises, 0 lecture `/children`
    pendant le défilement d'un arbre d'un autre domaine) n'est pas lancée par l'agent :
    à la session principale (ou avec 22b). Hors périmètre : `types()` et `objects()`
    comparent le type de la clé tel que lu (`queryKey[1]`) au type qualifié, si bien
    qu'une clé lue par le nom nu (`prefix`, comme la topographie) échappe à
    l'invalidation par type et par identifiant (les lectures d'arbre, qualifiées par
    `treeOf`, n'y échappent pas) : tâche 22i.
  - reste (revue) : `domainsOf` (`invalidate.ts` vers la ligne 75) balaie tout le cache à
    chaque événement borné, même sans lecture d'arbre en cache (O(cache) par changeset) ;
    sous `truncated`, `types: ["core.domain"]` ne relit plus une lecture `roots` de clé
    qualifiée (`TREES` sans entrée `core.domain`, sans effet aujourd'hui, à dire au
    `choix :`) ; le `[]` explicite de `write.ts` (vers la ligne 175) répète le défaut ; le
    tuple typé à la main de `events.test.ts` (vers la ligne 350), plus idiomatique en
    `as const`. Deux propriétés du serveur dont A40 dépend : tâche 22j.

- [x] 22i. Lectures d'objets sous le nom de type nu (relevé par la 22c, antérieur) :
  `types()` et `objects()` de `invalidate.ts` comparent `queryKey[1]` tel quel au type
  qualifié ; une lecture d'objet nue (`TopologyView.vue` vers la ligne 453) n'est relue que
  par `dependents.showing` et perd le court-circuit de révision ; une sous-ressource nue
  (`reads.sub("prefix", id, "properties")`, `NetworkVlan.vue` vers la ligne 45,
  `record/NetworkVlanTab.vue` vers la ligne 64) échappe entièrement sous `truncated` : la
  VLAN affichée d'un réseau peut rester périmée. Qualifier le type des clés à la
  comparaison (comme `treeOf`), ou à la lecture — verify : Vitest `a bare-typed read is
  invalidated by its type and its id` (objet et sous-ressource, `changes` et `truncated`)
  - où : `web/v2/sdk/src/data/invalidate.ts` (`types`, `objects`, `treeOf`, `qualify` de
    `types.ts`) ; test voisin `web/v2/sdk/test/events.test.ts`
  - dépend de : 22c
  - fait : `invalidate.ts`, `typeOf` (le type d'une clé d'objet ou de sous-ressource,
    qualifié comme `treeOf`), comparé par `objects()` à l'identifiant changé (le
    court-circuit `upToDate` inchangé) et par `types()` aux types nommés ; une clé par
    chemin garde sa comparaison au chemin ; la règle des arbres (A40) n'est pas touchée.
    Vitest `a bare-typed read is invalidated by its type and its id` (`events.test.ts` :
    objet et sous-ressource `properties` lus par `prefix`, sous `changes`, objet déjà à la
    révision épargné, et sous `truncated`, un autre type n'en relisant aucun), en échec
    sur l'ancienne comparaison. `k-web2-check --base main` : 950 Vitest ; tout, 2 719
    captures sur 2 719 (`invalidate.ts` est chargé par l'entrée du shell).
  - choix : qualifier à la comparaison plutôt qu'à la lecture (les clés gardent le type
    lu, aucun appelant réécrit) ; sous `changes`, une lecture `children` nue d'un préfixe
    changé suit désormais la règle de l'identifiant, comme la clé qualifiée l'était déjà,
    avant celle des arbres.
  - reste (revue) : `typeOf` (`invalidate.ts` vers la ligne 57) ne vérifie pas
    `key[0] === "v1"`, si bien qu'une clé hors `v1` à deux chaînes
    (`["simulate","draft",id,…]`, `drafts.ts` vers la ligne 348) reçoit le type
    `core.draft` (faux positif latent, sans effet aujourd'hui) ; `id` déstructuré puis
    redéstructuré par `typeOf` (vers les lignes 182 à 184, cast `id as string`) ; le test
    ne vérifie pas que les lectures nues sont relues sans `If-None-Match` sous `truncated`
    (`conditional(...)` du test voisin), ni la relecture du `children` nu d'un préfixe
    changé hors des domaines nommés.

- [x] 22j. Domaines d'un changeset publiés au flux (A40 en dépend, relevé par la revue de
  la 22c) : les `domains` de `changes` sont les domaines de verrouillage
  (`kartenn-service/src/pipeline.rs` vers la ligne 1163, `lock_targets`) ; vérifier
  qu'aucune opération de module n'écrit dans un domaine de préfixes sans le porter dans
  `plan.domains` (vers la ligne 1171) ; vérifier le filtre de visibilité
  (`kartenn-service/src/events.rs` vers la ligne 591) pour un sujet qui voit des préfixes
  d'un domaine sans voir le domaine (ses arbres ne seraient pas relus si un autre domaine
  visible est nommé). Correction si l'un ou l'autre manque ; un changement du contrat de
  la 022 attend un arbitrage — verify : test d'intégration `every written domain is
  named` sur les opérations de module (`kartenn-rest/tests/it`) et un test du filtre
  - où : `crates/kartenn-service/src/pipeline.rs`, `crates/kartenn-service/src/events.rs` ;
    tests voisins des événements dans `crates/kartenn-rest/tests/it/`
  - dépend de : 22c ; arbitrée le 2026-11-10 : **option A** (voir « arbitrage demandé »)
  - fait (2026-11-10) : premier point tenu, prouvé par `every_written_domain_is_named` (`kartenn-rest/tests/it/event_domains.rs` :
    création, modification, attribution, VLAN, restauration, clôture en cascade, sur deux
    domaines ; `lock_targets` est calculé après `cascade::expand` et à l'approbation ;
    aucun module n'écrit d'adressage hors des opérations du noyau). Second point, constaté
    puis corrigé après l'arbitrage (option A) : `render` ne gardait que les domaines que le
    sujet lit lui-même ; un sujet qui lit des préfixes de D1 par une liaison sur un
    sous-arbre, sans lire D1, et qui lit D2, recevait `domains: [D2]` et ne relisait pas
    ses `children` de D1 (A40). Désormais `events::visible` (`kartenn-store-pg`) rend aussi
    le domaine de chaque objet du lot (version jugée), le hub le donne au `Sight`
    (`Sight::domain`), et `render` garde un domaine lu ou porté par un objet remis ;
    REQ-022-16 et le design de la 022 (« Format », « Visibilité par lot ») amendés. Tests :
    `the_domain_of_a_visible_object_is_named_beside_one_seen` (unitaire, remplace
    `a_domain_unseen_is_not_named_beside_one_seen`, inversé) et
    `the_domain_of_a_delivered_object_is_named` (`kartenn-rest/tests/it/events.rs`, par
    pages : sous-arbre de D1 et D2 lus → `[D1, D2]` ; `core.prefix.read` global sans
    domaine → les trois ; D3 non nommé au premier, `domains=D3` ne lui remet rien).
  - choix : le domaine d'un objet vient de la requête de visibilité (une ligne de plus par
    objet porteur, `UNION ALL`, le plan de la jointure sujets × objets inchangé), et la
    requête part aussi quand tous les sujets lisent leurs types partout, pour qu'un sujet
    qui lit les préfixes partout sans lire les domaines les reçoive ; seuls les objets
    remis après les filtres `types` et `objects` portent leur domaine ; le filtre
    `domains` est maintenant appliqué aux domaines nommés (avant : à tous les domaines
    écrits, ce qui disait d'un changeset qu'il touchait un domaine invisible) ; un objet
    changé de domaine ne porte que le domaine de sa version jugée.
  - reste : le banc `bench-events` (budget de la requête de visibilité) n'est pas relancé
    ici (session principale) ; `update_vlan_group` et `policy.inference` : 22n.
  - arbitrage demandé : (A, recommandé) nommer aussi le domaine de tout préfixe, plage ou
    adresse visible même si le sujet ne lit pas le domaine : l'identifiant est déjà exposé
    par `GET /v1/prefixes/{id}` (`domain.id`), rien de neuf ne fuit ; amender REQ-022-16
    (« les domaines suivent la même règle ») et porter le domaine de chaque objet dans la
    requête `visible` du hub (elle joint déjà `carrier_object.domain_id`). (B) garder la
    règle et envoyer `domains: []` dès qu'un objet visible est dans un domaine non lu :
    rien de plus n'est montré, mais cela contredit REQ-022-4 (« les domaines visibles ») et
    perd le gain d'A40 pour ce sujet. Écarté : `[]` dès que le changeset touche un domaine
    non lu, un oracle contraire à REQ-022-16. Hors d'A40 : `update_vlan_group`
    (`kartenn-service/src/vlan.rs` vers la ligne 467) ne nomme que le nouvel ancrage d'un
    groupe déplacé, et une règle `policy.inference`, ancrée dans un domaine
    (`kartenn-policy/src/handler.rs` vers la ligne 584), laisse `plan.domains` vide (ligne
    256, sérialisée par la clé S17) ; ni l'un ni l'autre n'est dans un arbre de préfixes,
    mais les corriger change `domains` et le verrouillage : à décider avec A ou B (tâche à
    part).
  - arbitrage rendu (l'utilisateur, 2026-11-10) : **A**. REQ-022-16 est amendée (le
    domaine d'un objet visible est nommé même si le sujet ne lit pas le domaine), le hub
    porte le domaine de chaque objet visible, le test
    `a_domain_unseen_is_not_named_beside_one_seen` s'inverse. `update_vlan_group` et
    `policy.inference` : tâche à part, 22n.
  - filtre `domains=` resserré aux domaines nommés (choix de l'agent, au-delà de A) :
    gardé par l'utilisateur le 2026-11-10.
  - reste (revue) : la description OpenAPI de `domains` (`kartenn-rest/src/events/mod.rs`
    vers la ligne 214, recopiée dans les deux `schema.d.ts`) dit encore « the addressing
    domains written, those the subscriber may read » ; la requête de visibilité tourne
    pour un sujet qui lit tout, même s'il lit aussi `core.domain` partout (`hub.rs` vers
    la ligne 845) ; `bench-events` ne mesure pas le cas « portées vides, objets présents »,
    nouveau chemin chaud du hub ; seul le chemin par pages a un test d'intégration, pas la
    diffusion en direct (`hub.rs` vers la ligne 1107) ; `module_object.domain_id` est
    générique : à vérifier que la lecture de chaque objet de module expose bien son
    domaine (par exemple le domaine des pods d'un `kube.cluster`), sans quoi le nommer
    serait un oracle.
- [x] 22n. Domaines de verrouillage de `update_vlan_group` et de `policy.inference`
  (arbitrage de la 22j, 2026-11-10) : un groupe de VLAN déplacé ne nomme et ne verrouille
  que son nouvel ancrage, l'ancien domaine doit l'être aussi ; une règle
  `policy.inference` ancrée dans un domaine laisse `plan.domains` vide, elle doit porter
  son domaine d'ancrage (clé de verrou et `domains` de `changes`) — verify : le test
  `every_written_domain_is_named` étendu aux deux cas (deux domaines pour le groupe
  déplacé, le domaine d'ancrage pour la règle) ; les tests de verrou voisins verts
  - où : `crates/kartenn-service/src/vlan.rs` (`update_vlan_group`, vers la ligne 467),
    `crates/kartenn-policy/src/handler.rs` (vers les lignes 256 et 584),
    `crates/kartenn-rest/tests/it/event_domains.rs`
  - dépend de : 22j
  - fait (2026-11-10) : `Completed` (`kartenn-service/src/vlan.rs`) porte `anchor_domain`,
    le domaine d'ancrage du groupe avant la modification, que `prepare_group`
    (`kartenn-store-pg/src/vlan.rs`) lit avec son ancrage ; `plan_update_group` porte
    dans `plan.domains` l'ancien et le nouveau domaine, triés. Une règle d'inférence : la
    création portait déjà son domaine ; la modification (qui ne peut ni changer ni
    répéter le domaine) le reçoit de `infer::prepare_rule` (`completed.domain`, lu par
    `domain_of`) et le porte. L'ordre des verrous ne change pas : `lock_targets` réunit
    les domaines de tous les plans dans un `BTreeSet` avant de les prendre (ADR 0015).
    Tests : `every_written_domain_is_named` (déplacement de `lab` vers `annex` →
    `[lab, annex]`, renommage sur place → `[annex]`),
    `a_group_moved_writes_the_domain_it_leaves_and_the_one_it_joins` (unitaire),
    `a_rule_anchored_in_a_domain_locks_and_names_it` (`kartenn-policy/tests/it/inference.rs` :
    création et modification d'une règle ancrée → le domaine, règle sans domaine → `[]`).
  - choix : une modification d'un groupe sans déplacement porte aussi son domaine
    d'ancrage (elle écrit un objet de ce domaine, comme `update_vlan` porte celui de son
    groupe) ; le cas de la règle est testé dans `kartenn-policy` et non dans
    `event_domains.rs` : l'`Env` de `kartenn-rest` est une construction sans module, et
    celui de `kartenn-policy` installe déjà la politique ; une règle créée dans le même
    changeset (référence locale) est verrouillée par sa création, `prepare_rule` ne lit
    que les identifiants.
  - reste (revue) : l'ancrage lu par `prepare_group` précède les verrous (course traitée
    par la 22o) ; le helper `named` de `kartenn-policy/tests/it/inference.rs` recopie
    celui d'`event_domains.rs` ; aucun test des `domains` livrés à un abonné (rendu par
    vue) pour la règle ni pour le groupe déplacé ; le test unitaire du groupe ne couvre
    pas le passage d'un ancrage de domaine à un site ou une entité, ni l'inverse ; un
    `completed.domain` mal formé est ignoré en silence (`infer.rs` vers la ligne 813).
- [x] 22o. Domaines lus avant les verrous (relevé par la revue de la 22n) — **arbitrée le
  2026-11-10 : option A** (précision de l'ADR 0015, concurrence) : `prepare` (`prepare_group`,
  `prepare_vlan`) et `lock_targets` (clôture d'un `VlanGroup` ou d'un `Vlan`,
  `pipeline.rs` vers la ligne 1222) lisent le domaine d'un objet avant
  `tx.lock` (`pipeline.rs` vers la ligne 2131), et rien ne le relit sous verrou. Deux
  déplacements concurrents d'un même groupe G depuis D1 (vers D2 et vers D3) : le second
  attend D1, puis sort G de D2 sans tenir D2 ; un troisième changeset lie sous le verrou
  de D2 un préfixe de D2 à un VLAN de G ; les deux contrôles passent (READ COMMITTED) et
  l'invariant d'ancrage est violé ; l'outbox du second nomme `[D1, D3]` sans D2. Options :
  (A, recommandée) après la prise des verrous, relire les domaines des objets lus avant ;
  si l'ensemble diffère de celui tenu, annuler la transaction et rejouer l'étape depuis
  `prepare`, une ou deux fois au plus, puis refuser en conflit ; l'ordre déterministe
  des verrous reste intact ; (B) refuser aussitôt en conflit (409) et laisser le client
  rejouer : plus simple, mais visible au contrat ; (C) prendre le verrou du domaine
  manquant après coup : ordre non déterministe, risque d'interblocage, écartée — verify :
  test d'intégration à deux transactions (déplacements concurrents d'un groupe), le
  second rejoue ou refuse, l'invariant d'ancrage tient, l'outbox nomme les trois domaines
  - où : `crates/kartenn-service/src/pipeline.rs` (`lock_targets`, l'appel de
    `tx.lock`), `crates/kartenn-store-pg/src/vlan.rs` (`prepare_group`, `prepare_vlan`),
    `docs/adr/0015-serialisation-ecritures-par-domaine.md` (précision)
  - dépend de : 22n
  - fait (2026-11-10) : `stages` (`kartenn-service/src/pipeline.rs`) prend ses verrous
    dans une boucle ; sous verrou, `moved` relit le domaine des objets dont le domaine
    peut changer (groupe de VLAN, VLAN, objet de module : clôture, modification,
    restauration, affectation) et complète de nouveau, depuis la requête d'origine
    (`prepare_operation`, extrait de `prepare`), les opérations des VLAN du noyau et
    celles des modules dont le plan verrouille un domaine. Un domaine hors de l'ensemble
    tenu ou une complétion différente : annulation, et les étapes reprennent depuis
    `begin` ; après `MAX_REPLAYS` (2) rejeux, refus par `refuse` avec
    `changeset.operation.conflict` (opération et objet nommés). `domain_of`
    (`kartenn-store-pg/src/write.rs`) lit, pour ces trois sortes, la version en vigueur à
    `clock_timestamp()`, à défaut la dernière (il lisait une version quelconque : un
    groupe déplacé verrouillait et nommait son ancien domaine ; et `now()`, le début de
    la transaction, ne voit pas la version d'un changeset ouvert plus tard et servi
    avant). Correction de revue : sous verrou, le dernier domaine de l'objet d'un plan de
    module relu doit figurer dans les domaines de ce plan, sinon rejeu (`object_kind`
    rend `VlanGroup`/`Vlan` aux sortes du noyau). Précision datée dans l'ADR 0015.
    Test de revue : `a_move_that_started_first_and_locked_last_replays` (la validation
    d'un déplacement en attente ouvre sa transaction puis attend la ligne de son
    changeset, tenue par le test, constatée dans `pg_stat_activity` ; un déplacement
    ouvert après elle commit d'abord ; la validation rejoue et nomme `[annex, third]` ;
    sans la correction, elle écrivait à son début et échouait sur une période inversée).
    Test :
    `a_group_moved_twice_at_once_locks_the_domain_it_really_leaves`
    (`kartenn-rest/tests/it/event_domains.rs` : deux déplacements de `contested` depuis
    `lab`, vers `annex` et `third`, en file sur le verrou de `lab` ; les deux appliqués,
    le premier nomme `[lab, X]`, le second `[X, Y]` sans `lab`, le groupe et son VLAN
    ancrés sur `Y`) ; avec les rejeux désactivés à l'essai, le second est refusé.
  - choix : la complétion relue est comparée entière à la première (pas seulement ses
    domaines) : un nouveau plan tirerait de nouveaux identifiants, et une complétion lue
    sur un état changé rejoue de toute façon ; seules les opérations de VLAN du noyau et
    celles dont un plan nomme un domaine sont complétées de nouveau, pour ne pas doubler
    le triage des flux ; les domaines des préfixes, plages et adresses ne sont pas relus
    (jamais modifiés) ; le refus réutilise `changeset.operation.conflict` (REQ-004-11)
    sans nouveau code ; le test est une course à deux transactions et non à trois (aucun
    point d'arrêt entre contrôle et commit d'un changeset).
  - reste : le refus après deux rejeux n'a pas de test d'intégration (il faut trois
    déplacements intercalés) ; `cascade::expand` lit aussi avant verrou les enfants d'un
    objet fermé en cascade (un VLAN créé entre-temps dans un groupe fermé) ; `domain_of`
    d'un objet changeant lit sa version à l'instant présent, non sur la période d'un
    changeset daté qui couvrirait plusieurs ancrages ; (revue) l'assertion
    `all(!is_finished)` du premier test de course est vide et cette course repose sur
    des `sleep(200ms)` sans prouver le rejeu ; une opération de module n'est recomplétée
    que si son premier plan nomme un domaine (un domaine apparu entre-temps n'est pas
    vu) ; un objet de module dont la version en vigueur a `domain_id` nul donne `None` ;
    le sujet du refus se replie sur `Uuid::nil()` ; la création d'un VLAN dans un groupe
    déplacé par un changeset ouvert plus tard n'est pas vue (l'objet n'existe pas, le
    groupe est lu à `now()` par `prepare_vlan`) ; le risque général d'une écriture datée
    du début de transaction qui écrase une version plus récente est une tâche à part, 22p.
- [x] 22p. Écritures datées du début de leur transaction (relevé par la revue de la 22o) :
  une écriture de version valide à partir de `now()` (instant du début de la
  transaction, `FOR PORTION OF`) rencontre une version plus récente commise entre-temps
  par un changeset ouvert plus tard. Sans la relecture de la 22o, le cas des VLAN
  tombait en « range lower bound must be less than or equal to range upper bound » (500)
  plutôt qu'en écrasement silencieux. Diagnostic d'abord : pour chaque écriture du
  noyau et des modules, établir s'il y a écrasement silencieux, erreur brute ou refus
  propre. Ensuite, sans arbitrage s'il n'y a que des erreurs brutes, les traduire en
  conflit (`changeset.operation.conflict`, rejeu de la 22o) ; un écrasement silencieux,
  ou un changement de l'instant des écritures (`clock_timestamp()` au lieu de `now()`,
  D8, D10), attend un arbitrage — verify : test d'intégration à deux transactions sur
  une modification de préfixe ordinaire, avec le verrou de ligne de la 22o
  (`a_move_that_started_first_and_locked_last_replays`) comme point d'arrêt ; le
  diagnostic noté à la clôture
  - où : `crates/kartenn-store-pg/src/write.rs`, `crates/kartenn-store-pg/migrations/0002_versioning.sql`
    (`kartenn_portion`), `crates/kartenn-service/src/pipeline.rs` (boucle de rejeu),
    `crates/kartenn-rest/tests/it/event_domains.rs`
  - dépend de : 22o
  - diagnostic (2026-11-10) : aucun écrasement silencieux, des erreurs brutes seulement.
    Le temps système d'une version est le début de sa transaction (`now()`, INV-12) ;
    toute écriture qui remplace ou termine une version le fait par `UPDATE` ou `DELETE`,
    et le déclencheur `kartenn_versioning` copie l'ancienne dans l'historique avec
    `sys_end = now()`. Si un changeset ouvert plus tard l'a commise (`sys_start = T2`,
    après le début `T1`), la période système serait inversée, et l'index GiST
    `tstzrange(sys_start, sys_end)` de l'historique (migration 0019) lève « range lower
    bound must be less than or equal to range upper bound » (500). Par famille :
    modification (`kartenn_portion`, noyau, VLAN, registre, machines, kube, politiques,
    flux) : erreur brute (constatée par le test, à la suppression de la version de
    l'autre) ; clôture (`close_in`, `close_versioned` des modules) : erreur brute
    (l'`UPDATE` ou le `DELETE` touche la version de l'autre, que `kartenn_portion` a
    réécrite en morceaux datés `T2`) ; restauration (`kartenn_restore`) : erreur brute
    (suppression des versions courantes sur la portion) ; affectation et retrait
    (`kartenn_unassign` avant l'insertion) : erreur brute ; création : sans objet (un
    doublon tombe sur les exclusions, REQ-004-12). Avec `expected` (et `expect`), refus
    propre avant toute écriture (`changeset.operation.conflict`, REQ-004-11 :
    `object_version` relu `FOR UPDATE` voit la version de l'autre), constaté aussi.
  - fait (2026-11-10) : la migration 0077 refait `kartenn_versioning` : une version
    remplacée dont `sys_start` dépasse `now()` lève `serialization_failure` (40001),
    message `kartenn.version.stale <objet> of <table> …`, avant la copie dans
    l'historique. `StoreError::is_stale_version` et `stale_object` (`port.rs`,
    `STALE_VERSION`) la reconnaissent ; `run` (`pipeline.rs`) rejoue alors `stages`
    entières, dans une transaction neuve dont le début suit la version de l'autre,
    `MAX_REPLAYS` (2) fois au plus, puis refuse par `refuse` avec
    `changeset.operation.conflict` (objet nommé). L'instant des écritures ne change pas
    (D8, D10). Précision datée dans l'ADR 0015. Test :
    `a_write_that_started_first_and_wrote_last_replays` (`event_domains.rs` : la
    validation d'une modification de préfixe en attente ouvre sa transaction et attend la
    ligne de son changeset ; une modification ouverte après elle commit ; la validation
    rejoue et s'applique, la version de l'autre reste en vigueur jusqu'à la sienne ; sans
    la correction, 500). Le point d'arrêt est extrait en `held` et `validated_after`, que
    le test de la 22o réutilise ; la ligne attendue est cherchée dans la base du test
    (`datname = current_database()`) : sans ce filtre, deux tests de ce genre lancés
    ensemble voyaient l'attente l'un de l'autre et la course n'avait pas lieu.
  - choix : le cas est nommé par le déclencheur (code et message à nous) plutôt que
    reconnu au message de l'index (« range lower bound… », ambigu) ; il est reconnu à la
    marque du message, qui traverse les conversions d'erreurs des modules, et non au code
    SQLSTATE, que `StoreError` (texte) ne garde pas ; le rejeu entoure `stages` dans
    `run`, puisque l'erreur survient à l'écriture, après la boucle de la 22o : une
    tentative ratée a tout annulé, rien n'est écrit hors de sa transaction ; les deux
    boucles comptent chacune leurs rejeux ; le refus d'une soumission prend un nouvel
    identifiant, celui d'une validation le sien ; la violation n'a pas d'indice
    d'opération (l'erreur ne le porte pas).
  - reste : le refus après deux rejeux n'a pas de test d'intégration (trois écritures
    intercalées) ; seule la modification de préfixe a un test de course, les autres
    familles reposent sur le même déclencheur ; une version d'un module dont la ligne n'a
    ni `id` ni `object_id` n'est pas nommée (sujet absent).
  - reste (revue) : `is_stale_version` (`port.rs` vers la ligne 36) reconnaît la marque
    n'importe où dans le message, un texte saisi qui la contient déclencherait des rejeux
    (exiger le préfixe de l'erreur de base) ; `is_stale_version` et `stale_object` sans
    test unitaire ; les boucles de la 22o et de la 22p se multiplient, jusqu'à neuf
    passages de `stages`, borne totale à dire dans l'ADR 0015 ; une horloge système qui
    recule fait refuser en conflit jusqu'au rattrapage, non documenté.

- [x] 22d. Chargement de l'arbre arrêté à deux pages (une passe sur sept, 22a et 24e) :
  après la deuxième page, ni ligne « suite » ni avis, 100 enfants dessinés ;
  `treePages.ts` ne redemande jamais à la même taille et `onLoad` ne lit rien quand
  `levelOf(parent).more` est faux à ce moment — verify : un test Vitest qui reproduit
  l'ordre (réponse de la page 2 après la demande de la page 3) ; le banc, sept passes
  réduites, la ligne `children loaded before scrolling` en PASS à chaque fois
  - où : `web/v2/ui/src/components/internal/treePages.ts`,
    `web/v2/shell/src/views/TopologyView.vue` (`onLoad`, `levelOf`)
  - dépend de : 22a
  - fait le 2026-11-09 — cause : une invalidation pendant la lecture d'une page. `ready`
    (022, à chaque connexion du flux, donc au chargement de l'écran) et tout `changes`
    passent par `invalidateQueries`, qui annule la lecture en cours (`cancelRefetch`) et
    relit le niveau tel qu'il était : la page demandée est perdue, le niveau garde le même
    nombre d'enfants et l'arbre, qui ne demande qu'une fois par nombre d'enfants lus
    (`treePages.ts`, conçu ainsi), ne redemande plus. Deux défauts voisins : une seconde
    demande pendant la lecture d'une page l'annulait et la relisait (`fetchNextPage`
    annule par défaut), et une demande pendant la relecture d'une invalidation
    l'annulait, gardant les pages périmées ; une première page en échec, la ligne activée
    ne relisait rien (`levelOf(parent)` indéfini, `more` faux). Correction :
    `askNextPage` (`topology.ts`), appelé par `onLoad` sur l'observateur lui-même
    (`getCurrentResult`, pas le résultat recopié un instant plus tard) : page suivante
    sans annuler une lecture en cours (`cancelRefetch: false`, la lecture en cours
    attendue), redemandée quand la lecture finie n'a pas apporté de page, sans erreur, et
    tant que le niveau est celui de l'arbre (`live`) ; première page en échec relue. Le
    contrat est écrit sur `load` de `TreePagesHost` (`treePages.ts`, commentaire
    seulement). Vitest (`shell/test/topology.test.ts`, « a page the tree asks for,
    against the reads of an invalidation ») : quatre cas, les quatre en échec avant la
    correction (10 enfants au lieu de 20, page 2 lue deux fois, première page de la
    première lecture gardée, aucune relecture après l'échec). `k-web2-check --base
    main` : 946 Vitest ; ciblé : 1 254 captures sur 2 719.
  - choix : la logique d'`onLoad` sortie dans `askNextPage` (`topology.ts`, à côté de
    `levelOfPages`) pour être testée sur un `InfiniteQueryObserver` réel, comme la 24e ;
    `treePages.ts` garde sa règle d'une demande par nombre d'enfants (la corriger côté
    composant aurait demandé un état « en échec » par liste, faute de quoi un échec
    relancerait la lecture en boucle) ; page arrivée jugée au nombre de pages (le plafond
    `cap` égal au `maxPages` de la requête, comme `levelOfPages`), pas au curseur, qui
    peut changer d'une lecture à l'autre.
  - reste : les sept passes réduites du banc (`bench-web`, ligne `children loaded before
    scrolling`) ne sont pas lancées par l'agent : à la session principale, avec la 22b.
    `HostingTree.vue` (plugin `machines`, `onLoad` → `next()` de `data.ts`) semble exposé
    à la même perte (une invalidation qui annule la page en cours, le nombre d'enfants
    inchangé ensuite) : à vérifier, hors du périmètre de la tâche, tâche 22k.
  - reste (revue) : `onBeforeUnmount` (`TopologyView.vue` vers la ligne 183) arrête les
    observateurs sans vider `observers`, si bien que `live()` reste vrai après le
    démontage (une page de plus au plus, sans abonné) ; `settle()` de `topology.test.ts`
    (vers la ligne 447) tourne vingt fois à vide au lieu de boucler tant que `waiting`
    n'est pas vide ; le double transtypage `as unknown as LevelReader<Node>`
    (`TopologyView.vue` vers la ligne 202) masque tout écart avec
    `InfiniteQueryObserver`. La cause est reproduite en Vitest, pas encore mesurée au banc :
    la tâche ne se tient en entier qu'après les sept passes de la 22b.
  - passes du banc (session principale, 2026-11-09, faaac613, échelle réduite) : six sur
    sept en PASS ; la troisième s'arrête encore à deux pages (100 enfants, `aria-setsize`
    -1, pas de ligne « suite », pas d'avis du plafond) **sans aucun événement du flux
    pendant le chargement** : la cause corrigée ici est réelle mais n'est pas celle du
    banc. Suite en 22l.

- [x] 22l. Chargement de l'arbre arrêté à deux pages, seconde cause (relevé par les passes
  du banc après la 22d) : passe 3 sur 7, son journal : « after 2
  pages read, 1 of them the first (median 38 ms), a row for more: false, aria-setsize -1 »,
  « event stream while loading: nothing ». Sans invalidation, le niveau passe à « plus de
  page » après la deuxième : chercher qui met `more` à faux (réponse de la deuxième page
  sans curseur suivant côté serveur, `levelOfPages` sur un nombre de pages ou un
  `hasNextPage` lu trop tôt, `aria-setsize` inconnu à -1 alors que le total est servi) ;
  reproduire avant de corriger — verify : un test qui reproduit l'ordre trouvé (Vitest, ou
  test d'intégration de la route `children` si la cause est côté serveur) ; sept passes
  réduites du banc par la session principale, `children loaded before scrolling` en PASS
  à chaque fois
  - où : `web/v2/shell/src/views/topology.ts` (`levelOfPages`, `askNextPage`),
    `web/v2/shell/src/views/TopologyView.vue` (`onLoad`, `levelOf`),
    `web/v2/ui/src/components/internal/treePages.ts`, `web/v2/sdk/src/data/cache.ts`
    (lectures `children`, `getNextPageParam`), et côté serveur la route
    `/v1/prefixes/{id}/children` (curseur) ; mesure : `web/v2/e2e/budgets/budgets.spec.ts`
    (vers la ligne 504)
  - dépend de : 22d
  - fait le 2026-11-09 — cause : le banc, pas l'arbre. Personne ne met `more` à faux :
    `aria-setsize` -1 dit que le niveau a encore des pages, la ligne « suite » existe dans
    le modèle mais n'est pas dans le document, virtualisée hors de la fenêtre de l'arbre.
    La boucle de chargement de `budgets.spec.ts` cherchait une seule fois, au départ,
    l'élément qui défile (le premier ancêtre qui déborde). Jusqu'à 200 lignes l'arbre
    n'est pas virtualisé et n'a pas de hauteur à lui (`.k-tree-grid[data-virtual]` seul
    porte `max-block-size`) : c'est le cadre (`.k-frame__surface`) qui défile. Sa ligne
    « suite » coupe la racine de l'observateur (l'arbre lui-même, déroulé en entier), donc
    la deuxième page est demandée d'emblée, sans défilement. Quand la boucle démarre avant
    l'arrivée de cette page (38 ms ce jour-là), elle retient le cadre ; la page arrive,
    l'arbre passe à 202 lignes, se virtualise, prend la hauteur d'une liste et défile
    lui-même, mais la boucle fait défiler le cadre jusqu'à la fin : `scrollTop` de l'arbre
    reste à 0, aucune ligne au-delà de la centième n'est rendue (`last child reached:
    100`), la ligne « suite » jamais (`a row for more: false`), et la hauteur imprimée est
    celle du cadre (265 lignes, pas les 202 de l'arbre). Quand la page arrive avant, la
    boucle retient l'arbre (passes réussies : 10 002 lignes de hauteur, celle de l'arbre).
    Preuve : reproduit dans le catalogue (état `paged` de `TreeGrid`, modifié le temps de
    l'essai : 400 enfants par pages de 100, la deuxième donnée après le choix de l'élément
    qui défile), la boucle du banc recopiée : cherché une fois, l'élément retenu est
    `<html>`, l'arbre virtualisé à `scrollTop` 0, dernier enfant atteint 100, aucune ligne
    « suite », `aria-setsize` -1 (la signature de la passe 3) ; cherché à chaque tour,
    400 sur 400. Correction : `scrollerOf()` cherché à chaque tour de la boucle, et la
    ligne INFO dit si l'arbre défile lui-même à la fin (`scrolled by the tree itself`).
    `k-web2-check --base main` : 949 Vitest ; 2 719 captures sur 2 719 (tout :
    `budgets.spec.ts` est un fichier du projet Playwright des captures).
  - choix : correction dans le banc seul. Que l'arbre change d'élément qui défile en
    passant 200 lignes est voulu (`TreeGrid.vue`, « Virtualised: the rows scroll in the
    tree, a list's height at most ») et ne bloque pas un utilisateur, qui fait défiler
    l'arbre une fois borné. Le script de reproduction n'est pas versionné : il demande
    un état de catalogue de 400 enfants (de nouvelles captures) pour un défaut du banc ;
    le banc lui-même est le test de l'ordre trouvé.
  - passes du banc (session principale, 2026-11-10, main 34abb5a1) : sept passes
    réduites, `children loaded before scrolling` en PASS à chaque fois, `scrolled by the
    tree itself: true`, 0 lecture `/children` pendant le défilement. Tient aussi la partie
    banc de la 22d.

- [x] 22m. Scénario du banc pour A40 (relevé à la clôture de la 22c) : le verify de la 22c
  (0 lecture `/children` pendant le défilement d'un arbre d'un autre domaine, tâches
  périodiques remises) n'est pas tenable en l'état, le banc n'ayant ni second domaine
  écrit pendant la mesure ni moyen de remettre les tâches ; ajouter un écrivain de fond
  sur un autre domaine pendant le défilement (changesets réguliers) et imprimer les
  lectures `/children` du conteneur mesuré — verify : passe réduite par la session
  principale, 0 lecture `/children` du conteneur pendant la fenêtre, les changesets de
  l'autre domaine vus au flux
  - où : `web/v2/e2e/budgets/budgets.spec.ts`, `web/v2/e2e/budgets/global-setup.ts`,
    `web/v2/e2e/serve.sh` (branche `budgets`)
  - dépend de : 22c
  - mesuré le 2026-11-14 par la session principale (passe réduite `bench-web small`, main
    e86f6af1, seule sur la machine) : 20 changesets écrits sur `bench-a40`, 19 pendant la
    fenêtre, le premier au flux en 107 ms ; 19 changements nommant `bench-a40` vus au flux
    pendant le défilement (PASS) ; 0 lecture `/children` du conteneur (PASS, limite 0) ;
    19 lectures sous `/v1/` en tout, une par changeset, dont les chemins restent à
    nommer. Case cochée.
  - scénario écrit le 2026-11-14, mesure à la session principale (case non cochée) :
    `global-setup.ts` crée le domaine vide `bench-a40` (`OTHER_DOMAIN`) ; dans le test du
    conteneur de 10⁵, après la fenêtre de défilement d'une instance calme (inchangée,
    `scrollRows` et `readsWhile` en sont tirés), `scrollWhileWritten` refait le même
    défilement de 10⁴ lignes en 10 s pendant que `backgroundWriter` applique un changeset
    toutes les 500 ms sur `bench-a40` (un réseau /64 de 2001:db8::/32 chacun, par
    `page.request` sous la session d'Alice, hors de la page : le shell ne l'apprend que par
    son flux), après avoir vu au flux un premier `changes` nommant ce domaine (30 s au plus) ;
    l'écrivain s'arrête par `stop` dans un `finally`, ou de lui-même sur un refus ou un
    contexte fermé (échec, délai). La sonde garde les `domains` de chaque `changes`.
    Lignes imprimées : `INFO background writer on bench-a40: …`, `INFO reads while
    scrolling, bench-a40 written: …`, `INFO event stream while scrolling, bench-a40
    written: …`, `INFO while scrolling, bench-a40 written: N changes naming the domain of
    the container; dropped frames …`, `PASS|FAIL changes naming bench-a40 at the stream
    while scrolling: N (at least 1, M written, no write refused)` et `PASS|FAIL reads of
    the container's children while bench-a40 is written: N (limit 0)`. `benchDomain`
    écarte `bench-a40`. Passe à lancer : `bench-web small -g "container of"`. Prouvé hors
    banc (essai non versionné, sous `k-heavy --pg --all`, sur l'instance des parcours avec
    le vrai `budgets/global-setup.ts` et une copie des aides du banc) : sur un conteneur de
    300 enfants, 18 changesets écrits pendant la fenêtre, 18 `changes` nommant `bench-a40`
    vus au flux, 0 lecture `/children`, 0 changement nommant le domaine du conteneur ;
    l'écrivain arrêté sur un refus (400) et sur un contexte fermé. `k-web2-check --base
    main` : 1 096 Vitest ; tout, 2 739 captures sur 2 739 (`budgets.spec.ts` compté parmi
    les fichiers du projet des captures).
  - choix : une seconde fenêtre plutôt que l'écrivain dans la première, pour garder les
    budgets d'images et de tâches longues mesurés sur une instance calme (les images de la
    seconde en `INFO`) ; tâches périodiques laissées éteintes, l'écrivain du banc les
    remplaçant (régulier et borné, quand la vérification des drapeaux publics écrit 10³
    adresses par changeset des minutes durant) ; les écritures comptées par le flux
    (`domains` du `changes`), non par l'identifiant du changeset ; preuve « au moins 1 »
    changement vu pendant la fenêtre, le nombre écrit imprimé à côté ; `bench-a40` possédé
    par une entité de même slug (`core.owner` est exigé d'un domaine).
  - reste : la passe réduite (session principale, avec 22e, 22f et 22g). Relevé par
    l'essai, hors périmètre : un `create_prefix` sur un identifiant de domaine inconnu est
    appliqué (201, `applied`), sans violation ; à examiner côté service.

- [x] 22k. Arbre d'hébergement des machines contre les lectures d'une invalidation
  (relevé par la 22d) : `HostingTree.vue` (`onLoad` → `next()` de `data.ts`) perd sans
  doute la page demandée quand une invalidation annule sa lecture, comme la topographie
  avant la 22d ; le contrat de `TreePagesHost.load` (`treePages.ts`, une demande tient
  jusqu'à sa page, la fin de la liste ou un échec) n'y est pas tenu. Reprendre
  `askNextPage` de `topology.ts` (le partager si les deux hôtes s'y prêtent) — verify :
  Vitest du plugin sur le modèle du describe « a page the tree asks for, against the
  reads of an invalidation » de `web/v2/shell/test/topology.test.ts`
  - où : `web/v2/plugins/machines/src/HostingTree.vue`, `web/v2/plugins/machines/src/data.ts`
    (`next`), `web/v2/shell/src/views/topology.ts` (`askNextPage`, `LevelReader`)
  - dépend de : 22d
  - fait le 2026-11-09 — la perte confirmée : `next()` de `useList` et `useLists`
    (`data.ts`) appelait `fetchNextPage()` une fois, sans suite quand une invalidation
    annulait la page, et annulait (`cancelRefetch` par défaut) la relecture d'une
    invalidation en cours, gardant la première page périmée. Correction : `askPage`
    (`data.ts`), appelé par les deux `next()` : page suivante sans annuler une lecture en
    cours (`cancelRefetch: false`), redemandée quand la lecture finie n'a pas apporté de
    page, sans erreur, tant que la liste est suivie (`hasListeners`) et sous la même
    requête (`getCurrentQuery`), jamais au-delà de `MAX_PAGES`. `HostingTree.vue`
    inchangé. Vitest (`plugins/machines/test/hosting-pages.test.ts`, « a page the hosting
    tree asks for, against the reads of an invalidation ») : quatre cas sur l'arbre monté
    (premier niveau et invités d'un hôte, page demandée par Entrée sur la ligne qui la
    lit), trois en échec avant la correction (10 machines au lieu de 20 aux deux niveaux,
    première page de la première lecture gardée) ; la page demandée deux fois en vol
    était déjà lue une fois (garde `!fetching` d'`onLoad`). `k-web2-check --base main` :
    954 Vitest ; ciblé : 0 capture sur 2 719.
  - choix : version locale au plugin plutôt qu'`askNextPage` partagé par `@kartenn/sdk` :
    celle du shell relit une première page absente (ligne activée), ce que le plugin
    laisse à l'admission des listes d'invités (`limited`) et à « Réessayer », et plafonne à
    `CHILDREN_PAGES` quand les listes du plugin s'arrêtent à `MAX_PAGES` ; le partage
    aurait demandé plus de paramètres que la fonction n'a de lignes. La règle mise dans
    `next()` et non dans `onLoad` : tous ses appelants (`HostSection`, `AttachAction`,
    `ClusterRecord`, `InterfacesTab`, `InterfaceSearch`) gardent déjà `pages < MAX_PAGES`
    et gagnent la même tenue ; la vie de la demande lue sur l'observateur
    (`hasListeners`, la requête courante) plutôt qu'un drapeau de portée.
  - reste : aucun relecteur d'interface lancé (ni gabarit ni style touchés, aucune
    capture tirée).
  - reste (revue) : la raison écrite au `choix :` contre le partage d'`askNextPage`
    exagère ; la vraie raison est le contrat public du SDK (un export de plus pour les
    plugins) ; la liste des appelants de `next()` oublie `MachineList.vue` (vers la ligne
    289) et `MachineRecord.vue` (vers la ligne 126), qui gardent bien `pages <
    MAX_PAGES` ; le refus sur `isError` (`data.ts` vers la ligne 181) change `useList.next`
    pour ces deux appelants (après une relecture en échec qui garde ses données, le
    défilement ne demande plus la page suivante), non dit à la clôture ; aucun cas ne
    prouve la garde de vie (démontage, changement de clé pendant la chaîne) ni l'arrêt sur
    échec d'une page ; `/^r0-0@[3-9]/` (`hosting-pages.test.ts` vers la ligne 169) dépend
    de la numérotation des lectures et d'un seul chiffre.

- [ ] 22e. Défilement de l'arbre de 10⁴ lignes : 77 % de trames perdues et 137 tâches
  longues (la plus longue 151 ms) sans aucune lecture ; profil de trame, puis correction
  dans le rendu virtuel de l'arbre — verify : passe réduite du banc, trames perdues
  ≤ 2 % et aucune tâche longue pendant le défilement
  - où : `web/v2/ui/src/components/` (`Tree`, `TreeGrid`, rendu virtuel),
    `web/v2/e2e/budgets/budgets.spec.ts` (mesure du défilement)
  - dépend de : 22c, 22d
  - découpée le 2026-11-11 (option A, retenue par la session principale) : le profil de
    trame et les corrections sans risque en 22e-a, le reste du verify en 22e-b ; le budget
    ne change pas.
  - [x] 22e-a. Profil de trame et premières corrections — verify : passe réduite filtrée
    (`bench-web small -g "container of"`) avant et après ; tests de `rowSlots` ; focus
    conservé après défilement
    - où : `web/v2/shell/src/views/TopologyView.vue` (`nodeById`, `addLabel`),
      `web/v2/ui/src/components/internal/virtualRows.ts`, `TreeGrid.vue`, `DataGrid.vue`
      (`scheduleWindow`)
    - fait le 2026-11-11 — profil (trace CDP, CPU ×4, build non minifié) : `nodeById`
      de `TopologyView` prenait 3,3 s des ≈ 11 s de tâches longues (par ligne et par
      rendu, `rowsOf` aplatissait les 10⁴ enfants puis `find`, deux fois par ligne via
      `addLabel` et `addBlocked`), puis le montage et le démontage des lignes entrant et
      sortant de la fenêtre à chaque trame, puis le pipeline de rendu. Corrections :
      index `readById` (premier niveau qui tient le nœud, comme avant) ; noms du « + »
      mémorisés par libellé et par langue ; au-delà de `VIRTUAL_ROWS`, lignes recyclées
      (`rowSlots`, `keys` de `useVirtualRows`) : une ligne qui entre reprend l'élément
      d'une ligne sortie, patché, jamais en traversant une ligne gardée (focus, éditeur,
      à ce rendu et au précédent), l'emplacement d'une ligne gardée qui sort réservé
      (démonté, pas hérité) ; sous `VIRTUAL_ROWS` aussi, les lignes ont leur emplacement
      (sans reprise : une ligne qui entre en a un neuf), si bien qu'une ligne restée garde
      son élément au passage du seuil (un nœud ouvert ou fermé) ; `keys` calcule depuis le
      dernier rendu validé (`onMounted`, `onUpdated`), sans écrire cet état ; `measure`
      tient `elementsCache` du virtualiseur à jour ; `scheduleWindow` de `DataGrid` ignore
      le défilement d'un élément qui ne le contient pas. Changement de comportement : la
      ligne sélectionnée de `TreeGrid` reste montée hors de la fenêtre, comme celle du
      focus (recyclée, son fond s'effacerait sur la ligne qu'elle quitte). Passe réduite
      filtrée : 86,7 % → 39,0 % de trames perdues, 78 → 2 tâches longues, la plus longue
      213 → 52 ms. Tests : `rowSlots` (reprise, côté des lignes gardées, deux lignes
      gardées à la fois, emplacement réservé, passage du seuil ;
      `ui/test/components-data.test.ts`), élément du focus inchangé après le défilement
      et au passage du seuil dans les deux sens (`e2e/components/tree-grid.spec.ts`).
      `k-web2-check --base main` : 956 Vitest ; ciblé : 2 316 captures sur 2 720.
    - choix : correction principale dans la vue du shell (aucune API touchée) ;
      recyclage seulement au-delà de `VIRTUAL_ROWS` ; `Tree` non recyclé (les `TreeItem`
      de Reka dont la `value` change en place : risque, et le banc ne le mesure pas) ;
      `k-target-seed` depuis la copie principale (`K_TARGET_BASE`), seule à avoir un
      `kartenn-server` en release.
    - reste : aucun relecteur d'interface lancé (rien de visible ne change) ; mesures
      bruitées (écart de 2 à 3 entre passes identiques, machine partagée) ; (revue) sur
      un élément recyclé, la rotation du chevron et le fond de `data-drop` s'animent
      d'une ligne à l'autre ; `readById` refait l'aplatissement que fait déjà `items`.
  - [ ] 22e-b. Le reste du verify de la 22e : pipeline de rendu (double Paint du document
    à chaque trame, première cellule collante de chaque ligne, pistes `minmax(auto, …)`),
    lignes en composants (une ligne aux props inchangées ne se rend plus), appels de
    `stateOf` et `nodeState` par cellule (jusqu'à sept et six) — verify : celui de la 22e
    - où : `web/v2/ui/src/components/TreeGrid/TreeGrid.vue` (gabarit des lignes, styles
      `.k-tree-grid__row`), `web/v2/ui/src/components/DataGrid/grid.ts`
      (`columnsTemplate`)
    - dépend de : 22e-a
    - note : le budget (≤ 2 % à ×4 pour 1 000 lignes/s, huit fois le protocole du
      prototype D-1) est à mesurer par la session principale, seule sur la machine, avant
      l'arbitrage B (le garder, mesuré seul) ou C (réviser le protocole à 500 lignes/s).
    - mesuré le 2026-11-14 par la session principale, seule sur la machine (passe réduite
      `bench-web small`, main e86f6af1, les quatre verrous libres) : 28,25 % de trames
      perdues sur 10⁴ lignes en 10 s (budget 2 %), 1 tâche longue (51,0 ms, budget 0),
      0 lecture pendant le défilement ; avant la 22e-a : 77 % et 137 tâches longues.
      L'écart au budget reste d'un ordre de grandeur : l'arbitrage B ou C attend
      l'utilisateur, la 22e-b reste à faire dans les deux cas.
    - avancé le 2026-11-14 (agent, non cochée : la 22e se clôt sur la mesure de la session
      principale) — profil (trace CDP de la vue réelle, CPU ×4, 3 s à 1 000 lignes/s, build
      non minifié) : ≈ 25 à 30 ms par trame pour 16,7 lignes entrant par trame ; flush de
      Vue ≈ 9 ms (rendu ≈ 3, patch ≈ 4,4 : toute la fenêtre, ≈ 37 lignes, rendue à chaque
      trame, rappels de l'hôte ≈ 1,5), mise en page forcée par le `scrollTop` du protocole
      ≈ 6 ms (≈ 63 objets sales sur 1 274), PrePaint 2, Paint 2,5, Layerize 1,9, hit test
      et déplacement de souris synthétique ≈ 1. Le « double Paint » est le parcours de
      peinture de `LayoutView` et de la racine (deux événements par trame, peinture en
      cache, pas un repeint du document). Le banc du catalogue (`/_ui/bancs/TreeGrid`,
      5 100 lignes, une colonne, sans rappels) tient 1,6 à 2,7 % à ×4 : l'écart vient de ce
      que la vue réelle rend par ligne. Banc A/B du catalogue à ×8 (feuille injectée) :
      `will-change: scroll-position`, première cellule non collante, pistes fixes au lieu
      de `minmax(auto, …)`, `contain: strict` ou `content` sur les lignes — aucun effet
      au-dessus du bruit ; rien de changé dans les styles. Corrections : lignes de nœud en
      composant (`TreeGridRow`, contexte commun `row.ts`) : au défilement, seules les
      lignes dont la ligne du modèle change se rendent (17 rendus par trame à 1 000 lignes/s
      au lieu de toute la fenêtre, compté sur le banc du catalogue) ; chaque état de
      l'hôte demandé une fois par rendu de ligne (`nodeState` deux fois par ligne au lieu
      de six, `cellState` trois fois par ligne de deux cellules au lieu de sept par
      cellule) ; ids des raisons fixes par phrase (`cellReasons.ts`, `DataGrid` aussi),
      sans quoi la description de chaque cellule suivait la fenêtre et rendait toute
      ligne ; `tame` n'écrit plus un `tabIndex` déjà à −1. Avec l'emplacement `extension`
      et une colonne d'extension, les lignes se rendent à chaque rendu de l'arbre, comme
      avant (`tick`) ; sans colonne d'extension, l'emplacement ne coûte rien (la vue de la
      topographie le donne toujours). Tests : `ui/test/trees-mounted.test.ts` (états
      demandés une fois, aucun rendu de ligne quand l'arbre seul se rend, avec et sans
      l'emplacement). Passes réduites filtrées (`bench-web small -g "container of"`) :
      départ 29,49 %, 1 tâche longue de 56,0 ms (verrous libres) ; après les composants,
      mais avec `tick` posé dès que l'hôte donne l'emplacement (toutes les lignes rendues à
      chaque trame dans la topographie, erreur corrigée ensuite) 31,42 %, 2 tâches longues,
      la plus longue 56,0 ms (verrous libres, charge 1,6) ; la correction finale n'a pas
      été mesurée sur la vue réelle (quota de passes atteint). Profils sous verrou (vue
      réelle, spec de brouillon) : 30,42 % à 1 000 lignes/s et 18,47 % à 500 lignes/s
      (départ, verrous libres) ; 31,88 % (départ, non minifié, 21f tenant `pg` et les deux
      créneaux). `k-web2-check --base main` : 1 165 Vitest ; ciblé : 2 322 captures sur
      2 741 ; scripts `tree`, `tree-grid`, `data-grid` (41) ; parcours `tree` (10).
    - choix : composant de ligne plutôt que `v-memo` (les dépendances suivies par Vue, pas
      une liste à tenir) ; contexte en une prop stable plutôt que `provide` ; `TreeGridRow`
      interne, non exporté ; lignes « suite » et avis laissées dans le gabarit de l'arbre
      (sans état ni rappel) ; rien de changé dans les styles (aucun effet mesuré) ; aucun
      relecteur d'interface (le DOM rendu est le même, captures inchangées).
    - reste : la mesure de la session principale (passe réduite, machine calme) ; selon
      elle, 22e-c.
  - [ ] 22e-c. Ce que la 22e-b laisse, selon la mesure de la session principale : le coût
    d'une ligne qui entre (patch d'une ligne recyclée, mise en page ≈ 6 ms et peinture
    ≈ 6 ms par trame à ×4 pour ≈ 17 lignes) — moins de nœuds par ligne (icônes SVG, « + »
    caché monté sur chaque ligne, spans cachés), puis profil de la mise en page — ou
    l'arbitrage C (protocole à 500 lignes/s) — verify : celui de la 22e
    - où : `web/v2/ui/src/components/TreeGrid/TreeGridRow.vue`,
      `web/v2/ui/src/components/IconButton/IconButton.vue` (« + » de chaque ligne),
      `web/v2/shell/src/views/TopologyView.vue` (`cellState`, `readonlyReason` de chaque
      cellule)
    - dépend de : 22e-b, mesure de la session principale
    - avancé le 2026-11-14 (agent, branche `tache-021-22e-c`, non cochée : le verify revient
      à la passe de la session principale ; **budget toujours hors d'atteinte**) — profil :
      sonde montée de la vraie vue (`shell/test/tree-grid-work.test.ts` : la topographie,
      un conteneur de 1 000 enfants ouvert, l'arbre virtualisé, défilé de 17 lignes par pas,
      les rappels de l'hôte comptés par prop, les rendus par composant) : à chaque pas,
      17 lignes rendues (bien), mais 268 appels de `cellState`, 234 d'`editable`, 122 de
      `readonlyReason` — la table des raisons cachées relisait toutes les cellules des
      43 lignes rendues, et chaque ligne qui entre redemandait l'état de ses cellules
      (`messageOf`, `reasonOf`) ; un pas de focus rendait 31 lignes (`isStop` lu par
      chacune). Trace CDP de la vraie vue (passe 1, build minifié, CPU ×4) : ≈ 22 ms par
      trame — flush de Vue ≈ 6 ms (rendu des 17 lignes ≈ 1,3, patch, 17 `insertBefore`
      des lignes recyclées), mise en page forcée par le `scrollTop` du protocole ≈ 5,5 ms
      (83 objets sales), PrePaint ≈ 2, Paint ≈ 2,6, Layerize ≈ 1,3, puis une seconde mise
      en page (≈ 0,7, le survol du pointeur synthétique). Bancs A/B dans la vraie vue (même
      passe, feuilles construites qui retirent une partie des lignes, `K_BENCH_TREE_AB=1`,
      écart entre deux passages identiques jusqu'à 11 points) : base 26,5 et 24,7 % ; sans
      le « + » 6,8 et 7,2 % ; sans aucun SVG 4,9 et 4,4 % ; sans survol (`pointer-events`)
      13,5 et 8,2 % ; sans SVG ni survol 2,06 % ; pistes fixes 24,7 et 20,1 % ; cellules
      en bloc 15,8 et 24,1 % ; sans transition 23,5 et 19,0 % ; sans `sticky` 22,2 % ;
      `contain` 23,7 %. **Cause nommée** : ce que la ligne qui entre a de nœuds à remettre
      en page et à peindre — ses trois SVG (chevron, icône du type, icône du « + ») et le
      bouton « + » transparent — et le survol du pointeur posé sur l'arbre (la ligne
      survolée change à chaque trame) ; le JS n'en est plus que le quart. Corrections :
      ce que l'hôte dit des cellules d'un nœud (`cellState`, `editable`, raisons) demandé
      une fois par cellule et gardé dans un `computed` par nœud (`factsOf`), lu par la ligne
      et par la table des raisons (un pas : 51, 51 et 34 appels pour 17 lignes ; un retour
      sur des lignes vues : aucun) ; le point d'arrêt et la sélection lus par la clé de la
      ligne dans des `computed` (`stopIn`, comme `DataGrid/row.ts`) : un pas de focus, une
      sélection déplacée rendent deux lignes ; les commentaires de la chaîne
      `v-if`/`v-else` du corps passés dans le script (fragments en développement ; hors de
      la chaîne, un commentaire du gabarit est livré : 134 o gzip du shell initial) ;
      l'icône du « + » caché n'est
      plus mise en page (`display: none`, le bouton transparent gardé, dans l'arbre
      d'accessibilité, sa place tenue). Tests : `shell/test/tree-grid-work.test.ts` (un pas
      de défilement : au plus 18 lignes, chaque rappel au plus une fois par cellule des
      lignes qui entrent, aucun au retour ; un pas de focus : deux lignes, aucun
      `cellState`), `ui/test/trees-mounted.test.ts` (`cellState` une fois par cellule ; un
      pas de focus et une sélection déplacée : deux lignes) ; tous en échec sur main
      3a9e548e (268 appels pour 54 permis, 31 lignes pour 2). Passes réduites filtrées
      (`bench-web small -g "container of"`, verrous libres au départ ; 11p9 tenait un
      créneau et faisait tourner Chromium à la première, charge 1 à 22 ; seule à la
      troisième, charge 0,4 à 1,3) : après les corrections de JS 28,61 %, 3 tâches longues
      (56,0 ms) — rien gagné, comme la 22e-b ; avec l'icône du « + » cachée 24,79 %,
      1 tâche longue, dans l'écart des passes ; flèche dans l'arbre p75 13,1 ms.
    - choix : `factsOf` par nœud (un `computed` dans une `WeakMap` du nœud) plutôt que par
      ligne (la table des raisons et la ligne lisent le même) ; le contrat des rappels de
      l'hôte (lire un état réactif, comme un `computed`) écrit en tête de `TreeGrid.vue` ;
      l'icône du « + » cachée plutôt que le bouton (`visibility`, `display` ou non monté
      le retireraient de l'arbre d'accessibilité et des parcours `time`, `accessibility`
      qui le trouvent sur une ligne sans focus) ; le diagnostic A/B gardé dans
      `budgets.spec.ts`, éteint par défaut ; l'éditeur, l'ouverture d'un nœud et la cible
      d'un dépôt laissés lus directement par chaque ligne (rares, et le shell initial est à
      299 914 o pour un budget de 300 000 : main 3a9e548e à 299 875).
    - reste (à arbitrer par l'utilisateur, le budget de 2 % n'étant tenu qu'en retirant ce
      que les lignes montrent) : (B1) le « + » monté seulement sur les lignes où il se
      montre (focus, sélection, survol) — mesuré ≈ 7 % ; change l'arbre d'accessibilité
      (les « + » des autres lignes ne sont plus lus en mode navigation) et deux parcours ;
      (B2) les icônes des lignes en masques CSS plutôt qu'en SVG en ligne (système de
      design) — sans aucun SVG, ≈ 4,5 % ; (B3) le survol suspendu pendant un défilement
      (`pointer-events` coupé jusqu'à la fin du défilement) — sans SVG ni survol, 2,06 % ;
      (B4) piste à vérifier : la ligne recyclée déplacée par `insertBefore` recrée ses
      objets de mise en page, ses SVG avec, même inchangés — un recyclage sans déplacement
      changerait l'ordre du DOM (structurant) ; ou l'arbitrage C (protocole à
      500 lignes/s, 18,47 % mesuré par la 22e-b avant ces corrections).

- [ ] 22f. Listes au-delà de dix pages : la page de 100 lignes à 212 ms au p95 (budget
  100 ms) ; quand le cache lâche la première page (`MAX_PAGES`), la liste saute d'une
  page sous la fenêtre faute de compensation du défilement — verify : Vitest de
  `DataGrid` (une page lâchée ne déplace pas la ligne sous le pointeur) ; passe réduite,
  page de 100 lignes ≤ 100 ms au p95
  - où : `DataGrid` de `web/v2/ui/src/components/`, `MAX_PAGES` de `web/v2/sdk/src`
  - dépend de : 22a
  - code et Vitest faits le 2026-11-14 ; mesure à la session principale (passe réduite
    commune aux 22e, 22f, 22g et 22m), case à cocher après elle. Deux causes, par lecture
    du code et une mesure de `DataGrid` seul (happy-dom, profil V8). (1) Passé dix pages,
    le partage structurel du cache comparait chaque page à celle de sa place : la première
    lâchée, toutes montaient d'une place, et chaque ligne gardée devenait une copie, égale
    mais un autre objet ; la grille rendait toutes ses lignes à nouveau et
    `ExtensionElement` recréait l'élément de chaque cellule d'extension (une application
    Vue par cellule pour `machines`) : les pages 11 à 20 du banc, son p95. `sharePages`
    (`sdk/src/data/cache.ts`, `structuralSharing` de `pages`) compare par curseur : les
    pages gardées restent les mêmes objets. (2) Sans compensation, la fenêtre restait au
    même décalage et montrait les lignes d'une page plus loin, à monter en entier.
    `useVirtualRows` (option `anchor`, `DataGrid` seul) passe `anchorTo: "end"` au
    virtualiseur quand des lignes partent ou viennent en tête seulement (`headShift`) :
    il ancre la ligne du haut de la fenêtre dès le rendu, sans trame fausse ; un tri ou un
    filtre garde le décalage. `overflow-anchor: none` sur la grille virtualisée, comme
    `DiffList`. (3) Coûts en O(n) de chaque page : les clés des lignes lues une fois par
    changement (`rowKeyList`, au lieu d'un `keyOf` par les props pour chaque appel du
    virtualiseur, de l'index, des marques), `isEditing` sans lire la clé hors de la colonne
    éditée, `tameWidgets` sans les cellules, `shownRows` de `inline.ts` sans parcourir les
    lignes quand rien n'est écrit ni au brouillon. Sur `DataGrid` seul (happy-dom, 1 000 à 7 000
    lignes, ancrage compris), l'arrivée d'une page passe de 7 à 17 ms, croissant avec les
    lignes, à 7 à 8 ms, constant. Vérification
    : `ui/test/data-grid-mounted.test.ts` (défilé vers le bas, vers le haut, page revenue
    en tête, tri, focus et éditeur gardés ; échoue sans l'ancrage),
    `sdk/test/cache.test.ts` (objets gardés après la onzième page) ;
    `components/data-grid.spec.ts` 15 passés ; `k-web2-check --base main` (1104 tests,
    2738 captures sur 2738 : tout, `cache.ts` étant tiré par l'entrée du shell).
  - choix : l'ancrage du virtualiseur plutôt qu'une écriture du défilement après le rendu
    (une trame aux mauvaises lignes, deux rendus) ; ancré sur la ligne du haut de la
    fenêtre, non sur celle du pointeur (le même décalage pour toutes les lignes en vue) ;
    `anchor` réservé à `DataGrid` (les arbres inchangés).
  - corrections de la revue (2026-11-14) : (1) relue sous des curseurs neufs (chaque
    `next` fige son `as_of`), une liste perdait tous ses objets ; une page sous un curseur
    inconnu, ou changée sous le sien, garde ses lignes une à une (`shareRows` : par leur
    `id` où qu'elles fussent, à défaut à leur place), une ligne changée toujours neuve.
    (2) `headShift` rend le nombre de lignes déplacées ; l'ancrage seulement pour un
    déplacement d'au moins `ANCHORED_ROWS` (50, la moitié d'une page du serveur) et la
    grille défilée : une ligne créée ou retirée en tête d'une liste triée se voit à sa
    place, à 0 comme plus bas. (3) `anchorTo` rendu à `"start"` une fois les lignes
    rendues. Commentaire du banc (`budgets.spec.ts`) mis à jour.
  - choix (revue) : seuil fixe plutôt qu'un signal du cache (un prop de plus à `DataGrid`) ;
    `anchorTo` remis à `"start"` sur l'instance du virtualiseur (`options`), sans nouveau
    passage d'options, donc sans rendu de plus.
  - reste : la passe réduite doit montrer un p95 proche de la médiane des pages 2 à 10 ; s'il
    reste au-dessus de 100 ms, le poste suivant est le rendu de toutes les lignes de la
    fenêtre à chaque page (lignes en composants, comme la 22e-b pour `TreeGrid`) et le
    montage d'une application `machines` par cellule des dix lignes de débord ; (revue)
    `drafted` (`inline.ts` vers la ligne 284) parcourt toutes les lignes à chaque page, y
    compris dans le raccourci de `shownRows`, en O(n) ; les arbres (`TreeGrid`, `Tree`) n'ancrent
    pas une tête de liste lâchée (`roots` passé `CHILDREN_PAGES`) ; aucun relecteur
    d'interface lancé (rien de visible ne change) ; l'ancrage n'est vu que dans happy-dom,
    pas dans un navigateur.
  - suite après la mesure (2026-11-14, branche `tache-021-22f-banc`) : la passe réduite de la
    session principale (e86f6af1) échouait avant le p95, 15 pages dessinées sur 18 voulues,
    6 sans aucune ligne dessinée en 5 s. Le produit, pas le banc, deux causes vues dans
    Chromium. (1) La vue renvoyée en tête de liste : au bout des lignes, aucun intervalle
    après la fenêtre ; l'ancrage déplace la fenêtre d'une page, Vue démonte l'intervalle
    d'avant (sa clé `gap-<index>` change) puis patche les lignes une à une, et la `ref` de
    fonction de chaque ligne (`measure`) la mesurait au milieu du patch : le défileur, réduit
    aux lignes restantes (814 px), ramenait le défilement à 336 px (sonde : 27 552 → 24 752
    écrit par l'ancrage, puis 336 à la mesure suivante). `useVirtualRows` mesure désormais
    les lignes reçues par `measure` une fois le rendu fini (`commit`, `onMounted` et
    `onUpdated`), jamais dans leur `ref`. (2) Une page lue de trop : la page arrive avec la
    première lâchée, et `useNextPage` relisait la dernière fenêtre, donnée parmi les lignes
    d'avant, au bout de la liste : une page de plus aussitôt, posée sous les lignes en vue,
    jamais montrée et une page de plus lâchée (une page sur deux du banc, d'où les 6).
    `useNextPage` retire de la fenêtre les lignes lâchées en tête depuis qu'elle a été
    donnée (`droppedRows`, par le curseur de la première page gardée). Vérification : état
    `paged` du catalogue de `DataGrid` et `components/data-grid.spec.ts` « a paged list past
    ten pages » (échoue sans (1) : la ligne gardée hors du document) ;
    `shell/test/prefetch.test.ts` « reads nothing more when the first page is dropped »
    (échoue sans (2)) ; `components/data-grid.spec.ts` 16 passés ; `k-web2-check --base
    main` (1108 tests, ciblé : 2337 captures sur 2740). Passes réduites filtrées
    (`bench-web small -g "100 rows"`, machine chargée par d'autres agents) : (a) sonde,
    avant les corrections, 9 pages dessinées, 12 non, la grille à 336 px après chaque
    page ; (b) avec (1) seule, 15 dessinées, 6 non ; (c) avec (1) et (2), 20 pages
    dessinées sur 20, aucune non dessinée, p95 238,8 ms, médiane 105,9 ms, pire 598,6 ms :
    budget de 100 ms toujours dépassé, case laissée ouverte.
  - choix (suite) : mesure différée pour tous les hôtes de `useVirtualRows` (`DataGrid`,
    `TreeGrid`, `Tree` : la même `ref` au milieu du patch), plutôt qu'une hauteur minimale du
    corps ou des clés d'intervalles stables (un intervalle patché plus court aurait encore
    raboté la vue d'une dizaine de lignes) ; la fenêtre décalée des lignes lâchées plutôt
    qu'ignorée jusqu'à la suivante (pas d'attente d'une émission que la grille dédoublonne) ;
    le test dans un navigateur (état `paged`), happy-dom ne mesurant pas (tailles en cache).
  - reste (suite) : le p95 (238,8 ms sur machine chargée) contre 100 ms, à remesurer par la
    session principale sur machine calme ; les coûts suivants déjà nommés : re-rendu de
    toute la fenêtre à chaque page, une application `machines` par cellule d'extension,
    `drafted` en O(n) ; (relecture a11y) le shell ne passe pas `rowOffset` à `DataGrid`
    (`ListView`, `ChangesetsView`, `AuditView`, `ImportsView`) : passé dix pages,
    `aria-rowindex` repart de 2 pour la première ligne gardée ; le focus gardé à travers une
    page lâchée n'est vu que dans happy-dom (`data-grid-mounted.test.ts`), pas dans le
    navigateur.
  - relectures (suite) : `ui-reviewer` (légende de l'état `paged` alignée sur `virtual` :
    totaux, un écran ; `KEPT` dit copie de `MAX_PAGES`) et `a11y-auditor`
    (`aria-rowindex` et `aria-rowcount` vérifiés après chaque page lâchée ; minuteur de la
    démo arrêté au démontage), corrigés.
  - corrections de la revue (2026-11-14) : (1) les lignes de `Tree` sont rendues dans
    l'emplacement de `TreeRoot` (Reka), que Vue rend à nouveau sans `Tree` : son
    `onUpdated` ne partant pas, les lignes entrées restaient non mesurées. La file de
    `measure` se vide après la passe qui l'a remplie (`nextTick`, une fois par passe), quel
    que soit le composant rendu, et toujours dans `commit` pour l'hôte ; nœuds détachés
    écartés. Preuve : `components/tree.spec.ts` « each row that comes into the window
    measured… » (lignes de 8 px sous l'estimation de 28 : la vue reste pleine en défilant ;
    échoue sans la correction). (2) La cause (2) prouvée sur le parcours réel :
    `journeys/records.spec.ts` « past ten pages… » (`/prefixes?domain=production`, plus de
    1 100 réseaux, douze pages) : la onzième lue, la dernière ligne en vue reste celle de
    la dixième et aucune page n'est lue sans défilement ; chaque page lue a des lignes
    montrées ; la fin atteinte. Échoue sans `droppedRows` (12 pages lues au lieu de 11),
    Chromium et Firefox. (3) `droppedRows` testé directement (tête lâchée d'une ou deux
    pages ; relue sous curseurs neufs, autre filtre ou tri, page lâchée en queue, tête
    inconnue : 0) ; `given` suit les pages changées sans décalage de tête, et un hôte qui
    ne redonne pas de fenêtre après une tête lâchée voit la sienne comptée parmi les lignes
    gardées deux trames plus tard (`prefetch.test.ts` « reads the next page at the end
    when no window comes… », échoue sans). Passe réduite filtrée après les corrections
    (machine chargée) : 20 pages dessinées sur 20, aucune non dessinée ; p95 339,3 ms,
    médiane 109,3 ms, pire 393,9 ms (indicatif).
  - choix (revue) : `nextTick` depuis `measure` plutôt que `onUpdated` d'un composant (le
    patch fini, quel que soit le rendu) ; le repli à deux trames (la fenêtre de
    `DataGrid` part dans la trame qui suit son rendu) plutôt qu'une fenêtre forcée à
    chaque changement de lignes (un contrat de plus pour tous les hôtes) ; pages du
    parcours comptées par leur première ligne (une relecture, par l'invalidation d'un
    autre parcours, n'est pas une page lue).

- [x] 22g. Navigation en cache à 127 ms au p75 (budget 100 ms) : deux navigations sur
  dix (enregistrements 6 et 10) ne lisent pas `/properties` au survol ; trouver
  pourquoi le préchargement manque, puis exclure du p75 toute navigation non
  préchargée (ou l'imprimer à part) — verify : passe réduite, dix navigations
  préchargées et p75 ≤ 100 ms
  - où : `KLink` et `meta.queries` (tâche 7), `web/v2/e2e/budgets/budgets.spec.ts`
    (vers la ligne 647)
  - dépend de : 22a
  - code et preuves hors banc faits le 2026-11-14, case laissée ouverte : la passe réduite
    est à la session principale (avec 22e, 22f et 22m). Cause, trouvée par une sonde du
    protocole du banc sur les données des parcours (CPU ×4) : le préchargement ne manque
    pas, c'est la mesure qui le manquait. La colonne VLAN de la liste des préfixes
    (`views/parts/NetworkVlan.vue`, tâche 17, REQ-026-6) lit `/properties` de chaque
    réseau en vue, sous la clé même de la route de la fiche ; au survol, les propriétés
    sont fraîches (30 s) et `prefetchQuery` ne lit que la fiche. Le banc attendait une
    réponse `/properties` au survol : les réseaux (6 et 10 en passe réduite, 6, 7 et 9 en
    pleine échelle) passaient pour non préchargés, les conteneurs (sans VLAN, sans lecture)
    non. `budgets.spec.ts` suit désormais les lectures de la page par chemin
    (`freshReads`, depuis le premier écran de la liste) : une navigation est en cache si
    la fiche et ses propriétés ont répondu dans les 25 s, sans lecture en cours, quel que
    soit l'écran qui les a lues ; le p75 ne compte que celles-là (« p75 of N
    prefetched ») et les autres s'impriment à part (`INFO navigations not prefetched`),
    le `FAIL` par fiche gardé. Preuve : parcours `records` « a network of the list hovered
    reads its record alone… » (Chromium et Firefox) : avant tout survol, les propriétés du
    réseau 192.0.2.0/24 sont lues par sa ligne et aucune du conteneur 2001:db8:3::/48 ; le
    survol du conteneur lit la fiche et ses propriétés, celui du réseau la fiche seule ;
    ouvert, le réseau ne relit ni l'une ni les autres.
  - choix : correction dans la mesure, pas dans le code (le partage de la clé entre la
    cellule VLAN et la route est voulu : la fiche s'ouvre en cache) ; fraîcheur jugée à
    25 s, sous les 30 s de `FRESHNESS.object`, par chemin sans les dates (les navigations
    mesurées sont au présent) ; preuve en parcours plutôt qu'en Vitest (la cause tient à
    deux écrans et au réseau).
  - reste (banc) : la passe réduite doit montrer dix navigations préchargées
    (`p75 of 10 prefetched`, « not prefetched … none ») ; si le p75 reste au-dessus de
    100 ms, ce n'est plus le préchargement : la médiane était déjà à 117-125 ms avec des
    navigations en cache, le coût est celui du rendu de la fiche sous CPU ×4.
  - reste : la fiche d'un préfixe ouverte en cache montre encore des parties occupées
    sous Chromium (occupation, résumés des plugins), lectures hors `meta.queries` de sa
    route ; à mesurer avant de les y ajouter.

  - mesuré le 2026-11-14 par la session principale (passe réduite `bench-web small`, main
    e86f6af1, seule sur la machine) : dix navigations préchargées sur dix (« not
    prefetched … none »), la cause de la tâche est donc levée ; p75 à 126,5 ms (budget
    100 ms), médiane 120,5 ms, pire 313,0 ms, CLS 0,001. Le préchargement n'est plus en
    cause : le reste est le rendu de la fiche sous CPU ×4, porté par la 22o. Case cochée
    quand la 22o tient le budget.
- [x] 22n. Shell initial à 306,3 Ko gzip (budget 300 Ko), relevé par la passe réduite du
  2026-11-14 (main e86f6af1) : 295,6 Ko après la 22h, les tâches fusionnées depuis ont
  repris 10,7 Ko ; trouver ce qui est entré dans le premier lot (mesure module par
  module, comme la 22h) et le sortir à la demande — verify :
  `web/v2/shell/scripts/shell-size.ts` ≤ 300 Ko, sans banc
  - où : `web/v2/shell/scripts/shell-size.ts`, `web/v2/shell/src/main.ts` et ses imports
    statiques ; candidats : `frame/width.ts` et la porte de la 21c, `plugins/slots.ts`
    et `ExtensionElement` en cellule (11o2), catalogues `views/i18n/*.json`, `sharePages`
    du SDK (22f)
  - dépend de : 22h
  - fait le 2026-11-14 : 306,3 Ko avant (main 17984cee), 295,5 Ko après ; 295,9 Ko
    rebasée sur main 032a8593 (21c et 21d, +0,4 Ko : `frame/width.ts` et la porte de
    l'écriture, `edit.why.narrow` dans `shared/`). Mesure module
    par module entre la 22h (eab60b9c) et main (cartes de sources, octets gzip par
    source, approchés) : @kartenn/ui ≈ +5 Ko (cellules de la 11o2 et de la 21c :
    `CellSelect`, `cellReasons`, `extensionCell`, `TreeGrid`, `virtualRows`), catalogues
    JSON de l'entrée ≈ +2,9 Ko (`views` +2,2), SDK ≈ +1,3 Ko (`cache`, `dated`,
    `filters`), `QueriesObserver` de query-core ≈ +1 Ko (`useQueries`), modules du shell
    ≈ +1,5 Ko. Sortis du premier lot : les mots des écrans (`views/i18n/{fr,en}.json`,
    ≈ 9,9 Ko), lus par `lazyCatalog` avec le chunk de chaque écran (`worded`,
    `views/words.ts` ; `entryScreen` et `recordScreen` de `views/routes.ts`) et de chaque
    panneau que le cadre ouvre lui-même (édition, clôture, brouillon : `ShellFrame.vue`) ;
    ceux que le cadre lit avant tout écran (`labels`, `problem`, `views.retry` et les
    mots communs, noms et valeurs des champs, `edit.why.past` et `edit.why.narrow`)
    passent dans `views/i18n/shared/`. `useQueries` passe dans `data/queries.ts`, et ce que
    les routes lisent de `views/data.ts` dans `views/carried.ts` (≈ 0,9 Ko).
    `test/build.test.ts`, sur un build sous `NODE_ENV` `production` (comme `vite build`) :
    chaque phrase propre aux catalogues des écrans et de l'administration est dans le
    build et absente du premier lot ; aucune chaîne du premier lot ne nomme une clé que
    seuls ces catalogues portent, entière ou tête d'une clé construite (`a.b.${…}`) ; le
    premier lot tient son budget (échec au-delà de 300 Ko, avertissement au-delà de
    296 Ko). `refusalWords` (`time/index.ts`) nomme chaque clé de la porte entière, pour
    que l'analyse la voie. `test/setup.ts` lit les mots des écrans avant chaque fichier de
    tests, qui monte un écran sans sa route ; `test/words.test.ts`, hors de ce
    préchargement (registre remis à zéro, catalogue des écrans retenu) : chaque route du
    cadre à chunk (hors vues des plugins) et chaque panneau du cadre (`frame/panels.ts`)
    passent par `worded` (`isWorded`) ; l'accueil et « Clore… » ne rendent rien avant les
    mots, puis leurs mots, jamais une clé ; une lecture perdue du lot fait échouer la
    navigation, la suivante le relit. Vérification : `k-web2-check --base main`
    (1144 tests, 2739 captures sur 2739 : tout, `data/query.ts` étant tiré par l'entrée) ;
    parcours `draft`, `admin` et `accessibility`.
  - choix : la porte par écran (`worded` au retour de `entryScreen`/`recordScreen`) plutôt
    qu'une garde du routeur, qu'un routeur de test contournerait et qui retarderait la
    page de connexion ; « À propos » à la demande essayé puis laissé : −0,3 Ko pour trois
    chunks de plus au premier chargement.
  - reste : le premier poste de la hausse, @kartenn/ui (≈ 131 Ko gzip, partagé entier par
    la carte d'import, un seul chunk pour l'ordre de sa feuille), ne se découpe pas sans
    arbitrage ; l'analyse des clés ne voit pas une clé construite par concaténation
    (`"edit.why." + reason`, si le build la garde telle), ni une clé tenue
    dans une variable venue d'ailleurs que d'une chaîne du premier lot (une donnée du
    serveur), ni une clé d'un catalogue de plugin ; l'avertissement des 296 Ko s'imprime
    dans la sortie de Vitest, que le verdict d'une ligne de `k-web2-check` ne reprend pas ;
    `views/data.ts` réexporte `rowsOf`, `carrierOf`, `carriedRead` de `views/carried.ts` :
    une trentaine d'importeurs à pointer sur `carried.ts`, dont `admin/*` que la 21e
    modifie en ce moment, laissés pour une tâche hors de ces branches.
- [x] 22q. Relevé de la seconde passe réduite et ce qu'elle laisse (session principale, le
  2026-11-14, `bench-web small`, main 24622281, seule sur la machine, les quatre verrous
  libres, charge 1,4) ; entre parenthèses la première passe (main e86f6af1) :
  shell initial 296,1 Ko (306,3 ; budget 300) tenu ; navigation en cache, dix préchargées,
  p75 115,5 ms, médiane 112,2, pire 364,1 (126,5 ; 120,5 ; 313,0 ; budget 100) ; page de
  100 lignes, les vingt pages dessinées, p95 257,2 ms, médiane 106,2, pire 401,8 (test en
  échec avant son p95 ; budget 100) ; arbre de 10⁴ lignes, 28,53 % de trames perdues et
  1 tâche longue de 54,0 ms (28,25 % ; 1 de 51,0 ms ; budget 2 % et 0) : les lignes en
  composants de la 22e-b n'ont rien gagné sur la vraie vue ; fenêtre `bench-a40` écrite,
  31,38 % et 3 tâches longues, la plus longue 362,0 ms, 18 changements vus pour 19 écrits,
  0 lecture `/children` ; **flèche bas dans une grille, p75 104,9 ms, médiane 65,1, pire
  114,6 (89,1 ; budget 100) : budget tenu à la première passe, perdu depuis** — à trouver
  parmi ce qui est entré entre e86f6af1 et 24622281 (mesure différée des lignes et
  `rowKeyList` de la 22f, porte lue par cellule et `rankedRefusal` des 21c et 21h, fente
  `extension` de la 11o2) ; les autres mesures tenues (premier écran 757,5 ms, INP des cinq
  gestes de 40 à 96 ms, flèche dans l'arbre 15,6 ms, tas 25,5 Mo) — verify : la cause de
  la flèche dans une grille nommée et corrigée, Vitest qui borne le travail d'un pas de
  flèche ; passe réduite par la session principale, p75 ≤ 100 ms
  - où : `web/v2/ui/src/components/DataGrid/DataGrid.vue` (pas de flèche, `rowKeyList`,
    `isEditing`, `tameWidgets`), `internal/virtualRows.ts` (`measure`, `commit`),
    `web/v2/shell/src/views/edit/{inline,fields}.ts` (`editability`, raison de cellule),
    `web/v2/e2e/budgets/budgets.spec.ts` (« Arrow down in a grid »)
  - dépend de : 22f ; à faire avec la 22o-b (lignes de la grille en composants), qui vise
    le même rendu
  - cause cherchée le 2026-11-14 (branche `tache-021-22o-b`), case laissée ouverte (passe
    réduite à la session principale) : **le travail d'un pas de flèche n'a pas changé entre
    e86f6af1 et 24622281**. Sonde montée (Vitest, happy-dom, la liste des adresses du vrai
    shell, une page de 50 lignes éditables, `editability` compté par un `vi.mock` de
    `views/edit/fields.ts`, les rendus comptés par composant), passée sur les deux commits
    (un arbre de travail détaché pour e86f6af1) : à chaque pas, sur l'un comme sur l'autre,
    toute la `DataGrid` rendue, la `ListView` (`v-model:active`) et ses enfants, 100 `KLink`
    (deux liens par ligne) et 552 appels d'`editability` ; JS d'un pas 15,4 à 21,0 ms en
    médiane contre 11,6 à 17,1 (trois passages alternés, dans le bruit). Ni la mesure différée
    des lignes (la liste des adresses n'est pas virtualisée : 100 à 200 lignes chargées
    pendant les 50 appuis, `VIRTUAL_ROWS` 200), ni la porte lue par cellule (`useWrites`, un
    `computed`), ni `rankedRefusal`, ni la fente `extension` (aucune colonne de plugin) ne
    s'ajoutent au pas ; aucun CSS, aucun test de banc ni jeu de données n'a changé entre les
    deux. Le p75 tombe sur la frontière des deux modes d'un pas (médiane 65,1, p75 104,9 : la
    moitié des appuis fait défiler la grille, son `window` et la `ListView` en plus) : l'écart
    de 89,1 à 104,9 ms s'explique le plus simplement par la variance du banc, sur un pas qui
    rendait toute la grille. La sonde tourne sous happy-dom : elle prouve que le travail JS
    est inchangé, pas la mise en page ni la peinture, que `ShellFrame.vue` (mesure de la
    surface), `frame/width.ts` et la mesure différée de `virtualRows.ts` ont pu changer
    entre les deux commits. La passe réduite de référence après la 22o-b tranche. Borné
    par la 22o-b : un pas rend deux lignes (`shell/test/list-grid-work.test.ts`, qui échoue
    sur l'ancien code : 100 liens rendus au lieu de 4).
  - note (2026-11-14, 11p9) : `web/v2/e2e/kartenn.toml`, que `serve.sh budgets` sert aussi,
    relève depuis la 11p9 le quota de lecture (`[http.rate] read`, 1 000/s, 4 000 d'un
    coup) ; les passes de banc d'avant ce commit, ces deux-ci comprises, tournaient au quota
    par défaut (50/s, 200, REQ-006-29) et ne sont pas strictement comparables aux suivantes.
  - troisième passe réduite, de référence (session principale, le 2026-11-14, `bench-web
    small`, main 0d968c65 après les 22o-b, 11p9 et 22e-c, seule sur la machine, les quatre
    verrous libres, charge 1,3 ; aucun 429 ni 503 au journal) : shell initial 297,1 Ko
    tenu ; premier écran 697 ms ; navigation en cache, p75 91,2 ms, médiane 82,7, pire
    151,6, tenu : les 22g, 22o et 22o-b closes ; flèche bas dans une grille, p75 27,0 ms,
    médiane 16,4, tenu : la 22q close, l'écart de la deuxième passe ne tenait donc pas à
    une mise en page ajoutée ce jour-là ; flèche dans l'arbre 13,1 ms ; INP des cinq
    gestes 40 à 96 ms ; tas 26,6 Mo ; A40 tenu (19 vus pour 19 écrits, aucune lecture de
    `/children`). Restent en échec : la page de 100 lignes, p95 256,7 ms, médiane 98,5,
    pire 361,2 (budget 100 ; 22f, et sans doute 11p10 : une lecture de propriétés par
    ligne) ; l'arbre de 10⁴ lignes, 23,49 % de trames perdues et 1 tâche longue de 56 ms
    (budget 2 % et 0 ; 22e, dont la cause nommée par la 22e-c, les SVG des lignes et le
    survol, attend un arbitrage B1, B2, B3 ou C).
- [x] 22o. Rendu d'une fiche ouverte depuis le cache : p75 à 126,5 ms et médiane à
  120,5 ms sous CPU ×4 (budget 100 ms), toutes lectures de la route déjà fraîches
  (relevé par la 22g) ; profil du montage de la fiche (onglets, panneau des propriétés,
  parties occupées hors `meta.queries` : occupation, résumés des plugins), puis
  correction — verify : Vitest ou parcours qui borne le travail du montage ; passe
  réduite par la session principale, p75 ≤ 100 ms sur dix navigations préchargées
  - où : `web/v2/shell/src/views/RecordView.vue`, `views/record/*`, `views/routes.ts`
    (`meta.queries`), `web/v2/e2e/budgets/budgets.spec.ts` (« navigations to a prefetched
    target »)
  - dépend de : 22g
  - profil du 2026-11-14 (main 5dffa02a), hors banc : une sonde Playwright temporaire sur
    le jeu des parcours (Chromium, CPU ×4, 1440 × 900, liste des préfixes du domaine
    `production`, cinq fiches survolées puis ouvertes trois fois), trace CDP et
    échantillons du profileur ramenés aux sources par les cartes du build. Fenêtre du
    `pointerdown` au `k:view` : 170 ms en médiane (155 sans la trace) ; un clic de
    Playwright date son `click` pendant la réaction au `pointerdown`, que le banc compte.
    Postes, en ms par navigation (JS échantillonné, mises en page à part) :

    | poste | ms |
    | --- | --- |
    | la grille de la liste au focus du lien pressé (`focusin` : `active` et `focused` changent, tout le `DataGrid` et `ListView` (`v-model:active`) se rendent à nouveau, `cellState`, `rowState`, `stateOf`, `reasonOf`… de chaque cellule en vue) | 30 (tâche de 43) |
    | démontage de la liste (Vue, `removeChild`), dont une mise en page forcée de l'ancienne page dans `removeChild` | 21, dont 11 |
    | cadre et routeur (fil d'Ariane, barre latérale, `RouterView`, garde des lectures) | 24 |
    | montage de la fiche par Vue (composants et DOM) | 15 à 18 |
    | mesures du `DataGrid` des propriétés à son montage (`readEstimate`, `measureKey` : `getComputedStyle`, `offsetLeft`) | 6, plus 8 de mise en page forcée |
    | requêtes des lectures hors route (occupation, prochaine valeur libre, écarts), créées au montage | 6 |
    | options des lectures, faites trois fois chacune (`defaultQueryOptions`, hachage de la clé) | 2 |
    | mesure de la surface du cadre au changement de page (`scrollHeight`, mise en page forcée) | 2,5 |
    | éléments des plugins (`ExtensionElement`, résumé d'`example`) | 2 |

    Réponses aux questions de la tâche : seul l'onglet actif est monté (`Tabs`,
    `unmount-on-hide`) ; le panneau des propriétés pèse par son `DataGrid` (mesures
    ci-dessus) ; les emplacements de plugins, 2 ms ; les mots des écrans sont déjà lus à la
    navigation mesurée (`lazyCatalog` garde sa promesse, aucune requête, des microtâches
    seulement). La même ouverture au clavier (focus sur le lien, Entrée), sans la réaction
    au `pointerdown`, prend 94 ms en médiane contre 149 à la souris : l'écart est la grille
    de la liste. Une trace avec les piles des minuteurs (`devtools.timeline.stack`) gonfle
    `setTimeout` et `clearTimeout` (45 ms, 1 ms sans elle) : profil refait sans.
  - code fait le 2026-11-14, case laissée ouverte (passe réduite à la session
    principale) : les lectures hors route du panneau d'occupation et de l'encart des écarts
    partent une trame peinte après le montage (`views/record/painted.ts`, `usePainted` :
    une trame, puis une tâche) ; ce que le cache tient s'affiche aussitôt (une requête pas
    encore activée garde ses données). `useQuery` et `useInfiniteQuery`
    (`data/query.ts`, `watchedOptions`) font les options une fois au montage, puis à
    chaque changement. La surface du cadre (`ShellFrame.vue`) n'est plus mesurée quand le
    routeur y met une page : la première notification de l'observateur de tailles, après
    la mise en page, la mesure. Gain, mesuré dans un même passage (sonde, sans trace,
    l'ancien et le nouveau chemin alternés par tour sur les mêmes fiches) : 102,5 → 94 ms
    en médiane au clavier, 158,5 → 149 à la souris (plus bruité). Preuve :
    `test/mount-work.test.ts` (le panneau d'occupation monté n'a rien lu avant sa trame,
    puis lit ses deux lectures ; tenues en cache, elles s'affichent aussitôt sans relecture ;
    les options d'une lecture faites une fois au montage, une fois par changement), qui
    échoue sur l'ancien code. Passes réduites filtrées (`bench-web small -g "prefetched
    target"`, indicatives) : avant, p75 308,4 ms, médiane 263,5, pire 485,6, sous la charge
    de deux autres agents (bancs et parcours) ; après, créneaux libres au départ et à
    l'arrivée, p75 123,6 ms, médiane 111,5, pire 210,6, dix préchargées, CLS 0,001 (la
    référence calme de la session principale : 126,5, 120,5, 313,0).
  - coût : une fiche dont l'occupation n'est pas en cache la reçoit une trame et une tâche
    plus tard (≈ 20 à 30 ms sous CPU ×4), sous le même squelette ; « … » de la prochaine
    valeur libre, de même.
  - choix : lectures hors route différées plutôt qu'ajoutées à `meta.queries` (chaque
    survol de la liste lirait quatre chemins au lieu de deux, et la garde attendrait
    l'occupation d'un conteneur à chaque navigation non préchargée) ; une trame puis une
    tâche, pour que la requête parte après la peinture et non dans la trame mesurée ;
    `VlanOccupancyPanel` laissé tel quel (non mesuré) ; les mesures du `DataGrid` au
    montage laissées : les ôter repousserait la mise en page après la marque sans
    avancer la peinture.
  - reste : le budget n'est pas tenu par la fiche seule ; ses deux premiers postes sont
    dans la liste quittée, 22o-b et 22o-c. Mesures du `DataGrid` au montage
    (`readEstimate`, `measureKey`) et `Presence` de reka-ui dans `TabsContent`
    (`getComputedStyle(…).animationName`, ≈ 4 ms) : mises en page forcées pendant le
    montage, à reprendre avec la 22o-b si elle ne suffit pas. Parcours `records` : trois
    passages complets, chacun un échec différent dans les longues listes (« past ten
    pages… » sous Firefox puis Chromium, « a long list keeps a few dozen rows… » sous
    Firefox), le test seul passant trois fois sur trois ; un passage avec l'ancien
    `data/query.ts` échoue de même : instabilité de ces deux parcours sous deux
    navigateurs en parallèle, antérieure à la tâche, à suivre à part.
  - [x] 22o-b. La grille d'une liste ne se rend pas en entier quand on presse un lien de
    ses cellules : au `focusin` d'un appui de pointeur, `active` et `focused` changent et
    tout le `DataGrid` se rend à nouveau, avec la `ListView` qui le lie (`v-model:active`),
    ≈ 30 ms de JS sous CPU ×4 pour un clic qui quitte la page — verify : Vitest du
    `DataGrid` (un appui de pointeur sur un lien de cellule ne rend la grille qu'une fois
    l'appui fini, le point d'arrêt de tabulation et l'erreur ouverte suivent ; le clavier
    inchangé) ; scripts clavier de la grille ; parcours `records` ; passe réduite par la
    session principale, p75 ≤ 100 ms
    - où : `web/v2/ui/src/components/DataGrid/DataGrid.vue` (`onFocusin`, `tabindex`,
      `current`, `errorShown`, gabarit des cellules vers la ligne 1189),
      `web/v2/shell/src/views/ListView.vue` (`v-model:active`, vers la ligne 366)
    - options : différer la mise à jour d'`active` et `focused` d'un appui de pointeur
      jusqu'après son `click` (une tâche, annulée au démontage : un clic qui navigue ne
      rend rien), ou ne lire le point d'arrêt et l'état de chaque cellule que par ligne
      (composant de ligne, `computed` par ligne) ; la première est locale, la seconde
      profite aussi aux flèches
    - dépend de : 22o
    - code fait le 2026-11-14 (branche `tache-021-22o-b`), case laissée ouverte (passe
      réduite à la session principale) : la seconde option, arbitrée par la session
      principale. Chaque ligne du corps de la `DataGrid` est un composant
      (`DataGrid/DataGridRow.vue`, contexte `DataGrid/row.ts`, comme `TreeGridRow` de la
      22e-b) : l'état partagé de la grille (point d'arrêt, éditeur, erreur ouverte,
      sélection, ligne fraîche) lu par la clé de la ligne dans des `computed` qui gardent leur
      valeur, si bien qu'un pas de focus — flèche, appui sur un lien (`focusin`), Entrée —
      rend la ligne quittée et la ligne atteinte, jamais les autres ; chaque état de l'hôte
      demandé une fois par cellule (`cellsOf` : état, éditeur, raisons, erreur, extension ;
      l'état de la ligne une fois par ligne). Les emplacements de l'hôte (`cell`, `editor`,
      `extension`) sont transmis à la ligne par le gabarit de la grille : un hôte compilé
      (`ListView`) laisse les lignes, une fonction de rendu qui refait ses emplacements les
      rend avec la grille, comme Vue tout enfant ; aucun compteur. Les widgets des cellules
      sortent de la tabulation au montage de la grille, puis par son observateur des
      mutations (`useExtensionControls`, désormais avec ou sans colonne d'extension : un lien
      qu'un composant de l'hôte montre seul, après une lecture, `ObjectName`), jamais par un
      balayage de la grille à chacun de ses rendus. Une ligne montée avec une erreur ouverte
      (revenue dans la fenêtre sous le focus) se rend une fois de plus après son montage, pour
      que la bulle trouve sa cellule. La table des identifiants de raisons (`internal/cellReasons.ts`) ne garde que
      les phrases des lignes rendues (un compteur pour les nouvelles, jamais un numéro
      repris). Preuves : `ui/test/data-grid-rows-mounted.test.ts` (au montage, deux appels
      par cellule, la ligne et la table des phrases ; la grille rendue seule, aucune ligne ;
      un pas de flèche et un appui sur un lien, deux lignes ; sélection, éditeur, erreur, la
      seule ligne visée ; une ligne non rendue montre la marque de brouillon, la raison
      changée puis partie, la porte fermée puis rouverte, la langue ; un seul arrêt de
      tabulation, liens hors tabulation ; hôte compilé `test/fixtures/GridRowsHost.vue`) et
      `shell/test/list-grid-work.test.ts` (la liste des adresses : un pas de flèche et un
      appui sur un lien, au plus deux lignes, quatre liens et 18 appels d'`editability`, au
      lieu de toute la grille, 100 liens et 552 appels). JS d'un pas dans la sonde : 3,3 ms en
      médiane contre 15 à 20. Passes réduites filtrées (`bench-web small -g …`, indicatives :
      l'agent 11p9 tenait les deux créneaux et le verrou `pg` aux deux dernières, charge 3 à
      6) : flèche bas dans une grille p75 28,6 ms, médiane 15,7, pire 41,2 (104,9 ; 65,1 ;
      114,6 à la seconde passe de la session principale), créneaux libres au départ ;
      navigation en cache p75 85,9 ms, médiane 79,4, pire 232,6, CLS 0,001 (115,5 ; 112,2 ;
      364,1) ; page de 100 lignes p95 238,2 ms, médiane 96,7, pire 361,4 (257,2 ; 106,2 ;
      401,8) : **budget toujours dépassé**, l'arrivée d'une page monte ses cent lignes et la
      table des phrases cachées relit toutes les cellules (22f). Shell initial 297,0 Ko
      (296,1 ; budget 300).
    - choix : composant de ligne plutôt que différer `active` après le `click` (la
      seconde option, qui profite aux flèches et aux pages) ; emplacements transmis par le
      gabarit plutôt que lus par le contexte (aucune ligne périmée quand l'hôte refait ses
      emplacements) ; le commentaire du gabarit hors de la chaîne `v-if`/`v-else` (en
      développement, il faisait de chaque ligne un fragment patché en entier :
      `DEV_ROOT_FRAGMENT`) ; la `ListView` laissée telle quelle (son pied lit `active`, elle
      se rend à chaque pas, sans ses lignes) ; l'observateur des mutations pour toute
      `DataGrid` (relecture d'accessibilité : un widget qu'un composant de l'hôte rend seul
      gardait sinon un arrêt de tabulation ; la 11o le réservait aux colonnes d'extension) ;
      les fonctions de l'hôte (`cellState`, `rowState`, `editable`, raisons) lisent un état
      réactif, comme un `computed` (dit dans `DataGrid.vue`).
    - reste : la présence d'un emplacement de l'hôte (`editor`, `extension`) est lue au rendu
      de la ligne, non réactive (un hôte qui en ajoute un après le montage) ; la table des phrases cachées (`useCellReasons.reasons`) relit toutes les
      cellules rendues à chaque changement d'un état de l'hôte (un commit, une page) ; la
      `TreeGrid` garde deux écarts que la 22o-b évite (son `cells` lit `isStop` directement :
      chaque ligne se rend à chaque pas de focus ; un commentaire entre `v-if` et `v-else`
      devant `TreeGridRow`, sans effet en production) : à la 22e-c.
  - [ ] 22o-c. Mise en page forcée au démontage d'une liste : quand le routeur remplace la
    liste des préfixes par une fiche, `removeChild` de Vue force une mise en page de
    l'ancienne page (11 ms sous CPU ×4), au clavier comme à la souris ; ni le focus, ni la
    sélection, ni le survol (essayés : retirés avant, elle reste) — verify : trace de la
    navigation sans `Layout` sous `removeChild`
    - où : catégorie `disabled-by-default-devtools.timeline.invalidationTracking` d'une
      trace CDP ; `web/v2/shell/src/views/ListView.vue`, `DataGrid.vue` (ce que la page
      tient : grille défilante, en-tête collant, observateurs)
    - dépend de : 22o
- [ ] 22p. Place des lignes d'une liste passé dix pages : le shell ne passe jamais
  `rowOffset` à `DataGrid` ; la première page lâchée (`MAX_PAGES`), `aria-rowindex`
  repart de 2 pour la première ligne gardée et `aria-rowcount` dit les lignes gardées
  (`rowCount` = `rows.length` une fois la liste complète) : un lecteur d'écran annonce une
  place fausse (WCAG 1.3.1, 4.1.2). Passer le nombre de lignes lâchées en tête (somme des
  pages lâchées, comme `droppedRows`) et le total connu — verify : Vitest du calcul de
  l'écart depuis les pages du cache ; parcours `records` « past ten pages… » étendu :
  après la onzième page, la dernière ligne en vue garde son `aria-rowindex` (1 001) et
  `aria-rowcount` reste le total
  - où : `web/v2/shell/src/views/ListView.vue` (`:row-count`, vers la ligne 377),
    `ChangesetsView.vue` (vers 156), `AuditView.vue` (vers 214), `ImportsView.vue`
    (vers 91), `views/list.ts` (`rows`, `complete`), `prefetch/nextPage.ts`
    (`droppedRows`), prop `rowOffset` de `DataGrid` (déjà là, état `paged` du catalogue)
  - dépend de : 22f
- [x] 22h. Shell initial à 302,2 Ko gzip (budget 300 Ko) : trouver les 2,2 Ko en trop
  (import statique devenu lourd, module à charger à la demande) — verify :
  `web/v2/shell/scripts/shell-size.ts` ≤ 300 Ko
  - où : `web/v2/shell/scripts/shell-size.ts`, `web/v2/shell/src/main.ts` et ses imports
    statiques
  - dépend de : 22a
  - fait le 2026-11-10 : 302,6 Ko avant (main c3fb056d), 295,6 Ko après. Mesure module
    par module (build avec cartes de sources, octets gzip par source) : le premier poste
    hors bibliothèques était les catalogues JSON du shell, ≈ 28 Ko gzip dans `index.js`,
    FR et EN des quatre espaces de mots ; celui de l'administration (`admin`, ≈ 7 Ko) ne
    sert qu'aux écrans de l'espace et grossissait à chaque tâche 15. Il sort de
    `createShellI18n` : `lazyCatalog` (`shell/src/i18n/index.ts`) le lit à la première
    demande et le fusionne dans l'i18n du shell ; chaque écran et chaque détail de
    `admin/routes.ts` l'attend avec son propre chunk (`worded`), si bien qu'aucun écran
    ne s'affiche sans ses mots. Budget et mesure inchangés. Vérification :
    `k-web2-check --base main` (954 tests, 2719 captures sur 2719 : tout, `admin/routes.ts`
    étant tiré par l'entrée).
  - choix : les deux langues dans un seul chunk (`admin/i18n/index.ts`), pour qu'un
    changement de langue n'ait rien à lire ; un catalogue lu avant la création de l'i18n
    (tests) est gardé et fusionné à sa création ; les catalogues `views` et `changesets`
    restent dans l'entrée (le cadre et le panneau du brouillon les lisent).
  - reste : la marge n'est que de 4,4 Ko ; le catalogue `views` (≈ 11 Ko gzip) pourrait
    suivre le même chemin pour ses clés propres aux écrans, s'il fallait de la place.
  - reste (revue) : `lazyCatalog` (`i18n/index.ts` vers la ligne 97) sans test unitaire,
    le nouvel essai après un échec et la fusion d'un catalogue lu avant la création n'étant
    prouvés que par les captures ; rien dans `test/build.test.ts` n'empêche le retour du
    catalogue `admin` dans l'entrée (un import statique hors de `src/admin/` le ferait
    revenir, vu du seul budget global) ; `shell` (vers la ligne 132) ne retient que la
    dernière instance créée, sans effet dans l'application.

- [ ] 21. Accessibilité étendue : parcours clavier complet (palette, barre repliée,
  panneau du brouillon, édition en place), reflow 320 px des écrans de consultation,
  message d'édition sous 1024 px — REQ-021-29 — verify : `k-web-e2e` ; revue
  `a11y-auditor` avec lecteur d'écran en mode navigation
  - où : `web/e2e/journeys/accessibility.spec.ts`
  - dépend de : 16
  - étendue le 2026-11-03 (A33) : les gardes du parcours `smoke` de l'ancien shell (une
    violation axe et un script en ligne font chacun échouer un parcours) prouvées par un
    test des fixtures de `web/v2/e2e/`
  - découpée le 2026-11-03 en 21a et 21b
- [x] 21a. Accessibilité, portage et gardes : le parcours `accessibility` de l'ancien shell
  porté sous `web/v2/e2e/journeys/` (« every journey runs axe », « every screen of the shell
  passes axe » avec les listes `/ranges`, `/entities`, `/sites`, « the tree, a table, an
  action menu and a dialog by the keyboard alone ») ; les gardes des fixtures prouvées par
  un test (une violation axe et un script en ligne font chacun échouer un parcours) —
  verify : `k-web2-e2e journeys/accessibility.spec.ts` sous les deux navigateurs ; les six
  lignes de `web/v2/PARITE.md` qui nomment la 21 cochées ; `parity.test.ts` sans `todo` de
  parcours sans pendant
  - où : `web/e2e/journeys/accessibility.spec.ts` et `smoke.spec.ts` (anciens),
    `web/v2/e2e/fixtures/` (gardes axe et CSP), `web/v2/e2e/journeys/`
  - dépend de : 16
  - fait le 2026-11-03 — `web/v2/e2e/journeys/accessibility.spec.ts` : la couverture axe
    de chaque parcours (`served` y entre, axe sur sa page de connexion) ; chaque entrée des
    espaces du cœur (lue de `shell/src/frame/spaces.ts`, `/ranges`, `/entities`, `/sites`
    compris), la fiche du premier objet de chaque collection du référentiel, des VLAN et des
    changesets, l'aide des raccourcis, « À propos » et `/login`, chacun sous axe, aucune
    violation trouvée ; au clavier seul, l'arbre (le conteneur large ouvert, son premier
    enfant ouvert), le menu « Actions » de la fiche et le dialogue de l'action du plugin
    `example`, puis la grille des adresses ; les gardes : `expectAccessible` rejette sur
    `image-alt`, et un script en ligne fait échouer le parcours (`test.fail`). Gardes
    retirées à l'essai, les quatre tests des gardes échouent sous les deux navigateurs.
    `PARITE.md` : les six lignes cochées ; `parity.test.ts` sans plus aucun `todo`
  - choix : la garde axe prouvée par le rejet de `expectAccessible` (règle nommée) plutôt
    que par `test.fail`, plus précis ; la garde CSP, qui s'exerce à la fin de la fixture
    `page`, par `test.fail`, l'erreur de console du refus tolérée dans ce test pour que la
    garde CSP seule le fasse échouer (sinon la garde de la console le masque) ; écrans lus
    de `spaces.ts` par expression régulière plutôt qu'importés (le module tire
    `@kartenn/ui`, hors des dépendances de `web/v2/e2e`)
  - passe de corrections (revue) : la garde CSP déclare l'échec attendu (`test.fail()` dans
    le corps) seulement une fois le script refusé, sa violation reçue et le script non
    exécuté, de sorte qu'un autre échec (connexion cassée, CSP qui laisse passer le script)
    échoue vraiment ; `served` vérifie l'absence de `'unsafe-inline'` dans chaque
    directive `script-src*` ; les entrées de `spaces.ts` comptées aussi par leurs icônes,
    une entrée dont le chemin n'est pas lu fait échouer le parcours ; sous axe aussi la fiche
    d'un brouillon (créé vide puis supprimé), celle d'un groupe d'administration et la vue
    `/x/example/overview`. Écrans de l'ancien parcours sans pendant : `/search?q=` (la
    palette) et `/prefixes/new` (la création, un panneau), retirés par 021/A30
- [x] 21b. Accessibilité étendue, parcours clavier et reflow : parcours clavier complet
  (palette, barre repliée, panneau du brouillon, édition en place) ; reflow à 320 px des
  écrans de consultation (le message d'édition sous 1024 px : 21c à 21f) — REQ-021-29 —
  verify : `k-web2-e2e` (parcours `accessibility`) ; revue `a11y-auditor` avec lecteur
  d'écran en mode navigation
  - où : `web/v2/e2e/journeys/accessibility.spec.ts` (de la 21a), `web/v2/shell/src/frame/`
  - dépend de : 21a
  - découpée le 2026-11-14 : le message d'édition sous 1024 px passe en 21c à 21f (il
    ferme chaque surface d'écriture, une vingtaine de fichiers, `TopologyView.vue` compris)
  - fait le 2026-11-14 — `web/v2/e2e/journeys/accessibility.spec.ts` : au clavier seul,
    une modification en place (la grille des préfixes d'un domaine propre, un arrêt de
    tabulation, Entrée ouvre l'éditeur, Entrée range au brouillon, focus rendu à la
    cellule), le panneau du brouillon (le compteur par Maj+Tab, Entrée l'ouvre sur son
    onglet, « Retirer l'opération 1 » par Tab, focus sur la phrase du brouillon vide,
    Échap ferme et rend le focus au compteur), la palette (Ctrl+K, Échap rend le focus,
    une page par son nom, flèches, Entrée), la barre repliée par sa bascule
    (`aria-expanded`, liens nommés dans le rail, Entrée suit l'un, la bascule la rouvre),
    axe à chaque étape ; à 320 px, chaque écran de consultation (entrées de `spaces.ts`,
    fiche du premier objet de chaque collection, vue du plugin `example`, aide, « À
    propos », `/login`) sans rien qui défile de côté hors des grilles (`sideways` : la
    page, toute région `overflow-x: auto | scroll` hors `grid`/`treegrid`). Seul écart
    trouvé, la liste d'onglets des fiches, qui défilait : `Tabs` va à la ligne
    (`ui/src/components/Tabs/Tabs.vue`, `flex-wrap`), lignes d'inventaire de la 020 et de
    la skill. Sous les deux navigateurs
  - choix : une boîte qui coupe (`overflow: hidden` ou `clip`, celle du cadre comprise)
    ne compte pas comme défilement, mais tout contrôle focalisable montré qui dépasse la
    fenêtre ou une boîte qui coupe est compté comme contenu perdu ; seul un texte coupé
    par une ellipse dans sa propre boîte (infobulle) est admis ; les onglets vont à la
    ligne plutôt que de défiler (1.4.10 ne tolère le défilement en deux dimensions que
    pour les tableaux de données), un libellé plus large que la liste y va à la ligne lui
    aussi ; Alice pour le reflow (elle lit chaque espace, l'administration comprise)
  - passe de relecture (`ui-reviewer`) : les rangées d'onglets se touchent (`gap: 0
    var(--k-space-1)`), le soulignement de l'onglet choisi reste sur son libellé
  - passe de corrections (revue et audit) : `sideways` compte aussi les contrôles
    focalisables coupés par une boîte `hidden`/`clip` ou hors de la fenêtre, et se prouve
    lui-même (un bouton coupé, un autre hors de la fenêtre, nommés) ; à 320 px aucun écran
    n'en montre, la barre d'état (`.k-frame__bar-end`) comprise ; `Tabs` : onglet borné à
    la liste (`max-inline-size: 100%`), son libellé va à la ligne, état de catalogue
    `narrow` et son script clavier (`e2e/components/tabs.spec.ts` : plusieurs rangées,
    chaque onglet dans la liste, les flèches passent d'une rangée à l'autre) ; le
    brouillon du parcours clavier supprimé en `finally` ; la région `status` de la liste
    (« Ajouté au brouillon : … ») et celle de la palette (son compte) vérifiées
  - reste : `aria-activedescendant` de la palette et le focus après Entrée (à
    vérifier) ; le nom du repère « Panneau droit » ; `.k-cat__switches` du catalogue
    déborde à 320 px
- [x] 21c. Édition réservée aux largeurs ≥ 1024 px, la porte et son message : une seule
  porte d'écriture qui lit la largeur du cadre (classe `narrow` ou `compact`, sous
  1024 px) à côté de la période, et un motif `narrow` (« Modification à partir de 1024 px
  de large : élargissez la fenêtre pour modifier. ») dit là où le motif `past` l'est
  aujourd'hui : cellules en place (`aria-readonly`, description, pied `k-keyboard-hint`),
  touches `c` et `e` et commandes de la palette (statut poli), panneaux d'édition, de
  clôture et de changement de parent — REQ-021-29, Q-021-12 — verify : Vitest de la porte
  (largeur × période) ; parcours `accessibility` à 768 et 390 px : une cellule dit le
  motif, `e` le dit, le panneau d'édition ne s'ouvre pas ; à 1024 px, l'édition revient
  - où : `web/v2/shell/src/time/index.ts` (`writesOf`, `Writes`), `frame/ShellFrame.vue`
    (`width`, vers la ligne 220 ; la largeur publiée par un module du cadre),
    `shortcuts/view.ts` (`play`, `runnable`), `views/edit/inline.ts` (`editability`,
    motif `past` vers la ligne 176), `views/edit/properties.ts`, `action.ts`, `moves.ts`,
    `ClosePanel.vue`, `EditHint.vue`, `EditPanel.vue` (vers la ligne 492) ; catalogue
    `views/i18n/fr.json` (`edit.why`) ; modèle : le motif `past`
  - dépend de : 21b ; 11o est fusionnée (`TopologyView.vue` lit `inline.ts`)
  - fait le 2026-11-14 — la porte : `writesOf(time, width)` (`time/index.ts`) rend
    `refused: "past" | "narrow" | null` à côté de `quickEdit` et `draft`, `useWrites()` la
    lit à la période de la route et à la classe de largeur que publie le cadre
    (`frame/width.ts` : `frameWidth`, `followWindow` appelé par `ShellFrame.vue`, qui y
    perd ses propres requêtes ; `wide` sans cadre), `refusalWords` en dit la phrase
    (`edit.why.narrow`, fr et en). Dite là où `past` l'est : `editability` (`narrow` dans
    `NotEditable`), donc cellules de `inline.ts` et `properties.ts` (`aria-readonly`,
    description, infobulle, `whyWords`, pied `EditHint.vue`) et celles de l'arbre et du
    contenu qui les lisent ; `c`, `e` (`play`, `runnable` prennent la porte) ; la palette
    (`offered` : la commande gardée sous la largeur seule, son lancement dit le motif par
    le statut de la vue) ; panneaux d'édition, de clôture et de changement de parent
    (`action.ts` : les deux actions refusées avec le motif, Ctrl+Entrée le dit dans le
    statut du panneau) ; « Changer de parent… » (`moveEntryRefusal`, mots `dated` et
    `narrow`) et le glisser de l'arbre (`canMove`). Un éditeur ouvert quand la fenêtre
    passe sous 1024 px reste ouvert, sa saisie et le focus gardés : Entrée y est refusée
    par `validate` (la bulle et l'`alert` de la grille), et range au brouillon une fois la
    fenêtre élargie ; la ligne du motif (Ctrl+Entrée) reste aussi, `confirm` dit le motif ;
    un panneau ouvert reste ouvert, ses champs gardés. Tests : `test/writes.test.ts`
    (largeur × période, phrase, `followWindow` sur des requêtes simulées, `useWrites`),
    `edit.test.ts` (`editability`), `shortcuts.test.ts` (`play`, `offered`),
    `moves.test.ts`, `close.test.ts` (panneau ouvert puis rétréci, puis élargi) ; parcours
    `accessibility` à 768 et 390 px (cellule, pied, Entrée, `e`, la palette, aucun panneau)
    puis à 1024 px (l'édition revient, l'éditeur ouvert garde saisie et focus à 768 px)
  - choix : le passé avant la largeur (élargir n'ouvre pas une date passée), la largeur
    avant le droit manquant (comme le passé) ; sans largeur, `writesOf` lit la période
    seule (la simulation d'une clôture, une lecture) ; la palette garde « Créer… » et
    « Modifier… » quand la largeur seule les retient (`offered` lit la période sans la
    largeur : absents hors du présent et sans le droit, à toute largeur, comme avant) ; un
    déplacement refusé par la largeur ne marque pas le nœud « refusé » ; pas de garde dans `openEdit` : un panneau ouvert par une surface des 21d à 21f
    sous 1024 px montre ses actions refusées, sans perte
  - pour 21d à 21f : remplacer `computed(() => writesOf(time.value))` par `useWrites()`
    et `t("…why.past")` par `refusalWords(writes, t)` (ou le motif `narrow` de leur
    catalogue) au point d'action ; modèle de test : `test/writes.test.ts`, `setFrameWidth`
    pour un test monté ; `RecordView.vue` passe déjà les mots `narrow` à
    `moveEntryRefusal`, sa porte reste la période seule jusqu'à la 21f. La garde
    `PERIOD_ONLY` de `test/writes.test.ts` compte, dans `src/` hors `_ui/` et hors
    `time/index.ts`, commentaires ôtés, tout fichier qui n'appelle pas `useWrites()` et
    décide sur la période : `writesOf` sans largeur, un `present` (`x.present`,
    `present:` passé à un modèle) ou un test d'`asOf` (`!x.asOf`, `x.asOf === null`,
    `x.asOf ?`) ; une date passée à une lecture (`asOf: time.value.asOf`) ne compte pas.
    Elle échoue sur un fichier hors de sa liste fermée, ou sur un fichier listé qui ne
    décide plus ainsi : chaque fichier y est rattaché à sa sous-tâche (21d, 21e, 21f) et
    la quitte quand elle le reprend ; `kept` pour la simulation d'une clôture et
    `offered`. Elle ne voit pas une décision prise sur un `present` reçu nu (paramètre
    d'une fonction pure) : l'appelant qui le passe est compté
  - passe de relecture (`ui-reviewer`, `a11y-auditor`) : la liste `PERIOD_ONLY` (un
    `writesOf` sans largeur ne compile pas moins, il se compte) ; le statut poli d'une vue
    (`useStatus`, `say.ts`) se vide quand la porte change (« élargissez la fenêtre » n'y
    reste pas une fois la fenêtre large)
  - reste : la fiche (« Modifier », « Actions », `blockedBy` de `RecordView.vue`) et le
    « + » de l'arbre (`TopologyView.vue`, `addRefusal`) disent encore `past` et s'ouvrent
    sous 1024 px (21f), le brouillon de même (21d) ; un éditeur ouvert ne dit pas le
    rétrécissement de la fenêtre avant Entrée (une annonce au passage sous 1024 px, à
    peser) ; l'annonce du statut après une commande de la palette, et le motif d'une
    cible refusée au glisser ou par « Déplacer vers… », à écouter au lecteur d'écran ; la
    course de `properties.ts` (`onCommit`, vers les lignes 668-673) : la fenêtre rétrécie
    pendant `await explain()`, la saisie est perdue, rien n'est écrit (dire le motif et
    garder la valeur dans la cellule)
  - passe de corrections (revue) : la garde `PERIOD_ONLY` élargie aux décisions par
    `present` et `asOf`, commentaires ôtés, chaque fichier rattaché à sa sous-tâche ;
    `offered` lit la période (sous `valid_at` seul et à l'étroit, une action non datée
    reste absente) ; un refus `narrow` du glisser ne marque pas le nœud ; 21g écrite
- [x] 21d. Édition réservée aux largeurs ≥ 1024 px, brouillon et changesets : le motif
  `narrow` de la 21c sur la soumission, les opérations, « Mes brouillons », les
  soumissions perdues et les décisions d'un changeset — REQ-021-29 — verify : Vitest de
  `changesets/model.ts` (motif `narrow` à côté de `past`) ; parcours `accessibility` à
  768 px : « Appliquer » du brouillon refusé avec le motif
  - où : `web/v2/shell/src/draft/state.ts` (`writable`, vers la ligne 125 ; motif
    `drafts.why.past` vers la ligne 181), `draft/DraftSubmit.vue`,
    `draft/DraftOperations.vue`, `draft/DraftsView.vue`, `draft/LostSubmissions.vue` ;
    `changesets/model.ts` (motif `past`, vers la ligne 135),
    `changesets/ChangesetView.vue` (vers la ligne 121) ; catalogue
    `changesets/i18n/fr.json`, `en.json` (`drafts.why`, `changesets.why`) ; chaque fichier
    repris quitte `PERIOD_ONLY` (`web/v2/shell/test/writes.test.ts`, entrées `21d`)
  - dépend de : 21c
  - fait le 2026-11-14 — le brouillon lit la porte : `useDraftQueries` (`state.ts`) prend
    `useWrites()` et rend `refused` à côté de `writable`, `draftWhy` en dit la phrase
    (`drafts.why.past`, `drafts.why.narrow`, fr et en) ; dite là où `past` l'est : le
    bouton partagé du pied (« Appliquer », « Soumettre en revue », « Enregistrer le
    brouillon »), le motif et les dates (`CommitForm` et `DateTimeField` en lecture seule
    avec leur raison, saisie et focus gardés), « Réessayer » (pied et `LostSubmissions`),
    « Retirer » et « Modifier » d'une opération (`DraftOperations`), « Nouveau brouillon »
    (en-tête et état vide), « Renommer… » et « Supprimer » (`DraftsView`). Les décisions
    d'un changeset : `reviewActions` prend `refused` (`past` | `narrow` | `null`) au lieu de
    `present`, la porte avant `held`, `author` et le droit manquant ; `ChangesetView` le
    calcule (`past` hors du présent, `valid_at` seul compris, sinon la porte), motif
    `changesets.why.narrow`. Un dialogue ouvert quand la fenêtre rétrécit garde sa saisie :
    sa confirmation porte la raison, Entrée la dit dans le dialogue (renommage, rejet, la
    largeur avant le motif vide) ; la suppression et l'annulation, fermées par leur
    confirmation, la disent par un toast. Les cinq entrées `21d` ont quitté `PERIOD_ONLY`.
    Tests : `changesets.test.ts` (`reviewActions` sous `narrow` ; montés : le pied rétréci
    puis élargi, motif gardé et envoyé ; « Mes brouillons » et son dialogue de renommage ;
    le rejet d'un changeset, motif gardé) ; parcours `accessibility` à 768 px (« Appliquer »
    refusé avec le motif, atteint et pressé au clavier, rien de soumis ; revenu à 1024 px) ;
    parcours `changesets` (l'action repliée dans « … » à 420 px dit la largeur, plus
    l'auteur)
  - choix : le motif et les dates du pied passent en lecture seule sous 1024 px, comme
    sous `as_of` (la saisie reste, le focus aussi ; élargie, elle redevient modifiable) ;
    sous 1024 px l'action choisie du bouton partagé reste celle des droits (« Appliquer »
    refusé, non « Enregistrer »), sous `as_of` rien ne change ; l'avis « Lecture seule » en
    tête des opérations reste au seul `past` (pas d'avis permanent de la largeur) ;
    `drafts.why.narrow` reprend mot pour mot `edit.why.narrow`, `changesets.why.narrow` dit
    « Décision » comme `past` dit « rien ne se décide » ; une décision sous `valid_at` seul
    reste refusée `past` (elle s'applique d'emblée), même à l'étroit ; l'action « Nouveau
    brouillon » de l'état vide est montrée refusée au lieu d'être masquée (sous `as_of`
    aussi)
  - passe de relecture (`ui-reviewer`, `a11y-auditor`) : le refus d'un dialogue ouvert dit
    dans le dialogue (un toast reste sous la couche modale), la largeur avant le motif vide
    du rejet ; l'action de l'état vide de « Mes brouillons » montrée refusée
  - reste : aucune annonce au passage sous 1024 px quand on tape dans le motif du pied (le
    champ passe en lecture seule, sa raison lue au focus suivant ; même question qu'à la
    21c pour un éditeur ouvert) ; la suppression et l'annulation demandent leur
    confirmation avant d'être refusées si la fenêtre rétrécit dialogue ouvert (toast après
    fermeture) ; une entrée de menu désactivée avec sa raison (`Menu`, Reka) à écouter au
    lecteur d'écran
- [x] 21e. Édition réservée aux largeurs ≥ 1024 px, administration, groupes et rôles : le
  motif `narrow` de la 21c sur chaque action d'écriture des groupes et des rôles —
  REQ-021-29 — verify : Vitest de `admin/model.ts` (`groupRefusals`, `roleRefusals`) ;
  parcours `accessibility` à 768 px : « Créer » des groupes refusé avec le motif
  - où : `admin/model.ts` (vers les lignes 337, 351, 389 et 398), `GroupsView.vue` (79),
    `GroupDetail.vue` (116), `RolesView.vue` (110), `RoleDetail.vue`, leurs dialogues ;
    chaque fichier repris quitte `PERIOD_ONLY` (`web/v2/shell/test/writes.test.ts`,
    entrées `21e`)
  - dépend de : 21c
  - découpée le 2026-11-14 : onze fichiers et leurs dialogues débordaient un contexte ;
    les comptes, identités, émetteurs et imports passent en 21e2
  - fait le 2026-11-14 — la porte de l'administration : `useAdminGate()` (`admin/gate.ts`)
    lit `useWrites()` et rend `refused` (`past` hors du présent, `valid_at` seul compris :
    l'administration écrit au présent seulement ; sinon la porte, donc `narrow` sous
    1024 px) et sa phrase (`edit.why.past`, `edit.why.narrow`, sans clé nouvelle).
    `groupRefusals` et `roleRefusals` prennent `refused` au lieu de `present` et les mots
    `narrow` à côté de `past` (`GateWords`). Dite là où `past` l'est : « Créer » des groupes
    et des rôles, « Modifier », « Clore… », « Dupliquer », « Supprimer… », les membres,
    les rôles sur périmètres, les permissions et leurs « Ajouter » ; l'action d'une ligne
    que marque le brouillon (« Annuler le retrait », l'ajout retiré) prend la porte seule
    (elle écrit le brouillon). `c` et `e` disent le motif par un statut poli
    (`useStatus`, `say`, `why` : `refusalSentence`). La porte relue à l'envoi :
    `useAdminDraft` (`add`, `drop`, `replace`) n'envoie rien quand elle refuse et le dit
    par un toast (la clôture et la suppression, fermées par leur confirmation) ; les
    dialogues (`GroupForm`, `RoleForm`, `MemberDialog`, `BindingDialog`, « Ajouter une
    permission » de `RoleDetail`) restent ouverts, leur saisie gardée, la raison dans le
    dialogue (`InlineAlert`) et sur leur bouton. Les cinq fichiers repris ont quitté
    `PERIOD_ONLY`. Tests : `admin.test.ts` (`narrow` après le droit manquant ; sous la
    porte, aucune action offerte que la porte ouverte refuse, le droit manquant restant la
    raison), `admin-gate.test.ts` (monté : « Créer » refusé à l'étroit puis rendu ; le
    droit manquant gardé comme raison ; le dialogue de création ouvert puis rétréci, saisie
    gardée, rien envoyé, élargi puis envoyé) ; parcours `accessibility` à 768 px
    (« Créer » refusé, atteint et pressé au clavier, `c` dit le motif, aucun dialogue ;
    revenu à 1024 px, le dialogue s'ouvre)
  - choix : dans l'administration la porte reste APRÈS le droit manquant, à la place
    qu'y tenait déjà `past` (« refuses by the permission first, then by the past », testé
    depuis la 15a) : la largeur prend exactement le rang du passé, sans changer celui-ci ;
    la porte n'ajoute que des refus (ouverte, `refused: null`, mêmes actions qu'avant) ;
    l'avis « Lecture seule » d'un groupe livré ou plus large reste à la porte ouverte (comme
    sous `past`, pas d'avis de la largeur) ; la simulation d'escalade est demandée à
    l'étroit aussi (une lecture, prête une fois la fenêtre élargie), pas hors du présent ;
    un dialogue ouvert puis rétréci dit la raison dès le rétrécissement (`InlineAlert`
    warn, `role="status"`) plutôt qu'à Entrée : son bouton refusé n'envoie pas le
    formulaire, Entrée dans un champ ne fait donc rien ; l'action d'une ligne marquée
    refusée aussi hors du présent (elle écrit le brouillon, comme « Retirer » de la 21d)
  - passe de relecture (`ui-reviewer`, `a11y-auditor`) : rien de bloquant ; la span de
    statut dans le `Stack` du détail ne prend pas d'écart (absolue, `gap` du flex)
  - passe de corrections (revue) : le rang « raison durable d'abord (droit manquant,
    groupe ou rôle livré, portée plus large), la porte ensuite » est la cible (arbitrage de
    la session principale ; 21h l'étend aux autres écrans). `c` et `e` disent la raison du
    bouton : `allowed` et `why` lisent les refus durables (`refusalsAt(null)`,
    `createRefusal` sans porte), l'action est `lasting` (`ViewAction.lasting`,
    `shortcuts/view.ts` : `play` dit alors son `why` avant la porte ; sans lui, rien ne
    change pour les autres écrans) ; le droit manquant dit « Il faut core.rbac.admin »
    comme `BlockedReason` (clé `admin.why.forbidden`, fr et en) ; la palette garde
    « Modifier le groupe… », « Modifier le rôle… » et « Créer… » quand la porte seule les
    retient (lancée, elle dit la largeur) et les ôte quand un droit manque ;
    `createRefusal` (`model.ts`) seule fonction de « Créer » ; `gated` de `useAdminDraft`
    dit le refus par le statut de la vue sans toasts (`say`). Tests : `admin-gate.test.ts`
    (bouton et touche, `c` et `e`, quatre cas ; la palette, porte seule et droit manquant),
    `shortcuts.test.ts` (`lasting`), `admin.test.ts` (`createRefusal`)
  - reste : 21e2 ; d'ici là, les écrans des comptes, identités et émetteurs qui passent
    par `useAdminDraft` montrent leurs actions offertes sous 1024 px et ne les refusent
    qu'à l'envoi (toast, rien d'écrit) ; « Clore… » d'un groupe et « Supprimer… » d'un
    rôle, confirmation ouverte quand la fenêtre rétrécit, se ferment puis disent le refus
    par un toast (comme la suppression et l'annulation de la 21d : `AlertDialog` n'a ni
    raison ni maintien ouvert) ; Entrée refusée dans un dialogue ne redit pas la raison
    (dite une fois au rétrécissement, et par le bouton)
- [x] 21e2. Édition réservée aux largeurs ≥ 1024 px, administration, comptes, identités,
  émetteurs et imports : le motif `narrow` de la 21e sur leurs écritures — REQ-021-29 —
  verify : Vitest de `admin/access.ts` (`globalRefusal` : `narrow` après le droit
  manquant ; `refused: null`, mêmes refus qu'avant) ; un test monté sur le modèle de
  `test/admin-gate.test.ts` (un dialogue ouvert puis rétréci : saisie gardée, rien
  envoyé) ; parcours `accessibility` à 768 px : « Créer » des comptes de service refusé
  avec le motif
  - **à faire avant la bascule (27)** : trois écritures de l'administration ne passent pas
    par `useAdminDraft` et restent RÉELLEMENT possibles sous 1024 px tant que la 21e2 n'est
    pas faite : `secret.ts:83` et `:89` (création d'un compte de service et nouveau
    secret, POST direct), `IdentityDetail.vue:190` (pseudonymisation, POST direct),
    `ImportDetail.vue` (`steer` : reprise et annulation d'un import)
  - où : `web/v2/shell/src/admin/access.ts` (`globalRefusal`, vers la ligne 54 : `present`
    → `refused`, mots `GateWords` de `model.ts`), `AccountsView.vue` (vers la ligne 91),
    `AccountDetail.vue` (58), `IdentityDetail.vue` (95, 107 ; `pseudonymise` vers la
    ligne 184, un `api.post` direct : la porte relue à l'envoi, refus dit dans son
    dialogue), `IssuersView.vue` (89 ; `attach` vers la ligne 179 et son dialogue),
    `ImportDetail.vue` (`pastRefusal` vers la ligne 89, `steer` vers la ligne 108, appel
    direct), `SecretDialog.vue` et `secret.ts` (`run`, vers la ligne 49, POST direct) ;
    modèle : `useAdminGate` (`admin/gate.ts`), `GroupForm.vue` (`held`, `InlineAlert`,
    bouton à raison), `GroupsView.vue` (`createRefusal`, `c` : `allowed`/`why` durables,
    `lasting`, `say`) ; `useAdminDraft` relit déjà la porte à l'envoi ; le rang : la raison
    durable d'abord, la porte ensuite (21e). Chaque fichier repris quitte `PERIOD_ONLY` (entrées `21e` restantes de
    `web/v2/shell/test/writes.test.ts`)
  - dépend de : 21e
  - fait le 2026-11-14 — `globalRefusal` (`access.ts`) prend `refused` (`useAdminGate`) et
    les mots `GateWords` au lieu de `present` et `past`, et une raison durable propre à
    l'écran (`lasting`) : le droit manquant, puis cette raison, puis la porte. Dite là où
    `past` l'était : « Créer » des comptes de service (`c` : `allowed` et `why` durables,
    `lasting`, `say`, statut poli), « Nouveau secret » et « Révoquer » d'un compte,
    « Désactiver », « Révoquer les sessions » et « Pseudonymiser… » d'une identité,
    « Rattacher » (`c` de même) et « Détacher » des émetteurs, « Reprendre » et « Annuler
    l'import » ; l'action d'une ligne que marque le brouillon (« Annuler la révocation »,
    « Annuler le détachement », l'ajout retiré) prend la porte seule. Les trois écritures
    directes relisent la porte à l'envoi : `createSecretWrite` (`secret.ts`, dépendance
    `held`) n'envoie rien quand elle refuse, `SecretDialog` le dit dans le dialogue
    (`InlineAlert`, bouton à raison), saisie gardée, l'étape du secret montré intouchée
    (lisible et copiable jusqu'à la fermeture) ; la pseudonymisation de `IdentityDetail`
    de même (dialogue, motif gardé, rien envoyé) ; `steer` de `ImportDetail` n'envoie rien
    et le dit sous les actions (`InlineAlert` warn, ôtée quand la porte rouvre) ; le dialogue
    « Rattacher » des émetteurs comme `GroupForm`. Les six fichiers repris ont quitté
    `PERIOD_ONLY`. Tests : `access.test.ts` (`globalRefusal` : la porte après le droit
    manquant, la raison durable avant la porte, porte ouverte mêmes refus qu'avant dans
    cinq cas de droits ; `createSecretWrite` : rien envoyé sous la porte, envoyé une fois
    rouverte, un secret montré gardé quand elle se ferme), `admin-gate-access.test.ts`
    (monté : « Créer » et `c` des comptes dans les quatre cas ; la palette, porte seule et
    droit manquant ; la création d'un compte dialogue ouvert puis rétréci, saisie gardée,
    rien envoyé, élargi puis envoyé ; un secret montré puis rétréci, lisible, rien
    recréé ; la pseudonymisation confirmée à l'étroit, rien envoyé ; l'état d'une identité,
    le droit manquant avant la largeur ; l'annulation d'un import confirmée à l'étroit,
    rien envoyé, la raison dite) ; parcours `accessibility` à 768 px (« Créer » des comptes
    de service refusé, atteint et pressé au clavier, `c` dit le motif, aucun dialogue ;
    revenu à 1024 px, le dialogue s'ouvre)
  - choix : « Déjà au brouillon » de l'état d'une identité passe avant la porte (raison
    durable : elle ne change pas avec la largeur ; avant, la porte la masquait) ; le refus
    d'un `steer` à l'envoi est dit dans le panneau sous les actions plutôt que par un toast
    (l'`AlertDialog` fermé, le panneau reste là, testable sans hôte de toasts) ; `ImportDetail`
    n'a pas de droit côté client (le serveur juge) : la porte seule ; aucune clé nouvelle
    (`admin.why.forbidden`, `edit.why.narrow` déjà là)
  - passe de relecture (`ui-reviewer`, `a11y-auditor`) : l'alerte de `ImportDetail` dit la
    raison de la porte telle qu'elle est (passé devenu étroit), pas celle de l'envoi
    refusé ; `SecretDialog` ne dit pas « rien envoyé » pendant une écriture déjà partie
  - reste : la confirmation d'annulation d'un import (`AlertDialog`, sans raison) ne dit
    pas la largeur avant d'être confirmée, refus dit sous les actions après fermeture
    (annonce à écouter au lecteur d'écran, le focus revenu sur le bouton refusé) ; les
    dialogues rétrécis des comptes, identités et émetteurs testés montés, pas en parcours
- [x] 21f. Édition réservée aux largeurs ≥ 1024 px, fiche, anomalies et topographie : le
  motif `narrow` de la 21c sur les actions de la fiche (« Modifier », « Actions »), son
  onglet VLAN et son avis des trous, les anomalies et l'arbre de la topographie —
  REQ-021-29 — verify : parcours `accessibility` à 768 px : « Modifier » d'une fiche et une
  cellule de l'arbre disent le motif
  - où : `web/v2/shell/src/views/RecordView.vue` (`writes` vers la ligne 82, `blockedBy`
    vers la ligne 178), `views/record/NetworkVlanTab.vue` (vers la ligne 116),
    `views/record/GapsNotice.vue`, `views/anomalies/AnomalyGrid.vue` (vers la ligne 36),
    `views/TopologyView.vue` (vers les lignes 407, 473 et 594, `addRefusal`) ; chaque
    fichier repris quitte `PERIOD_ONLY` (`web/v2/shell/test/writes.test.ts`, entrées `21f`)
  - dépend de : 21c (11o et 11p5, qui touchaient la topographie et l'historique, sont
    fusionnées)
  - fait le 2026-11-14 — chaque surface lit `useWrites()` et dit `refusalWords` au point
    d'action, le droit manquant d'abord (raison durable), la porte ensuite : la fiche
    (`RecordView.vue` : `blockedBy` pour « Modifier », « Clore… », le « Créer un VLAN » de
    l'onglet du groupe ; `gateOpen` relit la porte quand « Modifier », une entrée du menu ou
    la création agit, la raison dite par le statut de la fiche), l'onglet VLAN d'un réseau
    (`NetworkVlanTab.vue` : la date, `valid_at` seul compris, puis la largeur), les
    anomalies (`AnomalyGrid.vue` : « Acquitter », la date puis la largeur, passées au
    dialogue), le « + » de l'arbre et « Ajouter dans » (`TopologyView.vue`, `addRefusal` ;
    la porte en un `computed` de la vue, jamais par ligne ; `addIn` la relit). Un dialogue
    ou un formulaire ouvert quand la fenêtre rétrécit garde sa saisie et n'envoie rien : le
    dialogue d'acquittement (`AcknowledgeDialog.vue`, prop `refused` : « Acquitter » refusé
    avec la raison, Entrée la dit dans le dialogue) et « Poser un VLAN existant » (Entrée
    dit le refus dans le formulaire). Les quatre fichiers repris quittent `PERIOD_ONLY` ;
    `GapsNotice.vue` y reste en `kept`. Tests : `test/record-gate.test.ts` (montés :
    « Modifier », le « + », « Acquitter » et son dialogue, « Poser le VLAN », chacun refusé
    à l'étroit puis rendu à la largeur, le droit manquant dit avant la largeur) ; parcours
    `accessibility` à 768 px (« Modifier » d'une fiche atteint et pressé au clavier, aucun
    panneau ; une cellule de l'arbre et le « + » disent le motif ; revenus à 1024 px)
  - choix : `GapsNotice.vue` passe en `kept` dans `PERIOD_ONLY` au lieu d'en sortir : c'est
    une lecture (les trous sont ceux du présent), il n'écrit rien ; l'onglet VLAN d'un
    réseau et l'acquittement gardent leur refus hors du présent sous `valid_at` seul
    (l'opération non datée s'appliquerait tout de suite), la date dite avant la largeur,
    comme `ChangesetView` (21d), avec leurs mots (`edit.why.past`,
    `views.anomalies.notPresent`) ; refusé, « Poser le VLAN » cesse d'être le bouton de
    soumission (`type="button"`) : Entrée dans le champ envoie le formulaire, qui dit la
    raison (droit, date, largeur, déclaration, réglage au brouillon) au lieu de rien ; le
    refus redit d'un dialogue ou du formulaire montre la raison du moment et compte un tour
    (`:key`), l'alerte réinsérée relue à chaque Entrée ; le dialogue garde « Acquitter »
    refusé au clic (infobulle), comme le renommage de la 21d
  - passe de relecture (`ui-reviewer`, `a11y-auditor`) : un second Entrée réannonce le
    refus (l'alerte réinsérée) ; la raison affichée suit celle du moment ; Entrée dans
    « Poser un VLAN existant » dit aussi les refus hors de la porte
  - reste : pour la 21h, l'ordre des fonctions communes de la 21c, appelées sans être
    changées : `play` (`shortcuts/view.ts`, `e` et `c` de la fiche et de l'arbre) dit la
    porte avant le droit, `moveEntryRefusal` (`edit/moves.ts`, « Changer de parent… » de
    la fiche) dit `narrow` avant le droit manquant, `editability` (cellules de l'arbre)
    de même ; `addIn` (`TopologyView.vue`) et `gateOpen` (`RecordView.vue`) relisent la
    porte seule au moment d'agir, sans le droit ; les actions des plugins du menu
    « Actions » ne passent pas par la porte (21g) ; la soumission implicite par Entrée du
    formulaire « Poser un VLAN existant » (sans bouton de soumission une fois refusé)
    n'est prouvée que par un `dispatchEvent("submit")` en Vitest, pas dans un navigateur ;
    l'annonce de l'alerte réinsérée et du statut de la fiche après la fermeture du menu, à
    écouter au lecteur d'écran
- [ ] 21g. Édition réservée aux largeurs ≥ 1024 px, plugins : la largeur du cadre transmise
  aux plugins avec la période (le contexte du SDK, `setContext` de `time/index.ts`, vers la
  ligne 90, ne porte que `time`), et leurs écritures et leurs commandes de palette
  (`commands.register`, `pluginEntries` de `shortcuts/palette.ts`) passant par la porte de
  la 21c : refusées sous 1024 px avec le motif `narrow`, comme `c` et `e` ; REQ-021-29 vaut
  pour eux aussi (l'arbre d'hébergement du plugin `machines` aujourd'hui, les plugins de
  la 017 demain) — REQ-021-29, Q-021-12 — verify : Vitest du contexte (la largeur publiée
  au plugin, changée au redimensionnement) et de `pluginEntries` (une commande d'écriture
  refusée sous 1024 px, son motif dit) ; parcours `accessibility` à 768 px : une écriture
  du plugin `machines` (son arbre d'hébergement) refusée avec le motif
  - où : `web/v2/shell/src/time/index.ts` (`installTime`, `setContext`),
    `web/v2/sdk/src/context.ts` (`setContext`, le contexte publié et son type ;
    `sdk/src/host.ts` l'exporte au shell), `shell/src/frame/width.ts`
    (`frameWidth`), `shell/src/shortcuts/palette.ts` (`pluginEntries`),
    `frame/ShellFrame.vue` (les commandes des plugins), `web/v2/plugins/machines/` ;
    modèle : `test/writes.test.ts`
  - touche le contrat du SDK des plugins (contexte) : à arbitrer avant code, avec la 24f
  - dépend de : 21c
- [x] 21h. Même rang partout : la raison durable (droit manquant, objet livré, portée plus
  large, auteur d'un changeset) avant la porte d'écriture (`past`, `narrow`) dans les vues
  et les changesets aussi, comme l'administration (21e, arbitrage de la session
  principale) — REQ-021-29, REQ-021-37 — verify : Vitest, sur un même écran, le bouton, la
  cellule, la touche (`c`, `e`) et la commande de palette disent la même raison dans les
  quatre cas (droit manquant seul, largeur seule, les deux, aucun) ; un relecteur sans le
  droit lit le droit, pas la largeur (cellule en place, `e`, décision d'un changeset) ; la
  palette garde une commande retenue par la porte seule et l'ôte quand un droit manque
  - où : `web/v2/shell/src/shortcuts/view.ts` (`play` vers la ligne 115, `offered` vers la
    ligne 92 ; `ViewAction.lasting` de la 21e, à généraliser puis à retirer),
    `views/edit/fields.ts` (`editability`, vers la ligne 518 : la porte avant le droit),
    `changesets/model.ts` (`reviewActions`, vers la ligne 138 : la porte avant `held`,
    `author` et le droit), `draft/state.ts` (`refused` à côté de `writable`, vers la ligne
    138) ; tests : `edit.test.ts` (`editability`), `shortcuts.test.ts` (`play`,
    `offered`), `changesets.test.ts` (`reviewActions`), modèle `test/admin-gate.test.ts`
  - amende les choix des 21c (« la largeur avant le droit manquant, comme le passé ») et
    21d (« la porte avant `held`, `author` et le droit manquant ») ; à faire après la 21f
  - dépend de : 21e, 21f
  - fait le 2026-11-14 — une règle, écrite une fois : `rankedRefusal` (`time/index.ts`,
    `RankedReasons` : `lasting`, `state`, `gate`), la raison durable (droit manquant, objet
    livré, champ sans éditeur, valeur non lue ou calculée, opération sans panneau) puis
    l'état (auteur seul, retenu par une politique, brouillon vide, simulation à venir ou en
    erreur, période figée d'une soumission en attente) puis la porte (`past` avant
    `narrow`) ; le rang ne change que la phrase. La suivent : `play` (`shortcuts/view.ts` :
    le `why` de la vue avant la porte pour toutes ; `ViewAction.lasting` retiré, les six
    écrans de l'administration n'en ont plus besoin), `editability` (`fields.ts`, un motif
    durable de l'appelant `own` après le droit, pris par `inline.ts` et `properties.ts`),
    `moveEntryRefusal` et `canMove` (`moves.ts` : cycle, type, droit avant la largeur, la
    largeur avant la date), `reviewActions` (`changesets/model.ts` : droit, puis `held` et
    `author`, puis la porte), `draftActions` (`draft/state.ts`, sortie de
    `useDraftQueries` pour être testée), `periodWhy` et `motiveWhy` (`DraftSubmit.vue`),
    `modifyReason` (`DraftOperations.vue`), `globalRefusal` et `createRefusal`
    (administration), `addRefusal` et `addIn` (`TopologyView.vue`), `blockedBy` et
    `gateOpen` (`RecordView.vue`), `writeBlocked` (`NetworkVlanTab.vue` : droit, puis la
    déclaration non lue, le réglage ou le retrait déjà au brouillon, puis la date et la
    largeur), `refusal` (`AnomalyGrid.vue`), `actions` et `begin` du panneau d'édition
    (`views/edit/action.ts` : le droit avant la largeur et la date) ; `addIn` et `gateOpen`
    relisent au moment d'agir la raison classée (droit puis porte) que l'action dessine :
    `addIn` sur le nœud que l'appelant a jugé, `gateOpen(refus)` avec le refus du bouton
    qui l'appelle (« Modifier » `editBlocked`, « Changer de parent… » `moveRefusal`,
    « Clore… » `closeRefusal`, « Créer un VLAN » `vlanBlocked` : `core.vlan.edit`, pas le
    droit de modifier le groupe, correction de la revue) ; une raison vide ne compte pas
    (`rankedRefusal`). Tests :
    `rank.test.ts` (chaque fonction contre son code de main à 5dffa02a recopié en oracle,
    pour toute combinaison de droit, période, largeur, auteur et politique : mêmes refus,
    seule la phrase change ; aucune décision de changeset offerte qui ne l'était pas),
    `record-gate.test.ts` (Topographie montée, quatre cas : « + », cellule, `c` et palette
    disent la même raison ; la fiche montée : « Créer un VLAN » d'un groupe avec
    `core.vlan.edit` seul s'ouvre, sans lui refusé par lui, rétréci au moment d'agir refusé
    puis dit la largeur ; « Modifier », « Changer de parent… », « Clore… » d'une entité dans
    les quatre cas, la relecture prouvée nécessaire : sans elle, les cas rétrécis au moment
    d'agir ouvrent le panneau), `moves.test.ts` (cycle et droit avant la largeur, nœud non
    marqué ; `canMove` contre l'oracle de main pour cinq couples, deux jeux de droits, deux
    périodes, quatre largeurs), `rank.test.ts` (`editability` avec un motif `own`, une
    raison vide), `action.test.ts` (panneau à l'étroit sans `approval.self` : « Appliquer » dit
    le droit, « Brouillon » la largeur) ; attentes changées : `refused-keys.test.ts` (`c`
    de la Topographie sans domaine sous `as_of` : « aucun domaine » ; `c` et `e` d'un
    lecteur sous `as_of` : le droit, plus le passé), `shortcuts.test.ts` (`e` sans le
    droit sous `as_of` et à l'étroit dit le droit), `edit.test.ts` (cellule sans le droit
    sous `as_of` et à l'étroit : le droit), `moves.test.ts` (« Changer de parent… » sans
    le droit à l'étroit : le droit), `changesets.test.ts` (« Retirer » d'un relecteur : `author` au passé et à
    l'étroit ; « Valider » de l'auteur retenu : `held` avant la largeur, le droit
    `approval.self` avant `held` ; « Annuler » sans `.edit` : le droit), parcours
    `changesets.spec.ts` à 420 px (« Retirer » dit l'auteur, plus la largeur ; « Valider »
    dit la largeur) ; skill `kartenn-ui` § 5
  - choix : palette inchangée hors du présent (absente sous `as_of`, comme avant) : « la
    porte seule » retient la commande à l'étroit seulement ; la simulation à venir compte
    parmi les raisons d'état, avant la porte, comme le brouillon vide ; un champ figé, sans
    éditeur, non lu ou calculé reste avant le droit dans `editability` (le champ d'abord),
    le motif de l'appelant (`own`) après le droit ; sous `as_of`, « Changer de parent… »
    dit encore `edit.move.dated` (inchangé) ; `canMove` garde la largeur avant la date
    servie par le SDK (`dated`), sans changer l'ordre du SDK
  - reste : `decide` et `reject` de `ChangesetView.vue` relisent au moment d'envoyer la
    porte seule (un dialogue ne s'ouvre qu'action offerte : seul la porte a pu changer
    depuis) ; `DraftsView.vue` n'a que la porte (« Reprendre » n'écrit pas) ;
    `refusalSentence` (`admin/gate.ts`) ne fait que dire un refus déjà classé ; `validate`
    d'un éditeur en place resté ouvert à l'étroit dit la largeur avant une valeur fautive
    (saisie, pas raison de refus de l'action) ; `NetworkVlanTab.vue` touché dans
    `writeBlocked` seul, à rapprocher du travail de la 22o sur `views/record/*`
- [x] 22. `bench-web` étendu : INP (Event Timing), navigation en cache, CLS, trames
  perdues de l'arbre (≤ 2 %), Flèche bas (≤ 100 ms p75), tas JS (≤ 64 Mo), plafond par
  plugin, shell avec modules partagés ; protocole de mesure du prototype D-1 repris —
  REQ-021-30 — verify : tableau rendu par `bench-runner` ; un budget dépassé → sortie non
  nulle ; mesure complète lancée une fois par l'orchestrateur (ADR 0032)
  - où : `web/e2e/budgets/budgets.spec.ts`, `web/e2e/budgets.config.ts`,
    `web/shell/scripts/size.mjs`, `devenv.nix` `scripts.bench-web` (vers la ligne 641) ;
    design § « Budgets »
  - dépend de : 18, 19
  - fait le 2026-11-03 — `bench-web [graine] [small] [args playwright…]` mesure le nouveau
    shell (verrous `--bench --pg`, ADR 0032) : poids du shell initial avec les modules
    partagés (`web/v2/shell/scripts/shell-size.ts`, clôture de chaque entrée du build,
    chaque fichier une fois), poids de chaque plugin de `web/v2/plugins`
    (`web/v2/tools/plugin-size.ts` lancé en script, échec aussi si un module partagé est
    empaqueté), puis le projet Playwright `budgets` (`web/v2/e2e/budgets.config.ts`,
    `budgets/budgets.spec.ts`, `budgets/global-setup.ts`) contre `serve.sh budgets`
    (release, `kartenn-bench seed`, plugin `slow` de `budgets/plugins`). Mesures, CPU ×4,
    protocole de `bench/measure.ts` : premier écran utile et son CLS ; plugin lent écarté ;
    page de 100 lignes (p95 de 20) ; arbre de 10⁵ (trames perdues, tâches longues), Flèche
    bas dans l'arbre et dans une grille (p75 de 50), tas après 10⁴ lignes et 50
    navigations ; navigation en cache vers `k:view` (p75 de 10 fiches survolées) et CLS de
    chaque navigation ; INP p75 de 20 pour ouvrir un nœud, filtrer, éditer une cellule,
    changer d'onglet, ouvrir la palette. Toutes les parties tournent, une seule en échec →
    sortie 1. Marque `k:view` : `shell/src/prefetch/viewMark.ts` (après chaque navigation
    sauf la première, trame après le rendu), posée dans `main.ts`. Vitest :
    `shell/test/viewMark.test.ts`, `shell/test/shell-size.test.ts`. `kartenn-bench
    affected` : `web/v2/ui/` propose `bench-ui`, le reste de `web/v2/` `bench-web`.
    Passe réduite (`bench-web 42 small`) : shell 299,8 Ko, plugins 3,5 et 23,2 Ko,
    premier écran 1 431 ms, CLS 0,002 et 0, Flèche bas arbre 72,9 ms, tas 18,2 Mo, INP
    nœud 80, cellule 200, onglet 88 ms : sous budget ; au-delà : page de 100 lignes p95
    806 ms, navigation en cache p75 182 ms, INP filtre 304 et palette 272 ms, Flèche bas
    grille 238 ms, trames perdues de l'arbre 80 % et 114 tâches longues (plus longue
    169 ms).
  - corrigé le 2026-11-03 (revue) : navigation en cache jugée sur la pire des 10, sans
    centile ; CLS d'une navigation : tous les glissements de la trame de `k:view` à 1 s
    après, `hadRecentInput` compris (le clic compte 500 ms comme entrée), prouvé par un
    témoin (bloc de 200 px inséré 200 ms après `k:view` : 0,114 vu) ; page de 100 lignes :
    `responseEnd` de la lecture de la page (par son URL) → première trame où sa première
    ligne (`data-row`) est dans le DOM, médiane et p95 ; arbre : lectures de `/children`
    comptées pendant le défilement (0), défilement arrêté avant la ligne qui lit la page
    suivante ; INP : cible de la plus longue interaction imprimée ; médiane et pire de
    chaque mesure répétée ; préchargement absent → échec ; `bench-web small` sans graine ;
    build de `web/v2` fait une fois (`K_WEB2_BUILT`) ; shell initial : ce que charge le
    document (entrée, ses imports, ce que nomme `index.html`, amorce comprise), sans les
    entrées de l'import map que seul un plugin charge : 300,3 Ko, au-delà.
  - reste : mesure complète (`devenv shell -- bench-web`, graine 42, machine libre) par
    l'orchestrateur ; dépassements de la passe réduite à traiter ou à arbitrer (aucun
    seuil relâché) ; 10⁴ enfants ne tiennent pas en cache : 100 par page (taille par
    défaut du serveur), `MAX_PAGES` = 10, et `treePages.ts` ne redemande une page que si
    le nombre d'enfants a changé (la onzième page laisse ce nombre égal) — le défilement
    de 10⁴ lignes fait des allers-retours sur ceux en mémoire, à arbitrer ; à la passe
    réduite, la lecture s'est arrêtée bien avant (2 puis 6 pages, plus de ligne « suite »
    au bout), cause non élucidée ; la page de 100 lignes n'a pas encore rendu de chiffre
    (une lecture de `/v1/addresses` sans corps lu : revalidation probable ; comptée à part
    depuis, sans passe pour le vérifier) ; le projet `budgets` de l'ancien shell
    (`web/e2e/`) n'a plus de script et part à la bascule (tâche 27).
  - choix : `bench-web` vise le nouveau shell (plus de banc pour l'ancien) ; premier écran
    et page de 100 lignes relevés par le banc (tuiles `.k-home-tiles`, première ligne de
    la page dans le DOM) plutôt que par des marques `k:ready`/`k:rows` dans les écrans,
    hors périmètre ; tas par `Runtime.getHeapUsage` comme les bancs de `@kartenn/ui` ;
    INP d'une répétition : la plus longue des interactions qu'elle contient (un filtre en
    compte six), 0 sous 16 ms ; CLS du premier écran sans les glissements après une
    entrée (définition des navigateurs) ; `alice` administratrice pour éditer, au
    brouillon seulement ; arguments Playwright passés après la graine et `small`.
- [x] 26. Liste de parité `web/v2/PARITE.md` : chaque route, écran, action, point
  d'extension et parcours de l'ancien shell et de ses plugins, avec son équivalent, son
  parcours porté et son état ; tenue à jour à chaque tâche de P2 et P3 et à chaque
  correctif de l'ancien shell — REQ-021-31 — verify : chaque fichier de
  `web/e2e/journeys/` a son pendant dans `web/v2/e2e/` (test) ; liste entièrement cochée
  - où : `web/shell/src/router.ts` (routes), `web/e2e/journeys/*.spec.ts`,
    `web/plugins/*/public/manifest.json`
  - dépend de : 25 (créée tôt, cochée au fil des tâches)
  - en cours : fait le 2026-11-03 — `web/v2/PARITE.md` complète : cadre, session et temps ;
    référentiel ; édition ; changesets et brouillon ; hôte des plugins ; plugin d'exemple ;
    VLAN (écrans du cœur, 026) ; Politique et Flux (017 : absents de l'ancien shell, rien à
    porter, la bascule ne les attend pas, design § « Migration », phase 6) ; « Parcours de
    bout en bout », une ligne par test de `web/e2e/journeys/` (43). Vitest
    `web/v2/shell/test/parity.test.ts` : une ligne par ancien test et aucune pour un test
    disparu ; le pendant nommé existe et tient les titres cités ; une ligne à porter nomme
    sa tâche ou « aucune tâche », une ligne retirée sa référence ; un `todo` par parcours
    sans pendant (`accessibility`, tâche 21) et par test à porter.
  - corrigé le 2026-11-03 (revue, arbitrages A30 à A33) : liens de la 007, opération en
    JSON brut et compte du brouillon de la vue d'exemple en `[-]` (021/A30, A32, A33) ;
    « Clore » à la 11g ; interface et mode 802.1Q à la 18g ; gardes du parcours `smoke` à
    la 21 ; pseudonymisation (15b) et filtres de période et d'objet de l'audit (15c) en
    lignes à porter, la ligne du parcours `admin` et celle de « then undone » des
    changesets (14 : « Appliqué » dans « Mes changesets ») décochées. Le test confronte
    aussi les lignes cochées des autres sections aux parcours qu'elles citent (fichier,
    test ou `describe`), lit la tâche d'une ligne à porter dans sa dernière colonne, et
    met un `todo` par ligne à porter de toutes les sections. État : 81 lignes cochées, 35
    à porter, 6 retirées ; aucune ligne sans tâche.
  - reste : la liste entièrement cochée, au fil des tâches 11, 11g, 12, 14, 15b, 15c, 16,
    18g, 21.
  - choix : une ligne n'est cochée que si un parcours de `web/v2/e2e/` tient tout son
    scénario (une étape manquante la laisse à porter ; une donnée de hasard, comme
    l'événement cherché dans l'audit, ne compte pas) ; ce que seul Vitest prouve reste à
    porter (« Ajout d'une interface » des Machines décoché) ; le test vit dans le projet
    Vitest `shell` (un fichier de `tools/` relancerait toutes les captures) ; un parcours
    est « sans pendant » quand aucune de ses lignes ne nomme un parcours de `web/v2/e2e/`.
  - cochée le 2026-11-03 : 116 lignes portées, 6 retirées par une spec, aucune à porter ;
    `web/v2/shell/test/parity.test.ts` vert sans `todo` (chaque parcours ancien a son pendant)
- [ ] 27. Commit de bascule : `web/v2/*` à la place de `web/*`, build branché dans
  `[web] root`, paquets Nix et OCI, `bench-web` et `scripts/ci.sh` ; ancien shell, SDK
  0.1 et anciens plugins supprimés ; un seul commit — REQ-021-31 — verify : liste de
  parité cochée ; `k-web-check`, `k-web-e2e`, `bench-web`, `checks.vm` et `scripts/ci.sh`
  verts sur ce commit
  - où : `flake.nix` (paquet `kartenn`, shell et plugins), 015 (paquetage) ;
    `devenv.nix` ; `web/e2e/serve.sh`
  - dépend de : 17, 18, 19, 21, 22, 24, 26
- [ ] 23. Synchronisation (après la bascule) : la 007 marquée « partie client remplacée par la 021 » (table
  du design § « Ce qui reste de la 007 ») ; `docs/spec/` (architecture web, budgets du
  navigateur) ; ADR 0037, 0038 acceptées ;
  `specs/README.md` ; audit `spec-keeper` — verify : liens croisés ; aucun écart bloquant
  - où : forme des synchronisations précédentes (007, tâche 25 ; 022, tâche 18)
  - dépend de : 27

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
- [ ] 5. Flux d'événements : `events` (ouverture avant les lectures, contrat de la 022,
  doublons, reconnexion avec attente croissante et `last_event_id`), un flux par
  navigateur (`navigator.locks` + `BroadcastChannel`, repli par onglet), repli sans flux
  (focus, reconnexion, `pageshow` persistant) — REQ-021-23 — verify : Vitest avec un
  `EventSource` simulé (`ready`, `changes`, `truncated`, `reset`, `changeset`, doublons,
  401 puis réouverture) ; élection entre deux contextes simulés ; parcours à deux
  navigateurs (renommage visible en < 2 s) ; trois onglets → une connexion
  - où : 022 design § « Contrat consommé par les clients », § « Doublons » ;
    `web/shell/src/draft/tab.ts` (`BroadcastChannel` existant) ; `web/sdk/src/context.ts`
  - dépend de : 3 ; 022-9, 022-10 (point d'accès servi)
  - en cours : SDK fait le 2026-10-24 — `web/v2/sdk/src/events/stream.ts` (`EventStream` :
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
    croissante, effets sur le cache, cycle bfcache. Reste : parcours à deux navigateurs et
    à trois onglets, après la configuration serveur de la tâche 25 et un premier écran.
- [x] 6. Amendement du contrat client de la 022 (A3) : comparaison sur `version`,
  invalidation des vues qui montrent l'objet, relecture en `If-None-Match` sans `version`
  — verify : paragraphe amendé, marqué « *amendé par la 021* »
  - où : `specs/022-evenements-sse/design.md` § « Contrat consommé par les clients »
  - dépend de : —
  - fait le 2026-10-23 avec la validation de la spec
- [ ] 7. Préchargement : `meta.queries` des routes, `KLink` (survol 60 ms, focus), garde
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
- [ ] 11. Référentiel (dépend aussi de 28 et 29) : accueil en tableau de bord
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
- [ ] 12. Édition en place et modification rapide : éditabilité (droits, présent,
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
  - reste : édition en place dans l'arbre de la Topographie et dans la grille du contenu
    d'un nœud (`TreeNode` de `roots`/`children` ne sert pas `version` : ni `expected` ni
    `If-Match` sans lire chaque objet, ou l'ajouter au contrat 006 — à arbitrer) ; cellules
    de propriété (`prop.<clé>`) : la règle est là, aucune liste n'a encore de colonne de
    propriété, et l'onglet Propriétés de la fiche ne s'édite pas encore en place ; éditeur à
    liste (`Select`) dans une cellule de grille (composant) ; motif de la réponse 202 dit
    par le toast, pas sur la cellule (la `DataGrid` n'a qu'un état « en revue ») ; « + »
    refusé visible avec sa permission sur un nœud du `TreeGrid` (REQ-021-37 ; le composant
    n'a ni `forbidden` ni `reason` pour lui) ; référence (domaine, parent) choisie dans la
    première page de sa collection seulement (au-delà, introuvable dans le panneau) ;
    captures des vrais écrans (comme la 11 : sans API dans le projet `visual`).
- [ ] 24. Déplacements et invariant des arbres : arbre des préfixes en `derived` (aucun
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
- [ ] 13. Libellés et codes : `label` (dictionnaire `GET /v1/labels` de la 018 en cache par
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
- [ ] 14. Changesets, revues, brouillon : listes, `DiffList`, actions selon les droits,
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
  - en cours : fait le 2026-11-01 — SDK : `writes.decide` (validation, rejet avec motif,
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
- [ ] 16. Hôte des plugins et session : emplacements dans le nouveau cadre, point `widget`
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
- [ ] 16g. Titres du shell par `Heading` (020-29) : les classes de titre recopiées
  (`k-login__title`, `k-about__title`, `k-plugin-view__title`, `k-pending__title`,
  `k-help__title`, `k-edit-panel__title`, `k-network-vlan__title`) remplacées par `Heading` ;
  `fieldText` passe une valeur absente à `AttributeList` au lieu d'un « — » lu « tiret »
  — verify : Vitest ; `k-web2-check --base main` (captures touchées listées) ; revue
  `ui-reviewer`
  - où : les fichiers des classes citées sous `web/v2/shell/src/` (grep), `fieldText`
    (`web/v2/shell/src/views/`) ; `Heading` et `AttributeList` de `web/v2/ui` (en lecture)
  - dépend de : 020-29

- [ ] 11g. « Clore » un objet depuis sa fiche (A31) : action de la fiche de chaque sorte du
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

## P3 — Plugins (phase 5)

- [ ] 17. Écrans VLAN du cœur (amendée par la 026 : plus un plugin, des écrans du shell)
  avec `@kartenn/ui`, espace `network`, filtres typés, édition en place — REQ-021-27,
  REQ-026-6 — verify : parcours `vlan` vert
  - où : `web/plugins/vlan/src/index.ts` (comportement actuel), `public/manifest.json`,
    `test/` ; repris de la tâche 16 : onglets et colonnes des plugins sur les types de
    module (`vlan`, `vlan_group`) dans la fiche et les listes VLAN
    (`useExtensions("tab" | "column", type)`, `tabItems`, `gridColumns`,
    `web/v2/shell/src/plugins/slots.ts`), parcours `plugins` étendu (onglet `notes` du
    plugin de test sur `vlan`)
  - dépend de : 11, 12, 16
  - en cours : fait le 2026-11-01 — le VLAN dans les vues génériques de la 11 : deux
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
  - reste : tuile de l'accueil « groupes les plus remplis » (design § « VLAN, écrans du
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
- [ ] 18b. Déplacement d'une machine vers un cluster par `canMove` du SDK (Q-021-2) :
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
  - reste — à arbitrer par l'utilisateur : l'abandon du glisser (un arbre sans framework
    dans `@kartenn/ui`) ; REQ-021-33 dit encore, en acceptation, « le serveur le refuse
    (`machines.host.site_mismatch`) » (`requirements.md`, à amender avec la même
    décision) ; ouverts : `writeRight` de `sdk/src/data/moves.ts` omet la ressource qui
    porte le nom du module par une heuristique du pluriel (`machines` ↔ `machine`), alors
    que `editRight` du shell (`views/edit/fields.ts`) écrit toujours `<type>.edit` — une
    table de permissions tirée du contrat les accorderait ; les
    hébergeurs au-delà de dix pages ne sont pas offerts ; sous Firefox, le parcours de la
    24 « an entity changes parent by its menu » a perdu une fois le focus du champ
    pendant le parcours complet (vert relancé seul, deux fois).

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

- [ ] 18e. `GET /v1/interfaces?network=<id>` (A28) : interfaces d'un réseau avec leurs
  adresses, paginées, sous le filtre de lecture ; l'onglet « Interfaces » d'un réseau
  du plugin la lit en une liste au lieu d'une lecture par machine — verify :
  `k-check -p kartenn-machines` (interfaces d'un réseau, pagination, lecture refusée
  hors droits) ; Vitest et parcours `machines` (onglet d'un réseau) ; contrat régénéré
  - où : `crates/kartenn-machines/src/rest.rs` (routes des interfaces, forme des
    filtres de `/v1/machines?network=`), `web/v2/plugins/machines/src/InterfacesTab.vue`
  - dépend de : 18cc
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

- [ ] 18g. Plugin Machines sur `Heading`, `AttributeList` et les états d'arbre (020-29) :
  fiches (`MachineRecord`, `InterfaceRecord`, `ClusterRecord`) en `AttributeList` et
  `Heading` au lieu de `Grid as="dl"` et de `h1`/`h2` nus ; `HostingTree` en
  `moved`/`preview` au lieu de `draft` — verify : Vitest du plugin ; parcours
  `machines` et `moves` sous les deux navigateurs ; poids du plugin
  - où : `web/v2/plugins/machines/src/{MachineRecord,InterfaceRecord,ClusterRecord,HostingTree}.vue`
  - dépend de : 18cd, 020-29
  - étendue le 2026-11-03 (A33) : l'ajout d'une interface et son mode 802.1Q depuis la
    fiche de l'interface prouvés par le parcours `machines` (lignes de la liste de parité)

## P4 — Budgets et clôture

- [ ] 21. Accessibilité étendue : parcours clavier complet (palette, barre repliée,
  panneau du brouillon, édition en place), reflow 320 px des écrans de consultation,
  message d'édition sous 1024 px — REQ-021-29 — verify : `k-web-e2e` ; revue
  `a11y-auditor` avec lecteur d'écran en mode navigation
  - où : `web/e2e/journeys/accessibility.spec.ts`
  - dépend de : 16
  - étendue le 2026-11-03 (A33) : les gardes du parcours `smoke` de l'ancien shell (une
    violation axe et un script en ligne font chacun échouer un parcours) prouvées par un
    test des fixtures de `web/v2/e2e/`
- [ ] 22. `bench-web` étendu : INP (Event Timing), navigation en cache, CLS, trames
  perdues de l'arbre (≤ 2 %), Flèche bas (≤ 100 ms p75), tas JS (≤ 64 Mo), plafond par
  plugin, shell avec modules partagés ; protocole de mesure du prototype D-1 repris —
  REQ-021-30 — verify : tableau rendu par `bench-runner` ; un budget dépassé → sortie non
  nulle ; mesure complète lancée une fois par l'orchestrateur (ADR 0032)
  - où : `web/e2e/budgets/budgets.spec.ts`, `web/e2e/budgets.config.ts`,
    `web/shell/scripts/size.mjs`, `devenv.nix` `scripts.bench-web` (vers la ligne 641) ;
    design § « Budgets »
  - dépend de : 18, 19
- [ ] 26. Liste de parité `web/v2/PARITE.md` : chaque route, écran, action, point
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

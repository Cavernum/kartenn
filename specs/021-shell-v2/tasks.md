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

- [ ] 25. Espace `web/v2/` : `pnpm-workspace.yaml` et lockfile propres, paquets `sdk`
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
  - en cours : espace, lockfile, `ui`, `shell`, `e2e`, lint, étape de `ci.sh` et commandes
    devenv faits avec la 020 (2026-10-23) ; paquet `sdk` copié le 2026-10-24 (1.0.0, MPL-2.0,
    `LICENSE`, contrat généré depuis `web/v2/sdk/scripts/gen.mjs`, test du paquet sur
    `tools/notices.ts`) ; `tokens` retiré du SDK (les jetons sont ceux de `@kartenn/ui`).
    Reste : `plugins`, configuration de test de kartenn-server sur les builds de `web/v2/`.

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
  - où : `web/shell/src/router.ts` ; `web/shell/src/plugins/loader.ts` (`modulepreload`)
    ; `web/shell/src/data/recent.ts`
  - dépend de : 2
  - en cours : partie SDK faite le 2026-10-24 — `web/v2/sdk/src/data/prefetch.ts` :
    `prefetch.intent(cible, requêtes)` (survol tenu 60 ms, focus immédiat, sortie avant le
    délai sans lecture), `prefetch.guard(requêtes)` (≤ 100 ms, `ready` ou `timeout`),
    `prefetch.prefetch` (lectures et listes). Vitest : `test/prefetch.test.ts`. Reste au
    shell : `KLink`, `meta.queries` des routes et garde `beforeResolve`, page suivante à
    l'approche de la fin, enfants survolés, `modulepreload` des plugins, parcours et banc.
- [ ] 8. Partage et manifeste : import map étendue (framework, `@kartenn/ui`, feuille liée
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

## P2 — Écrans du nouveau shell (phase 4)

Chaque zone : écrans du nouveau shell dans `web/v2/shell`, parcours de l'ancien portés dans
`web/v2/e2e` et verts, captures comparées aux maquettes validées, revues `ui-reviewer` et
`a11y-auditor`, lignes de la liste de parité cochées. L'ancien shell n'est pas modifié.

- [ ] 9. Cadre : squelette statique de `index.html` et feuille critique, barre latérale
  (espaces, repli), barre d'état (fil d'Ariane, pastille de date, « Brouillon (n) », menu
  de l'utilisateur : langue, thème, densité, raccourcis), bande hors présent, lien
  d'évitement — REQ-021-1, 2, 4, 5, 6 — verify : parcours `shell`, `time` ; CLS ≤ 0,05 ;
  2.4.11 (focus non masqué par la barre et la bande)
  - où : `web/shell/src/app/AppShell.vue`, `navigation.ts`, `UserMenu.vue`,
    `time/TimeBanner.vue`, `time/index.ts` ; maquettes E1, E6 ; `web/shell/index.html`
  - dépend de : 7, 8 ; 020-23
- [ ] 10. Raccourcis et palette : registre (portée, conflits, réglage à une touche), un
  écouteur, séquences `g`, aide `?` et `/help/shortcuts`, palette Ctrl+K (navigation,
  actions, recherche par valeur, récents, commandes des plugins) — REQ-021-3, 7, 8 —
  verify : Vitest du registre ; parcours : `g c`, `?`, saisie « gc » dans un champ sans
  effet, réglage désactivé ; palette au clavier seul
  - où : `web/shell/src/app/SearchBox.vue`, `views/SearchView.vue` (recherche actuelle) ;
    design § « Raccourcis » ; maquette E8
  - dépend de : 9
- [ ] 11. Référentiel (dépend aussi de 28 et 29) : arbre des préfixes (`TreeGrid`, colonnes de plugins), listes
  (`DataGrid`, filtres typés tirés du contrat : `filters.ts` généré comme `dated.ts`),
  fiche en panneaux, anomalies, propriétés — REQ-021-9, 18 — verify : parcours `records`,
  `tree` réécrits ; lien filtré copié rouvert à l'identique ; captures E1 à E3
  - où : `web/shell/src/views/{HomeView,ListView,ObjectView,AnomaliesView,PropertiesView}.vue`,
    `record/*.vue`, `tree/model.ts`, `data/collections.ts` ; `web/sdk/scripts/gen.mjs`
    (génération de `dated.ts`, à étendre)
  - dépend de : 10 ; 020-19, 020-25, 020-26
- [ ] 12. Édition en place et modification rapide : éditabilité (droits, présent,
  contrat), cellule → brouillon, Ctrl+Entrée → `write.quick` avec motif en ligne, rendu
  de 202, 412 (fusion), 422, 403 ; création (`c`) et modification (`e`) — REQ-021-15,
  16 — verify : parcours `edit` réécrit : lecteur sans éditeur ; brouillon à 1 ; 202 →
  « en revue » ; deux onglets → 412 sans perte
  - où : `web/shell/src/edit/{QuickEdit.vue,fields.ts,submit.ts}`, `rights/index.ts`,
    `views/NewView.vue`
  - dépend de : 11 ; 4
- [ ] 24. Déplacements et invariant des arbres : arbre des préfixes en `derived` (aucun
  déplacement ; valeur modifiée par le brouillon, aperçu tiré de la simulation, arbre
  relu après application) ; `canMove` du shell (cycle, types tirés du contrat, droits sur
  les deux bouts, présent, codes des règles serveur) ; déplacement → brouillon ou
  changeset, sans optimisme ; test de propriété de bout en bout — REQ-021-17, 32, 33,
  34 — verify : Vitest de `canMove` (chaque refus, ordre) ; parcours : aucun moyen de
  déplacer un préfixe ; cluster sur sa propre machine refusé ; machine vers un cluster
  d'un autre site acceptée par le client puis refusée par le serveur
  (`machines.host.site_mismatch`) sans que le nœud quitte sa place ; réponse ralentie →
  nœud « en attente » à sa place ; test de propriété (graine fixée, 200 étapes de
  déplacements et de modifications de valeurs de préfixes et de plages, acceptées ou
  refusées) : chaque ligne contenue par sa ligne parente et parent égal à celui de l'API
  - où : design § « Déplacements dans les arbres » ; `web/shell/src/tree/model.ts` ;
    `web/sdk/scripts/gen.mjs` (table des parents admis, comme `dated.ts`) ;
    `crates/kartenn-machines/src` (règles `machines.host.*`) ; 020 design § « Contrat de
    déplacement »
  - dépend de : 12 ; 020-26 ; le cas machines après 18
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
- [ ] 14. Changesets, revues, brouillon : listes, `DiffList`, actions selon les droits,
  « À instruire » et compteur, panneau et page du brouillon — REQ-021-6, 10, 11 —
  verify : parcours `changesets`, `draft` réécrits ; captures E4, E5
  - où : `web/shell/src/changesets/*`, `draft/{DraftDrawer.vue,store.ts,index.ts}`
  - dépend de : 12
- [ ] 15. Administration : rôles, liaisons, émetteurs, comptes de service et secret,
  identités, audit et export, imports ; permissions en nom servi et libellé —
  REQ-021-12 — verify : parcours `admin` réécrit ; capture E7
  - où : `web/shell/src/admin/*`, `views/AdminView.vue`
  - dépend de : 13
- [ ] 16. Hôte des plugins et session : emplacements dans le nouveau cadre, point `widget`
  de l'Accueil, actions sur le type `changeset`, onglets sur les types de module, frontière
  d'erreur avec code, entrées par espace, « À propos » ; session (reconnexion, échéance
  signalée) — REQ-021-13, 14 — verify : parcours `plugins`, `session`, `example`
  - où : `web/shell/src/plugins/{ExtensionElement.vue,PluginView.vue,registry.ts}`,
    `session/*`, `views/{AboutView,LoginView}.vue`
  - dépend de : 9

## P3 — Plugins (phase 5)

- [ ] 17. Plugin VLAN réécrit avec le framework partagé et `@kartenn/ui`, DOM léger,
  espace `network`, filtres typés, édition en place — REQ-021-27 — verify : parcours
  `vlan` vert ; poids ≤ 50 Ko gzip hors partagés
  - où : `web/plugins/vlan/src/index.ts` (comportement actuel), `public/manifest.json`,
    `test/`
  - dépend de : 11, 12, 16
- [ ] 18. Plugin Machines, même traitement, espace `hosts`, déplacement d'une machine vers
  un cluster (si Q-021-2 le retient) par le `canMove` de la tâche 24 (codes
  `machines.host.*`), sans optimisme — verify : parcours `machines` vert
  - où : `web/plugins/machines/`
  - dépend de : 17
- [ ] 19. Plugin d'exemple passé au SDK 1.0, **toujours sans framework**, en racine
  d'ombre avec `handle.styles()` sur `@kartenn/ui/style.css`, lecture par le
  `QueryClient` — REQ-021-25, 27 — verify : parcours `example` vert ; invalidation reçue
  par le plugin
  - où : `web/plugins/example/src/index.ts`
  - dépend de : 16
- [x] 20. Amendement de la 017 : U2 et U14 remplacées (020/A1, 021/A1 closes), tâches web
  de la 017 en attente du socle de la 020 et de la 021, sous `web/v2/plugins/` — verify :
  amendement accepté consigné dans la 017 (decisions, requirements, tasks, design)
  - où : `specs/017-plugins-politique-flux/decisions.md` § « Amendement — socle 020 et
    021 »
  - dépend de : —
  - fait le 2026-10-23 (arbitrage de l'utilisateur)

## P4 — Budgets et clôture

- [ ] 21. Accessibilité étendue : parcours clavier complet (palette, barre repliée,
  panneau du brouillon, édition en place), reflow 320 px des écrans de consultation,
  message d'édition sous 1024 px — REQ-021-29 — verify : `k-web-e2e` ; revue
  `a11y-auditor` avec lecteur d'écran en mode navigation
  - où : `web/e2e/journeys/accessibility.spec.ts`
  - dépend de : 16
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

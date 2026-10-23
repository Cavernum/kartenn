---
status: draft
updated: 2026-10-23
---

# 021 — Shell v2 : tâches

Brouillon du 2026-10-23 ; à valider avec les arbitrages A1 à A9 et les questions Q-021-1
à 14 ([`decisions.md`](decisions.md)). Chaque tâche est un commit, laisse le build vert
(`k-web-check`, `k-web-e2e` pour les écrans, `k-check` pour les tâches qui touchent le
serveur) et nomme ce qu'elle implémente ; chacune porte `où :` et `dépend de :`.
**[D-1]** : la tâche attend l'issue du prototype de framework. Commandes lourdes par
`k-heavy`.

Ordre (design, § « Migration ») : tâche 1 (prototype D-1) ; la 020 jusqu'à sa tâche 15
(maquettes validées) ; P1 (SDK 2.0) en parallèle de la 020, sans attendre D-1 sauf la
tâche 8 ; P2 (écrans) par zones, chacune après les composants qu'elle emploie (020, P4) ;
P3 (plugins) ; P4 (budgets, clôture). Trois sous-agents d'implémentation au plus, et
aucun en parallèle sur `@kartenn/ui` (020).

## P0 — Décision du framework

- [ ] 1. Prototype D-1 : la même grille éditable (10⁴ lignes virtualisées, édition au
  clavier, états `draft`/`review`/`error`) et le même arbre éditable (10⁵ nœuds, enfants
  paginés, déplacement au clavier) en Vue 3 + Reka UI et en React 19 + React Aria, sur les
  jetons de la 020 ; mesures : INP des interactions, trames perdues, tas après défilement,
  poids gzip ; violations CSP ; axe ; scripts APG ; lisibilité du code écrit par un agent.
  Résultat consigné dans `docs/spikes/` et arbitré par l'utilisateur ; plafond de mémoire
  proposé (Q-021-7) ; si React : amendements de D17 et de l'ADR 0011 rédigés — D-1,
  REQ-021-30 — verify : tableau comparatif, chiffres reproductibles par une commande
  - où : `docs/spikes/001-gist-exclusions.md` (forme d'un compte rendu de prototype) ;
    hors du build (`web/spikes/d1-*`, supprimé après la décision) ; 020 design § « Dépend
    de D-1 », 021 design § « Dépend de D-1 »
  - dépend de : 020-5 (jetons)

## P1 — SDK 2.0 (phase 3)

- [ ] 2. Cache : `@tanstack/query-core` dans le SDK, `queryClient` unique, fabriques de
  clés (`keys.object`, `keys.list`, `keys.sub`, `timeKey`), fonction de requête sur le
  client existant avec `If-None-Match` et réutilisation sur 304, fraîcheur par famille ;
  `SDK_VERSION` 2.0.0 — REQ-021-21, 25 — verify : Vitest : clés normalisées (ordre des
  filtres, présent = `null`), `as_of` passé jamais relu, 304 → donnée gardée sans
  nouveau rendu
  - où : `web/sdk/src/api/client.ts` (`createClient`, réponse `{data, etag}`),
    `dated.ts` ; `web/sdk/src/index.ts` (exports) ; `web/sdk/package.json` ; design
    § « Clés et fraîcheur »
  - dépend de : —
- [ ] 3. Dépendants et version : extraction des références `{type, id}` d'un corps
  (schémas de résumé du contrat), index `id → clés`, comparaison sur `version` ; relevé,
  par ressource, de la présence de `version` dans le corps — REQ-021-23 — verify : Vitest
  sur des corps du contrat (préfixe avec résumé de domaine, fiche de machine) ; tableau
  des ressources sans `version` consigné dans le design
  - où : `web/sdk/src/api/schema.d.ts` (`components['schemas']`) ;
    `crates/kartenn-rest/src/dated.rs` (`ETag::record`, `own_version`) pour la règle
    serveur ; 006/H19
  - dépend de : 2
- [ ] 4. Écritures : `write.quick` (annulation des lectures de la clé, optimisme au
  présent, retour arrière, issues `applied`/`proposed`/`conflict`/`invalid`/`forbidden`),
  invalidation de ses propres écritures (objets de la réponse, collections, dépendants) ;
  `draft.add` inchangé — REQ-021-16, 24 — verify : Vitest : 200, 202 (jamais
  « appliqué »), 412 (relecture, saisie rendue), 422, 403 ; aucun optimisme sous
  `valid_at` ou `as_of`
  - où : `web/shell/src/edit/submit.ts` (logique actuelle de la modification rapide) ;
    `web/sdk/src/draft.ts`, `host.ts` ; `web/sdk/src/api/problem.ts`
  - dépend de : 3
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
- [ ] 6. Amendement du contrat client de la 022 (A3) : comparaison sur `version`,
  invalidation des dépendants, relecture en `If-None-Match` sans `version` — verify :
  paragraphe amendé, marqué « *Amendé par la 021* », liens croisés
  - où : `specs/022-evenements-sse/design.md` § « Contrat consommé par les clients » ;
    marqueur comme REQ-004-37
  - dépend de : arbitrage A3
- [ ] 7. Préchargement : `meta.queries` des routes, `KLink` (survol 60 ms, focus), garde
  de navigation (attente ≤ 100 ms), page suivante et enfants préchargés, modules de
  plugins au survol — REQ-021-22 — verify : parcours : navigation vers une fiche survolée
  sans squelette ; marque `k:view` ≤ 100 ms (banc, tâche 22)
  - où : `web/shell/src/router.ts` ; `web/shell/src/plugins/loader.ts` (`modulepreload`)
    ; `web/shell/src/data/recent.ts`
  - dépend de : 2 ; **[D-1]** pour la garde du routeur
- [ ] 8. Partage et manifeste : import map étendue (framework, `@kartenn/ui`, feuille liée
  avec empreinte) ; manifeste `ui`, `framework`, `space`, `go` validés côté serveur et
  publiés dans `ui_plugins` (ajout additif) ; plages vérifiées au chargement ; adaptateur
  du cache dans `@kartenn/ui` ; API `commands`, `shortcuts`, `label` — REQ-021-25, 26 —
  verify : plugin `ui: ^3` sur `ui` 1.x → `incompatible` ; CSP inchangée hors empreinte de
  l'import map ; `k-check` (manifeste) **[D-1]**
  - où : `web/shell/vite.config.ts` (`sharedSdk`) ; `crates/kartenn-rest/src/web/plugins.rs`
    (`Manifest`, `sdk_range`), `capabilities.rs` ; `crates/kartenn-rest/src/web.rs`
    (`import_map`, `csp`) ; `web/sdk/src/semver.ts` ; `web/shell/src/plugins/loader.ts`
  - dépend de : 2, 1 ; 020-4

## P2 — Écrans du shell, par zones (phase 4) [D-1]

Chaque zone : vues 1.x remplacées, parcours de bout en bout réécrits et verts, captures
comparées aux maquettes validées, revues `ui-reviewer` et `a11y-auditor`.

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
- [ ] 11. Référentiel : arbre des préfixes (`TreeGrid`, colonnes de plugins), listes
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
- [ ] 13. Libellés et codes : `label` (API quand la 018 le servira, catalogues, clé),
  `KeyLabel` partout ; toute erreur par `ErrorState`, `InlineAlert`, `Toast` avec codes et
  `Request-Id` — REQ-021-19, 20 — verify : Vitest de `label` ; parcours : erreur 422
  montre le code de chaque violation, copiable
  - où : `web/shell/src/i18n/index.ts` (traduction des problèmes),
    `admin/WriteOutcome.vue`
  - dépend de : 11
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
- [ ] 16. Hôte des plugins et session : emplacements dans le nouveau cadre, frontière
  d'erreur avec code, entrées par espace, « À propos » ; session (reconnexion, échéance
  signalée) — REQ-021-13, 14 — verify : parcours `plugins`, `session`, `example`
  - où : `web/shell/src/plugins/{ExtensionElement.vue,PluginView.vue,registry.ts}`,
    `session/*`, `views/{AboutView,LoginView}.vue`
  - dépend de : 9

## P3 — Plugins (phase 5) [D-1]

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
- [ ] 19. Plugin d'exemple passé au SDK 2.0, **toujours sans framework**, en racine
  d'ombre avec `handle.styles()` sur `@kartenn/ui/style.css`, lecture par le
  `QueryClient` — REQ-021-25, 27 — verify : parcours `example` vert ; invalidation reçue
  par le plugin
  - où : `web/plugins/example/src/index.ts`
  - dépend de : 16
- [ ] 20. Amendement de la 017 : U2 et U14 amendées (A1 de la 020 et de la 021), tâches
  de la 017 réordonnées sur la 020 et la 021 — verify : note d'amendement complétée,
  `tasks.md` de la 017 à jour
  - où : `specs/017-plugins-politique-flux/decisions.md` (note d'amendement posée le
    2026-10-23), `tasks.md`
  - dépend de : arbitrages A1 (020 et 021)

## P4 — Budgets et clôture

- [ ] 21. Accessibilité étendue : parcours clavier complet (palette, barre repliée,
  panneau du brouillon, édition en place), reflow 320 px des écrans de consultation,
  message d'édition sous 1024 px — REQ-021-29 — verify : `k-web-e2e` ; revue
  `a11y-auditor` avec lecteur d'écran en mode navigation
  - où : `web/e2e/journeys/accessibility.spec.ts`
  - dépend de : 16
- [ ] 22. `bench-web` étendu : INP (Event Timing), navigation en cache, CLS, trames
  perdues de l'arbre, tas JS, plafond par plugin, shell avec modules partagés —
  REQ-021-30 — verify : tableau rendu par `bench-runner` ; un budget dépassé → sortie non
  nulle ; mesure complète lancée une fois par l'orchestrateur (ADR 0032)
  - où : `web/e2e/budgets/budgets.spec.ts`, `web/e2e/budgets.config.ts`,
    `web/shell/scripts/size.mjs`, `devenv.nix` `scripts.bench-web` (vers la ligne 641) ;
    design § « Budgets »
  - dépend de : 18, 19 ; 1 (plafond de mémoire)
- [ ] 23. Synchronisation : la 007 marquée « partie client remplacée par la 021 » (table
  du design § « Ce qui reste de la 007 ») ; `docs/spec/` (architecture web, budgets du
  navigateur) ; ADR 0037, 0038 acceptées ; D17 et ADR 0011 amendées si D-1 l'a voulu ;
  `specs/README.md` ; audit `spec-keeper` — verify : liens croisés ; aucun écart bloquant
  - où : forme des synchronisations précédentes (007, tâche 25 ; 022, tâche 18)
  - dépend de : 22

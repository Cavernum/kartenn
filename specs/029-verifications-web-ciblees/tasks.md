---
status: approved
updated: 2026-11-01
---

# 029 — Vérifications web ciblées : tâches

Outillage décidé par l'[ADR 0042](../../docs/adr/0042-verifications-web-ciblees.md)
(acceptée le 2026-11-01 ; passage complet par la session principale et cause des
instabilités de Firefox, choix de l'utilisateur). Chaque tâche est un commit qui laisse
le build vert. « verify » = preuve de la tâche ; « dépend de » = tâches à finir avant.

Commun à toutes les tâches : aucune capture, aucun instantané ni aucun script clavier ne
disparaît du passage complet ; les références ne changent pas, sauf correction voulue et
regardée.

- [ ] 1. Parallélisme et build du catalogue : `fullyParallel` sur le projet `visual` ;
  nombre de workers mesuré (2, 4, 6, 8) dans un créneau de `k-heavy` et fixé par
  `K_HEAVY_JOBS` ; build du catalogue réutilisé quand l'empreinte de ses sources n'a pas
  changé (ADR 0042, décision 4).
  - verify : tableau des durées par nombre de workers, machine au repos ; trois
    passages complets de suite au nombre retenu, sans capture instable ; un second
    passage sans changement ne refait pas le build.
  - où : `web/v2/e2e/playwright.config.ts` (`workers`, `webServer`) ;
    `devenv.nix` (`scripts.k-web-check`, `kHeavy` pour `K_HEAVY_JOBS`) ;
    `web/v2/shell/package.json` (`build:catalogue`).
  - dépend de : —
- [x] 2. Graphe d'impact : outil de `web/v2/tools` qui lit les imports des sources de
  `web/v2` (`.ts`, `.vue`), résout le point d'entrée de `@kartenn/ui` symbole par
  symbole, et rend pour une liste de fichiers les cibles touchées : pages du catalogue
  (composant), maquettes (identifiant), scripts clavier, specs transverses ; un fichier
  transverse rend « tout » (ADR 0042, décision 1).
  - verify : tests : chaque composant du catalogue et chaque maquette ont leurs cibles ;
    un composant importé par une maquette atteint cette maquette ; un symbole importé
    par le point d'entrée n'atteint que ses importateurs réels ; les fichiers
    transverses rendent « tout ».
  - où : `web/v2/tools/` (paquet existant) ; `web/v2/ui/src/index.ts` (point
    d'entrée) ; `web/v2/e2e/visual/{catalogue,ecrans}.spec.ts`,
    `web/v2/e2e/components/*.spec.ts` (forme des cibles) ;
    `web/v2/shell/src/_ui/ecrans/registry.ts` (maquettes).
  - dépend de : —
  - fait le 2026-11-01 : `web/v2/tools/impact/graph.ts` (imports des `.ts`, des `.vue` —
    `<script>`, `<style>`, `src` — et des `.css` — `@import`, `url()` — lus par l'API de
    TypeScript, déjà dépendance de l'espace ; chemins relatifs, `exports` des paquets de
    l'espace, globs de Vite ; fermeture d'une page qui suit les noms importés à travers
    les ré-exports, `export *` compris), `impact.ts` (pages : `main.ts` évalué, la mise en
    page du catalogue, index, jetons, icônes, revue, une par `<Nom>.catalogue.ts`, une par
    identifiant de `MOCKUPS` avec son écran de `SCREENS` ; specs lues pour les pages
    qu'elles ouvrent ; règles « tout » et « rien »), `cli.ts` ; règle stylelint
    `kartenn/component-prefix` (`tools/stylelint/plugins.js`, `prefixes.js`) ; tests
    `tools/test/impact.test.ts` (graphes écrits à la main, propriétés tirées du graphe de
    l'espace réel, sélection comparée au `--grep` de `playwright test --list`) et
    `stylelint.test.ts`. Vérifié par Vitest `tools`, tsc, eslint, prettier et stylelint
    sur tout `web/v2` ; le passage complet de `k-web2-check` est fait par la session
    principale à la fusion (ADR 0042, décision 3 ; aucun créneau `k-heavy` libre en
    2 × 30 min). Relecture `kartenn-revue` : corrections 1 à 7 faites.
  - hors périmètre, vu en relecture : `shell/src/_ui/ecrans/EcranCadre.vue` (l. 440-446)
    recopie les règles globales `.k-drawer .k-sidebar` de `shell/src/frame/ShellFrame.vue`
    (l. 44-52).
  - choix :
    - sortie : `node web/v2/tools/impact/cli.ts [--json] [fichier…]` (fichiers en
      arguments ou sur l'entrée standard, chemins du dépôt comme `git diff --name-only`,
      lus depuis sa racine quel que soit le répertoire courant ; un chemin ni présent ni
      supprimé selon git est signalé sur la sortie d'erreur) :
      une cible par ligne `<spec>\t<grep>` (spec relative à `e2e/`, expression pour
      `--grep` sur le titre que Playwright filtre, « projet fichier describe… titre »), ou
      `all` seul ; `--json` : `{all, reasons, targets: [{kind, name, spec, grep}]}`. Pour
      la tâche 3 : fichiers = 1re colonne, `--grep` = 2e colonne jointe par `|` ; les
      doutes qui élargissent vont sur la sortie d'erreur.
    - pages fines : `catalogue.spec.ts` par page (`mode … index`, `tokens`, `icons`, le
      `describe` du composant), `ecrans.spec.ts` par identifiant (`<px> px <id> `) ; une
      référence (`__screenshots__`) rejoue sa page seule. Les autres specs, entières,
      partent quand une page qu'elles ouvrent est touchée, lue dans leurs littéraux
      (`/_ui/c/<Nom>`, `/_ui/ecrans/<Id>`, un nom de composant ou un identifiant seul,
      `MOCKUPS` = toutes les maquettes, le dossier des composants = tous, une variable
      dont toutes les valeurs sont des littéraux = ces littéraux) ; une adresse calculée
      d'un genre (`/_ui/c/${x}`, `/_ui/ecrans/${y}`) = toutes les pages de ce genre, même
      à côté de littéraux ; une adresse illisible = toutes les pages. D'où : `csp.spec.ts`
      (il suit les liens de l'index) pour toute page ; `clavier.spec.ts` pour tous les
      composants et ses maquettes nommées ; `layout.spec.ts` pour tous les composants et
      toutes les maquettes ; `touch.spec.ts` pour 4 composants et toutes les maquettes ;
      chaque script clavier pour les composants qu'il ouvre (tous, s'il calcule
      l'adresse).
    - types : `import type`, `export type` ne comptent pas ; `import { type X }` compte
      comme évaluation (`verbatimModuleSyntax`).
    - évaluer un module d'un paquet déclaré sans effet de bord (`sideEffects` de
      `@kartenn/ui`, `@kartenn/sdk`) ne charge rien : `shared/ui.ts` (`export * from
      "@kartenn/ui"`, importé par `main.ts`) n'étend pas chaque page à toute la
      bibliothèque.
    - feuille unique de `@kartenn/ui` (morceau `ui` : composants, `internal/`, `demo/` ;
      les `*.catalogue.ts` n'ont pas de feuille) : la sélection compte qu'une feuille de
      composant ne touche que ce qu'il rend, ses emplacements compris ; la règle stylelint
      `kartenn/component-prefix` le garde : chaque sélecteur d'une feuille de
      `ui/src/components/<X>/` (`.vue`, `.css`), d'un fichier de `internal/` ou de
      `demo/` porte une classe de son unité, chaque `@keyframes` en porte le nom ; une
      classe appartient au plus long préfixe qu'elle prolonge (fin, `-` ou `_` : `k-tree-grid…`
      est à TreeGrid). Préfixes : `k-<nom-du-dossier>` (`k-demo-<nom>` pour une démo),
      plus ceux déjà pris (`prefixes.js`, `EXTRA_PREFIXES` : `k-palette`,
      `k-cell-error`, `k-row-fresh`, `k-date-field`, `k-date-calendar`, `k-matrix`,
      `k-sidebar`, `k-toasts`), sans toucher un composant. Exceptions, qui rendent
      « tout » : `components/shared.css`, `Select/options.css` (listes de Select,
      Combobox, FilterPicker), `Menu/Menu.vue` (le chevron d'IconButton). Ce que la règle
      ne voit pas : un composant qui poserait dans son balisage une classe d'un autre.
    - ce que l'entrée du shell charge statiquement (fermeture de `main.ts`, arêtes
      paresseuses seules coupées : `ShellFrame.vue`, `LoginView.vue`, `HelpView.vue`,
      `App.vue`… 76 fichiers) rend « tout » : ces modules s'exécutent et leurs feuilles
      rejoignent celle de l'entrée sur chaque page. Ceux de `@kartenn/ui` à part : sans
      effet de bord, leur feuille gardée par la règle ci-dessus.
    - coupures des pages : composants `.vue` des tables de routes du shell (`router.ts`,
      `**/routes.ts`), rendus sur leur route seulement (leur charge statique relève du
      point précédent) ; écrans de `EcranPage.vue` ; glob de `catalogue.ts`
      (`./components/*/*.catalogue.ts`, le composant nommé par le fichier) ; chaque page
      est enracinée de son côté, et l'index part pour tout fichier de ce glob (il les
      liste).
    - un nom ré-exporté compte aussi le baril : un changement de `ui/src/index.ts` (ou de
      ce que `main.ts` charge de `@kartenn/ui` : `i18n.ts`, `motion.ts`…) atteint toutes
      les pages, une liste de fichiers ne disant pas quelle ligne a changé.
    - « tout » : manifestes, lockfile, `tsconfig*`, tout fichier de configuration lu par
      son nom (`*.config.{js,ts,mjs,cjs}`, `.postcssrc`…), fixtures de Playwright,
      jetons, `style.css` et ses `@import`, polices, `index.html` et `public/` du shell,
      l'outil lui-même et `tools/stylelint/prefixes.js`, les exceptions de la règle de
      préfixes, la charge statique de l'entrée, `devenv.*` (script de capture), un
      fichier d'`e2e/` du projet `visual` autre qu'une spec ou une référence, une feuille
      ou un fichier qu'aucun module ne cite. « Rien » : documentation, tests unitaires,
      le reste de `tools/`, parcours et bancs, scripts de build, un module qu'aucune page
      capturée ne charge (vue paresseuse du shell, fichier supprimé).
    - doute : une importation non résolue (import ou glob calculé, alias inconnu,
      fichier absent) ouvre la page à tout changement (aucune aujourd'hui, testé) ; un nom
      qu'un module n'exporte pas le prend entier.
- [x] 3. `k-web2-check` ciblé : base `origin/main` par défaut, fichiers modifiés et non
  suivis ; captures, instantanés et scripts du graphe de la tâche 2 ; prettier, eslint,
  stylelint sur les fichiers touchés ; Vitest complet (décision 2 amendée, au lieu de
  `vitest --changed`) ; vue-tsc complet ; `--all`
  pour tout ; verdict qui dit la sélection (ADR 0042, décisions 1 et 2).
  - verify : essais sur quatre diffs jetables (un composant, une maquette, un jeton,
    une vue du shell hors maquettes) avec la sélection attendue et la durée ; `--all`
    rend le même verdict qu'avant la tâche.
  - où : `devenv.nix` (`scripts.k-web-check`, `scripts.k-web2-check`).
  - dépend de : 2
  - fait le 2026-11-01 : `devenv.nix`, `scripts.k-web-check` : `k-web-check [--all |
    --base <ref>] [args vitest…]`, voie ciblée là où l'espace a `tools/impact/cli.ts`
    (`web/v2` ; `web/` garde son passage complet, `--all` y est sans effet) ; changement =
    `git diff --name-only --no-renames $(git merge-base <base> HEAD)` (de la base commune
    à l'arbre de travail : commits, indexé, non indexé, suppressions) et `git ls-files
    --others --exclude-standard`, chemins non échappés (`core.quotePath=false`) ; base
    introuvable ou sans ancêtre commun : même `KO base`, code 2 (essayés) ; une spec
    supprimée (ou la référence d'une spec supprimée) ne donne aucune cible
    (`impact.ts`, `specFiles` ; test « runs nothing for a deleted spec ») ; la raison de
    « tout » arrive en `impact: all: …` ; `scripts/ci.sh` passe `k-web2-check --all`.
    Essais (`--base HEAD`, un commentaire jeté ; durée dans le
    créneau, attente exclue) : vue du shell `views/TopologyView.vue` → « ciblé : 0
    captures sur 2706 (aucune touchée) », 43 s, ni build ni projet `visual` ; composant
    `Switch.vue` (et un fichier non suivi) → 881 sur 2706 (catalogue `Switch` et
    `FormField`, cinq scripts clavier, `clavier`, `csp`, `layout`), 4 min 35 s ; maquette
    `E12Vlan.vue` → 1 157 sur 2706 (`E12`, `csp`, `layout`, `touch`), 4 min 27 s ; jeton
    `tokens/tokens.css` → « 2706 captures sur 2706 (… ; tout, … the tokens) », lints
    complets, 14 min 19 s (passé avec `vitest --changed`, qui n'y trouvait aucun test,
    d'où le choix ci-dessous) ; règle `tools/stylelint/plugins.js` → 0 captures, lints
    complets, 48 s ; `--all`, arbre propre → « OK — Tests 640 passed (640) ; captures
    2706 passed (15.3m) », la forme d'avant, 15 min 56 s. Après le passage à Vitest
    complet : composant `Switch.vue` → « OK — Tests 640 passed (640) ; ciblé : 881 captures sur
    2706 (881 passed (4.0m)) », 4 min 32 s. Après la relecture : spec supprimée seule
    (`components/switch.spec.ts`) → « ciblé : 0 captures sur 2699 (aucune touchée) »,
    42 s ; nouveau dossier de composant (`ui/src/components/Essai/`, non suivi) → 0
    captures, stylelint sur toutes les feuilles (seule différence des unités :
    `components/Essai`, donc rien sans lui), 40 s. `k-ci-lint` sans constat.
  - choix :
    - verdict : `OK — <Tests de Vitest> ; ciblé : N captures
      sur M (<passed>[ ; tout, <raison>]) ; K fichiers de <espace> changés depuis <base>` ;
      N et M comptés par `playwright test --list` (ni build ni serveur) ; une sélection
      vide ne lance pas le projet `visual` ; « tout » lance le projet entier. `--all` garde
      le verdict d'avant mot pour mot.
    - lints : sur les fichiers changés encore présents (prettier `--ignore-unknown`,
      eslint `--no-warn-ignored` sur `.js/.ts/.vue`, stylelint sur `.css/.vue`) ; sur
      tout l'espace quand le graphe rend « tout » (manifeste, configuration de prettier,
      eslint, stylelint…) ou qu'un fichier de `tools/` (règles de lint) ou un fichier
      caché de l'espace (`.prettierignore`) change ; stylelint sur toutes les feuilles
      quand une unité de `kartenn/component-prefix` (dossier de composant, fichier de
      `internal/` ou de `demo/`) apparaît ou disparaît entre la base commune et l'arbre
      de travail : l'attribution des préfixes bouge.
    - Vitest complet, ciblé ou non (choix de la session principale, ADR 0042,
      décision 2 amendée) : `vitest --changed` ne suit que le graphe de modules, et
      `ui/test/tokens.test.ts`, `fonts.test.ts`, `package.test.ts`, `sheets.test.ts`,
      `layout.test.ts` lisent jetons, feuilles et `.vue` sur le disque (l'essai du jeton
      n'y trouvait aucun test) ; ≈ 13 s. Des arguments Vitest donnés à l'appel restent
      prioritaires.
    - Playwright : specs de la 1re colonne en filtres de fichier, `--grep` = la 2e
      colonne jointe par `|` ; `--project=visual` (la forme séparée prendrait les specs
      pour des projets).
- [x] 4. Firefox, intégrité après un rechargement : reproduire sous charge l'erreur
  SRI dont l'empreinte est celle d'un corps vide (`shell.spec.ts`, « sidebar collapsed
  stays collapsed after a reload », et la tolérance de `moves.spec.ts`) ; écrire la
  cause ; corriger dans le shell si c'est un défaut au rechargement, sinon dans le
  parcours, puis retirer la tolérance de `moves.spec.ts` (ADR 0042, décision 5).
  - verify : le cas reproduit avant la correction (`--repeat-each`, sous charge) ;
    après, `--repeat-each=30` vert sous la même charge, Firefox et Chromium.
  - où : `web/v2/e2e/journeys/{shell,moves}.spec.ts` ; `web/v2/shell/vite.config.ts`
    (`modulepreload`, intégrité) ; `web/v2/shell/src/plugins/loader.ts`.
  - dépend de : —
  - cause (2026-11-01) : un parcours quitte la page (`goto`, `reload`) pendant qu'elle
    charge encore des modules : les plugins, chargés dans un moment d'inactivité après
    le premier écran, ou les vues paresseuses de l'accueil. Firefox annule ces chargements
    et vérifie l'empreinte de chacun sur les octets reçus, aucun :
    `ScriptLoader::OnStreamComplete` appelle `VerifySRI` avant de lire le statut du canal
    (`PrepareLoadedRequest`), d'où une « erreur d'intégrité » de la page quittée, dont
    l'empreinte calculée est le SHA-384 du vide (`dom/script/ScriptLoader.cpp`,
    searchfox.org/firefox-main ; aucun bogue Bugzilla trouvé qui le décrive). Pas un
    défaut du shell : la page rechargée est entière (plugins chargés, modules repris du
    cache) ; Chromium ne dit rien. Le cas `moves.spec.ts` (branche de la 021-24) est le
    même (`goto` juste après `login`), pas l'interception de Playwright. Les 3 échecs
    « focus » mesurés avant sont ce cas aussi, sur les vues paresseuses de l'accueil :
    empreinte du vide pour `HomeView`, `ViewPanel`, `slots`, `AttributeList`,
    `ExtensionElement`, et « Unable to preload CSS » de Vite pour la feuille
    d'`ExtensionElement` ; `login` n'attendait que l'URL `/`. `atRest` les couvre : le
    shell monte après `router.isReady()` (vue de l'accueil et ses dépendances chargées),
    puis seulement commence les plugins, dont la marque vient en dernier.
  - fait le 2026-11-01 (la tolérance de `moves.spec.ts`, absente de main, est retirée par
    la branche de la 021-24 après intégration de ce correctif, par la session principale) :
    marque `kartenn:plugins-fetched` (`PLUGINS_FETCHED`, `loader.ts`) quand chaque plugin
    a chargé son entrée (sans attendre `definePlugin`) ou est écarté, et que tous ses
    catalogues et modules sont réglés ; ou que les capacités sont illisibles. L'écart
    reste immédiat au premier échec (REQ-007-9) ; seule la marque attend le reste
    (`modulesSettled`, `preload.ts`). `atRest` (`e2e/fixtures/journeys.ts`), attendu par
    `loginIn` : la page de l'accueil quittée au repos, 15 s au plus (message qui nomme
    la marque et les modules de plugins encore en vol), sauf horloge installée dans le
    contexte (le moment d'inactivité attend alors sur elle). Tests :
    `test/plugins.test.ts` (écart au premier catalogue ou module refusé, marque après
    le règlement des autres ; marque après le chargement de l'entrée, `definePlugin`
    jamais appelé ; liste vide ; capacités illisibles). Avant : `shell.spec.ts` Firefox `--repeat-each=10`, 4 échecs sur 150
    (3 « neither the bar nor the band hides the focus », 1 « sidebar collapsed stays
    collapsed after a reload ») ; une sonde (rechargement ou `goto` dès la première
    requête d'un module de plugin) le reproduit 10 fois sur 10. Après :
    `shell.spec.ts` `--repeat-each=30`, Firefox et Chromium, 930 passés.
  - choix : une marque User Timing plutôt qu'un attribut du DOM (rien de visible, aucun
    style ni arbre d'accessibilité touché) ; `atRest` sans minuterie (un
    `PerformanceObserver`) ; tolérance du `describe("skeleton")` gardée : la requête du
    module y est refusée exprès (`route.abort`), même mécanisme de Firefox.
- [ ] 5. Firefox, fin de session dans le parcours `session` : reproduire sous charge
  l'avertissement absent (`session.spec.ts`, « the end of the session is said before it
  comes ») ; écrire la cause (horloge simulée, relecture de `/v1/me`, activité née d'un
  geste) ; corriger (ADR 0042, décision 5). Même traitement pour deux autres
  instabilités vues le 2026-11-01 : `[firefox] journeys/example.spec.ts:28`, axe
  `scrollable-region-focusable` sur `#k-surface` (deux passages complets, vert seul) ; et
  le script clavier `components/diff-list.spec.ts` (`toBeInViewport`, projet `visual`,
  vert relancé).
  - verify : chaque cas reproduit avant ; après, `--repeat-each=30` vert sous charge,
    Firefox et Chromium.
  - où : `web/v2/e2e/journeys/{session,example}.spec.ts` ; `web/v2/shell/src/session/` ;
    `web/v2/e2e/components/diff-list.spec.ts`.
  - dépend de : —
- [ ] 6. Règles et synchronisation : `.claude/agents/kartenn-tache.md` (passage ciblé
  seulement, `--all` réservé à la session principale) ; `CLAUDE.md` (« Charge de la
  machine », « Sorties courtes ») ; skill `kartenn-ui` (vérification) ;
  `specs/README.md` (ADR 0042, décision 3).
  - verify : relecture `kartenn-revue` sans écart bloquant.
  - où : fichiers cités.
  - dépend de : 1 à 3

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
- [ ] 3. `k-web2-check` ciblé : base `origin/main` par défaut, fichiers modifiés et non
  suivis ; captures, instantanés et scripts du graphe de la tâche 2 ; prettier, eslint,
  stylelint sur les fichiers touchés ; `vitest --changed` ; vue-tsc complet ; `--all`
  pour tout ; verdict qui dit la sélection (ADR 0042, décisions 1 et 2).
  - verify : essais sur quatre diffs jetables (un composant, une maquette, un jeton,
    une vue du shell hors maquettes) avec la sélection attendue et la durée ; `--all`
    rend le même verdict qu'avant la tâche.
  - où : `devenv.nix` (`scripts.k-web-check`, `scripts.k-web2-check`).
  - dépend de : 2
- [ ] 4. Firefox, intégrité après un rechargement : reproduire sous charge l'erreur
  SRI dont l'empreinte est celle d'un corps vide (`shell.spec.ts`, « sidebar collapsed
  stays collapsed after a reload », et la tolérance de `moves.spec.ts`) ; écrire la
  cause ; corriger dans le shell si c'est un défaut au rechargement, sinon dans le
  parcours, puis retirer la tolérance de `moves.spec.ts` (ADR 0042, décision 5).
  - verify : le cas reproduit avant la correction (`--repeat-each`, sous charge) ;
    après, `--repeat-each=30` vert sous la même charge, Firefox et Chromium.
  - où : `web/v2/e2e/journeys/{shell,moves}.spec.ts` ; `web/v2/shell/vite.config.ts`
    (`modulepreload`, intégrité) ; `web/v2/shell/src/plugins/loader.ts`.
  - dépend de : —
- [ ] 5. Firefox, fin de session dans le parcours `session` : reproduire sous charge
  l'avertissement absent (`session.spec.ts`, « the end of the session is said before it
  comes ») ; écrire la cause (horloge simulée, relecture de `/v1/me`, activité née d'un
  geste) ; corriger (ADR 0042, décision 5).
  - verify : le cas reproduit avant ; après, `--repeat-each=30` vert sous charge,
    Firefox et Chromium.
  - où : `web/v2/e2e/journeys/session.spec.ts` ; `web/v2/shell/src/session/`.
  - dépend de : —
- [ ] 6. Règles et synchronisation : `.claude/agents/kartenn-tache.md` (passage ciblé
  seulement, `--all` réservé à la session principale) ; `CLAUDE.md` (« Charge de la
  machine », « Sorties courtes ») ; skill `kartenn-ui` (vérification) ;
  `specs/README.md` (ADR 0042, décision 3).
  - verify : relecture `kartenn-revue` sans écart bloquant.
  - où : fichiers cités.
  - dépend de : 1 à 3

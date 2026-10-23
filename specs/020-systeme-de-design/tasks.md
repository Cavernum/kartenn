---
status: approved
updated: 2026-10-23
---

# 020 — Système de design : tâches

Validée le 2026-10-23 ([`decisions.md`](decisions.md)). Chaque tâche est un commit, laisse le build vert
(`k-web-check`) et nomme ce qu'elle implémente ; chacune porte `où :` et `dépend de :`.
D-1 est tranché (Vue 3, Reka UI encadré : REQ-020-33) ; les marques **[D-1]** sont levées.
Bascule en un bloc (021, § « Migration », 2026-10-23) : tout le code nouveau va sous
`web/v2/` (espace pnpm distinct, non servi) ; les chemins `web/shell`, `web/sdk`, `web/e2e`
des lignes `où :` désignent l'existant à reprendre ou à imiter.
Commandes lourdes par `k-heavy`.

Ordre : P0, P1, puis P2. P3 (version visuelle et
maquettes) précède P4 (composants complets) ; **la bibliothèque s'écrit séquentiellement,
par un seul agent à la fois** (ADR 0039, § 6) ; P4 ne se parallélise pas. Les écrans de
la 021 attendent la tâche 15 (maquettes validées) et la famille de composants qu'ils
emploient.

## P0 — Références et outillage

- [ ] 1. Note de référence `docs/reference/systeme-de-design.md` : critères WCAG 2.2
  retenus (1.4.1, 1.4.3, 1.4.10, 1.4.11, 1.4.13, 2.1.4, 2.2.1, 2.3.3, 2.4.7, 2.4.11,
  2.5.7, 2.5.8) avec leur texte normatif ; motifs APG de l'inventaire (rôles, états,
  clavier) ; couches en cascade (CSS Cascade 5) ; CSSOM et `style-src` (renvoi à
  `web-navigateur.md`) ; déterminisme des captures Playwright ; licences OFL et ISC ;
  normatif et choix de kartenn séparés — REQ-020-6, 11, 14, 18, 26, 28, 30 — verify :
  chaque affirmation a son URL et sa section ; points non relus marqués « à vérifier »
  - où : forme de `docs/reference/web-navigateur.md` (§ « Sources », « Ce que disent les
    sources », « Implications pour kartenn ») ; sous-agent `ref-researcher`
  - dépend de : —
- [ ] 2. Outillage Claude (ADR 0039) : agents `.claude/agents/ui-reviewer.md` et
  `a11y-auditor.md` ; skill `.claude/skills/kartenn-ui/SKILL.md` (contrat d'écriture,
  inventaire à compléter au fil de P4, clichés, raccourcis réservés) chargé sur `web/**` ;
  hook `PostToolUse` sur `Edit|Write` de `web/**` (Prettier, Stylelint, ESLint du seul
  fichier, sortie d'une ligne) ; `bench-runner` étendu à `bench-web` (tableau des
  budgets) ; une ligne dans CLAUDE.md qui renvoie au skill — REQ-020-1, 28 — verify :
  édition d'un `.css` avec une couleur brute → le hook rend le diagnostic ; les deux
  agents rendent une liste d'écarts sur une page du shell actuel
  - où : `.claude/agents/test-runner.md`, `bench-runner.md` (forme, frontmatter
    `tools`, `model`) ; `.claude/hooks/fin-tache.sh` (forme d'un hook) ; ADR 0039
  - dépend de : —
  - fait en partie (à cocher après les deux restes) : `.claude/agents/ui-reviewer.md`, `a11y-auditor.md` (sonnet ; Read, Grep, Glob,
    Bash, `mcp__playwright__*`) ; `.claude/skills/kartenn-ui/SKILL.md` (`paths: web/**`,
    contrat, CSS, CSP, inventaire avec statut, raccourcis réservés de la 021, pièges de
    Reka, clichés, outillage) ; hook `.claude/hooks/web-apres-edition.sh` déclaré dans
    `devenv.nix` (`claude.code.hooks.web-apres-edition`, `PostToolUse`,
    `Edit|MultiEdit|Write`) : outils de l'espace du fichier (`web/` ou `web/v2/`), une
    ligne `systemMessage` en succès, diagnostics en code 2 (vérifié : couleur brute dans
    un `.css` de `web/v2` → diagnostic Stylelint) ; `bench-runner` étendu à `bench-web`.
    Restent : la ligne de CLAUDE.md (non modifié par l'agent, texte proposé au rapport) ;
    l'essai des deux agents sur une page du shell actuel, à faire dans une session qui
    les charge (agents ajoutés en cours de session non invocables).
- [x] 3. Stylelint et lint du code : `web/v2/stylelint.config.js` (règles du design,
  § « Architecture CSS », dont la règle maison des couches et la liste fermée des
  composants flottants à ombre) ; interdiction des éléments interactifs bruts et du style
  en chaîne dans les écrans et plugins ; intégrés à `k-web-check` et au hook
  pre-commit — REQ-020-12, 13 — verify : fichier de cas interdits → échec ; corrigé →
  succès ; `vue/no-restricted-html-elements`, `vue/no-static-inline-styles`,
  `no-restricted-imports` des composants Reka interdits (REQ-020-33)
  - où : `web/eslint.config.js` ; `web/package.json` (scripts) ; `devenv.nix`
    `scripts.k-web-check` (vers la ligne 494) ; `.pre-commit-config.yaml` (entrée
    `eslint (web)`, forme)
  - dépend de : —
  - fait : `web/v2/stylelint.config.js` (`declaration-strict-value`, `use-logical`,
    `color-no-hex`, `function-disallowed-list`, `selector-max-*`, liste `FLOATING`) et
    quatre règles maison dans `web/v2/tools/stylelint/plugins.js` : `known-layer`,
    `token-reference` (seuls les `var(--k-…)` se lisent ; une propriété personnalisée hors
    jetons n'est qu'un alias de jeton), `raw-length` (longueur ou durée écrite, même dans
    `calc()`, que `declaration-strict-value` ignore ; `1px` en largeur de bordure
    seulement), `floating-shadow` (ombres de couche dans les feuilles de `FLOATING`,
    liseré `--k-shadow-inset-…` partout) ; `web/v2/eslint.config.js` : éléments bruts et
    `reka-ui` direct interdits dans `shell/src` (hors `_ui`) et `plugins/*/src`, liaison de
    style en chaîne, `v-html`, `cssText`, `<style>` créé, `setAttribute('style')`, parties
    Reka interdites partout ; tests de cas interdits et corrigés dans
    `web/v2/tools/test/` ; Stylelint lancé par `k-web-check` quand l'espace a une
    configuration (donc `k-web2-check`), hooks pre-commit `prettier`, `eslint`,
    `stylelint (web/v2)`. Choix de Claude : écrans et plugins n'importent pas `reka-ui`
    directement (ils passent par `@kartenn/ui`).

## P1 — Jetons, police, icônes

- [x] 4. Paquet `web/v2/ui` (`@kartenn/ui`, MPL-2.0) : `package.json` (`vue`, `reka-ui` en
  version exacte, `@tanstack/vue-virtual`), `LICENSE`, `src/index.ts`, `src/style.css`
  (ordre des couches en tête), espace pnpm — REQ-020-12, 30, 33 — verify : `pnpm -C web/v2/ui
  build` produit `style.css` et le module ; test de licences vert ; test : Reka sans `^`
  ni `~`
  - où : `web/sdk/package.json` et `web/sdk/LICENSE` (forme MPL-2.0) ;
    `web/pnpm-workspace.yaml` ; `web/shell/vite.config.ts` (`sharedSdk`, pour l'entrée de
    build en bibliothèque)
  - dépend de : 3
  - fait : `web/v2/ui/` (`package.json` MPL-2.0, `reka-ui` 2.11.0 exact, `vue`,
    `@tanstack/vue-virtual` ; `LICENSE`, texte officiel de la MPL 2.0 ; `src/index.ts`,
    `src/style.css` qui importe d'abord `styles/layers.css`) ; `vite.config.ts` en
    bibliothèque (`dist/index.js`, `dist/style.css`, `vue`, `reka-ui` et
    `@tanstack/vue-virtual` externes) ; `test/package.test.ts` (licence et en-têtes, Reka
    exact, ordre des couches) avec `web/v2/tools/notices.ts` copié du SDK. Choix de
    Claude : la règle `kartenn/known-layer` laisse les `@import` (les feuilles importées
    se lintent elles-mêmes), sans quoi `style.css` ne pouvait importer une feuille qui
    déclare sa propre couche.
- [x] 5. Jetons : `tokens/tokens.css` (table du design § « Jetons » : couleurs, états,
  environnements, espacement, rayons, bordures, élévation, typographie, densité,
  mouvement ; clair, sombre, confortable), `styles/reset.css`, `base.css`,
  `utilities.css` ; test de contraste étendu à toutes les paires déclarées ; liste des
  jetons publics égale au fichier — REQ-020-2 à 9, 11 — verify : Vitest (contrastes dans
  les deux modes, densité 28/36 px, durées nulles sous mouvement réduit, teintes
  d'environnement ≠ états)
  - où : `web/shell/src/styles/tokens.css`, `tokens.test.ts` (`block`, `luminance`,
    `contrast` à reprendre) ; `web/sdk/src/tokens.ts` (liste, renommée à la version
    majeure du SDK, tâche de la 021) ; direction « Ardoise » (design, table des
    couleurs)
  - dépend de : 4
  - fait : `web/v2/ui/src/tokens/tokens.css` (clair compacte, sombre du système et forcé,
    confortable, mouvement réduit), `tokens/index.ts` (`tokens`, `STATES`, `ENV_HUES`,
    exportés par `@kartenn/ui`), `styles/reset.css`, `base.css`, `utilities.css` ;
    `test/tokens.test.ts` : contrastes de toutes les paires déclarées dans les deux modes,
    `color-mix` en Oklab recalculé par le test, densités, durées, teintes, liste = fichier.
    Écart consigné (REQ-020-5) : le test a montré `ok` et `warn` clairs sous 4,5:1 sur
    leur propre fond atténué (4,27 et 4,41) ; foncés à `#147942` et `#9d5800`, table du
    design mise à jour. Constat : `--k-env-1` clair vaut l'accent, donc l'état `info` ;
    le test vérifie la distinction d'avec les quatre états de REQ-020-7 (à confirmer par
    l'utilisateur). Choix de Claude : jetons `--k-z-*` (empilement), `--k-icon-stroke`,
    `--k-env-dot-size`, `--k-shadow-inset-accent`, `--k-text-<taille>-line` pour
    l'interligne ; familles avec un repli métrique `"Geist Fallback"` (tâche 6).
- [ ] 6. Polices Geist et Geist Mono : `woff2` variables, sous-ensembles latin et latin
  étendu, `tokens/fonts.css` (`@font-face`, `font-display: swap`, `size-adjust` du
  repli), préchargement de la police sans, licence OFL livrée — REQ-020-10, 30 —
  verify : aucune requête de police hors origine (parcours) ; poids mesuré par
  `size.mjs` ≤ 120 Ko ; colonne de chiffres alignée (capture)
  - où : `web/shell/index.html` (`<link rel=preload>`, pas de style en ligne) ;
    `crates/kartenn-rest/src/web.rs` (types de média : `woff2` déjà servi) ;
    `web/shell/scripts/size.mjs`
  - dépend de : 4
  - fait en partie : `web/v2/ui/src/fonts/` (quatre `woff2` variables droits, latin et
    latin étendu, tirés sans retouche de `@fontsource-variable/geist` et `geist-mono`
    5.3.0, 83 708 octets ; `OFL-Geist.txt`, `OFL-GeistMono.txt`), `tokens/fonts.css`
    (`@font-face` en `@layer tokens`, `swap`, replis métriques `"Geist Fallback"` et
    `"Geist Mono Fallback"` sur Arial ou Liberation, `size-adjust` et surcharges
    calculés par fontTools) ; `test/fonts.test.ts` (fichiers, signature WOFF2, ≤ 120 Ko,
    déclarations, licence). Le build en bibliothèque de `@kartenn/ui` incorpore les
    polices en `data:` (mode bibliothèque de Vite) : c'est le build du shell qui les
    sert en fichiers à empreinte. Restent, avec le shell (tâches 8, 9) : préchargement
    dans `index.html`, parcours sans requête hors origine, capture de la colonne de
    chiffres.
- [ ] 7. Icônes : `@kartenn/ui/icons` (réexport de la liste fermée Lucide, trait 1,5,
  tailles 16 et 20) ; icônes métier dessinées (liste du design § « Icônes ») ; composant
  `Icon` (`aria-hidden` par défaut, nom obligatoire si porteuse de sens) — REQ-020-15 —
  verify : planche relue par l'utilisateur avec les maquettes (Q-020-7) ; test de type :
  icône porteuse de sens sans nom → erreur de compilation
  - où : design § « Icônes » ; `web/v2/ui/src/icons/`
  - dépend de : 4

## P2 — Catalogue et captures

- [ ] 8. Catalogue `/_ui` nu : routes hors production (index, jetons, icônes, page de
  composant générique qui lit `<Nom>.catalogue.*`), commutateurs mode/densité/langue dans
  l'URL ; absent du build de production — REQ-020-24 — verify : test sur le manifeste du
  build de production (aucun module `_ui`) ; parcours `/_ui/jetons` sans violation axe

  - où : `web/shell/src/router.ts` (routes paresseuses), `web/shell/vite.config.ts`
    (mode), `web/shell/src/theme.ts` (mode forcé) ; design § « Catalogue »
  - dépend de : 5, 6, 7
- [ ] 9. Régression visuelle et contrôles transverses : projet Playwright `visual`
  (Chromium, 1280 × 800, échelle 1, mouvement réduit, horloge, locale et fuseau fixes,
  `vite preview` sur `127.0.0.1:4173`) ; aide `snap(page, état)` (capture et instantané
  ARIA) ; parcours `csp.spec.ts` (aucune `securitypolicyviolation`) ; commande devenv
  `k-web-shots [--update]` sous `k-heavy`, refusée hors de devenv ; `k-web-check` lance
  `visual` — REQ-020-14, 26, 27 — verify : un jeton modifié fait échouer la capture de
  `/_ui/jetons` ; `k-web-shots` hors devenv → refus
  - où : `web/e2e/playwright.config.ts`, `web/e2e/budgets.config.ts` (forme d'un projet
    séparé) ; `web/e2e/fixtures/index.ts` (`expectAccessible`) ; `devenv.nix`
    `scripts.k-web-e2e` (vers la ligne 533, forme de la commande)
  - dépend de : 8

## P3 — Version visuelle des composants et maquettes

Composants rendus avec leurs jetons et leurs états visuels, sans tout le comportement
clavier (complété en P4) ; ce qu'il faut pour construire les maquettes.

- [ ] 10. Version visuelle : `Stack`, `Cluster`, `Grid`, `Split`, `Button`,
  `IconButton`, `TextField`, `Kbd`, `StatusBadge`, `KeyLabel`, `EnvChip`, `CodeBadge` —
  REQ-020-16, 17, 21, 22 — verify : pages de catalogue et capture d'alignement (bouton,
  champ, pastille, bouton-icône côte à côte)
  - où : design § « Inventaire » (lignes 1 à 4, 22 à 24, 28) ; `web/v2/ui/src/components/`
  - dépend de : 9
- [ ] 11. Version visuelle : `SidebarNav`, `Breadcrumb`, `DatePill`, `DraftCounter`,
  `TimeBand`, `Tabs`, `Menu`, `Drawer`, `Dialog`, `CommandPalette` — REQ-020-16 —
  verify : pages de catalogue, captures
  - où : design § « Inventaire » (lignes 10 à 13, 17 à 19, 27) ; écran de la direction
    « Ardoise » (barre latérale, barre d'état, bande)
  - dépend de : 10
- [ ] 12. Version visuelle : `DataGrid`, `TreeGrid`, `FilterChip`, `DiffList`,
  `Skeleton`, `EmptyState`, `ErrorState`, `InlineAlert` (données statiques, sans
  virtualisation) — REQ-020-16, 19, 20, 23 — verify : pages de catalogue, captures
  (ligne en brouillon, en revue, cellule invalide)
  - où : design § « Inventaire » (lignes 6, 20, 21, 25, 26, 29) ; `web/v2/ui/src/fixtures/`
  - dépend de : 11
- [ ] 13. Maquettes E1 à E5 (référentiel, fiche d'un préfixe, liste d'adresses, changeset
  et revue, brouillon) en clair et en sombre — REQ-020-25 — verify : captures de
  référence ; revue `ui-reviewer` contre le brief, écarts listés ; poids de la
  bibliothèque mesuré (Q-020-9)
  - où : design § « Maquettes » ; `web/v2/shell/src/_ui/ecrans/` ; écrans actuels
    `web/shell/src/views/`, `record/`, `changesets/`, `draft/` (contenu, pas le style)
  - dépend de : 12
- [ ] 14. Maquettes E6 à E10 (lecture au passé, administration, palette et aide, états,
  hôte de plugin et matrice de la 017) — REQ-020-25 — verify : idem 13
  - où : design § « Maquettes » ; `web/shell/src/admin/`, `time/` ; spec 017, design
    « Plugin `policy` » (matrice) ; `web/plugins/vlan/src/index.ts` (contenu de la vue)
  - dépend de : 13
- [ ] 15. Validation des maquettes par l'utilisateur, sur captures en clair et en sombre ;
  validation et écarts acceptés consignés dans `decisions.md` (§ « Validations des
  maquettes ») ; écarts reportés dans ce design et dans celui de la 021 ; captures figées
  comme références — REQ-020-25 — verify : une ligne datée par maquette ; aucune tâche
  d'écran de la 021 ni de la 017 ne commence avant
  - où : `decisions.md` ; 017/U14 (amendée, A1)
  - dépend de : 14

## P4 — Composants complets (un agent, en séquence)

Pour chaque famille : tous les états de l'inventaire, script clavier APG
(`web/v2/e2e/components/<nom>.spec.ts`), instantané ARIA, captures, axe, revue
`a11y-auditor` puis `ui-reviewer`, ligne du skill `kartenn-ui` complétée.

- [ ] 16. `Button`, `IconButton`, `Kbd`, `ShortcutHint`, `Tooltip` — REQ-020-16 à 18,
  28 — verify : scripts Button et Tooltip (Échap ferme, 1.4.13) ; cibles ≥ 24 px
  - où : design § « Inventaire », § « Accessibilité » ; tâche 10
  - dépend de : 15
- [ ] 17. `TextField`, `FormField`, `Checkbox`, `Switch`, `InlineAlert` — verify :
  erreur liée par `aria-describedby` et annoncée ; case à trois états
  - où : idem ; `web/shell/src/edit/fields.ts` (champs typés actuels, comportement)
  - dépend de : 16
- [ ] 18. `Select`, `Combobox` (suggestions par une fonction asynchrone, débrayage) —
  verify : script Combobox (Alt+Flèche bas, Échap, Entrée, `aria-activedescendant`)
  - où : idem ; `web/shell/src/app/SearchBox.vue` (recherche actuelle)
  - dépend de : 17
- [ ] 19. `FilterChip`, `FilterPicker` : propriété → opérateur → valeur, listes fermées,
  champs typés, sérialisation et lecture dans l'URL — REQ-020-20 — verify : ajout,
  modification, retrait au clavier seul ; valeur hors type refusée ; aller-retour URL
  (Vitest)
  - où : design § « Inventaire » ligne 6 ; `web/shell/src/data/collections.ts` (filtres
    actuels par collection)
  - dépend de : 18
- [ ] 20. `DateTimeField`, `PeriodPicker` (fuseau nommé, raccourcis) — verify : script
  Date Picker Dialog ; saisie au clavier seul ; date invalide refusée
  - où : `web/shell/src/time/` (contexte temporel actuel)
  - dépend de : 18
- [ ] 21. `Menu`, `MenuButton`, `Popover`, `Tabs`, `Breadcrumb` — verify : scripts Menu
  et Tabs (activation manuelle) ; raccourcis affichés dans le menu
  - où : design § « Inventaire »
  - dépend de : 16
- [ ] 22. `Dialog`, `AlertDialog`, `Drawer` (modal et non modal), `Toast` — verify :
  focus piégé et rendu ; Échap ; toast d'erreur persistant, pause au survol (2.2.1)
  - où : `web/shell/src/draft/DraftDrawer.vue`, `admin/SecretDialog.vue` (usages actuels)
  - dépend de : 21
- [ ] 23. `SidebarNav` (repliable, groupes), `CommandPalette`, `DatePill`,
  `DraftCounter`, `TimeBand` — verify : palette (Combobox dans Dialog) au clavier ;
  barre latérale repliée : noms accessibles et infobulles
  - où : `web/shell/src/app/AppShell.vue`, `navigation.ts` (entrées actuelles)
  - dépend de : 22
- [ ] 24. `CodeBadge` (copie annoncée), `KeyLabel`, `EnvChip`, `StatusBadge`,
  `Skeleton`, `EmptyState`, `ErrorState`, `DiffList` — REQ-020-21 à 23 — verify : copie
  au clavier ; repli sur la clé ; CLS 0 entre squelette et données
  - où : `web/shell/src/admin/WriteOutcome.vue` (problèmes affichés aujourd'hui) ;
    `web/sdk/src/api/problem.ts` (`ApiProblem`, `requestId`)
  - dépend de : 21
- [ ] 25. `DataGrid` **maison** (Reka n'a pas de grille) : motif APG Grid complet (index de
  lignes et colonnes, tabindex itinérant, Flèches, Début/Fin, Ctrl+Début/Fin, Page,
  sélection Espace/Maj+flèches/Ctrl+A, tri `aria-sort`, filtre d'en-tête par Alt+Flèche
  bas), `@tanstack/vue-virtual`, colonnes redimensionnables au clavier, colonne
  d'extension, contrat d'édition en place (design, § « Contrat d'édition ») — REQ-020-18,
  19 — verify : 10⁴ lignes → quelques centaines dans le DOM ; script APG Grid complet (au
  moins les 10 cas du prototype) ; états `pending`, `draft`, `review`, `error` ; bancs
  filtre/tri/édition (tâche 27)
  - où : `web/shell/src/data/DataTable.vue`, `admin/PagedTable.vue` (actuels, à
    remplacer) ; APG Grid
  - dépend de : 24
- [ ] 26. `TreeGrid`, `Tree` : virtualisation, enfants paginés, `aria-setsize="-1"`,
  édition en place, mode de parenté (`derived`/`declared`), déplacement contrôlé
  (`canMove`, intention, états `pending`/`refused`), garde d'invariant — REQ-020-19, 31,
  32, 33 — Reka `TreeRoot`/`TreeVirtualizer` avec les correctifs du design § « Vue et
  Reka UI » (sélection par identifiant, `markRaw`, Haut/Bas maison, interception de `*`,
  Entrée, tampon de recherche, ligne focalisée gardée montée, nom court) — verify : 10⁵
  nœuds ; script Tree View/Treegrid ; 500 × Flèche bas → focus exact ; déplacement après
  sélection : le bon nœud ; `derived` : aucun moyen de
  déplacer (glisser, Ctrl+X, menu) ; `declared` : cible refusée jamais mise en évidence,
  dépôt sans effet, motif et code affichés ; « Déplacer vers… » sans cible refusée ; nœud
  en attente à sa place ; test de propriété (dépôts, refus, réponses tardives et
  remplacements aléatoires du modèle → parent visuel = parent du modèle) ; ≤ 5 % de
  trames perdues (banc de la tâche 27)
  - où : `web/shell/src/tree/` (arbre actuel, pagination des enfants) ; APG Treegrid ;
    design § « Contrat de déplacement »
  - dépend de : 25

## P5 — Budgets et clôture

- [ ] 27. Bancs de composants (`components-bench`, protocole du prototype D-1) et poids
  (`size.mjs`) : tableau du design § « Budgets » — REQ-020-29 — verify : tableau rendu par
  `bench-runner` ; un plafond dépassé → sortie non nulle
  - où : `web/e2e/budgets/budgets.spec.ts`, `web/shell/scripts/size.mjs` ;
    `devenv.nix` `scripts.bench-web` (vers la ligne 641)
  - dépend de : 26
- [ ] 28. Synchronisation : `docs/spec/` (architecture web : `@kartenn/ui`, couches,
  catalogue) ; ADR 0036, 0038, 0039 acceptées ;
  `specs/README.md` ; audit `spec-keeper` — verify : liens croisés ; aucun écart
  bloquant
  - où : forme des synchronisations précédentes (007, tâche 25 ; 022, tâche 18)
  - dépend de : 27

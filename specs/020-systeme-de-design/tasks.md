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

- [x] 1. Note de référence `docs/reference/systeme-de-design.md` : critères WCAG 2.2
  retenus (1.4.1, 1.4.3, 1.4.10, 1.4.11, 1.4.13, 2.1.4, 2.2.1, 2.3.3, 2.4.7, 2.4.11,
  2.5.7, 2.5.8) avec leur texte normatif ; motifs APG de l'inventaire (rôles, états,
  clavier) ; couches en cascade (CSS Cascade 5) ; CSSOM et `style-src` (renvoi à
  `web-navigateur.md`) ; déterminisme des captures Playwright ; licences OFL et ISC ;
  normatif et choix de kartenn séparés — REQ-020-6, 11, 14, 18, 26, 28, 30 — verify :
  chaque affirmation a son URL et sa section ; points non relus marqués « à vérifier »
  - où : forme de `docs/reference/web-navigateur.md` (§ « Sources », « Ce que disent les
    sources », « Implications pour kartenn ») ; sous-agent `ref-researcher`
  - dépend de : —
  - fait : note écrite d'après le brouillon de `ref-researcher` (pages lues par un outil de
    résumé : citations marquées « à vérifier mot à mot ») et la lecture complète des
    fichiers de licence des paquets épinglés ; indexée dans `docs/reference/README.md`.
    Constats : 2.3.3 est de niveau AAA (REQ-020-11 le garde, hors objectif AA) ; la
    licence de Lucide inclut une part MIT (icônes dérivées de Feather), livrée ; Geist ne
    déclare aucun nom réservé ; écart MDN/WPT sur `cssText` laissé au parcours CSP.
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
- [x] 6. Polices Geist et Geist Mono : `woff2` variables, sous-ensembles latin et latin
  étendu, `tokens/fonts.css` (`@font-face`, `font-display: swap`, `size-adjust` du
  repli), préchargement de la police sans, licence OFL livrée — REQ-020-10, 30 —
  verify : aucune requête de police hors origine (parcours) ; poids mesuré par
  `size.mjs` ≤ 120 Ko ; colonne de chiffres alignée (capture)
  - où : `web/shell/index.html` (`<link rel=preload>`, pas de style en ligne) ;
    `crates/kartenn-rest/src/web.rs` (types de média : `woff2` déjà servi) ;
    `web/shell/scripts/size.mjs`
  - dépend de : 4
  - fait (restes fermés par les tâches 8 et 9) : `web/v2/ui/src/fonts/` (quatre `woff2` variables droits, latin et
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
  - **à valider par l'utilisateur** (planche `/_ui/icones`, tâche 8) ; fait :
    `lucide` 1.61.0 épinglé, liste fermée de 68 icônes (`icons/lucide.ts`, licence ISC
    et MIT livrée en `icons/LICENSE-lucide.txt`) ; treize icônes métier proposées
    (`icons/metier.ts` : `prefix`, `range`, `address`, `domain`, `vlan`, `interface`,
    `cluster`, `flow`, `environment`, `exception`, `changeset`, `draft`, `matrix`) ;
    `@kartenn/ui/icons` ; `components/Icon/Icon.vue` (16 ou 20 px par
    `--k-icon-size`, trait `--k-icon-stroke`, `aria-hidden` par défaut, `meaningful`
    exige `label` au typage : `test/icons.test.ts`, `@ts-expect-error` vérifiés par
    vue-tsc). Choix de Claude : `matrix` ajouté (matrice de politique, 017) ;
    composant au rendu des nœuds de Lucide (pas de `@lucide/vue`), pour un seul rendu
    des icônes Lucide et métier ; la règle ESLint `multi-word-component-names` est levée
    pour les composants de la bibliothèque (noms de l'inventaire).

## P2 — Catalogue et captures

- [x] 8. Catalogue `/_ui` nu : routes hors production (index, jetons, icônes, page de
  composant générique qui lit `<Nom>.catalogue.*`), commutateurs mode/densité/langue dans
  l'URL ; absent du build de production — REQ-020-24 — verify : test sur le manifeste du
  build de production (aucun module `_ui`) ; parcours `/_ui/jetons` sans violation axe

  - où : `web/shell/src/router.ts` (routes paresseuses), `web/shell/vite.config.ts`
    (mode), `web/shell/src/theme.ts` (mode forcé) ; design § « Catalogue »
  - dépend de : 5, 6, 7
  - fait : `web/v2/shell/` (Vue, vue-router, `@kartenn/ui`) : `router.ts` ajoute les
    routes de `src/_ui/routes.ts` si `import.meta.env.MODE !== "production"` (branche
    éliminée au build de production) ; pages `/_ui` (index), `/_ui/jetons` (nuanciers,
    états, échelles, typographie, colonne de chiffres, densité, mouvement, valeurs lues
    sur la racine), `/_ui/icones` (planche de revue des icônes métier avec leurs voisines
    Lucide, puis la liste fermée), `/_ui/c/<composant>` (états de
    `<Nom>.catalogue.ts`, lus par `@kartenn/ui/catalogue` ; `?etat=` isole un état) ;
    commutateurs `?mode=`, `?densite=`, `?langue=` sur la racine (`data-theme`,
    `data-density`, `lang`) ; `vite build --mode catalogue` vers `web/v2/dist/catalogue`,
    publié en artefact par `.github/workflows/ci.yml` ; préchargement de Geist par un
    greffon Vite (`preloadFont`, `<link rel=preload>` au nom à empreinte, tâche 6) ;
    `test/build.test.ts` (aucun module `_ui` au manifeste de production, polices en
    fichiers, préchargement, aucun style en ligne dans `index.html`). Le parcours axe de
    `/_ui/jetons` est dans le projet `visual` (tâche 9). Choix de Claude : jeton
    `--k-measure-prose` (72ch) ; la langue ne change pour l'instant que `lang` (pas encore
    de catalogue de traduction dans le nouveau shell).
- [x] 9. Régression visuelle et contrôles transverses : projet Playwright `visual`
  (Chromium, 1280 × 800, échelle 1, mouvement réduit, horloge, locale et fuseau fixes,
  `vite preview` sur `127.0.0.1:4173`) ; aide `snap(page, état)` (capture et instantané
  ARIA) ; parcours `csp.spec.ts` (aucune `securitypolicyviolation`) ; commande devenv
  `k-web-shots [--update]` sous `k-heavy`, refusée hors de devenv ; `k-web-check` lance
  `visual` — REQ-020-14, 26, 27 — verify : un jeton modifié fait échouer la capture de
  `/_ui/jetons` ; `k-web-shots` hors devenv → refus
  - amendé par la 024 (2026-10-24) : maquettes et écrans capturés aussi à 1440, 1024 et
    768 px (REQ-020-26) ; repris par la tâche 11 de la 024.
  - où : `web/e2e/playwright.config.ts`, `web/e2e/budgets.config.ts` (forme d'un projet
    séparé) ; `web/e2e/fixtures/index.ts` (`expectAccessible`) ; `devenv.nix`
    `scripts.k-web-e2e` (vers la ligne 533, forme de la commande)
  - dépend de : 8
  - fait : `web/v2/e2e/` (`@playwright/test` 1.63.0 comme `web/e2e`, `@axe-core/playwright`) ;
    `playwright.config.ts`, projet `visual` (Chromium, 1280 × 800, échelle 1, clair forcé
    par projet et mode par l'URL, `reducedMotion`, `fr-FR`, `Europe/Paris`, horloge figée par
    `open`), serveur `vite preview --mode catalogue` du build de test sur `127.0.0.1:4173`,
    qui envoie la CSP de kartenn-server ; références sans suffixe de plateforme sous
    `visual/__screenshots__/` ; `fixtures/index.ts` (`open`, `snap` : capture pleine page et
    `toMatchAriaSnapshot`, `expectAccessible` repris de `web/e2e`, `violations`) ;
    `visual/catalogue.spec.ts` (index, jetons dans les deux densités, icônes, chaque état de
    chaque composant déclaré, dans les deux modes ; axe sur chaque page) ;
    `visual/csp.spec.ts` (tout le catalogue : aucune `securitypolicyviolation`, aucune
    requête hors origine, polices servies sous `/assets/`) ; devenv : `k-web-check` lance
    `visual` quand l'espace a `e2e/visual` (donc `k-web2-check`), `k-web-shots [--update]`
    sous `k-heavy`, refusé hors de devenv (vérifié), `k-web2-e2e`. Vérifié : un jeton
    modifié (`--k-accent` `#0b5cad` → `#0b6cad`) fait échouer la capture de `/_ui/jetons`
    (3 991 pixels) — seulement après avoir abaissé `threshold` (écart de couleur par pixel,
    0,2 par défaut) à 0,01 : avec le défaut, la capture passait (choix de Claude, à
    confirmer : le design ne fixait que `maxDiffPixelRatio`). Ferment aussi les restes de
    la tâche 6 (préchargement, aucune requête de police hors origine, colonne de chiffres
    capturée sur `/_ui/jetons`) et le parcours axe de la tâche 8.

## P3 — Version visuelle des composants et maquettes

Composants rendus avec leurs jetons et leurs états visuels, sans tout le comportement
clavier (complété en P4) ; ce qu'il faut pour construire les maquettes.

- [x] 10. Version visuelle : `Stack`, `Cluster`, `Grid`, `Split`, `Button`,
  `IconButton`, `TextField`, `Kbd`, `StatusBadge`, `KeyLabel`, `EnvChip`, `CodeBadge` —
  REQ-020-16, 17, 21, 22 — verify : pages de catalogue et capture d'alignement (bouton,
  champ, pastille, bouton-icône côte à côte)
  - où : design § « Inventaire » (lignes 1 à 4, 22 à 24, 28) ; `web/v2/ui/src/components/`
  - dépend de : 9
  - fait : `Stack`, `Cluster`, `Grid`, `Split` (écart `data-k-gap` sur l'échelle, feuille
    commune `components/shared.css` avec l'indicateur `k-spinner`), `Button` (quatre
    variantes, trois tailles, icône, raccourci `Kbd` + `aria-keyshortcuts`, occupé =
    `aria-disabled` + `aria-busy`, bascule `aria-pressed`), `IconButton` (nom obligatoire
    au typage), `TextField` (étiquette, aide et erreur par `aria-describedby`, lecture
    seule, en attente, chasse fixe, étiquette cachée), `Kbd` (`kbd` imbriqués, noms de
    touches dans la langue), `StatusBadge`, `KeyLabel`, `EnvChip`, `CodeBadge` (copie,
    annonce `status`, nom = contenu) ; `i18n.ts` (`setLocale`, `t`), `clipboard.ts`,
    `fixtures/` (blocs de documentation RFC 5737/3849) ; catalogue : états avec `slot`,
    `demo`, `interaction` (survol, focus, appui joués avant la capture), `wide`, `isolated` ;
    captures : un état par mode, plus chaque page en densité confortable (choix de Claude :
    les quatre combinaisons par état doublaient la durée du projet `visual`, déjà ≈ 20 min
    sous charge) ; état `alignment` de `Button` (bouton, champ, bouton-icône, pastilles par
    taille). Jetons ajoutés (design, « Couleurs », « Mouvement », « Tailles ») :
    `--k-hover`, `--k-active`, `--k-accent-hover`, `--k-danger*`, `--k-scrim`,
    `--k-duration-spin`, `--k-ease-linear`, `--k-size-*`. Lint : un angle d'images clés
    (`rotate(1turn)`) est permis ; test `sheets.test.ts` : toute propriété lue existe.
    Infobulle de `IconButton`, clé en infobulle et copie de `KeyLabel` : version complète
    (tâches 16 et 24).
- [x] 11. Version visuelle : `SidebarNav`, `Breadcrumb`, `DatePill`, `DraftCounter`,
  `TimeBand`, `Tabs`, `Menu`, `Drawer`, `Dialog`, `CommandPalette` — REQ-020-16 —
  verify : pages de catalogue, captures
  - où : design § « Inventaire » (lignes 10 à 13, 17 à 19, 27) ; écran de la direction
    « Ardoise » (barre latérale, barre d'état, bande)
  - dépend de : 10
  - fait : `SidebarNav` (espaces en Disclosure, entrée active, compteurs, repliée en
    icônes avec les noms gardés, `[` en `aria-keyshortcuts`), `Breadcrumb` (dernier élément
    `aria-current`, clés en chasse fixe), `DatePill`, `DraftCounter`, `TimeBand`
    (`formatInstant` : langue et fuseau nommé), `Tabs` (Reka, activation manuelle, onglets
    de plugin en chargement et en erreur dits en mots), `Menu` (Reka DropdownMenu, bouton
    et menu en un composant, raccourcis à droite, entrée désactivée avec motif), `Dialog`,
    `AlertDialog` (Reka, focus sur Annuler), `Drawer` (non modal = `aside` qui pousse la
    surface ; modal = Reka Dialog), `CommandPalette` (Combobox `position="inline"` dans un
    Dialog, sans Viewport ; Reka laissait `aria-controls` vide : identifiant posé par
    nous). `provideLink` (liens du routeur) et `providePortalTarget` (le catalogue garde les
    couches dans `main`, pour les instantanés ARIA). Choix de Claude : `MenuButton` n'est
    pas un composant à part (le menu porte son bouton) ; les couches modales sont des états
    `isolated`, vérifiés par axe sur leur page sans la règle `aria-hidden-focus` (Reka cache
    le reste par `aria-hidden` et piège le focus) ; le parcours CSP visite ces états.
- [x] 12. Version visuelle : `DataGrid`, `TreeGrid`, `FilterChip`, `DiffList`,
  `Skeleton`, `EmptyState`, `ErrorState`, `InlineAlert` (données statiques, sans
  virtualisation) — REQ-020-16, 19, 20, 23 — verify : pages de catalogue, captures
  (ligne en brouillon, en revue, cellule invalide)
  - où : design § « Inventaire » (lignes 6, 20, 21, 25, 26, 29) ; `web/v2/ui/src/fixtures/`
  - dépend de : 11
  - fait : `DataGrid` (maison : `grid`, lignes et colonnes comptées et indexées, en-têtes
    `aria-sort`, tabindex itinérant, sélection, états de cellule `pending`, `draft`,
    `review`, erreur avec motif et code, états de ligne, éditeur en place ; gabarit
    `--k-grid-columns` en jetons, commun à l'en-tête et aux lignes, posé par CSSOM),
    `TreeGrid` (`flatten` : `aria-level`, `aria-setsize`, `aria-posinset`,
    `aria-expanded` ; états brouillon, enfants en chargement, cible acceptée ou refusée,
    déplacement en attente ou refusé avec motif et code ; garde `misplaced` hors
    production, REQ-020-32), `FilterChip`, `DiffList` (signe, mot caché, `del`/`ins`),
    `Skeleton`, `EmptyState`, `ErrorState` (code et `Request-Id` copiables), `InlineAlert`.
    API de la version complète déjà posée (contrat d'édition, `ParentMode`, `MoveCheck`) ;
    virtualisation, clavier, édition et déplacements : tâches 25 et 26. `Tree` (Reka) :
    tâche 26.
  - revues des tâches 10 à 12 (`ui-reviewer`, `a11y-auditor`, sur captures et code) :
    corrigés — bulle d'erreur de cellule coupée (côté selon la colonne, une ligne) ;
    ligne sélectionnée, en brouillon, en revue marquées autrement que par la teinte
    (liseré, icône, mot caché) ; option active de la palette et du menu avec liseré ;
    barre latérale (libellés tronqués, nom fixe du bouton de repli avec `aria-expanded`,
    `aria-keyshortcuts` de la palette, compteurs gardés repliée) ; focus initial d'un
    dialogue sur son premier champ ; pas d'`aria-describedby` sans description ; retour du
    focus à la fermeture du tiroir non modal ; motif d'un filtre invalide visible et lié,
    filtre en brouillon dit en mots ; alerte de `ErrorState` réduite à la phrase ; mot du
    squelette en `status` ; `aria-selected` de l'arbre seulement s'il sélectionne. Restent
    pour P4 : une alerte par cellule en erreur (fragile), annonce de `TimeBand` et
    `DraftCounter` au lecteur d'écran, alerte de `InlineAlert` qui contient les codes.
    À arbitrer (maquettes, tâche 15) : rayon plein de `DatePill`, barre latérale sombre
    en mode clair, motif d'une entrée de menu désactivée sur deux lignes.
- [x] 13. Maquettes E1 à E5 (référentiel, fiche d'un préfixe, liste d'adresses, changeset
  et revue, brouillon) en clair et en sombre — REQ-020-25 — verify : captures de
  référence ; revue `ui-reviewer` contre le brief, écarts listés ; poids de la
  bibliothèque mesuré (Q-020-9)
  - où : design § « Maquettes » ; `web/v2/shell/src/_ui/ecrans/` ; écrans actuels
    `web/shell/src/views/`, `record/`, `changesets/`, `draft/` (contenu, pas le style)
  - dépend de : 12
  - fait (avec la 14) : `/_ui/ecrans/<E1…E10>?vue=<id>` (`ecrans/registry.ts` : dix
    maquettes, 25 vues), hors du cadre du catalogue (pleine page, mêmes commutateurs dans
    l'adresse) ; page de revue `/_ui/revue` (chaque vue en clair, sombre, compacte,
    confortable ; planche des icônes ; composants à venir ; points à trancher), liens à
    l'index ; captures, instantané ARIA, axe et CSP par vue, mode et densité
    (`e2e/visual/ecrans.spec.ts`, 100 captures). Pièces locales aux maquettes, à trancher
    à la tâche 15 : cadre du shell (`EcranCadre`), panneau (`EcranPanneau`), liste
    d'attributs (`EcranAttributs`), repère « À venir » (`AVenir`, en tirets, nomme le
    composant P4 remplacé : `FilterPicker`, `PeriodPicker`, `Select`, `Checkbox`,
    `Popover`, `Toast`, `FormField`, `ShortcutHint`). Correctif de bibliothèque : `Tabs`
    borne la largeur de son panneau (une grille large le débordait). Poids (build en
    bibliothèque, `vue` et `reka-ui` externes) : JS 18,6 Ko gzip (`index` 14,8 +
    icônes 3,8), CSS 7,8 Ko gzip hors polices — sous B29 (90 et 20 Ko). Revues
    `ui-reviewer` et `a11y-auditor` faites ; corrigés : titre de page h1 (caché) et
    panneaux en h2, seul le panneau principal en région ; un bouton plein par zone (E4) ;
    un seul repère d'état par ligne ; motif visible près des boutons désactivés (E5) ;
    simulation en cours dite en mots ; noms distincts (« Lire au présent », « Retirer
    l'opération « Usage » sur … ») ; grilles sans débordement (E2, E10). Restent pour P4
    (composants) : bulle d'erreur de cellule qui couvre les lignes voisines et tronque le
    code (`DataGrid`) ; bouton désactivé non focalisable, donc motif inatteignable au
    clavier (`Button`, tâche 16) ; régions `status` sur du contenu statique
    (`InlineAlert`, `TimeBand`) et une région vide par `CodeBadge` ; grilles en lecture
    seule en rôle `grid` plutôt que `table` ; saisie initiale de la palette non montrée
    (`CommandPalette`) ; `aria-haspopup=dialog` de `DatePill`.
- [x] 14. Maquettes E6 à E10 (lecture au passé, administration, palette et aide, états,
  hôte de plugin et matrice de la 017) — REQ-020-25 — verify : idem 13
  - où : design § « Maquettes » ; `web/shell/src/admin/`, `time/` ; spec 017, design
    « Plugin `policy` » (matrice) ; `web/plugins/vlan/src/index.ts` (contenu de la vue)
  - dépend de : 13
  - fait : avec la 13 (même page de revue, mêmes captures). Choix de Claude : matrice de
    politique en `DataGrid` (une ligne par environnement source, verdict en
    `StatusBadge`), chronologie des versions en `DataGrid`, sélecteur de période en
    repère « À venir » sous la barre d'état ; points listés sur `/_ui/revue` pour la 15.
- [x] 15. Validation des maquettes par l'utilisateur, sur captures en clair et en sombre ;
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

- [x] 16. `Button`, `IconButton`, `Kbd`, `ShortcutHint`, `Tooltip` — REQ-020-16 à 18,
  28 — verify : scripts Button et Tooltip (Échap ferme, 1.4.13) ; cibles ≥ 24 px
  - où : design § « Inventaire », § « Accessibilité » ; tâche 10
  - dépend de : 15
  - fait : `IconButton` : infobulle automatique de son nom et de son raccourci
    (`shortcut`, `aria-keyshortcuts`), décorative et montée ouverte seulement ; refusé, son
    nom puis le motif, la description (`aria-describedby`) = le motif seul. `Button`,
    `IconButton` : un refus (`forbidden`, `reason`) l'emporte sur `disabled` (le motif
    reste atteignable : reste de la tâche 13) ; occupé ou refusé, un `submit` n'envoie
    rien. `useTooltip` : focus clavier seulement (`:focus-visible`, un clic n'ouvre rien),
    jamais sur un bouton dont le menu est ouvert (`aria-expanded`), un clic l'épingle
    (`pin`) ou la ferme ; Échap où que soit le focus (écouteur `window` en capture) :
    sous le pointeur seul, la ferme sans autre effet ; sous le focus, la couche autour
    reste, sauf `passEscape` ; le pointeur qui part laisse ouverte celle du focus ; un
    défilement ne la ferme que s'il déplace l'ancre. `ShortcutHint` (nouveau) : une
    combinaison ou une séquence (`[["g"], ["p"]]`, « g puis p »), `shortcutSteps`,
    `shortcutAria` (rien pour une séquence) ; `Kbd` : Maj, Origine, Fin, Pg préc./suiv.,
    Suppr, Retour arr. dans la langue ; l'état `sequence` de `Kbd` passe à `ShortcutHint`.
    E8 (aide) emploie `ShortcutHint` (repère « À venir » retiré, captures E8-aide
    réécrites). Catalogue : `Button` `forbidden`, `keyboard` ; `IconButton` `shortcut`,
    `forbidden` ; `Kbd` `shift` ; `ShortcutHint` (quatre états) ; un survol attend le délai
    de l'infobulle avant la capture. Scripts : `e2e/components/button.spec.ts` (Tab,
    Entrée, Espace, bascule, occupé, refusé, raccourci, instantané ARIA, cibles ≥ 24 px
    dans les deux densités) et `e2e/components/tooltip.spec.ts` (focus, survol,
    survolable, Échap sous le pointeur et sous le focus, persistant, menu ouvert, dans un
    dialogue), dans le projet `visual` (`testDir` `.`, références à côté de leur spec) ;
    `openState` dans `fixtures` ; test de l'infobulle de `clavier.spec.ts` déplacé ici.
    Captures réécrites (voulues) : `Button`, `IconButton` (survol, focus), `Kbd`, index,
    `SidePanel` `rail-focus`, `Toolbar` `forbidden`, `Tooltip` `icon` (l'infobulle d'un
    bouton-icône montre désormais son nom), E8-aide. Revues `a11y-auditor` (sur le code)
    et `ui-reviewer` (sur les captures) : corrigés, la persistance sous le focus, le
    défilement d'ailleurs, `disabled` avec un refus, Espace sur un refusé.
  - choix : l'infobulle qui ne fait que redire le nom d'un `IconButton` focalisé laisse
    Échap au contrôle (un tiroir, une grille ou un dialogue se ferme en un appui, comme
    avant), Échap ne fermant l'infobulle seule que sous le pointeur ; les scripts de P4
    vont sous `e2e/components/` dans le projet `visual` (lancés par `k-web2-check`).
    Restent hors de la tâche : `Menu` et `CommandPalette` montrent encore une séquence
    (`["g", "p"]`) dans un seul `Kbd` (tâches 21 et 23 : `ShortcutHint`) ; `Mod` vaut
    Ctrl partout, ⌘ sous macOS reste à faire (021, « Raccourcis ») ; à 390 px, la colonne
    « Portée » de l'aide E8 défile dans sa grille.
- [x] 17. `TextField`, `FormField`, `Checkbox`, `Switch`, `InlineAlert` — verify :
  erreur liée par `aria-describedby` et annoncée ; case à trois états
  - où : idem ; `web/shell/src/edit/fields.ts` (champs typés actuels, comportement)
  - dépend de : 16
  - fait : `FormField` (nouveau) : étiquette, erreur puis aide liées par
    `aria-describedby`, l'erreur dans une région `aria-live="polite"` présente vide avant
    elle (annoncée, dite de nouveau au focus) ; l'emplacement reçoit `id`, `labelId`,
    `describedBy`, `invalid`, `required` ; `group` : racine `group` nommée par
    l'étiquette et décrite (cases, bascules). `TextField` repose sur `FormField` (mêmes
    identifiants `<id>-error`, `<id>-hint`, en attente entre les deux) ; `CommitForm` y
    met sa description. `Checkbox` (nouveau) : case native, modèle `true`, `false` ou
    `"mixed"` posé sur `indeterminate` (exposé `mixed`, jamais `aria-checked`), mixte
    cochée au basculement ; étiquette englobante, rangée ≥ 24 px même étiquette cachée,
    aide hors du nom. `Switch` (nouveau) : `button` `role="switch"`, `aria-checked`,
    Espace et Entrée, nommé par `aria-labelledby`, pastille au bout et piste pleine
    (retournée en RTL). `InlineAlert` : contenu caché (`data-state="waiting"`,
    `visibility`) une image après sa région puis montré, donc annoncé, sans décalage.
    E3 (sélection) : « Tout sélectionner (1 284) » devient une `Checkbox` mixte (repère
    « À venir » retiré, `Checkbox` et `FormField` retirés de la liste de la revue).
    Catalogue : `TextField` `missing`, `keyboard` ; `FormField` `group`, `invalid`,
    `settings` ; `Checkbox` (onze états), `Switch` (neuf états) ; `InlineAlert` `icon`,
    `announce`. Scripts : `e2e/components/text-field.spec.ts` (erreur en quittant le champ,
    liée avant l'aide, dite dans la même région, remplacée, retirée ; lecture seule,
    désactivé, en attente, obligatoire ; groupe de `FormField`), `checkbox.spec.ts`
    (Espace, trois états, clic d'étiquette, Tab, cibles ≥ 24 px dans les deux densités),
    `switch.spec.ts` (Espace, Entrée, nom fixe, effet dit, cibles), `inline-alert.spec.ts`
    (région insérée en attente puis remplie, rôles) ; instantanés ARIA ; Vitest dans
    `components-base.test.ts`. Captures réécrites (voulues) : index (cinq composants de
    plus), pages entières `TextField`, `InlineAlert` (états ajoutés), E3-selection aux
    cinq largeurs (la case remplace le repère) ; nouvelles : `FormField`, `Checkbox`,
    `Switch`, les états ajoutés. Les autres captures, champs des maquettes et
    `CommitForm` compris, sont inchangées.
  - choix : erreur de champ en région `polite` (une erreur au départ du champ ne coupe
    pas la lecture du suivant) ; `InlineAlert` montré par `visibility` une image après sa
    région plutôt qu'inséré (pas de saut de hauteur) ; `Switch` en `button` (Entrée
    comprise) ; `FormField` sans espacement de grille (marges), pour qu'une région vide ne
    prenne pas de place. Restent hors de la tâche : la case de sélection des lignes dans
    `DataGrid` (colonne de cases, tâche de la grille) ; E7 n'a pas de formulaire de
    liaison à reprendre en `FormField` (l'entrée de la revue était sans repère).
- [x] 18. `Select`, `Combobox` (suggestions par une fonction asynchrone, débrayage) —
  verify : script Combobox (Alt+Flèche bas, Échap, Entrée, `aria-activedescendant`)
  - où : idem ; `web/shell/src/app/SearchBox.vue` (recherche actuelle)
  - dépend de : 17
  - fait : `Select` (nouveau) sur le `Select` de Reka (Select-Only Combobox : bouton
    `combobox` nommé par l'étiquette de `FormField`, erreur puis aide par
    `aria-describedby`, liste `listbox` nommée dont les options prennent le focus) ;
    Entrée, Espace, Bas, Haut ouvrent sur l'option choisie, flèches, Origine, Fin et
    saisie s'y déplacent, Échap ferme sans choisir, Tab choisit, ferme et passe au contrôle
    suivant ; `readonly` (`aria-readonly`, ne
    s'ouvre ni ne change à la saisie), `inlineLabel` (l'étiquette écrite avant la valeur,
    dans une barre), détail en chasse fixe compris dans le nom de l'option (2.5.3), aucune
    valeur = `undefined`. `Combobox` (nouveau) sur celui de Reka : texte en `v-model`,
    valeur prise en `v-model:value`, `suggest(texte, signal)` de l'hôte appelée après
    150 ms (`debounce`) et annulée par un texte plus récent ; sélection manuelle (la mise
    en avant automatique de Reka effacée), Bas va à la première, Alt+Bas ouvre sans
    bouger, Entrée prend la mise en avant (`select`) sinon `submit`, Échap ferme et laisse
    le suivant à la couche autour, Origine et Fin dans le texte, `aria-activedescendant`
    retiré à la fermeture, `aria-selected` sur la mise en avant, coche sur la valeur prise ;
    nombre, chargement, rien, échec dits par une région `polite` présente avant la liste et
    montrés dans une liste sans option ; indicateur de chargement à place gardée ; `filterOptions` pour
    une liste locale (casse et accents). Liste commune `Select/options.css` (dans
    `style.css`), au moins aussi large que le champ (CSSOM) ; cadre de champ partagé
    `k-field-frame` (`shared.css` : taille, survol, erreur, anneau, désactivé, lecture
    seule, champ et icônes) repris par `TextField`, `Select`, `Combobox`.
    Revues `a11y-auditor` et `ui-reviewer` (code et captures, tâches 17 et 18) ; corrigés :
    Tab dans la liste du `Select`, `aria-selected` de la suggestion mise en avant,
    `aria-live` écrit sur la région du `Combobox`, le ton d'`InlineAlert` dit en mots
    (caché, 1.3.1), pouce du `Switch` et option mise en avant visibles en couleurs forcées
    (contour transparent), `Select` de la barre E10 à la hauteur des boutons, champ du
    `Combobox` qui ne s'élargit plus au chargement, double indicateur, échec sans icône,
    doublons de règles de champ, descriptions de catalogue (`Select` `open`, `Checkbox`
    `mixed`).
    `SelectViewport` interdit (lint, `<style>` injecté). Maquettes : E1 (usage), E10 (type
    de flux, dans la barre), E12 (groupe) prennent un `Select` (repères « À venir » retirés,
    `Select` retiré de la revue) ; l'alignement de `Button` (REQ-020-17) a sa liste de
    choix. Catalogue : `Select` (treize états), `Combobox` (onze états, dont `open`,
    `loading`, `empty`, `failed` isolés). Scripts : `e2e/components/combobox.spec.ts`
    (nom, débrayage à un appel, aucune mise en avant à la saisie, Alt+Bas, Bas, Haut,
    Entrée, Entrée sans mise en avant, deux Échap, Origine et Fin, Tab, instantané ARIA,
    états sans suggestion, erreur liée), `select.spec.ts` (Bas, Entrée, Échap, Espace,
    Origine, Fin, saisie, Tab, lecture seule, erreur, barre, champs ≥ 24 px dans les deux
    densités) ; Vitest dans `components-base.test.ts` et `tools/test/eslint.test.ts`.
    Captures réécrites (voulues) : index (deux composants), `Button` `alignment` et page
    entière confortable (la liste de choix dans la rangée), E1-ajout, E10-matrice et
    E10-brouillon (1024 à 1440 px), E12-creation (le `Select` remplace le repère) ;
    `TextField` `pending` et page confortable (correctif : l'ancienne règle
    `:has(:read-only)` prenait le texte caché « Chargement » pour un champ en lecture seule
    et dessinait le cadre en pointillés) ; `Checkbox` `mixed` et page confortable
    (description) ; instantanés ARIA seuls (le ton d'`InlineAlert` dit) : `InlineAlert`,
    `SidePanel` `scroll-shade`, E2, E4, E7, E9, E11, E12-vlan, E13 ; nouvelles : `Select`,
    `Combobox`.
  - choix : `Combobox` en sélection manuelle (une recherche garde son texte à Entrée) ;
    `Select` laisse Reka cacher le reste de la page à l'ouverture (modal, focus dans la
    liste) ; fermé, la liste de Reka reste dans le champ (`SelectPortal` désactivé, une
    `div` vide ajoutait sinon un élément à la grille de la couche de l'hôte) ; feuille de
    liste partagée dans `style.css` (une feuille importée par un composant casse le build
    du shell). Restent des revues (à vérifier ou à arbitrer) : « obligatoire » dit deux
    fois (texte caché de l'étiquette et état `required`), `aria-busy` sur un champ en
    attente, la lecture du `Select` modal au lecteur d'écran ; hauteur maximale de la
    liste en `--k-size-dialog-sm` (comme `CommandPalette`, pas de jeton de liste) ; case
    de 16 px contre 14 px au design (§ « Densité ») ; mise en avant d'une ligne écrite trois
    fois (`Menu`, liste, `CommandPalette`) ; aucune capture d'option mise en avant (le
    catalogue ne joue pas de touche) ; `Combobox` sans chevron ni action « Réessayer » ;
    largeur propre du `Select` hors colonne de formulaire.
- [x] 19. `FilterChip`, `FilterPicker` : propriété → opérateur → valeur, listes fermées,
  champs typés, sérialisation et lecture dans l'URL — REQ-020-20 — verify : ajout,
  modification, retrait au clavier seul ; valeur hors type refusée ; aller-retour URL
  (Vitest)
  - où : design § « Inventaire » ligne 6 ; `web/shell/src/data/collections.ts` (filtres
    actuels par collection)
  - dépend de : 18
  - fait : `filters.ts` (nouveau) : modèle d'une liste (`FilterModel` : clé du paramètre,
    libellé, type `text`, `enum`, `number`, `prefix`, `address`, `date`, opérateurs servis,
    domaine fermé en `options` ou `suggest`, paramètre propre d'un opérateur), opérateurs
    `eq` (une valeur ou « l'un de »), `ne`, `contains`, `within` (un préfixe), `before`,
    `after`, `between` (bornes ouvertes), `exists`, `absent` ; `checkValue`, `checkFilter`
    (préfixe : forme, longueur, bits d'hôte avec le préfixe canonique RFC 5952 ; adresse
    IPv4 sans zéro de tête, IPv6 sous ses formes ; date réelle ; nombre ; liste fermée) ;
    `filtersToParams`, `filtersFromParams` dans la grammaire de la route (006 « Filtres » :
    `clé=v` répété, `clé:between=a..b`, `clé:exists=true|false`, `clé:<op>=v`), brouillons
    hors URL, paramètres étrangers gardés, valeur hors type relue et marquée invalide ;
    `filterWords`, `filterSentence` (libellé, sinon la clé en chasse fixe). `FilterPicker`
    (nouveau) : barre `group` nommée (pastilles, « Ajouter un filtre » avec son raccourci),
    popover non modal de Reka : propriété (`Combobox`, liste ouverte à l'ouverture, libellé
    puis clé, « Clé : » en aide une fois choisie), opérateur (`Select`, en lecture seule
    s'il n'y en a qu'un), valeur (`Combobox` d'un domaine fermé, plusieurs valeurs
    retirables pour `eq`, ajout et retrait dits ; sinon `TextField` typé avec son aide ;
    deux bornes pour `between`) refusée avant l'application, motif lié au champ et focus
    dessus ; deux Échap ; laissé à moitié, un filtre reste brouillon (pastille en
    pointillés, hors URL), « Annuler » l'écarte ; un filtre par paramètre (le nouveau
    remplace l'ancien) ; focus rendu à la pastille appliquée ou au déclencheur, le même
    déclencheur une seconde fois ferme ; appliqué et retiré dits par une région `polite` ;
    popover borné à la fenêtre et défilant (1.4.10). `FilterChip` : `propertyMono`,
    `expanded`, phrase qui va à la ligne dans sa pastille. Maquettes : E3 et E11 prennent
    `FilterPicker` sur un modèle typé (repères « À venir » retirés, `FilterPicker` retiré de
    la revue), colonnes filtrées d'E3 tirées des filtres appliqués ; E1 dit « dans » et
    « est ». Catalogue : `FilterPicker` (dix états : `rest`, `empty`, `draft`, `invalid`,
    `keys`, `open`, `values`, `refused`, `between`, `keyboard` avec l'URL écrite),
    `FilterChip` `key`. Script `e2e/components/filter-picker.spec.ts` (lecture de l'URL,
    ajout, valeur hors type, modification avec bits d'hôte, retrait, deux Échap, brouillon
    puis « Annuler », opérateur sans valeur et remplacement, Tab dans la liste des
    opérateurs, valeur retirée au clavier, déclencheur repris ou refermé, Tab hors de la
    barre) ; Vitest `ui/test/filters.test.ts` (grammaire, aller-retour sur 500 cas à
    graine fixe, vérifications par type, mots d'une pastille en deux langues, rendu).
    Revues `a11y-auditor` et `ui-reviewer` (code et captures) ; corrigés : popover borné
    à la fenêtre, ajout et retrait d'une valeur dits, clés stables des pastilles, focus
    sur la propriété inconnue, motif d'un `exists` hors valeur sur l'opérateur, déclencheur
    repris ou refermé, script au clavier seul (plus de `focus()` sur une option), pastille
    longue à 390 px (E11), clé de la propriété en aide, clé des valeurs lues par `suggest`,
    « Annuler » avant « Appliquer », brouillon rouvert sur sa pastille, colonnes filtrées
    d'E3, vocabulaire d'E1, commentaire orphelin, valeurs choisies d'un aspect neutre.
    Captures réécrites (voulues) : index (un composant de plus), `FilterChip` page entière
    (état `key`), E1-ajout, E1-noeud, E1-replie (mots « dans », « est »), E3-edition et
    E3-selection (le vrai `FilterPicker` à la place du repère, opérateurs en mots,
    « Propriétaire » non filtré tant qu'il est brouillon), E11-registre (de même ; à
    390 px la pastille « État » va à la ligne d'un bloc) ; nouvelles : `FilterPicker`,
    `FilterChip` `key`.
  - choix : popover non modal (design, leçon du prototype) nommé par `aria-label` (Reka
    cache les voisins de la liste ouverte) ; pastilles sans liste autour (le bouton reste
    sur la rangée) ; après la propriété, le focus va à l'opérateur s'il y en a plusieurs,
    sinon à la valeur ; plusieurs valeurs seulement par `eq` (paramètre répété = « l'un
    de ») ; `before`, `after`, `contains`, `ne` sérialisés en `clé:<op>` (la grammaire de
    la 006, servis seulement si le modèle tiré du contrat les déclare) ; `within` prend
    toujours un préfixe ; une valeur hors type lue dans l'URL y reste (l'URL tient ce que
    l'utilisateur a écrit), l'hôte n'envoie que ce que `checkFilter` accepte. Restent (à
    vérifier ou à arbitrer) : clé d'une pastille et d'une valeur choisie en infobulle et
    copiable (`KeyLabel` complet, tâche 24) ; une alerte par pastille invalide au
    chargement (conception de `FilterChip`, tâche 12) ; lecture au lecteur d'écran
    (annonce « Filtre appliqué » pendant que Reka lève `aria-hidden`, clé dans le nom d'une
    option) ; liste des propriétés qui dépasse le bord du popover de quelques pixels ;
    surface du popover recopiée de `.k-list` et du menu (`Popover`, tâche 21) ; le
    `Select` dit « est » même avec plusieurs valeurs ; aide de type répétée par l'erreur ;
    un Échap ou un Tab pendant le fondu d'une liste va à elle (Reka) ; modèle d'E3 sans
    filtres d'environnement, de site ni de VLAN alors que leurs colonnes ont un entonnoir
    (un modèle commun barre et colonnes, 021/024) ; deux Échap à dire dans l'aide (E8).
- [x] 20. `DateTimeField`, `PeriodPicker` (fuseau nommé, raccourcis) — verify : script
  Date Picker Dialog ; saisie au clavier seul ; date invalide refusée
  - où : `web/shell/src/time/` (contexte temporel actuel)
  - dépend de : 18
  - fait : `DateTimeField/dates.ts` (nouveau) : saisie par segments dans l'ordre d'ISO 8601
    (`parseEntry`, une saisie gardée telle que tapée), `checkEntry` (vide, incomplète, mois,
    jour du mois — 31 février —, heure, minute, année 0, heure sautée par le fuseau au
    passage à l'heure d'été ; de l'heure répétée, la plus tôt), valeur `AAAA-MM-JJ` ou
    instant RFC 3339 en UTC ; horloge murale d'un fuseau IANA par `Intl` seul (`wallAt`,
    `offsetAt`, `instantsOf`, `offsetLabel`), raccourcis (`shortcut` : maintenant à la
    minute, hier à la même heure, le 1er du mois à minuit). `DateTimeField` (nouveau) :
    `FormField` `group`, un `spinbutton` par segment (chiffres qui remplissent et avancent,
    séparateurs, Haut et Bas qui bouclent, Origine et Fin, Gauche et Droite, Retour arrière,
    Suppr, collage d'une date ISO, saisie d'un clavier virtuel par `beforeinput`, Entrée
    qui envoie le formulaire), `aria-valuenow` dans les bornes seulement, `aria-valuetext`
    (mois en mots, « vide »), fuseau et décalage écrits et décrivant l'heure, refus au
    départ du champ ou par `check()`, motif lié au groupe et à chaque segment, segment
    fautif souligné ; `DateCalendar` : bouton « Choisir la date : <étiquette> » (Alt+Bas)
    ouvrant un popover modal de Reka (`aria-modal`), grille du mois nommée par son titre
    `aria-live`, semaines du lundi, flèches, Origine, Fin, Pg préc./suiv. (Maj : année),
    Entrée ou Espace (minuit si l'heure manque), « Annuler », Échap. `PeriodPicker`
    (nouveau) : formulaire « Lire à une date », « Valide au » et « Connu au », raccourcis
    sous chacun dits par une région `polite`, refus à l'envoi (date impossible, `as_of`
    futur, 006) avec focus sur le champ, « Revenir au présent » hors présent ; `titleId`
    pour la couche de l'hôte. `fill` passe de `filters.ts` à `i18n.ts` ; icônes
    `chevrons-left`, `chevrons-right` ; `DateTimeField` dans la liste `FLOATING`. Maquette :
    E6 prend `PeriodPicker` (repère retiré, `PeriodPicker` retiré de la revue). Catalogue :
    `DateTimeField` (quatorze états : `empty`, `filled`, `summer`, `day`, `small`,
    `invalid`, `hour`, `gap`, `incomplete`, `readonly`, `disabled`, `focus`, `calendar`,
    `keyboard`), `PeriodPicker` (`present`, `past`, `both`, `refused`, `keyboard`).
    Scripts `e2e/components/date-time-field.spec.ts` (Tab de segment en segment, saisie au
    clavier seul, séparateurs et chiffres seuls, décalage d'été, bornes et boucles, 31
    février, 25 h et heure sautée refusés au départ du champ puis réparés, incomplète,
    Retour arrière, jour seul et 29 février ; grille : ouverture, flèches, Origine, Fin,
    Pg préc./suiv. et Maj, passage de mois, Échap, Tab piégé, Alt+Bas) et
    `period-picker.spec.ts` (ordre, Entrée qui applique, « Revenir au présent »,
    raccourcis, date impossible et `as_of` futur refusés) ; Vitest `ui/test/dates.test.ts`
    (calendrier, saisie, refus en deux langues, heures d'été, décalages, raccourcis,
    rendu). Revues `a11y-auditor` et `ui-reviewer` (code et captures) ; corrigés : focus
    du jour choisi invisible (anneau dehors), contour transparent pour les couleurs
    forcées (segment, jour choisi), motif lié à chaque segment, texte des segments caché
    (la valeur est `aria-valuetext`), `aria-valuenow` hors bornes retiré, décalage jamais
    coupé (le nom du fuseau seul), annonce répétée d'un même raccourci, clé inutilisée,
    anneau d'« aujourd'hui » (tables fusionnées par le reset), aujourd'hui et choisi,
    bouton à la hauteur du cadre (`sm`) et sans boîte désactivé, retrait de fin selon la
    présence du bouton, segment focalisé en lecture seule adouci, raccourcis alignés sur le
    bord du champ, formulaire non renommé quand l'hôte nomme sa couche (E6). Captures
    réécrites (voulues) : index (deux composants de plus), icônes (deux chevrons doubles),
    E6-selecteur aux cinq largeurs et son arbre (le vrai `PeriodPicker` à la place du
    repère) ; nouvelles : `DateTimeField`, `PeriodPicker`.
  - choix : composants écrits sans `@internationalized/date` (dépendance seulement
    transitive de Reka : `DateField` et `DatePicker` de Reka en demandent l'import direct
    et corrigent en silence un 31 février) ; ordre ISO dans les deux langues ; tous les
    segments dans l'ordre de tabulation (comme un champ date natif) ; segments
    `contenteditable` (`inputmode="numeric"`) pour les claviers virtuels ; valeur `null`
    tant que la saisie n'est pas valide, refus montré au départ du champ (comme
    `TextField`) ; dialogue nommé par son bouton (Reka écrase `aria-labelledby`), grille
    par le mois ; semaine du lundi (ISO 8601) ; « Hier » à la même heure ; « Appliquer »
    plutôt que « Lire à cette date » ; fuseau par défaut celui du navigateur, toujours
    écrit ; `PeriodPicker` sans surface propre (la couche est à la `DatePill`, tâche 23).
    Relecture avant fusion : un instant collé avec son `Z` ou son décalage est cet instant,
    montré dans le fuseau du champ (`pastedEntry`), une suite de séparateurs et d'espaces
    compte pour un (« 2025 - 11 - 03 09:30 ») ; un collage écrit la valeur s'il tient et ne
    refuse qu'au départ du champ, comme la frappe ; un instant dont l'année en UTC sort de
    0001…9999 est refusé (RFC 3339 `date-fullyear`) ; `as_of` futur refusé strictement à
    l'horloge du navigateur, le refus de l'API (`now()` de la base) rendu sur son champ par
    la prop `errors` (état `host`) ; aide de « Valide au » : passée ou à venir (REQ-007-23).
    Restent (à vérifier ou à arbitrer) : `FilterPicker` garde un `TextField` pour le type
    `date` (le prendre suppose `check()` à l'application, une valeur hors type lue dans
    l'URL et le focus sur un segment) ; lecture réelle des segments `contenteditable` au
    lecteur d'écran et sur mobile ; choix du fuseau par l'utilisateur (préférence) ;
    écriture d'une date dans le champ (« 2025 - 11 - 03 ») et dans une grille
    (« 2025-11-03 ») ; segment focalisé plein d'accent ou `accent-soft` ; élévation du
    calendrier ouvert depuis la couche de la pastille (tâche 23) ; « Lire au présent » de
    la barre d'E6 et « Revenir au présent » ; largeur d'un champ qui grandit avec son
    erreur dans un conteneur ajusté au contenu ; la couche d'E6 recouvre la page à 390 px
    (focus caché derrière, tâche 23).
- [x] 21. `Menu`, `MenuButton`, `Popover`, `Tabs`, `Breadcrumb` — verify : scripts Menu
  et Tabs (activation manuelle) ; raccourcis affichés dans le menu
  - où : design § « Inventaire »
  - dépend de : 16
  - fait : `Menu` : Haut sur le bouton fermé ouvre sur la dernière entrée, Bas et Haut en
    boucle, Tab et Maj+Tab ferment et passent au contrôle d'après ou d'avant le bouton
    (`internal/tabOrder.ts` : Reka, non modal, laissait le focus à la fin de la page) ;
    raccourci d'une entrée en `Shortcut` montré par `ShortcutHint` (une séquence « g puis
    p » en deux touches, jamais une combinaison ; combinaison en `aria-keyshortcuts`,
    séquence en description) ; états `shortcuts`, `checkbox`, `keyboard`. `Popover`
    (nouveau) : Dialog non modal de Reka, déclencheur `trigger` (`aria-haspopup="dialog"`,
    `aria-expanded`, marqué ouvert), nommé par son titre (`h2`, `aria-labelledby` posé à
    l'ouverture : Reka écrit le sien), focus sur le dialogue, Échap et la croix rendent le
    focus, Maj+Tab au début revient au déclencheur, Tab après la fin passe à ce qui le suit
    (écouteur en capture avant la boucle du `FocusScope`), borné à la fenêtre
    (`internal/room.ts`, partagé avec `FilterPicker`) ; états `closed`, `open`, `code`,
    `keyboard`. Surface commune `k-popover` (`Popover/surface.css`, dans `style.css`) prise
    par le menu et le `FilterPicker` (leur surface recopiée retirée). `Tabs` : état
    `keyboard`. `Breadcrumb` : un lien coupé par l'ellipse donne son libellé entier en
    infobulle (survol, focus) ; état `cut`. `CommandPalette` : raccourcis en `Shortcut`,
    séquences par `ShortcutHint` (catalogue, E8). `ShortcutHint` caché sous un doigt seul
    quand il l'est du nom. Maquettes : E2 prend `Popover` (bouton « Expliquer la valeur »
    dans la cellule « Source » de la valeur héritée ; nouvelle vue `explication`, le repère
    « À venir » et sa ligne de la revue retirés), « Changer de domaine… » désactivée avec son
    motif (elle avait le motif sans l'être). Scripts `e2e/components/menu.spec.ts`,
    `tabs.spec.ts`, `popover.spec.ts`. Revues `a11y-auditor` et `ui-reviewer` (code et
    captures) ; corrigés : popover d'E2 ouvert dans sa propre vue (il couvrait la colonne
    « Source » et les attributs), déclencheur d'E2 en bouton-icône après le texte (le
    texte gardait son alignement et son ton), déclencheurs des démos clavier à leur
    largeur, calcul de la place commun (`setRoom`), titre du popover en titre, sans
    `aria-describedby` (corps lu deux fois), déclencheur ouvert marqué, `leaving` remis à
    zéro à chaque ouverture ; popover gardé dans la fenêtre quand son déclencheur en sort
    (`sticky`, E2 à 390 px). Seconde revue (après la fusion de main) ; corrigés : corps du
    popover qui défile, arrêt de tabulation nommé par le titre tant qu'il déborde (2.1.1,
    1.4.10 ; le corps en colonne flexible, une piste `1fr` débordait au lieu de défiler ;
    état `long`) ; Tab qui sort d'un popover ou d'un menu borné au dialogue qui contient le
    déclencheur et en boucle, comme son piège (état `in-dialog`) ; gardes de focus de Reka
    hors de `tabbables()` (un Tab sans suite allait à la garde au lieu du déclencheur) ;
    `Select` passe par `internal/tabOrder.ts` ; infobulle du fil d'Ariane mesurée par
    miette ; scripts : Maj+Tab depuis « Fermer », repli sur le déclencheur, clic dehors,
    corps défilant à 320 × 256, popover et menu dans un dialogue modal,
    `e2e/components/breadcrumb.spec.ts` (lien entier sans infobulle, lien coupé).
    Captures réécrites (voulues) : index (un composant de plus),
    `Menu`, `Tabs`, `Breadcrumb` pages entières (états nouveaux), `CommandPalette` `open` et
    E8-palette (séquences), E2 (bouton d'explication dans la cellule, repère retiré ;
    menu d'actions avec « Changer de domaine… » désactivée) ; nouvelles : `Popover`, `Menu`
    `shortcuts`, `checkbox`, `keyboard`, `Tabs` `keyboard`, `Breadcrumb` `cut`,
    E2-explication.
  - choix : `MenuButton` reste le menu qui porte son bouton (choix de la tâche 11) ; menu en
    boucle (`loop`, comme l'exemple APG) ; Maj+Tab d'un menu va au contrôle avant le bouton
    (le menu est une liste du bouton, comme celle de `Select` : la quitter, c'est quitter le
    bouton), celui d'un popover au déclencheur (un dialogue qui se lit après lui, comme un
    contenu dévoilé : en revenir, c'est revenir au déclencheur) ;
    popover : focus sur le dialogue (son titre lu, ses contrôles ensuite), croix dans son
    en-tête, tailles `sm` et `md` ; surface partagée par une classe (`k-popover`) plutôt
    que `FilterPicker` réécrit sur `Popover` (ancre, ouverture tenue, focus propre). Restent
    (à vérifier ou à arbitrer) : texte coupé d'une miette sans lien ou de la page courante
    sans moyen de le lire (non focalisable ; la page courante est redite par le titre) ;
    infobulle d'un lien coupé inaccessible au doigt (un toucher suit le lien) ; miettes
    coupées toutes ensemble (« R… », « Cen… ») plutôt que repliées dans « … » ; liseré de
    focus épais autour d'un popover ouvert sans clic ; libellé d'un groupe du menu
    (« Aller à ») non lié à ses entrées (`group` nommé) ; raccourcis de la palette sans
    `aria-keyshortcuts` ni description (reportés sur la ligne de la tâche 23) ; surface du
    calendrier de `DateTimeField` (tâche 20) encore recopiée, hors `k-popover` ; ordre de lecture : la couche du
    catalogue est en tête de `main` (le dialogue avant son déclencheur en mode lecture) ;
    `.k-list` de `Select` garde sa surface recopiée ; lecture au lecteur d'écran (dialogue
    nommé, séquence « g puis p » en description) à faire.
- [x] 22. `Dialog`, `AlertDialog`, `Drawer` (modal), `Toast` — le panneau non modal est
  `SidePanel` (024, tâche 10) — verify :
  focus piégé et rendu ; Échap ; toast d'erreur persistant, pause au survol (2.2.1)
  - où : `web/shell/src/draft/DraftDrawer.vue`, `admin/SecretDialog.vue` (usages actuels)
  - dépend de : 21
  - fait : `Dialog`, `AlertDialog`, `Drawer` : `aria-modal="true"` écrit (Reka ne le pose
    pas) ; déclencheur du slot `trigger` fait déclencheur de Reka (`aria-haspopup="dialog"`,
    `aria-expanded` ; `ExpandableGrid` ne l'enveloppe plus lui-même) ; `internal/modal.ts` :
    `useOpener` (focus rendu à ce qui l'avait à l'ouverture, au bouton du menu quand c'était
    une entrée de menu, partie avec lui) et `focusFirstField` (`data-autofocus`, sinon le
    premier champ du corps ; `Drawer` aussi) ; `AlertDialog` : Échap et « Annuler »
    émettent `cancel`, le bouton de confirmation émet `confirm` puis ferme. `Toast`
    (nouveau, maison : le Toast de Reka ferme tous les toasts à tout Échap de la page) :
    `toasts.ts` (`createToasts`, `provideToasts`, `injectToasts`, `TOAST_DURATION`),
    `Toast.vue`, `ToastRegion.vue` (deux régions vivantes `alert` et `status` présentes
    vides, `aria-atomic="false"`, liste vide en `role="none"` ; repère « Notifications »
    tant qu'elle tient des toasts ; en bas au milieu de la fenêtre, sous une couche modale ;
    erreur, avertissement, toast à action sans durée, les autres 8 s ; pause au survol, au
    focus, page cachée ; Échap dans un toast le ferme ; focus au toast voisin, sinon d'où il
    venait, sinon au contenu principal ; entrée et sortie selon § « Mouvement ») ; tables
    `TONE_ICONS`, `TONE_WORDS` partagées avec `InlineAlert` (`alert.ts`). États `keyboard`
    de `Dialog` (bouton, entrée de menu), `AlertDialog`, `Drawer` ; `Toast` : `ok`,
    `error`, `stack`, `keyboard`. Scripts `e2e/components/dialog.spec.ts`,
    `toast.spec.ts` ; `ui/test/toasts.test.ts`. Maquettes : le cadre (`EcranCadre`) tient
    la région des toasts (prop `toasts`) ; E9 « revue » montre le 202 en toast (repère « À
    venir » et ligne de la revue retirés, table des composants à venir masquée vide).
    Revues `a11y-auditor` et `ui-reviewer` (code et captures) ; corrigés : région au milieu
    du bas (les coins tiennent « Soumettre » des tiroirs et du panneau), sous la couche
    modale (`--k-z-overlay` ; `--k-z-toast` reste celui des infobulles), haute de la moitié
    de la fenêtre au plus ; avertissement et toast à action sans durée (2.2.1) ; pause
    relâchée quand le dernier toast part (un nœud retiré n'émet ni `focusout` ni
    `pointerleave`) ; repli du focus sur le contenu principal ; listes vides hors de
    l'arbre ; 202 d'E9 et du catalogue alignés (code de motif, pas l'identifiant du
    changeset ; plus de code ni d'adresse en texte courant) ; croix centrée sur la première
    ligne ; phrase dans la figure des états isolés. Seconde revue (après la fusion de main,
    maquettes E14 à E27, aucune couche faite main) ; corrigés : couches flottantes (popover,
    menu, liste, calendrier, erreur de cellule) et feuilles de `SidePanel` et `BottomPanel`
    sur le plan des couches modales (`--k-z-modal` ; un popover ouvert dans un dialogue
    était sous son voile, une feuille sous les toasts) ; toasts retenus tant qu'une couche
    modale les couvre (`aria-hidden`, `inert`, observés) ; `returnTo` remis à rien quand le
    focus vient d'hors de la page ; Échap déjà pris (défaut empêché) laissé ; focus passé au
    toast voisin seulement au clavier, rien après un clic ; tons, icône, corps et titre
    communs à `InlineAlert` et `Toast` (`InlineAlert/tone.css`, dans `style.css`) ;
    `AlertDialog` n'émet `cancel` que pour Échap et « Annuler » ; scripts : popover sur le
    dialogue, toasts sous un dialogue et sous la feuille compacte, retenus sous le
    dialogue et page cachée, repli sur le toast précédent et sur le contenu principal,
    clic sans relais du focus. Captures réécrites (voulues) : index (un
    composant de plus), `Dialog`, `AlertDialog`, `Drawer` pages entières (état
    `keyboard`), E9-revue (toutes largeurs, aria) ; nouvelles : `Toast` (`ok`, `error`,
    `stack`, `keyboard`, pages), `Dialog`, `AlertDialog`, `Drawer` `keyboard`.
  - choix : Toast maison plutôt que celui de Reka ; atteint par Tab seulement (région après
    le reste du cadre), sans raccourci global (F8 serait un raccourci réservé de plus, spec
    021) ; une couche modale dit ses issues en elle (`InlineAlert`), un toast montré pendant
    qu'elle est ouverte attend sous son voile, caché ; durée de 8 s ; Échap dans un toast le
    ferme ; erreurs au-dessus des autres (deux régions) ; `AVenir.vue` gardé (le mécanisme
    de la revue sert encore). Restent (à vérifier ou à arbitrer) : raccourci pour atteindre
    les toasts ; focus rendu quand l'ouvreur a disparu et n'était pas une entrée de menu
    (ligne supprimée, action d'un toast : repli fourni par l'hôte ?) ; lecture au lecteur
    d'écran (toast lu en entier, noms des boutons compris ; régions `status` des `CodeBadge`
    imbriquées dans celles des toasts ; `aria-atomic="false"` sur `alert`) ; un toast
    ajouté sous une couche modale n'est pas redit quand elle se ferme ; texte d'un toast
    en chaînes seules (ni adresse ni code en chasse fixe dans la phrase).
- [ ] 23. `SidebarNav` (repliable, groupes), `CommandPalette`, `DatePill`,
  `DraftCounter`, `TimeBand` — verify : palette (Combobox dans Dialog) au clavier ;
  barre latérale repliée : noms accessibles et infobulles ; raccourci d'une entrée de la
  palette dit comme dans `Menu` (combinaison en `aria-keyshortcuts`, séquence en
  description, laissés par la tâche 21)
  - où : `web/shell/src/app/AppShell.vue`, `navigation.ts` (entrées actuelles) ; `DatePill`
    ouvre `PeriodPicker` (`web/v2/ui/src/components/PeriodPicker/PeriodPicker.vue`) et
    rend un 400 de l'API sur `valid_at` ou `as_of` sur son champ par la prop `errors`
    (état `host` du catalogue ; 006 compare `as_of` au `now()` de la base)
  - dépend de : 22
- [x] 24. `CodeBadge` (copie annoncée), `KeyLabel`, `EnvChip`, `StatusBadge`,
  `Skeleton`, `EmptyState`, `ErrorState`, `DiffList` — REQ-020-21 à 23 — verify : copie
  au clavier ; repli sur la clé ; CLS 0 entre squelette et données
  - où : `web/shell/src/admin/WriteOutcome.vue` (problèmes affichés aujourd'hui) ;
    `web/sdk/src/api/problem.ts` (`ApiProblem`, `requestId`)
  - dépend de : 21
  - fait : copie commune (`internal/copy.ts`, `useCopy`) : API asynchrone, à défaut la
    commande `copy` du document (son événement reçoit le texte : ni sélection ni focus
    déplacé, `clipboard.ts`), issue dite par une région `status` et montrée dans la bulle
    épinglée du bouton (« Copié » deux secondes ; les deux refusées : texte sélectionné,
    « copiez-le avec Ctrl+C » jusqu'à la fermeture de la bulle). `CodeBadge` : code coupé
    entier dans la bulle (survol, focus). `KeyLabel`, `EnvChip` : boutons qui copient la
    clé, nom = ce qu'ils montrent, description « Clé : prod. Clic, Entrée ou Espace : copier la
    clé. », clé dans la bulle ; `plain` (texte seul, dans un contrôle) ; `EnvChip` :
    teinte déduite de la clé (`envHue`, FNV-1a modulo 6, design « Environnements ») sauf
    `hue`, pastille cerclée de sa teinte (anneau gardé en couleurs forcées).
    `StatusBadge` : libellé par défaut de l'état. `ErrorState` : `problem` (`ProblemFacts`,
    un `ApiProblem` tel quel) : violations (message et règle), `type`, `Request-Id`
    copiables ; phrase en `alert` présente, montrée une image après. `TreeGrid` :
    `loading`, `loadingRows` (lignes de squelettes, arbre occupé) ; barre d'enfants en
    chargement sur `k-skeleton-fill`. `k-skeleton-fill` : bordure transparente (contour en
    couleurs forcées). `DiffList` : virtualisée au-delà de 200 lignes
    (`@tanstack/vue-virtual`, jeton `--k-size-list-max`), intervalles mesurés,
    `aria-setsize`/`aria-posinset`, en-têtes tous gardés. `FilterChip` : clés de la
    pastille (`filterQuery` : `usage=host`) en infobulle et en description. Catalogue :
    `CodeBadge` `cut`, `keyboard` ; `KeyLabel` `hover`, `focus`, `plain`,
    `fallback` ; `EnvChip` `hover`, `focus`, `plain`, `from-key` ; `StatusBadge`
    `defaults` ; `ErrorState` `precondition` (412), `violations`, `network` ; `DataGrid`
    et `TreeGrid` `settle` (`demo/settle.ts`), `TreeGrid` `loading` ; `DiffList` `long` ;
    `FilterChip` `query`. E7 : groupes en `KeyLabel` (clé à côté, montée à la main) ; E9 :
    `ErrorState` par `problem`. Scripts `e2e/components/code-badge.spec.ts` (copie au
    clavier, repli sur la commande, refus, code coupé, 412), `key-label.spec.ts` (repli
    sur la clé en anglais, copie, bulle, `EnvChip`), `skeleton.spec.ts` (CLS 0 grille et
    arbre), `diff-list.spec.ts`, `forced-colors.spec.ts`, `filter-picker.spec.ts`
    (clés d'une pastille) ; Vitest : boutons de clé, `envHue`, `filterQuery`, `ErrorState`,
    `TreeGrid` en chargement, `DiffList` sous 200 lignes. Revues `a11y-auditor` et
    `ui-reviewer` (code et captures) ; corrigés : cibles de `KeyLabel` et `EnvChip` de
    24 px sans place prise (marges négatives, `--k-size-line` ; cadre transparent autour de
    la pastille d'`EnvChip`) ; anneau de `KeyLabel` sur sa ligne (il était coupé par la
    cellule d'une grille, 2.4.7) ; la clé cède avant le libellé, le libellé coupé entier
    dans la bulle ; `stacked` (clé dessous) : E7 « Groupes » et « Droits effectifs » d'une
    même façon ; `DiffList` virtualisée positionnée (ses mots cachés étiraient la page) ;
    E11 : `KeyLabel` `plain` dans la grille des propositions (Entrée et le clic y
    choisissent la ligne) ; Échap d'une bulle qui en dit plus que le bouton (code coupé,
    clé cachée) ne ferme qu'elle ; curseur `copy` partout ; « Clic, Entrée ou Espace » ;
    « Ctrl+C, ⌘C sur Mac » ; `ErrorState` : violations alignées au début, `type` nommé,
    codes à transmettre écartés ; `TreeGrid` en chargement garde un arrêt de tabulation ;
    `KeyBubble` commun à `KeyLabel` et `EnvChip`. Captures réécrites (voulues) : `CodeBadge`
    `copied` (coche), pages entières confortables de `CodeBadge`, `KeyLabel`, `EnvChip`,
    `StatusBadge`, `ErrorState`, `FilterChip`, `DataGrid`, `TreeGrid`, `DiffList` (états
    nouveaux), `TreeGrid` `loading-children` (barre en `k-skeleton-fill`), E7 identité et
    rôles (groupes en `KeyLabel` empilé), E9 zones (`type` nommé), E13 politique-lecture
    (texte d'en-tête déjà changé sur main, référence restée sous la tolérance), instantanés
    ARIA des maquettes où une pastille devient bouton ; nouvelles : les états cités, et
    `e2e/components/__screenshots__/forced-colors.spec.ts/` (pastilles, états, squelette).
  - choix : copie de `KeyLabel` et `EnvChip` par le bouton qu'ils sont (nom = ce qu'ils
    montrent, l'action en description), plutôt qu'une icône de copie à côté (densité) ;
    repli sans API asynchrone par la commande `copy` du document plutôt qu'une zone de
    texte cachée (ni focus ni sélection déplacés ; après un refus de l'API, la commande
    part une microtâche plus tard, hors de l'activation : prouvé sur Chromium seul,
    d'autres moteurs peuvent la refuser, le chemin du refus tient alors) ; refus : bulle épinglée plutôt qu'un
    message dans le flux (aucun décalage) ; teinte d'environnement par FNV-1a ;
    `ErrorState` prend un `ApiProblem` tel quel (`ProblemFacts`, aucune dépendance au
    SDK) et remplace `code`/`requestId` ; `DiffList` : tous les en-têtes gardés hors
    virtualisation (Tab et titres atteignent chaque objet), liste qui défile en place
    (`--k-size-list-max`, 480 px) ; `FilterChip` : clés en infobulle et en description,
    la copie laissée au sélecteur (indice « Clé : » de la propriété) ; démos de CLS par
    un bouton, les données arrivant par un script dans le test (une entrée de
    l'utilisateur exclut le décalage du CLS). Restent (à vérifier ou à arbitrer) : copie
    des clés d'une pastille de filtre (la pastille est déjà un bouton : une action de
    plus ou Ctrl+C sur elle) ; un arrêt de tabulation par `KeyLabel`/`EnvChip` hors
    grille (listes et panneaux d'E7, E11 : `plain` par défaut dans les listes ?) ;
    `DiffList` virtualisée en mode lecture d'un lecteur d'écran (le curseur virtuel saute
    les changements non rendus, Ctrl+F ne les trouve pas : « Tout afficher » ?) ; une
    région `status` par pastille (annonceur commun ?) ; nommage différent de
    `CodeBadge` (« Copier » d'abord) et de `KeyLabel` (libellé, action en description) ;
    lignes de squelette 2 à N lues une à une ; titres d'E7 et d'E13 qui écrivent encore
    libellé et clé à la main (un objet et son identifiant), rôle d'une liaison d'E7
    (barré quand retiré) gardé à la main ; nom d'un en-tête de `DiffList` qui finit par
    un nombre nu (antérieur) ; icône de l'état `review` : `eye` dans le code et les
    maquettes validées, `git-pull-request` dans le design (§ « États ») ; lecture au
    lecteur d'écran (description dans une cellule,
    régions répétées).
  - seconde revue (après la fusion de main, 017-1 et 020-22) ; corrigés : focus sur la
    cellule de chargement de `TreeGrid` rendu à la première ligne quand les nœuds arrivent
    (2.4.3) ; anneau d'un widget de cellule (`EnvChip`, `KeyLabel`) entier : la valeur
    d'une cellule de `DataGrid` coupe en `clip` avec une marge d'anneau
    (`overflow-clip-margin`) ; anneau de `KeyLabel` sur sa ligne, marge négative jamais
    positive (ligne de plus de 24 px) ; Échap passe aussi pour un `EnvChip` sans libellé ;
    `plain` montre toujours la clé à côté du libellé ; `ErrorState` : gravité de chaque
    violation (un avertissement le dit), `about:blank` non montré, un `ApiProblem` du SDK
    vérifié comme `ProblemFacts` ; `envHue` sur les octets UTF-8 (FNV-1a usuel), teintes
    attendues fixées ; une copie ancienne qui aboutit après une nouvelle est ignorée ;
    scripts : défilement de `DiffList` au clavier et sans saut au remplacement d'un
    intervalle, refus de copie de `KeyLabel`, repli sans sélection ni focus déplacé, repli
    sans API ; audit tactile : une cible sous une couche (la surface sous le panneau
    droit) n'est plus comptée ; E18, E19, E22 (maquettes de la 017) : codes de règle en
    `CodeBadge`, `ErrorState` par `problem`. Captures réécrites (voulues) : nouvelles
    maquettes de la 017 (pastilles devenues boutons), E9 (fusion du `Toast`), états
    `plain` de `KeyLabel` et `EnvChip` (clé à côté), `ErrorState` `violations` (gravités) ;
    nouvelles : `DataGrid` `widget-env`, `widget-key`. Reste : la pastille de personne
    d'E21 (`k-e21-chip`, valeur retirable) n'a pas de composant de la bibliothèque.
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

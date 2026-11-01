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
- [ ] 2. Graphe d'impact : outil de `web/v2/tools` qui lit les imports des sources de
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
- [ ] 3. `k-web2-check` ciblé : base `origin/main` par défaut, fichiers modifiés et non
  suivis ; captures, instantanés et scripts du graphe de la tâche 2 ; prettier, eslint,
  stylelint sur les fichiers touchés ; `vitest --changed` ; vue-tsc complet ; `--all`
  pour tout ; verdict qui dit la sélection (ADR 0042, décisions 1 et 2).
  - verify : essais sur quatre diffs jetables (un composant, une maquette, un jeton,
    une vue du shell hors maquettes) avec la sélection attendue et la durée ; `--all`
    rend le même verdict qu'avant la tâche.
  - où : `devenv.nix` (`scripts.k-web-check`, `scripts.k-web2-check`).
  - dépend de : 2
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
    même (`goto` juste après `login`), pas l'interception de Playwright.
  - fait le 2026-11-01 (la tolérance de `moves.spec.ts`, absente de main, est retirée par
    la branche de la 021-24 après intégration de ce correctif, par la session principale) :
    marque `kartenn:plugins-fetched` (`PLUGINS_FETCHED`, `loader.ts`) quand chaque plugin
    a chargé son entrée ou est écarté, ou que les capacités sont illisibles ;
    `preloadModules` (`preload.ts`) n'écarte un plugin qu'une fois tous ses
    préchargements réglés ; `atRest` (`e2e/fixtures/journeys.ts`), attendu par `loginIn` :
    la page de l'accueil quittée au repos, sauf horloge installée par le parcours (le
    moment d'inactivité attend alors sur elle). Tests : `test/plugins.test.ts` (marque
    après le dernier plugin, liste vide, capacités illisibles ; écart après tous les
    préchargements). Avant : `shell.spec.ts` Firefox `--repeat-each=10`, 4 échecs sur 150
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

---
status: accepted
date: 2026-11-01
---

# 0042 — Vérifications web ciblées : captures du périmètre touché, parallélisme, Firefox

## Context

`k-web2-check` (`k-web-check` sur `web/v2`) enchaîne prettier, eslint, stylelint, vue-tsc
paquet par paquet, Vitest, puis le projet Playwright `visual` : captures du catalogue et
des maquettes, instantanés de l'arbre d'accessibilité et scripts clavier des composants.
Il ne cible jamais rien. Mesure du 2026-11-01 (main 2d61f14b, charge 6, un créneau
`k-heavy`) :

| Étape | Durée |
|---|---|
| prettier, eslint, stylelint | 15 s |
| vue-tsc (tous les paquets) | 18 s |
| Vitest (613 tests) | 13 s |
| captures `visual` (2 706, `workers: 2`) | 15 min 30 s, dont 9 min pour `catalogue.spec.ts` |

Chaque agent le lance en route et avant son commit, la session principale à nouveau
après fusion : 15 min par passage, pour un changement qui touche le plus souvent un ou
deux composants ou une maquette. Les tests d'un même fichier passent l'un après l'autre
(`fullyParallel` absent), le build du catalogue est refait à chaque passage.

Les parcours (`k-web2-e2e`) échouent par intermittence sous Firefox, sous charge, sur
deux cas sans rapport avec les tâches qui les rencontrent : une erreur d'intégrité (SRI)
dont l'empreinte calculée est celle d'un corps vide, après un rechargement ; et
l'avertissement de fin de session absent dans le parcours `session`, qui avance une
horloge simulée. Les agents relancent alors des suites entières.

## Decision

1. **Captures du périmètre touché.** `k-web2-check` déduit les cibles d'un changement
   (`git diff --name-only <base>...HEAD`, fichiers modifiés et non suivis, base
   `origin/main` par défaut) et ne lance que leurs captures, leurs instantanés et leurs
   scripts clavier. Les cibles se tirent d'un graphe d'imports des sources de `web/v2`
   (`.ts`, `.vue`), résolu symbole par symbole à travers le point d'entrée de
   `@kartenn/ui` : un composant modifié atteint sa page de catalogue, son script
   clavier, et chaque maquette ou partie du shell qui l'importe, de proche en proche.
   Un fichier transverse relance tout : jetons, CSS de base et couches, fixtures et
   configuration de Playwright, `package.json`, lockfile, configuration de Vite, script
   de capture lui-même. En cas de doute, la sélection s'élargit, jamais l'inverse.
   `--all` lance tout ; la ligne de verdict dit la sélection (« ciblé : N captures sur
   M ») pour qu'un passage ciblé ne passe jamais pour complet.
2. **Le reste du contrôle.** prettier, eslint et stylelint portent sur les fichiers
   touchés ; Vitest sur les tests liés au changement (`vitest --changed <base>`, qui
   suit le graphe de modules) ; vue-tsc reste complet (une vérification de types
   partielle ne prouve rien).
3. **Qui lance le passage complet** (choix de l'utilisateur, 2026-11-01). Les agents ne
   lancent que le passage ciblé, en route et avant leur commit. La session principale
   lance `k-web2-check --all` sur le résultat fusionné, avant chaque push.
4. **Parallélisme et build.** `fullyParallel` et un nombre de workers fixé par la
   mesure, dans le créneau de `k-heavy` (`K_HEAVY_JOBS`), gardé seulement si trois
   passages complets de suite ne montrent aucune capture instable. Le build du
   catalogue est réutilisé quand ses sources n'ont pas changé (empreinte des sources).
5. **Firefox : la cause, pas une relance** (choix de l'utilisateur). Chaque cas est
   reproduit sous charge (`--repeat-each`, traces Playwright gardées en échec), sa cause
   écrite, puis corrigée. Aucune relance automatique (`retries` reste à 0). Le cas SRI
   est traité comme un possible défaut du shell au rechargement, pas seulement du test.

## Alternatives

- **Moins de captures** (moins de largeurs, de modes ou de densités) : touche
  REQ-020-25 à 28, hors de cette décision.
- **Relancer une fois les parcours Firefox** : écartée, un vrai défaut intermittent
  passerait inaperçu.
- **Cibler par le nom des fichiers seulement** (un composant → ses captures) : ne voit
  pas les maquettes et les parties du shell qui l'utilisent ; le graphe d'imports les
  voit.

## Consequences

- Un passage d'agent sur un composant passe de ≈ 16 min à quelques minutes ; la
  session principale garde le passage complet, plus court par le parallélisme.
- Une cible manquée dans le graphe d'imports ne se voit qu'au passage complet de la
  session principale, avant le push : la fusion attend, main ne reçoit rien de non
  vérifié.
- Le graphe d'imports est un outil de plus à tenir juste ; ses tests vérifient que
  chaque composant du catalogue et chaque maquette ont leurs cibles.

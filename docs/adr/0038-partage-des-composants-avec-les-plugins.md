---
status: accepted
date: 2026-10-23
---

# 0038 — Partage du framework et de `@kartenn/ui` avec les plugins ; DOM léger en première partie

*Amende l'ADR 0025 (SDK seul module partagé) et 007/W7, W12 ; touche 017/U2.*

## Context

L'ADR 0025 fait du SDK la seule instance partagée par l'import map ; chaque plugin
« embarque son propre framework s'il en emploie un » (007/W7) et rend en Shadow DOM, ne
recevant du shell que les jetons CSS (W12). Résultat constaté : les plugins de première
partie (VLAN, Machines) écrivent leur DOM à la main (environ 640 lignes pour le seul
VLAN) et leur propre feuille ; les composants du shell leur sont inaccessibles ; le
langage visuel diverge. La 017 prévoyait deux plugins de plus sur le même modèle (U2).

Arbitrages de l'utilisateur du 2026-10-23 : D-2 (le framework et `@kartenn/ui` passent
par l'import map), D-3 (DOM léger pour les plugins de première partie), et **D-1 :
Vue 3**, retenu après un prototype comparatif (Vue 3 et Reka UI contre React 19 et React
Aria, même arbre et même table éditables) ; D17 et l'ADR 0011 restent telles quelles. Contraintes :
CSP `style-src 'self'` sans `unsafe-inline` (pas de `<style>`, pas d'attribut `style`
analysé, pas de `cssText`), SRI de chaque module (ADR 0025), Custom Element comme
frontière d'extension (D17, W5).

## Decision

1. **Import map étendue.** L'import map du shell désigne, en plus de `@kartenn/sdk` :
   `vue` (D-1), `@kartenn/ui` (module) et `@kartenn/ui/style.css`
   (feuille, servie avec empreinte). Ce sont des **instances uniques** ; le build des
   plugins de première partie les marque `external`. Leur empreinte SRI est publiée
   comme celle du SDK, l'import map reste le seul script en ligne permis par la CSP.
2. **Plages vérifiées.** Le manifeste d'un plugin déclare, en plus de `sdk`, les plages
   `ui` et `framework` qu'il attend (semver, mêmes règles que le SDK) ; un plugin dont
   une plage n'est pas satisfaite est écarté `incompatible`, comme aujourd'hui.
3. **Le Custom Element reste la frontière.** Chaque point d'extension (W5) est un
   élément personnalisé ; l'élément d'un plugin de première partie est un **hôte mince**
   qui monte une application du framework partagé, avec le `QueryClient` et le contexte
   du SDK (ADR 0037), et la démonte à sa déconnexion.
4. **DOM léger en première partie.** Les plugins de première partie rendent **sans
   racine d'ombre** : la feuille `@kartenn/ui` du document s'applique directement, les
   portails (dialogues, menus, infobulles) se comportent comme dans le shell. Leur propre
   CSS, s'il en faut, entre dans `@layer plugins`, sous un préfixe de classes
   `k-<plugin>-`, et suit les mêmes règles Stylelint.
5. **Les tiers gardent le choix.** Un plugin tiers peut : utiliser le framework et
   `@kartenn/ui` partagés (même contrat que la première partie) ; ou rester sans
   framework, en racine d'ombre, avec les jetons (qui traversent la frontière) et la
   feuille `@kartenn/ui/style.css` liée par `handle.styles()` (`<link>` avec empreinte).
   Le plugin d'exemple sans framework est gardé pour le prouver (REQ-007-21).
6. **Aucun style injecté.** Ni composant qui injecte un `<style>` (éléments
   personnalisés compilés par le framework avec styles embarqués), ni liaison de style
   en chaîne : la CSS de la bibliothèque est un fichier servi ; les styles dynamiques
   passent par le CSSOM (`setProperty`) ou des variables CSS.

## Résolution de D-1 : Vue 3 (2026-10-23)

- L'import map désigne **`vue`** : le build navigateur *runtime* de production, **sans le
  compilateur de gabarits** (qui évalue du code et serait refusé par une CSP sans
  `unsafe-eval`) ; les plugins livrent des gabarits précompilés. Plage du manifeste :
  `framework: "^3.5.0"`. vue-router et vue-i18n restent internes au shell : les plugins
  naviguent et traduisent par le SDK.
- Hôte d'un plugin de première partie : l'élément personnalisé crée une application par
  élément (`createApp`), lui fournit (`provide`) le contexte du SDK et le `QueryClient`
  (`VueQueryPlugin`), la monte dans son DOM léger à `connectedCallback` et la démonte à
  `disconnectedCallback`.
- CSP : vérifiée au prototype pour les primitives retenues (zéro violation en usage, après
  retrait de `ComboboxViewport`) ; la liste des composants interdits est dans l'ADR 0036.

## Alternatives

- **Bibliothèque de Custom Elements agnostique** : un seul code pour tous les
  frameworks, mais une seconde technologie de composants, et des composants complexes
  (arbre, grille éditable, combobox) bien plus durs à rendre accessibles.
- **Jetons et feuille seuls** (situation actuelle) : cause de l'incohérence constatée.
- **Racine d'ombre pour la première partie, feuille liée par empreinte** : isolation,
  mais portails et superpositions à gérer hors de la racine, double application des
  thèmes. Reste l'option des tiers.
- **Libre choix par plugin de première partie** : pas de règle, donc dérive.
- **Federation de modules liée au build** : écartée par D17.

## Consequences

- L'ADR 0025 reste vraie pour le serveur (répertoire, SRI, capacités, CSP, rechargement
  à chaud) ; seule sa règle « le SDK, seule instance partagée » est élargie.
- Une rupture de `@kartenn/ui` ou une version majeure du framework est une **rupture du
  contrat des plugins** : version majeure du SDK et des plages, plugins de première
  partie reconstruits avec le shell.
- Les plugins VLAN et Machines sont réécrits sur `@kartenn/ui` (phase 5 de la 021) ; les
  plugins Politique et Flux de la 017 s'écrivent directement ainsi (017/U2 amendée).
- Le poids partagé (framework et `@kartenn/ui`) est mis en cache une fois et compté à
  part dans les budgets de taille (spec 021).

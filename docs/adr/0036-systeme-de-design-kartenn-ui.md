---
status: proposed
date: 2026-10-23
---

# 0036 — Système de design : jetons, CSS en couches et bibliothèque `@kartenn/ui`

## Context

Le shell de la 007 a des choix sains (primitives accessibles, jetons CSS, virtualisation,
axe-core bloquant), mais pas de colonne vertébrale visuelle : neuf couleurs, quatre
espacements, un rayon, une taille de police, la police du système (rendu différent selon
l'OS, captures non reproductibles), une feuille globale de 504 lignes, et des plugins
qui construisent chacun leur DOM et leur feuille. Tables, formulaires, états vides et
erreurs existent trois fois, différemment. Aucune maquette, aucune capture de référence.

L'utilisateur a arbitré le 2026-10-23 la réécriture du web (débat D-1 à D-12) : jetons et
CSS par composant en couches, Stylelint strict (D-4) ; maquettes construites avec les
vrais composants (D-8) ; régression visuelle bloquante sur Chromium, références générées
dans devenv (D-9) ; police libre auto-hébergée avec chasse fixe et chiffres tabulaires
(D-11) ; icônes Lucide au trait de 1,5 px ; direction visuelle « Ardoise ». Specs 020
(système de design) et 021 (shell v2). Décisions D6, D17, ADR 0011, 0024, 0025 ; 007/W12.

## Decision

1. **Jetons à trois niveaux**, propriétés personnalisées `--k-*` :
   - *primitifs* : teintes, échelle typographique, espacements sur une base de 4 px,
     rayons, ombres, durées ;
   - *sémantiques* : surfaces, texte, bordures, accent, focus, états (`ok`, `warn`,
     `ko`, `review`), environnements, différentiel, bande hors présent ;
   - *de composant*, seulement si nécessaire (hauteur de ligne par densité).
   Source unique en CSS dans `@kartenn/ui` ; modes clair et sombre (thème du système par
   défaut, choix forcé par `data-theme`) ; densités confortable et compacte, compacte par
   défaut (lignes de 28 px). Le test de contraste existant couvre toutes les paires
   sémantiques texte/fond et contrôle/fond dans les deux modes.
2. **CSS par composant, en couches** : `@layer reset, tokens, base, components, plugins,
   utilities`. Chaque composant porte sa feuille, compilée en fichier ; les écrans
   n'écrivent pas de CSS hors des primitives de mise en page. Pas de framework
   d'utilitaires.
3. **Garde-fous mécaniques** : Stylelint refuse toute couleur, taille, rayon, ombre,
   durée ou police brute hors du fichier de jetons, les `!important`, les sélecteurs
   d'identifiant et les couches inconnues ; une règle de lint refuse les éléments
   interactifs bruts (bouton, champ, table, liste de choix) dans les écrans et les
   plugins de première partie ; aucune chaîne `style="…"`, aucun `<style>` injecté
   (CSP `style-src 'self'`, ADR 0025).
4. **Police auto-hébergée** : Geist et Geist Mono (licence SIL OFL 1.1), servies en
   `woff2` par kartenn (`font-src 'self'`), chiffres tabulaires pour les nombres et les
   colonnes, chasse fixe pour adresses, préfixes, codes et identifiants.
5. **Icônes** : Lucide (licence ISC), trait de 1,5 px, tailles 16 et 20 px, importées
   une à une (pas de police d'icônes) ; les icônes métier absentes sont dessinées dans
   la même grille et le même trait, dans `@kartenn/ui`.
6. **Bibliothèque `@kartenn/ui`** : environ 25 composants (inventaire de la spec 020)
   bâtis sur des primitives accessibles sans style ; un seul jeu de tailles et
   d'alignements ; chaque composant interactif suit son motif APG et a son test clavier.
7. **Catalogue vivant** : route de développement `/_ui` du shell, chaque composant dans
   tous ses états (repos, survol, focus, désactivé, chargement, vide, erreur), dans les
   deux densités et les deux modes ; les **maquettes** des écrans clés (`/_ui/ecrans/…`)
   sont construites avec ces composants sur des données factices, validées sur captures
   par l'utilisateur.
8. **Régression visuelle bloquante** : captures de référence du catalogue et des
   maquettes, Chromium seul, générées dans devenv (navigateur et polices fournis par
   Nix), animations coupées, horloge figée ; écart au-delà du seuil = échec de
   `k-web-check`.
9. **Licence** : `@kartenn/ui` en MPL-2.0, comme le SDK (Q-ouv-1), puisqu'il est importé
   par des plugins.

## Dépend de D-1 (framework, prototype en cours)

D-1 compare deux piles sur l'arbre éditable à 10⁵ nœuds et la table éditable. Restent
liés à son issue, et seulement eux :
- la bibliothèque de primitives accessibles sous `@kartenn/ui` (avec Vue 3 : Reka UI,
  déjà retenue par 007/W12 ; avec React 19 : React Aria) ;
- la forme des fichiers de composant (composants monofichiers dont le bloc de style est
  extrait en `.css` au build, ou module TypeScript avec feuille sœur) et la règle de lint
  des éléments bruts (`vue/no-restricted-html-elements` ou son équivalent JSX) ;
- l'outil de virtualisation intégré ou TanStack Virtual (disponible dans les deux).
Tout le reste (jetons, couches, Stylelint, police, icônes, catalogue, captures, budgets)
est indépendant du framework. D17 et l'ADR 0011 ne sont pas amendées par cette ADR.

## Alternatives

- **Utilitaires générés depuis les jetons, sans valeurs arbitraires** : écriture rapide
  par les agents, mais la décision de style se déplace dans chaque gabarit (dérive) et la
  feuille d'utilitaires doit entrer dans chaque racine d'ombre.
- **CSS Modules seuls** : portée locale, mais pas d'ordre de cascade garanti entre shell
  et plugins, et rien n'empêche une valeur brute.
- **Bibliothèque complète thémée** : esthétique et densité imposées, deux vocabulaires
  (la bibliothèque et ses surcharges), thème qui ne traverse pas les plugins.
- **Polices du système** : pas de dépendance, mais rendu et métriques variables selon
  l'OS, donc captures non reproductibles.
- **Maquettes en HTML statique jetable** (017/U14 initial) : rapides, mais à refaire en
  composants et sans comparaison automatique.
- **Captures non bloquantes ou service externe** : dérive tolérée, ou données de
  l'interface sorties de la machine.

## Consequences

- Nouveau paquet `web/ui` (`@kartenn/ui`) dans l'espace pnpm ; nouvelles dépendances de
  développement : Stylelint et ses règles, les paquets de police et d'icônes (ADR 0024
  étendue).
- `web/shell/src/styles/base.css` et les jetons actuels sont remplacés ; les noms de la
  liste `tokens` du SDK suivent les nouveaux noms (version majeure du SDK, spec 021).
- `k-web-check` ajoute Stylelint et les captures ; `k-web-e2e` garde les parcours sur
  Chromium et Firefox ; les références de captures se régénèrent par une commande devenv
  dédiée, jamais hors de devenv.
- Toute demande de composant nouveau passe par la bibliothèque : pas de composant local à
  un écran.
- Les jetons traversent toujours le Shadow DOM : un plugin tiers en racine d'ombre garde
  le thème sans code propre (REQ-007-30).

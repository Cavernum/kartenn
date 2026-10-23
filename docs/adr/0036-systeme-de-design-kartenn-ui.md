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
(D-11) ; icônes Lucide au trait de 1,5 px ; direction visuelle « Ardoise » ; et, après un
prototype comparatif, **D-1 : Vue 3**, D17 et l'ADR 0011 inchangées. Specs 020
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
   en Vue 3, bâtis sur **Reka UI** (primitives accessibles sans style) là où il convient,
   et écrits par kartenn là où il manque ou faillit (grille, navigation de l'arbre
   virtualisé) ; un seul jeu de tailles et
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

## Résolution de D-1 : Vue 3 et Reka UI (2026-10-23)

Le prototype (même arbre éditable de 10⁵ nœuds et même table éditable de 10⁴ lignes dans
les deux piles) a conduit l'utilisateur à retenir Vue 3. Règles qui en découlent :
- **Composants** : SFC Vue dont le bloc `<style>` est extrait en `.css` au build et placé
  dans `@layer components` ; jamais de `.ce.vue` ni de `defineCustomElement` avec styles ;
  `:style` en objet seulement (CSSOM). Lint : `eslint-plugin-vue`
  (`vue/no-restricted-html-elements`, `vue/no-static-inline-styles`, liaison de style en
  chaîne refusée).
- **Reka UI, version exacte épinglée** (sans `^`) : plusieurs comportements dont kartenn
  dépend ne sont pas documentés et ont été trouvés en lisant le code compilé. Toute
  montée de version passe par les scripts clavier de chaque composant qui s'en sert.
- **Composants interdits** (injection d'un `<style>`, refusé par la CSP) :
  `ComboboxViewport`, `Splitter`, et tout composant que le parcours CSP prendra en défaut ;
  refusés par `no-restricted-imports`. L'option `nonce` n'est pas une issue : la CSP de
  kartenn n'admet aucun `<style>` injecté.
- **Arbre** : `TreeRoot`, `TreeItem`, `TreeVirtualizer` de Reka, avec quatre correctifs à
  demeure dans `@kartenn/ui` : sélection résolue **par identifiant** (la sélection simple
  de Reka stocke une copie de l'élément, ce qui a fait supprimer le mauvais nœud au
  prototype) et nœuds non réactifs (`markRaw`, `shallowRef`) ; Haut et Bas gérés par
  kartenn sur l'index aplati (Reka déplace le focus vers le voisin rendu et perd des
  appuis en répétition rapide) ; `*`, Entrée (renommer plutôt que sélectionner) et filtre
  des touches non imprimables du tampon de recherche **interceptés avant** les
  gestionnaires de Reka (fragile : dépend de l'ordre des écouteurs, d'où l'épinglage et
  les tests) ; la ligne focalisée reste montée quand elle sort de la fenêtre virtualisée.
- **Grille** : Reka n'en a pas ; le motif APG Grid (index de lignes et colonnes, tabindex
  itinérant, navigation, sélection, édition, en-têtes et filtres) est **écrit dans
  `@kartenn/ui`** sur TanStack Virtual, avec ses scripts clavier complets (environ 500
  lignes au prototype, la part la plus risquée en accessibilité).
- **Combobox** : Reka masque le reste de la page (`aria-hidden`) tant que la liste est
  ouverte ; les filtres en popover ouvrent la liste au focus, et deux Échap ferment liste
  puis popover (documenté dans l'aide).
- **Mesures du prototype** (build de production, Chromium, sans ralenti du CPU) : JS
  104 Ko gzip (dont 91,5 Ko au premier chargement, framework compris) ; première ligne de
  l'arbre à 155 ms (génération de 10⁵ nœuds comprise) ; dépliage de 1 000 enfants 43 ms ;
  flèche bas p75 14,8 ms ; défilement de 5 000 lignes à 0,2 % de trames perdues, aucune
  tâche longue ; tas 19 à 25 Mo ; table : filtre 31 ms, tri 14 ms, édition 15 ms ; axe
  sans violation, 24 scripts clavier verts. Ils fondent les budgets de la 020 et de la
  021.

## Alternatives

- **React 19 et React Aria** (prototype jumeau, rejeté par D-1) : couverture accessible
  plus large (table, formulaires, couches), mais 220 Ko gzip au départ (182 Ko avec le
  greffon de locales), tas de 116 Mo sur la table de 10⁴ lignes (la collection matérialise
  toutes les cellules) et tri ou filtre de 130 à 260 ms ; l'arbre demandait lui aussi des
  contournements liés à la version ; changer D17 et l'ADR 0011.

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

- Nouveau paquet `web/v2/ui` (`@kartenn/ui`), dans l'espace distinct du nouveau shell
  jusqu'à la bascule (021) ; nouvelles dépendances de
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

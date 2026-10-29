# 020 — Système de design : journal des décisions

Rédigé le 2026-10-23 à partir des arbitrages rendus le même jour par l'utilisateur (débat
web D-1 à D-12, trois tours de brief, icônes, police, direction visuelle) et de ses
retours sur les maquettes du 2026-10-22. Les décisions B1 à B30 traduisent ces arbitrages
en choix de conception ; les choix peu coûteux à défaire sont marqués « choix de
Claude ». Restent : les **arbitrages demandés** (contradictions avec des décisions ou des
specs validées) et les **questions** Q-020-1 à 10, toutes closes le 2026-10-23 : la spec est validée.

Cadre : D6, D17, D31 (amendée), D37, Q-ouv-1, Q-ouv-6 de
[`../001-cadrage/decisions.md`](../001-cadrage/decisions.md) ; 007/W11 à W15 ;
017/U12, U14 ; ADR 0011, 0024, 0025, 0036, 0038, 0039 ; spec 022 (validée, précède la
020) ; spec 018 (retours d'interface, non fusionnée : permissions `module.action`,
libellés par langue).

## Arbitrages rendus (2026-10-23, utilisateur)

- **D-1 framework : Vue 3** (« Franchement on va prendre vue »), après le prototype
  comparatif (Vue 3 + Reka UI, React 19 + React Aria ; arbre éditable de 10⁵ nœuds, table
  éditable) ; D17 et l'ADR 0011 inchangées ; Reka UI comme base de primitives là où elle
  convient (B33).
- **D-4 CSS** : jetons et CSS par composant en `@layer`, Stylelint strict.
- **D-8 maquettes** : construites avec les vrais composants, dans le catalogue `/_ui`.
- **D-9 régression visuelle** : bloquante, Chromium seul, références générées dans devenv.
- **D-10 budgets** : bloquants (voir la 021 ; la 020 porte ceux de la bibliothèque).
- **D-11 police** : famille libre OFL auto-hébergée avec chasse fixe et chiffres
  tabulaires ; retenue au brief : Geist et Geist Mono.
- **D-12 outillage** : `ui-reviewer`, `a11y-auditor`, skill `kartenn-ui`, hook après
  édition, `bench-runner` étendu (ADR 0039).
- **Brief** : trois tours (design, § « Brief »).
- **Icônes** : Lucide (ISC), trait de 1,5 px.
- **Direction** : « Ardoise ».

## Synthèse
- B1 brief validé, couleur réservée au sens et doublée d'un libellé.
- B2 clichés à éviter listés au brief et au skill `kartenn-ui`.
- B3 densité compacte par défaut (28 px), confortable à 36 px ; cibles ≥ 24 px.
- B4 thème du système, choix forcé par `data-theme`.
- B5 jetons à trois niveaux, un seul fichier CSS, pas de génération.
- B6 couleurs de départ « Ardoise », deux écarts pour le contraste.
- B7 six états sémantiques (`ok`, `warn`, `ko`, `review`, `info`, `neutral`).
- B8 palette fermée de six teintes d'environnement, distinctes des états.
- B9 espacement base 4 ; rayons 3, 5, 8 et plein ; ombres des seules couches flottantes.
- B10 Geist et Geist Mono variables, `woff2` servis par kartenn, chiffres tabulaires.
- B11 trois durées en jetons, zéro sous mouvement réduit.
- B12 `@layer reset, tokens, base, components, plugins, utilities`.
- B13 Stylelint et lint du code stricts.
- B14 aucun style en chaîne, aucun `<style>` injecté ; parcours CSP.
- B15 Lucide importé à l'unité ; icônes métier sur la même grille.
- B16 environ 29 composants, chacun dans tous ses états.
- B17 trois hauteurs de contrôle partagées.
- B18 un motif APG et un script clavier par composant interactif.
- B19 édition en place et déplacement : contrat générique, l'hôte décide des droits.
- B20 filtres typés par pastilles et listes fermées ; langage écrit plus tard.
- B21 clé et libellé ; repli sur la clé.
- B22 codes et identifiant de requête visibles et copiables.
- B23 squelettes de taille fixe, états vides et d'erreur utiles.
- B24 catalogue `/_ui` hors du build de production.
- B25 dix maquettes validées sur captures avant tout écran.
- B26 captures Chromium, références dans devenv, seuil 0,1 %.
- B27 instantanés ARIA.
- B28 WCAG 2.2 AA, axe `wcag22aa`, revue `a11y-auditor`.
- B29 budgets de poids et de rendu de la bibliothèque.
- B30 `@kartenn/ui` en MPL-2.0.
- B31 déplacement contrôlé : aucun en parent déduit ; cibles vérifiées avant mise en évidence ; intention seulement.
- B32 invariant : un nœud n'est affiché que sous le parent de son modèle.
- B33 Reka UI épinglé et encadré ; grille maison ; budgets de composants tirés du prototype.

## B1 — Brief et principes
- Décision : le brief validé (design, § « Brief ») est la référence des revues
  `ui-reviewer` ; la couleur ne porte jamais seule une information.
- Pourquoi : retours du 2026-10-22 (cohérence) ; WCAG 1.4.1 ; 017/U12.
- Décidé par : l'utilisateur (brief, trois tours)

## B3 — Densité
- Décision : compacte par défaut (28 px), confortable (36 px) au choix, gardée localement.
- Pourquoi : brief tour 1 ; 2.5.8 exige 24 px, tenu par les hauteurs de contrôle.
- Rejeté : densité unique ; densité par écran.
- Décidé par : l'utilisateur (densité) ; 36 px : choix de Claude (Q-020-4)

## B5 — Jetons en CSS, sans génération
- Décision : le fichier `tokens.css` est la source ; pas de JSON au format DTCG converti
  au build.
- Pourquoi : une seule source lisible par les agents et par Stylelint ; pas d'outil de
  plus ; un export DTCG reste possible plus tard (lecture du CSS).
- Rejeté : DTCG puis génération (deux fichiers à garder synchrones, outil de build).
- Décidé par : choix de Claude (Q-020-3)

## B6 — Couleurs « Ardoise », ajustées au contraste
- Décision : valeurs de la direction ; `text-muted` clair `#5d6a7e` (au lieu de `#637084`,
  4,24:1 sur `accent-soft`) ; `border-control` ajouté (`#7d8a9e` / `#5f6f86`, ≥ 3:1).
- Pourquoi : WCAG 1.4.3 et 1.4.11 ; écarts imperceptibles à l'œil, mesurés.
- Décidé par : l'utilisateur (direction) ; écarts : choix de Claude, à confirmer (A6)

## B8 — Couleurs d'environnement
- Décision : six teintes distinctes des états ; attribution par empreinte de la clé ;
  plus tard, choix de l'administrateur par un attribut de la valeur.
- Pourquoi : la direction colorait trois environnements avec l'accent, l'ambre et le
  violet des états : un environnement « recette » se lisait comme un avertissement, un
  « développement » comme « en revue ».
- Rejeté : reprendre les couleurs d'état ; couleur libre par l'administrateur (contraste
  non garanti).
- Décidé par : l'utilisateur (2026-10-23, Q-020-1)

## B12 — Couches
- Décision : `@layer reset, tokens, base, components, plugins, utilities`, déclaré en tête
  de `style.css`.
- Pourquoi : l'ordre de cascade ne dépend plus de l'ordre de chargement des plugins ; un
  plugin ne peut pas battre un composant par la spécificité, seulement dans sa couche.
- Décidé par : l'utilisateur (D-4) ; noms des couches : choix de Claude

## B13 — Lint strict
- Décision : Stylelint (valeurs, `!important`, identifiants, profondeur, propriétés
  logiques, couches) et lint du code (éléments interactifs bruts, style en chaîne).
- Pourquoi : garde-fou mécanique plutôt que consigne (ADR 0039) ; source de la dérive
  constatée (504 lignes de CSS global).
- Décidé par : l'utilisateur (D-4, D-12)

## B16 — Inventaire
- Décision : 29 entrées (design, § « Inventaire »), dont les primitives de mise en page,
  la grille et l'arbre éditables, les filtres typés, le sélecteur de période, la palette
  de commandes, les pastilles clé/libellé, les badges de code et les éléments de la barre
  d'état.
- Pourquoi : l'étude comptait environ 25 composants ; les retours du 2026-10-22 ajoutent
  filtres typés, clé/libellé, codes ; la barre d'état et la palette viennent du brief.
- Décidé par : l'utilisateur (périmètre) ; découpage : choix de Claude

## B19 — Édition en place et déplacement
- Décision : la grille et l'arbre offrent un contrat générique (cellule éditable,
  `commit`, états `pending`/`draft`/`review`/`error` ; déplacement selon B31) ; la règle métier (droits,
  brouillon ou modification rapide, présent) est dans l'hôte (spec 021).
- Pourquoi : un composant ne connaît ni les droits ni le changeset ; la même grille sert
  le shell et les plugins.
- Rejeté : grille qui appelle l'API elle-même.
- Décidé par : l'utilisateur (édition en place) ; contrat : choix de Claude

## B31 — Déplacement contrôlé dans un arbre
- Décision : l'arbre distingue parent déduit (`derived`, aucun déplacement) et parent
  déclaré (`declared`) ; en `declared`, chaque cible est vérifiée par `canMove` avant
  d'être mise en évidence, une cible refusée montre motif et code et n'accepte pas le
  dépôt, « Déplacer vers… » ne liste que les cibles valides ; un dépôt n'émet qu'une
  intention, le nœud reste à sa place en attente.
- Pourquoi : retour de l'utilisateur du 2026-10-23 : un glisser dans un arbre déduit de
  l'inclusion peut afficher des conteneurs qui contiennent des plages plus grandes que ce
  que le préfixe permet ; la place d'un préfixe découle de sa valeur, jamais d'un geste.
- Rejeté : glisser partout avec refus à la réponse du serveur ; déplacement affiché tout
  de suite puis annulé.
- Décidé par : l'utilisateur (principe, 2026-10-23) ; contrat : choix de Claude

## B32 — Invariant d'affichage
- Décision : l'arbre ne rend que la relation parent-enfant de son modèle ; garde vérifiée
  à chaque rendu en développement et en test.
- Pourquoi : rend l'affichage incorrect décrit par l'utilisateur impossible par
  construction, et testable par propriété.
- Décidé par : l'utilisateur (2026-10-23)

## B33 — Vue 3, Reka UI encadré, budgets du prototype
- Décision : composants Vue 3 ; Reka UI en version exacte ; composants injectant un
  `<style>` interdits ; sélection par identifiant, Haut/Bas maison, interception de `*`,
  Entrée et du tampon de recherche dans l'arbre ; grille APG maison ; budgets de
  composants = 2 × les mesures du prototype.
- Pourquoi : D-1 ; constats du prototype (design § « Vue et Reka UI »).
- Rejeté : React 19 et React Aria (plus lourd : 220 Ko gzip, 116 Mo de tas sur la table,
  tri et filtre de 130 à 260 ms) ; Reka en plage `^` (comportements non documentés dont
  dépendent les contournements).
- Décidé par : l'utilisateur (D-1) ; règles et marges : choix de Claude

## B20 — Filtres typés
- Décision : pastilles `propriété opérateur valeur`, listes fermées avec autocomplétion ;
  pas de langage écrit dans cette spec.
- Pourquoi : retours du 2026-10-22 ; un langage écrit sans autocomplétion ni coloration
  serait moins bon que des pastilles.
- Décidé par : l'utilisateur

## B21, B22 — Clé et libellé ; codes visibles
- Décision : la clé interne est toujours accessible (infobulle, copie) ; repli sur la clé
  sans libellé ; le code de chaque problème et le `Request-Id` sont visibles et copiables.
- Pourquoi : retours du 2026-10-22 ; un exploitant cite un code, jamais un message traduit.
- Décidé par : l'utilisateur

## B24 — Catalogue hors production
- Décision : `/_ui` présent en développement et dans le build de test des captures,
  absent du build livré.
- Pourquoi : surface inutile en production ; données factices.
- Décidé par : choix de Claude (Q-020-2)

## B26 — Captures
- Décision : Chromium seul, viewport 1280 × 800, échelle 1, mouvement réduit, horloge,
  locale et fuseau fixes, `maxDiffPixelRatio` 0,001 ; commande `k-web-shots` dans devenv.
- Pourquoi : D-9 ; le rendu varie selon OS, matériel et polices (playwright.dev,
  « Visual comparisons »).
- Décidé par : l'utilisateur (D-9) ; seuil : choix de Claude (Q-020-5)

## B29 — Budgets de la bibliothèque
- Décision : JS ≤ 90 Ko gzip, CSS ≤ 20 Ko gzip, polices préchargées ≤ 120 Ko ; grille de
  100 lignes ≤ 100 ms ; arbre de 10⁵ ≤ 5 % de trames perdues.
- Décidé par : l'utilisateur (D-10, principe) ; chiffres de poids : choix de Claude
  (Q-020-9)

## B30 — Licence
- Décision : MPL-2.0, comme le SDK.
- Pourquoi : importée par des plugins, éventuellement tiers et propriétaires (Q-ouv-1).
- Décidé par : choix de Claude, conforme à Q-ouv-1

## Arbitrages (tranchés le 2026-10-23 par l'utilisateur)

- **A1 — 017/U14 et tâche 017-1** : la 017 s'aligne sur la 020 et la 021 ; U14 remplacée
  (maquettes au catalogue avec les vrais composants).
- **A2 — 007/W12 et REQ-007-30** : W12 remplacée par la 020 (Reka UI encadré,
  REQ-020-33) ; la traversée du Shadow DOM par les jetons reste vraie pour les tiers.
- **A3 — ADR 0024 (dépendances)** : Stylelint et ses règles, `lucide`, les polices Geist,
  `reka-ui` en version exacte et `@tanstack/vue-virtual` acceptés (ADR 0036).
- **A4 — D17** : D-1 retient Vue 3, D17 inchangée.
- **A5 — Captures Chromium seul et REQ-007-37** : pas d'amendement ; les parcours restent
  sur Chromium et Firefox.
- **A6 — Écarts à la direction** : `text-muted` clair `#5d6a7e`, `border-control`,
  palette d'environnement distincte des états : acceptés.

## Questions (closes le 2026-10-23)

- **Q-020-1** — Couleur d'un environnement : déduite de la clé (teinte stable de la
  palette accessible de six) ; choix de l'administrateur plus tard, par un attribut de la
  valeur.
- **Q-020-2** — Catalogue : hors du build de production ; publié aussi comme artefact de
  l'intégration continue (build de test du catalogue).
- **Q-020-3** — Jetons en CSS (`tokens.css`) ; DTCG possible plus tard par une conversion
  unique.
- **Q-020-4** — Densité confortable : 36 px.
- **Q-020-5** — Seuil des captures : 0,1 % des pixels (`maxDiffPixelRatio: 0.001`).
- **Q-020-6** — Polices : Geist et Geist Mono variables, sous-ensembles latin et latin
  étendu.
- **Q-020-7** — Icônes métier : dessinées par Claude dans le style Lucide, validées sur
  une planche.
- **Q-020-8** — Différentiel : couleurs d'état, signe et libellé.
- **Q-020-9** — Budgets de poids : fixés par le prototype D-1 (B33).
- **Q-020-10** — Langage de requête écrit : après la v0.2, dans une spec propre, partagée
  avec la CLI (023).

## Validations des maquettes

Validation par l'utilisateur le 2026-10-29, après les relectures des 2026-10-22, 24, 25,
26 et 28 (retours consignés par la 024, la 026 et la 027) ; captures de référence : celles
de `web/v2/e2e/visual/ecrans.spec.ts` (clair et sombre, 1440, 1280, 1024, 768 et 390 px) à
main 7ef0ded1.

| Maquette | Date | Écarts acceptés |
|---|---|---|
| E0 Accueil | 2026-10-29 | — |
| E1 Topographie | 2026-10-29 | — |
| E2 Fiche d'un préfixe | 2026-10-29 | — |
| E3 Liste d'adresses | 2026-10-29 | — |
| E4 Changeset et revue | 2026-10-29 | — |
| E5 Brouillon | 2026-10-29 | — |
| E6 Lecture au passé | 2026-10-29 | — |
| E7 Administration | 2026-10-29 | — |
| E8 Palette et aide | 2026-10-29 | — |
| E9 États | 2026-10-29 | — |
| E10 Hôte de plugin | 2026-10-29 | — |
| E11 Flux | 2026-10-29 | extrémité hors périmètre : cadenas et bulle `Tooltip` (celle des refus « Il faut `<permission>` »), pas la bulle d'erreur de cellule du `DataGrid` |
| E12 VLAN | 2026-10-29 | — |
| E13 Paramètres | 2026-10-29 | — |

Les retours de chaque relecture ont été reportés dans ce design et dans celui de la 021 par
les tâches de la 024, de la 026 et de la 027, au fil des relectures.

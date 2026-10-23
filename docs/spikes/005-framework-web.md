# 005 — Framework du shell web (D-1) : Vue 3 + Reka UI contre React 19 + React Aria

Prototype jetable du 2026-10-23 (tâche 021-1), construit hors du dépôt en deux jumeaux sur
les mêmes données et les mêmes jetons. Verdict de l'utilisateur : **Vue 3** ; D17 et
l'ADR 0011 inchangées. Suites : ADR 0036 à 0039 (sections « Résolution de D-1 »), spec 020
(REQ-020-29, 33 ; design § « Vue et Reka UI »), spec 021 (REQ-021-30).

## Ce qui a été construit (dans chaque pile)

- Données déterministes : 100 000 préfixes (agrégats /8, blocs /16, réseaux /24), un bloc
  à 1 000 enfants, environnements par clé avec libellés FR/EN, ~4 % d'anomalies codées.
- **Arbre éditable** virtualisé : flèches, Début/Fin, `*`, recherche à la saisie, Entrée/F2
  pour renommer, déplacement au glisser, par Ctrl+X/Ctrl+V et par « Déplacer vers… ».
- **Table éditable** de 10 000 lignes virtualisées : tri, filtres typés en popover (liste
  fermée avec autocomplétion, « contient », intervalle), édition en cellule avec erreur
  codée, sélection multiple, « Brouillon (n) ».
- Thèmes clair et sombre, densités, CSS en `@layer` sans valeur brute, CSP stricte
  (`style-src 'self'`) vérifiée par un parcours.

## Mesures (build de production, Chromium, sans ralenti, 5 passes, médiane)

| Mesure | Vue 3 + Reka UI | React 19 + React Aria |
|---|---|---|
| JS gzip | 104,3 Ko (91,5 au premier chargement) | 219,5 Ko (182 avec le greffon de locales) |
| Arbre : première ligne | 155 ms | — |
| Arbre : dépliage de 1 000 enfants | 43 ms | — |
| Arbre : Flèche bas, p75 | 14,8 ms | — |
| Arbre : défilement de 5 000 lignes | 0,2 % de trames perdues, 0 tâche longue | — |
| Tas : arbre chargé / après défilement / table | 19,4 / 24,6 / 21,6 Mo | 27,4 / 67,7 / 116 Mo |
| Table : filtre / tri / édition | 31 / 14 / 15 ms | tri et filtre 130 à 260 ms |
| axe (wcag2a à wcag22aa) | 0 violation | 0 hors un artefact `target-size` |
| Scripts clavier | 24/24 | 31/32 |

Machine partagée (charge 1,4 à 4,8). Les cases « — » du jumeau React n'ont pas été
relevées au même protocole.

## Constats (pile retenue)

1. La sélection simple de l'arbre Reka stocke une **copie** de l'élément : un déplacement
   a supprimé le mauvais nœud. Sélection par identifiant, nœuds `markRaw`.
2. Haut/Bas de l'arbre virtualisé : Reka vise le voisin rendu ; en répétition rapide, des
   appuis sont perdus. Navigation verticale reprise sur l'index aplati.
3. Pas de `*` ; Entrée sélectionne ; le tampon de recherche reçoit les touches non
   imprimables : interception avant les gestionnaires de Reka (fragile, version épinglée).
4. `ComboboxViewport` et `Splitter` injectent un `<style>` : violation CSP ; interdits.
5. Combobox ouverte : `aria-hidden` sur le reste de la page, sans option.
6. Pas de grille dans Reka : motif APG Grid écrit à la main (~490 lignes), la part la plus
   risquée en accessibilité.
7. Types perdus dans les slots de `TreeVirtualizer` (`Record<string, any>`).

Quatre des neuf difficultés sont des comportements non documentés de Reka, trouvés par les
tests ; deux auraient été des bogues de données silencieux. Reka reste productif et sûr
pour les composants « catalogue » (dialogue, popover, combobox, champ numérique).

## Pourquoi pas React 19 + React Aria

Couverture accessible plus large (table, formulaires, couches), mais : poids de départ
double, tas de 116 Mo sur la table (la collection matérialise toutes les cellules malgré
la virtualisation du DOM), tri et filtre de 130 à 260 ms ; sortir de la table de la
bibliothèque pour tenir ces budgets reprendrait le travail clavier d'une grille maison ;
l'arbre demandait aussi des contournements liés à la version.

## Lacunes du prototype, reprises dans les specs

Ligne focalisée démontée par la molette (focus sur `<body>`) ; nom accessible d'une ligne
trop long ; dépôt « entre frères » absent ; Alt+Flèche bas d'un en-tête, convention maison ;
aucun essai avec un vrai lecteur d'écran.

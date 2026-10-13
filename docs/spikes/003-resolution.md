# Prototype 003 — Résolution matérialisée des propriétés

- Tâche : 003-1. Exigences : REQ-003-11 à 15, 19, 25, 26 ; ADR 0016. Date : 2026-10-14.
- Environnement : PostgreSQL 18.6 (devenv), jeu de référence : 1 188 939 objets
  (192 219 préfixes, 996 717 adresses), 10 propriétés dont 3 héritables, 1 682 335
  affectations (héritables : toutes les racines de domaine + ≈ 3 % des préfixes ;
  non héritables : ≈ 20 % des objets).
- Scripts : [`003-resolution/`](003-resolution/). Mesures indicatives, **sans** le coût
  de l'historisation (prototype 004 : ×2 sur les écritures) ni la segmentation temporelle
  (une seule période applicative dans ce prototype).

## Variante V : matérialiser la valeur (conception actuelle)

| Mesure | Résultat |
|---|---|
| Construction complète (3 propriétés héritables, `LATERAL` ancêtre le plus proche) | 109 s pour 3,57 M lignes (≈ 30 µs par ligne) ; 5,2 M lignes au total, 718 Mo |
| Cas 1 : conteneur `/10` inséré avec une affectation, 18 123 objets dans sa portée, 15 825 changent | calcul 1,7 s + écriture 1,5 s = **3,2 s** ; budget < 1 s pour 10⁴ : **dépassé** |
| Cas 2 : valeur changée sur la racine de chaque domaine, 757 845 objets changent | calcul 0,2 s + écriture **16,2 s** ; budget < 10 s pour 10⁶ : **dépassé** |

Le calcul est rapide ; c'est **la réécriture de la valeur** sur chaque objet héritier
qui coûte, et elle doublerait avec l'historisation.

## Variante O : matérialiser l'origine (amendement proposé)

`resolved_origin (object_id, property_id, origin_id)` : pour chaque objet et propriété,
l'objet qui porte l'affectation applicable. La **valeur** se lit par l'affectation de
l'origine (une recherche indexée).

| Mesure | Résultat |
|---|---|
| Cas 1 : seule la propriété affectée sur le nouveau préfixe peut changer ; cibles = objets de sa portée dont l'origine est un ancêtre du nouveau préfixe | cibles 143 ms + différentiel 59 ms + écriture 428 ms = **0,63 s** pour 15 825 objets |
| Cas 2 : changement de valeur à l'origine | différentiel (décompte de 757 845 objets) **0,65 s**, **aucune réécriture** |
| Page filtrée : 100 adresses d'un domaine, valeur fréquente (`prod`) | **78 ms** |
| Page filtrée, valeur rare (`rec`) | **27 ms** |
| Résolution expliquée de 2 000 adresses (valeur, origine, longueur de chaîne) | 43 ms, soit ≈ 22 µs par objet |

Filtrage : un objet et son origine sont toujours dans le même domaine (arbre
d'adressage) ; la requête part des **origines** du domaine qui portent la valeur
(petit ensemble), puis des objets résolus vers elles (index `(property_id, origin_id)`).
Une première forme, partie des objets, prenait 1,25 s.

### Règles de maintenance qui en découlent

1. **Changement de valeur** d'une affectation existante : aucune écriture dans
   `resolved_origin` ; différentiel = objets dont l'origine est cet objet.
2. **Nouvelle affectation** sur un objet P (ou insertion d'un préfixe P qui en porte) :
   seules les propriétés affectées sur P bougent ; cibles = objets de la portée de P dont
   l'origine est **strictement plus haut** que P.
3. **Effacement ou suppression d'affectation**, **clôture** ou **redimensionnement** de P :
   objets dont l'origine est P → nouvelle origine = ancêtre le plus proche portant une
   affectation (recalcul ciblé).
4. **Insertion d'un préfixe sans affectation** : aucun changement de résolution.

Ces règles remplacent le recalcul de toute la portée structurelle pour toutes les
propriétés héritables (conception actuelle) : le travail devient proportionnel au nombre
d'objets dont **l'origine** change, pas au nombre d'objets dans la portée.

### Limite qui demeure

Une modification **structurelle** très haute (clore ou effacer l'affectation d'une
racine de domaine) change l'origine de presque tous les objets du domaine : environ 30 µs
par objet pour le recalcul plus l'écriture, soit au-delà de 10 s pour 10⁶ objets, et le
double avec l'historique. Cas rare, mais à cadrer : proposition d'assouplir le budget de
REQ-003-26 pour ce seul cas (changement d'**origine** de plus de 10⁵ objets : objectif
< 60 s, décompte d'abord) ou de le traiter par un import (lots), à trancher.

## Conséquences pour la conception (à valider)

1. **ADR 0016 amendé** : la table matérialisée stocke l'**origine** et non la valeur ;
   la valeur se lit par jointure sur l'affectation. Effets : changements de valeur
   gratuits en écriture, différentiel par origine, volume comparable.
2. **003/design** : remplacer l'algorithme « portée structurelle × toutes propriétés
   héritables » par les quatre règles ci-dessus ; filtrage par les origines du domaine.
3. **REQ-003-26** : budget du différentiel confirmé pour les changements de valeur
   (10⁶ objets < 1 s, mieux que demandé) et pour les changements d'origine jusqu'à
   ≈ 10⁴ objets (< 1 s) ; cas des changements d'origine au-delà de 10⁵ objets à décider
   (voir la limite ci-dessus).
4. Les propriétés **non héritables** n'ont pas besoin de matérialisation : l'affectation
   est la valeur ; à exclure de la table pour en réduire le volume.

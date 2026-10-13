# Prototype 004 — Temps système : table courante, historique, déclencheur

- Tâche : 004-1. Exigences : REQ-004-16, 17, 38 ; ADR 0017. Date : 2026-10-14.
- Environnement : PostgreSQL 18.6 (devenv), jeu de référence (996 717 adresses), base du
  prototype 001.
- Scripts : [`004-historique/`](004-historique/). Mesures indicatives (une machine, pas
  de répétitions).

## Montage

- `address_cur` : versions courantes, clé `PRIMARY KEY (id, valid_period WITHOUT
  OVERLAPS)` (index GiST, btree_gist pour l'`uuid`).
- `address_cur_history` : mêmes colonnes + `sys_end`, partitionnée par mois de `sys_end`
  (37 partitions), index `(id, sys_start)`.
- Historique simulé sur 3 ans : 0 à 4 versions par adresse, périodes système contiguës,
  **1 994 340 versions historiques** (356 Mo) pour 996 717 courantes (199 Mo).
- Deux fonctions de déclencheur : générique (SQL dynamique, `format(… %I_history …)`) et
  spécifique à la table (SQL statique).

## Résultats

| Mesure | Résultat |
|---|---|
| Lecture courante par identifiant (2 000 objets, `LATERAL`, clé GiST) | 11 à 26 ms, soit **≈ 13 µs** par objet |
| Lecture `as_of` = 2025-03-15 par identifiant (courante ∪ historique) | 51 ms, soit **≈ 26 µs** par objet : **≈ ×2**, dans le budget (REQ-004-38) |
| `UPDATE` de 10⁵ lignes sans déclencheur | 1,93 s |
| … avec le déclencheur générique (dynamique) | 3,78 s (×2) |
| … avec le déclencheur spécifique (statique) | 3,69 s |
| Temps système dans une transaction (INV-12) | une seule valeur, égale à `now()` ; versions historiques closes au même instant |
| Rôle `kartenn_app` : `UPDATE` de la table courante | accepté, historisé par le déclencheur |
| Rôle `kartenn_app` : `INSERT`, `UPDATE`, `DELETE` sur l'historique | **refusés** (`permission denied`) |

Observations :
- L'élagage des partitions ne retire que les mois antérieurs à `as_of` (`sys_end > t`) :
  une lecture loin dans le passé interroge toutes les partitions postérieures ; avec
  3 ans, c'est 23 sondes d'index très courtes. À surveiller au-delà (partitions
  annuelles pour les années anciennes, par exemple).
- Le déclencheur générique coûte autant que le spécifique : **une seule fonction**
  partagée par toutes les tables versionnées suffit.
- Le coût du déclencheur (≈ ×2 sur les mises à jour) pèse sur les imports massifs ;
  avec les ≈ 105 µs par insertion mesurés par le prototype 001, la part base de données
  d'un import de 10⁶ objets reste de l'ordre de quelques minutes sur les 15 du budget.

## Conséquences pour la conception

ADR 0017 **confirmé**, avec les précisions suivantes (à reporter dans 004/design et
l'aide de migration de la tâche 004-4) :

1. Fonction de déclencheur **générique** unique, `SECURITY DEFINER` (propriétaire : rôle
   de migration), `SET search_path = pg_catalog, public`, sans bloc `EXCEPTION`
   (règles du prototype 001).
2. Le rôle applicatif n'a **aucun droit d'écriture** sur les tables d'historique (pas
   même `INSERT`) : seules les écritures passées par le déclencheur y arrivent.
3. Partitionnement mensuel par `sys_end` et index `(id, sys_start)` par partition :
   suffisants pour 3 ans ; revoir la granularité des partitions anciennes au-delà.
4. Pas d'index B-tree supplémentaire sur `id` : la clé `WITHOUT OVERLAPS` suffit pour les
   lectures par identifiant.

# Prototypes de risque (P1)

Prototypes jetables qui lèvent les inconnues techniques avant le code définitif. Les
scripts restent rejouables (base `spike_gist`, jeu `kartenn-bench gen 42`).

| Prototype | Tâche | Verdict | Décision attendue |
|---|---|---|---|
| [001 — Contraintes d'exclusion GiST](001-gist-exclusions.md) | 002-6 | Conception confirmée ; règles de migration ajoutées | Amendement 002/design : plages sans `subtype_diff`, `CHECK` de famille |
| [004 — Temps système et historique](004-historique.md) | 004-1 | ADR 0017 confirmé | Aucune (précisions : `SECURITY DEFINER`, aucun droit d'écriture sur l'historique) |
| [003 — Résolution matérialisée](003-resolution.md) | 003-1 | Variante « valeur » hors budget ; variante « origine » dans le budget | Amendement ADR 0016 et 003/design (matérialiser l'origine) ; budget des changements d'origine > 10⁵ objets |

Règles transverses issues des prototypes (à appliquer dans toutes les migrations) :
- jamais de bloc `EXCEPTION` dans une fonction appelée par un type, un index ou une
  contrainte ;
- appels qualifiés par schéma ou `SET search_path` dans ces fonctions (`ANALYZE` et la
  maintenance s'exécutent avec un `search_path` restreint depuis PostgreSQL 17) ;
- recherches d'inclusion en `LATERAL`, plans vérifiés en test.

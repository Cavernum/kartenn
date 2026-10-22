---
status: accepted
date: 2026-10-22
---

# 0030 — Verdicts de flux évalués en mémoire sur un instantané versionné de la politique

## Context

Le module politique (spec [010](../../specs/010-politique-flux/design.md)) rend des
verdicts de flux (REQ-010-11 à 15) : pour une paire d'extrémités et un service, à
(`valid_at`, `as_of`), un verdict (autorisé, autorisé sous réserve, refusé) puis une issue
(`automatic`, `review`, `noncompliant`). Une requête de conformité peut développer des
dizaines d'adresses par extrémité, et le registre de flux (011) réévalue les flux touchés
dans la transaction de chaque changeset. Les règles par attributs (D28 amendée, S22)
s'évaluent mal en SQL : sélecteurs multi-axes, « même valeur », motifs requis, conditions
d'admissibilité. Le design de la 010 proposait cet ADR sans numéro ; il est rédigé à la
synchronisation (tâche 010-20), une fois l'implémentation faite.

## Decision

1. **Instantané en mémoire.** `PolicySnapshot` (`kartenn-policy/src/snapshot.rs`) réunit
   axes, valeurs, types, services (index transport → intervalles de ports), règles
   indexées par service, motifs, conditions, signaux et textes, lus une requête par table
   à (`valid_at`, `as_of`) (`read.rs`, tâche 010-12).
2. **Cache versionné.** `SnapshotCache` (`cache.rs`), un par processus
   (`cache::shared()`, partagé entre les requêtes et les préparations d'écriture), garde
   les instantanés dans une LRU de 16 entrées. Une entrée vaut pour l'intervalle de temps valide où aucune version ne
   commence ni ne finit, sous la révision `policy_revision.policy` : une écriture de la
   politique incrémente la révision et recharge le présent ; le passé, immuable, n'est
   jamais relu. Le cache n'est alimenté que depuis une transaction qui n'a pas écrit la
   politique : une révision non validée peut être annulée et nommer ensuite un autre
   contenu. Le cache le vérifie lui-même (`xmin` de la ligne de révision comparé à la
   transaction courante) et lit alors la politique sans la garder. La clé nomme la base :
   (identifiant du cluster `system_identifier`, oid de la base, révision), lus dans la
   même ligne que la révision ; deux bases servies par un même processus peuvent partager
   une révision, jamais une politique.
3. **Deux étapes.** `evaluate` (domaine pur, `evaluate.rs`) rend d'abord le verdict :
   non classifié par axe → refus ; une règle `deny` qui s'applique l'emporte ; une règle
   `allow` donne l'autorisation, sous réserve si elle exige des motifs ; sinon refus par
   défaut ; port sans service → refus « service inconnu » (S8, D28). Puis l'issue, à
   partir des conditions d'admissibilité et du dossier (S30) ; le rang des valeurs d'axe
   ne sert qu'aux conditions, jamais au verdict.
4. **Valeurs résolues, jamais attendues** (INV-24). Les valeurs des extrémités viennent de
   la lecture groupée du noyau (016-4) et du port de lecture de la 009 ; l'inférence ne
   produit que des anomalies.

## Alternatives

- **Évaluation en SQL** : une requête par verdict ; sélecteurs multi-axes et « même
  valeur » coûteux à exprimer, aucun partage entre les verdicts d'une même requête.
- **Verdicts matérialisés pour toute paire** : produit cartésien hors de portée à 10⁶
  objets ; seule la 011 matérialise, et seulement les verdicts des flux déclarés.

## Consequences

- Une instance garde au plus 16 instantanés (≈ 5 Mo chacun pour le jeu du budget,
  estimation du design).
- Le différentiel de politique (010-9) compare deux instantanés de la même transaction.
- Budgets REQ-010-28 et 54 (banc `bench-policy`, 010-19, qui tient lieu du prototype
  010-1). Au banc complet (10⁶ adresses, politique à la taille du budget ; 2026-10-23),
  l'évaluation sur instantané tient tout :
  - instantané de 10⁴ règles lu à froid en 137 ms ;
  - verdict d'une paire en 11 ms (limite 50 ms) ;
  - machine × machine 16 × 16 en 27 ms (limite 300 ms) ;
  - différentiel en 511 ms (limite 1 s).

  Trois correctifs de l'évaluation y ont contribué, dès la variante réduite :
  - le différentiel compte ses paires de classes avant de les construire (sinon 29 Go
    en mémoire) ;
  - une requête de conformité évalue une seule fois chaque paire d'extrémités
    distinctes ;
  - l'instantané indexe les axes cités par service.

  Le jeu complet a ensuite fait échouer machine × machine (1,5 s) : les valeurs des
  machines, lues sur l'instantané daté des affectations joint en entier, lisaient
  toutes les affectations du référentiel ; elles sont lues par clé (2 ms). Chiffres
  et rapport d'audit dans [`docs/spec/invariants.md`](../spec/invariants.md).

---
status: approved
updated: 2026-10-15
---

# 004 — Changesets et bitemporel : tâches

Chaque tâche est un commit, laisse le build vert et nomme ce qu'elle implémente. Phases :
voir [`../README.md`](../README.md).

## P1 — Prototype de risque

- [x] 1. **Prototype temps système** (`docs/spikes/004-historique.md`) : paire de tables
  courante + historique avec le déclencheur générique, sur le jeu de 002-5 et 3 ans
  d'historique simulé : lecture courante vs `as_of` passé (≤ ×2), coût du déclencheur sur
  une mise à jour de 10⁵ lignes, partitionnement mensuel de l'historique — REQ-004-16,
  17, 38 — après 002-5 — verify : rapport avec mesures ; ADR 0017 amendé si besoin

## P2 — Domaine pur

- [x] 2. Périodes `[début, fin)`, validation, soustraction (0, 1 ou 2 morceaux), fusion
  des segments adjacents — REQ-004-13 — verify : tests de propriétés
- [x] 3. Découpage « FOR PORTION OF » et clôture sur un modèle en mémoire (référence pour
  les tests d'intégration) — REQ-004-14, 15 — verify : tests de propriétés sur des suites
  aléatoires d'opérations

## P3 — Stockage

- [x] 4. Fonction PL/pgSQL générique d'historisation et rôle applicatif sans
  `UPDATE`/`DELETE` sur l'historique ; aide de migration « table versionnée » (paire de
  tables + déclencheurs + partitions) — REQ-004-16 — verify : tests (temps système
  identique dans une transaction ; `UPDATE` SQL direct historisé ; refus sur
  l'historique)
- [x] 5. Tables `person`, `changeset`, `changeset_operation`, `changeset_diff`,
  `idempotency_key`, `approval_policy`, `outbox`, `import`, `import_batch` — prépare
  REQ-004-1, 10, 31, 36 — verify : migrations appliquées, tests de contraintes
- [x] 6. Lectures datées génériques (`valid_at`, `as_of`) et historique d'un objet —
  REQ-004-17, 18 — verify : tests avant et après correction rétroactive

## P4 — Pipeline

- [x] 7. Pipeline de base : décodage, taille max, verrous de domaine, application en
  transaction avec références internes, validation (point d'accroche 002/003),
  atomicité, journalisation séparée des refus — REQ-004-1 à 7, 12 — verify : tests
  d'intégration, dont concurrence — REQ-004-2 (période par opération) complétée le
  2026-10-15 (`Within`, `portion.rs`)
- [x] 8. Découpage en base (remplacement de la version par ses morceaux), clôture,
  validation sur toute la période visée (versions futures) — REQ-004-8, 14, 15 —
  verify : comparaison au modèle en mémoire de la tâche 3
  (`kartenn-service/tests/portion.rs`) — fonction `kartenn_portion` (suppression et
  réinsertion), opérations `UpdatePrefix`, `UpdateAddress`, `UpdateRange`
- [x] 9. Simulation (ROLLBACK) — REQ-004-9 — verify : empreinte de base inchangée
- [x] 10. Idempotence et concurrence optimiste — REQ-004-10, 11 — verify : tests
  parallèles
- [x] 11. Interface `Authorizer` (`authorize`, `authorize_many`, permission d'approbation,
  `…propose`) et implémentation de test configurable — prépare REQ-004-21, 41, 42 —
  verify : tests unitaires — `authorize_many` suffit (un seul objet est un cas
  particulier) ; permission d'approbation `changeset.approve` (`APPROVE_PERMISSION`)
- [x] 12. Machine d'états, retrait, rejet, expiration (job) — REQ-004-19, 23, 24 —
  verify : tests de chaque transition (`kartenn-service/tests/review.rs`) — expiration :
  `review::expire_pending`, à planifier par le job (006)
- [x] 13. Politiques d'approbation (volume, propriété, périmètre, auteur), aucune par
  défaut, validation par un approbateur non auteur avec revalidation — REQ-004-20 à 22,
  39, 40 — verify : tests (`review.rs`) — opération `SetApprovalPolicy` ; périmètres par
  entité, site ou sous-arbre et auteurs par rôle ou groupe avec la 005
- [x] 14. Propositions : bascule si `…propose`, droits requis stockés, validation par un
  mainteneur, file filtrée, notifications de l'auteur — REQ-004-41 à 43 — verify : tests
  (exemple de la VM) (`review.rs`, droits de propriété ; droits d'objet avec la 005)
- [x] 15. Cascade explicite via `dependents()` du contrat de module — REQ-004-25, 26 —
  verify : tests (réseau avec 3 plages et 40 adresses) (`kartenn-service/tests/cascade.rs`)
  — dépendants du noyau ; modules (008, 009) et droits sur les objets (005) ensuite

## P5 — Exploitation

- [x] 16. Marquage rétroactif et recherche d'audit — REQ-004-27, 31, 32 — verify : tests
- [x] 17. Personnes et pseudonymisation (job, délai configurable, 1 an par défaut) —
  REQ-004-33 — verify : historique intact après pseudonymisation
- [x] 18. Imports : tri par dépendance, lots, worker, pause sur erreur, reprise —
  REQ-004-28, 30 — verify : import de 10⁵ opérations avec un lot fautif
  (`kartenn-service/tests/import.rs`, 203 éléments en lots de 60 ; 10⁵ au banc 23) —
  `import::{create_import, run_import, import_status}` ; planification du worker (006)
- [x] 19. Rejeu d'import par identifiant externe — REQ-004-29 — verify : double import
  sans doublon (`import.rs`) — opération `BindExternal` (REQ-002-41)
- [x] 20. Changeset inverse et conflits — REQ-004-34, 35 — verify : tests
  (`kartenn-service/tests/undo.rs`) — `undo::inverse`, opération `Restore`, fonction
  `kartenn_restore` (migration 0021) ; déclarations non annulées, listées
- [x] 21. Outbox CloudEvents écrite dans la transaction — REQ-004-36 — verify : tests
- [x] 22. Relais d'outbox sans trou (filtre `xid` < xmin), ordre par domaine, accusé —
  REQ-004-37 — verify : tests avec transactions concurrentes longues

## P6 — Budgets et synchronisation

- [ ] 23. Banc CI : changeset unitaire, `as_of` passé, import de 10⁶, 50 changesets/s —
  REQ-004-38 — verify : seuils bloquants
- [ ] 24. Synchroniser `docs/spec/`, ADR 0017 et 0018 → acceptés, statut `implemented`
  — verify : rapport spec-keeper sans écart
- [x] 25. Entretien de l'historique : partitions mensuelles par `sys_end` créées à
  l'avance par un job (la partition par défaut ne reçoit plus rien), index GiST
  `tstzrange(sys_start, sys_end)` sur chaque historique — REQ-004-17, 38 — avant 23 —
  verify : tests (partitions des mois à venir présentes, index présent sur chaque
  historique) ; usage de l'index mesuré au banc 23 (ajoutée le 2026-10-15) — `kartenn_history_maintenance`
  (migration 0019), appelée par `kartenn-server migrate` (`maintenance::deploy`, qui
  synchronise aussi les déclarations du noyau) ; à relancer au moins chaque mois
  (`maintenance::history_partitions`) jusqu'au planificateur de jobs

---
status: draft
updated: 2026-10-21
---

# 016 — Amendements du noyau pour la v0.2 : tâches

Exigences : [`requirements.md`](requirements.md). Conception : [`design.md`](design.md).
Chaque tâche = un commit ; « verify » = test qui prouve la tâche ; « dépend de » = tâches
à finir avant ; « débloque » = tâches de la 010 et de la 011 qui attendent celle-ci.
Tests en `tests/it/` (ADR 0027) avec `#[kartenn_testing::pg_test]` (ADR 0028).
**Préalable** : validation par l'utilisateur des choix structurants (Q-016-1,
[`decisions.md`](decisions.md) § Choix structurants).

Trois files parallèles (fichiers disjoints, voir le design) ; dans une file, ordre
imposé. B passe avant C sur `kartenn-service/src/module.rs`.

## File A — Propriétés (003)

- [ ] 1. Liste de valeurs partagée (`values_of`) — REQ-016-2 — verify :
  `a_declaration_borrows_the_values_of_another`, `a_borrowed_value_cannot_close_while_cited`,
  `a_chain_of_borrowings_is_refused`
  - où : `kartenn-core/src/properties.rs` (`PropertyDeclaration`), store `properties.rs`,
    `property_read.rs::enum_values`, `kartenn-service/src/declarations.rs` ; migration du
    noyau suivante
  - dépend de : —
  - débloque : 010-7
- [ ] 2. Type `external_ref` et liste `core.external_system` — REQ-016-3 — verify :
  égalité sans le libellé (unitaire, `values.rs`), filtrage par (système, identifiant),
  système clos refusé ; contrat de référence régénéré (ajout)
  - où : `kartenn-core/src/values.rs` (`Scalar`), `properties.rs` (`TypeSpec`), store
    `properties.rs`, `filter.rs`, `kartenn-rest` (types de valeur)
  - dépend de : 1
  - débloque : 011-1 (références externes F2), 010-3 (axe à référence externe)
- [ ] 3. Sources de valeurs attendues v2 (contexte, candidats, lot, révision) —
  REQ-016-4, 5 — verify : `two_candidates_no_default_no_mismatch`,
  `a_rule_written_in_the_changeset_applies_to_its_creations`,
  `ten_thousand_objects_in_batches` ; route de suggestion avec `candidates`
  - où : `kartenn-service/src/expected.rs`, `kartenn-rest/src/properties.rs`
    (`…/suggestion`), store `expected.rs` ; REQ-003-21 à 24
  - dépend de : 2, 4 (lecteur des ancêtres)
  - débloque : 010-15
- [ ] 4. Lecture groupée des valeurs résolues — REQ-016-1 — verify :
  `resolved_many_matches_explain_on_random_pairs`, objet illisible absent ; budget au banc
  `bench-properties` (512 × 6 < 30 ms)
  - où : `kartenn-store-pg/src/explain.rs` (modèle), `property_read.rs`,
    `kartenn-service/src/reads.rs`, `kartenn-bench`
  - dépend de : 2
  - débloque : 010-2 (qui devient l'appel de ce port), 010-4, 010-13
- [ ] 5. Axe déclaré par l'administrateur — REQ-016-6 — verify :
  `an_admin_enum_reads_in_batch_and_takes_a_module_obligation`
  - où : tests seuls (`kartenn-service/tests/it/`)
  - dépend de : 4

## File B — Contrat de module, outbox, tâches (ADR 0026, 0018, 0023)

- [x] 6. Clés d'ordre de l'outbox — REQ-016-14 — verify :
  `two_changesets_under_one_module_lock_are_delivered_in_order`, forme d'un événement v0.1
  inchangée ; migration avec remplissage par lots
  - où : `kartenn-store-pg/src/outbox.rs`, `write.rs` (insertion de l'événement),
    migration du noyau ; amendement 4 de l'ADR 0018
  - dépend de : —
  — migration `0050_outbox_order_keys.sql` (`order_keys text[]`, existant rempli par
  tranches de 10 000 numéros) ; `Journal::keys` (clés de verrou des opérations de module,
  `pipeline::stages`) ; insertion `domain:<uuid>` puis clés de module ;
  `outbox::Event::order_keys`. Tests `kartenn-service/tests/it/outbox.rs` :
  `two_changesets_under_one_module_lock_are_delivered_in_order` (verrou nommé tenu par un
  changeset en cours, le suivant attend, ordre de livraison, payload sans `order_keys`),
  `the_ordering_keys_of_a_changeset_name_its_domains`.
  choix : migration numérotée 0050 (0045 à 0049 laissés à la file A, qui ajoute ses
  migrations en parallèle) ; le relais ne change pas, l'ordre par clé découlant du verrou
  nommé ; clés de module brutes (déjà qualifiées par leur module, ex. `vlan.group:<uuid>`).
- [x] 7. Tâches de fond de module — REQ-016-16 — verify :
  `a_module_job_runs_once_per_due_time_across_instances`, tâche retirée désactivée
  - où : `kartenn-service/src/module.rs` (`Module::jobs`, `ModuleHandler::run_job`),
    `kartenn-server/src/jobs.rs`, store `jobs.rs`, `modules.rs::deploy` ; migration
    (`job.enabled`)
  - dépend de : 6
  - débloque : 010-14, 010-16, 011-8
  — `module.rs` : `JobSpec`, `Module::jobs`, `ModuleHandler::run_job`,
  `ModuleRegistry::jobs`, `RegistryError::InvalidJob`, `run_job(store, name)` (une
  transaction par exécution, annulée sur erreur) ; store `jobs.rs` : `claim` ne prend que
  les tâches `enabled`, `declare_module_jobs` ; migration `0051_job_module.sql` (`enabled`,
  `module`) ; `maintenance::deploy` déclare les tâches ; `kartenn-server/src/jobs.rs`
  route un nom qualifié vers son module. Tests : `kartenn-server/tests/it/jobs.rs`
  `a_module_job_runs_once_per_due_time_across_instances` (trois ordonnanceurs, une
  exécution par échéance, tâche retirée désactivée et conservée) ; test unitaire
  `registry_refuses_a_repeated_job`. ADR 0026, amendement 2.
  choix : la colonne `module` repère les tâches d'un module (les tâches du noyau et des
  imports n'en ont pas) ; `run_job` n'a pas de journal d'audit en paramètre, une tâche qui
  en écrit le fait par sa transaction ; la reprise après arrêt d'une instance est celle des
  tâches du noyau (bail, test `a_stopped_holder_is_relieved_after_its_lease`).
- [x] 8. Événements de module — REQ-016-15 — verify :
  `a_module_event_is_delivered_only_if_applied`, `an_undeclared_event_fails_the_registry`,
  événement émis par une tâche
  - où : `module.rs` (`Module::events`, `ModuleTx::emit`), store `write.rs`,
    `outbox.rs`
  - dépend de : 6, 7
  - débloque : 010-8, 011-8
  — `module.rs` : `Module::events`, `ModuleTx::emit`, `ModuleRegistry::declares_event`,
  `RegistryError::InvalidEvent` (contrôle des noms au démarrage), `run_job` écrit les
  événements de la tâche avant de valider ; `port.rs` : `WriteTx::journal_job` ; store
  `write.rs` : `PgTx` garde les événements émis (`modules`, `events`) et les écrit avec le
  journal (`journal_applied` sous les clés du changeset, `journal_job` sous
  `<module>:<tâche>`) ; migration `0052_outbox_task_events.sql` (`changeset_id` nul) ;
  `outbox::Event::changeset` devient `Option<Uuid>`. Tests
  `kartenn-service/tests/it/module_events.rs` :
  `a_module_event_is_delivered_only_if_applied` (simulation et refus n'émettent rien),
  `an_undeclared_event_fails_the_registry` (nom invalide refusé au démarrage, nom non
  déclaré émis : le changeset échoue sans rien écrire), `a_background_task_emits_its_events` ;
  module de test `dummy` doté d'un événement et d'une tâche. ADR 0026 amendement 2,
  ADR 0018 amendement 4.
  choix : le registre ne peut pas connaître à l'avance ce qu'un module émet ; il contrôle
  les noms déclarés au démarrage, et l'émission d'un nom non déclaré échoue à l'exécution
  (opération en erreur, transaction annulée) ; `id` CloudEvents propre à chaque événement
  de module (`gen_random_uuid`), `subject` facultatif ; `domain_ids` du changeset recopiés
  sur ses événements de module, pour les consommateurs de la v0.1.
- [x] 9. Mesures de module et politiques par défaut — REQ-016-7, 8 — verify :
  `a_policy_on_a_module_measure_holds_the_changeset`, mesure inconnue refusée,
  `a_default_policy_is_installed_once`
  - où : `module.rs` (`measures`, `measure`, `default_policies`),
    `kartenn-service/src/review.rs`, `port.rs` (`ApprovalPolicy`), store
    `changesets.rs`, `modules.rs::deploy`, REST des politiques ; migration
  - dépend de : 8
  - débloque : 010-9, 011-6
  — `module.rs` : `Module::measures`, `Module::default_policies` (`DefaultPolicy`),
  `ModuleHandler::measure`, `ModuleRegistry::{declares_measure, default_policies}`,
  `RegistryError::InvalidMeasure` ; `operations.rs` : `ModuleTrigger`,
  `SetApprovalPolicy::module_triggers` ; `port.rs` : `ApprovalPolicy::module_triggers` ;
  `pipeline.rs` : refus d'une mesure inconnue au plan (règle `core.approval_policy.trigger`,
  catalogue régénéré), mesures lues dans `check_modules` ; `review.rs` : déclencheur de
  mesures (`Exposure::measures`, une mesure absente vaut zéro) ; store `pending.rs`
  (désormais public) : `module_triggers` lu et écrit, `install_default_policies` appelée
  par `maintenance::deploy` ; migration `0053_module_policies.sql` (`module_triggers`,
  `origin`, `module_default_policy`) ; REST : `ModuleTrigger` dans l'opération, contrat et
  types du SDK régénérés (ajouts). Tests `kartenn-service/tests/it/module_measures.rs` :
  `a_policy_on_a_module_measure_holds_the_changeset`, `an_unknown_measure_is_refused`,
  `a_default_policy_is_installed_once` (désactivée puis supprimée, jamais réinstallée) ;
  test unitaire de `holds` étendu.
  choix : `measure` reçoit la transaction et le contexte (le design la donnait sur le seul
  contexte) et rend des noms locaux, qualifiés par le noyau ; les mesures sont calculées
  avant la branche de simulation, mais la simulation n'expose toujours pas la politique
  qui retiendrait le changeset (pas de changement de sa réponse) ; la règle porte le nom du
  design, `core.approval_policy.trigger` ; `origin` n'est pas dans `ApprovalPolicy` (lu
  par personne pour l'instant), une politique par défaut modifiée par changeset garde
  `origin = 'module'`.

## File C — Changesets et identités (004, 005, 006, 012)

- [ ] 10. Crochet de fin de proposition — REQ-016-9 — verify :
  `a_rejection_calls_the_module_in_its_transaction`, retrait et expiration, échec du
  module annule le rejet
  - où : `module.rs` (`ModuleHandler::ended`), `review.rs` (`reject`, `withdraw`,
    `expire_pending`)
  - dépend de : 9 (rebase sur `module.rs`)
  - débloque : 011-6
- [ ] 11. Proposition forcée, permission de validation, opérations de revue —
  REQ-016-10, 11 — verify : `a_reviewed_operation_makes_a_proposal_despite_write`,
  `validation_needs_the_review_permission`, `review_operations_apply_with_the_proposal`,
  opération non « de revue » jointe → 422
  - où : `module.rs` (`review`, `OperationType::review_only`), `rights.rs`,
    `review.rs` (`approve`), REST `POST /v1/changesets/{id}/approve` ; migration
    (`review_permission`)
  - dépend de : 10
  - débloque : 011-3
- [ ] 12. Pour le compte d'un demandeur — REQ-016-12, 13 — verify :
  `a_service_account_writes_on_behalf_of_a_requester`, sans permission → 403, droits du
  seul compte de service, audit validé par le schéma, pseudonymisation du demandeur
  - où : `kartenn-rest/src/authn.rs`, `kartenn-service/src/authn.rs`, identités (005),
    store `identities.rs`, `write.rs` (changeset), `docs/spec/evenements-audit.schema.json` ;
    migration ; catalogue des permissions (`core.identity.act_on_behalf`)
  - dépend de : 11
  - débloque : 011-3 (demandes venues d'un outil de tickets), 010-13 (pré-vérification au
    nom d'un demandeur)

## Synchronisation

- [ ] 13. `docs/spec/` (data, interfaces, architecture, permissions, événements d'audit),
  amendement 2 de l'ADR 0026, amendement 4 de l'ADR 0018, `specs/README.md`,
  spec-keeper sans écart bloquant ; REQ-003-7, 21 à 24, REQ-004-37, 40 à 42, REQ-006-38,
  REQ-012-10 annotées « amendée par la 016 »
  - où : `docs/spec/`, `docs/adr/0018-*`, `docs/adr/0026-*`
  - dépend de : 1 à 12

## Tâches de la 010 et de la 011 débloquées

| Tâche 016 | Débloque |
|---|---|
| 1 | 010-7 |
| 2 | 010-3, 011-1 |
| 3 | 010-15 |
| 4 | 010-2, 010-4, 010-13 |
| 7 | 010-14, 010-16, 011-8 |
| 8 | 010-8, 011-8 |
| 9 | 010-9, 011-6 |
| 10 | 011-6 |
| 11 | 011-3 |
| 12 | 011-3, 010-13 |

Les tâches 010-1 (prototype des budgets), 010-5 et 010-6 (domaine pur) ne dépendent
d'aucune tâche de la 016 et peuvent commencer en parallèle.

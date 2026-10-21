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

- [x] 1. Liste de valeurs partagée (`values_of`) — REQ-016-2 — verify :
  `a_declaration_borrows_the_values_of_another`, `a_borrowed_value_cannot_close_while_cited`,
  `a_chain_of_borrowings_is_refused`
  — `TypeSpec::Enum { values_of }` et `TypeInput::Enum { values_of }` (constantes `ENUM`),
  `Declaration::validate` (même déclarant, pas soi-même), `check_evolution` (le
  propriétaire ne change pas sous des affectations), règle `prop.declaration.values_of` ;
  service : `check_values_of` à la déclaration et à la mise à jour (cible déclarée, liste
  fermée qui n'emprunte pas, prêteur qui n'emprunte pas), valeur ajoutée à un emprunteur
  refusée ; port `declaration_by_key`, `borrowers_of` ; store : `values_of` dans
  `type_params`, migration `0045` (`kartenn_values_owner`) lue par `enum_values_covering`,
  `property_facts`, `property_read::enum_values`, le filtre ; REST : `values_of` dans le
  type `enum` (contrat de référence et types du SDK régénérés, ajout seul) ; tests
  `kartenn-service/tests/it/declarations.rs` (les trois du verify) et unitaire de
  `properties.rs`.
  choix : `values_of` porté par `type_params` (jsonb existant) plutôt qu'une colonne : pas
  de colonne versionnée de plus ; règle nommée `prop.declaration.values_of` (catalogue des
  règles de propriétés, `prop.*`) et non `core.…` ; une valeur ne s'ajoute qu'au
  propriétaire. Limite : clore la déclaration d'un prêteur encore emprunté n'est pas
  refusé (ses valeurs sont closes avec elle, les affectations de l'emprunteur ne sont pas
  vérifiées) ; à reprendre si un module en a besoin.
  - où : `kartenn-core/src/properties.rs` (`PropertyDeclaration`), store `properties.rs`,
    `property_read.rs::enum_values`, `kartenn-service/src/declarations.rs` ; migration du
    noyau suivante
  - dépend de : —
  - débloque : 010-7
- [x] 2. Type `external_ref` et liste `core.external_system` — REQ-016-3 — verify :
  égalité sans le libellé (unitaire, `values.rs`), filtrage par (système, identifiant),
  système clos refusé ; contrat de référence régénéré (ajout)
  — noyau : `Scalar::External(ExternalRef)` (égalité, ordre et empreinte sur système et
  identifiant), `Raw::External`, `ExternalRefType` (système valide sur la période, texte
  NFC 1 à 256), `ValueContext::external_system` (défaut : aucun), `TypeSpec::ExternalRef` ;
  service : `TypeInput::ExternalRef`, `RawInput::External`, déclaration du noyau
  `core.external_system` (liste fermée livrée vide, `EXTERNAL_SYSTEM`), systèmes lus à
  l'affectation dans `resolution.rs` ; store : migration `0046` (colonnes versionnées
  `v_json`, `v_jsons`, contrainte de valeur unique, index GIN `jsonb_path_ops`,
  `kartenn_external_json` qui rend le système par sa clé), codec des affectations, filtre
  `<système>:<identifiant>` par inclusion, une valeur de système citée par une référence
  ne se clôt pas ; REST : type `external_ref`, valeur `{system, id, label}` (aussi en
  Merge Patch), contrat et types du SDK régénérés (ajouts) ; tests : unitaire
  `external_references_compare_without_their_label`, intégration
  `external_references_filter_by_system_and_identifier` (`kartenn-service/tests/it/filter.rs`).
  choix : une liste fermée doit avoir un porteur : `core.external_system` est porté par
  `entity`, sans usage ; colonnes `jsonb` dédiées plutôt que la colonne de texte (le design
  disait « colonne de valeur existante », aucune n'est `jsonb`) ; filtre écrit
  `<clé du système>:<identifiant>` (une clé de liste fermée n'a pas de `:`), conditions
  d'égalité et d'appartenance seulement.
  - où : `kartenn-core/src/values.rs` (`Scalar`), `properties.rs` (`TypeSpec`), store
    `properties.rs`, `filter.rs`, `kartenn-rest` (types de valeur)
  - dépend de : 1
  - débloque : 011-1 (références externes F2), 010-3 (axe à référence externe)
- [x] 3. Sources de valeurs attendues v2 (contexte, candidats, lot, révision) —
  REQ-016-4, 5 — verify : `two_candidates_no_default_no_mismatch`,
  `a_rule_written_in_the_changeset_applies_to_its_creations`,
  `ten_thousand_objects_in_batches` ; route de suggestion avec `candidates`
  — service : `ExpectedValueSource` asynchrone (`revision`, `expected`, `expected_many`
  par défaut en boucle) lisant par la transaction du changeset (`ModuleTx`) ;
  `ExpectedContext` (date de validité) ; `Expected::{None, One, Candidates}`, seul un
  candidat unique fait défaut ou écart ; `Suggestion::detail` dans la provenance
  (`source:<id>@<révision>#<détail>`) ; défauts et contrôles d'un changeset en un appel
  `expected_many` par propriété ; `suggest` et `suggest_for` rendent `Expected` ; REST :
  `candidates` dans la suggestion (`suggestion` nulle s'il y en a plusieurs), contrat et
  types du SDK régénérés (ajout) ; tests dans `kartenn-service/tests/it/expected.rs`.
  choix : la transaction passe à part du contexte (contexte `Copy`, dates seules : un
  emprunt mutable dans le contexte rendait les futures non `Send`) ; une règle écrite dans
  le changeset se lit par `ModuleTx::connection` (rabattu sur `PgConnection`).
  - où : `kartenn-service/src/expected.rs`, `kartenn-rest/src/properties.rs`
    (`…/suggestion`), store `expected.rs` ; REQ-003-21 à 24
  - dépend de : 2, 4 (lecteur des ancêtres)
  - débloque : 010-15
- [x] 4. Lecture groupée des valeurs résolues — REQ-016-1 — verify :
  `resolved_many_matches_explain_on_random_pairs`, objet illisible absent ; budget au banc
  `bench-properties` (512 × 6 < 30 ms)
  — store : `explain::resolved_many` (valeur, origine, chaîne, provenance, sensibilité, par
  objet puis clé, comme `explain` un à un) ; port `Reader::resolved_many` (`Resolved`) ;
  service `reads::resolved_many` : sans sujet tout, sinon les objets dont le genre est
  lisible (les autres absents, sans erreur), valeurs sensibles masquées (`masked`) sauf
  divulgation tracée ; banc : budget « grouped resolution 512 × 6 » ; tests
  `kartenn-service/tests/it/grouped.rs` (1 000 tirages comparés à `explain`, objet
  illisible absent). Mesure : 25,4 ms p95 sur le jeu `small` (charge machine ~3,3) ;
  jeu complet 10⁶ non rejoué au calme.
  choix : objets lus en 4 parts côte à côte sur le pool (latence d'une part), la requête
  des valeurs et celles des ancêtres en parallèle (`tokio::try_join!`, dépendance `tokio`
  du store) ; plans personnalisés (`persistent(false)`) ; genre et ancêtres par sondes
  latérales (les clés `GiST` lisent un `= ANY` par bitmap, lent à 10⁶) ; migration `0047`,
  index btree partiel `property_origin (object_id)` pour les origines d'un lot (partiel
  sur `object_id IS NOT NULL`, sans quoi le saut de colonne de PostgreSQL 18 l'offrait
  aux filtres par propriété).
  - où : `kartenn-store-pg/src/explain.rs` (modèle), `property_read.rs`,
    `kartenn-service/src/reads.rs`, `kartenn-bench`
  - dépend de : 2
  - débloque : 010-2 (qui devient l'appel de ce port), 010-4, 010-13
- [x] 5. Axe déclaré par l'administrateur — REQ-016-6 — verify :
  `an_admin_enum_reads_in_batch_and_takes_a_module_obligation`
  — test seul (`kartenn-service/tests/it/grouped.rs`) : liste fermée `custom.axis`,
  obligation ajoutée par changeset sur un domaine (l'autre refusé,
  `prop.assignment.required`), héritage par un préfixe, lecture groupée avec chaînes
  `[domaine]` et `[préfixe, domaine]`. Aucun code : le noyau le permettait déjà.
  - où : tests seuls (`kartenn-service/tests/it/`)
  - dépend de : 4

## File B — Contrat de module, outbox, tâches (ADR 0026, 0018, 0023)

- [ ] 6. Clés d'ordre de l'outbox — REQ-016-14 — verify :
  `two_changesets_under_one_module_lock_are_delivered_in_order`, forme d'un événement v0.1
  inchangée ; migration avec remplissage par lots
  - où : `kartenn-store-pg/src/outbox.rs`, `write.rs` (insertion de l'événement),
    migration du noyau ; amendement 4 de l'ADR 0018
  - dépend de : —
- [ ] 7. Tâches de fond de module — REQ-016-16 — verify :
  `a_module_job_runs_once_per_due_time_across_instances`, tâche retirée désactivée
  - où : `kartenn-service/src/module.rs` (`Module::jobs`, `ModuleHandler::run_job`),
    `kartenn-server/src/jobs.rs`, store `jobs.rs`, `modules.rs::deploy` ; migration
    (`job.enabled`)
  - dépend de : 6
  - débloque : 010-14, 010-16, 011-8
- [ ] 8. Événements de module — REQ-016-15 — verify :
  `a_module_event_is_delivered_only_if_applied`, `an_undeclared_event_fails_the_registry`,
  événement émis par une tâche
  - où : `module.rs` (`Module::events`, `ModuleTx::emit`), store `write.rs`,
    `outbox.rs`
  - dépend de : 6, 7
  - débloque : 010-8, 011-8
- [ ] 9. Mesures de module et politiques par défaut — REQ-016-7, 8 — verify :
  `a_policy_on_a_module_measure_holds_the_changeset`, mesure inconnue refusée,
  `a_default_policy_is_installed_once`
  - où : `module.rs` (`measures`, `measure`, `default_policies`),
    `kartenn-service/src/review.rs`, `port.rs` (`ApprovalPolicy`), store
    `changesets.rs`, `modules.rs::deploy`, REST des politiques ; migration
  - dépend de : 8
  - débloque : 010-9, 011-6

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

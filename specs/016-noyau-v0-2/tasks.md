---
status: implemented
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
  choix : migration numérotée 0050 (0045 à 0049 laissés à la file A, qui n'a pris que 0045
  à 0047 : 0048 et 0049 restent volontairement inutilisés, sans effet pour sqlx ; ses
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

- [x] 10. Crochet de fin de proposition — REQ-016-9 — verify :
  `a_rejection_calls_the_module_in_its_transaction`, retrait et expiration, échec du
  module annule le rejet
  - où : `module.rs` (`ModuleHandler::ended`), `review.rs` (`reject`, `withdraw`,
    `expire_pending`)
  - dépend de : 9 (rebase sur `module.rs`)
  - débloque : 011-6
  — Service : `module.rs` (`Ending`, `EndContext { changeset, how, motive, by,
  operations }`, `ModuleHandler::ended` sans effet par défaut), `review.rs` (`end` et
  `notify` : la fin, ses événements et l'appel des modules dans une transaction ;
  `expire_pending` dans une transaction, les modules appelés pour chaque changeset expiré),
  `port.rs` (`close_pending` et `expire_pending` passent de `Store` à `WriteTx`,
  `journal_ended`). Store : `pending.rs` (`close`, `expire` sur la connexion de la
  transaction, `expire` rend les changesets), `write.rs`. Tests :
  `kartenn-service/tests/it/module_ending.rs` (`a_rejection_calls_the_module_in_its_transaction`,
  `a_withdrawal_and_an_expiry_call_the_module`, `a_failing_module_rolls_the_rejection_back`),
  module factice `dummy` doté d'une table `ending` (migration 2) et de `ended`.
  choix : le module reçoit ses opérations telles que soumises (`ModuleOperation`) et non
  des plans (`plans` du design) : planifier demanderait l'état de la soumission ; un module
  en échec à l'expiration laisse tous les changesets du passage en attente (relancés au
  passage suivant) ; les événements émis par un module à la fin d'une proposition sont
  ordonnés sous `changeset:<id>`.
- [x] 11. Proposition forcée, permission de validation, opérations de revue —
  REQ-016-10, 11 — verify : `a_reviewed_operation_makes_a_proposal_despite_write`,
  `validation_needs_the_review_permission`, `review_operations_apply_with_the_proposal`,
  opération non « de revue » jointe → 422
  - où : `module.rs` (`review`, `OperationType::review_only`), `rights.rs`,
    `review.rs` (`validate_pending_with`), REST `POST /v1/changesets/{id}/validation` ; migration
    (`review_permission`)
  - dépend de : 10
  - débloque : 011-3
  — Service : `module.rs` (`ModuleHandler::review(plan) -> Option<&'static str>`,
  `Module::review_operations`), `pipeline.rs` (`reviews` : à la soumission, les
  permissions de revue s'ajoutent aux droits requis et font une proposition même avec
  l'écriture ; à la validation d'une proposition, la permission est vérifiée dans la
  transaction sur les objets écrits), `review.rs` (`validate_pending_with` : opérations
  jointes, refus `core.review.operation` hors des opérations « de revue » ou hors d'une
  proposition, le changeset reste en attente). Noyau : règle `core.review.operation`
  (`docs/spec/regles-noyau.md` régénéré). REST : `POST /v1/changesets/{id}/validation`
  accepte un corps facultatif `{"operations": […]}` (ajout ; contrat de référence et types
  du SDK régénérés). Store : `write.rs` (la validation d'un changeset en attente
  réécrit sa requête, opérations jointes comprises). Tests :
  `kartenn-service/tests/it/module_review.rs` (les trois du verify, le 422 compris), module factice `dummy` (revue des choses `review-…`,
  `update_thing` de revue).
  choix : la permission de revue est enregistrée dans les droits requis de la proposition
  (`required_rights`, déjà persistés) et redemandée au module à la validation, au lieu
  d'une colonne `review_permission` : pas de migration ; `Module::review_operations` (liste
  de noms) au lieu d'un champ `OperationType::review_only`, pour ne pas changer la forme
  littérale des types d'opération de tous les modules ; la route existante de validation
  porte les opérations jointes (le design la nomme `approve`) ; une opération jointe hors
  revue rend 422 `changeset-refused` avec la violation, journalisée en
  `authz.access.denied` (correctif de l'audit du 2026-10-21) ; un module disparu
  du build (`core.review.module` du design) n'a pas de traitement propre : ses opérations
  ne se planifient plus et la validation est refusée par la règle d'opération inconnue.
- [x] 12. Pour le compte d'un demandeur — REQ-016-12, 13 — verify :
  `a_service_account_writes_on_behalf_of_a_requester`, sans permission → 403, droits du
  seul compte de service, audit validé par le schéma, pseudonymisation du demandeur
  - où : `kartenn-rest/src/authn.rs`, `kartenn-service/src/authn.rs`, identités (005),
    store `identities.rs`, `write.rs` (changeset), `docs/spec/evenements-audit.schema.json` ;
    migration ; catalogue des permissions (`core.identity.act_on_behalf`)
  - dépend de : 11
  - débloque : 011-3 (demandes venues d'un outil de tickets), 010-13 (pré-vérification au
    nom d'un demandeur)
  — Noyau : permission globale `core.identity.act_on_behalf` (`authz.rs`), événement
  `authn.requester.created`, détail `on_behalf_of` admis sur tout événement
  (`AuditEvent::on_behalf_of`, schéma `docs/spec/evenements-audit.schema.json` et
  catalogue régénérés). Service : `RequestContext::on_behalf_of` (repris par chaque
  événement de la requête), `Journal::on_behalf_of`, `Identities::requester`,
  `authn::{Requester, act_on_behalf}` (compte de service seul, permission globale, identité
  `requester` créée si inconnue, une personne connue gardée telle quelle). Store : migration
  `0055_on_behalf.sql` (genre `requester`, `changeset.on_behalf_of`, droit
  `UPDATE (request)` sur `changeset` pour la tâche 11), `identities.rs` (`requester`),
  `write.rs`, `changesets.rs`, `audit.rs` (la pseudonymisation
  accepte un `requester`). REST : `on_behalf.rs` (dictionnaire RFC 9651 : `sub`, `iss`,
  `name`, chaînes ou jetons, sans paramètres), `authn.rs` (couche 6 : 400 sur une lecture,
  un en-tête mal formé ou répété ; 403 `core.identity.act_on_behalf` hors d'un compte de
  service autorisé ; `Authenticator::on_behalf_of`, refusé par défaut), fiche du changeset
  (`on_behalf_of`, ajout ; contrat et types du SDK régénérés). Tests :
  `kartenn-service/tests/it/on_behalf.rs` (`a_service_account_writes_on_behalf_of_a_requester`,
  `without_the_permission_or_as_a_person_it_is_refused`, `the_requester_lends_no_right`,
  `the_requester_is_pseudonymised_like_a_person`), `kartenn-rest/tests/it/on_behalf.rs`
  (en-tête dans la couche : succès, lecture, mal formé, personne, refus), unitaires de
  `on_behalf.rs`.
  choix : `on_behalf_of` est un détail de l'événement (`details.on_behalf_of`) et non une
  colonne de `audit.event` : une colonne changerait le `to_jsonb` des événements déjà
  scellés et casserait leur vérification ; l'événement de création s'appelle
  `authn.requester.created` (le catalogue impose trois segments, le design disait
  `authn.on_behalf`) ; sans `iss`, l'émetteur du demandeur est
  `urn:kartenn:service-account:<id>` (l'espace du compte de service) ; un `sub` déjà connu
  à cet émetteur (personne qui s'est connectée) est repris sans changer de genre, et un
  demandeur qui se connecte plus tard par son émetteur garde son genre `requester`
  (l'application n'a pas le droit de changer un genre, `rbac_schema` ; le genre du
  principal vient de l'authentification, non de cette colonne) ; le
  contexte, et non `ChangesetRequest`, porte le demandeur (aucun changement de la forme des
  requêtes enregistrées).

## Synchronisation

- [x] 13. `docs/spec/` (data, interfaces, architecture, permissions, événements d'audit),
  amendement 2 de l'ADR 0026, amendement 4 de l'ADR 0018, `specs/README.md`,
  spec-keeper sans écart bloquant ; REQ-003-7, 21 à 24, REQ-004-37, 40 à 42, REQ-006-38,
  REQ-012-10 annotées « amendée par la 016 »
  - où : `docs/spec/`, `docs/adr/0018-*`, `docs/adr/0026-*`
  - dépend de : 1 à 12
  — `data.md` (listes partagées, `external_ref`, lecture groupée, sources à candidats,
  clés d'ordre, événements de module, `on_behalf_of`, identité `requester`, tâches et
  politiques de module), `interfaces.md` (cycle d'un changeset : revue forcée et
  opérations de revue ; en-tête `Kartenn-On-Behalf-Of` ; événements de module),
  `architecture.md` (forme du contrat complétée), `invariants.md` (INV-60, INV-61) ;
  `permissions.md` et `evenements-audit.*` déjà régénérés par les tâches 9 et 12 ;
  ADR 0026, amendement 2 complété (`ended`, `review`, `review_operations`, écarts au
  design) ; ADR 0018, amendement 4 déjà écrit par la tâche 6 ; REQ annotées ; design
  corrigé sur le code (pas de `review_permission` ni de `review_only`, `on_behalf_of`
  dans les `details`, `authn.requester.created`).
  choix : spec-keeper remplacé par une relecture spec ↔ code à la main (un sous-agent ne
  peut lancer spec-keeper) ; les écarts relevés sont notés au point d'étape de la 016.

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

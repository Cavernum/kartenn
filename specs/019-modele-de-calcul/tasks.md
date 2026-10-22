---
status: proposed
updated: 2026-10-22
---

# 019 — Modèle de calcul et Kubernetes : tâches

Exigences : [`requirements.md`](requirements.md). Conception : [`design.md`](design.md).
Chaque tâche = un commit, laisse le build vert (`devenv test`, `k-check`) et nomme ce
qu'elle implémente. « verify » = test qui prouve la tâche ; « dépend de » = tâches à
finir avant. Tests en `tests/it/` (ADR 0027) avec `#[kartenn_testing::pg_test]`
(ADR 0028).

**Préalable bloquant** : arbitrage de Q-019-1 à 13 par l'utilisateur
([`decisions.md`](decisions.md) § À arbitrer) ; les tâches sont écrites selon les
recommandations et seront corrigées après arbitrage.

Parallélisme : après 0, la file A (module, 1 à 8) et la file B (connecteur, 9 à 11)
avancent en parallèle sur des crates disjointes ; 12 et 13 (politique, flux) attendent 7 ;
14 à 17 viennent en dernier.

## P0 — Arbitrage et décisions

- [ ] 0. Arbitrage et ADR — Q-019-1 à 13 — verify : `decisions.md` porte les décisions ;
  D1, D8, D11, D22 et D31 amendées selon les réponses ; ADR « observations hors
  référentiel » et « connecteur Kubernetes » rédigées (statut `proposed`) ; mentions
  « v0.3 » de la 010 et de la 011 corrigées (Q-019-13)
  - où : `specs/001-cadrage/decisions.md` (D1, D8, D11, D22, amendement de D31 du
    2026-10-22) ; `docs/adr/` (numéros suivants, modèle `0031-evaluations-de-flux-materialisees.md`) ;
    `specs/010-politique-flux/requirements.md` et `specs/011-registre-flux/requirements.md`,
    § Non-objectifs
  - dépend de : —

## File A — Module `kube` (P1 domaine pur, P2 module)

- [ ] 1. Crate `kartenn-kube`, domaine pur : noms (étiquette et sous-domaine DNS), clés et
  valeurs de labels, sélecteurs (égalité, `In`, `NotIn`, `Exists`, `DoesNotExist`),
  remontée `ownerReferences` au workload, projection des champs suivis, règles (`Rule`
  statiques) — REQ-019-6 à 10 (contrôles locaux), X3, X4 — verify : unitaires
  `label_key_syntax`, `selector_and_semantics`, `owner_chain_to_deployment`,
  `cronjob_job_pod`, `bare_pod_is_its_own_workload`, `image_change_projects_to_nothing`,
  `rule_ids_unique_and_namespaced`
  - où : design « Crates », « Règles » ; réf. `docs/reference/kubernetes-calcul.md`
    § Namespace, labels, noms et UID, § Pod ; modèle `crates/kartenn-machines/src/name.rs`,
    `rules.rs`
  - dépend de : 0
- [ ] 2. Ajouts à la 009 : `machines.platform`, filtre `hypervisor`, lectures
  `machines_of_addresses` et `machine_by_name` — REQ-019-1, 2, 23 (lecture) — verify :
  `hypervisor_follows_hosting`, `platform_is_a_closed_list`,
  `machine_of_internal_ip`, tests de la 009 inchangés
  - où : `crates/kartenn-machines/src/module.rs` (déclarations), `read.rs`
    (`attachments_of_address` à imiter), `rest.rs` (filtres de `/v1/machines`) ;
    `crates/kartenn-machines/tests/it/`
  - dépend de : 0
- [ ] 3. Clusters, nœuds, namespaces : migration `0001_kube.sql` (`cluster`, `node`,
  `namespace`), gestionnaire `create/update_cluster`, opération `kube.sync` réduite à ces
  types, module au registre (dépendance `machines`), feature `kube` — REQ-019-4 à 7, 13 —
  verify : `kube_needs_machines`, `cluster_slug_unique`, `range_in_designated_domain`,
  `node_name_unique_per_cluster`, `node_machine_kind`, `node_range_created_and_closed`
  - où : design « Types d'objets », « Données », « Opérations » ; modèle
    `crates/kartenn-machines/src/handler.rs` (`plan`, affectations planifiées,
    clés de verrou) et `migrations/0001_machines.sql` ; `crates/kartenn-server/src/modules.rs`
  - dépend de : 1
- [ ] 4. Workloads, services, ingress : tables, `kube.sync` complet, adresses `vip`
  planifiées, sélection d'un service, références et cascades, arêtes — REQ-019-8 à 11,
  14 — verify : `workload_recreated_keeps_identity`, `service_vip_refuses_close`,
  `selection_lists_both_ways`, `ingress_backend_missing_warns`, `execution_chain_edges`
  - où : design « Données », « Références » ; `kartenn-machines/src/handler.rs`
    (références et cascades de la tâche 009-8) ; ADR 0026 clauses 11, 12
  - dépend de : 3
- [ ] 5. Objets miroirs et absence : règle `kube.object.managed`, `mark_absent`,
  `clear_absent`, anomalie `kube.object.absent`, section `kube.objects` —
  REQ-019-12, 22 — verify : `managed_field_refused_for_human`,
  `property_on_mirror_accepted`, `cited_workload_goes_absent`, `absent_cleared_on_return`,
  `section_lists_changed_objects`
  - où : design « Règles », « Section de différentiel » ; `CheckContext::changeset`
    (`crates/kartenn-service/src/module.rs`) ; section `machines.attachments`
    (`crates/kartenn-machines/src/handler.rs`) à imiter
  - dépend de : 4
- [ ] 6. Journal d'observation : table partitionnée, route de lot sous
  `kube.observation.write` (audit par lot), purge en tâche de fond, lectures `who_had` et
  observations d'un workload — REQ-019-15 à 17 — verify : `who_had_ip_at_time`,
  `retention_answers_empty_with_reason`, `purge_drops_old_partitions`,
  `batch_is_audited_once`
  - où : design « Données » (journal), « Routes » ; ADR 0023 et
    `crates/kartenn-server/src/jobs.rs` (tâches de module, 016/G6) ; ADR « observations »
    (tâche 0)
  - dépend de : 3
- [ ] 7. Port de lecture et propriétés : porteurs `kube.*`, axe `kube.namespace`,
  `axis_values` (composition workload → namespace → cluster → arbre), `workloads_of`,
  `execution_chain`, correspondances de labels et valeurs admises (tables et opérations)
  — REQ-019-32 à 34 (côté module), REQ-019-3 — verify :
  `axis_from_namespace_then_tree`, `axis_values_one_query`, `label_mapping_admitted`,
  `looks_like_pod_warns`
  - où : design « Propriétés », « Port de lecture » ;
    `crates/kartenn-machines/src/read.rs` (`machine_values_of_addresses`) ;
    `crates/kartenn-core/src/properties.rs` (`InheritTree`, un arbre par déclaration)
  - dépend de : 4
- [ ] 8. Routes REST et imports : collections du module, filtres, chaîne d'exécution,
  correspondances (`If-Match`), contrat de référence et types du SDK régénérés —
  REQ-019-38, 39 — verify : tests HTTP des routes, `connector_role_confined`,
  contrat de référence (ajout seul)
  - où : design « Routes » ; `crates/kartenn-machines/src/rest.rs` ;
    `kartenn-server openapi`
  - dépend de : 5, 6, 7

## File B — Connecteur

- [ ] 9. Crate `kartenn-kube-sync` : choix du client Kubernetes (rapport `crate-scout`,
  Q-019-12), configuration, identifiants en fichiers relus, CA obligatoire, faux serveur
  d'API de test (liste, watch, coupure, `410`, signets, revue d'accès) — REQ-019-18, 29 —
  verify : `config_refuses_missing_ca`, `token_file_reloaded`,
  `no_credential_in_logs`, faux serveur rejouable
  - où : design « Connecteur » ; réf. § API, § Sécurité d'accès ;
    `crates/kartenn-server/src/config.rs` (`url_file`) à imiter ; `kartenn-workspace-hack`
  - dépend de : 0
- [ ] 10. Boucle list/watch et rapprochement : pagination, reprise, reliste sur `410`,
  projection, lots idempotents, ordre des lots, observations, dates — REQ-019-19 à 21,
  24, 25, 31 — verify : `converges_after_410` (oracle état projeté = état kartenn),
  `replayed_window_no_duplicate`, `image_change_no_changeset`,
  `endpoints_deduplicated_across_slices`, `fallback_full_list`,
  `env_vars_never_leave`
  - où : design « Connecteur », étapes 1 à 6 ; client REST tiré du contrat (tâche 8)
  - dépend de : 9, 8
- [ ] 11. Nœuds et machines, sécurité au démarrage : rapprochement par `InternalIP` puis
  nom, `create_node_machines`, revue de ses droits et refus de démarrer, manifeste de
  ClusterRole livré, rôle `kube.connector` — REQ-019-23, 26 à 28, 30 — verify :
  `node_matched_by_internal_ip`, `ambiguous_node_unmatched`,
  `refuses_with_secret_rights`, `only_read_verbs_sent`, manifeste relu par un test
  (verbes et ressources)
  - où : design « Connecteur » (démarrage) ; X10 ; `docs/exploitation/` (nouvelle page) ;
    rôles livrés de la 005 (`crates/kartenn-service`, rôles K12) à imiter
  - dépend de : 10, 2

## P3 — Politique et flux

- [ ] 12. Amendement de la 010 : porteur workload, extrémités logiques, `hostNetwork`,
  labels admis dans le différentiel de politique — REQ-019-32, 35 — verify :
  `workload_pair_one_verdict`, `namespace_expands_workloads`,
  `service_from_outside_uses_vip`, `host_network_uses_node_addresses`,
  `label_value_opens_flow_in_diff`
  - où : `crates/kartenn-policy/src/evaluate.rs`, `verdict.rs`, `model.rs` (extrémités),
    `contribution.rs` ; `specs/010-politique-flux/requirements.md` REQ-010-13, 29 (à
    amender) ; ADR 0030
  - dépend de : 7
- [ ] 13. Amendement de la 011 : extrémités logiques, réévaluation sur `kube.objects` et
  axes des porteurs `kube.*`, `flow.endpoint.absent`, lots du connecteur sous approbation
  — REQ-019-36, 37 — verify : `flow_on_workload_reevaluated_on_label`,
  `absent_endpoint_keeps_flow`, `sync_batch_as_proposal_waits`
  - où : `crates/kartenn-flow/src/reevaluation.rs`, `evaluation.rs`, `rules.rs` ;
    `specs/011-registre-flux/requirements.md` REQ-011-2, 11 ; ADR 0031
  - dépend de : 12, 5

## P4 — Interface, budgets, synchronisation

- [ ] 14. Maquettes du plugin `kube` validées par l'utilisateur — REQ-019-40 — verify :
  maquettes dans `specs/019-modele-de-calcul/maquettes/`, validation notée
  - où : 017/U14 et ses maquettes
  - dépend de : 8
- [ ] 15. Plugin `web/plugins/kube` — REQ-019-40 — verify : parcours Playwright (vues,
  chaîne d'exécution, onglet d'adresse), axe-core sans violation
  - où : `web/plugins/machines/` à imiter ; ADR 0025
  - dépend de : 14
- [ ] 16. Bancs : jeu augmenté (X14), mesures du module, du journal et du connecteur
  contre le faux serveur — REQ-019-41 — verify : `bench-core` étendu, chiffres reportés
  dans `docs/spec/invariants.md` § Performance
  - où : `crates/kartenn-bench` (jeu de la 009, tâche 12) ; sémaphore `k-heavy`
  - dépend de : 10, 13
- [ ] 17. Synchronisation : `docs/spec/` (data : schéma `kube` ; interfaces : routes,
  connecteur ; règles des modules régénérées), ADR acceptées, `docs/exploitation/`
  (déployer le connecteur, RBAC, rotation des jetons), audit `spec-keeper`
  - où : `docs/spec/data.md`, `interfaces.md`, `regles-modules.md` ; tâche 009-14
  - dépend de : 11, 13, 15, 16

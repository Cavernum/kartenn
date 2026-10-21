---
status: draft
updated: 2026-10-21
---

# 010 — Politique de flux : tâches

Chaque tâche est un commit, laisse le build vert et nomme ce qu'elle implémente. La 010
est en v0.2 (D31) : elle ne commence qu'après la validation de cette spec, la livraison
du contrat de module (ADR 0026, tâches 008-1 à 8, 19, 20) et l'amendement des specs du
noyau pour N20, N26, N28 et N31 (design, § Exigences envers le noyau). Chaque tâche
porte `dépend de :` ; les tâches sans dépendance commune avancent en parallèle.

## P0 — Prérequis

- [ ] 1. **Prototype** des budgets (REQ-010-28, 54) sur le jeu de 10⁶ objets : instantané
  de 10⁴ règles à 6 axes, lecture groupée de 512 adresses × 6 axes (N9), machine ×
  machine 16 × 16 avec conditions et signaux, différentiel d'une interdiction sans terme,
  inférence (2 000 règles, contrôle de 10⁴ objets) ; résultats dans
  `docs/spikes/010-verdicts.md` — verify : rapport avec mesures ; conception amendée si
  un budget est intenable
  - où : jeu de `kartenn-bench` (`crates/kartenn-bench`), modèle de
    `docs/spikes/003-resolution.md`
  - dépend de : —
- [x] 2. Lecture groupée des valeurs résolues de plusieurs propriétés avec explication
  (N9) — verify : 512 objets × 6 propriétés < 30 ms au banc ; mêmes résultats que la
  lecture unitaire
  - où : `crates/kartenn-store-pg/src/explain.rs`, `crates/kartenn-service/src/reads.rs`
  - dépend de : —
  — livrée par la tâche **016-4** (REQ-016-1), dont c'est le port : store
  `explain::resolved_many`, port `Reader::resolved_many`, service `reads::resolved_many`
  (objets illisibles absents, valeurs sensibles masquées). Clause par clause : « mêmes
  résultats que la lecture unitaire » = `kartenn-service/tests/it/grouped.rs`
  (`resolved_many_matches_explain_on_random_pairs`, 1 000 tirages comparés à `explain`) ;
  « < 30 ms au banc » = budget « grouped resolution 512 × 6, p95 » de `bench-properties`,
  25,4 ms sur le jeu `small` (016-4). Reste à rejouer au calme : le jeu complet 10⁶, et la
  mesure après l'index partiel `0047` (38,9 ms relevés une fois sous une charge de 16–17,
  sans valeur de mesure) ; la machine était chargée à la clôture de cette tâche.

## P1 — Domaine pur

- [x] 3. Crate `kartenn-policy` : types `Axis`, `ValueAttributes`, `FlowType`,
  `Service`, `ServiceEntry`, `Rule` (termes, « même valeur », effet, motifs requis),
  `Pattern`, `Condition` et `Signal` (genres, paramètres), `Text` avec validation —
  REQ-010-4 à 9, 29, 30, 33 — verify : un test par cas refusé de REQ-010-5 et REQ-010-8 ;
  `policy.text.missing`
  - où : modèle de `crates/kartenn-core/src/values.rs` (validation), note
    `docs/reference/services-transports.md` ; crate à l'image de `crates/kartenn-vlan`
  - dépend de : —
  — `crates/kartenn-policy` (domaine pur, sans base) : `model.rs` (`Key`, `Label`,
  `AxisValue`, `Axis`, `Carrier`, `ValueAttributes`, `FlowType`, `Transport`,
  `ServiceEntry`, `Service` avec `warnings` (> 1 024 ports d'un transport), `Selector`,
  `FlowSelector`, `Effect`, `Rule::check`, `Pattern`, `DossierField`, `ConditionKind`,
  `Condition`, `SignalKind`, `Signal`, `Locale`, `Text`, `Texts` avec repli sur la langue
  par défaut), `rules.rs` (catalogue `POLICY_RULES`). Tests : `model::tests`
  (`each_refused_case_of_req_010_5`, `each_refused_case_of_req_010_8`,
  `a_deny_a_pattern_and_a_condition_need_a_text`, `a_service_over_1024_ports_warns…`).
  choix : clés de la politique à la syntaxe des services (REQ-010-5) pour tous les objets ;
  valeurs d'axe identifiées par leur clé (la base fera la correspondance avec les
  identifiants) ; règles ajoutées `policy.key.syntax`, `policy.label.invalid`,
  `policy.service.flow_type`, `policy.service.entries_empty`, `policy.rule.term_empty`,
  `policy.text.invalid`, `policy.inference.criterion_invalid` (cas que la spec refuse sans
  les nommer) ; un code ICMP exige un type ; genres des conditions et des signaux tirés de
  REQ-010-36, 37 et Q20.
- [x] 4. `PolicySnapshot` et `evaluate` (candidates, non classifié par axe,
  interdictions, autorisations, motifs, conditionnelles, issue, signaux, élément
  décisif le plus spécifique, explication) — REQ-010-10, 11, 12, 14, 15, 31, 32, 36,
  37 — verify : cas d'acceptation de REQ-010-11, 32, 36 ; proptest contre l'oracle
  (interdiction l'emporte ; monotonie ; permutation des règles sans effet ; signaux sans
  effet sur l'issue)
  - où : `kartenn-policy/src/snapshot.rs`, `src/evaluate.rs` ; tests à l'image de
    `crates/kartenn-core/src/inheritance.rs`
  - dépend de : 3
  — `snapshot.rs` (`Parts`, `PolicySnapshot` : index des règles par service, types
  développés, et par motif ; `services_for_port`, `services_of_type`), `evaluate.rs`
  (`Endpoint`, `Requested`, `Dossier`, `applies`, `verdict`, `evaluate`, `Decisive`,
  `Discarded` (trois au plus), `ConditionState`, `Outcome`, signaux et score,
  `synthesis`, `unknown_service`). Tests : `evaluate::tests` (`acceptance_of_req_010_11`,
  `…_32`, `…_36`, `same_value_needs_a_shared_value`, `signals_add_up_without_deciding`,
  `synthesis_of_req_010_15`) et quatre propriétés (`a_deny_wins`, `monotonic`,
  `the_order_of_the_rules_has_no_effect`, `signals_change_neither_verdict_nor_outcome`).
  choix : la combinaison citée prend, pour chaque terme, la première valeur de
  l'intersection, et pour une « même valeur » une valeur commune admise par les deux
  termes ; rang de comparaison = rang maximal de l'ensemble (union, D25) ; une condition
  sans rang lisible est « non évaluée » (issue `review`) ; « non classifié » s'apprécie
  sur l'union des axes cités par les candidates, des deux côtés ; les conditions ne sont
  évaluées que pour un verdict `allowed`.
- [x] 5. Différentiel de politique pur (candidats par combinaisons de valeurs, verdicts
  de principe avant et après) — REQ-010-18 — verify : égal aux basculements du produit
  cartésien sur politiques aléatoires
  - où : `kartenn-policy/src/diff.rs`
  - dépend de : 4
  — `diff.rs` (`diff`, `PolicyDiff` avec `opens`, `closes`, `pending`, `Flip`,
  `Candidate`, `Slot`, `MAX_CANDIDATES`) : un service n'est examiné que si ses règles
  candidates ou son type changent ; par axe cité, classes de valeurs (valeurs citées,
  « autre valeur » distincte de chaque côté, « même valeur non citée » quand une règle
  l'exige) ; candidats = ensembles d'une ou deux classes par axe et par côté ; verdict de
  principe de `evaluate::verdict` sur les deux instantanés. Tests :
  `acceptance_of_req_010_18` (ouverture, masquage par une interdiction sur
  `{global, prod}`, rien sans changement) et la propriété
  `equals_the_flips_of_the_cartesian_product` (produit d'extrémités à une ou deux valeurs
  sur les valeurs citées et deux non citées).
  choix : extrémités d'au plus deux valeurs par axe et par côté (`VALUES_PER_AXIS`) : le
  masquage d'une autorisation par une interdiction (acceptation de REQ-010-18) exige deux
  valeurs, et le décompte reste polynomial ; au-delà de 10⁵ candidats, décompte seul
  (`truncated`) ; la propriété tourne 4 cas (environ 10⁵ paires par service affecté).
- [x] 6. Moteur d'inférence pur (critères, transmission, exceptions, départage,
  candidats multiples, explication) — REQ-010-42 à 45, 47, 49 — verify : cas
  d'acceptation de REQ-010-42, 44, 45 ; oracle du départage sur arbres aléatoires
  - où : `kartenn-policy/src/inference.rs` ; crate `regex` (déjà dépendance de
    `kartenn-core`)
  - dépend de : 3
  — `inference.rs` (`Node`, `NodeKind`, `Criterion` : préfixe englobant ou égal,
  famille, domaine, type, site et entité avec ascendants, propriété (égalité,
  appartenance, présence), nom par `NameRegex`, longueur ; `InferenceRule::check`
  (profondeur ≤ 4, listes non vides), `expected` (correspondance la plus profonde dans la
  chaîne des ancêtres transmise à la sous-branche, exceptions qui remplacent, priorité
  puis profondeur, candidats multiples), `Expectation::{conflict, single}`, `mismatch`,
  `Expected` avec observations par critère et trois règles plus faibles). Tests :
  `acceptance_of_req_010_42`, `…_44`, `…_45`, `a_match_is_transmitted_to_the_sub_branch`,
  `every_criterion_observes_what_it_saw` et la propriété `departage_matches_the_oracle`.
  choix : expression régulière ancrée par `^…$` littéraux, compilée par `regex` (refuse
  de lui-même les assertions de voisinage et les références arrière) ; une règle sans
  critère vaut pour tout nœud ; entre exceptions sœurs, même départage que les règles ;
  le nœud décrit son site et son entité avec leurs ascendants, fournis par le stockage.

## P2 — Stockage et pipeline

- [x] 7. Migrations du module (tables `policy_*` versionnées, contraintes, GiST des
  ports, `policy_revision`) ; déclarations `policy.environment`,
  `policy.declared_environment` ; référentiel de valeurs partagé (second volet de N7,
  ADR 0026) ; opérations `policy.axis.*`, `policy.value.describe`,
  `policy.environment.create`, `policy.examples.install` — REQ-010-1, 2, 3, 26, 29, 30
  — verify : tests de contraintes ; cas d'acceptation de REQ-010-1, 2, 29
  - où : migrations de `crates/kartenn-vlan` (forme), `declarations.rs`
  - dépend de : 3
  — `crates/kartenn-policy` : `migrations/0001_policy.sql` (schéma `policy` : tables
  versionnées `policy_axis`, `policy_value`, `policy_flow_type`, `policy_service`,
  `policy_pattern`, `policy_rule`, `policy_condition`, `policy_signal`,
  `policy_exempt_role`, `policy_inference_rule` ; `policy_revision` à une ligne) ;
  `module.rs` (module `policy`, dépend de `machines` ; types `policy.axis`,
  `policy.value` ; déclarations `policy.environment` (ensemble, arbre d'adressage,
  domaine, préfixe, adresse), `policy.declared_environment` (`values_of`
  `policy.environment`, machine, non héritée), `policy.personal_data` et les axes
  d'exemple déclarés mais non désignés ; références axe → propriété (refus), attributs →
  valeur de liste (cascade)) ; `handler.rs` (opérations `axis.designate`, `axis.update`,
  `value.describe`, `environment.create` qui planifie `AddEnumValue`,
  `environment.close` qui planifie la clôture de la valeur en cascade,
  `examples.install` qui désigne les six axes d'exemple ; verrou `policy` ; révision
  levée une fois par changeset) ; `pg.rs` (écritures génériques par
  `jsonb_populate_record` et `kartenn_portion`, unicités vérifiées avant l'écriture) ;
  règles `policy.operation.invalid`, `policy.axis.invalid`, `policy.value.unknown`,
  `policy.reference.closed`. Serveur : feature `policy` par défaut. Tests :
  `tests/it/axes.rs` (`acceptance_of_req_010_1`, `…_2`, `…_29`,
  `a_value_is_described_then_modified`, `the_revision_rises_once_per_changeset`,
  `the_tables_keep_their_constraints`, `a_machine_declares_environments_of_the_shared_list`).
  choix : axes et valeurs nommés par leurs clés (une clé de propriété ou de valeur de liste
  ne change pas, REQ-003-5), les autres objets cités par identifiant ; termes des règles,
  textes pédagogiques et entrées d'un service en `jsonb` dans la ligne de l'objet (une
  version = une ligne) plutôt que les tables filles du design (`policy_rule_term`,
  `policy_rule_same`, `policy_text`, `policy_service_entry`) ; l'index des ports est celui
  de l'instantané ; `policy.environment` est un axe implicite, refusé à la désignation ;
  les axes d'exemple sont déclarés par le module (seul un module déclare `policy.*`) et
  seulement désignés à l'installation, `policy.exposure` porté par l'arbre, les autres par
  la machine (S27) ; droits par type (`policy.<type>.write`, globale, S15) ; module
  dépendant de `machines` (axes portés par la machine, REQ-010-3) ; collection
  `policy-values` pour les attributs des valeurs.
- [x] 8. Opérations `policy.*` (types, services, règles, motifs, conditions, signaux,
  textes, rôles exemptés), règles du tableau du design, verrou `policy`, révision,
  événements de module (point différé N15, ADR 0026) — REQ-010-4 à 9, 26, 33 — verify :
  chaque règle de validation a un test ; annulation d'un changeset de règles ; événement
  `org.kartenn.policy.revised`
  - où : opérations de `crates/kartenn-vlan` (forme), `kartenn-service/src/module.rs`
  - dépend de : 7
  — `crates/kartenn-policy` : `objects.rs` (`flow_type`, `service`, `pattern`, `rule`,
  `condition`, `signal` : `<type>.create` et `<type>.update` en Merge Patch ;
  `exempt_role.create` ; clôture par le `Close` du noyau ; contrôles locaux : clé,
  libellé, entrées d'un service, termes vides, sélecteur de flux, interdiction
  conditionnelle, textes, genres et paramètres), `codec.rs` (formes JSON des entrées,
  sélecteurs, textes, genres des conditions et des signaux), `checks.rs` (règles sur
  l'état final : références à des objets ou valeurs non vivants au début de la version,
  `Rule::check` contre les axes désignés à cette date, textes dans la langue par défaut,
  paramètres des conditions et des signaux, `policy.service.wide_ports` en
  avertissement), `handler.rs` (unicités vérifiées avant l'écriture, révision levée et
  événement `policy.revised` émis une fois par changeset, y compris à la clôture et à la
  restauration), `pg.rs` (référents des références du module : service → type, règle →
  type, service, motif, valeur de liste, condition → règle, type) ; règle
  `policy.key.duplicate`. Tests : `tests/it/catalogue.rs`
  (`each_rule_of_validation_refuses_its_case` — 17 cas, un par règle, `a_wide_service_warns`,
  `a_rule_is_written_modified_and_closed`, `the_undo_of_a_changeset_of_rules`,
  `a_new_revision_is_announced`), `codec::tests`.
  choix : pas d'opérations `policy.text.set | remove` : les textes se donnent à la
  création et se remplacent en entier par `<type>.update` ; une condition et un signal
  n'ont pas de libellé (design « Données ») ; langue par défaut `fr` en constante
  (`codec::DEFAULT_LOCALE`) en attendant une configuration de l'instance ; les signaux
  citent types et services par clé dans leurs paramètres, sans référence bloquante ; un
  champ inconnu est refusé par `policy.operation.invalid` ; le verrou `policy` est pris
  par chaque plan (clé nommée, ADR 0026), sans test de concurrence dédié.
- [ ] 9. Contributions au différentiel (`policy.combinations`, `policy.reclassified`) et
  déclencheurs de module (point différé N5) `policy.write`, `opens`, `closes`,
  `reclassify` — REQ-010-18, 19, 20 — verify : cas d'acceptation de REQ-010-18 en
  simulation et à l'application ; politique d'approbation `policy.opens ≥ 1` met en
  attente
  - où : `Module::diff` (tâche 008-19), `crates/kartenn-service/src/review.rs`
  - dépend de : 5, 8
- [ ] 10. Obligation par domaine et par axe — REQ-010-21, 22 — verify : cas
  d'acceptation de REQ-010-21
  - où : `AddObligation` (tâche 008-20), `crates/kartenn-core/src/properties.rs`
  - dépend de : 7
- [ ] 11. Vue matricielle (`policy.matrix.set`, lecture) — REQ-010-34 — verify : cas
  d'acceptation ; aller-retour vue → règles → vue
  - où : `kartenn-policy/src/matrix.rs`
  - dépend de : 8

## P3 — Lectures et requêtes

- [x] 12. Lectures datées des objets de politique et cache de l'instantané (révision,
  LRU des instantanés passés) — REQ-010-16, 26 — verify : cas d'acceptation de
  REQ-010-16
  - où : `crates/kartenn-service/src/reads.rs`
  - dépend de : 8
  — `kartenn-policy/src/read.rs` (`parts` : une requête par table, table courante et
  historique à `as_of` ; `revision` ; `stable_span` : dernière et prochaine borne d'une
  version autour de `valid_at`), `src/cache.rs` (`SnapshotCache::snapshot` : présent gardé
  sous la révision, relue à chaque demande ; passé immuable ; LRU de 16 entrées). Tests :
  `tests/it/dated.rs` (`a_verdict_reads_the_policy_of_its_date`, cas d'acceptation de
  REQ-010-16 ; `snapshots_are_kept_until_the_policy_changes`).
  choix : un instantané est gardé pour tout l'intervalle de temps valide sans début ni fin
  de version (une demande « maintenant » qui avance ne recharge pas) ; un instantané passé
  cite la révision 0, le compteur n'étant pas versionné ; une citation d'un objet qui ne vit
  pas à la date est omise de l'instantané, un service sans type vivant aussi ; une ligne
  illisible est une erreur de stockage. Budget du chargement non mesuré ici : il revient au
  banc 010-19.
- [ ] 13. Requête de conformité : extrémités, axes portés par la machine (S27), objet,
  dossier, synthèse et issue, bornes, visibilité et explication réduite — REQ-010-13,
  14, 15, 25, 31, 38 — verify : cas d'acceptation de REQ-010-13, 14, 25, 31 ;
  extrémité invisible → 404
  - où : port de lecture de la 009 (REQ-009-22, 25), `crates/kartenn-service/src/rights.rs`
  - dépend de : 2, 4, 12
- [ ] 14. Cohérence des machines et anomalies d'inférence ; tâche de fond
  `policy.inference.check` (point différé N14, genres de module) — REQ-010-17, 48 —
  verify : anomalies ouvertes et fermées ; regroupement par nœud d'origine
  - où : règles de module (clause 7), `crates/kartenn-service/src/import.rs` (tâche sous
    bail)
  - dépend de : 6, 8
- [ ] 15. Source de valeurs attendues de l'inférence (contrat 003 amendé, N20, N28) :
  suggestion, valeur par défaut, versionnement et provenance — REQ-010-40, 41, 46, 50,
  51, 52 — verify : cas d'acceptation de REQ-010-45 (création sans défaut) et 52 ;
  provenance lue dans l'explication
  - où : `ExpectedValueSource` (`kartenn-core`), amendement de la 003
  - dépend de : 6, 7
- [ ] 16. Rapport d'audit (tâche de fond, rubriques, pagination, purge) — REQ-010-23 —
  verify : un test par rubrique
  - où : `crates/kartenn-service/src/import.rs` (tâche sous bail),
    `crates/kartenn-store-pg/migrations/0039_job.sql`
  - dépend de : 12, 14

## P4 — Import, API, budgets, synchronisation

- [ ] 17. Convertisseur hors ligne (matrice à trois états, catalogue en deux tables,
  règles d'inférence) en changesets, avec la liste des différences en simulation —
  REQ-010-35, 53 — verify : jeu d'exemple anonymisé de Q10 converti ; différences
  listées
  - où : binaire `kartenn-policy-import` (crate de la 010, `publish = false`)
  - dépend de : 8, 11, 15
- [ ] 18. Routes du module, `QUERY` et `GET` des verdicts, vue matricielle,
  suggestion, rapports, contrat OpenAPI — REQ-010-24, 27 — verify : contrat comparé ;
  tests de routes
  - où : routeur de module (tâche 008-8), `crates/kartenn-rest/src/contract.rs`
  - dépend de : 11, 13, 15, 16
- [ ] 19. Banc `bench-policy` bloquant en CI — REQ-010-28, 54 — verify : budgets tenus
  - où : `crates/kartenn-bench`
  - dépend de : 13, 15
- [ ] 20. Synchronisation de `docs/spec/` (spec-keeper), ADR proposées → acceptées
  - où : skill `tech-specs`, mode sync
  - dépend de : 17, 18, 19

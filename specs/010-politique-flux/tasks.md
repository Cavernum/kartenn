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
- [x] 9. Contributions au différentiel (`policy.combinations`, `policy.reclassified`) et
  déclencheurs de module (point différé N5) `policy.write`, `opens`, `closes`,
  `reclassify` — REQ-010-18, 19, 20 — verify : cas d'acceptation de REQ-010-18 en
  simulation et à l'application ; politique d'approbation `policy.opens ≥ 1` met en
  attente
  - où : `Module::diff` (tâche 008-19), `crates/kartenn-service/src/review.rs`
  - dépend de : 5, 8
  — `crates/kartenn-policy` : `load.rs` (instantané lu en base à un instant de validité,
  état **après** les écritures du changeset ou **avant** elles : versions plus anciennes
  que la transaction plus celles qu'elle a remplacées, dans l'historique ; conditions et
  signaux non lus, ni le verdict de principe ni le différentiel n'en dépendent),
  `contribution.rs` (section, mesures), `handler.rs` (`diff`, `measure`), `module.rs`
  (`measures` : `write`, `opens`, `closes`, `reclassify`, qualifiées `policy.*` par le
  noyau, aucune appliquée par défaut). Tests : `tests/it/differential.rs`
  (`acceptance_of_req_010_18` en simulation et à l'application,
  `a_policy_on_opens_holds_the_changeset`, `a_reclassified_node_counts_its_heirs`).
  choix : le contrat donne une section par module : `policy.combinations` porte, par le
  genre de ses lignes, le décompte (`policy.summary`, objet nul), les combinaisons qui
  basculent (`policy.combination`, objet nul, avant et après : service, classes de
  valeurs par axe — `*other`, `*same` —, verdict, élément décisif) et les nœuds reclassés
  (`policy.reclassified`, objet = nœud, axe et nombre d'héritiers) ; verdicts de principe
  au présent de la transaction (`now()`), pas à la date d'effet du changeset ; héritiers
  groupés par l'origine de leur valeur **après** le changeset (un retrait d'affectation
  compte les héritiers sous l'ancêtre qui la reprend) ; `policy.reclassify` mesure le plus
  grand total d'héritiers reclassés sur un axe ; `policy.write` compte les objets de la
  politique écrits ; la troncature au-delà de 10⁵ candidats garde le décompte seul
  (`truncated`).
- [x] 10. Obligation par domaine et par axe — REQ-010-21, 22 — verify : cas
  d'acceptation de REQ-010-21
  - où : `AddObligation` (tâche 008-20), `crates/kartenn-core/src/properties.rs`
  - dépend de : 7
  — l'obligation d'un administrateur du noyau (008-20) sur la propriété de l'axe suffit,
  sans opération du module ; un correctif du noyau était nécessaire : la tenue d'une
  obligation ajoutée se contrôlait à l'application de l'opération, avant la résolution, si
  bien qu'un nœud classé dans le même changeset comptait encore comme sans valeur. Elle se
  contrôle maintenant sur l'état final, la portée entière, avec les obligations des
  déclarations (`kartenn-service/src/resolution.rs::check_obligations`, `pipeline.rs`
  ne vérifie plus que la portée ; `declarations::admin_obligation_held` retirée). Tests :
  `kartenn-policy/tests/it/obligation.rs` (`acceptance_of_req_010_21_required_environment`
  : refus listant les 12 adresses, accepté une fois classées dans le même changeset ;
  `acceptance_of_req_010_21_required_exposure` : un réseau qui hérite l'exposition est
  accepté sans environnement, un réseau qui l'arrête est refusé). REQ-010-22 (non
  classifiés comptés) relève du rapport d'audit (tâche 16).
- [x] 11. Vue matricielle (`policy.matrix.set`, lecture) — REQ-010-34 — verify : cas
  d'acceptation ; aller-retour vue → règles → vue
  - où : `kartenn-policy/src/matrix.rs`
  - dépend de : 8
  — `kartenn-policy/src/matrix.rs` : `view` (lecture pure sur un instantané : cellules,
  état combiné, motifs, décompte des règles hors vue), `cell_of` ; `prepare_set` (règle de
  la cellule lue dans la transaction) et `plan_set` (création, mise à jour ou clôture de
  la règle) ; opération `policy.matrix.set` (`module.rs`, `handler.rs`). Tests :
  `tests/it/matrix.rs` (`a_conditional_cell_is_an_allow_that_requires_its_patterns`, cas
  d'acceptation ; `a_view_written_and_read_back_keeps_one_rule_per_cell`, aller-retour ;
  `a_cell_is_refused_without_what_its_state_requires`).
  choix : quatrième état d'écriture `none` (vide la cellule : clôture de sa règle ; une
  cellule déjà vide est refusée) ; une règle de vue n'appartient à aucun motif, ne cite ni
  service ni « même valeur » ; plusieurs règles dans une cellule : `deny` l'emporte, puis
  `allow` sans motif, sinon `conditional` avec l'union des motifs, et l'écriture met à jour
  la première par clé ; clé générée `matrix-<uuid>`, libellé `source → destination` par
  défaut ; hors vue = règle qui cite le type de flux ou un de ses services sans être une
  règle de vue ; la route REST revient à 010-18 ; droits : ceux d'une règle
  (`policy.rule.admin`).

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
- [x] 13. Requête de conformité : extrémités, axes portés par la machine (S27), objet,
  dossier, synthèse et issue, bornes, visibilité et explication réduite — REQ-010-13,
  14, 15, 25, 31, 38 — verify : cas d'acceptation de REQ-010-13, 14, 25, 31 ;
  extrémité invisible → 404
  - où : port de lecture de la 009 (REQ-009-22, 25), `crates/kartenn-service/src/rights.rs`
  - dépend de : 2, 4, 12
  — `kartenn-policy/src/verdict.rs` (feature `pg`) : `query` (extrémités `EndpointRef`
  développées par `Reader::object` et le port de lecture de la 009, bornes
  `MAX_ADDRESSES`/`MAX_EVALUATIONS` → `QueryError::TooLarge`, valeurs par
  `Reader::resolved_many` en une lecture et `machine_values_of_addresses` pour les axes
  portés par la machine, instantané par `SnapshotCache`, `evaluate` et `synthesis`),
  `Access` (`policy.verdict.read` + lecture des extrémités → complète ; sinon
  `policy.flow.request` sur l'entité → réduite ; sinon `QueryError::NotFound`), adresses
  illisibles d'une machine omises et comptées (`hidden`) ; permissions
  `policy.verdict.read` (globale) et `policy.flow.request` au catalogue du module,
  `docs/spec/permissions.md` régénéré ; dépendance `kartenn-machines` (feature `pg`).
  Tests : `tests/it/verdict.rs` (`acceptance_of_req_010_13`, `_14`, `_25` avec extrémité
  invisible, inexistante ou d'un autre genre → 404, `_31` avec la VIP,
  `a_machine_of_too_many_addresses_is_refused`) ; unitaires de largeur et de valeurs.
  choix : une explication réduite ne nomme aucun objet du référentiel (ni origine, ni
  chaîne, ni machine, ni adresse : les points sont repérés par leur rang) ; les valeurs se
  lisent sans filtre de droits (le verdict d'un demandeur porte sur des objets qu'il ne
  lit pas), le filtre ne vaut que pour l'explication ; un service inconnu ou un type sans
  service donne `unknown_service` comme un port non couvert ; l'entité de la demande vient
  du champ `entity` de la requête (le dossier), sans repli sur l'entité de rattachement de
  l'appelant ; les routes `QUERY`/`GET /v1/verdicts` et le problème
  `policy-query-too-large` restent à la tâche 18.
- [x] 14. Cohérence des machines et anomalies d'inférence ; tâche de fond
  `policy.inference.check` (point différé N14, genres de module) — REQ-010-17, 48 —
  verify : anomalies ouvertes et fermées ; regroupement par nœud d'origine
  - où : règles de module (clause 7), `crates/kartenn-service/src/import.rs` (tâche sous
    bail)
  - dépend de : 6, 8, 15 (lecture des règles d'inférence et chaîne des ancêtres)
  — **partielle (2026-10-21)** : REQ-010-17 faite — `crates/kartenn-policy/src/coherence.rs`
  (machines touchées : écrites, interfaces ou adresses écrites, `policy.environment`,
  `policy.declared_environment` ou `machines.interface_role` changés au différentiel, rôle
  exempté écrit ; lecture des valeurs dans la transaction ; aucune lecture quand rien ne
  touche une machine), règle `policy.machine.environment_mismatch`, appelée par
  `Handler::check` ; tests `tests/it/coherence.rs` (acceptation de REQ-010-17, machine non
  déclarée, changeset sans rapport, liste des rôles exemptés administrée) et unitaires.
  **Reste REQ-010-48** (`policy.inference.mismatch` par nœud d'origine, tâche de fond
  `policy.inference.check`) : il lit les règles d'inférence en base et la chaîne des
  ancêtres d'un nœud, que la 15 construit (source de valeurs attendues), et les opérations
  `policy.inference_rule.*` ne sont écrites par aucune tâche faite ; à reprendre après 15.
  — REQ-010-48 faite (2026-10-21), sur l'amendement 4 de l'ADR 0026 (une tâche de fond rend
  des `Findings`, journalisés dans un changeset système) : `crates/kartenn-policy/src/mismatch.rs`
  (tâche `policy.inference_check`, `run`/`run_with`), migration du module
  `0002_inference_check.sql` (progression d'une passe : révision vérifiée, curseur
  d'origine et d'héritier, décompte), règle `policy.inference.mismatch` (avertissement sur le
  nœud d'origine, avec le nombre d'héritiers en écart), `Handler::run_job_findings` ;
  `kartenn-service/src/module.rs` (`ModuleHandler::run_job_findings`, `run_job` qui
  journalise les constats) ; tests `tests/it/mismatch.rs` (acceptation de REQ-010-48 :
  aucune règle, rien ; règle → anomalie sur l'origine comptant 3 héritiers, une seule ;
  seconde exécution sans effet ; règle corrigée → anomalie fermée ; passe en lots d'un
  héritier sur plusieurs exécutions).
  choix : la tâche s'appelle `policy.inference_check` (un nom de tâche n'a pas de point) ;
  une passe démarre quand la révision `inference` dépasse la dernière vérifiée, et couvre
  les domaines cités par les règles écrites ou closes depuis la passe précédente (tous si
  l'une n'en cite aucun) ; seuls les héritiers à attendu unique comptent (égalité ou aucun
  attendu : pas d'écart, REQ-010-45) ; 1 000 héritiers par exécution, période de 60 s ;
  seules les adresses classées comptent (une adresse sans environnement relève de
  l'obligation, REQ-010-21) ; l'anomalie porte sur la machine seule ; liste des rôles
  exemptés par défaut (`administration`, `backup`, `monitoring`) tant qu'aucun rôle n'a
  jamais été administré, et le premier rôle administré la remplace.
- [x] 15. Source de valeurs attendues de l'inférence (contrat 003 amendé, N20, N28) :
  suggestion, valeur par défaut, versionnement et provenance — REQ-010-40, 41, 46, 50,
  51, 52 — verify : cas d'acceptation de REQ-010-45 (création sans défaut) et 52 ;
  provenance lue dans l'explication
  - où : `ExpectedValueSource` (`kartenn-core`), amendement de la 003
  - dépend de : 6, 7
  — `kartenn-policy/src/infer.rs` : `criterion` (forme stockée des critères, un membre par
  critère), `rules_at` (règles valides à une date dans la transaction, arbres
  d'exceptions, domaine cité ajouté comme critère), `chain` (domaine, préfixes qui
  contiennent le nœud, nœud ; sites et entités résolus avec leurs ancêtres, valeurs
  résolues des propriétés que les critères testent), `InferenceSource` (source
  `policy.inference`, révision = compteur `inference` de `policy_revision`, détail = règle
  ou dernière exception retenue), `plan_rule` (`policy.inference_rule.create | update`),
  `check` (axe désigné pour l'inférence, parent du même axe, valeurs vivantes, critères,
  profondeur) ; type `policy.inference_rule` (collection `inference-rules`, ancré dans le
  domaine cité, écrit sous `policy.inference.admin`) ; révision `inference` levée une fois
  par changeset ; déclaration de `policy.environment` avec la source en mode `default` ;
  serveur : `modules::sources()`. Noyau (`kartenn-service/src/expected.rs`) :
  `ExpectedValueSource::defaults_when_inheriting` (méthode ajoutée, vraie par défaut :
  comportement de REQ-003-22 inchangé), et un module peut déclarer une source de son
  propre espace de noms avant que les sources du déploiement ne soient assemblées. Tests :
  `tests/it/inference.rs` (`a_single_expectation_is_written_as_a_default_with_its_provenance`,
  REQ-010-52 et 50, provenance lue dans l'explication ;
  `a_tie_gives_every_candidate_and_no_default`, cas d'acceptation de REQ-010-45 ;
  `a_rule_of_inference_is_checked_on_the_final_state`).
  choix : critères stockés en `jsonb` (`[{"within": […]}, {"property": {"key", "in"}}…]`) ;
  domaine d'une règle fixé à la création, comme sa clé, son axe et son parent ; le nom
  d'un nœud n'est porté que par les domaines (slug) : le noyau ne nomme ni préfixe ni
  adresse ; un mode par déclaration (contrat 003) : `default`, qui suggère aussi ; le
  contrôle d'écart (mode `check`, anomalies) revient à 010-14 et demandera de combiner
  les modes ; une source qui ne donne pas de défaut sous une valeur héritée matérialise
  d'abord les origines des objets créés (`refresh_origins`) ; droits par domaine : la
  portée du domaine cité, sans test dédié ici (environnement de test sans restriction).
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

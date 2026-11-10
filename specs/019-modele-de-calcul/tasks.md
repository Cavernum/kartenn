---
status: approved
updated: 2026-11-09
---

# 019 — Modèle de calcul et Kubernetes : tâches

Exigences : [`requirements.md`](requirements.md). Conception : [`design.md`](design.md).
Chaque tâche = un commit, laisse le build vert (`devenv test`, `k-check`) et nomme ce
qu'elle implémente. « verify » = test qui prouve la tâche ; « dépend de » = tâches à
finir avant. Tests en `tests/it/` (ADR 0027) avec `#[kartenn_testing::pg_test]`
(ADR 0028).

**Préalable bloquant** : Q-019-1 à 13 arbitrées par l'utilisateur le 2026-11-09, toutes
selon la recommandation (Q-019-4 avec garde-fous, [`decisions.md`](decisions.md)) ; spec
remise en accord le 2026-11-09 (tâche 0a) ; reste la validation de la spec par
l'utilisateur (tâche 0b), avant toute implémentation.

Parallélisme : après 0, la file A (module, 1 à 8) et la file B (connecteur, 9 à 11)
avancent en parallèle sur des crates disjointes ; 12 et 13 (politique, flux) attendent 7 ;
14 à 17 viennent en dernier.

## P0 — Arbitrage et décisions

- [x] 0. Arbitrage et ADR — Q-019-1 à 13 — verify : `decisions.md` porte les décisions ;
  D1, D8, D11, D22 et D31 amendées selon les réponses ; ADR « observations hors
  référentiel » et « connecteur Kubernetes » rédigées (statut `proposed`) ; mentions
  « v0.3 » de la 010 et de la 011 corrigées (Q-019-13)
  - où : `specs/001-cadrage/decisions.md` (D1, D8, D11, D22, amendement de D31 du
    2026-10-22) ; `docs/adr/` (numéros suivants, modèle `0031-evaluations-de-flux-materialisees.md`) ;
    `specs/010-politique-flux/requirements.md` et `specs/011-registre-flux/requirements.md`,
    § Non-objectifs
  - dépend de : —
  - fait (2026-11-09) : Q-019-1 à 13 portées dans `decisions.md` ; le reste de la tâche
    (amendements, ADR, mentions « v0.3 ») passe à la 0a, qui l'achève.
- [x] 0a. Remise en accord de la spec, rebasée sans réécriture sur une main qui a avancé
  depuis le 2026-10-22 (audit du 2026-11-09, citations de lignes approximatives, à
  recouper par `grep`) — verify : relecture par la session principale ; aucun
  « en attente », « si Q-019-n = A » ni « proposée » dans la 019 ; `grep -rn "v0\.3"
  specs/010-politique-flux specs/011-registre-flux docs/spec` ne rend que des mentions du
  modèle de calcul
  - écarts au dépôt actuel :
    - permissions : `kube.<type>.read|edit|apply|approval.self` et non `read|write|propose`
      (024/M7, ADR 0026 amendement 5, `docs/spec/permissions.md`), REQ-019-39, design
      (vers les lignes 169 et 304), tâche 5 ;
    - paramètres d'un module sous `kube.settings.*` (027/P4, REQ-027-5) pour les
      correspondances label → axe, les valeurs admises et les labels suivis (arbitré
      par l'utilisateur le 2026-11-09 : paramètres génériques, pas d'action propre) ; les axes eux-mêmes relèvent de
      `policy.settings.*` ; la fenêtre de lot et les clés suivies d'un cluster sont des
      propriétés du `kube.cluster`, en changeset (X4, REQ vers la ligne 106) ;
    - compte de service du connecteur : membre d'un groupe kartenn, rôle `kube.connector`
      lié au groupe sur un périmètre, sans liaison nominale (025/G1 à G3) ; REQ-019-30,
      design B7, X10, et la création du groupe et de la liaison dans la tâche 11 ;
    - plus de module `vlan` (026/C1, C2) : renvoyer à ADR 0026 et à la 009 amendée
      (design vers la ligne 39, requirements vers la ligne 59) ;
    - statut des requirements et du design, et synthèse X1 à X15 de `decisions.md` :
      retirer « proposée », « sous réserve » et « si Q-019-n = A », et noter « Décidé
      par : l'utilisateur (2026-11-09) » aux X concernés ;
    - route d'un cluster : `{id}` partout (006), ou un filtre `?slug=` (design vers les
      lignes 247 et 303) ;
    - rôles livrés (K12, ADR 0026 clause 8) : `kube.*.read` et la lecture des
      observations ajoutés au lecteur, à l'éditeur et à l'approbateur, ou exclus, dit ;
    - événements (022, ADR 0026 clause 10) : les types `kube.*` filtrables dans les
      événements `changes` ; une observation n'en produit aucun (REQ-019-40, design de
      l'interface).
  - garde-fous de Q-019-4 à porter dans X7, REQ-019-16 (une acceptation chacun) et le
    design (Données, Routes) : droit propre au nom admis par `catalogue_problems` et hors
    de `RENAMED_PERMISSIONS` (par exemple `kube.observation.read|record`, l'action `write`
    n'existant plus) ; IP hors des plages du cluster refusée ; ajout seul (une mise à jour
    hors fermeture de période refusée aux droits de base) ; nouvel événement d'audit par
    lot avec l'empreinte du contenu (`docs/spec/evenements-audit.md`,
    `evenements-audit.schema.json`, tâche 17), un lot altéré détecté ; lecture bornée au
    périmètre du cluster ; jamais d'écriture dans le référentiel ; limite de rétention
    assumée ; partitions par temps et index GiST (IP, période).
  - amendements : D1 (miroir écrit en changeset par le seul compte du connecteur de son
    cluster, découverte toujours exclue ailleurs) ; D8 et D11 (observations hors
    bitemporel et hors changesets, écrites par lots sous un droit propre, auditées par
    lot, jamais dans le référentiel) ; D22 (extrémité logique workload, namespace,
    service, résolue en adresses ou en valeurs d'axes ; préciser `hostNetwork`) ; D31
    (Q-019-13 : flux constatés, ticketing et campagnes de revue en v0.4) ; « v0.3 » →
    « v0.4 » dans la 010 (`requirements.md` vers les lignes 637 à 645), la 011
    (`requirements.md` lignes 19, 31, 348, 349, 353 ; `decisions.md` lignes 41, 58, 251,
    254), `docs/spec/data.md` (vers la ligne 335), `docs/spec/regles-modules.md` (vers la
    ligne 69, `flow.kind.reserved`) et la feuille de route de `docs/spec/README.md`
    (vers la ligne 60) ;
  - ADR à rédiger, statut `proposed` : 0043 « Observations : hors référentiel, hors
    changesets », avec les garde-fous ; 0044 « Connecteur Kubernetes » (Q-019-6, X8, X10)
  - où : `specs/019-modele-de-calcul/{requirements,design,decisions,tasks}.md`,
    `specs/001-cadrage/decisions.md`, `specs/010-politique-flux/requirements.md`,
    `specs/011-registre-flux/{requirements,decisions}.md`, `docs/spec/{README,data,regles-modules,evenements-audit}.md`,
    `docs/adr/` (modèle `0031-evaluations-de-flux-materialisees.md`)
  - dépend de : 0
  - fait (2026-11-09) : requirements, design, decisions (X1 à X16, nouvelle X16
    permissions, rôles livrés, événements) et tâches remis en accord ; D1, D8, D11, D22
    et D31 amendées avec la synthèse ; ADR 0043 et 0044 rédigées (`proposed`) ;
    « v0.3 » → « v0.4 » dans la 002, la 010, la 011, la 017 et `docs/spec/` (avec la
    chaîne de la règle `flow.kind.reserved` dans `kartenn-flow`, d'où le catalogue
    généré `regles-modules.md`).
  - choix : permission d'écriture du journal `kube.observation.record` ; rôle livré
    `kube_connector` (une clé de rôle n'admet pas de point), déclaré par le module
    (ajout au contrat, ADR 0044) ; confinement par ancrage de domaine de tous les types
    sauf le cluster et liaison sur les domaines désignés, plus la règle
    `kube.connector.direct` ; le cluster désigne un groupe (`connector_group_id`), pas
    une identité ; lot d'observations refusé en entier (`kube.observation.invalid`) ;
    événements d'audit `observation.batch.recorded` et `observation.batch.altered`,
    empreinte SHA-256 sur JSON canonique (RFC 8785) ; partitionnement du journal laissé
    à la tâche 6 sous contraintes (observation ouverte jamais purgée, rétention comptée
    depuis la fin) ; `kube.observation.read` hors des rôles livrés ; paramètres lus
    sous `kube.cluster.read` ; liste de labels suivis du module en paramètre
    (`kube.set_tracked_labels`) ; feature `kube` active par défaut ; fenêtre de lot
    bornée à 1-300 s et 1-10 000 opérations.
  - reste : les mentions « v0.3 » de `specs/010-politique-flux/besoin/` (expression du
    besoin, journal d'entretien, écarts) sont laissées telles quelles, comme trace datée
    du recueil ; X10 (confinement), X16 (lecture des observations) et X14 (chiffres)
    sont relus à la 0b.
- [x] 0b. Validation de la spec par l'utilisateur, puis fusion de la branche dans main —
  verify : statut `validée` dans les quatre fichiers et dans `specs/README.md`
  - dépend de : 0a
  - fait le 2026-11-09 : l'utilisateur valide les quatre points laissés par la 0a, tous
    sur la recommandation : confinement du connecteur (X10, ADR 0044), lecture des
    observations hors des rôles livrés (X16), rôle livré par un module et audit des lots
    par le noyau (contrat), budgets de X14. Les ADR 0043 et 0044 restent `proposed`
    jusqu'à la tâche 17 (design, § ADR). Les mentions « v0.3 » de
    `specs/010-politique-flux/besoin/` restent : elles datent l'entretien de besoin.

## File A — Module `kube` (P1 domaine pur, P2 module)

- [x] 1. Crate `kartenn-kube`, domaine pur : noms (étiquette et sous-domaine DNS), clés et
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
  - fait (2026-11-09) : `crates/kartenn-kube` (dépendances `kartenn-core`, `uuid`) —
    `names.rs` (`DnsLabel`, `DnsSubdomain`, `LabelKey`, `LabelValue`, `parse_labels`),
    `selector.rs` (`Requirement::parse` des `matchExpressions`, `Selector::of_labels`,
    `matches`, `matches_read`), `owner.rs` (`WorkloadKind`, `workload_of` sur une
    fonction de recherche du propriétaire), `tracked.rs` (`TrackedKeys`, projections
    `Observed*` → `Tracked*` du workload, du namespace, du nœud, du service et de
    l'ingress, `ObservedService::selects`), `rules.rs` (`KUBE_RULES`, les 18 règles du
    design) ; 19 unitaires dont ceux du verify, plus
    `operator_owned_statefulset_is_the_workload` et `mirror_pod_is_its_own_workload`
    (revue : remontée arrêtée aux quatre sortes racines, pod miroir, précision de X3) ; réf. `kubernetes-calcul.md` complétée
    (syntaxe citée des pages Names et Labels, relues le 2026-11-09, et choix de kartenn) ;
    hakari : aucun changement.
  - choix : la syntaxe contrôlée n'ajoute rien à la documentation (un miroir ne refuse
    pas ce que le cluster a admis) : étiquette à premier caractère chiffre admise (RFC 1123
    §2.1), sous-domaine sans borne par étiquette, pas de passage en minuscules (refus),
    préfixes réservés admis, valeurs d'`Exists`/`DoesNotExist` ignorées ; noms d'étiquette
    pour namespace et service, de sous-domaine pour nœud, workload, ingress et compte de
    service ; sorte reconnue par groupe d'API et `kind` ; un contrôleur inconnu (ressource
    propre) au-dessus d'un ReplicaSet, d'un Job ou d'un pod, un propriétaire absent ou d'un autre UID, une chaîne de plus de 4 maillons
    sont des erreurs (`OwnerChainError`) que le connecteur tranche (tâche 10), jamais un
    workload deviné ; sélecteur vide = tout, un service sans sélecteur ne sélectionne
    rien ; sélection calculée sur tous les labels du gabarit, pas les seuls suivis ;
    ports, adresses, plages et routes en ensembles (un réordonnancement ne fait pas de
    version) ; l'UID est versionné (X3) ; zone du nœud lue de
    `topology.kubernetes.io/zone` ; noms de ports, hôtes, classe et nom externe non
    contrôlés.
  - reste : RFC 1123 §2.1 absente de la table des sources de `kubernetes-calcul.md` ;
    citation « start with an alphabetic character » de « RFC 1123 Label Names » à
    vérifier sur la page ; `ObservedService::selects` relit le sélecteur à chaque couple
    service-workload ; `IngressBackend` sans la forme `resource` ; « label name » ambigu
    dans l'en-tête de `names.rs` (nom d'étiquette DNS ou segment de clé de label).
- [x] 2. Ajouts à la 009 : `machines.platform`, filtre `hypervisor`, lectures
  `machines_of_addresses` et `machine_by_name` — REQ-019-1, 2, 23 (lecture) — verify :
  `hypervisor_follows_hosting`, `platform_is_a_closed_list`,
  `machine_of_internal_ip`, tests de la 009 inchangés
  - où : `crates/kartenn-machines/src/module.rs` (déclarations), `read.rs`
    (`attachments_of_address` à imiter), `rest.rs` (filtres de `/v1/machines`) ;
    `crates/kartenn-machines/tests/it/`
  - dépend de : 0
  - fait (2026-11-09) : `module.rs` (`PLATFORM`, déclaration `TypeSpec::ENUM` portée par
    `machines.machine`, non héritée, livrée vide, libellés fr/en) ; `read.rs`
    (`machines_of_addresses` : pour chaque valeur d'un lot d'`IpAddr`, les détenteurs
    `HolderOf` machine, interface, adresse, domaine, dans tout domaine, une requête par
    l'index `address_host_suggested` ; `machine_by_name` : machine d'un nom, sans casse,
    sous un propriétaire, datée) ; `rest.rs` (filtre booléen `hypervisor` de
    `GET`/`QUERY /v1/machines`, SQL écrit dans le texte comme `host:exists`, helper
    `optional_bool` partagé) ; contrat `openapi-v1.json` et filtres des deux SDK
    (`web/sdk`, `web/v2/sdk`) régénérés ; tests `platform_is_a_closed_list`,
    `machine_of_internal_ip` (`tests/it/machines.rs`), `hypervisor_follows_hosting`
    (`tests/it/rest.rs`), ceux de la 009 inchangés.
  - choix : hyperviseur = machine `physical` membre d'un cluster ou hôte d'une machine
    `virtual` (un conteneur n'en fait pas un) ; l'invité ne compte que si l'appelant le
    lit (ADR 0021, comme les adresses d'un réseau, 021-18e), l'appartenance au cluster
    est une colonne de la machine ; `hypervisor=false` est le complément ; filtre booléen
    d'égalité (`{"field": "hypervisor", "value": true}` en `QUERY`), pas un
    `:exists` ; `machines_of_addresses` prend des valeurs (l'`InternalIP` lue) et non des
    identifiants d'adresse (ce que `machine_values_of_addresses` couvre déjà), sans
    filtre de domaine ni d'espèce : l'ambiguïté (plusieurs détenteurs ou domaines) et
    l'espèce `physical|virtual` (`kube.node.machine_kind`) sont à l'appelant ;
    `machine_by_name` rend une machine au plus (nom unique par propriétaire, M3), lue
    sur l'affectation propre de `core.owner` (la machine est racine de son arbre),
    datée par instantanés comme le reste du port ; libellé « Plateforme » / « Platform ».
  - reste (revue) : la doc de `machines_of_addresses` (`read.rs` vers la ligne 260) ne dit
    pas qu'elle lit tous les domaines sans droits, réservée à un appelant interne (le
    connecteur de la tâche 9 ne doit pas la brancher derrière une route) ;
    `name.to_lowercase()` côté Rust et `lower()` côté SQL peuvent diverger hors ASCII
    (sans effet pour un nom RFC 1123) ; sous `as_of` passé, l'index
    `address_host_suggested` ne sert pas `address_history` (à mesurer par un `EXPLAIN`) ;
    `hypervisor=true` évalue l'`EXISTS` pour chaque machine `physical` de la page, plan non
    mesuré à 10⁵ machines ; le cas `hypervisor=false` pour le lecteur restreint (l'hôte
    dont l'invité est illisible parmi les non-hyperviseurs) n'est pas testé.
- [ ] 3. Clusters, nœuds, namespaces : migration `0001_kube.sql` (`cluster`, `node`,
  `namespace`), gestionnaire `create/update_cluster`, opération `kube.sync` réduite à ces
  types, module au registre (dépendance `machines`), feature `kube` — REQ-019-4 à 7, 13 —
  verify : `kube_needs_machines`, `cluster_slug_unique`, `range_in_designated_domain`,
  `node_name_unique_per_cluster`, `node_machine_kind`, `node_range_created_and_closed`
  - où : design « Types d'objets », « Données », « Opérations » ; modèle
    `crates/kartenn-machines/src/handler.rs` (`plan`, affectations planifiées,
    clés de verrou) et `migrations/0001_machines.sql` ; `crates/kartenn-server/src/modules.rs`
  - dépend de : 1
  - découpée le 2026-11-10 (relevé par l'agent de la tâche, avant tout code) en 3a, 3b
    et 3c. Un module ne connaît pas les identifiants des objets du noyau qu'il planifie :
    `plan` (`kartenn-service/src/pipeline.rs` vers les lignes 955 à 1020) les tire
    (`Uuid::now_v7()`) après le plan du module et les applique dans l'ordre planifié (vers
    la ligne 1715) ; `ModuleHandler` n'a aucun crochet qui les lui rende. Le nœud ne peut
    donc pas citer dans `range_ids` les réseaux de ses plages de pods (REQ-019-13, B2),
    pas plus que la tâche 4 ses adresses `vip`. L'avertissement `kube.node.range_outside`
    n'a rien dans l'état final dont partir : le schéma ne garde pas les plages observées.
- [x] 3a. Identifiants créés rendus au plan d'un module — **arbitrée le 2026-11-10 :
  option A** (Q-019-14 ; crochet `planned`, amendement additif de la clause 6 de l'ADR
  0026, colonne `kube.node.pod_cidrs` posée par la 3c). Options relevées : (A, recommandée par
  l'agent) après que le pipeline a tiré les identifiants des opérations du noyau d'un
  module, il les lui rend (crochet par défaut, par exemple
  `ModuleHandler::planned(&mut ModulePlan, created: &[Uuid])`) pour que ses données les
  citent ; amendement additif de la clause 6 de l'ADR 0026 ; colonne des plages observées
  sur le nœud (par exemple `pod_cidrs cidr[]`), écart au design « Données », pour que
  `check` lève et close `range_outside`. (B) le connecteur envoie des `create_prefix`
  avec des locaux avant `kube.sync` dans le même lot, et le nœud cite `{"local": n}`
  (déjà résolu) : « crée et clôt les réseaux » passe du module au connecteur, contre la
  table « Opérations » ; `range_outside` demande encore une colonne, ou se vérifie au
  connecteur et la règle serveur se perd. (C) opération de module mise en file après les
  `CreatePrefix`, ou `range_ids` réécrits dans `check` : expédients (collisions de
  locaux, droits vérifiés sur un nœud qui n'existe pas encore), écartés — verify (si A) :
  test unitaire où un faux module planifie un `CreatePrefix` et ses données en citent
  l'identifiant
  - où : `crates/kartenn-service/src/module.rs` (`ModuleHandler`),
    `crates/kartenn-service/src/pipeline.rs` (`plan`), `docs/adr/0026-*.md` clause 6
  - dépend de : rien (arbitrée)
  - fait (2026-11-10) : `kartenn-service` — `module.rs` (`ModuleHandler::planned(&mut
    ModulePlan, created: &[Uuid])`, défaut sans effet) ; `pipeline.rs` (`plan` : la file
    des opérations du noyau ajoutées porte le plan de module qui les ajoute, les
    identifiants tirés lui sont rendus une fois la file vidée ; test
    `pipeline::tests::module_data_cite_the_core_objects_it_creates`, un faux module
    `shelf` dont le plan ajoute deux `CreatePrefix` et cite leurs identifiants dans ses
    `data`) ; ADR 0026 amendement 8 ; `docs/spec/architecture.md` ; B9 du design fourni.
    La validation d'un changeset `pending` replanifie la requête enregistrée par le même
    `plan` (`review.rs` → `run` → `stages`) : le crochet y tient sans autre code, avec
    des identifiants tirés de nouveau.
  - choix : crochet infaillible (pas de `Result`) : citer un identifiant ne se refuse
    pas, la validation reste à `plan` et `check` ; `created` aligné sur `plan.core`
    (`created[i]` pour `core[i]`, un identifiant sans emploi pour une opération qui ne
    crée rien) plutôt que réduit aux seules créations, pour que le module indexe sans
    compter ; crochet appelé seulement pour un plan qui ajoute des opérations du noyau ;
    un plan de module ajouté par un autre reçoit les siens de même (file en largeur, les
    ajouts d'un plan restent contigus).
  - reste : le plan rendu par `ModuleHandler::nullify` (cascade, `cascade.rs`) garde son
    `core` ignoré, comme avant, donc pas de crochet ; la colonne `kube.node.pod_cidrs` et la
    création des réseaux des plages restent à la 3c.
  - reste (revue) : la doc de `ModuleHandler::planned` (`module.rs` vers la ligne 467)
    dit « on every planning » alors que le crochet n'est appelé que pour un `core` non
    vide ; le test ne couvre ni une opération du `core` qui ne crée rien ni un plan de
    module ajouté par un autre ; l'amendement 8 de l'ADR 0026 ne dit pas que le
    différentiel montré à la proposition cite d'autres identifiants que l'écriture ; une
    ligne de `docs/spec/architecture.md` (vers la ligne 140) dépasse la largeur de
    repli.
- [x] 3b. Clusters, nœuds et namespaces sans plages : migration `0001_kube.sql`
  (`cluster`, `node`, `namespace`, comme le design « Données »), `create/update_cluster`
  (fenêtre de lot 1–300 s et 1–10 000 opérations, règle `kube.cluster.connector_group`
  vérifiée contre `rbac_group`, migration 0066), `kube.sync` qui crée, met à jour et clôt
  nœuds et namespaces sans leurs plages, module au registre (dépendance `machines`),
  feature `kube` active par défaut — REQ-019-4 à 7 — verify : `kube_needs_machines`,
  `cluster_slug_unique`, `range_in_designated_domain`, `node_name_unique_per_cluster`,
  `node_machine_kind`
  - où : comme la tâche 3
  - dépend de : 1
  - fait (2026-11-10) : `crates/kartenn-kube` — `migrations/0001_kube.sql` (`cluster`,
    `node`, `namespace` comme le design, `range_ids` gardée, vide jusqu'à la 3c ;
    index `node_by_machine`, `node_by_uid`, `namespace_by_uid`, GIN `namespace_labels`) ;
    `module.rs` (`Kube`, dépendance `machines`, types `kube-clusters`, `kube-nodes`,
    `kube-namespaces`, opérations `create_cluster`, `update_cluster`, `sync`,
    références du design sauf réseaux de plage et cibles de la tâche 4, permissions
    `kube.{cluster,node,namespace}.{read,edit,apply,approval.self}`), `labels.rs`,
    `handler.rs` (`prepare` : propriétés `core.owner`/`core.site`, objet vivant de
    l'identité (cluster, nom) et domaine des pods ; `plan` ; `apply` ; `close`,
    `restore`, `referrers`, `nullify` du nœud vers sa machine ; `check` :
    `kube.cluster.range_domain`, domaines désignés vivants, `kube.cluster.connector_group`
    sur `rbac_group`, `kube.node.machine_kind`), `pg.rs` ; `kartenn-server` (feature
    `kube` par défaut, module au registre) ; contrat `openapi-v1.json`, types des deux
    SDK, `docs/spec/permissions.md` et `regles-modules.md` régénérés ; `kube_needs_machines`
    (unitaire de `module.rs`), `cluster_slug_unique`, `range_in_designated_domain`,
    `node_name_unique_per_cluster`, `node_machine_kind` (`tests/it/clusters.rs`).
  - choix : règle `kube.operation.invalid` ajoutée (corps mal formé, bornes de rétention
    et de fenêtre, objet inconnu, domaine désigné mort ; ligne ajoutée à la table
    « Règles » du design), `KUBE_RULES` à 19 ; slug de cluster en double :
    `core.slug.duplicate` (REQ-002-47, unique par type), pas de règle `kube.cluster.*`
    nouvelle ; un `kube.sync` par objet (`action` `upsert` ou `close`, `type`
    `kube.node`|`kube.namespace`, `cluster`, `name`, champs suivis), l'objet retrouvé par
    (cluster, nom) vivant au début de la période, dans `prepare` (pas d'identifiant
    externe du noyau) ; `upsert` d'un objet existant = fusion des champs donnés ; `close`
    = `Close` du noyau planifié, sans cascade ; adresses d'un nœud en ensemble trié,
    zone en valeur de label, labels du namespace contrôlés en syntaxe mais pas filtrés
    par les clés suivies (projection du connecteur, X4) ; machine déjà désignée refusée
    avant l'écriture (`kube.node.machine_kind`) plutôt que par l'exclusion ; espèce et
    vie de la machine sur toutes les versions du nœud dans `check` ; droits de lecture
    demandés sur les domaines, conteneurs, réseaux (`core.*.read`) et la machine
    (`machines.read`) cités ; `gen.mjs` du SDK v2 : un nom de parent que deux modules
    déclarent (`cluster`) se lit dans le module du type ; `kartenn-kube/` ajouté aux
    chemins de la mesure de télémétrie (`kartenn-bench/src/affected.rs`).
  - revue (2026-11-10) : changer le domaine des pods d'un cluster qui a des nœuds ou des
    namespaces vivants est refusé (`kube.operation.invalid`, `pg::pod_domain_moved`), test
    `pod_domain_change_refused_with_living_objects` (accepté sans objet vivant).
  - reste : le ré-ancrage des objets d'un cluster dont le domaine des pods change est
    écarté (Q-019-15, 2026-11-10 : le refus reste la règle) ; une
    violation des exclusions `cluster_slug_unique`, `node_name_unique` ou
    `node_machine_unique` qui passerait les contrôles préalables remonte en `StoreError`
    (500), sans traduction en règle comme `constraint_rule` du noyau
    (`kartenn-store-pg/src/write.rs`) ; ni la création d'un cluster ni la désignation
    d'une machine ne prennent de clé de verrou ; `kube.sync` ne demande pas
    `kube.cluster.read` sur le cluster cité (à confronter à X10) ; l'index
    `node_by_machine` double le GiST de `node_machine_unique` ; un cluster cité en
    `local` dans `kube.sync` reçoit le message « not a living cluster » ; un groupe du connecteur clos
    après le cluster n'est pas refusé (pas de référence cluster → groupe au design) ;
    l'espèce d'une machine changée par la 009 après sa désignation n'est pas revérifiée ;
    ni `mark_absent` ni `clear_absent` (tâche 5) ; identifiant externe
    `kube:<cluster>/…` non lié (tâche 8 ou 10).
- [x] 3c. Réseaux des plages de pods d'un nœud créés et clos par `kube.sync`,
  avertissement `kube.node.range_outside` (migration 0002, colonne `pod_cidrs`,
  Q-019-14) — REQ-019-13 — verify : `node_range_created_and_closed`, un
  test de `range_outside`
  - où : comme la tâche 3 ; le crochet `planned` de la 3a
  - dépend de : 3a, 3b
  - fait (2026-11-10) : `crates/kartenn-kube` — `migrations/0002_node_pod_cidrs.sql`
    (`pod_cidrs cidr[]` par `kartenn_add_versioned_column`, index GIN `node_ranges` sur
    `range_ids`) ; `module.rs` (migration 2, référence `kube.node` → `prefix` en refus) ;
    `handler.rs` (champ `pod_cidrs` de `kube.sync` pour un nœud ; `prepare` lit les
    conteneurs de pods du cluster et les réseaux que cite le nœud ; `pod_ranges` : une
    plage nouvelle strictement dans un conteneur → `CreatePrefix` réseau du domaine des
    pods, une plage disparue → `Close` de son réseau, une plage inchangée garde le sien ;
    `close` d'un nœud clôt aussi ses réseaux ; `planned` ajoute à `range_ids` les
    identifiants des `CreatePrefix` ; `check` lève `kube.node.range_outside` et le
    réévalue pour chaque nœud écrit) ; `pg.rs` (`pod_containers`, `node_ranges`,
    `pod_ranges_outside`, référents `node` → `prefix`) ; tests
    `node_range_created_and_closed` et `node_range_outside_warns`
    (`tests/it/clusters.rs`).
  - choix : champ `pod_cidrs` (comme la colonne et `spec.podCIDRs`), ensemble trié de
    préfixes stricts (bits d'hôte refusés, `kube.operation.invalid`), absent = plages
    inchangées ; `range_ids` trié par identifiant (ensemble, pas de version pour un
    réordonnancement) ; « dans un conteneur » = strictement contenu (`>>`) : une plage
    égale au conteneur n'a pas de réseau et avertit, plutôt que de refuser le lot par
    `core.prefix.duplicate` ; la plage hors conteneur est gardée dans `pod_cidrs` (la
    règle en part) mais sans réseau (REQ-019-13) ; la règle ne lit que les versions du
    nœud vivantes à partir de maintenant (pas d'avertissement à la clôture d'un nœud) ;
    un réseau cité ne se clôt pas seul (`core.object.dependents`, référence en refus),
    le module le clôt avec le nœud ou la plage ; aucune règle ni droit nouveau : les
    `CreatePrefix` et `Close` passent sous `core.prefix.edit` du domaine des pods
    (opérations du noyau, ADR 0026 clause 6) ; index GIN `node_ranges` pour la
    recherche des référents à chaque clôture de préfixe.
  - reste : un nœud clos par la cascade de son cluster (`close` du cluster avec
    `cascade`) laisse ses réseaux vivants, orphelins (le module ne planifie rien dans
    une cascade, `nullify` seul) ; un changement des conteneurs d'un cluster ne
    réévalue pas `range_outside` sur ses nœuds tant qu'ils ne sont pas réécrits, et ne
    crée les réseaux devenus possibles qu'au `kube.sync` suivant du nœud ; les droits
    `core.prefix.edit` sur le domaine des pods ne sont pas testés pour un compte
    restreint (tests sous `Unrestricted`) ; le commentaire de `0001_kube.sql`
    (« `range_ids` stays empty until task 019-3c ») reste, la migration n'étant pas
    réécrite.
- [ ] 4. Workloads, services, ingress : tables, `kube.sync` complet, adresses `vip`
  planifiées, sélection d'un service, références et cascades, arêtes — REQ-019-8 à 11,
  14 — verify : `workload_recreated_keeps_identity`, `service_vip_refuses_close`,
  `selection_lists_both_ways`, `ingress_backend_missing_warns`, `execution_chain_edges`
  - où : design « Données », « Références » ; `kartenn-machines/src/handler.rs`
    (références et cascades de la tâche 009-8) ; ADR 0026 clauses 11, 12
  - dépend de : 3
- [ ] 5. Objets miroirs et absence : règles `kube.object.managed` (auteur compte de
  service membre du groupe du connecteur du cluster), `kube.connector.direct` et
  `kube.cluster.connector_group`, `mark_absent`, `clear_absent`, anomalie
  `kube.object.absent`, section `kube.objects`, permissions
  `kube.<type>.{read,edit,apply,approval.self}` déclarées (grammaire de la 024) —
  REQ-019-5, 12, 22, 30 (règles) — verify : `managed_field_refused_for_human`,
  `managed_field_refused_for_person_in_connector_group`,
  `property_on_mirror_accepted`, `connector_direct_operation_refused`,
  `cited_workload_goes_absent`, `absent_cleared_on_return`,
  `section_lists_changed_objects`, `catalogue_problems` vide pour `kube.*`
  - où : design « Règles », « Section de différentiel » ; `CheckContext::changeset`
    (`crates/kartenn-service/src/module.rs`) ; section `machines.attachments`
    (`crates/kartenn-machines/src/handler.rs`) à imiter
  - dépend de : 4
- [ ] 6. Journal d'observation et ses garde-fous : tables `observation_batch` et
  `pod_observation` partitionnée, droits PostgreSQL d'ajout seul et déclencheur, route
  de lot sous `kube.observation.record` (validation à l'entrée, refus du lot entier),
  inscription d'audit par le noyau (`observation.batch.recorded` avec l'empreinte,
  `observation.batch.altered`, catalogue d'audit régénéré), tâches de fond de
  vérification et de purge, lectures `who_had` et observations d'un workload bornées au
  périmètre — REQ-019-15 à 17 — verify : `who_had_ip_at_time`,
  `retention_answers_empty_with_reason`, `purge_drops_old_partitions`,
  `open_observation_never_purged`, `batch_is_audited_once_with_hash`,
  `batch_outside_pod_ranges_refused_whole`, `batch_from_other_cluster_connector_refused`,
  `app_role_cannot_rewrite_or_delete`, `altered_batch_detected`,
  `observations_read_bounded_to_cluster`, `batch_emits_no_changes_event`
  - où : design « Données » (journal et garde-fous), « Droits, rôles et audit »,
    « Routes » ; ADR 0043 ; ADR 0023 et `crates/kartenn-server/src/jobs.rs` (tâches de
    module, 016/G6) ; catalogue d'audit `kartenn-core` (`audit::catalogue_markdown`,
    `docs/spec/evenements-audit.md`, `evenements-audit.schema.json`) ; ADR 0019
  - dépend de : 3
- [ ] 7. Port de lecture et propriétés : porteurs `kube.*`, axe `kube.namespace`,
  `axis_values` (composition workload → namespace → cluster → arbre), `workloads_of`,
  `execution_chain`, correspondances de labels, valeurs admises et labels suivis du
  module (paramètres `kube.settings.*`, tables et opérations, `operation_permissions`)
  — REQ-019-32 à 34 (côté module), REQ-019-3 — verify :
  `axis_from_namespace_then_tree`, `axis_values_one_query`, `label_mapping_admitted`,
  `label_mapping_needs_kube_settings`, `looks_like_pod_warns`
  - où : design « Propriétés », « Port de lecture » ;
    `crates/kartenn-machines/src/read.rs` (`machine_values_of_addresses`) ;
    `crates/kartenn-core/src/properties.rs` (`InheritTree`, un arbre par déclaration)
  - dépend de : 4
- [ ] 8. Routes REST et imports : collections du module par `{id}` (clé naturelle en
  filtre, `?slug=`), filtres, chaîne d'exécution, paramètres (`If-Match`), permissions
  de lecture ajoutées aux rôles livrés (lecteur, éditeur, approbateur ; `apply` à
  l'approbateur ; pas `kube.observation.read`), types `kube.*` filtrables dans les
  événements `changes`, contrat de référence et types du SDK régénérés — REQ-019-38 à
  40 (API) — verify : tests HTTP des routes, `reader_reads_workloads_not_observations`,
  `changes_filtered_by_kube_type`, `docs/spec/permissions.md` régénéré, contrat de
  référence (ajout seul)
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
  projection, lots idempotents, ordre des lots, observations, dates, conduite devant une
  remontée sans workload (`OwnerChainError::ForeignController` : contrôleur de sorte
  inconnue au-dessus d'un ReplicaSet, d'un Job ou d'un pod ; `Missing` : propriétaire pas encore
  lu ou d'un autre UID ; ignorer le pod, le différer au lot suivant, ou retenir l'objet
  `below`, à trancher ici, X3) — REQ-019-19 à 21, 24, 25, 31 — verify :
  `converges_after_410` (oracle état projeté = état kartenn),
  `replayed_window_no_duplicate`, `image_change_no_changeset`,
  `endpoints_deduplicated_across_slices`, `fallback_full_list`,
  `env_vars_never_leave`, `pod_without_resolved_workload` (un pod sous un contrôleur
  inconnu et un pod dont le ReplicaSet n'est pas encore lu : conduite retenue tenue,
  convergence une fois le propriétaire lu)
  - où : design « Connecteur », étapes 1 à 6 ; client REST tiré du contrat (tâche 8) ;
    `kartenn_kube::owner::workload_of` (tâche 1)
  - dépend de : 9, 8
- [ ] 11. Nœuds et machines, sécurité au démarrage : rapprochement par `InternalIP` puis
  nom, `create_node_machines`, revue de ses droits et refus de démarrer, manifeste de
  ClusterRole livré ; rôle livré par le module `kube_connector` (ajout au contrat de
  module, ADR 0044) ; création du groupe du connecteur, du compte de service membre du
  groupe et de la liaison groupe × `kube_connector` × domaines désignés du cluster
  (procédure documentée et test, aucune liaison nominale) ; contrôle au démarrage que
  le compte est membre du groupe désigné — REQ-019-23, 26 à 28, 30 — verify :
  `node_matched_by_internal_ip`, `ambiguous_node_unmatched`,
  `refuses_with_secret_rights`, `only_read_verbs_sent`, `connector_role_confined`
  (cluster B refusé, même domaine partagé), `refuses_when_not_in_connector_group`,
  manifeste relu par un test (verbes et ressources)
  - où : design « Connecteur » (démarrage), « Droits, rôles et audit » ; X10 ; ADR 0044 ;
    `docs/exploitation/` (nouvelle page) ; rôles livrés de la 005
    (`crates/kartenn-core/src/authz.rs`, `READER`, `APPROVER`, K12) et groupes de la 025
    (`crates/kartenn-service/src/rbac.rs`) à imiter
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
- [ ] 17. Synchronisation : `docs/spec/` (data : schéma `kube` et journal ; interfaces :
  routes, connecteur ; règles des modules, permissions et événements d'audit
  régénérés, `observation.batch.recorded` et `.altered` présents), ADR 0043 et 0044
  acceptées (ADR 0026 amendée : rôle livré par un module, journal d'observations),
  `docs/exploitation/` (déployer le connecteur, groupe et liaison, RBAC, rotation des
  jetons), audit `spec-keeper`
  - où : `docs/spec/data.md`, `interfaces.md`, `regles-modules.md`, `permissions.md`,
    `evenements-audit.md`, `evenements-audit.schema.json` ; `docs/adr/0026-contrat-de-module.md` ;
    tâche 009-14
  - dépend de : 11, 13, 15, 16

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
  - reste (revue) : `.ok()?` (`handler.rs` vers les lignes 419 et 424) omet sans erreur
    une plage tenue ou un conteneur illisible, la plage passe pour nouvelle et donne un
    `CreatePrefix` en double ; la règle compte un conteneur dont la période chevauche
    (`&&`, `pg.rs` vers la ligne 490) quand la création ne le voit qu'à l'instant `at`
    (`@>`, vers la ligne 76) : un conteneur déclaré à une date future ne donne ni réseau
    ni avertissement ; « au plus une plage par famille » (X6) n'est pas contrôlée ; les
    droits `core.prefix.edit` sur le domaine des pods sont à rattacher au rôle
    `kube_connector` (X10, tâche 11) sans quoi le connecteur réel est refusé. Le constat
    important (cascade) est la tâche 3d.
- [x] 3d. Réseaux des nœuds clos avec la cascade de leur cluster (constat important de la
  revue de la 3c) — **arbitrée le 2026-11-10 : option A** (Q-019-16 ; contrat de module,
  ADR 0026) : le design
  « Références » (« nœud | réseau de plage | refus ; cascade : clôt le réseau avec le
  nœud ») et l'acceptation de REQ-019-13 ne sont pas tenus quand le nœud est clos par la
  cascade de son cluster : `cascade.rs` clôt les objets de module dépendants
  (`Planned::Close`) sans demander au module ce qu'ils emportent, et la référence
  `node` → `prefix` ne joue que dans le sens « le préfixe se clôt ». Les réseaux restent
  vivants et orphelins ; un cluster redéclaré sur le même domaine voit son premier lot
  refusé (`core.prefix.duplicate`). Le même besoin revient à la tâche 4 (« service,
  ingress | adresse `vip` | cascade : clôt l'adresse »). Options : (A, recommandée) une
  politique déclarative sur la référence, par exemple `Reference::owned` (ou
  `OnClose::…` du sens inverse) : quand la source se clôt, en cascade ou non, le noyau
  clôt aussi la cible ; amendement additif de l'ADR 0026 (clause des références), sans
  code de module ; (B) un crochet `ModuleHandler::closing(kind, id) -> Option<ModulePlan>`
  appelé par la cascade pour chaque objet de module clos, dont les opérations du noyau
  sont développées ; plus général, mais impératif et à répéter par module ; (C) refuser
  la clôture d'un cluster tant que ses nœuds citent des réseaux (le connecteur vide
  d'abord) : sans changement du noyau, mais contraire au design — verify : test
  d'intégration, `close` du cluster en `cascade` clôt nœuds et réseaux ; redéclaration
  du cluster puis `kube.sync` acceptée
  - où : `crates/kartenn-service/src/cascade.rs` (vers la ligne 142),
    `crates/kartenn-service/src/module.rs` (`Reference`, `OnClose`),
    `crates/kartenn-kube/src/module.rs` (référence `node` → `prefix`),
    `docs/adr/0026-contrat-de-module.md`
  - dépend de : 3c
  - fait (2026-11-10) : `kartenn-service` — `module.rs` (`OnClose::Owned`,
    `ModuleHandler::targets`, par défaut aucune cible) ; `cascade.rs` (`expand` suivi de
    `close_owned` : après chaque `Planned::Close`, cascade ou non, la clôture des cibles
    possédées, récursive, sauf celles déjà closes par le changeset ou citées par un autre
    objet vivant, fonction `cited`) ; module de test `dummy` (référence
    `dummy.thing` → `address` possédée, colonne `address_ref`) et test
    `owned_policy_closes_the_target_with_its_source` (`tests/it/module_pipeline.rs`).
    `kartenn-kube` — `module.rs` (référence `node` → `prefix` en `Owned`) ; `handler.rs`
    (`targets` par `pg::node_ranges` ; `close` d'un nœud par `kube.sync` ne planifie plus
    la clôture de ses réseaux, le noyau s'en charge) ; test
    `cluster_cascade_closes_node_networks` (`tests/it/clusters.rs`). ADR 0026,
    amendement 9.
  - choix : une variante `OnClose::Owned` plutôt qu'un champ `owned: bool` (refus quand
    la cible se clôt seule, et clôture avec la source : les besoins connus, nœud → réseau
    et service → `vip`, sont tous deux « refus » ; aucune des 25 déclarations de
    référence existantes à retoucher) ; une lecture `ModuleHandler::targets`, pendant de
    `referrers`, faute de quoi le noyau ne sait pas ce que cite la source (le module
    déclare, le noyau planifie : pas de crochet impératif, l'option B) ; la cible close
    sans cascade (ses dépendants du noyau la font refuser) ; « citée par un autre objet
    vivant » = un référent de module ou une affectation d'une propriété de référence
    hors du changeset ; clôtures ajoutées listées dans `cascaded` (REQ-004-26), même
    sans `cascade` demandée ; la double clôture évitée en retirant celle du module (une
    seule voie), le noyau sautant de toute façon une cible déjà close par le changeset ;
    `pipeline.rs` non touché.
  - reste : l'annulation (undo) d'un changeset qui a clos un nœud et ses réseaux n'est
    pas testée pour les clôtures ajoutées ; le changement d'une plage par `kube.sync`
    clôt toujours son réseau par le module (la plage quitte le nœud, la source ne se clôt
    pas) ; la tâche 4 doit déclarer `Owned` la référence service → adresse `vip` et
    implémenter `targets` pour elle (le défaut fait échouer le changeset).
  - correction (revue) : le `targets` par défaut rend une erreur pour une référence
    `Owned` (message qui nomme la référence), test
    `an_owned_reference_without_its_targets_fails_the_closure`
    (`tests/it/module_anchors.rs`, module `kit` qui déclare `kit.item` → `address`
    possédée sans `targets`).
  - reste (revue) : la description OpenAPI de `Cascaded`
    (`specs/006-api-rest/openapi-v1.json` vers les lignes 784 et 806) dit « closed by
    cascade » alors que les clôtures `Owned` y figurent sans `cascade` ; `cited` n'a de
    test ni de la branche `referencing` ni d'un référent `Refuse` ou `Nullify` ; `cited`
    ne voit que l'état d'avant écriture : un objet du même changeset qui cite la cible
    n'est pas vu, d'où un refus final peu clair (à vérifier pour un lot `kube.sync` qui
    déplace une plage d'un nœud à l'autre) ; `targets` est lu avant les verrous (lié à
    la 021-22o).
- [x] 4. Workloads, services, ingress : tables, `kube.sync` complet, adresses `vip`
  planifiées, sélection d'un service, références et cascades, arêtes — REQ-019-8 à 11,
  14 ; découpée le 2026-11-10 en 4a à 4d (trois types et les arêtes ne tiennent pas dans
  un contexte), complétée par 4e0, 4e et 4f (revues), cochée avec la dernière.
- [x] 4a. Workloads : table `kube.workload` (design « Données », migration suivante),
  `upsert` et `close` des workloads dans `kube.sync` par (namespace, sorte, nom),
  labels suivis du gabarit de pod (X4), compte de service, ports déclarés,
  `hostNetwork` ; référence workload → namespace (refus ; cascade : clôt) ; un
  workload recréé sous le même nom garde son identité, l'UID change dans son
  historique — REQ-019-8 — verify : `workload_recreated_keeps_identity`, la cascade du
  namespace qui clôt ses workloads
  - où : `crates/kartenn-kube/src/handler.rs`, `pg.rs`, `module.rs`, migrations de
    `crates/kartenn-kube/` ; design « Données », « Références » ; tests voisins
    `crates/kartenn-kube/tests/it/clusters.rs` (nœuds, namespaces de la 3b)
  - dépend de : 3d
  - fait (2026-11-10) : migration `0003_workload.sql` (table versionnée du design,
    exclusion (namespace, sorte, nom), index `workload_by_namespace`, `workload_by_uid`,
    GIN `workload_labels`) ; `module.rs` (type `kube.workload`, collection `workloads`,
    parent namespace, ancré dans le domaine des pods ; droits `kube.workload.read`,
    `edit`, `apply`, `approval.self` ; référence `kube.workload` → `kube.namespace` en
    `Cascade`) ; `labels.rs` (anglais des droits et de la sorte) ; `handler.rs`
    (`kube.sync` de `type` `kube.workload` : identité `namespace`, `kind`, `name`, champs
    `uid`, `labels`, `service_account`, `ports`, `host_network`) ; `pg.rs`
    (`workload_named`, `kube.workload.duplicate` avant la contrainte, `placement` sous le
    namespace, `referrers` du namespace) ; `tracked.rs` (`Protocol::as_str`, `parse`) ;
    tests `workload_recreated_keeps_identity` et `namespace_cascade_closes_workloads`
    (`tests/it/clusters.rs`) ; contrat (`openapi-v1.json`), `docs/spec/permissions.md` et
    types des deux SDK régénérés par leurs outils.
  - choix : le namespace d'un workload est désigné par son nom dans le cluster (comme
    l'identifiant externe `kube:<cluster>/<namespace>/<sorte>/<nom>`, X3), lu au
    `prepare` : il doit vivre avant le lot (namespaces avant workloads, design
    « Connecteur »), sinon `kube.operation.invalid` ; nom du workload en sous-domaine DNS
    (comme `tracked::TrackedWorkload`) ; ports `[{port, protocol, name}]`, protocole en
    minuscules (`tcp`, `udp`, `sctp`, comme les flux ; `tcp` à défaut, comme Kubernetes),
    port 0 refusé, nom de port contrôlé en `IANA_SVC_NAME` (RFC 6335 § 5.1, minuscules,
    `kube.name.invalid`), ensemble trié (un réordonnancement ne fait pas de version) ;
    compte de service en sous-domaine DNS ou `null` ; labels contrôlés en syntaxe
    seulement, comme ceux d'un namespace (le filtrage aux clés suivies est celui du
    connecteur, X4) ; l'UID change par `upsert` sur l'objet vivant de même identité ;
    le `close` d'un namespace par `kube.sync` reste sans cascade : refusé
    (`core.object.dependents`) tant que ses workloads vivent, accepté s'ils sont clos
    avant dans le même lot ; la cascade est celle d'un `Close` explicite ; `taken`
    (`pg.rs`) prend une liste de clés plutôt qu'un couple ; `mirror_row` extrait de
    `plan_sync` (limite de longueur de clippy).
  - reste : la propriété `kube.namespace` d'un workload (design « Propriétés ») n'est pas
    posée (tâche 7) ; ni la collection `/v1/workloads` ni son historique ne sont testés
    par REST (le test lit les versions en SQL ; tâche 8) ; la période d'un workload
    écrit avec une période propre n'est pas confrontée à celle de son namespace ;
    `pod_domain_moved` ne compte pas les workloads (couverts par leurs namespaces
    vivants) ; rien ne vérifie que les labels écrits sont des clés suivies.
  - correction (revue) : un workload clos (disparu sans dépendant, X9) puis recréé sous
    la même identité rouvre le même identifiant (X3, REQ-019-8) : `prepare` lit la
    dernière identité close du namespace vivant (`pg::workload_closed`, aucune version
    vivante à partir de l'instant) et l'`upsert` crée sous cet identifiant une version
    nouvelle après le trou (`creates`, droits vérifiés après l'écriture comme pour une
    création), le noyau rouvrant son enregistrement par `register` ; pas de `Restore` du
    noyau, ses opérations planifiées passant après l'écriture du module et rendant l'état
    d'avant la clôture plutôt que celui de la recréation ; les champs non donnés
    reprennent leur défaut ; deux réouvertures dans un lot : `kube.workload.duplicate`.
    Test `workload_recreated_keeps_identity` : clôture puis recréation, UID
    `[first, second, third]`, parent et propriétaire hérité.
  - reste (revue) : la même règle pour les nœuds et namespaces (tâche 5 ou 10) ; les
    affectations de propriétés d'un workload clos ne reviennent pas à sa réouverture ; le
    refus du `close` d'un workload inconnu donne l'UUID du namespace, non son nom ;
    `mirror_row` masque son paramètre `workload` et `kind` y double `workload.is_some()` ;
    un même nom de port sur deux ports est accepté (Kubernetes l'exige unique) ; pas de
    test d'un `service_account` invalide ou nul, d'un `host_network` non booléen, ni
    d'un réordonnancement de ports sans version ; l'ordre « namespaces avant workloads »
    (design « Connecteur ») vaut pour la création, pas la suppression : le connecteur ne
    peut faire de `Close` en cascade (`kube.connector.direct`), il clôt les workloads
    avant leur namespace (tâche 10).
- [x] 4b. Services : table `kube.service`, `upsert` et `close` dans `kube.sync`, ports
  (port, port cible, protocole, `appProtocol`, port de nœud), `external_name` ; IP de
  cluster dans le domaine des services et adresses externes dans le domaine externe du
  cluster, adresses `vip` du noyau planifiées par le lot (`CreateAddress`, identifiants
  rendus par `ModuleHandler::planned`, 3a) ; références service → adresse en
  `OnClose::Owned` avec `ModuleHandler::targets` (3d, sans quoi la clôture échoue) ;
  sélection `workload_ids` (référence service → workload en mise à nul) lue dans les
  deux sens — REQ-019-9, 14 — verify : `service_vip_refuses_close`,
  `selection_lists_both_ways`, un service clos qui clôt ses adresses `vip`
  - où : comme la 4a ; `ModuleHandler::targets` de la 3d (`handler.rs`, nœud → réseau)
    à imiter ; `kartenn-machines/src/handler.rs` (références de la tâche 009-8)
  - dépend de : 4a
  - fait (2026-11-10) : migration `0004_service.sql` (table du design, exclusion
    (namespace, nom), index `service_by_namespace`, `service_by_uid`, GIN
    `service_by_workload`, `service_cluster_addresses`, `service_external_addresses`) ;
    `module.rs` (type `kube.service`, collection `kube-services`, parent namespace, ancré
    dans le domaine des services ; droits `kube.service.read`, `edit`, `apply`,
    `approval.self` ; références `kube.service` → `kube.namespace` en `Cascade`,
    → `address` en `Owned`, → `kube.workload` en `Nullify`) ; `labels.rs` ; `tracked.rs`
    (`ServiceType::parse`) ; `handler.rs` (`kube.sync` de `type` `kube.service` ;
    `CreateAddress` d'usage `vip` dans le domaine des services ou le domaine externe,
    `Close` de l'adresse d'une valeur partie ; `targets` des adresses ; `nullify` qui
    retire le workload clos de la sélection) ; `pg.rs` (`InNamespace`, `namespaced`,
    `namespaced_closed`, `service_domains`, `service_addresses`, `workloads_named`,
    `kube.service.duplicate` avant la contrainte, `placement` et `referrers` du
    service) ; tests `service_vip_refuses_close`, `service_closed_closes_its_addresses`,
    `selection_lists_both_ways` (`tests/it/clusters.rs`) ; contrat (`openapi-v1.json`),
    `docs/spec/permissions.md` et types des deux SDK régénérés par leurs outils.
  - choix : champs du `sync` d'un service : `namespace` (par son nom, comme un
    workload), `service_type` (`type` nomme déjà le type d'objet du lot), `headless`,
    `external_name` (sous-domaine DNS, point final admis, gardé tel quel), `ports`
    `[{port, target_port, protocol, app_protocol, node_port, name}]` (port cible nombre
    ou nom `IANA_SVC_NAME`, protocole `tcp` à défaut, `app_protocol` en syntaxe de clé de
    label comme l'exige Kubernetes, `kube.label.invalid` sinon ; ports 0 refusés ;
    ensemble trié), `cluster_ips` et `external_ips` (listes d'IP, noms de
    `tracked::TrackedService`), `workloads` `[{kind, name}]` du namespace du service, lus
    au `prepare` (workloads avant services, design « Connecteur »), un workload inconnu
    refusant le lot (`kube.operation.invalid`) ; une valeur nouvelle crée toujours une
    adresse (jamais de reprise d'une adresse existante du domaine, qu'un service clos
    fermerait) : une valeur déjà prise dans le domaine est refusée par le noyau
    (`core.address.duplicate`), une valeur hors de tout réseau du domaine aussi
    (`core.address.outside_network`) ; sans domaine externe, le domaine des services
    (correction ci-dessous) ; une seule référence `Owned` vers
    `address` couvre les deux colonnes ; `planned` devient générique (`data.slots` :
    pour chaque opération du noyau, la colonne qui cite ce qu'elle crée), les plages de
    nœud y passent ; la réouverture d'identité de la 4a vaut pour le service (X3, X9 :
    `namespaced_closed`) ; `mirror_row` prend un `Home` (cluster, workload de sa sorte,
    service) au lieu du couple optionnel de la 4a (son reste « `mirror_row` masque son
    paramètre » est levé), `workload_identity` devient `home_of` ; la sélection ne
    demande pas `kube.workload.read` (comme le namespace désigné par son nom) ; le `close`
    par `kube.sync` d'un workload encore sélectionné le retire de la sélection
    (correction ci-dessous).
  - reste : pas de partage d'adresse entre services (IP de LoadBalancer partagée :
    refusée en doublon) ; aucune cohérence entre champs (headless sans IP de cluster,
    `external_name` seulement pour ce type) ; la sélection n'est vérifiée qu'au début
    de la période (un workload clos plus tôt qu'elle n'est pas vu) ; un nom de service est
    contrôlé en étiquette DNS (RFC 1123), non en RFC 1035 (lettre en tête) ; ni
    `/v1/kube-services`, ni la lecture `service_addresses` du port `kartenn_kube::read`
    (tâche 8) ; arêtes `kube.selects`, `kube.service_address` (4d) ; pas de test d'IP de
    cluster IPv6 ni d'un réordonnancement de ports sans version.
  - correction (revue) : le `close` d'un workload par `kube.sync` le retire de la
    sélection des services qui le citent, sans refus (design « Références », précision
    du 2026-11-10) : `prepare` lit ces services (`completed.selecting`) et le plan ajoute,
    avant la clôture, une opération `sync` d'action `unselect` par service, portée par
    `completed` seul (qu'une opération de client voit remplacé par `prepare` : refusée),
    sous le verrou du domaine du service ; pas de `cascade: true`, qui clorait d'autres
    dépendants ; une clôture directe sans cascade reste refusée. Décision du
    coordinateur : sans domaine externe, les `external_ips` vivent dans le domaine des
    services (design « Données », précision du 2026-11-10) ; une même valeur IP de
    cluster et adresse externe, domaines confondus, est une seule adresse citée par les
    deux colonnes (`vips` traite les deux sortes ensemble, par (domaine, valeur) ;
    `data.slots` porte une liste de colonnes par opération). Écart corrigé en route : un
    service est ancré dans le domaine des services (design « Types d'objets »), non dans
    celui des pods (`placement`, domaine verrouillé du plan). Tests
    `selection_lists_both_ways` (clôture par le connecteur acceptée, sélection vidée ;
    `unselect` d'un client refusé ; clôture directe refusée, en cascade acceptée) et
    `external_ips_fall_back_to_service_domain`.
  - reste (revue) : unicité de (port, protocole) et des noms de port d'un service ; le
    sens workload → services prouvé en SQL seulement ; le message de réouverture en double
    dit « the object {id} » sans le type ; une adresse stockée illisible est ignorée en
    silence (`filter_map` sur `host(a.address)`) ; un réseau des services non déclaré
    refuse chaque lot (`core.address.outside_network`), à faire voir au démarrage du
    connecteur (tâche 10) ; `kube.connector.direct` (tâche 5) devra laisser passer les
    opérations `unselect` et les plans de mise à nul du gestionnaire ; la mise à nul en
    cascade (`nullify`) ne déclare pas de domaine à verrouiller.
- [x] 4c. Ingress : table `kube.ingress` (classe, règles, hôtes TLS, adresses `vip`
  en `Owned` comme la 4b), `upsert` et `close` dans `kube.sync`, avertissement
  `kube.ingress.backend_missing` pour une règle qui vise un service absent du
  namespace (pas de refus) — REQ-019-10, 14 — verify : `ingress_backend_missing_warns`,
  un ingress clos qui clôt ses adresses
  - où : comme la 4b ; `rules.rs` (avertissement `range_outside` de la 3c à imiter)
  - dépend de : 4b
  - fait (2026-11-10) : migration `0005_ingress.sql` (exclusion (namespace, nom), index
    `ingress_by_namespace`, `ingress_by_uid`, GIN `ingress_addresses`) ; `module.rs`
    (type `kube.ingress`, collection `ingresses`, parent namespace ; droits
    `kube.ingress.read`, `edit`, `apply`, `approval.self` ; références → `kube.namespace`
    en `Cascade`, → `address` en `Owned`) ; `rules.rs` et `labels.rs`
    (`kube.ingress.duplicate`, type, droits) ; `tracked.rs` (`PathType::parse`,
    `as_str`) ; `handler.rs` (`kube.sync` de `type` `kube.ingress`, routes et hôtes TLS
    contrôlés, `vips` générique par ses `sorts`, ancrage et domaine verrouillé dans le
    domaine externe à défaut des services, `targets` des adresses, `check` de
    `backend_missing` sur les ingress écrits et sur ceux dont une route nomme un service
    écrit, réévalué pour clore l'anomalie) ; `pg.rs` (`InNamespace` porte sa table,
    `ingress_addresses`, `kube.ingress.duplicate` avant la contrainte, `placement`,
    `referrers`, `ingresses_routing_to`, `backends_missing`) ; tests
    `ingress_backend_missing_warns`, `ingress_closed_closes_its_addresses`,
    `ingress_addresses_fall_back_to_service_domain` (`tests/it/clusters.rs`) ; contrat,
    `docs/spec/permissions.md`, `docs/spec/regles-modules.md` et types des deux SDK
    régénérés par leurs outils ; design « Données » et « Règles » précisés.
  - choix : champs du `sync` d'un ingress : `namespace` (par son nom), `uid`, `class`
    (`ingressClassName`, sous-domaine DNS ou nul), `rules` (les routes
    `{host, path, path_type, service, port}` de `tracked::IngressRoute`, ensemble trié ;
    le backend par défaut est la route sans hôte, chemin ni type ; `path_type` `exact`,
    `prefix` ou `implementation_specific`, un chemin commence par `/`, `exact` et
    `prefix` exigent un chemin, une route d'hôte exige un type ; hôte en sous-domaine
    DNS, premier label `*` admis, une IP refusée en `kube.name.invalid` ; service en
    étiquette DNS ; port nombre ou nom `IANA_SVC_NAME`), `tls_hosts` (ensemble trié,
    hôtes contrôlés comme ceux des routes), `addresses` (IP de
    `status.loadBalancer.ingress`, une sorte unique, colonne `address_ids`) ; nom d'un
    ingress en sous-domaine DNS ; règle `kube.ingress.duplicate` ajoutée (la famille des
    doublons du design l'omettait ; même contrôle avant la contrainte que le service) ;
    `backend_missing` compare le seul nom du service (pas son port), sur la version de
    l'ingress vivante à partir de maintenant, et l'écriture d'un service (créé, modifié,
    clos) réévalue les ingress du namespace qui le nomment, pour que l'anomalie s'ouvre
    et se close sans réécrire l'ingress ; réouverture d'identité (X3) comme la 4b.
  - reste : pas de partage d'adresse entre un ingress et un service (l'IP d'un ingress
    est souvent celle du service LoadBalancer de son contrôleur, refusée en doublon
    `core.address.duplicate` quand les domaines se confondent : à trancher avant le
    connecteur, tâche 10), ni entre deux ingress d'une même classe ; adresses en nom
    d'hôte (`status.loadBalancer.ingress[].hostname`) ignorées ; backend `resource`
    (non service) non représenté ; un port de route absent du service n'avertit pas ;
    `kube.ingress.backend_missing` ne voit qu'à partir de maintenant (comme
    `range_outside`) ; ni `/v1/ingresses` ni lecture du port `kartenn_kube::read`
    (tâche 8) ; arêtes `kube.routes_to`, `kube.ingress_address` (4d) ; pas de test d'IP
    IPv6 ni de réordonnancement de routes sans version.
  - reste (revue) : `kube.ingress.duplicate`, ajoutée au catalogue (famille `*.duplicate`,
    précédent de `kube.operation.invalid`), prise acte ; aucun test ne déclenche
    `kube.ingress.duplicate` ni `kube.service.duplicate` ; pas de cas d'un service rouvert
    sous une route encore vivante (`backend_missing`) ; la doc de `owned_columns`
    (`handler.rs` vers la ligne 1320) a un retour à la ligne parasite ;
    `ingresses_routing_to` est appelée aussi pour un service touché par un `unselect`
    (requête inutile) ; deux routes identiques vers deux services différents passent sans
    avertissement. Partage d'adresse : tâche 4e ; domaines des services et externes
    changés sous des objets vivants : tâche 4f.
- [x] 4e0. Créations du noyau partagées entre les plans d'un changeset (arbitrage du
  2026-11-11, Q-019-17 : précision) : chaque opération `kube.sync` est préparée et
  planifiée seule, deux porteurs nouveaux d'une même IP dans un lot planifient chacun
  leur `CreateAddress` (refus `core.address.duplicate`), et une passation (A rend l'IP,
  B la prend dans le même lot) fait clore l'adresse par A avant que B la cite.
  Amendement additif 10 de l'ADR 0026 : un plan de module marque des opérations du
  noyau qu'il ajoute comme partageables (par exemple `ModulePlan::shared`, indices dans
  `core`, vide par défaut) ; dans `pipeline::plan`, un `CreateAddress` partageable
  identique (domaine, valeur, usage) à celui qu'un plan antérieur du changeset a ajouté
  n'est pas replanifié, et `planned` reçoit dans `created` l'identifiant antérieur ; la
  `Close` d'une adresse partageable par un module est retirée si un plan du même
  changeset la cite — verify : test du pipeline voisin de
  `module_data_cite_the_core_objects_it_creates` (deux plans, une création, deux
  citations ; la passation sans clôture) ; comportement inchangé sans marque
  - où : `crates/kartenn-service/src/module.rs` (`ModulePlan`),
    `crates/kartenn-service/src/pipeline.rs` (`plan`, vers la ligne 965),
    `docs/adr/0026-contrat-de-module.md` (amendements 8 et 9 à imiter)
  - dépend de : 4c
  - choix : un crochet `ModuleHandler::shared(plan) -> Shared` plutôt qu'un champ de
    `ModulePlan` (une trentaine de constructions dans six crates, défaut neutre comme
    `planned`) ; `Shared::cites`, les adresses que l'objet cite une fois écrit, rend la
    passation lisible au noyau ; la `Close` partageable se tranche dans
    `cascade::release`, sur l'état de la transaction (un porteur hors du lot compte, un
    porteur réécrit par un plan qui déclare ses `cites` est jugé par eux), et
    `close_owned` lit de même ; clé de dédoublonnage (domaine, valeur lue, usage,
    période propre) ; marque `shared` portée par `Planned::Close` ; deux clôtures
    partageables d'une adresse n'en font qu'une
  - choix (revue) : `Shared::scope`, la portée du partage, entre dans la clé avec le
    module (deux clusters d'un domaine ne partagent pas) ; une création partageable ne
    nomme pas de `local` (écrit au contrat) ; passation testée avec le module factice
    (`update_thing` qui rend son adresse, `create_thing` qui la déclare)
  - reste : un objet réécrit deux fois dans un lot est jugé par l'union de ses `cites` ;
    la clôture partageable d'une adresse encore visée par une affectation de propriété
    est retirée en silence (l'adresse reste sans porteur, le lot suivant qui la donne est
    refusé en `core.address.duplicate`), non testée (`cascade::cited`) ; pas de test de
    deux plans non marqués ; `cited` de `close_owned` et de `release` décidé avant les
    verrous
- [x] 4e. Adresses `vip` partagées (arbitrage du 2026-11-10, Q-019-17) : les ingress
  d'une classe ont l'IP du service LoadBalancer de leur contrôleur, deux services peuvent
  partager une IP ; un second porteur est aujourd'hui refusé (`core.address.duplicate`)
  et le lot entier du connecteur avec lui. Quand l'adresse existe déjà dans le domaine
  et qu'un objet `kube.*` vivant du même cluster la possède, le lot la réutilise
  (citation, pas de `CreateAddress`) ; elle se clôt avec son dernier porteur (`cited`
  de la 3d le tient déjà) ; une adresse que ne possède aucun objet `kube.*` du cluster
  (déclarée à la main, ou d'un autre cluster) n'est jamais adoptée : refus comme
  aujourd'hui — verify : deux ingress et un service sur la même IP, un seul
  `CreateAddress` ; l'adresse survit à la clôture des deux premiers et se clôt avec le
  dernier ; une adresse déclarée à la main refusée
  - où : `crates/kartenn-kube/src/handler.rs` (`vips`, `prepare`), `pg.rs` (lecture des
    adresses du domaine et de leurs porteurs), `crates/kartenn-service/src/cascade.rs`
    (`cited`) ; tests voisins de la 4b et de la 4c dans `tests/it/clusters.rs`
  - dépend de : 4c, 4e0
  - choix : `prepare` lit (`pg::shared_addresses`) les adresses vivantes des valeurs
    données qu'un service ou un ingress vivant d'un namespace vivant du cluster cite ;
    `vips` les cite comme les siennes ; toutes ses créations et clôtures d'adresses sont
    partageables, et le plan déclare dans `data.cites` les adresses existantes que l'objet
    cite une fois écrit : le noyau tranche la clôture (un porteur hors du lot la garde, la
    passation et le départ commun se lisent sur le lot), le module ne lit pas les autres
    porteurs ; partage borné au cluster (`data.scope`) ; une adresse citée sans être
    créée ni tenue déjà se lit (`core.address.read` dans `reads`, contrat 009/M16, revue) ;
    `prepare` découpé (`prepare_addresses`, limite de
    longueur) ; doc d'`owned_columns` réécrite (retour à la ligne parasite de la 4c)
  - reste : une adresse en partage n'est pas listée par un porteur comme « partagée » (ni
    lecture ni avertissement) ; deux syncs du même objet dans un lot sont jugées par
    l'union de leurs `cites` ; pas de test IPv6 ni d'un service et d'un ingress de
    namespaces différents (même chemin) ; la cascade d'un namespace qui clôt un porteur
    pendant qu'un autre porteur du lot rend l'adresse suit `close_owned`, non testée
- [x] 4f. Domaines des services et des adresses externes figés sous des objets vivants
  (revue de la 4c ; même règle que Q-019-15 pour le domaine des pods) : `update_cluster`
  qui change `service_domain_id` ou `external_domain_id` d'un cluster qui a des services
  ou des ingress vivants est refusé (`kube.operation.invalid`), comme
  `pg::pod_domain_moved` — verify : le changement refusé avec objets vivants, accepté
  sans
  - où : `crates/kartenn-kube/src/handler.rs` (`update_cluster`), `pg.rs`
    (`pod_domain_moved` à imiter) ; test `pod_domain_change_refused_with_living_objects`
  - dépend de : 4c
  - choix : le refus se tranche dans `apply` du plan `update_cluster`, à côté de celui du
    domaine des pods (`pg::service_domain_moved`, une requête par colonne nommée dans le
    correctif) ; un seul ensemble d'objets pour les deux domaines, services et ingress
    vivants d'un namespace du cluster : le domaine externe se replie sur celui des
    services (ancrage des ingress, adresses externes des services), changer l'un déplace
    les deux familles ; `IS DISTINCT FROM`, poser un domaine externe qui était nul ou le
    retirer compte comme un changement, redonner la même valeur (nulle comprise) non ;
    les namespaces seuls (ancrés dans le domaine des pods) ne bloquent pas — verify :
    `service_domains_change_refused_with_living_objects`
  - reste : pas de ré-ancrage (Q-019-15, A) ; pas de test sur une période passée ou
    future (`period` suivi comme pour le domaine des pods)
  - reste (revue) : le retrait d'un domaine externe (posé → nul) sous des objets vivants
    n'est pas testé ; `service_domain_moved` (`pg.rs` vers 573-579) part de tous les
    services et ingress vivants de la base avant de filtrer par namespace du cluster,
    partir de `kube.namespace` (`cluster_id`) et des index par namespace serait plus sûr
    (à mesurer, opération rare) ; poser un domaine externe égal au domaine des services
    alors qu'il était nul est refusé bien que l'ancrage ne bouge pas (prudent).
- [x] 4d. Arêtes : `kube.runs_on`, `kube.node_range`, `kube.selects`,
  `kube.service_address`, `kube.routes_to`, `kube.ingress_address` déclarées par le
  module (ADR 0026 clauses 11, 12) ; la chaîne d'exécution d'un workload (nœuds,
  machines, hyperviseurs à la date) passe par le lien observé workload → nœud et revient
  aux tâches 7 (`execution_chain`) et 8 (route) — REQ-019-11 (arêtes) — verify :
  `execution_chain_edges`, `GET /v1/graph/edges?object=<workload>` qui rend ses services
  - où : `crates/kartenn-kube/src/module.rs` ; arêtes de `kartenn-machines` à imiter ;
    ADR 0026 clauses 11, 12
  - dépend de : 4c
  - fait en partie (2026-11-11, non cochée) : les six arêtes déclarées (`module.rs`,
    `EDGES`) et lues aux dates de la lecture (`edges.rs`, appelé par `Handler::edges`),
    chaque branche filtrée par la clé ou l'index de sa table ; test
    `execution_chain_edges` (`tests/it/edges.rs`, par le routeur) : le workload rend le
    service qui le sélectionne, l'ingress son service et son adresse, le service son
    ingress, son workload et ses adresses, le nœud sa machine et sa plage, la machine
    son nœud et son hyperviseur (`machines.hosted_on`) ; une extrémité illisible retire
    l'arête ; un `as_of` passé rend la sélection d'alors.
  - choix : `kube.routes_to` résout le nom de service des routes dans le namespace de
    l'ingress à la date, une arête par couple (ingress, service) quelles que soient ses
    routes, sur la période commune des deux versions ; une route vers un service absent
    ne fait pas d'arête ; `kube.service_address` couvre IP de cluster et adresses
    externes (une adresse des deux à la fois : une arête) ; test dans un module à part,
    avec son propre `Env` et le routeur (dépendances de test `axum`, `http-body-util`,
    `kartenn-auth`, `kartenn-rest`, `tower`, déjà du workspace, comme
    `kartenn-machines`).
  - arrêtée : la chaîne « workload → nœud » n'a pas de lien dans le modèle versionné
    (design « Chaîne modélisée » : `workload ─(observé)─▶ nœud`) ; elle demande le
    journal des pods (tâche 6) et revient à `execution_chain` du port de lecture (tâche
    7) et à `GET /v1/workloads/{id}/chain` (tâche 8). Aucun lien inventé. Tranché par
    la session principale (2026-11-11) : 4d cochée sur les arêtes, la chaîne à la 7.
  - reste (revue) : `edges.rs` (vers la ligne 31) recopie `pg::time` au lieu de la
    partager ; sur un `as_of` passé, les recherches par l'extrémité cible dans les tables
    `_history` n'ont pas d'index (à mesurer au banc) ; le second `Env` de
    `tests/it/edges.rs` recopie les aides de `clusters.rs` ; pas de test d'une adresse à
    la fois IP de cluster et externe (une arête), de la période commune de `routes_to`
    quand les versions diffèrent, ni de `routes_to` sur un `as_of` passé.
- [x] 5. Objets miroirs et absence : règles `kube.object.managed` (auteur compte de
  service membre du groupe du connecteur du cluster), `kube.connector.direct` et
  `kube.cluster.connector_group`, `mark_absent`, `clear_absent`, anomalie
  `kube.object.absent`, section `kube.objects`, permissions
  `kube.<type>.{read,edit,apply,approval.self}` déclarées (grammaire de la 024) —
  REQ-019-5, 12, 22, 30 (règles) — verify : `managed_field_refused_for_human`,
  `managed_field_refused_for_person_in_connector_group`,
  `property_on_mirror_accepted`, `connector_direct_operation_refused`,
  `cited_workload_goes_absent`, `absent_cleared_on_return`,
  `section_lists_changed_objects`, `catalogue_problems` vide pour `kube.*` ; découpée
  le 2026-11-11 en 5a à 5c (permissions déjà déclarées par les tâches 3b à 4c), cochée
  avec la dernière.
- [x] 5a. Objets miroirs : règles `kube.object.managed` (un champ suivi écrit par un
  auteur qui n'est pas un compte de service membre du groupe du connecteur du cluster :
  refus ; une propriété sur un miroir reste écrite sous les droits ordinaires),
  `kube.connector.direct` (le connecteur n'écrit que par `kube.sync`, pas d'opération du
  noyau ni de `Close` en cascade à la main ; les plans que `kube.sync` ajoute lui-même,
  mise à nul `unselect` de la 4b, adresses et réseaux, passent) et
  `kube.cluster.connector_group` appliquées — REQ-019-5, 12 — verify :
  `managed_field_refused_for_human`,
  `managed_field_refused_for_person_in_connector_group`, `property_on_mirror_accepted`,
  `connector_direct_operation_refused`
  - où : design « Règles » ; `crates/kartenn-kube/src/rules.rs`, `handler.rs` (`check`) ;
    `CheckContext::changeset` (`crates/kartenn-service/src/module.rs`)
  - dépend de : 4c
  - fait : `crates/kartenn-kube/src/authorship.rs` (les deux règles, appelées par
    `check` avant son retour anticipé) ; `CheckContext::operations` et
    `CheckContext::author` (ADR 0026, amendement 11, Q-019-18 : les opérations écrites
    par l'auteur, sans celles qu'ajoutent les modules ni la revue d'un validateur ;
    l'auteur avec son émetteur et les groupes de sa session ; design B3 corrigé) ; tests
    `clusters::managed_field_refused_for_human` (un administrateur, puis le connecteur
    d'un autre cluster sur les mêmes domaines),
    `managed_field_refused_for_person_in_connector_group`, `property_on_mirror_accepted`,
    `connector_direct_operation_refused` (propriété seule, propriété à côté d'un lot,
    `Close` en cascade, personne membre par un groupe d'émetteur ; réseau d'un nœud créé
    et clos, IP de cluster créée et mise à nul `unselect` par les lots),
    `mirror_closed_by_hand_refused` (`Close` d'un workload et d'un namespace en cascade
    refusés, annulation d'un lot par restauration, clôture du cluster en cascade qui
    emporte ses miroirs) ; `tests/it/support.rs` : chaque cluster des tests désigne un
    groupe dont un compte de service soumet les lots ; les suites existantes passent sous
    ce compte, leurs clôtures de namespace ou de workload à la main remplacées par la
    clôture du cluster en cascade (`close_cluster`).
  - choix : `kube.object.managed` juge les objets miroirs d'un changeset qui porte un
    `kube.sync` (compte de service membre direct, `member_kind = 'identity'`, vivant
    maintenant, du groupe qu'une version du cluster vivante à partir de maintenant
    désigne, groupe vivant), et refuse tout `Close` d'un miroir écrit par l'auteur
    (Q-019-18) ; une restauration (annulation d'un lot, X5), la clôture d'un cluster en
    cascade et les propriétés restent sous les droits ordinaires ;
    `kube.connector.direct` vaut pour tout membre, direct ou par un groupe d'émetteur,
    par `kartenn_member_of` sur l'auteur du RBAC (Q-019-18), une violation par
    changeset qui nomme ses groupes, le nombre d'opérations hors `kube.sync` et la
    première ; `kube.cluster.connector_group` n'avait rien à compléter (existence et vie
    sur la période, à la création comme à la modification, tâche 3b) ; `check_modules`
    prend un contexte commun (limite d'arguments) ; `granted` des tests crée un compte de
    service du groupe.
  - reste : l'annulation d'un lot qui a créé des miroirs porte leur `Close` et est donc
    refusée (le noyau ne marque pas un changeset d'annulation) ; `connector_groups`
    s'appuie sur `kartenn_member_of` (groupe vivant), `unmanaged` sur un groupe vivant lu
    à part ; une version future du cluster qui change de groupe laisse passer les
    membres des deux groupes sur la période courante ; la clôture d'un groupe ou le
    retrait d'un membre ne relit pas les clusters ; une requête d'appartenance par
    changeset avec auteur, coût à mesurer (tâche 16) ; `granted` fait de toute identité
    un compte du connecteur (RBAC kube testé seulement sur ce chemin) ;
    `managed_field_refused_for_human` change un label au lieu de renommer (aligner sur
    l'acceptation de REQ-019-12).
- [x] 5b. Absence : `mark_absent` et `clear_absent` dans `kube.sync`, colonne
  `absent_since`, anomalie `kube.object.absent` qui nomme les dépendants, levée au
  retour d'un objet de même identité (avec la réouverture d'identité de la 4a) —
  REQ-019-22 — verify : `cited_workload_goes_absent`, `absent_cleared_on_return`
  - où : design « Règles », X9 ; `crates/kartenn-kube/src/handler.rs`, `pg.rs`, `rules.rs`
  - dépend de : 5a
  - fait : `handler.rs` (`plan_sync` : actions `mark_absent` et `clear_absent` sur
    l'objet vivant de l'identité, sans champ suivi ; `upsert` remet `absent_since` à nul ;
    `apply_mirror` date l'absence ; `check` : `absences`, `absence`) ; `pg.rs` (`absent`,
    `observed_at`, `backends_missing` qui compte un service absent comme manquant) ;
    colonnes `absent_since` déjà posées par les migrations 0001 à 0005 ; design « Règles »
    précisé ; tests `clusters::cited_workload_goes_absent` (sélection d'un service et
    propriété de référence d'un autre workload nommées, objet toujours vivant et
    sélectionné, date gardée par une seconde observation, dépendants réévalués quand le
    service cesse de le sélectionner, refus pour un administrateur, pour un champ suivi et
    pour un objet inconnu), `absent_cleared_on_return` (workload recréé sous un nouvel UID :
    même identifiant, présent, anomalie close ; service absent visé par un ingress :
    `kube.object.absent` et `kube.ingress.backend_missing`, closes par `clear_absent` ;
    workload absent clos par `kube.sync` : anomalie close).
  - choix : `absent_since` est le début de la période de l'opération, sinon l'instant de
    la transaction ; une `mark_absent` d'un objet déjà absent, une `clear_absent` d'un
    objet présent n'écrivent rien (plan `noop`, `completed.absent` lu par `prepare` ;
    correction de revue, test dans `cited_workload_goes_absent`) ; `kube.object.absent` est levée pour tout objet absent, même sans
    dépendant connu du module (les dépendants des autres modules, flux de la 011, ne se
    voient pas d'ici), ses dépendants dans le message et les sujets, sans `detail`
    structuré ; dépendants : services qui sélectionnent un workload, ingress qui routent
    vers un service, workloads, services et ingress d'un namespace, porteurs d'une
    propriété de référence (`property_assignment`) ; réévaluée sur les miroirs écrits et
    les absents qu'ils citent dans une de leurs versions ; ni index ni historisation
    propre pour `absent_since` (colonne versionnée comme les autres).
  - reste : un nœud n'a pas encore de dépendant dans le modèle (le journal des pods, tâche
    6) ; l'affectation ou le retrait d'une propriété de référence ne réévalue pas
    l'anomalie de sa cible (elle se rafraîchit à la prochaine écriture de l'objet absent) ;
    les flux de la 011 ne sont pas nommés tant qu'ils ne citent pas les objets `kube.*`
    (amendement de la 011) ; pas de test d'une `mark_absent` datée par une période, ni
    d'un namespace absent. Revue : `= ANY(a.v_uuids)` dans `pg::absent` contourne l'index
    GIN (`@>` attendu, même idiome dans `kartenn-flow`) ; un `upsert` daté d'avant une
    `mark_absent` lève l'absence qui suit ; `since::text` dépend du fuseau de la session ;
    les dépendants d'un namespace ne sont pas plafonnés dans le message ; le retour par la
    réouverture de la 4a d'un absent clos n'est pas testé.
- [x] 5c. Section de différentiel `kube.objects` et catalogue : les objets `kube.*`
  écrits par un changeset listés dans sa section ; `catalogue_problems` vide pour
  `kube.*` — REQ-019-30 (règles) — verify : `section_lists_changed_objects`,
  `catalogue_problems` vide
  - où : design « Section de différentiel » ; section `machines.attachments`
    (`crates/kartenn-machines/src/handler.rs`) à imiter
  - dépend de : 5a
  - fait : `handler.rs` (`OBJECTS_SECTION`, `diff` sur les miroirs touchés) ; `pg.rs`
    (`MIRRORS`, l'état explicite de chaque type ; `object_changes` : versions d'avant la
    transaction contre versions courantes, périodes agrégées une fois par miroir, une
    ligne par portion de temps qui diffère) ; `module.rs` (test `catalogue_problems_empty`,
    le module seul puis avec le noyau et `machines` : aucun problème, déclaration
    inchangée) ; test `clusters::section_lists_changed_objects` (création d'un nœud et
    d'un namespace puis d'un workload, changement de labels, même upsert sans ligne,
    propriété sur un miroir sans ligne, `mark_absent` puis seconde `mark_absent` sans
    ligne, `clear_absent`, clôture en cascade du cluster simulée puis appliquée : nœud,
    namespace et workload clos ; un cluster ne fait aucune ligne).
  - choix : miroirs seulement (nœuds, namespaces, workloads, services, ingress), pas le
    cluster, déclaré et sans champ suivi ; `before` et `after` en JSON explicite par type
    (Q-019-19, arbitrée par l'utilisateur à la revue : noms du design, `absent_since` en
    RFC 3339 UTC formaté en SQL comme le rend `time`) ; une ligne par portion où l'état
    diffère, sa période dans `period` (comme `machines.attachments`), aucune pour une
    portion à l'état inchangé (une version identique réécrite) ; objets pris dans
    `touched` du contexte (index par identifiant, pas de balayage sur `sys_start`) ;
    ordre : type (nœud, namespace, workload, service, ingress), identifiant, début.
  - reste : la section n'est pas encore lue par la 010 ni la 011 (REQ-019-36, leurs
    amendements) ; pas de test d'une écriture bornée dans le passé (plusieurs portions)
    ni d'un service ou d'un ingress dans la section (revue) ; le format de
    `absent_since` n'est pas testé sous un autre fuseau de session.
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
  `execution_chain` (nœuds par le journal d'observation, tâche 6, puis machines et
  hyperviseurs par les arêtes de la 4d), correspondances de labels, valeurs admises et labels suivis du
  module (paramètres `kube.settings.*`, tables et opérations, `operation_permissions`)
  — REQ-019-32 à 34 (côté module), REQ-019-3 — verify :
  `axis_from_namespace_then_tree`, `axis_values_one_query`, `label_mapping_admitted`,
  `label_mapping_needs_kube_settings`, `looks_like_pod_warns`, `execution_chain_at_date`
  - où : design « Propriétés », « Port de lecture » ;
    `crates/kartenn-machines/src/read.rs` (`machine_values_of_addresses`) ;
    `crates/kartenn-core/src/properties.rs` (`InheritTree`, un arbre par déclaration)
  - dépend de : 4, 6
- [ ] 8. Routes REST et imports : collections du module par `{id}` (clé naturelle en
  filtre, `?slug=`), filtres, chaîne d'exécution (`GET /v1/workloads/{id}/chain`), paramètres (`If-Match`), permissions
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

- [x] 9. Crate `kartenn-kube-sync` : choix du client Kubernetes (rapport `crate-scout`,
  Q-019-12), configuration, identifiants en fichiers relus, CA obligatoire, faux serveur
  d'API de test (liste, watch, coupure, `410`, signets, revue d'accès) — REQ-019-18, 29 —
  verify : `config_refuses_missing_ca`, `token_file_reloaded`,
  `no_credential_in_logs`, faux serveur rejouable
  - où : design « Connecteur » ; réf. § API, § Sécurité d'accès ;
    `crates/kartenn-server/src/config.rs` (`url_file`) à imiter ; `kartenn-workspace-hack`
  - dépend de : 0
  - fait : `crates/kartenn-kube-sync` : `config.rs` (TOML : `[kartenn]` `url`,
    `secret_file` ; `[[clusters]]` `slug`, `api_server` en https, `ca_file` exigé,
    `token_file` ou `cert_file` et `key_file`, `fallback_period_s` à 300 s,
    `allow_excess_rights` faux ; champs inconnus refusés, donc aussi
    `accept_invalid_certs`), `credentials.rs` (`CredentialFile` relu quand sa date, sa
    longueur ou son inode changent, `Secret` au `Debug` masqué, certificat client relu
    par la poignée de main qui suit un changement), `client.rs` (`connect` : `kube` sur
    une pile hyper-rustls et ring qui ne connaît que la CA du fichier, jeton en en-tête
    sensible relu à chaque requête ; `connect_with` pour un service quelconque) ;
    mesure de `k8s-openapi` dans `decisions.md` (Q-019-12, 59 s) ; `kartenn-workspace-hack`
    régénéré ; tests `config::config_accepts_token_or_certificate`,
    `config_refuses_missing_ca`,
    `config_refuses_accept_invalid_certs`, `config_refuses_bad_values`,
    `connect_refuses_unusable_ca`, `client::token_file_reloaded` (réécriture, renommage,
    fichier supprimé), `tls_trusts_configured_ca_only` (serveur TLS local, autre CA
    refusée), `no_credential_in_logs` (toutes les traces et logs du client réel, erreurs
    et `Debug`), `credentials::tests::client_certificate_read_again` ; faux serveur
    `tests/it/fake_api.rs` (`FakeApi`, `tower::Service` passé à `connect_with`) et
    `replay::*` (pages sur un instantané, jeton `continue` expiré, watch rejoué depuis
    une version puis en direct, signet, coupure et reprise, `410` en événement `ERROR` ou
    en réponse, revues `SelfSubjectRulesReview` et `SelfSubjectAccessReview`).
  - choix : `k8s-openapi` en `v1_33` (la plus ancienne version servie par la 0.29) ;
    `kube` avec la seule feature `client`, sans `rustls-tls` (qui tire
    `rustls-platform-verifier`), la pile TLS montée ici ; `kube-runtime` attend la
    tâche 10 qui s'en servira ; relecture du jeton par nos soins (métadonnées du fichier
    à chaque requête) plutôt que le `TokenFile` de `kube` (relu à la minute, sans test
    possible) ; la CA est lue au démarrage ; bornes de `fallback_period_s` 10 s à 24 h ;
    faux serveur dans les tests du crate, pas derrière une feature.
  - correction (revue) : `kartenn.url` et `api_server` analysés par `http::Uri` (schéma,
    hôte, ni information d'utilisateur ni fragment ; boucle locale = `localhost`,
    127.0.0.0/8, `::1`) au lieu d'une comparaison de préfixes ; tests
    `config_checks_the_kartenn_url`, `config_checks_the_api_server_url`.
  - reste : le `Debug` des erreurs de `kube` n'est couvert que par le test des logs, qui
    ne voit ni les enregistrements `log` (rustls) ni une `kube::Error::Api` ;
    `rustls-native-certs` reste dans l'arbre par le workspace-hack, qui unifie
    `hyper-rustls` avec la feature `native-tokio` (de `kartenn-auth`) et `tonic`
    (`tls-native-roots`, sortie OTLP), jamais appelé par le connecteur ; pas de contrôle
    que le certificat client et sa clé vont ensemble (`keys_match`) ; lectures de
    fichiers bloquantes dans `resolve` du certificat client (poignée de main).
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

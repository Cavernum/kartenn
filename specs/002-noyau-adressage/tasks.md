---
status: implemented
updated: 2026-10-17
---

# 002 — Noyau d'adressage : tâches

Chaque tâche est un commit, laisse le build vert et nomme ce qu'elle implémente. Phases :
voir [`../README.md`](../README.md). « verify » = commande ou test qui prouve la tâche.

## P0 — Fondations

- [x] 1. Workspace Cargo : crates vides `kartenn-core`, `kartenn-service`,
  `kartenn-store-pg`, `kartenn-server`, profils de [rust-build](../../docs/practices/rust-build.md)
  (dépendances optimisées en dev, `profiling`, release), `Cargo.lock` versionné,
  `rust-toolchain` non figé (nixpkgs) — prépare tout — verify : `cargo build --workspace`,
  `cargo clippy --workspace -- -D warnings`, `devenv test`
- [x] 2. cargo-hakari : crate `kartenn-workspace-hack`, `.config/hakari.toml`, vérification
  dans `devenv test` (`cargo hakari generate --diff`, `manage-deps --dry-run`) — verify :
  `devenv test`
- [x] 3. Base de test PostgreSQL : harnais d'intégration (une base jetable par test, via le
  PostgreSQL de devenv), extension btree_gist, rôle applicatif sans droits d'administration
  — verify : un test d'intégration trivial passe
- [x] 4. sqlx : dépendance, mode hors ligne (`.sqlx/` versionné, `SQLX_OFFLINE`),
  `cargo sqlx prepare --check` dans `devenv test` — verify : `devenv test`

## P1 — Prototypes de risque

- [x] 5. Générateur de jeu synthétique (crate `kartenn-bench`) : 20 domaines dont 5 sur
  `10.0.0.0/8`, conteneurs sur ≥ 6 niveaux, 10⁵ réseaux, 10⁶ adresses, IPv4 et IPv6, part
  d'espace public — prépare REQ-002-45 — verify : génération reproductible (graine) en
  < 2 min, décompte attendu
- [x] 6. **Prototype GiST** (`docs/spikes/001-gist-exclusions.md`) : (a) exclusion
  `domain_id WITH =, prefix inet_ops WITH =, valid_period WITH &&` ; (b) type `inetrange`
  avec `subtype_diff` et exclusion de chevauchement combinée à btree_gist (uuid) et
  `tstzrange` ; (c) exclusion partielle publique non anycast ; (d) deux transactions
  concurrentes ; (e) temps d'insertion et de recherche du parent sur le jeu de la tâche 5
  — REQ-002-15, 22, 26, 32, 45 — verify : rapport avec mesures ; conception amendée si une
  hypothèse tombe

## P2 — Domaine pur (`kartenn-core`)

- [x] 7. Valeurs d'adresse et de préfixe (`u128`, famille, longueur), contrat
  `AddressFamily`, inclusion, ordre, arithmétique — REQ-002-1 — verify : tests unitaires
  et de propriétés
- [x] 8. Familles IPv4 et IPv6 sur `std::net` : analyse, forme canonique, tests de
  non-régression du comportement vérifié (cas de la conception) — REQ-002-2, 3, 4, 6, 7
  — verify : tests ; propriété analyse(canonique(x)) = x
- [x] 9. Analyse des préfixes (longueur stricte, alignement, suggestion) ; refus des zones
  avec message dédié — REQ-002-4, 5 — verify : tests par cas de la table des erreurs
- [x] 10. Famille de test fictive de 64 bits passant la batterie de contrat commune —
  REQ-002-1 — verify : `cargo test -p kartenn-core --features test-family`
- [x] 11. Registres IANA : CSV versionnés dans `crates/kartenn-core/data/iana/`, structure
  de recherche du bloc le plus spécifique, indicateur public — REQ-002-33, 34 — verify :
  `192.0.0.9` → PCP Anycast ; `10.1.2.3` → Private-Use ; adresse publique → aucune entrée
- [x] 12. Cadre des règles : identifiant stable, sévérité, source, exigences ; catalogue
  généré (Markdown) — REQ-002-37 — verify : test qui compare le catalogue généré au
  fichier versionné
- [x] 13. Objets du noyau et règles locales : slugs, nature, point à point, bornes de
  plage, usages, adresses non attribuables, IPv4-mapped, heuristique ULA, documentation,
  espace partagé, IPv6 > /64 — REQ-002-13, 19, 21, 23, 24, 27, 29, 31, 35, 47, 48 — verify :
  un test valide et un test refusé par règle
- [x] 14. Capacité, prochain libre et occupation en pur (sur des listes triées) —
  REQ-002-43, 44 — verify : tests de propriétés (occupation ≤ capacité, prochain libre
  valide et minimal)

## P3 — Stockage (`kartenn-store-pg`)

- [x] 15. Migrations des objets du noyau (entité, site, domaine, préfixe, plage, adresse)
  en tables courantes + historique (004), contraintes d'exclusion `inet_ops`, `CHECK`,
  index — REQ-002-15, 22, 26, 32 — après 004-5 — verify : tests d'intégration des
  contraintes, y compris concurrence
- [x] 16. Table des registres à usage spécial chargée par migration ; recalcul de
  `public` à la mise à jour avec rapport — REQ-002-34 — verify : test de migration
  — rapport fait (plages comprises) ; le job de migration ne chargeait pas les registres,
  corrigé à la synchronisation (`special_registry.rs`,
  `the_migration_job_loads_the_embedded_registries`) ; la réécriture des drapeaux par un
  changeset système reste à faire, avec l'ordonnanceur (006)
- [x] 17. Identifiants externes et résolution de clé naturelle à une date — REQ-002-40,
  41 — verify : tests d'unicité et de résolution

## P4 — Validation globale (dans le pipeline 004)

- [x] 18. Port `AddressingView` : parent déduit, enfants, chevauchements, publics des
  autres domaines, à une date — REQ-002-14, 42 — après 004-7 — verify : tests
  d'intégration sur le jeu réduit
  — faits de validation livrés ; la navigation générale (parent, enfants) vient en 002-23
- [x] 19. Règles globales des préfixes : doublon, réseau feuille, changement de nature,
  redimensionnement — REQ-002-15 à 18 — verify : un test par règle et par cas de la
  table des erreurs — nature et redimensionnement avec les opérations de modification
  (004-8, `kartenn-service/tests/portion.rs`)
- [x] 20. Règles globales des plages et adresses : dans un réseau, chevauchement, usage
  de plage, unicité — REQ-002-21, 22, 25, 26, 28 — verify : idem
  (`kartenn-service/tests/ranges.rs`)
- [x] 21. Espace public et anycast entre domaines, verrou « espace public » —
  REQ-002-32, 46, 49 — verify : tests, dont concurrence
  (`kartenn-service/tests/anycast.rs` ; migrations 0009 et 0010)
- [x] 22. Ancres : cycles d'entités et de sites, clôture d'une ancre utilisée, domaine
  immuable ; `core.owner` et `core.site` déclarés comme propriétés du noyau (avec 003) —
  REQ-002-8 à 12, 20, 38 — après 003-8 — verify : tests, dont refus d'une manœuvre de
  structure qui change le propriétaire sans droit (`kartenn-service/tests/anchors.rs`) —
  héritage le long des arbres d'entités et de sites : tâche 003-18 (proposée)

## P5 — Lectures et anomalies

- [x] 23. Navigation : enfants directs, ancêtres, à une date — REQ-002-42 — verify : tests
  d'intégration
- [x] 24. Occupation et prochain libre en base — REQ-002-43, 44 — verify : tests
  d'intégration, comparaison avec l'implémentation pure (tâche 14)
- [x] 25. Anomalies : création, fermeture, acquittement, réouverture — REQ-002-35, 36 —
  verify : tests d'intégration
- [x] 26. Catalogue des règles exposé en lecture (pour 006) — REQ-002-37 — verify : test

## P6 — Budgets et synchronisation

- [x] 27. Banc CI : lecture d'un objet, page filtrée, sous-arbre et occupation, changeset
  unitaire sur le jeu de la tâche 5 ; seuils D14 bloquants — REQ-002-45 — verify :
  `kartenn-bench core` (script `bench-core`) sous les seuils, au lieu de `cargo bench`
  (A14) ; pas encore de CI, le banc est bloquant par son code de sortie
- [x] 28. Synchroniser `docs/spec/` (`data.md` : ancres en propriétés), ADR 0014 et 0015 →
  acceptés, statut `implemented` — verify : rapport spec-keeper sans écart — rapport du
  2026-10-17 : écarts de code corrigés (registres chargés au déploiement, anomalies par
  domaine et entité et plages comprises, opération `Rename`, contrat de famille
  complété), textes resynchronisés, contre-audit sans écart ; ce qui reste est différé
  et listé ci-dessous

## P7 — Intégrité des références (amendement du 2026-11-14 à REQ-002-12)

- [ ] 29. Toute référence d'une écriture est lue avant d'être écrite : à la résolution, une
  référence par identifiant (`Ref::Id`) est lue dans la transaction de l'écriture, à sa
  date de validité, avec le type attendu ; absente, d'un autre type ou close, l'écriture
  est refusée par la règle nouvelle `core.reference.unknown` (erreur, 422, champ nommé),
  comme l'est déjà une référence par nom introuvable ; une référence vers un objet créé
  plus tôt dans le même changeset reste acceptée. Recensement : toutes les opérations
  d'écriture du noyau et leurs références (domaine, parent, VLAN, groupe, interface,
  machine, entité…), et une requête qui compte les orphelins déjà en base, par table.
  Vérifier aussi qu'aucun droit porté par domaine ne se contourne par un domaine
  inconnu — REQ-002-12 (amendée), REQ-002-37 — verify : test d'intégration rouge
  d'abord (préfixe, plage, adresse créés sur un domaine inconnu, puis sur l'identifiant
  d'un objet d'un autre type : 422 `core.reference.unknown`, rien d'écrit) ; test qui
  parcourt le catalogue des opérations avec une référence fausse pour chacune ; la règle
  au catalogue (`docs/spec/regles-noyau.md`, `code_labels.rs`) ; `k-check` ; la requête
  des orphelins et son résultat sur la base de dev dans la note de clôture
  où : `crates/kartenn-service/src/pipeline.rs` (résolution des références, vers les
  lignes 471-494, 1079-1082, 1198-1200), `global.rs` (121, 451) ;
  `crates/kartenn-core/src/rules.rs` (modèle : `RBAC_UNKNOWN_ROLE`, `GROUP_UNKNOWN`) et
  `code_labels.rs` ; `crates/kartenn-rest/src/operation.rs` (36-43, 673-689) ;
  `crates/kartenn-store-pg/src/write.rs` (vers 1334) ; `docs/spec/regles-noyau.md`
  - découpée le 2026-11-14 (relevé par l'agent de la tâche, avant tout code) en 29a à 29d :
    le recensement nomme aussi les références des opérations de modules (VLAN, groupe,
    interface, machine), que chaque module résout par la fonction sans type que le
    pipeline passe à `ModuleHandler::plan` (`pipeline.rs` vers 1165) ; y donner le type
    attendu change le contrat des modules (ADR 0026) dans six crates.
- [x] 29a. Références des opérations du noyau : chaque `Ref::Id` que rend `references()`
  (`pipeline.rs` vers 366) pour une opération du noyau, demandée ou ajoutée par le plan
  d'un module, est relevée là où `plan()` la résout, puis lue en un seul lot par date de
  validité après les verrous ; absente, d'un autre type ou close au début de la période de
  l'opération : `core.reference.unknown` ; `Restore` hors du contrôle (sa cible peut être
  close). Les droits d'abord (INV-63) : un utilisateur lié au seul domaine A reçoit le même
  refus de ses droits pour un domaine inconnu, d'un autre type, clos ou hors de sa portée ;
  seul qui aurait le droit de lire l'objet désigné apprend `core.reference.unknown`, un
  `Bind` sur un domaine inventé compris. Requête des orphelins
  par table (domaines des préfixes, plages, adresses et objets de modules, parents
  d'entité et de site) — REQ-002-12 (amendée), REQ-002-37 — verify : test d'intégration
  rouge d'abord (`tests/it/reference_integrity.rs`) ; test qui parcourt le catalogue des
  opérations du noyau (un exemple par variante d'`Operation`, chaque `Ref::Id` de son JSON
  rendu par `references()` et refusé par la règle) ; la règle au catalogue ; `k-check`
  où : `crates/kartenn-service/src/pipeline.rs` (`plan`, `stages` après `lock_targets`),
  `port.rs` (`WriteTx`), `crates/kartenn-store-pg/src/write.rs` (modèle :
  `object_kind_covering`, vers 2612), `crates/kartenn-core/src/rules.rs`,
  `code_labels.rs`, `docs/spec/regles-noyau.md` ; tests voisins `tests/it/pipeline.rs`,
  `anchors.rs`
  - fait le 2026-11-14 : `references()` rend le champ de chaque référence (`Reference`),
    et aussi les portées et domaines d'une politique d'approbation et la portée d'une
    obligation ajoutée ; `plan()` relève chaque `Ref::Id` (`Designation`), y compris celles
    des opérations du noyau qu'ajoute le plan d'un module, hors celles d'un objet que le
    changeset crée (identifiant tiré par `plan`) ou que le plan d'un module écrit (ainsi
    l'affectation d'un flux dont le même changeset prolonge la période) ;
    `check_designations` les lit
    après `lock_targets` par `WriteTx::kinds_in_force` (une requête par date de début, sur
    `addressing_object`, entités, sites, groupes de VLAN, VLAN, déclarations, valeurs de
    liste fermée, objets de modules) ; règle `core.reference.unknown` (catalogue,
    `code_labels.rs`, `regles-noyau.md`), violation qui nomme l'opération, le champ
    (`detail.field`, chemin JSON dans l'opération : `domain`, `scope.id`, `scopes[].id`,
    `domains[]`, `id`) et l'objet. Correction après revue, les droits d'abord (INV-63) :
    une désignation inconnue n'est refusée tout de suite que si l'auteur peut lire un objet
    de son type à cet identifiant (`read_permission` ; sur un identifiant qui ne nomme rien,
    le RBAC ne l'accorde que par une liaison globale, ou sur cet identifiant même ; les
    déclarations se lisent par tous) ; sinon le changeset suit son cours et ses droits le
    refusent comme pour un objet hors de portée, `core.reference.unknown` ne venant qu'une
    fois tous les droits accordés. Tests `tests/it/reference_integrity.rs` (domaine inconnu,
    d'un autre type, clos ; référence locale ; écritures datées dans le passé et le futur ;
    catalogue : chaque étiquette d'opération a un exemple ou figure dans `NO_REFERENCE`,
    chaque `Ref::Id` du JSON d'un exemple est refusé) et
    `rbac::no_right_reaches_through_an_unknown_domain` (même refus `core.object.forbidden`,
    règle, message, opération, pour un domaine inconnu, une entité, un domaine clos et un
    domaine en vigueur hors de portée ; `core.reference.unknown` pour l'administrateur de
    tout, `Bind` compris). Règle attendue changée dans six
    tests (une cible ou une référence qui ne désigne rien du type attendu au début de la
    période, jusqu'ici refusée en aval par `changeset.operation.no_version` ou
    `prop.declaration.invalid`) : `pipeline.rs`, `module_pipeline.rs`, `anchors.rs`,
    `resolution.rs`, `declarations.rs` ; jeux de données corrigés là où une référence était
    fausse : `audit.rs` et `portion.rs` (préfixe daté avant son domaine), `import.rs`
    (liaison externe d'un objet inexistant), `kartenn-rest` `placements.rs` (préfixe daté
    d'une heure dans un domaine créé à l'instant). Requête des orphelins, sur toutes les
    versions (ligne sans aucune ligne visée, quelle qu'en soit la période) :
    `SELECT 'prefix.domain_id', count(*) FILTER (WHERE NOT EXISTS (SELECT 1 FROM
    addressing_domain d WHERE d.id = t.domain_id)), count(*) FROM prefix t`, de même pour
    `ip_range` et `address`, pour `module_object.domain_id` non nul, et pour
    `entity.parent_id` et `site.parent_id` non nuls (`FROM entity p WHERE p.id =
    t.parent_id`). Sur la base de dev : 0 partout, mais la base est vide (0 ligne dans
    chacune de ces tables) : ce zéro ne prouve rien
  - choix : « en vigueur » = au début de la période de l'opération (maintenant si elle
    n'en a pas), la couverture de toute la période restant à la tâche 30 ; `Restore` hors
    du contrôle ; cible absente au début = `core.reference.unknown`, absente sur une partie
    de la période = `changeset.operation.no_version` comme avant ; contrôle après les
    verrous, révélé après les droits (ci-dessus) ; hors du contrôle, sous une autre règle :
    les valeurs de type référence d'une affectation (`prop.assignment.dangling_reference`),
    la cible d'un `Restore` (`check_restored`, `global.rs`), la portée d'une obligation dans
    une déclaration et celle d'un retrait d'obligation (une obligation peut viser un
    domaine clos depuis) ; les identifiants nus du RBAC (appartenance, liaison, identité,
    compte) restent à ses règles
  - reste : rejouer la requête des orphelins sur une base peuplée (une copie de
    production ou le jeu du banc) avant la migration de la tâche 30
  - correctif du 2026-11-14 (parcours `web/v2/e2e/journeys/changesets.spec.ts`, annulation
    d'une proposition validée, rouge depuis la fusion) : une proposition sans période
    propre, validée plus tard, écrit à l'instant de sa validation, mais `default_period`
    gardait celui de sa soumission ; son inverse commençait donc avant l'objet qu'elle a
    créé, et `core.reference.unknown` refusait la fermeture. À l'application d'un
    changeset en attente, `write_journal` (`kartenn-store-pg/src/write.rs`) récrit
    `default_period` (inchangé pour une période explicite) ; test
    `undo::a_validated_proposal_is_undone` (`kartenn-service`), rouge d'abord. Choix : la
    période du changeset appliqué dit l'instant où il a écrit (`valid_from` lu par REST
    devient l'instant de validation, égal à `applied_at`), plutôt qu'un calcul de l'inverse
    à partir d'`applied_at` ; le contrôle des références reste entier
- [ ] 29b. Contrat de `ModuleHandler::plan` avec le type attendu — **arbitré le
  2026-11-14 : la résolution d'un module reçoit le type attendu** ; la tâche commence par
  écrire l'amendement de l'ADR 0026 (contrat du plan d'un module) — la fonction de
  résolution qu'un module
  reçoit prend le type attendu (`Fn(Ref, ObjectKind) -> Option<Uuid>`) et chaque
  `Ref::Id` qui la traverse entre dans le contrôle de la 29a ; VLAN et groupes de VLAN du
  cœur, modules de test. À reprendre de la 29a : `plan()` met `module_plan.object` dans
  `created` pour tout plan de module, création, mise à jour ou clôture, ce qui exempte du
  contrôle toute référence à cet objet dans le changeset ; la 29b décide si une mise à
  jour ou une clôture doit au contraire désigner un objet en vigueur — REQ-002-12
  (amendée) — verify : un groupe de VLAN ancré sur un
  identifiant inconnu ou d'un autre type, un VLAN d'un groupe inconnu : 422
  `core.reference.unknown` ; test qui parcourt les types d'opérations des modules du
  build ; `k-check`
  où : `crates/kartenn-service/src/module.rs` (`ModuleHandler::plan`, vers 525),
  `pipeline.rs` (vers 1165), `crates/kartenn-service/src/vlan.rs` (398-540),
  `crates/kartenn-store-pg/src/vlan.rs` (753), `tests/it/support/dummy_module.rs`,
  `kit_module.rs` ; ADR 0026
  dépend de : 29a
  - découpée le 2026-11-14 (relevé par l'agent de la tâche, avant tout code) en 29b1 et
    29b2 : changer la signature de la résolution casse d'un coup les six crates qui
    implémentent `ModuleHandler` (une vingtaine de fichiers, une soixantaine de sites
    d'appel) ; sans résolveur non typé provisoire, le contrat et tous ses sites d'appel
    passent dans un seul commit (29b1), les tests de refus suivent (29b2, 29c, 29d).
    Arbitré le même jour : le résolveur reçoit aussi le champ, que le refus nomme
    (`detail.field`, comme en 29a).
- [x] 29b1. Contrat typé de la résolution d'un module, en un commit atomique sur les six
  crates : l'amendement 17 de l'ADR 0026 (commit `docs(adr)` à part, avant le code), puis
  `ModuleHandler::plan` reçoit `resolve: &dyn Fn(Ref, ObjectKind, &str) -> Option<Uuid>`
  (référence, type attendu, champ) ; chaque `Ref::Id` qui la traverse devient une
  `Designation` du contrôle de la 29a (même règle, même ordre des droits avant les
  références, INV-63) ; chaque site d'appel de `kartenn-service` (VLAN, groupes de VLAN),
  `kartenn-store-pg`, `kartenn-machines`, `kartenn-kube`, `kartenn-flow`,
  `kartenn-policy` et des modules de test donne son type et son champ ; jeux de données
  réparés là où le contrôle les refuse désormais, chacun avec la raison de sa fausseté,
  sans affaiblir la règle. Aucun nouveau test de refus par module (29b2, 29c, 29d) —
  REQ-002-12 (amendée) — verify : `k-check` complet
  où : `crates/kartenn-service/src/module.rs` (`ModuleHandler::plan`, vers 525),
  `pipeline.rs` (`plan`, vers 1393 ; `Designation`, vers 1127), `vlan.rs` (398-580),
  `crates/kartenn-store-pg/src/vlan.rs` (vers 770), `crates/kartenn-machines/src/handler.rs`,
  `crates/kartenn-kube/src/handler.rs` (`reference`, vers 226), `settings.rs`,
  `crates/kartenn-flow/src/handler.rs`, `crates/kartenn-policy/src/handler.rs`,
  `matrix.rs`, `objects.rs` (`Fields`), `infer.rs`, `verdict.rs` ; modules de test
  (`kartenn-service`, `kartenn-rest`, `kartenn-server`, `kartenn-machines` sous
  `tests/it/`) ; ADR 0026
  dépend de : 29a
  - fait le 2026-11-14 : amendement 17 de l'ADR 0026 ; alias `module::Resolve` ; dans
    `pipeline::plan`, chaque `Ref::Id` que résout un module devient une `Designation`
    (champ en `Cow`, le chemin JSON du corps) lue par `check_designations` comme celles du
    noyau ; types et champs donnés partout : VLAN et groupes du cœur (`anchor.entity`,
    `anchor.site`, `anchor.domain`, `owner`, `group`, `id`), `machines` (propriétaire,
    site, `host.machine`, `host.cluster`, `member_of`, `machine`, `interface`,
    `address`, `vlan.access`, `vlan.native`, `vlan.tagged[]`, `id`), `kube` (domaines,
    préfixes `pod_containers[]`/`service_networks[]`, `machines_cluster`, `machine`,
    propriétaire, site, grappe, `id` et `namespace` des paramètres), `flow` (extrémités
    `source.id`/`destination.id` par leur type, service `policy.service`, entité, flux),
    `policy` (`Fields::object`/`objects` avec le type de chaque champ, axe, matrice,
    règles d'inférence), modules de test. Jeu de données : aucun refusé ; une attente
    changée, `machines::closed_vlan_refused` (un VLAN clos ou inconnu : la règle attendue
    passe de `machines.vlan.mode` à `core.reference.unknown`, le refus arrivant désormais
    avant le module, comme les six tests changés de la 29a)
  - choix : le groupe du connecteur d'une grappe `kube` (`connector_group`) est un groupe
    du RBAC, pas un `ObjectKind` : hors de `resolve`, lu par son identifiant, une référence
    locale refusée (aucune opération n'en définit sous un nom local) et jugé comme avant
    par `kube.cluster.connector_group` ; une désignation répétée (comparaison d'une
    extrémité inchangée dans `flow.update`) est relevée comme les autres
  - reste : l'exemption de `created` pour l'objet d'une mise à jour ou d'une clôture
    (29b2) ; tests de refus par module (29b2, 29c, 29d)
- [x] 29b2. Références des VLAN et groupes de VLAN du cœur et des modules de test, et
  objet écrit par un plan : seul un plan qui crée (`module_plan.creates`) met son objet
  dans `created` ; une mise à jour ou une clôture désigne un objet en vigueur, l'exemption
  d'une période prolongée ne valant que pour les opérations du noyau qu'ajoute ce même plan
  — REQ-002-12 (amendée) — verify : test rouge d'abord (une opération du noyau de la même
  requête qui désigne l'objet d'une mise à jour de module, cet objet inconnu : 422) ; un
  groupe de VLAN ancré sur un identifiant inconnu ou d'un autre type, un VLAN d'un groupe
  inconnu : 422 `core.reference.unknown` ; test qui parcourt les types d'opérations des
  modules du build ; un test des droits avant les références (INV-63) pour une opération
  de module ; `k-check`
  où : `crates/kartenn-service/src/pipeline.rs` (`plan`, vers 1393-1404),
  `tests/it/reference_integrity.rs`, `tests/it/rbac.rs`
  (`no_right_reaches_through_an_unknown_domain`), `tests/it/support/dummy_module.rs`,
  `kit_module.rs`
  dépend de : 29b1
  - fait le 2026-11-14 : dans `pipeline::plan`, seul un plan de module qui crée
    (`creates`) met son objet dans `created` ; l'objet d'une mise à jour ou d'une clôture
    est désigné (son `id` résolu) et lu comme toute référence, de même pour une opération
    du noyau de la même requête qui le désigne ; seules les opérations du noyau qu'ajoute
    ce plan visent son objet sans contrôle (prolongation d'un flux). Précision à
    l'amendement 17 de l'ADR 0026. Les droits d'abord (INV-63), deux fuites fermées que le
    test de la mise à jour d'un VLAN a montrées : un auteur refusé par ses droits sur les
    objets qu'il clôt ou change l'est avant toute écriture, par ces seuls refus (sinon
    `changeset.operation.no_version` révélait l'absence, et une requête à plusieurs
    opérations répondait `[forbidden, conflit]` pour une cible hors de portée, `[forbidden]`
    pour une cible inconnue), et `moved` ne lit plus le dernier domaine d'un objet caché,
    reconnu à son identifiant et à son type (un VLAN clos donnait
    `changeset.operation.conflict`). Correction après revue : le même cas (une clôture ou
    une mise à jour hors de portée, et une écriture en double dans son propre domaine :
    quatre réponses égales) dans `rbac::no_right_reaches_through_an_unknown_domain` (cœur)
    et `no_right_reaches_through_an_unknown_vlan` (VLAN), rouges d'abord
    (`core.prefix.duplicate`, `core.vlan.vid_duplicate` à côté du refus de droits) ; aucun
    test existant n'attendait de violations cumulées à côté d'un refus de droits.
    Tests `reference_integrity.rs` : `the_object_a_module_operation_changes_is_in_force`
    (rouge d'abord : `changeset.operation.no_version` au lieu de `core.reference.unknown`),
    `a_vlan_group_and_a_vlan_designate_objects_in_force` (ancre inconnue ou d'un autre
    type, propriétaire, groupe inconnu, d'une entité ou clos),
    `every_reference_of_every_module_operation_is_checked` (un exemple par type
    d'opération des VLAN du cœur, de `dummy` et de `kit`, chaque `Ref::Id` refusé ; rouge
    d'abord sur l'`id` de `update_vlan_group`), `the_writes_of_a_module_are_undone`
    (créer puis annuler, modifier puis annuler : VLAN, groupe réancré, objet de `dummy`) ;
    `rbac::no_right_reaches_through_an_unknown_vlan` (même refus `core.object.forbidden`
    pour un VLAN inconnu, une entité, un VLAN clos et un VLAN hors de portée ;
    `core.reference.unknown` pour l'administrateur de tout)
  - choix : un objet qu'un plan écrit sans le résoudre par `resolve` (l'objet existant
    d'une observation `kube`, lu par sa complétion) n'est pas une référence de l'auteur et
    reste hors du contrôle ; l'exemption des opérations ajoutées vaut pour le seul objet de
    leur plan, au type près ; arbitré après revue : tout refus de droits constaté avant
    écriture (objets clos ou changés) arrête le changeset avant toute écriture, la réponse
    ne portant que ces refus, que la désignation soit cachée ou non ; les droits sur les
    objets créés restent vérifiés après l'écriture
  - reste : l'annulation de la création d'un groupe de VLAN est refusée
    (`changeset.operation.no_version`, déjà avant cette tâche) : tâche 004-26. Ce que
    laissent ouvert les droits vérifiés après l'écriture (objets créés) : un auteur sans
    droit qui crée un objet désignant une référence cachée passe par les écritures, dont
    les refus (une écriture de module qui échoue sur la référence absente, une règle
    d'écriture qui cumule ses violations) peuvent différer de ceux d'une référence hors de
    portée et révéler l'absence ; non observé sur les modules de test, à vérifier par
    module en 29c et 29d
- [x] 29c. Références des opérations de `kartenn-machines` (VLAN, propriétaire, site, hôte
  machine ou grappe, `member_of`, machine, interface, adresse), typées par la 29b1 : tests
  de refus et cas particuliers — REQ-002-12 (amendée) — verify : une
  interface sur une machine inconnue, une adresse d'interface d'un autre type : 422
  `core.reference.unknown` ; la marche sur les types d'opération des modules du build
  (`every_reference_of_every_module_operation_is_checked`, `kartenn-service`) étendue à
  ceux de `kartenn-machines` ; une observation `kube` antidatée ne vise pas un objet qui
  n'est pas encore en vigueur au début de sa période (son objet existant, lu par la
  complétion, reste hors du contrôle, 29b2) ; un auteur sans droit qui crée un objet
  désignant une référence cachée reçoit le même refus que pour une référence hors de
  portée ; `k-check -p kartenn-machines`
  où : `crates/kartenn-machines/src/handler.rs` (14 résolutions) ; tests
  `crates/kartenn-machines/tests/it/machines.rs`
  dépend de : 29b1
  - découpée le 2026-11-15 (relevé par l'agent de la tâche, après les tests) en 29c1,
    29c2 et 29c3 : les tests de refus passent sans code, mais un auteur sans droit reçoit
    d'autres réponses pour une référence cachée que pour une référence hors de portée, et
    les fermer change l'ordre des droits et des écritures ou le modèle des droits (choix
    structurant) ; l'observation `kube` antidatée est d'une autre crate.
- [x] 29c1. Tests de refus et marche sur les types d'opération de `kartenn-machines` —
  REQ-002-12 (amendée) — verify : une interface sur une machine inconnue, une adresse
  d'interface d'un autre type : 422 `core.reference.unknown` ; marche sur chaque type
  d'opération du module ; `k-check`
  où : `crates/kartenn-machines/tests/it/machines.rs`
  dépend de : 29b1
  - fait le 2026-11-15 : aucun code changé, les 14 résolutions de la 29b1 suffisent.
    Tests `every_reference_of_every_operation_is_checked` (11 exemples pour les 8 types
    d'opération du module, chaque hôte et chaque mode 802.1Q avec ses références ; un
    type nouveau échoue tant qu'il n'a pas d'exemple ; chaque `Ref::Id` refusé par
    `core.reference.unknown`) et `the_references_of_the_module_designate_objects_in_force`
    (interface sur une machine inconnue, sur une adresse, sur une machine close ; adresse
    attachée qui est une machine, un site ou rien ; hôte `machine` qui est une grappe,
    hôte `cluster` qui est une machine, hôte clos ; `member_of` qui nomme une machine :
    422 `core.reference.unknown`, rien d'écrit)
  - choix : la marche vit dans `kartenn-machines` et non dans celle de `kartenn-service`
    (`every_reference_of_every_module_operation_is_checked`), qui ne peut dépendre du
    module sans cycle de dépendances de test
- [x] 29c2. Fuites vers un auteur sans droit sur une référence cachée de `kartenn-machines`
  (INV-63, legs de la 29b2) — arbitrée le 2026-11-15 — REQ-002-12 (amendée) —
  verify : test rouge `no_right_reaches_through_a_hidden_reference` (`machines.rs`, sous
  `#[ignore]` jusqu'à l'arbitrage) rendu vert ; `k-check`
  où : `crates/kartenn-service/src/pipeline.rs` (`stages` : droits avant écriture vers
  2861-2900, écritures 2918-2954, droits après 2956, `check_modules` 3012, désignations
  cachées 3025), `rights.rs` (`check_guards`, `Phase`), `crates/kartenn-machines/src/handler.rs`
  (`apply_attachment`, `check` : `dead_hosts`, `host_kinds`)
  dépend de : 29c1
  - constats du 2026-11-15 (auteur titulaire de tous les droits du module, des VLAN, des
    adresses et de `core.owner`/`core.site` sur `acme` ; cachée = inconnue, d'un autre
    type ou close ; hors de portée = en vigueur sous `other`) :
    - propriétaire ou site de `create_machine`/`create_cluster` caché :
      `prop.assignment.invalid_value` à l'écriture de l'affectation que le plan ajoute,
      avant les droits, dont le message dit « does not exist » ou « is a site, not a
      entity » ; hors de portée : `prop.assignment.forbidden_effect` et
      `core.object.forbidden` (propriétaire), appliqué (site) ;
    - adresse cachée d'un `attach_address` : `machines.address.closed`, levé à
      l'écriture ; hors de portée : `core.object.forbidden` (`core.address.read`), car les
      lectures exigées par le module (`ModuleHandler::reads`, M16) ne sont vérifiées
      qu'après les écritures ;
    - hôte ou `member_of` caché (création et mise à jour) : `machines.host.invalid`
      (« does not live », `dead_hosts`), car les règles du module passent avant la
      révélation des désignations cachées ; hors de portée : appliqué, aucun droit n'étant
      exigé sur l'hôte, si bien que l'existence se lit quelle que soit la règle ;
    - vu au code seulement, non testé : `host_kinds` dirait le type d'un hôte illisible
      (`machines.host.invalid` « une machine virtual ne peut pas être hébergée par une
      machine container ») ;
    - sans fuite : interface sur une machine cachée, VLAN d'une interface (création et
      mise à jour), interface d'un attachement
  - options :
    - A. Une référence par identifiant exige le droit de lire son objet (`<type>.read`),
      vérifié avant les écritures (hors objets que le changeset crée) : ferme tout,
      l'oracle des hôtes compris ; change le modèle des droits (spec 005, M16 de la
      spec 009 généralisée), l'amendement 17 de l'ADR 0026 et REQ-002-12 ; héberger une
      machine sur un hôte qu'on ne lit pas devient refusé
    - B. Modèle des droits inchangé : les lectures exigées par un module sur des objets
      qui existent déjà passent avant les écritures (comme la 29b2 pour les objets clos ou
      changés) ; une affectation dont la valeur est une désignation cachée n'est pas
      contrôlée à l'écriture (le changeset est refusé plus loin de toute façon) ; les
      désignations cachées sont révélées avant les règles des modules. Ferme les deux
      premières fuites et donne `core.reference.unknown` pour l'hôte ; l'oracle des hôtes
      reste (hors de portée appliqué, inconnu refusé) ; précision de l'amendement 17
    - C. Droits des objets créés avant les écritures, leur portée tirée du plan (ancres,
      parent) : le plus gros changement (pipeline, spec 005, ADR 0026)
    - recommandation de l'agent : B dans cette tâche, et A pour les seuls hôtes et grappes
      (`machines.read`, `machines.cluster.read` sur l'hôte et la grappe, extension de M16)
      comme décision de la spec 009
    - arbitrage de l'utilisateur (2026-11-15) : B, et A limitée aux hôtes et aux grappes
      (désigner un hôte ou une grappe par `host` ou `member_of` exige le droit de les
      lire, vérifié avant les écritures, hors objets que le changeset crée) ; le modèle
      des droits reste inchangé ailleurs pour la v0.2 ; A généralisée est retenue pour
      plus tard, après la v0.2 (tâche 31)
  - fait le 2026-11-15 : spec d'abord (REQ-002-12, précision du 2026-11-15 ; ADR 0026,
    amendement 17, précision du 2026-11-15 ; 009/M16 étendue, REQ-009-18 et son
    acceptation, design § Droits). Pipeline (`stages`) : les lectures exigées par un
    module (`ModuleHandler::reads`) sur les objets qui existent passent avant les
    écritures, avec les droits sur les objets clos ou changés
    (`rights::check_guards`, `Phase::Before`), celles sur un objet que le changeset crée
    (`resolution::created`) après ; une valeur d'affectation qui nomme une désignation
    cachée n'est pas contrôlée à l'écriture (`resolution::parse_hiding`) ; les
    désignations cachées sont refusées sitôt les droits accordés, avant les sections et
    les règles des modules. `kartenn-machines` : `host.machine`, `host.cluster` et
    `member_of` exigent `machines.read` / `machines.cluster.read`. Test
    `no_right_reaches_through_a_hidden_reference` vert, sans `#[ignore]` : 15 cas (après relecture) d'une
    opération (dont hôte d'un autre type hors de portée, `host_kinds` ; hôte grappe ;
    `member_of` à la création et à la mise à jour), 3 requêtes de plusieurs opérations
    (mêmes règles, messages, opérations, nombres, dans le même ordre), hôte et grappe
    créés dans le changeset acceptés
  - choix : les champs soumis au droit de lire sont une liste (`READ_CHECKED` de
    `handler.rs`), relevés par une enveloppe de `resolve` dans `ModuleHandler::plan` :
    la tâche 31 n'aura qu'à l'étendre ; la valeur cachée d'une affectation est écrite
    sans contrôle plutôt que sautée (sautée, `core.owner` manquant répondait
    `prop.assignment.required`), le changeset étant refusé de toute façon ; les gardes
    `Expect` restent vérifiées après les écritures (hors de l'arbitrage) ; un auteur à
    qui manquent la lecture d'un objet cité et l'édition d'un objet créé ne lit plus que
    la première (le changeset s'arrête avant les écritures) : le test
    `label_mapping_needs_kube_settings` accorde la lecture de l'espace de noms pour
    garder sa preuve de `kube.settings.edit`
  - reste : sous B, le site (`site` de `create_machine`, `create_cluster`,
    `update_cluster`) reste un oracle d'existence (hors de portée appliqué, caché
    refusé par `core.reference.unknown`), comme toute référence sans droit exigé ; fermé
    par la tâche 31. `dead_hosts` et `host_kinds` ne voient plus d'hôte caché, laissés
    tels quels
  - corrigé après relecture (2026-11-15) : la machine qui porte une interface créée
    (`machine` de `create_interface`) entre dans `READ_CHECKED` (`machines.read`,
    décision de la session principale) : hors de portée, une interface sous un nom pris
    répondait `machines.interface.duplicate` à l'écriture (cas
    `interface.machine.taken`, rouge d'abord). Autres champs relus : les `id`
    d'`update_*` et l'`interface` d'un rattachement sont les objets écrits, leur droit
    passe déjà avant les écritures. Tests ajoutés : en validation
    (`a_validation_refuses_alike_a_host_out_of_reach_or_closed`) ; machine ramenée chez
    l'auteur puis désignée comme hôte (`a_machine_brought_back_hosts_from_the_next_changeset`)
  - reste (relecture) : la lecture d'un objet existant se juge sur sa portée d'avant le
    changeset : ramener une machine chez soi et la désigner comme hôte dans le même
    changeset est refusé (`core.object.forbidden`, `machines.read`), en deux changesets
    accepté ; refus sûr, à revoir avec la tâche 31 si le cas se présente. Le slug d'une
    grappe, unique à tout instant, est refusé à l'écriture (`machines.cluster.duplicate`)
    pour qui ne lit pas la grappe qui le porte : oracle d'unicité, pas une référence, à la
    tâche 31. Les tuples `(changeset, hidden)` des signatures d'`apply`, `assignment` et
    `resolution::assign` : versés à la revue 017-26
- [x] 29c3. Une observation `kube` antidatée ne vise pas un objet qui n'est pas encore en
  vigueur au début de sa période (son objet existant, lu par la complétion, reste hors du
  contrôle, 29b2) — REQ-002-12 (amendée) — verify : test d'intégration rouge d'abord
  (`kartenn-kube`) ; `k-check -p kartenn-kube`
  où : `crates/kartenn-kube/src/handler.rs` (complétion des observations), tests
  `crates/kartenn-kube/tests/it/`
  dépend de : 29b1
  - fait le 2026-11-15 : aucun code changé, aucun test rouge trouvé. La complétion
    (`Kube::prepare`) et le contrôle des désignations lisent à la même date, le début de
    la période propre de l'opération (`pipeline::prepare_operation` et
    `unknown_designations`, `own.or(period)`), et chaque lecture de la complétion
    (`pg::named`, `namespaced`, `namespaced_closed`, `pod_domain`, domaines, sélection,
    adresses) prend cette date. Test `clusters::an_antedated_batch_reaches_nothing_not_yet_in_force`
    (lot du connecteur dans un `Within` qui commence après la grappe et son espace de
    noms, avant le reste) : clôture et absence d'un nœud, clôture d'une charge, d'un
    service, d'une entrée créés après ce début : `kube.operation.invalid` (rien à clore) ;
    leur `upsert` crée un autre objet, refusé par `kube.{node,workload,service,ingress}.duplicate`
    (le nom est pris sur la période) ; la machine d'un nœud créée après :
    `core.reference.unknown` (`detail.field` `machine`) ; une grappe créée après :
    `kube.operation.invalid` ; chaque fois, aucune ligne changée (versions des tables du
    module, `module_object`, préfixes, adresses, affectations)
  - reste : une grappe absente au début de la période (créée plus tard, inconnue ou
    close) est refusée par le module (`kube.operation.invalid`, « is not a living
    cluster », `plan_sync` sans domaine de pods), avant les droits et avant le contrôle
    des désignations, et non par `core.reference.unknown` (amendement 17) : à la 29d
- [ ] 29d. Références des opérations de `kartenn-kube`, `kartenn-flow` et `kartenn-policy`,
  typées par la 29b1 : tests de refus et cas particuliers (champs de `Fields` dans
  `kartenn-policy`) — REQ-002-12 (amendée) —
  verify : une référence inconnue par module : 422 `core.reference.unknown` ; `k-check`
  où : `crates/kartenn-kube/src/handler.rs`, `settings.rs` ;
  `crates/kartenn-flow/src/handler.rs` ; `crates/kartenn-policy/src/handler.rs`,
  `matrix.rs`, `objects.rs`, `infer.rs`
  dépend de : 29b1
  - constat de la 29c3 : la grappe d'un `sync` absente au début de sa période (inconnue,
    close ou créée plus tard) est refusée par `plan_sync` (`kube.operation.invalid`,
    « is not a living cluster », la complétion n'ayant pas trouvé son domaine de pods),
    à la planification, avant les droits et le contrôle des désignations ; l'amener à
    `core.reference.unknown` sans dire l'absence à qui ne lit pas la grappe (INV-63)
    touche l'ordre du pipeline, comme les options de la 29c2
  - découpée le 2026-11-16 (33 types d'opération dans trois crates, chacun son harnais,
    son parcours et son contrôle de fuite ; la 29c, huit types, s'était déjà découpée
    en trois) en 29d1 à 29d4 ; se coche avec la dernière.
  - [x] 29d1. `kartenn-kube`, hors la grappe d'un `sync` : parcours de `create_cluster`,
    `update_cluster`, `sync`, `set_label_mapping`, `set_admitted_values`,
    `set_tracked_labels`, une référence inconnue par champ : 422
    `core.reference.unknown` ; fuites pour un auteur sans droits (INV-63) — verify :
    `k-check -p kartenn-kube`
    - où : `crates/kartenn-kube/src/handler.rs` (lignes 226 à 270, 393 à 435),
      `settings.rs` (517 à 600) ; tests `crates/kartenn-kube/tests/it/{clusters,settings}.rs` ;
      modèle `crates/kartenn-machines/tests/it/machines.rs` (2833 à 2862)
    - fait le 2026-11-16 : deux refus du plan, avant les droits, comme celui de
      `plan_sync` (29d2), rouges d'abord : la fermeture d'une correspondance
      (`set_label_mapping`, `close`) qui ne désigne aucune correspondance en vigueur et les
      valeurs admises d'un espace de noms qui n'en est pas un
      (`kube.operation.invalid`, « is not a living label mapping / namespace ») ; un
      administrateur recevait ce refus au lieu de `core.reference.unknown`, un auteur sans
      lecture de l'espace de noms le recevait pour un espace inconnu ou clos, et
      `core.object.forbidden` pour un espace hors de portée. `settings.rs` : le plan ne
      refuse plus, le pipeline juge la désignation (révélée avant les écritures, sinon le
      droit de lire l'espace de noms avant les écritures) ; une correspondance absente a un
      plan qui n'écrit rien, si bien qu'aucune écriture ne dit son absence ; la lecture de
      l'espace de noms par la complétion, devenue inutile, est retirée. Tests nouveaux
      `tests/it/references.rs` : `every_reference_of_every_operation_is_checked` (un
      exemple par type, `set_tracked_labels` sans référence ; chaque `Ref::Id` refusé, rien
      d'écrit), `the_references_designate_objects_in_force` (domaine qui est un préfixe ou
      clos, préfixe qui est un domaine, grappe de machines qui est une machine, propriétaire
      qui est un domaine, grappe mise à jour qui est une grappe de machines ou close,
      machine d'un nœud qui est une grappe ou close, espace de noms qui est une grappe ou
      clos, correspondance qui est un espace de noms ou close),
      `no_right_reaches_through_a_hidden_reference` (dix cas : domaines, préfixes, grappe de
      machines, propriétaire, `id` et domaine d'une mise à jour, machine d'un nœud, espace
      de noms, correspondance, site). Attentes changées dans `settings.rs`
      (`settings_written_by_changeset` : trois refus passent de `kube.operation.invalid` à
      `core.reference.unknown`)
    - choix : une correspondance absente reçoit un plan `noop` plutôt qu'une fermeture
      (la fermeture d'un identifiant absent échouait à l'écriture, autre réponse) ; le
      groupe du connecteur reste hors du parcours (groupe du RBAC, 29b1) ; les objets
      « d'un autre type » des cas de fuite sont hors de portée de l'auteur : un objet qu'il
      lit (son entité, sa grappe) lui révèle `core.reference.unknown`, le droit de lire
      étant jugé sur l'identifiant quel que soit son type (29a), sans rien lui apprendre
    - reste : la correspondance est fermée par qui détient `kube.settings.edit` sans la
      lire (lecture `kube.cluster.read`, objet global) : appliquée en vigueur, refusée
      absente, un oracle comme le site (tâche 31) ; le site reste un oracle (29c2, tâche 31)
  - [x] 29d2. (arbitrée : option A) La grappe d'un `sync` absente au début de sa période :
    `plan_sync` (`handler.rs` 1597 à 1606) refuse à la planification
    (`kube.operation.invalid`), avant les droits ; un connecteur d'une autre grappe
    distingue ainsi une grappe absente d'une grappe vivante qu'il ne conduit pas
    (INV-63). (A) le champ `cluster` d'un `sync` exige `kube.cluster.read` avant les
    écritures, comme `host` et `member_of` depuis la 29c2, et `plan_sync` n'échoue plus
    sur une grappe absente — les rôles de connecteur doivent accorder cette lecture sur
    leur grappe ; (B) un plan vide pour une grappe absente, refusé après les droits —
    touche le contrat de module et le pipeline (ADR 0026) ; (C) renvoyer à la tâche 31 —
    verify : test rouge d'abord, mêmes octets et statut pour absente et hors d'atteinte
    - où : `crates/kartenn-kube/src/handler.rs` (`plan_sync`, `READ_CHECKED` à créer sur
      le modèle de `kartenn-machines/src/handler.rs`)
    - arbitrage de l'utilisateur (2026-11-16) : option A, le champ `cluster` d'un `sync`
      exige `kube.cluster.read` avant les écritures ; les rôles de connecteur accordent
      cette lecture sur leur grappe (rôles fournis, documentation et tests à suivre).
    - versé par la revue de la 29d1 (mineurs, 2026-11-16), à faire avec cette tâche :
      fuites non couvertes par champ dans `crates/kartenn-kube/tests/it/references.rs`
      (`service_domain`, `external_domain`, `service_networks` en création ; `owner`,
      `site`, `pod_domain`, `external_domain`, `pod_containers`, `service_networks`,
      `machines_cluster` en mise à jour) ; `admitted.namespace` avec `values: []` et
      avec un namespace hors d'atteinte portant déjà des valeurs admises (le plan
      diffère, `settings.rs` 612 à 621 : l'égalité des réponses n'y est pas prouvée) ;
      la comparaison des cas sans droit requis sur le message et l'indice, pas la règle
      seule ; un test par HTTP du statut et des octets (aucun test de `kartenn-kube`
      n'y passe) ; inscrire dans la tâche 31 la correspondance de labels close sans
      lecture (`kube.settings.edit` : réelle close, absente refusée)
    - fait le 2026-11-17, rouges d'abord (le plan refusait `kube.operation.invalid`,
      « is not a living cluster ») : `handler.rs`, `READ_CHECKED = ["cluster"]` et le
      relais de `resolve` dans `plan` sur le modèle de `kartenn-machines` ; le champ
      `cluster` d'un `sync` demande `kube.cluster.read` avant les écritures. `plan_sync`
      ne refuse plus une grappe absente : plan `withheld` qui n'écrit rien (créé, donc sans
      droit d'édition demandé avant les écritures), avec les lectures des champs de
      l'upsert (machine d'un nœud) résolus quelle que soit la grappe ; le pipeline juge la
      désignation (`core.reference.unknown`, révélée à qui lit la grappe, sinon après le
      refus de la lecture). Grappe vivante : les refus du plan qui lisent son contenu
      (espace de noms absent, objet d'un nom absent, sélection, domaines) passent au même
      plan `withheld` et sont dits par `apply`, une fois les droits accordés (même règle,
      même message, même opération). Tests : `references.rs`, échantillon `sync` à grappe
      inconnue dans `every_reference_of_every_operation_is_checked` ; trois cas de grappe
      (un espace de noms, close, créée plus tard) dans
      `the_references_designate_objects_in_force` ; dans
      `no_right_reaches_through_a_hidden_reference`, la grappe d'un `sync` (connecteur qui
      lit les grappes d'`acme` : inconnue, d'un autre type, close, créée plus tard ; connecteur
      de `prod` qui ne la lit pas, pour sa propre grappe) et les mineurs versés (création :
      `service_domain`, `external_domain`, `service_networks` ; mise à jour : `owner`,
      `site`, `pod_domain`, `external_domain`, `pod_containers`, `service_networks`,
      `machines_cluster` ; `admitted.namespace` avec `values: []` et sur un espace hors
      d'atteinte qui porte déjà des valeurs, changées ou closes), les cas sans droit requis
      comparés sur la règle, le message (identifiants masqués), l'opération et le nombre
      d'objets ; `a_cluster_not_read_answers_over_http_as_an_absent_one` (`POST
      /v1/changesets`, statut et octets égaux pour inconnue, close, créée plus tard et
      vivante non lue, une autre ou la sienne). Attente changée dans `clusters.rs`
      (`an_antedated_batch_reaches_nothing_not_yet_in_force` : la grappe pas encore en
      vigueur passe de `kube.operation.invalid` à `core.reference.unknown`, champ
      `cluster`). Rôles et documentation : aucun rôle de connecteur n'est encore livré
      (`kube_connector`, tâche 019-11) ; X10 et l'ADR 0044 lui donnent déjà
      `kube.cluster.read`, mais sa liaison aux seuls domaines désignés n'atteint pas le
      cluster, ancré dans aucun domaine : ADR 0044 (conséquences), design 019 (« Droits,
      rôles et audit ») et tâche 019-11 (« à garder ») le disent ; les rôles de connecteur
      des tests le tiennent sur l'entité. Correspondance close sans lecture inscrite à la
      tâche 31.
    - choix : la réponse HTTP comparée octet pour octet hors les identifiants propres à la
      requête (`instance` et `changeset` du problème, neufs à chaque requête, et la grappe
      que l'auteur a écrite, nommée dans `subjects`) ; le refus différé garde sa règle par
      son identifiant (`KUBE_RULES`, `kube.operation.invalid` à défaut) ; tout refus du
      plan après la résolution de la grappe est différé, syntaxe comprise, plutôt qu'une
      liste des seuls refus qui lisent la grappe (plus simple à tenir, même réponse pour
      qui la lit)
    - corrections de la revue (2026-11-17), rouges d'abord : (1) le `sync` d'une grappe
      que le même changeset crée (`Ref::Local`, que la complétion ne lit pas) était
      accepté sans rien écrire ; refusé net à la planification
      (`kube.operation.invalid`, « a cluster created by the same changeset is not synced
      by it »), et le plan `withheld` porte toujours un refus, dit à l'écriture, si bien
      qu'il n'est jamais appliqué ni compté parmi les objets écrits
      (`a_sync_of_a_cluster_created_by_its_changeset_is_refused`) ; (2) le refus de plus
      pour un objet qui existe sous un nom universel (`default`) d'une grappe vivante non
      lue est fermé : `rights::check_before` (appelé par `stages` à la place de
      `check_objects` puis `check_guards`, phase `Before`) écarte des droits d'édition les
      opérations dont une lecture exigée par leur module est refusée, ce refus sortant
      seul pour elles ; tests HTTP `a_cluster_not_read_answers_over_http_as_an_absent_one`
      (nom neuf et `default`) et, pour `kartenn-machines`,
      `a_host_not_read_answers_alone_and_as_an_absent_one` (`rest.rs` : hôte non lu,
      vivant, inconnu ou clos, la lecture seule sans `machines.edit`) ; (3) refus différés
      pour un auteur sans lecture de la grappe (workload dans un espace de noms absent,
      `close` d'un objet absent, action inconnue) comparés à ceux d'une grappe absente
      dans `no_right_reaches_through_a_hidden_reference` ; (4) `told` ne rend jamais un
      message vide (test unitaire `a_withheld_refusal_is_told_with_its_message`), l'upsert
      lit ses colonnes de leur seule présence (plus d'`expect`), ADR 0044 sans suggestion
      de liaison. Attente changée : l'espace de noms lu d'un autre type passe de `payments`
      à `away` (hors d'atteinte), un objet lu étant désormais nommé par le refus (29d4).
    - reste (par lecture, non testé) : l'upsert d'un service qui reprend une adresse
      qu'un autre détenteur de la grappe cite (`owned.adopted`) demande aussi
      `core.address.read` sur elle ; lecture d'un module comme celle de la grappe, elle
      sort avec elle (deux refus de lecture) quand la grappe vit, un seul quand elle est
      absente. Le fermer demande de ne dire, d'une opération, que le refus de la lecture
      qui la porte (la grappe) : un ordre entre les lectures d'un module, contrat de
      module (ADR 0026), à trancher avec la tâche 31
  - [x] 29d3. `kartenn-flow` : parcours de ses cinq types (extrémités, service, entité,
    identifiant de flux) et fuites — verify : `k-check -p kartenn-flow`
    - où : `crates/kartenn-flow/src/handler.rs` (`resolved` 215 à 221, `identity_changed`
      283 à 291, `plan_flow` 502 à 593, `plan_update` 595 à 708, `apply_plan` 852 à 854),
      `tests/it/references.rs`
    - fait le 2026-11-16 : deux fuites de `update`, rouges d'abord. Le plan comparait
      l'identité donnée (extrémités, service, type) à celle du flux lu par la complétion,
      sans droits : `flow.update.identity` avant les droits disait à un auteur sans droit
      sur un flux qu'il existe, et l'égalité servait d'oracle sur ses extrémités, son
      service et son type ; et il ne résolvait ces références que pour un flux existant,
      si bien qu'une extrémité close qu'il lit lui était refusée (`core.reference.unknown`,
      révélé avant les droits) pour un flux existant seulement. `handler.rs` : les
      références de l'identité sont résolues que le flux existe ou non ; un changement
      d'identité ne planifie plus rien (ni prolongation ni instruction) et `apply` le
      refuse (`WriteError::Rule`), après les droits. Tests nouveaux de
      `tests/it/references.rs` : `every_reference_of_every_operation_is_checked` (chaque
      champ de chaque type, dont l'identité d'un `update` d'un flux en vigueur),
      `the_references_designate_objects_in_force` (onze cas : clos, d'un autre type),
      `no_right_reaches_through_a_hidden_reference` (quinze couples opération × champ,
      plus le type d'un `update`, comparés sur règle, message, indice, objets et détail,
      l'identifiant cité masqué), `a_hidden_flow_answers_the_same_bytes` (`POST
      /v1/changesets` d'un `waive` : statut et octets égaux pour un flux inconnu, d'un
      autre type, clos et hors d'atteinte),
      `the_extension_of_a_flow_out_of_reach_answers_as_an_absent_one` (prolongation d'un
      flux daté en vigueur hors d'atteinte, dont le plan ajoute l'ancre `core.owner` sur
      son entité : entité hors d'atteinte, entité lue par l'auteur et close en base — le
      service refuse de clore une entité qui ancre un flux en vigueur,
      `core.anchor.in_use`, l'état ne naît que d'une clôture concurrente, tâche 30 —,
      identifiant d'un autre type, même réponse qu'un flux absent ; vert sans correction : l'objet de
      l'ancre est le flux que le plan écrit, exempté du contrôle des désignations,
      `pipeline.rs` 1368, et la valeur de l'entité n'est pas une désignation ; les droits
      sur le flux refusent avant l'écriture de l'ancre)
    - choix : les objets créés par la soumission (le flux d'un `request`) et les
      identifiants du changeset refusé et du problème, neufs à chaque appel, sont masqués
      dans les comparaisons ; les extrémités et le service d'un `request` (F12) et le
      service d'un `register` ne demandent aucun droit : hors d'atteinte, l'opération est
      acceptée, absente, refusée (`core.reference.unknown`) — un oracle laissé à la tâche
      31, plus large que le site pour les extrémités d'un `request` (le flux accepté est
      relu par son demandeur, valeurs et évaluations comprises : voir la tâche 31) ; seules les réponses absente, d'un autre type et close y sont
      comparées entre elles
    - reste : la prolongation d'un flux terminé, refusée par le plan avant les droits
      (`flow.update.ended`), test rouge `an_ended_flow_answers_as_an_absent_one` ignoré
      (29d5) ; un oracle antérieur à la 29d3, que la comparaison après les droits laisse :
      qui détient `flow.edit` sur un flux sans lire ses adresses (ni `flow.read`) donne une
      source : égale à la sienne, la mise à jour passe ; différente, en vigueur ou non,
      `apply` la refuse (`flow.update.identity`, `handler.rs` 646 et 852) avant le refus
      des désignations cachées (`core.reference.unknown`, `pipeline.rs` 3018) : il
      apprend, identifiant par identifiant, la source du flux qu'il édite — à trancher avec la tâche 31 (lecture exigée des références) ou en
      exigeant la lecture des extrémités pour `update` comme pour `register` (modèle de
      droits)
  - [ ] 29d7. Restes de `check_before` (revue ciblée de la 29d2, 2026-11-17,
    `crates/kartenn-service/src/rights.rs` 243 à 284, `pipeline.rs` 2867 à 2886) :
    (1) coût sur le chemin commun, à mesurer au banc (`bench-core`, `bench-http`, lot de
    connecteur de 500 opérations, import de 10⁴) : `kept` clone tous les `Planned` même
    sans lecture refusée, et chaque permission de lecture est autorisée deux fois
    (`check_before` puis `check_guards`) ; (2) `zip(origins)` tronque en silence si les
    longueurs divergent, ce qui sortirait des éléments du contrôle d'édition sans
    refus : une assertion, et `authored_origins` dérivé de `authored` plutôt que
    refiltré à part ; (3) tests manquants : plusieurs opérations (une non lue, une
    refusée à l'édition sur un autre objet ou sur le même), revalidation avec
    opérations de revue jointes ; (4) à arbitrer : l'upsert d'un service kube dont
    l'adresse est déjà citée dans une grappe vivante non lue rend deux refus de lecture
    (`kube.cluster.read`, `core.address.read`) contre un pour une grappe absente
    (INV-63, étroit : il faut l'identifiant de la grappe et une adresse tenue) — un
    ordre entre les lectures d'un module (contrat de module, ADR 0026 ; l'argument
    n'est pas acquis, à établir) ou la tâche 31 ; (5) `told` de `kartenn-kube`
    (`handler.rs` 1575) recopie le JSON interne du refus dans une violation visible —
    verify : test rouge d'abord par point ; banc par la session principale
    - où : `crates/kartenn-service/src/{rights.rs,pipeline.rs}`,
      `crates/kartenn-kube/src/handler.rs`, `crates/kartenn-kube/tests/it/references.rs`
    - dépend de : 29d2
    - fait en partie le 2026-11-17, avec la 29d6 : (2) `check_before` prend
      `(planned, origins)` et refuse (`assert_eq!`) des longueurs différentes ;
      `authored_origins` naît du même filtre que `authored` (`pipeline.rs`, `unzip`) ;
      (3) `tests/it/module_reads.rs` nouveau (le module de test cite une adresse par
      `update_thing.reads`) : `an_unread_operation_leaves_the_refusals_of_the_others` (la
      lecture seule ; plus une opération refusée à l'édition sur une autre chose, puis sur
      la même : son refus reste), `a_revalidation_with_joined_operations_keeps_each_refusal`
      (proposition revalidée, opération de revue jointe, l'auteur sans l'édition ni la
      lecture de l'adresse : les deux refus, rien d'écrit) — verts sans correction ;
      (5) `told` ne recopie plus le refus stocké (`a_withheld_refusal_is_told_with_its_message`,
      rouge d'abord). Restent (1), banc par la session principale, et (4), à arbitrer
    - choix : les tests de (3) dans `kartenn-service` plutôt que `kartenn-kube`, sans
      opération de revue : le module de test en a une
  - [x] 29d6. Restes du refus de droits qui ne nomme que ce que l'auteur lit (revue
    ciblée de la 29d4, 2026-11-17, `crates/kartenn-service/src/rights.rs` `named`) :
    (1) le message garde le compte des objets refusés, cachés compris : sur les effets
    de propriété (`check`), un auteur qui écrit une propriété sur un préfixe lu apprend
    combien d'objets non lus elle atteint (INV-63) ; (2) la raison stockée d'un
    changeset retenu (`StoredReason.objects`, `approval_self_missing`,
    `pipeline.rs` 3125 et 3311), servie à l'auteur à la soumission et au rejeu, garde
    tous les objets : un auteur qui édite une règle sans la lire en reçoit
    l'identifiant (gravité faible : il faut l'édition sans la lecture) ; (3) à
    vérifier : `forbidden(validator, denied)` (`pipeline.rs` 3297) rend au validateur
    tous les objets qu'il ne peut appliquer, lus ou non ; (4) à vérifier : un éditeur
    de politique lié par portée qui vide une cellule vide reçoit le refus des droits
    au lieu du 422 « the cell is empty » (`kartenn-policy/src/matrix.rs` 488,
    identifiant tiré, `creates=false`) ; (5) `kinds_in_force(&denied, None)` juge à
    maintenant et ignore la période du changeset ; mineurs : doc d'`edit_permission`,
    étiquette `policy.matrix.set` en dur, clé `"empty"` sans constante — verify : test
    rouge d'abord par point confirmé ; séparer les objets montrés de ceux que la revue
    emploie touche le pipeline : s'arrêter et rapporter si c'est le cas
    - où : `crates/kartenn-service/src/{rights.rs,pipeline.rs}`,
      `crates/kartenn-policy/src/matrix.rs`, `crates/kartenn-store-pg/src/idempotency.rs`
    - dépend de : 29d4
    - fait le 2026-11-17 : rouges d'abord, `kartenn-service` `tests/it/rights.rs`. (1)
      confirmé : `rights.rs` `refusal` nomme et compte les seuls objets que le sujet lit
      (`read_among`, ex-`named`), message `N object(s) need <p>`, ou `objects you do not
      read need <p>` quand il n'en nomme aucun, pour les effets, les objets et les lectures
      (`a_refusal_counts_only_the_objects_its_author_reads` : une adresse cachée ou deux,
      même refus) ; (2) confirmé : `approval_self_missing` (`pipeline.rs`), servie à
      l'auteur et stockée pour le rejeu, nomme ce qu'il lit ; les manques de qui applique
      (`unapplied`, dont la revalidation décide) et `required` gardent tous les objets
      (`the_reason_a_changeset_is_held_for_names_only_what_its_author_reads`, raison
      stockée et droits requis lus en base) ; (3) confirmé : `forbidden` nomme et compte ce
      que lit le validateur, `denied` décidant toujours du refus
      (`a_validator_is_told_only_the_objects_he_reads`) ; (4) confirmé, gardé : l'éditeur
      lié par portée reçoit le refus de ses droits pour une cellule vide comme pour une
      cellule tenue hors de sa portée et un type de flux absent ; le 422 lui dirait quelles
      cellules portent une règle (`kartenn-policy` `references.rs`
      `a_scoped_editor_empties_an_empty_cell_as_a_held_one`, vert) ; (5) confirmé :
      `read_among` juge au début de la période du changeset (`window`), passé à
      `check_objects`, `check_guards`, `check_before`
      (`a_refusal_names_the_objects_in_force_at_its_period`). Mineurs : doc
      d'`edit_permission` rendue à sa fonction, `matrix.rs` clé `EMPTY` dont la valeur est
      l'étiquette de l'opération. Attentes changées (message) : `kartenn-kube`
      `clusters.rs`, `kartenn-machines` `machines.rs` et `rest.rs`
    - corrigé après revue (2026-11-17), rouges d'abord : (a) `Violation::withheld`
      (`kartenn-core`, `Box<[Uuid]>`, ni répondu ni journalisé) garde les objets refusés
      non nommés ; `audit.rs` `denial` cible `objects()`, tous les objets refusés
      (REQ-012-8, 10 ; régression de la 29d4 pour l'édition) ; la validation décide sur
      tous (`objects()`) ; la raison stockée garde les seuls nommés
      (`the_audit_of_a_refusal_targets_what_its_answer_does_not_name`, cibles de
      `a_validator_is_told_only_the_objects_he_reads`) ; seule la consultation du journal
      d'audit (`core.audit.read`) lit ces cibles ; (b) ce que nomment la raison et
      `forbidden` est jugé quand le refus est trouvé (avant les écritures pour les objets
      que le changeset ferme), plus après : `approval_self_missing` redevient une fonction
      sans lecture (`the_reason_names_an_object_the_changeset_closes`,
      `a_validator_is_told_an_object_the_changeset_closes`) ; (c) `check_before` rend une
      `StoreError` interne au lieu de paniquer (`aligned`, test unitaire
      `origins_out_of_step_are_an_internal_error`)
    - choix : le message d'un refus qui ne nomme rien ne compte rien (`objects you do not
      read need <p>`) ; les manques de qui applique comptent dans leur message ce qu'ils
      nomment, les autres objets dans `withheld` ; les objets d'une revue que refuse le
      validateur, qui existent dans la transaction, sont jugés après les écritures
    - reste : un refus `policy.admin` de qui applique ne nomme pas, dans la raison de
      l'auteur, les politiques d'approbation (hors `kinds_in_force`), gardées pour l'audit ;
      coût : une lecture de plus par refus d'application (banc de la 29d7, point 1)
  - [ ] 29d5. (à arbitrer) La prolongation d'un flux terminé (F20) : `plan_update`
    (`crates/kartenn-flow/src/handler.rs`, `UPDATE_ENDED`) refuse `flow.update.ended` à la
    planification, avant les droits, sur le flux lu par la complétion : un auteur sans
    droit sur un flux distingue un flux terminé d'un flux inconnu ou hors d'atteinte
    (INV-63). Plus large qu'une existence : le refus ne sort que pour une prolongation,
    une fin donnée postérieure à la fin stockée ou `end: null` (`extends`, `handler.rs`
    299 ; `complete` 353 à 367 ; `plan_update` 677 à 688), une fin antérieure recevant le
    refus de ses droits ; un auteur sans aucun droit qui connaît l'identifiant apprend que
    le flux a existé, qu'il est terminé, et sa date de fin exacte par dichotomie sur
    `end`. Ce poids pèse sur les options : (C) laisse fuir cette date jusqu'après la v0.2
    (la tâche 31 est après elle) ; (A) et (B) la ferment dès leur livraison. Refusée
    après les droits (dans `apply`), la référence du flux terminé, qui
    n'est plus en vigueur au début de la période, est refusée avant par le contrôle des
    désignations : `core.reference.unknown` pour qui le lit, là où REQ-011-4 (critère
    « prolonger un flux `expired` : refusé (`flow.update.ended`) ») et le test
    `acceptance_of_req_011_4_an_ended_flow_is_not_extended` disent `flow.update.ended`.
    (A) amender REQ-011-4 et F20 : un flux terminé est une référence qui ne désigne rien
    en vigueur, `core.reference.unknown`, la règle `flow.update.ended` retirée — dans le
    module seul ; (B) le résolveur d'un module désigne un objet à une autre date que le
    début de la période (la dernière du flux), le refus du module venant après les
    droits — touche le contrat de module (ADR 0026) et le pipeline ; (C) renvoyer à la
    tâche 31 — verify : `an_ended_flow_answers_as_an_absent_one` (ignoré) vert ;
    `k-check -p kartenn-flow`
    - où : `crates/kartenn-flow/src/handler.rs` (`plan_update`, `apply_plan`),
      `tests/it/extensions.rs` (370 à 392), `specs/011-registre-flux/requirements.md` (101)
  - [x] 29d4. `kartenn-policy` : parcours de ses 22 types, dont chaque champ de `Fields`
    (`objects.rs` 26 à 91), `axis.update`, `matrix.set` et `infer.rs` (729 à 820), et
    fuites — verify : `k-check -p kartenn-policy`
    - où : `crates/kartenn-policy/tests/it/rights.rs` et un module de tests nouveau
    - fait le 2026-11-17 : deux fuites, rouges d'abord. (1) `policy.matrix.set` à l'état
      `none` sur une cellule vide était refusé par le plan (`policy.operation.invalid`,
      « the cell is empty »), avant les droits ; un auteur sans aucun droit recevait ce
      refus pour un type de flux inconnu, clos ou d'un autre type, et le refus de ses
      droits pour un type de flux en vigueur dont la cellule porte une règle ; un lecteur
      recevait ce refus au lieu de `core.reference.unknown`. `matrix.rs` (`planned_set`,
      `check_set`) : le même refus, à l'écriture, après les droits vérifiés avant les
      écritures et la lecture du type de flux ; le plan n'écrit rien et n'est jamais
      appliqué. (2) Relevée en revue : le refus des droits nommait en `subjects` les
      objets refusés, dont ceux qu'un plan complète pour l'auteur (la règle d'une cellule
      tenue, la même à chaque soumission ; un identifiant tiré pour une cellule vide) ;
      `kartenn-service/src/rights.rs` (`named`) : un refus du droit d'éditer
      (`check_objects`, `check_guards`, effets de propriétés) ne nomme que les objets en
      vigueur que l'auteur lit, les autres restent comptés dans le message ; les manques
      de qui applique gardent leurs objets, dont l'instruction se sert (REQ-024-11).
      Attentes changées : `kartenn-kube` `clusters::vip_shared_read_required`,
      `kartenn-machines` `machines::a_machine_brought_back_hosts_from_the_next_changeset`
      (objet compté, non nommé) ; `kartenn-service` `tests/it/rights.rs` (`with_objects`
      accorde la lecture des objets dont ces tests comparent la liste). Module nouveau
      `tests/it/references.rs` :
      `every_reference_of_every_operation_is_checked` (26 couples opération × champ sur les
      13 types qui citent par identifiant, le type de flux d'une cellule sous `allow` et
      sous `none`, les 9 autres types listés sans référence ; pour chacun
      un identifiant inconnu, un objet d'un autre type et un objet clos : une seule violation
      `core.reference.unknown` comparée entière, message, opération, `detail` et objets ;
      rien d'écrit, révisions comprises), `no_right_reaches_through_a_hidden_reference`
      (chaque couple, un auteur sans aucun droit : refus de ses droits égal pour l'objet en
      vigueur et les trois cachés, comparé entier, identifiants tirés masqués, aucun objet
      nommé), `a_refusal_of_rights_names_no_object_out_of_reach` (cellule tenue et type de
      flux absent vidés deux fois, objets comparés sans masque),
      `a_writer_who_does_not_read_is_answered_alike` (tous les droits d'écrire sans lire la
      politique, règles d'inférence dans `lab` : refus des droits égaux là où la référence
      en exige — domaine d'une règle d'inférence, règle qu'une mise à jour écrit —,
      ailleurs l'objet en vigueur appliqué et les cachés refusés à l'identique),
      `a_hidden_domain_answers_the_same_bytes` (par HTTP, `POST /v1/changesets` : statut
      422 et octets égaux pour un domaine inconnu, clos et hors d'atteinte, une fois
      masqués l'identifiant donné, le changeset et la requête ; créée dans `lab`) ;
      `tests/it/matrix.rs` : `an_empty_cell_emptied_is_refused_at_its_write` (sans et
      sous `if_match` lu : `policy.operation.invalid` sans objet nommé ; autre empreinte :
      `policy.matrix.stale` ; ni version, ni objet de module, ni révision écrits) ;
      `rest.rs` (tests) : `Env::app` partagé ; spec 010 : choix de la tâche 18 précisé,
      tâche 23 nouvelle
    - choix : le refus d'une cellule vide vidée passe du plan à l'écriture (`check_set`)
      plutôt qu'en plan appliqué sans écriture, qui aurait rangé un objet tiré parmi les
      objets touchés du changeset (`object_version`, sections) ; contrat REST inchangé
      (422) ; la comparaison des réponses masque l'identifiant donné et ceux que la requête
      tire, garde ceux des objets du test ; l'auteur des règles d'inférence est lié au
      domaine `lab` (spec 010, tâche 23)
    - reste : sous `if_match`, un type de flux absent rend la cellule périmée
      (`policy.matrix.stale`) avant les références et les droits, comme un type de flux en
      vigueur dont l'empreinte diffère : pas de fuite sans connaître l'empreinte, mais un
      lecteur reçoit `stale` et non `core.reference.unknown` ; la mise à jour d'un objet de
      la politique caché à un auteur qui l'écrit sans le lire (identifiant inconnu, d'un
      autre type ou clos) est refusée par son écriture (`changeset.operation.no_version`),
      avant les désignations cachées, à l'identique pour les trois : l'oracle d'existence
      de la tâche 31 (renvoi inscrit), mais pas sa règle ; de même une cellule vide vidée
      par un tel auteur, pour un type de flux caché : refusée par son écriture ; les
      références de la politique n'exigent aucune lecture (tâche 31) ; une liaison de
      `policy.inference.admin` sur une entité n'atteint pas les règles d'inférence ancrées
      dans ses domaines (spec 010, tâche 23) ; le motif d'un changeset mis en attente faute
      d'`approval.self` (`approval_self_missing`, objets de `unapplied`) nomme encore tous
      les objets, lus ou non : séparer les objets servis de ceux de l'instruction touche le
      pipeline (`pipeline.rs`, `Pending`, `StoredReason`)
- [ ] 30. Clé étrangère temporelle sur les références du noyau : la base refuse d'elle-même
  une ligne dont la référence ne désigne pas une ligne en vigueur sur toute sa période de
  validité (PostgreSQL 18, `FOREIGN KEY (…, PERIOD …) REFERENCES … (…, PERIOD …)`), en
  commençant par `prefix.domain_id`, `range.domain_id` et `address.domain_id` ; une
  migration nouvelle (jamais une migration existante réécrite), précédée du contrôle des
  orphelins : elle échoue avec leur liste plutôt que de les supprimer. À établir avant
  d'écrire la migration : compatibilité avec le modèle bitemporel (la période de validité
  seule entre dans la clé, pas le temps de transaction), contrainte d'exclusion ou clé
  `WITHOUT OVERLAPS` requise sur la table visée, coût sur le banc `bench-core` (seuils
  D14), clôture d'un domaine qui porte encore des objets (acceptée aujourd'hui : le test
  `audit.rs` ferme un domaine qui porte un préfixe) — un ADR si le modèle doit
  bouger. Course que la 29a laisse ouverte : son contrôle lit les références après les
  verrous du changeset, mais seuls un domaine, les arbres d'entités et de sites et un objet
  d'adressage sont verrouillés par qui les ferme ; une clôture concurrente d'une propriété,
  d'une valeur de liste fermée, de la portée d'une liaison ou d'une politique, d'un objet
  lié à un identifiant externe ou d'un objet de module, validée entre la lecture et la
  validation du changeset, laisse une référence vers un objet clos ; de même une clôture
  d'un domaine qui porte encore des préfixes, que rien ne refuse — REQ-002-12 (amendée) —
  verify : test d'intégration (une insertion directe en
  base, hors service, d'un préfixe sur un domaine inconnu ou clos est refusée par la
  contrainte ; deux transactions, une clôture validée pendant l'écriture d'une référence
  vers l'objet clos, la seconde refusée) ; `k-check` ; `bench-core` réduit sous ses seuils
  où : `crates/kartenn-store-pg/migrations/` (`0004_core_objects.sql:44-61` pour la forme
  des tables) ; `docs/spec/data.md` ; ADR 0014 et 0015 ; `docs/practices/`
  dépend de : 29
- [ ] 31. (après la v0.2, arbitrage de l'utilisateur du 2026-11-15) Le droit de lire exigé
  sur toute référence par identifiant (option A de la 29c2, généralisée) : désigner un
  objet exige `<type>.read` sur lui, vérifié avant les écritures, hors objets que le
  changeset crée ; ferme tout oracle d'existence par une référence, pour tous les modules,
  et remplace l'exception posée par la 29c2 pour les seuls hôtes et grappes. Spec d'abord :
  le modèle des droits (spec 005, M16 de la spec 009 généralisée), l'amendement 17 de
  l'ADR 0026, REQ-002-12 ; à établir : les usages légitimes qui désignent un objet sans le
  lire (propriétaire, site, adresse d'un rattachement, service d'une demande ou d'une
  écriture directe de flux : 29d3) et ce qu'ils deviennent ; à part, les extrémités
  d'une demande de flux (F12 de la spec 011), qui ne sont pas de la classe du site :
  au-delà de l'existence (inconnue ou close : `core.reference.unknown` ; en vigueur :
  acceptée), la demande acceptée crée un flux que son demandeur relit sous
  `flow.request` (`crates/kartenn-flow/src/rest.rs` 10 à 16), servi avec ses extrémités
  par type, identifiant et valeur à la date — l'adresse, le préfixe, la plage, les
  adresses d'une interface ou d'une machine (`ENDPOINT_VALUES`, `rest.rs` 730) — et ses
  évaluations entières (REQ-027-2, `rest.rs` 13 et 14), rendues aussi par la réponse de
  `POST /v1/flows` ; son triage, calculé à la complétion contre la politique sur
  l'extrémité cachée (`handler.rs`, `triage` 448 à 500), décide de l'issue (appliquée,
  201, ou à instruire, 202) : le demandeur apprend la valeur de l'extrémité qu'il ne lit
  pas, le verdict de chaque règle sur elle, donc sa classification par la politique
  (environnement, zone). REQ-011-17 promet une « explication réduite comme
  REQ-010-25 » au demandeur ; le code sert les évaluations entières — verify :
  un auteur sans `<type>.read` sur la cible reçoit des réponses égales pour une cible
  hors de portée, inconnue, d'un autre type et close, pour chaque champ de référence de
  chaque module ; `k-check`
  - versé par la 29d4 (2026-11-17) : la mise à jour d'un objet caché à qui l'écrit sans
    le lire est refusée par son écriture (`changeset.operation.no_version`), avant les
    désignations cachées (`core.reference.unknown`) ; une fois la lecture exigée avant
    les écritures, ce refus ne doit plus pouvoir précéder celui des droits
  où : `crates/kartenn-service/src/pipeline.rs` (`check_designations`, `read_permission`,
  `stages`), `rights.rs` ; `specs/005-*/`, `specs/009-*/` (M16), `docs/adr/` (0026)
  dépend de : 29c2, 29d ; publication de la v0.2
  - versé par la 29d2 (revue de la 29d1, 2026-11-16) : la correspondance de labels
    (`kube.set_label_mapping`, `close`) est fermée par qui détient `kube.settings.edit`
    sans la lire (lecture `kube.cluster.read`, objet global) : réelle, elle se ferme ;
    absente ou close, elle est refusée (`core.reference.unknown`) — un oracle, comme le
    site.

## Différé à d'autres specs (constaté à la synchronisation)

- Restitution de la classification IANA de chaque objet (REQ-002-33) : calculée par
  `SpecialRegistry::classify`, servie par l'API (006).
- Réécriture de l'indicateur `public` après une mise à jour des registres, par un
  changeset système planifié (006).
- Permissions par type d'objet sur les écritures du noyau (005).

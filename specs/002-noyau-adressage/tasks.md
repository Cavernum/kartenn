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
    `no_right_reaches_through_a_hidden_reference` vert, sans `#[ignore]` : 14 cas d'une
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
  lire (propriétaire, site, adresse d'un rattachement) et ce qu'ils deviennent — verify :
  un auteur sans `<type>.read` sur la cible reçoit des réponses égales pour une cible
  hors de portée, inconnue, d'un autre type et close, pour chaque champ de référence de
  chaque module ; `k-check`
  où : `crates/kartenn-service/src/pipeline.rs` (`check_designations`, `read_permission`,
  `stages`), `rights.rs` ; `specs/005-*/`, `specs/009-*/` (M16), `docs/adr/` (0026)
  dépend de : 29c2, 29d ; publication de la v0.2

## Différé à d'autres specs (constaté à la synchronisation)

- Restitution de la classification IANA de chaque objet (REQ-002-33) : calculée par
  `SpecialRegistry::classify`, servie par l'API (006).
- Réécriture de l'indicateur `public` après une mise à jour des registres, par un
  changeset système planifié (006).
- Permissions par type d'objet sur les écritures du noyau (005).

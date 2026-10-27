# 026 — Le VLAN dans le cœur : tâches

Statut : spec **validée le 2026-10-27**.

- [x] 1. Modèle, règles et noms du cœur : `core.vlan_group`, `core.vlan`, codes
  `core.vlan.*`, permissions et propriété `core.vlan` dans `kartenn-core` — REQ-026-1,
  2 — verify : tests d'acceptation de REQ-008-12 à 18 sous les nouveaux noms ; `k-check`
  - où : `crates/kartenn-vlan/src/{vlan,group,rules,labels}.rs` (à déplacer) ;
    `crates/kartenn-core/src/authz.rs`, `rules.rs`, `code_labels.rs` (objets du cœur à
    imiter : préfixe, adresse)
  - fait : `kartenn_core::vlan` (VID, nom, statut, plage, occupation, prochain VID
    libre) ; groupe de règles `VLAN_RULES` du catalogue du noyau ; permissions
    `core.vlan_group.*` et `core.vlan.*` dans les rôles livrés ; libellés FR/EN et
    sortes `core.vlan_group`, `core.vlan` ; anciens noms dans `RENAMED_PERMISSIONS`
    (`prop.vlan.vlan.*` compris) ; `docs/spec/regles-noyau.md` régénéré.
  - choix : codes selon la grammaire `core.<objet>.<règle>` sous `core.vlan.*` :
    `vid_invalid`, `vid_out_of_range`, `vid_duplicate`, `name_invalid`,
    `name_duplicate`, `group_anchor`, `prefix_not_network`, `site_mismatch`,
    `domain_mismatch`.
- [x] 2. Stockage et migration : tables dans le schéma du cœur, retrait de
  `module_object` et du registre, renommage de la propriété, des rôles et du journal —
  REQ-026-3 — verify : test de migration sur une base peuplée par la 008, seconde
  exécution sans effet
  - où : `crates/kartenn-vlan/migrations/` ; `crates/kartenn-store-pg/migrations/`
    (0071 `approval_self` à imiter) ; `kartenn-service` (`rbac::rename_permissions`)
  - fait : migration `0072_vlan_core.sql` (tables, historiques et partitions déplacés
    par `SET SCHEMA`, ou créés sur une base neuve ; schéma `vlan` supprimé ; lignes de
    `module_object`, de son historique et de `module_migration` retirées ; propriété,
    porteurs, obligations, affectations, identifiants externes renommés déclencheur de
    version suspendu ; journal des changesets, anomalies, brouillons, morceaux d'import
    réécrits ; vues `vlan_tree`, `carrier_object` et `kartenn_site` étendues) ;
    `kartenn_store_pg::vlan` (stockage et gestionnaire) ; test
    `kartenn-rest/tests/it/vlan/migration.rs` (base remise dans l'état de la 008,
    lectures à `valid_at`/`as_of`, rôle `vlan.write` → `core.vlan.edit` +
    `core.vlan.approval.self`, seconde exécution sans effet).
  - choix : réécriture du journal par un parcours JSON (fonctions `pg_temp`, contexte
    du membre pour distinguer la sorte `vlan` de la propriété `core.vlan`) ; l'audit et
    l'outbox restent historiques.
- [x] 3. Opérations, lectures et routes du cœur ; crate `kartenn-vlan` retirée ;
  imports ; statistiques — REQ-026-1, 7 — verify : `k-check`, contrat OpenAPI régénéré
  - où : `crates/kartenn-vlan/src/{handler,pg,rest,stats}.rs` ; `kartenn-service`,
    `kartenn-rest`, `kartenn-server/src/modules.rs`
  - fait : sortes `ObjectKind::VlanGroup`/`Vlan` ; opérations du cœur
    `create_vlan_group`, `update_vlan_group`, `create_vlan`, `update_vlan` (planifiées
    par `kartenn_service::vlan`, documentées dans `Operation`) ; références, cascade,
    droits, héritage le long de l'arbre des VLAN, lectures par identifiant, historique,
    propriétés, verrous ; routes `/v1/vlan-groups`, `/v1/vlans` dans `kartenn-rest`
    (étiquette `core`) ; statistiques : sortes `core.vlan_group`, `core.vlan` et champ
    `vlans` (`reserved`, `deprecated`) ; feature `vlan` du serveur retirée ; crate
    `kartenn-vlan` supprimée, ses tests repris dans `kartenn-rest/tests/it/vlan/` ;
    contrat régénéré ; `docs/spec/permissions.md` et `regles-modules.md` régénérés.
  - choix : les opérations du VLAN passent par le même contrat de gestionnaire que
    celles d'un module (`ModuleHandler`), fourni par le stockage du cœur
    (`WriteTx::core_handler`) et non par le registre, sous le type qualifié interne
    `core.<sorte>` ; aucun `module_object`, les tables du VLAN servent d'enregistrement.
    Les compteurs `reserved`/`deprecated` quittent `modules.vlan` pour `vlans`. Les
    tâches 2, 3 et 4 forment un seul commit : la migration, le pipeline et le retrait
    du module ne compilent pas séparément.
- [x] 4. Machines, Politique, Flux sur le port du cœur — REQ-026-4, 5 — verify : build
  avec Machines seul ; `k-check`
  - où : `crates/kartenn-machines/src/handler.rs` (`VLAN_READ`, `vlan_columns`) ;
    tests de Politique et Flux qui montent le module VLAN
  - fait : Machines sans dépendance de module (`machines_stands_alone`), lecture
    `core.vlan.read`, tables `public.vlan`, propriété `core.vlan` ; tests de
    Machines, Politique et Flux sans le module VLAN ; bancs à jour.
- [x] 5. Clients, SDK, CLI, shell v1 (plugin toujours servi), bancs (`vlan_budgets`
  dans `bench-core`), `docs/spec/` — REQ-026-2, 6, 7 — verify : `k-py-check`,
  `k-web-check` ; bancs lancés par l'agent principal
  - fait : manifeste du plugin v1 sans `module` (toujours actif), permissions
    `core.vlan.*`, propriété `core.vlan`, clés i18n `field.*` ; SDK v1 régénéré ;
    client Python et CLI (ressources `vlans`, `vlan-groups` du noyau, opérations
    `create_vlan`…, instantanés régénérés), e2e Python ; `docs/spec/` (architecture,
    données, interfaces, invariants, catalogues régénérés). `vlan_budgets` dans
    `bench-core` compile ; bancs non lancés.
  - choix : la CLI range `vlans` et `vlan-groups` dans la section « Noyau » ;
    la résolution d'une référence d'un objet du noyau (`--group`) passe par la
    collection au pluriel (`vlan-groups`) ; l'ADR 0026 et les specs closes gardent
    leurs exemples d'époque. Les compteurs du VLAN de `GET /v1/stats` sont servis
    dans une section `vlans` (`reserved`, `deprecated` ; anciennement
    `modules.vlan`), à reprendre par la maquette E0 qui lit `vlan`.
- [x] 6. Maquettes v2 : écran VLAN du cœur, E10 sans le VLAN, ligne ajoutée au tableau
  des maquettes de la 020 — REQ-026-6 — verify : `k-web2-check`, `k-web2-e2e`
  - fait : maquette E12 « VLAN » (`web/v2/shell/src/_ui/ecrans/E12Vlan.vue`, quatre
    vues : groupes, un groupe, fiche d'un VLAN, création pour un réseau), entrée « VLAN »
    de l'espace Référentiel après Topographie (espace « Réseau » retiré) ; E10 garde la
    matrice seule ; tuile VLAN d'E0, onglet et encart VLAN d'E2, menus d'E1 et E8 sans
    marque de plugin ; noms `core.vlan.*`, `core.vlan_group.*`,
    `core.vlan.site_mismatch` (E4, E7) ; lien vers la fiche d'interface depuis le choix
    d'interface d'E11 ; lignes E0, E2, E10, E12 du tableau de la 020 ; captures.
    `k-web2-e2e ecrans clavier touch layout` : 1015 réussis, axe sans violation.
  - choix : la création d'un VLAN part de la liste des préfixes, dont la colonne VLAN
    (`core.vlan`) est montrée là ; le dashboard lit les compteurs du VLAN dans une
    section `vlan` de `GET /v1/stats` et les sortes `core.vlan_group`, `core.vlan` dans
    `objects` (forme à confirmer par la tâche 3) ; le VLAN reste une seule entrée de la
    barre latérale (les groupes en sont la page d'accueil).
  - SDK v2 régénéré à la fusion (filtre `media` de la 009-15) : `k-web2-check` vert sur
    main (312 tests, 1196 captures).
- [x] 7. Amendements : D18 dans `001-cadrage/decisions.md`, 008 (parties B à F
  « reprises par la 026 »), 009 (REQ-009-12), 021, `specs/README.md`
  - fait : D18 amendée (journal et synthèse) ; 008 : note en tête des exigences et de
    la conception, « Reprise par la 026 » sous B à F ; REQ-009-12 amendée ; 021 :
    écrans VLAN du cœur au lieu du plugin (conception, REQ-021-3, tâche 17, phase 5) ;
    ligne 026 et ligne 008 de `specs/README.md`.
  - choix : les routes v2 des écrans VLAN notées dans la 021 (`/vlans`, `/vlan-groups`)
    suivent les collections de l'API, à aligner sur la maquette E12 au portage ; les specs closes (005, 018, 022, 023, 024)
    et l'ADR 0026 gardent leurs exemples d'époque.

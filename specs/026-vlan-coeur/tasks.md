# 026 — Le VLAN dans le cœur : tâches

Statut : spec **en brouillon**, à valider.

- [ ] 1. Modèle, règles et noms du cœur : `core.vlan_group`, `core.vlan`, codes
  `core.vlan.*`, permissions et propriété `core.vlan` dans `kartenn-core` — REQ-026-1,
  2 — verify : tests d'acceptation de REQ-008-12 à 18 sous les nouveaux noms ; `k-check`
  - où : `crates/kartenn-vlan/src/{vlan,group,rules,labels}.rs` (à déplacer) ;
    `crates/kartenn-core/src/authz.rs`, `rules.rs`, `code_labels.rs` (objets du cœur à
    imiter : préfixe, adresse)
- [ ] 2. Stockage et migration : tables dans le schéma du cœur, retrait de
  `module_object` et du registre, renommage de la propriété, des rôles et du journal —
  REQ-026-3 — verify : test de migration sur une base peuplée par la 008, seconde
  exécution sans effet
  - où : `crates/kartenn-vlan/migrations/` ; `crates/kartenn-store-pg/migrations/`
    (0071 `approval_self` à imiter) ; `kartenn-service` (`rbac::rename_permissions`)
- [ ] 3. Opérations, lectures et routes du cœur ; crate `kartenn-vlan` retirée ;
  imports ; statistiques — REQ-026-1, 7 — verify : `k-check`, contrat OpenAPI régénéré
  - où : `crates/kartenn-vlan/src/{handler,pg,rest,stats}.rs` ; `kartenn-service`,
    `kartenn-rest`, `kartenn-server/src/modules.rs`
- [ ] 4. Machines, Politique, Flux sur le port du cœur — REQ-026-4, 5 — verify : build
  avec Machines seul ; `k-check`
  - où : `crates/kartenn-machines/src/handler.rs` (`VLAN_READ`, `vlan_columns`) ;
    tests de Politique et Flux qui montent le module VLAN
- [ ] 5. Clients, SDK, CLI, shell v1 (plugin toujours servi), bancs (`vlan_budgets`
  dans `bench-core`), `docs/spec/` — REQ-026-2, 6, 7 — verify : `k-py-check`,
  `k-web-check` ; bancs lancés par l'agent principal
- [ ] 6. Maquettes v2 : écran VLAN du cœur, E10 sans le VLAN, ligne ajoutée au tableau
  des maquettes de la 020 — REQ-026-6 — verify : `k-web2-check`, `k-web2-e2e`
- [ ] 7. Amendements : D18 dans `001-cadrage/decisions.md`, 008 (parties B à F
  « reprises par la 026 »), 009 (REQ-009-12), 021, `specs/README.md`

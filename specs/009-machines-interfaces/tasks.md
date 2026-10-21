---
status: draft
updated: 2026-10-20
---

# 009 — Machines et interfaces : tâches

Chaque tâche est un commit, laisse le build vert (`devenv test`, `k-check`) et nomme ce
qu'elle implémente. « verify » = test qui prouve la tâche ; « dépend de » = tâches à
finir avant, ici ou dans la 008. Tests d'intégration et HTTP : comme dans la 008.

Parallélisme : la tâche 1 (domaine pur) démarre tout de suite ; P2 attend la P1 de la
008 (contrat) et le VLAN de la 008 (tâches 11, 12) ; dans P2, 4 et 6 avancent en
parallèle après 3 ; 9 et 10 en parallèle après 8.

## P1 — Domaine pur

- [x] 1. Crate `kartenn-machines` et domaine pur : nom d'hôte (RFC 1123 §2.1, minuscules),
  MAC EUI-48 (entrées `-`, `:`, `.` ; restitution RFC 9542 §1.1 ; bit de groupe), table
  d'hébergement M5 et détection de cycle, modes VLAN 802.1Q, règles (`Rule` statiques) ;
  `workspace-hack` à jour — REQ-009-4, 5, 6, 12 (contrôles locaux) — verify : tests
  unitaires `hostname_labels`, `hostname_lowercased`, `mac_forms_and_canonical`,
  `mac_group_bit_refused`, `hosting_truth_table`, `hosting_cycle`, `vlan_modes`
  où : design « Règles » ; `docs/reference/vlan-mac-ieee-802.md`,
  `docs/reference/noms-dates-texte.md` (noms d'hôte) ; modèle `kartenn-vlan` (008, tâche 9)
  dépend de : —
  — `name.rs` (`HostName`), `mac.rs` (`Mac`, `is_local` pour `machines.mac.duplicate`),
  `hosting.rs` (`MachineKind`, `Host`, `check_host`, `check_membership`,
  `check_no_cycle` sur une fonction « hébergeur de »), `vlan_mode.rs` (`VlanMode::from_parts`,
  `vlans`), `rules.rs` (`MACHINES_RULES`, 13 règles du design, plus le test
  `rule_ids_unique_and_namespaced`) ; hakari : aucun changement ;
  choix : un nom fait de seules étiquettes numériques (`10.1.2.3`, `42`) est refusé, car il
  se lit comme une adresse (RFC 1123 §2.1) ; la MAC refuse les séparateurs mêlés et les
  groupes `.` de deux chiffres ; le parcours de cycle s'arrête à 8 niveaux (refus) pour
  ne jamais boucler sur un état corrompu ; le contrôle « VLAN vivants » reste au
  gestionnaire (tâche 4), le nom d'interface à la tâche 3.

## P2 — Module

- [x] 2. Clusters et machines : migration `0001_machines.sql` (`cluster`, `machine`),
  adaptateur `pg`, gestionnaire `create/update_cluster`, `create/update_machine`
  (affectations `core.owner`/`core.site` planifiées, clés de verrou), unicité du nom par
  propriétaire résolu, hébergement ; module au registre (dépendance `vlan`), feature
  `machines` — REQ-009-1, 2, 3, 4, 5 — verify : `machines_needs_vlan`,
  `name_unique_per_owner`, `owner_change_into_taken_name_refused`,
  `vm_on_cluster_container_on_vm`, `hosting_cycle_refused`,
  `kind_change_with_host_refused`, `cluster_slug_unique`
  où : design « Données », « Opérations » ; gestionnaire VLAN `kartenn-vlan/src/handler.rs`
  (008, tâches 10, 11) comme modèle
  dépend de : 1 ; 008 : 3, 5, 6
  — `kartenn-machines` : `module.rs` (`Machines`, types `machines.cluster` et
  `machines.machine`, permissions, dépendance `vlan`), `handler.rs` (`prepare` : propriétés
  du noyau et propriétaire résolu ; `plan` des quatre opérations, affectations
  `core.owner`/`core.site` planifiées, clés `machines.name:<owner>` et `machines.hosting` ;
  `check` : nom par propriétaire résolu, sorte de l'hébergeur, hébergeur vivant, cycle),
  `pg.rs` (écritures refusées avant d'écrire ce que les contraintes refuseraient),
  migration `0001_machines.sql`, feature `pg` ; serveur : feature `machines` (par défaut,
  active `vlan`), registre ; contrat de référence, types du SDK et `permissions.md`
  régénérés. Tests : `kartenn-machines/tests/machines.rs` (les sept du verify).
  choix : règle `machines.operation.invalid` ajoutée pour une opération mal formée (champ
  inconnu, sorte inconnue, objet introuvable), un corps de module n'ayant pas de refus
  structurel (400) ; un changement de propriétaire par affectation du noyau est relu dans
  le détail du différentiel (`pg_temp.diff_detail`) ; propriétaires comparés maintenant si
  le chevauchement le couvre, sinon à son début ; cycle cherché à l'instant présent ;
  `update_machine` ne porte pas `owner`/`site` (opérations du noyau), `update_cluster` oui.
- [x] 3. Interfaces : table `interface`, `create/update_interface`, unicité du nom par
  machine, MAC, état, héritage des ancres — REQ-009-6, 7 — verify :
  `interface_name_unique_per_machine`, `mac_stored_as_macaddr`,
  `interface_cannot_change_machine`, `owner_change_moves_interface_scope`
  où : design « Types d'objets », « Données »
  dépend de : 2
  — `kartenn-machines` : migration `0002_interface.sql` (table `interface` avec ses
  colonnes 802.1Q, `CHECK` du bit de groupe par `macaddr &`), type `machines.interface`
  (parent : sa machine), permissions, `create/update_interface` (`handler.rs` : nom de 1 à
  64 caractères imprimables sans espace, MAC canonique, clé `machines.machine:<id>`,
  enregistrement sous sa machine dans `module_object` ; `check` : machine vivante sur la
  période), `pg.rs` (nom pris refusé avant d'écrire) ; contrat de référence, types du SDK
  et `permissions.md` régénérés. Tests : `kartenn-machines/tests/machines.rs` (les quatre du
  verify).
  choix : migration `0002_interface.sql` plutôt qu'une réécriture de `0001` déjà livrée par
  la tâche 2 ; colonnes 802.1Q créées dès maintenant (mode `none` par défaut), leurs règles
  restent à la tâche 4 ; nom d'interface mal formé et machine changée refusés par
  `machines.operation.invalid`.
- [x] 4. VLAN d'une interface : modes `none`/`access`/`trunk`, VLAN vivants, droits
  `vlan.vlan.read` — REQ-009-12, 18 (VLAN) — verify : `access_needs_one_vlan`,
  `native_not_in_tagged`, `closed_vlan_refused`, `vlan_read_required`
  où : design « Opérations », « Routes » (droits) ; propriété et type `vlan.vlan` (008)
  dépend de : 3 ; 008 : 11
  — Contrat : `ModuleHandler::reads` (droits sur les objets cités sans être écrits,
  contrôlés comme une garde dans `rights::check_guards`, `core.object.forbidden`, jamais
  en proposition). `kartenn-machines` : `handler.rs` (champ `vlan` `{mode, access,
  native, tagged}` de `create_interface` et `update_interface`, `null` ou absent : `none` ;
  `VlanMode::from_parts` ; colonnes 802.1Q ; `vlan.vlan.read` sur chaque VLAN cité ;
  `check` : VLAN vivant sur chaque version de l'interface), `pg.rs` (`insert_interface`
  depuis les colonnes planifiées, `dead_vlans`). Tests : `kartenn-machines/tests/machines.rs`
  (les quatre du verify).
  choix : droit de lecture d'un objet cité porté par le contrat de module
  (`ModuleHandler::reads`, ajout additif à l'ADR 0026) plutôt qu'un contrôle dans le
  gestionnaire, qui ne connaît pas le demandeur ; un patch de `vlan` remplace tout le
  mode ; un VLAN inconnu est refusé par `machines.vlan.mode` (non vivant).
- [x] 5. Rattachements : table `attachment`, `attach_address`/`detach_address`, unicité
  hors `vip`/`anycast`, recopie de l'usage (`shared`) et réévaluation sur changement
  d'usage, adresse principale par famille, droits M16 — REQ-009-9, 10, 11, 18 —
  verify : `two_concurrent_attachments_one_wins`, `vip_on_two_machines`,
  `usage_back_to_host_refused_when_shared`, `two_primary_v4_refused`,
  `attach_needs_address_read_only`
  où : design « Données » (`shared`), « Opérations », « Routes » (droits) ; usage des
  adresses `kartenn-core/src/objects.rs::AddressUsage`
  dépend de : 3
  — Contrat : `CheckContext::changeset` (le module réécrit sa recopie sous le changeset).
  `kartenn-machines` : migration `0003_attachment.sql` (versionnée, exclusions de la
  conception), `module.rs` (opérations `attach_address`, `detach_address`), `handler.rs`
  (écriture de l'interface, clés `machines.address:<id>` et `machines.interface:<id>`,
  `core.address.read` par `reads`, `apply_attachment`, `check_interfaces`, restauration des
  rattachements à l'annulation), `pg.rs` (`attach` qui refuse avant d'écrire ce que les
  contraintes refuseraient, `attachments`, `dead_addresses`, `attachments_outside`,
  `reshare`). Tests : `kartenn-machines/tests/machines.rs` (les cinq du verify).
  choix : un rattachement écrit l'interface (`machines.interface.write`, nouvelle
  inscription pour l'historique et l'annulation) ; `shared` recopié portion par portion
  des versions de l'adresse (`kartenn_portion`), le refus ne portant que sur une portion
  non partagée tenue par une autre interface ; un rattachement hors de la vie de son
  interface est refusé par `machines.operation.invalid` en attendant les cascades de la
  tâche 8 (une interface ou une adresse close avec des rattachements vivants est refusée).
- [x] 6. Propriétés du module : `machines.interface_role` (valeurs livrées, extensible),
  `machines.fqdn` ; porteurs pour d'autres modules — REQ-009-8, 24 — verify :
  `interface_role_values`, `admin_adds_a_role_value`,
  `foreign_module_property_on_a_machine`
  où : design « Propriétés déclarées » ; `declarations.rs` (`ModuleDeclarations`)
  dépend de : 3
  — `kartenn-machines/src/module.rs` : `Module::declarations` (`machines.interface_role`,
  liste fermée simple portée par les interfaces, six valeurs livrées ;
  `machines.fqdn`, FQDN porté par les machines ; ni héritage ni obligation),
  `INTERFACE_ROLES`. Tests : `kartenn-machines/tests/machines.rs` (les trois du verify,
  module de test `Inventory` portant deux listes fermées sur `machines.machine`) ;
  l'environnement de test synchronise aussi les déclarations des modules.
  choix : rien de propre au module pour REQ-009-24, les types de module étant porteurs
  admissibles (008-5) ; une valeur ajoutée par l'administrateur survit à la
  synchronisation (vérifié par le test).
- [ ] 7. Cohérence : `machines.mac.duplicate`, `machines.address.vlan_mismatch`,
  `machines.host.site_mismatch` en anomalies, ouvertes puis fermées — REQ-009-13 —
  verify : `each_warning_opens_and_closes`, `vlan_change_on_network_reevaluates`
  où : design « Règles » ; ADR 0026 clause 7 ; anomalies du noyau
  dépend de : 4, 5 ; 008 : 12
- [ ] 8. Références et cascades ; section `machines.attachments` — REQ-009-14, 23 —
  verify : `closing_a_machine_lists_interfaces`, `cascade_machine_closes_attachments`,
  `cascade_cluster_nullifies_hosts`, `closing_an_attached_address_refused`,
  `closing_a_used_vlan_cascades_to_interfaces`, `attachments_section_in_simulation`,
  `dependent_module_reads_the_section`
  où : design « Références », « Section » ; 008 tâches 7, 13, 19
  dépend de : 5 ; 008 : 7, 19
- [ ] 9. Port de lecture daté et lectures croisées ; valeurs de propriétés des machines
  d'un lot d'adresses — REQ-009-16, 22, 25 — verify :
  `addresses_of_machine_with_roles`, `attachments_of_address_as_of`,
  `machines_in_network_paginated`, `dependent_module_reads_in_transaction`,
  `machine_values_of_addresses_batch` (VIP à deux machines, adresse sans machine)
  où : design « Port de lecture » ; `kartenn-store-pg/src/temporal.rs`, `read.rs` ;
  lecture groupée des valeurs résolues (tâche 010-2, N9) si elle existe, sinon lecture
  par machine
  dépend de : 6, 8
- [ ] 10. Routes REST : clusters, machines, interfaces, rattachements, lectures
  croisées, arêtes, filtres et `QUERY` — REQ-009-15, 16, 17, 18 — verify :
  `interfaces_by_address_at_a_date`, `machine_card_with_interfaces`,
  `machines_of_a_network_hide_unreadable`, `graph_edge_address_to_interface`,
  `patch_interface_needs_if_match`, `mac_canonical_over_http`
  où : design « Routes » ; routes du module VLAN (008, tâche 14)
  dépend de : 8, 9 ; 008 : 8
- [ ] 11. Imports — REQ-009-19 — verify :
  `import_machines_interfaces_attachments_by_local_names`
  où : 008 tâche 15
  dépend de : 10

## P3 — Budgets, interface, synchronisation

- [ ] 12. Banc : jeu augmenté (10⁵ machines, 10³ clusters, 3·10⁵ interfaces et
  rattachements), mesures REQ-009-21, bloquantes — verify : `bench-core` vert (sous-agent
  `bench-runner`)
  où : `kartenn-bench` ; 008 tâche 16
  dépend de : 10 ; 008 : 16
- [ ] 13. Plugin `web/plugins/machines` — REQ-009-20 — verify : parcours Playwright
  `web/e2e/journeys/machines.spec.ts` avec axe-core
  où : design « Interface » ; plugin VLAN (008, tâche 17)
  dépend de : 10 ; 008 : 17
- [ ] 14. Synchronisation : `docs/spec/` (data : schéma `machines` ; interfaces : routes,
  opérations, port de lecture ; permissions), spec passée à `implemented` — verify :
  rapport `spec-keeper` sans écart bloquant
  où : skill `tech-specs` (mode sync)
  dépend de : 1 à 13

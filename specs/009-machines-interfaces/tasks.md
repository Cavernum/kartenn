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

- [ ] 2. Clusters et machines : migration `0001_machines.sql` (`cluster`, `machine`),
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
- [ ] 3. Interfaces : table `interface`, `create/update_interface`, unicité du nom par
  machine, MAC, état, héritage des ancres — REQ-009-6, 7 — verify :
  `interface_name_unique_per_machine`, `mac_stored_as_macaddr`,
  `interface_cannot_change_machine`, `owner_change_moves_interface_scope`
  où : design « Types d'objets », « Données »
  dépend de : 2
- [ ] 4. VLAN d'une interface : modes `none`/`access`/`trunk`, VLAN vivants, droits
  `vlan.vlan.read` — REQ-009-12, 18 (VLAN) — verify : `access_needs_one_vlan`,
  `native_not_in_tagged`, `closed_vlan_refused`, `vlan_read_required`
  où : design « Opérations », « Routes » (droits) ; propriété et type `vlan.vlan` (008)
  dépend de : 3 ; 008 : 11
- [ ] 5. Rattachements : table `attachment`, `attach_address`/`detach_address`, unicité
  hors `vip`/`anycast`, recopie de l'usage (`shared`) et réévaluation sur changement
  d'usage, adresse principale par famille, droits M16 — REQ-009-9, 10, 11, 18 —
  verify : `two_concurrent_attachments_one_wins`, `vip_on_two_machines`,
  `usage_back_to_host_refused_when_shared`, `two_primary_v4_refused`,
  `attach_needs_address_read_only`
  où : design « Données » (`shared`), « Opérations », « Routes » (droits) ; usage des
  adresses `kartenn-core/src/objects.rs::AddressUsage`
  dépend de : 3
- [ ] 6. Propriétés du module : `machines.interface_role` (valeurs livrées, extensible),
  `machines.fqdn` ; porteurs pour d'autres modules — REQ-009-8, 24 — verify :
  `interface_role_values`, `admin_adds_a_role_value`,
  `foreign_module_property_on_a_machine`
  où : design « Propriétés déclarées » ; `declarations.rs` (`ModuleDeclarations`)
  dépend de : 3
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

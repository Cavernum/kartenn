<!-- Généré par kartenn-server (modules::tests) : ne pas modifier à la main. -->

# Catalogue des règles des modules

Règles déclarées par les modules livrés (ADR 0026, clause 7), au même format que le
[catalogue du noyau](regles-noyau.md) : identifiants stables, sévérités fixées par la
spécification du module.

## `vlan`

| Identifiant | Sévérité | Règle | Exigences | Source |
|---|---|---|---|---|
| `vlan.vid.invalid` | erreur | Un VID est un entier de 1 à 4094. | REQ-008-12, REQ-008-14 | IEEE 802.1Q-2022 §9.6 |
| `vlan.vid.out_of_range` | erreur | Un VID est dans la plage de son groupe, et la plage d'un groupe garde ses VLAN. | REQ-008-13, REQ-008-14 | 008/V6 |
| `vlan.vid.duplicate` | erreur | Un VID est vivant au plus une fois par groupe à tout instant. | REQ-008-15 | 008/V2 |
| `vlan.name.invalid` | erreur | Un nom de VLAN a de 1 à 64 caractères, sans caractère de contrôle. | REQ-008-14 | 008/V7 |
| `vlan.name.duplicate` | erreur | Un nom de VLAN est vivant au plus une fois par groupe à tout instant. | REQ-008-15 | 008/V7 |
| `vlan.group.anchor` | erreur | Un groupe est ancré sur une entité, un site ou un domaine vivant sur toute sa période. | REQ-008-12 | 008/V3 |
| `vlan.prefix.not_network` | erreur | La propriété vlan.vlan ne se pose que sur un réseau. | REQ-008-17 | 008/V4 |
| `vlan.site.mismatch` | avertissement | Le site d'un réseau et celui du groupe de son VLAN concordent. | REQ-008-18 | 008/V9 |
| `vlan.domain.mismatch` | avertissement | Le domaine d'un réseau et celui du groupe de son VLAN concordent. | REQ-008-18 | 008/V9 |

## `machines`

| Identifiant | Sévérité | Règle | Exigences | Source |
|---|---|---|---|---|
| `machines.operation.invalid` | erreur | Une opération du module porte les champs qu'il attend, avec des valeurs admises et des objets qui existent. | REQ-009-3, REQ-009-6 | ADR 0026 |
| `machines.name.invalid` | erreur | Un nom de machine est une suite d'étiquettes LDH de 1 à 63 caractères, 253 au total, conservée en minuscules. | REQ-009-4 | RFC 1123 §2.1 |
| `machines.name.duplicate` | erreur | Deux machines vivantes de même propriétaire résolu ne portent pas le même nom sur des périodes qui se chevauchent. | REQ-009-4 | 009/M3 |
| `machines.cluster.duplicate` | erreur | Un slug de cluster est unique à tout instant. | REQ-009-2 | 009/M6 |
| `machines.host.invalid` | erreur | Une machine n'est hébergée que selon les combinaisons permises, jamais par elle-même ni en cycle, ni par un hébergeur clos. | REQ-009-5 | 009/M5 |
| `machines.interface.duplicate` | erreur | Une machine ne porte pas deux interfaces de même nom au même instant. | REQ-009-6 | 009/M7 |
| `machines.mac.invalid` | erreur | Une adresse MAC d'interface est un EUI-48 unicast écrit en hexadécimal. | REQ-009-6 | RFC 9542 §1.1, §2.1.1 |
| `machines.vlan.mode` | erreur | Le mode 802.1Q d'une interface et ses VLAN sont cohérents, et ses VLAN vivants sur la période. | REQ-009-12 | IEEE 802.1Q-2022, 009/M9 |
| `machines.address.taken` | erreur | Une adresse non partagée n'est rattachée qu'à une interface à la fois. | REQ-009-10 | 009/M8 |
| `machines.address.primary` | erreur | Une interface a au plus une adresse principale par famille. | REQ-009-11 | 009/M8 |
| `machines.address.closed` | erreur | Une adresse rattachée est vivante sur toute la période du rattachement. | REQ-009-9 | 009/M8 |
| `machines.mac.duplicate` | avertissement | Une MAC universelle n'est portée que par une interface vivante. | REQ-009-13 | 009/M11 |
| `machines.address.vlan_mismatch` | avertissement | Le réseau d'une adresse rattachée ne porte pas de VLAN étranger à l'interface. | REQ-009-13 | 009/M11 |
| `machines.host.site_mismatch` | avertissement | Une machine hébergée et son hébergeur ont le même site résolu. | REQ-009-13 | 009/M11 |

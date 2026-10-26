<!-- Généré par kartenn-core (rules::catalogue_markdown) : ne pas modifier à la main. -->

# Catalogue des règles du noyau et des propriétés

Identifiants stables (REQ-002-37) : un identifiant n'est jamais réutilisé pour un autre sens.
Sévérités fixées par la spécification (002/A4).

| Identifiant | Sévérité | Règle | Exigences | Source |
|---|---|---|---|---|
| `core.addr.syntax` | erreur | Une adresse est écrite sous la forme stricte de sa famille. | REQ-002-2, REQ-002-3 | RFC 6943 §3.1.1, RFC 4291 §2.2 |
| `core.addr.zone` | erreur | Une adresse ou un préfixe ne porte pas d'identifiant de zone. | REQ-002-4 | RFC 4007 §6 |
| `core.addr.ipv4_mapped` | erreur | Aucun préfixe ni adresse IPv6 IPv4-mapped n'est stocké ; on utilise la valeur IPv4. | REQ-002-31 | 002/A4 |
| `core.prefix.syntax` | erreur | Un préfixe s'écrit adresse/longueur, la longueur explicite et dans la largeur de la famille. | REQ-002-5 | RFC 4632 §3.1 |
| `core.prefix.unaligned` | erreur | Un préfixe n'a aucun bit d'hôte à 1. | REQ-002-5 | 002/design |
| `core.prefix.duplicate` | erreur | Un préfixe existe au plus une fois par domaine sur toute période. | REQ-002-15 | D34 |
| `core.prefix.network_not_leaf` | erreur | Un réseau ne contient aucun autre préfixe. | REQ-002-16, REQ-002-17 | 002/A2 |
| `core.prefix.not_empty_for_nature` | erreur | Un préfixe ne change de nature que si son contenu le permet. | REQ-002-17 | 002/A2 |
| `core.prefix.p2p_length` | erreur | Seuls les réseaux /31 et /32 (IPv4) ou /127 et /128 (IPv6) sont point à point. | REQ-002-19 | RFC 3021, RFC 6164 |
| `core.prefix.anycast_network` | erreur | Seuls les réseaux sont marqués anycast. | REQ-002-49 | 002/A13 |
| `core.anycast.address_usage` | erreur | Les adresses d'un réseau anycast sont d'usage anycast. | REQ-002-49 | 002/A13 |
| `core.prefix.site_on_container` | erreur | Seuls les réseaux sont rattachés à un site. | REQ-002-20 | 002/A7 |
| `core.range.bounds` | erreur | Une plage a des bornes d'une même famille, la première au plus égale à la dernière. | REQ-002-21 | 002/design |
| `core.range.outside_network` | erreur | Une plage est entièrement contenue dans un seul réseau de son domaine. | REQ-002-21 | 002/A2 |
| `core.range.overlap` | erreur | Les plages d'un réseau ne se chevauchent pas. | REQ-002-22 | D34 |
| `core.range.reserved_bounds` | erreur | Une plage exclut les adresses de réseau et de diffusion (IPv4) et l'IID nul (IPv6). | REQ-002-23 | RFC 1122 §3.2.1.3, RFC 4291 §2.6.1 |
| `core.address.outside_network` | erreur | Une adresse appartient à exactement un réseau de son domaine. | REQ-002-25 | 002/A2 |
| `core.address.duplicate` | erreur | Une adresse existe au plus une fois par domaine sur toute période. | REQ-002-26 | D4 |
| `core.address.range_usage` | erreur | Les adresses respectent l'usage de la plage où elles tombent. | REQ-002-28 | 002/A8 |
| `core.address.not_assignable` | erreur | Les adresses de réseau, de diffusion, d'IID nul, de boucle locale, non spécifiées, réservées et multicast ne sont pas attribuées. | REQ-002-29 | RFC 1122 §3.2.1.3, RFC 4291 §2.6.1, registres IANA |
| `core.public.overlap` | erreur | L'espace public est unique entre domaines, sauf adresses anycast partout et réseaux anycast partout, de même valeur. | REQ-002-32, REQ-002-46 | 002/A3, 002/A11, 002/A13 |
| `core.domain.immutable` | erreur | Un objet d'adressage ne change jamais de domaine. | REQ-002-12 | 002/A1 |
| `core.anchor.cycle` | erreur | Les arbres des entités et des sites n'ont pas de cycle. | REQ-002-8, REQ-002-9 | 002/design |
| `core.anchor.in_use` | erreur | Une entité ou un site n'est clos que si rien ne le référence. | REQ-002-10 | D33 |
| `core.slug.syntax` | erreur | Un slug compte 1 à 63 caractères [a-z0-9-], sans tiret au début ni à la fin. | REQ-002-47 | 002/A9 |
| `core.slug.duplicate` | erreur | Un slug est unique par type d'objet à tout instant. | REQ-002-47 | 002/A9 |
| `core.external_id.duplicate` | erreur | Un identifiant externe désigne un seul objet par source. | REQ-002-41 | 002/A9 |
| `core.addr.documentation` | avertissement | Les blocs de documentation ne servent pas en production. | REQ-002-35 | RFC 5737, RFC 3849, RFC 9637 |
| `core.addr.shared_space` | avertissement | L'espace partagé est destiné au NAT d'opérateur (CGN). | REQ-002-35 | RFC 6598 |
| `core.prefix.ipv6_long_subnet` | avertissement | Les réseaux d'hôtes IPv6 sont des /64, sauf point à point /127 et /128. | REQ-002-35 | RFC 7136 §5 |
| `core.prefix.ula_global_id` | avertissement | Un préfixe unique local utilise un Global ID pseudo-aléatoire. | REQ-002-35 | RFC 4193 §3.2 |
| `core.public.anycast_shared` | avertissement | Une adresse ou un réseau public anycast figure dans plusieurs domaines. | REQ-002-46 | 002/A11, 002/A13 |
| `prop.declaration.invalid` | erreur | Une déclaration de propriété a une clé, un type et des paramètres valides. | REQ-003-2, REQ-003-3 | 003/P1, P2 |
| `prop.declaration.namespace` | erreur | Un module déclare dans son espace, un administrateur dans l'espace custom avec les seuls types intégrés. | REQ-003-1, REQ-003-2 | 003/P1 |
| `prop.declaration.duplicate` | erreur | Une clé de propriété désigne une seule déclaration sur toute période. | REQ-003-2 | 003/P8 |
| `prop.declaration.values_of` | erreur | Une liste fermée n'emprunte les valeurs que d'une autre liste fermée du même déclarant, qui ne les emprunte à aucune. | REQ-016-2 | 016/G5 |
| `prop.enum_value.invalid` | erreur | Une valeur de liste fermée a une clé [a-z0-9-] unique dans sa propriété, de type liste fermée, valide sur sa période. | REQ-003-5, REQ-003-7 | 003/P8 |
| `prop.declaration.incompatible` | erreur | Une évolution de déclaration n'invalide aucune affectation et ne change aucune valeur résolue sans affectation explicite. | REQ-003-6 | 003/P8 |
| `prop.assignment.not_carrier` | erreur | Une propriété n'est affectée qu'aux types d'objets qui la portent. | REQ-003-4 | 003/P1 |
| `prop.assignment.invalid_value` | erreur | Une valeur respecte son type et ses paramètres. | REQ-003-8 | 003/P2 |
| `prop.assignment.required` | erreur | Un objet de la portée d'une propriété obligatoire a une valeur résolue. | REQ-003-16 | 003/P4, D27 |
| `prop.assignment.forbidden_effect` | erreur | L'auteur détient la permission de la propriété sur chaque objet dont la valeur résolue change. | REQ-003-17, REQ-003-18 | 003/P6, P7 |
| `prop.assignment.dangling_reference` | erreur | Une référence désigne un objet valide sur toute la période de l'affectation. | REQ-003-10 | D33 |
| `prop.resolution.expected_mismatch` | avertissement | La valeur résolue diffère de la valeur attendue par la source configurée. | REQ-003-24 | D23 |
| `changeset.operation.no_version` | erreur | Une opération ne vise un objet que sur une période où il existe. | REQ-004-14, REQ-004-15 | 004/design |
| `changeset.operation.conflict` | erreur | Une opération qui précise la version attendue d'un objet échoue si la version courante diffère. | REQ-004-11 | 004/design |
| `changeset.state.not_pending` | erreur | Seul un changeset en attente peut être validé, rejeté ou retiré. | REQ-004-19, REQ-004-24 | 004/design |
| `changeset.state.not_author` | erreur | Seul l'auteur retire son changeset en attente. | REQ-004-23 | 004/design |
| `changeset.approval.self` | erreur | L'auteur n'applique pas son propre changeset quand une politique d'approbation le retient ou qu'un module exige sa revue ; il ne le rejette pas. | REQ-024-11, REQ-024-12 | 024/M7 |
| `changeset.approval.self.missing` | erreur | L'auteur n'applique son propre changeset que s'il détient `approval.self` sur tout ce qu'il touche, objets et effets. | REQ-024-11, REQ-024-13 | 024/M7 |
| `changeset.approval.forbidden` | erreur | Qui applique ou rejette le changeset d'un autre détient la permission apply de tout ce qu'il touche. | REQ-024-11 | 024/M7 |
| `core.review.operation` | erreur | Seules les opérations qu'un module déclare « de revue » se joignent à la validation d'une proposition. | REQ-016-11 | 016/G12 |
| `changeset.policy.invalid` | erreur | Une politique d'approbation a un nom et au moins un déclencheur. | REQ-004-40 | 004/C6 |
| `core.approval_policy.trigger` | erreur | Un déclencheur de politique nomme une mesure déclarée par un module du build. | REQ-016-7 | 016/G3 |
| `core.approval_policy.author` | erreur | Les auteurs d'une politique d'approbation se désignent par un rôle ou un groupe kartenn en vigueur, jamais par un compte. | REQ-025-13 | 025/G10 |
| `changeset.policy.forbidden` | erreur | Créer, modifier ou désactiver une politique d'approbation exige le droit d'administration des politiques. | REQ-004-39, REQ-004-40 | 004/C6 |
| `core.role.invalid` | erreur | Un rôle a une clé, des libellés et des entrées du catalogue ; un émetteur n'est pas vide. | REQ-005-17, REQ-005-25 | 005/K12 |
| `core.rbac.forbidden` | erreur | Administrer les rôles, les émetteurs, les comptes ou les liaisons exige la permission d'administration correspondante. | REQ-005-26 | 005/K7, K11 |
| `core.binding.escalation` | erreur | On ne lie un rôle sur un périmètre que si l'on détient soi-même chacune de ses entrées sur ce périmètre. | REQ-005-26 | 005/K7 |
| `core.role.builtin` | erreur | Un rôle livré ne change que par la synchronisation du noyau. | REQ-005-17 | 005/K12 |
| `core.role.unknown` | erreur | Une liaison désigne un rôle en vigueur. | REQ-005-25 | 005/K6 |
| `core.binding.requester` | erreur | Un groupe n'a pas pour membre une identité « requester » : un demandeur n'est pas un compte ; la personne qui se connecte ensuite par son émetteur en devient un. | REQ-016-12, REQ-025-2 | 016/G10, 025/G2 |
| `core.binding.group_closed` | erreur | Une liaison désigne un groupe en vigueur : un groupe clos depuis l'écriture d'un changeset en attente qui le lie met ce changeset en conflit à son approbation. | REQ-025-4 | 025/G1 |
| `core.group.invalid` | erreur | Un groupe a une clé et des libellés ; un membre est une identité ou un groupe non vide d'un émetteur, présent au plus une fois dans un groupe à une même date. | REQ-025-1, REQ-025-2 | 025/G2 |
| `core.group.unknown` | erreur | Une appartenance désigne un groupe en vigueur. | REQ-025-2 | 025/G2 |
| `core.group.nested` | erreur | Un groupe kartenn n'est membre d'aucun autre : un membre est une identité ou un groupe de l'émetteur, la forme d'une appartenance n'en admet pas d'autre. | REQ-025-3 | 025/G6 |
| `core.group.escalation` | erreur | On n'ajoute ni ne retire un membre d'un groupe, ni ne le clôt, que si l'on pourrait créer chacune de ses liaisons : administrer les droits sur leur périmètre et détenir chaque entrée de leurs rôles. | REQ-025-7, REQ-025-8 | 025/G5 |
| `core.group.builtin` | erreur | Un groupe livré, ses libellés et ses liaisons ne changent que par la synchronisation du noyau ; ses membres se gèrent comme ceux d'un autre groupe. | REQ-025-9 | 025/G7 |
| `core.object.forbidden` | erreur | Créer, modifier ou clore un objet exige la permission d'édition de sa sorte sur l'objet. | REQ-005-22 | 005/K12 |
| `core.label.invalid` | erreur | Un libellé a de 1 à 200 caractères sans caractère de contrôle, sous une étiquette de langue BCP 47 ; une table de libellés non vide contient la langue par défaut. | REQ-018-9 | 018/R7, R9, RFC 5646 §2.1 |
| `core.object.dependents` | erreur | Un objet n'est clos que si aucun objet vivant d'un module n'y fait référence ; la cascade explicite clôt ou détache les dépendants selon la politique de chaque référence. | REQ-008-10 | D33, ADR 0026 |

## Règles du tri d'un import

Codes des violations de l'`error` d'un import (006/H18), d'une forme antérieure à
celle des identifiants ci-dessus ; servis avec eux par `GET /v1/rules` (018).

| Identifiant | Sévérité | Règle | Exigences | Source |
|---|---|---|---|---|
| `import.missing-external-id` | erreur | Un élément d'import qui crée un objet porte l'identifiant externe de sa source. | REQ-006-17 | 006/H18 |
| `import.unknown-reference` | erreur | Une référence locale d'un import désigne un élément du même import. | REQ-006-17 | 006/H18 |
| `import.order` | erreur | Chaque nom local d'un import est défini une fois et ses références ne forment pas de cycle. | REQ-006-17 | 006/H18 |
| `import.invalid` | erreur | Les éléments d'un import se trient ; une autre erreur de tri le ramène en dépôt. | REQ-006-17, REQ-006-19 | 006/H18 |

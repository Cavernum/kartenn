<!-- Généré par kartenn-core (rules::catalogue_markdown) : ne pas modifier à la main. -->

# Catalogue des règles du noyau

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
| `core.prefix.site_on_container` | erreur | Seuls les réseaux sont rattachés à un site. | REQ-002-20 | 002/A7 |
| `core.range.bounds` | erreur | Une plage a des bornes d'une même famille, la première au plus égale à la dernière. | REQ-002-21 | 002/design |
| `core.range.outside_network` | erreur | Une plage est entièrement contenue dans un seul réseau de son domaine. | REQ-002-21 | 002/A2 |
| `core.range.overlap` | erreur | Les plages d'un réseau ne se chevauchent pas. | REQ-002-22 | D34 |
| `core.range.reserved_bounds` | erreur | Une plage exclut les adresses de réseau et de diffusion (IPv4) et l'IID nul (IPv6). | REQ-002-23 | RFC 1122 §3.2.1.3, RFC 4291 §2.6.1 |
| `core.address.outside_network` | erreur | Une adresse appartient à exactement un réseau de son domaine. | REQ-002-25 | 002/A2 |
| `core.address.duplicate` | erreur | Une adresse existe au plus une fois par domaine sur toute période. | REQ-002-26 | D4 |
| `core.address.range_usage` | erreur | Les adresses respectent l'usage de la plage où elles tombent. | REQ-002-28 | 002/A8 |
| `core.address.not_assignable` | erreur | Les adresses de réseau, de diffusion, d'IID nul, de boucle locale, non spécifiées, réservées et multicast ne sont pas attribuées. | REQ-002-29 | RFC 1122 §3.2.1.3, RFC 4291 §2.6.1, registres IANA |
| `core.public.overlap` | erreur | L'espace public est unique entre domaines, sauf adresses anycast partout. | REQ-002-32, REQ-002-46 | 002/A3, 002/A11 |
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
| `core.public.anycast_shared` | avertissement | Une adresse publique anycast figure dans plusieurs domaines. | REQ-002-46 | 002/A11 |

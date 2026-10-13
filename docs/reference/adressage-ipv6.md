# Adressage IPv6

## Sources

| Document | Statut | Consulté |
|---|---|---|
| [RFC 4291](https://www.rfc-editor.org/rfc/rfc4291) — IPv6 Addressing Architecture | Draft Standard ; mise à jour par RFC 5952, 6052, 7136, 7346, 7371, 8064 | 2026-10-13 |
| [RFC 7136](https://www.rfc-editor.org/rfc/rfc7136) — Significance of IPv6 Interface Identifiers | Standards Track | 2026-10-13 |
| [RFC 4193](https://www.rfc-editor.org/rfc/rfc4193) — Unique Local IPv6 Unicast Addresses | Standards Track | 2026-10-13 |
| [RFC 4007](https://www.rfc-editor.org/rfc/rfc4007) — Scoped Address Architecture | Standards Track | 2026-10-13 |
| [RFC 6164](https://www.rfc-editor.org/rfc/rfc6164) — /127 on Inter-Router Links | Standards Track | 2026-10-13 |
| [Registre IANA IPv6 à usage spécial](https://www.iana.org/assignments/iana-ipv6-special-registry/) | mis à jour le 2025-10-09 | 2026-10-13 |

## Ce que disent les sources

### Types d'adresses (RFC 4291 §2.4)

| Type | Préfixe |
|---|---|
| Non spécifiée | `::/128` |
| Boucle locale | `::1/128` |
| Multicast | `ff00::/8` |
| Lien local unicast | `fe80::/10` |
| Unicast global | tout le reste |

- Les adresses anycast proviennent de l'espace unicast et ne s'en distinguent pas
  syntaxiquement (§2.4).
- Sites locaux (`fec0::/10`) : dépréciés ; les nouvelles implémentations « must
  treat this prefix as Global Unicast » (§2.5.7).
- IPv4-compatible (`::a.b.c.d`) : déprécié (§2.5.5.1). IPv4-mapped
  (`::ffff:a.b.c.d`) : représente un nœud IPv4 sous forme IPv6 (§2.5.5.2).
- Multicast (§2.7) : 8 bits à 1, 4 bits de drapeaux, 4 bits de portée (1 interface,
  2 lien, 4 admin, 5 site, 8 organisation, E global), 112 bits de groupe.
- Subnet-Router anycast (§2.6.1) : le préfixe du sous-réseau avec un identifiant
  d'interface à zéro.

### Identifiant d'interface (RFC 4291 §2.5.1, remplacé par RFC 7136 §5)
- « Interface IDs are required to be 64 bits long » est **maintenu**.
- Le format EUI-64 modifié n'est exigé que si l'IID est dérivé d'une adresse MAC.
- « the bits in an IID have no generic semantics » : les bits u/g ne sont pas un
  indicateur fiable d'unicité (§2).

### Adresses uniques locales, ULA (RFC 4193)
- §3.1 : `fc00::/7` + bit L + Global ID de 40 bits + Subnet ID de 16 bits + IID de 64 bits.
- L = 1 : préfixe attribué localement, soit `fd00::/8`. L = 0 (`fc00::/8`) : « may be
  defined in the future » ; aucune méthode d'attribution n'est définie à ce jour.
- §3.2 : le Global ID est pseudo-aléatoire ; « They MUST NOT be assigned sequentially
  or with well-known numbers. »
- §4.1 et §4.3 : ces préfixes ne sont pas annoncés entre domaines et ne sortent pas du site.

### Zones (RFC 4007 §11)
- Format `<adresse>%<zone_id>` (SHOULD), pour les adresses non globales seulement.
- §6 : « The zone indices are strictly local to the node. »
- §11.7 : avec une longueur de préfixe, la zone précède la longueur (`fe80::%2/64`).

### Liens point à point en /127 (RFC 6164 §6)
- « Routers MUST support the assignment of /127 prefixes on point-to-point
  inter-router links. »
- « Routers MUST disable Subnet-Router anycast for the prefix when /127 prefixes are used. »
- §5.3 : beaucoup d'opérateurs réservent tout de même un /64 par lien et n'en
  configurent qu'un /127.

## Pièges

- `fc00::/7` (ULA) n'est pas l'équivalent exact de RFC 1918 : la plage utilisable
  aujourd'hui est `fd00::/8` avec un Global ID aléatoire. Un `fd00::/48` « tout à
  zéro » viole RFC 4193 §3.2.
- Une zone (`%eth0`) n'a de sens que sur un nœud : elle ne doit pas être stockée comme
  partie de l'identité d'une adresse dans un référentiel.
- Il n'y a ni adresse de réseau ni adresse de diffusion en IPv6, mais l'IID nul est
  l'anycast Subnet-Router (§2.6.1) : il ne doit pas être attribué comme adresse
  unicast d'hôte, sauf sur un /127 (RFC 6164).
- Deux nouvelles plages de documentation coexistent : `2001:db8::/32` (RFC 3849) et
  `3fff::/20` (RFC 9637).

## Implications pour kartenn (propositions pour la spec 002)

1. **Pas de zone** dans le modèle d'adresse : `%zone` est refusé à la saisie (ou,
   au mieux, accepté et ignoré avec un avertissement : à trancher). Le lien local
   `fe80::/10` est un préfixe comme un autre, mais son domaine d'adressage est le lien.
2. **Capacité** : 2^(128-n), sans réservation de diffusion. L'attribution de l'IID nul
   est refusée sauf sur un /127 marqué point à point.
3. **Taille des sous-réseaux** : avertissement (non bloquant) pour un sous-réseau
   d'hôtes plus long qu'un /64, l'IID de 64 bits étant la règle (RFC 7136 §5) ;
   exceptions explicites pour /127 et /128.
4. **ULA** : aide à la génération d'un Global ID aléatoire ; anomalie si un préfixe ULA
   a un Global ID manifestement non aléatoire (zéros, séquentiel). À trancher : simple
   avertissement.
5. **IPv4-mapped** : `::ffff:a.b.c.d` refusé comme adresse IPv6 stockée ; on stocke
   l'adresse IPv4 (une seule identité par adresse, voir
   [representation-textuelle.md](representation-textuelle.md)).

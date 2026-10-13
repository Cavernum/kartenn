# Registres IANA des adresses à usage spécial

## Sources

| Document | Statut | Consulté |
|---|---|---|
| [RFC 6890](https://www.rfc-editor.org/rfc/rfc6890) — Special-Purpose IP Address Registries | BCP 153 ; mis à jour par RFC 8190 ; remplace RFC 4773, 5156, 5735, 5736 | 2026-10-13 |
| [RFC 8190](https://www.rfc-editor.org/rfc/rfc8190) — Updates to the Special-Purpose IP Address Registries | BCP 153 (2017), met à jour RFC 6890 | 2026-10-13 |
| [Registre IPv4](https://www.iana.org/assignments/iana-ipv4-special-registry/) | mis à jour le 2025-10-09 | 2026-10-13 |
| [Registre IPv6](https://www.iana.org/assignments/iana-ipv6-special-registry/) | mis à jour le 2025-10-09 | 2026-10-13 |

## Ce que disent les sources

### Champs d'une entrée (RFC 6890 §2.2.1)
- **Address Block**, **Name**, **RFC**, **Allocation Date**, **Termination Date**
  (allocations temporaires).
- **Source** : valide comme adresse source d'un datagramme qui traverse deux équipements.
- **Destination** : valide comme adresse de destination, même définition.
- **Forwardable** : un routeur peut transmettre un datagramme ayant cette destination.
- **Global**, renommé **Globally Reachable** par RFC 8190 §2.1 : un datagramme à
  destination du bloc peut être transmis au-delà d'un domaine administratif donné.
- **Reserved-by-Protocol** : RFC 6890 le définit vrai si la RFC d'origine « requires all
  compliant IP implementations to behave in a special way ».
- « If the value of "Destination" is FALSE, the values of "Forwardable" and "Global"
  must also be false. »

### Registre IPv4 (extrait, état au 2025-10-09)

| Bloc | Nom | Réf. | Src | Dst | Fwd | Glob. | Rés. |
|---|---|---|---|---|---|---|---|
| 0.0.0.0/8 | « This network » | RFC 791 | V | F | F | F | V |
| 10.0.0.0/8 | Private-Use | RFC 1918 | V | V | V | F | F |
| 100.64.0.0/10 | Shared Address Space | RFC 6598 | V | V | V | F | F |
| 127.0.0.0/8 | Loopback | RFC 1122 | F | F | F | F | V |
| 169.254.0.0/16 | Link Local | RFC 3927 | V | V | F | F | V |
| 172.16.0.0/12 | Private-Use | RFC 1918 | V | V | V | F | F |
| 192.0.0.0/24 | IETF Protocol Assignments | RFC 6890 | F | F | F | F | F |
| 192.0.2.0/24 | Documentation (TEST-NET-1) | RFC 5737 | F | F | F | F | F |
| 192.168.0.0/16 | Private-Use | RFC 1918 | V | V | V | F | F |
| 198.18.0.0/15 | Benchmarking | RFC 2544 | V | V | V | F | F |
| 198.51.100.0/24 | Documentation (TEST-NET-2) | RFC 5737 | F | F | F | F | F |
| 203.0.113.0/24 | Documentation (TEST-NET-3) | RFC 5737 | F | F | F | F | F |
| 240.0.0.0/4 | Reserved | RFC 1112 | F | F | F | F | V |
| 255.255.255.255/32 | Limited Broadcast | RFC 8190, RFC 919 | F | V | F | F | V |

Le registre compte 25 entrées, dont plusieurs entrées /32 dans 192.0.0.0/24 (PCP,
TURN, NAT64/DNS64…) et des services anycast globaux (AS112, AMT).

### Registre IPv6 (extrait, état au 2025-10-09)

| Bloc | Nom | Réf. | Src | Dst | Fwd | Glob. | Rés. |
|---|---|---|---|---|---|---|---|
| ::1/128 | Loopback | RFC 4291 | F | F | F | F | V |
| ::/128 | Unspecified | RFC 4291 | V | F | F | F | V |
| ::ffff:0:0/96 | IPv4-mapped | RFC 4291 | F | F | F | F | V |
| 64:ff9b::/96 | IPv4-IPv6 Translation | RFC 6052 | V | V | V | V | F |
| 64:ff9b:1::/48 | IPv4-IPv6 Translation (local) | RFC 8215 | V | V | V | F | F |
| 100::/64 | Discard-Only | RFC 6666 | V | V | V | F | F |
| 2001::/23 | IETF Protocol Assignments | RFC 2928 | F¹ | F¹ | F¹ | F¹ | F |
| 2001:db8::/32 | Documentation | RFC 3849 | F | F | F | F | F |
| 2002::/16 | 6to4 | RFC 3056 | V | V | V | N/A | F |
| 3fff::/20 | Documentation | RFC 9637 | F | F | F | F | F |
| 5f00::/16 | SRv6 SIDs | RFC 9602 | V | V | V | F | F |
| fc00::/7 | Unique-Local | RFC 4193, RFC 8190 | V | V | V | F² | F |
| fe80::/10 | Link-Local Unicast | RFC 4291 | V | V | F | F | V |

¹ sauf allocation plus spécifique. ² voir RFC 4193.

## Pièges

- **Les registres évoluent** : `3fff::/20` (RFC 9637, 2024), `5f00::/16` (RFC 9602) et
  `100:0:0:1::/64` (RFC 9780) sont récents. Une liste figée dans le code devient fausse.
- Les entrées s'emboîtent : 192.0.0.0/24 contient des /32 aux propriétés différentes.
  Il faut appliquer l'entrée **la plus spécifique**.
- Certaines valeurs portent des notes (exceptions à la boucle locale, RFC 8029 et 5884)
  ou valent N/A (6to4, Teredo).
- RFC 8190 (§2.2 et §2.3) a modifié des valeurs : 255.255.255.255/32 devient
  Reserved-by-Protocol ; Teredo passe à N/A ; ULA reste « False » avec une note
  précisant que le bloc est pris dans l'unicast global mais n'est pas routé globalement.

## Implications pour kartenn (propositions pour la spec 002)

1. Embarquer les deux registres comme **données versionnées** (fichier généré depuis les
   CSV ou XML publiés par l'IANA), avec la date de mise à jour du registre ; jamais en
   dur dans le code.
2. Exposer pour chaque préfixe et adresse une **classification calculée** (entrée la plus
   spécifique, avec ses drapeaux). Elle est en lecture seule et sert aux contrôles
   (voir les implications des notes IPv4 et IPv6).
3. Une tâche de maintenance signale qu'une nouvelle version du registre est disponible ;
   la mise à jour passe par une version de kartenn, pour rester reproductible.
4. Ce référentiel est une donnée **par famille d'adresses** (D5) : une future famille
   apporte son propre registre.

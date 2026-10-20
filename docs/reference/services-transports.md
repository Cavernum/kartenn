# Services, protocoles de transport et numéros de port

Base du catalogue de services nommés du module Politique (spec 010, D29).

## Sources

| Document | Statut | Consulté |
|---|---|---|
| [RFC 6335](https://www.rfc-editor.org/rfc/rfc6335) — IANA Procedures for the Management of the Service Name and Transport Protocol Port Number Registry | Best Current Practice (BCP 165) | 2026-10-20 |
| [Registre IANA Service Name and Transport Protocol Port Number](https://www.iana.org/assignments/service-names-port-numbers/) | registre vivant | 2026-10-20 |
| [Registre IANA Protocol Numbers](https://www.iana.org/assignments/protocol-numbers/) | registre vivant | 2026-10-20 |
| [RFC 9293](https://www.rfc-editor.org/rfc/rfc9293) — Transmission Control Protocol (TCP) | Internet Standard (STD 7) | 2026-10-20 |
| [RFC 768](https://www.rfc-editor.org/rfc/rfc768) — User Datagram Protocol | Internet Standard (STD 6) | 2026-10-20 |
| [RFC 9260](https://www.rfc-editor.org/rfc/rfc9260) — Stream Control Transmission Protocol | Proposed Standard | 2026-10-20 |
| [RFC 4340](https://www.rfc-editor.org/rfc/rfc4340) — Datagram Congestion Control Protocol (DCCP) | Proposed Standard | 2026-10-20 |
| [RFC 792](https://www.rfc-editor.org/rfc/rfc792) — Internet Control Message Protocol | Internet Standard (STD 5) | 2026-10-20 |
| [RFC 4443](https://www.rfc-editor.org/rfc/rfc4443) — ICMPv6 | Internet Standard (STD 89) | 2026-10-20 |

## Ce que disent les sources

### Numéros de protocole (couche réseau)
- Le registre *Protocol Numbers* attribue le champ « Protocol » d'IPv4 et « Next
  Header » d'IPv6 (8 bits) : ICMP = 1, TCP = 6, UDP = 17, DCCP = 33, ICMPv6 = 58,
  SCTP = 132. Les autres protocoles (ESP = 50, GRE = 47…) n'ont pas de ports.

### Ports et noms de service (RFC 6335)
- Quatre protocoles de transport partagent le registre des ports : TCP, UDP, SCTP et
  DCCP (RFC 6335 §1). Un port est attribué **par protocole de transport** : le même
  numéro peut désigner des services différents selon le transport.
- Trois plages (RFC 6335 §6) : *System Ports* 0–1023, *User Ports* 1024–49151,
  *Dynamic and/or Private Ports* 49152–65535 ; ces derniers « MUST NOT » être attribués
  par l'IANA. Le port 0 est réservé.
- Syntaxe d'un nom de service (RFC 6335 §5.1) : « MUST be at least 1 character and no
  more than 15 characters long », uniquement lettres ASCII, chiffres et traits d'union,
  au moins une lettre, sans trait d'union en tête, en fin ni doublé ; comparaison
  insensible à la casse.
- Un nom de service peut avoir plusieurs ports et plusieurs transports ; un port peut
  porter plusieurs noms (alias historiques).

### ICMP et ICMPv6
- ICMP (RFC 792) et ICMPv6 (RFC 4443 §2.1) n'ont pas de port : un message est identifié
  par son **type** et son **code** (8 bits chacun), dans des registres IANA distincts
  pour IPv4 et IPv6.

## Pièges
- L'attribution d'un port par l'IANA dit à quel service il est **réservé**, pas ce qui
  circule réellement dessus ; un service peut tourner sur n'importe quel port.
- `ssh` est 22/tcp, mais 22/udp et 22/sctp existent aussi dans le registre : un service
  du catalogue doit nommer son transport.
- Les noms du registre (`http`, `https`, `domain`…) ne coïncident pas avec les usages
  (« DNS » est `domain`) ; ils ne doivent pas servir d'identifiant fonctionnel.
- Les plages dynamiques (49152–65535) servent souvent de ports source éphémères : un
  service déclaré sur une plage large ouvre beaucoup.

## Implications pour kartenn (propositions, tranchées dans la spec 010)
- Transports du catalogue : `tcp`, `udp`, `sctp`, `dccp` (ports), `icmp`, `icmpv6`
  (type et code facultatifs), `ip` (numéro de protocole 0–255, sans port) — D29.
- Ports : entiers 1–65535, plages fermées `[début, fin]` ; le port 0 est refusé.
- Le registre IANA sert de **référence importable** (suggestion de nom et de port à la
  saisie), pas de catalogue : un service kartenn porte un identifiant propre et un seul
  type de flux (D29), et peut regrouper plusieurs couples transport/port.
- Un avertissement signale un service ouvert sur une plage de plus de 1 024 ports ou
  sur la plage dynamique entière.

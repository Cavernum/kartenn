# VLAN IEEE 802.1Q et adresses MAC (EUI-48)

Note d'appui des specs 008 (VLAN) et 009 (Machines et interfaces).

## Sources

| Document | Statut | Consulté |
|---|---|---|
| IEEE Std 802.1Q-2022 — Bridges and Bridged Networks, §9.6 « VLAN Tag Control Information », Table 9-2 « Reserved VID values » ([page de la norme](https://standards.ieee.org/ieee/802.1Q/10323/)) | norme en vigueur, texte payant | 2026-10-20 (valeurs du tableau, non recopié) |
| [RFC 9542](https://www.rfc-editor.org/rfc/rfc9542) — IANA Considerations and IETF Protocol and Documentation Usage for IEEE 802 Parameters | BCP 141 (2024), remplace RFC 7042 | 2026-10-20 |

## Ce que disent les sources

### Identifiant de VLAN (IEEE 802.1Q-2022, §9.6)
- Le VID est un champ de **12 bits** de l'étiquette (C-TAG ou S-TAG) : valeurs 0 à 4095.
- Table 9-2, valeurs réservées :
  - `0` (0x000) : **VID nul** ; la trame ne porte qu'une priorité (« priority-tagged »),
    elle n'appartient à aucun VLAN par ce champ ;
  - `1` (0x001) : valeur **par défaut du PVID** d'un port ; utilisable comme VID ordinaire ;
  - `4095` (0xFFF) : **réservé** à l'implémentation ; ne doit être ni configuré comme PVID
    ou membre d'un ensemble de VLAN, ni transmis dans une étiquette.
- VID utilisables : **1 à 4094**.
- L'empilement d'étiquettes (S-TAG de 802.1ad, intégré à 802.1Q) et les identifiants
  étendus (VXLAN, 24 bits, RFC 7348) sont d'autres mécanismes ; kartenn ne les modélise
  pas en v0.1 (008, décision de l'utilisateur).

### Adresses MAC (RFC 9542)
- §1.1 : notation par octets en hexadécimal, « Successive octets are separated by a
  hyphen » (`00-00-5E-00-53-01`).
- §2.1.1 : dans le premier octet, le bit **M** (0x01, « Group ») indique une adresse de
  groupe (multicast ou diffusion) s'il vaut 1, unicast sinon ; le bit **X** (0x02,
  « universal/local ») indique une adresse administrée localement s'il vaut 1.
- §2.1.4 (valeurs de documentation) : `00-00-5E-00-53-00` à `00-00-5E-00-53-FF` est
  réservée aux exemples de documentation ; les specs et tests de kartenn l'utilisent.

## Choix propres à kartenn

- Un VLAN porte un VID de 1 à 4094 ; 0 et 4095 sont refusés à l'écriture (REQ-008-2).
- Une adresse MAC d'interface est un EUI-48 unicast : le bit M à 1 est refusé ; le bit X
  est accepté (adresses locales des machines virtuelles). Entrée acceptée avec `-`, `:`
  ou en groupes de quatre chiffres séparés par `.` ; restituée en **majuscules séparées par des
  tirets**, comme RFC 9542 §1.1 (REQ-009-6).
- EUI-64 et adresses de 20 octets (InfiniBand) : hors v0.1.

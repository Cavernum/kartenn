# Adressage IPv4

## Sources

| Document | Statut | Consulté |
|---|---|---|
| [RFC 4632](https://www.rfc-editor.org/rfc/rfc4632) — CIDR | BCP 122 (2006), remplace RFC 1519 | 2026-10-13 |
| [RFC 1122 §3.2.1.3](https://www.rfc-editor.org/rfc/rfc1122#section-3.2.1.3) — Addressing | Internet Standard (STD 3) | 2026-10-13 |
| [RFC 3021](https://www.rfc-editor.org/rfc/rfc3021) — préfixes /31 | Standards Track | 2026-10-13 |
| [RFC 6598](https://www.rfc-editor.org/rfc/rfc6598) — Shared Address Space | BCP 153 | 2026-10-13 |
| [Registre IANA IPv4 à usage spécial](https://www.iana.org/assignments/iana-ipv4-special-registry/) | mis à jour le 2025-10-09 | 2026-10-13 |

## Ce que disent les sources

### Préfixes (RFC 4632)
- §3.1 : un préfixe s'écrit `adresse/longueur`, la longueur étant « a decimal value
  between 0 and 32 that describes the number of significant bits ».
- §5.1 : « Forwarding in the Internet is done on a longest-match basis. »
- §5.1 : « the mask must be left contiguous » : pas de masque non contigu.
- Le texte ne pose pas explicitement de règle « bits d'hôte à zéro » ; ses tableaux
  montrent les bits de poids faible à zéro. L'exigence d'alignement est une
  convention universelle (et une contrainte du type `cidr` de PostgreSQL), pas une
  citation de RFC 4632.

### Valeurs particulières (RFC 1122 §3.2.1.3)
- (a) `{0, 0}` : « This host on this network. MUST NOT be sent, except as a source address ».
- (c) `{-1, -1}` (255.255.255.255) : « Limited broadcast. It MUST NOT be used as a source address. »
- (d) `{<Network-number>, -1}` : « Directed broadcast […] MUST NOT be used as a source address. »
- (g) `{127, <any>}` : « MUST NOT appear outside a host. »
- Règle générale : les champs Host-number, Network-number et Subnet-number ne prennent
  pas les valeurs 0 ou -1 (tous bits à 1), hors cas ci-dessus. D'où l'usage de
  réserver l'adresse de réseau et l'adresse de diffusion d'un sous-réseau.

### Liens point à point en /31 (RFC 3021)
- §2.1 : seules deux adresses existent, `{<Network-number>, 0}` et
  `{<Network-number>, -1}` ; sur un lien point à point en /31, elles « MUST be
  interpreted as host addresses ».
- §2.2 : la diffusion limitée « MUST be used for all broadcast traffic » ; la
  diffusion dirigée est impossible (§2.2.1).
- §5 : un lien dont une seule extrémité gère le /31 « may not operate correctly ».

### Espace partagé 100.64.0.0/10 (RFC 6598)
- §7 : « The Shared Address Space address range is 100.64.0.0/10. »
- §4 : destiné aux opérateurs pour le CGN ; ne doit pas franchir les frontières
  d'opérateur, ni figurer dans des zones DNS.
- §4 : une entité qui l'emploie à d'autres fins est « likely to experience problems »
  si elle a ensuite besoin d'un service CGN. C'est un avertissement, pas une
  interdiction.

### Blocs à usage spécial
Voir [registres-usage-special.md](registres-usage-special.md) : RFC 1918
(10/8, 172.16/12, 192.168/16), lien local 169.254/16, documentation (192.0.2/24,
198.51.100/24, 203.0.113/24), réservé 240/4, etc.

## Pièges

- Un /31 n'a ni adresse de réseau ni adresse de diffusion : un calcul naïf de la
  « capacité » (2^(32-n) − 2) donne 0 au lieu de 2. Un /32 contient une seule adresse.
- 100.64.0.0/10 est souvent utilisé à tort comme « RFC 1918 bis ». Ce n'est pas de
  l'adressage privé au sens de RFC 1918.
- 0.0.0.0/8 et 240.0.0.0/4 sont réservés par le protocole : ils apparaissent dans
  des configurations, mais ne doivent pas être attribués à des hôtes.

## Implications pour kartenn (propositions pour la spec 002)

1. **Capacité d'un préfixe** : règle par longueur.
   - IPv4 /0 à /30 : 2^(32-n) − 2 adresses attribuables (réseau et diffusion réservés) ;
   - /31 : 2 (RFC 3021) ;
   - /32 : 1.
   Le calcul d'occupation (budget D14) applique la même règle.
2. **Attribution d'adresses** : par défaut, refuser d'attribuer l'adresse de réseau et
   l'adresse de diffusion d'un préfixe IPv4 de longueur ≤ 30 (validation stricte,
   D1). Le préfixe peut porter un indicateur explicite « point à point » pour les /31.
3. **Classification automatique** : chaque préfixe et adresse se voit associer les
   propriétés du bloc à usage spécial le plus spécifique qui le contient (privé,
   partagé, documentation, réservé…), en lecture seule et calculées depuis un
   référentiel embarqué et versionné (voir la note des registres).
4. **Contrôles bloquants ou avertissements** (à trancher) : attribuer une adresse dans
   127/8, 0/8, 240/4 ou 255.255.255.255 est refusé ; utiliser de la documentation
   (192.0.2/24…) ou 100.64/10 hors contexte opérateur produit une anomalie, non
   bloquante.
5. Masques non contigus : refusés à la saisie (pas de notation `adresse masque` libre
   sans conversion vers une longueur de préfixe).

# Représentation textuelle : analyse stricte et forme canonique

Socle de la validation stricte (D1) : une adresse ou un préfixe a **une seule identité**
et **une seule forme canonique** en sortie, quelle que soit sa forme en entrée.

## Sources

| Document | Statut | Consulté |
|---|---|---|
| [RFC 4291 §2.2–2.3](https://www.rfc-editor.org/rfc/rfc4291#section-2.2) — formes textuelles IPv6 | Draft Standard | 2026-10-13 |
| [RFC 5952](https://www.rfc-editor.org/rfc/rfc5952) — A Recommendation for IPv6 Address Text Representation | Proposed Standard ; met à jour RFC 4291 | 2026-10-13 |
| [RFC 6943 §3.1](https://www.rfc-editor.org/rfc/rfc6943#section-3.1) — Issues in Identifier Comparison for Security Purposes | Informational | 2026-10-13 |
| [RFC 4007 §11](https://www.rfc-editor.org/rfc/rfc4007#section-11) — zones | Standards Track | 2026-10-13 |

## Ce que disent les sources

### IPv4 : forme stricte et forme lâche (RFC 6943 §3.1.1)
- `inet_addr()` (POSIX) accepte la forme « lâche » : octal, hexadécimal, moins de
  quatre parties. Exemples cités : `10.0.258`, `0xA000102` et `012.0x102` désignent
  **la même adresse**.
- `inet_pton()` n'accepte que la forme stricte (quatre décimaux pointés).
- « New protocols and data formats should similarly consider using the strict form
  rather than the loose form. »
- Sécurité : une chaîne valide en forme lâche mais pas en forme stricte est sans danger
  « as long as invalid identifiers are denied privilege ».

### IPv6 : formes acceptées (RFC 4291 §2.2)
- Forme complète `x:x:x:x:x:x:x:x`, de 1 à 4 chiffres hexadécimaux par groupe.
- `::` remplace une suite de groupes nuls et « can only appear once in an address ».
- Forme mixte `x:x:x:x:x:x:d.d.d.d` (32 bits de poids faible en décimal pointé).
- Préfixes (§2.3) : `adresse/longueur` ; on peut omettre les zéros de tête d'un groupe,
  pas ses zéros de fin : `2001:0DB8:0:CD3/60` est illégal.

### IPv6 : en entrée, tout accepter (RFC 5952 §4)
- « all implementations MUST accept and be able to handle any legitimate [RFC4291] format. »

### IPv6 : forme canonique en sortie (RFC 5952 §4–5, 7)
- §4.1 : « Leading zeros MUST be suppressed. » Un groupe `0000` s'écrit `0`.
- §4.2.1 : `::` « MUST be used to its maximum capability ».
- §4.2.2 : `::` « MUST NOT be used to shorten just one 16-bit 0 field ».
- §4.2.3 : la plus longue suite de groupes nuls est compressée ; à égalité, la première.
- §4.3 : chiffres hexadécimaux en **minuscules** (MUST).
- §5 : notation mixte RECOMMENDED seulement si l'adresse est reconnaissable comme
  contenant une IPv4 dans ses 32 bits de poids faible (ex. `::ffff:192.0.2.1`).
- §6 : avec un port, la forme `[adresse]:port` SHOULD être utilisée.
- §7 : un préfixe se représente comme une adresse.

### Zones (RFC 4007 §11)
- `<adresse>%<zone_id>` ; la zone n'a de sens que sur le nœud (§6) ; elle précède la
  longueur de préfixe (§11.7).

## Pièges

- Bibliothèques C historiques (`inet_aton`, `inet_addr`) : `010.0.0.1` y vaut
  `8.0.0.1` (octal). Une validation qui compare des chaînes plutôt que des valeurs
  binaires laisse passer des doublons (RFC 6943 §3.1.2).
- Une chaîne comme `10.11.12` peut être lue comme une adresse ou comme un nom d'hôte
  (RFC 6943 §3.1.1).
- PostgreSQL accepte en entrée de `cidr` des formes abrégées héritées des classes
  (`10` → `10.0.0.0/8`, `128.1` → `128.1.0.0/16`) : voir
  [postgresql-18-reseau.md](postgresql-18-reseau.md). L'application ne doit donc pas
  transmettre de texte brut de l'utilisateur à la base.
- Le comportement exact des bibliothèques d'analyse en Rust (std, crates de réseau)
  face aux zéros de tête et aux zones doit être **vérifié par des tests** lors du choix
  des crates (ADR), et non supposé.

## Implications pour kartenn (propositions pour la spec 002)

1. **IPv4 en entrée** : forme stricte uniquement, quatre décimaux de 0 à 255 **sans
   zéro de tête** (`010.0.0.1` refusé, pas réinterprété). Refus explicite avec un
   message qui cite la forme attendue.
2. **IPv6 en entrée** : toutes les formes légitimes de RFC 4291 (exigence MUST de
   RFC 5952 §4), y compris majuscules et forme mixte. La zone `%…` est refusée (voir
   la note IPv6).
3. **Préfixes en entrée** : `adresse/longueur` obligatoire (pas de longueur implicite
   « classful ») ; les bits d'hôte doivent être à zéro, sinon refus avec la forme
   corrigée suggérée (`192.168.1.5/24` → « vouliez-vous 192.168.1.0/24 ? »).
4. **Sortie** : toujours la forme canonique, quel que soit le canal (API, Web, exports) :
   décimal pointé pour IPv4, RFC 5952 pour IPv6, notation mixte seulement pour
   IPv4-mapped (que kartenn ne stocke de toute façon pas).
5. **Identité** : égalité et unicité sur la valeur binaire (famille, bits, longueur),
   jamais sur la chaîne.
6. Ces règles appartiennent au **module de famille d'adresses** (D5) : chaque famille
   fournit son analyseur strict, sa forme canonique et ses tests de propriétés
   (aller-retour analyse → canonique → analyse).

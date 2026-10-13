# Noms d'hôte, noms internationalisés, dates et texte Unicode

Base des types FQDN, date et texte des propriétés (spec 003, REQ-003-8).

## Sources

| Document | Statut | Consulté |
|---|---|---|
| [RFC 1035](https://www.rfc-editor.org/rfc/rfc1035) — Domain Names, Implementation and Specification | Internet Standard (STD 13) | 2026-10-14 |
| [RFC 1123 §2.1](https://www.rfc-editor.org/rfc/rfc1123#section-2.1) — Host Names and Numbers | Internet Standard (STD 3) | 2026-10-14 |
| [RFC 5890](https://www.rfc-editor.org/rfc/rfc5890) — IDNA2008, définitions | Proposed Standard | 2026-10-14 |
| [UTS #46](https://www.unicode.org/reports/tr46/) — Unicode IDNA Compatibility Processing | version 18.0.0, révision 36, 2026-08-31 | 2026-10-14 |
| [RFC 3339](https://www.rfc-editor.org/rfc/rfc3339) — Date and Time on the Internet | Proposed Standard | 2026-10-14 |
| [UAX #15](https://www.unicode.org/reports/tr15/) — Unicode Normalization Forms | Unicode Standard Annex | non relu (référence de NFC) |

## Ce que disent les sources

### Noms d'hôte
- RFC 1035 §2.3.1 : `<label> ::= <letter> [ [ <ldh-str> ] <let-dig> ]` ; « Labels must be
  63 characters or less. »
- RFC 1123 §2.1 : la première lettre peut être un **chiffre** ; « Host software MUST
  handle host names of up to 63 characters and SHOULD handle host names of up to 255
  characters » ; une chaîne doit être testée comme adresse décimale pointée avant
  d'être cherchée dans le DNS.
- RFC 1035 §2.3.4 et §3.1 : labels de 63 octets au plus, noms de 255 octets au plus **au
  format filaire** (octets de longueur et label racine compris). En présentation
  textuelle sans point final, cela donne 253 caractères au plus (déduction, le RFC ne
  cite pas ce nombre).

### Noms internationalisés (IDNA2008)
- RFC 5890 §2.3.1 : un label LDH est fait de lettres ASCII, chiffres et tirets, sans
  tiret initial ni final, 63 octets au plus. Les labels avec `--` en 3ᵉ et 4ᵉ position
  sont réservés (R-LDH) ; seuls ceux qui commencent par `xn--` servent à IDNA.
- §2.3.2.1 : un **A-label** commence par `xn--` et doit se décoder en un **U-label**
  (Unicode en NFC, valide IDNA) qui se réencode à l'identique (symétrie). Un `xn--` qui
  échoue à ce test est un « Fake A-label ».
- UTS #46 : couche de compatibilité IDNA2003 / IDNA2008. Le traitement « transitional »
  (`ß` → `ss`…) est **déprécié** : « Nontransitional Processing, which is fully
  compatible with IDNA2008, should be used in all cases. » Drapeaux pertinents :
  `UseSTD3ASCIIRules`, `CheckHyphens`.

### Dates (RFC 3339 §5.6–5.7)
- `full-date = date-fullyear "-" date-month "-" date-mday`, 4, 2 et 2 chiffres.
- Jours max : 31 (janvier, mars, mai, juillet, août, octobre, décembre), 30 (avril, juin,
  septembre, novembre), 28 ou 29 en février ; année bissextile si divisible par 4, sauf
  les siècles non divisibles par 400.

## Pièges

- `_` n'est pas permis dans un nom d'hôte, mais apparaît dans des noms DNS de service
  (`_sip._tcp…`, `_dmarc…`) : un type « FQDN d'hôte » et un type « nom DNS » ne sont pas
  le même type.
- Un label `xn--…` arbitraire n'est pas forcément un A-label valide (Fake A-label) : il
  faut vérifier l'aller-retour.
- Traitement transitional (déprécié) et non transitional donnent des A-labels différents
  pour `ß`, `ς`, ZWJ, ZWNJ (ex. `straße.example`).
- Une chaîne entièrement numérique et pointée (`10.1.2.3`) est syntaxiquement un nom
  LDH valide mais doit être traitée comme une adresse (RFC 1123 §2.1).
- Texte : deux chaînes visuellement identiques peuvent différer sans normalisation
  (`é` précomposé ou `e` + accent combinant).

## Implications pour kartenn (propositions pour la spec 003)

1. **Type FQDN = nom d'hôte** : traitement UTS #46 **non transitional**, avec
   `UseSTD3ASCIIRules`, `CheckHyphens`, contrôle des longueurs DNS ; stockage en A-label,
   minuscules, sans point final ; refus des Fake A-labels, des labels R-LDH autres que
   `xn--`, et des noms entièrement numériques en forme d'adresse IPv4. Limites : label
   ≤ 63 octets, nom ≤ 253 caractères.
2. Un type « nom DNS » permettant `_` (enregistrements de service) relève du futur
   module DNS, pas de la v0.1.
3. **Date** : `full-date` stricte (RFC 3339), sans heure ni fuseau ; validation du jour
   selon le mois et l'année.
4. **Texte** : normalisation NFC avant validation et stockage ; longueur comptée en
   caractères Unicode (points de code) après normalisation.
5. Implémentation : crates `idna` (UTS #46) et `unicode-normalization` : leurs options
   exactes et leur conformité sont à vérifier par des tests lors de leur introduction.

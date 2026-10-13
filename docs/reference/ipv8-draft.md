# « IPv8 » : état de draft-thain-ipv8

## Sources

| Document | Statut | Consulté |
|---|---|---|
| [draft-thain-ipv8-02](https://www.ietf.org/archive/id/draft-thain-ipv8-02.txt) | Internet-Draft **individuel**, « Intended status: Standards Track », daté du 17 avril 2026, **expire le 19 octobre 2026** | 2026-10-13 |
| [Datatracker draft-thain-ipv8](https://datatracker.ietf.org/doc/draft-thain-ipv8/) | révisions 00 à 02 (14, 15 et 17 avril 2026), sans groupe de travail | 2026-10-13 |
| [Registre IANA IP Version Numbers](https://www.iana.org/assignments/version-numbers/) | mis à jour le 2021-01-28 | 2026-10-13 |

## Ce que dit le draft

- Résumé : « a managed network protocol suite » ; « IPv4 is a proper subset of IPv8 » ;
  une adresse IPv8 dont le préfixe de routage est nul est une adresse IPv4.
- §4.1 : adresse de 64 bits notée `r.r.r.r.n.n.n.n` :
  - `r.r.r.r` : « 32-bit ASN Routing Prefix » (le numéro d'AS encodé en entier non
    signé, ordre réseau, §4.4) ;
  - `n.n.n.n` : « 32-bit Host Address (identical semantics to IPv4) ».
- §4.3 : `0.0.0.0.n.n.n.n` est routé selon les règles IPv4.
- Pas de notation CIDR définie pour une adresse IPv8 complète ; des notations à barre
  oblique apparaissent pour des plages du champ `r.r.r.r` (§4.5, §9.3).
- §20.1 : demande à l'IANA d'attribuer le **numéro de version 8**.

## Constats

- Le numéro de version 8 est **« Reserved (Historic) »** dans le registre IANA (RFC 1621,
  PIP), comme les versions 5, 7 et 9. La demande du draft entre donc en conflit avec le
  registre existant.
- Le draft contient des renvois internes incohérents (sections inexistantes ou décalées).
- Il expire le 19 octobre 2026 ; sans nouvelle révision, il deviendra caduc.
- Aucune implémentation de référence ni adoption par un groupe de travail n'a été trouvée.

## Implications pour kartenn

1. Confirme D5 : **IPv8 hors périmètre**. On ne fige rien sur ce format.
2. Leçon de conception utile, indépendamment du sort du draft : une famille d'adresses
   peut avoir une **largeur différente** de 32 et 128 bits, une **notation différente**
   (8 octets pointés) et une relation d'**inclusion avec une autre famille** (IPv4 ⊂
   IPv8). Le contrat de famille d'adresses de la spec 002 doit permettre ces trois cas,
   sans les implémenter.
3. À revoir si une révision 03 ou une adoption par un groupe de travail apparaît.

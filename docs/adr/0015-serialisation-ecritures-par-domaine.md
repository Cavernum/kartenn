---
status: accepted
date: 2026-10-14
---

# 0015 — Sérialisation des écritures par domaine d'adressage

## Context

Les règles globales du noyau (doublons, chevauchements, appartenance à un réseau,
unicité publique) décident en lisant l'état. Sans précaution, deux changesets
concurrents valides séparément produiraient ensemble une incohérence (INV-2, INV-5,
INV-6). Toutes ne sont pas exprimables en contraintes PostgreSQL (un réseau est une
feuille, unicité publique entre réseaux). Volume visé : 50 écritures/s soutenues (D14).

## Decision

Un changeset prend, dans sa transaction et dans un ordre déterministe, un verrou
consultatif (`pg_advisory_xact_lock`) par domaine d'adressage touché, plus un verrou
« espace public » s'il écrit un objet public. Les contraintes d'exclusion restent le
dernier rempart. Les lectures ne prennent aucun verrou.
Les arbres d'entités et de sites, qui ne relèvent d'aucun domaine, ont chacun un verrou
de même nature, pris par toute création, déplacement ou clôture d'une entité ou d'un
site (absence de cycle). Une modification de préfixe ou d'adresse prend le verrou
« espace public » quelle que soit sa valeur, car elle peut y entrer ou en sortir.

**Précisions (acceptation, 2026-10-17)** :
- Une clôture de préfixe, de plage ou d'adresse ne prend que le verrou de son domaine :
  retirer un objet ne peut pas créer de doublon dans l'espace public.
- La clé d'un verrou de domaine est faite des 64 bits de poids fort de son UUID v7. Une
  collision entre deux domaines sérialise inutilement leurs écritures, sans risque
  d'interblocage, puisque l'ordre de prise est déterministe.

## Alternatives

- Isolation SERIALIZABLE : correcte sans verrou explicite, mais échecs de sérialisation
  à rejouer et coût difficile à prévoir sur des prédicats d'inclusion. À réévaluer si
  les verrous deviennent un goulet.
- Contraintes en base seules : ne couvrent pas tous les invariants.

## Consequences

- Les écritures d'un même domaine sont sérialisées ; des domaines distincts écrivent en
  parallèle, sauf dans l'espace public.
- Un import massif dans un domaine bloque les autres écritures de ce domaine pendant
  sa transaction : les imports doivent être découpés en changesets de taille bornée
  (spec 004).

## Amendement (2026-10-21) — clés de verrou nommées des modules

Une opération de module (ADR 0026) qui n'est ancrée dans aucun domaine déclare des
**clés de verrou nommées** (`vlan.group:<uuid>`, spec 008, REQ-008-27). Le pipeline les
prend après les verrous de domaine et de l'espace public, triées et dédupliquées, pour la
durée de la transaction : verrous consultatifs à deux clés (une classe propre aux clés
nommées, puis le hachage du nom), qui ne se confondent jamais avec ceux des domaines.
L'ordre canonique (domaines, puis clés triées) exclut l'interblocage entre changesets
qui nomment les mêmes clés dans des ordres différents ; un changeset qui ne touche aucun
domaine se sérialise sur ses seules clés. Deux noms de même hachage partagent un verrou :
sérialisation de trop, jamais de trop peu.

## Amendement (2026-10-22) — clés nommées rangées en seaux

Le verrou d'une clé nommée n'est plus pris sur le hachage du nom mais sur son **seau** :
`hashtext(clé) & 1023`, l'un de 1024 seaux fixes, toujours dans la classe propre aux
clés nommées. Le pipeline prend les seaux distincts, en ordre croissant, un verrou par
seau, en une seule requête.

Raison : un verrou par clé épuisait la table des verrous partagée de PostgreSQL
(`max_locks_per_transaction` × `max_connections` entrées) dès qu'un changeset nommait des
milliers de clés, par exemple l'import de 10⁴ machines en un changeset (« out of shared
memory »). Élargir ce paramètre ne ferait que repousser la limite.

Conséquences :
- **Borne** : une transaction tient au plus 1024 verrous de clés nommées, quel que soit
  le nombre de clés qu'elle cite.
- **Exclusion préservée** : une même clé tombe toujours dans le même seau, donc deux
  changesets qui la nomment se sérialisent. Deux clés sans rapport qui partagent un seau
  se sérialisent aussi : sérialisation de trop, jamais de trop peu, comme une collision
  de hachage.
- **Pas d'interblocage** : l'ordre canonique demeure (domaines, puis seaux croissants).
- **Pas de seuil** : toutes les clés passent par les seaux, quel que soit leur nombre ;
  mêler verrous par clé exacte et verrous par seau casserait l'exclusion entre deux
  changesets de tailles différentes.
- Les verrous de domaine restent un par domaine : un changeset ne touche que des
  domaines de l'instance, en nombre réduit.

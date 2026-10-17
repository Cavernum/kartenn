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

---
status: accepted
date: 2026-10-14
---

# 0004 — PostgreSQL ≥ 18 comme premier adaptateur de stockage

## Context

Invariants d'unicité et de non-chevauchement sous concurrence, types réseau, temporel,
volumétrie de 10⁶ objets avec historique. Décision D15 ([journal](../../specs/001-cadrage/decisions.md)) ; sources :
[postgresql-18-reseau](../reference/postgresql-18-reseau.md),
[temporel](../reference/temporel-sql2011-postgresql.md).

## Decision

Nous utilisons PostgreSQL ≥ 18 derrière des ports de stockage. Les invariants critiques
sont validés dans l'application **et** garantis par des contraintes en base. Les
préfixes sont des `cidr`, les adresses des `inet`. Les contraintes d'exclusion sur
`inet`/`cidr` nomment explicitement `inet_ops` : la classe par défaut de btree_gist
pour ces types est défectueuse en 18 (défaut de conception, non rétroporté ; corrigé
par le changement de défaut de la 19). On reste en 18 jusqu'à la sortie de la 19 et de
sa première version corrective.

## Alternatives

- Bases distribuées (CockroachDB, YugabyteDB) : écritures réparties, mais support
  partiel de GiST, des exclusions et des types réseau ; inutile à 10⁶ objets.
- Base orientée graphe : naturelle pour la topologie, faible sur les contraintes et le
  temporel ; envisageable plus tard comme projection en lecture.

## Consequences

- PostgreSQL 18 n'a ni tables versionnées système ni `FOR PORTION OF` : kartenn les
  implémente (ADR 0005).
- Les clés étrangères temporelles n'offrent que `NO ACTION` : cascades par
  l'application (D33).
- Un seul primaire en écriture (ADR 0012).
- Une famille d'adresses hors v4/v6 exigera une autre représentation dans l'adaptateur.

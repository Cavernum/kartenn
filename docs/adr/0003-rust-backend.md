---
status: accepted
date: 2026-10-13
---

# 0003 — Rust pour le backend

## Context

Performance à l'échelle de 10⁶ objets (D14), validation stricte (D1), modules compilés
(ADR 0001). Décision D16 ([journal](../../specs/001-cadrage/decisions.md)).

## Decision

Le backend est écrit en Rust, en workspace Cargo ; un module = une ou plusieurs crates,
sélectionnées par features. Pistes d'écosystème, chacune à confirmer par la conception
qui l'introduit : axum/tower (HTTP), sqlx (PostgreSQL), serde, utoipa (OpenAPI).

## Alternatives

- Go : simple et rapide à compiler, mais typage moins expressif pour encoder les
  invariants (« parse, don't validate »).
- Python (FastAPI, Pydantic) : très productif, validation forte, mais moindre marge sur
  les budgets et la concurrence.

## Consequences

- Les invariants peuvent être portés par les types (une valeur validée n'existe que
  validée).
- Temps de compilation à maîtriser : voir [rust-build](../practices/rust-build.md).

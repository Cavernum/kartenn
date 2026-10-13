---
status: accepted
date: 2026-10-13
---

# 0001 — Architecture hexagonale, modularité par couche

## Context

kartenn doit servir plusieurs interfaces (REST, Web, puis CLI, TUI, MCP), accueillir des
modules métier (VLAN, machines, politique de flux…) et des sources de données externes,
tout en garantissant des invariants transactionnels stricts (D1). Décisions D6, D7
([journal](../../specs/001-cadrage/decisions.md)).

## Decision

Nous adoptons une architecture hexagonale : un domaine pur (noyau + modules), une
couche service (cas d'usage, pipeline d'écriture), des ports de stockage, et des
adaptateurs (REST, PostgreSQL). La modularité diffère selon la couche :
- domaine, service et données : **modules compilés** (crates Rust), sélectionnés au build ;
- interfaces non Web : binaires séparés, clients de l'API ;
- frontend : **plugins chargés à chaud** ;
- connecteurs : processus externes.

## Alternatives

- Microservices par module : isolation forte, mais les invariants entre modules
  deviennent des transactions distribuées, ce qui contredit D1.
- Plugins backend chargés à chaud (dynamiques, WASM) : ABI stable à maintenir, modèle de
  sécurité du code tiers, migrations dynamiques ; trop coûteux.

## Consequences

- Un module tiers côté données exige une recompilation.
- Le contrat de module (migrations, propriétés, règles, permissions, ressources) devient
  une interface interne à stabiliser tôt.
- Le domaine reste testable sans base ni HTTP.

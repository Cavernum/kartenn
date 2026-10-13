# Specs de fonctionnalités

| Spec | Exigences | Conception | Tâches |
|---|---|---|---|
| [001-cadrage](001-cadrage/decisions.md) | — (journal D1–D37) | — | — |
| [002-noyau-adressage](002-noyau-adressage/) | approuvées | approuvée | [tasks](002-noyau-adressage/tasks.md) |
| [003-proprietes](003-proprietes/) | approuvées | approuvée | [tasks](003-proprietes/tasks.md) |
| [004-changesets-bitemporel](004-changesets-bitemporel/) | approuvées | approuvée | [tasks](004-changesets-bitemporel/tasks.md) |
| 005-authn-rbac | à écrire | — | — |
| 006-api-rest-conventions | à écrire | — | — |
| 007-web-shell-plugins | à écrire | — | — |
| 008-vlan, 009-machines-interfaces | à écrire | — | — |
| 010-politique-flux, 011-registre-flux | à écrire avant de figer le noyau (D31) | — | — |

## Ordre d'ensemble du noyau v0.1 (002, 003, 004)

Les trois specs sont imbriquées : on avance par phases, et chaque tâche porte sa phase.
Une phase ne commence que lorsque la précédente est verte.

| Phase | But | Tâches |
|---|---|---|
| **P0 — Fondations** | Workspace Cargo, outillage, base de test | 002 : 1 à 4 |
| **P1 — Prototypes de risque** | Lever les trois inconnues techniques **avant** le code définitif ; résultats consignés dans `docs/spikes/` | 002 : 5, 6 ; 004 : 1 ; 003 : 1 |
| **P2 — Domaine pur** | Familles d'adresses, règles, types de valeurs, arbres, découpage des périodes : sans base | 002 : 7 à 14 ; 003 : 2 à 5 ; 004 : 2, 3 |
| **P3 — Stockage** | Migrations, tables courantes et d'historique, contraintes, lectures datées | 004 : 4 à 6 ; 002 : 15 à 17 ; 003 : 6, 7 |
| **P4 — Pipeline d'écriture** | Changesets, validation globale, résolution, différentiel, droits, approbations, propositions | 004 : 7 à 15 ; 002 : 18 à 22 ; 003 : 8 à 13 |
| **P5 — Lectures et exploitation** | Arbre, occupation, prochain libre, filtrage, anomalies, audit, imports, annulation, outbox | 002 : 23 à 26 ; 003 : 14, 15 ; 004 : 16 à 22 |
| **P6 — Budgets et synchronisation** | Bancs CI bloquants, synchronisation de `docs/spec/`, ADR proposés → acceptés | 002 : 27, 28 ; 003 : 16, 17 ; 004 : 23, 24 |

Le RBAC (005) n'est pas encore spécifié : en P4, les contrôles de droits passent par une
interface (`Authorizer`, avec `authorize_many`) et une implémentation de test
configurable ; l'implémentation réelle viendra avec la 005.

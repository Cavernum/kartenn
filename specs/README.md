# Specs de fonctionnalités

| Spec | Exigences | Conception | Tâches |
|---|---|---|---|
| [001-cadrage](001-cadrage/decisions.md) | — (journal D1–D37) | — | — |
| [002-noyau-adressage](002-noyau-adressage/) | implémentées | implémentée | [tasks](002-noyau-adressage/tasks.md) (28/28) |
| [003-proprietes](003-proprietes/) | implémentées | implémentée | [tasks](003-proprietes/tasks.md) (19/19, amendement P17) |
| [004-changesets-bitemporel](004-changesets-bitemporel/) | implémentées | implémentée | [tasks](004-changesets-bitemporel/tasks.md) (25/25) |
| [005-authn-rbac](005-authn-rbac/) | implémentées (un arbitrage ouvert, REQ-005-25) | implémentée | [tasks](005-authn-rbac/tasks.md) (17/17) |
| 006-api-rest-conventions | à écrire | — | — |
| 007-web-shell-plugins | à écrire | — | — |
| 008-vlan, 009-machines-interfaces | à écrire | — | — |
| 010-politique-flux, 011-registre-flux | à écrire avant de figer le noyau (D31) | — | — |
| [012-journalisation-audit](012-journalisation-audit/) | implémentées | implémentée | [tasks](012-journalisation-audit/tasks.md) (22/22) |

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

En P4, les contrôles de droits passaient par une interface (`Authorizer`, avec
`authorize_many`) et une implémentation de test configurable ; la 005 l'a implémentée
(`PgAuthorizer`).

## Point d'étape — 2026-10-17

Noyau v0.1 implémenté : 002 : 28/28 · 003 : 18/18 · 004 : 25/25 ; budgets du noyau tenus
au banc `bench-core` (002/A14) ; `docs/spec/` synchronisée, ADR 0014, 0015, 0017 et 0018
acceptés ; 227 tests (nextest, toutes fonctionnalités), `devenv test` vert. Ce qui a été
différé à la synchronisation est listé à la fin des tâches de la 002 et de la 004
(permissions par type d'objet → 005 ; configuration, ordonnanceur des jobs et lectures
servies par l'API → 006).

## Point d'étape — 2026-10-17 (soir) : spec 012

Journalisation et audit implémentés (22/22) : journal d'audit en base, scellé et exporté
(ADR 0019 acceptée) ; logs techniques sur plusieurs sorties à la fois (stdout, fichier,
journald, syslog, OTLP) ; OpenTelemetry (traces avec spans de base de données, métriques,
logs) ; bibliothèque `syslog-sender` (MPL-2.0) ; console de développement (collecteur
OpenTelemetry, OpenObserve et son serveur MCP) ; 315 tests. Deux questions ouvertes de la
012 : le volume de l'audit des lectures est accepté, réglable (L25) ; reste, avant la
mise en production, le surcoût de bout en bout du changeset unitaire séquentiel.

Défauts trouvés par les tests et le banc de la 012, corrigés :
- fuite de secrets : champs bruts exportés en OTLP (logs et spans), rapport de panique
  par défaut, mot de passe de l'URL dans l'erreur de connexion ;
- relais d'audit, scellement et relais d'outbox relisant toute l'histoire à chaque passe
  (planchers, migration 0027, amendement 3 de l'ADR 0018) ; débit borné à un lot par
  tour ;
- service et store sans aucune instrumentation, métriques de changeset jamais
  alimentées ;
- sortie fichier rendue avant l'écriture effective ; arrêt d'OpenTelemetry bloquant le
  runtime ; corps des événements d'audit en OTLP non structuré, JSON des messages syslog
  non analysé par le collecteur de développement.

### Décisions de la v0.1 (tranchées le 2026-10-14)

Toutes tranchées le 2026-10-14, selon la recommandation : 002/A12 (1), 003/P9 (2),
003/P10 (3), 002/A13 (4). Exigences, conceptions et ADR 0016 amendés en conséquence.

| # | Sujet | Où | Ce qui attend |
|---|---|---|---|
| 1 | Plages : type `inetrange` **sans** `subtype_diff`, `CHECK` de famille | [prototype 001](../docs/spikes/001-gist-exclusions.md) | table des plages (002-15), règles des plages (002-20), clé naturelle (002-17), occupation avec plages (002-24) |
| 2 | Résolution des propriétés : matérialiser l'**origine** et non la valeur | [prototype 003](../docs/spikes/003-resolution.md) | tables des propriétés (003-6) et toute la P4 de la 003 ; ancres `core.owner`/`core.site` (002-22) |
| 3 | Budget des changements d'origine au-delà de 10⁵ objets | [prototype 003](../docs/spikes/003-resolution.md) | REQ-003-26 |
| 4 | Contradiction anycast entre domaines (REQ-002-25, 32, 46) | [002/implementation.md](002-noyau-adressage/implementation.md) (I1) | REQ-002-46 inatteignable en l'état |

### Écarts corrigés en cours de route

- ADR 0018 : le curseur unique de l'outbox pouvait sauter un événement ; remplacé par un
  enregistrement des livraisons (migration 0006, test qui reproduit le cas).
- ADR 0018 (second amendement) : la barrière `xmin` du relais d'outbox couvrait toute
  l'instance ; retirée, l'ordre reste garanti par domaine.
- ADR 0017 (amendement) : l'historique copiait les colonnes par position, ce qui cassait
  toute table versionnée après un `ADD COLUMN` ; copie par nom (migration 0009).
- REQ-002-46 précisée : conteneurs publics hors unicité, occurrences anycast de même valeur.
- Règles de migration issues des prototypes : pas de bloc `EXCEPTION` dans les fonctions
  utilisées par types, index ou déclencheurs ; noms qualifiés ou `search_path` fixé.

### Écarts corrigés à la synchronisation (rapport spec-keeper du 2026-10-17)

- Le job de migration ne chargeait pas les registres IANA (`kartenn_is_public` vide hors
  tests).
- Les anomalies d'une plage acquittée ne revenaient jamais ; filtres par domaine et entité
  ajoutés (REQ-002-36).
- Aucune opération ne renommait un slug (REQ-002-47) : `Rename`.
- N'importe quel auteur pouvait créer ou désactiver une politique d'approbation :
  `core.approval_policy.admin` exigé.
- L'événement d'un changeset ne portait pas le différentiel résumé (REQ-004-36) ; l'ordre
  par domaine n'avait pas de test (REQ-004-37, précisée).
- Le contrat de famille d'adresses portait toutes les particularités d'IPv4 et d'IPv6
  (REQ-002-1).

## Point d'étape — 2026-10-18 : spec 005

Authentification et RBAC implémentés (17/17) : BFF OIDC à cookie chiffré (ADR 0020),
jetons d'accès RFC 9068, comptes de service à secret généré (HMAC, K8), comptes de
secours à phrase de passe et TOTP ; RBAC par périmètre évalué en SQL, émetteurs confinés
à leurs entités (ADR 0021), administration par changesets sans escalade ; 372 tests.
Budgets tenus avec 1 000 liaisons et 50 groupes (banc `bench-rbac`) ; le surcoût relatif
n'est pas borné (K15). Corrigés après l'audit spec-keeper : un code TOTP accepté deux
fois par deux requêtes simultanées, un compte de secours désactivable par changeset,
l'annulation muette des changesets d'administration, des événements d'audit jamais
émis. Un arbitrage ouvert : REQ-005-25 « annulable ». Ce qui revient à la couche HTTP est
listé à la fin du design de la 005.

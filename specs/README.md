# Specs de fonctionnalités

| Spec | Exigences | Conception | Tâches |
|---|---|---|---|
| [001-cadrage](001-cadrage/decisions.md) | — (journal D1–D37) | — | — |
| [002-noyau-adressage](002-noyau-adressage/) | implémentées | implémentée | [tasks](002-noyau-adressage/tasks.md) (28/28) |
| [003-proprietes](003-proprietes/) | implémentées | implémentée | [tasks](003-proprietes/tasks.md) (19/19, amendement P17) |
| [004-changesets-bitemporel](004-changesets-bitemporel/) | implémentées | implémentée | [tasks](004-changesets-bitemporel/tasks.md) (25/25) |
| [005-authn-rbac](005-authn-rbac/) | implémentées | implémentée | [tasks](005-authn-rbac/tasks.md) (18/18) |
| [006-api-rest](006-api-rest/) | implémentées | implémentée | [tasks](006-api-rest/tasks.md) (26/26) |
| [007-web-shell-plugins](007-web-shell-plugins/) | implémentées sauf REQ-007-32 (budgets du navigateur) | implémentée | [tasks](007-web-shell-plugins/tasks.md) (25/26, reste 24 : banc `bench-web`) |
| [008-vlan](008-vlan/) | implémentées, sauf REQ-008-22 en écart (422 au lieu de 403) | implémentée (ADR 0026, amendement 1) | [tasks](008-vlan/tasks.md) (20/20) |
| [009-machines-interfaces](009-machines-interfaces/) | implémentées, sauf REQ-009-21 (banc complet à relancer) | implémentée | [tasks](009-machines-interfaces/tasks.md) (13/14, reste 12 : banc complet) |
| [010-politique-flux](010-politique-flux/), [011-registre-flux](011-registre-flux/) | rédigées, révisées par l'expression du besoin (D28, D30 amendées) ; v0.2 | rédigée | [010](010-politique-flux/tasks.md), [011](011-registre-flux/tasks.md) (0) |
| [012-journalisation-audit](012-journalisation-audit/) | implémentées | implémentée | [tasks](012-journalisation-audit/tasks.md) (22/22) |
| [013-binaires-de-tests](013-binaires-de-tests/) | — (outillage, [ADR 0027](../docs/adr/0027-un-binaire-de-tests-par-crate.md)) | ADR 0027 | [tasks](013-binaires-de-tests/tasks.md) (9/9) |
| [014-base-de-test-gabarit](014-base-de-test-gabarit/) | — (outillage, [ADR 0028](../docs/adr/0028-base-de-test-gabarit.md)) | ADR 0028 | [tasks](014-base-de-test-gabarit/tasks.md) (7/7) |
| [015-paquetage-deploiement](015-paquetage-deploiement/) | implémentées ([ADR 0029](../docs/adr/0029-paquetage-nix-oci.md), D35 amendée) | implémentée | [tasks](015-paquetage-deploiement/tasks.md) (9/10 : reste l'essai `compose.yaml`) |
| [016-noyau-v0-2](016-noyau-v0-2/) | implémentées (N9 à remesurer au calme) | implémentée | [tasks](016-noyau-v0-2/tasks.md) (13/13) |

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

Authentification et RBAC implémentés (18/18) : BFF OIDC à cookie chiffré (ADR 0020),
jetons d'accès RFC 9068, comptes de service à secret généré (HMAC, K8), comptes de
secours à phrase de passe et TOTP ; RBAC par périmètre évalué en SQL, émetteurs confinés
à leurs entités (ADR 0021), administration par changesets sans escalade ; 372 tests.
Budgets tenus avec 1 000 liaisons et 50 groupes (banc `bench-rbac`) ; le surcoût relatif
n'est pas borné (K15). Corrigés après l'audit spec-keeper : un code TOTP accepté deux
fois par deux requêtes simultanées, un compte de secours désactivable par changeset,
l'annulation muette des changesets d'administration, des événements d'audit jamais
émis. REQ-005-25 « annulable » tranché (K16, tâche 18) : l'annulation inverse rôles,
liaisons et rattachements par des opérations ordinaires, sous les mêmes contrôles
(380 tests). Ce qui revient à la couche HTTP est listé à la fin du design de la 005.

## Point d'étape — 2026-10-19 : spec 006

API REST implémentée (26/26) : `/v1` additive et contrat OpenAPI 3.1 comparé à un
contrat de référence (ADR 0022 acceptée, Q-ouv-7 close) ; lectures datées et cache,
curseurs signés, filtres et `QUERY` ; écritures par changeset, Merge Patch, `If-Match`
obligatoire, `Idempotency-Key` ; imports par morceaux exécutés par des tâches de fond
sous bail dans PostgreSQL (ADR 0023 acceptée) ; Problem Details ; cookies, Bearer,
Basic, CSRF, quotas, délestage, HTTP/1.1 et HTTP/2 sur TLS ; 561 tests. Budgets tenus à
travers HTTP (banc `bench-http`), 200 utilisateurs et 50 écritures/s ; le banc a révélé
une page filtrée à 3 s, ramenée à 52 ms. `docs/spec/` synchronisée (spec-keeper du
2026-10-19 : chaque REQ-006 a son code ; aucun écart bloquant).

Reste hors de la 006 : la `source` des CloudEvents, encore `urn:kartenn:instance`, que
la 004 renvoyait à la configuration de la 006 ; elle rejoint le transport des
événements sortants, non-objectif de la 006.

## Point d'étape — 2026-10-21 : spec 007

Shell Web et plugins implémentés (25/26) : kartenn-server sert le shell et les plugins
sur la même origine que l'API, CSP avec l'empreinte de l'import map, SRI par
`modulepreload` (ADR 0025 acceptée) ; SDK `@kartenn/sdk` (MPL-2.0) partagé par import
map, client tiré du contrat ; quatre points d'extension et une colonne de liste, plugin
d'exemple sans framework et plugin VLAN de première partie ; contexte temporel dans
l'URL, modification rapide, brouillon local simulé en continu, changesets,
administration, arbre virtualisé ; i18n FR et EN ; WCAG 2.2 AA avec axe-core bloquant ;
émetteur de développement `kartenn-dev-idp` ; parcours Playwright contre le vrai
serveur sur Chromium et Firefox (82 parcours, 154 tests Vitest, 708 tests Rust).
Ajouts additifs à l'API : `GET /v1/domains/{id}/roots`, `GET /v1/imports`,
`GET /v1/auth/issuers`.

Reste : le banc `bench-web` (007-24, budgets REQ-007-32), en cours ; ses chiffres
rejoindront la spec système à sa clôture.

Corrigés en route : enfants d'un préfixe relus en entier avant le curseur (17 s par page
à 10⁴ enfants, ramenés à 7 ms) ; garde `Expect` d'un objet de module sur une permission
inexistante ; relais Vite de `/v1` et `/plugins` absent (REQ-007-36), ajouté à la
synchronisation.

Écarts restants, à arbitrer :
- une écriture hors du périmètre répond 422 `core.object.forbidden` (comportement du
  noyau), que le shell traite comme un refus ; la 007 et la 008 (REQ-008-22) parlaient
  de 403 ;
- aucune opération ne lie le premier administrateur d'une instance neuve : les parcours
  de bout en bout le posent en SQL (`web/e2e/serve.sh`) ;
- `dev/kartenn.toml` n'a pas de `[web] plugins_dir` : la validation exige un répertoire
  existant, relatif au répertoire courant, ce que le test de la configuration de
  développement, lancé depuis le crate, ne tient pas.

## Point d'étape — 2026-10-21 : spec 008

Contrat de module et module VLAN implémentés (20/20). Le noyau accueille des modules
compilés : registre ordonné selon le DAG (`ModuleRegistry`), migrations et schéma
PostgreSQL par module, table `module_object`, opérations en enveloppe
`Operation::Module` sous les contrôles du pipeline (verrous, droits, simulation,
annulation), ancres `core.owner`/`core.site` héritées le long de l'arbre du module,
catalogues de permissions et de règles fusionnés, références et suppression (refus,
cascade, mise à nul), sections de différentiel (`?section=`), obligations
d'administrateur, routes montées sous `/v1` avec `kartenn_rest::kit` et arêtes du
graphe. Le module `vlan` : groupes ancrés sur une entité, un site ou un domaine, VLAN
802.1Q, occupation et prochain VID libre, propriété `vlan.vlan` des réseaux et ses
avertissements, imports par morceaux, plugin `web/plugins/vlan`. La 009 (machines) a
éprouvé le contrat sans le refondre. ADR 0026 amendée (amendement 1) des ajouts faits en
route : `prepare`, `reads`, `CheckContext::{objects, changeset}`, `Findings`, catalogues
du processus, contrat écrit par `kartenn-server openapi`.

Budgets tenus au banc complet (2·10⁴ groupes, 10⁵ VLAN) : lecture d'un VLAN 3,7 ms,
page filtrée 38,5 ms (654 ms avant de lire les droits après la limite), occupation
2,9 ms, création 51,9 ms ; les budgets du noyau tiennent avec le module.

Corrigés en route : l'import ignorait les références locales dans le corps d'une
opération de module ; la garde `Expect` d'un objet de module visait une permission
inexistante ; une anomalie de module ne se fermait jamais (réévaluation par `Findings`,
bornée aux objets touchés) ; le délestage comptait les fichiers du shell et des plugins
(REQ-006-30 précisée : `/v1` seul).

Écarts restants, à arbitrer :
- REQ-008-22 demande 403 pour une écriture hors périmètre ; le code répond 422
  `core.object.forbidden`, comme le noyau ;
- aucune opération ne lie le premier administrateur d'une instance neuve ;
- le dépôt n'a pas de CI : la matrice « serveur sans module et avec » est une
  vérification locale (`cargo check -p kartenn-server --no-default-features`, sans
  module puis avec `vlan`), à porter dans la CI quand elle existera.

## Point d'étape — 2026-10-21 : spec 009

Machines et interfaces implémentées (13/14) : crate `kartenn-machines`, deuxième module
du contrat (ADR 0026), éprouvé sans refonte. Clusters, machines (nom d'hôte RFC 1123
unique par propriétaire, hébergement par une machine ou un cluster, sans cycle),
interfaces (MAC EUI-48, modes 802.1Q `none`/`access`/`trunk` sur des VLAN vivants),
rattachements datés d'adresses (M16 : écriture de l'interface, lecture de l'adresse),
propriétés `machines.interface_role` et `machines.fqdn`, anomalies de cohérence (MAC en
double, VLAN d'adresse, site de l'hôte) refermées une fois corrigées et réévaluées sur
les seuls objets touchés, références et cascades, section `machines.attachments`, port
de lecture daté pour la 010 (`kartenn_machines::read`), routes REST et lectures
croisées, imports, plugin `web/plugins/machines`. Règles des modules publiées dans
`docs/spec/regles-modules.md`, généré depuis le code.

Corrigés en route : la reprise d'un import fait seulement de rattachements (rejoué puis
refusé) ; la fiche d'une machine (91 ms → 7 ms, adresses lues par ses interfaces).

Reste : la tâche 12, banc complet des machines (10⁵ machines) ; la variante réduite tient
tous les budgets (fiche 7,2 ms, page filtrée 35 ms, machines d'un /24 24 ms, création
39 ms). Le banc complet tient le verrou PostgreSQL près d'une heure : à lancer quand rien
d'autre ne tourne.

## Point d'étape — 2026-10-21 : spec 015

Paquetage et déploiement implémentés (8/10) : `flake.nix` qui suit le nixpkgs de devenv,
paquet `kartenn` (binaire, shell, plugins `vlan` et `machines`), une seule image OCI non
root et sans shell (32 Mo), module NixOS `services.kartenn` éprouvé par une machine
virtuelle (`checks.vm` : migration oneshot, deux instances, amorçage du premier
administrateur, secrets absents, durcissement 1,1, migration en échec bloquante, base en
retard à 503, migration tuée puis rejouée, sauvegarde et restauration avec `audit
verify`). URL de la base par fichier (`[database] url_file`), `--version`, version
`0.1.0` et `CHANGELOG.md`. Documentation de l'exploitant dans `docs/exploitation/`,
intégration continue neutre `scripts/ci.sh` (verte).

Corrigés en route : `/health/ready` restait à 503 en production, le rôle d'instance ne
pouvant pas lire `_sqlx_migrations` (migration 0054, `SELECT` seul) ; le paquet en liens
symboliques, que le registre des plugins refuse ; une suppression forcée de base de test
refusée par un autovacuum (`kartenn-testing`).

Reste : démarrer l'essai `compose.yaml` avec l'image (aucun moteur de conteneurs sur la
machine de développement) et l'audit `spec-keeper` de la 015. Question ouverte : registre
et signature de l'image (E13).

## Point d'étape — 2026-10-21 : spec 016

Amendements du noyau pour la v0.2 implémentés (13/13), en trois files : propriétés
(listes partagées, type `external_ref`, lecture groupée expliquée `resolved_many`,
sources de valeurs attendues dans la transaction avec candidats multiples, axe déclaré
par l'administrateur) ; contrat de module et outbox (clés d'ordre, tâches de fond,
événements et mesures de module, politiques par défaut installées une fois) ;
changesets et identités (fin de proposition notifiée au module, proposition forcée et
opérations de revue, écriture pour le compte d'un demandeur sous
`Kartenn-On-Behalf-Of`). Migrations 0045 à 0047 et 0050 à 0055 ; ADR 0026
(amendement 2) et ADR 0018 (amendement 4) ; INV-60 et INV-61.

Écarts au design retenus, le code faisant foi (design corrigé) : `on_behalf_of` dans les
`details` des événements d'audit et non dans une colonne (scellement) ; événement
`authn.requester.created` (trois segments) ; permission de revue portée par les droits
requis de la proposition (pas de colonne `review_permission`) ;
`Module::review_operations` au lieu d'`OperationType::review_only` ; `measure` reçoit la
transaction. Corrigé en route : un droit `UPDATE (kind)` sur `person`, qui aurait permis
au rôle applicatif de changer la sorte d'un compte de secours, retiré avant fusion
(`rbac_schema`).

Reste : le budget N9 (512 objets × 6 propriétés < 30 ms) mesuré à 25,4 ms au banc réduit
avant l'index partiel 0047, à remesurer au calme ; clore une liste fermée dont une autre
emprunte les valeurs n'est pas refusé ; la simulation n'expose pas la politique qui
retiendrait le changeset. Relecture spec ↔ code faite à la main (pas d'audit
`spec-keeper` lancé depuis la tâche).

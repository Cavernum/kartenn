---
status: approved
updated: 2026-10-19
---

# kartenn — spécification système

> État : **cible de la v0.1**, en cours d'implémentation. Les specs 002 à 006 et 012
> sont livrées (noyau d'adressage, propriétés, changesets, authentification et RBAC,
> API REST, journalisation et audit ; voir
> [`../../specs/README.md`](../../specs/README.md)) ; les autres parties décrivent le
> système tel qu'il est conçu. Le document est resynchronisé (mode *sync*) à chaque
> livraison.

## Contexte

Les grands SI (≈ 10⁶ adresses et interfaces, ≈ 10⁵ réseaux, plusieurs entités juridiques
dont des SI hérités de rachats, des prestataires interconnectés) tiennent leur
cartographie réseau dans des outils vieillissants, peu extensibles, sans contrôle de
cohérence fort et sans vérification de sécurité.

kartenn est un **référentiel réseau déclaratif** : il est la source de vérité de
l'adressage et de la topologie, refuse toute écriture incohérente, garde l'historique
bitemporel, et sert de socle à des contrôles de sécurité (environnements, matrice de
flux, v0.2).

Systèmes externes :
- fournisseurs d'identité **OIDC** (plusieurs émetteurs : organisation, prestataires) ;
- **PostgreSQL ≥ 18** ;
- collecteur **OpenTelemetry** (OTLP) ;
- sources de données existantes, via des **connecteurs** hors processus (à terme).

## Objectifs et non-objectifs

Objectifs :
- Validation stricte : aucune incohérence acceptée en écriture (D1).
- Performance à l'échelle : budgets p95 chiffrés sur 10⁶ objets (D14), vérifiés en CI.
- Historique bitemporel interrogeable (D8, D10).
- Modularité : modules métier compilés, plugins UI à chaud, connecteurs externes (D6).
- Accès : OIDC multi-émetteurs par un BFF à cookie chiffré, jetons d'accès pour les
  clients, comptes de service en Basic, comptes de secours à deux facteurs ; RBAC par
  périmètre et par propriété, émetteurs confinés à leurs entités (D13, D36, 003/P6–P7,
  spec 005).
- Interfaces : REST et Web en v0.1 ; CLI, TUI et MCP ensuite, comme clients REST (D7).

Non-objectifs :
- Découverte automatique du réseau (D1) : la source est déclarative ; des connecteurs
  pourront importer.
- Pilotage des équipements (DHCP, DNS, pare-feu) : autre classe de produit (D1).
- Multi-tenant strict : un déploiement par organisation (D3).
- Atteignabilité effective des flux en v0.1/v0.2 : étape ultérieure (D20).

## Feuille de route (D31)

| Version | Contenu |
|---|---|
| v0.1 | Noyau d'adressage (002), propriétés (003), changesets et bitemporel (004), authentification et RBAC (005), API REST (006), shell Web et plugins (007), VLAN (008), machines et interfaces (009) |
| v0.2 | Politique de flux (010), registre de flux (011) — spécifiés avant de figer le noyau |
| v0.3 | Passerelles, routes, NAT |
| ensuite | Atteignabilité, connecteur de la source historique, CLI, TUI, MCP |

## Glossaire

| Terme | Définition |
|---|---|
| Entité | Organisation propriétaire (groupe, filiale, prestataire…), en arbre. Ancre RBAC. |
| Site | Emplacement (région, site, bâtiment…), en arbre. Ancre RBAC. |
| Domaine d'adressage | Espace où une adresse est unique (VRF, espace de routage). Toute donnée d'adressage y appartient. |
| Préfixe | Bloc CIDR d'un domaine ; soit **conteneur**, soit **réseau**. |
| Conteneur | Préfixe qui ne contient que des préfixes (agrégat, supernet). |
| Réseau | Préfixe de niveau 3 qui contient plages et adresses ; feuille de l'arbre des préfixes. |
| Plage | Intervalle d'adresses d'un réseau, avec un usage (DHCP, réservée, statique). |
| Adresse | Adresse IP d'un réseau, avec un usage (hôte, passerelle, VIP, anycast, réservée). |
| Famille d'adresses | Implémentation d'un protocole d'adressage (IPv4, IPv6) : analyse, forme canonique, arithmétique, registre à usage spécial. |
| Slug | Identifiant technique lisible et unique d'une entité, d'un site ou d'un domaine ; pas un nom affiché. |
| Propriété | Attribut typé attaché à un objet, déclaré par un module ou un administrateur, éventuellement héritable. |
| Affectation | Valeur posée explicitement sur un objet pour une propriété (y compris « aucune valeur »). |
| Valeur résolue | Valeur effective d'une propriété pour un objet à une date (affectation propre, sinon héritée). |
| Changeset | Ensemble d'opérations d'écriture validé puis appliqué atomiquement ; unité d'audit, d'approbation et d'événement. |
| Différentiel | Objets existants dont une valeur résolue change du fait d'un changeset. |
| Temps applicatif (`valid_at`) | Période où un fait est vrai dans le réseau réel ; fournie par l'utilisateur. |
| Temps système (`as_of`) | Période où kartenn a affirmé ce fait ; fixée par la base, immuable. |
| Anomalie | Résultat d'une règle d'avertissement : écriture acceptée, signalement consultable et acquittable. |
| Module | Paquet compilé qui ajoute objets, propriétés, règles, permissions, routes et vues. |
| Plugin UI | Extension du shell Web chargée à l'exécution (Web Component). |
| Connecteur | Processus externe qui importe ou synchronise des données via l'API. |
| Événement d'audit | Trace attribuée d'une action (qui, quoi, sur quoi, avec quel résultat), écrite en base dans la transaction de l'action ; fait foi. |
| Log technique | Enregistrement d'exploitation émis par `tracing` ; ne fait pas foi. |
| Sévérité | Gravité d'un enregistrement (ERROR, WARN, INFO, DEBUG, TRACE), traduite en syslog et OpenTelemetry. |
| Criticité | Classe de conservation d'un événement d'audit (`low`, `normal`, `high`), qui fixe sa rétention. |
| Sceau | Empreinte chaînée d'un lot d'événements d'audit, éventuellement signée ; rend une altération détectable. |
| Sortie | Destination d'un flux : sortie technique (stdout, fichier, journald, syslog, OTLP) ou sortie d'audit (fichier, syslog, OTLP). |

## Index

| Document | Contenu |
|---|---|
| [architecture.md](architecture.md) | Composants, pipeline d'écriture, modules, déploiement |
| [interfaces.md](interfaces.md) | API REST, authentification, SDK de plugins, connecteurs, configuration, journalisation et audit |
| [data.md](data.md) | Modèle de données, bitemporel, propriétés, stockage PostgreSQL |
| [invariants.md](invariants.md) | Propriétés qui doivent toujours tenir, et comment elles sont garanties |
| [regles-noyau.md](regles-noyau.md) | Catalogue des règles de validation du noyau (généré depuis le code) |
| [permissions.md](permissions.md) | Catalogue des permissions et des rôles livrés (généré depuis le code) |
| [evenements-audit.md](evenements-audit.md) | Catalogue des événements d'audit (généré depuis le code) ; [schéma JSON](evenements-audit.schema.json) de leur forme exportée |
| [`../adr/`](../adr/) | Architecture Decision Records |
| [`../../specs/`](../../specs/) | Specs de fonctionnalités et journaux de décisions |
| [`../reference/`](../reference/README.md) | Notes de lecture des sources primaires |
| [`../practices/`](../practices/) | Bonnes pratiques |

## Décisions

| ADR | Titre | Statut |
|---|---|---|
| [0001](../adr/0001-architecture-hexagonale-modulaire.md) | Architecture hexagonale, modularité par couche | accepted |
| [0002](../adr/0002-rest-frontiere-publique-unique.md) | L'API REST, seule frontière publique | accepted |
| [0003](../adr/0003-rust-backend.md) | Rust pour le backend | accepted |
| [0004](../adr/0004-postgresql-18.md) | PostgreSQL ≥ 18 comme premier adaptateur de stockage | accepted |
| [0005](../adr/0005-bitemporel.md) | Modèle bitemporel natif | accepted |
| [0006](../adr/0006-changesets.md) | Toute écriture passe par un changeset | accepted |
| [0007](../adr/0007-domaines-arbre-deduit.md) | Domaines d'adressage et arbre de préfixes déduit | accepted |
| [0008](../adr/0008-noyau-minimal-proprietes.md) | Noyau minimal et propriétés typées | accepted |
| [0009](../adr/0009-authentification.md) | OIDC multi-émetteurs et Basic Auth | accepted |
| [0010](../adr/0010-rbac.md) | RBAC composable, par périmètre et par propriété | accepted |
| [0011](../adr/0011-frontend-web-components.md) | Shell Vue 3 et plugins Web Components | accepted |
| [0012](../adr/0012-instances-sans-etat.md) | Instances sans état, base comme point de cohérence | accepted |
| [0013](../adr/0013-licence-agpl.md) | Licence AGPL-3.0 | accepted |
| [0014](../adr/0014-ancres-proprietes-du-noyau.md) | L'entité propriétaire et le site, propriétés du noyau | accepted |
| [0015](../adr/0015-serialisation-ecritures-par-domaine.md) | Sérialisation des écritures par domaine | accepted |
| [0016](../adr/0016-resolution-materialisee-proprietes.md) | Résolution matérialisée et versionnée des propriétés | accepted |
| [0017](../adr/0017-temps-systeme-tables-historique.md) | Temps système : table courante, historique, déclencheur | accepted |
| [0018](../adr/0018-outbox-ordonnee-par-domaine.md) | Outbox transactionnelle ordonnée par domaine (CloudEvents) | accepted |
| [0019](../adr/0019-journal-audit-scelle-en-base.md) | Journal d'audit en base, en échec fermé, scellé par lots chaînés | accepted |
| [0020](../adr/0020-session-bff-cookie-sans-etat.md) | Session du navigateur : BFF à cookie chiffré sans état | accepted |
| [0021](../adr/0021-rbac-predicats-sql-confinement.md) | RBAC évalué en SQL : périmètres hérités et confinement des émetteurs | accepted |
| [0022](../adr/0022-contrat-http-api.md) | Contrat HTTP de l'API : `/v1`, curseurs signés, `QUERY`, Merge Patch, `If-Match` | accepted |
| [0023](../adr/0023-taches-de-fond-sous-bail.md) | Tâches de fond sous bail dans PostgreSQL | accepted |
| [0024](../adr/0024-outillage-frontend.md) | Outillage du frontend : pnpm, Vite, ESLint, Vitest, Playwright | accepted |
| [0025](../adr/0025-plugins-web.md) | Plugins Web : répertoire servi, code de confiance, SDK partagé | accepted |
| [0026](../adr/0026-contrat-de-module.md) | Contrat de module : registre, schéma et migrations par module, `module_object`, opérations en enveloppe, ancres `core.owner`/`core.site` | accepted |
| [0027](../adr/0027-un-binaire-de-tests-par-crate.md) | Un binaire de tests d'intégration par crate (`tests/it/`), `kartenn-server::http` à part | accepted |
| [0028](../adr/0028-base-de-test-gabarit.md) | Bases de test clonées d'un gabarit migré (`#[kartenn_testing::pg_test]`, `WAL_LOG`) | accepted |

## Questions ouvertes

- Q-ouv-1 à Q-ouv-4 et Q-ouv-6 de
  [`001-cadrage`](../../specs/001-cadrage/decisions.md#questions-ouvertes) (licence du
  SDK, volumétrie réelle, critères d'inférence, stockage IPv8, langue).
- ~~Q-ouv-5~~ (identités multi-émetteurs) : close par 005/K2 ; ~~Q-ouv-7~~
  (versionnement d'API) : close par 006/H12 et l'ADR 0022 (`/v1` dans le chemin).
- ~~Implémentation du temps système~~ : tranchée par l'ADR 0017, accepté avec la
  spec 004.

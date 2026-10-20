---
status: proposed
date: 2026-10-20
---

# 0026 — Contrat de module

## Context

Le cadrage range le contrat de module parmi les « contrats conçus maintenant,
implémentation plus tard » (point B de `specs/001-cadrage/decisions.md`) ;
`docs/spec/architecture.md` (§ Modularité) en dresse la liste sans la détailler. Les
modules VLAN (008) et Machines et interfaces (009) de la v0.1 (D18, D31), puis
Politique (010) et Flux (011) en v0.2, en sont les premiers consommateurs. D6 fixe le
modèle : modules **compilés** (crates du workspace, sélectionnées par features),
recompilation et migration pour en ajouter un ; plugins UI à chaud (007/W6).

Ce que le noyau offre déjà et que le contrat doit réutiliser sans refonte :

- **Pipeline d'écriture unique** (`kartenn-service::pipeline`) : changesets, période,
  verrous par domaine (ADR 0015), validation locale puis globale, différentiel, droits
  (`WriteTx::rbac_outside`), approbation, journal, outbox (ADR 0018), audit (ADR 0019).
  `Operation` et `ObjectKind` sont des énumérations fermées ; `WriteTx` est un trait
  générique (méthodes `impl Future`), donc non utilisable en objet dynamique.
- **Propriétés** (spec 003) : un module déclare déjà ses propriétés dans son code
  (`declarations::ModuleDeclarations`, synchronisées par `sync_modules`, P12) ;
  `InheritTree::Module(String)` prévoit l'arbre d'héritage d'un module ; les porteurs
  (`CORE_CARRIERS`) et cibles de référence (`REF_TARGETS`) sont des listes fixes.
- **Ancres du RBAC** (ADR 0014, 0021) : `kartenn_owner` lit la valeur résolue de
  `core.owner` de **n'importe quel objet** ; `kartenn_site` ne connaît que les réseaux ;
  `kartenn_visible`/`kartenn_readable` raisonnent par type d'objet.
- **Catalogues statiques** : permissions (`kartenn-core::authz::PERMISSIONS`), règles
  (`kartenn-core::rules`), `docs/spec/permissions.md` généré.
- **Temps** : tables versionnées par `kartenn_make_versioned` (ADR 0017), période de
  validité, `object_version` (ETag), `external_id`, anomalies, `Restore` (annulation).
- **REST** (`kartenn-rest`, ADR 0022) : routeur `OpenApiRouter` par ressource, contrat
  OpenAPI comparé à une référence, évolution additive de `/v1`, `modules` dans
  `GET /v1/capabilities`.

## Decision

Un **module** est une crate `kartenn-<module>` du workspace, activée par une feature de
`kartenn-server`, qui implémente le trait `Module` de `kartenn-service` et s'enregistre
dans un **registre de modules** construit au démarrage. Le contrat comporte douze
clauses.

1. **Identité et version.** Un nom `[a-z][a-z0-9_]*` (l'espace de noms de ses clés,
   permissions, règles, types d'objets et opérations : `vlan.vlan`, `vlan.vid.duplicate`)
   et la version de la crate. Le nom `core` est réservé. `GET /v1/capabilities` liste
   chaque module actif (nom, version) dans `modules` (déjà présent).
2. **Dépendances (DAG).** Le module déclare les modules dont il dépend. Le registre
   refuse au démarrage un cycle, une dépendance absente ou un nom en double ; il ordonne
   migrations, synchronisation des déclarations et validations selon un tri topologique.
   Le noyau est la racine implicite.
3. **Migrations.** Chaque module livre ses migrations SQL (`crates/kartenn-<module>/
   migrations/`), appliquées par le job de migration **après** celles du noyau et de ses
   dépendances, dans un schéma PostgreSQL propre (`vlan`, `machines`). Leur suivi est
   tenu par module (version, somme de contrôle) dans une table du noyau
   `module_migration`. Une migration de module ne modifie jamais un objet du noyau ni le
   schéma d'un autre module. Les tables d'objets sont bitemporelles comme celles du noyau
   (colonnes `valid_period`, `sys_start`, `changeset_id`, `kartenn_make_versioned`).
4. **Types d'objets et ancres.** Chaque type d'objet d'un module (`vlan.group`,
   `vlan.vlan`, `machines.machine`, `machines.interface`) est inscrit, ligne par ligne,
   dans une table versionnée du noyau `module_object` (identifiant, module, type, parent
   éventuel, domaine d'adressage éventuel, période) écrite par le pipeline dans la même
   transaction. Cette table rend
   les objets de module visibles des mécanismes transverses sans les connaître :
   `carrier_object`, `object_version`, `external_id`, anomalies, historique,
   annulation. Les **ancres** de périmètre (D12) sont les propriétés du noyau
   `core.owner` et `core.site`, que le contrat étend aux types de module : un type
   déclare s'il les porte et si elles sont obligatoires ; il peut déclarer un arbre
   d'héritage (`InheritTree::Module`) le long de `module_object.parent_id` (une interface
   hérite du propriétaire de sa machine). `kartenn_owner` fonctionne tel quel ;
   `kartenn_site` et `kartenn_visible` apprennent les objets de module (périmètres
   entité et site, et périmètre de domaine pour un objet dont le type déclare un
   **ancrage de domaine** : la colonne `module_object.domain_id`, renseignée par le
   module, place l'objet dans le domaine pour le RBAC et le verrou par domaine (ADR
   0015) ; un groupe de VLAN ancré sur un domaine s'en sert. Un objet de module n'est
   jamais dans un périmètre de préfixe).
5. **Propriétés.** Inchangé : `ModuleDeclarations`, synchronisées par changeset
   système (P12). Les types d'objets de module deviennent **porteurs** et **cibles de
   référence** admissibles (`TypeSpec::Ref { target: "vlan.vlan" }`), ce qui permet à un
   module d'attacher ses objets à ceux du noyau sans toucher au schéma du noyau (le VLAN
   d'un réseau est la propriété `vlan.vlan` d'un préfixe).
6. **Opérations.** Un changeset porte les opérations d'un module dans une enveloppe
   `Operation::Module` dont l'étiquette JSON est qualifiée (`{"op": "vlan.create_vlan",
   …}`). Le module fournit un **gestionnaire** compatible avec un objet dynamique :
   analyse et validation locale (planification), cibles et domaines à verrouiller,
   application sur la transaction, validation globale après application, inverse pour
   l'annulation (`Restore`). Le gestionnaire accède au stockage par un accès dynamique à
   la transaction en cours (`ModuleTx`), que l'adaptateur PostgreSQL du module convertit
   vers sa connexion : le module garde son adaptateur dans sa crate (feature `pg`), le
   noyau ne connaît pas ses tables. Les opérations génériques du noyau (`Close`,
   `AssignProperty`, `Expect`, `BindExternal`, `Restore`, `Within`) s'appliquent aux
   objets de module par `ObjectKind::Module`.
7. **Règles.** Chaque règle a un identifiant qualifié, une sévérité et un message
   (`kartenn_core::rules::Rule`) ; les règles du module rejoignent le catalogue servi
   par l'API et les anomalies. Une règle de module ne peut pas affaiblir une règle du
   noyau.
8. **Permissions.** Catalogue `<module>.<type>.<action>` (`read`, `write`, `propose`,
   et des actions propres déclarées), fusionné au catalogue du noyau pour la validation
   des rôles et la génération de `docs/spec/permissions.md`. Les rôles livrés (K12)
   reçoivent les permissions de lecture des modules ; `*` les couvre.
9. **Ressources REST.** Le module fournit un `OpenApiRouter` monté sous `/v1` (collections
   au pluriel, en kebab-case, préfixées par rien d'autre que leur nom : `/v1/vlans`,
   `/v1/machines`), qui suit les conventions de la 006 (lectures datées, curseurs,
   filtres, `If-Match`, Problem Details, écritures par changeset d'une opération). Les
   routes transverses par objet (`history`, `properties`, `explanation`) sont fournies
   par le noyau pour chaque type de module. Ajouter un module est une évolution
   **additive** du contrat (006/H12) ; le contrat de référence OpenAPI est régénéré avec
   les modules livrés.
10. **Événements.** Aucun type d'événement propre en v0.1 : les objets de module
    figurent dans `org.kartenn.changeset.applied` comme ceux du noyau (objets, résumé du
    différentiel). L'ordre par domaine (ADR 0018) vaut pour un changeset qui touche un
    domaine ; un changeset qui ne touche que des objets de module est ordonné sur la
    clé « hors domaine » existante.
11. **Graphe de topologie.** Le module peut déclarer des **arêtes** typées entre objets
    (`machines.interface → address`, `machines.interface → vlan.vlan`) servies, à une
    date donnée, par une lecture du registre. En v0.1 elles servent à l'affichage et aux
    lectures croisées ; leur forme (source, cible, type, attributs, période) est celle
    que l'atteignabilité (D20) exploitera.
12. **Références et suppression (D33).** Toute référence d'un objet vers un autre (d'un
    module vers le noyau, entre modules) est déclarée avec sa politique : **refus**
    (par défaut), **cascade** (clôture explicite du dépendant dans le même changeset,
    sous ses propres droits) ou **mise à nul**. Clore un objet du noyau interroge les
    modules qui y font référence ; le refus liste les dépendants (RFC 9457).

Le **plugin UI** d'un module est un plugin de première partie (`web/plugins/<module>/`,
007/W6, REQ-007-11, ADR 0025), écrit avec le seul SDK public. Son manifeste nomme le
module (`"module": "vlan"`) : le serveur ne le publie dans `ui_plugins` que si le module
figure dans `modules` des capacités. Ses points d'extension (`view`, `tab`, `panel`,
`column`, `action`, 007/design « Plugins côté serveur ») visent les types du noyau
(`prefix`, `address`…) par leur nom court et les types de module par leur nom qualifié
(`vlan.vlan`, `machines.machine`) ; les écrans propres aux objets d'un module sont des
vues (`view`) de son plugin, le shell n'ayant pas de fiche générique pour eux. Le plugin
n'est pas une clause du contrat serveur : il ne passe que par l'API publique.

## Alternatives

- **Opérations de module dans l'énumération `Operation`, derrière des features** :
  typage statique, mais `kartenn-service` dépendrait des crates de module, qui dépendent
  de lui (cycle), et ajouter un module modifierait le noyau.
- **Opérations de module hors changeset** (routes propres, transactions propres) :
  perd l'atomicité, l'historique, la revue, l'annulation et l'audit communs (D11).
- **Pas de table `module_object`**, chaque module étend les vues du noyau
  (`carrier_object`, RBAC) par migration : deux modules réécrivent la même vue, ordre de
  migration fragile, et le noyau ne sait plus énumérer ses porteurs.
- **Tables de module dans le schéma `public`** : collisions de noms, droits et purge
  moins lisibles.
- **Ancres propres à chaque module** (colonne `entity_id` dans chaque table) : duplique
  la résolution de `core.owner`, que le RBAC lit déjà ; un objet de module ne pourrait
  pas hériter de son parent.
- **Modules chargés dynamiquement** (bibliothèques partagées) : rejeté par D6 (ABI Rust
  instable, sécurité du chargement).

## Consequences

- Le noyau gagne : la table `module_object` et `module_migration`, le registre de
  modules, l'enveloppe `Operation::Module`, `ObjectKind::Module`, l'accès dynamique
  `ModuleTx`, des catalogues de permissions et de règles fusionnés, des porteurs et
  cibles de référence étendus, `kartenn_site`/`kartenn_visible` étendus. Tout est
  additif ; aucune donnée du noyau ne migre.
- Un module se teste seul (sa crate, ses migrations, une base de test) et dans le
  serveur complet ; une matrice CI construit le serveur sans module, et avec chacun.
- Le gestionnaire dynamique coûte une allocation par opération de module ; négligeable
  devant le budget d'un changeset (D14).
- Une opération de module mal formée est refusée à la planification, avant tout verrou,
  comme une opération du noyau.
- À réexaminer quand un module tiers hors du monorepo sera envisagé (contrat publié,
  versionné indépendamment).

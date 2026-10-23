---
status: accepted
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
dans un **registre de modules** construit au démarrage. Le contrat comporte quinze
clauses ; ses points différés et la couverture des besoins de la 010 et de la 011
suivent.

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
   `Operation::Module` dont l'étiquette JSON est qualifiée par le nom du module, le reste
   étant libre (`[a-z][a-z0-9_.]*` : `vlan.create_vlan`, `policy.cell.set`). Le module
   fournit un **gestionnaire** compatible avec un objet dynamique : analyse et
   validation locale (planification) ; **opérations du noyau** que l'opération planifie
   dans le même changeset (affectation de `core.owner` à la création d'un groupe,
   valeur de liste fermée créée avec ses attributs), soumises à leurs propres contrôles et
   droits ; **clés de verrou** (clause 14) ; application sur la transaction ; inverse pour
   l'annulation (`Restore`, REQ-004-34). Simulation, idempotence (`Idempotency-Key`),
   droits, approbation et journal sont ceux du pipeline, sans code du module. Le
   gestionnaire accède au stockage par un accès dynamique à la transaction en cours
   (`ModuleTx`), que l'adaptateur PostgreSQL du module convertit vers sa connexion : le
   module garde son adaptateur dans sa crate (feature `pg`), le noyau ne connaît pas ses
   tables. Dans cette transaction, le module lit l'état **avant** de ses tables par leur
   historique de temps système (ADR 0017). Les opérations génériques du noyau (`Close`,
   `AssignProperty`, `Expect`, `BindExternal`, `Restore`, `Within`) s'appliquent aux
   objets de module par `ObjectKind::Module`.
7. **Règles et anomalies.** Chaque règle a un identifiant qualifié, une sévérité et un
   message (`kartenn_core::rules::Rule`). Le gestionnaire les évalue sur l'**état final**,
   dans la transaction, pour les objets touchés par le changeset : les siens, ceux du
   noyau et ceux des modules dont il dépend (le pipeline lui passe la liste typée des
   objets touchés et les sections de différentiel, clause 13). Les avertissements
   deviennent des **anomalies** par le chemin commun du noyau (REQ-002-36) : listables,
   filtrables, acquittables, et fermées quand une réévaluation par le module ne trouve
   plus la cause : `check` rend ses constats (`Findings`), les violations et les objets
   réévalués avec la règle ou le préfixe de règles réévalué (009-7). Une règle de module
   ne peut pas affaiblir une règle du noyau.
8. **Permissions.** Catalogue `<module>.<type>.<action>` (`read`, `write`, `propose`,
   et des actions propres déclarées), avec des permissions **globales** (sans périmètre,
   `global_only`) comme celles de l'administration du noyau ; fusionné au catalogue du
   noyau pour la validation des rôles et la génération de `docs/spec/permissions.md`.
   Les rôles livrés (K12) reçoivent les permissions de lecture des modules ; `*` les
   couvre.
9. **Ressources REST.** Le module fournit un `OpenApiRouter` monté sous `/v1`. Ses
   collections sont au pluriel, en kebab-case (`/v1/vlans`, `/v1/machines`, `/v1/flows`) ;
   le registre refuse deux modules qui déclarent la même collection, ou une collection du
   noyau. Ses routes suivent les conventions de la 006 (lectures datées, curseurs,
   filtres et `QUERY`, `If-Match`, Problem Details, écritures par changeset d'une
   opération). Les routes transverses par objet (`history`, `properties`,
   `explanation`) sont fournies par le noyau pour chaque type de module. Ajouter un
   module est une évolution **additive** du contrat (006/H12) ; le contrat OpenAPI de
   référence est régénéré avec les modules livrés, et le module figure dans `modules`
   de `GET /v1/capabilities`.
   *Note (2026-10-23, spec 023, A5) :* le module décrit ses collections aux clients
   découverts (ADR 0035) par l'extension `x-kartenn` que produit le kit :
   `kit::presented` (clé naturelle parmi les filtres de la liste, colonnes) sur chaque
   collection, `kit::commanded` (nom de commande) sur une opération qui s'écarte des
   verbes conventionnels. Sans elle, les clients suivent les conventions ci-dessus.
10. **Événements.** Aucun type d'événement propre en v0.1 : les objets de module
    figurent dans `org.kartenn.changeset.applied` comme ceux du noyau (objets, résumé du
    différentiel). L'ordre par domaine (ADR 0018) vaut pour un changeset qui touche un
    domaine ; un changeset qui ne touche que des objets de module est ordonné sur la
    clé « hors domaine » existante. Types propres et clés d'ordre : différés (voir
    ci-dessous).
11. **Graphe de topologie.** Le module peut déclarer des **arêtes** typées entre objets
    (`machines.interface → address`, `machines.interface → vlan.vlan`) servies, à une
    date donnée, par une lecture du registre. En v0.1 elles servent à l'affichage et aux
    lectures croisées ; leur forme (source, cible, type, attributs, période) est celle
    que l'atteignabilité (D20) exploitera.
12. **Références et suppression (D33).** Toute référence d'un objet vers un autre (d'un
    module vers le noyau, valeurs de liste fermée comprises, ou entre modules) est
    déclarée avec sa politique : **refus** (par défaut), **cascade** (clôture explicite
    du dépendant dans le même changeset, sous ses propres droits) ou **mise à nul**.
    Clore un objet, du noyau ou d'un module, interroge les références déclarées par tous
    les modules : les dépendants d'un objet sont ainsi extensibles par les modules qui en
    dépendent (la 011 refuse la clôture d'une adresse ou d'une machine citée par un
    flux) ; le refus liste les dépendants (RFC 9457).
13. **Contributions au différentiel.** Après le différentiel des propriétés (étape G de
    la 003), le pipeline appelle, dans l'**ordre du DAG**, la contribution de chaque
    module qui en déclare une : elle lit dans la transaction le différentiel des
    propriétés, les objets touchés et les **sections des modules précédents**, et produit
    une **section** nommée (`<module>.<nom>`) de lignes typées (objet, avant, après,
    période). Les sections sont calculées en simulation comme à l'application, stockées
    avec le changeset, visibles des règles (clause 7), paginées et servies de façon
    générique (`GET /v1/changesets/{id}/diff?section=`).
14. **Clés de sérialisation hors domaine.** Outre les domaines (ADR 0015), un
    gestionnaire déclare des clés de verrou nommées (`vlan.group:<uuid>`, `policy`,
    `flow:<uuid>`), prises par le pipeline dans un ordre canonique **après** les verrous
    de domaine (verrous consultatifs de transaction). Amendement de l'ADR 0015 à
    l'implémentation de la 008.
15. **Ports de lecture et identités.** Un module peut exposer à ceux qui en dépendent
    des fonctions de lecture typées de sa crate (dépendance Rust conforme au DAG),
    utilisables dans la transaction par `ModuleTx` (la 009 expose les adresses d'une
    machine et les machines d'une adresse). Un objet de module ne référence une
    **identité** (005) que par son identifiant, sans copier de donnée personnelle : la
    pseudonymisation (REQ-004-33) s'y applique donc sans code du module.

Chaque capacité facultative est une méthode du trait `Module` ou `ModuleHandler` avec
une implémentation par défaut neutre : ajouter plus tard une capacité différée est
additif pour les modules existants.

Le **plugin UI** d'un module est un plugin de première partie (`web/plugins/<module>/`,
007/W6, REQ-007-11, ADR 0025), écrit avec le seul SDK public. Son manifeste nomme le
module (`"module": "vlan"`) : le serveur ne le publie dans `ui_plugins` que si le module
figure dans `modules` des capacités. Ses points d'extension (`view`, `tab`, `panel`,
`column`, `action`, 007/design « Plugins côté serveur ») visent les types du noyau
(`prefix`, `address`…) par leur nom court et les types de module par leur nom qualifié
(`vlan.vlan`, `machines.machine`) ; les écrans propres aux objets d'un module sont des
vues (`view`) de son plugin, le shell n'ayant pas de fiche générique pour eux. Le plugin
n'est pas une clause du contrat serveur : il ne passe que par l'API publique.

### Points différés à la v0.2

Aucun n'est utilisé par la 008 ni la 009 ; chacun s'ajoute par une méthode à
implémentation par défaut et une migration additive, sans toucher aux modules
existants. Ils sont livrés par la tâche de la 010 qui en a besoin.

- **Déclencheurs d'approbation de module** (N5) : déclencheurs nommés et paramétrés
  fournis par un module, évalués après les sections de différentiel, activés par
  l'administrateur seul (REQ-004-39) ; colonne de déclencheurs de module dans
  `approval_policy`. Différé : `approval_policy` change de schéma, et aucun module de la
  v0.1 n'a de déclencheur.
- **Référentiel de valeurs partagé** entre deux propriétés à liste fermée (N7, second
  volet) : amendement de la 003 (déclaration qui reprend les valeurs d'une autre).
  Différé : touche le modèle des propriétés, sans usage avant la 010.
- **Tâches de fond de module** (N14) : genres de tâches déclarés par un module et
  exécutés par l'ordonnanceur à bail (ADR 0023). Différé : aucune tâche en v0.1 ;
  l'ordonnanceur accepte un genre nouveau par enregistrement.
- **Événements de module** (N15) : types `org.kartenn.<module>.*` et **clés d'ordre de
  l'outbox** hors domaine (N6, second volet ; amendement de l'ADR 0018). Différé :
  aucun consommateur en v0.1 ; les objets de module voyagent déjà dans
  `org.kartenn.changeset.applied`.

### Couverture des besoins de la 010 et de la 011

Besoins listés par `specs/010-politique-flux/design.md` et `specs/011-registre-flux/
design.md` (§ « Exigences envers le noyau, le contrat de module et la 009 »).

| # | Besoin | Réponse | v0.1 |
|---|---|---|---|
| N1 | tables versionnées, migrations ordonnées par le DAG | clause 3 | fourni |
| N2 | opérations planifiées, validées, simulables, idempotentes, annulables, sous droits | clause 6 | fourni |
| N3 | règles sur l'état final, objets du noyau et des dépendances compris | clause 7 | fourni |
| N4 | contribution nommée au différentiel, stockée, paginée, servie | clause 13 | fourni |
| N5 | déclencheurs d'approbation de module | points différés | différé (v0.2) |
| N6 | clés de verrou hors domaine ; clés d'ordre de l'outbox | clause 14 ; points différés | verrous fournis, ordre différé |
| N7 | opérations du noyau planifiées par une opération de module ; référentiel de valeurs partagé | clause 6 ; points différés | premier volet fourni, second différé |
| N8 | obligations d'une propriété de module modifiables par l'administrateur | décision : les obligations ajoutées par un administrateur (changeset, portées domaine, entité, type, contrôle de REQ-003-6) sont marquées comme telles et la synchronisation (P12) ne compare et ne réécrit que celles du code ; aujourd'hui elle remplace la déclaration entière | fourni (008, tâche 6) |
| N10 | références vers le noyau, valeurs de liste fermée comprises | clause 12 | fourni |
| N11 | anomalies de module, sur ses objets et ceux des dépendances, fermées par le module | clause 7 | fourni |
| N12 | permissions de module globales, rôles livrés étendus | clause 8 | fourni |
| N13 | routes du module, `QUERY`, contrat de référence, capacités | clause 9 (collections sous `/v1/<collection>`, pas `/v1/<module>/`) | fourni |
| N14 | tâches de fond de module | points différés | différé (v0.2) |
| N15 | événements de module | clause 10 ; points différés | différé (v0.2) |
| N16 | état avant des tables du module dans la transaction | clause 6 (ADR 0017) | fourni |
| N21 | contributions dans l'ordre du DAG, lisant les précédentes | clause 13 | fourni |
| N22 | dépendants des objets du noyau extensibles par un module | clause 12 | fourni |
| N23 | référence à une identité sans copie, pseudonymisation | clause 15 | fourni (règle de conception) |

N9, N17 à N20 et N24 portent sur le noyau (002 à 004), hors de ce contrat ; M1 à M7
sur la 009 (`specs/009-machines-interfaces/`).

### Amendement 1 — 2026-10-21, à l'implémentation de la 008 et de la 009

Ajouts faits en implémentant le VLAN et les machines, tous **additifs** (méthodes à
défaut neutre, champs de contexte) ; aucun ne change une clause, ils en précisent la
forme. Sources : `crates/kartenn-service/src/module.rs`, `kartenn-rest/src/kit.rs`.

| Ajout | Clause | Tâche | Raison |
|---|---|---|---|
| `ModuleHandler::prepare` : complète une opération sur l'état de la transaction **avant** la planification ; l'opération complétée est celle que le changeset enregistre | 6 | 008-10 | `plan` ne lit aucune base ; un groupe ancré sur un domaine prend par défaut le propriétaire résolu du domaine |
| `ModuleHandler::reads` : couples (permission, objet) exigés pour les objets qu'une opération **cite sans les écrire**, contrôlés comme une garde (`core.object.forbidden`), jamais comme une proposition | 6, 8 | 009-4, 009-5 | M16 : rattacher une adresse exige la seule lecture de l'adresse ; le VLAN d'une interface exige `vlan.read` |
| `diff` et `check` portés par `ModuleHandler` (et non `Module`), avec `DiffContext` et `CheckContext` | 7, 13 | 008-19 | ils lisent la transaction, comme `apply` ; `Module` reste une description statique |
| Les règles des modules s'évaluent **après** le différentiel des propriétés et les sections, chaque module une fois, dans l'ordre du registre | 7 | 008-19 | une règle lit les sections (clause 13) |
| `CheckContext::objects` : objets de tout type écrits par le changeset (noyau compris) | 7 | 008-12 | règles d'un module sur des objets du noyau (`vlan.prefix.not_network`) |
| `CheckContext::changeset` : identifiant du changeset | 7 | 009-5 | copies qu'un module réécrit sur l'état final (usage partagé d'une adresse) |
| `check` rend des `Findings` : violations et objets réévalués par règle ; une anomalie ouverte d'un objet réévalué que rien ne relève est close | 7 | 009-8 | sans cela, une anomalie de module ne se fermait jamais ; la réévaluation se borne aux objets que le changeset touche (écrits, ou dont une valeur résolue utile change) |
| Catalogues de permissions et de règles du **processus**, complétés une fois à la construction du registre (`authz::register`, `rules::register`), refus d'un nom hors de l'espace du module ou déjà pris | 7, 8 | 008-6 | les violations relues du journal et la portée d'une permission globale n'ont pas le registre sous la main |
| Règle du noyau `core.object.dependents` (et non `core.dependents`) pour le refus de clôture d'un objet référencé | 12 | 008-7 | le catalogue impose trois segments |
| Corps d'opération de module invalide : refusé par une **règle** du module (`machines.operation.invalid` ; le VLAN rapporte ses erreurs de décodage sous ses règles d'ancre), pas par un 400 structurel | 6 | 009-2 | l'enveloppe est libre au-delà de l'étiquette ; une opération inconnue reste un 400 `bad-request` |
| Contrat OpenAPI de référence écrit par `kartenn-server openapi` (seul à connaître les modules livrés) ; `/v1/openapi.json` sert les routes réellement montées ; le binaire `openapi` de `kartenn-rest` est retiré | 9 | 008-8 | le contrat dépend des modules du build |
| Kit REST `kartenn_rest::kit` : lectures datées, curseurs, ETag, `create`, `modify`, `modify_nullable` (retrait d'un membre par `null`), `act` (opération sous l'`If-Match` d'un objet), `close` | 9 | 008-14, 009-10 | un module sert ses routes aux conventions de la 006 sans dupliquer le socle |
| Une arête a une seule extrémité par type : l'hébergement par un cluster est l'arête `machines.hosted_in` | 11 | 009-10 | forme de `EdgeType` |

Écart ouvert : une écriture hors du périmètre d'un rôle répond **422**
`core.object.forbidden`, comme pour le noyau, là où REQ-008-22 écrit 403 ; à trancher
(aligner la 008 sur le noyau, ou passer noyau et modules à 403).

### Amendement 2 — 2026-10-21, spec 016 (amendements du noyau pour la v0.2)

Les points différés à la v0.2 (N5, N14, N15) et les autres besoins du contrat de la 010
et de la 011, ajoutés eux aussi à défaut neutre. Source :
`crates/kartenn-service/src/module.rs`.

| Ajout | Clause | Tâche | Raison |
|---|---|---|---|
| `Module::jobs` (`JobSpec { name, period }`), `ModuleHandler::run_job(name, tx)` dans une transaction propre, validée si la tâche réussit ; `ModuleRegistry::jobs` ; le déploiement déclare `<module>.<tâche>` dans `job` (colonnes `module`, `enabled`) et désactive sans la supprimer une tâche que le build ne déclare plus ; l'ordonnanceur route un nom qualifié vers son module | 15 | 016-7 | N14 : tâches de fond d'un module sous le bail de l'ADR 0023 (REQ-016-16) |
| `Module::events` (noms `[a-z][a-z0-9_]*`, une fois ; sinon `RegistryError::InvalidEvent` au démarrage), `ModuleTx::emit(event, subject, data)` : un nom que le build ne déclare pas échoue l'opération ; les événements attendent le journal de la transaction (`journal_applied` pour un changeset, sous ses clés d'ordre ; `WriteTx::journal_job` pour une tâche, sous `<module>:<tâche>`, sans changeset), de type `org.kartenn.<module>.<nom>`, chacun son `id` CloudEvents | 15 | 016-8 | N15 : un événement de module n'existe que si sa transaction aboutit (REQ-016-15) |
| `Module::measures` et `ModuleHandler::measure(tx, context)`, lue après les règles du module sur le même `CheckContext` (une mesure absente vaut zéro, une mesure non déclarée échoue le changeset) ; `ApprovalPolicy::module_triggers` (`<module>.<mesure>`, seuil), refus `core.approval_policy.trigger` d'une mesure qu'aucun module du build ne déclare ; `Module::default_policies` (`DefaultPolicy`), installées au premier déploiement seulement (table `module_default_policy`, `origin = 'module'`) ; `RegistryError::InvalidMeasure` | 7 | 016-9 | N5 : déclencheurs d'approbation fournis par un module (REQ-016-7, 8) |
| `ModuleHandler::ended(tx, changeset)` : une proposition qui porte des opérations du module prend fin sans être appliquée (rejet, retrait, expiration), dans la transaction qui la clôt ; une erreur annule la clôture (`close_pending` et `expire_pending` passent du store à sa transaction) | 6 | 016-10 | N38 : le module libère ce qu'il réservait pour la proposition (REQ-016-9) |
| `ModuleHandler::review(plan)` : permission dont le détenteur valide le plan ; le changeset devient une proposition même si l'auteur peut écrire, et cette permission rejoint les **droits requis** de la proposition, vérifiés dans la transaction de la validation (au lieu d'une colonne `review_permission`) ; `Module::review_operations` : opérations qu'un valideur joint à la validation (corps de `POST …/validation`), toute autre refusée par `core.review.operation` (au lieu d'un champ `OperationType::review_only`) | 6, 7 | 016-11 | N39 : proposition forcée et revue par un tiers (REQ-016-10, 11 ; G12) |

Écarts au design de la 016 retenus à l'implémentation (le code fait foi) : `measure`
reçoit la transaction en plus du contexte ; la simulation n'expose pas la politique qui
retiendrait le changeset ; l'en-tête `Kartenn-On-Behalf-Of` (hors contrat de module) est
audité par `authn.requester.created` (le catalogue impose trois segments) et
`on_behalf_of` vit dans les `details` des événements, pas dans une colonne.

### Amendement 3 — 2026-10-21, droits déclarés par un module (010 S15, 011 F12)

Trois ajouts additifs, à défaut neutre : les droits d'un type (tâche 010-9, décision
S15 de la 010), puis ceux d'une opération et les acteurs du changeset (tâche 011-3,
décision F12 de la 011 et séparation des rôles). Source :
`crates/kartenn-core/src/authz.rs`, `crates/kartenn-service/src/module.rs`, `rights.rs`,
`reads.rs`, `pipeline.rs`, `review.rs`.

| Ajout | Clause | Raison |
|---|---|---|
| `Module::type_permissions` (`TypePermissions { kind, read, write }`) : un type est lu et écrit sous des permissions déclarées **à la place de** `<type>.read` et `<type>.write` (plusieurs types peuvent partager une permission) ; le noyau les résout partout par `kartenn_core::authz::type_read_permission` et `type_write_permission` (lectures, écritures, portée des listes) ; la proposition implicite reste `<write sans .write>.propose` si le nom finit par `.write`. Le registre exige que le type soit du module et que les deux permissions soient déclarées par lui (`RegistryError::UnknownPermission`) | 5 | S15 : la politique se lit sous `policy.catalogue.read` et s'administre par famille (`policy.axis.admin`…) |
| `Module::operation_permissions` : `(opération, permission)`, la permission exigée sur l'objet écrit **à la place de** l'écriture du type (`type_write_permission`) ; une opération absente de la liste exige celle-ci. La permission doit être une de celles que le module déclare (`Module::permissions`, donc dans son espace de noms et au catalogue), sur une opération qu'il accepte, une fois ; sinon `RegistryError::UnknownPermission` et le serveur ne démarre pas. Elle est lue de la déclaration, jamais du plan (`Planned::Module.permission`) | 5, 6 | F12 : une demande de flux s'écrit sous `flow.request`, pas sous `flow.write` |
| `ModuleActors { author, on_behalf_of, validator }` et `ModuleTx::actors()` : qui se tient derrière le changeset, posé par le pipeline seul (`WriteTx::set_module_actors`, hors de la vue d'un module) — à la soumission l'auteur et le demandeur (`Kartenn-On-Behalf-Of`) ; à la validation l'auteur, le demandeur (gardé dans `Pending.on_behalf_of`) et le validateur ; à la fin d'une proposition l'auteur, le demandeur et, pour un rejet, celui qui rejette ; `None` hors changeset (tâche de fond) | 6 | `requester_id`, `reviewed_by`, `granted_by`, `flow.separation` de la 011 |

Sécurité : une permission de type ou d'opération est vérifiée par le même
`authorize_in`, sur les mêmes objets, que `<type>.write` : mêmes ancres, mêmes portées,
même refus (`core.object.forbidden`). Elle déplace le droit exigé, elle ne retire aucun
contrôle ; c'est la déclaration du module, revue avec lui, qui dit qu'un type ou une
opération relève d'une autre permission que l'écriture, et le registre refuse toute
permission que le module ne déclare pas lui-même (pas d'emprunt au noyau ni à un autre
module). La permission d'une opération l'emporte sur celle du type. Une permission sans `.write` n'ouvre pas la voie
de la proposition implicite (`<type>.propose`) : une opération qui doit pouvoir être
proposée passe par `ModuleHandler::review` (amendement 2). Les acteurs ne sont jamais une
source de droits ; ils servent au journal du module et à ses règles (séparation des
rôles). Tests : `module_anchors::an_operation_requires_the_permission_its_module_declares`
(permission déclarée accordée, autre permission du module, hors périmètre ; acteurs lus),
`module_catalogues::a_required_permission_must_be_the_module_own` (permission absente,
du noyau, d'un autre module, opération inconnue, doublon ; type étranger ou permissions de
type non déclarées), `kartenn-policy` `rights.rs` (droits S15 de la politique).

### Amendement 4 — 2026-10-21, constats d'une tâche de fond (010 REQ-010-48)

Un ajout additif, à défaut neutre (tâche 010-14) : une tâche de fond d'un module
(clause 13, tâche 016-7) peut rendre des constats. Source :
`crates/kartenn-service/src/module.rs` (`ModuleHandler::run_job_findings`, `run_job`).

| Ajout | Clause | Raison |
|---|---|---|
| `ModuleHandler::run_job_findings(name, tx) -> Findings` : par défaut `run_job` puis aucun constat. Quand la tâche rend des violations ou des objets réévalués, le noyau les journalise dans un changeset de kartenn (auteur système, motif `background task <module>.<tâche>`, état `applied`, sans opération), dans la transaction de la tâche, comme la réévaluation d'une source de valeurs attendues (REQ-003-24) : les avertissements ouvrent des anomalies, les couples (objet, règle) réévalués ferment celles qu'aucun avertissement ne relève ; les événements émis par la tâche partent avec ce changeset, sous la clé de la tâche. Sans constat, rien ne change (`journal_job`) | 7, 13 | REQ-010-48 : l'écart d'inférence par nœud d'origine se réévalue quand les règles changent, hors de tout changeset d'un utilisateur ; une tâche ne pouvait ni ouvrir ni fermer une anomalie |

Pas de changement du schéma des déclarations ni du contrat REST. Un changeset système
n'écrit aucun objet et ne passe par aucun contrôle de droits : il ne fait que consigner
des constats que seul le code du module produit. Le module borne lui-même le travail
d'une exécution (la politique : 1 000 héritiers par exécution, reprise par un curseur).
Test : `kartenn-policy` `mismatch.rs` (anomalie ouverte sur le nœud d'origine avec le
décompte, puis fermée quand la règle change ; décompte réparti sur plusieurs exécutions).

### Amendement 5 — 2026-10-23, noms des permissions (spec 018, R2, R4)

Une permission de module s'écrit `<module>.<ressource>.<action>`, ou `<module>.<action>`
quand la ressource porte le nom du module (`flow.read`, `vlan.write`) ; aucune ressource
ne porte le nom d'une action (`kartenn_core::authz::catalogue_problems`, vérifié sur le
catalogue du build par `kartenn-server`). Sans `TypePermissions`, un type `m.m` se lit et
s'écrit sous `m.read` et `m.write` ; un type dont le nom diffère de celui du module
(`machines.machine`) déclare ses `TypePermissions` pour en faire autant
(`machines.read`). Les sortes ne changent pas. Les anciens noms
(`kartenn_core::authz::RENAMED_PERMISSIONS`) sont refusés dans un rôle, sans alias ; le
job de migration renomme ceux des rôles personnalisés par un changeset système
(018/Q-018-3).

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

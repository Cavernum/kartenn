---
status: proposed
updated: 2026-10-22
---

# 018 — Retours d'usage de la v0.2 : conception

Statut : **proposée, en attente de validation**. Les sections qui dépendent d'une
question à arbitrer décrivent l'option recommandée et le signalent (« selon Q-018-n »).

## Vue d'ensemble

```mermaid
flowchart LR
  subgraph srv[kartenn-server]
    cat[catalogue des permissions + RENAMED_PERMISSIONS]
    mig[job de migration : rôles renommés, libellés repris]
    lab[labels jsonb des données administrables]
    sug[suggestions : /values, q, /v1/rules]
  end
  subgraph sdk[@kartenn/sdk]
    lbl[labelOf]
    code[k-code]
    edit[édition en place → draft.add]
  end
  subgraph shell[shell 007]
    tables[listes en grille]
    tree[arbre : action de nœud]
    filters[constructeur de filtres]
    draft[brouillon simulé]
  end
  srv -->|REST /v1, clés + labels| sdk
  sdk --> shell
  shell --> draft -->|changeset| srv
  sdk --> plugins[plugins vlan, machines, policy, flow]
```

Cinq changements indépendants, qui partagent les écrans : (A) renommage des permissions,
côté serveur d'abord ; (B) libellés des données en base, codes du logiciel dans les
catalogues ; (C) un élément `<k-code>` et des violations conservées ; (D) cellules et
nœuds éditables qui écrivent dans le brouillon ; (E) constructeur de filtres typés sur la
syntaxe de filtre existante.

## État des lieux (inventaire du 2026-10-22)

### Permissions

- 61 permissions statiques : 32 du noyau (`kartenn-core/src/authz.rs`, l. 43 à 74),
  6 `vlan` (`kartenn-vlan/src/module.rs`, l. 124), 9 `machines`
  (`kartenn-machines/src/module.rs`, l. 168), 9 `policy` (`kartenn-policy/src/module.rs`,
  l. 311), 5 `flow` (`kartenn-flow/src/module.rs`, l. 92) ; plus la famille dynamique
  `prop.<clé>.{read,write,propose}` (`kartenn-core/src/properties.rs`).
- Rôles livrés : `BUILTIN_ROLES` (`authz.rs`, l. 269), resynchronisés au démarrage
  (`kartenn-service/src/rbac.rs`) ; `BuiltinRole::entries_with` ajoute les lectures des
  modules aux rôles qui lisent les entités.
- Stockage : `role_permission.permission text` (migration 0028), bitemporel, avec une
  contrainte qui exige **trois segments** ; `binding` cite le rôle par identifiant ;
  aucune migration SQL ni `approval_policy` ne cite une permission.
- Permissions par défaut des sortes : `type_read_permission` / `type_write_permission`
  (`authz.rs`, l. 133-154) rendent `<sorte>.read`, d'où `vlan.vlan.read` aujourd'hui.
- Occurrences : ~190 usages hors tests dans `crates/*/src`, ~500 dans les tests, ~50
  dans les bancs ; web : `web/shell/src/{admin/admin.ts,navigation.ts}`, manifestes de
  `web/plugins/{vlan,machines,example}`, `web/e2e/` ; contrat : aucun nom concret ;
  docs : `docs/spec/permissions.md` (généré), `interfaces.md`, `invariants.md`, ADR 0014,
  0026, specs 005 à 017.
- Jokers : serveur, `*` et `prop.*.<action>` seulement (`valid_role_entry`, `grants`) ;
  shell, un `*` générique (`web/shell/src/rights/permissions.ts`, `covers`), plus large et
  sans effet sur la sécurité (le serveur juge).

### Clés et libellés

| Valeur | Stockage | Libellé aujourd'hui | Affichage web |
|---|---|---|---|
| Valeur de liste fermée (`property_enum_value.key`) | migration 0011 | aucun | clé brute (`record/PropertiesPanel.vue`, `data/collections.ts:valueText`) |
| Déclaration de propriété | `property_declaration.key` | aucun | clé brute (`views/PropertiesView.vue`, `PropertiesPanel.vue`) |
| Environnement (valeur de `policy.environment`) | clé au noyau, `policy_value.label` (nullable, une langue) | une langue | pas d'écran (017 à venir) |
| Valeur d'un autre axe | `policy_value.label` | une langue | pas d'écran |
| Type de flux, service, motif, règle | `label NOT NULL`, une langue | une langue | pas d'écran |
| Condition, signal, règle d'inférence, rôle exempté | clé seule | aucun | pas d'écran |
| Texte pédagogique | `texts jsonb {langue: {body, url}}` | **par langue** (précédent) | pas d'écran |
| Rôle | `role.key`, `role.label` (une langue ; rôles livrés en français) | une langue | clé et libellé (`admin/RolesView.vue`) ; clé brute dans `admin/BindingsView.vue` |
| État et sorte de changeset, état d'import | code | catalogues FR/EN du shell | traduit |
| Sorte d'objet du noyau | code | `object.types.*` | traduit ; sorte de module → clé brute (`views/ObjectView.vue`) |
| Nature, usage (préfixe, plage, adresse) | code | aucun | clé brute (`collections.ts:cell`, `edit/fields.ts`) |
| Sorte de machine, mode VLAN, état de VLAN | code | catalogues des plugins | traduit |
| État, issue, sorte d'un flux | code | aucun | pas d'écran |
| Règle (`Rule.id`, `summary` en français, non servi) | code | aucun servi | identifiant brut (anomalies, violations) |
| Problem Details (`urn:kartenn:problem:<slug>`) | code | `problem.*` (24 sur 26) | message seul, code jamais montré |
| Permission | code, description française | aucun | nom brut (`admin/RolesView.vue`) |

Aucune lecture d'`Accept-Language` côté serveur ; W11 l'a écartée. Aucun cas de libellé
servant de clé : les clés sont stables (REQ-003-5) et la politique cite ses objets par
identifiant.

### Écrans

- Listes : `views/ListView.vue` + `data/DataTable.vue` (virtualisé), colonnes et filtres
  de `data/collections.ts` (`COLUMNS`, `FILTERS` : texte libre sur des champs du noyau,
  aucun filtre de propriété) ; anomalies : `views/AnomaliesView.vue` (texte libre).
- Arbre : `tree/PrefixTree.vue`, `tree/model.ts`.
- Édition : `edit/QuickEdit.vue` (fiche, `PATCH` immédiat), `edit/fields.ts` ; brouillon :
  `draft/{index.ts,store.ts,DraftDrawer.vue}` ; aucune édition de propriété dans le web.
- Problèmes : `i18n/index.ts:problemText` (message traduit ou `detail`) ;
  `data/api.ts:problemOf` perd `violations`.

## Changements

### A. Permissions (REQ-018-1 à 6)

#### Correspondance des permissions

Onze permissions changent de nom ; toutes les autres sont inchangées. Q-018-1 décide du
nom de la dérogation.

| Ancien nom | Nouveau nom |
|---|---|
| `vlan.vlan.read` | `vlan.read` |
| `vlan.vlan.write` | `vlan.write` |
| `vlan.vlan.propose` | `vlan.propose` |
| `machines.machine.read` | `machines.read` |
| `machines.machine.write` | `machines.write` |
| `machines.machine.propose` | `machines.propose` |
| `flow.flow.read` | `flow.read` |
| `flow.flow.request` | `flow.request` |
| `flow.flow.review` | `flow.review` |
| `flow.flow.write` | `flow.write` |
| `flow.flow.waive` | `flow.waiver.grant` (Q-018-1, recommandé) ou `flow.waive` |

Inchangées (50 + la famille `prop`) :

- `core` : `core.{entity,site,domain,prefix,range,address}.{read,write,propose}`,
  `core.changeset.approve`, `core.changeset.read`, `core.anomaly.acknowledge`,
  `core.property.admin`, `core.approval_policy.admin`, `core.rbac.read`,
  `core.rbac.admin`, `core.role.admin`, `core.issuer.admin`, `core.account.admin`,
  `core.audit.read`, `core.identity.pseudonymise`, `core.identity.act_on_behalf`,
  `core.observability.admin` (aucune ressource `core`).
- `vlan` : `vlan.group.{read,write,propose}`.
- `machines` : `machines.cluster.{read,write,propose}`,
  `machines.interface.{read,write,propose}`.
- `policy` : `policy.catalogue.read`, `policy.verdict.read`, `policy.flow.request`,
  `policy.{axis,environment,flow_type,service,rule}.admin`, `policy.inference.admin`
  (aucune ressource `policy` ; `policy.flow.request` garde sa ressource `flow`).
- `prop.<clé>.{read,write,propose}`, `prop.*.<action>`, `*` : inchangés (la clé d'une
  propriété n'est pas une ressource de module).

#### Code

- `authz.rs` : `RENAMED_PERMISSIONS: &[(&str, &str)]` ; catalogues de module renommés ;
  `type_read_permission` et `type_write_permission` appliquent R2 (`m.m` → `m.<action>`)
  quand la sorte n'a pas de `TypePermissions` ; `valid_role_entry` inchangé dans sa
  logique (le catalogue fait foi) ; test : aucune ressource nommée comme une action,
  aucun nom `m.m.a`, table de correspondance sans cible inconnue.
- Migration SQL `00NN_permission_names.sql` : contrainte de
  `role_permission.permission` relâchée (R5) ; la même sur la table d'historique si elle
  la porte.
- Rôles personnalisés : selon Q-018-3 (recommandé : étape `rename_permissions` du job de
  migration, après `sync_modules`, un changeset `système` par instance, motif
  `permission rename 018`, rien si aucun ancien nom).
- Compatibilité : selon Q-018-2 (recommandé : `normalise_role_entry` à l'entrée de
  `rbac::check`, violation `core.permission.renamed` de sévérité avertissement ; le
  chargeur de plugins du shell applique la même table aux champs `permission` des
  manifestes ; retrait inscrit pour la v0.3).
- `docs/spec/permissions.md` : la génération (`catalogue_markdown`) ajoute la section
  « Anciens noms » depuis `RENAMED_PERMISSIONS`.
- Shell : `admin/RolesView.vue`, `admin/AuditView.vue` affichent un ancien nom comme
  « nouveau (anciennement ancien) » (R6) ; la table est servie au shell par le
  catalogue généré dans le SDK (`web/sdk/src/permissions.ts`, généré, pour ne pas
  dupliquer la table à la main).

### B. Clés et libellés (REQ-018-7 à 14)

#### Modèle (selon Q-018-4 à 7, options recommandées)

- **Données administrables** : colonne `labels jsonb NOT NULL DEFAULT '{}'` sur
  `property_declaration`, `property_enum_value`, `role`, `policy_flow_type`,
  `policy_service`, `policy_pattern`, `policy_rule`, `policy_condition`,
  `policy_signal` ; versionnée avec la ligne (Q-018-5 A) ; `CHECK
  (jsonb_typeof(labels) = 'object')` ; validation par la règle `core.label.invalid`
  (REQ-018-10) dans le pipeline, partagée par les modules (remplace `policy.label.invalid`,
  gardée comme alias de règle le temps de la v0.2).
- Migration : `labels = jsonb_build_object(<langue par défaut>, label)` pour chaque
  colonne `label` existante ; `policy_value.label` → `labels` de la valeur de
  `property_enum_value` de même clé (Q-018-7 A), puis colonne supprimée par une migration
  ultérieure. Les colonnes `label` des tables de la politique et de `role` restent
  calculées pour la lecture (langue par défaut) le temps de la v1 du contrat.
- **Codes du logiciel** : catalogues FR/EN du shell (`web/shell/src/i18n/`) pour le
  noyau (natures, usages, sortes du noyau, états, problèmes) ; catalogues des plugins pour
  les codes d'un module, sous des clés conventionnelles (R8) : `kinds.<sorte>`,
  `rules.<id>`, `permissions.<nom>`, `states.<état>`. Le SDK expose
  `moduleLabel(module, key)` qui cherche dans le catalogue du plugin du module, puis rend
  le code.
- **Langue par défaut** : `[i18n] default_locale` (`kartenn-server` configuration,
  `fr`), lue par le noyau et passée aux modules ; elle remplace
  `kartenn-policy/src/codec.rs:DEFAULT_LOCALE`. Le type `Locale` de la politique
  (`model.rs`) migre au noyau.

#### API (selon Q-018-4 A)

- Chaque représentation d'une donnée administrable gagne `labels: {<langue>: <texte>}` ;
  `key` toujours présent ; `label` (s'il existait) = libellé dans la langue par défaut
  (additif, 006/H12).
- `GET /v1/property-declarations` et `/{key}` : `labels` ;
  `GET /v1/property-declarations/{key}/values` : `{key, labels, valid_from, valid_to}`,
  paramètres `valid_at`, `q` (Q-018-8 A) ; `GET /v1/capabilities` : `properties[]` gagne
  `labels`, et `default_locale`.
- Écritures : `declare_property`, `update_property`, `add_enum_value` acceptent `labels` ;
  nouvelle opération `update_enum_value {property, key, labels}` (libellés seuls, la clé
  ne change pas) ; `PUT /v1/roles/{key}` accepte `labels` (`label` gardé en entrée comme
  libellé de la langue par défaut) ; opérations de la politique : `labels` à côté de
  `label`.
- Synchronisation des modules (R10) : les descripteurs (`PropertySpec`, valeurs) portent
  `labels: &[(&str, &str)]` ; `sync_modules` compare et écrit dans son changeset ; une
  version de libellé dont le changeset n'est pas de l'auteur `système` n'est pas
  écrasée.

#### Interface

- SDK : `labelOf(labels, key, locale)` (R9 : langue, troncatures, langue par défaut,
  clé) rend `{text, lang, fallback}` ; un élément `<k-label>` rend le texte avec `lang` et,
  en repli sur la clé, la clé en monospace.
- Affichage : libellé dans les tableaux et l'arbre ; la clé reste lisible (infobulle
  accessible et colonne masquable « clé ») ; dans les listes de choix : « Libellé
  (`clé`) ».
- Formulaires de libellés : une ligne par langue livrée (FR, EN) plus « ajouter une
  langue » ; dans `views/PropertiesView.vue` (déclarations et valeurs) et
  `admin/RolesView.vue` ; ceux de la politique dans la 017 (catalogue).

### C. Codes visibles (REQ-018-15 à 17)

- SDK : `<k-code code="…">` (R12) : `<code>` monospace, bouton de copie
  (`navigator.clipboard.writeText`, nom accessible « Copier le code … »), région
  `role="status"` partagée qui annonce « Code copié » ; repli sans presse-papiers :
  sélection du texte.
- `ApiProblem` (SDK) et `problemOf` (shell) gardent `type`, `title`, `detail`,
  `instance`, `violations[]` ; `problemText` rend `{message, code}` (code = slug du
  `type`).
- Emplacements : avis d'erreur du shell, dialogue 412, violations des formulaires
  (`edit/QuickEdit.vue`), brouillon (`draft/DraftDrawer.vue`), changeset
  (`changesets/ChangesetView.vue`), anomalies (`record/AnomaliesPanel.vue`,
  `views/AnomaliesView.vue`), imports (`admin/ImportView.vue`), audit
  (`admin/AuditView.vue` : nom de l'événement), plugins (`vlan`, `machines`, et `policy`,
  `flow` par la 017 : règle décisive, clé d'objet, codes des non-conformités).
- Ordre de lecture : message puis code, sur la même ligne si la place le permet ; jamais
  le code seul, jamais le message seul.

### D. Édition en place (REQ-018-18 à 23)

#### Écrans concernés

| Écran | Valeurs éditables | Opération ajoutée au brouillon | Droit lu dans `/v1/me` |
|---|---|---|---|
| Listes `prefixes`, `ranges`, `addresses` | nature, usage, propriétés affichées en colonnes | `update_prefix`, `update_range`, `update_address`, `assign_property`, `clear_property`, `unassign_property` | `core.<type>.write` ; `prop.<clé>.write` |
| Listes `entities`, `sites`, `domains` | propriétés en colonnes | `assign_property`, `clear_property`, `unassign_property` | `prop.<clé>.write` |
| Arbre des préfixes | propriété affichée sur le nœud (environnement…) | idem | `prop.<clé>.write` |
| Panneau des propriétés (fiche) | toute propriété non masquée | idem | `prop.<clé>.write` |
| Plugin `machines` | sorte, cluster, VLAN d'une interface | opérations du module | `machines.write`, `machines.interface.write` |
| Plugin `vlan` | état, groupe | opérations du module | `vlan.write` |
| Plugins `policy`, `flow` (017) | matrice (déjà), catalogue | ensemble en préparation (017/Q-017-1) | `policy.*.admin` |

- Colonnes de propriétés : un sélecteur de colonnes ajoute une propriété déclarée à une
  liste ; la valeur résolue vient de la lecture groupée existante ou d'une lecture par
  ligne visible (à mesurer, REQ-018-32 ; sinon amendement de la 006 pour servir les
  propriétés dans la page).
- Droits : `Rights.has` (REQ-007-20), sans portée ; un refus du serveur (violation
  `core.object.forbidden` à la simulation) marque la cellule en erreur avec son code.
- Temps : `as_of` passé → lecture seule ; `valid_at` hors présent → l'opération va au
  brouillon avec `valid_from` = `valid_at` (007/W9).
- Valeur modifiée : icône « modifié » nommée et texte « dans le brouillon », valeur
  nouvelle affichée, ancienne dans l'infobulle ; retirer l'opération du brouillon rend la
  valeur lue.
- Héritage (D26) : l'éditeur affiche « hérité de <origine> » et propose « remplacer pour
  ce nœud », « revenir à l'héritage », « effacer ».

#### Clavier et accessibilité

- Tableaux : motif `grid` de l'APG WAI-ARIA
  ([w3.org/WAI/ARIA/apg/patterns/grid/](https://www.w3.org/WAI/ARIA/apg/patterns/grid/)) :
  flèches entre cellules, Début/Fin, Entrée ou F2 → édition, Échap → annule, Entrée →
  valide et reste, Tab → valide et passe à la cellule éditable suivante ; un seul arrêt
  de tabulation pour la grille (tabindex itinérant).
- Listes : motif `combobox` avec `listbox`
  ([w3.org/WAI/ARIA/apg/patterns/combobox/](https://www.w3.org/WAI/ARIA/apg/patterns/combobox/)),
  primitives Reka UI (W12) ; ensemble (D25) : liste à choix multiples, valeurs choisies en
  puces retirables au clavier.
- Arbre : motif `tree` conservé ; action « Changer <propriété> » par menu contextuel
  (touche Menu ou Maj+F10) et par la barre d'actions du nœud focalisé (R14).
- Virtualisation (TanStack Virtual) : la ligne focalisée et celle en édition restent
  rendues.
- Critères WCAG 2.2 visés en particulier : 1.4.1, 2.1.1, 2.1.2, 2.4.7, 2.4.11, 2.5.8,
  3.2.2, 3.3.1, 4.1.2, 4.1.3 ([WCAG 2.2](https://www.w3.org/TR/WCAG22/)).

#### SDK

- `<k-editable-value>` (ou `editValue(spec)`) : reçoit le type de valeur, la valeur, la
  permission, la fabrique d'opération ; lit les droits et le temps du contexte ; appelle
  `draft.add`. Les plugins `vlan` et `machines` l'utilisent ; la 017 aussi pour le
  catalogue.

### E. Filtres typés (REQ-018-24 à 29)

- Constructeur `<FilterBuilder>` (shell) et `<k-filter-builder>` (SDK) : ligne
  « champ — opérateur — valeur » ; champs = champs du noyau de la collection
  (`collections.ts:FILTERS`, typés) + propriétés de `capabilities.properties` portées par
  le type de la collection ; opérateurs selon le type (tableau ci-dessous) ; valeur par
  un contrôle typé.

| Type | Opérateurs | Contrôle de valeur | Source des suggestions |
|---|---|---|---|
| liste fermée (`enum`, ensemble) | =, ∈, présent, absent | combobox (multiple pour ∈) | `/values?valid_at&q` |
| référence | =, ∈, présent, absent | combobox de recherche | collection cible `?q=` |
| date | =, entre, après, avant, présent, absent | sélecteur de date (ISO 8601) | — |
| entier | =, entre, au-dessus, au-dessous, présent, absent | nombre borné par `params` | — |
| booléen | =, présent, absent | oui / non | — |
| texte, FQDN | =, entre, présent, absent | saisie validée | — |
| règle (anomalies) | = | combobox | `GET /v1/rules` |

- Sérialisation : la syntaxe existante (`prop.<clé>`, `:above`, `:below`, `:between`,
  `:exists`) ; « absent » = `:exists=false` (déjà servi, `filter.rs`) ; une URL
  illisible est montrée comme filtre en erreur avec son code.
- Suggestions (selon Q-018-8 A) : `/values` étendu ; `q` sur `machines`, `interfaces`,
  `clusters`, `vlans`, `entities`, `sites`, `domains` (préfixe insensible à la casse,
  index `text_pattern_ops` ou `lower()` existants à vérifier) ; `GET /v1/rules` (le
  catalogue `rules::catalogue()`, sans `summary`). Débit : appel après 200 ms sans
  frappe, le précédent annulé ; 20 résultats.

#### Évolution différée : langage de requête (REQ-018-30)

Hors de la 018. Points à reprendre par la spec qui le concevra : grammaire publiée (EBNF),
traduction vers les filtres du constructeur, autocomplétion contextuelle par le même
service de suggestions (Q-018-8 B à reconsidérer), coloration par jetons doublée d'un
autre indice que la couleur, erreurs avec position et code, éditeur accessible (champ
texte natif plutôt qu'un éditeur de code opaque aux lecteurs d'écran).

## Impact sur la 017 et ses maquettes

La 017 est validée et à 0/24 ; elle n'est pas modifiée ici. Amendements à y porter après
validation de la 018 :

- **Noms** : vocabulaire (requirements, l. 25 à 45), tableau « Rôles et permissions » du
  design, manifestes prévus (tâches 017-2, 017-3), tests et parcours E2E :
  `flow.flow.*` → `flow.*`, `flow.flow.waive` → selon Q-018-1. Les tâches 017-2 et
  017-3 suivent les tâches 2 à 6 de la 018 (R18).
- **Libellés** : en-têtes de la matrice, listes de règles, services, types de flux,
  environnements, formulaire de demande : libellé affiché, clé lisible (R7) ; formulaires
  du catalogue (REQ-017-10, 30) : libellés par langue (FR, EN) au lieu d'un `label`
  unique ; l'écriture de `value.describe` porte les libellés du noyau (Q-018-7).
- **Codes** : règle décisive, clé d'objet, codes des violations et des non-conformités
  par `<k-code>` ; le design de la 017 (« Temps, droits, langue ») ajoute le code à côté
  de la traduction `rules.<id>`.
- **Édition** : le catalogue (REQ-017-10, 30) peut utiliser `<k-editable-value>` ; la
  matrice garde son ensemble en préparation (Q-017-1), sans changement.
- **Filtres** : liste des flux (REQ-017-17) et non-conformités (REQ-017-24) par le
  constructeur typé (service, environnement, entité, état).
- **Maquettes (tâche 017-1)** : intègrent `<k-code>`, `<k-label>`, cellules éditables et
  constructeur de filtres ; les maquettes de la 018 (tâche 1) et de la 017 se font
  ensemble et partagent ces éléments.

## Amendements d'autres specs (à rédiger après validation, non faits ici)

| Spec | Amendement |
|---|---|
| 005 | K12 : forme des noms (R2) ; REQ-005-16 : grammaire des entrées ; rôles livrés et libellés FR/EN |
| 003 | REQ-003-1 à 5 : `labels` des déclarations et des valeurs ; opération `update_enum_value` ; P12 : libellés synchronisés (R10) ; REQ-003-25 : `/values` avec `q`, `valid_at` |
| 006 | Contrat : `labels`, `default_locale`, `q`, `GET /v1/rules`, `update_enum_value` ; description des entrées de rôle ; comparateur de contrat |
| 007 | W11 : libellés de données servis en table par l'API (l'API reste neutre en langue) ; REQ-007-25, 26 : édition en place par le brouillon ; affichage des problèmes avec code ; filtres typés |
| 008, 009 | Noms `vlan.*`, `machines.*` ; manifestes ; libellés des codes dans les catalogues des plugins |
| 010 | S2 : libellés des environnements au noyau (Q-018-7) ; S5 : `core.label.invalid` ; S29 : langue par défaut en configuration ; tableau des tables (`labels`) |
| 011 | F12 : noms `flow.*` et dérogation (Q-018-1) |
| 012 | Note : les événements d'audit antérieurs gardent les anciens noms (R6) |
| 017 | Voir « Impact sur la 017 » |
| `docs/spec/` | `permissions.md` (généré), `interfaces.md`, `invariants.md`, `data.md`, `regles-noyau.md` (`core.label.invalid`, `core.permission.renamed`) |
| ADR | 0010 (RBAC) : amendement sur le nommage ; nouvelle ADR 0033 « Libellés de données par langue » (Q-018-4 à 7) |

## Erreurs et cas limites

- Rôle personnalisé citant un ancien **et** le nouveau nom : la migration ne garde qu'une
  entrée.
- Ancien nom dans un joker inexistant (`flow.flow.*`) : refusé aujourd'hui par
  `valid_role_entry`, donc absent des bases.
- Valeur de liste fermée close à `valid_at` mais affectée à un objet affiché : libellé
  rendu, valeur non proposée à l'édition.
- Libellé dans une langue que l'interface ne livre pas (`de`) : conservé, servi, affiché
  seulement à un utilisateur en `de` par repli sur `fr`/`en` sinon.
- Deux éditions de la même cellule dans le brouillon : la seconde remplace la première
  (une opération par objet et propriété).
- Édition en place hors des droits d'une portée (droit détenu ailleurs) : la simulation
  rend `core.object.forbidden`, la cellule l'affiche avec son code.
- Presse-papiers refusé par le navigateur : le code est sélectionné, un message invite à
  copier au clavier.

## Alternatives considérées

- Renommer aussi les sortes d'objets (`vlan.vlan` → `vlan`) : rejeté (R4).
- Libellés traduits par le serveur (`Accept-Language`) : Q-018-4 B, non recommandé.
- Édition en place par `PATCH` immédiat (modification rapide) : rejetée par l'utilisateur
  (R13 : par le brouillon).
- `treegrid` éditable pour l'arbre : rejeté (R14).
- Filtres en texte libre avec validation a posteriori : rejetés (R15).

## Risques

- Volume du renommage : ~750 occurrences de code et de tests ; un renommage mécanique
  par crate, vérifié par `k-check` et un test d'absence des anciens noms.
- Sortes de module sans `TypePermissions` : le changement de `type_*_permission` change
  silencieusement leurs droits ; tests de droits par sorte dans chaque module.
- Colonnes de propriétés dans les listes : une lecture par ligne peut dépasser le budget
  de page ; mesure avant d'amender la 006.
- Libellés bitemporels (Q-018-5 A) : un correctif de libellé passe par changeset et
  éventuellement par approbation ; accepté, mais à observer.
- Alias temporaires (Q-018-2 B) : ne pas oublier leur retrait ; date inscrite et test qui
  échoue à la v0.3.

## Stratégie de test

- Rust : catalogue (noms, ressources ≠ actions, correspondance), `type_*_permission`,
  migration des rôles (idempotence, liaisons inchangées, lecture datée), alias,
  `core.label.invalid`, `update_enum_value`, synchronisation des libellés (R10),
  `/values` avec `q` et `valid_at`, `q` des collections sous RBAC, `GET /v1/rules`.
- Bancs : `bench-core` (suggestions, listes avec `labels`).
- Vitest : `labelOf` (repli), `<k-code>` (copie, annonce), `problemOf` (violations),
  grille (clavier), cellule → opération de brouillon, héritage, constructeur ↔ URL
  (aller-retour), parité des catalogues des codes.
- Playwright : REQ-018-31, axe-core à chaque étape.

## ADR

- Amendement de l'ADR 0010 (RBAC) : forme des noms de permissions.
- ADR 0033 « Libellés de données par langue » (proposée avec la tâche 8) : décisions
  Q-018-4 à 7 une fois arbitrées.

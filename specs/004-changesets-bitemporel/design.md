---
status: approved
updated: 2026-10-14
---

# 004 — Changesets et bitemporel : conception

## Vue d'ensemble

La 004 fournit le mécanisme d'écriture et de temps dont dépendent la 002 et la 003 :

1. **Stockage temporel : table courante + table d'historique** par type d'objet. La table
   courante contient toutes les versions applicatives actuellement affirmées ; la table
   d'historique reçoit, par **déclencheur**, chaque version dont l'affirmation se termine.
   Le temps système est posé par la base (`now()`), jamais par une instance. (ADR 0017,
   proposé.)
2. **Pipeline de changeset** dans `kartenn-service` : décodage, verrous de domaine (ADR
   0015), application en mémoire de transaction avec découpage des périodes (« FOR
   PORTION OF » émulé), validation 002, résolution et différentiel 003, droits,
   politiques d'approbation et propositions, écriture, audit, outbox.
3. **Outbox ordonnée par domaine**, au format CloudEvents 1.0, relayée par un worker qui
   ne lit que les transactions terminées (ADR 0018, proposé).

Ce que la 004 ne fait pas : le format HTTP (006), le modèle RBAC et l'évaluation des
permissions (005).

## Changements

### Stockage temporel (REQ-004-13 à 18) — ADR 0017

Pour chaque type d'objet versionné (objets du noyau, affectations et déclarations de
propriétés, valeurs résolues, objets des modules) :

| Table | Contenu | Contraintes |
|---|---|---|
| `<objet>` (courante) | colonnes métier, `id`, `valid_period tstzrange`, `sys_start timestamptz`, `changeset_id` | `PRIMARY KEY (id, valid_period WITHOUT OVERLAPS)` ; contraintes métier (002, 003) sans clause partielle ; `CHECK (NOT isempty(valid_period))` |
| `<objet>_history` | mêmes colonnes + `sys_end timestamptz` (donc `system_period = [sys_start, sys_end)`) | aucune contrainte métier (validée quand la version était courante, SQL:2011) ; index `(id, sys_start)`, GiST `tstzrange(sys_start, sys_end)` |

**Déclencheur générique** (`BEFORE UPDATE OR DELETE` sur la table courante, fonction
PL/pgSQL commune) : copie l'ancienne ligne dans `<objet>_history` avec `sys_end = now()` ;
pour un `UPDATE`, pose `sys_start = now()` sur la nouvelle ligne. Un déclencheur
`BEFORE INSERT` pose `sys_start = now()`. Ainsi :
- toutes les versions d'un changeset ont le même temps système, celui de sa transaction
  (`now()` est fixe dans une transaction) — REQ-004-16, INV-12 ;
- aucune écriture ne peut contourner l'historique, même une correction manuelle en SQL.

**Immutabilité** (REQ-004-16, INV-11) : le rôle applicatif a `INSERT` sur les tables
d'historique (via la fonction du déclencheur, `SECURITY DEFINER`) mais ni `UPDATE` ni
`DELETE` ; seul un rôle d'administration hors application peut les modifier (et ne le
fait pas : contrôlé en test).

**Chaîne contiguë** : pour un même `(id, portion de valid_period)`, les versions
historiques successives se touchent (`sys_end` de l'une = `sys_start` de la suivante),
par construction du déclencheur.

**Lecture datée** (REQ-004-17) : fonctions du port de stockage, une par type :

| `as_of` | Requête |
|---|---|
| maintenant (défaut) | table courante, `valid_period @> valid_at` |
| passé | `UNION ALL` de la table courante filtrée `sys_start <= as_of` et de l'historique filtré `tstzrange(sys_start, sys_end) @> as_of`, puis `valid_period @> valid_at` |

La table courante reste petite (une ligne par segment applicatif affirmé) ; l'historique
est **partitionné par plage de `sys_end`** (mensuel) pour borner les parcours et faciliter
l'exploitation. Budget visé : `as_of` passé ≤ 2× la lecture courante (REQ-004-38), à
mesurer.

**Historique d'un objet** (REQ-004-18) : union des versions courantes et historiques de
l'`id`, triées par `valid_period` puis `sys_start`, avec `changeset_id`.

### Découpage des périodes (REQ-004-14, 15)

PostgreSQL 18 n'a pas `FOR PORTION OF` : la couche service l'émule. Pour une opération
de modification sur la période P d'un objet :

1. lire les versions courantes de l'objet qui recoupent P ;
2. pour chacune, calculer `version.valid_period − P` (0, 1 ou 2 morceaux) en Rust ;
3. `UPDATE` de la version (le déclencheur l'historise) pour la ramener à la portion
   intérieure avec le nouvel état ; `INSERT` des morceaux extérieurs avec l'ancien état.

La clôture à une date d (REQ-004-15) est une modification de la période :
`valid_period` devient `[début, d)` ; les versions entièrement postérieures à d sont
supprimées de la table courante (donc historisées).

Les morceaux adjacents de même état sont fusionnés (pas de fragmentation inutile).

### Pipeline d'un changeset (REQ-004-1 à 12, 19 à 27, 39 à 43)

```mermaid
flowchart TD
  S[Soumission] --> V0{Taille ≤ max ? clé d'idempotence déjà vue ?}
  V0 -->|déjà vue| R0[Résultat mémorisé]
  V0 -->|trop gros| X0[Refus]
  V0 --> T[BEGIN ; verrous de domaine dans l'ordre ; verrou public si besoin]
  T --> A[Application en mémoire de transaction : découpage, références internes, cascade explicite]
  A --> V1[Validation 002 sur l'état final, à toute la période visée]
  V1 --> P[Résolution 003 : différentiel, obligations, valeurs attendues]
  P --> D[Droits : write sur objets et effets ; à défaut propose ?]
  D --> AP{Proposition ? politique d'approbation ?}
  AP -->|erreurs| RB1[ROLLBACK ; changeset refusé journalisé]
  AP -->|simulation| RB2[ROLLBACK ; résultat renvoyé]
  AP -->|attente| RB3[ROLLBACK des données ; changeset en attente journalisé]
  AP -->|appliquer| W[COMMIT : versions, audit, outbox]
```

- **Application en mémoire de transaction** : les opérations sont réellement exécutées
  (`INSERT`/`UPDATE`) dans la transaction, ce qui donne aux règles globales l'état final
  par de simples requêtes ; en cas de refus, d'attente ou de simulation, `ROLLBACK`, puis
  journalisation du changeset dans une **transaction séparée**.
- **Verrous** (REQ-004-12, ADR 0015) : `pg_advisory_xact_lock` par domaine touché (clé
  dérivée de l'UUID), dans l'ordre des UUID ; verrou « espace public » si un objet public
  est écrit. Les changesets en attente ne tiennent aucun verrou.
- **Références internes** (REQ-004-4) : une opération peut désigner un objet par une
  référence locale (`$1`, `$2`…) résolue en UUID v7 généré à l'application.
- **Concurrence optimiste** (REQ-004-11) : une opération peut porter
  `expected_changeset_id` (dernier changeset ayant modifié l'objet, exposé comme ETag par
  la 006) ; différence → conflit.
- **Idempotence** (REQ-004-10) : table `idempotency_key (identity_id, key, request_hash,
  changeset_id, expires_at)`, 24 h ; même clé + autre hash → refus.
- **Rétroactivité** (REQ-004-27) : `retroactive = (lower(période visée) < now())` pour au
  moins une opération ; stocké et indexé.

### Cascade explicite (REQ-004-25, 26)

Chaque type d'objet déclare, via le contrat de module, une fonction `dependents(objet,
période)` : pour un réseau, ses plages et adresses ; pour un objet référencé par une
propriété de type référence, ces affectations (003/REQ-003-10) ; pour une entité ou un
site, ses enfants et rattachements. Une clôture avec `cascade = true` ajoute au
changeset la clôture de ces dépendants, récursivement, avant validation ; toutes passent
par les mêmes contrôles de droits. Sans cascade, la validation 002/003 produit l'erreur
qui liste les dépendants.

### Approbation et propositions (REQ-004-19 à 24, 39 à 43)

**Politiques** (C6) : table `approval_policy` (`id`, `name`, déclencheurs :
`min_operations`, `min_diff_objects`, `property_keys[]`, `perimeters[]`,
`authors[]` (rôles, groupes, comptes), `enabled`). Évaluées après le différentiel ; aucune
par défaut. Gérées par changeset (permission d'administration, 005).

**Proposition** (C7) : si la vérification des droits échoue pour des opérations ou des
effets, la couche service vérifie, pour chacun, la permission `…propose` correspondante ;
si toutes sont couvertes, le changeset devient une proposition. La liste des droits
d'écriture requis (permission × ancre de périmètre) est stockée avec la proposition.

**Changeset en attente** : opérations, période visée, motif, auteur, nature
(`approval` ou `proposal`), droits requis, politique déclenchée, expiration (30 jours
configurable), résultat de la validation à la soumission.

**Validation** : l'approbateur ou le mainteneur déclenche l'application ; le pipeline est
rejoué intégralement dans une nouvelle transaction (revalidation, REQ-004-22), avec le
contrôle du valideur à la place de celui de l'auteur :
- politique : permission d'approbation sur tout le périmètre, valideur ≠ auteur ;
- proposition : droits d'écriture requis, valideur ≠ auteur (REQ-004-42).
L'auteur reste l'auteur du changeset appliqué ; le valideur est enregistré.

**File des propositions** (REQ-004-43) : requête sur les changesets en attente de nature
`proposal`, filtrée par les ancres de périmètre de leurs droits requis où le lecteur
détient ces droits (prédicat RBAC, 005). Volume attendu faible ; pagination standard.

**Expiration** : job de worker périodique.

### Imports (REQ-004-28 à 30)

- `import` (`id`, auteur, source, état, progression, rapport) et `import_batch` (`index`,
  opérations, `changeset_id`, état).
- À la création : opérations validées syntaxiquement, **triées par dépendance** (entités,
  sites, domaines, conteneurs du plus court au plus long préfixe, réseaux, plages,
  adresses, affectations), découpées en lots ≤ taille maximale.
- Un worker applique les lots dans l'ordre, chacun comme un changeset ordinaire ; échec
  d'un lot → import en pause avec le rapport ; reprise au premier lot non appliqué.
- **Rejeu** : les opérations d'import sont des « upsert par identifiant externe »
  (002/REQ-002-41) : un objet déjà importé est mis à jour ou laissé tel quel, jamais
  dupliqué.

### Annulation (REQ-004-34, 35)

Pour un changeset C appliqué au temps système t :
1. objets touchés = versions dont `changeset_id = C` (courantes et historiques) ;
2. état antérieur = lecture `as_of` juste avant t sur la même période applicative ;
3. génération d'opérations qui rétablissent cet état (créations closes, modifications
   inverses, clôtures annulées par recréation sur la période) ;
4. **conflit** : si une version postérieure à t, d'un autre changeset, recoupe le même
   objet et la même période, l'opération correspondante est marquée en conflit et exclue,
   avec l'explication ; l'utilisateur décide.
Le changeset généré n'est pas soumis automatiquement.

### Audit et identités (REQ-004-31 à 33)

| Table | Contenu |
|---|---|
| `changeset` | `id`, `author_id`, `validator_id`, `motive`, `state`, `kind`, `default_period`, `retroactive`, `submitted_at`, `applied_at`, `expires_at`, `policy_id`, `violations`, `anomalies`, `diff_summary` |
| `changeset_operation` | `changeset_id`, `ordinal`, `payload` (JSON validé à la soumission) |
| `changeset_diff` | `changeset_id`, `property_id`, `object_id`, `before`, `after` (détail paginé, 003) |
| `person` | `id` (interne, stable), `display_name`, `email`, `issuer`, `subject`, `departed_at`, `pseudonymized_at` |

- L'audit ne référence que `person.id`. La **pseudonymisation** remplace les champs
  d'identité par `personne #<id>` et vide `email`, `issuer`, `subject` ; job déclenché à
  `departed_at + délai` (1 an proposé, configurable).
- Index de recherche : `(author_id)`, `(state)`, `(submitted_at)`, `(retroactive)` ; par
  objet via `changeset_diff` et les `changeset_id` des tables versionnées.

### Événements sortants (REQ-004-36, 37) — ADR 0018

- Table `outbox` (`seq bigint identity`, `xid xid8 DEFAULT pg_current_xact_id()`,
  `changeset_id`, `domain_ids uuid[]`, `payload jsonb`), insertion dans la transaction du
  changeset.
- Format **CloudEvents 1.0** : `specversion = "1.0"`, `id` = UUID du changeset (unique
  pour la source), `source` = URI de l'instance kartenn, `type` =
  `org.kartenn.changeset.applied` (préfixe à confirmer), `time` = temps système (RFC 3339),
  `data` = objets touchés et différentiel résumé. Les consommateurs dédoublonnent par
  `source` + `id`.
- **Relais** (un à la fois par consommateur, verrou consultatif) : ne lit que les lignes
  dont `xid < pg_snapshot_xmin(pg_current_snapshot())`, c'est-à-dire de transactions
  terminées, non encore livrées à ce consommateur, par `seq` croissant, et **enregistre
  chaque livraison** (`outbox_delivery`). Un curseur unique « dernier `seq` livré » est
  proscrit : il peut sauter l'événement d'une transaction encore en cours (amendement
  de l'ADR 0018).
- **Ordre garanti par domaine d'adressage** (réponse à la question ouverte) : les
  changesets d'un même domaine sont sérialisés par le verrou de domaine, donc leurs `seq`
  suivent l'ordre d'application. Entre domaines différents, l'ordre global n'est pas
  garanti (et n'est pas nécessaire).
- Livraison au moins une fois : une livraison n'est enregistrée qu'après accusé du
  consommateur.

## Erreurs et cas limites

| Cas | Comportement | REQ |
|---|---|---|
| Changeset vide | Refus `changeset.empty` | 1 |
| Motif vide | Refus `changeset.motive_required` | 1 |
| Période vide ou inversée | Refus `time.invalid_period` | 13 |
| Opération qui modifie un objet sur une période où il n'existe pas | Refus `changeset.no_version` | 14 |
| Modification dont la période déborde la période d'existence de l'objet | Refus (pas de création implicite) ; créer explicitement | 14 |
| Changeset planifié qui heurte une version future d'un autre objet | Refus de la règle concernée, avec la date du conflit | 8 |
| Deux soumissions avec la même clé d'idempotence en parallèle | La seconde attend la première (verrou sur la clé) puis renvoie son résultat | 10 |
| Validation d'un changeset en attente devenu invalide | État `échoué à la revalidation`, violations jointes | 22 |
| Auteur qui valide sa propre proposition | Refus `approval.self` | 21, 42 |
| Retrait par un autre que l'auteur | Refus `changeset.not_author` | 23 |
| Import interrompu pendant un lot | Le lot est atomique : rejoué entièrement à la reprise | 29 |
| Annulation d'un changeset dont tous les objets ont été remodifiés | Changeset inverse vide avec la liste des conflits | 35 |
| Relais d'outbox arrêté | Les événements s'accumulent ; reprise au dernier `seq` livré | 37 |

## Alternatives considérées

- **Table unique avec `system_period` et contraintes partielles** (`WHERE
  upper_inf(system_period)`) : moins de tables, mais chaque index doit exister en double
  (partiel pour le courant, complet pour l'historique), la table courante grossit avec
  l'historique, et toute requête courante doit penser au prédicat. Rejetée.
- **Historisation par la couche service seule** (sans déclencheur) : plus visible dans le
  code, mais une écriture SQL directe (maintenance, migration) contournerait
  l'historique. Rejetée au profit du déclencheur, qui reste trivial et générique.
- **Extension `temporal_tables` ou équivalent** : dépendance externe pour une fonction
  de quelques lignes ; non retenue.
- **Changeset unique asynchrone pour les imports** : rejeté par C3.
- **Ordre global des événements** : exigerait de sérialiser toutes les écritures ;
  rejeté, l'ordre par domaine suffit.

## Risques et migration

- **Volume de l'historique** : ~10⁶ objets × versions × 3 ans, plus la table matérialisée
  de la 003 ; partitionnement mensuel de l'historique, à dimensionner par le banc.
- **Déclencheurs et imports massifs** : coût d'historisation sur les mises à jour en
  masse ; mesuré par le banc d'import (< 15 min pour 10⁶ objets).
- **Découpage des périodes** : bugs subtils (bornes, fusion) ; tests de propriétés
  comparant à un modèle de référence en mémoire.
- **Dépendance 005** : contrôle des droits, permission d'approbation, prédicat de file des
  propositions ; interfaces fixées ici, implémentées là.
- **Préfixe du `type` CloudEvents** : dépend d'un nom de domaine du projet, à fixer.

## Stratégie de test

| Exigences | Type de test |
|---|---|
| 1 à 5 | Intégration : structure, périodes par opération, références internes, taille |
| 6, 7 | Intégration : changeset de 100 opérations avec une erreur → aucune trace ; toutes les violations renvoyées |
| 8 | Intégration : conflits avec des versions futures et passées |
| 9 | Intégration : simulation → base inchangée (comparaison d'empreintes) |
| 10, 11, 12 | Intégration et concurrence : clés d'idempotence parallèles ; ETag ; deux changesets concurrents |
| 13 à 15 | **Tests de propriétés** du découpage contre un modèle en mémoire (suites aléatoires d'opérations sur des périodes) |
| 16 | Intégration : temps système identique dans un changeset ; droits refusés sur l'historique ; historisation déclenchée par un `UPDATE` SQL direct |
| 17, 18 | Intégration : lectures `as_of` avant et après une correction rétroactive |
| 19 à 24, 39 à 43 | Intégration : machine d'états complète ; politiques (volume, propriété, périmètre, auteur) ; propositions, file des mainteneurs, notifications |
| 25, 26 | Intégration : refus sans cascade, cascade avec droits manquants |
| 27 | Intégration : marquage et filtre des changesets rétroactifs |
| 28 à 30 | Intégration : import de 10⁵ opérations avec un lot fautif, reprise, rejeu sans doublon |
| 31 à 33 | Intégration : recherche d'audit ; pseudonymisation sans altération de l'historique |
| 34, 35 | Intégration : annulation simple, annulation avec conflit |
| 36, 37 | Intégration : événement dans la transaction ; relais sans trou avec transactions concurrentes longues ; ordre par domaine |
| 38 | Banc CI : changeset unitaire, lecture `as_of`, import de 10⁶, 50 changesets/s |

## ADR

- [`docs/adr/0017-temps-systeme-tables-historique.md`](../../docs/adr/0017-temps-systeme-tables-historique.md) (proposé)
- [`docs/adr/0018-outbox-ordonnee-par-domaine.md`](../../docs/adr/0018-outbox-ordonnee-par-domaine.md) (proposé)

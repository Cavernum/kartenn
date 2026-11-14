---
status: implemented
updated: 2026-10-17
---

# 002 — Noyau d'adressage : tâches

Chaque tâche est un commit, laisse le build vert et nomme ce qu'elle implémente. Phases :
voir [`../README.md`](../README.md). « verify » = commande ou test qui prouve la tâche.

## P0 — Fondations

- [x] 1. Workspace Cargo : crates vides `kartenn-core`, `kartenn-service`,
  `kartenn-store-pg`, `kartenn-server`, profils de [rust-build](../../docs/practices/rust-build.md)
  (dépendances optimisées en dev, `profiling`, release), `Cargo.lock` versionné,
  `rust-toolchain` non figé (nixpkgs) — prépare tout — verify : `cargo build --workspace`,
  `cargo clippy --workspace -- -D warnings`, `devenv test`
- [x] 2. cargo-hakari : crate `kartenn-workspace-hack`, `.config/hakari.toml`, vérification
  dans `devenv test` (`cargo hakari generate --diff`, `manage-deps --dry-run`) — verify :
  `devenv test`
- [x] 3. Base de test PostgreSQL : harnais d'intégration (une base jetable par test, via le
  PostgreSQL de devenv), extension btree_gist, rôle applicatif sans droits d'administration
  — verify : un test d'intégration trivial passe
- [x] 4. sqlx : dépendance, mode hors ligne (`.sqlx/` versionné, `SQLX_OFFLINE`),
  `cargo sqlx prepare --check` dans `devenv test` — verify : `devenv test`

## P1 — Prototypes de risque

- [x] 5. Générateur de jeu synthétique (crate `kartenn-bench`) : 20 domaines dont 5 sur
  `10.0.0.0/8`, conteneurs sur ≥ 6 niveaux, 10⁵ réseaux, 10⁶ adresses, IPv4 et IPv6, part
  d'espace public — prépare REQ-002-45 — verify : génération reproductible (graine) en
  < 2 min, décompte attendu
- [x] 6. **Prototype GiST** (`docs/spikes/001-gist-exclusions.md`) : (a) exclusion
  `domain_id WITH =, prefix inet_ops WITH =, valid_period WITH &&` ; (b) type `inetrange`
  avec `subtype_diff` et exclusion de chevauchement combinée à btree_gist (uuid) et
  `tstzrange` ; (c) exclusion partielle publique non anycast ; (d) deux transactions
  concurrentes ; (e) temps d'insertion et de recherche du parent sur le jeu de la tâche 5
  — REQ-002-15, 22, 26, 32, 45 — verify : rapport avec mesures ; conception amendée si une
  hypothèse tombe

## P2 — Domaine pur (`kartenn-core`)

- [x] 7. Valeurs d'adresse et de préfixe (`u128`, famille, longueur), contrat
  `AddressFamily`, inclusion, ordre, arithmétique — REQ-002-1 — verify : tests unitaires
  et de propriétés
- [x] 8. Familles IPv4 et IPv6 sur `std::net` : analyse, forme canonique, tests de
  non-régression du comportement vérifié (cas de la conception) — REQ-002-2, 3, 4, 6, 7
  — verify : tests ; propriété analyse(canonique(x)) = x
- [x] 9. Analyse des préfixes (longueur stricte, alignement, suggestion) ; refus des zones
  avec message dédié — REQ-002-4, 5 — verify : tests par cas de la table des erreurs
- [x] 10. Famille de test fictive de 64 bits passant la batterie de contrat commune —
  REQ-002-1 — verify : `cargo test -p kartenn-core --features test-family`
- [x] 11. Registres IANA : CSV versionnés dans `crates/kartenn-core/data/iana/`, structure
  de recherche du bloc le plus spécifique, indicateur public — REQ-002-33, 34 — verify :
  `192.0.0.9` → PCP Anycast ; `10.1.2.3` → Private-Use ; adresse publique → aucune entrée
- [x] 12. Cadre des règles : identifiant stable, sévérité, source, exigences ; catalogue
  généré (Markdown) — REQ-002-37 — verify : test qui compare le catalogue généré au
  fichier versionné
- [x] 13. Objets du noyau et règles locales : slugs, nature, point à point, bornes de
  plage, usages, adresses non attribuables, IPv4-mapped, heuristique ULA, documentation,
  espace partagé, IPv6 > /64 — REQ-002-13, 19, 21, 23, 24, 27, 29, 31, 35, 47, 48 — verify :
  un test valide et un test refusé par règle
- [x] 14. Capacité, prochain libre et occupation en pur (sur des listes triées) —
  REQ-002-43, 44 — verify : tests de propriétés (occupation ≤ capacité, prochain libre
  valide et minimal)

## P3 — Stockage (`kartenn-store-pg`)

- [x] 15. Migrations des objets du noyau (entité, site, domaine, préfixe, plage, adresse)
  en tables courantes + historique (004), contraintes d'exclusion `inet_ops`, `CHECK`,
  index — REQ-002-15, 22, 26, 32 — après 004-5 — verify : tests d'intégration des
  contraintes, y compris concurrence
- [x] 16. Table des registres à usage spécial chargée par migration ; recalcul de
  `public` à la mise à jour avec rapport — REQ-002-34 — verify : test de migration
  — rapport fait (plages comprises) ; le job de migration ne chargeait pas les registres,
  corrigé à la synchronisation (`special_registry.rs`,
  `the_migration_job_loads_the_embedded_registries`) ; la réécriture des drapeaux par un
  changeset système reste à faire, avec l'ordonnanceur (006)
- [x] 17. Identifiants externes et résolution de clé naturelle à une date — REQ-002-40,
  41 — verify : tests d'unicité et de résolution

## P4 — Validation globale (dans le pipeline 004)

- [x] 18. Port `AddressingView` : parent déduit, enfants, chevauchements, publics des
  autres domaines, à une date — REQ-002-14, 42 — après 004-7 — verify : tests
  d'intégration sur le jeu réduit
  — faits de validation livrés ; la navigation générale (parent, enfants) vient en 002-23
- [x] 19. Règles globales des préfixes : doublon, réseau feuille, changement de nature,
  redimensionnement — REQ-002-15 à 18 — verify : un test par règle et par cas de la
  table des erreurs — nature et redimensionnement avec les opérations de modification
  (004-8, `kartenn-service/tests/portion.rs`)
- [x] 20. Règles globales des plages et adresses : dans un réseau, chevauchement, usage
  de plage, unicité — REQ-002-21, 22, 25, 26, 28 — verify : idem
  (`kartenn-service/tests/ranges.rs`)
- [x] 21. Espace public et anycast entre domaines, verrou « espace public » —
  REQ-002-32, 46, 49 — verify : tests, dont concurrence
  (`kartenn-service/tests/anycast.rs` ; migrations 0009 et 0010)
- [x] 22. Ancres : cycles d'entités et de sites, clôture d'une ancre utilisée, domaine
  immuable ; `core.owner` et `core.site` déclarés comme propriétés du noyau (avec 003) —
  REQ-002-8 à 12, 20, 38 — après 003-8 — verify : tests, dont refus d'une manœuvre de
  structure qui change le propriétaire sans droit (`kartenn-service/tests/anchors.rs`) —
  héritage le long des arbres d'entités et de sites : tâche 003-18 (proposée)

## P5 — Lectures et anomalies

- [x] 23. Navigation : enfants directs, ancêtres, à une date — REQ-002-42 — verify : tests
  d'intégration
- [x] 24. Occupation et prochain libre en base — REQ-002-43, 44 — verify : tests
  d'intégration, comparaison avec l'implémentation pure (tâche 14)
- [x] 25. Anomalies : création, fermeture, acquittement, réouverture — REQ-002-35, 36 —
  verify : tests d'intégration
- [x] 26. Catalogue des règles exposé en lecture (pour 006) — REQ-002-37 — verify : test

## P6 — Budgets et synchronisation

- [x] 27. Banc CI : lecture d'un objet, page filtrée, sous-arbre et occupation, changeset
  unitaire sur le jeu de la tâche 5 ; seuils D14 bloquants — REQ-002-45 — verify :
  `kartenn-bench core` (script `bench-core`) sous les seuils, au lieu de `cargo bench`
  (A14) ; pas encore de CI, le banc est bloquant par son code de sortie
- [x] 28. Synchroniser `docs/spec/` (`data.md` : ancres en propriétés), ADR 0014 et 0015 →
  acceptés, statut `implemented` — verify : rapport spec-keeper sans écart — rapport du
  2026-10-17 : écarts de code corrigés (registres chargés au déploiement, anomalies par
  domaine et entité et plages comprises, opération `Rename`, contrat de famille
  complété), textes resynchronisés, contre-audit sans écart ; ce qui reste est différé
  et listé ci-dessous

## P7 — Intégrité des références (amendement du 2026-11-14 à REQ-002-12)

- [ ] 29. Toute référence d'une écriture est lue avant d'être écrite : à la résolution, une
  référence par identifiant (`Ref::Id`) est lue dans la transaction de l'écriture, à sa
  date de validité, avec le type attendu ; absente, d'un autre type ou close, l'écriture
  est refusée par la règle nouvelle `core.reference.unknown` (erreur, 422, champ nommé),
  comme l'est déjà une référence par nom introuvable ; une référence vers un objet créé
  plus tôt dans le même changeset reste acceptée. Recensement : toutes les opérations
  d'écriture du noyau et leurs références (domaine, parent, VLAN, groupe, interface,
  machine, entité…), et une requête qui compte les orphelins déjà en base, par table.
  Vérifier aussi qu'aucun droit porté par domaine ne se contourne par un domaine
  inconnu — REQ-002-12 (amendée), REQ-002-37 — verify : test d'intégration rouge
  d'abord (préfixe, plage, adresse créés sur un domaine inconnu, puis sur l'identifiant
  d'un objet d'un autre type : 422 `core.reference.unknown`, rien d'écrit) ; test qui
  parcourt le catalogue des opérations avec une référence fausse pour chacune ; la règle
  au catalogue (`docs/spec/regles-noyau.md`, `code_labels.rs`) ; `k-check` ; la requête
  des orphelins et son résultat sur la base de dev dans la note de clôture
  où : `crates/kartenn-service/src/pipeline.rs` (résolution des références, vers les
  lignes 471-494, 1079-1082, 1198-1200), `global.rs` (121, 451) ;
  `crates/kartenn-core/src/rules.rs` (modèle : `RBAC_UNKNOWN_ROLE`, `GROUP_UNKNOWN`) et
  `code_labels.rs` ; `crates/kartenn-rest/src/operation.rs` (36-43, 673-689) ;
  `crates/kartenn-store-pg/src/write.rs` (vers 1334) ; `docs/spec/regles-noyau.md`
  - découpée le 2026-11-14 (relevé par l'agent de la tâche, avant tout code) en 29a à 29d :
    le recensement nomme aussi les références des opérations de modules (VLAN, groupe,
    interface, machine), que chaque module résout par la fonction sans type que le
    pipeline passe à `ModuleHandler::plan` (`pipeline.rs` vers 1165) ; y donner le type
    attendu change le contrat des modules (ADR 0026) dans six crates.
- [ ] 29a. Références des opérations du noyau : chaque `Ref::Id` que rend `references()`
  (`pipeline.rs` vers 366) pour une opération du noyau, demandée ou ajoutée par le plan
  d'un module, est relevée là où `plan()` la résout, puis lue en un seul lot par date de
  validité après les verrous ; absente, d'un autre type ou close au début de la période de
  l'opération : `core.reference.unknown` ; `Restore` hors du contrôle (sa cible peut être
  close). Test des droits : un utilisateur lié au seul domaine A ne crée pas de préfixe sur
  un domaine inconnu, et un `Bind` sur un domaine inventé est refusé. Requête des orphelins
  par table (domaines des préfixes, plages, adresses et objets de modules, parents
  d'entité et de site) — REQ-002-12 (amendée), REQ-002-37 — verify : test d'intégration
  rouge d'abord (`tests/it/references.rs`) ; test qui parcourt le catalogue des
  opérations du noyau (un exemple par variante d'`Operation`, chaque `Ref::Id` de son JSON
  rendu par `references()` et refusé par la règle) ; la règle au catalogue ; `k-check`
  où : `crates/kartenn-service/src/pipeline.rs` (`plan`, `stages` après `lock_targets`),
  `port.rs` (`WriteTx`), `crates/kartenn-store-pg/src/write.rs` (modèle :
  `object_kind_covering`, vers 2612), `crates/kartenn-core/src/rules.rs`,
  `code_labels.rs`, `docs/spec/regles-noyau.md` ; tests voisins `tests/it/pipeline.rs`,
  `anchors.rs`
- [ ] 29b. Contrat de `ModuleHandler::plan` avec le type attendu — **arbitrage de
  l'utilisateur : amendement de l'ADR 0026** — la fonction de résolution qu'un module
  reçoit prend le type attendu (`Fn(Ref, ObjectKind) -> Option<Uuid>`) et chaque
  `Ref::Id` qui la traverse entre dans le contrôle de la 29a ; VLAN et groupes de VLAN du
  cœur, modules de test — REQ-002-12 (amendée) — verify : un groupe de VLAN ancré sur un
  identifiant inconnu ou d'un autre type, un VLAN d'un groupe inconnu : 422
  `core.reference.unknown` ; test qui parcourt les types d'opérations des modules du
  build ; `k-check`
  où : `crates/kartenn-service/src/module.rs` (`ModuleHandler::plan`, vers 525),
  `pipeline.rs` (vers 1165), `crates/kartenn-service/src/vlan.rs` (398-540),
  `crates/kartenn-store-pg/src/vlan.rs` (753), `tests/it/support/dummy_module.rs`,
  `kit_module.rs` ; ADR 0026
  dépend de : 29a
- [ ] 29c. Références des opérations de `kartenn-machines` (VLAN, propriétaire, site, hôte
  machine ou grappe, `member_of`, machine, interface, adresse) — **arbitrage de
  l'utilisateur : amendement de l'ADR 0026** — REQ-002-12 (amendée) — verify : une
  interface sur une machine inconnue, une adresse d'interface d'un autre type : 422
  `core.reference.unknown` ; `k-check -p kartenn-machines`
  où : `crates/kartenn-machines/src/handler.rs` (14 résolutions) ; tests
  `crates/kartenn-machines/tests/it/machines.rs`
  dépend de : 29b
- [ ] 29d. Références des opérations de `kartenn-kube`, `kartenn-flow` et `kartenn-policy`
  — **arbitrage de l'utilisateur : amendement de l'ADR 0026** — REQ-002-12 (amendée) —
  verify : une référence inconnue par module : 422 `core.reference.unknown` ; `k-check`
  où : `crates/kartenn-kube/src/handler.rs`, `settings.rs` ;
  `crates/kartenn-flow/src/handler.rs` ; `crates/kartenn-policy/src/handler.rs`,
  `matrix.rs`, `infer.rs`
  dépend de : 29b
- [ ] 30. Clé étrangère temporelle sur les références du noyau : la base refuse d'elle-même
  une ligne dont la référence ne désigne pas une ligne en vigueur sur toute sa période de
  validité (PostgreSQL 18, `FOREIGN KEY (…, PERIOD …) REFERENCES … (…, PERIOD …)`), en
  commençant par `prefix.domain_id`, `range.domain_id` et `address.domain_id` ; une
  migration nouvelle (jamais une migration existante réécrite), précédée du contrôle des
  orphelins : elle échoue avec leur liste plutôt que de les supprimer. À établir avant
  d'écrire la migration : compatibilité avec le modèle bitemporel (la période de validité
  seule entre dans la clé, pas le temps de transaction), contrainte d'exclusion ou clé
  `WITHOUT OVERLAPS` requise sur la table visée, coût sur le banc `bench-core` (seuils
  D14), clôture d'un domaine qui porte encore des objets — un ADR si le modèle doit
  bouger — REQ-002-12 (amendée) — verify : test d'intégration (une insertion directe en
  base, hors service, d'un préfixe sur un domaine inconnu ou clos est refusée par la
  contrainte) ; `k-check` ; `bench-core` réduit sous ses seuils
  où : `crates/kartenn-store-pg/migrations/` (`0004_core_objects.sql:44-61` pour la forme
  des tables) ; `docs/spec/data.md` ; ADR 0014 et 0015 ; `docs/practices/`
  dépend de : 29

## Différé à d'autres specs (constaté à la synchronisation)

- Restitution de la classification IANA de chaque objet (REQ-002-33) : calculée par
  `SpecialRegistry::classify`, servie par l'API (006).
- Réécriture de l'indicateur `public` après une mise à jour des registres, par un
  changeset système planifié (006).
- Permissions par type d'objet sur les écritures du noyau (005).

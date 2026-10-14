---
status: approved
updated: 2026-10-14
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
  — rapport fait ; la réécriture des drapeaux passera par un changeset système (P4, 004-7)
- [ ] 17. Identifiants externes et résolution de clé naturelle à une date — REQ-002-40,
  41 — verify : tests d'unicité et de résolution
  — fait sauf la clé naturelle des plages (table en attente, voir 15)

## P4 — Validation globale (dans le pipeline 004)

- [x] 18. Port `AddressingView` : parent déduit, enfants, chevauchements, publics des
  autres domaines, à une date — REQ-002-14, 42 — après 004-7 — verify : tests
  d'intégration sur le jeu réduit
  — faits de validation livrés ; la navigation générale (parent, enfants) vient en 002-23
- [ ] 19. Règles globales des préfixes : doublon, réseau feuille, changement de nature,
  redimensionnement — REQ-002-15 à 18 — verify : un test par règle et par cas de la
  table des erreurs
  — **partiel** : doublon et réseau feuille faits ; nature et redimensionnement attendent les
  opérations de modification (implementation.md, I3)
- [x] 20. Règles globales des plages et adresses : dans un réseau, chevauchement, usage
  de plage, unicité — REQ-002-21, 22, 25, 26, 28 — verify : idem
  (`kartenn-service/tests/ranges.rs`)
- [ ] 21. Espace public et anycast entre domaines, verrou « espace public » —
  REQ-002-32, 46, 49 — verify : tests, dont concurrence
  — **partiel** : adresses faites ; réseaux anycast (A13, REQ-002-49) à ajouter
- [ ] 22. Ancres : cycles d'entités et de sites, clôture d'une ancre utilisée, domaine
  immuable ; `core.owner` et `core.site` déclarés comme propriétés du noyau (avec 003) —
  REQ-002-8 à 12, 20, 38 — après 003-8 — verify : tests, dont refus d'une manœuvre de
  structure qui change le propriétaire sans droit

## P5 — Lectures et anomalies

- [x] 23. Navigation : enfants directs, ancêtres, à une date — REQ-002-42 — verify : tests
  d'intégration
- [ ] 24. Occupation et prochain libre en base — REQ-002-43, 44 — verify : tests
  d'intégration, comparaison avec l'implémentation pure (tâche 14)
  — fait sans les plages (I2) : les plages DHCP et réservées s'y ajouteront
- [x] 25. Anomalies : création, fermeture, acquittement, réouverture — REQ-002-35, 36 —
  verify : tests d'intégration
- [x] 26. Catalogue des règles exposé en lecture (pour 006) — REQ-002-37 — verify : test

## P6 — Budgets et synchronisation

- [ ] 27. Banc CI : lecture d'un objet, page filtrée, sous-arbre et occupation, changeset
  unitaire sur le jeu de la tâche 5 ; seuils D14 bloquants — REQ-002-45 — verify :
  `cargo bench -p kartenn-bench` sous les seuils
- [ ] 28. Synchroniser `docs/spec/` (`data.md` : ancres en propriétés), ADR 0014 et 0015 →
  acceptés, statut `implemented` — verify : rapport spec-keeper sans écart

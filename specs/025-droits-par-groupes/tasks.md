# 025 — Droits par groupes : tâches

Statut : spec **validée le 2026-10-25**.

- [ ] 1. Migration `0066_rbac_groups.sql`, tables `rbac_group`, `group_member`, `binding`
  sur `group_id` ; résolution `kartenn_member_of` ; `visibility_on`, droits effectifs et
  libellés sur la nouvelle résolution — REQ-025-1, 2, 4, 5 — verify : `k-check`
  - où : `crates/kartenn-store-pg/migrations/0028_rbac.sql` (modèle de `role`,
    `binding`) ; `crates/kartenn-store-pg/src/rbac.rs::visibility_on` (prédicat
    `subject_kind`) ; `rbac_read.rs`, `labels_read.rs` ; test voisin `tests/rbac.rs`.
- [ ] 2. Opérations `SetGroup`, `CloseGroup`, `AddMember`, `RemoveMember`, `Bind` sur
  groupe ; règles `core.group.escalation`, `core.group.builtin` ; annulation —
  REQ-025-3, 6, 7, 8, 10 — verify : `k-check`, INV-27 étendu
  - où : `crates/kartenn-store-pg/src/rbac_admin.rs`, `undo.rs` ;
    `crates/kartenn-core/src/rules.rs` (`RBAC_ESCALATION`) ; `docs/spec/regles-noyau.md`.
- [ ] 3. Groupe livré `secours`, amorçage du compte de secours par appartenance —
  REQ-025-9 — verify : `k-check`, parcours e2e `web/e2e/serve.sh`
  - où : `crates/kartenn-store-pg/src/breakglass.rs` ; synchronisation des rôles livrés.
- [ ] 4. Migration des liaisons existantes en changeset système, sortie de `migrate` —
  REQ-025-14 — verify : test de migration sur une base de la 005
  - où : job `migrate` de `kartenn-server` ; `crates/kartenn-server/tests/it/`.
- [ ] 5. Routes `groups`, `groups/{id}/members`, `bindings` sur groupe, `GET /v1/me` et
  identités — REQ-025-11 — verify : `k-check`, OpenAPI régénérée
  - où : `crates/kartenn-rest/src/rights.rs` ; `specs/006-api-rest/design.md` ligne 94.
- [ ] 6. Politiques d'approbation : auteurs par rôle ou groupe kartenn — REQ-025-13
  - où : `specs/004-changesets-bitemporel/design.md` ligne 180 et son code.
- [ ] 7. Client Python et CLI : `group`, `group member`, `bind --group` — REQ-025-11
  - où : spec 023.
- [x] 8. Maquette E7 par groupes (onglets, fiche d'identité en lecture) et design 021 —
  REQ-025-12 — verify : `k-web2-check`
  - où : `web/v2/shell/src/_ui/ecrans/E7Administration.vue` ; `specs/021-shell-v2/design.md`
    § Administration.
  - Choix tranchés :
    - Les « onglets » sont les entrées de l'espace Administration de la barre latérale
      (Groupes, Rôles, Identités, Comptes de service, Émetteurs, Audit), comme les autres
      espaces ; Émetteurs et Audit n'ont pas encore de vue de maquette.
    - Vues : `groupes` (groupe choisi), `groupe-edition` (retrait et ajout d'un membre et
      d'un rôle en brouillon, « Brouillon 4 »), `groupe-refus` (groupe `astreinte` lié sur
      la portée globale, vu par un administrateur d'entité : membres et clôture refusés
      par `core.group.escalation`, liaison par `core.binding.escalation`), `secours`
      (groupe livré : badge « Groupe livré », Modifier, Clore et ses liaisons refusés par
      `core.group.builtin`, membres par `core.group.escalation`), `identite`, `roles`,
      `secret` ; `edition` et `liaisons` supprimées. Une vue de refus à part, parce que
      G5 interdit de toucher les membres d'un groupe dès qu'une de ses liaisons dépasse le
      périmètre : une même vue ne peut montrer à la fois un brouillon de membres et ce refus.
    - Le motif de refus porte le code de règle pointé entre parenthèses (« Périmètre plus
      large que le vôtre (core.group.escalation). »).
    - Lignes du panneau : l'entrée prend la place restante et passe à la ligne (avant la
      clé, et dans un nom de groupe de l'IdP seulement au « / ») ; la valeur ne se coupe
      pas, la date de fin d'une appartenance ou d'une liaison temporaire en seconde ligne.
      Les rôles des lignes sont écrits libellé puis clé en texte (pas `KeyLabel`, qui ne
      passe pas à la ligne dans un panneau de 420 px).
    - Fiche d'identité : « via » le groupe de l'IdP ou « direct » ; droits effectifs avec
      le groupe qui les donne ; groupes de l'IdP de la dernière session avec leur groupe
      kartenn, « aucun groupe kartenn » sinon. Groupes de la migration (`compte-<clé>`)
      montrés sans libellé, clé seule.
    - La fiche d'un rôle garde ses permissions éditables et liste ses groupes en lecture.
    - Fixtures dans `donnees.ts` (rôles, groupes, membres, liaisons, identités, comptes),
      émetteur inventé `idp-interne`.
- [ ] 9. Bancs `bench-rbac` à 50 groupes de l'IdP par identité — REQ-025-15
  - où : `crates/kartenn-bench/src/rbac.rs`.
- [ ] 10. Amendements de texte : 005 (K4, K7, K17, REQ-005-18, 26, 27 renvoient à la
  025), `docs/spec/` (interfaces, invariants, permissions), README des specs.

# 025 — Droits par groupes : tâches

Statut : spec **validée le 2026-10-25**.

- [x] 1. Migration `0066_rbac_groups.sql`, tables `rbac_group`, `group_member`, `binding`
  sur `group_id` ; résolution `kartenn_member_of` ; `visibility_on`, droits effectifs et
  libellés sur la nouvelle résolution — REQ-025-1, 2, 4, 5 — verify : `k-check`
  - où : `crates/kartenn-store-pg/migrations/0028_rbac.sql` (modèle de `role`,
    `binding`) ; `crates/kartenn-store-pg/src/rbac.rs::visibility_on` (prédicat
    `subject_kind`) ; `rbac_read.rs`, `labels_read.rs` ; test voisin `tests/rbac.rs`.
  - Résultat : `binding.group_id` lu par `kartenn_member_of(identité, émetteur, groupes,
    date)` (fonction SQL `STABLE`) dans `visibilities_on`, `roles_of_on`, `bindings_of`,
    `global_entries`, `labels_read::held` ; `Operation::Bind { group }` (clé) ;
    `BindingSubject` retiré ; `rbac_schema` (forme des liaisons et des membres),
    `an_issuer_is_confined_to_its_entities` (plus de liaison d'émetteur entier).
  - Choix tranchés :
    - `rbac_group` porte `labels` (jsonb par langue, comme `role` depuis la 018) plutôt
      qu'un `label` texte ; clé `^[a-z][a-z0-9_-]*$`, 128 caractères au plus (le tiret
      sert aux clés `idp-…` et `compte-…` de la migration, G9).
    - `group_member.identity_id` sans clé étrangère, comme `binding.subject_id` avant
      lui ; unicité d'un membre par groupe et par période par contrainte d'exclusion.
    - Pas de clé étrangère `binding.group_id` (clé logique, comme `role_id`).
    - Les colonnes `subject_kind`, `subject_id`, `issuer`, `group_name` de `binding`
      restent, nullables, pour la tâche 4 : une ligne neuve ne nomme que son groupe
      (`CHECK`), une ancienne ne donne plus rien ; l'historique les garde.
    - Une liaison vers un groupe absent ou clos : `core.binding.group_closed` (le nom du
      design), à l'écriture comme à l'approbation.
    - `rbac.binding.changed` porte aussi `group`.
    - REST minimal : `POST /v1/bindings` et la vue d'une liaison prennent `group` (clé) au
      lieu de `subject` ; contrat et `schema.d.ts` régénérés.
    - Tests : `kartenn_testing::BIND_IDENTITY` et `GROUP_OF` (un groupe par identité) pour
      les raccourcis SQL des tests ; le compte de secours, en attendant la tâche 3, reçoit
      un groupe à lui lié au rôle administrateur.
- [x] 2. Opérations `SetGroup`, `CloseGroup`, `AddMember`, `RemoveMember`, `Bind` sur
  groupe ; règles `core.group.escalation`, `core.group.builtin` ; annulation —
  REQ-025-3, 6, 7, 8, 10 — verify : `k-check`, INV-27 étendu
  - où : `crates/kartenn-store-pg/src/rbac_admin.rs`, `undo.rs` ;
    `crates/kartenn-core/src/rules.rs` (`RBAC_ESCALATION`) ; `docs/spec/regles-noyau.md`.
  - Résultat : opérations `set_group`, `close_group`, `add_member`, `remove_member` (service,
    miroir REST de `POST /v1/changesets`) ; `rbac::check_group`/`group_rights` ; écriture
    `rbac_admin::write_group` ; annulation par `groups_touched` (groupes, appartenances) en
    cinq étages ; tests `rights_come_and_go_with_the_memberships`,
    `a_delegate_cannot_escalate_through_a_group` (INV-27), `group_changes_are_undone_in_their_order`,
    `a_new_group_is_undone_after_its_members`, refus `core.binding.requester` par
    `add_member` ; `regles-noyau.md`, `permissions.md`, `evenements-audit.md` régénérés.
  - Choix tranchés :
    - Les droits du sujet se lisent **tels qu'affirmés avant les écritures de la
      transaction** (vues `binding_asserted`, `role_asserted`, `role_permission_asserted`,
      `rbac_group_asserted`, `group_member_asserted`, lues par `kartenn_member_of` et
      `visibilities_on`) : sans cela, s'entrer soi-même dans un groupe (REQ-025-7) ou lier un
      rôle à son propre groupe passait le contrôle, fait après les écritures. Hors d'une
      transaction qui écrit, ces vues sont les tables. Effet de bord voulu : un changeset
      ne donne jamais de droit à son auteur en son sein.
    - L'état d'un groupe contrôlé (liaisons, appartenance retirée, liaison retirée) est
      celui d'avant le changeset (`stored_group`, `stored_binding`, `member_group`) ; cela
      corrige au passage `Unbind`, que le contrôle, venu après l'écriture, ne voyait plus.
    - Un administrateur ne se retire pas du dernier groupe qui lui donne le droit de le
      faire : le refus le protège de se verrouiller dehors.
    - Groupe sans liaison (création, ajout, clôture) et renommage : `core.rbac.admin` sur
      un périmètre quelconque (`scopes_holding` puis le décideur) ; refus
      `core.rbac.forbidden`.
    - Liaison et retrait de liaison d'un groupe livré refusés hors synchronisation
      (`core.group.builtin`), comme ses libellés ; ses membres se gèrent librement (G7).
    - Règles ajoutées : `core.group.invalid` (clé, libellés, membre, doublon d'un membre),
      `core.group.unknown` (appartenance à un groupe absent), `core.group.nested`
      (documentaire : la forme d'un membre n'admet pas de groupe kartenn),
      `core.group.escalation`, `core.group.builtin` ; `core.binding.requester` garde son
      code et vise désormais l'appartenance.
    - `RemoveMember` désigne l'appartenance par son identifiant (comme `Unbind`) ; une
      appartenance reprise par l'annulation est une nouvelle ligne.
    - Audit : `rbac.group.changed` (`group`, `change` : `set`, `closed`, `member_added`,
      `member_removed`).
    - Libellés de groupe hors du dictionnaire `GET /v1/labels` (pas de révision des
      libellés) : à revoir avec les routes (tâche 5).
    - Web et clients, au minimum pour ne rien casser : `BindingsView.vue` (colonne et
      champ `group`), `web/e2e/global-setup.ts` et `budgets/global-setup.ts` (groupes de
      même clé que ceux de l'IdP, par changeset), session `k-py-e2e` (groupe
      `kartenn-admins` par `changeset add`, puis `bindings create --set group=…`).
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
- [ ] 8. Maquette E7 par groupes (onglets, fiche d'identité en lecture) et design 021 —
  REQ-025-12 — verify : `k-web2-check`
  - où : `web/v2/shell/src/_ui/ecrans/E7Administration.vue` ; `specs/021-shell-v2/design.md`
    § Administration.
- [ ] 9. Bancs `bench-rbac` à 50 groupes de l'IdP par identité — REQ-025-15
  - où : `crates/kartenn-bench/src/rbac.rs`.
- [ ] 10. Amendements de texte : 005 (K4, K7, K17, REQ-005-18, 26, 27 renvoient à la
  025), `docs/spec/` (interfaces, invariants, permissions), README des specs.

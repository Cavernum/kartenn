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
- [x] 3. Groupe livré `secours`, amorçage du compte de secours par appartenance —
  REQ-025-9 — verify : `k-check`, parcours e2e `web/e2e/serve.sh`
  - où : `crates/kartenn-store-pg/src/breakglass.rs` ; synchronisation des rôles livrés.
  - Résultat : `kartenn_core::authz::SECOURS` (`BUILTIN_GROUPS`), synchronisé par
    `rbac::sync_roles` (libellés, liaison `administrator` globale) ;
    `breakglass::create` écrit l'appartenance `[création, valid_until)` ; test
    `the_group_secours_is_delivered_and_kept` ; `k-web-e2e` (86 parcours, dont
    `admin.spec`, par `serve.sh` ; un échec Firefox de `plugins.spec`, sans lien, repasse
    seul) et `k-py-e2e` verts.
  - Choix tranchés :
    - Le groupe livré déclare ses rôles globaux (`BuiltinGroup::roles`) ; la
      synchronisation n'ajoute que les liaisons manquantes, dans le même changeset système
      que les rôles (`roles sync`), sans retirer une liaison qu'elle n'a pas posée.
    - `breakglass create` exige le groupe `secours` livré (sinon « run `kartenn-server
      migrate` first »), comme il exigeait le rôle.
    - `account.breakglass.created` porte `group` (`secours`) au lieu de `role` et `scope`.
- [x] 4. Migration des liaisons existantes en changeset système, sortie de `migrate` —
  REQ-025-14 — verify : test de migration sur une base de la 005
  - où : job `migrate` de `kartenn-server` ; `crates/kartenn-server/tests/it/`.
  - Résultat : migration `0068_rbac_bindings_reported.sql` (report en SQL, table
    `rbac_migration_report`, colonnes de sujet retirées de `binding`, `group_id NOT
    NULL`, index `binding_by_identity`/`binding_by_issuer` retirés) ;
    `kartenn_store_pg::rbac_migration::report` (liste, audit `rbac.migration.reported`,
    marque) appelé par `maintenance::deploy`, qui rend `Deployed { rbac_report }` ;
    `kartenn-server migrate` imprime les lignes ; test
    `rbac_report::bindings_of_the_005_are_reported_on_groups` (base arrêtée avant la
    0066, trois sortes de liaisons, homonymes, liaison bornée, annulations).
  - Choix tranchés :
    - Report **dans une migration SQL** plutôt que dans le job après les migrations : le
      retrait des colonnes doit suivre la conversion et le job ne passe qu'après toutes
      les migrations en attente ; une migration qui refuserait de tourner bloquerait le
      premier déploiement, et un code Rust appelé à mi-chemin dépendrait d'un schéma
      figé à la 0066. Le changeset système est écrit en SQL (auteur système, `applied`,
      motif `bindings reported on groups 025`), comme `breakglass::create` ; le job ne
      fait que lister et auditer, une fois (`reported_at`).
    - Conversion **sur place** : la liaison garde son identifiant et sa période (un
      `Unbind` en attente reste valable), prend `group_id` et le changeset du report ; la
      version d'avant reste dans `binding_history`, qui **garde les anciennes colonnes**
      (le design : « colonnes archivées dans l'historique »), seules celles de `binding`
      partent. Groupes et appartenances commencent à la plus ancienne des liaisons
      qu'ils portent (le changeset est `retroactive` si c'est dans le passé).
    - Seules les liaisons en vigueur ou à venir sont reportées ; une liaison close avant
      sort de la table (sa trace reste dans l'historique), sans groupe créé pour elle.
    - Liaison d'émetteur entier : listée puis retirée de la table (`DELETE`) ; une
      clôture écrite dans la même transaction ne serait pas gardée par l'historique (une
      version remplacée dans sa transaction ne l'est pas, 0018), la table du report et
      l'audit en sont la trace.
    - Clés : nom en minuscules, toute autre suite de caractères en `-` ; `compte-` +
      nom affiché de l'identité (l'identité n'a pas de clé), `idp-` + émetteur + `-` +
      groupe ; une clé trop longue, vide, en double ou déjà prise finit par 8 chiffres
      hexadécimaux du md5 de sa source.
    - Annulation : une liaison d'avant les groupes rendue par l'annulation d'un changeset
      de la 005 l'est **sur le groupe que le report a donné à son ancien sujet** (table
      du report) ; sans tel groupe (liaison d'émetteur, sujet sans liaison reportée), elle
      est listée en `unsupported` (`binding`) au lieu d'une erreur. Les liaisons
      converties ont le report pour changeset ultérieur : conflit, comme toute écriture
      postérieure. Le changeset du report lui-même ne s'annule pas (`unsupported`
      `rbac_migration`).
    - Migration numérotée 0068 après la `0067_policy_authors` de la tâche 6 ; la sortie
      de `migrate` ne reprend pas les politiques de la 0067 (ses `WARNING` vont au journal
      de PostgreSQL, rien n'est gardé à relire).
- [x] 5. Routes `groups`, `groups/{id}/members`, `bindings` sur groupe, `GET /v1/me` et
  identités — REQ-025-11 — verify : `k-check`, OpenAPI régénérée
  - où : `crates/kartenn-rest/src/rights.rs` ; `specs/006-api-rest/design.md` ligne 94.
  - Résultat : routes de `rights.rs` (`groups`, `group`, `create_group`, `set_group`,
    `close_group`, `group_members`, `group_member`, `add_member`, `remove_member`),
    `?group=` sur `GET /v1/bindings` ; lectures `rbac_read::{groups, group,
    group_by_key, members, member, created_member, groups_of}` ;
    `authn::Rights.groups` (`GroupVia`), `HeldBinding.group`, port
    `Identities::record_login` ; migration `0069_person_last_login.sql` ; libellés de
    groupe dans `labels_read::stored` (espace `group`) et révision par `rbac_write` ;
    `CORE_COLLECTIONS` gagne `groups` ; contrat, `schema.d.ts` (web et web/v2),
    instantanés du client Python régénérés ; test
    `rights::groups_and_members_are_administered`, `sessions` (groupes gardés à la
    connexion).
  - Choix tranchés :
    - `{id}` d'un groupe : son identifiant (UUID), comme le dit le design ; une clé de
      groupe peut ressembler à un UUID, on n'accepte pas les deux.
    - `POST /v1/groups {key, labels}` → 409 si la clé existe (le `SetGroup` sous-jacent
      renommerait) ; `PATCH {labels}` seulement, la clé ne change pas ; `DELETE` clôt
      groupe, appartenances et liaisons (`CloseGroup`).
    - `GET /v1/groups/{id}/members/{member}` ajouté (cible du `Location` d'un ajout).
      Un membre se donne `{identity}` ou `{issuer, group}` ; autre forme → 422.
    - `GET /v1/me` : `groups` devient la liste des groupes kartenn (`[{key, via}]`, une
      entrée par appartenance), les groupes de l'émetteur passent en `idp_groups`
      (rupture assumée avant la première version) ; `bindings` gagne `group`.
    - Q-025-1 : les sessions sont des cookies scellés (ADR 0020), le serveur ne les garde
      pas. Les groupes de l'émetteur et l'instant de la **dernière connexion** sont donc
      écrits sur `person` (`last_login_at`, `last_login_groups`) par `logged_in` (pas
      par un jeton d'accès, qui n'est pas une session) ; effacés par la
      pseudonymisation. La fiche d'identité résout ses groupes et droits effectifs avec
      ces groupes-là.
    - Libellés d'un groupe au dictionnaire, espace `group` : servis à un lecteur des
      droits, et à chacun pour ses propres groupes (comme les rôles) ; écrire ou clore un
      groupe change la révision.
- Notes pour la tâche 7 (client et CLI) : le catalogue dérivé du contrat expose déjà
  `groups` (instantané `test_tree.ambr`) ; `kartenn me` affiche `key (via)` ; à faire :
  `group member …` (sous-ressource), `bind --group`, `idp_groups` et `last_session`
  dans les sorties.
- [x] 6. Politiques d'approbation : auteurs par rôle ou groupe kartenn — REQ-025-13
  - où : `specs/004-changesets-bitemporel/design.md` ligne 180 et son code.
  - Résultat : `ApprovalPolicy::authors` retiré, `author_groups` devient une liste de clés
    de groupes kartenn ; `WriteTx::groups_of` (`rbac::groups_of_on`, par
    `kartenn_member_of` à `now()`, vues `_asserted`) lu par `review::holding_policy` ;
    validation à l'écriture dans `pending::set_policy` ; migration `0067_policy_authors.sql` ;
    tests `policies_designate_their_authors_by_group_of_kartenn` (service),
    `policy_authors_keep_roles_and_groups_of_kartenn` (rejeu de la migration),
    `an_author_is_designated_by_role_or_group` ; contrat, `schema.d.ts` et
    `regles-noyau.md` régénérés ; 004 (design, REQ-004-40) et 005 (REQ-005-27, design)
    amendées.
  - Choix tranchés :
    - Forme : `author_groups: ["clé", …]` (plus `[émetteur, groupe]`), champ `authors`
      (comptes) supprimé de `set_approval_policy` ; stockage `{"group": clé}` à côté de
      `{"role": clé}`. Un champ `authors` encore envoyé est ignoré, comme tout champ
      inconnu d'une opération.
    - Règle `core.approval_policy.author` : un rôle ou un groupe absent à l'écriture de la
      politique est refusé (un identifiant de compte mis en clé de groupe tombe dessous) ;
      contrôle dans le magasin (`set_policy`), sur les tables et non les vues
      `_asserted`, pour qu'un groupe créé plus tôt dans le même changeset compte. Un groupe
      clos ensuite laisse la politique en place, sans auteur qui corresponde.
    - Évaluation : par rôle, les rôles des liaisons des groupes de l'auteur
      (`roles_of`, inchangé) ; par groupe, l'appartenance directe ou par un groupe de l'IdP
      de la session (instantané gardé avec le changeset en attente) à `now()`, donc à la
      soumission puis à la revalidation.
    - Migration des politiques existantes : la clé `compte-<clé>` de la tâche 4 n'étant
      pas fixée et ses groupes n'étant créés que pour les identités liées, pas de
      conversion : les auteurs par compte (`person`) et par groupe de l'IdP sont retirés
      par `0067`, chaque politique listée en `WARNING` de la migration ; une politique
      restée sans déclencheur est désactivée (et listée) plutôt que laissée active sans
      effet. La sortie de `migrate` (tâche 4) pourra reprendre cette liste. Numéro `0067`
      à renuméroter si la tâche 4 en prend un.
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

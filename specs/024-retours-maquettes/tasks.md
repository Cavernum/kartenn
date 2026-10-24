---
status: approved
updated: 2026-10-24
---

# 024 — Retours sur les maquettes : tâches

Brouillon du 2026-10-24, à valider avec les exigences et la conception. Phases : P0
(permissions) avant tout ; P1 (brouillons, statistiques, simulation) en parallèle ; P2
(révision des specs 020, 021) puis P3 (maquettes) ; P4 clôture.

## P0 — Deux chemins de droits

- [x] 1. Catalogue `edit`/`apply`/`apply_own`, migration des rôles par changeset système,
  `write`, `propose` et `core.changeset.approve` retirés — REQ-024-9 — verify :
  `k-check` ; aucune occurrence des anciens noms hors migrations et audit (test du type
  018/REQ-018-6)
  - où : `docs/spec/permissions.md` ; catalogues des modules (`crates/kartenn-*/src/`,
    `registry.rs`) ; migration `crates/kartenn-store-pg/migrations/0063_permission_verbs.sql`
    sur le modèle de `0058_permission_names.sql` ; test voisin : celui de 018-1 à 4
  - Résultat : `authz::retired`/`retired_names` composent la 018 et la 024, rôles
    migrés par `rbac::rename_permissions` (`custom_roles_take_the_verbs_of_spec_024_once`),
    `no_old_permission_name_is_left` étendu à tous les anciens noms ; `k-check` vert.
- [x] 2. Soumission toujours en attente sous `edit` ; validation par `apply` (autre) ou
  `apply_own` (auteur) ; politique qui retient → `changeset.approval.self` —
  REQ-024-10, 11, 12 — verify : matrice auteur/autre × apply/apply_own × politique
  - où : `crates/kartenn-service/src/pipeline.rs`, `rights.rs`,
    `review.rs::validate_pending_with` ; design « B »
  - Résultat : `rights::Check { subject, verb }` (`authz::verb_permission`), droits de
    l'auteur revérifiés puis application du valideur ; matrice
    `the_application_follows_the_author_and_the_policies` (service `review.rs`).
- [x] 3. `?apply=own` sur `POST /v1/changesets` (une transaction, deux audits, 202 en
  repli) ; imports et annulation suivent la règle ; aides de test — REQ-024-13 — verify :
  test de repli 202, audit des deux événements
  - où : `crates/kartenn-rest/src/changesets.rs` ; `crates/kartenn-service/src/import.rs`,
    `undo.rs` ; aides de `crates/kartenn-testing`
  - Résultat : `ChangesetRequest::apply_own`, lu de la requête par l'authentification ;
    202 avec `reason` (rejoué à l'identique) ; `apply_own_applies_or_falls_back_to_pending`
    (REST) ; aides `kartenn_testing::applied_by_author`, `common::applied`.
- [x] 4. Amendements de texte : 004 (C1, REQ-004-21, 41, 42 renvoient à la 024), 005,
  018, `docs/spec/` (règles, permissions) ; client Python et CLI (noms, `apply=own`) —
  verify : `k-py-check`, `spec-keeper` sur la 004
  - où : `specs/004-changesets-bitemporel/requirements.md` § D ;
    `clients/python/kartenn/`, `clients/python/kartenn-cli/`
  - Résultat : 004 (C1, C7, REQ-004-21, 39, 41, 42, 43), 005 (K12, REQ-005-16, 22), 018
    (R2, R3, REQ-018-1, 4), `docs/spec/` (interfaces, permissions, règles, événements) ;
    client : `submit(apply_own=…)`, `Pending.reason`, CLI `changeset apply --submit-only` ;
    `k-py-check`, `k-py-e2e` verts.

### Choix tranchés (P0)

- Une permission d'une autre action que `edit` (`core.role.admin`, `policy.rule.admin`,
  `flow.request`, `flow.review`) vaut édition, application et application de ses propres
  changesets (`authz::verb_permission`) : la 024 ne lui donne pas de verbes.
- `flow.write` (sans `propose`) devient aussi `flow.edit`, `flow.apply`, `flow.apply_own`.
- `core.changeset.approve` migre vers chaque `….apply` du catalogue du build et
  `prop.*.apply` (les propriétés sensibles demandent leur permission propre) ; le rôle
  `approver` livré reçoit les `apply` des objets du noyau et des modules,
  `editor` `edit` + `apply_own` du noyau et `prop.*`.
- `?apply=own` est lu pour toute écriture (pas seulement `POST /v1/changesets`) par
  l'authentification, porté par `RequestContext::apply_own` puis
  `ChangesetRequest::apply_own` ; l'OpenAPI le déclare sur toute opération qui répond 202.
- Changeset appliqué dans la requête qui le soumet : `kind = direct`, `validator_id` =
  l'auteur (« appliqué par son auteur ») ; audits `changeset.state.pending` puis
  `changeset.state.applied`. La contrainte `validator_id <> author_id` tombe (0063).
- La marque « appliqué par son auteur » de l'historique est `validator_id = author_id` (le
  champ `validator` de la vue d'un changeset) ; aucun champ dédié n'est ajouté au
  contrat. La soumission d'un brouillon (`/v1/drafts/{id}/submission?apply=own`) vient
  avec les brouillons (tâche 6).
- Deux motifs de repli (arbitré le 2026-10-24) : `changeset.apply_own.missing` quand
  l'auteur n'a pas `apply_own` sur un objet ou un effet (le message nomme chaque
  permission manquante), `changeset.approval.self` quand une politique le retient ou
  qu'un module exige sa revue ; gardés dans `required_rights.reason` pour rejouer la 202.
  La validation par l'auteur sans `apply_own` répond 403 `changeset.apply_own.missing`.
- À la validation, les droits d'édition de l'auteur sont revérifiés (comme pour une
  approbation), sauf sur les opérations de revue que joint le valideur.
- Un module qui exige une revue (REQ-016-10) interdit l'auto-application, comme une
  politique.
- `GET /v1/proposals` ne liste pas les changesets de l'appelant ; un changeset en attente
  sans exigences enregistrées (journalisé avant la 024, retenu par une politique) n'est
  rejetable par personne, il se valide (droits recalculés) ou expire.
- Clients : le SDK web, le client Python et la CLI demandent `apply=own` sur toute
  écriture par défaut (M9) ; `submit(apply_own=False)` / `--submit-only` soumettent seuls.
- Migration du schéma numérotée 0063 ; la table des brouillons (tâche 5) prendra 0064.
- Motif du changeset de migration des rôles : `permission rename 024` (018 et 024).
- Maquette E4 : droits requis `core.prefix.apply`, `vlan.apply`, `prop.*.apply` ; seuls
  les instantanés d'accessibilité (`.aria.yml`) sont mis à jour, les images restent à
  reprendre (`k-web-shots --update`) une fois les polices de Playwright corrigées.

## P1 — Serveur (en parallèle, après P0)

- [x] 5. Table `draft`, routes, 404 d'un autre, If-Match, limites, rétention annoncée,
  reprise par `core.draft.admin` (lecture, suppression, attribution, toutes auditées) —
  REQ-024-1, 2, 3, 7, 7a — verify : tests d'intégration par route ; audit d'une lecture
  par l'administrateur
  - où : `crates/kartenn-store-pg/migrations/0064_drafts.sql` ;
    `crates/kartenn-store-pg/src/drafts.rs` ; `crates/kartenn-rest/src/drafts.rs`
    (modèle : `changesets.rs`) ; job `draft-retention` dans `crates/kartenn-server/src/jobs.rs` ;
    design « A »
  - Résultat : `crates/kartenn-rest/tests/it/drafts.rs` (cycle sous If-Match, 404 d'un
    autre, compte de service, limites, rétention annoncée puis appliquée, administrateur :
    liste, lecture, suppression, reprise auditées) ; `k-check` vert.
- [x] 6. Simulation et soumission d'un brouillon (`?apply=own`), audit, SSE `draft`
  filtré par auteur — REQ-024-4, 5, 6 — verify : brouillon gardé en 422, supprimé en
  201/202 ; flux d'un autre sans l'événement
  - où : `drafts.rs` ; distributeur de la 022 (`crates/kartenn-rest/src/events/hub.rs`,
    `stream.rs` : contrôle des droits à la remise) ; `crates/kartenn-store-pg/src/write.rs`
    (journal du changeset) ; `kartenn_service::events` (`Body::Draft`, `Sight::is`)
  - Résultat : `a_submission_replaces_the_draft_or_leaves_it`,
    `a_draft_submitted_pending_is_deleted_too` (REST), `a_draft_event_reaches_its_author_alone`
    (flux), `a_draft_reaches_its_author_alone` (rendu) ; client `kn.drafts`, CLI
    `kartenn drafts list|show|submit|delete` ; `k-py-check` vert.
  - Choix tranchés (5, 6) :
    - Migration 0064 (0062, 0063 prises) ; `changeset.draft_id` sans référence (le
      brouillon disparaît), exposé `draft` dans la vue d'un changeset ; tâche
      `draft-retention` quotidienne.
    - Compte de service : 403 sur toute route `/v1/drafts…` (message explicite), plutôt
      que 404 : il n'a jamais de brouillon, rien à cacher.
    - Brouillon d'un autre sans `core.draft.admin` : 404 sans audit de refus ;
      `GET /v1/identities/{id}/drafts` d'un autre sans elle : 403 audité
      (`authz.access.denied`), le sien permis. Permission globale vérifiée sur l'objet
      sentinelle `reads::DRAFTS` (`Uuid::from_u128(4)`).
    - Reprise : `If-Match` exigé, 409 si le brouillon est déjà à l'appelant, 422
      `draft-limit` si l'appelant en garde 50 ; l'ancien auteur reçoit `{id, deleted}`.
    - Audit : `draft.state.created` (`operations`), `draft.state.submitted` (`state`, dans
      la transaction du changeset), `draft.state.deleted` (`cause` `author` | `admin` |
      `expired`, `author`), `draft.admin.read` (`author`, `count` : liste et lecture),
      `draft.admin.taken_over` (`author`) ; criticité haute pour les deux derniers.
    - Problème nouveau `draft-limit` (422) : 51ᵉ brouillon, plus de 10 000 opérations.
      Titre de 1 à 200 caractères (422 `invalid-member`) ; opération invalide → 400 avec
      `pointer`.
    - `expires_at` présent seulement à partir du 83ᵉ jour sans modification (absent
      avant).
    - Motif : `reason {summary, description}` → `summary`, puis une ligne vide et la
      description ; la simulation prend le titre à défaut de résumé ; la soumission sans
      résumé répond 422 `motive-required`, le brouillon reste.
    - Soumission : corps facultatif `{valid_from, valid_to}` ; `If-Match` facultatif (412
      si le brouillon a changé) ; `?simulate=true` vaut simulation. Le brouillon est
      supprimé dans la transaction du changeset (`RequestContext::draft` →
      `Journal::draft`), à la version lue seulement : modifié entre-temps, il reste.
      Rejouée avec la même `Idempotency-Key` une fois le brouillon parti, la soumission
      rend la première réponse (`idempotency_key` × `changeset.draft_id`).
    - Événement : ligne d'outbox `org.kartenn.draft.changed` sans changeset
      (`{draft, author, version}` ou `deleted`), sorte `draft` sélectionnable par `kinds`,
      remise si l'identité du flux est l'auteur (`Sight::is`). Un relais CloudEvents futur
      devra l'écarter (aucun en production).
    - Liste sans pagination (50 au plus) : `{items}`, compte `operation_count`.
    - Client : `submit` lit le brouillon pour son `If-Match` ; CLI : création,
      modification, ajout, simulation et reprise restent à `kartenn api` (exclues de la
      couverture), la collection `drafts` passe en commande nommée.
- [x] 7. `GET /v1/stats`, port `StatsContributor` des modules, cache par empreinte des
  droits, SSE `stats` regroupé, scénario de banc — REQ-024-14, 15, 16 — verify : tests
  de filtrage par les droits ; `bench-core` scénario `stats` p95 ≤ 200 ms (lancé par
  l'agent principal)
  - où : `crates/kartenn-rest/src/stats.rs` ; `crates/kartenn-service/src/reads.rs`
    (prédicats de lecture) ; `crates/kartenn-bench`
  - Résultat : `kartenn_rest::stats` (`compute`, route), `kartenn_service::module::
    StatsContributor` (`Module::stats`), `list::count`/`module_counts`,
    `anomalies::open_count`, `changesets::applied_by_day`/`pending_of`,
    `outbox::revision` ; SSE `Kind::Stats` (`Hub::signal_stats`) ; tests
    `each_counter_follows_the_rights_of_the_caller`, `the_etag_and_the_cache_follow_the_outbox`,
    `stats_events_are_coalesced` (REST), `stats_count_the_vlans_the_caller_reads` (VLAN) ;
    `stats_budgets` dans `bench-core` (petite échelle validée, mesure complète à faire) ;
    `client.stats()`, `kartenn stats`.

### Choix tranchés (tâche 7)

- Port : `StatsContributor::contribute(ModuleRead, StatsScope)` dans
  `kartenn-service::module`, rendu par `Module::stats()` ; chaque module l'implémente sous
  sa fonctionnalité `rest` (`src/stats.rs`). `StatsScope` porte l'argument JSON de
  `kartenn_scopes`, `reads(type)` et l'identité de l'appelant ; `Contribution` porte ses
  compteurs et, au besoin, le compte d'un de ses types en remplacement de celui du noyau.
- Clés de `objects` : sortes qualifiées (`core.prefix`, `vlan.vlan`). Noyau : table
  courante et prédicat des pages (`list::count`) ; types des modules : `module_object` et
  le prédicat générique (`list::module_counts`) ; une sorte lisible sans objet vaut 0,
  une sorte lue nulle part est absente. Visibilité globale sans confinement : `count(*)`
  sans prédicat. Les flux sont comptés par leur module (lecture par `flow.read` ou comme
  demandeur, REQ-011-17).
- Compteurs des modules : VLAN `reserved`, `deprecated` ; machines `physical`, `virtual`
  (virtuelles et conteneurs) ; flux `noncompliant` (synthèse non conforme),
  `waivers_expiring` (dérogation qui finit sous 30 jours) ; politique `rules_allow`,
  `rules_deny`.
- `changesets` : présent seulement sous `core.changeset.read` (lecture globale des
  changesets) ; tous les changesets appliqués, ceux de kartenn compris ; `by_day` par
  jour UTC, le plus ancien d'abord, aujourd'hui en dernier ; `applied_7d`/`applied_30d`
  sont les sommes des 7 et 30 derniers jours calendaires (pas une fenêtre glissante).
  Index partiel `changeset (applied_at) WHERE state = 'applied'` (migration 0065).
- `pending.to_apply` = le nombre de `GET /v1/proposals` (`review::proposals_for`) ;
  `pending.mine` = ses propres changesets en attente, comptés sans
  `core.changeset.read` (ce sont les siens). `anomalies.open` toujours présent : la liste
  des anomalies n'a pas de permission globale, seulement la lecture de leur objet.
- Instantanés (tâche 7b, M18) : deux niveaux de `Snapshots` — la partie qui ne dépend
  que des droits par (SHA-256 du périmètre de lecture, lecture des changesets, base),
  `pending` et les compteurs des modules (le flux compte ceux dont l'appelant est
  demandeur) par (sujet, groupes, clé des droits) ; chaque clé garde de quoi se
  recalculer (périmètre, sujet). Révision = `max(seq)` de l'outbox, lue avant le calcul :
  un changement validé pendant le calcul le rend dû de nouveau. Comptes lancés en
  parallèle sur le pool (≈ 10 connexions par calcul).
- Recalcul : déclenché par la lecture qui trouve l'instantané dû (elle le sert tel quel)
  et par `Snapshots::run`, qui balaie toutes les 5 s (`Freshness::tick`) et ne lit la
  révision que si une clé active a l'âge voulu. Dû aussi quand l'instantané date d'un
  jour UTC antérieur, sans changement : `by_day` glisse à minuit. Deux recalculs à la
  fois au plus (sémaphore) ; le calcul en ligne d'une clé neuve n'y passe pas (un
  appelant attend), mais tourne dans une tâche à part : une requête abandonnée ne laisse
  pas la clé occupée, et les premiers appels simultanés d'une clé attendent le même
  calcul. Un échec garde l'instantané précédent (journalisé `stats.snapshot.failed`).
- Pleine : on oublie d'abord les clés non lues depuis 24 h, puis la moins récemment lue
  qui n'est pas en calcul ; le balayage oublie aussi. Réglages dans
  `kartenn_rest::stats::Freshness` (non exposés dans la configuration), passés par une
  `Extension<Arc<Snapshots>>` ; sans elle (tests, bancs anciens), un jeu du processus aux
  réglages par défaut, recalculé seulement par les lectures.
- `computed_at` : RFC 3339, date du plus ancien des deux niveaux ; recalculé à
  l'identique, le corps change quand même de date, donc d'ETag (sans événement `stats`).
- ETag fort : 128 premiers bits du SHA-256 du corps ; `Cache-Control` d'une lecture du
  présent. Aucun `data.objects.read` : un compte ne divulgue aucun objet.
- SSE `stats` : `Message::Signal`, sans `id:` (le curseur ne bouge pas), données `{}` (un
  navigateur ne distribue pas un événement sans données), sélectionnable par `kinds` ;
  dû quand un recalcul a changé les compteurs d'un niveau (`Hub::stats_changed`, appelé
  par `Snapshots::on_changed` ; depuis la tâche 7b, plus à chaque notice), envoyé à
  toutes les connexions (sans contenu, rien à filtrer), au plus un par
  `Settings::stats_every` (30 s, non exposé dans `[events]`) ; un signal retenu part au
  premier passage du lecteur après l'intervalle.
- Client : `Client.stats()`/`AsyncClient.stats()` rendent le corps ; `stats` n'est pas une
  ressource dérivée du catalogue ; `kartenn stats [-o FORMAT]` parmi les outils.
- Banc : deux budgets dans `bench-core`, après VLAN et machines — un lecteur de tout
  (`*` global) et le sujet de `bench-rbac` (50 groupes, 1 000 liaisons) —, chacun mesuré
  sur la route servie de son instantané (200 lectures par le routeur, après une première
  qui calcule, budget 200 ms) et sur le calcul (`stats::compute`, 20 calculs après 2 de
  mise en route, budget 10 s ; 20 et non 40 pour borner la durée du banc complet) ;
  p50/p95/p99 dans le rapport.
- [x] 7b. Statistiques servies d'un instantané recalculé en arrière-plan (M18) —
  REQ-024-16 — verify : tests (instantané servi sans calcul, recalcul dû après une
  notice et 30 s, un seul calcul par clé, clé oubliée, `computed_at`) ; `bench-core`
  scénario `stats` : route p95 ≤ 200 ms, calcul p95 ≤ 10 s (lancé par l'agent principal)
  - où : `crates/kartenn-rest/src/stats.rs` (les deux caches de 30 s deviennent des
    instantanés) ; relais de l'outbox et `Hub::signal_stats`
    (`crates/kartenn-rest/src/events/hub.rs`) ; design § C ; banc `stats_budgets`
    (`crates/kartenn-bench`) : ajouter la mesure de la route servie et p50/p99 ;
    test voisin `the_etag_and_the_cache_follow_the_outbox`
  - Résultat : `kartenn_rest::stats::{Snapshots, Freshness}` (`Snapshots::run`,
    `sweep`, `on_changed`, `computations`), `computed_at` dans `Stats` ;
    `Hub::stats_changed` (l'événement `stats` ne suit plus les notices) ; câblage dans
    `kartenn-server` (`app.rs`) ; tests `the_etag_and_the_snapshot_follow_the_outbox`,
    `nothing_changed_nothing_is_recomputed`, `one_computation_per_key_at_a_time`,
    `the_sweep_keeps_the_keys_read_lately` (REST), `stats_events_are_coalesced`,
    `a_recomputation_that_changed_the_statistics_sends_stats` (SSE) ; `stats_budgets`
    mesure la route servie (200 lectures) et le calcul (20), p50/p95/p99 (petite
    échelle validée, mesure complète à faire).
- [x] 8. `?simulate=true` sur toute route d'écriture, `x-kartenn.simulate`,
  `simulate.unsupported` ; xfail `--dry-run` de la CLI levé — REQ-024-17 — verify :
  test générique sur toutes les routes d'écriture de l'OpenAPI ; `k-py-e2e` sans xfail
  - où : `crates/kartenn-rest/src/kit.rs`, `resources.rs`, `cycle.rs` ;
    `clients/python/kartenn-cli/` (dry-run) ; `clients/python/e2e/`
  - Résultat : `RequestContext::simulate` lu par l'authentification, honoré par
    `changesets::submit_answered` (point de passage de toute écriture à changeset) ;
    `simulation::gate` refuse le reste avant le handler ; test générique
    `simulated::every_write_simulates_or_says_it_cannot` (81 écritures simulées, 14
    refusées, changesets, outbox et lectures inchangés) ; client `simulate=True`, CLI
    `--dry-run` par la route elle-même ; `k-py-e2e` sans xfail.
  - Choix tranchés :
    - Une écriture simule si et seulement si son 202 est un changeset en attente
      (`ChangesetPending`, `UndoPending`) : `x-kartenn.simulate` en est tiré dans
      `openapi::finish`, et la porte (`route_layer`) en est tirée à son tour ; une route
      ajoutée sans 202 de changeset (brouillons, tâches 5–6) refuse donc
      `simulate=true` par défaut, sans rien écrire.
    - Le code de la 024 `simulate.unsupported` est la classe de problème
      `urn:kartenn:problem:simulate-unsupported` (400), à la forme des autres types,
      libellée en français et en anglais dans l'espace `problem` (018).
    - Imports (`create`, `chunks`, `start`, `resume`, `cancel`) : refusés ; un import ne
      soumet pas de changeset dans la requête (tâche de fond par lots). Rapports de
      politique, acquittement d'anomalie, pseudonymisation, filtres de journal,
      connexion : refusés.
    - Secrets de compte de service (ajout, révocation) et création de compte : ils
      soumettent un changeset, ils simulent donc (aucun secret n'est montré par une
      simulation) ; la liste « secrets » du design visait les routes sans changeset.
    - OpenAPI : paramètre `simulate` et 200 `ChangesetSimulated` ajoutés aux écritures
      qui simulent ; à côté d'un 200 « appliqué » (PATCH, DELETE), le schéma devient
      `oneOf` des deux.
    - Une simulation journalise une ligne de lecture (`data.objects.read`, cible
      `changeset`, critères `{simulate, valid}`), `POST /v1/changesets?simulate=true`
      compris : le chemin existant n'en écrivait aucune (M6).
    - `simulate=false` est accepté partout (sans effet) ; une valeur autre que
      `true`/`false` ou répétée → 400 `bad-request`.
    - Client Python : `Simulation` passe dans `kartenn.resources` (réexportée par
      `kartenn.changesets`) ; `simulate=True` sur `create`, `update`, `close`, `verb` et
      `call`, refusé localement (`client.simulate-unsupported`) quand le contrat dit
      non ; `Operation.simulate` lu de `x-kartenn`. CLI : `--dry-run` envoie l'écriture
      elle-même (plus de traduction en opération de changeset), motif par défaut
      « simulation » ; `--into-changeset` garde la traduction.

## P2 — Révision des specs de l'interface

- [x] 9. Report de REQ-024-18 à 28 dans les designs et exigences des specs 020 et 021
  (cadre, inventaire des composants, régression visuelle, écrans) ; SDK : brouillons
  serveur, statistiques ; section « Structure avant style » du skill `kartenn-ui` —
  verify : `spec-keeper` sur 020 et 021
  - où : `specs/020-systeme-de-design/design.md` (« Inventaire », « Architecture CSS »,
    « Régression visuelle ») ; `specs/021-shell-v2/design.md` (« Cadre », « Écrans ») ;
    `.claude/skills/kartenn-ui/SKILL.md`
  - Résultat : 021 — REQ-021-1, 6 (panneau droit, pied du brouillon), 9 (accueil,
    Topographie), 11 (brouillons du serveur), 13, 14, 15, 20, 25 révisées, REQ-021-36
    (panneau d'édition par sorte) et 37 (droits au survol) ajoutées ; design « Cadre »,
    « Routes », « Raccourcis », « Écrans », catalogue, « Édition en place », fraîcheur,
    flux, « Écritures » (`sdk.drafts`, `sdk.stats`), erreurs. 020 — REQ-020-3, 23, 26
    révisées, REQ-020-34 à 37 ajoutées ; design « Structure avant style », inventaire
    30 à 34 (`SidePanel`, `CommitForm`, `SplitButton`, `Toolbar`, `MatrixGrid`), `Drawer`
    modal seulement, maquette E0, captures à quatre largeurs. Skill : section « Structure
    avant style », inventaire, raccourcis. `spec-keeper` : couverture complète ; écarts
    corrigés (`?apply=own` sur toute écriture reporté dans REQ-024-13 comme livré par la
    tâche 3, verbes `edit` dans 021, notes d'amendement sur les tâches faites).
  - Choix tranchés (9) :
    - Raccourcis : `g o` Topographie, `]` panneau droit ; `g b` mène à `/drafts`.
    - À 768 px : barre latérale repliée, le panneau couvre la surface ; défilement
      horizontal contrôlé de 1024 à 1920 px (768 seulement capturé).
    - État du panneau (ouvert, onglet, largeur) en `localStorage` `kartenn.panel`.
    - « Appliquer » par défaut si `/v1/me` donne `apply_own` sur tous les objets du
      brouillon et que la simulation ne signale aucune politique qui retient.
    - Thème clair vérifié par la luminance relative des fonds du cadre (≥ 0,7).
    - `Drawer` reste pour un dialogue latéral modal ; le non-modal devient `SidePanel`.
    - `EditPanel` est un composite du shell (schéma de la sorte), pas un composant de
      `@kartenn/ui`.

## P3 — Maquettes révisées

- [x] 10. Composants : `SidePanel`, `CommitForm`, `SplitButton`, `Toolbar` repliable,
  `Tooltip` des actions grisées, `MatrixGrid` ; `DataGrid` en grille unique (subgrid),
  tri, filtre, squelettes animés — REQ-024-19, 20, 23, 25, 26 — verify : catalogue,
  captures aux quatre largeurs, axe
  - où : `web/v2/ui/src/components/` ; prototype de référence
    `/tmp/…/scratchpad/proto-panneaux/PanneauxProto.vue` (hors dépôt, à reprendre
    proprement) ; design « E »
  - Résultat : nouveaux `SidePanel`, `CommitForm`, `SplitButton` (`SplitAction`),
    `Toolbar` (`ToolbarAction`, `foldActions`), `Tooltip` (`useTooltip`), `MatrixGrid`
    (`MatrixAxis`, `MatrixCell`), aide `moveInGrid` (`internal/gridKeys.ts`) ; `Button`,
    `IconButton` et entrées de `Menu` prennent `forbidden` (permission) et `reason` ;
    `DataGrid` en une grille (lignes en `subgrid`, en-tête collant dans le même conteneur),
    tri (`v-model:sort`, `nextSort`), filtre (`filtered`, événement `filter`), navigation
    au clavier (`v-model:active`, `activate`), chargement (`loading`, squelettes dans les
    cellules) ; `Skeleton` animé d'un reflet (immobile sous mouvement réduit), variante
    `cell` ; `Drawer` modal seulement ; `DraftCounter` bascule et menu des brouillons
    (`DraftSummary`) ; E5 passe au `SidePanel`. Pages de catalogue et captures 1280 × 800
    par état ; `e2e/visual/clavier.spec.ts` (motifs APG au clavier) et
    `e2e/visual/layout.spec.ts` (bords gauches en-tête/cellule et absence de défilement
    horizontal à 1440, 1280, 1024 et 768 px, en-têtes collants) ; tests Vitest
    `components-024.test.ts` ; inventaire du skill à jour.
  - Choix tranchés (10) :
    - Infobulle écrite par kartenn (`useTooltip`, bulle téléportée dans la couche des
      surimpressions, placée par CSSOM) plutôt que le `Tooltip` de Reka : celui-ci n'écrit
      `aria-describedby` que bulle ouverte et l'écrase sur l'enfant ; la bulle reste dans
      le document, cachée, pour que la description tienne. Délai de 300 ms au survol,
      immédiate au focus, Échap la ferme sans fermer la couche autour, survolable (1.4.13),
      fermée au défilement ; `z-index` `--k-z-toast` pour passer au-dessus d'un dialogue.
    - Action refusée : `forbidden` (« Il faut `<permission>` », permission en chasse fixe)
      ou `reason` (phrase de l'hôte, politique) ; dans un menu, l'entrée garde le focus
      des flèches, son motif est sa description (texte caché) et une bulle décorative à
      côté d'elle ; le motif n'est plus imprimé sous le libellé.
    - `SidePanel` : positionné en absolu dans un conteneur que l'hôte positionne, la
      surface en `isolation: isolate` ; bornes 320–720 px, pas de 16 px, Début et Fin aux
      bornes, glisser au pointeur ; pas d'Entrée de repli (facultative dans l'APG) ; le
      pied (`footer`, avec l'onglet actif) porte simulation, motif et action ; ombre
      `--k-shadow-overlay` (ajouté à `FLOATING`).
    - `CommitForm` : le résumé est un `TextField`, la description un `textarea` du même
      dessin ; résumé requis vide à la soumission → erreur « Le résumé est obligatoire. »
      et focus rendu ; le raccourci est montré par le `SplitButton`, pas par le
      formulaire ; ⌘+Entrée vaut Ctrl+Entrée.
    - `SplitButton` : l'action choisie interdite rend le bouton lui-même refusé avec le
      même motif ; pas d'entrée radio (`menuitemradio`) : le bouton dit l'action choisie.
    - `Toolbar` : largeurs mesurées une fois toutes les actions dans la barre, remesurées
      quand la liste change ; repli par priorité croissante puis de la fin ; le bouton
      « … » est un `Menu` ; une action désactivée nativement sort du parcours des flèches.
    - `DataGrid` : tri sans retour à « aucun » (ascendant puis alternance) ; le filtre est
      tenu par l'hôte, la grille ne fait que le demander (Alt+Bas, clic sur l'entonnoir)
      et le montrer (`filtered`, mot caché « filtrée ») ; Entrée sur une cellule émet
      `activate`.
    - `MatrixGrid` : le coin est l'entrée de la grille ; première colonne
      `minmax(--k-size-col-md, max-content)`, colonnes `minmax(--k-size-col-sm, 1fr)` ;
      la virtualisation au-delà de 50 × 50 (design, ligne 34) reste à faire avec l'écran
      qui en a besoin.
    - Reflet du `Skeleton` : durée `2 × --k-duration-spin` (pas de nouveau jeton), image
      et animation retirées sous mouvement réduit.
    - Catalogue : un état `wide` a une piste `minmax(0, 1fr)` (une table large défilait la
      page à 768 px) ; les scripts clavier ouvrent les pages sans horloge figée (Vue ignore
      un événement daté de l'instant où le gestionnaire d'un ancêtre a été posé).
    - E5 : le `Drawer` non modal devient le `SidePanel` (onglets Détail et Brouillon, pied
      provisoire) ; la maquette complète du pied revient à la tâche 11.
- [ ] 11. Maquettes E1 à E10 reprises, maquette de l'accueil ajoutée, captures 1440,
  1280, 1024, 768 px — REQ-024-21, 22, 24, 27, 28, 29 — verify : `k-web2-check`,
  revues `ui-reviewer` et `a11y-auditor`
  - où : `web/v2/shell/src/_ui/ecrans/` ; `registry.ts` ; `e2e/visual/ecrans.spec.ts`
- [ ] 12. Validation des maquettes révisées par l'utilisateur (reprend la tâche 15 de la
  020) — REQ-024-29

## P4 — Clôture

- [ ] 13. Synchronisation `docs/spec/` (API : brouillons, statistiques, simulation ;
  permissions ; invariants), index `specs/README.md` — verify : `spec-keeper`

---
status: in-progress
updated: 2026-10-22
---

# 011 — Registre de flux : tâches

Chaque tâche est un commit, laisse le build vert et nomme ce qu'elle implémente. La 011
suit la 010 (v0.2, D31) : elle commence quand les tâches 010-1 à 010-13 sont faites ;
elle s'appuie sur N21, N22 (ADR 0026, fournis) et M7 (REQ-009-23), et ajoute N38 et N39
(design, § Exigences). Chaque tâche porte `dépend de :`.

## P1 — Domaine et stockage

- [x] 1. Crate `kartenn-flow` : type `Flow` (sorte, dossier, références externes),
  extrémités, dérogation, validation ; état dérivé et validité dérivée — REQ-011-1, 2, 4,
  5, 9, 20, 24 — verify : un test par état de REQ-011-5 ; cas refusés de REQ-011-1, 9,
  20
  - où : modèle de `kartenn-policy` (010-3) ; crate à l'image de `crates/kartenn-vlan`
  - dépend de : —
  - fait : `crates/kartenn-flow` (`model.rs` : `Flow`, `Endpoint`, `Facts`, `check`,
    `check_update`, `Waiver`, `check_waiver`, `check_review`, `add_months` ;
    `state.rs` : `summarize`, `state_at`, `holds` ; `rules.rs` : les règles du design et
    `flow.field.invalid`, `flow.update.identity`) ; 20 tests unitaires (un par état de
    REQ-011-5, refus de REQ-011-1, 2, 4, 9, 20, séparation et fin d'instruction de
    REQ-011-22).
  - choix : mois calendaires en UTC, jour borné à la fin du mois ; un flux sans paire
    d'adresses est `denied` et `noncompliant` ; `allowed` sous réserve compte comme
    autorisant dans la synthèse (l'issue porte la réserve) ; deux règles ajoutées au
    tableau du design : `flow.field.invalid` (justification, identifiant externe) et
    `flow.update.identity` (REQ-011-4).
- [x] 2. Migrations du module : `flow`, `flow_waiver`, `flow_endpoint_address`,
  `flow_evaluation`, `flow_summary`, `flow_registration`, `flow_rejection`,
  `flow_notice` versionnées — REQ-011-16, 23 — verify : tests de contraintes ; lecture
  `as_of` passé
  - où : migrations de `crates/kartenn-vlan` (forme), forme de `property_origin`
    (`crates/kartenn-store-pg/migrations/0011_properties.sql`)
  - dépend de : 1
  - fait : `crates/kartenn-flow/migrations/0001_flow.sql` (schéma `flow`), module `Flows`
    (`src/module.rs` : type `flow.flow` ancré sur l'entité, permissions de REQ-011-17,
    règles, migration) ; tests `crates/kartenn-flow/tests/it/schema.rs` (cible `flow_it`) :
    contraintes de `flow` (référence, justification, sorte, plage), de `flow_waiver` (fin,
    motif, risque), de `flow_summary` (accord synthèse et décomptes) et de
    `flow_evaluation` (réserve), lecture `as_of` passé dans `flow_summary_history`,
    `flow_registration` jamais réécrite par le rôle de l'application.
  - choix : extrémités et service sans clé étrangère (polymorphes, contrôlés par les
    règles du module) ; `flow_waiver`, `flow_endpoint_address`, `flow_evaluation`,
    `flow_summary` versionnées ; `flow_registration`, `flow_rejection`, `flow_notice`
    en ajout seul (`SELECT, INSERT`) ; `flow_notice.kind` distingue les échéances du flux
    et celles de la dérogation ; le module n'est pas encore monté par `kartenn-server`
    (opérations et gestionnaire en 011-3).

- [x] 3. Opérations `flow.flow.*`, verrou `flow`, règles du design ; triage (proposition
  forcée et permission de validation, N39, amendement additif de la 004 et du contrat) ;
  instruction (`approve_review`, séparation des tâches) ; dérogation — REQ-011-1 à 4,
  8 à 10, 17, 21, 22 — verify : cas d'acceptation de REQ-011-8, 9, 21, 22 ; demande
  par `flow.flow.request` vers un objet illisible acceptée ; écriture directe par
  `flow.flow.write` vers un objet illisible → 404
  - où : opérations de `crates/kartenn-vlan`, `crates/kartenn-service/src/review.rs`
    (propositions, REQ-004-41, 42), `rights.rs`
  - dépend de : 2 ; et, pour le triage, 010-12 (instantané de la politique lu en base) et
    010-13 (évaluation de deux extrémités : développement en adresses, axes portés,
    explication), absents au 2026-10-21
  - levé le 2026-10-21 : la permission propre à une opération
    (`Module::operation_permissions`, `flow.flow.request`, REQ-011-17, F12) et les
    acteurs du changeset (`ModuleTx::actors` : auteur, demandeur, validateur, pour
    `requester_id`, `reviewed_by`, `granted_by`, `flow.separation`) sont l'amendement 3
    de l'ADR 0026 ; reste à attendre 010-12 et 010-13
  - fait (2026-10-22) : `src/handler.rs` (opérations `request`, `register`, `update`,
    `waive`, `approve_review` ; `prepare` complète l'opération — fin lue comme instant,
    propriété `core.owner`, triage de la demande — et l'écrase à chaque préparation ;
    `review` force la proposition d'une demande non conforme ou non automatique ; `reads`
    exige la lecture des deux extrémités pour `register` ; `check` sur l'état final ;
    `ended` alimente `flow_rejection` au rejet), `src/evaluation.rs` (évaluation dans la
    transaction : extrémités développées par le port de lecture de la 009, valeurs des
    axes lues sur les origines matérialisées, axes portés par la machine, dossier,
    `kartenn_policy::evaluate` par paire, synthèse), `src/pg.rs` (insertion, portion,
    marques du changeset `pg_temp.flow_written`, couverture des périodes, évaluations,
    enregistrement figé, dérogation, rejet), `src/module.rs` (opérations, opérations de
    revue `approve_review` et `waive`, permissions `flow.flow.request`, `.waive`,
    `.review`). Tests `crates/kartenn-flow/tests/it/requests.rs` : REQ-011-21 (conforme
    appliquée et figée ; refusée → proposition), REQ-011-8 (écriture directe refusée,
    `flow.review.required` et `flow.verdict.denied`), REQ-011-22 et 9 (instruite et
    dérogée par le validateur ; le demandeur, servi par un compte de service, n'instruit
    pas sa demande ; plafonds de 12 et 6 mois), rejet → non-conformité, REQ-011-17
    (demande sans lecture des extrémités ; écriture directe sans lecture refusée,
    `core.object.forbidden`), REQ-011-1 (référence manquante), REQ-011-4 (identité).
  - choix : la demande d'un flux exige aussi `prop.core.owner.write` sur l'entité (le
    module ancre le flux par une affectation ordinaire de `core.owner`, contrôlée comme
    telle) ; `register` est le nom de l'écriture directe de maintenance (le design ne la
    nommait pas) ; le demandeur est l'identité pour le compte de laquelle le changeset est
    soumis, sinon son auteur, jamais la parole du client ; le triage n'est fait que pour
    des extrémités, un service et une entité donnés par identifiant (sinon la demande est
    instruite) ; l'évaluation d'un flux créé couvre toute sa période en un segment (la
    segmentation aux bornes de la politique et des rattachements vient avec la
    réévaluation, tâche 5) ; un flux sans paire d'adresses compte comme refusé ;
    l'évaluation dans la transaction recharge la politique sans le cache des requêtes
    (une simulation lève une révision qu'elle annule) ; `update` ne fait que raccourcir la
    période (`end`), la prolongation reste à faire ; les conditions non remplies sont
    reliées aux conditions par leur clé.
- [x] 4. Références des extrémités : clôture refusée ou cascade — REQ-011-3 — verify :
  clôture d'une adresse citée refusée ; cascade clôt le flux
  - où : références de module (ADR 0026 clause 12, tâche 008-7)
  - dépend de : 3
  - fait avant la 3, sur des flux écrits comme le module les écrira (ligne de `flow.flow`
    et enregistrement dans `module_object`) : `src/module.rs` (références `flow.flow` →
    `address`, `prefix`, `machines.interface`, `machines.machine`, en cascade ; dépend de
    `policy` et `machines` ; gestionnaire), `src/handler.rs` (`referrers`, `close` qui clôt
    aussi dérogation, extrémités développées et évaluations, `restore`), `src/pg.rs` ;
    module monté par `kartenn-server` (feature `flow`, par défaut, qui active `policy`) ;
    contrat de référence et types du SDK régénérés. Tests
    `crates/kartenn-flow/tests/it/references.rs` :
    `closing_a_cited_address_is_refused_with_its_flows`,
    `the_cascade_closes_the_flow_with_its_endpoint`,
    `an_ended_flow_does_not_hold_its_endpoint`.
  - choix : le refus passe par la règle du noyau `core.object.dependents` (mécanisme
    générique, 008-7), qui liste les flux, et non par `flow.referenced` ; un flux vit
    s'il recouvre la période à partir du début de la clôture (actif ou à venir) ; la
    restauration ne rétablit que la ligne du flux, pas ses tables dérivées (recalculées
    par la réévaluation, 011-5).
- [x] 5. Réévaluation dans la transaction (flux touchés par les sections 003, 009, 010,
  recalcul segmenté avec dossier, écriture des seuls segments changés), évaluation figée
  à l'enregistrement, différentiel des flux — REQ-011-11, 12, 13, 23 — verify : cas
  d'acceptation de REQ-011-11 et 13 ; oracle contre recalcul complet sur suites
  aléatoires ; changement de règle d'inférence sans réévaluation
  - où : `Module::diff` (tâche 008-19), `kartenn-policy/src/evaluate.rs`
  - dépend de : 3
  - fait (2026-10-22) : `src/reevaluation.rs` (flux touchés : écrits par le changeset,
    extrémité, service ou adresse développée parmi les objets écrits, du différentiel des
    propriétés ou de `machines.attachments`, adresse ou extrémité dont l'origine d'une
    valeur est un tel objet, machine portant une adresse d'extrémité ; tous les flux
    vivants quand le changeset écrit la politique hors règles d'inférence ; recalcul de
    maintenant à la fin du flux, coupé aux bornes des versions de la politique, des
    rattachements, des origines et valeurs lues et d'un préfixe extrémité ; comparaison
    à l'état stocké, segments contigus égaux fusionnés, réécriture seulement s'il
    diffère ; section `flow.flows` : décompte `breaks`, `heals`, `outcome` puis un rang
    par flux existant dont la synthèse, un verdict ou l'issue bascule, avant, après et
    élément décisif ; `drift`, l'oracle), `src/pg.rs` (`state_of`, `read_states`,
    `write_states` remplacent `write_evaluation`), `src/handler.rs` (`diff` ; `check`
    n'écrit plus l'évaluation). Tests `crates/kartenn-flow/tests/it/reevaluation.rs` :
    REQ-011-11 (conteneur `{dev}` inséré au-dessus de la source : flux refusé dans le
    changeset accepté, enregistrement figé), REQ-011-13 (retrait de la règle : la
    simulation annonce 3 flux `allowed → denied` sans rien écrire, puis l'application),
    écriture de la politique sans changement de verdict : aucune réécriture ni section,
    oracle sur trois suites aléatoires (classements présents et à venir, règles créées et
    closes, demandes).
  - choix : l'écriture d'une politique réévalue tous les flux vivants (filtre par service
    et valeurs d'axes reporté au banc, tâche 10) ; la règle d'inférence n'a pas encore
    d'opération : son exclusion est celle de tout type de la politique dont le nom porte
    `inference`, et le test vérifie l'absence de réécriture sur une écriture de politique
    neutre ; la réécriture porte sur tout le flux à partir de maintenant quand un segment
    change ; la révision de politique ne compte pas dans la comparaison ; un flux créé
    par le changeset n'a pas de rang dans la section (il a son triage) ; `breaks` compte
    les flux qui passent à `denied` (synthèse absente comprise), `heals` l'inverse ;
    héritiers d'une valeur à venir absents du différentiel des propriétés (fenêtre
    future) : rattrapés par l'index des origines ; anomalies et déclencheurs en tâche 6.
- [x] 6. Déclencheurs `flow.created`, `flow.breaks` (politique par défaut à
  l'installation), `flow.waiver` ; anomalies `flow.verdict.denied` ouvertes et fermées ;
  crochet de rejet (N38) et `flow_rejection` — REQ-011-6, 12, 25 — verify : politique
  `flow.breaks ≥ 1` installée et active ; rejet motivé enregistré
  - où : déclencheurs de module (tâche 010-9), anomalies (ADR 0026 clause 7)
  - dépend de : 5
  - fait : `src/approval.rs` (mesures `created`, `breaks`, `waiver` ; politique par défaut
    `flow.breaks` = `breaks ≥ 1` ; anomalies des flux qui basculent, lues dans la section
    `flow.flows`), `Handler::measure`, `Handler::check`, `Module::{measures,
    default_policies}`, `pg::written_counts` ; crochet de rejet déjà en 011-3 ; tests
    `tests/it/approval.rs` (politique installée, active, changeset cassant en attente
    d'approbation ; anomalie ouverte, laissée par un changeset sans rapport, fermée à la
    guérison), `requests.rs::a_rejected_request_is_a_nonconformity`.
  - choix : `flow.verdict.denied` devient l'avertissement (anomalie) ; le refus à
    l'enregistrement sans dérogation prend la règle d'erreur `flow.waiver.required` (une
    règle n'a qu'une sévérité) ; seuls les flux dont l'évaluation bascule sont réévalués
    sous cette règle (pas de parcours des anomalies ouvertes) ; la dérogation est lue à
    l'instant du changeset.

## P3 — Lectures, échéances, événements, API

- [x] 7. Filtres, flux d'un objet, évaluations datées, non-conformités — REQ-011-14, 15,
  16, 25 — verify : un test par filtre ; sous-arbre d'un préfixe ; un demandeur voit ses
  non-conformités et pas celles d'un autre
  - où : `crates/kartenn-store-pg/src/filter.rs`
  - dépend de : 5, 6
  - fait : `src/read.rs` — `flows` (tous les filtres de REQ-011-14, état dérivé par
    `state::state_at`, datée `valid_at`/`as_of` sur les tables courantes et leur
    historique), `of_object` (adresse, préfixe et sous-arbre, interface, machine par ses
    interfaces et rattachements), `evaluations` (relues, jamais recalculées),
    `nonconformities` (rejets, flux `noncompliant` et `waived`, règle, libellé et textes à
    la date, filtres de REQ-011-25) ; tests `tests/it/reads.rs` (filtres par extrémité,
    service, type, sorte, entité, demandeur, référence, état, synthèse, issue, règle
    décisive, dérogation, échéance, sous-arbre, axe, machine ; évaluations à un `as_of`
    passé ; non-conformités d'un demandeur et pas d'un autre).
  - choix : lectures sans droits (le prédicat RBAC sur `entity_id` revient aux routes,
    tâche 9) sauf le filtre `requester` des non-conformités ; les jointures du noyau
    (adresses, préfixes, origines des valeurs, machines) sont lues à l'assertion courante,
    seules les tables du module suivent `as_of` ; filtre d'état en mémoire après la
    requête (la page est coupée après lui) ; `closed_early` = une version antérieure du
    flux finissait plus tard ; les demandes en attente (`requested`) ne sont pas listées
    ici (elles ne sont pas dans `flow.flow`).
- [x] 8. Tâche `flow.expiry` (flux datés, dérogations) et événements `expiring`,
  `expired`, `outcome_changed`, `closed` — REQ-011-7 — verify : J-30, J-7, terme ;
  prolongation sans doublon ; flux sans fin jamais notifié ; événement d'issue émis à la
  bascule
  - où : tâches de fond et événements de module (tâches 010-8, 010-14)
  - dépend de : 5
  - fait : `src/expiry.rs` — tâche `flow.expiry` quotidienne (`Module::jobs`), avis
    inscrits dans `flow_notice` (un par flux, sorte et fin : sans doublon, une nouvelle fin
    est annoncée à nouveau), événements `org.kartenn.flow.expiring` (`days` 30 ou 7, la
    fenêtre la plus étroite) et `expired`, pour le flux (`of: flow`) et sa dérogation
    (`of: waiver`) ; l'échéance d'une dérogation d'un flux encore refusé ouvre l'anomalie
    `flow.verdict.denied` (constats de la tâche, ADR 0026 amendement 4, REQ-011-9) ;
    `outcome_changed` émis par `check` pour chaque ligne de `flow.flows` dont la synthèse
    ou l'issue bascule ; `closed` émis par `Handler::close` ; tests
    `tests/it/expiry.rs` (J-30, J-7 seul, pas de doublon à la relance, flux sans fin
    jamais annoncé, terme une fois ; `outcome_changed` et `closed`).
  - choix : fenêtres 30 et 7 jours en constantes (le réglage `[flow]
    expiry_notice_days` demande une configuration de module, absente du contrat) ; un
    flux clos avant sa fin n'est pas annoncé `expired` (il a eu `closed`) ; au premier
    passage, une fin déjà passée est annoncée une fois.
  - constat hors périmètre : une demande datée (`end`) soumise dans un changeset à période
    ouverte est refusée (`changeset.operation.no_version`) : l'ancre `core.owner` et les
    valeurs par défaut (`expected::defaults`) sont posées sur la période ouverte du
    changeset, au-delà de la fin du flux ; les tests soumettent les flux datés dans un
    changeset dont la période finit avec eux.
- [x] 9. Routes `/v1/flows`, `/v1/flows/{id}/evaluations`, `/v1/flow-nonconformities`,
  `/v1/{collection}/{id}/flows`, contrat OpenAPI — REQ-011-18 — verify : contrat
  comparé ; tests de routes ; `POST /v1/flows` rend l'évaluation et l'état
  - où : routeur de module (tâche 008-8), `crates/kartenn-rest/src/contract.rs`
  - dépend de : 7
  — `crates/kartenn-flow/src/rest.rs` (feature `rest`, montée par kartenn-server) :
  `GET`/`QUERY /v1/flows` (filtres de REQ-011-14), `POST /v1/flows` (opération `request`,
  201 avec `flow` et `evaluations`), `GET`/`PATCH`/`DELETE /v1/flows/{id}`,
  `GET /v1/flows/{id}/evaluations`, `POST /v1/flows/{id}/waiver`,
  `GET /v1/flow-nonconformities`, `GET /v1/{addresses,prefixes,interfaces,machines}/{id}/flows` ;
  contrat de référence et types du SDK régénérés. Tests : `tests/it/rest.rs` (demande
  rendue avec son état et ses évaluations, lecture réduite du demandeur, 404 d'un
  inconnu, liste et filtres d'un lecteur, flux d'une adresse, non-conformités).
  choix : un flux se lit sous `flow.flow.read` par le prédicat du noyau, ou par son
  demandeur (évaluations réduites : adresses et élément décisif masqués) ; une page est
  coupée avant le filtre des droits (page parfois plus courte, jamais de fuite) ; les
  non-conformités se lisent par entité (`flow.flow.read` globale ou sur l'entité) ou par
  le demandeur, sans curseur (une page) ; un `POST` retenu pour instruction (202) rend le
  changeset seul ; la liste des flux d'un objet ne vérifie pas la lecture de l'objet
  (elle ne rend que des flux lisibles) ; route `waiver` ajoutée (F6), additive ; pas de
  route d'écriture directe `register` (maintenance par `POST /v1/changesets`).

## P4 — Budgets et synchronisation

- [x] 10. Banc `bench-flow` (10⁵ flux) bloquant en CI — REQ-011-19 — verify : budgets
  tenus
  - où : `crates/kartenn-bench`
  - dépend de : 9
  - fait (2026-10-22) : banc `bench-flow` (`flow_budgets.rs`, politique au gabarit du
    banc 010 sans motifs, conditions ni signaux) ; réévaluation ensembliste (lots de 512
    flux, une requête par étape et par lot, flux d'un même instant évalués ensemble,
    seules les tables dérivées qui changent réécrites), index B-tree sur `flow_id`
    (migration 0002), politique d'une révision validée gardée par l'instance, candidats
    d'un service indexés dans l'instantané (010), adresses d'une machine lues par
    identifiant (009), non-conformités lues depuis les synthèses refusées.
    Mesure au banc complet (`K_BENCH_SCALE=full`, seed 42, 10⁵ flux, pic mémoire du
    banc 1,6 Go), avant → après (budget) : demande 16 × 16 1,1 s → 297 ms (300 ms, marge mince : ~210 ms sont le différentiel
    des propriétés du noyau pour l'ancre `core.owner`) ; reclassement de 10⁴ flux
    17,8 s → 2,4 s (3 s) ; changement de politique sur 10⁵ flux 96,5 s → 12,8 s (60 s) ;
    règle d'un type de flux (5·10³ flux) 7,5 s → 2,0 s (60 s) ; page filtrée 140 →
    130 ms (150 ms) ; non-conformités 17,6 s → 26 ms (150 ms). Non mesuré : la
    réévaluation de fond (5 min), faute de tâche de fond qui la porte.
- [x] 11. Synchronisation de `docs/spec/`, ADR proposée → acceptée, amendement de l'ADR
  0026 (N38, N39)
  - où : skill `tech-specs`, mode sync
  - dépend de : 10
  - fait (2026-10-22) pour les tâches 1 à 10, 12 et 13 : `docs/spec/` (data : schéma
    `flow`, migrations 0001 et 0002 ; interfaces : routes des flux, F19, F20 ; invariants
    INV-64 à 66 ; permissions et règles générées),
    [ADR 0031](../../docs/adr/0031-evaluations-de-flux-materialisees.md) acceptée, avec
    la réévaluation ensembliste, le cache de politique partagé et les budgets du banc
    complet ; amendements N38 et N39 de l'ADR 0026 et de la 004 déjà faits par la 016 ;
    écarts au design reportés (design, § « Écarts au design », ceux résorbés par F19 et
    F20 signalés comme tels) ; budgets de `bench-flow` dans `docs/spec/invariants.md`
    § Performance ; audit spec-keeper. Statuts laissés `in-progress` : la réévaluation
    complète de fond de REQ-011-19 (< 5 min) n'est portée par aucune tâche de fond, donc
    non mesurée (à arbitrer : tâche à créer ou exigence à amender) ; REQ-011-4 (« changer
    une extrémité crée un nouveau flux ») diverge du code qui refuse (`flow.update.identity`,
    écart du design), à arbitrer.

## P5 — Amendements (spec 017)

- [x] 12. Demandes en attente servies par la lecture (F19) : `read::flows` ajoute, quand
  `state` cite `requested` ou avec les filtres `review=pending` et `changeset=<id>`, les
  propositions en attente à `as_of` qui portent `flow.request` (et, après la tâche 13,
  les prolongations `flow.update`) ; ligne `id: null` (demande) avec le membre `review`
  (`changeset`, `kind`, `triage` lu dans l'opération complétée, sans recalcul ; triage
  étendu aux évaluations par paire s'il ne les garde pas) ; filtres applicables et
  exclusions du design ; curseur sur le changeset ; droits `flow.flow.read`,
  `flow.flow.review` sur l'entité ou demandeur, triage réduit ; contrat OpenAPI (schéma
  `Flow` : `id` nullable, membre `review` ; paramètres `review`, `changeset`) et SDK
  régénérés — REQ-011-5, 14, 17 — verify : demande sous réserve → 202, puis
  `GET /v1/flows?state=requested` la rend avec `id` nul, son changeset et un triage égal
  à celui de la réponse ; absente sans `requested` ; réduite pour le demandeur sans
  lecture des extrémités, visible d'un instructeur, invisible sans droit sur l'entité ;
  absente à un `as_of` antérieur et après rejet ; `changeset=<id>` rend la seule ligne ;
  filtre par sous-arbre de préfixe : exclue ; `docs/spec/interfaces.md` à jour
  - où : design « État dérivé » (paragraphe « Lecture des demandes en attente ») et
    « API » ; `crates/kartenn-flow/src/read.rs` (`flows`, `Creation::Applied` à
    compléter par `Creation::Pending`, `state_name`) ; `src/state.rs` (`state_at`,
    `Creation::Pending`) ; `src/handler.rs` (`complete` : forme de `completed.triage`) ;
    `src/rest.rs` (`list`, analyse des filtres, schéma de la ligne, déclaration du
    contrat) ; lecture des propositions de la 004 (`GET /v1/proposals`,
    `crates/kartenn-service`) ; tests `crates/kartenn-flow/tests/it/reads.rs` et
    `tests/it/rest.rs` (lecture réduite du demandeur à imiter) ;
    `web/sdk/src/api/schema.d.ts`
  - dépend de : 9
  - fait (2026-10-22) : `src/read.rs` — `FlowFilter` porte `states` (liste),
    `review_pending`, `changeset` ; `flows` lit les flux appliqués (`applied`) puis, sur
    demande (`wants_pending`), les lignes en attente (`pending`) : propositions en attente
    à `as_of` dont une opération est `flow.request` ou `flow.update` triée, chacune rendue
    par `pending_view` (`id` nul, champs à la forme d'un flux stocké, période complétée)
    ou `extension_view` (`id` du flux, champs de `completed.current`, période = portion
    ajoutée), état `requested` par `state_at` (`Creation::Pending`), `Review` (changeset,
    rang, `kind`, triage tel que gardé, entité, demandeur) ; filtres par `FlowFilter::passes`
    ; curseur `Cursor::Flow | Pending { changeset, ordinal }`. `src/pg.rs` —
    `triage_of` : le triage complété porte conditions non remplies, score et
    avertissements de la paire décisive et les évaluations par paire (forme de
    `FlowEvaluation`), pour la demande (`handler::triage`) et la prolongation
    (`complete`). `src/rest.rs` — `Flow.id` nullable (toujours présent), membre `review`
    (`FlowReview`, `FlowTriage`) omis hors attente, filtres `state` (liste), `review`,
    `changeset`, droits par `reviews_seen` (lecture ou instruction sur l'entité : complet ;
    demandeur : réduit), changesets lus journalisés ; contrat et SDK régénérés ;
    `docs/spec/interfaces.md`. Tests : `tests/it/rest.rs`
    (`a_pending_request_is_read_with_its_triage` : 202 puis ligne `id` nul, triage égal à
    celui de l'opération, réduit pour le demandeur, complet pour un lecteur, vu d'un
    instructeur, invisible d'un lecteur d'une autre entité, absent sans `requested`, à un
    `as_of` antérieur, avec un préfixe, après rejet, présent à un `as_of` entre soumission
    et rejet ; `changeset=<id>`, `review=pending`, `state=active,requested` et filtres lus
    dans la demande ; `pending_requests_are_paged_after_the_applied_flows` ;
    `a_patch_shortens_or_holds_an_extension` : ligne `extension`).
  - choix : « triage égal à celui de la réponse » vérifié contre le triage de l'opération
    complétée (la réponse 202 ne porte que le changeset, `interfaces.md`) ; propositions
    de sorte `proposal` seulement (une approbation n'est pas une instruction) ; fin d'une
    proposition à une date passée : `applied_at`, sinon l'événement
    `changeset.state.{rejected,withdrawn,expired,stale}` du journal (012), à défaut la
    soumission ; candidates bornées par `expires_at > as_of` ; `changeset=<id>` ne rend que
    les lignes en attente ; « réduit » = vu comme demandeur seulement (même règle que
    `GET /v1/flows/{id}/evaluations`), l'instructeur voit le triage complet ; ligne
    d'une demande aux champs d'un flux stocké (`source_kind`, `service_id`…), extrémité
    nommée par nom local → `null` ; `decisive` filtre sur les évaluations par paire du
    triage ; conditions, score et avertissements du triage pris sur la paire décisive ;
    curseur `(changeset, rang)` (plusieurs demandes par proposition) ; prolongation à
    l'état `requested`, période = portion ajoutée.
- [x] 13. Prolongation d'un flux (F20) : `update` distingue raccourcissement et
  prolongation (fin plus tardive ou retirée d'un flux daté) ; `complete` trie la portion
  ajoutée ; `review` rend `REVIEW` pour une prolongation non automatique ; application
  qui étend la version, l'ancre `core.owner` et les adresses développées, et réévalue la
  portion ajoutée ; `check_review` (plafond depuis la décision), `flow.waiver.required`,
  `flow.separation` contre le demandeur et l'auteur de la prolongation, nouvelle règle
  `flow.update.ended` ; mesure `created` comptant les prolongations ; `PATCH` → 200 ou
  202 `ChangesetPending` ; rejet consigné par `ended` (N38) ; contrat OpenAPI
  (`FlowPatch.end` : plus tôt ou plus tard ; réponse 202) et SDK régénérés — REQ-011-4,
  6, 22 — verify : un test par cas d'acceptation de REQ-011-4 (raccourcir 200 ;
  prolongation sous réserve 202 puis approbation à 12 mois appliquée et à 13 mois
  refusée ; prolongation refusée sans dérogation `flow.waiver.required`, avec dérogation
  appliquée ; auteur qui approuve `flow.separation` ; flux expiré `flow.update.ended`) ;
  prolongation automatique appliquée en 200 ; politique `flow.created ≥ 1` qui retient
  une prolongation (REQ-011-6) ; `flow.expiry` annonce la nouvelle fin ; ligne
  `review.kind = extension` servie par la tâche 12 ; `docs/spec/interfaces.md` et règles
  générées à jour
  - où : design « Opérations et règles » (paragraphe « Prolongation ») ;
    `crates/kartenn-flow/src/handler.rs` (`complete` branche `update`, `plan_update`,
    `review`, `apply_plan` branche `update`, `check_flow` autour de `check_review`) ;
    `src/model.rs` (`check_review`, `REVIEW_MAX_MONTHS`, `WAIVER_MAX_MONTHS`) ;
    `src/rules.rs` ; `src/approval.rs` et `src/pg.rs` (`written_counts`, `patch_flow`) ;
    `src/rest.rs` (`FlowPatch`, réponses du `PATCH`) ; tests
    `crates/kartenn-flow/tests/it/requests.rs` (triage et instruction à imiter),
    `tests/it/approval.rs`, `tests/it/expiry.rs`, `tests/it/rest.rs` ;
    `web/sdk/src/api/schema.d.ts`
  - dépend de : 9 ; 12 pour la ligne `extension` de la lecture
  - fait (2026-10-22) : `src/handler.rs` — `complete` (branche `update`) range dans
    `completed.current` l'entité, la fin courante et `ended`, note `end_removed`
    (`end: null`) et, pour une prolongation, le triage de `[fin courante, nouvelle fin)`
    à la fin courante (`completed.triage`, même forme qu'une demande) ; `plan_update`
    refuse `flow.update.ended`, ajoute l'ancre `core.owner` bornée à la portion ajoutée
    (`extension_anchor`) et porte `extend` (fin courante) dans le plan ; `review` rend
    `REVIEW` pour une prolongation non automatique ; `extend` étend la dernière version
    (`pg::extend_flow`, l'ancienne gardée dans l'historique) et enregistre l'objet du
    module sur la portion ajoutée, la réévaluation du différentiel écrivant évaluations
    et adresses développées jusqu'à la nouvelle fin ; `check_flow` évalue la portion
    ajoutée (validation : `flow.review.required` sans `approve_review`,
    `flow.waiver.required` sans dérogation qui la couvre) et oppose `flow.separation` à
    l'auteur de la prolongation ou à son `on_behalf_of` ; `ended` consigne le rejet d'une
    prolongation dans `flow_rejection` ; `src/pg.rs` (`mark_at`, `extended_from`,
    `extend_flow`, `written_counts` avec `extend`) ; `src/rules.rs`
    (`flow.update.ended`) ; `src/rest.rs` (`PATCH` par `modify_nullable`, `end`
    effaçable ; contrat et SDK régénérés) ; `docs/spec/interfaces.md`,
    `docs/spec/regles-modules.md`. Tests : `tests/it/extensions.rs` (raccourcir ;
    prolongation automatique, ancre, objet et évaluation jusqu'à la nouvelle fin, fin
    retirée ; sous réserve : proposition, 13 mois refusée, 12 mois appliquée ; refusée
    sans dérogation puis appliquée avec ; séparation par `on_behalf_of` ; flux expiré ;
    politique `flow.created ≥ 1` ; rejet consigné), `tests/it/expiry.rs` (nouvelle fin
    annoncée), `tests/it/rest.rs` (`PATCH` 200 puis 202).
    Ligne `review.kind = extension` de la lecture : servie par la tâche 12 après sa
    fusion (test `a_patch_shortens_or_holds_an_extension`, `GET /v1/flows?review=pending`
    rend la prolongation avec l'`id` du flux et son triage).
  - choix : la prolongation garde le droit de l'`update` (`flow.flow.write`, plus
    `prop.core.owner.write` pour l'ancre) ; `approve_review` inscrit l'instructeur de
    maintenant à la fin du flux (un flux daté n'était pas couvert jusqu'à l'infini) et ne
    raccourcit que si la fin approuvée précède la fin courante (sinon le plafond seul est
    contrôlé) ; un flux expiré se refuse dès la planification (`now()` lu à la
    préparation) ; la portion ajoutée est évaluée à `max(fin courante, maintenant)` ;
    tests réunis dans un module `extensions`.

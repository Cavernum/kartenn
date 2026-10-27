---
status: approved
updated: 2026-10-24
---

# Invariants

Chaque invariant indique comment il est garanti et testé. « App » = validation dans la
couche service ou le domaine ; « Base » = contrainte PostgreSQL (dernier rempart sous
concurrence).

## Cohérence de l'adressage (002)

| ID | Invariant | Garantie | Test |
|---|---|---|---|
| INV-1 | Aucune donnée incohérente n'est acceptée : toute règle d'erreur violée refuse tout le changeset | App | Tests par règle (identifiant stable) |
| INV-2 | Un préfixe n'existe qu'une fois par domaine sur une période donnée | App + Base (`EXCLUDE … inet_ops`) | Tests concurrents (deux changesets simultanés) |
| INV-3 | Un réseau ne contient aucun autre préfixe | App (+ Base si faisable) | REQ-002-16 |
| INV-4 | Toute plage et toute adresse appartiennent à exactement un réseau de leur domaine | App | REQ-002-21, REQ-002-25 |
| INV-5 | Les plages d'un réseau sont disjointes | App + Base (exclusion sur intervalle d'adresses) | REQ-002-22 |
| INV-6 | L'espace public est unique tous domaines confondus (hors anycast) | App (verrou « espace public », ADR 0015) + Base (exclusion `address_public_unique`) | REQ-002-32, REQ-002-46 |
| INV-7 | Adresses et préfixes sont stockés et comparés par valeur, restitués sous forme canonique | App (familles d'adresses) | Tests de propriétés analyse ↔ canonique |
| INV-8 | Le noyau ne porte aucun attribut d'interprétation | Revue (table de traçabilité) | REQ-002-48 |

## Temps (D10)

| ID | Invariant | Garantie | Test |
|---|---|---|---|
| INV-10 | Toute période est `[début, fin)`, non vide | App + Base | Tests de bornes |
| INV-11 | Les versions historiques côté système sont immuables et contiguës par objet | App + droits PostgreSQL (pas d'`UPDATE`/`DELETE` sur l'historique) | Tests de propriétés sur suites de changesets |
| INV-12 | Le temps système d'un changeset est unique : celui de sa transaction | App (horodatage de la base) | Test multi-instances |

## Sécurité (D13, D36, 003)

| ID | Invariant | Garantie | Test |
|---|---|---|---|
| INV-20 | Aucun secret Basic n'est accepté hors TLS ; aucun secret n'est stocké en clair (comptes de service : HMAC-SHA-256 sous le trousseau ; secours : Argon2id) | App (couche de transport : TLS direct ou `Forwarded` d'un proxy de confiance) | `auth/tests/service_accounts.rs` (secret absent de la base et de l'audit) ; `rest/tests/authn.rs` (`basic_and_cookies_need_tls_after_the_trusted_proxies`), `sessions.rs` (`no_cookie_is_set_without_tls`) |
| INV-21 | Un jeton OIDC n'est accepté que d'un émetteur configuré, avec `iss`, `aud`, signature par une clé de l'émetteur et un algorithme de sa liste fermée (clé publique seulement, jamais `none`), expiration valide | App + Config | `auth/tests/access_tokens.rs`, `oidc.rs` (RFC 8725, RFC 9068) |
| INV-22 | Toute écriture exige le droit sur les objets visés **et** sur chaque changement de valeur résolue d'une propriété (effets indirects compris) | App | REQ-003-17, REQ-003-18 |
| INV-23 | Un plugin UI n'obtient jamais plus de droits que l'utilisateur | Architecture (accès par l'API seulement, avec la session de l'utilisateur) + CSP (`connect-src 'self'`, pas de cadre tiers) + SRI de chaque module | `web/e2e/journeys/plugins.spec.ts` (`may not reach another origin`, module altéré écarté avant d'exécuter son code, action absente sans sa permission) ; toute violation CSP fait échouer un parcours (`web/e2e/fixtures/index.ts`) ; `rest/tests/web.rs` (CSP) |
| INV-24 | Les décisions de flux (v0.2) se fondent sur la valeur résolue, jamais sur la valeur attendue | App (verdict lu sur la lecture groupée des valeurs résolues ; l'inférence ne produit que des anomalies, ADR 0030) | REQ-003-24 ; `policy/tests/it/verdict.rs` (`acceptance_of_req_010_13`) |
| INV-25 | Refus par défaut : sans liaison qui l'accorde, aucune lecture ni écriture ; un objet non lisible est répondu absent (même 404 que l'objet absent) ; une liste ne contient que ce que la décision unitaire autorise | App + prédicat SQL | `service/tests/rbac.rs` (`lists_hold_what_the_decision_allows`) ; `rest/tests/objects.rs` (`an_invisible_object_answers_as_an_absent_one`), `errors.rs` (`line_404_an_absent_or_invisible_object`) |
| INV-26 | Un émetteur confiné n'accorde rien hors des sous-arbres des entités qui lui sont rattachées, quelles que soient les liaisons | Prédicat SQL | `an_issuer_is_confined_to_its_entities` |
| INV-27 | Aucun administrateur délégué n'accorde un droit qu'il ne détient pas sur le périmètre, ni par une liaison ni par l'appartenance à un groupe (025, G5) | App | `a_delegate_cannot_escalate`, `a_delegate_cannot_escalate_through_a_group` |
| INV-28 | Un compte de secours exige phrase de passe **et** code TOTP, un code n'est jamais accepté deux fois (même par deux requêtes simultanées), et aucune opération de l'API ne le crée ni ne le modifie | App + Base (mise à jour conditionnelle, droits du rôle applicatif) | `auth/tests/breakglass.rs`, `rbac_schema.rs` |
| INV-60 | Écrire pour le compte d'un demandeur (`Kartenn-On-Behalf-Of`) n'ouvre aucun droit : seul un compte de service détenteur de `core.identity.act_on_behalf` le peut, sous ses propres droits ; le demandeur n'en prête aucun et figure dans l'audit de la requête, revue comprise (validation, rejet, retrait) ; une tentative refusée est journalisée (`authz.access.denied`). Un demandeur n'est pas un compte et ne réserve rien : aucune liaison ne le désigne (`core.binding.requester`), et la personne qui se connecte ensuite par l'émetteur et le sujet qu'il porte reprend la fiche, promue `person` sous le nom donné par l'émetteur (016/G10) | App | `service/tests/it/on_behalf.rs` (`the_requester_lends_no_right`, `without_the_permission_or_as_a_person_it_is_refused`, `the_requester_who_logs_in_becomes_a_person_under_the_issuer_name`) ; `service/tests/it/module_review.rs` (`a_validation_for_a_requester_names_it_in_its_events`) ; `rest/tests/it/on_behalf.rs` (`the_header_is_refused_where_it_does_not_apply`, `a_repeated_header_is_refused`) ; de bout en bout, authentification réelle (Basic, jetons de l'émetteur) devant l'API : `rest/tests/it/on_behalf_e2e.rs` (`a_service_account_writes_for_its_requester_through_the_real_authentication`, `a_service_account_without_the_permission_is_refused_and_audited`, `a_person_with_the_header_is_refused_and_audited`, `a_malformed_or_repeated_header_is_a_bad_request`, `the_real_person_logging_in_takes_back_the_requester_record`) |
| INV-61 | Une opération dont le module exige une revue n'est jamais appliquée sans validation par un autre que son auteur, même un auteur qui détient `approval.self` ; le valideur n'y joint que des opérations de revue, sur une proposition et sous ses droits requis ; une opération jointe hors revue est refusée et journalisée (`authz.access.denied`, raison `core.review.operation`) (016/G12) | App | `service/tests/it/module_review.rs` (`a_reviewed_operation_makes_a_proposal_despite_write`, `validation_needs_the_review_permission`, `review_operations_apply_with_the_proposal`, `without_the_review_permission_joined_operations_are_refused_too`, `review_operations_join_a_proposal_only`) ; `rest/tests/it/cycle.rs` (`a_validation_body_joins_operations_of_review_only`) |
| INV-62 | Un verdict de flux refuse par défaut : sans règle `allow` qui s'applique, extrémité non classifiée ou port sans service → refus ; une règle `deny` qui s'applique l'emporte toujours ; l'ordre des règles n'a aucun effet (010/S8, D28) | App (`kartenn-policy::evaluate`) | `policy/src/evaluate.rs` (`a_deny_wins`, `monotonic`, `the_order_of_the_rules_has_no_effect`, `a_port_stands_for_each_service_that_covers_it`) |
| INV-63 | Une requête de conformité ne révèle rien qu'on ne peut lire : extrémité invisible → même 404 qu'un objet absent ; sous `policy.flow.request` seul, l'explication ne nomme aucun objet du référentiel (010/REQ-010-25) | App (`kartenn-policy::verdict`) | `policy/tests/it/verdict.rs` (`acceptance_of_req_010_25`) |
| INV-64 | Un flux non conforme n'est jamais enregistré ni instruit par son demandeur : une demande refusée ou non automatique devient une proposition validée par un tiers détenteur de `flow.review` ; le demandeur (l'identité pour le compte de laquelle le changeset est soumis, sinon son auteur) ne l'instruit pas et n'accorde pas sa dérogation (011/REQ-011-21, 22, F5) | App (`kartenn-flow`, revue forcée 016-11) | `flow/src/model.rs` (`the_requester_does_not_review_and_an_instructed_flow_ends`, `a_waiver_without_risk_beyond_six_months_or_by_the_requester_is_refused`) ; `flow/tests/it/requests.rs` (`acceptance_of_req_011_21_a_denied_request_waits_for_review`, `acceptance_of_req_011_22_the_requester_does_not_review`, `acceptance_of_req_011_8_a_denied_direct_write_is_refused`) |
| INV-65 | Une dérogation est toujours datée (fin obligatoire, plafond 6 mois, flux instruit 12 mois), motivée, porte un risque et un accordant distinct du demandeur ; à son échéance, le flux encore refusé redevient une anomalie `flow.verdict.denied` (011/REQ-011-9, F6) | App + Base (contraintes de `flow_waiver`) | `flow/tests/it/requests.rs` (`acceptance_of_req_011_9_ceilings_of_waiver_and_review`) ; `flow/tests/it/schema.rs` ; `flow/tests/it/expiry.rs` |
| INV-66 | L'état et la validité d'un flux sont dérivés (date, synthèse des évaluations, dérogation), jamais stockés ; les évaluations matérialisées sont égales à un recalcul complet après tout changeset (011/REQ-011-5, 11, 24, F4, ADR 0031) | App (`kartenn-flow::state`, `reevaluation`) | `flow/src/state.rs` (un test par état) ; `flow/tests/it/reevaluation.rs` (`the_reevaluation_agrees_with_a_full_recompute`, `acceptance_of_req_011_11_a_container_above_the_source`) |
| INV-29 | Une méthode non sûre authentifiée par cookie n'est servie que si `Sec-Fetch-Site` (ou, à défaut, `Origin`) la dit de même origine ; aucun secret Basic ni cookie ne circule hors TLS ; un curseur altéré ou signé d'une clé retirée est refusé et les droits sont réévalués à chaque page | App (couches CSRF, transport, curseurs signés) | `rest/tests/csrf.rs`, `authn.rs`, `cursor.rs` ; `auth/src/signed.rs` (`a_retired_key_no_longer_verifies`) |

| INV-72 | Une suggestion (`q`, `/values`) ne montre jamais un objet que l'appelant ne peut lire : elle passe par le prédicat de lecture de la liste ; le dictionnaire de libellés ne sert un espace de données qu'aux lecteurs de sa collection, les valeurs d'une propriété sensible qu'à `prop.<clé>.read` (018/REQ-018-19, 23, R24) | App + prédicat SQL | `machines/tests/it/rest.rs` (`a_suggestion_never_shows_an_unreadable_object`) ; `rest/tests/it/labels.rs` (`a_space_is_served_only_to_its_readers`) |

## Libellés (spec 018, ADR 0040)

| ID | Invariant | Garantie | Test |
|---|---|---|---|
| INV-73 | Les corps de l'API sont neutres en langue : clés et `labels` par langue, jamais un libellé résolu ; seule `GET /v1/labels` varie selon la langue, et un ETag d'objet ne dépend pas d'`Accept-Language` (R20, Q-018-4) | App | `rest/tests/it/labels.rs` (`labels_are_negotiated`) |
| INV-74 | La clé d'une valeur de liste fermée ne change jamais ; changer ses libellés est un changeset (`update_enum_value`) daté et audité (REQ-018-9) | App | `service/tests/it/labels.rs` (`a_value_key_never_changes`, `relabelling_a_value_is_a_changeset`) |
| INV-75 | Toute table de libellés non vide contient la langue par défaut de l'instance ; chaque libellé a 1 à 200 caractères sans caractère de contrôle, sous une étiquette BCP 47 (`core.label.invalid`) ; tout code compilé a un libellé FR et EN (REQ-018-9, 10) | App | `core/src/labels.rs` (`labels_are_checked`) ; `core/src/code_labels.rs` (`every_code_has_french_and_english_labels`) |
| INV-76 | La révision des libellés croît à chaque changeset qui écrit une colonne `labels`, dans sa transaction : l'ETag du dictionnaire change avant l'événement `labels`, qui n'est qu'un avis (R22, R23) | Base (ligne `label_revision`) + App | `rest/tests/it/labels.rs` (`the_labels_etag_changes_with_the_revision`) ; `rest/tests/it/events.rs` (`a_label_change_is_pushed_as_a_labels_event`) |
| INV-77 | Un ancien nom de permission (018, R3 ; `….write`, `….propose`, `….approve` des changesets retirés par la 024, REQ-024-9) est refusé dans un rôle ; les rôles personnalisés sont migrés une fois ; l'historique et l'audit gardent les noms d'alors (R6) | App | `an_old_permission_name_is_refused`, `custom_roles_take_the_new_names_once`, `custom_roles_take_the_verbs_of_spec_024_once`, `no_old_permission_name_is_left` |

## Changesets, brouillons et statistiques (spec 024)

| ID | Invariant | Garantie | Test |
|---|---|---|---|
| INV-78 | Un changeset soumis est toujours enregistré en attente : rien ne s'applique sans application explicite. Son auteur ne l'applique que s'il détient `….approval.self` sur tout ce qu'il touche (objets et effets), et jamais quand une politique d'approbation le retient ou qu'un module exige sa revue (`changeset.approval.self`) ; un autre ne l'applique ou ne le rejette que sous `….apply` ; les droits d'édition de l'auteur sont revérifiés à l'application. `?approval=self` refusé ne perd jamais le changeset : 202 en attente avec son motif (amende 004/C1, REQ-024-10 à 13) | App (pipeline, `rights::Check`, `review::validate_pending_with`) | `service/tests/it/review.rs` (`the_application_follows_the_author_and_the_policies`, `an_author_who_may_only_edit_proposes`) ; `rest/tests/it/changesets.rs` (`approval_self_applies_or_falls_back_to_pending`) ; `rest/tests/it/cycle.rs` (`a_policy_keeps_approval.self_pending`) |
| INV-79 | Un brouillon n'est lu, modifié, simulé, soumis ou supprimé que par son auteur (404 pour tout autre, existence non révélée), sauf par le détenteur de `core.draft.admin`, dont chaque action, lecture comprise, est auditée avec l'auteur d'origine ; un brouillon n'applique rien et n'entre ni dans l'historique ni dans l'audit des changesets ; il disparaît dans la transaction du changeset qu'il a créé, et reste intact en 422 (REQ-024-2, 5, 7a) | App (requêtes filtrées par auteur, `Journal::draft`) | `rest/tests/it/drafts.rs` (`the_draft_of_someone_else_is_absent`, `a_submission_replaces_the_draft_or_leaves_it`, `a_draft_submitted_pending_is_deleted_too`, `an_administrator_reads_deletes_and_takes_over_audited`) |
| INV-80 | L'événement `draft` n'atteint que les flux dont l'identité est l'auteur du brouillon ; il ne porte ni titre ni opération (REQ-024-6) | App (`Sight::is` à la remise) | `rest/tests/it/events.rs` (`a_draft_event_reaches_its_author_alone`) ; `service/src/events.rs` (`a_draft_reaches_its_author_alone`) |
| INV-81 | `GET /v1/stats` ne compte que ce que l'appelant peut lire (prédicats de lecture des listes ; changesets sous `core.changeset.read`) et ne divulgue aucun objet ; un instantané n'est servi qu'à un appelant de même jeu de droits (clé : empreinte du périmètre de lecture, et sujet pour la partie propre) (REQ-024-15) | App + prédicat SQL | `rest/tests/it/stats.rs` (`each_counter_follows_the_rights_of_the_caller`) ; `rest/tests/it/vlan/rest.rs` (`stats_count_the_vlans_the_caller_reads`) |
| INV-82 | Une écriture simulée (`?simulate=true`) n'écrit rien et n'émet aucun événement (ni changeset, ni outbox) ; une écriture qui ne soumet pas de changeset refuse la simulation (400 `simulate-unsupported`) avant son handler (REQ-024-17) | App (porte tirée de `x-kartenn.simulate`, transaction annulée) | `rest/tests/it/simulated.rs` (`every_write_simulates_or_says_it_cannot`, `a_write_simulated_answers_what_applying_would_do`) |

## Performance (D14) — p95 serveur, 10⁶ objets, 3 ans d'historique

Le banc `bench-core` (002/A14) est bloquant par son code de sortie ; il n'y a pas encore
d'intégration continue pour l'exécuter à chaque changement. Le banc `bench-http`
(REQ-006-39) rejoue les scénarios de `bench-core` à travers le routeur, sur un port
local avec une authentification réelle, et rapporte le surcoût de HTTP ; il mesure aussi
les 200 utilisateurs concurrents et les 50 écritures/s d'INV-36. Le banc `bench-rbac` (005/K15)
mesure INV-30 à 34 pour une personne à 1 000 liaisons et 50 groupes, avec et sans
contrôle d'accès ; les changesets de 10⁴ objets et les imports prennent la même décision
une fois par changeset ou par lot.

Les bancs tournent sur une instance PostgreSQL à part (port 5433, `k-pg-bench`), sous le
verrou `k-heavy --bench`, et à deux échelles ([ADR 0032](../adr/0032-instance-postgresql-des-bancs.md)) :

- `K_BENCH_SCALE=reduced`, par défaut : les volumes divisés par 10 environ (jeu de 20
  domaines, 1,1·10⁴ réseaux, 1,1·10⁵ adresses, et ce qui en dérive : flux, machines,
  VLAN, règles), quelques minutes par banc. Les budgets sont vérifiés, mais la dernière
  ligne rappelle qu'un banc réduit ne vaut pas mesure de budget ;
- `K_BENCH_SCALE=full` : le jeu de référence, seule mesure qui coche une tâche de budget.

Un agent d'implémentation n'utilise que l'échelle réduite, pour vérifier qu'un banc tourne
et repérer une régression grossière. La mesure `full` qui coche une tâche de budget est
lancée une fois, par l'orchestrateur, en fin de série.

| ID | Budget | Test |
|---|---|---|
| INV-30 | Lecture d'un objet < 50 ms | Bancs `bench-core`, `bench-rbac`, `bench-http` |
| INV-31 | Page filtrée (curseur, 100) < 150 ms, y compris filtre sur valeur résolue | Bancs `bench-core`, `bench-rbac`, `bench-http` |
| INV-32 | Sous-arbre et occupation < 200 ms | Bancs `bench-core`, `bench-rbac`, `bench-http` |
| INV-33 | Lecture « à la date X » ≤ ×2 la lecture courante | Bancs `bench-core`, `bench-rbac`, `bench-http` |
| INV-34 | Changeset unitaire validé et appliqué < 200 ms | Bancs `bench-core`, `bench-rbac`, `bench-http` |
| INV-35 | Changeset touchant jusqu'à 10⁴ objets < 2,5 s (changeset entier) ; différentiel < 10 s jusqu'à 10⁶ | Banc `bench-core` |
| INV-36 | Import de 10⁶ objets < 15 min ; 50 écritures/s soutenues ; 200 utilisateurs concurrents | Bancs `bench-core` (import), `bench-http` (charge) |
| INV-83 | `GET /v1/stats` servie de son instantané < 200 ms ; calcul d'un instantané, en arrière-plan, < 10 s (lecteur de tout et sujet à 1 000 liaisons, 50 groupes ; 024/REQ-024-16, M18) | Banc `bench-core` (`stats_budgets` : 200 lectures par le routeur, 20 calculs) ; banc complet tenu le 2026-10-24 : route p95 1,0 ms et 18,2 ms, calcul p95 1,32 s et 7,59 s |

Les VLAN (dans le noyau depuis la spec 026) et les modules livrés tiennent INV-30, 31 et
34 sur le jeu de référence augmenté, mesurés par `bench-core` après les budgets du
noyau : VLAN (008, REQ-008-25 : 2·10⁴ groupes, 10⁵ VLAN ; banc complet tenu le
2026-10-21, quand ils étaient un module) et machines (009, REQ-009-21 : 10⁵
machines, 10³ clusters, 3·10⁵ interfaces et rattachements, plus un import de 10⁴
machines ; banc complet tenu le 2026-10-22, p95 par le routeur : fiche d'une machine
5,7 ms, page filtrée de 100 machines 36,8 ms, machines d'un /24 64 ms, création 29 ms ;
import de 10⁴ machines en 61 s ; repassé le 2026-10-23 avec la page de 100 machines
d'un site, 32 ms pour 150 ms, le filtre `site` lisant désormais les affectations en
ensemble).

Politique de flux (010, REQ-010-28 et 54) : banc `bench-policy` (010-19), qui tient lieu
du prototype 010-1. **Tenu au banc complet** le 2026-10-23, avec la politique à la taille
du budget (10⁴ règles à 6 axes, 1 000 services, 50 conditions, 30 signaux, 2 000 règles
d'inférence) sur le jeu de référence (1,06·10⁶ adresses ; p95 serveur ; charge 1,5 ; pic
RSS du banc 1,6 Go) :

| Budget | Mesuré | Limite |
|---|---|---|
| Instantané de la politique, lu à froid | 137 ms | — (information) |
| Verdict d'une paire d'adresses pour un service | 11 ms | 50 ms |
| Machine × machine 16 × 16, type de flux de 10 services | 27 ms | 300 ms |
| Port brut couvert par 20 services | 19 ms | 100 ms |
| Différentiel d'une interdiction sans terme | 511 ms | 1 s |
| Rapport d'audit, dont une valeur transverse | 37 s | 10 min |
| Suggestion de l'inférence pour un objet | 6 ms | 50 ms |
| Contrôle de l'inférence, changeset de 10⁴ objets (ajouté) | ≈ 0,3 s | 1 s |
| Réévaluation complète de l'inférence | 218 s | 10 min |
| Aperçu d'une règle d'inférence dans sa simulation, 30 001 nœuds (ajouté ; S36) | 1,54 s | 4 s |

Repassé au banc complet le 2026-10-24 après l'aperçu S36 et le masquage des sections
(ADR 0026, amendement 6) : tous les budgets tiennent. L'aperçu mesurait 5,7 s avant la
correction de 010-22 (règles indexées par préfixe, lignée et valeurs déclarées lues en
ensemble) ; la réévaluation complète passe de 218 s à 121 s, le rapport d'audit à 35 s.
Le contrôle de l'inférence, mesuré par différence de deux médianes de 5 s, reste dans le
bruit (0 à 0,7 s selon la charge) : la mesure, pas le code, est à revoir.

Le contrôle de l'inférence se mesure par différence des médianes de trois changesets de
10⁴ adresses avec et sans la source (5,5 s et 5,9 s au passage retenu) : l'écart est
dans le bruit, et la part de la source, chronométrée à part, est la lecture des vues
(≈ 0,3 s). Le jeu complet a d'abord échoué sur trois budgets (2026-10-22) ; correctifs :
- machine × machine, 1,5 s : les valeurs des machines lisaient toutes les affectations
  du référentiel en plan générique, lues depuis par clé ;
- rapport d'audit, 920 s : les fenêtres par identifiant triaient à chaque pas toutes
  les lignes restantes (la clé primaire temporelle est un index GiST), elles suivent
  depuis l'index keyset de leur table ; la rubrique des valeurs non portées cherchait
  les affectations valeur par valeur, elle les lit depuis une fois ; la rubrique des
  valeurs transverses, absente du jeu jusqu'au 2026-10-23, aurait coûté ≈ 9 min (coût
  quadratique en plan générique) : ses fenêtres suivent l'index par objet, 1 191
  fenêtres en 13 s au banc complet du 2026-10-23, rapport entier 37 s ;
- contrôle de l'inférence, +1,9 s : les vues du changeset hachaient toutes les origines
  dans les plans personnalisés des premières exécutions, et les origines rafraîchies
  par les valeurs par défaut étaient recalculées par la maintenance.

Dictionnaire de libellés (018/REQ-018-20, `bench-http`) : complet d'une langue
p95 < 50 ms, revalidation 304 < 10 ms, < 1 Mo ; 2,4 ms, 2,3 ms et 12 060 octets à
l'échelle de test (2026-10-24). Suggestions par préfixe (018/REQ-018-23, `bench-core`,
après les modules) : p95 < 50 ms par route (entités, sites, domaines, préfixes,
adresses, machines, interfaces, clusters, groupes de VLAN, VLAN, valeurs d'une liste
fermée, règles, et machines lues par le lecteur d'un seul propriétaire) ; au banc réduit
(2026-10-24) de 0,02 ms (règles) à 8,5 ms (machines), 22,6 ms pour le lecteur d'un
propriétaire. Pleine échelle à passer pour les deux.

La lecture groupée N9 (016-4) est
mesurée à 16,3 ms pour 30 ms au banc `bench-properties` réduit (2026-10-21).

Registre de flux (011, REQ-011-19) : banc `bench-flow` (011-10), 10⁵ flux sur le jeu de
référence, politique au gabarit du banc 010 sans motifs, conditions ni signaux. **Tenu**
au banc complet le 2026-10-27 (p95 serveur ; extrémités interface × interface depuis
011/F21) :

| Budget | Mesuré | Limite |
|---|---|---|
| Demande interface × interface 16 × 16, triage compris | 59,5 ms | 300 ms |
| Réévaluation de 10⁴ flux dans un changeset (ajoutée) | 2,37 s | 3 s |
| Réévaluation de 10⁵ flux par un changement de politique | 11,95 s | 60 s |
| Règle d'un type de flux (5·10³ flux) | 1,23 s | 60 s |
| Page filtrée de flux | 142 ms | 150 ms |
| Page de non-conformités | 25,2 ms | 150 ms |

La demande, mesurée à 297 ms machine × machine le 2026-10-22, retrouve de la marge.
Restent minces la page filtrée de flux (95 % du budget) et la réévaluation de 10⁴ flux
(79 %). Au même passage, `bench-policy` complet tient : interface × interface 16 × 16
pour un type de flux de 10 services, 22,6 ms pour 300 ms. La réévaluation complète en tâche de fond (< 5 min)
n'est **pas mesurée** : aucune tâche de fond ne la porte encore.

## Flux d'événements (spec 022, ADR 0034)

| ID | Invariant | Garantie | Test |
|---|---|---|---|
| INV-67 | Un abonné ne reçoit jamais un objet qu'il ne peut lire au moment de la remise, ni la trace d'un événement caché (ni événement, ni curseur hors des points de reprise) | App (visibilité par lot, `kartenn_visible`) | `rest/tests/it/events.rs` (`the_objects_delivered_are_those_each_subscriber_may_read`, `a_subscriber_receives_only_what_it_may_read_and_asked_for`) |
| INV-68 | Une reprise depuis un curseur ne saute aucun événement validé après lui et visible, même après une transaction longue ; sinon `reset`. Doublons seulement à la reprise, reconnaissables (identité, révision) | Plancher porté par le curseur chiffré | `rest/tests/it/events.rs` (`resumptions_lose_nothing_and_duplicate_only_on_resumption`, `a_resumption_on_another_instance_loses_nothing`) ; `service/tests/it/events_floor.rs` |
| INV-69 | Le coût du flux en base ne dépend pas du nombre de connexions | Lecteur partagé par instance, une requête de visibilité par lot | Banc `bench-events` (requêtes du lecteur comptées) |
| INV-70 | Un abonné lent ne retarde ni le lecteur ni les autres abonnés | Tampon borné, `try_send`, `reset` `overflow` | `rest/tests/it/events.rs` (`a_slow_subscriber_delays_no_one_and_starts_over`) |
| INV-71 | Latence validation → écriture p95 < 500 ms, p99 < 1 s à 50 écritures/s et 2 000 flux ; 5 000 flux par instance, < 64 Kio par flux ; 5 000 reprises en 10 s ; surcoût du lecteur < 5 % du changeset unitaire | Banc `bench-events` | Banc `bench-events` |

**Tenu au banc complet** le 2026-10-23 (2 000 flux pour 200 identités, 50
changesets/s, 2·10⁶ remises sans perte ; 5 000 flux inactifs ; charge machine partagée) :

| Budget | Mesuré (complet) | Budget |
|---|---|---|
| Validation → événement écrit, p95 / p99 | 208 ms / 597 ms | 500 ms / 1 s |
| Événements perdus | 0 | 0 |
| Mémoire par flux inactif | 5,3 Kio | 64 Kio |
| Reprise des 5 000 flux d'une instance | 556 ms | 10 s |
| Changeset unitaire pendant la tempête, p95 (INV-34) | 13,3 ms | 200 ms |
| Visibilité d'un lot, p95 (333 portées × 1 000 objets) | 20,8 ms | 250 ms |
| Surcoût du lecteur sur le changeset unitaire, p95 | 0,72 ms | 1 ms (plancher) |

Le p99 à 597 ms est le chiffre le plus proche de sa limite : à surveiller.

Mesures à l'échelle réduite (2026-10-23 ; 200 puis 500 flux, 20 identités, 50
changesets/s) :

| Budget | Mesuré (réduit) | Budget |
|---|---|---|
| Validation → écriture, p95 / p99 | 3,7 ms / 4,5 ms | 500 ms / 1 s |
| Mémoire par flux au repos | 5,4 Kio | 64 Kio |
| Reprise de tous les flux (500) | 81 ms | 10 s |
| Changeset unitaire pendant la tempête, p95 | 8,0 ms | 200 ms (INV-34) |
| Surcoût du lecteur (`NOTIFY` compris) | 0,05 ms sur 6,8 ms | 5 % |
| Visibilité d'un lot (33 sujets × 1 000 objets) | 9,9 ms | 250 ms (choix du banc) |

## Audit et journalisation (spec 012, ADR 0019)

| ID | Invariant | Garantie | Test |
|---|---|---|---|
| INV-50 | Toute écriture, tout refus d'autorisation et toute lecture de l'API produit un événement d'audit attribué (le flux `/v1/events` : un par connexion, amendé par la 022/A4) ; s'il ne peut être écrit, l'action n'a pas lieu (échec fermé) | App + Base (même transaction) | `kartenn-service/tests/audit_events.rs`, `audit_reads.rs`, `audit_journal.rs` |
| INV-51 | Le journal d'audit est en ajout seul pour le rôle applicatif | Base (droits) | `kartenn-store-pg/tests/audit_schema.rs` |
| INV-52 | Toute altération, suppression ou insertion dans un lot scellé est détectée par `audit verify` | Chaîne SHA-256, signature facultative | `kartenn-service/tests/audit_verify.rs` |
| INV-53 | Un événement n'est purgé qu'une fois scellé et livré à chaque sortie | Purge conditionnelle | `kartenn-telemetry/tests/audit_retention.rs` |
| INV-54 | Chaque sortie d'audit reçoit chaque événement au moins une fois, sans trou, même après une transaction longue | Livraisons enregistrées, planchers | `kartenn-telemetry/tests/audit_export.rs`, `kartenn-service/tests/pass_floor.rs` |
| INV-55 | Aucun secret (mot de passe, jeton, en-tête `Authorization`, chaîne de connexion) n'atteint une sortie, OTLP et rapport de panique compris | Masquage à la capture et à l'export des spans | `kartenn-server/tests/secrets.rs`, `kartenn-telemetry/tests/panic.rs` |
| INV-56 | Une sortie technique lente ou bloquée ne ralentit pas une requête : file bornée, abandons comptés | App | tests unitaires de `kartenn-telemetry/src/capture.rs` ; banc `bench-telemetry` |
| INV-57 | Surcoût des sorties de référence sur le p95 de chaque budget < 5 % ; avec une sortie bloquée < 10 % | Coût mesuré par événement et par span | Banc `bench-telemetry` (surcoût modélisé) |

## Exploitation (D9)

| ID | Invariant | Garantie |
|---|---|---|
| INV-40 | Les instances du serveur sont interchangeables (aucun état local faisant autorité) | Architecture ; test avec N ≥ 2 instances (`server/tests/jobs.rs`, `an_import_ends_on_another_instance_without_duplicates`) |
| INV-41 | Une tâche de fond n'est exécutée que par une instance à la fois ; celle d'une instance arrêtée est reprise par une autre à l'expiration du bail | Bail en base (`job`, `FOR UPDATE SKIP LOCKED`, ADR 0023) ; `server/tests/jobs.rs` (`three_schedulers_run_each_due_task_once`, `a_stopped_holder_is_relieved_after_its_lease`) |
| INV-42 | Les migrations ne s'exécutent qu'une fois, hors démarrage des instances | Job dédié |

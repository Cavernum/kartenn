---
status: approved
updated: 2026-10-21
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
| INV-27 | Aucun administrateur délégué n'accorde un droit qu'il ne détient pas sur le périmètre | App | `a_delegate_cannot_escalate` |
| INV-28 | Un compte de secours exige phrase de passe **et** code TOTP, un code n'est jamais accepté deux fois (même par deux requêtes simultanées), et aucune opération de l'API ne le crée ni ne le modifie | App + Base (mise à jour conditionnelle, droits du rôle applicatif) | `auth/tests/breakglass.rs`, `rbac_schema.rs` |
| INV-60 | Écrire pour le compte d'un demandeur (`Kartenn-On-Behalf-Of`) n'ouvre aucun droit : seul un compte de service détenteur de `core.identity.act_on_behalf` le peut, sous ses propres droits ; le demandeur n'en prête aucun et figure dans l'audit de la requête, revue comprise (validation, rejet, retrait) ; une tentative refusée est journalisée (`authz.access.denied`). Un demandeur n'est pas un compte et ne réserve rien : aucune liaison ne le désigne (`core.binding.requester`), et la personne qui se connecte ensuite par l'émetteur et le sujet qu'il porte reprend la fiche, promue `person` sous le nom donné par l'émetteur (016/G10) | App | `service/tests/it/on_behalf.rs` (`the_requester_lends_no_right`, `without_the_permission_or_as_a_person_it_is_refused`, `the_requester_who_logs_in_becomes_a_person_under_the_issuer_name`) ; `service/tests/it/module_review.rs` (`a_validation_for_a_requester_names_it_in_its_events`) ; `rest/tests/it/on_behalf.rs` (`the_header_is_refused_where_it_does_not_apply`, `a_repeated_header_is_refused`) ; de bout en bout, authentification réelle (Basic, jetons de l'émetteur) devant l'API : `rest/tests/it/on_behalf_e2e.rs` (`a_service_account_writes_for_its_requester_through_the_real_authentication`, `a_service_account_without_the_permission_is_refused_and_audited`, `a_person_with_the_header_is_refused_and_audited`, `a_malformed_or_repeated_header_is_a_bad_request`, `the_real_person_logging_in_takes_back_the_requester_record`) |
| INV-61 | Une opération dont le module exige une revue n'est jamais appliquée sans validation, même par un auteur qui peut écrire ; le valideur n'y joint que des opérations de revue, sur une proposition et sous ses droits requis ; une opération jointe hors revue est refusée et journalisée (`authz.access.denied`, raison `core.review.operation`) (016/G12) | App | `service/tests/it/module_review.rs` (`a_reviewed_operation_makes_a_proposal_despite_write`, `validation_needs_the_review_permission`, `review_operations_apply_with_the_proposal`, `without_the_review_permission_joined_operations_are_refused_too`, `review_operations_join_a_proposal_only`) ; `rest/tests/it/cycle.rs` (`a_validation_body_joins_operations_of_review_only`) |
| INV-62 | Un verdict de flux refuse par défaut : sans règle `allow` qui s'applique, extrémité non classifiée ou port sans service → refus ; une règle `deny` qui s'applique l'emporte toujours ; l'ordre des règles n'a aucun effet (010/S8, D28) | App (`kartenn-policy::evaluate`) | `policy/src/evaluate.rs` (`a_deny_wins`, `monotonic`, `the_order_of_the_rules_has_no_effect`, `a_port_stands_for_each_service_that_covers_it`) |
| INV-63 | Une requête de conformité ne révèle rien qu'on ne peut lire : extrémité invisible → même 404 qu'un objet absent ; sous `policy.flow.request` seul, l'explication ne nomme aucun objet du référentiel (010/REQ-010-25) | App (`kartenn-policy::verdict`) | `policy/tests/it/verdict.rs` (`acceptance_of_req_010_25`) |
| INV-29 | Une méthode non sûre authentifiée par cookie n'est servie que si `Sec-Fetch-Site` (ou, à défaut, `Origin`) la dit de même origine ; aucun secret Basic ni cookie ne circule hors TLS ; un curseur altéré ou signé d'une clé retirée est refusé et les droits sont réévalués à chaque page | App (couches CSRF, transport, curseurs signés) | `rest/tests/csrf.rs`, `authn.rs`, `cursor.rs` ; `auth/src/signed.rs` (`a_retired_key_no_longer_verifies`) |

## Performance (D14) — p95 serveur, 10⁶ objets, 3 ans d'historique

Le banc `bench-core` (002/A14) est bloquant par son code de sortie ; il n'y a pas encore
d'intégration continue pour l'exécuter à chaque changement. Le banc `bench-http`
(REQ-006-39) rejoue les scénarios de `bench-core` à travers le routeur, sur un port
local avec une authentification réelle, et rapporte le surcoût de HTTP ; il mesure aussi
les 200 utilisateurs concurrents et les 50 écritures/s d'INV-36. Le banc `bench-rbac` (005/K15)
mesure INV-30 à 34 pour une personne à 1 000 liaisons et 50 groupes, avec et sans
contrôle d'accès ; les changesets de 10⁴ objets et les imports prennent la même décision
une fois par changeset ou par lot.

| ID | Budget | Test |
|---|---|---|
| INV-30 | Lecture d'un objet < 50 ms | Bancs `bench-core`, `bench-rbac`, `bench-http` |
| INV-31 | Page filtrée (curseur, 100) < 150 ms, y compris filtre sur valeur résolue | Bancs `bench-core`, `bench-rbac`, `bench-http` |
| INV-32 | Sous-arbre et occupation < 200 ms | Bancs `bench-core`, `bench-rbac`, `bench-http` |
| INV-33 | Lecture « à la date X » ≤ ×2 la lecture courante | Bancs `bench-core`, `bench-rbac`, `bench-http` |
| INV-34 | Changeset unitaire validé et appliqué < 200 ms | Bancs `bench-core`, `bench-rbac`, `bench-http` |
| INV-35 | Changeset touchant jusqu'à 10⁴ objets < 2,5 s (changeset entier) ; différentiel < 10 s jusqu'à 10⁶ | Banc `bench-core` |
| INV-36 | Import de 10⁶ objets < 15 min ; 50 écritures/s soutenues ; 200 utilisateurs concurrents | Bancs `bench-core` (import), `bench-http` (charge) |

Les modules livrés tiennent INV-30, 31 et 34 sur le jeu de référence augmenté, mesurés
par `bench-core` après les budgets du noyau : VLAN (008, REQ-008-25 : 2·10⁴ groupes,
10⁵ VLAN ; banc complet tenu le 2026-10-21) et machines (009, REQ-009-21 : 10⁵
machines, 10³ clusters, 3·10⁵ interfaces et rattachements, plus un import de 10⁴
machines ; variante réduite tenue, banc complet encore à relancer, tâche 009-12).

Politique de flux (010, REQ-010-28 et 54 : instantané de 10⁴ règles à 6 axes, lecture
groupée de 512 adresses × 6 axes, requête de conformité, différentiel d'une politique) :
**à mesurer** par le prototype (010-1) et le banc `bench-policy` (010-19). Seule mesure
disponible : la lecture groupée N9 (016-4), 16,3 ms pour 30 ms au banc
`bench-properties` réduit (2026-10-21).

## Audit et journalisation (spec 012, ADR 0019)

| ID | Invariant | Garantie | Test |
|---|---|---|---|
| INV-50 | Toute écriture, tout refus d'autorisation et toute lecture de l'API produit un événement d'audit attribué ; s'il ne peut être écrit, l'action n'a pas lieu (échec fermé) | App + Base (même transaction) | `kartenn-service/tests/audit_events.rs`, `audit_reads.rs`, `audit_journal.rs` |
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

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
| INV-24 | Les décisions de flux (v0.2) se fondent sur la valeur résolue, jamais sur la valeur attendue | App | REQ-003-24 |
| INV-25 | Refus par défaut : sans liaison qui l'accorde, aucune lecture ni écriture ; un objet non lisible est répondu absent (même 404 que l'objet absent) ; une liste ne contient que ce que la décision unitaire autorise | App + prédicat SQL | `service/tests/rbac.rs` (`lists_hold_what_the_decision_allows`) ; `rest/tests/objects.rs` (`an_invisible_object_answers_as_an_absent_one`), `errors.rs` (`line_404_an_absent_or_invisible_object`) |
| INV-26 | Un émetteur confiné n'accorde rien hors des sous-arbres des entités qui lui sont rattachées, quelles que soient les liaisons | Prédicat SQL | `an_issuer_is_confined_to_its_entities` |
| INV-27 | Aucun administrateur délégué n'accorde un droit qu'il ne détient pas sur le périmètre | App | `a_delegate_cannot_escalate` |
| INV-28 | Un compte de secours exige phrase de passe **et** code TOTP, un code n'est jamais accepté deux fois (même par deux requêtes simultanées), et aucune opération de l'API ne le crée ni ne le modifie | App + Base (mise à jour conditionnelle, droits du rôle applicatif) | `auth/tests/breakglass.rs`, `rbac_schema.rs` |
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

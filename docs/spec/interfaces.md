---
status: approved
updated: 2026-10-18
---

# Interfaces

Les détails normatifs viendront des specs 006 (conventions REST) et 007 (shell et
plugins) ; l'authentification et le RBAC sont fixés par la spec 005. Ce document fixe les
principes décidés.

## API REST (D7, ADR 0002)

Seule frontière publique : Web, CLI, TUI, MCP et connecteurs passent tous par elle.

| Sujet | Principe | Source |
|---|---|---|
| Transport | HTTPS uniquement ; JSON (RFC 8259) | D13, RFC 9110 |
| Contrat | OpenAPI 3.1 généré depuis le code ; SDK clients générés (TypeScript, Rust) | D7 |
| Ressources | Une ressource par type d'objet du noyau et des modules ; désignation par UUID ou clé naturelle (slug, (domaine, préfixe)…) | 002/A9 |
| Écriture | Par **changeset** (opérations multiples, atomiques) ; raccourcis unitaires acceptables s'ils produisent un changeset | D11 |
| Simulation | Soumettre un changeset sans l'appliquer → validation, anomalies, différentiel | 003/REQ-003-20 |
| Temps | Paramètres `valid_at` (temps applicatif) et `as_of` (temps système), « maintenant » par défaut | D10 |
| Concurrence | ETag / `If-Match` (RFC 9110 §13) sur les objets ; conflit → 412 | Cadrage (« Noyau ou module », point A.8) |
| Erreurs | `application/problem+json` (RFC 9457) ; identifiants de règles stables ; objets en cause | 002/REQ-002-37 |
| Pagination | Par curseur ; tri stable | Cadrage (« Noyau ou module », point A.10) |
| Filtrage | Champs du noyau et valeurs résolues des propriétés | 003/REQ-003-25 |
| Capacités | Endpoint listant les modules actifs, leurs versions, propriétés et plugins UI | D7, D17 |
| Versionnement | À décider (Q-ouv-7) | — |

## Authentification (D13, ADR 0009 amendée par 005/K8, ADR 0020 ; spec 005)

- **Personnes, navigateur** : BFF (ADR 0020). kartenn est client confidentiel OIDC de
  chaque émetteur (code d'autorisation, PKCE S256 RFC 7636, `state`, `nonce`, paramètre
  `iss` RFC 9207) ; le navigateur ne reçoit qu'un cookie de session chiffré (AES-256-GCM,
  trousseau partagé par les instances), sans état côté serveur. Inactivité 30 min,
  durée absolue 8 h (réglables, au plus 60 min et 24 h) ; groupes rafraîchis par le
  jeton de rafraîchissement de l'émetteur.
- **Personnes, clients non navigateurs** : jetons d'accès JWT de l'émetteur (RFC 9068 :
  `typ` `at+jwt`, `iss` exact, audience de kartenn, algorithme de la liste fermée de
  l'émetteur, `exp` à 60 s près) ; clés JWKS en cache, rechargées sur `kid` inconnu au
  plus une fois par minute.
- **Identités** : une par (émetteur, `sub`), créée à la première connexion avec le seul
  nom affiché (pas d'adresse électronique). Désactivation et révocation des sessions
  par changeset, effectives à la requête suivante (cache d'état de 5 s par instance au
  plus).
- **Machines** (CLI, connecteurs, scripts) : comptes de service en **Basic** (RFC 7617)
  sur TLS ; secret de 256 bits généré par kartenn, montré une fois, stocké en empreinte
  HMAC-SHA-256 sous le trousseau (K8, pas d'Argon2id : le secret est aléatoire) ; deux
  secrets actifs au plus (rotation), expiration, révocation immédiate.
- **Secours** : comptes locaux créés hors de l'API par le propriétaire du schéma
  (`kartenn-server breakglass create`), valables 1 à 24 h ; phrase de passe (Argon2id,
  RFC 9106) **et** code TOTP (RFC 6238), un code jamais accepté deux fois ; 100 échecs
  consécutifs désactivent le compte ; chaque usage audité en criticité haute.
- **Refus** : une seule réponse à l'appelant ; la raison codée (`state`, `iss`, `nonce`,
  `signature`, `expired`, `audience`, `secret`, `totp`, `disabled`, `revoked`…) et
  l'émetteur vont à l'audit (`authn.login.failed`), l'identifiant revendiqué en empreinte.
- Couche HTTP (006) : routes, attributs des cookies (`__Host-`, `Secure`, `HttpOnly`,
  `SameSite`), refus hors TLS, limitation de débit des échecs Basic, réponse
  `invalid_token`, appel de `authn::session_closed` à la fin d'une session.

## Autorisation (D36, 003/P6–P7, ADR 0010, ADR 0021 ; spec 005)

- Catalogue des permissions généré : [`permissions.md`](permissions.md)
  (`core.<objet>.<action>`, `prop.<clé>.<action>`, `prop.*.<action>` hors propriétés
  sensibles, `*`) ; rôles livrés `reader`, `editor`, `approver`, `administrator`,
  synchronisés par le job de migration et non modifiables.
- Rôle = ensemble de permissions ; liaison = sujet (identité, groupe d'un émetteur,
  émetteur entier) × rôle × périmètre (global, entité, site, domaine, sous-arbre de
  préfixes), héritée vers le bas. **Refus par défaut**, pas de règle de refus.
- **Confinement** : un émetteur rattaché à des entités n'accorde rien hors de leurs
  sous-arbres, quelles que soient les liaisons ; rattachement global possible. Les
  rattachements vivent en base, pas dans la configuration.
- Une lecture sans droit répond l'objet absent (`authz.access.denied` audité) ; les
  listes (`filter_in`, `children_in`) sont filtrées par le même prédicat SQL que la
  décision ; une valeur sensible sans `prop.<clé>.read` est retirée et sa clé listée.
- Une écriture exige `core.<sorte>.write` (ou `propose`, qui en fait une proposition)
  sur chaque objet visé, décidé dans la transaction du changeset, **et** le droit sur
  les effets indirects (différentiel des propriétés).
- Administration par changesets : rôles, liaisons, rattachements, comptes de service,
  état des identités ; un administrateur délégué n'accorde que ce qu'il détient sur le
  périmètre (`core.binding.escalation`). L'annulation inverse les rôles, liaisons et
  rattachements par des opérations ordinaires (`CloseRole`/`SetRole`,
  `Unbind`/`Bind`, `DetachIssuer`/`AttachIssuer`), soumises aux mêmes contrôles ; une
  liaison rendue reçoit un nouvel identifiant ; ce qu'un changeset plus récent a écrit
  est un conflit laissé de côté (005/K16). Comptes de service, secrets et état des
  identités ne s'annulent pas : listés comme non pris en charge.
- Politiques d'approbation dans le même modèle : périmètre (domaine, entité, site,
  sous-arbre), auteur (personne, rôle, groupe), rôle lié (`bound_roles`, par exemple un
  second administrateur pour toute liaison du rôle administrateur) ; les modifier exige
  `core.approval_policy.admin`.
- Budgets : tenus avec 1 000 liaisons et 50 groupes (banc `bench-rbac`, 005/K15).

## SDK de plugins UI (D17, ADR 0011)

- Plugin = module ES chargé dynamiquement, qui enregistre des Custom Elements.
- Le SDK TypeScript versionné fournit : client API généré, contexte (utilisateur,
  `valid_at`, `as_of`, objet courant), points d'extension (onglets, vues, panneaux
  d'objet).
- Un plugin n'a que les droits de l'utilisateur ; chargement soumis à CSP et SRI.
- Licence du SDK : question ouverte Q-ouv-1.

## Opérations d'un changeset (interface du service)

Avant l'API (006), le service expose `submit` et le type `Operation`
(`crates/kartenn-service/src/operations.rs`) : création, modification, clôture et
restauration des objets du noyau ; déplacement et renommage (`Rename`, l'UUID est
conservé) des entités, des sites et des domaines ; déclarations, valeurs de listes et
affectations de propriétés ; liaison d'identifiants externes ; politiques d'approbation ;
administration du RBAC (`SetRole`, `CloseRole`, `Bind`, `Unbind`, `AttachIssuer`,
`DetachIssuer`, `CreateServiceAccount`, `AddServiceSecret`, `RevokeServiceSecret`,
`SetIdentityState`) ; `Within` pour donner sa propre période à une opération.

## Événements sortants (D11, ADR 0018)

CloudEvents 1.0 en JSON, relayés depuis l'outbox au moins une fois, dans l'ordre par
domaine d'adressage : `org.kartenn.changeset.applied` (changeset, domaines, objets,
auteur, valideur, résumé du différentiel) et `org.kartenn.changeset.<état>` pour les
changesets en attente. Le transport (webhook, file de messages) est l'objet de la 006.

## Connecteurs (D1, D7)

- Processus externes, clients REST, authentifiés par compte de service.
- Importent par changesets (lots) ; référencent les objets par identifiant externe
  (source, id externe → UUID, REQ-002-41).

## Configuration et exploitation (D35)

- Configuration 12-factor : fichier TOML, puis variables `KARTENN_*` (imbrication par
  `__`, par exemple `KARTENN_AUDIT__SEAL_INTERVAL_MS`). Une sortie inconnue, un champ
  manquant ou inconnu, un filtre invalide refusent le démarrage
  (`kartenn_server::config`, REQ-012-31).
- Sondes : `/health/live`, `/health/ready` (avec l'API, 006).
- Binaire `kartenn-server` :
  - `migrate <url>` : job de migration (D9) ;
  - `run <url> [<config>]` : une instance, jusqu'à `SIGTERM` ou Ctrl-C ; à l'arrêt,
    l'événement d'arrêt est scellé et exporté, les files des sorties sont vidées et
    OpenTelemetry est arrêté, dans 10 s (REQ-012-34) ;
  - `audit verify <url> [<clé-publique-hex>]` : vérifie la chaîne des sceaux et leurs
    signatures ; code de sortie 2 si le journal est altéré (REQ-012-14) ;
  - `audit purge <url> [<config>]` : rétention par criticité (REQ-012-16), pour une
    minuterie quotidienne (cron, systemd).
  - `breakglass create <url> <nom> <heures> <fichier-de-clés>` : compte de secours valable
    de 1 à 24 h, avec la connexion du propriétaire du schéma ; affiche une fois la phrase
    de passe et l'URI `otpauth://` du second facteur (REQ-005-14).

## Configuration de l'authentification (spec 005)

**Réglages** (`[auth]`, facultatif : sans lui, personne ne peut se connecter) :
`key_file` (trousseau, une clé de 256 bits par ligne, la première courante),
`session_idle_minutes` (30, de 1 à 60), `session_absolute_hours` (8, de 1 à 24),
`redirect_uri` (requis avec des émetteurs) et `[[auth.issuers]]` : `id` (minuscules,
chiffres, tirets ; unique), `issuer`, `client_id`, `client_secret_file`, `audience`,
`groups_claim` (`groups`), `algorithms` (`["RS256", "ES256"]`).

- `redirect_uri` et `issuer` sont en `https`, ou en `http` sur la boucle locale pour le
  développement (RFC 8252 §7.3), sans fragment (RFC 6749 §3.1.2) ; un `issuer` sans
  requête (OpenID Connect Discovery 1.0 §3).
- `algorithms` ne prend que des signatures à clé publique (RS, PS, ES, EdDSA) : `none` et
  les HMAC sont refusés (RFC 8725 §3.1 ; choix de kartenn).
- Le trousseau et les secrets des clients sont lus au démarrage : un fichier absent, un
  trousseau mal formé ou un secret vide refusent le démarrage (`kartenn_server::auth`).
- Le rattachement d'un émetteur à une entité vit en base (K11), pas dans la
  configuration.

## Journalisation et audit (spec 012, ADR 0019)

Deux flux distincts : le **journal d'audit**, en base, qui fait foi (qui a fait quoi, avec
quel résultat) ; les **logs techniques**, pour l'exploitation. Le code métier n'émet que
par le port `AuditJournal` et la façade `tracing` ; aucune bibliothèque de sortie
n'atteint `kartenn-core` ni `kartenn-service` (REQ-012-2, test d'architecture).

**Événements d'audit** : catalogue généré
[`evenements-audit.md`](evenements-audit.md) (nom en trois parties, sévérité,
criticité, champs, correspondances OCSF et ECS) et son schéma JSON
[`evenements-audit.schema.json`](evenements-audit.schema.json) (forme exportée, une
branche par événement ; REQ-012-10). Chaque événement est attribué
(personne, compte de service, `system`, ou empreinte HMAC de l'identifiant revendiqué).

**Sorties de l'audit** (`[audit] sinks`, plusieurs à la fois, chacune reçoit chaque
événement au moins une fois, dans l'ordre de `seq`) :

| `kind` | Format | Transport |
|---|---|---|
| `file` | une ligne JSON par événement (la ligne stockée, que les sceaux couvrent) | fichier, synchronisé après chaque lot |
| `syslog` | RFC 5424, JSON dans MSG | TCP ou TLS, comptage d'octets (RFC 6587 §3.4.1, RFC 5425) |
| `otlp` | log OTLP, l'événement en corps structuré | OTLP/gRPC, un `Export` acquitté par lot |

**Réglages de l'audit** (`[audit]`) : `seal_interval_ms` (15 000), `relay_interval_ms`
(1 000), `signing_key_file` (facultatif), `retention_days = { low = 90, normal = 365,
high = 1095 }` (au moins un jour chacune), `read_ids_max` (100 : identifiants enregistrés
par événement de lecture, les autres comptés dans `targets_omitted` ; 0 = critères
seuls ; L25).

**Sorties techniques** (`[telemetry] outputs`, plusieurs à la fois, chacune avec son
filtre `EnvFilter` et sa file bornée de `queue` enregistrements ; une file pleine compte
et abandonne, sans ralentir la requête) :

| `kind` | Format |
|---|---|
| `stdout` | texte ou JSON (`format`) |
| `file` | lignes JSON |
| `journald` | protocole natif (`MESSAGE_ID` dérivé du nom) |
| `syslog` | RFC 5424 ; transport `local`, `udp`, `tcp` ou `tls` |
| `otlp` | logs, traces et métriques OTLP/gRPC vers `endpoint`, ou `OTEL_EXPORTER_OTLP_ENDPOINT` |

- Chaque enregistrement porte un horodatage RFC 3339 en UTC à la microseconde, une
  sévérité (table unique : ERROR 3/17, WARN 4/13, INFO 6/9, DEBUG 7/5, TRACE 7/1, en
  syslog/OpenTelemetry) et un nom stable (`event.name`).
- Événements techniques du pipeline : `pipeline.changeset.<état>` (`applied`, `refused`,
  `pending`, `stale`, `replayed`, `simulated`, `rejected`, `failed`), avec `state`,
  `operations`, `violations`, `duration_ms` ; `audit.write.failed` ; `sys.*` (démarrage,
  arrêt, horloge, panique, filtres).
- Traces : span `pipeline.changeset`, spans `db.*` du store (`db.system.name`,
  `db.operation.name`, `db.query.summary`, jamais de paramètre) ; échantillonnage
  `ParentBased(TraceIdRatio(sampling_ratio))`.
- Métriques : `kartenn.changeset.count{state}`, `kartenn.pipeline.duration`,
  `kartenn.log.dropped{output}`, `kartenn.audit.export.lag{sink}`,
  `kartenn.audit.unsealed`, `kartenn.audit.write.failures`, `kartenn.clock.skew`,
  `http.server.request.duration` (avec l'API).
- Filtre d'une sortie changé à chaud par un administrateur
  (`core.observability.admin`), audité, appliqué par toutes les instances, borné à 24 h
  (REQ-012-27).
- Masquage : valeurs des champs nommés comme un secret (`password`, `token`,
  `authorization`, `cookie`, `secret`, `dsn`, `connection_string`…) et mot de passe des
  URL, sur toutes les sorties, OTLP comprises, et dans le rapport de panique
  (REQ-012-28) ; valeurs bornées à 2 Kio, caractères de contrôle neutralisés
  (REQ-012-29).

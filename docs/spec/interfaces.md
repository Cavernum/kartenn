---
status: approved
updated: 2026-10-17
---

# Interfaces

Les détails normatifs viendront des specs 005 (authentification, RBAC), 006 (conventions
REST) et 007 (shell et plugins). Ce document fixe les principes déjà décidés.

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

## Authentification (D13, ADR 0009)

- **Humains** : OIDC, authorization code + PKCE (RFC 7636), plusieurs émetteurs de
  confiance configurés ; le serveur valide les JWT (signature via JWKS en cache,
  `iss`, `aud`, expiration ; RFC 8725). Les claims (groupes) sont mappés sur des liaisons
  RBAC.
- **Machines** (CLI, connecteurs, scripts) : comptes de service en **Basic** (RFC 7617)
  sur TLS ; secrets hachés en Argon2id (RFC 9106).
- **Secours** : comptes humains locaux en Basic, réservés au break-glass.

## Autorisation (D36, 003/P6–P7, ADR 0010)

- Permissions déclarées par les modules (`<module>.<objet>.<action>`, et
  `prop.<clé>.write` par propriété).
- Rôle = ensemble de permissions ; liaison = sujet × rôle × périmètre (entité, site,
  domaine, sous-arbre de préfixes), héritée vers le bas.
- Une écriture exige le droit sur les objets visés **et** sur les effets indirects
  (différentiel des propriétés).
- Politiques d'approbation dans le même modèle ; les créer, les modifier ou les
  désactiver exige `core.approval_policy.admin` (règle `changeset.policy.forbidden`).
- En attendant la 005, le service vérifie les permissions `prop.<clé>.write` et
  `prop.<clé>.propose` et celle des politiques, par le port `Authorizer`
  (`crates/kartenn-service/src/authz.rs`) ; les permissions par type d'objet
  (`core.address.write`…) viendront avec le catalogue de la 005.

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
`Within` pour donner sa propre période à une opération.

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

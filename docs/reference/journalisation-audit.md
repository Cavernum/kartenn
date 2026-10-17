# Journalisation, observabilité et audit

Note de lecture pour la spec [012-journalisation-audit](../../specs/012-journalisation-audit/).
Recherche documentaire du 2026-10-17 ; ce qui n'a pas pu être relu à la source est marqué
**non vérifié**.

## Sources

| Document | Statut | Consulté |
|---|---|---|
| [RFC 5424](https://www.rfc-editor.org/rfc/rfc5424.html) — The Syslog Protocol | Proposed Standard (2009) | 2026-10-17 |
| [RFC 5425](https://www.rfc-editor.org/rfc/rfc5425.html) — TLS Transport Mapping for Syslog, mise à jour par [RFC 9662](https://www.rfc-editor.org/info/rfc9662) | Proposed Standard (2009, 2024) | 2026-10-17 |
| [RFC 6587](https://www.rfc-editor.org/rfc/rfc6587.html) — Transmission of Syslog Messages over TCP | Historic | 2026-10-17 |
| [RFC 5426](https://www.rfc-editor.org/rfc/rfc5426.html) (UDP), [RFC 3164](https://www.rfc-editor.org/rfc/rfc3164.html) (BSD syslog), [RFC 5848](https://www.rfc-editor.org/rfc/rfc5848) (messages signés) | Proposed Standard ; Informational ; Proposed Standard | 2026-10-17 |
| [RFC 3339](https://www.rfc-editor.org/rfc/rfc3339.html) §5.6 et [RFC 9557](https://www.rfc-editor.org/rfc/rfc9557.html) (mise à jour du §4.3) | Proposed Standard | 2026-10-17 |
| [Protocole natif de journald](https://systemd.io/JOURNAL_NATIVE_PROTOCOL/) | documentation systemd | 2026-10-17 |
| [OpenTelemetry — modèle de données des logs](https://opentelemetry.io/docs/specs/otel/logs/data-model/) et [annexe](https://opentelemetry.io/docs/specs/otel/logs/data-model-appendix/) | stable | 2026-10-17 |
| [OpenTelemetry — état des signaux](https://opentelemetry.io/docs/specs/status/), [OTLP](https://opentelemetry.io/docs/specs/otlp/), [conventions sémantiques 1.44](https://opentelemetry.io/docs/specs/semconv/) | voir ci-dessous | 2026-10-17 |
| [W3C Trace Context](https://www.w3.org/TR/trace-context/) ; [W3C Baggage](https://www.w3.org/TR/baggage/) | Recommendation (2021) ; Candidate Recommendation (2024) | 2026-10-17 |
| [OWASP Logging Cheat Sheet](https://cheatsheetseries.owasp.org/cheatsheets/Logging_Cheat_Sheet.html), [Logging Vocabulary](https://cheatsheetseries.owasp.org/cheatsheets/Logging_Vocabulary_Cheat_Sheet.html) | guides | 2026-10-17 |
| [OWASP ASVS 5.0, chapitre V16](https://github.com/OWASP/ASVS/blob/master/5.0/en/0x25-V16-Security-Logging-and-Error-Handling.md) | standard de vérification | 2026-10-17 |
| [OWASP Top 10:2025 A09](https://top10.owasp.org/2025/A09_2025-Security_Logging_and_Alerting_Failures) | référentiel | 2026-10-17 |
| [NIST SP 800-53 r5](https://csrc.nist.gov/pubs/sp/800/53/r5/upd1/final), famille AU ; [SP 800-92 r1](https://csrc.nist.gov/pubs/sp/800/92/r1/ipd) | final ; avant-projet (2023) | 2026-10-17 (AU **non vérifié** au texte) |
| ANSSI, [Recommandations de sécurité pour l'architecture d'un système de journalisation](https://messervices.cyber.gouv.fr/documents-guides/anssi-guide-recommandations_securite_architecture_systeme_journalisation.pdf) | v2.0, 2022-01-28 | 2026-10-17 |
| CNIL, délibération n° 2021-122 (durée de conservation des journaux) | recommandation | 2026-10-17 (citée par le guide ANSSI) |
| [CWE-117](https://cwe.mitre.org/data/definitions/117.html), [CWE-532](https://cwe.mitre.org/data/definitions/532.html) | catalogue | 2026-10-17 |
| [OCSF 1.9](https://schema.ocsf.io/), ECS 9.5, [DMTF CADF DSP0262](https://www.dmtf.org/sites/default/files/standards/documents/DSP0262_1.0.0.pdf) | schémas d'événements de sécurité | 2026-10-17 |

## Ce que disent les sources

### Syslog (RFC 5424 et transports)

- Format (§6) : `<PRI>1 TIMESTAMP HOSTNAME APP-NAME PROCID MSGID [SD] MSG` ; sévérités
  0 à 7 et facilités (§6.2.1, tables 1 et 2).
- Taille (§6.1) : un récepteur MUST accepter 480 octets et SHOULD accepter 2048 octets ;
  une troncature MUST se faire en fin de message.
- Horodatage (§6.2.3) : profil de RFC 3339 ; « T » et « Z » MUST être en majuscules ; les
  secondes intercalaires MUST NOT être utilisées ; au plus 6 chiffres de fraction,
  SHOULD être présents si l'horloge le permet ; NILVALUE MUST si l'heure est inconnue.
- Données structurées (§6.3) : un SD-ID privé est `nom@<numéro d'entreprise privé
  IANA>` (§6.3.2) ; dans une valeur, `"`, `\` et `]` MUST être échappés (§6.3.3).
- MSG en UTF-8 (§6.4) : MUST commencer par le BOM.
- SD-ID enregistrés (§7) : `timeQuality` (`tzKnown`, `isSynced`, `syncAccuracy`) ;
  `meta` avec `sequenceId` (§7.3.1), qui MUST valoir 1 au démarrage et s'incrémenter :
  il permet de détecter une perte.
- Sécurité (§8) : le transport TLS est REQUIRED à implémenter et RECOMMENDED (§8.6) ;
  troncature, rejeu, fiabilité, intégrité et déni de service y sont discutés.
- TLS (RFC 5425, mise à jour par RFC 9662) : TLS 1.2 obligatoire à implémenter, TLS 1.3
  SHOULD et MUST être préféré s'il est implémenté ; 0-RTT MUST NOT ; cadrage par comptage
  d'octets. Port 6514 : **non vérifié**.
- TCP (RFC 6587, historique) : comptage d'octets (§3.4.1) fiable ; cadrage par fin de
  ligne (§3.4.2) fragile.
- UDP (RFC 5426) : aucune fiabilité. RFC 3164 : horodatage sans année ni fuseau ; utile
  seulement en réception.

### journald

- Datagrammes `AF_UNIX` vers `/run/systemd/journal/socket`, champs `CLÉ=VALEUR` ou forme
  binaire ; memfd scellé pour les grosses entrées.
- Champs utiles : `MESSAGE`, `PRIORITY` (0–7), `SYSLOG_IDENTIFIER`, `MESSAGE_ID`
  (128 bits, identifie le type d'événement). Les champs commençant par `_` sont posés par
  journald et ignorés s'ils viennent du client.

### Horodatage

- RFC 3339 §5.6 pour la grammaire ; RFC 9557 change le sens de `Z` : « heure UTC connue,
  décalage local inconnu », ce qui convient à un serveur qui raisonne en UTC.

### OpenTelemetry

- État : traces stables (API, SDK, protocole) ; logs stables (pont d'API, SDK,
  protocole) ; métriques : API et protocole stables, SDK mixte.
- Modèle de log : `Timestamp`, `ObservedTimestamp`, `SeverityText`, `SeverityNumber`
  (TRACE 1–4, DEBUG 5–8, INFO 9–12, WARN 13–16, ERROR 17–20, FATAL 21–24),
  `TraceId`/`SpanId`, `EventName` (SHOULD identifier la structure de l'événement),
  `Body`, `Attributes`, `Resource`.
- Correspondance syslog (annexe B) : Emergency → 21, Alert → 19, Critical → 18,
  Error → 17, Warning → 13, Notice → 10, Informational → 9, Debug → 5.
- Conventions sémantiques : HTTP stable ; spans de base de données stables ;
  `db.query.text` SHOULD n'être collecté que désinfecté, `db.query.parameter.*` SHOULD
  NOT l'être par défaut ; les exceptions (`exception.*`) peuvent contenir des données
  sensibles. **Aucune convention d'audit** n'existe encore (ticket ouvert).
- OTLP : gRPC (4317) et HTTP (4318, `/v1/logs`, `/v1/traces`, `/v1/metrics`).
- Le processeur par lots du SDK **abandonne** quand sa file est pleine : OTLP n'est pas
  un transport d'audit sans perte.

### W3C Trace Context et Baggage

- `traceparent` invalide : MUST être ignoré (§3.2.2.3). Les en-têtes MUST NOT porter de
  données personnelles (§6). Un drapeau `sampled` forgé est un vecteur de déni de
  service (§7.2).
- Baggage : 64 membres, 8192 octets (§3.3.2) ; mise en garde sur les données qui
  franchissent une frontière de confiance (§4.1).

### Référentiels de sécurité

- **OWASP Logging Cheat Sheet** : quand, où, qui, quoi, avec un identifiant
  d'interaction ; ne jamais journaliser mots de passe, jetons, identifiants de session,
  clés, chaînes de connexion ; neutraliser CR, LF et délimiteurs ; détecter
  l'altération ; journaliser l'accès aux journaux ; tester le comportement quand la
  journalisation échoue.
- **OWASP Logging Vocabulary** : noms d'événements `authn_login_success|fail|lock`,
  `authz_fail|change|admin`, `privilege_permissions_changed`,
  `sensitive_read|create|update|delete`, `sys_startup|shutdown|monitor_disabled`,
  `user_created|deleted`, `excess_rate_limit_exceeded`, `input_validation_fail`…
- **ASVS 5.0 V16** : inventaire de la journalisation (16.1.1) ; contexte quand, où, qui,
  quoi (16.2.1) ; horloges synchronisées, UTC ou décalage explicite (16.2.2) ;
  destinations inventoriées seulement (16.2.3) ; format commun (16.2.4) ; données
  sensibles masquées (16.2.5) ; toutes les tentatives d'authentification (16.3.1) ; refus
  d'autorisation (16.3.2) ; contournements (16.3.3) ; erreurs inattendues (16.3.4) ;
  encodage contre l'injection (16.4.1) ; protection contre la modification (16.4.2) ;
  envoi vers un système séparé (16.4.3) ; erreurs génériques et gestionnaire de dernier
  recours (16.5).
- **OWASP Top 10:2025 A09** : piste d'audit avec contrôle d'intégrité (tables en ajout
  seul), alertes ; CWE 117, 221, 223, 532, 778.
- **NIST SP 800-53 r5, famille AU** (intitulés, **non vérifiés** au texte) : AU-2
  événements, AU-3 contenu, AU-5 réaction aux échecs de journalisation, AU-8 horodatage,
  AU-9 protection (système séparé, protection cryptographique, accès restreint), AU-10
  non-répudiation, AU-11 rétention, AU-12(3) changement de journalisation par des
  personnes autorisées.
- **ISO/IEC 27001:2022, annexe A** : 8.15 journalisation, 8.16 surveillance, 8.17
  synchronisation des horloges (intitulés seulement, texte payant).
- **ANSSI (2022)** : journalisation prévue dès la conception (R1) ; horodater tous les
  événements, horloges synchronisées, UTC conseillé (R3–R5) ; centraliser, collecte
  résiliente, transfert en temps réel, protocoles fiables et sécurisés (R9–R17) ;
  rotation et rétention conformes à la réglementation (R24, R25) ; droits d'écriture,
  de suppression et de lecture au strict besoin (R26, R27).
- **CNIL 2021-122** (citée par l'ANSSI) : conservation recommandée de 6 mois à 1 an,
  jusqu'à 3 ans dans certains cas.

### Pratique d'autres projets libres (syslog)

- **Keycloak** ([documentation de la journalisation](https://www.keycloak.org/server/logging),
  consultée le 2026-10-17) : RFC 5424 par défaut (RFC 3164 en option), TCP, UDP ou TLS
  (`ssl-tcp`), comptage d'octets par défaut en TCP, longueur maximale 2048 octets,
  événement en JSON (ou ECS) dans MSG, aucune donnée structurée privée ; plusieurs
  sorties simultanées (console, fichier, syslog), chacune avec son niveau.
- **Coffre-fort de secrets à audit syslog**
  ([exemple](https://developer.hashicorp.com/vault/docs/audit/syslog)) : envoi au seul
  agent syslog local ; la documentation conseille d'activer aussi une sortie fichier pour
  qu'un échec syslog ne bloque pas le service, ce qui confirme que l'audit y est en échec
  fermé.
- **syslog-ng** : ses champs structurés propres utilisent le numéro d'entreprise IANA de
  son éditeur (`…@18372.4`).

### Formats d'événements de sécurité

- **OCSF 1.9** : classes « Authentication », « API Activity », « Account Change »… ; la
  cible d'export la plus répandue vers un SIEM.
- **ECS 9.5** : vocabulaire `event.action`, `event.outcome`, `event.category`, en cours de
  convergence avec les conventions OpenTelemetry.
- **CADF** : modèle acteur / action / cible / résultat / observateur / raison ; bon
  modèle conceptuel, format peu utilisé.
- **CEF, LEEF** : formats de ligne propres à des éditeurs, à produire au besoin par le
  collecteur.

## Pièges

- **Deux couches OpenTelemetry pour Rust** : `tracing-opentelemetry` change aussi les
  événements `tracing` en événements de span, et `opentelemetry-appender-tracing` en
  enregistrements de log ; avec les deux, chaque événement part deux fois. Filtrer les
  événements de la couche de traces (méthode exacte à valider par un test).
- **Contexte de trace dans les logs** : il vient de l'activation du contexte OTel à
  l'entrée d'un span par `tracing-opentelemetry` ; dépend de l'ordre des couches.
- **Boucle de télémétrie** : filtrer `hyper`, `tonic`, `h2`, `reqwest` sur la couche OTLP.
- **Rechargement à chaud** : recharger le filtre, pas la couche (le downcast de la couche
  OTel ne passe pas à travers `reload::Layer`).
- **Arrêt** : `shutdown()` des fournisseurs et `WorkerGuard` gardés jusqu'à la fin, sinon
  les derniers événements sont perdus.
- **Pertes silencieuses par défaut** : `tracing-appender::non_blocking` est en mode
  « lossy » (128 000 lignes), le processeur par lots d'OTLP abandonne au-delà de 2048.
- **Crates syslog** : `syslog` 7.0 n'a ni cadrage TCP, ni TLS, ni BOM ; `syslog-tracing`
  passe par `libc::syslog()` (RFC 3164 local). Un formateur RFC 5424 propre, testé, est
  plus sûr.
- **journald et sévérités** : `tracing-journald` envoie INFO en Notice (5), alors que
  l'annexe OTel place INFO sur Informational (9 ≈ syslog 6) : fixer une table unique.
- **Récepteur syslog du collecteur OTel** : `enable_octet_counting` vaut `false` par
  défaut.

## Implications pour kartenn (propositions, tranchées dans la spec 012)

- Syslog sans numéro d'entreprise : en-tête RFC 5424 renseigné, SD-ID enregistrés
  seulement (`meta`, `timeQuality`), événement en JSON dans MSG (pratique de Keycloak).
- Séparer les **logs techniques** (pertes possibles, comptées) du **journal d'audit**
  (sans perte silencieuse, écrit en base dans la transaction de l'action ou avant la
  réponse) ; les changesets restent la piste d'audit des écritures (D8, D11).
- Une table unique de correspondance des sévérités pour toutes les sorties.
- Horodatage UTC en RFC 3339 avec `Z`, à la microseconde.
- Intégrité de l'audit : table en ajout seul, scellement par chaînage de hachés,
  vérifiable par une commande, têtes exportées hors du système.
- Sorties multiples sur tampons bornés, une sortie lente ne freinant ni les autres ni les
  requêtes.
- Vocabulaire d'événements dérivé d'OWASP ; correspondance documentée vers OCSF et ECS,
  appliquée à l'export.

# 012 — Journalisation, observabilité et audit : journal des décisions

Contexte : D8, D10, D11 (changesets, audit des écritures), D13 (authentification), D35
(OpenTelemetry, logs JSON), D36 (RBAC), C4 de la 004 (personnes pseudonymisables) ;
ADR 0001 (hexagonal). Sources : note
[`docs/reference/journalisation-audit.md`](../../docs/reference/journalisation-audit.md).

Demande de l'utilisateur (2026-10-17) : exigences d'auditabilité selon les bonnes
pratiques ; logs structurés compatibles avec les systèmes classiques (syslog au moins) ;
OpenTelemetry ; plusieurs sorties simultanées à coût limité ; sévérité et horodatage sur
chaque log ; actions journalisées et attribuées ; implémentations derrière des interfaces ;
collecteur et console dans l'environnement de développement. Les autres décisions lui ont
été déléguées ; elles sont à valider avec les exigences.

## L1 — Deux flux distincts
- Décision : les **logs techniques** (diagnostic, pertes possibles mais comptées,
  niveau réglable) et le **journal d'audit** (événements de sécurité, sans perte
  silencieuse, schéma fixe, indépendant du niveau de log) sont deux flux séparés, avec
  leurs propres sorties, droits d'accès et rétention.
- Pourquoi : un log technique peut être échantillonné ou abandonné sous charge ; un
  événement d'audit, jamais (ASVS 16.4, NIST AU-5, OWASP).
- Rejeté : un seul flux à niveaux (un réglage de niveau ou une saturation ferait perdre
  de l'audit).
- Décidé par : délégué (2026-10-17)

## L2 — Les changesets restent l'audit des écritures, le journal d'audit les relaie
- Décision : le journal d'audit couvre ce que les changesets ne couvrent pas
  (authentification, refus de droits, administration, configuration, lectures
  sensibles, consultation de l'audit, démarrage et arrêt) et reçoit en plus un événement
  par changement d'état d'un changeset, qui le référence, écrit dans sa transaction.
  Un SIEM voit ainsi toutes les actions dans un seul flux.
- Pourquoi : D11 fait du changeset l'unité d'audit des écritures ; on ne duplique pas son
  contenu (opérations, différentiel), seulement sa trace.
- Rejeté : journal d'audit sans les écritures (le SIEM devrait lire deux sources) ;
  copie complète des changesets (volume, double vérité).
- Décidé par : délégué (2026-10-17)

## L3 — Audit persistant en base, échec fermé
- Décision : un événement d'audit est écrit dans PostgreSQL (`audit_event`, en ajout
  seul pour le rôle applicatif), dans la transaction de l'action quand elle écrit, sinon
  avant la réponse. Si l'écriture échoue, l'action échoue (« fail closed »). L'export vers
  les systèmes externes se fait ensuite, au moins une fois, par un relais (comme
  l'outbox, ADR 0018).
- Pourquoi : OTLP et les écrivains non bloquants abandonnent sous saturation ; la base
  est déjà le point de cohérence (D9) et garantit l'atomicité avec l'action.
- Conséquence : une écriture en base par action auditée ; un refus d'authentification
  coûte une insertion (limitée par L13).
- Rejeté : audit seulement vers syslog ou OTLP (pertes) ; « fail open » (une panne de
  l'audit rendrait les actions invisibles, NIST AU-5(4)).
- Décidé par : délégué (2026-10-17)

## L4 — Intégrité : scellement par lots chaînés
- Décision : un job scelle régulièrement (60 s par défaut) les événements non scellés en
  lots : empreinte SHA-256 du lot chaînée à celle du lot précédent, signée en Ed25519 si
  une clé est configurée. Chaque sceau est lui-même exporté comme événement
  (`audit.seal.created`), donc ancré hors du système. Une commande vérifie la chaîne.
- Pourquoi : un chaînage événement par événement sérialiserait toutes les instances
  (D9) ; le scellement par lots, sur les événements validés, ne crée aucune contention et
  ne laisse pas de trou (l'appartenance au lot est enregistrée, comme les livraisons de
  l'outbox).
- Rejeté : chaînage à l'écriture (verrou global) ; arbre de Merkle avec preuves
  d'inclusion (utile pour un journal public, surdimensionné ici) ; aucune intégrité
  (OWASP A09, ASVS 16.4.2).
- Décidé par : délégué (2026-10-17)

## L5 — Attribution
- Décision : tout événement d'audit porte un **acteur** : personne (identifiant interne
  stable, C4), compte de service, ou `system` ; avec la méthode d'authentification et
  l'émetteur. Seul un échec d'authentification peut ne pas avoir d'acteur établi : on
  garde alors l'identifiant revendiqué sous forme d'empreinte à clé (HMAC), jamais en
  clair (il peut contenir un mot de passe saisi dans le mauvais champ).
- Pourquoi : « les actions doivent être journalisées et attribuées » (utilisateur, D11) ;
  C4 impose l'identifiant interne pour la pseudonymisation.
- Décidé par : utilisateur (attribution) ; délégué (forme, 2026-10-17)

## L6 — Sévérité et horodatage
- Décision : cinq niveaux (TRACE, DEBUG, INFO, WARN, ERROR), avec **une seule table** de
  correspondance vers syslog, OpenTelemetry et journald, appliquée par toutes les
  sorties. Horodatage UTC en RFC 3339 avec `Z` et six décimales ; un événement d'audit
  porte aussi l'heure d'enregistrement par la base.
- Pourquoi : demande de l'utilisateur ; les bibliothèques divergent par défaut
  (`tracing-journald` envoie INFO en Notice) ; RFC 5424 plafonne à la microseconde.
- Décidé par : utilisateur (présence) ; délégué (table, précision, 2026-10-17)

## L7 — Sorties multiples, non bloquantes
- Décision : sorties configurables et cumulables : sortie standard (texte ou JSON),
  fichier JSON, journald, syslog RFC 5424 (socket local, TCP ou TLS, cadrage par
  comptage d'octets), OTLP (gRPC ou HTTP). Chacune a son filtre de niveau, son format et
  un tampon borné ; une sortie pleine abandonne et compte, sans ralentir les requêtes ni
  les autres sorties. L'audit a ses propres sorties d'export (syslog, OTLP, fichier),
  alimentées par le relais (L3), jamais par abandon.
- Pourquoi : demande de l'utilisateur (plusieurs sorties, coût limité) ; ANSSI R16, R17 ;
  RFC 5425.
- Rejeté : UDP (aucune garantie) ; formateur syslog de bibliothèque sans cadrage ni TLS
  (voir les pièges de la note).
- Décidé par : utilisateur (besoin) ; délégué (liste, 2026-10-17)

## L8 — Format et vocabulaire
- Décision : chaque événement a un **nom stable** (`<domaine>.<objet>.<action>`, par
  exemple `authn.login.failed`), dérivé du vocabulaire OWASP, un message et des champs
  structurés. Le catalogue des événements d'audit est généré depuis le code et publié,
  comme celui des règles (`docs/spec/regles-noyau.md`). Une correspondance vers OCSF et
  ECS est documentée et appliquée à l'export (collecteur), pas dans le cœur.
- Pourquoi : OpenTelemetry n'a pas encore de convention d'audit ; OCSF est la cible SIEM
  la plus répandue, mais elle évolue vite.
- Décidé par : délégué (2026-10-17)

## L9 — OpenTelemetry
- Décision : traces, métriques et logs techniques en OTLP ; variables `OTEL_*` standard
  respectées ; attributs de ressource `service.name`, `service.version`,
  `service.instance.id` ; conventions sémantiques HTTP et base de données, sans les
  paramètres des requêtes SQL. Le contexte W3C entrant sert à la corrélation, mais son
  drapeau d'échantillonnage n'est suivi que pour les appelants de confiance configurés ;
  le Baggage entrant est ignoré.
- Pourquoi : D35 ; W3C Trace Context §7.2 et Baggage §4.1.
- Décidé par : D35 (OTLP) ; délégué (détails, 2026-10-17)

## L10 — Données sensibles
- Décision : jamais de secret dans aucune sortie (type `Secret` à affichage masqué, liste
  de champs masqués) ; dans les logs techniques, les personnes ne sont désignées que par
  leur identifiant interne, sans adresse IP ; les données personnelles (IP, agent
  utilisateur) ne figurent que dans l'audit. Tout texte non fiable est neutralisé
  (caractères de contrôle) et borné.
- Pourquoi : CWE-117, CWE-532, ASVS 16.2.5 et 16.4.1, C4.
- Décidé par : délégué (2026-10-17)

## L11 — Rétention de l'audit
- Décision : `audit_event` est partitionné par mois ; la rétention par défaut est
  d'**un an**, configurable ; une partition n'est supprimée que si tous ses événements
  sont scellés et exportés vers toutes les sorties d'audit. Les changesets et
  l'historique des objets ne sont pas concernés (conservés sans limite, C4).
- Pourquoi : CNIL 2021-122 (6 mois à 1 an), ANSSI R24, R25 ; l'export garantit la
  conservation à plus long terme hors de kartenn si besoin.
- Point d'attention : précise C4 sans le contredire (C4 vise l'historique des objets).
- Décidé par : délégué (2026-10-17), confirmé par l'utilisateur (2026-10-17) ; durée par criticité : L22

## L12 — Changement de niveau à chaud
- Décision : le filtre de chaque sortie technique est modifiable à chaud par une
  opération d'administration authentifiée (permission dédiée), journalisée dans l'audit,
  qui revient d'elle-même à la configuration après un délai (1 h par défaut). Aucun
  réglage ne peut réduire le journal d'audit.
- Pourquoi : NIST AU-12(3) ; un niveau DEBUG oublié est une fuite et un coût.
- Décidé par : délégué (2026-10-17)

## L13 — Surveillance de la journalisation
- Décision : métriques des événements abandonnés par sortie, du retard d'export de
  l'audit, des échecs d'écriture d'audit, de l'écart d'horloge avec PostgreSQL ;
  avertissements limités en débit ; événement d'audit quand une sortie est désactivée ou
  inaccessible. Les échecs d'authentification répétés d'une même source sont agrégés
  (un événement par minute avec leur nombre exact) pour qu'une attaque ne sature pas
  l'audit.
- Pourquoi : NIST AU-5, OWASP `sys_monitor_disabled`, ANSSI R12, R22.
- Décidé par : délégué (2026-10-17)

## L14 — Horloge
- Décision : kartenn compare son horloge à celle de PostgreSQL au démarrage puis
  périodiquement ; au-delà d'une seconde d'écart, avertissement et métrique.
- Pourquoi : la synchronisation NTP elle-même relève de l'hôte, mais un écart entre
  instances et base fausserait l'ordre des événements (ISO 27001 A.8.17, ANSSI R4, R5,
  INV-12).
- Décidé par : délégué (2026-10-17)

## L15 — Interfaces et adaptateurs
- Décision : les crates `kartenn-core` et `kartenn-service` ne dépendent que de **ports** :
  `AuditJournal` (enregistrer un événement, dans une transaction ou seul) et la façade
  `tracing` pour les logs techniques. Les sorties (stdout, fichier, journald, syslog,
  OTLP) et les exporteurs d'audit sont des **adaptateurs**, assemblés par
  `kartenn-server` selon la configuration ; chaque port a un adaptateur en mémoire pour
  les tests.
- Pourquoi : demande de l'utilisateur ; ADR 0001.
- Décidé par : utilisateur

## L16 — Configuration
- Décision : sorties déclarées dans le fichier de configuration et les variables
  d'environnement (D35, 12-factor) ; une sortie inconnue ou invalide empêche le
  démarrage ; sans configuration, une seule sortie : JSON sur la sortie standard, niveau
  INFO.
- Pourquoi : ASVS 16.2.3 (destinations inventoriées) ; un défaut sûr et compatible avec
  les conteneurs.
- Décidé par : délégué (2026-10-17)

## L17 — Environnement de développement
- Décision : `devenv up` lance un collecteur OpenTelemetry (service natif de devenv, build
  contrib) et **OpenObserve** comme console : OTLP sur 4317/4318 et syslog RFC 5424 (TCP,
  comptage d'octets) sur 5514 vers le collecteur, qui transmet logs, traces et métriques
  à OpenObserve (interface sur 5080).
- Pourquoi : une seule interface pour les trois signaux, dans nixpkgs, sous AGPL-3.0
  comme kartenn, en un binaire, sans moteur de conteneurs. Jaeger v2, même récent, reste
  limité aux traces dans son interface (il reçoit l'OTLP, mais n'affiche pas de logs).
- Rejeté : Jaeger (traces seulement ; d'abord retenu, commit 659913f, remplacé le
  2026-10-17 à la demande de l'utilisateur) ; tableau de bord Aspire (conteneur ou .NET) ;
  SigNoz (ClickHouse en plus, absent de nixpkgs) ; pile Grafana LGTM (cinq composants).
- Décidé par : utilisateur (une console qui affiche tout) ; délégué (OpenObserve,
  2026-10-17) — fait (commit 170387a)

## L18 — Rapport avec D35
- D35 prévoyait « logs JSON via OpenTelemetry ». La 012 le conserve et l'étend (sorties
  multiples, syslog, journald). Ce n'est pas une contradiction, mais une précision à
  reporter dans D35 si la spec est approuvée.
- Décidé par : délégué (2026-10-17)

## L19 — Syslog sans numéro d'entreprise IANA
- Constat : les projets libres qui émettent du RFC 5424 sur le réseau ne déclarent en
  général pas de données structurées privées. Keycloak, par exemple, émet du RFC 5424 en
  TCP, UDP ou TLS, avec comptage d'octets en TCP, et met l'événement en JSON (ou ECS)
  dans MSG, sans SD-ID privé. D'autres serveurs se contentent du `syslog(3)` local
  (RFC 3164), dont certains coffres-forts de secrets pour leur audit. Ceux qui ont des
  SD-ID privés utilisent le numéro d'entreprise de leur éditeur.
- Décision : comme Keycloak : RFC 5424, champs d'en-tête renseignés (`APP-NAME`,
  `PROCID`, `MSGID` = nom de l'événement), données structurées limitées aux SD-ID
  enregistrés à l'IANA par la RFC 5424 §7 (`meta` avec `sequenceId`, `timeQuality`,
  `origin`), qui ne demandent aucun numéro, et l'événement complet en JSON dans MSG. Un
  SD-ID privé `kartenn@<PEN>` pourra s'ajouter si un numéro est obtenu, sans rien changer
  d'autre.
- Pourquoi : conforme à la RFC 5424 sans démarche préalable ; les collecteurs (rsyslog,
  le récepteur syslog d'OpenTelemetry) analysent le JSON de MSG ; vérifié dans
  l'environnement de développement (en-tête et `meta` extraits par le collecteur).
- Rejeté : SD-ID privé avec le numéro réservé à la documentation (RFC 5612) en
  production ; attendre un numéro IANA.
- Décidé par : délégué (2026-10-17), à la question de l'utilisateur

## L20 — Emplacement du journal d'audit
- Décision : dans la base PostgreSQL de kartenn, schéma dédié `audit` : `audit_event`
  (partitions mensuelles), `audit_seal` (sceaux), `audit_delivery` (livraisons par
  sortie). Le rôle applicatif n'y a que `INSERT` et `SELECT` ; la purge et la création
  des partitions passent par le rôle propriétaire (job de maintenance, comme les
  partitions d'historique de la 004). Les sauvegardes de la base l'incluent. Hors de
  kartenn, les copies exportées (collecteur, syslog, SIEM) relèvent de la rétention de
  l'exploitant, qui peut être plus longue.
- Pourquoi : même transaction que l'action (L3) ; un seul système à sauvegarder ; droits
  séparés par schéma. Une base distincte rendrait l'écriture atomique impossible sans
  transaction distribuée.
- Option : un tablespace dédié pour isoler le volume (à mesurer en conception).
- Décidé par : délégué (2026-10-17), rétention d'un an confirmée par l'utilisateur

## L21 — Ordre de réalisation
- Décision : concevoir et réaliser la 012 (mécanisme) avant la 005, qui émettra les
  événements d'authentification et de droits ; les événements HTTP viendront avec la 006.
- Décidé par : utilisateur (2026-10-17)

## L22 — Criticité et expiration ; toutes les lectures auditées
- Décision : chaque événement d'audit porte, en plus de sa **sévérité** (L6), une
  **criticité** (`low`, `normal`, `high`) fixée par son type dans le catalogue ; la
  rétention se configure par criticité. Défauts : `low` 90 jours, `normal` 1 an, `high`
  3 ans. Toutes les lectures de l'API sont auditées (un événement par requête : critères
  et objets renvoyés, jamais les valeurs), en criticité `low` ; les lectures de
  propriétés ou d'exports sensibles, la consultation de l'audit, les refus de droits et
  l'administration sont `high`.
- Pourquoi : réponse de l'utilisateur (« expiration selon la criticité… criticité avec
  des périodes d'expiration configurables ») ; la sévérité dit la gravité d'un fait,
  la criticité dit combien de temps il faut pouvoir le prouver. Un an reste la valeur
  courante (L11, confirmée) ; 3 ans est le maximum que la CNIL admet dans certains cas.
- Conséquence : une insertion par requête de lecture ; partitions par criticité et par
  mois ; un événement `low` qui n'a pas été exporté n'est pas purgé (REQ-012-16).
- Rejeté : lectures sensibles seulement (proposé, non retenu) ; une rétention unique.
- Décidé par : utilisateur (principe, 2026-10-17) ; délégué (niveaux et défauts) — à
  confirmer à la relecture de la conception

## L23 — Signature des sceaux facultative
- Décision : la signature Ed25519 des sceaux est facultative ; la chaîne de hachés,
  exportée hors du système, suffit à détecter une altération.
- Décidé par : utilisateur (2026-10-17)

## L24 — Bibliothèque syslog indépendante
- Constat (crates.io, sources lues le 2026-10-17) :

  | Crate | TLS | Cadrage TCP | Horodatage | Remarques |
  |---|---|---|---|---|
  | `fasyslog` 1.0.1 | rustls, native-tls | fin de ligne (RFC 6587 §3.4.2) | heure d'envoi | synchrone, sans BOM, sans reconnexion |
  | `syslog-rs` 6.6.1 | tokio-rustls | aucun | heure locale, à la seconde | gouvernance atypique |
  | `syslog` 7.0.0, `syslog-tls` 7.1.1 | non ; native-tls | aucun | — | — |
  | `tracing-rfc-5424` 0.2.1 | non | fin de ligne | — | GPL-3.0 |
  | `syslog_proto` 0.1.0-alpha0 | — | — | — | sans réseau, alpha |

  Aucune ne pratique le comptage d'octets qu'exige RFC 5425 §4.3 en TLS, ni n'accepte
  l'horodatage de l'événement. (Le rapport de recherche affirmait qu'aucune n'avait de
  TLS : c'était inexact.)
- Décision : une bibliothèque séparée, `syslog-sender`, dans `crates/` mais sans
  dépendance vers kartenn ni vers `tracing`, conçue pour être publiée seule :
  formatage RFC 5424 (et RFC 3164 pour le socket local), validation des champs,
  données structurées, BOM, troncature ; cadrage par comptage d'octets ou par fin de
  ligne ; transports socket local, UDP, TCP, TLS (rustls) en asynchrone tokio, avec
  reconnexion. `kartenn-telemetry` l'adapte à ses sorties.
- Licence : **MPL-2.0** (décidé par l'utilisateur, 2026-10-17), sans la mention
  « Incompatible With Secondary Licenses » : copyleft par fichier, compatible avec la
  famille GPL, donc avec kartenn (AGPL-3.0, D37, inchangée). Écartées : GPLv3 (contraint
  tout binaire qui l'utilise, l'édition de liens étant statique en Rust), LGPLv3
  (exigence de réédition des liens inapplicable sans bibliothèque partagée ni ABI stable,
  et génériques compilés chez l'appelant), AGPLv3 (encore plus contraignante), MIT ou
  Apache-2.0 (sans copyleft).
- Rejeté : contribuer le cadrage à `fasyslog` (modèle synchrone, horodatage à l'envoi :
  changements d'API profonds ; reste possible plus tard) ; formateur interne à
  `kartenn-telemetry` (non réutilisable).
- Décidé par : utilisateur (« créer une librairie à part ») ; délégué (périmètre, nom,
  2026-10-17)

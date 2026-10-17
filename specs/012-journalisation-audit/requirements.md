---
status: implemented
updated: 2026-10-17
---

# 012 — Journalisation, observabilité et audit : exigences

## Résumé

kartenn sert des équipes sécurité : chaque action doit y être journalisée, attribuée et
vérifiable après coup. Cette spec fixe deux flux distincts. Le **journal d'audit**
couvre les événements de sécurité : il est durable, sans perte silencieuse, scellé contre
l'altération et exporté hors du système. Les **logs techniques** servent au diagnostic :
ils sont structurés, ont plusieurs sorties simultanées et ne doivent jamais ralentir le
service. S'y ajoutent les traces et métriques OpenTelemetry. Les changesets restent la
piste d'audit détaillée des écritures (D11, spec 004) ; le journal d'audit les référence.

Décisions : [`decisions.md`](decisions.md) (L1–L23) ; D8, D9, D11, D13, D35, D36
([`../001-cadrage/decisions.md`](../001-cadrage/decisions.md)) ; C4 de la
[004](../004-changesets-bitemporel/decisions.md). Sources :
[`docs/reference/journalisation-audit.md`](../../docs/reference/journalisation-audit.md).

### Vocabulaire

- **Événement d'audit** : enregistrement d'une action ou d'un fait de sécurité, à schéma
  fixe, conservé par kartenn (`audit_event`) et exporté.
- **Log technique** : message de diagnostic du serveur, à niveau réglable.
- **Sortie** : destination d'un flux (sortie standard, fichier, journald, syslog, OTLP).
- **Acteur** : personne, compte de service ou `system` à l'origine d'une action.
- **Sceau** : empreinte d'un lot d'événements d'audit, chaînée au sceau précédent.
- **Criticité** : `low`, `normal` ou `high`, fixée par le type d'événement ; elle
  détermine la durée de conservation, indépendamment de la sévérité.

### Conventions

- Forme EARS en français, comme dans
  [`002/requirements.md`](../002-noyau-adressage/requirements.md#conventions).
- Les références entre crochets renvoient aux sources de la note de référence :
  [ASVS 16.x], [AU-x] (NIST SP 800-53), [ANSSI Rx], [RFC 5424 §x], [OWASP].

## Exigences

### A. Deux flux

#### REQ-012-1 — Journal d'audit et logs techniques séparés
kartenn doit produire deux flux distincts, chacun avec ses propres sorties : le journal
d'audit et les logs techniques. Aucun réglage des logs techniques (niveau, filtre,
échantillonnage, désactivation d'une sortie) ne doit modifier le journal d'audit. [L1,
AU-12(3)]

Acceptation :
- Le niveau technique passé à ERROR, un refus d'autorisation produit toujours son
  événement d'audit.

#### REQ-012-2 — Ports et adaptateurs
kartenn doit accéder à l'audit par un port (`AuditJournal`) et aux logs techniques par la
façade `tracing` ; chaque sortie et chaque exporteur doit être un adaptateur choisi par
la configuration, avec un adaptateur en mémoire pour les tests. [L15, ADR 0001]

Acceptation :
- `kartenn-core` et `kartenn-service` ne dépendent d'aucune crate de sortie (syslog,
  journald, OpenTelemetry) : vérifié sur le graphe des dépendances.
- Les tests du service vérifient les événements d'audit émis par l'adaptateur en mémoire.

### B. Contenu commun

#### REQ-012-3 — Horodatage
kartenn doit horodater chaque événement (audit et technique) en UTC, au format RFC 3339
avec `Z` et six décimales (`2026-10-17T08:30:12.123456Z`), sans seconde intercalaire. Un
événement d'audit doit porter aussi l'heure de son enregistrement par la base. [RFC 5424
§6.2.3, RFC 9557, ASVS 16.2.2, AU-8, ANSSI R3]

Acceptation :
- Sur chaque sortie, l'horodatage correspond à l'expression
  `^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}\.\d{6}Z$` (ou au champ natif : `timeUnixNano` en
  OTLP, `__REALTIME_TIMESTAMP` en journald).

#### REQ-012-4 — Sévérité
kartenn doit attribuer à chaque événement une sévérité parmi TRACE, DEBUG, INFO, WARN,
ERROR, et la traduire sur chaque sortie selon cette table unique : [L6, annexe B du
modèle de log OpenTelemetry, RFC 5424 §6.2.1]

| kartenn | syslog (code, nom) | OpenTelemetry (`SeverityNumber`, `SeverityText`) | journald `PRIORITY` |
|---|---|---|---|
| ERROR | 3, Error | 17, ERROR | 3 |
| WARN | 4, Warning | 13, WARN | 4 |
| INFO | 6, Informational | 9, INFO | 6 |
| DEBUG | 7, Debug | 5, DEBUG | 7 |
| TRACE | 7, Debug | 1, TRACE | 7 |

Acceptation :
- Un même événement WARN donne `<PRI>` de sévérité 4 en syslog, `SeverityNumber` 13 en
  OTLP et `PRIORITY=4` en journald.

#### REQ-012-5 — Structure et nom d'événement
kartenn doit émettre chaque événement sous forme structurée : un nom stable
(`<domaine>.<objet>.<action>`, en minuscules), un message lisible et des champs typés ;
aucune information ne doit n'exister que dans le texte du message. [L8, ASVS 16.2.4,
OpenTelemetry `EventName`]

Acceptation :
- Les sorties JSON, OTLP, journald et syslog portent le nom de l'événement dans un champ
  dédié (`event.name`, `EventName`, `MESSAGE_ID` et `MSGID` respectivement).

#### REQ-012-6 — Corrélation
kartenn doit attacher à chaque événement émis pendant une requête l'identifiant de trace
et de span W3C et un identifiant de requête, et renvoyer ce dernier au client (en-tête et
corps des erreurs). [L9, W3C Trace Context, ASVS 16.2.1]

Acceptation :
- Pour une requête refusée, la réponse `problem+json`, le log technique et l'événement
  d'audit portent le même identifiant de requête et le même `trace_id`.

#### REQ-012-7 — Contexte de trace entrant
kartenn doit ignorer un `traceparent` invalide, ne suivre le drapeau d'échantillonnage
entrant que pour les appelants de confiance configurés, et ignorer le Baggage entrant.
[W3C Trace Context §3.2.2.3 et §7.2, Baggage §4.1]

Acceptation :
- Un `traceparent` malformé démarre une nouvelle trace ; un `sampled=1` d'un appelant non
  configuré ne force pas l'échantillonnage.

### C. Journal d'audit

#### REQ-012-8 — Événements audités
kartenn doit enregistrer au moins : [ASVS 16.3.1 à 16.3.4, AU-2, OWASP Logging
Vocabulary, L2]

| Catégorie | Événements |
|---|---|
| Authentification | réussite et échec (avec la méthode), verrouillage, réutilisation de jeton refusée, expiration, déconnexion |
| Autorisation | **tout** refus de droit ; validation et rejet d'un changeset en attente |
| Administration | rôles, liaisons et permissions modifiés ; comptes de service créés, renouvelés ou révoqués ; politiques d'approbation ; déclarations de propriétés |
| Écritures | chaque changement d'état d'un changeset (soumis, appliqué, refusé, en attente, retiré, rejeté, expiré), avec sa référence |
| Exploitation | démarrage, arrêt, migration, changement de configuration ou de niveau de log, sortie désactivée ou inaccessible, scellement, échec de vérification de la chaîne |
| Données | toute requête de lecture (REQ-012-18) ; consultation et export du journal d'audit ; exports en masse ; pseudonymisation d'une personne |
| Abus | dépassement de limite de débit ; entrée refusée par validation à la frontière (006) |

Acceptation :
- Un test par ligne du tableau vérifie l'événement émis, son nom et son acteur.

#### REQ-012-9 — Attribution
kartenn doit attribuer chaque événement d'audit à un acteur : personne (identifiant
interne stable, C4), compte de service ou `system`, avec la méthode d'authentification et
l'émetteur. Si l'acteur n'est pas établi (échec d'authentification), kartenn doit
enregistrer l'identifiant revendiqué sous forme d'empreinte HMAC, jamais en clair. [L5,
D11, AU-3]

Acceptation :
- Aucun événement d'audit n'a d'acteur vide, sauf `authn.*.failed`, qui porte
  l'empreinte.
- Après pseudonymisation d'une personne (REQ-004-33), ses événements d'audit restent
  attribués à son identifiant interne et ne contiennent plus son nom ni son courriel.

#### REQ-012-10 — Contenu d'un événement d'audit
kartenn doit inclure dans chaque événement d'audit : identifiant unique, numéro de
séquence, nom, sévérité, horodatages (REQ-012-3), acteur, source (adresse IP du client
derrière les mandataires de confiance, agent utilisateur borné), action, cible (type et
identifiant, jamais le contenu lu), criticité, résultat (`success`, `failure`, `denied`) et raison
codée, motif quand l'action en a un, identifiants de trace et de requête, version et
instance du serveur, référence du changeset s'il y a lieu. [ASVS 16.2.1, AU-3, CADF]

Acceptation :
- Chaque événement du catalogue (REQ-012-30) valide un schéma JSON publié.

#### REQ-012-11 — Durabilité, échec fermé
kartenn doit enregistrer un événement d'audit dans sa base PostgreSQL (schéma `audit`,
L20) dans la transaction de
l'action qu'il décrit quand elle écrit, sinon avant de répondre. Si l'enregistrement
échoue, alors kartenn doit faire échouer l'action (réponse 503, aucun effet) et produire
un log technique ERROR. [L3, AU-5(4), OWASP A09]

Acceptation :
- Avec l'insertion d'audit rendue impossible (droit retiré dans un test), un changeset
  n'est pas appliqué et une authentification réussie n'ouvre pas de session.
- Aucun événement d'audit n'est jamais abandonné par manque de place dans un tampon.

#### REQ-012-12 — Ajout seul
kartenn doit empêcher la modification et la suppression des événements d'audit par le
rôle applicatif ; seule la purge de rétention (REQ-012-16), exécutée avec le rôle
propriétaire, peut en retirer. [AU-9, ASVS 16.4.2, ANSSI R26]

Acceptation :
- `UPDATE` et `DELETE` sur `audit_event` sont refusés au rôle applicatif (test sous
  `SET ROLE`).

#### REQ-012-13 — Scellement
kartenn doit sceller les événements d'audit validés, au plus 60 s (configurable) après
leur enregistrement, en lots : chaque sceau contient l'empreinte SHA-256 du lot et celle
du sceau précédent, et sa signature Ed25519 si une clé est configurée. Chaque sceau doit
être exporté comme événement (`audit.seal.created`). [L4, AU-9(3), AU-10, OWASP A09]

Acceptation :
- Aucun événement validé ne reste non scellé plus de 60 s en fonctionnement normal, y
  compris un événement d'une transaction longue validée après un lot plus récent.

#### REQ-012-14 — Vérification de l'intégrité
kartenn doit fournir une commande qui vérifie la chaîne des sceaux et le contenu de
chaque lot, et qui désigne le premier lot altéré. Quand la vérification planifiée échoue,
kartenn doit émettre un événement d'audit ERROR. [ASVS 16.4.2]

Acceptation :
- Un événement modifié en base par le rôle propriétaire est détecté, avec son lot.
- Une ligne supprimée ou insérée dans un lot scellé est détectée.

#### REQ-012-15 — Export de l'audit
kartenn doit exporter chaque événement d'audit au moins une fois, dans l'ordre de sa
séquence, vers chaque sortie d'audit configurée (syslog RFC 5424 en TLS ou TCP, OTLP
logs, fichier JSON), en enregistrant les livraisons par sortie, et reprendre sans trou
après une interruption. [L3, ASVS 16.4.3, AU-9(2), ANSSI R9, R10, R14, R16]

Acceptation :
- Collecteur arrêté 5 min puis relancé : tous les événements de la période arrivent,
  chacun au moins une fois, avec un numéro de séquence permettant le dédoublonnage.
- Une sortie d'audit ne passe pas par un tampon qui abandonne (le processeur par lots
  OTLP est remplacé par un envoi acquitté avec reprise).

#### REQ-012-16 — Rétention
kartenn doit conserver les événements d'audit, dans sa base (schéma `audit`, L20), pendant
une durée configurable par criticité (par défaut `low` 90 jours, `normal` 1 an, `high`
3 ans), puis les purger par mois entiers et par criticité, à condition que tous soient scellés et
exportés vers chaque sortie d'audit ; les sceaux restent, pour que la chaîne des mois
conservés reste vérifiable. Les changesets et l'historique des objets ne sont pas
concernés (C4). [L11, AU-11, ANSSI R24, R25, CNIL 2021-122]

Acceptation :
- Une partition dont un événement n'a pas été exporté n'est pas purgée ; la vérification
  de la chaîne passe après une purge.

#### REQ-012-17 — Consultation de l'audit
kartenn doit réserver la consultation et l'export du journal d'audit à une permission
dédiée, et journaliser chaque consultation (critères de recherche, nombre de résultats).
[ASVS 16.4.2, AU-9(4), OWASP, ANSSI R27]

Acceptation :
- Sans la permission, la recherche est refusée et le refus est audité ; avec elle,
  l'événement `audit.events.read` est enregistré.

#### REQ-012-18 — Lectures
kartenn doit enregistrer un événement d'audit pour chaque requête de lecture de l'API
(acteur, critères, identifiants des objets renvoyés, jamais les valeurs), en criticité
`low`. Là où une propriété ou un export est déclaré sensible, la lecture de sa valeur doit
produire un événement de criticité `high` qui nomme la propriété. [L22, ASVS 16.3.2 (L3),
OWASP `sensitive_read`]

Acceptation :
- Une page de 100 adresses produit un événement `data.objects.read` (`low`) listant
  leurs identifiants ; la lecture d'une propriété sensible produit en plus
  `data.sensitive.read` (`high`).
- Le surcoût de cet événement reste dans le budget de REQ-012-23.

#### REQ-012-19 — Agrégation des échecs répétés
Quand une même source échoue à s'authentifier de façon répétée, kartenn doit agréger ces
échecs en un événement par minute et par (source, identifiant revendiqué), avec leur
nombre exact, au-delà d'un seuil configurable (10 par défaut). [L13]

Acceptation :
- 1 000 échecs en une minute depuis une source produisent au plus 11 événements, dont le
  total compté est 1 000.

### D. Logs techniques et télémétrie

#### REQ-012-20 — Sorties multiples
kartenn doit permettre d'activer simultanément plusieurs sorties de logs techniques,
chacune avec son propre filtre (niveau global et par module) et son format : sortie
standard (texte lisible ou JSON par ligne), fichier JSON, journald (protocole natif),
syslog RFC 5424 (socket local, TCP ou TLS avec cadrage par comptage d'octets), OTLP
(gRPC ou HTTP). [L7, RFC 5425, RFC 6587 §3.4.1, ANSSI R16, R17]

Acceptation :
- Avec les cinq sorties actives, un même événement INFO arrive sur chacune avec le même
  nom, la même sévérité (REQ-012-4) et le même horodatage.
- Une sortie à WARN et une autre à DEBUG reçoivent chacune ce que leur filtre laisse
  passer.

#### REQ-012-21 — Conformité syslog
kartenn doit émettre en syslog des messages RFC 5424 : version 1, `APP-NAME` `kartenn`,
`PROCID` égal au PID, `MSGID` égal au nom de l'événement, données structurées limitées
aux SD-ID enregistrés (`meta` avec `sequenceId`, `timeQuality`), événement complet en
JSON dans MSG, en UTF-8 précédé du BOM ; en TLS, TLS 1.2 au moins, 1.3 préféré, sans
0-RTT. Là où un numéro d'entreprise IANA est configuré, kartenn peut ajouter un SD-ID
privé `kartenn@<PEN>` (valeurs échappées, RFC 5424 §6.3.3). [L19 ; RFC 5424 §6, §6.3,
§7 ; RFC 9662]

Acceptation :
- Les messages émis sont acceptés par un analyseur RFC 5424 strict et par le récepteur
  syslog du collecteur OpenTelemetry (comptage d'octets activé).

#### REQ-012-22 — Non-blocage et pertes comptées
kartenn doit écrire chaque sortie technique depuis un tampon borné, hors du chemin des
requêtes ; si le tampon est plein, kartenn doit abandonner le nouvel événement, le
compter par sortie et ne ralentir ni les requêtes ni les autres sorties. [L7]

Acceptation :
- Une sortie syslog TCP dont le récepteur ne lit plus pendant 60 s : la mémoire des
  tampons reste sous sa borne configurée (64 Mio au total par défaut), les autres
  sorties reçoivent tout, et le compteur d'abandons de la sortie bloquée augmente.

#### REQ-012-23 — Coût borné
kartenn doit tenir les budgets de D14 (INV-30 à 36) avec la configuration de référence :
JSON sur la sortie standard, syslog TCP et OTLP actifs au niveau INFO, traces
échantillonnées à 100 %, audit actif. Le surcoût sur le p95 de chaque budget, par rapport
à aucune sortie, doit rester sous 5 %. Une sortie bloquée (REQ-012-22) ne doit pas
dégrader un p95 de plus de 10 %. [L7]

Acceptation :
- Mesuré par le banc `bench-core` étendu, dans les trois configurations.

#### REQ-012-24 — OpenTelemetry
kartenn doit exporter en OTLP (gRPC ou HTTP) ses traces, ses métriques et ses logs
techniques, avec les attributs de ressource `service.name`, `service.version` et
`service.instance.id`, les conventions sémantiques HTTP et base de données (requête SQL
sans paramètres, `db.query.parameter.*` jamais collecté), et respecter les variables
d'environnement `OTEL_*` standard. Chaque événement ne doit être exporté qu'une fois
(pas de doublon entre événement de span et enregistrement de log). [L9, D35]

Acceptation :
- Dans le collecteur de développement, une requête produit une trace HTTP avec ses spans
  de base de données, et ses logs portent le même `trace_id` ; chaque log n'apparaît
  qu'une fois.

#### REQ-012-25 — Métriques
kartenn doit exposer au moins : requêtes HTTP (débit, erreurs, durée), changesets par
état et durée du pipeline, pool de connexions, événements abandonnés par sortie, retard
d'export de l'audit par sortie (âge du plus ancien non livré), événements non scellés,
échecs d'écriture d'audit, écart d'horloge avec PostgreSQL. [D35, L13]

Acceptation :
- Les métriques sont présentes dans l'export OTLP avec leurs unités.

#### REQ-012-26 — Surveillance de la journalisation
Si une sortie abandonne des événements, devient inaccessible ou est désactivée, alors
kartenn doit émettre un log technique WARN (au plus un par minute et par sortie) et, pour
une sortie d'audit ou une désactivation, un événement d'audit (`sys.output.unavailable`,
`sys.output.disabled`). [AU-5(2), OWASP `sys_monitor_disabled`, ANSSI R12]

Acceptation :
- Couper le récepteur d'une sortie d'audit produit l'événement et fait croître la
  métrique de retard ; le rétablir les fait cesser.

#### REQ-012-27 — Changement de niveau à chaud
kartenn doit permettre de modifier à chaud le filtre d'une sortie technique, par une
opération d'administration soumise à une permission dédiée, journalisée dans l'audit, qui
revient d'elle-même à la configuration après un délai (1 h par défaut, 24 h au plus).
[L12, AU-12(3)]

Acceptation :
- Un passage à DEBUG est audité, effectif sans redémarrage sur toutes les instances
  visées, et annulé à l'échéance.

### E. Sécurité du contenu

#### REQ-012-28 — Secrets
kartenn ne doit écrire dans aucune sortie (audit, logs, traces, métriques) de mot de
passe, jeton, en-tête `Authorization`, cookie, secret de compte de service, clé, chaîne de
connexion ni paramètre de requête SQL. [L10, CWE-532, ASVS 16.2.5, OWASP]

Acceptation :
- Un test de bout en bout fait circuler des secrets témoins (authentification Basic,
  jeton, chaîne de connexion) et ne les retrouve sur aucune sortie, y compris dans les
  messages d'erreur et les traces de panique.
- Les types qui portent un secret ont un affichage masqué (`Debug`, `Display`).

#### REQ-012-29 — Données personnelles et injection
kartenn doit limiter les données personnelles des logs techniques à l'identifiant
interne des personnes (ni nom, ni courriel, ni adresse IP) ; les réserver au journal
d'audit. kartenn doit neutraliser dans tout texte non fiable les retours à la ligne et
caractères de contrôle des sorties texte, échapper les données structurées syslog, et
borner chaque valeur (2 Kio par défaut) avec une marque de troncature. [L10, CWE-117,
ASVS 16.4.1, RFC 5424 §6.1]

Acceptation :
- Un motif de changeset contenant `\n<14>1 …` ne produit qu'une ligne et un seul message
  syslog.
- Un test de fuzzing des champs ne casse le cadrage d'aucune sortie.

### F. Exploitation

#### REQ-012-30 — Catalogue publié
kartenn doit générer depuis le code le catalogue des événements d'audit (nom, sévérité,
champs, exigences) et le publier dans `docs/spec/`, vérifié par un test comme celui des
règles ; la correspondance vers OCSF et ECS doit y figurer. [L8, ASVS 16.1.1]

Acceptation :
- Le test échoue si un événement émis par le code manque au catalogue publié.

#### REQ-012-31 — Configuration des sorties
kartenn doit lire ses sorties dans le fichier de configuration et les variables
d'environnement. Si une sortie est inconnue ou invalide, alors kartenn doit refuser de
démarrer en nommant l'erreur. Sans configuration, kartenn doit écrire les logs
techniques en JSON sur la sortie standard au niveau INFO, et l'audit en base seulement.
[L16, ASVS 16.2.3]

Acceptation :
- Une sortie `syslog` sans adresse empêche le démarrage avec un message explicite.

#### REQ-012-32 — Horloge
kartenn doit comparer son horloge à celle de PostgreSQL au démarrage puis toutes les
5 min ; au-delà d'une seconde d'écart, kartenn doit émettre un WARN et publier l'écart en
métrique. [L14, ANSSI R4, R5, ISO 27001 A.8.17]

Acceptation :
- Avec une horloge décalée simulée, le WARN et la métrique apparaissent.

#### REQ-012-33 — Erreurs et pannes
kartenn doit répondre aux clients par des erreurs génériques (RFC 9457) portant
l'identifiant de requête, le détail allant dans les logs techniques. Quand une panique
survient, kartenn doit l'écrire en ERROR avec son contexte de trace avant de libérer la
requête. [ASVS 16.5.1, 16.5.4]

Acceptation :
- Une erreur de base simulée renvoie 500 sans détail interne ; le log contient l'erreur
  et le même identifiant de requête.

#### REQ-012-34 — Arrêt propre
Quand kartenn s'arrête, il doit vider ses tampons et exporteurs dans un délai borné
(10 s par défaut), puis consigner le nombre d'événements techniques qui n'ont pas pu être
écrits. Les événements d'audit non exportés restent en base et partent au démarrage
suivant. [L3]

Acceptation :
- `SIGTERM` sous charge : les derniers logs arrivent au collecteur, et aucun événement
  d'audit n'est perdu (relais rejoué au redémarrage).

#### REQ-012-35 — Environnement de développement
Là où l'environnement de développement est utilisé, kartenn doit pouvoir exporter vers
le collecteur local (OTLP sur 127.0.0.1:4317/4318, syslog RFC 5424 sur 127.0.0.1:5514),
dont la console affiche logs, traces et métriques. [L17]

Acceptation :
- `devenv up`, puis une requête : la trace, les logs et les métriques sont visibles dans
  OpenObserve. Fait pour l'outillage (collecteur, OpenObserve, syslog vérifié), à
  compléter par l'application.

## Non-objectifs

- Un système de gestion des logs (stockage, recherche, tableaux de bord, alertes) :
  kartenn exporte vers ceux de l'exploitant (collecteur, SIEM).
- Les formats CEF et LEEF : produits au besoin par le collecteur.
- Un arbre de Merkle avec preuves d'inclusion individuelles (L4).
- La synchronisation NTP de l'hôte (L14) : seulement sa surveillance.
- Le format de l'API d'administration (niveau à chaud, consultation de l'audit) : 006 ;
  le modèle de permissions : 005.

## Questions ouvertes

- [x] ~~Numéro d'entreprise IANA~~ : non requis (L19) ; facultatif si obtenu plus tard.
- [x] ~~Rétention par défaut de l'audit~~ : un an, confirmé ; emplacement : L20.
- [x] ~~Emplacement de la clé de signature facultative des sceaux (L23)~~ : fichier PKCS#8
  PEM désigné par `audit.signing_key_file` ; clés publiques dans `audit.instance_key`.
- [x] ~~Déclarer une propriété « sensible »~~ : attribut `sensitive` de la déclaration
  (amendement de la 003, P17, REQ-003-27).
- [ ] Volume de l'audit des lectures : mesuré à 8 Kio par page de 100 objets (dont
  5,8 Kio d'index GIN par objet), soit 3,3 Gio par jour à 5 pages par seconde et
  environ 300 Gio pour 90 jours de `low`, contre 0,9 Gio par jour estimé. Leviers :
  borner les identifiants enregistrés par événement, n'enregistrer que les critères,
  retirer l'index par objet de la criticité `low` (la recherche « qui a lu cet objet »
  devient un parcours), rétention `low` plus courte. (utilisateur, avant la mise en
  production)
- [ ] Changeset unitaire séquentiel plus lent de 41 à 57 % en moyenne avec des sorties
  (2,5 → 3,5 ms), alors que le surcoût modélisé du chemin de la requête est de 1,4 % :
  cause à établir sur un poste calme (piste : validations des tâches de fond sur le
  WAL). Le budget INV-34 (200 ms) reste tenu. (Claude, avant la mise en production)
- [x] ~~Ordre de réalisation~~ : la 012 avant la 005 (L21).

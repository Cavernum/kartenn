# 010 — Politique de flux : journal des décisions

Rédigé le 2026-10-20 sans entretien : l'utilisateur a délégué les choix de cette spec
(« prends les décisions qui s'imposent »), en réservant les **règles d'inférence** à une
session dédiée (S18, [`inference-questions.md`](inference-questions.md)). Chaque décision
retient la recommandation la plus standard ; elles restent à relire avant
l'implémentation (v0.2, D31).

Cadre : D19 à D30, § « Approche — module Politique de flux » et § « Noyau ou module » de
[`../001-cadrage/decisions.md`](../001-cadrage/decisions.md) ; propriétés héritables
(spec 003, ADR 0016) ; changesets, bitemporel, approbations (004) ; RBAC (005) ; API
(006) ; contrat de module (ADR 0026, en cours d'écriture : on suppose le contrat esquissé
dans [`docs/spec/architecture.md`](../../docs/spec/architecture.md) § Modularité) ;
Machines et interfaces (009, en cours). Note de référence :
[`services-transports.md`](../../docs/reference/services-transports.md).

## Synthèse
- S1 périmètre : vocabulaires, catalogue, garde-fous, matrice, conformité, différentiel
  de politique, audit ; inférence isolée (S18).
- S2 environnement : valeur de la liste fermée `policy.environment` (ensemble, héritable
  sur l'arbre d'adressage) ; attributs (libellé, transverse) dans le module.
- S3 porteurs : domaine, préfixes, adresses ; pas les plages ; machine par une propriété
  distincte non héritable.
- S4 types de flux : objets du module, liste administrable.
- S5 services : nom, un type de flux, couples transport/ports ; registre IANA en
  référence.
- S6 garde-fous : sélecteur (services ou type de flux) × paire d'environnements
  facultative ; « service interdit » = garde-fou sans paire.
- S7 matrice : cellules d'autorisation orientées, sans joker.
- S8 évaluation : non classifié, garde-fous, matrice ; refus par défaut ; élément
  décisif cité.
- S9 requête de conformité : extrémités adresse, préfixe, interface ou machine ; par
  service, port brut ou type de flux ; verdict par paire et synthèse.
- S10 cohérence des machines : anomalie si les adresses divergent, sauf rôle
  d'interface.
- S11 différentiel de politique : autorisations (env, env, service) qui basculent, en
  plus du différentiel des propriétés ; flux concrets par la 011.
- S12 approbation : déclencheurs fournis par le module, posés par l'administrateur.
- S13 classification obligatoire : obligation 003 par domaine, modifiable à
  l'exécution.
- S14 audit de masse : rapport calculé par une tâche de fond.
- S15 droits : administration globale de la politique ; un verdict exige la lecture
  des deux extrémités.
- S16 temps : tout objet de politique bitemporel, écrit par changeset.
- S17 sérialisation : une file d'écriture propre à la politique, hors domaines.
- S18 inférence : non tranchée ; point d'intégration = source de valeurs attendues 003.
- S19 API : collections sous `/v1/policy/`, verdicts par `QUERY`.
- S20 budgets chiffrés.
- S21 tension D23 / D30 signalée.

## S1 — Périmètre
- Décision : la 010 livre les vocabulaires (environnements, types de flux), le catalogue
  de services (D29), les garde-fous et la matrice (D28), la requête de conformité
  expliquée (D20 étape 1, D22), la cohérence des machines (D22), le différentiel de
  politique (D25 mitigation 2), les déclencheurs d'approbation (D25), l'audit de masse
  (D25 mitigation 3, D27) et l'API. Les règles d'inférence (D23, D24) sont posées comme
  questions (S18) ; le registre de flux est la 011 (D30).
- Pourquoi : tout ce qui fonde un verdict repose sur le **déclaré** (D23) ; l'inférence
  ne produit qu'anomalies et suggestions, elle peut donc arriver après sans refonte.
- Rejeté : attendre l'arbitrage de l'inférence pour écrire la 010 (bloquerait le
  garde-fou D31 pour une partie non structurante pour le noyau).
- Décidé par : Claude (délégation de l'utilisateur, 2026-10-20)

## S2 — Modèle de l'environnement
- Décision : l'environnement d'un nœud est la propriété **`policy.environment`**,
  déclarée par le module : liste fermée, **ensemble** (D25), **héritable sur l'arbre
  d'adressage** (D21, REQ-003-11), remplacement de l'ensemble hérité (D26,
  REQ-003-12). Chaque valeur de la liste porte, dans une table du module, un libellé, une
  description et un drapeau **transverse** (D25 mitigation 3). Créer un environnement
  crée la valeur de liste et ses attributs dans la même opération.
- Pourquoi : réutilise résolution matérialisée, explication, filtrage et différentiel de
  la 003 sans rien dupliquer ; l'exemple de REQ-003-5 (`preprod`) reste vrai.
- Rejeté : environnement comme objet du module visé par une propriété de type
  référence (l'exemple 003 et le filtrage par clé de liste changent, sans gain) ;
  étiquettes libres (D21).
- Décidé par : Claude (délégation de l'utilisateur, 2026-10-20)

## S3 — Porteurs
- Décision : `policy.environment` est portée par les domaines, conteneurs, réseaux et
  adresses ; **pas** par les plages (une adresse n'hérite pas d'une plage,
  REQ-003-11). Une machine déclare son environnement par une propriété distincte,
  **`policy.declared_environment`** (ensemble, non héritable, portée par les machines de
  la 009), confrontée à ses adresses (S10) mais jamais utilisée dans un verdict (D22).
- Pourquoi : D22 (la machine n'est pas un nœud de l'arbre) et P16 (un porteur d'une
  propriété héritable appartient à son arbre).
- Rejeté : même clé portée par les machines (contredit P16) ; plages porteuses
  (deux sources d'héritage pour une adresse).
- Décidé par : Claude (délégation de l'utilisateur, 2026-10-20)

## S4 — Types de flux
- Décision : un type de flux est un objet du module (identifiant court, libellé,
  description), liste administrable et versionnée ; aucun type livré n'est imposé, un
  jeu d'exemple (`database`, `application`, `file`, `admin`) est proposé à
  l'installation.
- Pourquoi : D21 cite ces quatre types comme pratique actuelle, pas comme liste fermée
  du produit.
- Décidé par : Claude (délégation de l'utilisateur, 2026-10-20)

## S5 — Services nommés
- Décision : un service (D29) a un identifiant (syntaxe des noms de service RFC 6335
  §5.1, mais espace propre à kartenn), un libellé, **un seul** type de flux et une liste
  non vide de **couples** : transport `tcp`, `udp`, `sctp`, `dccp` avec ports ou plages
  1–65535 ; `icmp`, `icmpv6` avec type et code facultatifs ; `ip` avec un numéro de
  protocole 0–255. Un même port peut figurer dans plusieurs services. Le registre IANA
  est une référence d'aide à la saisie, pas le catalogue. Avertissement au-delà de 1 024
  ports dans un service.
- Pourquoi : D29 ; RFC 6335 §1 et §6, registre des numéros de protocole.
- Rejeté : catalogue = registre IANA (un port n'a pas un seul type de flux, D29) ;
  ports seuls (ICMP, ESP, GRE impossibles à décrire).
- Décidé par : Claude (délégation de l'utilisateur, 2026-10-20)

## S6 — Garde-fous
- Décision : un garde-fou a un sélecteur (un ensemble de services **ou** un type de
  flux), un environnement source et un environnement destination **facultatifs** (absent
  = tous, y compris « non classifié »), un motif obligatoire. Le « protocole interdit »
  du catalogue existant (D21-3) est un garde-fou sans environnement. Les garde-fous ne
  font que refuser ; l'ordre entre eux est sans effet.
- Pourquoi : D28 niveau 1 ; un refus n'a pas besoin de priorité.
- Rejeté : statut « interdit » sur le service (deux mécanismes de refus) ; garde-fous
  ordonnés avec exceptions (rouvrirait la matrice tri-état rejetée par D28).
- Décidé par : Claude (délégation de l'utilisateur, 2026-10-20)

## S7 — Matrice
- Décision : une **cellule** (type de flux, environnement source, environnement
  destination) vaut autorisation ; elle est orientée ; une cellule absente vaut refus.
  Pas de joker « tout environnement » : chaque autorisation est écrite.
  Unicité d'une cellule par période.
- Pourquoi : D28 niveau 2 et D25 ; sans joker, l'audit de ce qui est ouvert se lit
  directement.
- Rejeté : joker (une seule cellule ouvrirait tout un type de flux vers un environnement
  créé plus tard).
- Décidé par : Claude (délégation de l'utilisateur, 2026-10-20)

## S8 — Évaluation d'un verdict
- Décision : pour une paire d'adresses et un service, à (`valid_at`, `as_of`) :
  (1) environnements résolus des deux extrémités ; une extrémité sans environnement →
  **refus « non classifié »** (D27) ; (2) un garde-fou qui s'applique → **refus**, qui le
  cite ; (3) une cellule pour au moins une paire (env A, env B) du type du service →
  **autorisation**, qui cite la paire et la cellule (D25) ; (4) sinon **refus par
  défaut**. Un port sans service correspondant → refus « service inconnu ».
- Pourquoi : D25, D27, D28, D29 ; garde-fous avant la matrice.
- Décidé par : Claude (délégation de l'utilisateur, 2026-10-20)

## S9 — Requête de conformité
- Décision : extrémités : adresse, préfixe (évalué sur lui-même), interface ou machine
  (développées en adresses par la 009) ; objet : service, couple transport/port (un
  verdict par service correspondant, synthèse sur le plus restrictif, D29) ou type de
  flux (un verdict par service de ce type) ; dates `valid_at` et `as_of`. Réponse : un
  verdict par (paire d'adresses, service), l'explication de résolution des deux
  extrémités (REQ-003-14) et une synthèse : `allowed` (tout autorisé), `partial`,
  `denied`.
- Pourquoi : D20, D22, D29 ; répond à la question type de D19.
- Rejeté : requête par type de flux sans développement en services (garde-fous par
  service inapplicables, D29).
- Décidé par : Claude (délégation de l'utilisateur, 2026-10-20)

## S10 — Cohérence des machines
- Décision : quand une machine porte `policy.declared_environment`, chaque adresse de
  ses interfaces doit résoudre un ensemble **égal** ; sinon anomalie (avertissement)
  `policy.machine.environment_mismatch`, sauf pour les interfaces dont le **rôle**
  (009) figure dans la liste des rôles exemptés (administration, sauvegarde,
  supervision par défaut, administrable). Sans environnement déclaré, des adresses qui
  divergent entre elles (hors rôles exemptés) donnent la même anomalie.
- Pourquoi : D22.
- Décidé par : Claude (délégation de l'utilisateur, 2026-10-20)

## S11 — Différentiel de politique
- Décision : un changeset qui touche la matrice, les garde-fous, les services, les
  types de flux ou les attributs d'environnement renvoie, en plus du différentiel des
  propriétés (003), un **différentiel de politique** : les triplets (env source, env
  destination, service) dont l'autorisation bascule, avec l'élément décisif avant et
  après. Les changements d'environnement des nœuds sont déjà dans le différentiel de
  propriétés ; leur effet sur des flux concrets est le différentiel de la 011.
- Pourquoi : D25 mitigation 2, borné (environnements × services) et calculable sans
  parcourir 10⁶ nœuds.
- Rejeté : différentiel « par nœud » de toutes les paires gagnées ou perdues
  (combinatoire, illisible).
- Décidé par : Claude (délégation de l'utilisateur, 2026-10-20)

## S12 — Approbation
- Décision : le module fournit des **déclencheurs** d'approbation (`policy.opens` : au
  moins une autorisation ouverte ; `policy.closes` ; seuils de nombre) que
  l'administrateur choisit dans une politique d'approbation (004/C6). Le changement de
  `policy.environment` est déjà couvert par le déclencheur de propriété de la 004.
  Aucune approbation n'est imposée par défaut (REQ-004-39).
- Pourquoi : D25 (« éventuellement soumis à approbation ») sans contredire C6.
- Décidé par : Claude (délégation de l'utilisateur, 2026-10-20)

## S13 — Classification obligatoire par domaine
- Décision : D27 s'exprime par une **obligation** de `policy.environment` portée sur le
  domaine (REQ-003-16), que l'administrateur ajoute ou retire par changeset sans
  redéploiement ; l'ajout est refusé tant que des nœuds du domaine n'ont pas de valeur
  résolue (REQ-003-6), avec leur liste. Hors obligation, le nœud non classifié est
  signalé (S14) et ses verdicts sont des refus (S8).
- Décidé par : Claude (délégation de l'utilisateur, 2026-10-20)

## S14 — Audit de masse
- Décision : rapport calculé par une tâche de fond (ADR 0023), daté, consultable et
  exportable : nœuds non classifiés par domaine ; nœuds dont l'ensemble contient un
  environnement transverse ; environnements sans aucune cellule ; cellules visant un
  environnement que plus aucun nœud ne porte ; garde-fous qui masquent entièrement une
  cellule ; anomalies de machines (S10) ; écarts déclaré/attendu quand l'inférence
  existera.
- Pourquoi : D25 mitigation 3, D27 ; volume 10⁶ nœuds hors du chemin des requêtes.
- Décidé par : Claude (délégation de l'utilisateur, 2026-10-20)

## S15 — Droits
- Décision : permissions du module : `policy.environment.admin`,
  `policy.flow_type.admin`, `policy.service.admin`, `policy.matrix.admin`,
  `policy.guardrail.admin` (globales, sans périmètre) ; `policy.read` (lire la
  politique) ; `policy.verdict.read` (requête de conformité). Une requête de conformité
  exige en plus la lecture des deux extrémités (même 404 qu'un objet invisible,
  REQ-006-20). Affecter `policy.environment` exige `policy.environment.write` sur le
  périmètre du nœud (REQ-003-17). Rôles livrés : le lecteur reçoit `policy.read` et
  `policy.verdict.read`.
- Pourquoi : D36 ; un verdict révèle les environnements des deux extrémités.
- Décidé par : Claude (délégation de l'utilisateur, 2026-10-20)

## S16 — Temps
- Décision : environnements, types de flux, services, garde-fous et cellules sont
  versionnés en bitemporel (D10) et écrits par changeset (D11) ; toute lecture et toute
  requête de conformité acceptent `valid_at` et `as_of` (« la politique au 1er mars »).
- Décidé par : Claude (délégation de l'utilisateur, 2026-10-20)

## S17 — Sérialisation des écritures
- Décision : les objets de politique n'appartiennent à aucun domaine d'adressage ; ils
  sont sérialisés par un verrou propre au module (clé `policy`), pris après les verrous
  de domaine dans l'ordre canonique de l'ADR 0015 ; leurs événements sortent sur une
  clé d'ordre `policy` de l'outbox (ADR 0018).
- Pourquoi : le différentiel de politique doit voir un état stable ; une écriture de
  matrice reste rare.
- Décidé par : Claude (délégation de l'utilisateur, 2026-10-20)

## S18 — Règles d'inférence : non tranchées
- Décision : la conception du moteur d'inférence (D24) est **réservée à une session
  dédiée** avec l'utilisateur (arbitrage du 2026-10-20) ; les questions sont posées dans
  [`inference-questions.md`](inference-questions.md). La 010 n'en dépend pas : un verdict
  ne lit que le déclaré (D23). Le seul point d'attache est la source de valeurs
  attendues de la 003 (REQ-003-21 à 24), dont la 010 précise ce qu'elle devra offrir
  (design, § Exigences envers le noyau).
- Décidé par : l'utilisateur (report), Claude (isolement)

## S19 — API
- Décision : collections `/v1/policy/environments`, `flow-types`, `services`,
  `guardrails`, `matrix-cells` ; écritures par changeset et routes de ressource (006/H2) ;
  `QUERY /v1/policy/verdicts` (corps JSON, méthode sûre, RFC 10008) et sa variante `GET`
  à paramètres pour un couple simple ; `GET /v1/policy/reports` et `POST
  /v1/policy/reports` (calcul par tâche de fond) ; additif à `/v1` (006/H12).
- Décidé par : Claude (délégation de l'utilisateur, 2026-10-20)

## S20 — Budgets
- Décision (p95 serveur, jeu de 10⁶ objets, 50 environnements, 20 types de flux,
  1 000 services, 10⁴ cellules, 500 garde-fous) : verdict d'une paire d'adresses et d'un
  service < 50 ms ; machine × machine (16 × 16 adresses) par type de flux < 300 ms ;
  port brut correspondant à 20 services < 100 ms ; différentiel de politique d'un
  changeset de matrice ou de garde-fou < 1 s ; rapport d'audit complet < 10 min en
  tâche de fond.
- Pourquoi : D14 ; même ordre que la résolution expliquée (REQ-003-26).
- Décidé par : Claude (délégation de l'utilisateur, 2026-10-20)

## S21 — Tension entre D23 et D30, signalée
- Constat : D30 dit que tout changement « de règle » réévalue les flux touchés ; or D23
  fonde les verdicts sur le seul déclaré, donc une règle d'inférence ne change aucun
  verdict existant (elle ne change que les anomalies et les valeurs par défaut des
  objets futurs).
- Lecture retenue : « règle » dans D30 désigne la matrice et les garde-fous ; un
  changement de règle d'inférence réévalue les anomalies d'écart, pas les flux.
- À confirmer par l'utilisateur (pas de changement de D23).

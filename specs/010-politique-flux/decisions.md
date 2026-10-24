# 010 — Politique de flux : journal des décisions

Rédigé le 2026-10-20 sans entretien : l'utilisateur a délégué les choix de cette spec
(« prends les décisions qui s'imposent »), en réservant les **règles d'inférence** à une
session dédiée (S18, [`inference-questions.md`](inference-questions.md)). Chaque décision
retient la recommandation la plus standard ; elles restent à relire avant
l'implémentation (v0.2, D31).

**Révision du 2026-10-21.** L'entretien sécurité (Q1 à Q24, [`besoin/`](besoin/)) a
consolidé le besoin ; les écarts sont dans [`besoin/ecarts.md`](besoin/ecarts.md). Les
décisions S1 à S21 portent leurs amendements ; S22 à S34 sont ajoutées. L'utilisateur a
confirmé le même jour les amendements de D28 (forme par attributs) et de D30 (validité
dérivée).

Cadre : D19 à D30, § « Approche — module Politique de flux » et § « Noyau ou module » de
[`../001-cadrage/decisions.md`](../001-cadrage/decisions.md) ; propriétés héritables
(spec 003, ADR 0016) ; changesets, bitemporel, approbations (004) ; RBAC (005) ; API
(006) ; contrat de module (ADR 0026, en cours d'écriture : on suppose le contrat esquissé
dans [`docs/spec/architecture.md`](../../docs/spec/architecture.md) § Modularité) ;
Machines et interfaces (009, en cours). Note de référence :
[`services-transports.md`](../../docs/reference/services-transports.md).

## Synthèse
- S1 périmètre : axes, catalogue, règles et motifs, conditions et signaux, verdict
  expliqué et triage, différentiel de politique, inférence, audit (amendée).
- S2 environnement : valeur de la liste fermée `policy.environment` (ensemble, héritable
  sur l'arbre d'adressage) ; attributs de valeur (transverse, rang) dans le module, pour
  tout axe (amendée).
- S3 porteurs : domaine, préfixes, adresses ; pas les plages ; machine par une propriété
  distincte non héritable.
- S4 types de flux : objets du module, liste administrable.
- S5 services : nom, un type de flux, couples transport/ports ; registre IANA en
  référence.
- S6 garde-fous : règles d'effet interdire (amendée par S23).
- S7 matrice : vue des règles mono-axe ; sélecteur d'extrémité vide = toute extrémité ;
  sélecteur de flux jamais vide (amendée par S23, S28).
- S8 évaluation : non classifié, interdictions, autorisations inconditionnelles, puis
  conditionnelles, refus par défaut ; puis admissibilité et signaux (amendée).
- S9 requête de conformité : extrémités adresse, préfixe, interface ou machine ; par
  service, port brut ou type de flux ; dossier facultatif ; verdict, issue, synthèse.
- S10 cohérence des machines : anomalie si les adresses divergent, sauf rôle
  d'interface.
- S11 différentiel de politique : combinaisons de valeurs d'axes et services qui
  basculent (amendée).
- S12 approbation : déclencheurs fournis par le module, posés par l'administrateur ;
  écriture de politique et reclassement en plus (amendée).
- S13 classification obligatoire : par domaine et par axe (amendée).
- S14 audit de masse : rapport calculé par une tâche de fond, rubriques étendues.
- S15 droits : administration globale de la politique ; verdict avec lecture des
  extrémités, ou permission « demander un flux » avec explication réduite (amendée).
- S16 temps : tout objet de politique bitemporel, écrit par changeset.
- S17 sérialisation : une file d'écriture propre à la politique, hors domaines.
- S18 inférence : tranchée par l'entretien (I1 à I15, I6 adouci) ; section G.
- S19 API : collections selon l'ADR 0026 (amendée par S34).
- S20 budgets chiffrés, conditions et signaux compris (amendée).
- S21 tension D23 / D30 : close, lecture confirmée.
- S22 axes multiples, désignés par l'administrateur ; vocabulaires d'exemple.
- S23 règles par attributs, deux effets, sans ordre.
- S24 motifs d'architecture : groupes nommés de règles.
- S25 autorisation conditionnelle sous motif : sinon « à instruire ».
- S26 rôle de conduit : un axe porté par la machine.
- S27 axes portés par la machine : union des machines de l'adresse, sinon défaut hérité.
- S28 vue matricielle à trois états et import hors ligne de l'existant.
- S29 textes pédagogiques par langue, obligatoires sur interdictions, motifs, conditions.
- S30 conditions d'admissibilité et issue ; rang de valeur pour les comparaisons.
- S31 explication complète, élément décisif le plus spécifique.
- S32 signaux et score informatif.
- S33 dossier, pré-vérification et permission « demander un flux ».
- S34 API : collections au pluriel sous `/v1` (ADR 0026, clause 9).
- S35 écriture d'une cellule de la matrice sous `If-Match` (amende S28, REQ-010-34).
- S36 aperçu de l'inférence dans le différentiel du changeset, servi par la simulation
  (amende S11 et S19, REQ-010-55) : acceptée le 2026-10-23.

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
- Amendé le 2026-10-21 (entretien, Q2 à Q9, Q17) : s'ajoutent les axes multiples, les
  motifs, les conditions d'admissibilité et l'issue de triage, les signaux et le score
  informatif, les textes pédagogiques, la permission « demander un flux », la vue
  matricielle et l'import de l'existant, l'inférence (S18). Les non-conformités
  consultables relèvent de la 011.

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
- Amendé le 2026-10-21 (Q5, ecarts N27) : les attributs de valeur (transverse, rang
  S30) valent pour toute valeur de tout axe, dans une table du module ; le noyau n'a pas
  à porter d'attribut par valeur.
- Amendé le 2026-10-24 (spec 018, Q-018-7) : les libellés d'un environnement vivent
  sur la valeur de liste fermée du noyau (`labels` de `property_enum_value`), écrits par
  `policy.value.describe` ; `policy_value` ne garde que ses attributs propres.

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
- Amendé le 2026-10-24 (spec 018, R7, R9) : le `label` d'un service, d'un type de
  flux, d'une règle et des autres objets de la politique devient `labels` par langue,
  contrôlé par `core.label.invalid` ; la colonne `label` disparaît (migrations `policy`
  0004, 0005).

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
- Amendé le 2026-10-21 (Q7, D28 amendée) : un garde-fou est une règle d'effet
  **interdire** à sélecteurs multi-axes (S23) ; « service interdit » reste une
  interdiction sans sélecteur d'extrémité ; le motif devient le texte pédagogique (S29).

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
- Amendé le 2026-10-21 (Q7, Q11, Q19) : la cellule devient une règle d'autorisation à
  sélecteurs (S23), éventuellement conditionnelle (S25) ; la matrice est une **vue** de
  ces règles (S28). Un sélecteur d'extrémité vide vaut « toute extrémité » (joker
  explicite, nécessaire aux motifs) ; le sélecteur de flux n'est jamais vide (les jokers
  restent réservés aux extrémités, Q19).

## S8 — Évaluation d'un verdict
- Décision : pour une paire d'adresses et un service, à (`valid_at`, `as_of`) :
  (1) environnements résolus des deux extrémités ; une extrémité sans environnement →
  **refus « non classifié »** (D27) ; (2) un garde-fou qui s'applique → **refus**, qui le
  cite ; (3) une cellule pour au moins une paire (env A, env B) du type du service →
  **autorisation**, qui cite la paire et la cellule (D25) ; (4) sinon **refus par
  défaut**. Un port sans service correspondant → refus « service inconnu ».
- Pourquoi : D25, D27, D28, D29 ; garde-fous avant la matrice.
- Décidé par : Claude (délégation de l'utilisateur, 2026-10-20)
- Amendé le 2026-10-21 (Q2, Q6, Q11, Q19, expression du besoin § 6.3) : ordre de
  priorité **non classifié > interdiction > autorisation inconditionnelle >
  autorisation conditionnelle satisfaite > autorisation conditionnelle non satisfaite
  (à instruire) > refus par défaut**, indépendant de l'ordre d'écriture ; « non
  classifié » s'apprécie par axe, sur les axes que cite au moins une règle candidate pour
  le service. Puis l'admissibilité (S30) décide de l'issue et les signaux (S32) donnent
  le score. Rigueur confirmée (Q19) : non classifié refusé ; port sans service connu
  refusé, mais issue « à instruire » avec l'invitation à nommer ou faire ajouter le
  service, sans type fourre-tout.

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
- Amendé le 2026-10-21 (Q3, Q24) : la requête accepte un **dossier** facultatif (S33)
  et rend aussi l'issue, les conditions non remplies et le score ; elle sert de
  pré-vérification sans écriture, avec le même résultat que l'enregistrement (011).

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
- Amendé le 2026-10-21 (Q15) : le différentiel abstrait porte sur des combinaisons de
  valeurs d'axes (source, destination) et des services, et distingue `allowed` sous
  réserve ; la 003 donne les nœuds reclassés, le module y ajoute le nombre d'héritiers
  reclassés ; la 011 nomme les flux dont le verdict **ou l'issue** bascule.

## S12 — Approbation
- Décision : le module fournit des **déclencheurs** d'approbation (`policy.opens` : au
  moins une autorisation ouverte ; `policy.closes` ; seuils de nombre) que
  l'administrateur choisit dans une politique d'approbation (004/C6). Le changement de
  `policy.environment` est déjà couvert par le déclencheur de propriété de la 004.
  Aucune approbation n'est imposée par défaut (REQ-004-39).
- Pourquoi : D25 (« éventuellement soumis à approbation ») sans contredire C6.
- Décidé par : Claude (délégation de l'utilisateur, 2026-10-20)
- Amendé le 2026-10-21 (Q15) : déclencheurs ajoutés : `policy.write` (toute écriture
  de politique) et `policy.reclassify` (au moins N héritiers reclassés sur un axe, N30).
  Le seul défaut imposé reste celui de la 011 (« un flux se ferme », 011/F8).

## S13 — Classification obligatoire par domaine
- Décision : D27 s'exprime par une **obligation** de `policy.environment` portée sur le
  domaine (REQ-003-16), que l'administrateur ajoute ou retire par changeset sans
  redéploiement ; l'ajout est refusé tant que des nœuds du domaine n'ont pas de valeur
  résolue (REQ-003-6), avec leur liste. Hors obligation, le nœud non classifié est
  signalé (S14) et ses verdicts sont des refus (S8).
- Décidé par : Claude (délégation de l'utilisateur, 2026-10-20)
- Amendé le 2026-10-21 (Q6) : l'obligation se règle **par domaine et par axe** ; le
  « non classifié » vaut par axe cité par la politique.

## S14 — Audit de masse
- Décision : rapport calculé par une tâche de fond (ADR 0023), daté, consultable et
  exportable : nœuds non classifiés par domaine ; nœuds dont l'ensemble contient un
  environnement transverse ; environnements sans aucune cellule ; cellules visant un
  environnement que plus aucun nœud ne porte ; garde-fous qui masquent entièrement une
  cellule ; anomalies de machines (S10) ; écarts déclaré/attendu quand l'inférence
  existera.
- Pourquoi : D25 mitigation 3, D27 ; volume 10⁶ nœuds hors du chemin des requêtes.
- Décidé par : Claude (délégation de l'utilisateur, 2026-10-20)
- Amendé le 2026-10-21 (UC-9) : rubriques ajoutées : non classifiés par axe,
  autorisations qui ne tiennent qu'à une valeur transverse, règles jamais appliquées,
  combinaisons autorisées sans flux déclaré, écarts d'inférence. Non-conformités,
  dérogations et échéances : rapports de la 011.

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
- Amendé le 2026-10-21 (Q17) : permission **`policy.flow.request`**, portée par entité :
  verdict sur toute paire sans lecture de l'arbre, explication réduite aux valeurs
  d'axes, règles, motifs et conditions (pas de nœud d'origine, de chaîne ni d'objet du
  référentiel) ; un compte de service qui la détient transmet l'identité du demandeur
  (« pour le compte de », N31). Permissions `policy.axis.admin`, `policy.rule.admin`
  (règles, motifs, conditions, signaux) et `policy.inference.admin` (par domaine, I7)
  remplacent `policy.matrix.admin` et `policy.guardrail.admin`.

- Amendé le 2026-10-21 (tâche 010-8, réalignement) : `policy.read` devient
  **`policy.catalogue.read`**. Le catalogue des permissions impose la forme
  `<module>.<objet>.<action>` (K12, contrainte `role_permission_permission_check`) ; un
  nom à deux segments ne peut pas entrer dans un rôle. Les objets de la politique se
  lisent tous sous cette permission ; leur administration suit les familles ci-dessus.
- Décidé par : Claude (délégation de l'utilisateur, 2026-10-21), à relire

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
- Close le 2026-10-21 (Q10 à Q13) : I1 à I15 acceptés tels que recommandés, sauf I6
  **adouci** : priorité explicite puis nœud le plus profond, mais une égalité restante
  ne bloque pas l'écriture ; le nœud reçoit plusieurs candidats, sans valeur par défaut
  ni anomalie d'écart, et le jeu de règles porte l'anomalie « règles en conflit ».
  L'inférence est un confort de saisie et un contrôle de vraisemblance (D23 renforcé).
  Exigences REQ-010-40 à 54 (section G).
- Décidé par (clôture) : l'utilisateur (Q12, Q13)

## S19 — API
- Décision : collections `/v1/policy/environments`, `flow-types`, `services`,
  `guardrails`, `matrix-cells` ; écritures par changeset et routes de ressource (006/H2) ;
  `QUERY /v1/policy/verdicts` (corps JSON, méthode sûre, RFC 10008) et sa variante `GET`
  à paramètres pour un couple simple ; `GET /v1/policy/reports` et `POST
  /v1/policy/reports` (calcul par tâche de fond) ; additif à `/v1` (006/H12).
- Décidé par : Claude (délégation de l'utilisateur, 2026-10-20)
- Remplacé le 2026-10-21 par S34 (forme des chemins alignée sur l'ADR 0026).

## S20 — Budgets
- Décision (p95 serveur, jeu de 10⁶ objets, 50 environnements, 20 types de flux,
  1 000 services, 10⁴ cellules, 500 garde-fous) : verdict d'une paire d'adresses et d'un
  service < 50 ms ; machine × machine (16 × 16 adresses) par type de flux < 300 ms ;
  port brut correspondant à 20 services < 100 ms ; différentiel de politique d'un
  changeset de matrice ou de garde-fou < 1 s ; rapport d'audit complet < 10 min en
  tâche de fond.
- Pourquoi : D14 ; même ordre que la résolution expliquée (REQ-003-26).
- Décidé par : Claude (délégation de l'utilisateur, 2026-10-20)
- Amendé le 2026-10-21 : jeu porté à 6 axes, 10⁴ règles, 100 motifs, 50 conditions,
  30 signaux ; le verdict < 50 ms inclut conditions et signaux ; budgets de l'inférence
  repris de I14 (REQ-010-54).

## S21 — Tension entre D23 et D30, signalée
- Constat : D30 dit que tout changement « de règle » réévalue les flux touchés ; or D23
  fonde les verdicts sur le seul déclaré, donc une règle d'inférence ne change aucun
  verdict existant (elle ne change que les anomalies et les valeurs par défaut des
  objets futurs).
- Lecture retenue : « règle » dans D30 désigne la matrice et les garde-fous ; un
  changement de règle d'inférence réévalue les anomalies d'écart, pas les flux.
- À confirmer par l'utilisateur (pas de changement de D23).
- Close le 2026-10-21 (Q12, Q13, Q19) : lecture confirmée ; D30 amendée en ce sens.
  Une règle d'inférence ne réévalue que les anomalies d'écart.
- Décidé par : l'utilisateur

## S22 — Axes de classification
- Décision : la politique lit plusieurs **axes**. Un axe est une propriété de liste
  fermée (ou de référence externe, N26) qu'un administrateur désigne comme axe, avec son
  porteur (arbre d'adressage ou machine). Seul `policy.environment` est imposé par le
  module ; `policy.exposure`, `policy.sensitivity` (avec l'indicateur `personal_data`),
  `policy.criticality`, `policy.application`, `policy.owner` et `policy.conduit_role`
  sont livrés comme configuration d'exemple, remplaçable. Environnement et exposition
  sont portés par l'arbre ; sensibilité, criticité, application, propriétaire et rôle de
  conduit par la machine (S27).
- Pourquoi : Q5, Q6, Q22 ; générique sans imposer le vocabulaire d'une organisation.
- Rejeté : environnement seul (D21 d'origine, insuffisant pour le zonage multi-axes) ;
  axes codés dans le module (une organisation ne pourrait pas ajouter les siens).
- Décidé par : l'utilisateur (Q5, Q6) ; forme : recommandation acceptée

## S23 — Règles par attributs
- Décision : une règle = sélecteur source, sélecteur destination (ET entre axes, OU
  entre valeurs d'un axe ; vide = toute extrémité), contraintes « même valeur » par axe,
  sélecteur de flux non vide (types ou services), effet `allow` ou `deny`, motifs requis
  (autorisation seulement), appartenance à un motif, texte, période. **Aucun ordre** :
  le résultat ne dépend ni de l'édition ni de l'import. L'élément décisif est la règle
  applicable la plus spécifique (le plus de critères), départagée par la clé.
- Pourquoi : Q7, Q9, D28 amendée ; les résultats actuels sont conservés (une cellule et
  un garde-fou s'expriment chacun en une règle).
- Rejeté : matrices par axe combinées ; règles ordonnées « première qui correspond » ;
  langage d'expressions complet (explication, validation, budgets).
- Décidé par : l'utilisateur (Q7, Q9)

## S24 — Motifs d'architecture
- Décision : un motif est un groupe nommé de règles (`allow` et `deny`) avec un texte
  pédagogique ; il correspond à une paire et un service si l'une de ses règles `allow`
  s'applique et qu'aucune de ses règles `deny` ne s'applique. Une règle appartient à au
  plus un motif ; les règles d'un motif valent aussi comme règles ordinaires de la
  politique.
- Pourquoi : Q9 ; un motif nommé se cite dans l'explication et se requiert par une
  autorisation conditionnelle (S25).
- Décidé par : l'utilisateur (Q9) ; « au plus un motif » : Claude (délégation,
  2026-10-21)

## S25 — Autorisation conditionnelle
- Décision : une règle `allow` peut exiger un ou plusieurs motifs. Si un motif requis
  correspond, verdict `allowed` (élément décisif : le motif) ; sinon `allowed` **sous
  réserve**, issue « à instruire » avec les motifs attendus et leur texte. Une
  autorisation inconditionnelle applicable l'emporte (union D25). « Partiel » de la
  matrice existante s'importe ainsi.
- Pourquoi : Q11 ; garde le verdict binaire de D28 et place le cas douteux dans le
  triage plutôt que dans le refus.
- Rejeté : partiel = refus orienté ; troisième effet.
- Décidé par : l'utilisateur (Q11)

## S26 — Rôle de conduit
- Décision : le rôle de conduit (stockage d'objets, passerelle d'API, bastion…) est un
  **axe** `policy.conduit_role`, liste fermée administrable, porté par la machine avec
  défaut hérité de l'arbre (S27), cité par les sélecteurs comme tout axe. Il est
  distinct du rôle d'interface de la 009 (administration, sauvegarde…), qui qualifie
  l'usage d'une interface et sert à S10.
- Pourquoi : un conduit est un rôle d'architecture de la machine, pas d'une de ses
  interfaces ; aucun mécanisme nouveau dans la 009 (M9 tenu par les porteurs de
  propriétés, REQ-009-24) ni dans l'évaluation.
- Rejeté : extension du vocabulaire du rôle d'interface (mélange deux notions, une VM
  d'un seul attachement serait conduit par interface).
- Décidé par : l'utilisateur (2026-10-21), sur la proposition de Claude (ecarts D22 et
  M8 laissaient le choix)

## S27 — Axes portés par la machine
- Décision : pour un axe porté par la machine, la valeur d'une adresse est l'**union**
  des valeurs des machines rattachées à l'adresse à la date (VIP, anycast) ; si aucune
  machine n'en porte, la valeur héritée de l'arbre d'adressage sert de **défaut
  hérité**. L'explication nomme la source (machine et interface, ou nœud d'origine).
- Pourquoi : Q6 (« une valeur posée sur un préfixe sert de défaut hérité ») ; union
  cohérente avec D25.
- Décidé par : l'utilisateur (Q6) ; union pour plusieurs machines : l'utilisateur
  (2026-10-21), sur la proposition de Claude

## S28 — Vue matricielle et import
- Décision : la matrice (environnement source × environnement destination × type de
  flux, trois états `allow`, `deny`, `conditional`) est une vue en lecture et écriture
  des règles qui ne citent qu'une valeur d'environnement de chaque côté et un type de
  flux. L'existant (matrice à trois états, catalogue en deux tables, règles d'inférence)
  s'importe par un **convertisseur hors ligne** qui produit des changesets et, en
  simulation, liste les différences de résultat ; aucun format historique ne devient un
  contrat du serveur.
- Pourquoi : Q7, Q10, Q11, I13 ; UC-11.
- Décidé par : l'utilisateur (Q7, Q13)

## S29 — Textes pédagogiques
- Décision : texte obligatoire sur toute règle `deny`, tout motif et toute condition ;
  facultatif sur une règle `allow`. Un texte = corps (texte simple avec liens) et lien
  facultatif vers une documentation, **par langue**, avec repli sur la langue par défaut
  de l'instance ; versionné avec son objet. Restitué avec le verdict et les
  non-conformités.
- Pourquoi : complément de l'utilisateur après Q9 (accompagnement) ; Q-ouv-6 (i18n). Un
  texte écrit par un administrateur ne peut pas vivre dans les catalogues du shell : il
  est stocké par langue plutôt que par clé.
- Rejeté : clés de traduction résolues par le shell (texte d'exploitation hors du
  référentiel).
- Décidé par : l'utilisateur (pédagogie) ; stockage par langue : l'utilisateur
  (2026-10-21), sur la proposition de Claude
- Amendé le 2026-10-24 (018/REQ-018-12) : la langue par défaut de l'instance se
  configure (`[i18n] default_locale`, publiée par les capacités) ; les textes exigés
  dans la langue par défaut le sont dans celle-ci.

## S30 — Conditions d'admissibilité et issue
- Décision : une condition est un prédicat nommé d'un **genre** pris dans une liste
  fermée extensible par le code (champ du dossier présent ; largeur maximale d'une
  extrémité ; extrémité autre qu'un domaine entier ; service nommé plutôt que port brut ;
  rang d'une valeur du dossier au plus celui d'un axe de la destination), avec
  paramètres, sélecteur facultatif (règles, types de flux) et texte. Issue : verdict
  refusé → `noncompliant` ; autorisé sous réserve ou condition non remplie → `review` ;
  sinon `automatic`. Les comparaisons d'ordre utilisent un **rang** facultatif posé sur
  les valeurs d'axe (S2), jamais lu par le verdict (D21 : axes non ordonnés).
- Pourquoi : Q2, Q4 ; un genre fermé reste explicable et borné (D14).
- Rejeté : prédicats en langage libre.
- Décidé par : l'utilisateur (Q2, Q4) ; genres et rang : l'utilisateur (2026-10-21), sur
  la proposition de Claude

## S31 — Explication
- Décision : chaîne complète (expression du besoin § 6.4) : demande normalisée et
  dates ; par extrémité, adresse et axe cité, valeurs, origine et chaîne (ou machine
  source) ; services ; élément décisif avec sa clé, la révision, la combinaison et le
  texte ; trois règles écartées au plus ; conditions non remplies ; score ;
  avertissements ; issue et suite attendue. Réduite pour `policy.flow.request` (S15).
- Décidé par : l'utilisateur (Q14)

## S32 — Signaux et score
- Décision : un signal est un prédicat nommé d'un genre pris dans une liste fermée
  extensible (portée des extrémités, nature du flux, croisement des axes), avec
  paramètres et **poids** réglable ; le score est la somme des poids déclenchés, rendu
  avec la liste. Il n'influence ni le verdict ni l'issue. Le groupe « dossier et
  historique » attend la 009 et les référentiels annexes.
- Décidé par : l'utilisateur (Q4, Q20)

## S33 — Dossier et pré-vérification
- Décision : la requête de conformité accepte un dossier facultatif (références
  externes, demandeur et « pour le compte de », responsables, nature des données,
  période) et rend, sans écrire, la même évaluation que l'enregistrement d'un flux (011)
  ; c'est la pré-vérification des outils externes (UC-1, Q24). Sans dossier, les
  conditions qui le lisent sont « non évaluées » et l'issue n'est pas automatique.
- Décidé par : l'utilisateur (Q3, Q24)

## S34 — API
- Décision : collections au pluriel, en kebab-case, sous `/v1` (ADR 0026, clause 9) :
  `/v1/policy-axes`, `/v1/environments`, `/v1/flow-types`, `/v1/services`,
  `/v1/policy-rules`, `/v1/policy-patterns`, `/v1/admissibility-conditions`,
  `/v1/risk-signals`, `/v1/inference-rules`, `/v1/policy-reports` ; vue
  `/v1/policy-matrix` ; verdicts par `QUERY /v1/verdicts` et `GET /v1/verdicts` ;
  suggestion par `GET /v1/{collection}/{id}/suggestion` (route transverse du noyau).
- Pourquoi : l'ADR 0026, acceptée, fixe la forme des chemins de module ; l'écart N13
  relevé à sa rédaction est ainsi résolu.
- Décidé par : l'utilisateur (2026-10-21), sur la proposition de Claude

## S35 — Écriture d'une cellule sous `If-Match` (amende S28, REQ-010-34)
- Constat (2026-10-22, spec 017, Q-017-7) : `PUT /v1/policy-matrix/{source}/{flow_type}/{destination}`
  passe par `kit::submit` (opération sans objet ni `If-Match`) ; deux responsables qui
  écrivent la même matrice : le dernier l'emporte sans le savoir. Toutes les autres
  écritures de ressource exigent `If-Match` (006/H8). La matrice porte déjà un ETag
  (empreinte de la vue d'un type de flux).
- Options : (A) le client relit l'ETag avant d'écrire (contrôle non atomique) ;
  (B) `PUT` sous `If-Match` de la matrice, comme toute écriture.
- Décision : **B**. Le `PUT` d'une cellule exige `If-Match` portant l'ETag de la matrice
  du type de flux lue au présent : absent → 428, périmé → 412 (006/H8). La comparaison
  se fait dans la transaction d'écriture, sous le verrou `policy` (S17), en recalculant
  la vue du type de flux avant le changeset : elle est atomique. L'opération
  `policy.matrix.set` reçoit un membre facultatif `if_match` (l'ETag lu) ; la route le
  remplit depuis l'en-tête ; dans un changeset composé (`POST /v1/changesets`), il reste
  facultatif, et un écart y est la violation `policy.matrix.stale` (erreur, 422
  `changeset-refused`). Plusieurs cellules d'un même type de flux dans un changeset sont
  comparées à la matrice d'avant le changeset, pas à celle qu'ont laissée les opérations
  précédentes.
- Pourquoi : un contrat uniforme (toute écriture de ressource sous condition) ; la
  matrice est l'écran où deux responsables travaillent le plus souvent en même temps.
  Grain retenu : la matrice d'un type de flux, pas la cellule (l'ETag servi est celui
  de la vue ; deux cellules distinctes d'un même type se gênent, ce qui est voulu : la
  vue relue montre l'autre changement).
- Décidé par : l'utilisateur (2026-10-22, Q-017-7, option B) ; grain, membre `if_match`
  et violation du changeset composé : Claude, choix de conception

## S36 — Aperçu de l'inférence dans le différentiel (amende S11 et S19, REQ-010-55)
- Statut : **acceptée** par l'utilisateur le 2026-10-23 (rédigée par Claude le
  2026-10-22) ; implémentée par la tâche 010-22 (débloque 017-15, 021-31).
- Constat (2026-10-22, spec 017, REQ-017-34, tâche 017-15) : avant d'écrire une règle
  d'inférence, le responsable veut voir les nœuds qu'elle toucherait et les écarts
  qu'elle ouvrirait ou fermerait. Aucune lecture ne le sert : la suggestion
  (REQ-010-51) se lit nœud par nœud sur les règles déjà écrites ; les écarts
  (REQ-010-48) n'existent qu'après application, par la tâche de fond ; le différentiel
  du changeset (S11) ne couvre que les règles de politique, pas l'inférence.
- Options :
  (A) l'inférence contribue au différentiel du changeset, lu par la simulation
  existante (`POST /v1/changesets?simulate=true`, REQ-004-9) et, une fois soumis, par
  `GET /v1/changesets/{id}/diff?section=…` ;
  (B) une route dédiée `QUERY /v1/inference-rules/preview` dont le corps est un
  brouillon de règle ;
  (C) l'interface calcule l'aperçu elle-même en lisant les règles et l'arbre.
- Décision : **A**. Quand un changeset crée, modifie ou clôt une règle
  d'inférence, la contribution du module (section `policy.combinations`, ADR 0026
  clause 13) ajoute une ligne `policy.inference.summary` (décomptes) et une ligne
  `policy.inference.node` par nœud dont l'**attendu** change entre l'état d'avant et
  celui d'après le changeset, avec l'écart d'avant et d'après. Les écarts se comptent
  en nœuds et en nœuds d'origine (l'unité des anomalies `policy.inference.mismatch`).
  Portée : les sous-arbres des nœuds où une règle touchée (avant ou après, exceptions
  comprises) correspond ; au-delà de 10⁵ nœuds dans la portée, la ligne de synthèse
  donne le seul décompte de la portée (`truncated: true`), sans lignes de nœuds. La
  tâche de fond `policy.inference_check` reste seule à lever les anomalies.
- Pourquoi : aucun contrat nouveau (ni route, ni permission, ni problème) ; l'aperçu
  porte sur ce qui sera réellement écrit, changeset composé compris (règle et ses
  exceptions, reclassement dans le même changeset) ; les droits sont ceux de
  l'écriture, puisque la simulation exige les permissions des opérations (une règle qui
  cite un domaine ne s'évalue que dans ce domaine, que son auteur administre) ;
  l'approbateur d'un changeset retenu voit le même aperçu. (B) dupliquerait la
  validation des opérations et la simulation ; (C) exposerait au client tout l'arbre et
  toutes les règles, et dupliquerait le moteur.
- Coût : le différentiel d'un changeset de règles d'inférence s'allonge, simulé ou
  appliqué ; borné par la portée (10⁵ nœuds) et chiffré dans REQ-010-55 (à confirmer
  par le banc 010-19).
- Hors décision : une mesure d'approbation sur les écarts ouverts (`policy.inference_opens`,
  REQ-010-20) ; à proposer à part si le besoin s'en fait sentir.
- Décidé par : l'utilisateur (2026-10-23, option A) ; grain, bornes, ordre des lignes
  et date d'évaluation : choix de conception de Claude, acceptés avec l'option

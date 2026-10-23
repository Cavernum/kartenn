# 001 — Cadrage : journal des décisions

Issu de la session de cadrage (grilling) du 2026-10-13 : besoin → architecture.
Chaque décision indique ce qui a été décidé, pourquoi, ce qui a été rejeté et par qui.
Les specs ultérieures référencent ces décisions par leur identifiant (`D<k>`).

## Synthèse des décisions (détail dans le journal ci-dessous)
- **Produit** : kartenn, un référentiel réseau **déclaratif**, source de vérité, avec une validation de cohérence **bloquante** (D1). Environ 10⁶ objets, budgets jusqu'à 10⁷ (D2). Une organisation, éventuellement un groupe d'entités, avec des opérateurs externes (D3).
- **Modèle** : domaines d'adressage obligatoires (D4) ; v4 et v6, familles d'adresses extensibles en vue d'IPv8 (D5) ; ancres entité, site et domaine (D12) ; arbre de préfixes déduit de l'inclusion CIDR (D34) ; **bitemporel** natif (D8, D10) ; écritures par **changesets** avec revue optionnelle (D11) ; suppression refusée par défaut, cascade explicite (D33) ; **propriétés héritables** dans le noyau (D19).
- **Architecture** : hexagonale. Modules métier compilés (crates), plugins UI à chaud (Web Components), connecteurs hors processus (D6) ; **API HTTP `/v1` (REST et flux d'événements), seule frontière publique** (D7 amendée) ; N instances sans état, PostgreSQL ≥ 18 en primaire unique pour les écritures (D9, D15) ; Rust (D16) ; Vue 3 + TS (D17).
- **Sécurité d'accès** : OIDC multi-émetteurs (humains) et Basic sur TLS (machines, secours) (D13) ; RBAC composable avec liaisons portées (D36).
- **Qualité** : budgets p95 chiffrés et bancs d'essai en CI (D14) ; OCI, module NixOS, OpenTelemetry (D35) ; AGPL-3.0 (D37).
- **Sécurité métier, objectif final** : conformité à la politique, puis atteignabilité (D20) ; environnements en ensembles, combinaison permissive (D21, D25) ; résolution par adresse (D22) ; le déclaré fait foi, l'inféré contrôle (D23) ; règles d'inférence avec exceptions et priorité explicite (D24) ; remplacement de l'ensemble hérité (D26) ; classification obligatoire réglée par domaine (D27) ; politique en règles par attributs multi-axes, sans ordre, où l'interdiction l'emporte, avec autorisations conditionnelles sous motif et conditions d'admissibilité (D28 amendée) ; catalogue de services nommés (D29) ; registre de flux à validité dérivée de la politique (D30 amendée).
- **Feuille de route** (D31) : v0.1 = noyau, VLAN, Machines et interfaces, REST et Web ; v0.2 = Politique et Flux ; v0.3 = passerelles, routes, NAT ; ensuite atteignabilité, connecteur historique, MCP, CLI, TUI ; les événements poussés (022) précèdent la réécriture du web (020, 021) (D31 amendée).

## D1 — Source de vérité et sens de « contrôler »
- Décision : l'outil est déclaratif et constitue la source de vérité. « Contrôler » veut dire qu'une validation de cohérence est bloquante : aucune incohérence n'est acceptée en écriture. Pas de découverte ni de pilotage actif pour l'instant.
- Pourquoi : la qualité des données est prioritaire. Il existe une source historique vieillissante qui sera importée à terme, mais on en fait abstraction pour le moment.
- Conséquence : on prévoit une architecture de **connecteurs modulaires** pour les sources de données futures (import ou synchronisation). Le premier connecteur à écrire sera celui de la source historique.
- Rejeté : découverte automatique (données bruitées, va contre la validation stricte), intention et observé dès le départ (périmètre doublé), pilotage actif (autre classe de produit).
- Décidé par : l'utilisateur

## D2 — Volumétrie cible
- Décision : environ 10⁶ objets (IP et interfaces), environ 10⁵ subnets, environ 10⁴ VLAN pour le plus gros SI. Les budgets de performance sont dimensionnés avec une marge ×10, donc jusqu'à 10⁷.
- Pourquoi : c'est l'ordre de grandeur réaliste d'un très grand SI d'entreprise, et une seule instance relationnelle bien indexée le tient.
- Rejeté : 10⁵ (sous-dimensionné), 10⁷ comme cible nominale (imposerait un partitionnement prématuré).
- Décidé par : recommandation acceptée

## D3 — Utilisateurs et cloisonnement
- Décision : un seul déploiement par organisation, avec un RBAC restreint par **périmètre**. L'organisation peut être un **groupe de plusieurs entités juridiques**, dont des SI hérités de rachats. Des **opérateurs externes** (prestataires interconnectés) doivent pouvoir éditer leur propre portion du SI sans appartenir à l'organisation.
- Conséquences : (a) l'« entité » ou « organisation propriétaire » est une notion du modèle, distincte du tenant technique ; (b) les identités ne viennent pas toutes de l'IdP interne, ce qui impose plusieurs émetteurs OIDC ou des comptes locaux (voir plus loin) ; (c) le périmètre RBAC doit pouvoir porter sur une entité, un domaine d'adressage ou un sous-arbre de préfixes ; (d) l'agglomération de SI implique très probablement des plages qui se chevauchent (voir D4).
- Rejeté : RBAC global seul (insuffisant), multi-tenant strict (isolation inutile, coût élevé).
- Décidé par : l'utilisateur

## D4 — Espaces d'adressage qui se chevauchent
- Décision : un **domaine d'adressage** de premier rang, obligatoire, comparable à une VRF ou un espace de routage. Unicité et non-chevauchement sont garantis **par domaine**. Il existe un domaine « global » pour l'adressage public. Les NAT et passerelles relient explicitement deux domaines.
- Pourquoi : c'est le seul modèle compatible à la fois avec la validation stricte (D1) et avec des SI agglomérés qui réutilisent les plages RFC 1918 (D3).
- Rejeté : espace unique (faux au premier rachat), VRF optionnelle façon NetBox (réintroduit des incohérences tolérées).
- Décidé par : recommandation acceptée

## D5 — Familles d'adresses
- Décision : IPv4 et IPv6 sont supportés dès le MVP (RFC 4291, forme canonique RFC 5952, formats RFC 4632/CIDR). La **famille d'adresse est une abstraction extensible** (module par famille : parsing, canonicalisation, arithmétique, longueur en bits) pour pouvoir ajouter « IPv8 » plus tard. IPv8 est un non-objectif du MVP.
- État d'IPv8 (vérifié le 2026-10-13) : il s'agit de `draft-thain-ipv8` (révisions 00 à 02, avril 2026), un Internet-Draft **individuel**, sans groupe de travail ni consensus. Une adresse IPv8 y serait sur 64 bits (préfixe de routage lié à l'ASN et hôte sur 32 bits), et IPv4 en serait un sous-ensemble. Le statut est instable : on ne fige rien dessus.
- Conséquences : aucune hypothèse « 32 ou 128 bits » codée en dur dans le domaine. Côté stockage, les types natifs (`inet`/`cidr` en PostgreSQL) ne couvrent que v4 et v6. L'adaptateur de stockage doit donc isoler la représentation (voir le choix de la BDD). `polars-network` (u32, IPv4) n'est pas réutilisable tel quel.
- Décidé par : l'utilisateur (v4 + v6, IPv8 anticipé), la recommandation précise le traitement d'IPv8

## D6 — La modularité diffère selon la couche
- Décision : il n'y a pas de modèle unique. **Modules de données et métier** : compilés avec l'application, la modularité porte sur l'organisation du code, et le schéma de la BDD dépend des modules présents. **Frontend** : plugins à chaud possibles (par exemple, un onglet de visualisation graphique ajouté ou mis à jour sans reconstruire).
- Pourquoi : les invariants de données imposent une compilation et une transaction communes. L'UI gagne à évoluer indépendamment.
- Rejeté : microservices par module (transactions distribuées), plugins backend à chaud pour les données.
- Décidé par : l'utilisateur

## D7 — REST, frontière publique unique
- Décision : CLI, TUI, MCP, connecteurs et frontend Web sont tous des **clients de l'API REST**. Aucun accès direct à la couche service hors du serveur.
- Pourquoi : un seul point où s'appliquent authentification, RBAC, validation, audit et limitation de débit ; l'API est utilisée par ses propres outils ; les binaires se déploient indépendamment.
- Conséquences : l'API doit offrir des **opérations par lots atomiques** performantes (imports de 10⁵ à 10⁶ objets) et un endpoint de **capacités** (modules actifs). Le CLI et le MCP peuvent partager un SDK client généré depuis l'OpenAPI.
- Rejeté : MCP dans le processus (deux surfaces), connecteurs dans le processus (contredit le « à chaud » et contourne l'audit).
- Décidé par : recommandation acceptée
- Amendement (spec 022, A1, 2026-10-23) : la frontière publique unique est l'**API HTTP `/v1` (REST et flux d'événements)**. Le flux `GET /v1/events` (server-sent events) en fait partie : mêmes authentification, RBAC, audit, quotas et contrat OpenAPI. Un futur serveur MCP est un adaptateur séparé (`POST /mcp`), branché sur la même source d'événements, jamais sur `/v1/events`.
- Décidé par (amendement) : l'utilisateur, après une étude des normes ([`docs/reference/push-serveur.md`](../../docs/reference/push-serveur.md))

## D8 — Historique temporel natif
- Décision : chaque objet est **versionné dans le temps** et l'on peut interroger directement l'état du réseau à une date X. Plus le journal d'audit (auteur, motif).
- Pourquoi : choix de l'utilisateur, pour consulter le passé et éventuellement planifier des changements.
- Conséquences : les contraintes d'unicité et de non-chevauchement deviennent **temporelles** (pas de chevauchement de préfixes dans un domaine *sur des périodes qui se recouvrent*) ; chaque requête porte un instant (par défaut « maintenant ») ; index et partitionnement doivent tenir compte des versions ; l'API expose un paramètre « à la date ». Le budget de performance doit l'intégrer.
- À préciser : temps applicatif seul, temps système seul, ou bitemporel (voir D10).
- Rejeté : audit simple (perd la reconstitution), journal plus rejeu (reconstitution lente, pas de requêtes directes).
- Décidé par : l'utilisateur

## D9 — Mise à l'échelle horizontale
- Décision : le serveur applicatif doit pouvoir tourner en **N instances** derrière un répartiteur de charge.
- Conséquences : instances **sans état** (pas de session en mémoire ; jetons auto-portés ou stockage de session partagé) ; aucun cache local faisant autorité (invalidation par événements) ; jobs et relais outbox distribués sans doublon (verrous en base, par exemple `FOR UPDATE SKIP LOCKED`, ou élection de leader) ; migrations exécutées une seule fois, en job dédié et non au démarrage de chaque instance ; plugins UI servis de façon identique par toutes les instances. **La base devient le point de cohérence** : les invariants stricts (D1) imposent un primaire unique en écriture, avec des réplicas en lecture possibles (attention au retard de réplication, à gérer pour la lecture de ses propres écritures).
- Décidé par : l'utilisateur

## D10 — Bitemporel
- Décision : **temps applicatif** (période d'existence dans le réseau réel ; permet de planifier et de corriger le passé) **et temps système** (période où l'outil l'affirmait ; immuable, jamais réécrit). Modèle de référence : SQL:2011, périodes applicatives et tables versionnées système.
- Pourquoi : planification plus audit infalsifiable ; le temps système ne coûte presque rien s'il est prévu dès le départ.
- Conséquences : les invariants se vérifient sur le temps applicatif, dans l'état système courant ; une écriture « planifiée » est une version dont le temps applicatif est futur, et elle est validée dès sa saisie contre les autres versions futures. Les requêtes « à la date X » acceptent deux axes (`valid_at`, `as_of`). Les index et le partitionnement séparent les versions courantes des versions historiques.
- Rejeté : temps système seul (ni planification ni correction), temps applicatif seul (audit non fiable).
- Décidé par : recommandation acceptée

## D11 — Changesets
- Décision : toute écriture passe par un **changeset**, un ensemble d'opérations validé puis appliqué atomiquement. Par défaut, il s'applique dans le même appel. Une **approbation** peut être exigée par politique RBAC, selon le périmètre et le rôle (par exemple, pour les prestataires externes).
- Pourquoi : un mécanisme unique pour l'unitaire, les lots, les imports de connecteurs et la revue ; c'est aussi l'unité naturelle d'audit et d'événement.
- Conséquences : un changeset en attente est validé à la soumission **puis revalidé à l'application** (l'état a pu changer : contrôle optimiste et rejet si conflit) ; le changeset porte l'auteur, le motif et le temps applicatif visé ; c'est l'unité du temps système (D10) et de l'outbox.
- Rejeté : écriture directe (revue impossible plus tard sans refonte), branches façon Git (fusion bitemporelle trop complexe).
- Décidé par : recommandation acceptée

## D12 — Ancres du noyau
- Décision : le noyau contient trois axes d'organisation : **entité** (propriétaire), **domaine d'adressage** (routabilité) et **site** (emplacement hiérarchique : région > site > bâtiment…). Ils servent d'ancres de périmètre au RBAC et de portée aux groupes de VLAN. Le détail physique (baies, étages, coordonnées) va dans un module.
- Rejeté : site en module (fragilise le RBAC), arbres génériques (moins typés, sur-ingénierie).
- Décidé par : recommandation acceptée

## D13 — Authentification
- Décision : **humains** via OIDC (authorization code + PKCE, RFC 7636 ; OIDC Core 1.0), avec **plusieurs émetteurs de confiance** configurables. Chaque émetteur est rattaché à des entités et ses claims sont mappés sur les rôles et périmètres. **Machines** (CLI, connecteurs, scripts) via des comptes de service en **Basic Auth sur TLS** (RFC 7617), secrets hachés en Argon2id (RFC 9106). Comptes humains locaux en Basic seulement pour le secours (break-glass).
- Pourquoi : les opérateurs externes s'authentifient chez eux, ce qui évite de gérer leurs mots de passe ; Basic reste le standard simple pour l'automatisation.
- Conséquences : validation des JWT par émetteur (JWKS en cache, contrôle `iss`/`aud`, RFC 8725) ; un même humain peut-il venir de deux émetteurs ? À traiter comme deux identités. Les instances sans état (D9) valident les jetons localement.
- Rejeté : un seul IdP fédéré (dépendance forte à l'IdP central), comptes locaux pour les externes (cycle de vie, pas de MFA).
- Décidé par : recommandation acceptée
- Amendement (spec 005, K8, 2026-10-17) : les secrets des comptes de service sont générés par kartenn (256 bits) et stockés en HMAC-SHA-256 avec une clé serveur ; Argon2id reste pour les mots de passe humains de secours.
- Décidé par (amendement) : utilisateur, sur recommandation

## D14 — Budgets de performance (p95 serveur, 10⁶ objets et 3 ans d'historique)
- Lecture d'un objet < 50 ms ; page de liste filtrée (curseur, 100) < 150 ms ; sous-arbre de préfixes et occupation < 200 ms ; « à la date X » ≤ ×2 la lecture courante ; changeset unitaire < 200 ms ; import de 10⁶ objets < 15 min ; 200 utilisateurs concurrents et 50 écritures/s soutenues ; vue graphique fluide à 10⁴ nœuds, agrégation au-delà.
- Ces budgets sont vérifiés par des bancs d'essai reproductibles en CI, sur un jeu de données synthétique à l'échelle.
- Décidé par : recommandation acceptée

## D15 — PostgreSQL ≥ 18 comme premier adaptateur
- Décision : PostgreSQL 18 ou plus. Types `inet`/`cidr`, index GiST et SP-GiST, contraintes d'exclusion (non-chevauchement par domaine), contraintes temporelles `WITHOUT OVERLAPS` et clés étrangères `PERIOD`, partitionnement, réplicas en lecture.
- Défense en profondeur : les invariants sont validés dans l'application (messages riches) **et** garantis par des contraintes en base (dernier rempart, y compris en concurrence).
- Limite connue : IPv8 n'est pas couvert par `inet`, l'adaptateur devra prévoir une représentation dédiée le moment venu (D5).
- À vérifier pendant les specs : le détail des contraintes temporelles de PG 18 (documentation officielle) et leur combinaison avec `inet && inet` dans une exclusion GiST.
- Rejeté : bases distribuées (invariants hors base, inutile à 10⁶), bases orientées graphe (envisageables plus tard comme projection en lecture).
- Décidé par : recommandation acceptée

## D16 — Backend en Rust
- Décision : Rust, en workspace Cargo. Chaque module (noyau, VLAN, NAT…) est une ou plusieurs crates ; sélection au build par features. Pistes d'écosystème, à confirmer par des ADR : axum/tower (HTTP), sqlx (PostgreSQL, requêtes vérifiées), serde, utoipa (OpenAPI).
- Pourquoi : performance, invariants encodés dans les types (« parse, don't validate »), langage déjà maîtrisé par l'utilisateur.
- Rejeté : Go (typage moins expressif), Python (marge de performance plus faible).
- Décidé par : recommandation acceptée

## D17 — Frontend
- Décision : shell en **Vue 3 + TypeScript**. Les plugins UI sont des **modules ES chargés dynamiquement qui enregistrent des Custom Elements** (Web Components), sans dépendre d'un framework. Un **SDK TypeScript versionné** fournit le client API généré, le contexte (utilisateur, `valid_at`/`as_of`, objet courant) et les points d'extension. Les plugins sont déclarés par l'endpoint capacités et protégés par CSP et SRI.
- Rejeté : React + Module Federation (contrat lié au build et aux versions partagées), Svelte (écosystème plus petit).
- Décidé par : recommandation acceptée

## D18 — Modules métier du MVP
- Décision : noyau + **VLAN** + **Machines et interfaces**. Passerelles, routes et NAT viendront en v2, une fois le contrat de module éprouvé.
- Décidé par : recommandation acceptée

## D19 — Nouveau besoin : vérifications de sécurité
- Besoin : les référentiels actuels ne permettent pas d'y intégrer des vérifications de sécurité. Sous forme de module, il faut pouvoir : (1) **classer** les éléments réseau (environnement : sandbox, recette, production…) ; (2) déclarer une **matrice de flux** (quelles classes peuvent communiquer, avec quelles catégories de flux, par exemple « administration » = SSH, RDP) ; (3) **répondre à des questions** du type « la machine A (prod) peut-elle ouvrir un flux d'administration vers la machine B (dev) selon la matrice ? ».
- Impact sur le noyau (porte à sens unique) : il faut un mécanisme de **classification typée attachable à tout objet, avec héritage** (entité → site → domaine → préfixe → adresse → machine), résolu à une date donnée (bitemporel). Sans cela, le module sécurité serait impossible ou dupliquerait le modèle. C'est le point B « tags et attributs » remonté en A.
- Statut : à approfondir (D20 et suivantes).
- Amendé par 002/A10 : les propriétés sont **typées** (liste fermée, texte, FQDN…), validées par le module qui les déclare, héritables ou non ; tout attribut descriptif du noyau devient une propriété.

## D20 — Sens de « peut communiquer »
- Décision : **étape 1, conformité à la politique** : résolution des classifications héritées de A et de B, consultation de la matrice, et réponse expliquée (règle appliquée, chaîne d'héritage). **Étape 2, module ultérieur : atteignabilité effective** (routes, NAT, pare-feu) confrontée à la politique.
- Conséquence sur le noyau : le contrat de graphe de topologie (point B) doit permettre plus tard une analyse de chemins. Les modules passerelles, routes et NAT (v2) devront fournir des arêtes exploitables pour l'atteignabilité, pas seulement pour l'affichage.
- Rejeté : politique seule (fermerait la porte), atteignabilité dès le départ (projet à part entière).
- Décidé par : recommandation acceptée

## D21 — Environnements : nature et finalité
- Constat utilisateur : le module sécurité (environnements et matrice de flux) est **l'objectif final** du logiciel pour l'utilisateur, mais il doit cohabiter avec les besoins des autres équipes (réseau). C'est un problème à part entière, à concevoir ensemble avant de figer le reste.
- Décision : un environnement est une **propriété d'un nœud** de l'arbre (supernets → subnets → … → machines), **non ordonnée**. On l'affecte à un nœud et elle est héritée par la sous-branche ; une sous-branche peut être réaffinée avec un autre environnement.
- Existant de l'utilisateur, à reprendre dans le modèle :
  1. un outil d'**heuristiques en TOML** qui déduit l'environnement attendu (dev, prod…) ;
  2. une **matrice de flux** à double entrée (environnement source × environnement destination) qui indique, par **type de flux** (base de données, applicatif, fichier, administratif), si c'est autorisé ;
  3. un **catalogue de protocoles** classés par type de flux, chacun autorisé ou interdit.
- Rejeté : valeurs ordonnées, étiquettes libres.
- Décidé par : l'utilisateur

## D22 — Résolution pour les machines multi-attachées
- Décision : l'environnement se résout **par adresse ou interface**, feuille d'un vrai arbre (domaine → préfixes → adresse). Les requêtes de flux portent sur les adresses source et destination ; « machine A vers machine B » évalue toutes les paires d'adresses et renvoie un verdict par paire, plus une synthèse. Une machine peut déclarer un environnement : si ses adresses résolvent vers des environnements différents, c'est une **anomalie**, sauf pour une interface marquée d'un **rôle** explicite (administration, sauvegarde, supervision…).
- Conséquence sur le noyau : l'arbre d'héritage, c'est l'arbre des préfixes plus les adresses, par domaine d'adressage. Les machines sont rattachées aux adresses, elles ne sont pas des nœuds de l'arbre. Le rôle d'interface est un attribut du module Machines et interfaces, exploitable par le module politique.
- Rejeté : un environnement unique par machine (interdit bastions et sauvegarde), ensemble d'environnements (un bastion devient un pont).
- Décidé par : recommandation acceptée

## D23 — Le déclaré fait foi, l'inféré contrôle
- Décision : les verdicts de flux se fondent **uniquement sur l'environnement déclaré** (explicite ou hérité). Les règles d'inférence produisent un environnement **attendu** : un écart donne une anomalie, avec la règle en cause. Pour un nœud non déclaré, l'inférence donne une **suggestion** acceptable par changeset ; elle n'écrit jamais seule.
- Rejeté : l'inféré qui comble les trous (dérive silencieuse des verdicts), l'inféré qui fait foi (contraire à D1).
- Décidé par : recommandation acceptée

## D24 — Règles d'inférence
- Existant de l'utilisateur : un DSL TOML. Une règle = des contraintes (champ `cidr` avec `subnet_of`, champs `name` et `site` avec `regex`…) et une liste de tags. Un élément qui correspond reçoit les tags, **ainsi que tous ses enfants**. Les exceptions sont **imbriquées** (`[regle.exception]`, limitées aux éléments de la règle parente) ou **implicites par ordre de lecture** (une règle lue plus tard l'emporte).
- Décision : le modèle peut changer, mais il **doit garder l'héritage et les exceptions**. Les règles sont des données (DSL déclaratif fermé, importable depuis le format existant) ; critères en liste fermée et extensible ; ET entre contraintes ; exceptions imbriquées. L'ordre de lecture implicite est remplacé par une **priorité explicite** (ou la spécificité), pour qu'un import, un tri ou une édition concurrente ne change pas le résultat. C'est une recommandation à valider dans la spec.
- Usages : (1) **autocomplétion** du champ environnement dans le frontend ; (2) **valeur par défaut** si une requête d'écriture ne fournit pas d'environnement. La valeur est alors écrite dans le changeset avec la provenance « règle X, version Y », et une modification ultérieure de la règle ne réécrit pas silencieusement le passé ; (3) audit des écarts (D23).
- Décidé par : l'utilisateur (fonctionnalités) ; priorité explicite recommandée

## D25 — Environnements multiples, combinaison permissive
- Décision : un nœud porte un **ensemble** d'environnements (par exemple {prod, global}), tous définis dans la matrice. Le verdict A → B pour un type de flux est **autorisé si au moins une paire** (env de A, env de B) est autorisée.
- Risque accepté, à mitiger : ajouter un environnement à un nœud peut **ouvrir** des flux. Mitigations prévues : (1) le verdict cite la paire qui autorise ; (2) toute modification d'un ensemble d'environnements, d'une règle ou de la matrice produit un **différentiel des flux ouverts ou fermés** dans le changeset, visible avant l'application et éventuellement soumis à approbation (D11) ; (3) un audit liste les nœuds dont l'autorisation ne tient qu'à un environnement transverse.
- Note : cela ne contredit pas D22. D22 refusait de fusionner implicitement les environnements de plusieurs interfaces d'une machine ; ici, l'ensemble est affecté explicitement à un nœud.
- Rejeté : strict, toutes les paires (ne correspond pas à l'usage actuel) ; priorité entre environnements.
- Décidé par : l'utilisateur

## D26 — Une affectation remplace l'ensemble hérité
- Décision : une affectation explicite ou une exception sur une sous-branche **remplace** entièrement l'ensemble hérité ({prod, global} puis exception {rec} donne {rec}). Il n'y a pas d'opérateur d'ajout ou de retrait.
- Conséquence : pour conserver un environnement transverse (« global ») dans une exception, il faut le répéter. L'explication de résolution montre l'ensemble hérité remplacé et le nœud qui le remplace.
- Rejeté : ajout et retrait explicites (non souhaités), ajout systématique (dangereux avec D25).
- Décidé par : l'utilisateur

## D27 — Classification obligatoire, réglée par domaine
- Décision : chaque domaine d'adressage indique si la classification est **obligatoire**. Si elle l'est, une écriture qui laisse un nœud sans environnement résolu (hérité, explicite ou par défaut via les règles) est **refusée**. Sinon, le nœud est « non classifié », signalé en anomalie, et ses flux sont **refusés par défaut**.
- Pourquoi : permet d'importer progressivement un SI racheté puis de durcir la contrainte.
- Décidé par : recommandation acceptée

## Note — réouverture de D24 à D26
L'utilisateur précise que D24 à D26 décrivaient **la pratique actuelle**, que l'on peut faire évoluer. Analyse :
- D24 : l'ordre de lecture est déjà remplacé par une priorité explicite.
- D25, combinaison permissive : on la **garde**. C'est la sémantique standard des appartenances multiples (union des autorisations, comme les security groups des clouds), **à condition que la matrice ne contienne que des autorisations et applique un refus par défaut**. Les refus explicites sont traités à part (D28).
- D26, remplacement : on le **garde**. C'est le choix le plus sûr avec une union permissive ; le coût (répéter « global ») est acceptable.

## D28 — Interdictions à deux niveaux, le refus l'emporte
- Décision : **niveau 1, garde-fous absolus** : protocoles interdits du catalogue, éventuellement limités à des paires d'environnements (« jamais d'administratif dev → prod »). Ils l'emportent toujours. **Niveau 2, matrice d'autorisations seulement** (type de flux × env source × env destination), combinée en union (D25), avec refus par défaut. Évaluation : garde-fous, puis matrice ; le verdict cite l'élément décisif.
- Rejeté : matrice tri-état (incompatible avec D25), matrice seule (pas de garde-fou).
- Décidé par : recommandation acceptée
- Amendé le 2026-10-21 par la 010 (entretien sécurité, Q2, Q7, Q9, Q11) — décidé par :
  l'utilisateur. Les deux niveaux et l'ordre d'évaluation tiennent (l'interdiction
  l'emporte, union des autorisations, refus par défaut). La **forme** change : la
  politique est un ensemble de **règles par attributs**, sans ordre entre elles ; une
  règle a un sélecteur source, un sélecteur destination (valeurs d'un ou plusieurs axes
  de classification, contrainte « même valeur » entre les deux extrémités), un sélecteur
  de flux (types de flux ou services) et un effet **autoriser** ou **interdire**. Les
  garde-fous sont les règles d'effet interdire ; la matrice devient une **vue** des règles
  qui ne citent que l'environnement. S'y ajoutent l'autorisation **conditionnelle sous
  motif** (sans motif correspondant, le flux part « à instruire » au lieu d'être refusé),
  les **motifs** d'architecture nommés, les **conditions d'admissibilité** (évaluées hors
  verdict, elles décident de l'issue : automatique, à instruire, non conforme) et un
  **texte pédagogique** par interdiction et par motif. La matrice à trois états de
  l'existant s'importe sans troisième effet (« partiel » = autorisation conditionnelle).

## D29 — Catalogue de services nommés
- Décision : une entrée du catalogue est un **service nommé** : transport (tcp, udp, sctp, icmp…), ports ou plages (registre IANA comme référence) et **un seul** type de flux. Un même port peut figurer dans plusieurs services. Une requête par service n'est pas ambiguë ; une requête par port brut renvoie un verdict par service correspondant, avec une synthèse sur le plus restrictif. Plus tard, une machine pourra déclarer les services qu'elle expose.
- Rejeté : port associé à un seul type (faux pour 22 et 443), requête par type seulement (rend les garde-fous de D28 inapplicables).
- Décidé par : recommandation acceptée

## D30 — Registre de flux déclarés
- Décision : un module **Flux** distinct, qui dépend de Politique : un flux = source, destination, service, justification, demandeur, période de validité (temps applicatif). Cycle de vie par changesets (demandé → approuvé → actif → expiré). Tout changement d'environnement, de règle, de matrice ou de garde-fou **réévalue les flux touchés**, et le différentiel du changeset les nomme. C'est le socle de l'atteignabilité future (D20).
- Conséquence de performance : la résolution des environnements doit être **matérialisée et maintenue de façon incrémentale** (index environnement → nœuds, et inversement), pour calculer les différentiels sur 10⁶ nœuds dans les budgets de D14. À traiter dans la spec du module.
- Rejeté : questions ponctuelles seules (on ne sait pas ce qu'un changement casse), import des pare-feux dès maintenant (relève de l'étape 2).
- Décidé par : recommandation acceptée
- Amendé le 2026-10-21 par la 010 et la 011 (entretien sécurité, Q3, Q12, Q13, Q16,
  Q18) — décidé par : l'utilisateur. Un flux **conforme** n'a plus de période de validité
  obligatoire : sa validité est **dérivée en continu** de la politique (il reste actif
  tant que la politique du jour l'autorise et que ses extrémités existent) ; s'il
  bascule, il devient une non-conformité à instruire. Seuls les flux **instruits** par un
  humain et les **dérogations** portent une date de fin obligatoire et une
  recertification humaine ; rien ne se prolonge seul. La référence externe du dossier
  devient obligatoire, la justification en texte libre facultative. « Tout changement de
  règle réévalue les flux » vaut pour la politique (règles, motifs, conditions, services)
  et les classifications ; une règle d'inférence ne change aucun verdict et ne réévalue
  que les anomalies (D23). Le registre réserve une **sorte** « constaté » (flux déjà
  ouvert, audité a posteriori) pour la v0.3.

## D31 — Feuille de route
- Décision : **v0.1 (MVP), orientée réseau** : noyau (dont le mécanisme de **propriétés héritables**, construit et testé dès v0.1 même sans le module Politique), VLAN, Machines et interfaces, REST et Web. **v0.2, sécurité** : Politique (environnements, règles, matrice, garde-fous, services, conformité, anomalies) et Flux. **v0.3** : passerelles, routes, NAT. **Ensuite** : atteignabilité effective, connecteur de la source historique, MCP, CLI, TUI.
- Pourquoi : convaincre d'abord l'équipe réseau, dont l'adoption conditionne celle de l'outil.
- Garde-fou : les specs de Politique et Flux sont **écrites avant de figer le noyau** de v0.1, pour que le noyau les accueille sans refonte. Un prototype de résolution d'héritage sur 10⁶ nœuds est fait en v0.1.
- Rejeté : Politique dans le MVP, tout dans le MVP.
- Décidé par : l'utilisateur
- Amendement (spec 022, A7, 2026-10-23) : les **événements poussés** (spec 022, flux SSE alimenté par l'outbox) sont une brique transverse qui précède les specs de réécriture du web (020, 021, à venir) ; la bibliothèque cliente et ses commandes `watch` en consomment le contrat.
- Décidé par (amendement) : l'utilisateur

## D32 — Nom et emplacement
- Décision : **kartenn** (« carte » en breton), dans `~/Repos/kartenn`, en monorepo git avec devenv. Structure : `crates/` (kartenn-core, kartenn-<module>, kartenn-server…), `web/` (shell et SDK de plugins), `docs/` (spec système, ADR, notes RFC et bonnes pratiques), `specs/` (specs de fonctionnalités), `devenv.nix` (Rust, Node, PostgreSQL 18).
- Décidé par : l'utilisateur (consonance bretonne), nom recommandé

## D33 — Suppression
- Décision : par défaut, **refus** s'il existe des dépendants (erreur RFC 9457 qui les liste). Une **cascade explicite** est possible : le changeset contient toutes les suppressions, le différentiel les montre, le RBAC s'applique à chacune. Supprimer veut dire **clore la période de validité** (bitemporel), sans effacement physique. Chaque module déclare la politique de ses références (refus, cascade, mise à nul).
- Rejeté : cascade implicite (dangereuse), re-parentage automatique (reclassification silencieuse).
- Décidé par : recommandation acceptée

## D34 — Arbre de préfixes déduit de l'inclusion
- Décision : le parent d'un préfixe ou d'une adresse est **toujours le plus petit préfixe englobant du même domaine**, à l'instant considéré. Rien n'est stocké comme vérité (une matérialisation en cache est permise). Insérer ou retirer un préfixe intermédiaire modifie l'héritage, et le changeset affiche le **différentiel de reclassification**, avec approbation possible.
- Précision sur D4 : deux blocs CIDR sont soit imbriqués, soit disjoints. L'invariant devient donc : **pas de doublon de préfixe par domaine et par période**, et **pas de chevauchement partiel entre plages** non CIDR.
- Rejeté : parent explicite (divergence possible avec la réalité CIDR).
- Décidé par : recommandation acceptée

## D35 — Déploiement et observabilité
- Décision : images **OCI** (serveur, job de migration, web statique), manifestes d'exemple (Compose pour le dev, Helm ou Kustomize pour Kubernetes), **module NixOS**. Configuration 12-factor (fichier et variables d'environnement). **OpenTelemetry** (OTLP) pour les traces, métriques et logs JSON ; sondes `/health/live` et `/health/ready` ; métriques métier.
- Décidé par : recommandation acceptée
- Précision (spec 012, 2026-10-17) : OpenTelemetry est une sortie parmi d'autres, toutes actives à la fois : stdout (texte ou JSON), fichier, journald, syslog RFC 5424 (local, UDP, TCP, TLS) et OTLP/gRPC, chacune avec son filtre et sa file bornée. Le journal d'audit, distinct des logs techniques, est en base, scellé et exporté (ADR 0019). Configuration : fichier TOML et variables `KARTENN_*`. Console de développement : collecteur OpenTelemetry et OpenObserve dans devenv.
- Décidé par : délégué (2026-10-17), spec 012 approuvée
- Amendement (spec 015, ADR 0029, 2026-10-21) : **une seule image OCI** non root et sans
  shell (serveur et shell web), qui sert aussi de job de migration (`migrate`), au lieu
  de trois ; construite par un `flake.nix` qui suit le nixpkgs de devenv ; module NixOS
  `services.kartenn` (migration en unité oneshot, `DynamicUser`, secrets par
  `LoadCredential`) ; URL de la base lue d'un fichier (`[database] url_file`), jamais sur
  la ligne de commande. Registre et signature de l'image : question ouverte (015/E13).
- Décidé par : l'utilisateur (2026-10-21)

## D36 — RBAC
- Décision : chaque module déclare un **catalogue de permissions** ; un **rôle** est un ensemble nommé de permissions (rôles livrés : lecteur, éditeur, approbateur, admin ; rôles personnalisés possibles) ; une **liaison** associe un sujet (utilisateur, groupe OIDC, compte de service) à un rôle sur un **périmètre** (entité, site, domaine, sous-arbre de préfixes), hérité vers le bas. Les politiques d'approbation (D11) s'expriment dans le même modèle. Le filtrage des listes par droits est traduit en prédicats SQL pour tenir les budgets de D14.
- Rejeté : rôles fixes (trop grossiers), OPA ou Cedar (filtrage de listes coûteux ; envisageable plus tard comme adaptateur).
- Décidé par : recommandation acceptée

## Approche — module « Politique de flux »
Découpage noyau / module :
- **Noyau : propriétés héritables**. Mécanisme générique : une propriété a un vocabulaire fermé, est affectée explicitement à un nœud, héritée par les descendants, peut être surchargée dans une sous-branche, et sa résolution est **expliquée** (valeur, nœud d'origine, chaîne d'héritage) à une date donnée (bitemporel). Le noyau ne connaît pas « environnement » : c'est le module qui le déclare.
- **Module politique de flux** :
  - vocabulaire des environnements (liste fermée, administrable) ;
  - types de flux (liste fermée : BDD, applicatif, fichier, administratif…) ;
  - catalogue de protocoles (transport, ports ou plages, éventuellement identifiant applicatif) → type de flux, et statut autorisé ou interdit ;
  - matrice orientée (env source, env destination, type de flux) → autoriser ou refuser ; **refus par défaut** pour toute cellule absente ;
  - règles d'inférence (reprise des heuristiques TOML) qui produisent un environnement **attendu**, confronté à l'environnement **déclaré** : un écart devient une **anomalie** signalée ;
  - requête de conformité : (A, B, protocole ou type de flux, date) → verdict, règle appliquée, chaîne d'héritage de A et de B ;
  - audit de masse : toutes les anomalies (écarts d'inférence, nœuds sans environnement, cellules de matrice non définies…).
- La matrice, le catalogue et les règles sont eux aussi **versionnés en bitemporel** (« la politique au 1er mars »).

Les questions de conception de ce module sont tranchées par D22 à D30.

## Architecture logique
```
                 ┌────────────── Clients ───────────────┐
 Navigateur ──► Web shell (SPA) + plugins UI à chaud ───┐│
 CLI / TUI / MCP (binaires séparés) ────────────────────┤│
 Connecteurs (workers hors processus) ──────────────────┤│
                                                        ▼▼
 ┌──────────────────── Serveur applicatif ────────────────────┐
 │ Adaptateurs entrants : REST (contrat public unique)         │
 │ AuthN (Basic sur TLS, OIDC/JWT multi-émetteurs) → AuthZ RBAC │
 │ Couche service : cas d'usage, pipeline d'écriture,          │
 │   validation, transactions, audit, outbox                    │
 │ Domaine : noyau (familles d'adresses, entités, domaines,    │
 │   préfixes, adresses) + modules métier compilés (VLAN, NAT…)│
 │ Ports de stockage (interfaces) ─► adaptateur BDD (PostgreSQL ≥ 18, D15)   │
 │ Workers : relais outbox, jobs (imports, rapports)           │
 └──────────────────────────────────────────────────────────────┘
         ▼                        ▼
   Base de données          IdP OIDC (externes, N émetteurs)
```
Modularité par couche :
| Couche | Modèle | Ajout d'un module |
|---|---|---|
| Domaine, données, service | paquets compilés du monorepo, contrat typé, DAG de dépendances, migrations par module | recompilation et migration |
| Adaptateurs d'interface | binaires séparés, clients de l'API REST | indépendants |
| Frontend | shell plus plugins chargés à l'exécution via un SDK versionné (points d'extension : onglets, vues, panneaux) ; un plugin n'a que les droits de l'utilisateur via l'API | à chaud |
| Connecteurs | processus externes s'appuyant sur l'API (lots, identifiants externes) | à chaud |

## Noyau ou module
Critère retenu : relève du **noyau** tout ce qui est coûteux à rattraper (« porte à sens unique »), parce que chaque module en dépend ou parce que le changer impose de migrer toutes les données ou de casser les contrats d'API.

**A. Noyau, conçu ET construit maintenant**
1. Identité des objets : UUID stable, plus correspondance avec des identifiants externes pour les connecteurs.
2. Abstraction des familles d'adresses (D5).
3. Entités, domaines d'adressage, arbre de préfixes, adresses (D4).
4. Moteur de validation : règles locales (champ) et invariants transactionnels globaux. Les modules peuvent l'étendre.
5. Pipeline d'écriture unique : authentification → autorisation → validation → persistance → audit → événement.
6. Modèle RBAC : rôle × action × type d'objet × périmètre. Chaque objet doit avoir une « ancre » de périmètre (entité, domaine, sous-arbre de préfixes).
7. Historique et audit (qui, quoi, quand, avant/après).
8. Concurrence optimiste (ETag/If-Match, RFC 9110) et lots atomiques (imports de 10⁶ objets).
9. Événements sortants (outbox) : base des connecteurs, webhooks et caches.
10. Contrat d'API transverse : erreurs RFC 9457, pagination par curseur, filtrage, tri, versionnement, OpenAPI généré.
11. Couche service indépendante des interfaces (hexagonale) : REST, Web, MCP, CLI et TUI ne sont que des adaptateurs.

**B. Contrats conçus maintenant, implémentation plus tard (points d'extension)**
- Contrat de module : schéma et migrations propres, validateurs, permissions, routes d'API, vues Web, contribution au graphe de topologie, dépendances déclarées (DAG).
- Références entre modules (le NAT pointe vers des adresses du noyau) et règles de suppression.
- Contrat de connecteur (import, synchronisation, réconciliation, identifiants externes).
- Contrat de graphe de visualisation (nœuds et arêtes fournis par chaque module).
- Recherche et filtres transverses ; tags et attributs personnalisés.

**C. Modules purs, peu coûteux plus tard si A et B sont faits**
VLAN, machines et interfaces, passerelles et routes, NAT, détail physique des sites (D12), DNS, DHCP, circuits, cloud, connecteurs (dont la source historique), rapports et exports, découverte ; interfaces MCP, CLI et TUI.

**Questions portes à sens unique, toutes tranchées** : Q-a donne D6 ; Q-b donne D8 et D10 ; Q-c donne D11 ; Q-d donne D12. Mise à jour par D19 : « propriétés héritables » passe de B à A (noyau). Le contrat de graphe (B) doit permettre l'analyse de chemins (D20).

## D37 — Licence
- Décision : **AGPL-3.0**.
- Conséquence : un plugin UI ou un connecteur qui passe par l'API REST n'est a priori pas une œuvre dérivée, mais le SDK TypeScript et le client généré, s'ils sont embarqués dans des plugins tiers, posent la question (voir Q-ouv-1).
- Décidé par : l'utilisateur

## Questions ouvertes
- ~~**Q-ouv-1**~~ : licence du SDK de plugins et des clients générés (AGPL, ou MPL-2.0 ou Apache-2.0 pour permettre des plugins et connecteurs propriétaires). Bloque : la spec 007 (web shell et plugins). Close le 2026-10-19 (utilisateur) : **MPL-2.0** pour le SDK de plugins et les clients générés ; le shell reste en AGPL-3.0 (D37).
- **Q-ouv-2** : volumétrie réelle de la source historique, pour confirmer D2. Bloque : le jeu de données des bancs d'essai.
- ~~**Q-ouv-3**~~ : critères exacts des règles d'inférence (exemples anonymisés du TOML existant). Bloque : la spec 010. Close le 2026-10-21 (utilisateur, entretien sécurité de la 010, Q10 à Q13) : exemples anonymisés consignés dans `specs/010-politique-flux/besoin/journal.md` (Q10) ; moteur générique de règles d'inférence (critères en liste fermée extensible, ET entre critères, exceptions imbriquées, priorité explicite puis nœud le plus profond, égalité non bloquante) ; import de l'existant par convertisseur hors ligne.
- **Q-ouv-4** : représentation de stockage des familles non natives (IPv8). Bloque : rien avant que le draft progresse ; à concevoir comme point d'extension dans la spec 002.
- ~~**Q-ouv-5**~~ : une même personne authentifiée par deux émetteurs OIDC = deux identités ou une identité liée ? Bloque : la spec 005. Close par 005/K2 (deux identités).
- **Q-ouv-6** : langue de l'interface et de la documentation (FR, EN, i18n). Bloque : la spec 007 et la convention de `docs/`. Interface tranchée le 2026-10-19 (utilisateur) : **i18n dès le départ**, textes par clés, catalogues français et anglais livrés, catalogues propres à chaque plugin. Reste ouverte pour la documentation (en français pour l'instant).
- ~~**Q-ouv-7**~~ : stratégie de versionnement de l'API (`/v1` dans l'URL ou par type de média). Bloque : la spec 006, à trancher après lecture des guides. Close par 006/H12 et l'ADR 0022 : version majeure dans le chemin (`/v1`), évolution additive.

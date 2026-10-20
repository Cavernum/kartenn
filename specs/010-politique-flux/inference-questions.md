---
status: draft
updated: 2026-10-20
---

# 010 — Règles d'inférence d'environnement : questions à arbitrer

Point d'entrée autonome de la session dédiée aux règles d'inférence (Q-ouv-3, D24),
réservée par l'utilisateur le 2026-10-20. Ce document ne décide rien : il pose les
questions dans l'ordre où elles se tranchent, avec les options et une recommandation.
Les réponses iront dans [`decisions.md`](decisions.md) (décisions S22 et suivantes),
puis dans `requirements.md` (section réservée « G. Inférence ») et `design.md`.

## Contexte

kartenn est un référentiel réseau déclaratif (D1). Le module Politique (spec 010) classe
les nœuds de l'arbre d'adressage (domaine → conteneurs → réseau → adresse) par
**environnement**, propriété `policy.environment` : ensemble de valeurs d'une liste
fermée, héritable, une affectation remplaçant l'ensemble hérité (D21, D25, D26, 010/S2).
Les verdicts de flux ne lisent **que** l'environnement déclaré (D23).

L'inférence produit un environnement **attendu**, à partir de règles écrites comme des
données. Elle sert à trois choses (D24) : (1) l'autocomplétion du champ environnement ;
(2) une valeur par défaut à la création, écrite dans le changeset avec la provenance
« règle X, version Y » ; (3) l'audit des écarts entre déclaré et attendu (anomalies,
D23).

Pratique actuelle (D24) : un DSL TOML ; une règle = des contraintes (`cidr` avec
`subnet_of`, `name` et `site` avec `regex`…) et une liste d'étiquettes ; un élément qui
correspond reçoit les étiquettes **ainsi que tous ses enfants** ; exceptions
**imbriquées** (`[regle.exception]`, limitées aux éléments de la règle parente) ou
**implicites par ordre de lecture** (une règle lue plus tard l'emporte).

Ce que la spec 010 tient déjà sans l'inférence : vocabulaires, catalogue de services,
garde-fous, matrice, requête de conformité, différentiel de politique, audit (010/S1 à
S17). Ce que le noyau offre (spec 003) : le contrat `ExpectedValueSource`
(`crates/kartenn-service/src/expected.rs`), trois modes par déclaration (`suggest`,
`default`, `check`), l'anomalie `prop.resolution.expected_mismatch`, la provenance
`source:<id>@<version>` dans l'explication (REQ-003-14, 21 à 24).

### Décisions liées
- D19 : classer les éléments réseau, matrice, questions de conformité.
- D21 : environnement = propriété d'un nœud, non ordonnée, héritée, réaffinable.
- D22 : résolution par adresse ; la machine n'est pas un nœud de l'arbre.
- D23 : le déclaré fait foi, l'inféré contrôle ; suggestion jamais écrite seule.
- D24 : règles en données, héritage et exceptions conservés ; priorité explicite (ou
  spécificité) recommandée à la place de l'ordre de lecture.
- D25, D26 : ensembles, union permissive, remplacement de l'ensemble hérité.
- D27 : classification obligatoire par domaine ; la valeur par défaut des règles compte
  comme résolue.
- D30 : le différentiel d'un changeset nomme les flux touchés (voir 010/S21 : une règle
  d'inférence ne change aucun verdict).
- Q-ouv-3 : critères exacts de l'existant, à fournir sous forme anonymisée.

## Questions, dans l'ordre

### I1 — Propriétés visées
- Options : (a) `policy.environment` seulement ; (b) moteur générique pour toute
  propriété de liste fermée (ou ensemble) qui le désigne comme source.
- Recommandation : **(b)**, avec `policy.environment` comme seul usage livré. Le coût
  est nul (le contrat 003 est déjà par propriété) et d'autres classements (criticité,
  zone de sécurité) s'y brancheront.

### I2 — Objets évalués
- Options : (a) nœuds de l'arbre d'adressage (domaine, préfixes, adresses) ;
  (b) plus les machines (009), pour `policy.declared_environment`.
- Recommandation : **(a)** d'abord : c'est l'arbre de l'héritage (D22) ; (b) plus tard,
  avec des critères propres aux machines.

### I3 — Forme des prédicats
- Questions : quels critères ? quelles combinaisons ?
- Options de critères (liste fermée, extensible par module) : préfixe ou adresse inclus
  dans un CIDR (`within`), égal à un CIDR, famille, domaine, type d'objet (conteneur,
  réseau, adresse), site et entité résolus (avec leurs sous-arbres), valeur d'une
  propriété (égalité, appartenance, présence, expression régulière sur un texte ou un
  FQDN), longueur de préfixe ; plus tard VLAN (008) et attributs de machine (009).
- Combinaison : (a) ET seul (D24) ; (b) ET et OU ; (c) ET, OU, NON.
- Recommandation : **critères ci-dessus, ET seul plus une liste de valeurs par critère**
  (« site ∈ {A, B} » couvre l'essentiel du OU sans arbre booléen) ; expressions
  régulières au moteur à temps linéaire, ancrées, longueur bornée (pas de ReDoS).
- À fournir : les critères réellement employés par l'existant (Q-ouv-3).

### I4 — Sens de « et tous ses enfants »
- Options : (a) la règle s'évalue sur chaque nœud indépendamment (un enfant ne
  correspond que s'il satisfait lui-même les critères) ; (b) un nœud qui correspond
  transmet l'attendu à toute sa sous-branche, comme l'héritage (D24 actuel) ; un enfant
  peut être repris par une règle plus prioritaire ou une exception.
- Recommandation : **(b)**, fidèle à l'existant et cohérent avec l'héritage déclaré ;
  l'explication cite le nœud qui a correspondu.

### I5 — Exceptions
- Questions : imbriquées seulement ? profondeur ? effet ?
- Options : (a) exceptions imbriquées, à profondeur quelconque, limitées aux éléments
  de la règle parente, qui **remplacent** son ensemble (D26) ; (b) exceptions plus
  opérateurs d'ajout et de retrait.
- Recommandation : **(a)** ; profondeur bornée à 4 pour la lisibilité ; pas d'ajout ni
  de retrait (rejetés par D26).

### I6 — Priorité ou spécificité
- Options : (a) **priorité explicite d'abord**, puis profondeur du nœud qui correspond
  (le plus profond l'emporte), une égalité restante étant interdite à l'écriture ;
  (b) profondeur d'abord, puis priorité ; (c) priorité seule, unique par propriété.
- Recommandation : **(a)** : un import qui donne la priorité selon l'ordre de lecture
  reproduit exactement le comportement actuel (« la règle lue plus tard l'emporte »),
  et la spécificité ne sert qu'à départager. Entre exceptions sœurs, même règle.
- Point d'attention : (b) correspondrait mieux à l'héritage déclaré (le plus proche
  l'emporte), mais changerait des résultats de l'existant à l'import.

### I7 — Portée et administration des règles
- Options : (a) un jeu de règles global ; (b) des jeux par domaine (D4 : les espaces se
  recouvrent) ; (c) global avec un critère de domaine.
- Recommandation : **(c)** pour le modèle, avec une permission
  `policy.rule.admin` portée par domaine quand la règle cite un domaine, globale sinon.

### I8 — Confrontation déclaré / attendu
- Options : (a) égalité d'ensemble ; (b) l'attendu doit être inclus dans le déclaré ;
  (c) intersection non vide.
- Recommandation : **(a)**, la plus simple à expliquer ; « aucun attendu » ne produit
  jamais d'anomalie.

### I9 — Anomalies
- Questions : granularité, sévérité, acquittement, volume.
- Options : (a) une anomalie par nœud en écart ; (b) une par **nœud d'origine** de la
  valeur déclarée (les héritiers d'une même origine sont regroupés) ; sévérité
  avertissement (REQ-003-24).
- Recommandation : **(b)** avec le décompte des héritiers ; acquittement par
  REQ-002-36 ; réévaluation par tâche de fond quand les règles changent.

### I10 — Explicabilité
- Recommandation : l'explication d'un attendu cite la règle (et la chaîne
  d'exceptions), sa version, le nœud qui a correspondu, chaque critère avec la valeur
  observée, et les règles plus faibles écartées (trois au plus).

### I11 — Versionnement bitemporel
- Questions : comment versionner les règles ? que vaut « version Y » dans la provenance ?
  à quelle date évaluer ?
- Options pour la version : (a) révision globale du jeu de règles (compteur incrémenté
  par tout changeset qui touche une règle) ; (b) version propre à chaque règle.
- Options pour la date : (a) attendu au `valid_at` demandé selon les règles valides à ce
  `valid_at` ; (b) selon les règles d'aujourd'hui.
- Recommandation : règles **objets bitemporels** écrits par changeset ; provenance
  `source:policy.rules@<révision>#<règle>` ; évaluation **(a)** pour les deux axes, comme
  toute lecture datée. Une valeur par défaut écrite reste figée (REQ-003-22).

### I12 — Valeur par défaut
- Questions : seulement à la création ? qu'en est-il d'un objet existant dont
  l'affectation est retirée ? interaction avec l'obligation (D27) ?
- Recommandation : **création seule** (REQ-003-22) ; une valeur par défaut n'est posée
  que si l'objet n'hériterait d'aucune valeur (sinon l'héritage déclaré prime, D23) ;
  elle compte pour l'obligation (D27).
- Point d'attention : la seconde clause est plus restrictive que REQ-003-22 (qui
  affecte dès qu'il n'y a pas d'affectation propre) ; à confirmer.

### I13 — Import de l'existant
- Options : (a) type d'import serveur (`POST /v1/imports`, genre `policy.rules`) qui lit
  le format TOML ; (b) convertisseur hors ligne qui produit un changeset JSON.
- Recommandation : **(b)** : le format historique ne devient pas un contrat du serveur ;
  le convertisseur traduit l'ordre de lecture en priorités (I6) et, en simulation,
  liste les nœuds dont l'attendu diffère de l'outil actuel.
- À fournir : exemples anonymisés (Q-ouv-3).

### I14 — Budgets
- Proposition : suggestion pour un objet < 50 ms ; contrôle `check` sur les objets d'un
  changeset de 10⁴ objets < 1 s ajoutée ; réévaluation complète de 10⁶ nœuds en tâche
  de fond < 10 min ; 2 000 règles au plus.

### I15 — Ce que le contrat 003 doit offrir
Constat sur `ExpectedValueSource` (v0.1) : appel synchrone, objet seul (pas ses
ancêtres), version statique (`&str` fixé à la compilation), aucune lecture des données
du module. Pour (b) de I4, I6 et I11, il faudra :
- lire les ancêtres du nœud à la date et leurs propriétés résolues ;
- une version calculée à l'exécution (révision du jeu de règles) ;
- lire les règles du module dans la transaction du changeset (une règle écrite dans le
  même changeset s'applique à ses créations) ;
- une évaluation par lot (tâche de fond, contrôle de 10⁴ objets).
- Recommandation : amender la 003 (contrat additif) au moment de la session, pas avant ;
  aucun autre élément de la 010 n'en dépend.

## Questions renvoyées à cette session par l'utilisateur (2026-10-20)

- **S21 (tension D23 / D30)** : D30 fait réévaluer les flux à chaque changement de
  « règle », alors que D23 dit qu'une règle d'inférence ne change aucun verdict. Lecture
  proposée : dans D30, « règle » désigne la matrice et les garde-fous. À confirmer ou à
  reformuler (voir `decisions.md`, S21).
- **S7 et S8 (rigueur par défaut)** : matrice sans joker ; un élément non classé est
  refusé ; un port sans service connu est refusé. À confirmer, ou à assouplir (jokers,
  traitement des non-classés), au regard de l'objectif d'une politique applicable à toute
  organisation.

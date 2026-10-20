---
status: draft
updated: 2026-10-20
---

# 010 — Besoin « politique de flux » : journal de l'entretien

Entretien de cadrage du besoin de sécurité (session du 2026-10-20), une question à la
fois, avec une recommandation à chaque question. Cadre : D19 à D31 et § « Approche —
module Politique de flux » de [`../../001-cadrage/decisions.md`](../../001-cadrage/decisions.md),
synthèses des specs 003 (propriétés) et 004 (changesets, bitemporel). Les décisions S1 à
S21 de [`../decisions.md`](../decisions.md) et F1 à F15 de la spec 011 ont été prises
par délégation, sans entretien : quand une réponse ci-dessous les contredit ou les
complète, [`ecarts.md`](ecarts.md) le note.

Le besoin consolidé est dans [`expression-besoin.md`](expression-besoin.md).

## Questions et réponses

### Q1 — Utilisateur principal de la v0.2
- Question : quel utilisateur ordonne les cas d'usage de la v0.2 ?
- Réponse : la réponse est un partage des rôles plutôt qu'un seul persona. L'**équipe
  sécurité** est responsable de la sécurité des flux du SI ; l'**équipe réseau** est
  chargée de la maintenance technique et de l'application des décisions de sécurité. Les
  décisions suivent un **triage** :
  1. si l'ouverture de flux ne dégrade pas la sécurité du SI, l'autorisation est
     **automatique**. Les critères : conformité à la matrice de flux (environnement
     source × environnement destination × quatre types de flux : administratif, base de
     données, applicatif, fichier) ; demande **correctement documentée** (ticket de mise
     en production portant les informations nécessaires) ; **score de risque** acceptable,
     à définir sur des critères comme la largeur des plages d'adresses ou le type de flux.
     Aujourd'hui seule la matrice sert, plus quelques règles fixes (par exemple aucune
     plage plus large qu'un /20). À terme, l'outil devrait reconnaître et appliquer des
     **motifs d'architecture d'entreprise** : les transferts de fichiers des nouvelles
     architectures passent par un service de stockage d'objets ; les flux de base de
     données restent confinés à une application identifiée ; l'exposition de données à
     d'autres applications passe par une API.
  2. sinon, **validation manuelle** : contact du demandeur, revue du besoin, orientation
     vers une bonne pratique ; à défaut, **dérogation** sous forme d'exception.
- Lecture : l'équipe sécurité décide, l'équipe réseau applique ; le cas d'usage qui
  ordonne les autres est l'**instruction d'une demande de flux**, avec un verdict
  expliqué et un triage automatique ou manuel.
- Décidé par : l'utilisateur

### Q2 — Articulation du triage dans la réponse de l'outil
- Question : comment articuler conformité à la matrice, dossier complet et risque
  acceptable dans la réponse à une demande de flux ?
- Réponse : **verdict binaire, conditions d'admissibilité séparées**. La politique garde
  un verdict autorisé ou refusé (D28 inchangé). Des conditions d'admissibilité (dossier
  complet, largeur de plage, score de risque) décident si un flux autorisé est enregistré
  automatiquement ou part en revue manuelle ; un flux refusé part toujours en revue
  (dérogation ou refus). Trois issues : **automatique**, **à instruire**, **non
  conforme**.
- Rejeté : verdict à trois états dans la politique (remet en cause D28 et D25) ; verdict
  binaire seul (le triage resterait humain).
- Décidé par : recommandation acceptée
- Complément de l'utilisateur : le **demandeur** est le « client » du dispositif. Il doit
  pouvoir remplir lui-même un **formulaire d'ouverture de flux** et être informé **au plus
  vite et le plus clairement possible** du statut de sécurité de son ouverture. L'issue
  du triage et son explication lui sont donc destinées en premier, pas seulement à
  l'équipe sécurité.

### Q3 — Contenu d'une demande « correctement documentée »
- Question : quels groupes d'informations rendre obligatoires pour l'admissibilité à
  l'autorisation automatique ?
- Réponse : les **quatre groupes** retenus : extrémités et service ; justification et
  références ; période et responsables ; **nature des données échangées** (sensibilité,
  caractère personnel, volume), qui nourrit le score de risque et un futur axe de
  classification. Nuance sur le **motif en texte libre** : il n'est **pas** demandé à
  kartenn. La solution est consommée **comme une API** depuis l'outil de ticketing (flux
  appliqués sur les pare-feux sur site) ou depuis la plateforme de dépôt de code (flux
  déclarés en infrastructure as code pour le cloud, revue par pull request) ; le motif
  reste dans le ticket ou la pull request. La **référence externe** devient donc
  l'élément obligatoire, le motif facultatif. Un connecteur (module ou microservice)
  vers ces outils est imaginable, mais faire tout dans la console de kartenn n'est pas
  le cœur du besoin aujourd'hui.
- Conséquences : le « formulaire du demandeur » (Q2) peut être celui de l'outil
  externe, kartenn répondant par API ; écart avec 011/F2 (justification texte non vide
  obligatoire), noté dans `ecarts.md`.
- Décidé par : l'utilisateur

### Q4 — Score de risque en v0.2
- Question : que doit faire la v0.2 du score de risque « à définir » ?
- Réponse : des **conditions d'admissibilité nommées** décident de l'issue (automatique ou
  à instruire) ; chaque condition non remplie est citée. En plus, un **score chiffré et des
  avertissements** sont produits **à destination de la revue humaine seulement** : ils
  n'influencent pas l'issue, ils aident l'équipe sécurité à repérer les points
  problématiques et les oublis du dossier.
- Rejeté : score pondéré décisionnel dès la v0.2 (poids difficiles à justifier et à
  auditer) ; hors périmètre (le besoin d'aide à la revue existe).
- Décidé par : l'utilisateur (recommandation amendée : score informatif ajouté)

### Q5 — Axes de classification au-delà de l'environnement
- Question : quels axes la politique doit-elle pouvoir lire dès la v0.2 ?
- Réponse : les **quatre** : exposition ou zone réseau ; sensibilité des données ;
  criticité ou niveau de confiance ; propriétaire ou application. Précision : les
  propriétaires et les applications sont des **données extérieures**, tenues dans des
  référentiels annexes et rapportées dans kartenn (référence, pas de maîtrise).
- Lecture : le mécanisme est générique (un axe = une propriété de liste fermée, ou une
  référence pour le propriétaire et l'application, déclarée au sens de P1) ; la politique
  peut citer n'importe quel axe. L'environnement reste le seul axe dont le vocabulaire
  est imposé par le module ; les autres sont de la configuration livrée en exemple.
- Décidé par : l'utilisateur (recommandation étendue aux quatre axes)

### Q6 — Porteurs des axes et valeur manquante
- Question : qui porte chaque axe, et que vaut le verdict quand un axe cité manque ?
- Réponse : recommandation telle quelle. **Environnement et exposition** : portés par
  l'arbre d'adressage, hérités (D21, D22). **Sensibilité, criticité, application,
  propriétaire** : portés par la machine ou l'application, lus par l'adresse (D22) ; une
  valeur posée sur un préfixe sert de défaut hérité. Une valeur manquante sur un axe cité
  par la politique vaut **« non classifié »** : refus par défaut et anomalie par axe (D27
  généralisé) ; l'**obligation** est réglable par domaine et par axe (P4).
- Rejeté : refus pour l'environnement seul (un oubli pourrait ouvrir un flux) ; valeur de
  repli par axe (contredit D23 sauf déclaration explicite).
- Décidé par : recommandation acceptée

### Q7 — Forme de la politique
- Question : quelle forme donner à la politique quand plusieurs axes entrent en jeu ?
- Réponse : **règles par attributs, deux effets, sans ordre**. Une règle = sélecteur
  source (valeurs sur un ou plusieurs axes), sélecteur destination, sélecteur de flux
  (types de flux ou services), effet **autoriser** ou **interdire**. Les interdictions
  l'emportent toujours (garde-fous, D28), les autorisations s'unissent (D25), refus par
  défaut. Aucun ordre entre règles : le résultat ne dépend pas de l'édition ni de
  l'import. La matrice actuelle devient une **vue** de saisie et de lecture des règles qui
  ne citent que l'environnement ; son import est direct.
- Rejeté : matrices par axe combinées (croisement d'axes impossible, combinaison stricte
  contre D25) ; règles ordonnées « première qui correspond » (l'ordre porte le sens).
- Généralise D28 et 010/S6, S7 (sélecteurs multi-axes au lieu de paires
  d'environnements) sans changer l'ordre d'évaluation ni les résultats actuels.
- Décidé par : recommandation acceptée

### Q8 — Dérogations
- Question : où vivent les dérogations et avec quelles contraintes ?
- Réponse : **au niveau du flux déclaré, jamais de la politique**. Une dérogation est un
  attribut d'un flux du registre (011/F6) : portée = ce flux seul ; date de fin
  obligatoire ; motif et risque accepté ; approbateur distinct du demandeur (C1) ;
  recertification à l'échéance. La politique reste intacte ; les audits listent les flux
  qui ne tiennent que par dérogation.
- Rejeté : règle d'autorisation temporaire (élargit la politique, verdict « autorisé »
  sans trace) ; les deux selon la portée (deux mécanismes à gouverner). Une règle de
  politique garde néanmoins une période de validité (bitemporel, S16) : c'est un
  changement de politique assumé, pas une dérogation.
- Décidé par : recommandation acceptée

### Q9 — Motifs d'architecture
- Question : comment exprimer les motifs d'architecture, qui comparent la source et la
  destination entre elles ?
- Réponse : **opérateur « même valeur » par axe, motifs = règles nommées**. Un sélecteur
  peut exiger que source et destination partagent la valeur d'un axe (même application,
  même entité) ou qu'une extrémité porte un **rôle de conduit** désigné (stockage
  d'objets, passerelle d'API, bastion). Un **motif** est un groupe nommé de règles
  (autorisations et interdictions) que l'explication cite par son nom. Couvre les trois
  exemples sans moteur d'expressions.
- Rejeté : valeurs fixes seules (duplication par application) ; langage d'expressions
  complet (explication, validation et budgets D14).
- Décidé par : recommandation acceptée

### Complément de l'utilisateur — accompagnement, pédagogie, audit a posteriori
- L'équipe sécurité est aussi une **équipe d'accompagnement**. Un refus doit
  s'accompagner d'un **enseignement des bonnes pratiques** et d'une sensibilisation : les
  non-conformités doivent être **documentées et aisément consultables** (par le
  demandeur comme par l'équipe).
- Il faut pouvoir **auditer a posteriori ce qui est déjà ouvert** : un flux existant est
  traité comme une demande sur laquelle on **appose une évaluation** ; on identifie ainsi
  les flux existants problématiques.
- Fil conducteur : une solution **assez souple pour laisser passer les bonnes pratiques,
  qui bloque les mauvaises** et les **rapatrie vers un accompagnement**, plutôt qu'un
  simple refus.
- Conséquences retenues : chaque règle d'interdiction et chaque motif portent un texte
  pédagogique (**bonne pratique attendue**, lien vers une documentation) restitué avec
  le verdict ; les non-conformités sont des objets consultables et filtrables, pas
  seulement des réponses d'API ; le registre accepte l'enregistrement d'un flux
  **constaté** (déjà ouvert) avec son évaluation, à côté des flux demandés.
- Décidé par : l'utilisateur

### Q10 — Exemples anonymisés de l'existant (Q-ouv-3)
- Question : exemples représentatifs des règles d'inférence, de la matrice et du
  catalogue actuels.
- Réponse (reformulée en motifs génériques) :
  - **Règle d'inférence** : une table nommée porte une liste de contraintes et une liste
    d'étiquettes. Contraintes vues : `name` avec une expression régulière ancrée en
    début (« le nom commence par le marqueur de production ») ; `cidr` avec
    `subnet_of = "<préfixe>/24"` ; `site` avec une expression régulière (D24). ET entre
    contraintes. Les étiquettes s'appliquent à l'élément et à ses enfants.
  - **Exception imbriquée** (`[regle.exception]`) : contraintes propres, étiquettes
    propres ; ne s'applique qu'aux éléments qui ont correspondu à la règle parente
    (« les éléments nommés production, sauf ceux du sous-réseau X, qui sont recette »).
    L'ensemble d'étiquettes est **remplacé** (D26).
  - **Exception par ordre de lecture** : une règle lue plus tard réécrit les étiquettes
    de tout élément qui lui correspond, qu'il ait ou non correspondu à une règle
    antérieure. L'ordre vient du nom des fichiers, préfixés par un nombre
    (`00-globals`, `10-sites`…) : la convention est **du général au particulier**, le
    plus particulier l'emportant. C'est une priorité par couche, implicite ; elle se
    traduit directement en priorité explicite (I6).
  - **Matrice** : cellules (environnement source, type de flux, environnement
    destination) à **trois états** : autorisé (`prod --app-> prod`), interdit
    (`rec --bdd-> prod`), **partiel** (`prod --fic-> global`) = soumis à revue manuelle
    pour vérifier le respect des motifs d'entreprise, idéalement automatisable.
  - **Catalogue** en deux tables : (port, transport udp/tcp, protocole nommé) et
    (protocole nommé, autorisé vrai/faux, description). Le protocole nommé correspond au
    service de D29 ; « autorisé = faux » est un garde-fou sans paire (S6).
- Décidé par : l'utilisateur (données fournies)

### Q11 — L'état « partiel » de la matrice
- Question : comment modéliser « partiel » avec un verdict binaire (Q2) ?
- Réponse : **autorisation conditionnelle, « autoriser sous motif »**. Une règle
  d'autorisation peut exiger un ou plusieurs motifs nommés (Q9). Si un motif requis
  correspond, le flux est autorisé et le verdict cite le motif ; sinon le flux n'est
  **pas refusé** mais part **à instruire**, avec la bonne pratique attendue. Une
  autorisation inconditionnelle qui correspond aussi l'emporte (union D25). D28 et D25
  tiennent ; « partiel » s'importe comme une cellule autorisée avec condition.
- Rejeté : partiel = refus orienté (chaque flux conforme deviendrait une dérogation) ;
  troisième effet (place dans l'union à définir, doublon des conditions de Q2).
- Décidé par : recommandation acceptée

### Q12 — Départage des règles d'inférence
- Question : comment départager deux règles d'inférence qui correspondent au même nœud ou
  à ses ancêtres ?
- Réponse de l'utilisateur : il ne sait pas départager. L'inférence sert avant tout à
  **préremplir** les champs ; les mainteneurs corrigent la valeur réelle dans
  l'interface Web. Les règles sont utiles mais **rarement maintenues** par l'équipe en
  l'absence de leur auteur.
- Décision (déléguée) : mécanisme de la recommandation, **priorité explicite puis nœud
  le plus profond**, mais une égalité restante **ne bloque rien** : le nœud reçoit
  plusieurs valeurs attendues candidates, l'interface les propose toutes, aucune valeur
  par défaut n'est écrite, aucune anomalie d'écart n'est levée sur le nœud ; le jeu de
  règles porte une anomalie « règles en conflit » que son auteur peut corriger ou
  ignorer. L'import de l'existant donne la priorité selon la couche et l'ordre de
  lecture, ce qui reproduit les résultats actuels.
- Conséquence sur le besoin : l'inférence est un **confort de saisie et un contrôle de
  vraisemblance**, jamais une source de vérité (D23 renforcé) ; le bruit doit rester bas
  (anomalies regroupées, acquittables) pour qu'un jeu de règles vieillissant ne gêne
  pas la maintenance courante.
- Décidé par : délégué (recommandation adoucie)

### Q13 — Points restants de l'inférence
- Question : lequel des points restants de l'inférence discuter ?
- Réponse : **aucun, tout accepter**. Consigné comme recommandations acceptées :
  critères en liste fermée extensible (inclusion dans un préfixe, nom, site, entité,
  domaine, type d'objet, valeur d'une autre propriété ; plus tard VLAN et attributs de
  machine) ; ET entre critères, chaque critère acceptant une liste de valeurs ;
  expressions régulières ancrées, bornées, moteur à temps linéaire ; un nœud qui
  correspond transmet l'attendu à sa sous-branche ; exceptions imbriquées (profondeur
  bornée) qui remplacent l'ensemble ; écart = ensembles déclaré et attendu différents,
  « aucun attendu » ne produisant jamais d'anomalie ; anomalies regroupées par nœud
  d'origine, acquittables ; valeur par défaut à la création seule, avec provenance
  « règle, révision » ; règles bitemporelles écrites par changeset ; import de
  l'existant par convertisseur hors ligne avec simulation. Détail dans
  [`../inference-questions.md`](../inference-questions.md) (I1 à I15).
- Décidé par : recommandation acceptée

### Q14 — Contenu de l'explication d'un verdict
- Question : que doit contenir l'explication pour qu'un demandeur la comprenne seul et
  qu'un auditeur la tienne pour preuve ?
- Réponse : **chaîne complète, lisible par le demandeur**. Pour chaque extrémité : valeur
  résolue de chaque axe cité, nœud d'origine et chaîne d'héritage. Le service et son type
  de flux. L'élément décisif (interdiction, autorisation, motif, ou « aucune règle :
  refus par défaut ») avec son nom, sa révision et son texte pédagogique. Les conditions
  d'admissibilité non remplies, le score et les avertissements. La date applicative et
  l'instant système de l'évaluation. Les règles écartées (trois au plus).
- Rejeté : élément décisif seul (le demandeur ne sait pas quoi changer) ; deux niveaux
  (deux formats à maintenir ; une représentation compacte peut rester un choix d'API).
- Décidé par : recommandation acceptée

### Q15 — Simulation et approbation d'un changement de politique
- Question : que doit montrer la simulation avant d'appliquer un changement de politique,
  et qui approuve ?
- Réponse : **différentiel des flux déclarés, différentiel abstrait, approbation par
  politique configurée**. Le changeset montre (1) les flux du registre dont le verdict ou
  l'issue bascule, avec l'élément décisif avant et après (D25, D30) ; (2) les
  combinaisons abstraites (valeurs d'axes source, destination, service) qui s'ouvrent ou
  se ferment ; (3) les nœuds dont la classification change et leur nombre d'héritiers.
  L'approbation suit C6 : déclencheurs fournis par le module (toute écriture de politique,
  tout flux qui se ferme), posés par l'administrateur ; séparation des tâches (C1).
- Rejeté : flux déclarés seuls (une ouverture sans flux déclaré passerait inaperçue) ;
  approbation imposée par le module (contredit C6).
- Décidé par : recommandation acceptée

### Q16 — Expiration et recertification des flux déclarés
- Question : quelle règle d'expiration et de recertification ?
- Réponse de l'utilisateur : la recertification humaine unitaire est impossible, il
  s'agit de **dizaines à centaines de milliers** d'ouvertures de flux ; la prolongation
  automatique lui semblait obligatoire. Après discussion, modèle retenu :
  - un flux **conforme** n'a pas de date de fin propre ; sa validité est **dérivée en
    continu** : il reste actif tant que la politique du jour l'autorise, que ses
    extrémités existent et que son dossier est complet. Tout changement de politique ou
    de classification le réévalue (D30, 011/F7) ; s'il bascule, il devient une
    non-conformité constatée, à instruire. Aucune écriture automatique : l'état est
    calculé ;
  - les flux **à instruire** et les **dérogations** gardent une date de fin obligatoire et
    une recertification humaine ;
  - la revue humaine se fait par **campagnes ciblées** (une application, un
    environnement, les flux dont le responsable a quitté l'organisation, plus tard les
    flux sans trafic observé), pas flux par flux.
- Rejeté : date de fin et prolongation automatique pour tous (écriture périodique sans
  humain) ; recertification unitaire (volume) ; date de fin facultative sur les flux
  conformes (non retenue, mais la période applicative de D30 reste disponible si un
  demandeur borne lui-même son flux).
- Écart : amende D30 (période de validité obligatoire) et 011/F2, F10 ; noté dans
  `ecarts.md`.
- Décidé par : recommandation acceptée (après demande d'avis de l'utilisateur)

### Q17 — Droits du demandeur et du connecteur
- Question : quelle règle de droits pour un demandeur, ou un outil externe qui appelle
  kartenn pour lui, sans lecture de tout l'arbre (010/S15) ?
- Réponse : **permission « demander un flux », explication réduite aux
  classifications**. Une permission portée par entité permet d'obtenir un verdict sur
  toute paire d'extrémités sans lecture de l'arbre : l'explication montre les valeurs
  d'axes et les règles, pas les nœuds d'origine ni la chaîne d'héritage, réservés aux
  lecteurs des extrémités. Le connecteur de ticketing agit avec un compte de service
  portant cette permission et transmet l'identité du demandeur (« pour le compte de »).
- Rejeté : lecture requise avec compte de service large (un compte qui voit tout, aucun
  demandeur direct) ; verdict public (révèle la classification de toute adresse).
- Écart : amende 010/S15 et 011/F12 ; noté dans `ecarts.md`.
- Décidé par : recommandation acceptée

### Q18 — Limites de la v0.2
- Question : la v0.2 doit-elle inclure l'import en masse de flux constatés (fichier
  structuré évalué à l'import) ? Hors v0.2 proposés : lecture des configurations de
  pare-feu et d'infrastructure as code (connecteurs), atteignabilité effective, NAT et
  routes (v0.3), observation du trafic réel, services exposés déclarés par une machine.
- Réponse : **retirer l'import des flux constatés de la v0.2**. La v0.2 traite les
  demandes nouvelles ; l'audit a posteriori des flux déjà ouverts attend la v0.3. Le
  modèle **réserve** néanmoins la place d'un flux « constaté » à côté d'un flux
  « demandé » (complément après Q9), pour que la v0.3 l'ajoute sans refonte. Les autres
  limites proposées sont retenues.
- Décidé par : l'utilisateur

## Clôture

Aucune question ouverte ne changerait les exigences ou le modèle de la v0.2. Les points
laissés ouverts, et ce qu'ils bloquent :

- **O-1 — Critères du score informatif** (Q4) : liste et pondération des signaux
  (largeur de plage, type de flux, paire d'axes, sensibilité des données, dossier). Bloque
  la section correspondante de `requirements.md`, pas le modèle (le score est un
  attribut de l'évaluation, sans effet sur l'issue).
- **O-2 — Référentiels annexes** (Q5) : forme de la référence vers une application ou un
  propriétaire (identifiant externe, système source, rafraîchissement). Bloque le type de
  la propriété « application » dans la 009 ou la 010.
- **O-3 — Vocabulaires d'exemple** des axes exposition, sensibilité et criticité livrés
  en configuration. Bloque le jeu de données des bancs et les exemples de la
  documentation, pas le modèle.
- **O-4 — Campagnes de revue** (Q16) : objet propre ou simple jeu de filtres enregistré ?
  Bloque la 011 au-delà de la v0.2.
- **O-5 — Connecteurs** ticketing et plateforme de dépôt de code (Q3) : événements
  sortants nécessaires (changement d'issue d'un flux) et format. Bloque la v0.3.

Synthèse des décisions, une ligne chacune, et le besoin consolidé dans
[`expression-besoin.md`](expression-besoin.md) ; écarts et attentes envers le noyau dans
[`ecarts.md`](ecarts.md).

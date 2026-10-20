---
status: draft
updated: 2026-10-21
---

# 010 — Écarts du besoin avec les décisions et attentes envers le noyau

Ce document confronte le besoin exprimé ([`expression-besoin.md`](expression-besoin.md),
[`journal.md`](journal.md)) aux décisions D19 à D30 de la spec 001, aux décisions S1 à
S21 de la spec 010 et F1 à F15 de la spec 011 (prises par délégation, sans entretien),
puis liste ce que le noyau (002 à 004), la spec 009 (machines) et le contrat de module
doivent offrir en plus de ce que le `design.md` de la 010 demande déjà (N1 à N20, M1 à
M6).

Trois niveaux : **contredit** (un arbitrage est demandé si la décision tient encore),
**amende** (la décision reste vraie, un de ses termes change), **complète** (ajout
compatible).

## 1. Décisions D19 à D30

| Décision | Niveau | Écart | Source |
|---|---|---|---|
| D19 (classer, matrice, questions) | complète | La classification porte plusieurs axes, pas seulement l'environnement ; la « matrice » devient une vue des règles qui ne citent que l'environnement. Les questions de conformité gagnent une **issue** de triage et un score informatif. | Q2, Q4, Q5, Q7 |
| D20 (conformité d'abord, atteignabilité ensuite) | inchangée | L'atteignabilité reste en étape 2 ; la v0.2 n'audite pas les flux déjà ouverts (renvoyé v0.3). | Q18 |
| D21 (environnement = propriété non ordonnée héritée) | complète | L'environnement reste l'axe livré par le module. Les axes exposition, sensibilité, criticité, application et propriétaire s'ajoutent comme configuration ; application et propriétaire sont des références vers des référentiels annexes. | Q5, Q6 |
| D22 (résolution par adresse, rôle d'interface) | complète | Résolution par adresse conservée. Le rôle d'interface se généralise en **rôle de conduit** cité par les sélecteurs (stockage d'objets, passerelle d'API, bastion) ; à décider si c'est le même vocabulaire que le rôle d'interface de la 009 ou une valeur d'axe. | Q9 |
| D23 (le déclaré fait foi) | renforcée | L'inférence est un confort de saisie et un contrôle de vraisemblance ; une égalité entre règles ne produit ni défaut ni anomalie d'écart. | Q12 |
| D24 (règles d'inférence, priorité explicite) | amende | Priorité explicite confirmée, départage par nœud le plus profond ; **l'égalité restante n'est pas refusée à l'écriture** (I6 le proposait), elle donne plusieurs candidats et une anomalie sur le jeu de règles. | Q12, Q13 |
| D25 (union permissive) | complète | L'union s'étend aux autorisations **conditionnelles** : une autorisation inconditionnelle l'emporte sur une conditionnelle ; une conditionnelle non satisfaite donne « à instruire », pas un refus. | Q11 |
| D26 (remplacement de l'ensemble hérité) | inchangée | Vaut pour tous les axes. | Q6 |
| D27 (classification obligatoire par domaine) | complète | L'obligation et le « non classifié » valent **par axe** cité par la politique, réglables par domaine et par axe. | Q6 |
| D28 (garde-fous puis matrice d'autorisations) | amende | Les deux niveaux et l'ordre d'évaluation tiennent. La forme change : **règles par attributs** à sélecteurs multi-axes (y compris « même valeur » et rôle de conduit), sans ordre ; les garde-fous sont les règles d'effet interdire. S'ajoutent l'autorisation **conditionnelle sous motif**, les **motifs** nommés, les **conditions d'admissibilité** (hors verdict), le texte pédagogique. La matrice tri-état de l'existant s'importe sans troisième effet. | Q7, Q9, Q11, Q2 |
| D29 (services nommés) | inchangée | Le catalogue en deux tables de l'existant se traduit directement (service nommé, type de flux, « autorisé = faux » = interdiction sans sélecteur). | Q10 |
| D30 (registre de flux, période de validité) | amende | Un flux **conforme** n'a plus de période de validité obligatoire : sa validité est **dérivée** de la politique en continu. Seuls les flux à instruire et les dérogations sont datés et recertifiés. « Tout changement de règle réévalue les flux » : confirmé pour la politique et les classifications ; une règle d'inférence ne change aucun verdict (lecture S21 confirmée par Q12 et Q13). Le registre devra accueillir une **sorte** « constaté » (v0.3). | Q16, Q18 |

**Arbitrage demandé** : aucun au sens strict, aucune décision n'est contredite. Deux
amendements méritent une confirmation explicite de l'utilisateur lors de la relecture
de la 010 et de la 011 : D28 (forme par attributs, motifs, conditions) et D30 (validité
dérivée des flux conformes).

## 2. Décisions S1 à S21 (spec 010)

| Décision | Niveau | Écart | Source |
|---|---|---|---|
| S1 (périmètre) | complète | S'ajoutent : axes multiples, motifs, conditions d'admissibilité, signaux et score informatif, textes pédagogiques, non-conformités consultables, permission « demander un flux », import de la politique existante par convertisseur. | Q2 à Q9, Q17 |
| S2 (`policy.environment`, attributs dans le module) | complète | Les autres axes sont des propriétés de liste fermée déclarées par un administrateur (P1) ou livrées en configuration ; le module lit tout axe qu'une règle cite. Attribut **transverse** à généraliser à toute valeur d'axe. | Q5 |
| S3 (porteurs : domaine, préfixes, adresses ; machine par propriété distincte) | complète | Les axes sensibilité, criticité, application, propriétaire sont portés par la machine (ou l'application) et lus par l'adresse ; une valeur posée sur un préfixe sert de défaut hérité. | Q6 |
| S4, S5 (types de flux, services) | inchangées | | Q10 |
| S6 (garde-fou = sélecteur × paire d'environnements facultative) | amende | Le garde-fou devient une règle d'effet interdire à sélecteurs multi-axes ; « service interdit » reste une interdiction sans sélecteur d'extrémité. | Q7 |
| S7 (cellules d'autorisation orientées, sans joker) | amende | La cellule devient une règle d'autorisation à sélecteurs ; un sélecteur vide vaut « toute extrémité » (joker explicite, nécessaire aux motifs) ; l'autorisation peut être conditionnelle. | Q7, Q11 |
| S8 (non classifié, garde-fous, matrice ; refus par défaut) | complète | Ordre conservé ; s'insèrent l'autorisation conditionnelle (sous motif, sinon à instruire) puis l'admissibilité et les signaux. Le résultat porte un verdict **et** une issue. | Q2, Q11 |
| S9 (requête : extrémités et formes de flux) | complète | La requête accepte le dossier (références, responsables, nature des données) pour évaluer l'admissibilité ; sans dossier, seul le verdict est rendu. | Q3 |
| S10 (cohérence des machines) | inchangée | | |
| S11 (différentiel : autorisations (env, env, service) qui basculent) | amende | Le différentiel abstrait porte sur des combinaisons de valeurs d'axes, pas seulement d'environnements ; il nomme aussi les flux déclarés dont l'**issue** bascule. | Q15 |
| S12 (déclencheurs d'approbation) | complète | Déclencheurs ajoutés : dérogation, reclassement d'un sous-arbre au-delà d'un seuil d'héritiers. | Q15 |
| S13 (obligation par domaine) | complète | Par domaine **et par axe**. | Q6 |
| S14 (audit en tâche de fond) | complète | Rapports supplémentaires : non-conformités, dérogations et échéances, combinaisons autorisées sans flux, règles jamais appliquées. | Q9 (complément), UC-9 |
| S15 (droits : verdict exige la lecture des deux extrémités) | amende | Permission **« demander un flux »** portée par entité : verdict sans lecture de l'arbre, explication réduite aux valeurs d'axes et aux règles ; compte de service avec « pour le compte de ». | Q17 |
| S16, S17 (bitemporel, file d'écriture) | inchangées | Les nouveaux objets (motifs, conditions, signaux, textes) suivent S16. | |
| S18 (inférence non tranchée) | close | Tranchée par Q10 à Q13 : I1 à I15 acceptés, I6 adouci (égalité non bloquante). | Q12, Q13 |
| S19, S20 (API, budgets) | à réviser | Les budgets doivent couvrir l'évaluation des conditions et des motifs ; la réponse de verdict porte issue, conditions, score. | Q2, Q4 |
| S21 (tension D23/D30) | close | Lecture confirmée : une règle d'inférence ne réévalue que les anomalies. | Q12 |

## 3. Décisions F1 à F15 (spec 011)

| Décision | Niveau | Écart | Source |
|---|---|---|---|
| F1 (périmètre) | complète | S'ajoutent : triage à l'enregistrement, non-conformités consultables, campagnes (filtres en v0.2), sorte « constaté » réservée pour la v0.3. | Q2, Q18 |
| F2 (contenu : justification non vide, période de validité) | amende | **Référence externe obligatoire**, justification texte **facultative** ; **nature des données** ajoutée ; responsables des extrémités ajoutés ; période de validité **facultative** pour un flux conforme. | Q3, Q16 |
| F3 (extrémités) | inchangée | | |
| F4 (cycle de vie dérivé) | amende | États dérivés à compléter : « à instruire » (en attente de revue humaine), « non conforme » (verdict basculé après enregistrement), « sous dérogation » ; l'expiration ne concerne que les flux datés. | Q2, Q16 |
| F5 (approbation par la 004) | complète | Un flux « à instruire » est une proposition ; la revue humaine est une approbation ou un rejet documenté (non-conformité). | Q2, Q3 |
| F6 (flux non conforme refusé sauf dérogation) | complète | La dérogation porte date de fin obligatoire, motif, risque accepté, approbateur distinct du demandeur, recertification. Un flux « autorisé sous réserve » (conditionnel non satisfait) n'est **pas** refusé : il est enregistré en attente d'instruction. | Q8, Q11 |
| F7 (verdicts matérialisés par segment) | complète | L'évaluation matérialisée porte aussi l'issue, les conditions non remplies, le score ; elle est **figée** à l'enregistrement et recalculée à chaque changement pertinent, avec historique. | Q2, Q16 |
| F8 (changement qui casse des flux accepté et montré) | inchangée | Un flux conforme qui bascule devient une non-conformité à instruire, pas un flux expiré. | Q16 |
| F9 (différentiel des flux) | complète | Basculements d'issue en plus des basculements de verdict. | Q15 |
| F10 (échéances à J-30 et J-7) | amende | Ne concerne que les flux datés (à instruire, dérogations) ; ajout d'un événement de **changement d'issue** pour les outils externes. | Q16, Q3 |
| F11 (requêtes) | complète | Filtres par application, demandeur, règle en cause, issue ; vue des non-conformités. | Q9 (complément) |
| F12 (droits par entité du flux, plus lecture des extrémités) | amende | La création par un demandeur ou un compte de service n'exige plus la lecture des extrémités mais la permission « demander un flux » ; l'explication complète reste réservée aux lecteurs. | Q17 |
| F13 à F15 | à réviser | Budgets : réévaluation continue de 10⁵ flux à chaque changement de politique ; API : champs du dossier, issue. | Q16 |

## 4. Ce que le noyau, la 009 et le contrat de module doivent offrir

Les points N1 à N20 et M1 à M6 du `design.md` de la 010 restent valables. S'y ajoutent,
numérotés à la suite :

### Noyau (002 à 004)

| # | Attente | Source |
|---|---|---|
| N21 | Une propriété de liste fermée **déclarée par un administrateur** (P1) doit pouvoir être désignée comme axe lu par la politique, avec les mêmes garanties qu'une propriété de module : héritage, remplacement, explication, obligation par domaine, lecture groupée (N9). | Q5, Q6 |
| N22 | Un type de valeur **référence externe** (système source, identifiant, libellé en cache) pour application et propriétaire, ou l'usage des identifiants externes du noyau (A.1) sur un objet de module ; à trancher avec O-2. | Q5 |
| N23 | Un attribut par **valeur** de liste fermée (ici « transverse »), administrable, lisible par le module ; aujourd'hui les attributs de valeur sont ceux du module (S2). | Q5 |
| N24 | Le contrat de source de valeurs attendues (N20) doit permettre **plusieurs candidats** sans valeur par défaut, et un mode « suggestion seule » quand le jeu de règles est en conflit. | Q12 |
| N25 | **Simulation à blanc** d'un changeset non soumis (différentiel complet, contributions de module comprises) pour instruire un changement de politique avant de le proposer ; à vérifier avec la simulation de la 004. | Q15 |
| N26 | Déclencheur d'approbation sur un **seuil d'héritiers** reclassés (nombre d'objets dont la valeur résolue change), en plus des déclencheurs par propriété et par volume (C6). | Q15 |
| N27 | Une **identité agissant pour le compte d'un demandeur** : un compte de service transmet l'identité du demandeur, enregistrée comme auteur d'intention dans l'audit et le changeset, sans que ce demandeur ait de compte kartenn ; à concilier avec la 005 et la 012. | Q3, Q17 |
| N28 | Une permission **sans lecture des objets visés** (« demander un flux ») : autoriser une opération dont le résultat est filtré (valeurs d'axes seules) alors que l'appelant ne détient pas la lecture des extrémités ; à concilier avec le droit sur l'effet (P7). | Q17 |
| N29 | **Textes pédagogiques** attachés aux règles, motifs et conditions : texte long, liens, i18n par clés (Q-ouv-6), versionnés avec l'objet. | Q9 (complément) |

### Module Machines et interfaces (009)

| # | Attente | Source |
|---|---|---|
| M7 | Machines porteuses de **plusieurs** propriétés déclarées par d'autres modules ou par l'administrateur (sensibilité, criticité, application, propriétaire), lues par l'adresse à la date (M3, M4 généralisés). | Q6 |
| M8 | **Rôle de conduit** : soit une extension du rôle d'interface (M2) à des rôles d'architecture (stockage d'objets, passerelle d'API, bastion), soit un axe porté par la machine ; la 009 doit permettre l'un ou l'autre et le rendre lisible par clé. | Q9 |
| M9 | Un **défaut hérité** pour un axe porté par la machine : la valeur résolue d'une adresse vaut celle de la machine si elle en porte une, sinon celle héritée de l'arbre d'adressage ; le port de lecture doit rendre cette composition explicable. | Q6 |

### Contrat de module (ADR 0026)

| # | Attente | Source |
|---|---|---|
| N30 | Un module expose une **évaluation sans écriture** (la pré-vérification de UC-1) qui lit les mêmes règles que l'enregistrement et rend le même résultat, pour qu'un outil externe l'appelle avant de soumettre. | Q1, Q2 |
| N31 | Un module publie des **événements de changement d'état dérivé** (issue d'un flux) et pas seulement d'écriture, calculés par une tâche de fond ou à la réévaluation (N14, N15). | Q3, Q16 |
| N32 | Un module peut exposer une **vue de saisie** alternative d'un même ensemble d'objets (la matrice comme vue des règles mono-axe) sans dupliquer les objets ; concerne surtout la 007 (plugin Web). | Q7 |
| N33 | Un module peut déclarer un objet de type « **non-conformité** » consultable, filtrable et acquittable, distinct des anomalies de contrôle (N11) : porté par une évaluation, visible par le demandeur concerné. | Q9 (complément) |

## 5. Points à relire dans la 010 et la 011

- Requirements 010 § C (garde-fous et matrice) et § D (verdicts) : réécrire en règles
  par attributs, autorisation conditionnelle, motifs, conditions, issue, score.
- Requirements 010 § G (inférence, réservée) : alimenter depuis Q10 à Q13 et I1 à I15.
- Requirements 010 § H et I : obligation par axe, permission « demander un flux ».
- Requirements 011 : contenu du flux (F2), états dérivés (F4), validité dérivée (F7,
  F10), droits (F12), non-conformités (F11), sorte « constaté » réservée.
- Non-objectifs des deux specs : y porter la liste de § 8 de
  [`expression-besoin.md`](expression-besoin.md).

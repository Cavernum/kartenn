---
status: draft
updated: 2026-10-21
---

# 010 — Expression du besoin « politique de flux »

Besoin consolidé à partir de l'entretien du 2026-10-20 ([`journal.md`](journal.md), Q1 à
Q18). Ce document reste au niveau du modèle : objets, relations, sémantique d'évaluation,
explication du verdict. Il ne décrit ni schéma de stockage ni API ; ces choix reviennent
à `requirements.md` et `design.md` de la spec 010 et à la spec 011. Les écarts avec les
décisions D19 à D30 et les attentes envers le noyau sont dans [`ecarts.md`](ecarts.md).

## 1. Finalité

kartenn doit permettre à une organisation d'exprimer sa **politique de flux** et de la
faire respecter sans freiner ceux qui la suivent. Trois objectifs, par ordre de priorité :

1. **Intégrer les besoins de sécurité dans le contexte de l'organisation** : classer les
   éléments réseau sur plusieurs axes, déclarer qui a le droit de communiquer, répondre
   à des questions de conformité à une date donnée avec une explication complète, et
   détecter les écarts entre ce qui est déclaré et ce qui est attendu.
2. **Affiner la politique sans refonte** : granularité plus fine, nouveaux axes de
   classification, règles conditionnelles, dérogations bornées, nouveaux types de flux et
   services, motifs d'architecture.
3. **Rester général** : la politique de n'importe quelle organisation doit s'exprimer avec
   les mêmes objets (zonage réseau, sensibilité des données, environnements, niveaux de
   confiance, exigences réglementaires), à partir des principes des cadres reconnus,
   sans rien supposer d'une organisation ou d'un produit particulier.

Le fil conducteur, formulé par l'utilisateur : **laisser passer les bonnes pratiques,
bloquer les mauvaises, et rapatrier ces dernières vers un accompagnement** plutôt que
vers un simple refus.

## 2. Principes directeurs

Les principes ci-dessous viennent des cadres de sécurité reconnus ; chacun se traduit par
un trait du modèle.

- **Refus par défaut et moindre privilège.** Un flux qu'aucune règle n'autorise est
  refusé ; une autorisation ne porte que sur le service et les classes nécessaires.
- **Défense en profondeur.** Des garde-fous absolus (interdictions) l'emportent sur toute
  autorisation ; la politique s'applique indépendamment des dispositifs techniques
  (pare-feux, listes de contrôle) qui l'appliqueront.
- **Zones et conduits** (segmentation à la manière de l'IEC 62443). Une **zone** est une
  classe d'éléments qui partagent les mêmes exigences : ici une combinaison de valeurs
  d'axes (environnement, exposition, sensibilité…). Un **conduit** est un chemin de
  communication autorisé entre zones, pour des services identifiés : ici une règle
  d'autorisation, ou un **motif** d'architecture qui impose un point de passage
  (stockage d'objets, passerelle d'API, bastion).
- **Zero trust : ne pas se fier à la seule position réseau.** La classification repose
  sur plusieurs axes, pas sur le seul préfixe ; le verdict ne lit que ce qui est
  **déclaré** et vérifié, jamais ce qui est inféré (D23).
- **Séparation des tâches et traçabilité.** Celui qui demande n'approuve pas ; toute
  décision (règle, dérogation, classification) est écrite par changeset, datée sur deux
  axes de temps, et toute évaluation cite ses sources.
- **Explicabilité et pédagogie.** Un verdict n'est jamais un simple oui ou non : il
  nomme l'élément décisif, la chaîne qui y mène et la bonne pratique attendue, dans
  des termes que le demandeur comprend seul.

## 3. Acteurs

| Acteur | Rôle | Ce qu'il attend de kartenn |
|---|---|---|
| **Demandeur** (équipe projet, « client ») | Demande l'ouverture d'un flux, le plus souvent depuis un outil externe (ticketing, revue de code d'infrastructure as code) | Une réponse immédiate, claire et argumentée ; savoir quoi changer si ce n'est pas conforme |
| **Équipe sécurité** | Responsable de la sécurité des flux : écrit la politique, instruit les cas non automatiques, accorde les dérogations, accompagne | Un triage fiable qui ne lui laisse que les cas qui méritent un humain ; des signaux pour repérer les oublis ; des non-conformités consultables pour la sensibilisation |
| **Équipe réseau** | Maintient le référentiel et applique les décisions | Que la politique ne ralentisse pas la maintenance courante ; des suggestions de classification ; un différentiel clair avant d'écrire |
| **Auditeur** | Vérifie a posteriori | « La politique au 1er mars », les flux qui ne tiennent que par dérogation, les écarts, la preuve de chaque décision |
| **Automates** (connecteurs, pipelines) | Interrogent et enregistrent pour le compte d'un demandeur | Une API de verdict et d'enregistrement, des événements quand l'issue d'un flux change |
| **Administrateur** | Configure les axes, les vocabulaires, les obligations, les politiques d'approbation | Que tout cela soit de la configuration, pas du code |

## 4. Cas d'usage

Les cas d'usage sont numérotés UC-n. Le marqueur « v0.3 » signale ce qui est hors de la
v0.2 mais que le modèle doit déjà accueillir.

- **UC-1 — Instruire une demande de flux.** Un demandeur (ou un outil pour lui) soumet :
  source, destination, service (ou transport et port, ou type de flux), sens, période
  éventuelle, références externes, responsables, nature des données. kartenn répond
  dans la même requête par un **verdict** (autorisé, refusé), une **issue** de triage
  (automatique, à instruire, non conforme), une **explication** complète (§ 7) et, pour
  la revue humaine, un **score informatif** et des avertissements. La même requête
  fonctionne en **pré-vérification**, avant toute soumission, pour que le demandeur
  ajuste sa demande.
- **UC-2 — Enregistrer un flux autorisé automatiquement.** Si le verdict est autorisé et
  que toutes les conditions d'admissibilité sont remplies, le flux entre au registre
  (spec 011) sans intervention humaine, avec son évaluation figée (règle et révision
  citées), et un événement est émis pour l'outil appelant.
- **UC-3 — Revue manuelle d'une demande à instruire ou non conforme.** L'équipe
  sécurité voit la demande, l'évaluation, les conditions non remplies, le score et les
  avertissements. Elle oriente le demandeur vers la bonne pratique citée par la
  politique, enregistre le flux après correction, le refuse en documentant la
  non-conformité, ou accorde une dérogation (UC-4). Chaque issue est tracée et
  consultable par le demandeur.
- **UC-4 — Accorder et recertifier une dérogation.** Une dérogation porte sur un seul
  flux : date de fin obligatoire, motif, risque accepté, approbateur distinct du
  demandeur. Avant l'échéance, les responsables sont prévenus ; la recertification
  réévalue le flux contre la politique du jour et prolonge par changeset. Rien ne se
  prolonge seul.
- **UC-5 — Classer un élément.** Un mainteneur affecte à un nœud de l'arbre d'adressage
  (domaine, conteneur, réseau, adresse) ou à une machine les valeurs d'un ou plusieurs
  axes. L'interface propose les valeurs **attendues** par les règles d'inférence ; la
  valeur écrite est toujours une décision humaine, ou une valeur par défaut tracée à la
  création. Une affectation remplace l'ensemble hérité (D26).
- **UC-6 — Détecter les écarts.** kartenn signale, par anomalies regroupées et
  acquittables : les nœuds non classifiés sur un axe cité par la politique ; les écarts
  entre classification déclarée et attendue ; les machines dont les adresses résolvent
  vers des environnements différents sans rôle d'interface ; les règles d'inférence en
  conflit ; les flux dont l'autorisation ne tient qu'à une valeur transverse ; les flux
  déclarés devenus non conformes.
- **UC-7 — Écrire ou modifier la politique.** Un administrateur de politique crée ou
  modifie règles, motifs, conditions d'admissibilité, services, types de flux,
  vocabulaires. Le changeset montre, avant application, le **différentiel** : flux
  déclarés dont le verdict ou l'issue bascule, combinaisons abstraites qui s'ouvrent ou
  se ferment, nœuds reclassés et leur nombre d'héritiers. Une politique d'approbation
  configurée par l'administrateur peut exiger une approbation (C1, C6).
- **UC-8 — Répondre à une date.** Toute question (verdict, classification, règle
  applicable, liste des flux) accepte une date applicative et un instant système : « que
  disait la politique au 1er mars ? », « ce flux était-il autorisé le jour de l'incident,
  selon ce que kartenn savait alors ? ».
- **UC-9 — Auditer en masse.** Rapports calculés en tâche de fond : non-conformités
  ouvertes, flux par dérogation et leurs échéances, flux à instruire en attente, nœuds
  non classifiés par axe et par domaine, écarts d'inférence, règles jamais appliquées,
  combinaisons autorisées sans aucun flux déclaré.
- **UC-10 — Consulter les non-conformités et les bonnes pratiques.** Chaque
  non-conformité (demande refusée, flux devenu non conforme, dérogation) est un objet
  consultable, filtrable par application, demandeur, règle, période, avec le texte
  pédagogique de la règle en cause. Sert à l'accompagnement et à la sensibilisation.
- **UC-11 — Importer la politique existante.** Matrice à double entrée (y compris les
  cellules « partiel »), catalogue de protocoles en deux tables et règles d'inférence
  TOML s'importent par un convertisseur hors ligne qui produit des changesets et, en
  simulation, liste les différences de résultat avec l'outil actuel.
- **UC-12 — Revue par campagnes** (v0.3). L'équipe sécurité sélectionne un ensemble de
  flux (une application, un environnement, un responsable parti) et les passe en revue
  comme un lot ; la v0.2 fournit seulement les filtres de UC-9 et UC-10.
- **UC-13 — Auditer les flux constatés** (v0.3). Des flux déjà ouverts, lus dans des
  exports de pare-feux ou d'infrastructure as code, entrent au registre comme flux
  **constatés** et reçoivent une évaluation ; la v0.2 réserve la place de ce type de
  flux sans l'implémenter.

## 5. Exigences de haut niveau

Formulées en EARS, préfixées EB (expression du besoin). Elles seront reprises et
détaillées dans `requirements.md`.

### Classification
- **EB-1** Le système doit permettre de déclarer plusieurs **axes** de classification,
  chaque axe étant un vocabulaire fermé administrable (P1, P2), et de les affecter aux
  nœuds de l'arbre d'adressage ou aux machines selon le porteur de l'axe (Q6).
- **EB-2** Le système doit résoudre la valeur de chaque axe d'une extrémité par son
  adresse (D22), par héritage sur l'arbre d'adressage et, pour les axes portés par la
  machine, par la machine rattachée à l'adresse à la date demandée.
- **EB-3** Quand un axe cité par la politique n'a pas de valeur résolue sur une
  extrémité, le système doit traiter l'extrémité comme **non classifiée** sur cet axe :
  refus par défaut et anomalie ; l'obligation de classification est réglable par domaine
  et par axe (D27, P4).
- **EB-4** Le système doit fonder tout verdict sur les seules valeurs **déclarées**
  (explicites, héritées ou écrites par défaut à la création avec provenance), jamais sur
  les valeurs attendues (D23).

### Politique
- **EB-5** Le système doit représenter la politique par des **règles par attributs**
  sans ordre : sélecteur source, sélecteur destination, sélecteur de flux, effet
  autoriser ou interdire (Q7). Un sélecteur cite des valeurs d'un ou plusieurs axes, un
  rôle de conduit, ou exige la **même valeur** d'un axe aux deux extrémités (Q9).
- **EB-6** Le système doit faire l'emporter toute interdiction applicable sur toute
  autorisation (D28), unir les autorisations applicables (D25) et refuser par défaut.
- **EB-7** Le système doit permettre une autorisation **conditionnelle** : elle exige un
  ou plusieurs motifs nommés ; si aucun motif requis ne correspond, le flux n'est pas
  refusé mais son issue est « à instruire » (Q11).
- **EB-8** Le système doit permettre de grouper des règles en **motifs** nommés
  d'architecture, cités par leur nom dans l'explication (Q9).
- **EB-9** Le système doit associer à chaque interdiction et à chaque motif un **texte
  pédagogique** (bonne pratique attendue, lien vers une documentation) restitué avec le
  verdict et dans les non-conformités (complément après Q9).
- **EB-10** Le système doit tenir un **catalogue de services nommés** (D29) : transport,
  ports ou plages, un type de flux ; un service interdit est une interdiction sans
  sélecteur d'extrémité (S6).
- **EB-11** Le système doit offrir une **vue matricielle** (environnement source ×
  environnement destination × type de flux) pour saisir et lire les règles qui ne citent
  que l'environnement, avec les trois états autorisé, interdit et conditionnel (Q7,
  Q11).

### Triage et admissibilité
- **EB-12** Le système doit évaluer, séparément du verdict, des **conditions
  d'admissibilité** nommées (dossier complet, largeur de plage maximale, extrémités
  nommées plutôt que « tout le monde », service du catalogue plutôt que port brut,
  compatibilité de la sensibilité des données avec la destination…) et en déduire
  l'**issue** : automatique, à instruire, non conforme (Q2, Q4).
- **EB-13** Le système doit produire, à destination de la revue humaine seulement, un
  **score** chiffré et des **avertissements** qui n'influencent pas l'issue (Q4).
- **EB-14** Le système doit considérer la **référence externe** (ticket, revue de code)
  comme obligatoire dans le dossier et le motif en texte libre comme facultatif (Q3).

### Flux déclarés
- **EB-15** Le système doit enregistrer automatiquement un flux dont le verdict est
  autorisé et l'issue automatique, et soumettre tout autre flux à une revue humaine
  (Q2).
- **EB-16** Le système doit porter la **dérogation** au niveau du flux seul, avec date
  de fin, motif, risque accepté, approbateur distinct du demandeur et recertification ;
  la politique n'est jamais modifiée par une dérogation (Q8).
- **EB-17** Le système doit dériver la validité d'un flux **conforme** de la politique
  en continu, sans date de fin propre : tout changement de politique ou de classification
  réévalue les flux touchés ; un flux qui bascule devient une non-conformité à instruire.
  Les flux à instruire et les dérogations gardent une date de fin et une recertification
  humaine (Q16).
- **EB-18** Le système ne doit jamais prolonger, créer ou reclasser seul : toute écriture
  passe par un changeset d'un humain ou d'un compte de service agissant pour un
  demandeur identifié (D1, Q16).

### Inférence
- **EB-19** Le système doit évaluer des **règles d'inférence** (données, pas du code) qui
  produisent des valeurs **attendues** pour un axe : critères en liste fermée et
  extensible, ET entre critères avec listes de valeurs, transmission à la sous-branche,
  exceptions imbriquées qui remplacent l'ensemble, priorité explicite puis nœud le plus
  profond (D24, Q12, Q13).
- **EB-20** Quand plusieurs règles de même priorité et de même profondeur produisent des
  valeurs différentes, le système doit proposer toutes les candidates, n'écrire aucune
  valeur par défaut, ne lever aucune anomalie d'écart sur le nœud, et signaler le
  conflit sur le jeu de règles (Q12).
- **EB-21** Le système doit utiliser l'attendu pour : la suggestion à la saisie ; la
  valeur par défaut à la création seule, avec provenance « règle, révision » ; l'anomalie
  d'écart avec le déclaré, regroupée par nœud d'origine et acquittable (D24, Q13).

### Explication, temps, gouvernance
- **EB-22** Le système doit accompagner tout verdict d'une **explication** complète
  (§ 7), et en réduire le contenu aux classifications et aux règles pour un appelant qui
  ne détient que la permission « demander un flux » (Q14, Q17).
- **EB-23** Le système doit versionner en **bitemporel** règles, motifs, conditions,
  services, vocabulaires et classifications, et évaluer toute question aux date
  applicative et instant système demandés (S16, Q15).
- **EB-24** Le système doit montrer, avant l'application d'un changeset qui touche la
  politique ou une classification, le **différentiel** : flux déclarés qui basculent,
  combinaisons abstraites ouvertes ou fermées, nœuds reclassés et héritiers (Q15).
- **EB-25** Le système doit fournir des **déclencheurs d'approbation** (écriture de
  politique, flux qui se ferme, dérogation) qu'un administrateur pose dans une politique
  d'approbation ; le module n'impose rien, à l'exception de la politique par défaut
  « un flux se ferme » installée avec le registre et désactivable (C6, Q15, 011/F8
  amendée par l'utilisateur le 2026-10-20).
- **EB-26** Le système doit offrir une permission **« demander un flux »** portée par
  entité, qui autorise le verdict sur toute paire d'extrémités sans lecture de l'arbre,
  et accepter l'identité du demandeur transmise par un compte de service (Q17).
- **EB-27** Le système doit rendre toute **non-conformité** consultable et filtrable
  (application, demandeur, règle, période), avec le texte pédagogique de la règle en
  cause (complément après Q9).
- **EB-28** Le système doit importer la politique existante par un convertisseur hors
  ligne qui traduit l'ordre de lecture en priorités et simule les différences (Q10,
  Q13).

## 6. Modèle conceptuel

### 6.1 Objets

- **Axe de classification.** Nom, vocabulaire fermé de **valeurs**, porteur (arbre
  d'adressage, machine ou application), héritable ou non, obligation par domaine. Une
  valeur peut être marquée **transverse** (par exemple « global ») pour l'audit des
  autorisations qui ne tiennent qu'à elle. L'environnement (D21) est l'axe livré par le
  module ; exposition, sensibilité des données, criticité sont livrés en configuration
  d'exemple ; application et propriétaire sont des **références** vers des référentiels
  annexes.
- **Classification.** Affectation d'un **ensemble** de valeurs d'un axe à un nœud ou à
  une machine, à une période applicative ; remplace l'ensemble hérité (D26) ; peut
  valoir « aucune valeur » (P4). Résolue par héritage avec origine et chaîne.
- **Type de flux.** Liste administrable (administratif, base de données, applicatif,
  fichier…).
- **Service.** Nom, transport, ports ou plages, un type de flux (D29).
- **Sélecteur d'extrémité.** Conjonction de critères : valeurs admises par axe (ET entre
  axes, OU entre valeurs d'un même axe), rôle de conduit requis, et contraintes
  croisées « même valeur que l'autre extrémité » sur un axe. Un sélecteur vide
  correspond à toute extrémité.
- **Sélecteur de flux.** Types de flux ou services nommés ; vide = tout flux.
- **Règle de politique.** Sélecteur source, sélecteur destination, sélecteur de flux,
  effet **autoriser** ou **interdire**, motifs requis (autorisation conditionnelle), texte
  pédagogique, appartenance éventuelle à un motif, période applicative. Pas d'ordre.
- **Motif d'architecture.** Nom, description pédagogique, ensemble de règles. Un motif
  « correspond » à une demande si l'une de ses règles d'autorisation s'applique et
  qu'aucune de ses interdictions ne s'applique.
- **Condition d'admissibilité.** Nom, prédicat sur la demande et ses extrémités
  résolues (largeur de plage, présence d'un champ du dossier, nature des données face à
  la destination, service nommé plutôt que port brut…), texte pédagogique. Toutes les
  conditions doivent être remplies pour l'issue automatique.
- **Signal de risque.** Nom, prédicat, poids ; la somme des signaux déclenchés donne le
  score informatif ; chaque signal déclenché est un avertissement.
- **Règle d'inférence.** Axe visé, critères (ET), valeurs attendues, priorité,
  exceptions imbriquées, période applicative ; appartient à un **jeu de règles** dont la
  révision sert de provenance.
- **Demande de flux.** Extrémités (adresse, préfixe, interface ou machine), service ou
  transport-port ou type de flux, sens, période demandée, références externes,
  demandeur et « pour le compte de », responsables des extrémités, nature des données.
- **Évaluation.** Résultat d'une demande à une date : verdict, issue, élément décisif,
  explication, conditions non remplies, score et avertissements, révision de politique.
  Figée quand le flux est enregistré ; recalculée à chaque changement pertinent.
- **Flux déclaré** (spec 011). Une demande enregistrée avec son évaluation courante et
  son historique ; **sorte** : demandé ou constaté (v0.3) ; **dérogation** éventuelle.
- **Dérogation.** Date de fin, motif, risque accepté, approbateur, recertifications.
- **Non-conformité.** Toute évaluation dont l'issue n'est pas automatique, ou tout flux
  déclaré dont le verdict a basculé ; consultable, filtrable, acquittable.
- **Anomalie.** Signal de contrôle de la classification ou des règles (non classifié,
  écart d'inférence, machine incohérente, règles en conflit, autorisation transverse
  seule), regroupé et acquittable.

### 6.2 Relations

- Un axe porte des valeurs ; une classification lie un nœud ou une machine à un ensemble
  de valeurs d'un axe pour une période.
- Une règle de politique cite des valeurs d'axes, des rôles, des types de flux et des
  services ; un motif regroupe des règles ; une autorisation conditionnelle cite des
  motifs.
- Un service appartient à un type de flux ; une demande cite un service, ou un transport
  et un port (résolus en services, D29), ou un type de flux.
- Une évaluation cite la demande, la révision de la politique, les classifications
  résolues des extrémités et l'élément décisif.
- Un flux déclaré porte une évaluation courante et un historique d'évaluations ; une
  dérogation appartient à un flux.
- Une règle d'inférence cite un axe et produit des valeurs attendues ; une anomalie
  d'écart cite la règle, sa révision et le nœud d'origine de la valeur déclarée.

### 6.3 Sémantique d'évaluation

Pour une demande (S, D, F) à une date applicative t et un instant système τ :

1. **Résolution des extrémités.** Pour chaque extrémité, l'ensemble de ses adresses à t
   (une machine ou un préfixe en a plusieurs) ; pour chaque adresse et chaque axe cité
   par la politique, l'ensemble de valeurs résolu, avec origine et chaîne d'héritage. Une
   paire (adresse source, adresse destination) est l'unité d'évaluation ; la réponse
   donne un verdict par paire et une synthèse sur le plus restrictif (D22, S9).
2. **Résolution du flux.** F devient un ensemble de services (un service nommé ; les
   services qui contiennent le port donné ; tous les services du type donné). Le verdict
   est rendu par service, la synthèse sur le plus restrictif (D29). Un port qu'aucun
   service ne contient donne un verdict **refusé**, élément décisif « service inconnu »,
   et l'issue « à instruire » avec l'invitation à nommer ou faire ajouter le service
   (Q19).
3. **Non classifié.** Si une extrémité n'a aucune valeur sur un axe que cite au moins une
   règle candidate, le verdict de la paire est **refusé**, élément décisif « non
   classifié sur l'axe X », et une anomalie est levée.
4. **Interdictions.** Toute règle d'effet interdire dont les sélecteurs correspondent
   (pour une combinaison de valeurs des deux extrémités et le service) rend le verdict
   **refusé** ; l'élément décisif est l'interdiction la plus spécifique (celle qui cite
   le plus de critères), les autres étant citées comme écartées.
5. **Autorisations.** Sinon, l'ensemble des règles d'effet autoriser dont les sélecteurs
   correspondent à **au moins une** combinaison de valeurs (union permissive, D25) :
   - s'il contient une autorisation inconditionnelle, verdict **autorisé** ; l'élément
     décisif est l'autorisation la plus spécifique, avec la paire de valeurs qui l'a
     déclenchée ;
   - sinon, s'il contient une autorisation conditionnelle dont un motif requis
     correspond, verdict **autorisé** ; l'élément décisif est le motif ;
   - sinon, s'il contient une autorisation conditionnelle sans motif correspondant,
     verdict **autorisé sous réserve** : issue « à instruire », élément décisif
     l'autorisation conditionnelle et les motifs attendus ;
   - sinon, verdict **refusé** par défaut, élément décisif « aucune règle ».
6. **Admissibilité.** Si le verdict est autorisé, chaque condition d'admissibilité est
   évaluée ; toutes remplies, issue **automatique** ; sinon issue **à instruire** avec la
   liste des conditions non remplies. Un verdict refusé donne l'issue **non conforme**.
7. **Signaux.** Les signaux de risque déclenchés sont listés et sommés ; ils n'influencent
   ni le verdict ni l'issue.

**Ordre de priorité, résumé** : non classifié > interdiction > autorisation
inconditionnelle > autorisation conditionnelle satisfaite > autorisation conditionnelle
non satisfaite (à instruire) > refus par défaut. Puis l'admissibilité décide de
l'automatisme. Aucune étape ne dépend de l'ordre d'écriture des règles.

### 6.4 Explication du verdict

L'explication est produite avec chaque évaluation et conservée avec le flux enregistré :

- la demande normalisée et la date (t, τ) ;
- par extrémité, par adresse et par axe cité : valeurs résolues, nœud d'origine, chaîne
  d'héritage (réservés aux lecteurs des extrémités ; un appelant « demander un flux » ne
  voit que les valeurs) ;
- les services résolus et leur type de flux ;
- l'élément décisif : règle, motif ou cause (« non classifié », « aucune règle »), son
  nom, sa révision, la combinaison de valeurs qui l'a déclenché, son texte pédagogique ;
- les règles écartées (trois au plus) et pourquoi ;
- les conditions d'admissibilité non remplies, avec leur texte pédagogique ;
- le score et les avertissements ;
- l'issue et, si elle n'est pas automatique, la suite attendue (revue, bonne pratique à
  appliquer, dérogation possible).

### 6.5 Inférence

Les règles d'inférence sont évaluées sur l'arbre d'adressage (plus tard les machines) :
une règle dont les critères correspondent à un nœud transmet ses valeurs attendues à la
sous-branche ; une exception imbriquée reprend une partie des éléments de sa règle
parente et remplace l'ensemble ; entre règles, la priorité explicite l'emporte, puis le
nœud correspondant le plus profond ; une égalité restante donne plusieurs candidats, sans
défaut ni anomalie d'écart, et une anomalie « règles en conflit » sur le jeu. L'attendu
sert à la suggestion, à la valeur par défaut à la création (provenance « jeu de règles,
révision, règle ») et à l'anomalie d'écart, regroupée par nœud d'origine du déclaré. Le
détail des points réglés est dans [`../inference-questions.md`](../inference-questions.md).

## 7. Gouvernance

- **Qui écrit.** La politique (règles, motifs, conditions, services, vocabulaires) est
  écrite par les détenteurs de la permission d'administration de la politique, portée
  globalement ou par domaine quand une règle cite un domaine. Les classifications sont
  écrites par les mainteneurs du référentiel, avec le droit sur l'effet (P7). Les
  dérogations sont accordées par une personne distincte du demandeur.
- **Revue et approbation.** Tout passe par changeset (D11). Aucune approbation par défaut
  (C6) ; le module fournit des déclencheurs (écriture de politique, flux qui se ferme,
  dérogation, reclassement d'un sous-arbre au-delà d'un seuil d'héritiers) que
  l'administrateur compose en politique d'approbation ; une approbation, séparation des
  tâches, expiration (C1).
- **Versions datées.** Tous les objets de politique sont bitemporels ; une révision du
  jeu de politique est incrémentée par tout changeset qui le touche et sert de provenance
  aux évaluations. « La politique au 1er mars » est une lecture datée ordinaire.
- **Simulation.** Le différentiel d'un changeset (§ UC-7) est calculé avant application
  et affiché dans la proposition ; il est aussi disponible pour un changeset simulé
  (non soumis), afin d'instruire un changement de politique à blanc.
- **Transparence vers le demandeur.** Chaque changement d'issue d'un de ses flux lui est
  notifié par événement sortant (vers l'outil externe) et visible dans ses
  non-conformités.

## 8. Limites de la v0.2 et renvois

**Dans la v0.2** : axes, classifications, politique (règles, motifs, conditions, signaux),
catalogue, verdict expliqué et triage, registre des flux demandés avec dérogations et
validité dérivée, inférence (suggestion, défaut, écart), différentiel et déclencheurs
d'approbation, audits de masse, non-conformités consultables, import de la politique
existante, permission « demander un flux », événements sortants de changement d'issue.

**Hors v0.2, place réservée dans le modèle** :
- flux **constatés** et audit a posteriori des flux déjà ouverts (UC-13, v0.3) ;
- lecture des exports de pare-feux et d'infrastructure as code, connecteurs vers le
  ticketing et la plateforme de revue de code (v0.3) ;
- **atteignabilité effective**, NAT, routes, passerelles (D20 étape 2, v0.3 et suite) ;
- observation du trafic réel, flux sans trafic ;
- services exposés déclarés par une machine (D29) ;
- campagnes de revue comme objet propre (UC-12) ;
- score de risque décisionnel (le score reste informatif en v0.2).

## 9. Synthèse des décisions de l'entretien

- Q1 : l'équipe sécurité décide, l'équipe réseau applique ; le cas qui ordonne les autres
  est l'instruction d'une demande de flux, avec triage automatique ou manuel.
- Q2 : verdict binaire (D28) plus conditions d'admissibilité ; trois issues :
  automatique, à instruire, non conforme ; le demandeur est le client et doit être
  informé au plus vite et clairement.
- Q3 : dossier = extrémités et service, références, période et responsables, nature des
  données ; référence externe obligatoire, motif texte facultatif ; kartenn consommé
  comme une API depuis les outils externes.
- Q4 : conditions d'admissibilité nommées décisionnelles ; score et avertissements
  informatifs pour la revue humaine.
- Q5 : quatre axes en plus de l'environnement : exposition, sensibilité, criticité,
  propriétaire ou application (références externes).
- Q6 : environnement et exposition portés par l'arbre ; les autres par la machine ou
  l'application, lus par l'adresse ; manquant = non classifié, refus par défaut,
  obligation par domaine et par axe.
- Q7 : règles par attributs, deux effets, sans ordre ; la matrice devient une vue.
- Q8 : dérogation au niveau du flux seul, datée, approuvée, recertifiée.
- Q9 : opérateur « même valeur », rôles de conduit, motifs nommés ; textes pédagogiques
  et non-conformités consultables.
- Q10 : existant anonymisé consigné (règles TOML, matrice tri-état, catalogue en deux
  tables).
- Q11 : « partiel » = autorisation conditionnelle sous motif.
- Q12 : inférence = confort de saisie ; priorité explicite puis profondeur ; égalité
  non bloquante.
- Q13 : points restants de l'inférence réglés par recommandation (I1 à I15).
- Q14 : explication complète, lisible par le demandeur.
- Q15 : différentiel des flux déclarés et différentiel abstrait ; approbation par
  politique configurée (C6).
- Q16 : flux conformes à validité dérivée, sans date de fin ; flux à instruire et
  dérogations datés et recertifiés ; revue par campagnes.
- Q17 : permission « demander un flux », explication réduite aux classifications.
- Q18 : flux constatés hors v0.2, place réservée.
- Q19 : port sans service connu refusé mais « à instruire » ; jokers réservés aux
  sélecteurs d'extrémité ; lecture de S21 confirmée.

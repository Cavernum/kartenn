---
status: in-progress
updated: 2026-10-23
---
<!-- Implémentées sauf REQ-010-28 et 54 (budgets : banc 010-19 écrit, tenu en variante réduite, jeu complet à mesurer) et REQ-010-55 (S36, acceptée le 2026-10-23, tâche 010-22). -->


# 010 — Politique de flux : exigences

## Résumé

Le module Politique (`policy`, v0.2, D31) répond à la question qui motive kartenn
(D19) : « la machine A peut-elle ouvrir tel flux vers la machine B selon la politique ? »,
et la pose comme l'**instruction d'une demande de flux** : verdict, issue de triage,
explication pédagogique. Il classe les éléments réseau sur plusieurs **axes**
(environnement livré par le module, exposition, sensibilité, criticité, application,
propriétaire, rôle de conduit), décrit un **catalogue de services** classés par **type de
flux** et exprime la politique par des **règles par attributs** sans ordre (D28 amendée) :
les interdictions l'emportent, les autorisations s'unissent (D25), le refus est par
défaut. Une autorisation peut exiger un **motif** d'architecture ; des **conditions
d'admissibilité** décident si un flux autorisé passe automatiquement ou part à
l'instruction ; des **signaux** donnent un score informatif. Tout changement de politique
montre, avant d'être appliqué, ce qu'il ouvre ou ferme. Des **règles d'inférence**
proposent les classifications attendues et signalent les écarts, sans jamais fonder un
verdict (D23).

Le fil conducteur, formulé par l'utilisateur : laisser passer les bonnes pratiques,
bloquer les mauvaises et rapatrier ces dernières vers un accompagnement plutôt qu'un
simple refus.

Sources : besoin consolidé [`besoin/expression-besoin.md`](besoin/expression-besoin.md)
(UC-1 à UC-13, EB-1 à EB-28), écarts [`besoin/ecarts.md`](besoin/ecarts.md), entretien
[`besoin/journal.md`](besoin/journal.md) (Q1 à Q24) ; décisions
[`decisions.md`](decisions.md) (S1 à S34) ; D19 à D30
([`../001-cadrage/decisions.md`](../001-cadrage/decisions.md), D28 et D30 amendées le
2026-10-21) ; 003 (propriétés), 004 (changesets), 005 (RBAC), 006 (API), 009 (machines).
Registre de flux : spec 011.

### Vocabulaire

- **Axe** : propriété de liste fermée (ou de référence externe) que la politique peut
  citer ; un nœud ou une machine en porte un **ensemble** de valeurs (D25).
- **Environnement** : axe `policy.environment`, livré par le module.
- **Nœud** : domaine, conteneur, réseau ou adresse (arbre d'adressage, REQ-003-11).
- **Non classifié sur l'axe X** : extrémité dont X n'a pas de valeur résolue.
- **Type de flux** : catégorie de flux (base de données, administration…).
- **Service** : entrée nommée du catalogue : un type de flux et des couples
  transport/ports (D29).
- **Sélecteur d'extrémité** : conjonction de critères : valeurs admises par axe (ET entre
  axes, OU entre valeurs d'un même axe) ; vide = toute extrémité.
- **Règle** : sélecteur source, sélecteur destination, contraintes « même valeur »,
  sélecteur de flux, effet **autoriser** ou **interdire**, motifs requis éventuels.
- **Motif** : groupe nommé de règles d'architecture, cité par son nom.
- **Condition d'admissibilité** : prédicat nommé sur la demande et ses extrémités,
  évalué hors verdict.
- **Signal** : prédicat nommé et pondéré ; la somme des poids déclenchés est le **score**.
- **Verdict** : `allowed` ou `denied` pour une paire d'adresses et un service, avec
  l'élément décisif ; `allowed` peut être **sous réserve** (autorisation conditionnelle
  sans motif correspondant).
- **Issue** : `automatic`, `review` (à instruire) ou `noncompliant`.
- **Dossier** : références externes, responsables, nature des données, période et
  demandeur joints à une demande (Q3).
- **Différentiel de politique** : combinaisons abstraites (valeurs d'axes source,
  valeurs d'axes destination, service) dont le verdict de principe bascule.
- Conventions EARS, « écriture », « période », sévérités : comme dans
  [`002/requirements.md`](../002-noyau-adressage/requirements.md#conventions).

## Exigences

### A. Axes et classification

#### REQ-010-1 — Propriété d'environnement
Le module doit déclarer la propriété `policy.environment` : liste fermée, ensemble,
héritable sur l'arbre d'adressage, portée par les domaines, conteneurs, réseaux et
adresses (S2, S3).

Acceptation :
- Conteneur `10.0.0.0/8` en `{prod, global}`, réseau `10.1.0.0/24` affecté `{rec}` :
  l'adresse `10.1.0.5` résout `{rec}` et l'explication montre `{prod, global}` remplacé
  (D26, REQ-003-12, 14).
- Affecter un environnement à une plage est refusé (REQ-003-4).

#### REQ-010-2 — Attributs d'un environnement
Le module doit permettre de créer, modifier et clore un environnement avec une clé
(syntaxe des valeurs de liste fermée), un libellé et une description, en une seule
opération qui crée aussi la valeur de liste (S2).

Acceptation :
- Créer `preprod` le 1er mars : la valeur n'est pas proposée pour une affectation au
  15 février (REQ-003-5).
- Clore `sandbox` alors qu'un nœud ou une règle la cite : refusé, avec la liste (D33,
  REQ-003-6).

#### REQ-010-3 — Machine : environnement déclaré
Où le module Machines (009) est présent, le module doit déclarer
`policy.declared_environment` : liste fermée partageant les valeurs de
`policy.environment`, ensemble, non héritable, portée par les machines ; elle n'entre
dans aucun verdict (S3, D22).

#### REQ-010-29 — Axes de classification
Le module doit permettre à un administrateur de désigner comme **axe** de la politique
toute propriété de liste fermée (déclarée par un module ou par un administrateur, P1) ou
de référence externe (N26), avec son porteur (arbre d'adressage ou machine) ; seul
`policy.environment` est imposé (S22, Q5).

Acceptation :
- L'installation propose en exemple, sans les imposer, `policy.exposure` (internet,
  partenaires, interne, administration), `policy.sensitivity` (public, interne,
  confidentiel, restreint, plus l'indicateur `personal_data`), `policy.criticality`
  (standard, importante, vitale), `policy.application` et `policy.owner` (références
  externes) et `policy.conduit_role` (S26, Q22).
- Une règle qui cite une propriété non désignée comme axe : refusée
  (`policy.rule.unknown_axis`).

#### REQ-010-30 — Attributs d'une valeur d'axe
Le module doit permettre de marquer toute valeur d'un axe comme **transverse** et de lui
donner un **rang** facultatif (entier) ; le rang ne sert qu'aux conditions et aux
signaux, jamais au verdict (S2, S30).

Acceptation :
- `global` transverse : le rapport d'audit liste les autorisations qui ne tiennent qu'à
  elle (REQ-010-23).

#### REQ-010-31 — Axes portés par la machine
Où le module Machines est présent, quand un axe est porté par la machine, le module doit
résoudre sa valeur pour une adresse comme l'union des valeurs des machines rattachées à
l'adresse à la date, et, si aucune machine n'en porte, comme la valeur héritée de l'arbre
d'adressage (défaut hérité) ; l'explication dit laquelle des deux sources a servi
(S27, Q6, M10).

Acceptation :
- Réseau `{interne}` sur `policy.sensitivity`, machine `db-1` `{confidentiel}` rattachée
  à `10.1.0.5` : l'adresse résout `{confidentiel}` (source : machine) ; `10.1.0.6` sans
  machine résout `{interne}` (source : arbre).
- Une VIP rattachée à deux machines `{interne}` et `{confidentiel}` résout l'union.

#### REQ-010-4 — Types de flux
Le module doit permettre de créer, modifier et clore un type de flux (clé, libellé,
description) ; la clôture est refusée tant qu'un service ou une règle le cite (S4).

### B. Catalogue de services

#### REQ-010-5 — Service nommé
Le module doit permettre de déclarer un service avec un identifiant unique (1 à 63
caractères, `[a-z0-9]` et traits d'union non doublés ni en bord), un libellé, **un seul**
type de flux et une liste non vide de couples (S5, D29) :
- `tcp`, `udp`, `sctp`, `dccp` : ports ou plages fermées dans 1–65535 ;
- `icmp`, `icmpv6` : type 0–255 et code 0–255 facultatifs ;
- `ip` : numéro de protocole 0–255, sans port.

Acceptation :
- `ssh` = `tcp/22`, type `admin` ; `rdp` = `tcp/3389`, `udp/3389`, type `admin` ;
  `https` = `tcp/443`, type `application`.
- `tcp/0`, `tcp/70000`, `icmp` avec un port, deux types de flux : refusés.

#### REQ-010-6 — Ports partagés
Le module doit accepter qu'un même couple transport/port figure dans plusieurs
services (D29).

Acceptation :
- `https` (`tcp/443`, `application`) et `https-admin` (`tcp/443`, `admin`) coexistent.

#### REQ-010-7 — Avertissement d'ouverture large
Quand un service couvre plus de 1 024 ports d'un même transport, le module doit
accepter l'écriture avec l'avertissement `policy.service.wide_ports`.

### C. Politique

#### REQ-010-8 — Règle de politique
Le module doit permettre de déclarer une règle avec : une clé unique, un libellé, un
sélecteur source et un sélecteur destination (pour chaque axe cité, un ensemble non vide
de valeurs ; vide = toute extrémité), des contraintes « **même valeur** » sur des axes
(les deux extrémités partagent au moins une valeur), un sélecteur de flux **non vide**
(types de flux ou services), un effet `allow` ou `deny`, des motifs requis (effet `allow`
seulement), l'appartenance éventuelle à un motif, et une période ; les règles n'ont pas
d'ordre (S6, S7, S23, D28 amendée, Q7, Q9, Q19).

Acceptation :
- « Jamais d'`admin` de `dev` vers `prod` » : `deny`, source `{environment: dev}`,
  destination `{environment: prod}`, flux type `admin`.
- « `telnet` interdit » : `deny`, sélecteurs d'extrémité vides, flux `{telnet}`.
- « Même application, `database` autorisé en production » : `allow`, source et
  destination `{environment: prod}`, même valeur sur `policy.application`.
- Un sélecteur de flux vide (« tout flux ») : refusé (`policy.rule.flow_selector_empty`).
- Changer l'ordre d'écriture ou d'import de règles ne change aucun verdict.

#### REQ-010-9 — Motif d'architecture
Le module doit permettre de déclarer un motif (clé, libellé, texte pédagogique) qui
regroupe des règles ; un motif **correspond** à une paire et un service si l'une de ses
règles `allow` s'applique et qu'aucune de ses règles `deny` ne s'applique (S24, Q9).

Acceptation :
- Motif `object-storage-relay` : `allow` `file` de toute extrémité vers
  `{conduit_role: object_storage}` et de `{conduit_role: object_storage}` vers toute
  extrémité ; l'explication d'un verdict qui le cite donne son nom et son texte.

#### REQ-010-32 — Autorisation conditionnelle
Quand une règle `allow` exige des motifs, le module doit la traiter comme satisfaite si
l'un des motifs requis correspond à la paire et au service, et sinon comme **autorisation
sous réserve** : verdict `allowed` sous réserve, issue `review`, motifs attendus cités
(S25, Q11, D25).

Acceptation :
- `prod --file-> global` exige `object-storage-relay` : vers un stockage d'objets,
  `allowed` (élément décisif : le motif) ; vers un serveur de fichiers, `allowed` sous
  réserve, issue `review`, texte du motif rendu.
- Une autorisation inconditionnelle applicable en même temps l'emporte : `allowed`, issue
  selon l'admissibilité.

#### REQ-010-10 — Refus par défaut
Le module doit refuser toute paire et tout service qu'aucune règle `allow` n'atteint
(D28).

#### REQ-010-33 — Texte pédagogique
Le module doit exiger un texte pédagogique (bonne pratique attendue, lien facultatif) sur
toute règle `deny`, tout motif et toute condition d'admissibilité, le permettre sur une
règle `allow`, le conserver par langue avec repli sur la langue par défaut de
l'instance, et le restituer avec le verdict et les non-conformités (S29, EB-9).

Acceptation :
- Une règle `deny` sans texte : refusée (`policy.text.missing`).
- Verdict demandé en anglais pour une règle rédigée en français seulement : texte
  français rendu, langue indiquée.

#### REQ-010-34 — Vue matricielle
Le module doit offrir une vue (environnement source × environnement destination × type
de flux) des règles qui ne citent qu'une valeur d'environnement de chaque côté et un type
de flux, en lecture et en écriture, avec trois états : `allow`, `deny`, `conditional`
(autorisation qui exige des motifs) ; l'écriture d'une cellule doit exiger `If-Match`
portant l'ETag de la matrice du type de flux, comparé de façon atomique (S28, S35,
EB-11, N36, 006/H8).

Acceptation :
- Écrire la cellule `(prod, file, global)` en `conditional` crée une règle `allow` qui
  exige les motifs choisis ; la lire rend `conditional`.
- Une règle multi-axes n'apparaît pas dans la vue ; la vue le signale par un décompte des
  règles hors vue.
- `PUT` d'une cellule sans `If-Match` → 428 ; avec l'ETag d'une matrice modifiée depuis
  (autre cellule du même type de flux écrite entre-temps) → 412, rien n'est écrit.
- Changeset composé dont une opération `policy.matrix.set` porte un `if_match` périmé :
  refusé (`policy.matrix.stale`).

#### REQ-010-35 — Import de la politique existante
Le module doit accepter, par un convertisseur hors ligne qui produit des changesets, une
matrice à trois états (« partiel » = autorisation conditionnelle), un catalogue en deux
tables (service, transport, port ; service, autorisé, description : « autorisé = faux »
devient une règle `deny` sans sélecteur d'extrémité) et des règles d'inférence (REQ-010-53),
et lister en simulation les différences de résultat avec l'outil d'origine (S28, UC-11,
EB-28).

### D. Verdicts et triage

#### REQ-010-11 — Évaluation
Quand on demande le verdict d'une paire d'adresses (A, B) pour un service S à
(`valid_at`, `as_of`), le module doit, sans dépendre de l'ordre des règles (S8, D28
amendée) :
1. refuser avec le motif `unclassified` (axe cité) si A ou B n'a aucune valeur sur un axe
   que cite au moins une règle dont le sélecteur de flux couvre S ;
2. refuser si une règle `deny` s'applique à S et à au moins une combinaison de valeurs de
   A et de B, en citant la plus spécifique (celle qui cite le plus de critères) ;
3. autoriser si une règle `allow` inconditionnelle s'applique à au moins une combinaison
   (D25), en citant la plus spécifique et la combinaison ;
4. autoriser si une règle `allow` conditionnelle s'applique et qu'un motif requis
   correspond, en citant le motif ;
5. autoriser sous réserve si seule une règle `allow` conditionnelle sans motif
   correspondant s'applique (REQ-010-32) ;
6. sinon refuser avec le motif `no_rule`.

Acceptation :
- A `{prod, global}`, B `{dev}`, règle `allow admin global → dev` seule : `ssh` autorisé,
  combinaison `(global, dev)` citée.
- Même cas plus `deny admin prod → dev` : refusé, la règle `deny` citée (le refus
  l'emporte même si une autre combinaison est autorisée).
- B sans environnement : refusé `unclassified` (axe `policy.environment`), quelle que
  soit la politique.
- B sans sensibilité alors qu'aucune règle candidate pour `ssh` ne cite la sensibilité :
  pas de refus `unclassified`.

#### REQ-010-12 — Explication
Le module doit joindre à chaque verdict : la demande normalisée et (`valid_at`,
`as_of`) ; par extrémité, adresse et axe cité, les valeurs résolues avec origine et
chaîne (REQ-003-14) ou la machine source (REQ-010-31) ; le service et son type ;
l'élément décisif (règle, motif ou cause) avec sa clé, la révision de politique, la
combinaison de valeurs qui l'a déclenché et son texte pédagogique ; trois règles
écartées au plus et pourquoi ; les conditions non remplies et leur texte ; le score et
les avertissements ; l'issue et la suite attendue (S31, Q14).

#### REQ-010-13 — Extrémités
Le module doit accepter comme extrémité une adresse, un préfixe (évalué sur ses propres
classifications) et, où le module Machines est présent, une interface ou une machine,
développées en leurs adresses à `valid_at` ; il doit rendre un verdict par paire
d'adresses (D22).

Acceptation :
- Machine A (2 adresses) vers machine B (3 adresses), service `ssh` : 6 verdicts.
- Une machine sans adresse à la date : réponse vide avec le motif `no_address`.

#### REQ-010-14 — Objet de la requête
Le module doit accepter comme objet de la requête : un service ; un couple
transport/port, évalué pour chaque service qui le couvre ; ou un type de flux, évalué
pour chacun de ses services (S9, D29). Un couple qu'aucun service ne couvre doit donner
`denied`, motif `unknown_service`, et l'issue `review` avec l'invitation à nommer le
service ou à demander son ajout au catalogue (Q19).

Acceptation :
- `tcp/443` avec `https` (`application`) et `https-admin` (`admin`) : deux verdicts.
- `tcp/9999` sans service : `denied` `unknown_service`, issue `review`.

#### REQ-010-15 — Synthèse
Le module doit accompagner les verdicts d'une synthèse : `allowed` si tous les verdicts
autorisent, `denied` si aucun, `partial` sinon, avec les décomptes ; et d'une issue de
synthèse, la plus défavorable des issues (`noncompliant` > `review` > `automatic`).

#### REQ-010-16 — Verdicts datés
Le module doit évaluer classifications, règles, motifs, conditions, signaux et services
tels qu'ils étaient à (`valid_at`, `as_of`) (S16, D10).

Acceptation :
- Règle `allow` ajoutée le 1er avril : un verdict au 1er mars est `denied`, au 2 avril
  `allowed` ; avec `as_of` antérieur à l'ajout, `denied` à toute date.

#### REQ-010-36 — Conditions d'admissibilité et issue
Le module doit permettre de déclarer des conditions d'admissibilité nommées, d'un genre
pris dans une liste fermée extensible par le code (champ du dossier présent, largeur
maximale d'une extrémité, extrémité qui n'est pas un domaine entier, service nommé
plutôt que port brut, rang d'une valeur du dossier au plus celui d'un axe de la
destination), avec paramètres, sélecteur facultatif (règles, types de flux) et texte
pédagogique ; et déduire l'issue : verdict `denied` → `noncompliant` ; verdict `allowed`
sous réserve ou condition applicable non remplie → `review` ; sinon `automatic`
(S30, Q2, Q4, EB-12).

Acceptation :
- Condition `max-width` (au plus 256 adresses par extrémité) : un flux autorisé depuis
  un `/16` part en `review` avec la condition et son texte.
- Condition `data-sensitivity` : dossier `confidentiel` (rang 3) vers une destination
  `interne` (rang 2) : `review`.

#### REQ-010-37 — Score et avertissements
Le module doit permettre de déclarer des signaux nommés d'un genre pris dans une liste
fermée extensible (portée des extrémités, nature du flux, croisement des axes), avec
paramètres et poids réglable, et rendre avec chaque évaluation la liste des signaux
déclenchés (avertissements) et la somme de leurs poids ; ni l'un ni l'autre ne doit
influencer le verdict ou l'issue (S32, Q4, Q20, EB-13).

Acceptation :
- Les signaux d'exemple de Q20 (paliers /24, /20, /16 ; type `admin` ou `database` ;
  service en clair ; environnements différents ; destination plus sensible ; extrémité
  exposée à internet ; autorisation qui ne tient qu'à une valeur transverse ; criticité
  haute) sont livrés en configuration d'exemple.

#### REQ-010-38 — Dossier et pré-vérification
Le module doit accepter dans la requête de conformité un dossier facultatif (références
externes, demandeur et « pour le compte de », responsables, nature des données, période)
et rendre, sans aucune écriture, la même évaluation que l'enregistrement d'un flux
(011) ; sans dossier, les conditions qui portent sur le dossier sont rendues comme non
évaluées et l'issue n'est pas `automatic` (S9, S33, UC-1, N34).

### E. Cohérence des machines

#### REQ-010-17 — Divergence d'environnement
Où le module Machines est présent, quand une machine porte
`policy.declared_environment` et qu'une adresse de l'une de ses interfaces résout un
ensemble différent, sauf interface d'un rôle exempté, le module doit produire
l'anomalie (avertissement) `policy.machine.environment_mismatch` ; sans environnement
déclaré, la même anomalie naît de deux adresses (hors rôles exemptés) dont les ensembles
diffèrent (S10, D22). La liste des rôles exemptés est administrable (par défaut :
administration, sauvegarde, supervision).

Acceptation :
- Machine déclarée `{prod}`, adresse de service `{prod}`, adresse d'administration
  (rôle administration) `{admin}` : aucune anomalie ; sans le rôle : anomalie.

### F. Différentiel et approbation

#### REQ-010-18 — Différentiel de politique
Quand un changeset touche une règle, un motif, un service, un type de flux ou un
attribut de valeur d'axe, le module doit calculer, en simulation comme à l'application,
la liste des combinaisons abstraites (valeurs d'axes source, valeurs d'axes
destination, service) dont le verdict de principe bascule (`denied`, `allowed`,
`allowed` sous réserve), avec l'élément décisif avant et après, décompte d'abord et
détail paginé (S11, D25, Q15).

Acceptation :
- Ajouter `allow admin global → dev` avec 3 services `admin` : 3 combinaisons
  `denied → allowed`.
- Ajouter `deny admin prod → dev` qui masque une autorisation : les combinaisons
  concernées `allowed → denied`, règle citée.

#### REQ-010-19 — Différentiel des nœuds
Le module doit laisser les changements de classification des nœuds au différentiel de
propriétés de la 003 (REQ-003-19), sans le dupliquer, et y ajouter pour chaque nœud
reclassé sur un axe le nombre d'héritiers dont la valeur résolue change ; leurs effets
sur des flux déclarés relèvent de la 011.

#### REQ-010-20 — Déclencheurs d'approbation
Le module doit fournir aux politiques d'approbation (REQ-004-40) les déclencheurs
`policy.write` (toute écriture de politique), `policy.opens` (au moins N combinaisons
`denied → allowed`), `policy.closes` (au moins N `allowed → denied`) et
`policy.reclassify` (au moins N héritiers reclassés sur un axe), sans qu'aucun ne
s'applique par défaut (S12, REQ-004-39, Q15, N30).

### G. Inférence

#### REQ-010-40 — Moteur générique
Le module doit évaluer des règles d'inférence qui produisent des valeurs **attendues**
pour tout axe désigné comme visé par l'inférence ; seul `policy.environment` l'est à
l'installation (I1, S18).

#### REQ-010-41 — Objets évalués
Le module doit évaluer les règles d'inférence sur les nœuds de l'arbre d'adressage
(domaines, conteneurs, réseaux, adresses) ; les machines sont hors v0.2 (I2).

#### REQ-010-42 — Critères
Le module doit permettre, dans une règle, des critères pris dans une liste fermée
extensible par le code : inclusion dans un préfixe, égalité à un préfixe, famille,
domaine, type d'objet, site et entité résolus (avec leurs sous-arbres), valeur d'une
autre propriété (égalité, appartenance, présence), nom par expression régulière,
longueur de préfixe ; combinés en ET, chaque critère acceptant une liste de valeurs (OU)
(I3, Q10, Q13).

Acceptation :
- « Le nom commence par le marqueur de production et le site est A ou B » s'écrit en une
  règle de deux critères.
- Une expression régulière non ancrée, de plus de 256 caractères ou hors de la syntaxe du
  moteur à temps linéaire : refusée (`policy.inference.regex_invalid`).

#### REQ-010-43 — Transmission à la sous-branche
Quand une règle correspond à un nœud, le module doit transmettre ses valeurs attendues à
toute la sous-branche du nœud, sauf là où une règle plus forte ou une exception
s'applique (I4).

#### REQ-010-44 — Exceptions
Le module doit permettre des exceptions imbriquées sous une règle (profondeur au plus
4) ; une exception ne s'évalue que sur les nœuds qui ont correspondu à sa règle parente et
**remplace** l'ensemble attendu (I5, D26).

Acceptation :
- « Éléments nommés production, sauf ceux du sous-réseau X, qui sont recette » : les
  nœuds de X attendent `{rec}`.
- Une cinquième imbrication : refusée.

#### REQ-010-45 — Départage
Quand plusieurs règles produisent des attendus pour un nœud, le module doit retenir la
priorité explicite la plus haute, puis le nœud de correspondance le plus profond ; si
une égalité demeure entre des attendus différents, il doit rendre **tous les candidats**,
n'écrire aucune valeur par défaut, ne lever aucune anomalie d'écart sur ce nœud, et
lever l'anomalie (avertissement) `policy.inference.conflict` sur le jeu de règles
(I6 adouci, Q12, D24 amendée).

Acceptation :
- Deux règles de priorité 10 sur le même conteneur, `{prod}` et `{rec}` : la suggestion
  propose les deux ; la création d'une adresse n'écrit aucun défaut ; une anomalie de
  conflit nomme les deux règles.

#### REQ-010-46 — Portée et droits
Le module doit tenir un jeu de règles d'inférence global, dont une règle peut citer un
domaine ; écrire une règle qui cite un domaine doit exiger `policy.inference.admin` sur
ce domaine, et sur tout le référentiel sinon (I7).

#### REQ-010-47 — Confrontation
Le module doit comparer l'ensemble déclaré et l'ensemble attendu d'un nœud par égalité ;
un nœud sans attendu ou à plusieurs candidats ne doit jamais produire d'écart (I8).

#### REQ-010-48 — Anomalies d'écart
Le module doit produire une anomalie (avertissement) `policy.inference.mismatch` par
**nœud d'origine** de la valeur déclarée, avec le décompte des héritiers en écart,
acquittable (REQ-002-36), et la réévaluer par une tâche de fond quand les règles
changent (I9, Q13).

#### REQ-010-49 — Explication de l'attendu
Le module doit expliquer un attendu par : la règle et sa chaîne d'exceptions, la
révision du jeu, le nœud qui a correspondu, chaque critère avec la valeur observée, et
trois règles plus faibles écartées au plus (I10).

#### REQ-010-50 — Versionnement
Le module doit écrire les règles d'inférence par changeset, les conserver en bitemporel,
incrémenter une révision du jeu à chaque changeset qui le touche, et évaluer l'attendu à
(`valid_at`, `as_of`) avec les règles valides à ces dates ; la provenance d'une valeur
issue de l'inférence est `source:policy.inference@<révision>#<règle>` (I11).

#### REQ-010-51 — Suggestion
Le module doit proposer à la saisie, pour un nœud et un axe, le ou les attendus avec
leur explication ; la valeur écrite reste une décision humaine (EB-21, D23).

#### REQ-010-52 — Valeur par défaut
Quand un nœud est créé sans affectation propre de l'axe et qu'il n'hériterait d'aucune
valeur, le module doit écrire l'attendu unique comme valeur par défaut avec sa
provenance ; il ne doit rien écrire à une autre occasion ni quand plusieurs candidats
existent ; la valeur par défaut compte pour l'obligation (I12, REQ-003-22, D27).

#### REQ-010-53 — Import des règles existantes
Le convertisseur de REQ-010-35 doit traduire les règles d'inférence de l'existant en
règles et exceptions, la couche et l'ordre de lecture en priorités explicites, et lister
en simulation les nœuds dont l'attendu diffère de l'outil d'origine (I13, Q10).

#### REQ-010-54 — Budgets de l'inférence
Le module doit respecter (p95 serveur, jeu de 10⁶ objets, 2 000 règles au plus) :
suggestion pour un objet < 50 ms ; contrôle des objets d'un changeset de 10⁴ objets
< 1 s ajoutée ; réévaluation complète de 10⁶ nœuds en tâche de fond < 10 min (I14).

#### REQ-010-55 — Aperçu de l'inférence (S36)
> S36 acceptée le 2026-10-23 (tâche 010-22).

Quand un changeset crée, modifie ou clôt une règle d'inférence, le module doit ajouter
à sa contribution au différentiel, en simulation comme à l'application, les nœuds dont
l'attendu change entre l'état d'avant et celui d'après le changeset, avec l'écart
(REQ-010-47) d'avant et d'après, et les décomptes des nœuds changés, des écarts ouverts
et fermés (en nœuds et en nœuds d'origine), sans lever ni clore d'anomalie
(REQ-017-34, S36).

Bornes (p95 serveur, jeu de 10⁶ objets, 2 000 règles au plus ; à confirmer par le banc
010-19) : < 1 s ajoutée pour une portée de 10⁴ nœuds, < 10 s pour 10⁵ ; au-delà de 10⁵
nœuds dans la portée, décompte de la portée seul.

Acceptation :
- Simulation d'une règle `prod` sur un préfixe de 12 nœuds sans attendu, dont 3
  déclarés `dev` et les autres non classés : 12 nœuds changés, 3 écarts ouverts, rien
  n'est écrit et aucune anomalie n'est levée.
- Simulation de la clôture de cette règle, une fois appliquée : 12 nœuds changés,
  3 écarts fermés.
- Règle de priorité inférieure à une règle qui couvre déjà le préfixe : aucun nœud
  changé.
- Règle dont la portée dépasse 10⁵ nœuds : synthèse avec le décompte de la portée et
  `truncated`, sans ligne de nœud ni décompte d'écarts.
- Simulation par une identité sans `policy.inference.admin` sur le domaine cité :
  refusée comme l'écriture (`core.object.forbidden`, `valid: false`, aucune section ;
  l'écriture répond 403).

### H. Classification obligatoire et audit

#### REQ-010-21 — Obligation par domaine et par axe
Le module doit permettre à un administrateur de rendre un axe obligatoire sur un
domaine, et de lever l'obligation, par changeset et sans redéploiement ; l'ajout est
refusé, avec la liste des nœuds, tant qu'un nœud du domaine n'a pas de valeur résolue sur
cet axe (S13, D27, REQ-003-6, 16, Q6).

Acceptation :
- Domaine D avec 12 adresses sans environnement : rendre `policy.environment`
  obligatoire est refusé avec les 12 adresses ; après leur classification dans le même
  changeset : accepté.
- D obligatoire sur `policy.exposure` seul : créer un réseau sans exposition résolue est
  refusé, sans environnement accepté.

#### REQ-010-22 — Non classifié signalé
Tant qu'un nœud d'un domaine sans obligation n'a pas de valeur sur un axe cité par la
politique, le module doit le compter, par axe, dans le rapport d'audit (REQ-010-23) ;
ses verdicts sont des refus (REQ-010-11).

#### REQ-010-23 — Rapport d'audit
Le module doit produire, à la demande et par une tâche de fond, un rapport daté qui
liste : nœuds non classifiés par axe et par domaine ; nœuds dont l'ensemble résolu
contient une valeur transverse, avec le nœud d'origine ; autorisations qui ne tiennent
qu'à une valeur transverse ; valeurs d'axe citées par aucune règle ; règles dont une
valeur n'est portée par aucun nœud ; règles `allow` entièrement masquées par une règle
`deny` ; règles jamais appliquées à un flux déclaré (011) ; combinaisons autorisées
sans aucun flux déclaré ; écarts d'inférence ; anomalies de machines (S14, D25, D27,
UC-9).

Acceptation :
- Le rapport donne un décompte par rubrique et un détail paginé, consultable par l'API
  et exportable en JSON.
- Les rapports se listent, les plus récents d'abord, par état et par date, page par page
  (021-30), sous les droits du rapport.

### I. Droits, temps, API

#### REQ-010-24 — Permissions
Le module doit déclarer les permissions globales `policy.axis.admin`,
`policy.environment.admin`, `policy.flow_type.admin`, `policy.service.admin`,
`policy.rule.admin` (règles, motifs, conditions, signaux) ; `policy.inference.admin`
(portée par domaine, REQ-010-46) ; `policy.catalogue.read`, `policy.verdict.read` ;
`policy.flow.request`, portée par entité ; et la permission d'écriture de chaque axe
portée par le périmètre du nœud ou de la machine (S15, REQ-003-17, Q17).

#### REQ-010-25 — Verdict et visibilité
Le module doit rendre un verdict à l'appelant qui détient `policy.verdict.read` et la
lecture des deux extrémités, avec l'explication complète ; à l'appelant qui détient
`policy.flow.request` sur l'entité de la demande, avec une explication réduite aux
valeurs d'axes, aux règles, motifs et conditions (sans nœud d'origine, chaîne ni objet
du référentiel) ; et, à tout autre appelant, répondre comme pour un objet absent (404)
(S15, REQ-006-20, Q17, N32).

Acceptation :
- Un compte de service avec `policy.flow.request` sur l'entité E, sans lecture de
  l'arbre, obtient le verdict d'une paire et les valeurs d'environnement, pas l'adresse
  du conteneur d'origine.

#### REQ-010-26 — Versionnement
Le module doit écrire axes, attributs de valeurs, types de flux, services, règles,
motifs, conditions, signaux, textes et règles d'inférence par changeset et les conserver
en bitemporel ; ils doivent être lisibles à toute date ; chaque changeset qui touche la
politique incrémente une révision citée par les évaluations (S16, D10, D11,
REQ-004-17).

#### REQ-010-27 — API
Le module doit exposer, de manière additive et selon la clause 9 de l'ADR 0026, ses
collections (`/v1/policy-axes`, `/v1/environments`, `/v1/flow-types`, `/v1/services`,
`/v1/policy-rules`, `/v1/policy-patterns`, `/v1/admissibility-conditions`,
`/v1/risk-signals`, `/v1/inference-rules`), la vue matricielle
`/v1/policy-matrix`, la requête de conformité par `QUERY /v1/verdicts` et `GET
/v1/verdicts` (paramètres), la suggestion d'inférence et les rapports
`/v1/policy-reports` (S19).

### J. Performance

#### REQ-010-28 — Budgets
Le module doit respecter (p95 serveur, jeu de 10⁶ objets de REQ-002-45 avec 6 axes,
50 environnements, 20 types de flux, 1 000 services, 10⁴ règles, 100 motifs,
50 conditions, 30 signaux) : verdict d'une paire d'adresses pour un service, conditions
et signaux compris, < 50 ms ; machine × machine (16 × 16 adresses) pour un type de flux
de 10 services < 300 ms ; port brut couvert par 20 services < 100 ms ; différentiel de
politique d'un changeset de règles < 1 s ; rapport d'audit < 10 min (S20, D14).

## Non-objectifs

- Le registre de flux déclarés, leurs dérogations, leur réévaluation et les
  non-conformités : spec 011.
- Flux constatés et audit a posteriori des flux déjà ouverts (v0.3, place réservée par
  la 011).
- Lecture des exports de pare-feux et d'infrastructure as code, connecteurs vers le
  ticketing et la revue de code (v0.3) ; la v0.2 en pose le contrat (pré-vérification,
  enregistrement pour le compte d'un demandeur, événements sortants).
- Atteignabilité effective, NAT, routes, passerelles (D20 étape 2, v0.3 et suite).
- Observation du trafic réel ; services exposés déclarés par une machine (D29).
- Inférence sur les machines (I2).
- Campagnes de revue comme objet propre (v0.3) ; la v0.2 livre les filtres.
- Score de risque décisionnel : le score reste informatif.
- Les extrémités hors du référentiel (Internet) : à modéliser comme préfixes d'un
  domaine dédié.
- L'interface Web : plugin de première partie `web/plugins/policy/` (007/W6), à
  spécifier avec la 007.

## Questions ouvertes

- [x] S36 et REQ-010-55 (aperçu de l'inférence dans le différentiel, pour REQ-017-34) :
  acceptés par l'utilisateur le 2026-10-23.
- [ ] Budgets de REQ-010-28 et REQ-010-54 : tenus en variante réduite par le banc
  `bench-policy` (2026-10-22, tâche 010-19) ; à confirmer sur le jeu de 10⁶ objets.
  Le rapport d'audit y dépassait le budget (≈ 990 s). (Claude, tâches 010-1 et 010-19)
- [ ] Exigences envers le noyau N25 à N33 (design, § Exigences) : à porter dans les specs
  003 à 005 avant la v0.2. (Claude)

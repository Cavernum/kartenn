---
status: implemented
updated: 2026-10-17
---

# 003 — Propriétés : exigences

## Résumé

Le noyau ne porte que les attributs dont dépendent ses invariants et le RBAC (002/A10).
Tout le reste (noms affichés, FQDN, environnements, centres de coût…) est une
**propriété** : déclarée par un module ou par un administrateur, typée, validée
strictement, éventuellement héritée le long d'un arbre, résolue à une date donnée avec
une explication, protégée par sa propre permission. C'est le mécanisme dont dépend le
module Politique de flux (v0.2) ; il est construit et mesuré dès la v0.1 (D31).

Décisions : [`decisions.md`](decisions.md) (P1–P14) ; D19, D21–D27, D30, D34
([`../001-cadrage/decisions.md`](../001-cadrage/decisions.md)) ; A10
([`../002-noyau-adressage/decisions.md`](../002-noyau-adressage/decisions.md)).

### Vocabulaire

- **Déclaration** : définition d'une propriété (clé, type, cardinalité, héritage,
  objets porteurs, obligations, permission).
- **Affectation** : valeur posée explicitement sur un objet, y compris « aucune valeur ».
- **Valeur résolue** : valeur effective d'une propriété pour un objet, à une date :
  affectation propre, sinon héritée, sinon absente.
- **Différentiel** : liste des objets existants dont une valeur résolue change du fait
  d'un changeset.
- Conventions EARS, « écriture », « période » et sévérités : comme dans
  [`002/requirements.md`](../002-noyau-adressage/requirements.md#conventions).

## Exigences

### A. Déclaration

#### REQ-003-1 — Déclarants
kartenn doit permettre à un module de déclarer des propriétés dans son code, et à un
administrateur d'en déclarer à l'exécution avec les seuls types intégrés (P1).

Acceptation :
- Un administrateur crée `custom.centre_cout` (texte) sans redéploiement ; il ne peut pas
  choisir un type qu'aucun module ne fournit.

#### REQ-003-2 — Clé de propriété
kartenn doit identifier chaque propriété par une clé unique de la forme
`<espace>.<nom>` (`[a-z][a-z0-9_]*` pour chaque partie), où l'espace est le nom du
module déclarant, ou `custom` pour les propriétés d'administrateur.

Acceptation :
- `policy.environment`, `dns.fqdn`, `custom.centre_cout` acceptées ; un administrateur ne
  peut pas créer de clé dans un espace de module ; deux déclarations de même clé
  valides sur une période commune sont refusées.

#### REQ-003-3 — Contenu d'une déclaration
kartenn doit exiger qu'une déclaration précise : type et paramètres du type,
cardinalité (valeur unique ou ensemble), types d'objets porteurs, héritabilité et arbre
suivi (REQ-003-11) et permission d'écriture (REQ-003-17). Une déclaration ne porte
aucune exigence d'approbation (004/C6).

#### REQ-003-4 — Objets porteurs
Si une affectation vise un objet d'un type non déclaré porteur de la propriété, alors
kartenn doit refuser l'écriture.

Acceptation :
- `dns.fqdn` porté par les adresses : l'affecter à un domaine est refusé.

#### REQ-003-5 — Déclarations versionnées
kartenn doit conserver les déclarations et les valeurs de listes fermées en bitemporel
(D10), et résoudre une propriété selon la déclaration valide à la date demandée (P8).

Acceptation :
- La valeur `preprod` ajoutée à `policy.environment` le 1er mars n'est pas proposée pour
  une affectation valide au 15 février.

#### REQ-003-6 — Évolution compatible
Si une modification de déclaration rendrait invalide une affectation existante ou
changerait une valeur résolue sans affectation explicite, alors kartenn doit la refuser
en listant les objets concernés (P8).

Acceptation :
- Réduire la longueur maximale d'un texte sous celle d'une valeur existante : refusé.
- Clore la valeur `sandbox` alors que des objets la portent : refusé ; après avoir clos
  ces affectations dans le même changeset : accepté.
- Passer une propriété affectée d'héritable à non héritable : refusé si des objets
  perdraient leur valeur résolue.

### B. Types et valeurs

#### REQ-003-7 — Types intégrés
kartenn doit fournir les types : liste fermée, texte, entier, booléen, date, FQDN et
référence (P2), chacun en valeur unique ou en ensemble.

#### REQ-003-8 — Validation par type
Si une valeur ne respecte pas son type et ses paramètres, alors kartenn doit refuser
l'écriture en citant la contrainte violée. Règles :
- liste fermée : valeur valide à la date de l'affectation ;
- texte : normalisé en Unicode NFC, longueur (en caractères) dans les bornes déclarées,
  expression régulière déclarée respectée, aucun caractère de contrôle ;
- entier : 64 bits signé, dans les bornes déclarées ;
- date : `full-date` de RFC 3339 ;
- FQDN : RFC 1123 §2.1 (labels LDH de 1 à 63 caractères, 253 au total), noms
  internationalisés convertis en A-label, stocké en minuscules sans point final ;
- référence : objet existant, du type déclaré, valide sur toute la période de
  l'affectation.

Acceptation :
- FQDN : `Srv-01.Example.ORG.` → `srv-01.example.org` ; `srv_01.example.org` refusé ;
  `café.example` → `xn--caf-dma.example`.
- Texte : une chaîne contenant U+0007 est refusée.

#### REQ-003-9 — Ensembles
kartenn doit traiter une valeur d'ensemble comme un ensemble mathématique : sans
doublon, sans ordre, comparé par égalité d'ensemble, et non vide (P11).

Acceptation :
- `{prod, global}` et `{global, prod}` sont égaux ; `{prod, prod}` est stocké `{prod}`.
- `{}` est refusé : l'absence de valeur s'exprime par l'effacement (REQ-003-13).
  Ajouté le 2026-10-14 (P11).

#### REQ-003-10 — Références et clôture
Si la clôture d'un objet laisserait une affectation de type référence sans cible valide
sur une période, alors kartenn doit refuser l'écriture en listant ces affectations (D33).

### C. Héritage et résolution

#### REQ-003-11 — Arbres d'héritage
Là où une propriété est déclarée héritable, kartenn doit la propager le long d'un seul
arbre : adressage (domaine → conteneurs → réseau → plage ou adresse), entités ou sites
(P3). La propagation ne passe jamais d'un arbre à l'autre, et une adresse n'hérite pas
d'une plage.

Acceptation :
- `custom.criticite` héritable sur l'arbre d'adressage, posée sur le conteneur
  `10.0.0.0/8` : l'adresse `10.1.2.3` (réseau `10.1.2.0/24`) l'hérite ; le site du réseau
  ne joue aucun rôle.

#### REQ-003-12 — Remplacement
Quand un objet porte une affectation, kartenn doit retenir cette affectation comme valeur
résolue, en remplaçant entièrement la valeur héritée, ensembles compris (D26).

Acceptation :
- Conteneur `{prod, global}`, réseau enfant affecté `{rec}` : les adresses du réseau
  résolvent `{rec}`.

#### REQ-003-13 — Effacement explicite
kartenn doit permettre d'affecter explicitement « aucune valeur » à une propriété ;
l'objet et ses descendants sans affectation propre résolvent alors une valeur absente (P4).

#### REQ-003-14 — Explication
kartenn doit restituer pour toute valeur résolue : la valeur, l'objet d'origine (lui-même
ou un ancêtre), la chaîne d'objets parcourue, la provenance de l'affectation (saisie, ou
valeur par défaut d'une source d'inférence avec sa version, REQ-003-22) et, s'il y a lieu,
la valeur héritée qu'elle remplace.

#### REQ-003-15 — Résolution datée
kartenn doit résoudre une propriété à tout couple (`valid_at`, `as_of`) (D10), en
tenant compte de l'arbre tel qu'il était à `valid_at` (D34).

Acceptation :
- Un conteneur intermédiaire créé le 1er mars change la valeur résolue d'une adresse à
  partir du 1er mars, pas avant.

### D. Obligation

#### REQ-003-16 — Obligation portée
Là où une propriété est déclarée obligatoire sur une portée (domaine, entité ou type
d'objet), si une écriture laisse un objet de cette portée sans valeur résolue, alors
kartenn doit refuser l'écriture en listant les objets concernés ; l'effacement explicite
y est refusé (P4, D27).

Acceptation :
- `policy.environment` obligatoire sur le domaine D : créer un réseau sous un conteneur
  sans environnement est refusé, sauf si le réseau reçoit une affectation ou une valeur
  par défaut (REQ-003-22).

### E. Droits

#### REQ-003-17 — Permission par propriété
kartenn doit exiger, pour toute affectation, la permission d'écriture propre à la
propriété sur le périmètre de l'objet (D36), indépendamment du droit de modifier
l'objet (P6). L'approbation éventuelle relève des politiques facultatives de la 004
(004/C6).

Acceptation :
- Un prestataire autorisé à modifier le réseau `10.5.0.0/24` mais pas
  `policy.environment` ne peut pas y affecter d'environnement.

#### REQ-003-18 — Droit sur l'effet
Si un changeset modifie la valeur résolue d'une propriété sur un objet existant, y compris
indirectement par une modification de structure (préfixe inséré, redimensionné, clos,
changement de nature), alors kartenn doit exiger de l'auteur la permission de cette
propriété sur chacun de ces objets, et refuser le changeset à défaut, sauf si l'auteur
détient le droit de proposer : le changeset devient alors une proposition
(004/REQ-004-41) (P7).

Acceptation :
- Conteneur `10.0.0.0/8` en `{prod}`, conteneur `10.5.0.0/16` en `{dev}` : un prestataire
  sans droit sur `policy.environment` qui crée un conteneur `10.5.0.0/20` sans affectation
  ne change rien (ses descendants restent `{dev}`) : accepté ; s'il clôt `10.5.0.0/16`, les
  objets dessous passeraient en `{prod}` : refusé.
- Créer une nouvelle adresse qui hérite `{dev}` : accepté (objet nouveau).

### F. Différentiel

#### REQ-003-19 — Différentiel de changeset
Quand un changeset est soumis, kartenn doit calculer et restituer le différentiel :
pour chaque propriété, les objets existants dont la valeur résolue change (avant,
après), avec un décompte et un détail paginé.

Acceptation :
- Insérer un conteneur `{dev}` au-dessus de 4 096 adresses `{prod}` : le différentiel
  indique 4 096 objets `policy.environment` : `{prod}` → `{dev}`.

#### REQ-003-20 — Différentiel avant application
kartenn doit permettre d'obtenir le différentiel d'un changeset sans l'appliquer
(simulation).

### G. Valeurs attendues (inférence)

#### REQ-003-21 — Contrat de source de valeurs attendues
kartenn doit permettre à un module d'associer à une propriété une source de valeurs
attendues, qui calcule pour un objet une valeur attendue (ou aucune) à partir des seuls
objets du noyau et de leurs propriétés, avec une explication (P5).

#### REQ-003-22 — Valeur par défaut
Quand un objet est créé sans affectation d'une propriété qui a une source de valeurs
attendues configurée comme défaut, kartenn doit y affecter la valeur attendue, avec la
provenance « source X, version Y », dans le même changeset (D24).

Acceptation :
- La provenance figure dans l'explication (REQ-003-14) ; modifier ensuite la source ne
  change pas cette affectation.

#### REQ-003-23 — Suggestion
kartenn doit restituer, à la demande, la valeur attendue d'une propriété pour un objet
existant ou en cours de saisie, sans rien écrire.

#### REQ-003-24 — Écart déclaré / attendu
Quand la valeur résolue d'une propriété diffère de sa valeur attendue, kartenn doit
produire une anomalie (avertissement) qui cite la propriété, les deux valeurs et
l'explication de la source (D23). Les verdicts et contrôles utilisent toujours la valeur
résolue, jamais la valeur attendue.

### H. Lecture et performance

#### REQ-003-25 — Filtrage
kartenn doit permettre de filtrer les objets par valeur résolue d'une propriété
(égalité, appartenance pour une liste fermée ou un ensemble, intervalle pour entier et
date, présence ou absence), à une date donnée.

#### REQ-003-26 — Budgets
kartenn doit respecter, sur le jeu de 10⁶ objets de REQ-002-45 avec 10 propriétés dont 3
héritables : résolution expliquée d'une propriété pour un objet < 50 ms ; page filtrée
par valeur résolue < 150 ms ; changeset touchant jusqu'à 10⁴ objets < 2,5 s, historique
et journal compris (P14) ; différentiel jusqu'à 10⁶ objets < 10 s, décompte d'abord, détail paginé (p95 serveur). Par
exception, quand plus de 10⁵ objets changent d'**objet d'origine** (clôture ou effacement
très haut dans l'arbre), le différentiel et l'application doivent tenir en < 60 s, la
simulation donnant d'abord le décompte (P10).

### I. Sensibilité (amendement du 2026-10-17, P17, spec 012)

#### REQ-003-27 — Propriété sensible
kartenn doit permettre de marquer une propriété comme sensible, et de retirer la marque,
par un changeset audité ; la marque est versionnée avec la déclaration. La lecture de la
valeur d'une propriété sensible doit être auditée (012/REQ-012-18).

Acceptation :
- `custom.contact` marquée sensible : la lecture d'un objet qui la porte produit
  `data.sensitive.read` ; la marque retirée, plus rien ; les deux changesets sont audités.

## Non-objectifs

- Le moteur de règles d'inférence (DSL, exceptions, priorités, D24) : v0.2, spec 010.
- Les propriétés propres au module Politique (environnements, matrice) : spec 010.
- Le JSON libre et tout type non validable (P2).
- Les types fournis par des administrateurs avec du code (P1).
- Les arbres d'héritage propres aux modules (VLAN, machines) : leurs specs.
- Le format exact des requêtes de filtrage dans l'API : spec 006.

## Questions ouvertes

- [x] L'entité propriétaire héritée (REQ-002-38) devient-elle une propriété de référence
  héritable du noyau, ou reste-t-elle un cas particulier ? Propriété du noyau `core.owner`
  (ADR 0014, implémentée en 002-22). (conception 002/003, Claude)
- [x] Budgets du différentiel (REQ-003-26) : confirmés par le prototype 003, sauf les
  changements d'origine de plus de 10⁵ objets, assouplis à < 60 s (P10). (Claude)
- [x] Budget du changeset touchant jusqu'à 10⁴ objets (REQ-003-26) : 2,07 s mesurés au banc
  003-16 pour 1 s visé ; seuil porté à 2,5 s pour le changeset entier (P14). (utilisateur)
- [x] Note de référence à rédiger avant la conception : noms de domaine (RFC 1123 §2.1,
  RFC 1035, RFC 5890–5891 IDNA), RFC 3339 et normalisation Unicode (UAX #15) :
  [`noms-dates-texte.md`](../../docs/reference/noms-dates-texte.md). (Claude)

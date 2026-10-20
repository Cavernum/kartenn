---
status: draft
updated: 2026-10-20
---

# 010 — Politique de flux : exigences

## Résumé

Le module Politique (`policy`, v0.2, D31) répond à la question qui motive kartenn
(D19) : « la machine A peut-elle ouvrir tel flux vers la machine B selon la politique ? ».
Il classe les nœuds de l'arbre d'adressage par **environnement** (une propriété
héritable de la 003), décrit un **catalogue de services** classés par **type de flux**,
des **garde-fous** de refus absolus et une **matrice** d'autorisations, et rend un
**verdict expliqué** pour toute paire d'extrémités, à toute date. Tout changement de
politique montre, avant d'être appliqué, ce qu'il ouvre ou ferme.

Les règles d'inférence (environnement attendu, D23, D24) sont hors de ces exigences
tant que la session dédiée ne les a pas arbitrées
([`inference-questions.md`](inference-questions.md)) ; aucune exigence ci-dessous n'en
dépend.

Décisions : [`decisions.md`](decisions.md) (S1–S21) ; D19 à D30
([`../001-cadrage/decisions.md`](../001-cadrage/decisions.md)) ; 003 (propriétés), 004
(changesets), 005 (RBAC), 006 (API). Registre de flux : spec 011.

### Vocabulaire

- **Environnement** : valeur de la liste fermée de `policy.environment` ; un nœud en
  porte un **ensemble** (D25).
- **Nœud** : domaine, conteneur, réseau ou adresse (arbre d'adressage, REQ-003-11).
- **Non classifié** : nœud dont `policy.environment` n'a pas de valeur résolue.
- **Type de flux** : catégorie de flux (base de données, administration…).
- **Service** : entrée nommée du catalogue : un type de flux et des couples
  transport/ports (D29).
- **Garde-fou** : refus absolu (D28, niveau 1).
- **Cellule** : autorisation (type de flux, environnement source, environnement
  destination) de la matrice (D28, niveau 2).
- **Verdict** : `allowed` ou `denied` pour une paire d'adresses et un service, avec
  l'élément décisif.
- **Différentiel de politique** : triplets (environnement source, environnement
  destination, service) dont le verdict de principe bascule du fait d'un changeset.
- Conventions EARS, « écriture », « période », sévérités : comme dans
  [`002/requirements.md`](../002-noyau-adressage/requirements.md#conventions).

## Exigences

### A. Environnements et types de flux

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
(syntaxe des valeurs de liste fermée), un libellé, une description et un drapeau
**transverse**, en une seule opération qui crée aussi la valeur de liste (S2).

Acceptation :
- Créer `preprod` le 1er mars : la valeur n'est pas proposée pour une affectation au
  15 février (REQ-003-5).
- Clore `sandbox` alors qu'un nœud, une cellule ou un garde-fou la cite : refusé, avec
  la liste (D33, REQ-003-6).

#### REQ-010-3 — Machine : environnement déclaré
Où le module Machines (009) est présent, le module doit déclarer
`policy.declared_environment` : liste fermée partageant les valeurs de
`policy.environment`, ensemble, non héritable, portée par les machines ; elle n'entre
dans aucun verdict (S3, D22).

#### REQ-010-4 — Types de flux
Le module doit permettre de créer, modifier et clore un type de flux (clé, libellé,
description) ; la clôture est refusée tant qu'un service ou une cellule le cite (S4).

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

### C. Garde-fous et matrice

#### REQ-010-8 — Garde-fou
Le module doit permettre de déclarer un garde-fou : un sélecteur (ensemble de services
**ou** type de flux), un environnement source et un environnement destination
facultatifs, un motif non vide (S6, D28).

Acceptation :
- « Jamais d'`admin` de `dev` vers `prod` » : type `admin`, source `dev`, destination
  `prod`.
- « `telnet` interdit » : services `{telnet}`, sans environnement.

#### REQ-010-9 — Cellule de matrice
Le module doit permettre d'ajouter et de retirer des cellules d'autorisation (type de
flux, environnement source, environnement destination), orientées, uniques par période
(S7, D28).

Acceptation :
- `(admin, bastion, prod)` n'autorise pas `(admin, prod, bastion)`.
- Une seconde cellule identique sur une période qui recouvre la première : refusée.

#### REQ-010-10 — Refus par défaut
Le module doit traiter toute combinaison sans cellule comme un refus (D28).

### D. Verdicts

#### REQ-010-11 — Évaluation
Quand on demande le verdict d'une paire d'adresses (A, B) pour un service S à
(`valid_at`, `as_of`), le module doit, dans cet ordre (S8) :
1. refuser avec le motif `unclassified` si A ou B est non classifié ;
2. refuser si un garde-fou s'applique à S et à au moins une paire (env de A, env de B),
   en citant le garde-fou ;
3. autoriser si une cellule (type de S, env de A, env de B) existe pour au moins une
   paire, en citant la cellule et la paire (D25) ;
4. sinon refuser avec le motif `no_cell`.

Acceptation :
- A `{prod, global}`, B `{dev}`, cellule `(admin, global, dev)` seule : `ssh` autorisé,
  paire `(global, dev)` citée.
- Même cas plus garde-fou `(admin, prod, dev)` : refusé, garde-fou cité (le refus
  l'emporte même si une autre paire est autorisée).
- B non classifié : refusé `unclassified`, quelle que soit la matrice.

#### REQ-010-12 — Explication
Le module doit joindre à chaque verdict les ensembles d'environnements résolus de A et
de B avec leur explication (origine, chaîne, valeur remplacée, REQ-003-14), l'élément
décisif et les paires examinées.

#### REQ-010-13 — Extrémités
Le module doit accepter comme extrémité une adresse, un préfixe (évalué sur ses propres
environnements) et, où le module Machines est présent, une interface ou une machine,
développées en leurs adresses à `valid_at` ; il doit rendre un verdict par paire
d'adresses (D22).

Acceptation :
- Machine A (2 adresses) vers machine B (3 adresses), service `ssh` : 6 verdicts.
- Une machine sans adresse à la date : réponse vide avec le motif `no_address`.

#### REQ-010-14 — Objet de la requête
Le module doit accepter comme objet de la requête : un service ; un couple
transport/port, évalué pour chaque service qui le couvre ; ou un type de flux, évalué
pour chacun de ses services (S9, D29).

Acceptation :
- `tcp/443` avec `https` (`application`) et `https-admin` (`admin`) : deux verdicts.
- Couple qu'aucun service ne couvre : `denied`, motif `unknown_service`.

#### REQ-010-15 — Synthèse
Le module doit accompagner les verdicts d'une synthèse : `allowed` si tous les verdicts
autorisent, `denied` si aucun, `partial` sinon, avec les décomptes.

#### REQ-010-16 — Verdicts datés
Le module doit évaluer environnements, garde-fous, cellules et services tels qu'ils
étaient à (`valid_at`, `as_of`) (S16, D10).

Acceptation :
- Cellule ajoutée le 1er avril : un verdict au 1er mars est `denied`, au 2 avril
  `allowed` ; avec `as_of` antérieur à l'ajout, `denied` à toute date.

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
Quand un changeset touche une cellule, un garde-fou, un service, un type de flux ou un
attribut d'environnement, le module doit calculer, en simulation comme à l'application,
la liste des triplets (environnement source, environnement destination, service) dont
le verdict de principe bascule, avec l'élément décisif avant et après, décompte d'abord
et détail paginé (S11, D25).

Acceptation :
- Ajouter la cellule `(admin, global, dev)` avec 3 services `admin` : 3 triplets
  `denied → allowed`.
- Ajouter un garde-fou `(admin, prod, dev)` qui masque une cellule : les triplets
  concernés `allowed → denied`, garde-fou cité.

#### REQ-010-19 — Différentiel des nœuds
Le module doit laisser les changements d'environnement des nœuds au différentiel de
propriétés de la 003 (REQ-003-19), sans le dupliquer ; leurs effets sur des flux
déclarés relèvent de la 011.

#### REQ-010-20 — Déclencheurs d'approbation
Le module doit fournir aux politiques d'approbation (REQ-004-40) les déclencheurs
`policy.opens` (au moins N triplets `denied → allowed`) et `policy.closes` (au moins N
`allowed → denied`), sans qu'aucun ne s'applique par défaut (S12, REQ-004-39).

### G. Inférence (réservée)

Section réservée aux exigences de l'inférence (D23, D24), à écrire après la session
dédiée ([`inference-questions.md`](inference-questions.md)). Identifiants réservés :
REQ-010-40 à REQ-010-59.

### H. Classification obligatoire et audit

#### REQ-010-21 — Obligation par domaine
Le module doit permettre à un administrateur de rendre `policy.environment` obligatoire
sur un domaine, et de lever l'obligation, par changeset et sans redéploiement ; l'ajout
est refusé, avec la liste des nœuds, tant qu'un nœud du domaine est non classifié (S13,
D27, REQ-003-6, 16).

Acceptation :
- Domaine D avec 12 adresses non classifiées : rendre l'obligation est refusé avec les
  12 adresses ; après leur classification dans le même changeset : accepté.
- D obligatoire : créer un réseau sans environnement résolu est refusé (REQ-003-16).

#### REQ-010-22 — Non classifié signalé
Tant qu'un nœud d'un domaine sans obligation est non classifié, le module doit le
compter dans le rapport d'audit (REQ-010-23) ; ses verdicts sont des refus
(REQ-010-11).

#### REQ-010-23 — Rapport d'audit
Le module doit produire, à la demande et par une tâche de fond, un rapport daté qui
liste : nœuds non classifiés par domaine ; nœuds dont l'ensemble résolu contient un
environnement transverse, avec le nœud d'origine ; environnements cités par aucune
cellule ; cellules dont un environnement n'est porté par aucun nœud ; cellules
entièrement masquées par un garde-fou ; anomalies de machines (S14, D25, D27).

Acceptation :
- Le rapport donne un décompte par rubrique et un détail paginé, consultable par l'API
  et exportable en JSON.

### I. Droits, temps, API

#### REQ-010-24 — Permissions
Le module doit déclarer les permissions `policy.environment.admin`,
`policy.flow_type.admin`, `policy.service.admin`, `policy.matrix.admin`,
`policy.guardrail.admin` (globales), `policy.read`, `policy.verdict.read`, et la
permission d'écriture de `policy.environment` portée par le périmètre du nœud (S15,
REQ-003-17).

#### REQ-010-25 — Verdict et visibilité
Si l'appelant n'a pas le droit de lire l'une des extrémités, alors le module doit
répondre comme pour un objet absent (404), sans verdict (S15, REQ-006-20).

#### REQ-010-26 — Versionnement
Le module doit écrire environnements, types de flux, services, garde-fous et cellules
par changeset et les conserver en bitemporel ; ils doivent être lisibles à toute date
(S16, D10, D11, REQ-004-17).

#### REQ-010-27 — API
Le module doit exposer ses collections sous `/v1/policy/` (`environments`, `flow-types`,
`services`, `guardrails`, `matrix-cells`), la requête de conformité par
`QUERY /v1/policy/verdicts` et `GET /v1/policy/verdicts` (paramètres), et les rapports
sous `/v1/policy/reports`, de manière additive (S19, 006/H12).

### J. Performance

#### REQ-010-28 — Budgets
Le module doit respecter (p95 serveur, jeu de 10⁶ objets de REQ-002-45 avec 50
environnements, 20 types de flux, 1 000 services, 10⁴ cellules, 500 garde-fous) :
verdict d'une paire d'adresses pour un service < 50 ms ; machine × machine (16 × 16
adresses) pour un type de flux de 10 services < 300 ms ; port brut couvert par 20
services < 100 ms ; différentiel de politique d'un changeset de matrice ou de
garde-fous < 1 s ; rapport d'audit < 10 min (S20, D14).

## Non-objectifs

- Les règles d'inférence (S18) : session dédiée.
- Le registre de flux déclarés et leur réévaluation : spec 011.
- L'atteignabilité effective (routes, NAT, pare-feu) : étape 2 de D20.
- Les services exposés par une machine (D29, « plus tard »).
- Les extrémités hors du référentiel (Internet) : à modéliser comme préfixes d'un
  domaine dédié.
- L'interface Web : plugin de première partie `web/plugins/policy/` (007/W6), à
  spécifier avec la 007.

## Questions ouvertes

- [ ] Règles d'inférence : [`inference-questions.md`](inference-questions.md).
  (utilisateur, session dédiée)
- [ ] Q-ouv-3 : critères de l'existant, nécessaires aux règles par défaut seulement.
  (utilisateur)
- [ ] S21 : lecture de « règle » dans D30 (matrice et garde-fous, pas l'inférence).
  (utilisateur)
- [ ] Budgets de REQ-010-28 : à confirmer par un prototype sur le jeu de 10⁶ objets.
  (Claude, tâche 010-1)

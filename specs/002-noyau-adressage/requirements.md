---
status: approved
updated: 2026-10-13
---

# 002 — Noyau d'adressage : exigences

## Résumé

Définit les objets du noyau de kartenn et leurs règles de validité : les ancres
(entités, sites), les domaines d'adressage, les préfixes (conteneurs et réseaux), les
plages et les adresses, pour IPv4 et IPv6, au travers d'un contrat de famille d'adresses
extensible. C'est le socle sur lequel s'appuient les propriétés héritables (003), les
changesets et le bitemporel (004), le RBAC (005), l'API (006) et tous les modules.

Décisions : [`decisions.md`](decisions.md) (A1–A9) et
[`../001-cadrage/decisions.md`](../001-cadrage/decisions.md) (D1–D37). Sources :
[`docs/reference/`](../../docs/reference/README.md).

### Conventions

- Forme EARS en français : « kartenn doit… » (permanent), « Quand…, kartenn doit… »
  (événement), « Tant que…, kartenn doit… » (état), « Si…, alors kartenn doit… » (cas
  indésirable), « Là où…, kartenn doit… » (option).
- **Écriture** : toute création, modification ou clôture d'objet, soumise dans un
  changeset (D11, spec 004). Une écriture **refusée** l'est pour tout le changeset, avec
  une erreur qui cite l'identifiant de la règle violée et les objets en cause.
- **Période** : chaque règle s'évalue sur la période de validité applicative (D10) :
  « deux objets en conflit » veut dire « en conflit sur une période commune ».
- **Erreur** et **avertissement** : niveaux de sévérité de A4.

## Exigences

### A. Familles d'adresses et représentation

#### REQ-002-1 — Familles prises en charge
kartenn doit prendre en charge les familles IPv4 et IPv6 au travers d'un contrat de
famille qui fournit : largeur en bits, analyse stricte, forme canonique, arithmétique,
relations d'inclusion et registre à usage spécial.

Acceptation :
- Les deux familles passent la même batterie de tests de contrat (tests de propriétés
  communs).
- Aucun composant hors des implémentations de familles ne suppose une largeur de 32 ou
  128 bits (vérifié en revue et par une famille de test fictive de 64 bits dans les tests).

#### REQ-002-2 — Analyse stricte IPv4
Si une adresse IPv4 n'est pas en notation décimale pointée stricte (quatre nombres de 0
à 255, sans zéro de tête), alors kartenn doit la refuser.

Acceptation :
- `192.0.2.1` accepté ; `010.0.0.1`, `10.1`, `0x0a.0.0.1`, `10.0.258`, `256.0.0.1`,
  `192.0.2.1 ` (espace) refusés, avec un message qui cite la forme attendue.

#### REQ-002-3 — Analyse IPv6
kartenn doit accepter toute forme textuelle légitime d'une adresse IPv6 selon
RFC 4291 §2.2 (RFC 5952 §4).

Acceptation :
- `2001:DB8::1`, `2001:0db8:0000:0000:0000:0000:0000:0001`, `::ffff:192.0.2.1` (puis
  REQ-002-31) et `0:0:0:0:0:0:0:1` sont analysés ; `2001:db8::1::2`, `2001:db8:::1`,
  `12345::1` sont refusés.

#### REQ-002-4 — Zones refusées
Si une adresse ou un préfixe contient un identifiant de zone (`%…`), alors kartenn doit
le refuser (RFC 4007 §6 : la zone n'a de sens que sur un nœud).

Acceptation :
- `fe80::1%eth0` et `fe80::%2/64` refusés avec un message qui l'explique.

#### REQ-002-5 — Préfixes alignés
Si un préfixe a des bits d'hôte non nuls ou n'a pas de longueur explicite, alors kartenn
doit le refuser en proposant le préfixe aligné.

Acceptation :
- `192.168.1.5/24` refusé, suggestion `192.168.1.0/24` ; `10` ou `10/8` (abréviation
  héritée des classes) refusé ; `2001:db8::/32` accepté ; longueur hors de 0 à la largeur
  de la famille refusée.

#### REQ-002-6 — Forme canonique en sortie
kartenn doit restituer toute adresse et tout préfixe sous forme canonique, quel que soit
le canal : décimal pointé pour IPv4, RFC 5952 pour IPv6.

Acceptation :
- `2001:0DB8:0:0:1:0:0:1` saisi est restitué `2001:db8::1:0:0:1` ; `2001:db8:0:1:1:1:1:1`
  reste sans `::` (un seul groupe nul) ; les hexadécimaux sont en minuscules.
- Test de propriété : analyse(canonique(x)) = x pour toute valeur x.

#### REQ-002-7 — Égalité sur la valeur
kartenn doit comparer adresses et préfixes sur leur valeur (famille, bits, longueur),
jamais sur leur texte.

Acceptation :
- Créer `2001:DB8::/32` puis `2001:0db8::/32` dans le même domaine est refusé comme doublon.

### B. Entités et sites

#### REQ-002-8 — Arbre des entités
kartenn doit gérer les entités sous forme d'arbre (une entité a au plus un parent) et
refuser toute écriture qui créerait un cycle.

Acceptation :
- Rattacher une entité à l'un de ses descendants est refusé.

#### REQ-002-9 — Arbre des sites
kartenn doit gérer les sites sous forme d'arbre (région > site > bâtiment… sans niveaux
imposés) et refuser toute écriture qui créerait un cycle.

Acceptation :
- Même critère que REQ-002-8, pour les sites.

#### REQ-002-10 — Clôture d'une ancre utilisée
Si la clôture d'une entité ou d'un site laisserait des objets qui le référencent
(enfant, domaine, préfixe, réseau) sans référence valide sur une période, alors kartenn
doit refuser l'écriture en listant ces objets (D33).

Acceptation :
- Clore un site auquel un réseau est rattaché est refusé ; clore d'abord le rattachement
  puis le site est accepté.

### C. Domaines d'adressage

#### REQ-002-11 — Domaine et propriétaire
kartenn doit exiger que chaque domaine d'adressage ait un slug (REQ-002-47) et une entité
propriétaire.

Acceptation :
- Créer un domaine sans entité est refusé.

#### REQ-002-12 — Appartenance au domaine
kartenn doit exiger que chaque préfixe, plage et adresse appartienne à exactement un
domaine, et ne doit pas permettre de changer ce domaine après création.

Acceptation :
- Modifier le domaine d'un préfixe existant est refusé ; le déplacement se fait par
  clôture puis création.

### D. Préfixes

#### REQ-002-13 — Nature du préfixe
kartenn doit exiger que chaque préfixe soit soit un **conteneur**, soit un **réseau** (A2).

Acceptation :
- Un préfixe sans nature est refusé.

#### REQ-002-14 — Arbre déduit
kartenn doit déterminer le parent d'un préfixe, d'une plage ou d'une adresse comme le plus
petit préfixe de son domaine qui le contient strictement, à l'instant considéré (D34).

Acceptation :
- Avec `10.0.0.0/8` et `10.1.0.0/16`, le parent de `10.1.2.0/24` est `10.1.0.0/16` ;
  après création de `10.1.2.0/23`, il devient `10.1.2.0/23`.

#### REQ-002-15 — Pas de doublon
Si deux préfixes d'un même domaine ont la même valeur sur une période commune, alors
kartenn doit refuser l'écriture (règle d'erreur).

Acceptation :
- `10.0.0.0/8` déjà valide de janvier à juin : le même préfixe de mars à décembre est
  refusé ; de juillet à décembre, il est accepté.

#### REQ-002-16 — Un réseau est une feuille
Si un réseau contiendrait un autre préfixe (réseau ou conteneur), alors kartenn doit
refuser l'écriture.

Acceptation :
- Réseau `10.1.2.0/24` existant : créer `10.1.2.0/25` (quelle que soit sa nature) est
  refusé ; créer un conteneur `10.1.0.0/16` au-dessus est accepté.

#### REQ-002-17 — Changement de nature
Quand la nature d'un préfixe change, kartenn doit revalider REQ-002-16 et REQ-002-21.

Acceptation :
- Un conteneur qui contient des préfixes ne peut pas devenir réseau ; un réseau qui
  contient des adresses ou des plages ne peut pas devenir conteneur.

#### REQ-002-18 — Redimensionnement
Quand la valeur d'un préfixe est modifiée, kartenn doit conserver son identifiant et
revalider toutes les règles pour le préfixe et pour les objets dont le parent change.

Acceptation :
- Passer un réseau `10.1.2.0/24` à `10.1.2.0/23` est accepté si `10.1.3.0/24` n'existe
  pas ; le réduire à `/25` est refusé si une adresse `10.1.2.200` y est déclarée.

#### REQ-002-19 — Point à point
Là où un réseau est marqué **point à point**, kartenn doit l'accepter seulement en /31 ou
/32 (IPv4) et /127 ou /128 (IPv6), et y autoriser toutes les adresses comme adresses
d'hôte (RFC 3021, RFC 6164).

Acceptation :
- Réseau `/31` point à point : `192.0.2.0` et `192.0.2.1` attribuables ; marquer
  point à point un `/30` est refusé.

#### REQ-002-20 — Rattachement à un site
Là où un réseau est rattaché à un site, kartenn doit l'accepter ; si un conteneur est
rattaché à un site, alors kartenn doit refuser l'écriture (A7).

### E. Plages

#### REQ-002-21 — Plage dans un réseau
kartenn doit exiger que chaque plage (adresse de début, adresse de fin, même famille,
début ≤ fin) soit entièrement contenue dans un seul réseau de son domaine.

Acceptation :
- `10.1.2.10–10.1.2.50` dans le réseau `10.1.2.0/24` accepté ; une plage à cheval sur
  deux réseaux est refusée ; une plage hors de tout réseau est refusée.

#### REQ-002-22 — Plages disjointes
Si deux plages d'un même réseau se chevauchent sur une période commune, alors kartenn
doit refuser l'écriture (D34).

Acceptation :
- `.10–.50` existante : `.40–.60` refusée ; `.51–.60` acceptée.

#### REQ-002-23 — Bornes réservées
Si une plage contient l'adresse de réseau ou de diffusion d'un réseau IPv4 de longueur
≤ 30, ou l'IID nul d'un réseau IPv6 non point à point, alors kartenn doit refuser
l'écriture.

Acceptation :
- Dans `10.1.2.0/24`, `.0–.20` et `.200–.255` sont refusées.

#### REQ-002-24 — Usage de plage
kartenn doit exiger qu'une plage porte un usage parmi DHCP, réservée et statique (A5),
et appliquer les règles d'adresses de REQ-002-28.

### F. Adresses

#### REQ-002-25 — Adresse dans un réseau
kartenn doit exiger que chaque adresse appartienne à exactement un réseau de son domaine.

Acceptation :
- `10.1.2.7` sans réseau qui la contienne est refusée ; avec seulement le conteneur
  `10.0.0.0/8`, elle est refusée.

#### REQ-002-26 — Unicité de l'adresse
Si deux adresses d'un même domaine ont la même valeur sur une période commune, alors
kartenn doit refuser l'écriture.

#### REQ-002-27 — Usage d'adresse
kartenn doit exiger qu'une adresse porte un usage parmi hôte, passerelle, VIP, anycast et
réservée (A5, A11).

#### REQ-002-28 — Adresses et plages
Si une adresse est déclarée dans une plage DHCP sans l'usage « réservée », ou dans une
plage d'usage « réservée », alors kartenn doit refuser l'écriture ; dans une plage
statique, toute adresse est permise (A8).

Acceptation :
- Plage DHCP `.100–.199` : `.150` en hôte refusée, `.150` en réservée acceptée.
- Plage réservée `.1–.9` : `.5` refusée, quel que soit son usage.

#### REQ-002-29 — Adresses interdites (erreur)
Si une adresse attribuée est l'adresse de réseau ou de diffusion d'un réseau IPv4 de
longueur ≤ 30, l'IID nul d'un réseau IPv6 non point à point, ou appartient à 0.0.0.0/8,
127.0.0.0/8, 240.0.0.0/4, 255.255.255.255/32, `::/128`, `::1/128` ou à un bloc
multicast (224.0.0.0/4, ff00::/8), alors kartenn doit refuser l'écriture (A4).

Acceptation :
- `10.1.2.0` et `10.1.2.255` dans `10.1.2.0/24` refusées ; `2001:db8:1::` dans
  `2001:db8:1::/64` refusée ; `2001:db8:1::` dans un `/127` point à point acceptée.

#### ~~REQ-002-30 — Nom DNS~~
Retirée (A10) : le nom DNS est une propriété d'interprétation, hors du noyau. Elle sera
déclarée comme propriété typée « FQDN » (spec 003) par le module qui en a besoin, avec
la même validation (RFC 1123 §2.1, A-label, minuscules sans point final).

#### REQ-002-31 — Pas d'IPv4-mapped
Si un préfixe ou une adresse IPv6 appartient à `::ffff:0:0/96`, alors kartenn doit
refuser l'écriture en indiquant l'adresse IPv4 équivalente (A4).

### G. Unicité de l'espace public

#### REQ-002-32 — Public unique entre domaines
Si deux réseaux, plages ou adresses de domaines différents se chevauchent sur une période
commune dans l'espace public (aucune entrée du registre à usage spécial, ou entrée
« Globally Reachable »), alors kartenn doit refuser l'écriture (A3), sauf entre adresses
d'usage « anycast » (A11).

Acceptation :
- `198.51.100.0/24` est de la documentation, donc non concerné ; un même bloc public
  réseau dans les domaines A et B est refusé ; un même bloc RFC 1918 dans A et B est accepté.

#### REQ-002-46 — Anycast entre domaines
Quand une adresse publique d'usage « anycast » est présente dans plusieurs domaines sur
une période commune, kartenn doit accepter l'écriture si toutes ces occurrences sont
d'usage « anycast », et renvoyer un avertissement qui liste les autres domaines (A11).

Acceptation :
- Soit P une adresse publique : P anycast dans A et anycast dans B → accepté avec
  avertissement ; P anycast dans A et hôte dans B → refusé ; un **réseau** public dupliqué reste refusé même s'il ne contient
  que des adresses anycast.

### H. Classification et anomalies

#### REQ-002-33 — Classification calculée
kartenn doit associer à chaque préfixe, plage et adresse l'entrée la plus spécifique des
registres IANA à usage spécial qui la contient (nom, RFC, drapeaux Source, Destination,
Forwardable, Globally Reachable, Reserved-by-Protocol), en lecture seule.

Acceptation :
- `192.0.0.9` → « Port Control Protocol Anycast » (et non « IETF Protocol Assignments ») ;
  `10.1.2.3` → « Private-Use » ; une adresse publique → aucune entrée.

#### REQ-002-34 — Registres versionnés
kartenn doit embarquer les registres à usage spécial comme données versionnées portant
leur date de mise à jour IANA, et les exposer en lecture.

Acceptation :
- La version et la date du registre utilisé sont consultables ; mettre à jour le registre
  ne demande aucune modification de code.

#### REQ-002-35 — Avertissements
Quand une écriture produit un cas listé en avertissement par A4 (bloc de documentation,
100.64.0.0/10, réseau IPv6 plus long qu'un /64 hors point à point, ULA au Global ID non
aléatoire, adresse anycast présente dans plusieurs domaines), kartenn doit accepter
l'écriture et renvoyer une anomalie qui cite l'identifiant de la règle et l'objet.

#### REQ-002-36 — Anomalies consultables et acquittables
kartenn doit permettre de lister les anomalies en cours (par domaine, entité, règle) et
de les acquitter avec un motif ; une anomalie acquittée n'est plus remontée pour cet
objet tant qu'il n'est pas modifié.

#### REQ-002-37 — Identifiants de règle stables
kartenn doit donner à chaque règle de validation un identifiant stable, documenté avec
sa sévérité et sa source (RFC ou décision).

### I. Propriété

#### REQ-002-38 — Propriétaire résolu
kartenn doit résoudre l'entité propriétaire de chaque préfixe, plage et adresse :
propriétaire explicite du préfixe, sinon celui du préfixe parent le plus proche qui en
a un, sinon celui du domaine (A7) ; et restituer la chaîne de résolution.

Acceptation :
- Domaine D (entité E1), conteneur `10.0.0.0/8` sans propriétaire, réseau `10.5.0.0/24`
  propriétaire E2 : l'adresse `10.5.0.7` → E2, via `10.5.0.0/24` ; l'adresse d'un autre
  réseau sans propriétaire → E1, via le domaine.

### J. Identifiants

#### REQ-002-39 — UUID immuable
kartenn doit attribuer à chaque objet un UUID (v7) immuable.

#### REQ-002-40 — Clés naturelles
kartenn doit permettre de désigner un objet par sa clé naturelle : slug (entité, site,
domaine), (domaine, préfixe), (domaine, début–fin) pour une plage, (domaine, adresse).

#### REQ-002-41 — Identifiants externes
kartenn doit conserver des correspondances (source, identifiant externe) → UUID, uniques
par source.

Acceptation :
- Deux objets ne peuvent pas porter le même identifiant externe pour une même source.

#### REQ-002-47 — Slug, identifiant technique
kartenn doit exiger pour chaque entité, site et domaine un slug : identifiant technique
(pas un nom affiché) de 1 à 63 caractères `[a-z0-9-]`, ne commençant ni ne finissant par
un tiret, unique par type d'objet à tout instant. Un slug peut être modifié ; l'identité
reste l'UUID (A9, A10).

Acceptation :
- `siege-paris` accepté ; `Siège`, `-x`, `a_b` refusés ; deux domaines `prod` valides sur
  une même période refusés ; renommer `prod` en `prod-eu` conserve l'UUID et l'historique.

### K. Périmètre du noyau

#### REQ-002-48 — Noyau sans attribut d'interprétation
kartenn ne doit porter dans les objets du noyau que les attributs dont dépend un invariant
du noyau ou une ancre du RBAC (A10) ; tout attribut descriptif (nom affiché, FQDN,
description, commentaire, coordonnées…) doit être une propriété (spec 003).

Acceptation :
- Revue de conception : chaque attribut du noyau est justifié par l'exigence qui
  l'utilise (table de traçabilité dans `design.md`).
- Les objets du noyau sont : entité (slug, parent), site (slug, parent), domaine (slug,
  entité), préfixe (domaine, valeur, nature, point à point, entité, site), plage (domaine,
  bornes, usage), adresse (domaine, valeur, usage), plus UUID et période de validité.

### L. Lecture et performance

#### REQ-002-42 — Navigation dans l'arbre
kartenn doit restituer, pour un préfixe, ses enfants directs et ses ancêtres à un instant
donné.

#### REQ-002-43 — Occupation
kartenn doit calculer l'occupation d'un préfixe : pour un réseau, adresses attribuables
selon la famille et la longueur (REQ-002-19, REQ-002-29), adresses déclarées, plages DHCP
et réservées comptées en entier ; pour un conteneur, part de l'espace couverte par ses
enfants.

Acceptation :
- `/24` : 254 attribuables ; `/31` point à point : 2 ; `/32` : 1 ; IPv6 `/64` :
  2⁶⁴ − 1 (IID nul exclu) ; conteneur `/16` avec deux `/24` : 2/256 couvert.

#### REQ-002-44 — Prochain préfixe ou adresse libre
Quand on demande un préfixe libre d'une longueur donnée dans un conteneur, ou une adresse
libre dans un réseau, kartenn doit proposer le premier disponible en respectant toutes
les règles, sans l'attribuer.

#### REQ-002-45 — Budgets
kartenn doit respecter les budgets de D14 sur un jeu de 10⁶ objets : lecture d'un objet
< 50 ms, page filtrée < 150 ms, sous-arbre et occupation < 200 ms, validation et
application d'un changeset unitaire < 200 ms (p95 serveur).

Acceptation :
- Banc d'essai reproductible en CI sur un jeu synthétique (10⁵ réseaux, 10⁶ adresses,
  arbres de profondeur ≥ 6, IPv4 et IPv6, plusieurs domaines dont des plages RFC 1918
  dupliquées).

## Non-objectifs

- Familles autres qu'IPv4 et IPv6 (IPv8 : seul le contrat doit le permettre, REQ-002-1).
- Changesets, bitemporel et API (specs 004 et 006) : la 002 n'en décrit que les règles de
  validité.
- Propriétés (typées, héritables ou non) et environnements (003, v0.2), y compris tous
  les attributs descriptifs : noms affichés, FQDN, descriptions (A10).
- VLAN, machines, interfaces (008, 009), DNS (module futur), DHCP (module futur),
  passerelles, routes et NAT (v0.3).
- Masques non contigus et notation `adresse masque`.
- Découverte ou synchronisation automatique.

## Questions ouvertes

- [ ] Le propriétaire hérité (REQ-002-38) réutilise-t-il le mécanisme de la spec 003 ?
  (conception 002/003, Claude)
- [ ] Critère précis de « Global ID ULA manifestement non aléatoire » (REQ-002-35) :
  zéros, motifs répétés, séquence ? (conception, Claude)
- [ ] Dépendance : la spec 003 doit offrir des propriétés **typées** (liste fermée, texte,
  FQDN…), validées par le module qui les déclare, héritables ou non (A10). À vérifier
  quand la 003 sera rédigée. (Claude)
- [x] ~~Exception anycast~~ : tranchée par A11 (usage « anycast » sur l'adresse).
- [x] ~~Attributs des sites~~ : tranché par A10 (slug et parent seulement).
- [x] ~~Note de référence sur les noms DNS~~ : sans objet pour la 002 (REQ-002-30
  retirée) ; à rédiger pour la spec 003.

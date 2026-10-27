---
status: implemented
updated: 2026-10-27
---

# 009 — Machines et interfaces : exigences

## Résumé

Second module métier de la v0.1 (D18, D31), construit sur le contrat de module
(ADR 0026) implémenté par la 008. Il décrit les **machines** (physiques, virtuelles,
conteneurs, appliances), leurs **interfaces logiques** (MAC, adresses, VLAN, rôle), et
l'**hébergement** d'une machine virtuelle ou d'un conteneur sur un hôte ou un cluster.
Il fournit à la politique de flux (010) le lien entre une machine et ses adresses, et le
rôle d'interface de D22 ; il ne place pas les machines dans l'arbre d'adressage.

Décisions : [`decisions.md`](decisions.md) (M1–M16) ; D12, D18, D22, D31, D33, D36
([`../001-cadrage/decisions.md`](../001-cadrage/decisions.md)) ; ADR 0026 ; spec 008.
Référence : [`docs/reference/vlan-mac-ieee-802.md`](../../docs/reference/vlan-mac-ieee-802.md).

### Vocabulaire

- **Machine** : objet `machines.machine` de type `physical`, `virtual`, `container` ou
  `appliance`.
- **Cluster** : objet `machines.cluster`, groupe d'hôtes de virtualisation.
- **Hébergeur** : machine ou cluster sur lequel tourne une machine `virtual` ou
  `container`.
- **Interface** : objet `machines.interface`, interface logique d'une machine.
- **Rattachement** : lien daté entre une interface et une adresse du noyau.
- Conventions EARS, « écriture », « période », sévérités : comme dans
  [`002/requirements.md`](../002-noyau-adressage/requirements.md#conventions) ;
  « objet de module », « ancre » : comme dans [`008/requirements.md`](../008-vlan/requirements.md).

## Exigences

### A. Module

#### REQ-009-1 — Déclaration du module
kartenn doit fournir le module `machines`, dépendant du module `vlan`, avec les types
d'objets `machines.cluster`, `machines.machine` et `machines.interface` (parent : sa
machine), et refuser de démarrer sans le module `vlan` (REQ-008-1).

Acceptation :
- `GET /v1/capabilities` liste `vlan` et `machines` ; une build avec `machines` sans
  `vlan` ne démarre pas (message qui nomme la dépendance).

### B. Clusters et machines

#### REQ-009-2 — Cluster
kartenn doit gérer des clusters identifiés par un slug unique à tout instant
(REQ-002-47), portant `core.owner` (obligatoire) et `core.site` (facultatif).

Acceptation :
- Deux clusters vivants de même slug : refus ; renommage par changeset : accepté.

#### REQ-009-3 — Machine
kartenn doit gérer des machines munies d'un type (`physical`, `virtual`, `container`,
`appliance`), d'un nom d'hôte, de `core.owner` (obligatoire) et de `core.site`
(facultatif).

Acceptation :
- Machine sans propriétaire résolu : refus (obligation de la 003).
- Type inconnu : refus (400 à la structure).

#### REQ-009-4 — Nom d'une machine
kartenn doit n'accepter comme nom de machine qu'une suite d'étiquettes LDH séparées par
des points (1 à 63 caractères par étiquette, sans `-` en tête ni en fin, 253 au total,
RFC 1123 §2.1), le conserver en minuscules, et refuser deux machines vivantes de même
nom et de même propriétaire résolu sur des périodes qui se chevauchent
(`machines.name.duplicate`).

Acceptation :
- `SRV01` est conservé `srv01` ; `-srv`, `srv_01`, une étiquette de 64 caractères :
  refus (`machines.name.invalid`).
- `srv01` dans les entités E et F : accepté ; deux fois dans E : refus.
- Changer le propriétaire d'une machine vers une entité où son nom est pris : refus.

#### REQ-009-5 — Hébergement
kartenn doit permettre à une machine `virtual` d'être hébergée par une machine
`physical` ou par un cluster, à une machine `container` d'être hébergée par une machine
`physical` ou `virtual` ou par un cluster, à une machine `physical` d'être membre d'un
cluster, et refuser tout autre hébergement, une machine qui s'héberge elle-même ou un
cycle (`machines.host.invalid`).

Acceptation :
- VM sur un hôte physique, conteneur sur une VM, VM sur un cluster : acceptés.
- Appliance hébergée, VM sur une VM, conteneur A sur B et B sur A : refus.
- Changer le type d'une machine hébergée en `physical` : refus tant qu'elle a un
  hébergeur.

### C. Interfaces

#### REQ-009-6 — Interface
kartenn doit gérer des interfaces, chacune liée à une machine fixe, avec un nom de 1 à
64 caractères imprimables sans espace, unique par machine à tout instant
(`machines.interface.duplicate`), une MAC EUI-48 facultative et un état `enabled`.
La MAC s'accepte en hexadécimal avec `-`, `:` ou en trois groupes de quatre chiffres
séparés par `.`, se restitue en majuscules séparées par `-` (RFC 9542 §1.1), et est
refusée si son bit de groupe vaut 1 (`machines.mac.invalid`).

Acceptation :
- `00:00:5e:00:53:01` et `0000.5e00.5301` sont restituées `00-00-5E-00-53-01`.
- `01-00-5E-00-00-01` (groupe) : refus ; `02-00-5E-00-53-01` (locale) : accepté.
- Changer la machine d'une interface : refus (structure) ; deux `eth0` sur une machine :
  refus.

#### REQ-009-7 — Ancres d'une interface
kartenn doit faire hériter une interface du propriétaire et du site résolus de sa
machine.

Acceptation :
- Changer le propriétaire d'une machine change, dans le différentiel, le périmètre de
  ses interfaces ; un lecteur de l'entité voit la machine et ses interfaces.

#### REQ-009-8 — Rôle d'interface
kartenn doit fournir la propriété `machines.interface_role`, liste fermée simple,
portée par les interfaces, non héritée, avec les valeurs `administration`, `backup`,
`monitoring`, `storage`, `cluster`, `service`, extensible par un administrateur.

Acceptation :
- Poser `administration` sur `eth1` ; l'explication de la propriété la montre ; une
  valeur hors liste : refus.

#### REQ-009-26 — Média d'interface
kartenn doit fournir la propriété `machines.interface_media`, liste fermée simple,
portée par les interfaces, non héritée, facultative, avec les valeurs `ethernet`
(cuivre), `sfp` (module enfichable, fibre ou cuivre), `wifi`, `aggregate` (agrégat de
liens), `virtual` (interface d'une machine virtuelle ou d'un pont) et `loopback`,
extensible par un administrateur (demande de l'utilisateur, relecture de la maquette
E11 du 2026-10-26, M17).

Acceptation :
- Poser `sfp` sur `eth2` ; la fiche de l'interface et l'explication de la propriété la
  montrent ; une valeur hors liste : refus ; une interface sans média reste valide.
- Filtrer les interfaces par média.

### D. Adresses et VLAN

#### REQ-009-9 — Rattacher une adresse
kartenn doit permettre de rattacher une adresse vivante du noyau à une interface sur une
période, et de l'en détacher, par changeset.

Acceptation :
- `eth0` reçoit `192.0.2.10` (domaine D1) et `2001:db8::10` (domaine D2) ; l'historique
  de l'interface montre les rattachements datés.

#### REQ-009-10 — Unicité d'un rattachement
kartenn doit refuser qu'une adresse soit rattachée à plus d'une interface sur des
périodes qui se chevauchent (`machines.address.taken`), sauf si l'usage de l'adresse est
`vip` ou `anycast`.

Acceptation :
- Deux rattachements concurrents de la même adresse `host` : l'un réussit, l'autre est
  refusé avec l'interface qui la détient.
- Une adresse `vip` sur les interfaces de deux machines : accepté.
- Changer l'usage d'une adresse rattachée deux fois de `vip` à `host` : refus.

#### REQ-009-11 — Adresse principale
kartenn doit permettre de marquer au plus un rattachement principal par famille
d'adresses et par interface (`machines.address.primary`).

Acceptation :
- Deux IPv4 principales sur `eth0` : refus ; une IPv4 et une IPv6 principales : accepté.

#### REQ-009-12 — VLAN d'une interface
kartenn doit gérer le mode 802.1Q d'une interface : `none` sans VLAN, `access` avec
exactement un VLAN, `trunk` avec un ensemble non vide de VLAN étiquetés et un VLAN natif
facultatif hors de cet ensemble ; tout autre état est refusé (`machines.vlan.mode`).

Acceptation :
- `access` sans VLAN, `trunk` dont le natif est aussi étiqueté, `none` avec des VLAN :
  refus.
- Un VLAN clos à la date de la période : refus.

Amendée par la 026 : les VLAN cités sont des objets du noyau (`core.vlan`), lus par le
port du noyau sous `core.vlan.read` ; Machines ne dépend plus du module `vlan`, qui
disparaît, et se construit seul.

### E. Cohérence et suppression

#### REQ-009-13 — Avertissements
kartenn doit signaler en anomalie (avertissement) : une MAC universelle (bit local à 0)
présente sur deux interfaces vivantes (`machines.mac.duplicate`) ; une adresse rattachée
dont le réseau porte un VLAN (`vlan.vlan`) absent des VLAN d'une interface en mode
`access` ou `trunk` (`machines.address.vlan_mismatch`) ; une machine hébergée par une
machine d'un autre site résolu (`machines.host.site_mismatch`).

Acceptation :
- Chaque cas apparaît dans la simulation, puis en anomalie après application, et
  disparaît quand il est corrigé.

#### REQ-009-14 — Suppression
kartenn doit refuser, en listant les dépendants, la clôture d'une machine qui a des
interfaces vivantes, d'une interface qui a des rattachements, d'une adresse rattachée,
d'un VLAN utilisé par une interface, d'un hôte ou d'un cluster qui héberge ou regroupe
des machines ; et, avec la cascade explicite, inclure : la clôture des interfaces et des
rattachements, le retrait du VLAN de l'interface, la mise à nul de l'hébergeur ou du
cluster (D33). Les dépendants d'une machine ou d'une interface restent extensibles par
les modules qui dépendent de `machines` (ADR 0026 clause 12 ; besoin M6 : un flux de la
011 qui cite une machine).

Acceptation :
- Clore une machine en cascade : le différentiel montre ses interfaces et leurs
  rattachements clos ; ses adresses restent.
- Clore un cluster en cascade : ses VM n'ont plus d'hébergeur.

### F. Lectures, droits, interface, budgets

#### REQ-009-15 — Listes et lectures datées
kartenn doit lister clusters, machines et interfaces avec les conventions de la 006 :
machines par nom (exact ou préfixe), type, hébergeur, cluster, propriétaire, site ;
interfaces par machine, nom, MAC, adresse rattachée, VLAN.

Acceptation :
- `GET /v1/interfaces?address=<id>&valid_at=2026-01-01T00:00:00Z` rend l'interface qui
  portait l'adresse à cette date, avec sa machine.

#### REQ-009-16 — Lectures croisées
kartenn doit rendre, à une date donnée et filtrés par les droits de lecture : les
adresses d'une machine (par interface) ; les machines dont une adresse est dans un
réseau ; les machines hébergées par un hôte ou un cluster ; les membres d'un cluster.

Acceptation :
- Machines d'un /24 avec 200 machines : rendues par pages ; une machine d'une entité
  illisible n'apparaît pas.

#### REQ-009-22 — Port de lecture pour les modules dépendants
kartenn doit offrir aux modules qui dépendent de `machines`, dans la transaction d'un
changeset comme en lecture, à (`valid_at`, `as_of`) : les adresses d'une machine ou
d'une interface, chacune avec son interface et le rôle résolu de celle-ci
(`machines.interface_role`, lu par sa clé) ; et, à l'inverse, les interfaces et
machines d'une adresse (ADR 0026 clause 15 ; besoins M2, M4 de la 010).

Acceptation :
- Test d'un module factice dépendant : il lit, dans la transaction, les adresses et
  rôles d'une machine telle que le changeset en cours la laisse, et telle qu'elle était
  à un `as_of` passé.

#### REQ-009-23 — Section de différentiel des rattachements
kartenn doit publier, pour tout changeset qui rattache, détache une adresse ou change
l'adresse d'une interface (y compris par cascade, ou par la clôture d'une interface ou
d'une machine), une section de différentiel `machines.attachments` listant chaque
interface et machine dont les rattachements changent, l'adresse, l'avant, l'après et la
période (ADR 0026 clause 13 ; besoins M5, M7 de la 011).

Acceptation :
- Clore en cascade une machine à deux interfaces et trois adresses : la section liste
  les trois rattachements clos, avec leur machine ; elle figure dans la simulation et
  après application, et un module dépendant la lit avant ses propres contributions.

#### REQ-009-24 — Porteurs pour les autres modules
kartenn doit admettre les machines et les interfaces comme porteuses de propriétés
déclarées par d'autres modules ou par un administrateur, héritables ou non (besoin M3 de
la 010 : `policy.declared_environment` sur une machine).

Acceptation :
- Un module factice déclare une propriété non héritable portée par `machines.machine` ;
  elle s'affecte, s'explique et se filtre comme sur un préfixe.
- Une machine porte en même temps plusieurs propriétés déclarées par d'autres modules
  ou par un administrateur (sensibilité, criticité, application, rôle de conduit) :
  chacune s'affecte et s'explique indépendamment (besoin M8 de la 010).

#### REQ-009-25 — Valeurs des machines d'un lot d'adresses
kartenn doit offrir aux modules qui dépendent de `machines`, dans la transaction d'un
changeset comme en lecture, à (`valid_at`, `as_of`), pour un lot d'adresses et une liste
de clés de propriétés : pour chaque adresse, les machines rattachées (avec l'interface
qui porte l'adresse) et, pour chacune, les valeurs résolues de ces propriétés avec leur
explication, ou l'absence de valeur ; la composition avec la valeur héritée de l'arbre
d'adressage revient au module lecteur (besoins M8, M10 de la 010 ; 010/S27).

Acceptation :
- Lot de 512 adresses dont une VIP rattachée à deux machines aux sensibilités
  différentes et une adresse sans machine : la VIP rend les deux machines et leurs
  valeurs, l'adresse sans machine rend une liste vide, en une lecture.

#### REQ-009-17 — Arêtes du graphe
kartenn doit déclarer les arêtes `machines.has_address` (interface → adresse),
`machines.access`, `machines.tagged`, `machines.native` (interface → VLAN),
`machines.hosted_on` (machine → machine hôte), `machines.hosted_in` (machine → cluster
hôte), `machines.member_of` (machine → cluster) (REQ-008-11).

Acceptation :
- `GET /v1/graph/edges?object=<adresse>` rend l'arête vers l'interface qui la porte.

#### REQ-009-18 — Permissions
kartenn doit contrôler le module par `machines.cluster|machine|interface.read|write|
propose`, évaluées sur le périmètre hérité ; rattacher ou détacher une adresse exige
`machines.interface.write` sur l'interface et `core.address.read` sur l'adresse ; poser
un VLAN exige `vlan.read` sur le VLAN.

Acceptation :
- Un écrivain des machines de E sans droit sur l'adressage rattache une adresse qu'il
  lit ; une adresse qu'il ne lit pas : refus par la violation `core.object.forbidden`
  du changeset (422 ; 403 reste le refus d'une route entière, 008/V16).

#### REQ-009-19 — Imports
kartenn doit accepter les opérations du module dans les imports par morceaux de la 006,
y compris des machines, interfaces et rattachements créés et référencés par noms locaux
dans un même morceau.

Acceptation :
- Import de 10⁴ machines avec deux interfaces et deux adresses chacune : aboutit ;
  reprise sans doublon.

#### REQ-009-20 — Plugin d'interface
Où le shell web (007) est livré, kartenn doit fournir un plugin de première partie
`machines` : vues des machines, clusters et interfaces ; onglet « Machines » sur une
adresse et un réseau ; colonne machine dans la liste des adresses ; action « rattacher
à une interface » sur une adresse ; servi seulement si le module est actif.

Acceptation :
- Parcours Playwright : créer une machine, une interface, rattacher une adresse, poser
  un VLAN ; axe-core sans violation.

#### REQ-009-21 — Budgets
kartenn doit respecter les budgets de D14 sur le jeu de référence augmenté de 10⁵
machines (60 % virtuelles), 10³ clusters, 3·10⁵ interfaces et 3·10⁵ rattachements :
lecture d'une machine avec interfaces et adresses < 50 ms, page filtrée < 150 ms,
machines d'un réseau /24 < 150 ms, changeset unitaire < 200 ms (p95 serveur).

Acceptation :
- `bench-core` mesure ces budgets, bloquant en CI.

## Couverture des besoins de la 010 et de la 011

Besoins M1 à M10 (`specs/010-politique-flux/design.md`, `specs/011-registre-flux/design.md`,
§ « Exigences envers le noyau, le contrat de module et la 009 ») ; M8 à M10 viennent de
l'entretien sécurité de la 010 (2026-10-21, `besoin/ecarts.md`, où ils portaient les
numéros M7 à M9).

| # | Besoin | Exigences |
|---|---|---|
| M1 | machines et interfaces bitemporelles ; rattachement daté, multi-attachement VIP et anycast | REQ-009-3, 6, 9, 10 |
| M2 | rôle d'interface en liste fermée administrable, lisible par clé | REQ-009-8, 22 |
| M3 | machines et interfaces porteuses de propriétés d'autres modules | REQ-009-24 |
| M4 | adresses d'une machine ou interface avec interface et rôle, et l'inverse, à (`valid_at`, `as_of`) | REQ-009-15, 16, 22 |
| M5 | rattachements par opérations typées, visibles des contributions et règles des dépendants | REQ-009-9, 23 |
| M6 | dépendants des machines et interfaces extensibles | REQ-009-14 |
| M7 | section des interfaces et machines dont les rattachements changent, avec la période | REQ-009-23 |
| M8 | machines porteuses de plusieurs propriétés d'autres modules ou de l'administrateur, lues par l'adresse à la date | REQ-009-24, 25 |
| M9 | rôle de conduit lisible par clé | tenu par M8 : la 010 en fait un axe porté par la machine (010/S26), distinct du rôle d'interface (REQ-009-8) |
| M10 | défaut hérité pour un axe porté par la machine, explicable | REQ-009-25 (valeurs des machines d'une adresse, avec la source) ; composition par la 010 (010/S27) |

## Non-objectifs

- Baies, emplacements, ports physiques, câblage, liens entre interfaces (M1).
- Agrégats de liens (LAG) et sous-interfaces comme objets liés : une sous-interface est
  une interface nommée (`bond0.100`) ; le lien de parenté entre interfaces viendra avec
  le détail physique.
- Services exposés par une machine (D29, « plus tard ») et environnement d'une machine
  (010) : la machine est déjà porteuse de propriétés, ce qui suffit à la 010.
- Découverte automatique (connecteurs).

## Questions ouvertes

- Aucune bloquante. À relire par l'utilisateur : M16 (droits d'un rattachement, choix
  de sécurité).

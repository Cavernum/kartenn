# 009 — Machines et interfaces : journal des décisions

Entretien du 2026-10-20. L'utilisateur a tranché le périmètre (M1) et délégué le reste :
les autres questions ont été posées et tranchées par Claude selon la recommandation la
plus standard des référentiels d'inventaire et d'adressage. Cadre : D12 (ancres), D18 et
D31 (module de la v0.1), D19 à D23 (la machine dans la future politique), D22 (résolution
par adresse, rôle d'interface), D33 (suppression), D36 (RBAC) ; ADR 0026 (contrat de
module), spec 008 (VLAN, première implémentation du contrat) ; 003 (propriétés).
Référence : [`docs/reference/vlan-mac-ieee-802.md`](../../docs/reference/vlan-mac-ieee-802.md).

## Synthèse
- M1 périmètre : modèle logique (machines, interfaces logiques, MAC, adresses, VLAN) et
  hébergement (VM → hôte ou cluster) ; ni baies, ni ports physiques, ni câblage.
- M2 types de machine : `physical`, `virtual`, `container`, `appliance`, colonne fermée.
- M3 nom de machine : nom d'hôte (étiquettes LDH), unique par propriétaire à tout instant.
- M4 ancres : `core.owner` obligatoire, `core.site` facultatif ; l'interface hérite.
- M5 hébergement : une machine virtuelle ou un conteneur désigne un hôte (machine) ou un
  cluster ; une machine physique peut appartenir à un cluster.
- M6 cluster : objet du module, slug unique, ancré comme une machine.
- M7 interface : logique, nom unique par machine, MAC EUI-48 unicast facultative, état
  activé.
- M8 adresses : rattachement interface ↔ adresse daté ; une adresse sur une seule
  interface à la fois, sauf usage `vip` ou `anycast`.
- M9 VLAN : mode `access` (un VLAN non étiqueté) ou `trunk` (VLAN étiquetés et VLAN natif
  facultatif) ; le module dépend de `vlan`.
- M10 rôle d'interface : propriété `machines.interface_role` (liste fermée extensible).
- M11 cohérence : avertissements MAC en double, adresse hors des VLAN de l'interface,
  machine physique hébergée.
- M12 suppression : refus, cascades déclarées (interfaces, rattachements, hébergement mis
  à nul).
- M13 lectures : machines d'une adresse et d'un réseau, adresses d'une machine.
- M14 API, permissions, plugin : `/v1/machines`, `/v1/interfaces`, `/v1/clusters`.
- M15 budgets : 10⁵ machines, 3·10⁵ interfaces, 3·10⁵ rattachements.
- M16 droits d'un rattachement : écriture sur l'interface et lecture de l'adresse.

## M1 — Périmètre
- Décision : **modèle logique et hébergement**. Machines (physiques, virtuelles,
  conteneurs, appliances) ancrées sur une entité et un site ; interfaces **logiques**
  (nom, MAC, adresses, VLAN) ; lien d'une machine virtuelle ou d'un conteneur vers son
  hôte ou son cluster de virtualisation. Pas de baies, de ports physiques, de câblage
  ni de liens entre interfaces en v0.1.
- Pourquoi : c'est ce dont la politique de flux (D22, résolution par adresse et rôle
  d'interface) et l'équipe réseau ont besoin ; le détail physique est un autre module
  (D12, « le détail physique va dans un module »).
- Rejeté : modèle DCIM complet (hors v0.1) ; machines sans interfaces (adresses
  rattachées directement à la machine : perd le rôle d'interface de D22).
- Décidé par : l'utilisateur (2026-10-20)

## M2 — Types de machine
- Décision : colonne `kind` fermée : `physical`, `virtual`, `container`, `appliance`.
  Le type porte des invariants (M5) ; tout autre classement (système, fonction,
  modèle) est une propriété.
- Pourquoi : liste donnée par l'utilisateur ; les règles d'hébergement en dépendent.
- Rejeté : propriété libre (invariants d'hébergement impossibles) ; liste extensible
  (les règles ne sauraient pas traiter une valeur nouvelle).
- Décidé par : l'utilisateur (types), Claude (forme, 2026-10-20)

## M3 — Nom d'une machine
- Décision : **nom d'hôte** : une ou plusieurs étiquettes LDH (lettres, chiffres, `-`, 1 à
  63 caractères chacune, sans `-` en tête ni en fin, RFC 1123 §2.1), 253 caractères au
  plus, comparé sans casse et conservé en minuscules. Unique, à tout instant, parmi les
  machines vivantes de **même propriétaire résolu** (`machines.name.duplicate`). Un FQDN
  éventuel est une propriété (`machines.fqdn`, type FQDN de la 003).
- Pourquoi : les règles d'inférence (D24) filtrent par nom ; deux entités peuvent avoir
  leur `srv01` ; un nom unique par propriétaire rend un import par nom non ambigu.
- Rejeté : nom global unique (collisions entre entités) ; nom libre (inexploitable par
  les règles et le DNS).
- Décidé par : Claude (délégation de l'utilisateur, 2026-10-20)

## M4 — Ancres
- Décision : une machine et un cluster portent `core.owner` (obligatoire) et `core.site`
  (facultatif) ; une interface les hérite de sa machine (arbre `machines`, ADR 0026
  clause 4). Une machine virtuelle n'hérite pas de son hôte : son propriétaire peut
  différer (hébergement mutualisé).
- Pourquoi : D12, ADR 0014 ; le RBAC par entité et site couvre les machines.
- Rejeté : machine dans l'arbre d'adressage (contraire à D22 : « les machines ne sont pas
  des nœuds de l'arbre ») ; héritage depuis l'hôte (mélange les propriétaires).
- Décidé par : Claude (délégation de l'utilisateur, 2026-10-20)

## M5 — Hébergement
- Décision : `virtual` et `container` peuvent désigner **un** hébergeur : une machine
  (`physical` ou `virtual` pour un conteneur ; `physical` pour une VM) ou un cluster.
  `physical` et `appliance` n'ont pas d'hébergeur ; une machine `physical` peut être
  **membre** d'un cluster. L'hébergeur est facultatif (inventaire incomplet) et daté.
  Pas de cycle d'hébergement.
- Pourquoi : modèle usuel (VM sur un hôte ou un cluster, conteneur sur une VM ou un
  hôte) ; le placement exact d'une VM dans un cluster bouge trop pour être déclaré.
- Rejeté : hébergeur obligatoire (imports progressifs bloqués) ; appliance virtuelle
  distincte (une appliance virtuelle est une `virtual` avec une propriété).
- Décidé par : l'utilisateur (lien VM → hôte ou cluster), Claude (règles, 2026-10-20)

## M6 — Cluster de virtualisation
- Décision : type d'objet `machines.cluster` : slug unique à tout instant, `core.owner`
  obligatoire, `core.site` facultatif. Ses membres sont les machines `physical` qui le
  désignent ; les VM et conteneurs le désignent comme hébergeur.
- Pourquoi : un cluster s'administre (droits, propriétés) indépendamment de ses hôtes.
- Rejeté : cluster comme propriété texte (pas de référence, pas de suppression contrôlée).
- Décidé par : Claude (délégation de l'utilisateur, 2026-10-20)

## M7 — Interface
- Décision : une interface appartient à une machine (fixe : changer de machine = clore
  et recréer), a un **nom** unique par machine à tout instant (1 à 64 caractères,
  imprimables, sans espace : `eth0`, `bond0.100`, `Gi1/0/1`), une **MAC** EUI-48
  facultative (bit de groupe à 0, RFC 9542 §2.1.1) et un état `enabled` (vrai par
  défaut). Description, débit, type… sont des propriétés.
- Pourquoi : les noms d'interface viennent des systèmes et des équipements ; MAC absente
  pour les interfaces virtuelles sans adresse de niveau 2 (tunnel, boucle locale).
- Rejeté : MAC unique (machines virtuelles clonées, adresses locales réutilisées) ;
  types d'interface en colonne (aucun invariant n'en dépend en v0.1).
- Décidé par : Claude (délégation de l'utilisateur, 2026-10-20)

## M8 — Rattachement des adresses
- Décision : un **rattachement** relie une interface à une adresse du noyau sur une
  période. Une adresse est rattachée à **une seule** interface à la fois, sauf si son
  usage est `vip` ou `anycast` (plusieurs interfaces, multi-attachement). Une interface
  porte un nombre quelconque d'adresses, IPv4 et IPv6, de domaines quelconques ; un
  rattachement peut être marqué `primary` (une par famille et par interface au plus).
- Pourquoi : D22 résout par adresse ; une VIP est par nature partagée ; le reste est un
  conflit d'adresse.
- Rejeté : adresse sur plusieurs interfaces sans contrôle (conflits invisibles) ;
  rattachement à la machine (perd le rôle d'interface).
- Décidé par : Claude (délégation de l'utilisateur, 2026-10-20)

## M9 — VLAN d'une interface
- Décision : mode 802.1Q de l'interface : `none` (par défaut), `access` (un VLAN non
  étiqueté) ou `trunk` (ensemble de VLAN étiquetés, plus un VLAN natif facultatif, non
  étiqueté, hors de l'ensemble). Le module `machines` **dépend** du module `vlan`.
- Pourquoi : les deux modes standard ; la dépendance évite un modèle à options.
- Rejeté : VLAN en propriété multiple (pas de mode, pas de natif) ; dépendance
  facultative (deux modèles à tester pour un module de la même livraison).
- Décidé par : Claude (délégation de l'utilisateur, 2026-10-20)

## M10 — Rôle d'interface (D22)
- Décision : propriété du module `machines.interface_role`, liste fermée, simple, portée
  par les interfaces, non héritée ; valeurs livrées `administration`, `backup`,
  `monitoring`, `storage`, `cluster`, `service` ; l'administrateur peut ajouter des
  valeurs (003, REQ-003-5).
- Pourquoi : D22 nomme le rôle « attribut du module Machines », exploité par le module
  Politique ; une propriété a historique, droits et explication.
- Rejeté : colonne (liste figée) ; texte libre (inexploitable par la politique).
- Décidé par : Claude (délégation de l'utilisateur, 2026-10-20)

## M11 — Cohérence
- Décision : avertissements (anomalies, non bloquants) : `machines.mac.duplicate` (même
  MAC universelle sur deux interfaces vivantes), `machines.address.vlan_mismatch` (adresse
  dont le réseau porte un `vlan.vlan` qui n'est ni le VLAN d'accès, ni le natif, ni un
  VLAN étiqueté d'une interface en mode `access` ou `trunk`), `machines.host.site_mismatch`
  (VM dont l'hôte a un autre site résolu). Erreurs : règles de M3, M5, M7, M8, M9.
- Pourquoi : signaler sans bloquer les inventaires imparfaits ; préparer les anomalies
  de la 010.
- Décidé par : Claude (délégation de l'utilisateur, 2026-10-20)

## M12 — Suppression (D33)
- Décision : refus par défaut ; cascades déclarées (ADR 0026 clause 12) : machine →
  interfaces (clôture) ; interface → rattachements (clôture) ; adresse du noyau →
  rattachements (clôture) ; VLAN → interfaces (retrait du VLAN, mise à nul) ; hôte ou
  cluster → machines hébergées et membres (mise à nul de l'hébergeur).
- Décidé par : Claude (délégation de l'utilisateur, 2026-10-20)

## M13 — Lectures croisées
- Décision : interfaces et machines d'une adresse (`GET /v1/interfaces?address=`),
  machines d'un réseau (adresses contenues), adresses d'une machine, machines hébergées
  par un hôte ou un cluster ; datées. Elles sont la base de la résolution « machine A
  vers machine B » de la 010 (D22).
- Décidé par : Claude (délégation de l'utilisateur, 2026-10-20)

## M14 — API, permissions, interface
- Décision : `/v1/machines`, `/v1/interfaces`, `/v1/clusters` et leurs sous-ressources ;
  opérations `machines.create_machine`, `update_machine`, `create_cluster`,
  `update_cluster`, `create_interface`, `update_interface`, `attach_address`,
  `detach_address`. Permissions `machines.machine|interface|cluster.read|write|propose`.
  Plugin `web/plugins/machines` : vues, onglet « Machines » sur une adresse et un réseau,
  colonne machine des adresses, action « rattacher à une interface ».
- Décidé par : Claude (délégation de l'utilisateur, 2026-10-20)

## M15 — Budgets
- Décision : budgets de D14 sur le jeu de référence augmenté de 10⁵ machines (dont 60 %
  virtuelles), 10³ clusters, 3·10⁵ interfaces et 3·10⁵ rattachements ; lecture d'une
  machine avec ses interfaces et adresses < 50 ms ; machines d'un réseau /24 < 150 ms.
- Décidé par : Claude (délégation de l'utilisateur, 2026-10-20)

## M16 — Droits d'un rattachement
- Décision : rattacher ou détacher une adresse demande `machines.interface.write` sur
  l'interface et `core.address.read` sur l'adresse ; ce n'est pas une écriture de
  l'adresse (elle ne change pas de version).
- Pourquoi : les équipes système déclarent leurs machines sans droit d'écriture sur
  l'adressage ; l'unicité de M8 empêche de s'approprier une adresse déjà rattachée.
- Rejeté : `core.address.write` exigé (bloque l'usage courant) ; aucune exigence sur
  l'adresse (rattacher une adresse qu'on ne voit pas).
- À relire : choix de sécurité, signalé dans le rapport.
- Décidé par : Claude (délégation de l'utilisateur, 2026-10-20)

## M17 — Média d'une interface (REQ-009-26)
- Constat (2026-10-26, relecture de la maquette E11) : l'utilisateur veut lire sur la
  fiche d'une interface si elle est en ethernet, SFP, Wi-Fi, etc. ; le modèle n'en dit
  rien (REQ-009-6).
- Décision : une propriété `machines.interface_media`, liste fermée simple, non
  héritée, facultative, extensible par un administrateur, sur le modèle du rôle
  (REQ-009-8) ; valeurs livrées `ethernet`, `sfp`, `wifi`, `aggregate`, `virtual`,
  `loopback`.
- Pourquoi : une propriété hérite gratuitement de l'historique, de l'explication, des
  filtres, du différentiel et de l'extension par l'administrateur ; une colonne typée
  aurait demandé tout cela à nouveau. Le débit et le connecteur précis (SFP+, QSFP)
  restent hors champ : une valeur ajoutée par l'administrateur suffit si le besoin
  vient.
- Rejeté : colonne `media` sur `interface` (schéma et contrat figés pour une liste qui
  évolue).
- Décidé par : l'utilisateur pour le besoin (2026-10-26) ; la forme : Claude, choix peu
  coûteux et réversible.

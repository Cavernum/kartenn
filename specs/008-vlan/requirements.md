---
status: draft
updated: 2026-10-20
---

# 008 — VLAN : exigences

## Résumé

Premier module métier de kartenn (D18, D31). Il décrit les VLAN IEEE 802.1Q, rangés en
**groupes** (espaces de numérotation) ancrés sur une entité, un site ou un domaine
d'adressage, et les relie aux réseaux par une propriété. C'est aussi la spec qui
**implémente le contrat de module** (ADR 0026) : la partie A vaut pour tout module, la
009 (Machines et interfaces) puis la 010 et la 011 s'en servent.

Décisions : [`decisions.md`](decisions.md) (V1–V15) ; D4, D6, D11, D12, D18, D31, D33,
D36 ([`../001-cadrage/decisions.md`](../001-cadrage/decisions.md)) ; ADR 0026.
Référence : [`docs/reference/vlan-mac-ieee-802.md`](../../docs/reference/vlan-mac-ieee-802.md).

### Vocabulaire

- **Module** : crate `kartenn-<module>` qui implémente le contrat de l'ADR 0026.
- **Objet de module** : objet d'un type déclaré par un module (`vlan.group`,
  `vlan.vlan`), inscrit dans `module_object`.
- **Groupe de VLAN** : espace de numérotation des VID, ancré sur une entité, un site ou
  un domaine d'adressage.
- **VID** : identifiant de VLAN 802.1Q, entier de 1 à 4094.
- **Ancre** : objet du noyau qui place un objet dans un périmètre RBAC (D12, ADR 0014).
- Conventions EARS, « écriture », « période » et sévérités : comme dans
  [`002/requirements.md`](../002-noyau-adressage/requirements.md#conventions).

## Exigences

### A. Contrat de module (ADR 0026)

#### REQ-008-1 — Registre des modules
kartenn doit construire au démarrage un registre des modules compilés, et refuser de
démarrer si deux modules portent le même nom, si un module porte le nom `core`, si une
dépendance déclarée est absente ou si les dépendances forment un cycle.

Acceptation :
- Un module de test qui dépend d'un module absent : le serveur s'arrête avec un message
  qui nomme le module et la dépendance ; même chose pour un cycle A → B → A.
- `GET /v1/capabilities` liste `vlan` (nom, version) dans `modules` quand le module est
  compilé, et ne le liste pas sinon.

#### REQ-008-2 — Migrations d'un module
kartenn doit appliquer les migrations d'un module dans son propre schéma PostgreSQL,
après celles du noyau et de ses dépendances, une seule fois chacune, et refuser de
démarrer si une migration déjà appliquée a changé (somme de contrôle).

Acceptation :
- Base neuve : schéma `vlan` créé, migrations suivies dans `module_migration`.
- Une migration appliquée puis modifiée : le job de migration échoue en nommant le
  module et la version.
- Le serveur compilé sans le module ne touche pas au schéma `vlan` existant.

#### REQ-008-3 — Objets de module
kartenn doit inscrire chaque version d'un objet de module dans `module_object` (type,
parent, ancrage de domaine éventuel, période) dans la transaction du changeset qui
l'écrit, pour que version (ETag), identifiants externes, anomalies, historique,
propriétés et annulation s'appliquent aux objets de module comme à ceux du noyau.

Acceptation :
- Un VLAN créé a un ETag, un historique, des propriétés, et peut recevoir un
  identifiant externe (`BindExternal`).
- L'annulation d'un changeset qui a créé un VLAN le clôt ; celle d'une modification
  rend la version précédente.

#### REQ-008-4 — Opérations de module
kartenn doit accepter dans un changeset les opérations d'un module, désignées par une
étiquette qualifiée (`<module>.<opération>`), et les faire passer par le même pipeline
que celles du noyau : validation locale, verrous, application, validation globale,
droits, différentiel, approbation, journal, événement et audit.

Acceptation :
- Un changeset mêlant `create_prefix` et `vlan.create_vlan` (qui se référencent par nom
  local) est atomique : une erreur de l'un annule l'autre.
- Une étiquette `vlan.inconnue` ou d'un module non compilé est refusée avant tout verrou
  (400, problème `changeset-structure`).
- Une politique d'approbation qui retient les changesets de plus de N opérations compte
  les opérations de module.

#### REQ-008-5 — Opérations génériques sur les objets de module
kartenn doit appliquer aux objets de module les opérations génériques du noyau :
clôture (avec ou sans cascade), affectation et retrait de propriété, garde de version
(`Expect`), identifiant externe, restauration, période propre (`Within`).

Acceptation :
- `close` d'un VLAN, `assign_property` d'une propriété `custom.*` portée par
  `vlan.vlan`, `expect` sur un VLAN : comportements identiques à ceux sur un préfixe.

#### REQ-008-6 — Ancres d'un objet de module
kartenn doit résoudre le propriétaire (`core.owner`) et le site (`core.site`) d'un objet
de module par ses propriétés, héritées le long de l'arbre du module, et placer dans le
périmètre d'un domaine l'objet dont le type déclare un ancrage de domaine, pour le RBAC
en lecture et en écriture (ADR 0014, 0021).

Acceptation :
- Un utilisateur lié au rôle de lecture sur l'entité E voit les groupes de E et leurs
  VLAN, et pas ceux d'une autre entité ; un groupe ancré sur le site S est visible d'une
  liaison sur S ou un site parent ; un groupe ancré sur le domaine D, d'une liaison sur D.
- Un objet dont le type exige `core.owner` sans valeur résolue est refusé (erreur
  d'obligation de la 003).

#### REQ-008-7 — Propriétés vers et sur les objets de module
kartenn doit admettre les types d'objets de module comme porteurs de propriétés et
comme cibles d'une propriété de référence.

Acceptation :
- Un administrateur déclare `custom.contact` portée par `vlan.vlan` ; la propriété
  `vlan.vlan` (référence vers `vlan.vlan`) se pose sur un réseau.
- Une référence vers un VLAN clos à la date de la période est refusée.

#### REQ-008-8 — Catalogues fusionnés
kartenn doit fusionner les permissions et les règles déclarées par chaque module avec
celles du noyau : validation des rôles, catalogue des règles (violations et anomalies) et
`docs/spec/permissions.md` généré.

Acceptation :
- Un rôle qui liste `vlan.vlan.write` est accepté avec le module, refusé sans
  (`rbac.unknown_permission`).
- Le test de génération de `docs/spec/permissions.md` couvre les permissions du module.

#### REQ-008-9 — Ressources REST d'un module
kartenn doit monter sous `/v1` les routes d'un module actif, avec les conventions de la
006, et servir pour chaque type d'objet de module les routes transverses d'historique,
de propriétés et d'explication ; le contrat OpenAPI de référence inclut les modules
livrés et reste additif.

Acceptation :
- `GET /v1/vlans/{id}/history`, `/properties`, `/properties/{key}/explanation`
  répondent comme pour un préfixe.
- La comparaison du contrat produit avec la référence (006) passe avec les modules
  livrés ; le contrat du serveur sans module est un sous-ensemble.

#### REQ-008-10 — Références et suppression
kartenn doit refuser la clôture d'un objet, du noyau ou d'un module, auquel un objet
vivant d'un module fait référence par une référence déclarée « refus », en listant les
dépendants (RFC 9457), et, avec la cascade explicite, inclure dans le changeset la
clôture ou la mise à nul des dépendants selon la politique déclarée, sous leurs propres
droits (D33).

Acceptation :
- Clore un VLAN porté par un réseau : refus qui liste le réseau ; avec cascade : la
  propriété `vlan.vlan` du réseau est retirée, le différentiel le montre.

#### REQ-008-11 — Arêtes du graphe
Où un module déclare des arêtes, kartenn doit les servir à une date donnée (source,
cible, type, période), filtrées par les droits de lecture des deux extrémités.

Acceptation :
- `GET /v1/graph/edges?object=<réseau>` rend l'arête `vlan.carries` entre le VLAN et le
  réseau.

### B. Groupes de VLAN

#### REQ-008-12 — Groupe
kartenn doit gérer des groupes de VLAN identifiés par un slug unique à tout instant
(REQ-002-47), ancrés sur exactement une entité, un site ou un domaine d'adressage, munis
d'une plage de VID admise (`vid_min` ≤ `vid_max`, dans 1–4094, par défaut 1–4094) et d'un
propriétaire (`core.owner`, obligatoire).

Acceptation :
- Création d'un groupe ancré sur un site sans `core.owner` : refus. Ancré sur un
  domaine sans propriétaire fourni : propriétaire = propriétaire résolu du domaine.
- Deux groupes de même slug à des périodes qui se chevauchent : refus.
- `vid_min` = 0 ou `vid_max` = 4095 : refus.

#### REQ-008-13 — Modifier un groupe
kartenn doit permettre de renommer un groupe, de changer son ancre et sa plage de VID
par changeset, et refuser une plage qui exclut un VLAN vivant du groupe sur la période,
en listant les VLAN.

Acceptation :
- Groupe avec le VLAN 100, plage réduite à 1–99 : refus qui liste le VLAN.
- Changer l'ancre d'un groupe : le différentiel liste ses VLAN dont le périmètre change.

### C. VLAN

#### REQ-008-14 — VLAN
kartenn doit gérer des VLAN, chacun dans un groupe, avec un VID entier de 1 à 4094 dans
la plage du groupe, un nom de 1 à 64 caractères (NFC, sans caractère de contrôle) et un
statut `active`, `reserved` ou `deprecated` (`active` par défaut).

Acceptation :
- VID 0, 4095 ou hors de la plage du groupe : refus (`vlan.vid.invalid`,
  `vlan.vid.out_of_range`).
- Nom vide, de 65 caractères ou avec un caractère de contrôle : refus.

#### REQ-008-15 — Unicités
kartenn doit refuser deux VLAN vivants d'un même groupe, à des périodes qui se
chevauchent, de même VID (`vlan.vid.duplicate`) ou de même nom (`vlan.name.duplicate`).

Acceptation :
- Deux changesets concurrents créant le VID 10 dans le même groupe : l'un réussit,
  l'autre est refusé (contrainte d'exclusion en dernier recours).
- Le même VID dans deux groupes : accepté.
- Un VLAN créé à partir de 2027-01-01 avec le VID d'un VLAN clos au 2026-12-31 :
  accepté.

#### REQ-008-16 — Modifier un VLAN
kartenn doit permettre de changer le VID, le nom, le statut et le groupe d'un VLAN par
changeset, sous les mêmes règles qu'à la création.

Acceptation :
- Déplacer un VLAN vers un groupe où son VID est pris : refus.

### D. Lien aux réseaux

#### REQ-008-17 — Propriété `vlan.vlan`
kartenn doit fournir la propriété `vlan.vlan`, référence simple vers un VLAN, portée par
les préfixes, non héritée, et refuser de la poser sur un préfixe qui n'est pas de nature
réseau (`vlan.prefix.not_network`).

Acceptation :
- Poser `vlan.vlan` sur un conteneur : refus. Sur deux réseaux (IPv4 et IPv6) : accepté.
- Passer un réseau en conteneur alors qu'il porte `vlan.vlan` : refus (même règle).

#### REQ-008-18 — Cohérence site et domaine
kartenn doit signaler en anomalie (avertissement) un réseau dont le VLAN appartient à un
groupe ancré sur un site hors du sous-arbre du `core.site` du réseau
(`vlan.site.mismatch`), ou sur un domaine autre que celui du réseau
(`vlan.domain.mismatch`).

Acceptation :
- Réseau du site Lyon, VLAN d'un groupe ancré sur Paris : avertissement à la
  simulation, anomalie après application, acquittable.

### E. Lectures

#### REQ-008-19 — Listes et lectures datées
kartenn doit lister les groupes et les VLAN avec les conventions de la 006 (datation,
curseur, filtres en conjonction, `QUERY`) : groupes par slug, ancre ; VLAN par groupe,
VID, plage de VID, nom (exact ou préfixe), statut.

Acceptation :
- `GET /v1/vlans?group=<id>&vid=10..20&status=active` rend les VLAN correspondants dans
  l'ordre du VID.

#### REQ-008-20 — Occupation et prochain VID libre
kartenn doit rendre, à une date donnée, l'occupation d'un groupe (tranches contiguës de
VID utilisés, réservés et libres dans sa plage, compteurs) et le prochain VID libre, le
plus petit ou à partir d'un VID donné.

Acceptation :
- Groupe 1–10 avec les VID 1, 2 (actifs) et 5 (réservé) : tranches [1–2 utilisés],
  [3–4 libres], [5 réservé], [6–10 libres] ; prochain libre 3 ; à partir de 5 : 6.
- Groupe plein : prochain libre absent (404 problème `exhausted`, comme REQ-002-24).

#### REQ-008-21 — Réseaux d'un VLAN
kartenn doit lister, à une date donnée, les réseaux qui portent un VLAN, filtrés par les
droits de lecture de l'appelant.

Acceptation :
- Un réseau d'une entité que l'appelant ne lit pas n'apparaît pas ; le compteur ne le
  révèle pas.

### F. Droits, imports, interface, budgets

#### REQ-008-22 — Permissions
kartenn doit contrôler les écritures et lectures du module par `vlan.group.read|write|
propose` et `vlan.vlan.read|write|propose`, évaluées sur le périmètre hérité du groupe.

Acceptation :
- Écrivain VLAN sur l'entité E : crée un VLAN dans un groupe de E ; refus (403 au
  changeset) dans un groupe d'une autre entité.

#### REQ-008-23 — Imports
kartenn doit accepter les opérations du module dans les imports par morceaux de la 006.

Acceptation :
- Un import de 10⁴ VLAN répartis en 100 groupes aboutit ; une reprise après arrêt ne
  duplique rien.

#### REQ-008-24 — Plugin d'interface
Où le shell web (007) est livré, kartenn doit fournir un plugin de première partie
`vlan` (vues des groupes et des VLAN, onglet VLAN d'un réseau, colonne VLAN de la liste
des préfixes, action de création d'un VLAN pour un réseau), servi seulement si le module
est actif.

Acceptation :
- Parcours Playwright : créer un groupe, un VLAN, le poser sur un réseau ; axe-core sans
  violation (007/W14).

#### REQ-008-25 — Budgets
kartenn doit respecter les budgets de D14 pour les lectures, l'occupation et le
changeset unitaire du module, sur le jeu de référence augmenté de 2·10⁴ groupes et 10⁵
VLAN dont la moitié porte un ou deux réseaux.

Acceptation :
- `bench-core` mesure lecture d'un VLAN < 50 ms, page filtrée < 150 ms, occupation
  < 200 ms, création d'un VLAN < 200 ms (p95 serveur) ; bloquant en CI.

## Non-objectifs

- QinQ (802.1ad), VXLAN et autres identifiants de superposition (V1).
- Configuration des équipements, découverte des VLAN sur le réseau.
- Plusieurs tranches de VID par groupe (V6).
- Types d'événements propres aux modules (ADR 0026, clause 10).
- Rattachement d'une interface à un VLAN : 009.

## Questions ouvertes

- Aucune bloquante. L'ADR 0026 est `proposed` : à relire par l'utilisateur avant la
  phase P1 des tâches (choix structurant : schéma `module_object`, enveloppe
  d'opération).

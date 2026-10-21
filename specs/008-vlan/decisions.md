# 008 — VLAN : journal des décisions

Entretien du 2026-10-20. L'utilisateur a tranché le périmètre (V1, V2, V3) et délégué le
reste : les autres questions ont été posées et tranchées par Claude selon la
recommandation la plus standard des référentiels IPAM. Cadre : D4 (domaines), D6
(modules compilés), D11 (changesets), D12 (ancres entité, site, domaine, « portée aux
groupes de VLAN »), D18 et D31 (VLAN dans la v0.1), D33 (suppression), D36 (RBAC) ;
ADR 0026 (contrat de module, proposée, implémentée par cette spec) ; 003 (propriétés),
006 (API), 007 (plugins de première partie, W6). Référence :
[`docs/reference/vlan-mac-ieee-802.md`](../../docs/reference/vlan-mac-ieee-802.md).

## Synthèse
- V1 standard : IEEE 802.1Q, VID 1 à 4094 ; ni QinQ ni VXLAN en v0.1.
- V2 groupes : un VLAN appartient à un groupe, espace de numérotation où le VID est unique.
- V3 ancrage des groupes : entité, site ou domaine d'adressage (un seul).
- V4 lien aux réseaux : propriété `vlan.vlan` d'un préfixe réseau, référence vers un VLAN.
- V5 attributs : VID, nom, statut (actif, réservé, obsolète) ; le reste en propriétés.
- V6 bornes du groupe : plage de VID autorisée par groupe, 1–4094 par défaut.
- V7 unicité du nom : par groupe, erreur ; nom sans contrainte de format au-delà de 1–64.
- V8 VLAN hors groupe : interdit ; un groupe par défaut n'est pas créé d'office.
- V9 cohérence site et domaine : avertissements quand un réseau et son VLAN divergent.
- V10 suppression : refus si un réseau ou une interface y fait référence ; cascade explicite.
- V11 lectures : occupation et prochain VID libre d'un groupe, réseaux d'un VLAN.
- V12 API : `/v1/vlan-groups`, `/v1/vlans`, conventions de la 006.
- V13 permissions : `vlan.group.*`, `vlan.vlan.*` ; ancres héritées du groupe.
- V14 plugin : `web/plugins/vlan`, après la 007.
- V15 budgets : ceux de D14, banc `bench-core` étendu à 2·10⁴ groupes et 10⁵ VLAN.

## V1 — Norme et étendue
- Décision : VLAN **IEEE 802.1Q** (C-VLAN), identifiant de 12 bits, VID **1 à 4094**
  (0 et 4095 réservés par la norme). Pas d'empilement 802.1ad (QinQ) ni de VXLAN ou
  d'autre identifiant de superposition en v0.1.
- Pourquoi : couvre le besoin de l'équipe réseau visée par la v0.1 (D31) ; un
  identifiant de superposition se modélisera par un autre type d'objet du module sans
  migrer les VLAN.
- Rejeté : QinQ et VXLAN dès la v0.1 (modèle plus lourd, aucun besoin exprimé).
- Décidé par : l'utilisateur (2026-10-20)

## V2 — Groupes de VLAN
- Décision : un VLAN appartient à exactement un **groupe de VLAN**, espace de
  numérotation dans lequel le VID est **unique à tout instant** ; deux groupes peuvent
  réutiliser le même VID. Un groupe a un slug unique (comme un domaine, REQ-002-47) et
  peut être renommé.
- Pourquoi : la même valeur de VID est réutilisée d'un site, d'une fabrique ou d'un
  client à l'autre ; c'est le modèle usuel des référentiels.
- Rejeté : VID unique globalement (faux dès deux sites) ; VID unique par site seulement
  (une fabrique couvre plusieurs sites, un site peut avoir plusieurs commutateurs
  isolés).
- Décidé par : l'utilisateur (2026-10-20)

## V3 — Ancrage d'un groupe
- Décision : un groupe est ancré sur **une** entité, **un** site ou **un** domaine
  d'adressage (D12). L'ancre fixe le périmètre RBAC du groupe et de ses VLAN :
  - ancre entité : le groupe n'a que son `core.owner`, cette entité ;
  - ancre site : `core.site` = ce site ;
  - ancre domaine : ancrage de domaine de l'ADR 0026 (`module_object.domain_id`, le
    groupe est dans le périmètre du domaine et sous son verrou).
  Dans tous les cas le groupe porte un `core.owner` (obligatoire, affecté explicitement
  à la création ; pour une ancre domaine, la valeur proposée par défaut est le
  propriétaire résolu du domaine), comme tout objet du RBAC (ADR 0014).
  L'ancre peut changer par changeset (déplacement de groupe), avec le différentiel de
  reclassification des VLAN.
- Pourquoi : D12 nomme ces trois axes comme portée des groupes de VLAN ; un seul ancre
  garde le périmètre lisible.
- Rejeté : plusieurs ancres à la fois (périmètre ambigu) ; ancre libre vers un préfixe
  (un VLAN porte souvent des réseaux de plusieurs préfixes et familles).
- Décidé par : l'utilisateur (ancres), Claude (règles d'ancres, 2026-10-20)

## V4 — Lien entre VLAN et réseaux
- Décision : le module déclare la propriété **`vlan.vlan`** : référence vers un VLAN,
  cardinalité simple, portée par les **préfixes**, non héritée. Seul un préfixe de
  nature réseau peut la porter (règle `vlan.prefix.not_network`, erreur). Un VLAN peut
  porter plusieurs réseaux (IPv4 et IPv6, secondaires). Le réseau d'un VLAN se lit dans
  les deux sens (réseaux d'un VLAN, VLAN d'un réseau).
- Pourquoi : réutilise le mécanisme des propriétés (historique, différentiel, droits par
  propriété, explication) au lieu d'une table de liaison ; le noyau ne change pas
  (ADR 0026, clause 5).
- Rejeté : colonne `vlan_id` dans `prefix` (le noyau dépendrait du module) ; table de
  liaison propre (réimplémente historique et droits) ; propriété héritée (un conteneur
  n'a pas de VLAN).
- Décidé par : l'utilisateur (lien), Claude (mécanisme, 2026-10-20)

## V5 — Attributs d'un VLAN
- Décision : attributs de table : groupe, **VID**, **nom**, **statut** (`active`,
  `reserved`, `deprecated`). Description, rôle, contact… sont des **propriétés** (002/A10,
  003). Le statut `reserved` réserve le VID sans usage ; `deprecated` le garde visible
  avant clôture.
- Pourquoi : seuls VID, groupe et nom portent des invariants (unicités) ; le statut sert
  à l'occupation et au prochain libre.
- Rejeté : description en colonne (contraire à A10) ; statut libre (non comparable).
- Décidé par : Claude (délégation de l'utilisateur, 2026-10-20)

## V6 — Plage de VID d'un groupe
- Décision : un groupe déclare la plage de VID admise (`vid_min`, `vid_max`, 1 et 4094
  par défaut). Un VLAN hors plage est refusé ; réduire la plage sous un VLAN existant est
  refusé (le refus liste les VLAN).
- Pourquoi : les sites réservent souvent des tranches (production, invités, gestion) ;
  le prochain libre doit rester dans la tranche.
- Rejeté : plusieurs tranches par groupe (rare ; deux groupes y suffisent).
- Décidé par : Claude (délégation de l'utilisateur, 2026-10-20)

## V7 — Nom d'un VLAN
- Décision : texte de 1 à 64 caractères (Unicode normalisé NFC, sans caractère de
  contrôle), **unique par groupe à tout instant** (erreur `vlan.name.duplicate`), sans
  autre format imposé.
- Pourquoi : les équipements limitent la longueur du nom mais pas de façon uniforme ; 64
  couvre les usages ; l'unicité évite l'ambiguïté d'un import par nom.
- Rejeté : unicité en simple avertissement (imports ambigus) ; format de slug (noms
  existants avec espaces et majuscules).
- Décidé par : Claude (délégation de l'utilisateur, 2026-10-20)

## V8 — VLAN sans groupe
- Décision : interdit : le groupe est obligatoire. Aucun groupe par défaut n'est créé
  par la migration ; un import crée d'abord ses groupes.
- Pourquoi : un groupe implicite serait un périmètre RBAC implicite.
- Rejeté : groupe « global » créé d'office (périmètre et propriétaire arbitraires).
- Décidé par : Claude (délégation de l'utilisateur, 2026-10-20)

## V9 — Cohérence entre réseau et VLAN
- Décision : deux avertissements (anomalies, non bloquants) : `vlan.site.mismatch` quand
  le `core.site` d'un réseau diffère du site d'ancrage du groupe de son VLAN (si les deux
  sont définis et que le site du réseau n'est pas dans le sous-arbre du site du groupe) ;
  `vlan.domain.mismatch` quand le groupe est ancré sur un domaine autre que celui du
  réseau. Un même VLAN portant des réseaux de domaines différents est permis.
- Pourquoi : signaler une erreur probable sans bloquer les cas réels (VLAN étendu entre
  sites, VRF multiples sur un même segment).
- Rejeté : erreurs bloquantes (cas réels refusés) ; aucun contrôle.
- Décidé par : Claude (délégation de l'utilisateur, 2026-10-20)

## V10 — Suppression
- Décision : clore un VLAN porté par un réseau (`vlan.vlan`) ou rattaché à une interface
  (009) est refusé ; la cascade explicite retire ces affectations dans le même
  changeset (D33, ADR 0026 clause 12). Clore un groupe qui contient des VLAN est refusé ;
  la cascade clôt ses VLAN (et leurs affectations).
- Pourquoi : D33.
- Rejeté : mise à nul implicite des réseaux.
- Décidé par : Claude (délégation de l'utilisateur, 2026-10-20)

## V11 — Lectures propres au module
- Décision : occupation d'un groupe (VID utilisés, réservés, libres, par tranches
  contiguës) et prochain VID libre (le plus petit, ou à partir d'un VID donné) ; réseaux
  d'un VLAN ; VLAN d'un réseau par le filtre de propriété du noyau. Toutes datées
  (`valid_at`, `as_of`).
- Pourquoi : symétrique de l'occupation des préfixes (REQ-002-24) ; besoin quotidien.
- Rejeté : réservation atomique « prochain libre » côté serveur (une création avec VID
  choisi par le client et refus en cas de conflit suffit, comme pour les adresses).
- Décidé par : Claude (délégation de l'utilisateur, 2026-10-20)

## V12 — API
- Décision : `/v1/vlan-groups` et `/v1/vlans` (liste filtrée, lecture datée, création,
  Merge Patch sous `If-Match`, clôture, historique, propriétés),
  `/v1/vlan-groups/{id}/occupancy`, `/v1/vlan-groups/{id}/next-free`,
  `/v1/vlans/{id}/prefixes`. Opérations de changeset `vlan.create_group`,
  `vlan.update_group`, `vlan.create_vlan`, `vlan.update_vlan`. Filtres : groupe, VID,
  nom, statut, ancre.
- Pourquoi : conventions de la 006 (H2, H3, H13) ; évolution additive (H12).
- Décidé par : Claude (délégation de l'utilisateur, 2026-10-20)

## V13 — Permissions et ancres
- Décision : `vlan.group.read|write|propose`, `vlan.vlan.read|write|propose`. Un VLAN
  hérite des ancres de son groupe (arbre `vlan` le long de `module_object.parent_id`) :
  ses droits sont ceux du périmètre du groupe. Les rôles livrés de lecture reçoivent les
  permissions `read`. Poser `vlan.vlan` sur un réseau demande `prop.vlan.vlan.write` sur
  le réseau et `vlan.vlan.read` sur le VLAN cible.
- Pourquoi : D36 ; un VLAN n'a pas d'ancre propre.
- Décidé par : Claude (délégation de l'utilisateur, 2026-10-20)

## V14 — Interface
- Décision : plugin de première partie `web/plugins/vlan` (007/W6) : vues des groupes et
  des VLAN, onglet « VLAN » sur la fiche d'un réseau, colonne VLAN dans la liste des
  préfixes, action « créer un VLAN pour ce réseau ». Réalisé après le SDK de la 007.
- Décidé par : Claude (délégation de l'utilisateur, 2026-10-20)

## V15 — Budgets
- Décision : budgets de D14 (lecture < 50 ms, page filtrée < 150 ms, occupation
  < 200 ms, changeset unitaire < 200 ms, p95 serveur) sur le jeu de référence étendu de
  2·10⁴ groupes et 10⁵ VLAN, dont la moitié porte un ou deux réseaux ; mesurés par
  `bench-core` (bloquant en CI).
- Pourquoi : un VLAN est un petit objet ; le risque est la jointure par propriété vers
  10⁵ réseaux et le RBAC hérité du groupe.
- Décidé par : Claude (délégation de l'utilisateur, 2026-10-20)

## V16 — Refus d'une écriture hors du périmètre : 422, pas 403
- Décision : une opération de module sur un objet hors du périmètre d'écriture de
  l'appelant est refusée par la violation `core.object.forbidden` du changeset (422),
  rapportée avec les autres violations, comme pour le noyau depuis la 004 ; 403 reste
  réservé au refus d'une route entière (permission absente pour la route). REQ-008-22
  est alignée ; la 007 (REQ-007-20) et la 009 (REQ-009-18) aussi.
- Pourquoi : un changeset rapporte toutes ses violations en une réponse ; un droit
  manquant sur un objet cité en est une ; même comportement pour le noyau et les
  modules, contrat REST inchangé.
- Rejeté : 403 dès qu'un droit manque (contrat 006 modifié, clients et shell à adapter).
- Décidé par : l'utilisateur (2026-10-21)

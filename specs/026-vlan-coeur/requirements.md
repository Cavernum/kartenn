---
status: draft
updated: 2026-10-27
---

# 026 — Le VLAN dans le cœur : exigences

Statut : **brouillon, à valider par l'utilisateur.**

## Résumé

Le VLAN quitte le statut de module métier (D18, spec 008) pour devenir un objet du
**cœur**, au même titre que le préfixe et l'adresse (arbitrage de l'utilisateur du
2026-10-27, « Cœur complet ») : ses tables entrent dans le schéma du cœur, ses routes
dans `/v1` sans feature de build ni registre de module, ses règles parmi celles du cœur,
ses écrans dans le shell. Le comportement fonctionnel décrit par la 008 (REQ-008-12 à 25)
ne change pas : la 026 le reprend sous des noms du cœur. Le **contrat de module** (008,
partie A, REQ-008-1 à 11 et 26 à 28 ; ADR 0026) reste, pour Machines, Politique et Flux.

Motif : le VLAN est une notion de base d'un référentiel réseau ; un réseau sans son VLAN
est une fiche incomplète, et le module Machines (VLAN d'une interface, REQ-009-12) en
dépendait déjà. Le garder optionnel obligeait chaque module voisin et chaque écran à
prévoir son absence.

Décisions : [`decisions.md`](decisions.md) (C1 à C7) ; D4, D6, D12, D18 (amendée), D31,
D33, D36 ([`../001-cadrage/decisions.md`](../001-cadrage/decisions.md)) ; spec 008
(V1 à V16), 009 (REQ-009-12), 021. Pas de compatibilité à garder avant le gel : ni alias
d'ancien nom, ni double route.

## Exigences

### A. Place dans le cœur

#### REQ-026-1 — Objets du cœur
kartenn doit gérer les groupes de VLAN et les VLAN comme des objets du cœur
(`core.vlan_group`, `core.vlan`), présents dans tout build, hors du registre des modules,
avec les règles, unicités, modifications et cohérences de REQ-008-12 à 18, inchangées sur
le fond.

Acceptation :
- Un build sans aucun module sert `/v1/vlan-groups` et `/v1/vlans`.
- `GET /v1/modules` (capacités) ne cite plus `vlan`.
- Les cas d'acceptation de REQ-008-12 à 18 passent sous les nouveaux noms.

#### REQ-026-2 — Noms du cœur
kartenn doit nommer les permissions, codes de règle, propriété et événements du VLAN
dans l'espace `core` : permissions `core.vlan_group.*` et `core.vlan.*` (actions `read`,
`edit`, `apply`, `approval.self`, comme les autres objets du cœur, 024) ; codes
`core.vlan.*` (ex. `vlan.vid.duplicate` → `core.vlan.vid_duplicate`, selon la grammaire
des codes du cœur) ; propriété du réseau `core.vlan` (ex-`vlan.vlan`) ; types d'objet de
l'audit et du flux d'événements `vlan_group` et `vlan`.

Acceptation :
- `git grep` des anciens noms (`vlan.group.`, `vlan.read`, `vlan.vlan`, codes `vlan.*`)
  hors historique des migrations et notes « anciennement » : vide ; le test serveur qui
  traque les anciens noms de permission couvre aussi ceux-ci.

#### REQ-026-3 — Reprise des données
kartenn doit reprendre, à la migration, les groupes, les VLAN, leurs versions et leur
historique, les valeurs de la propriété, les rôles qui citent les anciennes permissions
et le journal des changesets qui cite les anciens codes, sans perte ni réécriture de
l'audit.

Acceptation :
- Une base peuplée par la 008 (groupes, VLAN, réseaux qui portent un VLAN, interfaces en
  mode accès ou trunk) relit, après migration, les mêmes objets à toute date
  (`valid_at`, `as_of`) ; un rôle personnalisé qui citait `vlan.write` cite
  `core.vlan.edit` et `core.vlan.approval.self`.
- Une seconde exécution de la migration ne change rien.

### B. Liens avec les modules

#### REQ-026-4 — Machines
Le module Machines doit lire le VLAN d'une interface (REQ-009-12) dans le cœur, sans
dépendre d'un autre module.

Acceptation :
- Build avec Machines seul : poser un VLAN en mode accès sur une interface fonctionne.

#### REQ-026-5 — Politique et Flux
Les modules Politique et Flux doivent lire le VLAN par le port de lecture du cœur là où
ils le lisaient par le module (axes, cohérences, tests).

### C. Interface

#### REQ-026-6 — Écrans du VLAN dans le shell
Le shell v2 (021) doit porter les écrans du VLAN comme des écrans du cœur (groupes,
VLAN, occupation et prochain VID libre, onglet VLAN d'un réseau, colonne VLAN de la
liste des préfixes, création d'un VLAN pour un réseau) ; le plugin `web/plugins/vlan`
du shell v1 est retiré au profit d'écrans du shell v1 seulement si la 021 n'a pas encore
remplacé v1 au moment de la tâche (C6).

Acceptation :
- Maquette : l'écran VLAN quitte E10 (hôte de plugin) pour un écran du cœur ; E10 garde
  la matrice de la Politique comme exemple de plugin.

### D. Lectures, imports, budgets

#### REQ-026-7 — Reprise de REQ-008-19 à 23 et 25
kartenn doit garder les lectures (listes datées, occupation, prochain VID libre, réseaux
d'un VLAN filtrés par droits), les imports par morceaux et les budgets de la 008, sous
les noms du cœur.

Acceptation :
- `bench-core` garde les quatre mesures de REQ-008-25 et leurs budgets.

## Non-objectifs

- Changer le modèle du VLAN (Q-in-Q, VLAN privés, VXLAN) : hors champ, comme en V1.
- Retirer le contrat de module : il reste, éprouvé par Machines, Politique et Flux.

## Questions ouvertes

Aucune ; les choix sont dans [`decisions.md`](decisions.md).

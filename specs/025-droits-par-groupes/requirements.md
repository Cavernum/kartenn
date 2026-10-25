---
status: approved
updated: 2026-10-25
---

# 025 — Droits par groupes : exigences

Statut : **validée le 2026-10-25** (Q-025-1 tranchée).

## Résumé

Les droits ne se donnent plus à une personne ni à un émetteur entier : un rôle se lie à un
groupe tenu par kartenn, sur un périmètre, et les comptes entrent dans des groupes. Un
groupe de l'IdP peut être membre d'un groupe kartenn. Personne ne donne, par une liaison
ni par une appartenance, un droit plus large que le sien. L'administration des droits se
fait groupe par groupe.

Décisions : [`decisions.md`](decisions.md) (G1 à G10) ; D36 ; 005/K4, K6, K7, K13, K17
(amendées) ; REQ-005-18, 26, 27 (amendées) ; 004 (politiques d'approbation) ; 006 ; 021
(E7).

### Vocabulaire

- **Groupe** : groupe tenu par kartenn ; seul sujet d'une liaison.
- **Membre** : identité (personne, compte de service, compte de secours) ou groupe de
  l'IdP (émetteur et nom du groupe tel que le jeton le sert).
- **Liaison** : rôle × groupe × périmètre, sur une période.
- Conventions EARS : comme dans
  [`002/requirements.md`](../002-noyau-adressage/requirements.md#conventions).

## Exigences

### A. Groupes et appartenances

#### REQ-025-1 — Groupes
kartenn doit tenir des groupes (clé unique, libellé, livré ou non), versionnés comme les
rôles, créés, renommés et clos par changeset. [G2]

Acceptation :
- Un groupe clos n'a plus ni membres ni liaisons valides après sa clôture.

#### REQ-025-2 — Membres
kartenn doit tenir les membres de chaque groupe : identités et groupes de l'IdP, chaque
appartenance versionnée et bornable dans le temps. [G2]

Acceptation :
- Un compte de service ajouté à un groupe reçoit ses droits à la requête suivante.
- Une appartenance dont la période est échue ne donne plus rien.

#### REQ-025-3 — Pas d'imbrication
kartenn doit refuser qu'un groupe kartenn soit membre d'un autre groupe. [G6]

### B. Décision

#### REQ-025-4 — Liaisons aux seuls groupes
kartenn doit accorder une permission à une identité sur un objet si et seulement si une
liaison valide associe à un rôle qui contient la permission, sur un périmètre qui contient
l'objet, un groupe dont l'identité est membre à la date de la requête, directement ou par
l'un des groupes de l'IdP de sa session ; **et** si l'objet est dans le confinement de son
émetteur. Amende REQ-005-18. [G1, G3, K13]

Acceptation :
- Une identité membre d'aucun groupe n'a aucun droit, même si son émetteur est rattaché.
- Le groupe `reseau` de l'émetteur A, membre du groupe kartenn `lecteurs-ouest`, ouvre
  aux identités de A qui l'ont dans leur jeton les droits de `lecteurs-ouest`, et à
  elles seules.
- Une liaison ne peut viser ni une identité ni un émetteur (refus à l'écriture).

#### REQ-025-5 — Changesets en attente
Quand un changeset en attente est revalidé (approbation, reprise d'un import), kartenn
doit résoudre les groupes kartenn de son auteur à la date de la revalidation, à partir
des groupes de l'IdP gardés dans l'instantané de l'auteur. [005, 004]

### C. Délégation sans escalade

#### REQ-025-6 — Liaison
kartenn doit n'autoriser une liaison que si son auteur détient `core.rbac.admin` sur un
périmètre qui contient celui de la liaison et toutes les permissions du rôle lié sur ce
périmètre. Reprend REQ-005-26 avec le groupe pour sujet. [K7, G5]

#### REQ-025-7 — Appartenance
kartenn doit n'autoriser l'ajout ou le retrait d'un membre d'un groupe que si son auteur
satisfait REQ-025-6 pour chacune des liaisons valides du groupe ; refus :
`core.group.escalation`. [G5]

Acceptation :
- Un administrateur de l'entité E ne peut ni entrer quelqu'un, ni s'entrer lui-même, dans
  un groupe lié à un rôle sur le périmètre global ou sur une entité hors de E.
- Il ne peut pas non plus retirer un membre d'un tel groupe.
- Il gère librement un groupe dont toutes les liaisons sont dans E et portent des rôles
  dont il détient toutes les permissions dans E.

#### REQ-025-8 — Clôture d'un groupe
kartenn doit n'autoriser la clôture d'un groupe que sous les conditions de REQ-025-7 ;
la clôture clôt ses liaisons et ses appartenances dans le même changeset. [G5]

#### REQ-025-9 — Groupes livrés
kartenn doit livrer le groupe `secours`, lié au rôle administrateur global, synchronisé
comme les rôles livrés et non modifiable hors synchronisation ; le compte de secours y
entre à sa création pour la durée de sa validité. Amende K17. [G7]

### D. Administration et API

#### REQ-025-10 — Par changesets, annulable
Les opérations sur les groupes, leurs membres et leurs liaisons doivent passer par des
changesets, être auditées et être annulables (004). [005, REQ-005-25]

#### REQ-025-11 — Routes
L'API doit servir les groupes, leurs membres et leurs liaisons ; `GET /v1/me` et la fiche
d'une identité doivent servir ses groupes kartenn (direct ou par quel groupe de l'IdP) et
ses droits effectifs ; la fiche d'une identité sert aussi les groupes de l'IdP de sa
dernière session, avec la date de cette session. [006, G4, Q-025-1]

#### REQ-025-12 — Écran
L'écran Administration (021, E7) doit administrer les droits groupe par groupe (membres,
rôles sur périmètres) ; la fiche d'une identité montre ses groupes et droits effectifs en
lecture ; aucun écran ne lie un rôle à une identité. [G4]

#### REQ-025-13 — Politiques d'approbation
Les auteurs d'une politique d'approbation doivent se désigner par rôle ou par groupe
kartenn. Amende REQ-005-27. [G10]

### E. Migration et performance

#### REQ-025-14 — Migration
La migration doit reporter les liaisons existantes sur des groupes kartenn (groupe de
l'IdP → groupe qui l'a pour membre ; identité → groupe qui l'a pour membre), retirer les
liaisons d'émetteur entier, lister ces reports et retraits, et passer par un changeset
système. [G9]

#### REQ-025-15 — Budgets
La résolution des groupes ne doit pas faire sortir la décision des budgets de la 005
(REQ-005-29), mesurée par `bench-rbac` avec 50 groupes de l'IdP par identité et
1 000 liaisons. [D14]

---
status: draft
updated: 2026-10-20
---

# 011 — Registre de flux : exigences

## Résumé

Le module Flux (`flow`, v0.2, D31), qui dépend de Politique (010), tient le **registre
des flux déclarés** : qui doit pouvoir parler à qui, pour quel service, pourquoi et
jusqu'à quand. Chaque flux est confronté en continu à la politique : tout changement
d'environnement, de matrice, de garde-fou, de service ou de rattachement **réévalue les
flux touchés**, et le changeset qui le porte les nomme avant d'être appliqué (D30).
C'est le socle de l'atteignabilité future (D20).

Décisions : [`decisions.md`](decisions.md) (F1–F15) ; D20, D22, D25, D28 à D30, D33
([`../001-cadrage/decisions.md`](../001-cadrage/decisions.md)) ; spec 010.

### Vocabulaire

- **Flux** : déclaration (source, destination, service, justification, demandeur,
  entité, période de validité).
- **Extrémité** : adresse, préfixe, interface ou machine.
- **Verdict d'un flux** : verdicts (010) de chaque paire d'adresses des extrémités pour
  son service, et leur **synthèse** (`allowed`, `partial`, `denied`).
- **Dérogation** : acceptation explicite, motivée et bornée d'un flux non conforme.
- **Différentiel des flux** : flux dont la synthèse ou un verdict bascule du fait d'un
  changeset.
- Le reste comme dans [`010/requirements.md`](../010-politique-flux/requirements.md).

## Exigences

### A. Déclaration

#### REQ-011-1 — Contenu
Le module doit permettre de créer un flux avec : extrémité source, extrémité
destination, un service actif sur toute la période, une justification non vide (1 à
4 000 caractères), un demandeur (identité ; texte facultatif « pour le compte de »), une
entité, une référence externe facultative et une période de validité (F2).

Acceptation :
- Flux `ssh` de la machine `bastion-1` vers le réseau `10.5.0.0/24`, du 1er mars au
  31 août : accepté si conforme.
- Service clos le 1er juin pour un flux jusqu'au 31 août : refusé.

#### REQ-011-2 — Extrémités
Le module doit accepter comme extrémité une adresse, un préfixe ou, où le module
Machines est présent, une interface ou une machine, valides sur toute la période du
flux ; il doit refuser une plage (F3).

#### REQ-011-3 — Clôture d'une extrémité
Si un changeset clôt un objet désigné comme extrémité d'un flux actif ou à venir, alors
le module doit refuser la clôture en listant les flux, sauf cascade explicite qui clôt
aussi ces flux (D33, REQ-004-25, 26).

#### REQ-011-4 — Modification
Le module doit permettre de modifier justification, référence externe et période d'un
flux ; changer une extrémité ou le service crée un nouveau flux (le précédent est clos),
pour que l'historique d'un flux garde un sens constant.

### B. Cycle de vie

#### REQ-011-5 — État dérivé
Le module doit restituer pour chaque flux, à une date, l'état `requested`, `rejected`,
`approved`, `active`, `expired` ou `withdrawn`, dérivé du changeset de création et de la
période de validité, sans écriture d'état (F4).

Acceptation :
- Proposition en attente : `requested` ; validée, période au 1er mars, lue le 20 février :
  `approved` ; le 2 mars : `active` ; le 1er septembre : `expired`.
- Flux clos le 1er juin alors que sa période allait au 31 août : `withdrawn` après le
  1er juin.

#### REQ-011-6 — Approbation
Le module doit fournir aux politiques d'approbation (REQ-004-40) les déclencheurs
`flow.created` (création ou prolongation d'un flux) et `flow.breaks` (au moins N flux qui
passent de `allowed` ou `partial` à `denied`). `flow.created` ne s'applique pas par
défaut (F5, REQ-004-39) ; à son installation, le module doit créer une politique
d'approbation active `flow.breaks ≥ 1`, que l'administrateur peut désactiver (F8).

#### REQ-011-7 — Échéances
Le module doit émettre l'événement `kartenn.flow.expiring` 30 jours et 7 jours (réglable)
avant la fin de validité d'un flux, et `kartenn.flow.expired` à son terme, sans écrire
dans le flux (F10).

### C. Conformité

#### REQ-011-8 — Conformité à l'enregistrement
Si une création ou une prolongation de flux a un verdict `denied` sur une partie de sa
période, alors le module doit refuser l'écriture (`flow.verdict.denied`) en citant les
paires et les éléments décisifs, sauf si le flux porte une dérogation (F6).

Acceptation :
- Flux `ssh` de `dev` vers `prod` avec un garde-fou `(admin, dev, prod)` : refusé, le
  garde-fou est cité.

#### REQ-011-9 — Dérogation
Le module doit accepter un flux non conforme qui porte une dérogation (motif non vide,
date de fin au plus tard celle du flux) si l'auteur détient `flow.flow.waive` sur son
entité ; la dérogation est restituée par toutes les lectures du flux.

#### REQ-011-10 — Partiel
Quand la synthèse d'un flux est `partial` (une machine dont seule une partie des
adresses est autorisée), le module doit accepter l'écriture avec l'avertissement
`flow.verdict.partial`.

#### REQ-011-11 — Réévaluation continue
Quand un changeset modifie l'environnement résolu d'une adresse d'extrémité, une
cellule, un garde-fou, un service, un type de flux ou un rattachement d'interface d'une
extrémité, le module doit recalculer, dans la même transaction, les verdicts des flux
touchés sur la fenêtre du changeset (F7, D30).

Acceptation :
- Insérer un conteneur `{dev}` au-dessus d'une adresse source `{prod}` d'un flux
  autorisé par `(admin, prod, prod)` : le flux passe `denied` dans le résultat du
  changeset.

#### REQ-011-12 — Changement qui casse des flux
Quand un changeset fait passer des flux à `denied`, le module doit l'accepter, ouvrir
l'anomalie (avertissement) `flow.verdict.denied` sur chacun, et la fermer quand le flux
redevient conforme (F8).

### D. Différentiel

#### REQ-011-13 — Différentiel des flux
Quand un changeset est soumis ou simulé, le module doit restituer les flux dont la
synthèse ou un verdict bascule, avec avant, après et élément décisif, décompte d'abord
et détail paginé (F9, D25, D30).

Acceptation :
- Retirer la cellule `(application, rec, prod)` alors que 120 flux en dépendent : la
  simulation annonce 120 flux `allowed → denied` avant toute écriture.

### E. Lecture

#### REQ-011-14 — Filtres
Le module doit permettre de filtrer les flux à une date par : extrémité (exacte, préfixe
et son sous-arbre, machine et ses adresses), service, type de flux, état, synthèse,
dérogation, entité, demandeur, échéance avant une date (F11).

#### REQ-011-15 — Flux d'un objet
Le module doit lister, pour un objet (adresse, préfixe, interface, machine), les flux
dont il est extrémité ou dont une extrémité le contient.

#### REQ-011-16 — Verdicts datés
Le module doit restituer les verdicts d'un flux à tout (`valid_at`, `as_of`), sans
recalcul pour une date passée (F7).

### F. Droits et API

#### REQ-011-17 — Permissions
Le module doit déclarer `flow.flow.read`, `flow.flow.write`, `flow.flow.propose` et
`flow.flow.waive`, portées sur l'entité du flux, et exiger à la création la lecture des
deux extrémités (F12).

Acceptation :
- Un utilisateur sans lecture du réseau destination ne peut pas créer de flux vers lui
  (même 404 qu'un objet absent).

#### REQ-011-18 — API
Le module doit exposer `/v1/flows` (liste, filtres, `QUERY`, routes de ressource) et
`/v1/flows/{id}/verdicts`, de manière additive (F15).

### G. Performance

#### REQ-011-19 — Budgets
Le module doit respecter (p95 serveur, 10⁶ objets, 10⁵ flux, politique du banc 010) :
création d'un flux machine × machine 16 × 16 < 300 ms ; réévaluation de 10⁴ flux dans un
changeset < 3 s ajoutées ; page filtrée de flux < 150 ms ; réévaluation complète en
tâche de fond < 5 min (F14).

## Non-objectifs

- Import des règles de pare-feu, atteignabilité effective (D20 étape 2).
- Services exposés par une machine (D29, plus tard).
- Génération de règles de pare-feu à partir du registre.
- Interface Web : plugin `web/plugins/flow/` (007/W6).

## Questions ouvertes

- [ ] F8 : un changement qui casse des flux est accepté (avec anomalie et déclencheur
  d'approbation) plutôt que refusé ; à confirmer. (utilisateur)
- [ ] F6 : dérogation bornée dans le temps obligatoire ; durée maximale à fixer ?
  (utilisateur)

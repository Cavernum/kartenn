---
status: in-progress
updated: 2026-10-22
---

# 011 — Registre de flux : exigences

## Résumé

Le module Flux (`flow`, v0.2, D31), qui dépend de Politique (010), tient le **registre
des flux déclarés** : qui doit pouvoir parler à qui, pour quel service, sur quelle
référence externe. Une demande est **triée** à l'enregistrement : un flux autorisé dont
l'issue est automatique entre au registre sans intervention humaine ; tout autre part à
l'**instruction** de l'équipe sécurité, qui l'approuve, le rejette en documentant une
**non-conformité** ou accorde une **dérogation** datée. Un flux conforme n'a pas de date
de fin : sa validité est **dérivée en continu** de la politique (D30 amendée) ; tout
changement de politique, de classification ou de rattachement **réévalue les flux
touchés**, et le changeset qui le porte les nomme avant d'être appliqué. C'est le socle
de l'audit des flux constatés (v0.3) et de l'atteignabilité future (D20).

Sources : besoin [`../010-politique-flux/besoin/`](../010-politique-flux/besoin/)
(UC-2 à UC-4, UC-10, UC-13 ; EB-14 à EB-18, EB-25 à EB-27 ; Q3, Q8, Q16 à Q18, Q24) ;
décisions [`decisions.md`](decisions.md) (F1 à F18) ; D20, D22, D25, D28 à D30, D33
([`../001-cadrage/decisions.md`](../001-cadrage/decisions.md), D28 et D30 amendées le
2026-10-21) ; spec 010.

### Vocabulaire

- **Flux** : demande enregistrée (source, destination, service, dossier, demandeur,
  entité), avec son évaluation courante et son historique.
- **Sorte** : `requested` (demandé, v0.2) ou `observed` (constaté, réservé à la v0.3).
- **Extrémité** : adresse, préfixe, interface ou machine.
- **Évaluation d'un flux** : évaluations (010) de chaque paire d'adresses des extrémités
  pour son service (verdict, issue, conditions non remplies, score), et leur
  **synthèse** (`allowed`, `partial`, `denied` ; issue la plus défavorable).
- **Instruction** : revue humaine d'une demande dont l'issue n'est pas automatique.
- **Dérogation** : acceptation explicite, motivée, datée et recertifiée d'un flux dont le
  verdict est refusé ; elle ne modifie jamais la politique.
- **Non-conformité** : demande rejetée à l'instruction, flux devenu non conforme, ou
  flux qui ne tient que par dérogation.
- **Différentiel des flux** : flux dont la synthèse, un verdict ou l'issue bascule du
  fait d'un changeset.
- Le reste comme dans [`010/requirements.md`](../010-politique-flux/requirements.md).

## Exigences

### A. Déclaration

#### REQ-011-1 — Contenu
Le module doit permettre de demander un flux avec : extrémité source, extrémité
destination, un service actif sur toute la période, **au moins une référence externe**
(système source en liste fermée administrable, identifiant), une justification
**facultative** (1 à 4 000 caractères), un demandeur (identité, et « pour le compte de »
facultatif), des responsables des extrémités facultatifs, la nature des données
facultative (valeurs de l'axe de sensibilité, indicateur de données personnelles), une
entité et une période dont la fin est **facultative** (F2, Q3, Q16).

Acceptation :
- Flux `ssh` de la machine `bastion-1` vers le réseau `10.5.0.0/24`, référence
  `ticketing:CHG-1042`, sans justification ni date de fin : accepté s'il est conforme.
- Sans référence externe : refusé (`flow.reference.missing`).
- Le système se nomme par la clé de sa valeur de liste fermée (`ticketing`), celle que
  publie l'API, ou par son identifiant ; un système inconnu est refusé
  (`flow.reference.system`), jamais ignoré (correctif du 2026-10-23).
- Service clos le 1er juin pour un flux qui court au-delà : refusé.

#### REQ-011-2 — Extrémités
Le module doit accepter comme extrémité une adresse, un préfixe ou, où le module
Machines est présent, une interface ou une machine, valides sur toute la période du
flux ; il doit refuser une plage (F3).

#### REQ-011-3 — Clôture d'une extrémité
Si un changeset clôt un objet désigné comme extrémité d'un flux actif ou à venir, alors
le module doit refuser la clôture en listant les flux, sauf cascade explicite qui clôt
aussi ces flux (D33, REQ-004-25, 26).

#### REQ-011-4 — Modification
Le module doit permettre de modifier les références externes, la justification, les
responsables, la nature des données et la période d'un flux ; changer une extrémité ou
le service crée un nouveau flux (le précédent est clos), pour que l'historique d'un flux
garde un sens constant. Avancer la fin s'applique sans instruction ; une **prolongation**
(fin plus tardive, ou fin retirée d'un flux daté) doit être triée comme une demande
(REQ-011-21) sur la portion ajoutée, contre la politique du jour, et instruite si elle
n'est pas automatique (REQ-011-22) ; un flux terminé ne se prolonge pas (F20).

Acceptation :
- Avancer la fin d'un flux actif : appliqué (200), sans instruction.
- Prolonger de 6 mois un flux instruit dont la portion ajoutée est `allowed` sous
  réserve : proposition à instruire (202), le flux garde sa fin jusqu'à l'approbation ;
  approuvée avec une fin à 13 mois de la décision : refusée
  (`flow.review.end_required`).
- Prolongation dont la portion ajoutée est refusée par une règle `deny` : approuvable
  seulement avec une dérogation (`flow.waiver.required` sinon).
- L'auteur de la prolongation l'approuve lui-même : refusé (`flow.separation`).
- Prolonger un flux `expired` : refusé (`flow.update.ended`).

#### REQ-011-20 — Sorte de flux
Le module doit porter la sorte de chaque flux et refuser, en v0.2, la sorte `observed`
(`flow.kind.reserved`) ; le modèle (évaluation, filtres, non-conformités) doit déjà
l'admettre (F16, Q18, UC-13).

### B. Triage, instruction, cycle de vie

#### REQ-011-21 — Triage à l'enregistrement
Quand une demande de flux est soumise, le module doit l'évaluer (010, dossier compris) ;
si la synthèse est `allowed` et l'issue `automatic`, il doit l'enregistrer dans le
changeset ; sinon il doit enregistrer le changeset comme **proposition à instruire**
(REQ-004-41), jamais appliquée sans instruction ; dans les deux cas, la réponse porte
l'évaluation et son explication (F5, F6, EB-15, UC-1, UC-2).

Acceptation :
- Demande conforme, toutes conditions remplies : flux `active` dans la réponse.
- Demande `allowed` sous réserve (motif attendu) : proposition en attente, issue
  `review`, motif attendu et son texte dans la réponse.
- Demande refusée par une règle `deny` : proposition en attente, issue `noncompliant`,
  règle et texte cités.

#### REQ-011-22 — Instruction
Le module doit permettre à un instructeur (`flow.review` sur l'entité, distinct du
demandeur et de l'auteur d'une prolongation) d'**approuver** une proposition à
instruire, demande ou prolongation (avec une date de fin obligatoire, au plus
`[flow] review_max_months` après la décision, 12 par défaut ; une dérogation si le
verdict est refusé, REQ-011-9) ou de la **rejeter** avec un motif obligatoire et la
règle en cause, ce qui crée une non-conformité (F5, F17, F20, UC-3).

Acceptation :
- Le demandeur approuve sa propre demande : refusé (séparation des tâches, C1).
- Rejet motivé : la non-conformité est listée pour le demandeur avec le texte
  pédagogique de la règle.

#### REQ-011-5 — État dérivé
Le module doit restituer pour chaque flux, à une date, l'état `requested`, `rejected`,
`scheduled`, `active`, `noncompliant`, `waived`, `expired` ou `withdrawn`, dérivé du
changeset de création, de la période, de l'évaluation courante et de la dérogation, sans
écriture d'état (F4). Les demandes en attente doivent être servies par la lecture des
flux (`state=requested`), chacune avec le changeset à instruire et l'évaluation de son
triage, lue sans recalcul (F19).

Acceptation :
- Proposition en attente : `requested` ; rejetée : `rejected`.
- `GET /v1/flows?state=requested` rend la demande en attente avec `id` nul, son
  changeset et l'évaluation du triage (issue `review`, motif attendu) égale à celle de
  la réponse 202 ; sans `requested` dans `state`, elle n'apparaît pas.
- Le demandeur la lit avec une évaluation réduite ; un instructeur (`flow.review`
  sur l'entité) la lit ; un utilisateur sans droit sur l'entité ne la voit pas.
- À un `as_of` antérieur à sa soumission, elle n'apparaît pas ; après son rejet, elle
  n'apparaît plus.
- Flux conforme sans date de fin, appliqué : `active` aussi longtemps que son verdict
  est `allowed`.
- Une règle `deny` ajoutée le 1er mai le rend refusé : `noncompliant` à partir du 1er
  mai ; avec une dérogation valide : `waived`.
- Flux instruit dont la fin est passée : `expired` ; clos avant son terme :
  `withdrawn`.

#### REQ-011-24 — Validité dérivée
Le module doit tenir pour valide, sans date de fin ni écriture périodique, un flux
`requested` enregistré automatiquement tant que sa synthèse est `allowed` et que ses
extrémités existent ; il ne doit jamais prolonger, créer ou reclasser seul (F4, F18,
EB-17, EB-18).

#### REQ-011-6 — Approbation
Le module doit fournir aux politiques d'approbation (REQ-004-40) les déclencheurs
`flow.created` (création ou prolongation d'un flux, REQ-011-4), `flow.nonconforming` (au
moins N flux qui deviennent non conformes à la matrice de flux : ils passent de `allowed`
ou `partial` à `denied`) et `flow.waiver` (octroi ou prolongation d'une dérogation). Seul
`flow.nonconforming` s'applique par défaut : à son installation, le module doit créer une
politique d'approbation active `flow.nonconforming ≥ 1`,
que l'administrateur peut désactiver (F5, F8, F20, EB-25).

Acceptation :
- Politique `flow.created ≥ 1` active : une prolongation triée automatique est retenue
  pour approbation comme une création.

#### REQ-011-7 — Échéances et événements
Le module doit émettre `org.kartenn.flow.expiring` 30 et 7 jours (réglable) avant la fin
d'un flux daté ou d'une dérogation, `org.kartenn.flow.expired` à son terme,
`org.kartenn.flow.outcome_changed` quand l'état ou l'issue d'un flux change et
`org.kartenn.flow.closed` à sa clôture, sans écrire dans le flux (F10, Q24, N35).

### C. Conformité

#### REQ-011-8 — Conformité à l'enregistrement
Si une création ou une prolongation de flux appliquée sans instruction a un verdict
`denied` ou une issue autre que `automatic` sur une partie de sa période, alors le module
doit refuser l'écriture (`flow.review.required`), sauf approbation d'instruction
(REQ-011-22) et, pour un verdict `denied`, dérogation (REQ-011-9) (F6).

Acceptation :
- Écriture directe d'un flux `ssh` de `dev` vers `prod` avec une règle `deny admin dev →
  prod` : refusée, la règle citée.

#### REQ-011-9 — Dérogation
Le module doit accepter un flux dont le verdict est `denied` s'il porte une dérogation :
motif et risque accepté non vides, date de fin au plus tôt de celle du flux et de la
date d'octroi plus `[flow] waiver_max_months` (6 par défaut), accordée par un auteur qui
détient `flow.waiver.grant` sur l'entité et n'est pas le demandeur ; la dérogation ne
modifie pas la politique ; sa **recertification** est une prolongation par changeset,
qui réévalue le flux contre la politique du jour (F6, Q8, UC-4).

Acceptation :
- Dérogation sans risque accepté, au-delà de 6 mois ou accordée par le demandeur :
  refusée (`flow.waiver.invalid`).
- À l'échéance d'une dérogation non recertifiée, le flux encore refusé passe
  `noncompliant`.

#### REQ-011-10 — Partiel
Quand la synthèse d'un flux est `partial` (une machine dont seule une partie des
adresses est autorisée), le module doit soumettre la demande à l'instruction (l'issue
de synthèse est la plus défavorable, REQ-010-15) et signaler l'avertissement
`flow.verdict.partial`.

#### REQ-011-11 — Réévaluation continue
Quand un changeset modifie la valeur résolue d'un axe cité par la politique sur une
adresse d'extrémité, une règle, un motif, une condition, un signal, un service, un type
de flux ou un rattachement d'interface d'une extrémité, le module doit recalculer, dans
la même transaction, les évaluations (verdict, issue, conditions, score) des flux
touchés sur la fenêtre du changeset ; un changement de règle d'inférence ne réévalue
aucun flux (F7, D30 amendée, D23).

Acceptation :
- Insérer un conteneur `{dev}` au-dessus d'une adresse source `{prod}` d'un flux
  autorisé par `allow admin prod → prod` : le flux passe `denied` dans le résultat du
  changeset.

#### REQ-011-12 — Changement qui rend des flux non conformes
Quand un changeset fait passer des flux à `denied`, le module doit l'accepter, les
rendre `noncompliant` (sauf dérogation valide), ouvrir l'anomalie (avertissement)
`flow.verdict.denied` sur chacun et la fermer quand le flux redevient conforme ; quand
seule l'issue se dégrade (`automatic` → `review`), il doit le signaler dans le
différentiel et par événement, sans changer l'état (F8).

#### REQ-011-23 — Évaluation figée et historique
Le module doit conserver l'évaluation de l'enregistrement (élément décisif, révision de
politique, issue, instructeur éventuel) et l'historique des évaluations courantes,
restituables à tout (`valid_at`, `as_of`) (F7, Q2, Q16).

### D. Différentiel

#### REQ-011-13 — Différentiel des flux
Quand un changeset est soumis ou simulé, le module doit restituer les flux dont la
synthèse, un verdict ou l'issue bascule, avec avant, après et élément décisif, décompte
d'abord et détail paginé (F9, D25, D30).

Acceptation :
- Retirer `allow application rec → prod` alors que 120 flux en dépendent : la simulation
  annonce 120 flux `allowed → denied` avant toute écriture.

### E. Lecture

#### REQ-011-14 — Filtres
Le module doit permettre de filtrer les flux à une date par : extrémité (exacte, préfixe
et son sous-arbre, machine et ses adresses), service, type de flux, sorte, état,
synthèse, issue, règle décisive, dérogation, échéance avant une date, entité, demandeur,
référence externe, valeur d'un axe d'une extrémité (application, propriétaire) ; et
par instruction en attente (`review=pending` : demandes et prolongations) ou par
changeset (F11, F19).

#### REQ-011-15 — Flux d'un objet
Le module doit lister, pour un objet (adresse, préfixe, interface, machine), les flux
dont il est extrémité ou dont une extrémité le contient.

#### REQ-011-16 — Évaluations datées
Le module doit restituer les évaluations d'un flux à tout (`valid_at`, `as_of`), sans
recalcul pour une date passée (F7).

#### REQ-011-25 — Non-conformités consultables
Le module doit rendre consultables et filtrables (application, demandeur, règle,
période, état, sorte) les non-conformités — demandes rejetées à l'instruction, flux
`noncompliant`, flux `waived` — avec le motif, la règle en cause et son texte
pédagogique ; un demandeur doit voir celles de ses propres demandes ; un flux
`noncompliant` doit être acquittable comme une anomalie (REQ-002-36) (F17, UC-10,
EB-27, N37).

### F. Droits et API

#### REQ-011-17 — Permissions
Le module doit déclarer, portées sur l'entité du flux : `flow.read` ;
`flow.request` (demander un flux sans lecture des extrémités, explication réduite
comme REQ-010-25, lecture de ses propres demandes) ; `flow.review` (instruire) ;
`flow.waiver.grant` (accorder une dérogation) ; `flow.write` (maintenance directe,
qui exige la lecture des deux extrémités) ; l'instructeur et l'auteur d'une dérogation
doivent être distincts du demandeur (F12, Q17).

Acceptation :
- Un compte de service avec `flow.request` sur E, agissant pour un demandeur sans
  compte kartenn, crée une demande vers un réseau qu'il ne lit pas ; l'audit enregistre
  le compte et le demandeur (N31).
- Un utilisateur avec `flow.write` seul, sans lecture du réseau destination : même
  404 qu'un objet absent.

#### REQ-011-18 — API
Le module doit exposer, de manière additive et selon la clause 9 de l'ADR 0026,
`/v1/flows` (liste, filtres, `QUERY`, routes de ressource), `/v1/flows/{id}/evaluations`,
`/v1/flow-nonconformities`, et les flux d'un objet ; l'instruction passe par les routes
des changesets de la 004 (approbation, rejet motivé) ; `PATCH /v1/flows/{id}` répond 202
avec le changeset en attente pour une prolongation à instruire (F15, F20).

### G. Performance

#### REQ-011-19 — Budgets
Le module doit respecter (p95 serveur, 10⁶ objets, 10⁵ flux, politique du banc 010) :
demande machine × machine 16 × 16, triage compris, < 300 ms ; réévaluation de 10⁴ flux
dans un changeset < 3 s ajoutées ; réévaluation de 10⁵ flux par un changement de
politique dans le budget exceptionnel de la 003 (< 60 s) ; page filtrée de flux ou de
non-conformités < 150 ms ; réévaluation complète en tâche de fond < 5 min (F14).

## Non-objectifs

- Flux constatés : import des exports de pare-feux et d'infrastructure as code, audit a
  posteriori des flux déjà ouverts (v0.3, sorte réservée).
- Campagnes de revue comme objet propre (v0.3) ; la v0.2 livre les filtres.
- Atteignabilité effective (D20 étape 2), observation du trafic, flux sans trafic.
- Services exposés par une machine (D29, plus tard).
- Génération de règles de pare-feu à partir du registre.
- Connecteurs vers le ticketing et la revue de code (v0.3, hors kartenn) ; la v0.2 en
  pose le contrat (pré-vérification 010, demande pour le compte de, événements).
- Interface Web : plugin `web/plugins/flow/` (007/W6).

## Questions ouvertes

- [x] Durées maximales par défaut d'une instruction (12 mois) et d'une dérogation
  (6 mois) : réglables ; valeurs confirmées par l'utilisateur le 2026-10-21 (F5, F6).

---
status: approved
updated: 2026-10-23
---

# 017 — Plugins Politique et Flux : exigences

## Résumé

Écrans des modules Politique (010) et Flux (011), livrés comme deux plugins de première
partie du shell (007/W6) : `web/v2/plugins/policy/` pour l'équipe sécurité qui écrit et
éprouve la politique, `web/v2/plugins/flow/` pour le demandeur qui dépose un flux et pour
l'instructeur qui le tranche. Ils s'appuient sur la surface REST servie au 2026-10-22
et sur trois amendements de module décidés avec eux : `If-Match` de la matrice
(010/S35), demandes en attente lues comme des flux (011/F19), prolongation instruite
(011/F20). Les questions Q-017-1 à 7 sont arbitrées
([`decisions.md`](decisions.md)) ; les écrans suivent des maquettes validées par
l'utilisateur (U14).

*Amendement accepté le 2026-10-23* : les maquettes et les plugins de la 017 s'appuient
sur le système de design de la [020](../020-systeme-de-design/requirements.md) et le shell
v2 de la [021](../021-shell-v2/requirements.md) : Vue 3 et `@kartenn/ui` par l'import
map, DOM léger, maquettes au catalogue avec les vrais composants (U2 et U14 remplacées ;
[`decisions.md`](decisions.md), § « Amendement — socle 020 et 021 »). La matrice écrit par
le brouillon du shell (plus d'ensemble propre au plugin), les deux files d'instruction
sont liées, chaque plugin apporte une tuile à l'Accueil.

Décisions : [`decisions.md`](decisions.md) (U1–U14, Q-017-1 à 7) ; 007/W4 à W14 ; 010/S15, S28, S29,
S31, S33, N36 ; 011/F5, F6, F10, F12, F17. Exigences servies : REQ-010 et REQ-011
(table de traçabilité en fin de document). Précédents : plugins `vlan` (008, tâche 17)
et `machines` (009, tâche 13).

### Vocabulaire

- **Responsable sécurité** : utilisateur qui détient `policy.rule.admin` (et, selon
  l'objet, les autres `policy.*.admin`, `policy.inference.admin` pour l'inférence) ;
  écrit la politique et lit le rapport d'audit.
- **Demandeur** : utilisateur qui détient `flow.request` (et `policy.flow.request`
  pour la pré-vérification) sur l'entité de la demande.
- **Instructeur** : utilisateur qui détient `flow.review` (et, pour une dérogation,
  `flow.waiver.grant`) sur l'entité, distinct du demandeur.
- **Lecteur des flux** : utilisateur qui détient `flow.read` sur l'entité.
- **Requête de conformité** : `QUERY /v1/verdicts` (ou `GET` à paramètres), sans
  écriture (REQ-010-38).
- **Brouillon** : celui du shell (021) ; les changements de politique y sont ajoutés
  (`policy.matrix.set` avec `if_match`, opérations des collections), simulés, puis
  soumis en un seul changeset avec un motif (Q-017-1 amendée, 021/X36).
- **Explication réduite** : réponse `access: reduced` d'un verdict, ou évaluation de flux
  dont les adresses et l'élément décisif sont `null` (REQ-010-25).
- Conventions EARS : comme dans
  [`002/requirements.md`](../002-noyau-adressage/requirements.md#conventions) ; « vue »,
  « onglet », « action », « modification rapide », « brouillon » : comme dans
  [`007/requirements.md`](../007-web-shell-plugins/requirements.md).

## Exigences

### A. Plugins

#### REQ-017-1 — Plugin `policy`
Où le shell web (007) est livré, kartenn doit fournir le plugin de première partie
`policy` (manifeste `"module": "policy"`) et ne le publier que si le module `policy`
figure dans `modules` des capacités. [U1, REQ-007-11, REQ-010-27]

Acceptation :
- Build avec `policy` : `ui_plugins` liste `policy` ; build sans : absent.
- Le manifeste déclare les vues, onglets et actions du design (« Plugin `policy` »),
  chacun avec sa permission.

#### REQ-017-2 — Plugin `flow`
Où le shell web est livré, kartenn doit fournir le plugin de première partie `flow`
(manifeste `"module": "flow"`), publié seulement si le module `flow` est actif, et qui
n'importe rien du plugin `policy`. [U1, REQ-007-11, REQ-011-18]

Acceptation :
- Build avec `policy` sans `flow` : `policy` publié, `flow` absent.
- Le bundle de `flow` ne contient aucun module de `web/v2/plugins/policy/`.

#### REQ-017-3 — Forme et langue
Les deux plugins doivent être écrits en Vue 3 avec `@kartenn/ui` et le SDK 1.0 publics,
partagés par l'import map, rendre en DOM léger, livrer leurs catalogues FR et EN avec les
mêmes clés, et traduire les erreurs de l'API par leur `type`, codes visibles. [U2
amendée, U6, REQ-021-25 à 27, REQ-007-29]

Acceptation :
- Un test compare les clés FR et EN de chaque plugin.
- Une violation `policy.text.missing` s'affiche par son libellé traduit, `detail` en repli.

### B. Écrire la politique (responsable sécurité)

#### REQ-017-4 — Règles
Le plugin `policy` doit lister les règles (`GET /v1/policy-rules`, datée, filtrable par
clé, effet, motif, type de flux et service), montrer une règle (sélecteurs source et
destination par axe, contraintes « même valeur », sélecteur de flux, effet, motifs
requis, motif d'appartenance, textes par langue, période) et son historique. [REQ-010-8,
26, U4]

Acceptation :
- Une règle `deny admin dev → prod` se lit « interdire, source `{environment: dev}`,
  destination `{environment: prod}`, flux type `admin` » avec son texte.
- Avec `valid_at` antérieur à sa création, la règle est absente de la liste.

#### REQ-017-5 — Écrire une règle ou un motif
Quand le responsable sécurité crée, modifie ou clôt une règle ou un motif, le plugin
doit passer par la route de ressource (motif du changeset obligatoire, `If-Match` de la
lecture), exiger un texte pédagogique avant l'envoi d'une règle `deny` ou d'un motif, et
placer les violations du serveur (`policy.rule.flow_selector_empty`,
`policy.rule.unknown_axis`, `policy.rule.conditional_deny`, `policy.text.missing`,
`policy.reference.closed`) sur les champs. [REQ-010-8, 9, 33, REQ-007-25, U5]

Acceptation :
- Règle `deny` sans texte : bouton d'envoi refusé côté client ; forcée, le 422 du serveur
  place `policy.text.missing` sur le champ texte.
- Deux onglets modifient la même règle : le second reçoit 412 et garde sa saisie.

#### REQ-017-6 — Vue matricielle
Le plugin `policy` doit afficher, pour un type de flux choisi, la matrice environnement
source × environnement destination à la date du contexte (`GET /v1/policy-matrix`) :
chaque cellule `allow`, `deny`, `conditional` (avec ses motifs) ou vide (refus par
défaut), et le nombre de règles hors vue (`outside`) avec un lien vers les règles du type de
flux, celles hors vue marquées (une règle est dans la vue si une cellule la cite). [REQ-010-34, U9, N36]

Acceptation :
- Une règle multi-axes du type `admin` n'apparaît pas dans la matrice et incrémente le
  décompte affiché.
- Chaque état se distingue par un texte ou une icône nommée, pas par la seule couleur.

#### REQ-017-7 — Écrire la matrice
Quand le responsable sécurité change l'état d'une cellule, le plugin doit exiger les
motifs d'une cellule `conditional` et le texte d'une cellule `deny`, et ajouter le
changement au **brouillon du shell** comme opération `policy.matrix.set` portant l'ETag
de la matrice lue (`if_match`), soumise avec le brouillon ; Ctrl+Entrée applique une seule
cellule par `PUT /v1/policy-matrix/{source}/{flow_type}/{destination}` sous `If-Match`,
motif demandé ; une réponse 202 doit mener au changeset en attente ; un 412 ou un refus
`policy.matrix.stale` doit relire la matrice en gardant le brouillon. [REQ-010-34, 20,
U9 ; Q-017-1 amendée, Q-017-7 ; 010/S35 ; 021/X15, X36]

Acceptation :
- Écrire `(prod, file, global)` en `conditional` avec le motif `object-storage-relay` :
  relue, la cellule rend `conditional` et son motif.
- `none` sur une cellule existante clôt sa règle ; la cellule redevient vide.
- Une autre cellule du même type de flux écrite entre la lecture et l'envoi : 412 (ou
  `policy.matrix.stale`), la matrice relue, le brouillon intact et resoumis sur le
  nouvel ETag.

#### REQ-017-8 — Différentiel avant soumission
Tant que le brouillon du shell contient des changements de politique, le plugin `policy`
doit montrer en pied de matrice, à partir de la dernière simulation du brouillon
(`POST /v1/changesets?simulate=true`, lancée par le shell), et le panneau du brouillon
doit montrer aussi : le décompte des combinaisons qui
s'ouvrent, se ferment ou passent sous réserve (`policy.combinations`) avec leurs
premières lignes et l'élément décisif avant et après ; les nœuds reclassés et leur nombre
d'héritiers (`policy.reclassified`) ; et, où le module `flow` est présent, les flux
déclarés qui deviennent non conformes à la matrice de flux, redeviennent conformes ou
changent d'issue (`flow.flows`). [REQ-010-18, 19,
REQ-011-13, UC-7 ; Q-017-1 amendée, 021/X36]

Acceptation :
- Ajouter `allow admin global → dev` avec 3 services `admin` : la simulation annonce 3
  combinaisons `denied → allowed`.
- Retirer une autorisation dont dépendent 120 flux : 120 flux `allowed → denied` annoncés
  avant toute écriture.

#### REQ-017-9 — Changement de politique en attente
Le plugin `policy` doit offrir, pour un changeset en attente ou appliqué, la lecture
paginée de ses sections `policy.combinations`, `policy.reclassified` et `flow.flows`
(`GET /v1/changesets/{id}/diff?section=`), pour que l'approbateur d'une politique
d'approbation (`policy.opens`, `policy.closes`, `flow.nonconforming`) voie ce qu'il valide.
[REQ-010-18, 20, REQ-011-6, 13 ; Q-017-1]

Acceptation :
- Un changeset retenu par `policy.opens ≥ 1` : la vue montre ses combinaisons ouvertes ;
  la page suivante suit `links.next`.

#### REQ-017-10 — Catalogue
Le plugin `policy` doit lister et montrer services (couples transport/ports, type de
flux), types de flux et environnements, et en permettre la création, la modification
(description d'un environnement) et la clôture à qui détient l'administration
correspondante. [REQ-010-2, 4, 5 ; Q-017-5]

Acceptation :
- Service à plus de 1 024 ports d'un transport : l'avertissement
  `policy.service.wide_ports` est affiché, l'écriture aboutit.
- Clore un type de flux qu'un service cite : refus, liste des objets qui le citent.

#### REQ-017-30 — Axes et valeurs
Le plugin `policy` doit lister et montrer les axes (propriété, porteurs, visé ou non par
l'inférence) et leurs valeurs (libellé, description, transverse, rang), et permettre à
qui détient `policy.axis.admin` de désigner, modifier et retirer un axe et de décrire
ses valeurs ; les valeurs elles-mêmes (la liste fermée) restent déclarées par la
propriété du noyau. [REQ-010-29, 30 ; Q-017-5]

Acceptation :
- Désigner une propriété liste fermée comme axe porté par la machine : relu, l'axe
  l'indique ; retirer un axe qu'une règle cite : refus, objets qui le citent listés.
- Donner un rang aux valeurs de sensibilité : relues dans l'ordre du rang.

#### REQ-017-31 — Conditions, signaux, rôles exemptés
Le plugin `policy` doit lister, montrer, créer, modifier et clore les conditions
d'admissibilité (sélecteur, exigence, textes), les signaux de risque (sorte,
paramètres, poids) et les rôles exemptés (création et clôture), sous
`policy.rule.admin`, avec les mêmes parcours d'écriture que les règles. [REQ-010-26,
36, 37 ; Q-017-5 ; U5]

Acceptation :
- Condition sans texte pédagogique : envoi refusé côté client ; forcé,
  `policy.text.missing` placé sur le champ.
- Modifier le poids d'un signal (ajouté au brouillon) : la simulation du brouillon annonce les flux dont le
  score change (section `flow.flows`, si `flow` est actif).

#### REQ-017-32 — Règles d'inférence
Le plugin `policy` doit lister, montrer (avec historique et révision du jeu), créer,
modifier et clore les règles d'inférence : axe visé, valeurs attendues, critères pris
dans la liste fermée du serveur, exceptions imbriquées (profondeur au plus 4), priorité
explicite, domaine cité ; sous `policy.inference.admin` (sur le domaine cité, ou
global). [REQ-010-40 à 46, 50 ; Q-017-5]

Acceptation :
- Exception de profondeur 5 : refusée côté client ; forcée, `policy.inference.depth`
  placée sur l'exception.
- Expression régulière invalide : `policy.inference.regex_invalid` sur le critère.
- Règle qui cite un domaine hors du périmètre de l'utilisateur : 403 affiché sans perte
  de saisie.

#### REQ-017-33 — Attendus et écarts d'inférence
Le plugin `policy` doit offrir sur les fiches `address` et `prefix` une action « Attendu
d'inférence » qui montre, pour un axe visé, le ou les attendus et leur explication
(règle et chaîne d'exceptions, révision, nœud de correspondance, critères et valeurs
observées, règles écartées) par les routes de suggestion et d'explication du noyau ;
et une vue des écarts (anomalies `policy.inference.mismatch` : nœud d'origine, décompte
des héritiers en écart), acquittables. [REQ-010-45, 47 à 49, 51]

Acceptation :
- Deux candidats à égalité : tous affichés, aucun retenu, aucun écart signalé.
- Acquitter un écart : il quitte la liste des écarts ouverts.

#### REQ-017-34 — Aperçu d'une règle d'inférence
Avant d'écrire une règle d'inférence, le plugin `policy` doit montrer les nœuds qui
lui correspondraient et les écarts qu'elle ouvrirait ou fermerait. [REQ-010-48, 49 ;
Q-017-5]

Route : celle de l'amendement 010/S36 (accepté le 2026-10-23), servie par la tâche
021-31, sans écriture.

Acceptation :
- Règle `prod` sur un préfixe de 12 nœuds dont 3 déclarés `dev` : aperçu de 12 nœuds et
  de 3 écarts ouverts, sans écriture.

### C. Éprouver la politique

#### REQ-017-11 — Requête de conformité
Le plugin `policy` doit permettre de poser une requête de conformité : extrémités
(adresse, préfixe, interface ou machine), objet (service, couple transport/port, ou type
de flux), dossier facultatif, entité et dates ; et rendre la synthèse (verdict, issue,
décomptes), puis par évaluation : paire de points, service, verdict (`allowed`,
`allowed_pending`, `denied`), issue, élément décisif, combinaison, règles écartées,
conditions et leur état, signaux et score, textes pédagogiques. [REQ-010-11 à 16, 36 à
38, U4]

Acceptation :
- Machine A (2 adresses) vers machine B (3 adresses), `ssh` : 6 évaluations et leur
  synthèse.
- `tcp/9999` sans service : `denied`, `unknown_service`, issue `review`, invitation à
  nommer le service.
- Sans dossier : les conditions sur le dossier sont « non évaluées » et l'issue n'est pas
  `automatic`.
- Problème `policy-query-too-large` (422) : message traduit qui invite à restreindre les
  extrémités.

#### REQ-017-12 — Explication complète ou réduite
Quand la réponse est `access: full`, le plugin doit montrer pour chaque point et axe les
valeurs, leur source (`tree` ou `machine`), le nœud d'origine et la chaîne ; quand elle
est `reduced`, il doit montrer seulement les rangs des points, les valeurs, règles,
motifs et conditions, sans aucune lecture complémentaire ; dans les deux cas, il doit
afficher le décompte `hidden` s'il n'est pas nul. [REQ-010-12, 25, U8]

Acceptation :
- Un demandeur sans lecture de l'arbre voit « point 1 → point 2 » et les valeurs
  d'environnement, aucun identifiant d'adresse, aucune requête vers `/v1/addresses`.

#### REQ-017-13 — Vérifier depuis une fiche
Le plugin `policy` doit offrir sur les fiches `address` et `prefix` une action « Vérifier
un flux » qui ouvre la requête de conformité avec la source pré-remplie. [REQ-010-13, U3]

Acceptation :
- Action absente pour qui ne détient ni `policy.verdict.read` ni `policy.flow.request`.

#### REQ-017-14 — Rapport d'audit
Le plugin `policy` doit permettre au responsable sécurité de demander un rapport daté
(`POST /v1/policy-reports`), de lister les rapports (`GET /v1/policy-reports`, tâche
021-30), d'en suivre l'état (`pending`, `running`, `done`), d'en
afficher le décompte par rubrique et le détail paginé (`…/items?rubric=`), et de
l'exporter en JSON ; un 409 (rapport en cours) doit mener au rapport en cours.
[REQ-010-22, 23]

Acceptation :
- Rubrique « non classifiés » de 12 nœuds : 12 lignes, chacune liée à la fiche du nœud.
- Une rubrique `null` (non servie par la build) est affichée « indisponible », pas zéro.

### D. Demander un flux (demandeur)

#### REQ-017-15 — Formulaire de demande et pré-vérification
Le plugin `flow` doit offrir un formulaire de demande (source, destination, service,
références externes — au moins une, système pris dans `core.external_system` —,
justification facultative, « pour le compte de » facultatif, responsables facultatifs,
nature des données et données personnelles facultatives, entité, début et fin
facultative) et, avant l'envoi, la pré-vérification de la même demande par la requête de
conformité avec son dossier, affichée comme en REQ-017-11. [REQ-011-1, 2, REQ-010-38,
UC-1 ; Q-017-2]

Acceptation :
- Sans référence externe : envoi refusé côté client ; forcé, `flow.reference.missing`
  sur le champ.
- Une plage choisie comme extrémité : refusée (`flow.endpoint.invalid`).
- La pré-vérification n'écrit rien (aucun changeset créé).

#### REQ-017-16 — Envoi et triage
Quand le demandeur envoie la demande, le plugin doit l'envoyer par `POST /v1/flows` avec
une clé d'idempotence gardée jusqu'à la réponse ; sur 201, montrer le flux `active` et
ses évaluations ; sur 202, montrer que la demande part à l'instruction, son issue
(`review` ou `noncompliant`), l'élément décisif et le texte pédagogique de la
pré-vérification, et le lien vers le changeset en attente. [REQ-011-21, REQ-007-26,
UC-2]

Acceptation :
- Demande conforme : flux `active` affiché.
- Demande sous réserve : « à instruire », motif attendu et son texte.
- Coupure réseau pendant l'envoi puis nouvel essai : un seul changeset.

### E. Lire les flux

#### REQ-017-17 — Liste des flux
Le plugin `flow` doit lister les flux à la date du contexte (`GET /v1/flows`) avec les
filtres de REQ-011-14 portés par l'URL, l'état dérivé, la synthèse, l'issue et la
dérogation ; « Mes demandes » est la même liste filtrée sur `requester` = l'utilisateur,
demandes en attente comprises (`state=requested`, 011/F19). [REQ-011-5, 14, 17, U4 ;
Q-017-6]

Acceptation :
- Lien `?state=noncompliant&service=<ssh>` rejoué dans un autre onglet : même liste.
- Filtres `machine=` et `endpoint=interface:<id>` acceptés (flux d'une machine, d'une
  interface).
- Une demande en attente du demandeur figure dans « Mes demandes », état `requested`,
  sans lien de fiche de flux (pas encore d'identifiant), avec le lien vers son
  changeset.

#### REQ-017-18 — Fiche d'un flux
Le plugin `flow` doit montrer un flux : champs, période, état, évaluation de
l'enregistrement et évaluations courantes par paire (`GET /v1/flows/{id}/evaluations`,
réduites pour le demandeur sans `flow.read`), dérogation, historique ; à toute date
du contexte. [REQ-011-16, 23, U8]

Acceptation :
- Flux rendu `noncompliant` le 1er mai par une règle `deny` : au 30 avril `active`, au
  2 mai `noncompliant` avec la règle et son texte.

#### REQ-017-19 — Onglet « Flux »
Le plugin `flow` doit ajouter aux fiches `address` et `prefix` un onglet « Flux » qui
liste les flux dont l'objet est extrémité ou dont une extrémité le contient
(`GET /v1/{addresses,prefixes}/{id}/flows`), et aux fiches de machine et d'interface
(types de module) un onglet « Flux » par les filtres `machine=` et `endpoint=interface:`.
[REQ-011-15, U3, 021/X40]

Acceptation :
- Un flux vers `10.5.0.0/24` apparaît dans l'onglet de l'adresse `10.5.0.7`.

### F. Instruire (instructeur)

#### REQ-017-20 — File d'instruction
Le plugin `flow` doit lister à l'instructeur les demandes et prolongations en attente
qu'il peut instruire (`GET /v1/flows?review=pending`, 011/F19), avec nature (demande ou
prolongation), demandeur, entité, extrémités, service, issue, élément décisif, score et
avertissements de l'évaluation du triage, sans recalcul ni lecture par changeset, et ne
pas lui offrir d'instruire ses propres demandes ou prolongations ; sur les lignes
d'« À instruire » du shell issues d'une demande ou d'une prolongation, il doit apporter
l'action « Instruire dans Flux » (type `changeset`), qui remplace « Valider » et ouvre
la fiche d'instruction. [REQ-011-5, 21, 22, UC-3 ; Q-017-3 ; 021/X37]

Acceptation :
- Une demande de l'instructeur lui-même figure sans bouton de décision.
- La file de 100 demandes se charge en une requête par page (aucune lecture par
  changeset).

#### REQ-017-21 — Approuver ou rejeter
Quand l'instructeur approuve une demande ou une prolongation, le plugin doit exiger une
date de fin d'au plus 12 mois après la décision (plafond `review_max_months`) et, si le verdict est refusé, une dérogation
(motif, risque accepté, fin au plus tôt de celle du flux et de la date d'octroi plus 6
mois), puis valider le changeset avec les opérations de revue `approve_review` et
`waive` ; quand il rejette, exiger un motif et rejeter le changeset. Les refus du serveur
(`flow.separation`, `flow.review.end_required`, `flow.waiver.invalid`, 403) doivent
s'afficher sans perte de saisie. [REQ-011-9, 22, U10]

Acceptation :
- Approbation à 13 mois : refusée côté client ; forcée, `flow.review.end_required`.
- Rejet motivé : la non-conformité apparaît dans la liste du demandeur avec le texte de
  la règle.

#### REQ-017-22 — Dérogation et recertification
Le plugin `flow` doit permettre à qui détient `flow.waiver.grant` d'accorder ou de
recertifier la dérogation d'un flux refusé (`POST /v1/flows/{id}/waiver` : motif, risque
accepté, fin), et montrer qu'une recertification réévalue le flux contre la politique du
jour. [REQ-011-9, UC-4]

Acceptation :
- Dérogation au-delà de 6 mois ou par le demandeur : `flow.waiver.invalid` affichée.

#### REQ-017-23 — Échéances
Le plugin `flow` doit lister les flux datés et les dérogations dont la fin tombe dans les
30 jours, distinguer ceux à 7 jours, et offrir depuis chaque ligne la recertification
d'une dérogation ou la prolongation du flux (REQ-017-25). [REQ-011-7, U11 ; Q-017-4 ;
011/F20]

Acceptation :
- Un flux qui finit dans 6 jours apparaît marqué « J-7 » ; un flux sans fin n'apparaît
  jamais.

#### REQ-017-24 — Non-conformités
Le plugin `flow` doit lister les non-conformités (`GET /v1/flow-nonconformities`) avec
les filtres entité, demandeur, règle, période, état et sorte, le motif, la règle en cause
et son texte pédagogique dans la langue de l'utilisateur ; un demandeur doit y voir les
siennes ; un flux `noncompliant` doit pouvoir être acquitté (anomalie
`flow.verdict.denied`, `POST /v1/anomalies/{id}/acknowledgement`). [REQ-011-25, UC-10,
U6]

Acceptation :
- Texte rédigé en français, utilisateur en anglais : texte français, langue indiquée.

#### REQ-017-25 — Modifier, prolonger ou clore un flux
Le plugin `flow` doit permettre de modifier les références, la justification, les
responsables, la nature des données d'un flux, d'avancer sa fin, de le prolonger
(`PATCH`) et de le clore (`DELETE`), sous `If-Match` ; une prolongation doit montrer
avant l'envoi la pré-vérification de la portion ajoutée (requête de conformité avec le
dossier du flux), puis, sur 202, qu'elle part à l'instruction avec le lien vers son
changeset. [REQ-011-4, 22 ; 011/F20 ; Q-017-4]

Acceptation :
- Changer une extrémité ou le service n'est pas proposé ; le plugin offre « nouvelle
  demande pré-remplie ».
- Prolonger un flux instruit dont la portion ajoutée est sous réserve : 202, la fiche
  garde l'ancienne fin et signale la prolongation en attente.
- « Prolonger » est absent d'un flux `expired` ou `withdrawn` (seule la nouvelle demande
  pré-remplie est offerte).

### G. Transverse

#### REQ-017-26 — Temps et droits
Les deux plugins doivent suivre `valid_at` et `as_of` du contexte en lecture, n'offrir
d'écriture qu'au présent, masquer ce que `GET /v1/me` dit hors des droits, et afficher
tout 404 d'une extrémité ou d'un flux comme « absent ou non autorisé ». [REQ-007-20, 24,
U4, U7]

Acceptation :
- `as_of` passé : aucun bouton d'écriture, matrice et verdicts relus à cette date.

#### REQ-017-27 — Accessibilité
Les deux plugins doivent satisfaire WCAG 2.2 AA : matrice navigable au clavier (flèches
entre cellules, en-têtes annoncés), formulaires étiquetés, états jamais portés par la
seule couleur ; axe-core sans violation dans chaque parcours. [REQ-007-33, U12]

Acceptation :
- Parcours de la matrice au clavier seul : choisir une cellule, changer son état, simuler,
  soumettre.

#### REQ-017-28 — Parcours de bout en bout
kartenn doit fournir deux parcours Playwright (Chromium et Firefox) sur un vrai serveur :
le responsable sécurité (règle, matrice, différentiel, requête de conformité, rapport) et
le couple demandeur-instructeur (demande conforme, demande à instruire, approbation avec
dérogation, rejet, non-conformité). [REQ-007-34, 37]

Acceptation :
- Les deux parcours passent dans `k-web-e2e`, axe-core à chaque étape.

#### REQ-017-29 — Budgets
Les deux plugins doivent tenir les budgets de REQ-021-30 (dont ≤ 50 Ko gzip par plugin
hors modules partagés, INP ≤ 200 ms) : matrice de 50 × 50 environnements et page de 100
flux rendues en moins de 100 ms après la réponse de l'API. [REQ-021-30]

Acceptation :
- Mesures dans `bench-web` (021, tâche 22).

#### REQ-017-35 — Tuiles de l'Accueil
Chaque plugin doit apporter au plus une tuile en lecture seule à l'Accueil du shell
(point `widget`) : `policy`, écarts d'inférence ouverts et dernier rapport d'audit ;
`flow`, demandes à instruire, échéances à 7 jours, demandes en attente de l'utilisateur ;
filtrées par les droits de la vue liée, relues par le flux d'événements. [021/X38]

Acceptation :
- Un demandeur sans `flow.review` ne voit pas la ligne « à instruire » ; un
  utilisateur sans `policy.catalogue.read` ne voit pas la tuile `policy`.

## Traçabilité vers la 010 et la 011

| Exigence servie | Exigences 017 |
|---|---|
| REQ-010-2, 4, 5 (environnements, types de flux, services) | 10 |
| REQ-010-8, 9, 33 (règles, motifs, textes) | 4, 5 |
| REQ-010-11 à 16, 36 à 38 (verdicts, issue, dossier) | 11, 12, 13, 15 |
| REQ-010-18, 19 (différentiels) | 8, 9 |
| REQ-010-20 (déclencheurs d'approbation) | 7, 9 |
| REQ-010-22, 23 (rapport d'audit) | 14 |
| REQ-010-24, 25 (droits, explication réduite) | 12, 26 |
| REQ-010-26 (versionnement, lecture datée) | 4, 26 |
| REQ-010-26, 29, 30, 36, 37 (axes, valeurs, conditions, signaux, rôles exemptés) | 30, 31 |
| REQ-010-34 (vue matricielle) | 6, 7 |
| REQ-010-40 à 51 (inférence) | 32, 33, 34 (34 par l'amendement 010/S36) |
| REQ-011-1, 2 (contenu, extrémités) | 15 |
| REQ-011-4 (modification, prolongation) | 23, 25 |
| REQ-011-5, 14, 15 (état, demandes en attente, filtres, flux d'un objet) | 17, 19, 20 |
| REQ-011-7 (échéances) | 23 |
| REQ-011-9 (dérogation) | 21, 22 |
| REQ-011-13 (différentiel des flux) | 8, 9 |
| REQ-011-16, 23 (évaluations datées, figées) | 18 |
| REQ-011-17 (permissions) | 18, 20, 26 |
| REQ-011-21, 22 (triage, instruction) | 16, 20, 21 |
| REQ-011-25 (non-conformités) | 24 |

Non couvertes par l'interface : REQ-010-1, 3, 17, 21 (classification et obligations :
panneau des propriétés et administration du shell) ; REQ-010-35, 53 (import hors ligne) ;
REQ-011-3, 8, 10 à 12, 20, 24 (règles du serveur, rendues par les écrans ci-dessus) ;
REQ-010-28, 52, 54, REQ-011-19 (valeur par défaut d'inférence, budgets serveur).

## Non-objectifs

- Toute route nouvelle ou tout changement de contrat hors des amendements 010/S35, S36,
  011/F19, F20 (U13) et des tâches serveur de la 021 (021-28 à 31).
- Le rendu des sections de module dans le panneau du brouillon : fourni par le shell
  (021/X36).
- Fiches shell des objets de module (machine, interface, objets de politique) (U3).
- Classification des nœuds : faite par le panneau des propriétés du shell (003, 007).
- Notifications d'échéance : événements sortants du serveur (U11).
- Campagnes de revue, flux constatés (v0.3, 010 et 011).

## Questions ouvertes

- [x] Q-017-1 à Q-017-7 ([`decisions.md`](decisions.md), « Questions arbitrées ») :
  arbitrées par l'utilisateur le 2026-10-22.
- [x] Écart de la 011 relevé en rédigeant (Q-017-3) : REQ-011-5 promettait l'état
  `requested`, que la lecture des flux ne rendait pas ; corrigé par 011/F19 (tâche
  011-12).
- [x] Amendement de la 010 pour l'aperçu d'une règle d'inférence (REQ-017-34) : 010/S36,
  accepté le 2026-10-23 ; servi par la tâche 021-31.

# 024 — Retours sur les maquettes : journal des décisions

Statut : **brouillon du 2026-10-24**, à valider. Écrit à partir de la revue des maquettes
E1 à E10 par l'utilisateur (2026-10-24) et des arbitrages du même jour (prototype des
panneaux `/_ui/panneaux`, questions posées en séance). Les questions ouvertes sont en fin
de journal.

Cadre : D11 (changesets), D14 (budgets), D36 (RBAC) ; 004/C1 (l'auteur n'approuve pas),
REQ-004-20, 40, 41, 42 ; 006/H19 (ETag) ; 007/REQ-007-26, 27 (brouillon local, amendées) ;
020 (système de design), 021 (shell v2) ; 022 (flux SSE) ; 023 (CLI, `--dry-run`).

Règle permanente de l'utilisateur : rien n'est gardé pour compatibilité avant le gel.

## Synthèse

- M1 périmètre : quatre ajouts au serveur (brouillons, statistiques, proposition
  volontaire, simulation de toute écriture) et une révision des specs 020 et 021 avant la
  reprise des maquettes. Les maquettes révisées repassent en validation (020, tâche 15).
- M2 brouillons côté serveur, ressource à part (`/v1/drafts`), visibles de leur seul auteur.
- M3 un brouillon devient changeset à la soumission ; l'historique et l'audit des
  changesets ne voient jamais les brouillons.
- M4 tableau de bord : `GET /v1/stats`, filtré par les droits, enrichi par les modules.
- M5 proposition volontaire : retirée (M7 la rend ordinaire).
- M6 simulation de toute écriture : `?simulate=true` sur chaque route d'écriture.
- M7 deux chemins de droits par ressource : `edit` (créer, soumettre), `apply` (valider
  le changeset d'un autre), `apply_own` (valider le sien) ; amende C1 ; les politiques
  priment.
- M8 panneaux : un seul panneau à droite, onglets Détail et Brouillon ; compteur du
  brouillon toujours visible, bascule du panneau.
- M9 soumission : motif façon commit et bouton scindé, action par défaut selon la
  simulation.
- M10 accueil en tableau de bord modulaire ; l'arbre passe dans « Topographie ».
- M11 thème clair entièrement clair.
- M12 tables : en-têtes alignés par construction, tri et filtre partout, squelettes animés.
- M13 un seul panneau d'édition par sorte d'objet, où qu'on l'ouvre.
- M14 droits manquants au survol des actions grisées, plus d'encart permanent.
- M15 stabilité de la mise en page : emplacements fixes, redimensionnement testé.
- M16 messages d'erreur en langage courant, code à côté.
- M17 évolution notée : recherche d'erreurs et spec de documentation (hors 024).

## Décisions

### M1 — Périmètre
La revue du 2026-10-24 juge les maquettes « plutôt jolies », valide le thème sombre, les
icônes et E2, E3, E8, E9 sur le fond, et demande une révision de structure : cohérence du
thème clair, panneau de droite unique, tableau de bord, tables solides au
redimensionnement. Quatre demandes touchent le contrat du serveur (M2, M4, M5, M6) ; le
reste révise les specs 020 et 021. La 024 porte les deux, comme la 018 portait le serveur
de ses retours.

### M2 — Brouillons côté serveur
Arbitré le 2026-10-24 (« Côté serveur »). L'utilisateur doit pouvoir enregistrer un
brouillon, y revenir plus tard, changer d'onglet ou de poste sans le perdre. Le brouillon
de la 007 (IndexedDB par identité et par onglet, vidé à la déconnexion) ne le permet pas.

Choix : une ressource `drafts` à part, plutôt qu'un état `draft` des changesets. Raisons :
le cycle de vie des changesets (004, sept états) et leur historique bitemporel restent
intacts ; un brouillon se modifie sans cesse, un changeset jamais ; les droits diffèrent
(seul l'auteur lit un brouillon, alors qu'un changeset se lit selon le périmètre). La
question posée parlait d'un « changeset à l'état draft » : l'écart est signalé en
Q-024-1.

Un brouillon a un titre, des opérations au format `Operation` du contrat (W10), un motif
facultatif, une version (ETag, If-Match pour chaque modification, plusieurs onglets
possibles) et une date de dernière modification. Il n'est lu, modifié, simulé, soumis ou
supprimé que par son auteur. Un compte de service n'en a pas : il soumet directement.

### M3 — Soumission
`POST /v1/drafts/{id}/submission` crée le changeset par le pipeline ordinaire, sous les
droits de l'auteur au moment de la soumission : 201 appliqué, 202 en attente (politique,
proposition), 422 refusé. Le brouillon est supprimé quand le changeset est créé (201 ou
202) ; il reste en place en 422, avec les violations. Clé d'idempotence comme
`POST /v1/changesets`.

Audit : création, soumission et suppression d'un brouillon sont auditées ; les
modifications ne le sont pas (bruit, et rien n'est appliqué). Le changeset soumis porte
l'identifiant du brouillon d'origine.

### M4 — Statistiques de l'accueil
Arbitré le 2026-10-24 (« Route de statistiques »). `GET /v1/stats` rend en un appel :
nombre d'objets par sorte, changesets appliqués sur 7 et 30 jours (et par jour sur 30
jours), propositions à instruire par l'appelant, ses changesets en attente, anomalies
ouvertes ; chaque module ajoute ses compteurs sous sa clé (VLAN, machines, demandes de
flux en attente…). Tout compteur est filtré par les droits de lecture de l'appelant : un
objet illisible n'est pas compté. ETag ; invalidation par un événement SSE `stats`
regroupé (au plus un toutes les 30 s).

### M5 — Proposition volontaire (retirée)
Proposée puis retirée le 2026-10-24 : avec M7, un changeset soumis attend toujours une
validation ; demander un second regard, c'est soumettre sans `apply=own`.

### M6 — Simulation de toute écriture
Arbitré le 2026-10-24 (`--dry-run` de la CLI). `?simulate=true` est accepté sur chaque
route d'écriture (POST, PATCH, PUT, DELETE des ressources), avec la réponse de
`POST /v1/changesets?simulate=true` (200 `{valid, violations, diff, sample, cascaded}`) ;
rien n'est écrit, aucun événement, une ligne d'audit de lecture. Lève le dernier xfail des
parcours de la CLI.

### M7 — Deux chemins de droits : édition et application
Arbitré par l'utilisateur le 2026-10-24 ; **amende C1** (004), REQ-004-21, 41, 42 et le
catalogue des permissions (005/K12, 018/R2).

Toute modification passe par un changeset. Deux chemins de droits, par ressource (le
périmètre des liaisons s'applique comme aujourd'hui) :

- **Édition** `<ressource>.edit` : créer un changeset qui touche la ressource et le
  soumettre. Un changeset soumis est **en attente** ; il ne s'applique pas seul.
- **Application** `<ressource>.apply` : valider (appliquer) ou rejeter le changeset
  **d'un autre** qui touche la ressource.
- **Application de ses propres changesets** `<ressource>.apply_own` : valider son propre
  changeset. C'est la permission de l'équipe qui maintient le référentiel ; les autres
  équipes (sécurité…) n'ont que `edit` et proposent.

Les trois remplacent `…write`, `…propose` et `core.changeset.approve`, sans
compatibilité. Un changeset qui touche plusieurs ressources demande le droit sur chacune
(comme aujourd'hui pour `write`). Un brouillon (M2) ne demande aucun droit : il est privé
et n'applique rien ; la simulation suit les droits de lecture.

Soumettre et valider soi-même s'enchaînent en une requête atomique
(`POST /v1/changesets?apply=own`, et `POST /v1/drafts/{id}/submission?apply=own`) : une
transaction, deux lignes d'audit (soumission, validation), le changeset marqué
« appliqué par son auteur » dans l'historique. L'interface en fait l'action par défaut de
qui détient `apply_own` (M9), avec un motif générique modifiable.

Les politiques d'approbation priment (arbitré) : un changeset qu'une politique retient
attend la validation d'un autre que l'auteur, même si l'auteur détient `apply_own` ; pour
que l'équipe réseau n'y soit pas soumise, la politique se cible par auteur
(REQ-004-40). La notice `changeset.approval.self` reste pour ce cas et pour qui n'a pas
`apply_own`.

Conséquence : la « proposition volontaire » (M5) n'a plus d'objet, c'est la soumission
ordinaire.

### M8 — Panneaux
Arbitré sur prototype le 2026-10-24 (disposition B). Le détail de la sélection et le
brouillon partagent un panneau à droite, en surimpression non modale de la surface (la
table garde sa largeur), à onglets « Détail » et « Brouillon n ». Le compteur du brouillon
de la barre d'état est toujours visible et fait bascule : il ouvre le panneau sur l'onglet
Brouillon, ou le ferme s'il y est déjà. L'onglet Brouillon passe devant les autres
contenus du panneau. La barre latérale et le panneau se replient. Le panneau qui coupait
la surface en deux (E1, E7, E10) disparaît.

### M9 — Soumission depuis le panneau
Arbitré le 2026-10-24. Pied du panneau Brouillon : résultat de la simulation en une
ligne ; motif en deux champs (résumé sur une ligne, description facultative), à la manière
d'un formulaire de commit ; bouton scindé dont l'action par défaut suit les droits et la
simulation (« Appliquer » = soumettre puis valider soi-même, pour qui détient `apply_own`
et si aucune politique ne retient le changeset ; « Soumettre en revue » sinon) et dont le
chevron choisit « Appliquer », « Soumettre en revue », « Enregistrer le brouillon ». Une
action impossible reste visible, grisée, avec son motif (permission manquante ou politique
qui retient). Ctrl+Entrée lance l'action choisie. Pour « Appliquer », un motif générique
est prérempli (« Maintenance du référentiel ») et reste modifiable.

### M10 — Accueil et Topographie
L'accueil devient un tableau de bord modulaire (tuiles du cœur et `widget` des plugins,
M4) ; l'arbre des préfixes et le contenu d'un nœud passent dans une vue « Topographie ».
Ajout direct depuis l'arbre (« + » sur un nœud) ; le menu « Créer » reste pour un ajout
dans plusieurs réseaux.

### M11 — Thème clair
Le thème clair est clair partout : barre latérale, barres, panneaux. Fait le 2026-10-24
(jetons `--k-sidebar-*`). Le thème sombre ne change pas.

### M12 — Tables
L'en-tête et les cellules partagent la même définition de colonnes (une seule grille, ou
`<table>` avec `table-layout: fixed`) : aucun décalage possible. Toute table se trie et se
filtre. Les cellules en chargement montrent un squelette animé (sauf mouvement réduit).
Une table à double entrée (matrice de politique) a son composant : en-tête de colonnes et
première colonne de même nature, figées au défilement.

### M13 — Un panneau d'édition par sorte
Ajouter ou modifier une adresse, un préfixe, une plage… ouvre le même panneau, qu'on vienne
de l'arbre, d'une table, de la palette ou du menu.

### M14 — Droits manquants
Plus d'encart permanent « droits nécessaires » (E4) : une action interdite reste visible,
grisée, et son infobulle nomme la permission manquante. La notice
`changeset.approval.self` reste.

### M15 — Stabilité de la mise en page
Les emplacements (barre latérale, barre d'état, surface, panneau) sont fixés par le cadre ;
un contenu long (violations, E5) défile dans son emplacement sans le déformer. Le
sélecteur de période s'ouvre en surimpression (E6). Les barres d'outils se replient en menu
« … » plutôt que de passer à la ligne. Chaque maquette est capturée à 1440, 1280, 1024 et
768 px ; une capture montre la ligne d'en-tête et les barres d'outils à chaque largeur.

### M16 — Messages d'erreur
Une erreur s'affiche en langage courant, son code à côté (« L'objet a changé depuis votre
lecture » plutôt que « Condition If-Match non remplie », code `core.precondition.failed`).

### M17 — Évolution notée
Coller une erreur dans la palette pour obtenir son explication et des ressources ;
documentation et guides : une spec à part, plus tard.

## Questions

Toutes tranchées le 2026-10-24.

- **Q-024-1** Brouillon : ressource à part `/v1/drafts` ou état `draft` des changesets ?
  → **ressource à part** (M2).
- **Q-024-2** Rétention d'un brouillon inactif → **90 jours**, suppression annoncée 7
  jours avant dans l'interface.
- **Q-024-3** Limites → **50 brouillons** par identité, **10 000 opérations** par
  brouillon.
- **Q-024-4** Un administrateur et les brouillons d'un autre (départ d'une personne) →
  **lire et reprendre** : permission globale nouvelle `core.draft.admin` pour lister,
  lire, supprimer et s'attribuer les brouillons d'une autre identité, chaque action
  auditée, lecture comprise (REQ-024-7a). Un brouillon n'est donc privé que vis-à-vis des
  autres utilisateurs, pas de cet administrateur ; l'interface le dit à la création du
  premier brouillon.
- **Q-024-5** Compteurs « changements récents » → **tous les changesets lisibles**
  (choix peu coûteux, tranché sans question).
- **Q-024-6** Permissions de propriété → **mêmes trois verbes** (`prop.<clé>.edit`,
  `prop.<clé>.apply`, `prop.<clé>.apply_own`).

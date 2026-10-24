---
status: draft
updated: 2026-10-24
---

# 024 — Retours sur les maquettes : exigences

Statut : **brouillon du 2026-10-24**, à valider (questions Q-024-1 à 6).

## Résumé

Suite de la revue des maquettes E1 à E10 (2026-10-24). Côté serveur : des brouillons
gardés par le serveur, deux chemins de droits (édition, application) qui remplacent
`write`, `propose` et `core.changeset.approve`, une route de statistiques pour l'accueil,
la simulation de toute écriture. Côté interface : une révision des specs 020 et 021
(panneau unique à droite, soumission façon commit, accueil en tableau de bord, thème clair
entièrement clair, tables alignées et triables, stabilité au redimensionnement), puis la
reprise des maquettes, qui repassent en validation.

Décisions : [`decisions.md`](decisions.md) (M1–M17, Q-024-1 à 6) ; D11, D14, D36 ;
004/C1 (amendée), REQ-004-20, 21, 40, 41, 42 (amendées) ; 005/K12, 018/R2 (catalogue
amendé) ; 006/H19 ; 007/REQ-007-26, 27 (amendées) ; 020, 021 (révisées) ; 022 ; 023.

### Vocabulaire

- **Brouillon** : ensemble d'opérations privé, gardé par le serveur, qui n'applique rien.
- **Soumission** : création d'un changeset à partir d'opérations ; le changeset soumis
  est en attente.
- **Application** : validation d'un changeset en attente, qui l'applique (REQ-004-22).
- **Auto-application** : application de son propre changeset, sous `apply_own`.
- Conventions EARS : comme dans
  [`002/requirements.md`](../002-noyau-adressage/requirements.md#conventions).

## Exigences

### A. Brouillons gardés par le serveur

#### REQ-024-1 — Ressource
kartenn doit garder les brouillons d'une identité humaine : titre, opérations au format
`Operation` du contrat, motif facultatif, version, dates de création et de dernière
modification ; routes `GET`/`POST /v1/drafts`, `GET`/`PATCH`/`DELETE /v1/drafts/{id}`,
`POST /v1/drafts/{id}/operations`. [M2, Q-024-1]

#### REQ-024-2 — Seul l'auteur
kartenn doit refuser toute lecture ou écriture d'un brouillon par une autre identité que
son auteur (404, l'existence n'est pas révélée), sauf la reprise prévue par
REQ-024-7a ; un compte de service n'a pas de brouillon. [M2]

#### REQ-024-3 — Version
kartenn doit exiger `If-Match` sur toute modification d'un brouillon et répondre 412
quand la version a changé (deux onglets, deux postes). [M2, 006/H19]

#### REQ-024-4 — Simulation
kartenn doit simuler un brouillon (`POST /v1/drafts/{id}/simulation`) comme
`POST /v1/changesets?simulate=true` : validation, politiques, différentiel, sous les
droits de l'auteur au moment de la simulation. [M2]

#### REQ-024-5 — Soumission
`POST /v1/drafts/{id}/submission` doit créer un changeset par le pipeline ordinaire avec
les opérations et le motif du brouillon, et une clé d'idempotence ; le brouillon doit
disparaître quand le changeset est créé, et rester inchangé en 422 avec les violations ;
le changeset porte l'identifiant du brouillon d'origine. [M3]

#### REQ-024-6 — Audit et événements
kartenn doit auditer la création, la soumission et la suppression d'un brouillon, pas ses
modifications ; il doit émettre l'événement SSE `draft` (identifiant et version, sans
contenu) vers les seuls flux de l'auteur. [M3, 022]

#### REQ-024-7 — Rétention et limites
kartenn doit supprimer un brouillon inactif depuis 90 jours, garder au plus 50 brouillons
par identité et 10 000 opérations par brouillon (422 au-delà), et annoncer la suppression
7 jours avant (champ `expires_at`). [Q-024-2, 3]

#### REQ-024-7a — Reprise par un administrateur
kartenn doit permettre à qui détient la permission globale `core.draft.admin` de lister
les brouillons d'une autre identité, de les lire, de les supprimer et de se les
attribuer (le brouillon change d'auteur et sort de la liste de l'ancien) ; chacune de
ces actions, lecture comprise, doit être auditée avec l'identité d'origine. [Q-024-4]

#### REQ-024-8 — Brouillon du shell
Le shell doit garder son brouillon sur le serveur (REQ-024-1) au lieu d'IndexedDB ; le
brouillon actif d'un onglet est retenu par l'onglet (sessionStorage), le plus récent
par défaut ; la déconnexion ne le supprime pas. [M2, amende REQ-007-26, 27 et
REQ-021-11]

### B. Deux chemins de droits

#### REQ-024-9 — Permissions d'édition et d'application
kartenn doit offrir, pour chaque ressource qui avait `…write` et `…propose`, trois
permissions : `….edit` (créer et soumettre un changeset qui la touche), `….apply`
(appliquer ou rejeter le changeset d'un autre qui la touche), `….apply_own` (appliquer
son propre changeset) ; `…write`, `…propose` et `core.changeset.approve` disparaissent,
sans compatibilité ; les rôles existants sont migrés (`write` → `edit` + `apply_own`,
`propose` → `edit`, `approve` → `apply`) par un changeset système. [M7, amende 005/K12,
018/R2, Q-024-6]

#### REQ-024-10 — Soumission
Quand l'auteur détient `….edit` sur tout ce que le changeset touche (objets et effets),
kartenn doit l'enregistrer en attente ; sinon le refuser (403). Aucun changeset ne
s'applique sans application explicite. [M7, amende REQ-004-41]

#### REQ-024-11 — Application
kartenn doit appliquer un changeset en attente (`POST …/{id}/validation`) quand celui qui
valide détient `….apply` sur tout ce qu'il touche et n'en est pas l'auteur, ou en est
l'auteur et détient `….apply_own` sur tout ce qu'il touche ; la revalidation de
REQ-004-22 s'applique. [M7, amende C1, REQ-004-21, 42]

#### REQ-024-12 — Les politiques priment
Quand une politique d'approbation retient un changeset (REQ-004-20, 40), kartenn doit
refuser son application par son auteur (`changeset.approval.self`), même sous
`….apply_own`. [M7]

#### REQ-024-13 — Soumettre et appliquer en une requête
`?apply=own` sur `POST /v1/changesets` et sur `POST /v1/drafts/{id}/submission` doit
soumettre puis appliquer le changeset dans une seule transaction, avec deux événements
d'audit (soumission, application) ; quand l'application est refusée (droit absent,
politique), la réponse doit être 202 (changeset en attente) avec le motif, jamais un
changeset perdu. L'historique marque le changeset « appliqué par son auteur ». [M7]

### C. Statistiques de l'accueil

#### REQ-024-14 — Route
`GET /v1/stats` doit rendre : le nombre d'objets lisibles par sorte, le nombre de
changesets appliqués sur 7 et 30 jours et par jour sur 30 jours, les changesets en
attente que l'appelant peut appliquer, ses propres changesets en attente, les anomalies
ouvertes lisibles ; chaque module y ajoute ses compteurs sous sa clé. [M4, Q-024-5]

#### REQ-024-15 — Droits
kartenn ne doit compter que ce que l'appelant peut lire. [M4, D36]

#### REQ-024-16 — Fraîcheur et budget
`GET /v1/stats` doit porter un ETag ; kartenn doit émettre l'événement SSE `stats` au plus
une fois toutes les 30 s quand un compteur a changé ; p95 ≤ 200 ms au banc complet. [M4,
D14]

### D. Simulation de toute écriture

#### REQ-024-17 — `?simulate=true` partout
Chaque route d'écriture de l'API (POST, PUT, PATCH, DELETE des ressources et actions)
doit accepter `?simulate=true` et répondre 200
`{valid, violations, diff, sample, cascaded}` sans rien écrire ni émettre d'événement ;
la CLI s'en sert pour `--dry-run`. [M6, 023]

### E. Interface (révision des specs 020 et 021)

#### REQ-024-18 — Thème clair
Le thème clair doit être clair sur toutes les surfaces (barre latérale, barres, panneaux,
menus). [M11]

#### REQ-024-19 — Panneau unique à droite
Le shell doit montrer le détail de la sélection et le brouillon dans un panneau à droite,
non modal, en surimpression de la surface, à onglets « Détail » et « Brouillon n » ; le
compteur du brouillon de la barre d'état doit rester visible et basculer le panneau sur
l'onglet Brouillon ; la barre latérale et le panneau doivent se replier. [M8, amende
REQ-021-1, 6]

#### REQ-024-20 — Soumission façon commit
Le pied de l'onglet Brouillon doit montrer le résultat de la simulation, un motif en deux
champs (résumé, description facultative) et un bouton scindé : action par défaut selon
les droits et la simulation, chevron pour « Appliquer », « Soumettre en revue »,
« Enregistrer le brouillon » ; une action impossible reste visible, grisée, avec son
motif ; Ctrl+Entrée lance l'action choisie ; « Appliquer » préremplit un motif générique.
[M9]

#### REQ-024-21 — Brouillons enregistrés
Le shell doit lister les brouillons de l'utilisateur, en reprendre un, en créer un autre
et en supprimer un. [M2]

#### REQ-024-22 — Accueil et Topographie
L'accueil doit être un tableau de bord de tuiles (cœur et `widget` des plugins) nourri par
`GET /v1/stats` ; l'arbre des préfixes et le contenu d'un nœud doivent passer dans la vue
« Topographie », avec un ajout direct sur un nœud de l'arbre. [M10, amende REQ-021-9, 13]

#### REQ-024-23 — Tables
Toute table doit aligner son en-tête sur ses colonnes par construction, se trier et se
filtrer sur ses colonnes, et montrer un squelette animé dans les cellules en chargement
(sans animation sous mouvement réduit) ; une table à double entrée doit avoir son
composant, en-tête et première colonne figés et de même nature. [M12]

#### REQ-024-24 — Un panneau d'édition par sorte
Le shell doit ouvrir le même panneau d'édition pour une sorte d'objet, d'où que vienne
l'ajout ou la modification. [M13]

#### REQ-024-25 — Droits manquants au survol
Une action interdite doit rester visible, grisée, et nommer au survol et au focus la
permission manquante ; aucun encart permanent des droits. [M14]

#### REQ-024-26 — Stabilité de la mise en page
Un contenu long doit défiler dans son emplacement sans le déformer ; les menus et
sélecteurs (période) s'ouvrent en surimpression ; une barre d'outils trop étroite replie
ses actions dans un menu « … » au lieu de passer à la ligne ; aucune page ne défile
horizontalement de 1024 à 1920 px. [M15]

#### REQ-024-27 — Captures à plusieurs largeurs
Chaque maquette et chaque écran doivent être capturés à 1440, 1280, 1024 et 768 px, en
clair et en sombre. [M15, amende REQ-020-26]

#### REQ-024-28 — Messages d'erreur
Une erreur doit s'afficher en langage courant, son code à côté. [M16]

#### REQ-024-29 — Maquettes révisées
Les maquettes E1 à E10 doivent être reprises selon REQ-024-18 à 28 (plus une maquette de
l'accueil) et revalidées par l'utilisateur avant les composants complets (020 P4). [M1]

## Non-objectifs

- Auto-application sous une politique qui retient le changeset (REQ-024-12).
- Partage d'un brouillon entre identités.
- Recherche d'erreurs dans la palette, documentation et guides : spec à venir (M17).

## Questions ouvertes

Q-024-1 à 6 : [`decisions.md`](decisions.md), § « Questions ».

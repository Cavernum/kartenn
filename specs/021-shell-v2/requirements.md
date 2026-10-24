---
status: approved
updated: 2026-10-24
---

# 021 — Shell v2 : exigences

## Résumé

Réécriture de la **partie client** de la 007 sur le système de design de la 020 : nouvelle
mise en page (barre latérale repliable groupée par espace, palette de commandes Ctrl+K,
fil d'Ariane, barre d'état haute avec pastille de date et « Brouillon (n) »), raccourcis
clavier complets et découvrables, écrans du référentiel, des changesets et revues, du
brouillon, de l'administration et des plugins réécrits avec `@kartenn/ui`, édition en
place selon les droits, filtres typés, clés et libellés, codes visibles. Le SDK passe en
**version 1.0** (réécriture qui repart à 1.0, sans couche de compatibilité avec la 0.1 de la 007) : cache de données partagé (`@tanstack/query-core`), préchargement à
l'intention, invalidation par le flux SSE de la 022, optimisme au présent seulement ; le
framework et `@kartenn/ui` sont partagés avec les plugins par l'import map. Les budgets
du navigateur s'étendent (INP, navigation en cache, CLS, trames perdues, mémoire) et
restent bloquants.

La 007 garde le **serveur** (service du shell, en-têtes, CSP, répertoire des plugins, SRI,
capacités, rechargement à chaud, émetteur de développement, ajouts à l'API) ; sa partie
client est remplacée ou amendée par cette spec (design, § « Ce qui reste de la 007 »).

Sources : arbitrages de l'utilisateur du 2026-10-23 (D-1 à D-12, brief, direction) ;
retours sur les maquettes du 2026-10-22 ; D7 (amendée), D8, D10, D11, D17, D31 (amendée),
D36 ([`../001-cadrage/decisions.md`](../001-cadrage/decisions.md)) ; specs 007, 017,
020, 022 (validée), 006 (H19, ETag composite) et 018 (non fusionnée : permissions
`module.action`, libellés par langue) ; ADR 0025,
[0037](../../docs/adr/0037-couche-de-donnees-du-client.md),
[0038](../../docs/adr/0038-partage-des-composants-avec-les-plugins.md) ; note
[`web-navigateur.md`](../../docs/reference/web-navigateur.md).

**Validée le 2026-10-23** : l'utilisateur a tranché tous les arbitrages et toutes les
questions (Q-021-1 à 14) de [`decisions.md`](decisions.md), et les décisions de la planche
des squelettes d'écrans (X35 à X42). Le framework est tranché (D-1, 2026-10-23, après
prototype : [`005-framework-web.md`](../../docs/spikes/005-framework-web.md)) : **Vue 3**,
avec Reka UI encadré (020, REQ-020-33) ; D17 et l'ADR 0011 inchangées. Rien n'est
implémenté.

Ordre (D31 amendée) : la 022 précède ; la 020 fournit jetons, composants et maquettes
validées avant tout écran ; la 017 (Politique, Flux) vient après, sur ce socle.

### Vocabulaire

- **Espace** : groupe d'entrées de la barre latérale : Référentiel, Changesets, Réseau,
  Machines, Politique, Flux, Administration (en dernier) ; les espaces des modules sont
  déclarés par les manifestes des plugins.
- **Palette** : dialogue de commandes ouvert par Ctrl+K (⌘K sur macOS).
- **Pastille de date** : bouton de la barre d'état qui montre le contexte temporel
  (« Présent » ou la date) et ouvre le sélecteur de période.
- **Hors présent** : `valid_at` ou `as_of` posé dans l'URL (007/W9).
- **Édition en place** : modification d'une cellule de table ou d'un nœud d'arbre sans
  quitter la vue.
- **Modification rapide** : changeset d'une opération par la route de ressource, avec
  motif et `If-Match` (REQ-007-25).
- **Requête** (du cache) : lecture identifiée par une clé, gardée, dédupliquée et
  revalidée par le `QueryClient` du SDK.
- **Préchargement à l'intention** : lecture lancée au survol, au focus ou à l'approche d'un
  lien, avant le clic.
- **Optimisme** : mise à jour de l'affichage avant la réponse du serveur, défaite en cas
  d'échec.

## Exigences

### A. Mise en page

#### REQ-021-1 — Cadre
Le shell doit présenter : une barre latérale à gauche, une barre d'état en haut (fil
d'Ariane, pastille de date, « Brouillon (n) », menu de l'utilisateur), une surface de
travail et un panneau à droite (REQ-021-6) ; la barre latérale et le panneau se replient,
et le cadre fixe leurs emplacements : un contenu long défile dans le sien sans le
déformer (024/REQ-024-26) ; les repères ARIA `navigation`, `banner`, `main` et un lien
d'évitement vers le contenu. `index.html` doit contenir le squelette statique de ce cadre
(feuille en fichier, aucune règle en ligne), remplacé sans décalage par l'application.
[X1, maquette E1, REQ-020-23, 024/REQ-024-19, 26]

Acceptation :
- CLS ≤ 0,05 du premier rendu à l'écran utile ; aucun écran blanc pendant le chargement.
- Le lien d'évitement est le premier élément focalisable.

#### REQ-021-2 — Barre latérale
La barre latérale doit grouper ses entrées par espace, accueillir les entrées des plugins
dans l'espace que déclare leur manifeste, se replier en icônes (avec noms accessibles et
infobulles) et garder son état localement ; elle doit afficher les compteurs utiles
(propositions à instruire, demandes à instruire, brouillon, anomalies), relus par le
flux d'événements, jamais par sondage. Les espaces, dans cet ordre, sont `referential`,
`changes`, `network`, `hosts`, `policy`, `flows`, `admin` ; un espace sans entrée visible
(module absent ou droits) est masqué. [X2, REQ-020-16]

Acceptation :
- Un plugin qui déclare l'espace `policy` apparaît sous « Politique » ; sans module
  correspondant, l'espace est absent.
- Un demandeur de flux sans autre droit ne voit que les espaces qui lui montrent une
  entrée (Flux, et la conformité de Politique).
- Repliée puis rechargée, la barre reste repliée.

#### REQ-021-3 — Palette de commandes
Ctrl+K (⌘K) doit ouvrir la palette depuis tout écran : navigation vers chaque vue,
actions de la vue courante (créer, modifier, ouvrir le brouillon, revenir au présent),
recherche d'objets par valeur (adresse, préfixe) et par nom, objets récents ; les plugins
doivent pouvoir y ajouter leurs commandes par le SDK ; une commande hors des droits est
absente. [X3, REQ-007-20]

Acceptation :
- Ctrl+K, « 10.1. », Entrée → la fiche du préfixe ou de l'adresse correspondante.
- Une commande « Créer un groupe de VLAN » est absente sans `vlan.group.edit`.

#### REQ-021-4 — Fil d'Ariane
Chaque écran doit montrer son fil d'Ariane (espace, collection, objet et ses ancêtres
pertinents), dont chaque élément sauf le dernier est un lien. [X4]

#### REQ-021-5 — Contexte temporel dans la barre d'état
La pastille de date doit montrer « Présent » ou la date affichée (avec fuseau) et ouvrir
le sélecteur de période (`valid_at`, `as_of`). Hors présent, une **bande ambre** doit
traverser la page sous la barre d'état, nommer le contexte (`role="status"`) et offrir le
retour au présent. Deux cas, selon 007/W9 :
- **date de validité autre que le présent** (`valid_at` seul) : la bande dit « Lecture au
  <date> — brouillon daté possible » et **ne dit pas** que les écritures sont
  désactivées ; l'édition en place et la modification rapide sont désactivées, le
  brouillon reste permis avec `valid_from` prérempli à `valid_at` ;
- **état connu à une date système passée** (`as_of`) : la bande dit « État connu au
  <date> — lecture seule » ; toute écriture est désactivée (édition en place,
  modification rapide, brouillon et sa soumission).
Chaque contrôle désactivé dit pourquoi (infobulle). [X5, REQ-007-22, 23, W9]

Acceptation :
- `as_of` passé : bande « lecture seule », boutons d'écriture désactivés avec motif,
  soumission du brouillon refusée par le shell.
- `valid_at` futur seul : la bande ne contient pas « écritures désactivées » ; « Ajouter au
  brouillon » est actif et préremplit `valid_from`.
- « Revenir au présent » retire `valid_at` et `as_of` de l'URL.

#### REQ-021-6 — Panneau à droite et brouillon
Le détail de la sélection et le brouillon doivent partager un panneau à droite, non modal,
en surimpression de la surface (la table garde sa largeur), à onglets « Détail » et
« Brouillon n ». « Brouillon (n) » dans la barre d'état doit compter les opérations du
brouillon actif, rester visible et faire bascule : il ouvre le panneau sur l'onglet
Brouillon, ou le ferme s'il y est déjà. L'onglet Brouillon montre les opérations et la
simulation continue (différentiel, violations avec leurs codes, cascades) ; son pied
porte le résultat de la simulation, le motif en deux champs (résumé, description
facultative) et un bouton scindé : action par défaut selon les droits et la simulation
(« Appliquer » pour qui détient `apply_own` quand aucune politique ne retient le
changeset, « Soumettre en revue » sinon), chevron pour « Appliquer », « Soumettre en
revue », « Enregistrer le brouillon » ; une action impossible reste visible, grisée, avec
son motif ; Ctrl+Entrée lance l'action choisie ; « Appliquer » préremplit un motif
générique modifiable. [X6, 024/REQ-024-19, 20, M8, M9]

Acceptation :
- Ajouter une opération depuis une table met à jour le compteur sans recharger la vue ;
  le compteur ouvre puis referme le panneau ; Échap le ferme et rend le focus.
- Avec `apply_own` et sans politique qui retient : l'action par défaut est « Appliquer »
  et le changeset sort appliqué ; sans `apply_own` : « Appliquer » grisé, la permission
  manquante nommée, l'action par défaut est « Soumettre en revue ».

### B. Raccourcis

#### REQ-021-7 — Jeu de raccourcis
Le shell doit offrir : Ctrl+K (palette), `/` (champ de filtre ou de recherche de la vue),
`g` puis une lettre (navigation, table du design), `c` (créer dans la collection
courante), `e` (modifier l'objet ou la cellule courante), `?` (aide des raccourcis),
Échap (fermer la couche ouverte) ; chaque raccourci doit être affiché dans les menus, les
infobulles des boutons et l'aide `?`, et localisé. Les plugins peuvent déclarer des
raccourcis de leur vue, sans redéfinir ceux du shell. [X7, brief tour 3]

Acceptation :
- `g` puis `c` → liste des changesets ; `?` → aide qui liste tous les raccourcis actifs,
  ceux du plugin courant compris.

#### REQ-021-8 — Raccourcis sans conflit
Un raccourci à une touche ne doit s'activer ni quand le focus est dans un champ éditable,
ni dans un composant qui consomme les caractères (recherche à la saisie d'un arbre,
édition de cellule), ni avec une touche de modification autre que Maj ; aucun raccourci ne
doit utiliser une combinaison réservée aux lecteurs d'écran ou au navigateur (Insert,
Verr. Maj, Ctrl+Alt+flèches, F6, Ctrl+L, Ctrl+T…) ; un réglage doit permettre de
désactiver les raccourcis à une touche. [X8, WCAG 2.1.4]

Acceptation :
- Saisie de « gc » dans un champ de filtre : aucune navigation.
- Raccourcis à une touche désactivés : `g c` ne fait rien, Ctrl+K reste actif.

### C. Écrans

#### REQ-021-9 — Référentiel
Le shell doit offrir les écrans du référentiel : un **accueil** en tableau de bord de
tuiles (tuiles du cœur et `widget` des plugins) nourri par `GET /v1/stats` ; la vue
**Topographie** : arbre des préfixes (arbre en grille : préfixe, occupation, colonnes de
plugins) et contenu du nœud choisi, avec un ajout direct sur un nœud ; domaines, plages, adresses, entités, sites ;
fiche d'objet (attributs, propriétés résolues et leur explication, occupation et prochain
libre d'un préfixe, historique, anomalies, onglets et panneaux de plugins, menu
d'actions) ; anomalies et leur acquittement ; propriétés (déclarations, valeurs, écarts).
Chaque vue de données a ses trois états (chargement, vide, erreur). [X9, REQ-007-17,
REQ-020-23, maquettes E1 à E3, 024/REQ-024-22]

#### REQ-021-10 — Changesets et revues
Le shell doit montrer la liste filtrée des changesets, le différentiel groupé par objet
(`DiffList`), l'état, et permettre selon les droits validation, rejet, retrait,
annulation ; les propositions à instruire de l'utilisateur doivent avoir leur vue
(« À instruire ») et leur compteur. Deux files liées coexistent : « À instruire » (cœur)
et « Instruction » (plugin Flux) ; sur une ligne d'« À instruire » issue d'une demande de
flux, l'action du plugin « Instruire dans Flux » remplace « Valider », qui n'ajouterait pas
l'opération de revue du module. [X10, X37, REQ-007-18, maquette E4]

#### REQ-021-11 — Brouillons
Les brouillons sont gardés par le serveur (`/v1/drafts`, spec 024) et visibles de leur
seul auteur ; le brouillon actif d'un onglet est son identifiant. Le shell doit lister
les brouillons de l'utilisateur (menu du compteur et page `/drafts`), en reprendre un, en
créer un autre et en supprimer un, annoncer la suppression d'un brouillon inactif
(`expires_at`) et suivre l'événement `draft` du flux. La simulation reste continue
(débrayée), la soumission garde sa clé d'idempotence. [X11, 024/REQ-024-1 à 8, 21,
maquette E5]

Acceptation :
- Un brouillon commencé dans un onglet se reprend depuis un autre navigateur de la même
  identité ; un 412 à l'écriture relit le brouillon et rejoue l'ajout.

#### REQ-021-12 — Administration
Le shell doit offrir l'administration de la 007 (rôles, liaisons, émetteurs, comptes de
service et secrets montrés une fois, identités, audit et son export, imports) ; les
permissions y sont affichées sous leur nom servi (`module.action` quand la ressource est
le module, spec 018), en chasse fixe, avec leur libellé quand il existe. [X12,
REQ-007-19, maquette E7]

#### REQ-021-13 — Hôte des plugins
Le shell doit accueillir les cinq emplacements de la 007 (vue, onglet, panneau, colonne,
action) dans le nouveau cadre, et un sixième, **`widget`** : une tuile en lecture seule du
tableau de bord de l'accueil (REQ-021-9), au plus une par plugin, montrée selon
la permission de la vue liée ; une action peut viser le type `changeset` ; un onglet ou
une colonne peut viser un type d'objet de module (par exemple Interfaces sur un VLAN, Flux
sur une machine ou une interface) ; une erreur d'un plugin reste confinée à son emplacement
(encart d'erreur avec code) ; la page « À propos » liste les plugins chargés et écartés
avec leur motif. [X13, REQ-007-9, 13, maquette E10]

#### REQ-021-14 — Session
La connexion, la reconnexion sur 401 (retour à la même URL et au même contexte), la
déconnexion (effacement des préférences sensibles ; les brouillons restent au serveur,
024) suivent REQ-007-28 ;
l'approche de l'échéance de session est signalée avant qu'elle n'arrive (le flux SSE ne
compte pas comme activité, 022/A6). [X14, REQ-007-28, Q-022-11]

### D. Écriture

#### REQ-021-15 — Édition en place
Les tables et l'arbre doivent permettre l'édition en place d'une cellule quand
l'utilisateur détient `X.edit` du type et `prop.<clé>.edit` de la propriété (`/v1/me`,
verbes de la 024) et que la
vue est au présent ; par défaut, une édition ajoute une **opération au brouillon** et
marque la cellule « en brouillon » ; « Appliquer maintenant » (Ctrl+Entrée) passe par la
modification rapide (REQ-021-16, écriture avec `?apply=own`), motif demandé en ligne et
prérempli d'un motif générique. Le brouillon s'applique **en
lot, avec un seul motif**. Une cellule non éditable ne doit pas offrir l'éditeur et dit
pourquoi. [X15, retour du 2026-10-22, REQ-020-19, REQ-007-25 amendée]

Acceptation :
- Lecteur : aucune cellule éditable ; auteur : édition d'un nom de préfixe au clavier
  seul → compteur du brouillon à 1, cellule marquée.
- Ctrl+Entrée avec motif → 200 : valeur appliquée ; 202 : cellule « en revue », avec le
  motif de la réponse (`changeset.apply_own.missing`, `changeset.approval.self`).

#### REQ-021-16 — Modification rapide, optimisme et réponses
Une modification rapide au présent doit mettre l'affichage à jour avant la réponse
(optimisme) et le défaire sur erreur ; aucune mise à jour optimiste hors présent. Les
réponses doivent être rendues ainsi : 200/201 → valeur appliquée, avis avec lien vers le
changeset ; **202 → « proposé, en revue »**, jamais « appliqué », lien vers la
proposition ; **412 → relecture de l'objet, saisie gardée** et montrée à côté des valeurs
actuelles ; 422 → violations sur les champs, avec leurs codes ; 403 ou
`core.object.forbidden` → refus, saisie gardée. [X16, D-7, REQ-007-20, 25, ADR 0037]

Acceptation :
- Deux onglets modifient le même préfixe : le second reçoit 412, voit la nouvelle valeur
  et retrouve sa saisie.
- Politique d'approbation active : la ligne passe « en revue » sans jamais afficher
  « appliqué ».

#### REQ-021-36 — Un panneau d'édition par sorte
Ajouter ou modifier un objet d'une sorte (adresse, préfixe, plage…) doit ouvrir le même
panneau d'édition, construit sur le schéma de la sorte, qu'on vienne de l'arbre, d'une
table, de la palette ou d'un menu ; le menu « Créer » reste pour un ajout dans plusieurs
réseaux. [024/REQ-024-24, M10, M13]

Acceptation :
- Ajouter une adresse depuis un nœud de la Topographie et depuis la liste des adresses
  ouvre le même panneau, aux mêmes champs.

#### REQ-021-37 — Droits manquants au survol
Une action que les droits interdisent doit rester visible, grisée, et nommer au survol et
au focus la permission manquante (nom servi, spec 018) ; aucun encart permanent des droits
nécessaires. La notice `changeset.approval.self` reste. [024/REQ-024-25, M14]

Acceptation :
- Sans `core.prefix.apply_own`, « Appliquer » est grisé et son infobulle, atteinte au
  clavier, dit « Il faut `core.prefix.apply_own` ».

#### REQ-021-17 — Arbres à parent déduit
Dans l'arbre des préfixes, et dans tout arbre dont le parent découle des données
(inclusion, D34), le shell ne doit offrir **aucun** déplacement (mode `derived`,
REQ-020-31) : l'utilisateur ne choisit jamais le parent. Changer la place d'un préfixe
passe par la **modification de sa valeur**, par le brouillon (le domaine est immuable,
`core.domain.immutable` : aucune action « Changer de domaine ») ; l'arbre
est alors recalculé à partir de la réponse du serveur (changeset appliqué, ou différentiel
de la simulation du brouillon pour un aperçu marqué comme tel), jamais d'une déduction du
client. [X17, D34, retour de l'utilisateur du 2026-10-23]

Acceptation :
- Arbre des préfixes : aucun glisser, aucune action « Déplacer vers… », Ctrl+X sans effet.
- Valeur d'un préfixe changée de `10.1.2.0/24` en `10.1.0.0/23` dans le brouillon : l'arbre
  au présent reste inchangé ; l'aperçu du brouillon montre la place rendue par la
  simulation ; après application, l'arbre relu la montre.

#### REQ-021-32 — Déplacement dans les arbres à parent déclaré
Le glisser n'est offert que pour l'hébergement d'une machine (par une machine ou un
cluster) et l'ancrage d'un groupe de VLAN ; une entité ou un site change de parent par
l'action de menu « Changer de parent… », jamais par glisser, avec les seules cibles
valides et le code `core.anchor.cycle` en refus. Dans ces cas, le shell doit vérifier
chaque cible avant de la mettre en évidence ou de la proposer
(`canMove`, REQ-020-31) : pas de cycle (ni le nœud, ni un de ses descendants) ; type du
parent compatible avec celui de l'enfant selon le contrat de l'API (schéma de l'opération
de changement de parent) ; droit d'écriture sur le nœud **et** sur la cible (`/v1/me`) ;
vue au présent (aucun déplacement sous `valid_at` ou `as_of`). Une cible refusée montre
son motif, et le code de la règle serveur correspondante quand il existe (par exemple
`machines.host.invalid`) ; y déposer ne fait rien ; « Déplacer vers… » ne liste que les
cibles valides. Un déplacement accepté devient une opération du brouillon, ou, avec
Ctrl+Entrée, un changeset immédiat. [X32, retour de l'utilisateur du 2026-10-23,
REQ-020-31]

Acceptation :
- Glisser un cluster sur une de ses machines hébergées : cible en refus (cycle), dépôt sans
  effet.
- Cible sur laquelle l'utilisateur n'a pas le droit d'écriture : refus avec motif.
- Sous `as_of` : aucun glisser possible.

#### REQ-021-33 — Aucun optimisme pour un déplacement
Même au présent, un déplacement ne doit **jamais** être affiché à sa nouvelle place avant
que la simulation du changeset ou la réponse du serveur ne le confirme ; jusque-là, le
nœud reste à sa place dans l'état « déplacement en attente ». Un refus (simulation
invalide, 422, 403, 412) doit ramener visiblement le nœud à l'état normal à sa place, avec
le code du refus. Le serveur reste seul juge : un déplacement accepté par le contrôle du
client peut être refusé par le serveur, et ce refus doit être rendu sans perte. Cette
règle déroge à l'optimisme de REQ-021-16, réservé aux valeurs. [X33, D-7, retour de
l'utilisateur du 2026-10-23]

Acceptation :
- Parcours : déplacement d'une machine vers un cluster d'un autre site ; le contrôle du
  client l'accepte, le serveur le refuse (`machines.host.site_mismatch`) : le nœud n'a
  jamais quitté sa place, l'état « refusé » montre le code.
- Réponse lente simulée : pendant l'attente, le nœud est à sa place, marqué « en attente ».

#### REQ-021-34 — Invariant d'affichage des arbres
Aucune vue d'arbre ne doit afficher un nœud sous un parent que les données ne soutiennent
pas ; pour l'arbre des préfixes, aucun préfixe, plage ou adresse ne doit apparaître sous un
préfixe qui ne le contient pas. [X34, REQ-020-32, D34]

Acceptation :
- Test de propriété de bout en bout : suites aléatoires de déplacements (arbres à parent
  déclaré) et de modifications de valeurs de préfixes et de plages (par le brouillon et en
  application directe, acceptées ou refusées), puis lecture de l'arbre rendu : chaque
  ligne est contenue par sa ligne parente (comparaison CIDR des valeurs affichées) et a
  pour parent celui que rend l'API.

### E. Filtres, libellés, codes

#### REQ-021-18 — Filtres typés
Chaque liste doit offrir les filtres typés de la 020 sur les propriétés que sa route sait
filtrer (paramètres du contrat OpenAPI et propriétés déclarées) ; les valeurs d'un domaine
fermé (environnements, sites, entités, états, listes fermées de propriétés) sont proposées
par autocomplétion depuis l'API ; les filtres sont dans l'URL. [X18, REQ-020-20,
REQ-007-22]

Acceptation :
- Adresses filtrées par `site = <site>` et `core.owner = <entité>` ; le lien copié rouvre
  la même vue.

#### REQ-021-19 — Clé et libellé
Les objets ne portent que des **clés** ; le shell résout les libellés par un dictionnaire
séparé, `GET /v1/labels` (spec 018), négocié par `Accept-Language`, gardé en cache par
langue et révision, invalidé par l'événement `labels` du flux (022, amendé par la 018).
Toute valeur à clé (environnement, axe, état, type d'objet, permission, rôle) doit être
affichée par son libellé dans la langue de l'utilisateur, à défaut celui des catalogues du
shell ou du plugin, à défaut sa clé, la clé restant accessible et le code toujours visible
(REQ-020-21, 22). [X19, X42, spec 018]

Acceptation :
- Changement de libellé d'un environnement par un autre utilisateur : l'écran ouvert
  l'affiche sans recharger (événement `labels`) ; aucune lecture d'objet n'est relancée.

#### REQ-021-20 — Codes visibles
Toute erreur affichée (avis, encart, dialogue, cellule) doit dire en langage courant ce
qui s'est passé (« L'objet a changé depuis votre lecture », pas « Condition If-Match non
remplie ») et montrer à côté le code du problème et de chaque violation, et
l'identifiant de la requête, copiables (REQ-020-22) ; la phrase est la traduction du
`type` (REQ-007-29). [X20, retour du 2026-10-22, 024/REQ-024-28]

### F. Données

#### REQ-021-21 — Cache partagé
Le shell et les plugins doivent lire par le `QueryClient` unique du SDK, avec des clés
produites par les fabriques du SDK, qui portent la ressource, ses paramètres, les filtres
normalisés et le contexte temporel ; une lecture à `as_of` passé est gardée sans
revalidation ; toute autre relecture part en `If-None-Match` avec l'ETag gardé, opaque.
[X21, ADR 0037, 006/H19]

Acceptation :
- Retour sur une fiche déjà vue : aucune requête réseau si fraîche, sinon une requête
  304 et aucun squelette.

#### REQ-021-22 — Préchargement
Un lien interne doit précharger les requêtes déclarées par sa route cible au survol (après
un court délai) ou au focus ; la navigation attend au plus 100 ms puis affiche la donnée en
cache ou un squelette ; une liste précharge sa page suivante à l'approche de la fin, un
arbre les enfants d'un nœud survolé ; les modules d'un plugin sont préchargés au survol de
son entrée. [X22, D-5, ADR 0037]

Acceptation :
- Navigation vers une fiche survolée : marque de rendu ≤ 100 ms après le clic (banc).

#### REQ-021-23 — Invalidation par le flux d'événements
Le SDK doit tenir **un** flux `GET /v1/events` par navigateur, partagé entre ses onglets,
et appliquer le contrat de la 022 : ouvrir le flux avant les lectures ; `ready` et
`reset` invalident tout ; `changes` invalide chaque objet nommé dont la `version` connue
diffère de `changeset`, les listes de son type, et les requêtes qui montrent cet objet
(résumés de liés, fiches composites) ; `truncated` invalide par type et domaine ;
`changeset` rafraîchit le changeset et les propositions. Sans flux (refus, coupure
durable), le cache revalide au retour sur l'onglet, à la reconnexion et au retour du
bfcache. [X23, 022, D-6]

Acceptation :
- Deux navigateurs : un renommage dans le premier apparaît dans la fiche ouverte du second
  en moins de 2 s, sans action.
- Renommage d'un domaine : les fiches de ses préfixes ouvertes se mettent à jour (ETag
  composite, H19).
- Trois onglets ouverts : une seule connexion `/v1/events`.

#### REQ-021-24 — Invalidation de ses propres écritures
Après un changeset appliqué par l'utilisateur, le SDK doit invalider les objets nommés par
la réponse, leurs collections et leurs dépendants connus, sans attendre le flux. [X24,
ADR 0037]

### G. API des plugins (SDK 1.0)

#### REQ-021-25 — SDK 1.0
Le nouveau SDK doit être publié en version 1.0 et offrir, en plus de ce qu'offre la 0.1 (007) : le `QueryClient`
partagé, les fabriques de clés, l'abonnement aux événements, des aides d'écriture
(modification rapide avec optimisme et rendu des réponses), les brouillons du serveur
(liste, lecture, ajout d'opérations avec If-Match et nouvel essai après relecture,
simulation, soumission), les statistiques de l'accueil (`GET /v1/stats`, relues sur
l'événement `stats`), la déclaration de commandes, de raccourcis et d'entrées de barre latérale ; il reste sans
dépendance à un framework. [X25, REQ-007-12, ADR 0037, 024/REQ-024-21, 22]

Acceptation :
- Un plugin sans framework lit par le `QueryClient` et reçoit les invalidations.

#### REQ-021-26 — Modules partagés et manifeste
L'import map doit désigner `@kartenn/sdk`, `vue` (build *runtime* sans compilateur), `@kartenn/ui` et sa
feuille, chacun avec son empreinte ; le manifeste d'un plugin doit pouvoir déclarer les
plages `ui` et `framework`, vérifiées au chargement comme `sdk`, et l'espace de ses
entrées. [X26, ADR 0038, REQ-007-5, 9]

Acceptation :
- Plugin qui exige `ui: ^3` sur un shell en `@kartenn/ui` 2.x → écarté `incompatible`.

#### REQ-021-27 — Rendu des plugins
Les plugins de première partie doivent rendre en DOM léger avec `@kartenn/ui` ; un plugin
tiers peut garder une racine d'ombre et lier la feuille par `handle.styles()` ; le plugin
d'exemple reste sans framework. [X27, ADR 0038, REQ-007-21]

### H. Langue

#### REQ-021-28 — i18n
REQ-007-29 reste exigée (FR et EN, mêmes clés, `Intl`, erreurs par `type`, catalogues des
plugins) ; les noms des touches et les raccourcis sont localisés ; les libellés suivent
REQ-021-19. [X28, REQ-007-29]

### I. Accessibilité

#### REQ-021-29 — WCAG 2.2 AA
REQ-007-33 reste exigée pour tous les écrans ; en plus : parcours complet au clavier de la
palette, de la barre latérale repliée, du panneau du brouillon, de l'édition en place ;
focus jamais masqué par la barre d'état ou la bande (2.4.11) ; consultation lisible à
320 px CSS de large (1.4.10), édition réservée aux largeurs ≥ 1024 px avec un message.
[X29, REQ-007-33, REQ-020-28 ; décidé le 2026-10-23]

### J. Budgets

#### REQ-021-30 — Budgets du navigateur
`bench-web` doit mesurer et faire échouer au-delà de : shell initial ≤ 300 Ko gzip
(framework et `@kartenn/ui` compris, plugins exclus) ; chaque plugin ≤ 50 Ko gzip hors
modules partagés ; premier écran utile ≤ 2 s ; page de 100 lignes ≤ 100 ms ; **INP ≤
200 ms** au 75ᵉ centile de chaque interaction scénarisée (ouvrir un nœud, filtrer, éditer
une cellule, changer d'onglet, ouvrir la palette) ; **navigation entre routes en cache ≤
100 ms** jusqu'à la marque de rendu ; **CLS ≤ 0,05** au premier écran et à chaque
navigation ; arbre de 10⁵ nœuds défilé avec **≤ 2 % de trames perdues** et aucune tâche
longue ; Flèche bas dans l'arbre et dans une grille ≤ 100 ms au 75ᵉ centile ; **tas JS**
après 10⁴ lignes défilées et 50 navigations **≤ 64 Mo** ; plugin lent écarté à 5 s.
Mesures sur Chromium, CPU ralenti ×4, jeu à 10⁶ objets. Les seuils tirés du prototype D-1
(trames perdues, flèches, tas) se resserrent après le premier passage complet, jamais ne
se relâchent sans décision. [X30, D-10, REQ-007-32, D14, REQ-020-29]

Acceptation :
- `bench-web` : chaque mesure sous son budget, sortie non nulle sinon ; tableau rendu par
  `bench-runner`.

### K. Migration

#### REQ-021-35 — Contrat du serveur complété
Avant les écrans, le serveur doit compléter son contrat : paramètres de filtre
implémentés mais non déclarés (`prop.<clé>` et ses variantes, `within`, `usage`,
`nature`, et tout autre filtre servi) décrits dans les `parameters` de l'OpenAPI, pour
que les filtres typés se tirent du contrat ; curseurs sur `GET /v1/anomalies`,
`/v1/property-gaps` et `/v1/flow-nonconformities` ; liste `GET /v1/policy-reports` ;
route d'aperçu d'une règle d'inférence (010, amendement S36). [X41, 006, 010, 011, 016]

Acceptation :
- Test de contrat : chaque paramètre accepté par une route de liste figure dans ses
  `parameters` ; `filters.ts` généré les contient.
- Une liste d'anomalies de plus d'une page se parcourt par curseur jusqu'au bout.

#### REQ-021-31 — Bascule en un bloc
Le nouveau shell doit être écrit **complètement** avant de remplacer l'ancien : il vit sur
`main` dans un dossier séparé, `web/v2/` (espace pnpm distinct : `sdk` 1.0, `ui`,
`shell`, `plugins`, `e2e`), avec sa propre étape d'intégration continue, et **n'est pas
servi** ; pendant ce temps, l'ancien shell ne reçoit que des correctifs. Avant la
bascule, une liste de parité (chaque écran, action, extension et parcours de l'ancien
shell, avec son équivalent) doit être cochée et tous les parcours de bout en bout portés
sur le nouveau shell doivent être verts ; le jour de la bascule, **un seul commit**
branche le nouveau shell (build servi par `[web] root`, paquets Nix et OCI) et supprime
l'ancien. [X31, décidé le 2026-10-23]

Acceptation :
- Avant la bascule : `[web] root` sert l'ancien shell ; l'étape d'intégration continue de
  `web/v2/` (lint, Vitest, captures, parcours contre kartenn-server avec `[web] root`
  pointé sur le build de `web/v2/shell`) est verte.
- Commit de bascule : liste de parité entièrement cochée ; après lui, plus aucun fichier
  de l'ancien shell, de l'ancien SDK ni des anciens plugins ; parcours et `bench-web`
  verts.

## Non-objectifs

- Le serveur du shell et des plugins (service, CSP, SRI, répertoire, capacités) : la 007,
  inchangée, sauf l'entrée de l'import map.
- Les plugins Politique et Flux : la 017, sur ce socle.
- Le langage de requête écrit : Q-020-10.
- Le stockage et le versionnement des libellés par langue : la 018.
- Le rendu serveur, une application mobile, le hors-ligne.
- Une couche de compatibilité avec le SDK 0.1 (Q-021-14 : aucune ; le SDK repart à 1.0).

## Questions

Q-021-1 à 14, toutes closes le 2026-10-23 : [`decisions.md`](decisions.md), § « Questions ».

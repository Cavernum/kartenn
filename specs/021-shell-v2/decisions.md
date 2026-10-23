# 021 — Shell v2 : journal des décisions

Rédigé le 2026-10-23 à partir des arbitrages rendus le même jour par l'utilisateur (débat
web D-1 à D-12, brief, direction) et de ses retours sur les maquettes du 2026-10-22. Les
décisions X1 à X31 traduisent ces arbitrages en choix de conception ; les choix peu
coûteux à défaire sont marqués « choix de Claude ». Restent : les **arbitrages demandés**
et les **questions** Q-021-1 à 14, toutes closes le 2026-10-23, ainsi que les décisions de la
planche des squelettes (X35 à X42) : la spec est validée.

Cadre : D7 (amendée), D8, D10, D11, D17, D31 (amendée), D34, D36 de
[`../001-cadrage/decisions.md`](../001-cadrage/decisions.md) ; 007/W1 à W18 ; 017/U2,
U14 ; 022 (validée, A1 à A8) ; 006/H19 ; 020 ; ADR 0025, 0037, 0038 ; spec 018 (non
fusionnée).

## Arbitrages rendus (2026-10-23, utilisateur)

- **D-1 framework : Vue 3** (« Franchement on va prendre vue »), après le prototype
  comparatif Vue 3 + Reka UI contre React 19 + React Aria (tâche 021-1, faite ;
  [`005-framework-web.md`](../../docs/spikes/005-framework-web.md)) ; D17 et l'ADR 0011
  inchangées.
- **D-2** framework et `@kartenn/ui` par l'import map (amende l'ADR 0025).
- **D-3** DOM léger pour les plugins de première partie.
- **D-5** `@tanstack/query-core` dans le SDK, adaptateur fin selon D-1, invalidation par
  SSE.
- **D-6** tranché par la 022 (SSE, un point d'accès).
- **D-7** optimisme au présent seulement ; 202 « proposé, en revue » ; 412 relit sans
  perdre la saisie.
- **D-10** budgets bloquants : INP ≤ 200 ms p75, navigation en cache ≤ 100 ms, CLS ≤ 0,05,
  arbre de 10⁵ ≤ 5 % de trames perdues, plafond de mémoire chiffré après le prototype.
- **Brief tour 2** : barre latérale repliable, palette Ctrl+K, fil d'Ariane ; barre d'état
  haute (pastille de date, bande hors présent, « Brouillon (n) » ouvrant un panneau).
- **Brief tour 3** : raccourcis complets et découvrables (Ctrl+K, `/`, `g` + lettre, `c`,
  `e`, `?`) ; états sobres et utiles.
- **Retours du 2026-10-22** : édition en place selon les droits (par le brouillon ou le
  changeset), filtres typés, clé et libellé, codes visibles, permissions `module.action`.

## Synthèse
- X1 cadre : barre latérale, barre d'état, surface en panneaux ; squelette statique.
- X2 espaces de la barre latérale, déclarés par les plugins.
- X3 palette Ctrl+K : navigation, actions, objets, commandes des plugins.
- X4 fil d'Ariane sur chaque écran.
- X5 pastille de date et bande ambre ; règles d'écriture de W9 gardées.
- X6 « Brouillon (n) » et panneau non modal.
- X7 jeu de raccourcis, un registre, affichage partout.
- X8 raccourcis sans conflit, désactivables (2.1.4).
- X9 à X14 écrans : référentiel, changesets, brouillon, administration, plugins, session.
- X15 édition en place vers le brouillon par défaut ; Ctrl+Entrée applique.
- X16 optimisme au présent ; rendu de 202, 412, 422, 403.
- X17 aucun déplacement dans un arbre à parent déduit ; la place d'un préfixe suit sa valeur, relue du serveur.
- X18 filtres typés tirés du contrat ; URL = paramètres de l'API.
- X19 libellés : API (018), puis catalogue, puis clé.
- X20 codes et `Request-Id` dans toute erreur.
- X21 cache partagé, clés datées, ETag opaque, `as_of` immuable.
- X22 préchargement à l'intention, attente bornée à 100 ms.
- X23 un flux SSE par navigateur, contrat de la 022, dépendants, comparaison sur `version`.
- X24 invalidation de ses propres écritures sans attendre le flux.
- X25 SDK 1.0 sans framework.
- X26 import map étendue, plages `ui` et `framework`, espace au manifeste.
- X27 DOM léger en première partie ; tiers au choix ; exemple sans framework.
- X28 i18n de la 007 gardée.
- X29 WCAG 2.2 AA étendu ; consultation à 320 px, édition ≥ 1024 px.
- X30 budgets étendus, bloquants, resserrés par le prototype D-1.
- X31 bascule en un bloc : nouveau shell complet dans `web/v2/`, un commit de bascule.
- X32 déplacement à parent déclaré : cibles vérifiées (cycle, type, droits, présent) avant mise en évidence.
- X33 aucun optimisme pour un déplacement ; le serveur reste seul juge.
- X34 invariant d'affichage des arbres, test de propriété de bout en bout.
- X35 espaces : Référentiel, Changesets, Réseau, Machines, Politique, Flux, Administration.
- X36 la matrice de politique écrit dans le brouillon du shell.
- X37 deux files d'instruction liées ; actions de plugin sur le type `changeset`.
- X38 point d'extension `widget` pour l'Accueil.
- X39 parent d'entité ou de site par menu ; glisser pour l'hébergement et l'ancrage seulement ; domaine immuable.
- X40 vue Interfaces ; onglets sur les types de module.
- X41 contrat du serveur complété avant les écrans.
- X42 libellés par dictionnaire `GET /v1/labels`, invalidé par l'événement `labels`.

## X2 — Espaces
- Décision : `referential`, `changes`, `network`, `hosts`, `policy`, `flows`, `admin`
  (dans cet ordre, Administration en dernier) ; un plugin déclare `space` ; un espace sans
  entrée visible est masqué.
- Pourquoi : planche des squelettes : référentiel d'abord, travail en cours, puis les
  modules du réseau vers la sécurité ; Politique et Flux séparés (deux publics : le
  responsable sécurité ; les demandeurs et instructeurs).
- Rejeté : un seul espace « Sécurité » (neuf entrées, deux publics).
- Décidé par : l'utilisateur (2026-10-23, Q-021-5)

## X5 — Contexte temporel
- Décision : forme du brief (pastille, bande ambre) ; règles de W9 gardées :
  `valid_at` seul → lecture datée, brouillon daté permis (`valid_from` prérempli),
  édition en place et modification rapide désactivées, la bande ne dit pas « écritures
  désactivées » ; `as_of` passé → lecture seule, toute écriture désactivée.
- Pourquoi : l'écriture au futur passe par la date d'effet d'un changeset (W9).
- Décidé par : l'utilisateur (2026-10-23, Q-021-13)

## X7, X8 — Raccourcis
- Décision : table du design ; un registre unique ; un seul écouteur ; touches ignorées
  dans les champs, les composants à saisie, avec Ctrl/Alt/Méta ; réglage qui coupe les
  raccourcis à une touche.
- Pourquoi : brief tour 3 ; WCAG 2.1.4 exige de pouvoir couper un raccourci à un
  caractère, ou de ne l'activer qu'au focus ; les lecteurs d'écran en mode navigation
  consomment les lettres avant la page, mais pas en mode formulaire : la portée « hors
  champ » évite le conflit.
- Décidé par : l'utilisateur (jeu) ; règles : choix de Claude

## X15 — Édition en place
- Décision : une édition de cellule ajoute une opération au brouillon (« Brouillon (n) »),
  appliqué en lot avec un seul motif ; Ctrl+Entrée applique cette seule édition tout de
  suite par la modification rapide, motif demandé alors.
- Pourquoi : REQ-007-25 exige un motif à chaque modification rapide : à la cellule, ce
  serait une friction ; le brouillon groupe plusieurs cellules en un changeset, avec un
  motif, une simulation et une revue éventuelle.
- Rejeté : application directe à chaque cellule ; brouillon seul (pas de chemin rapide).
- Décidé par : l'utilisateur (2026-10-23, Q-021-1) ; REQ-007-25 amendée

## X17 — Arbres à parent déduit
- Décision : aucun déplacement (glisser, couper-coller, « Déplacer vers… ») dans l'arbre
  des préfixes ni dans tout arbre déduit de l'inclusion ; la place d'un préfixe change par
  sa valeur, par le brouillon ; l'arbre est relu du serveur ou montré en aperçu depuis la
  simulation, jamais recalculé par le client.
- Pourquoi : retour de l'utilisateur du 2026-10-23 : un glisser dans cet arbre peut
  afficher des conteneurs qui contiennent des plages plus grandes que ce que le préfixe
  permet ; la place découle de la valeur (D34).
- Décidé par : l'utilisateur (2026-10-23)

## X32 — Déplacement à parent déclaré
- Décision : `canMove` vérifie cycle, compatibilité de types (contrat), droits sur les deux
  bouts, présent ; cible refusée avec motif et code, dépôt sans effet ; « Déplacer vers… »
  limité aux cibles valides.
- Pourquoi : retour du 2026-10-23 ; une cible mise en évidence doit être une cible
  possible.
- Décidé par : l'utilisateur (principe) ; ordre des contrôles : choix de Claude

## X33 — Pas d'optimisme structurel
- Décision : un déplacement n'est montré à sa nouvelle place qu'après confirmation
  (simulation ou réponse) ; attente à la place d'origine ; refus visible avec code ; tests
  où le client accepte et le serveur refuse.
- Pourquoi : retour du 2026-10-23 ; un déplacement change la structure lue par d'autres
  vues (occupation, héritage) : l'annuler après coup affiche, un temps, un arbre faux.
  D-7 (optimisme au présent) reste vrai pour les valeurs.
- Décidé par : l'utilisateur (2026-10-23)

## X34 — Invariant d'affichage
- Décision : aucun nœud sous un parent que les données ne soutiennent pas ; garde du
  composant (020/B32) et test de propriété de bout en bout.
- Décidé par : l'utilisateur (2026-10-23)

## X18 — Filtres
- Décision : les paramètres de l'URL sont ceux de l'API ; le modèle des filtres est tiré
  du contrat au build (comme `dated.ts`) et des déclarations de propriétés à l'exécution.
- Pourquoi : un lien partagé est une requête de l'API (REQ-007-22) ; pas de seconde
  grammaire à maintenir.
- Décidé par : choix de Claude

## X21, X23 — Cache et flux
- Décision : ADR 0037 ; un flux par navigateur élu par `navigator.locks` et rediffusé par
  `BroadcastChannel` ; comparaison sur `version` ; index des dépendants.
- Pourquoi : 022 (R13 : un flux par navigateur) ; 006/H19 rend l'ETag opaque et composite :
  le contrat client de la 022 comparait l'ETag à `changeset`, ce qui ne tient plus.
- Décidé par : l'utilisateur (D-5, D-6) ; élection, dépendants : choix de Claude

## X30 — Budgets
- Décision : table du design ; shell initial ≤ 300 Ko gzip **framework et `@kartenn/ui`
  compris** ; plugin ≤ 50 Ko gzip hors partagés ; arbre ≤ 2 % de trames perdues (au lieu
  des 5 % de D-10) ; Flèche bas p75 ≤ 100 ms ; tas ≤ 64 Mo.
- Pourquoi : D-10 ; le partage par l'import map déplace le framework des plugins vers le
  shell ; le prototype D-1 mesure 0,2 % de trames perdues, des flèches à ≈ 15 ms et 19 à
  25 Mo de tas par écran sans ralenti : les seuils de D-10 laissaient trop de marge.
- Décidé par : l'utilisateur (D-10 ; 2 % de trames perdues confirmé le 2026-10-23) ; 50 Ko
  et périmètre des 300 Ko : choix de Claude (Q-021-11)

## X31 — Bascule en un bloc
- Décision : le nouveau shell est écrit complètement avant de remplacer l'ancien ; il vit
  sur `main` dans `web/v2/` (espace pnpm distinct, étape d'intégration continue propre,
  non servi) ; l'ancien ne reçoit que des correctifs ; liste de parité et parcours portés
  avant la bascule ; un seul commit branche le nouveau et supprime l'ancien.
- Pourquoi : jamais d'interface mêlant deux styles ; le SDK 1.0 et l'import map étendue
  sont incompatibles avec l'ancien shell et ses plugins, qui ne peuvent donc pas
  coexister dans le même espace.
- Rejeté : bascule en place par zones (recommandation initiale de Claude) ; branche
  longue hors de `main` (divergence, conflits).
- Décidé par : l'utilisateur (2026-10-23, Q-021-4)

## X35 à X42 — Planche des squelettes (2026-10-23, utilisateur)
- **X35 Espaces** : voir X2.
- **X36 Matrice** : les changements de la matrice vont au brouillon du shell
  (`policy.matrix.set` avec `if_match`) ; l'effet simulé (sections `policy.*` et
  `flow.flows`) est montré en pied de matrice et dans le panneau du brouillon ; remplace
  l'« ensemble en préparation » propre au plugin (017/Q-017-1).
- **X37 Files** : « À instruire » (cœur) et « Instruction » (Flux) coexistent, liées par
  l'action « Instruire dans Flux » sur les lignes issues d'une demande ; une action de
  plugin peut viser le type `changeset` (aussi « Effet sur la politique »).
- **X38 Widget** : nouveau point d'extension pour l'Accueil, au plus un par plugin, en
  lecture seule, filtré par la permission de la vue liée.
- **X39 Parents** : entités et sites par « Changer de parent… » (cibles valides,
  `core.anchor.cycle`), sans glisser ; glisser pour l'hébergement des machines et
  l'ancrage des groupes de VLAN seulement ; pas de « Changer de domaine » (domaine
  immuable, `core.domain.immutable`).
- **X40 Machines** : vue Interfaces (recherche par MAC, adresse, VLAN) ; onglets apportés
  sur des types de module (Interfaces sur un VLAN, Flux sur machine et interface).
- **X41 Serveur** : contrat complété avant les écrans (REQ-021-35) : paramètres de filtre
  déclarés dans l'OpenAPI, curseurs des anomalies, lacunes et non-conformités, liste des
  rapports de politique, aperçu d'une règle d'inférence (010/S36).
- **X42 Libellés** (spec 018 validée) : objets à clés seules ; dictionnaire
  `GET /v1/labels` négocié par `Accept-Language`, en cache par langue et révision,
  invalidé par l'événement `labels` ; codes toujours visibles.
- Lettres `g` : `h d p r a e s c i b x` pour le shell, `v m t q f` déclarées par les
  plugins, refusées si prises.

## Arbitrages (tranchés le 2026-10-23 par l'utilisateur)

- **A1 — 017/U2** : la 017 s'aligne ; U2 et U14 remplacées.
- **A2 — ADR 0025 et 007/W7** : amendés par l'ADR 0038, acceptée.
- **A3 — Contrat client de la 022** : le client compare `changeset` à la `version` du corps
  et invalide aussi les vues qui montrent l'objet ; design de la 022 amendé.
- **A4 — REQ-007-25** : édition en place vers le brouillon, appliqué en lot avec un
  motif ; Ctrl+Entrée applique une édition avec son motif ; REQ-007-25 amendée.
- **A5 — REQ-007-32** : remplacée par REQ-021-30 (300 Ko modules partagés compris, 50 Ko
  par plugin, bloquants).
- **A6 — Permissions** : le renommage est fait par la spec 018 (validée), côté serveur,
  avant le nouveau shell ; le shell montre les noms servis avec leurs libellés.
- **A7 — D17 et ADR 0011** : inchangées (D-1 : Vue 3).
- **A8 — 007/W17** : précisée (le cache n'est pas un magasin d'état d'application).
- **A9 — Bande hors présent** : W9 gardée ; pas d'« écritures désactivées » sous
  `valid_at` seul.

## Questions (closes le 2026-10-23)

- **Q-021-1** — Édition en place : brouillon par défaut, en lot avec un motif ;
  Ctrl+Entrée pour une édition.
- **Q-021-2** — Déplacements : glisser pour l'hébergement des machines et l'ancrage des
  groupes de VLAN ; « Changer de parent… » au menu pour entités et sites ; pas de
  « Changer de domaine » (X39).
- **Q-021-3** — Un flux SSE par navigateur (`navigator.locks` et `BroadcastChannel`).
- **Q-021-4** — Bascule en un bloc depuis `web/v2/`.
- **Q-021-5** — Espaces : X2.
- **Q-021-6** — Lettres `g` : table de la planche (X35 à X42).
- **Q-021-7** — Mémoire : 64 Mo (prototype D-1).
- **Q-021-8** — Préchargement au survol : 60 ms.
- **Q-021-9** — `staleTime` 30 s pour le présent ; lectures datées passées fraîches pour
  toujours.
- **Q-021-10** — Libellés : dictionnaire de la 018 (X42).
- **Q-021-11** — 300 Ko gzip pour le shell, modules partagés compris ; 50 Ko par plugin ;
  bloquants.
- **Q-021-12** — Lecture dès 320 px, édition dès 1024 px.
- **Q-021-13** — Brouillon daté permis sous `valid_at` seul (W9).
- **Q-021-14** — Aucune couche de compatibilité ; le nouveau SDK est versionné **1.0**
  (règle de l'utilisateur : avant le gel, on casse librement, une réécriture repart à
  1.0).

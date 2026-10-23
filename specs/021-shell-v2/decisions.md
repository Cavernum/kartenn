# 021 — Shell v2 : journal des décisions

Rédigé le 2026-10-23 à partir des arbitrages rendus le même jour par l'utilisateur (débat
web D-1 à D-12, brief, direction) et de ses retours sur les maquettes du 2026-10-22. Les
décisions X1 à X31 traduisent ces arbitrages en choix de conception ; les choix peu
coûteux à défaire sont marqués « choix de Claude ». Restent : les **arbitrages demandés**
et les **questions** Q-021-1 à 14.

Cadre : D7 (amendée), D8, D10, D11, D17, D31 (amendée), D34, D36 de
[`../001-cadrage/decisions.md`](../001-cadrage/decisions.md) ; 007/W1 à W18 ; 017/U2,
U14 ; 022 (validée, A1 à A8) ; 006/H19 ; 020 ; ADR 0025, 0037, 0038 ; spec 018 (non
fusionnée).

## Arbitrages rendus (2026-10-23, utilisateur)

- **D-1** framework : prototype comparatif en cours ; D17 et l'ADR 0011 inchangées.
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
- X25 SDK 2.0 sans framework.
- X26 import map étendue, plages `ui` et `framework`, espace au manifeste.
- X27 DOM léger en première partie ; tiers au choix ; exemple sans framework.
- X28 i18n de la 007 gardée.
- X29 WCAG 2.2 AA étendu ; consultation à 320 px, édition ≥ 1024 px.
- X30 budgets étendus, bloquants.
- X31 bascule en place, par zones.
- X32 déplacement à parent déclaré : cibles vérifiées (cycle, type, droits, présent) avant mise en évidence.
- X33 aucun optimisme pour un déplacement ; le serveur reste seul juge.
- X34 invariant d'affichage des arbres, test de propriété de bout en bout.

## X2 — Espaces
- Décision : liste fermée extensible d'espaces (`referential`, `changes`, `admin`,
  `network`, `hosts`, `security`) ; un plugin déclare `space` ; un espace vide est masqué.
- Pourquoi : brief tour 2 (barre latérale groupée) ; un plugin ne doit pas créer un groupe
  par module (dérive).
- Rejeté : un groupe par plugin ; espaces libres.
- Décidé par : choix de Claude (Q-021-5)

## X5 — Contexte temporel
- Décision : forme du brief (pastille, bande ambre) ; règles de W9 et REQ-007-23
  inchangées : `as_of` → aucune écriture ; `valid_at` seul → brouillon permis avec
  `valid_from`, édition en place et modification rapide désactivées.
- Pourquoi : le brief dit « écritures désactivées » ; W9 distinguait déjà les deux cas et
  l'écriture au futur passe par la date d'effet d'un changeset.
- Décidé par : à confirmer (Q-021-13)

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
- Décision : une édition de cellule ajoute une opération au brouillon (sans motif à
  chaque cellule) ; Ctrl+Entrée applique tout de suite par la modification rapide, motif
  demandé en ligne.
- Pourquoi : REQ-007-25 exige un motif à chaque modification rapide : à la cellule, ce
  serait une friction ; le brouillon groupe plusieurs cellules en un changeset, avec un
  motif, une simulation et une revue éventuelle.
- Rejeté : application directe à chaque cellule ; brouillon seul (pas de chemin rapide).
- Décidé par : à arbitrer (Q-021-1, A4)

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
  compris** ; plugin ≤ 50 Ko gzip hors partagés ; plafond de mémoire après D-1.
- Pourquoi : D-10 ; le partage par l'import map déplace le framework des plugins vers le
  shell.
- Décidé par : l'utilisateur (D-10) ; 50 Ko et périmètre des 300 Ko : choix de Claude
  (Q-021-11)

## X31 — Bascule
- Décision : en place, par zones, parcours verts à chaque commit.
- Pourquoi : une seule application à maintenir ; la v0.1 sort avant, avec l'interface
  actuelle.
- Rejeté : paquet parallèle `web/shell2`.
- Décidé par : à confirmer (Q-021-4)

## Arbitrages demandés

- **A1 — 017/U2** (plugins Politique et Flux « en Custom Elements sans framework ») :
  contredit D-2/D-3 et l'ADR 0038. *Recommandation* : amender U2 (note posée dans la 017) ;
  les plugins de la 017 s'écrivent avec le framework partagé et `@kartenn/ui`, en DOM
  léger, après la phase 5.
- **A2 — ADR 0025 et 007/W7** (le SDK, seule instance partagée ; un plugin embarque son
  framework) : amendés par l'ADR 0038. *Recommandation* : accepter l'ADR 0038 avec la 021 ;
  le reste de l'ADR 0025 est inchangé.
- **A3 — Contrat client de la 022** (design § « Contrat consommé par les clients » :
  « invalider la requête de l'objet si son ETag connu diffère de `changeset` ») :
  incompatible avec l'ETag composite et opaque de 006/H19 (fusionné le 2026-10-23).
  *Recommandation* : amender ce paragraphe de la 022 : comparer `changeset` à la `version`
  du corps ; invalider aussi les requêtes qui montrent l'objet ; sans `version`, relire en
  `If-None-Match`. Amendement de forme, sans effet sur le serveur.
- **A4 — REQ-007-25** (motif obligatoire à chaque modification rapide) et édition en
  place : *Recommandation* : garder REQ-007-25 ; l'édition en place passe par le
  brouillon par défaut (X15), le motif est demandé à la soumission ou en ligne pour
  Ctrl+Entrée.
- **A5 — REQ-007-32** (« shell ≤ 300 Ko gzip hors plugins ») : le framework et
  `@kartenn/ui`, partagés, entrent désormais dans le shell. *Recommandation* : garder le
  plafond de 300 Ko en y comptant les modules partagés ; ajouter le plafond par plugin.
- **A6 — Permissions `module.action`** (retours du 2026-10-22, spec 018 non fusionnée) :
  la 010 a renommé `policy.read` en `policy.catalogue.read` (« trois segments exigés »).
  La 021 n'affiche que les noms servis ; le renommage relève de la 018. *Recommandation* :
  pas de renommage dans la 021 ; dépendance notée.
- **A7 — D17 et ADR 0011** : inchangées tant que D-1 garde Vue ; amendées (shell en
  React) si le prototype retient React. *Recommandation* : trancher D-1 avant la phase 2 ;
  rien dans la 021 n'est perdu dans un cas comme dans l'autre hors P2 à P5.
- **A8 — 007/W17** (« sans magasin global ») : le `QueryClient` est un cache de lectures,
  pas un magasin d'état d'application. *Recommandation* : pas d'amendement ; préciser W17
  (l'état de vue reste dans l'URL).
- **A9 — Brief « écritures désactivées hors présent »** et W9 (brouillon permis sous
  `valid_at`) : *Recommandation* : garder W9 (X5, Q-021-13).

## Questions

- **Q-021-1** — Édition en place : brouillon par défaut et Ctrl+Entrée pour appliquer
  (recommandé), application directe avec motif, ou brouillon seul ?
- **Q-021-2** — L'arbre des préfixes n'a aucun déplacement (X17, décidé) ; quels arbres
  à parent déclaré offrent le déplacement en v0.2, et « Changer de domaine… » au menu si
  l'opération existe ? *Recommandation* :
  hébergement d'une machine (cluster) et ancrage d'un groupe de VLAN, rien d'autre.
- **Q-021-3** — Un flux par navigateur : verrou `navigator.locks` et `BroadcastChannel`
  (recommandé) ou `SharedWorker` ? Repli : un flux par onglet.
- **Q-021-4** — Bascule en place par zones (recommandé) ou paquet parallèle ?
- **Q-021-5** — Liste des espaces et leurs libellés (Référentiel, Changesets,
  Administration, Réseau, Machines, Sécurité) : *Recommandation* : celle-ci, extensible
  par amendement.
- **Q-021-6** — Lettres de `g` (table du design) : *Recommandation* : la table ; une
  lettre de plugin proposée au manifeste, refusée si prise.
- **Q-021-7** — Plafond de mémoire : *Recommandation* : mesure au prototype D-1 (tas
  après 10⁴ lignes et 50 navigations), plafond = mesure × 1,5, fixé à la tâche du banc.
- **Q-021-8** — Délai du préchargement au survol : *Recommandation* : 60 ms (entre les
  50 ms d'un routeur courant et les 100 ms de l'étude).
- **Q-021-9** — `staleTime` des objets au présent (30 s) avec le flux actif :
  *Recommandation* : 30 s ; le flux invalide plus tôt, la valeur ne sert qu'au repli.
- **Q-021-10** — Libellés avant la 018 : *Recommandation* : catalogues du shell et des
  plugins pour les valeurs de code (états, types, espaces) ; clés seules pour les valeurs
  de données (environnements, axes).
- **Q-021-11** — Plafond par plugin : *Recommandation* : 50 Ko gzip hors partagés.
- **Q-021-12** — Petites largeurs : *Recommandation* : consultation lisible à 320 px CSS ;
  édition et administration à partir de 1024 px, message en dessous.
- **Q-021-13** — Sous `valid_at` seul, brouillon permis (W9, recommandé) ou toute
  écriture désactivée (lecture littérale du brief) ?
- **Q-021-14** — Couche de compatibilité SDK 1.x : *Recommandation* : aucune ; seuls des
  plugins de première partie existent, portés en phase 5.

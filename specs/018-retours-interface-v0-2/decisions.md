# 018 — Retours d'usage de la v0.2 : journal des décisions

Statut : **validée le 2026-10-23**. Brouillon du 2026-10-22 à partir des retours de
l'utilisateur du même jour ; questions Q-018-1 à 8 arbitrées par l'utilisateur le
2026-10-23 (en fin de journal). La Q-018-4 a reçu une conception nouvelle, qui remplace
les options du brouillon : un **dictionnaire de libellés** négocié par langue (R19 à R24).
La partie web de la 018 (codes visibles, édition en place, filtres typés, affichage clé
et libellé) est **reprise par la spec 021** (shell v2) ; la 018 garde le serveur, qui doit
être livré avant le nouveau shell (R1).

Cadre : D8, D10 (bitemporel), D11 (changesets), D14 (budgets), D36 (RBAC) ; 003/P2, P8,
P12 et REQ-003-5 (clés de liste fermée immuables) ; 005/K12 (nommage des permissions) ;
006/H12, H19 (ETag composite) et ADR 0022 ; 007/W11 (amendée par R19) ; 010/S2, S5, S29 ;
011/F12 ; 017/U14 ; 022/R2, R3 (sortes d'événement, charge minimale) ; 023/T15, T20.
Inventaire du code au 2026-10-22 : [`design.md`](design.md), « État des lieux ».

Règle permanente de l'utilisateur : **rien n'est gardé pour compatibilité avant le gel**
de l'API ; il n'y a pas encore d'utilisateurs.

## Synthèse
- R1 périmètre : serveur (permissions, libellés, routes de suggestion) ; écrans repris par
  la 021, après la livraison serveur.
- R2 nommage des permissions : la ressource s'omet quand elle porte le nom du module.
- R3 table de correspondance dans le code, pour la migration et l'historique.
- R4 sortes d'objets inchangées ; permission par défaut d'une sorte selon R2.
- R5 grammaire : deux segments ou plus ; `prop.<clé>.<action>` inchangé.
- R6 audit et historique jamais réécrits.
- R7 clé stable, libellés par langue, repli sur la clé.
- R8 libellés des données en base, versionnés avec leur objet ; libellés des codes du
  logiciel dans le code du noyau et des modules.
- R9 langue par défaut de l'instance en configuration ; repli BCP 47.
- R10 libellés fournis par les modules ; celui d'un administrateur n'est jamais écrasé.
- R11, R13, R15 : codes visibles, édition en place, filtres typés : décisions de
  l'utilisateur, **reprises par la 021** ; ici, leurs besoins serveur seulement.
- R12, R14, R16, R17 : choix d'écran du brouillon, transmis à la 021.
- R18 ordre : renommage et libellés avant la 021 et avant les manifestes de la 017.
- R19 dictionnaire de libellés `GET /v1/labels`, seul endroit où le serveur sert du texte
  par langue (amende W11).
- R20 corps des objets neutres en langue : clés seules pour les références ; table
  `labels` sur l'objet qui les possède.
- R21 négociation : `Accept-Language` ou `?lang=`, `Content-Language`, `Vary` sur cette
  route seulement.
- R22 révision des libellés, ETag par langue et révision, lecture datée.
- R23 invalidation par l'événement SSE `labels` (amende 022).
- R24 dictionnaire filtré par les droits de lecture de chaque espace.
- R25 trois flux de travail parallèles au plus.
- Q-018-1 `flow.waiver.grant` ; Q-018-2 aucune compatibilité ; Q-018-3 changeset système ;
  Q-018-4 dictionnaire négocié ; Q-018-5 libellés versionnés avec leur objet ; Q-018-6
  `labels jsonb` sur chaque table propriétaire ; Q-018-7 le noyau porte les libellés de
  toute valeur de liste fermée ; Q-018-8 extension des routes existantes.

## R1 — Périmètre
- Décision : la 018 livre le côté serveur des retours du 2026-10-22 : (1) noms de
  permissions, (2) clés et libellés par langue, (3) routes de suggestion pour les filtres
  typés et l'édition en place. Les écrans (codes visibles, édition en place, filtres
  typés, affichage clé et libellé) sont spécifiés par la **021** (shell v2) à partir de
  R11 à R17. Le langage de requête écrit reste une évolution différée (REQ-018-24).
- Pourquoi : le shell est réécrit (021) ; écrire ces écrans dans le shell actuel serait
  du travail jeté ; le serveur doit être prêt avant le nouveau shell.
- Décidé par : l'utilisateur (2026-10-23)

## R2 — Nommage des permissions
- Décision : une permission s'écrit `<module>.<ressource>.<action>`, sauf quand la
  ressource porte le nom du module : elle s'omet, `<module>.<action>`. Ainsi
  `flow.flow.read` → `flow.read`, `flow.flow.review` → `flow.review`,
  `machines.machine.write` → `machines.write`. Les autres restent à trois segments
  (`policy.service.admin`, `core.changeset.read`, `flow.waiver.grant`).
- Amende : 005/K12.
- Décidé par : l'utilisateur (2026-10-22)
- Amendé par la 024 (M7, Q-024-6, 2026-10-24) : la règle de nommage tient ; les actions
  `write` et `propose` (et `core.changeset.approve`) sont remplacées par `edit`, `apply`,
  `approval.self` (`machines.write` → `machines.edit` + `machines.approval.self`).

## R3 — Table de correspondance
- Décision : la correspondance ancien → nouveau nom est une table statique du code
  (`RENAMED_PERMISSIONS`, `kartenn-core/src/authz.rs`), seule source pour la migration des
  rôles (Q-018-3) et l'affichage des entrées anciennes de l'historique et de l'audit
  (R6), servie par le dictionnaire (R19, espace `permission`, « anciennement … »). Elle
  n'ouvre **aucun alias** (Q-018-2). Table complète : [`design.md`](design.md),
  « Correspondance des permissions ».
- Décidé par : Claude (choix interne)
- Complété par la 024 (REQ-024-9, 2026-10-24) : `authz::retired` compose la table de la
  018 avec le remplacement des verbes de la 024 ; la migration des rôles personnalisés
  (motif `permission rename 024`) et le dictionnaire des anciens noms en dérivent.

## R4 — Les sortes d'objets ne changent pas
- Décision : seules les permissions sont renommées ; les sortes (`vlan.vlan`,
  `machines.machine`, …) restent. La permission par défaut d'une sorte
  (`type_read_permission`, `type_write_permission`) applique R2 : sorte `m.m` →
  `m.read`, `m.write`.
- Décidé par : Claude (périmètre minimal de R2)

## R5 — Grammaire des entrées de rôle
- Décision : une entrée de rôle est `*`, une permission du catalogue (deux segments ou
  plus), `prop.<clé>.<action>` ou `prop.*.<action>`. La contrainte SQL de
  `role_permission.permission` (migration 0028), qui exige trois segments, est relâchée :
  `^(\*|[a-z][a-z0-9_]*(\.[a-z0-9_.*]+)?\.[a-z_]+)$`. Aucune ressource de module ne porte
  le nom d'une action (test du catalogue).
- Décidé par : Claude (conséquence directe de R2)

## R6 — Audit et historique jamais réécrits
- Décision : le journal d'audit scellé (ADR 0019) et l'historique des rôles gardent les
  anciens noms. Le dictionnaire (R19) libelle aussi les anciens noms (« Lire les flux —
  anciennement `flow.flow.read` ») pour qu'une lecture datée reste lisible.
- Décidé par : Claude (conséquence de D8, D10, ADR 0019)

## R7 — Clé interne, libellés par langue
- Décision : chaque valeur visible a une **clé** stable (`prod`) et des **libellés par
  langue** (`fr` : « Production », `en` : « Production »), avec repli sur la clé. La clé
  est toujours servie ; le libellé est ce qu'un client affiche.
- Décidé par : l'utilisateur (2026-10-22)

## R8 — Origine des libellés
- Décision : (a) **données administrables** — valeurs de listes fermées (environnements
  compris), déclarations de propriétés, rôles, types de flux, services, motifs, règles,
  conditions et signaux de la politique : colonne `labels jsonb` sur la table
  propriétaire, versionnée avec la ligne (Q-018-5, 6). (b) **Codes du logiciel** —
  permissions, codes de règles, types de problèmes, sortes d'objets, états, natures et
  usages, issues des flux : libellés FR et EN compilés dans le code du noyau et des
  modules, à côté de leur définition (descriptions de permissions, résumés de règles,
  titres de problèmes) ; ils suivent la version du serveur. Les deux familles sont
  servies par le même dictionnaire (R19). Les textes de l'interface elle-même (boutons,
  titres d'écran) restent dans les catalogues des clients (W11, 023/T21).
- Pourquoi : chaque libellé est écrit là où sa clé est définie ; un client n'a pas à
  connaître les codes de chaque module.
- Décidé par : Claude, d'après l'arbitrage de Q-018-4 (« tout ce qui a un libellé »)

## R9 — Langue par défaut et repli
- Décision : `[i18n] default_locale` (configuration du noyau, `fr` par défaut) remplace
  la constante de la politique (`kartenn-policy/src/codec.rs`, `DEFAULT_LOCALE`) ; elle
  est obligatoire dans toute table `labels` et sert de repli. Étiquettes BCP 47
  ([RFC 5646 §2.1](https://www.rfc-editor.org/rfc/rfc5646#section-2.1)) ; choix de la
  langue servie par le schéma « Lookup »
  ([RFC 4647 §3.4](https://www.rfc-editor.org/rfc/rfc4647#section-3.4) : troncature
  progressive de l'étiquette demandée) parmi les langues disponibles, puis la langue par
  défaut ; une entrée sans libellé dans la langue servie prend celui de la langue par
  défaut, et une entrée sans aucun libellé est absente (le client rend la clé).
- Décidé par : Claude (réversible)

## R10 — Libellés fournis par les modules
- Décision : les descripteurs de module portent les libellés FR et EN de leurs
  propriétés et valeurs ; `sync_modules` (003/P12) les écrit s'ils manquent ou si la
  version courante venait du module ; un libellé posé par un administrateur n'est jamais
  écrasé.
- Décidé par : Claude (réversible)

## R11 — Codes internes toujours visibles (repris par la 021)
- Décision : l'interface affiche toujours le code technique à côté du message lisible :
  slug du `type` d'un Problem Details, identifiants de règles des violations
  (`import.unknown-reference`, `flow.update.ended`…), règles des anomalies, règle
  décisive d'un verdict. Le message peut venir du dictionnaire (R19) ; le code est
  toujours montré.
- Côté serveur (018) : les codes sont déjà servis (`type`, `violations[].rule`,
  `anomaly.rule`) ; le dictionnaire libelle chacun d'eux.
- Décidé par : l'utilisateur (2026-10-22)

## R12 — Forme du code affiché (transmis à la 021)
- Décision proposée à la 021 : `<code>` monospace, sélectionnable, bouton de copie au nom
  accessible, confirmation par `role="status"` ; `instance` d'un problème affichée.
- Décidé par : Claude (proposition ; la 021 tranche)

## R13 — Édition en place (reprise par la 021)
- Décision : pour qui a le droit d'écriture, tableaux et arbre sont interactifs (cliquer
  l'environnement d'un préfixe pour en choisir un autre) ; l'édition passe par le
  brouillon et le changeset ; les droits se lisent dans `/v1/me`.
- Côté serveur (018) : aucune route nouvelle ; les valeurs proposées viennent des routes
  de suggestion (Q-018-8), leurs libellés du dictionnaire.
- Décidé par : l'utilisateur (2026-10-22)

## R14 — Arbre : édition par une action du nœud (transmis à la 021)
- Décision proposée : motif `tree` gardé, action de nœud au clavier ; tableaux au motif
  `grid` de l'APG WAI-ARIA.
- Décidé par : Claude (proposition ; la 021 tranche)

## R15 — Filtres typés, langage de requête différé
- Décision : les filtres sur les propriétés se choisissent selon le type (liste fermée,
  référence, date, nombre), avec autocomplétion ; une syntaxe de requête écrite viendra
  plus tard, avec autocomplétion et coloration syntaxique (REQ-018-24).
- Côté serveur (018) : routes de suggestion (Q-018-8) et catalogue des règles.
- Décidé par : l'utilisateur (2026-10-22)

## R16 — L'URL reste l'état des filtres (transmis à la 021)
- Décision : la syntaxe de filtre servie (`prop.<clé>[:op]=valeur`, `filter.rs`) ne
  change pas ; le constructeur de la 021 l'écrit et la relit.
- Décidé par : Claude

## R17 — Maquettes (transmis à la 021)
- Décision : les écrans issus de R11 à R15 sont maquettés et validés avec ceux de la 021.
- Décidé par : Claude

## R18 — Ordre
- Décision : le renommage des permissions, le dictionnaire et les routes de suggestion
  sont livrés **avant** les tâches web de la 021 et avant les manifestes de la 017
  (017-2, 017-3), qui naissent avec les nouveaux noms.
- Décidé par : l'utilisateur (2026-10-23, « le serveur avant le nouveau shell »)

## R19 — Dictionnaire de libellés (Q-018-4)
- Décision : `GET /v1/labels` rend, pour une langue, la table clé → libellé de tout ce
  qui a un libellé, rangée par espace (`value`, `property`, `role`, `permission`,
  `rule`, `problem`, `kind`, `state`, `policy.*`…). C'est le **seul** endroit où le
  serveur sert du texte selon la langue. Les Problem Details gardent leur `title` neutre
  et leur `type` ; leur message lisible peut venir de l'espace `problem`, celui d'une
  violation de l'espace `rule`.
- Taille bornée : catalogues seulement (valeurs de listes, rôles, permissions, règles,
  objets de politique), jamais les 10⁶ objets du référentiel ; paramètre `spaces=` pour
  n'en demander qu'une partie.
- Amende : 007/W11, qui rejetait les messages traduits par le serveur : la traduction des
  codes et des données passe par le dictionnaire ; les textes de l'interface restent aux
  clients. 023/T15 (libellés « à traiter avec la réécriture du web ») : la bibliothèque
  et la CLI lisent le dictionnaire.
- Décidé par : l'utilisateur (2026-10-23)

## R20 — Corps des objets neutres en langue
- Décision : un objet qui **cite** une valeur porte sa clé seule (un préfixe porte
  `policy.environment: ["prod"]`, jamais « Production ») ; un objet qui **possède** des
  libellés les porte en table neutre (`labels: {fr, en}`), lue et réécrite avec lui sous
  `If-Match` ; aucun champ `label` résolu. Un seul ETag par objet (006/H19). Les champs
  `label` actuels (rôles, objets de la politique) disparaissent (aucune compatibilité,
  Q-018-2).
- Pourquoi : un corps qui ne varie pas avec la langue garde un seul ETag et un seul
  cache ; l'objet propriétaire doit pouvoir s'éditer en entier.
- Décidé par : l'utilisateur (« objets à clés seules ») ; Claude (table `labels` sur
  l'objet propriétaire, nécessaire à l'écriture)

## R21 — Négociation de la langue
- Décision : la langue se demande par `?lang=<étiquette>` (prioritaire) ou
  `Accept-Language` ([RFC 9110 §12.5.4](https://www.rfc-editor.org/rfc/rfc9110#section-12.5.4)) ;
  la réponse porte `Content-Language`
  ([RFC 9110 §8.5](https://www.rfc-editor.org/rfc/rfc9110#section-8.5)) et
  `Vary: Accept-Language`
  ([RFC 9110 §12.5.5](https://www.rfc-editor.org/rfc/rfc9110#section-12.5.5)) **sur cette
  route seulement**. Langue choisie selon R9.
- Décidé par : l'utilisateur (2026-10-23) ; détails RFC : Claude

## R22 — Révision, ETag, lecture datée
- Décision : une **révision des libellés** : compteur en base incrémenté dans la
  transaction de tout changeset qui écrit une colonne `labels`, combiné à l'empreinte des
  libellés compilés de la version du serveur. ETag (forme 006/H19) :
  `"<révision>.<empreinte du corps>"`, l'empreinte couvrant la langue et le filtrage par
  droits (R24). `valid_at` et `as_of` comme toute lecture ; une réponse à `as_of` passé ne
  change plus que si la version du serveur change : `Cache-Control: private,
  max-age=86400` avec revalidation par ETag (pas `immutable`, les libellés compilés
  suivant la version du serveur) ; au présent, `no-cache` et revalidation.
- Décidé par : l'utilisateur (lecture datée, mise en cache longue du passé) ; Claude
  (forme de la révision et des en-têtes)

## R23 — Invalidation par SSE
- Décision : une sorte d'événement nouvelle `labels` sur `GET /v1/events` (022),
  `data: {"revision": "<révision>"}`, émise après tout changeset qui modifie la révision ;
  remise à toute connexion (la révision ne révèle rien) ; un redémarrage sur une autre
  version est couvert par `ready` (tout invalider). Le client relit `/v1/labels` avec
  `If-None-Match`. Amendement additif de 022/R2 (« une sorte nouvelle amende la spec »).
- Décidé par : l'utilisateur (2026-10-23)

## R24 — Dictionnaire filtré par les droits
- Décision : chaque espace du dictionnaire n'est rendu qu'à qui détient, sur une portée
  quelconque, la lecture de la collection qui le possède (`policy.*` sous
  `policy.catalogue.read`, `role` sous `core.rbac.read` ou pour les rôles de ses propres
  liaisons, `value` et `property` pour tout authentifié, hors valeurs d'une propriété
  sensible sans `prop.<clé>.read`) ; les espaces de codes du logiciel (`permission`,
  `rule`, `problem`, `kind`, `state`) à tout authentifié. Route authentifiée, jamais
  publique.
- Pourquoi : le dictionnaire ne doit pas devenir un canal de lecture de ce que les routes
  REST cachent (même principe que 022/R3).
- Décidé par : Claude (choix de sécurité prudent, conforme à D36 ; réversible vers plus
  ouvert)

## R25 — Trois flux parallèles
- Décision : les tâches se rangent en trois flux indépendants (A permissions, B libellés,
  C suggestions), plus une clôture ; pas plus de trois sous-agents à la fois (CLAUDE.md).
- Décidé par : l'utilisateur (2026-10-23)

## Questions arbitrées (2026-10-23)

### Q-018-1 — Nom de la permission de dérogation
- Options : (A) `flow.waiver.grant` (la dérogation est une ressource : `flow_waiver`) ;
  (B) `flow.waive` (R2 appliquée mécaniquement).
- Recommandation : A.
- Décision : **`flow.waiver.grant`**.
- Décidé par : l'utilisateur (2026-10-23)

### Q-018-2 — Compatibilité des anciens noms
- Options : (A) aucune ; (B) alias à l'entrée jusqu'à la v0.3 ; (C) alias permanents.
- Recommandation : B.
- Décision : **A, aucune compatibilité** : un ancien nom est refusé
  (`core.role.invalid`) ; un manifeste qui le cite ne s'affiche pas. Règle permanente :
  rien n'est gardé pour compatibilité avant le gel ; il n'y a pas d'utilisateurs.
- Décidé par : l'utilisateur (2026-10-23)

### Q-018-3 — Migration des rôles en base
- Options : (A) changeset système au job de migration ; (B) `UPDATE` des lignes
  courantes ; (C) `UPDATE` courant et historique.
- Recommandation : A.
- Décision : **A** : auteur `système`, motif `permission rename 018`, idempotent, audité ;
  l'historique garde les anciens noms.
- Décidé par : l'utilisateur (2026-10-23)

### Q-018-4 — Forme de l'API des libellés
- Options du brouillon : (A) table `labels` sur chaque objet, `label` résolu dans la
  langue par défaut ; (B) `label` résolu par `Accept-Language` sur chaque objet ; (C) les
  deux.
- Recommandation : A.
- Décision : **conception nouvelle, qui remplace les trois options** : objets neutres en
  langue (R20) et dictionnaire séparé `GET /v1/labels` négocié par langue (R19, R21),
  daté et mis en cache (R22), invalidé par SSE (R23) ; les clients (SDK web de la 021,
  bibliothèque et CLI de la 023) le gardent en cache et résolvent localement, repli sur
  la clé. Écriture : les libellés s'écrivent avec leur objet propriétaire
  (`labels: {fr, en}`) dans un changeset.
- Décidé par : l'utilisateur (2026-10-23)

### Q-018-5 — Les libellés sont-ils bitemporels ?
- Options : (A) versionnés avec leur objet ; (B) hors du temps ; (C) versionnés, libellé
  courant toujours affiché.
- Recommandation : A.
- Décision : **A** (D10, D11) : renommer passe par un changeset, apparaît à l'historique ;
  une vue datée montre le libellé de la date.
- Décidé par : l'utilisateur (délégué à Claude, 2026-10-23)

### Q-018-6 — Stockage des libellés
- Options : (A) `labels jsonb` sur chaque table propriétaire ; (B) table partagée.
- Recommandation : A.
- Décision : **A** ; le dictionnaire s'assemble depuis ces colonnes et les libellés
  compilés.
- Décidé par : l'utilisateur (2026-10-23)

### Q-018-7 — Libellés des valeurs d'axes et des environnements
- Options : (A) le noyau porte les libellés de toute valeur de liste fermée,
  `policy_value.label` migre et disparaît ; (B) libellés dans `policy_value` pour les
  axes.
- Recommandation : A.
- Décision : **A**.
- Décidé par : l'utilisateur (2026-10-23)

### Q-018-8 — Autocomplétion
- Options : (A) étendre les routes existantes ; (B) route générique `/v1/suggestions` ;
  (C) client seul.
- Recommandation : A.
- Décision : **A** : `/values` (clés, période, `valid_at`, `q` par préfixe), `q` sur les
  collections citables par une référence, `GET /v1/rules` ; p95 < 50 ms serveur, 20
  résultats par défaut. Les libellés des suggestions viennent du dictionnaire.
- Décidé par : l'utilisateur (2026-10-23)

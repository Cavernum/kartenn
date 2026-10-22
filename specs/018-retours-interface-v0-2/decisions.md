# 018 — Retours d'usage de la v0.2 : journal des décisions

Statut : **proposée, en attente de validation**. Brouillon du 2026-10-22, rédigé à partir
des retours de l'utilisateur du même jour. Cinq décisions sont **prises par l'utilisateur**
(R2, R7, R11, R13, R15) et consignées telles quelles ; les choix réversibles et invisibles
de l'extérieur sont tranchés par Claude (« Décidé par : Claude ») ; les choix structurants
(contrat, schéma, compatibilité) sont des questions **à arbitrer**, en fin de journal,
chacune avec sa recommandation et les options écartées.

Cadre : D8, D10 (bitemporel), D11 (changesets), D14 (budgets), D36 (RBAC) ; 003/P2, P8,
P12 et REQ-003-5 (clés de liste fermée immuables) ; 005/K12 (nommage des permissions) ;
006/H12 et ADR 0022 (évolution additive) ; 007/W9, W10, W11, W14, W17 ; 010/S2, S5, S29 ;
011/F12 ; 017/U2, U6, U7, U14. Inventaire du code au 2026-10-22 (commit `d0297ac`) :
[`design.md`](design.md), « État des lieux ».

## Synthèse
- R1 périmètre : cinq retours (permissions, clés et libellés, codes visibles, édition en
  place, filtres typés), plus un langage de requête différé.
- R2 nommage des permissions : la ressource s'omet quand elle porte le nom du module
  (utilisateur).
- R3 table de correspondance : onze permissions renommées, tenue dans le code, servie par
  le catalogue.
- R4 types d'objets inchangés : `vlan.vlan`, `machines.machine` restent des sortes ; la
  permission par défaut d'un type suit la règle de R2.
- R5 grammaire : deux segments ou plus ; `prop.<clé>.<action>` inchangé.
- R6 audit et historique : jamais réécrits ; l'interface montre l'ancien nom avec le
  nouveau.
- R7 clé stable et libellés par langue, repli sur la clé (utilisateur).
- R8 deux familles de libellés : données administrables (servies par l'API) et codes du
  logiciel (catalogues du shell et des plugins, W11).
- R9 langue par défaut de l'instance en configuration ; repli BCP 47 par troncature.
- R10 libellés fournis par les modules ; un libellé d'administrateur n'est jamais écrasé.
- R11 codes internes toujours visibles (utilisateur).
- R12 forme du code : `<code>` monospace, bouton de copie, après le message.
- R13 édition en place pour qui a les droits, par le brouillon (utilisateur).
- R14 arbre : édition par une action du nœud, pas par une grille.
- R15 filtres typés ; langage de requête différé (utilisateur).
- R16 l'URL reste l'état des filtres ; syntaxe serveur inchangée.
- R17 maquettes avant les écrans (comme 017/U14).
- R18 ordre : renommage des permissions avant les manifestes de la 017.
- Q-018-1 à 8 : à arbitrer (nom de la dérogation, compatibilité, migration des rôles,
  forme de l'API des libellés, bitemporalité, stockage, libellés des environnements,
  autocomplétion).

## R1 — Périmètre
- Décision : la 018 regroupe les retours du 2026-10-22 pour la v0.2 : (1) noms de
  permissions, (2) clé interne et libellé par langue, (3) codes internes visibles,
  (4) édition en place, (5) filtres typés. Le langage de requête écrit (formules) est une
  évolution **différée** : ses exigences minimales sont écrites (REQ-018-30), pas sa
  conception.
- Pourquoi : retours d'un même essai, qui touchent les mêmes écrans ; les livrer ensemble
  évite de reprendre deux fois les tableaux et les maquettes de la 017.
- Décidé par : l'utilisateur (périmètre) ; Claude (regroupement en une spec)

## R2 — Nommage des permissions
- Décision : une permission s'écrit `<module>.<ressource>.<action>`, sauf quand la
  ressource porte le nom du module : elle s'omet, `<module>.<action>`. Ainsi
  `flow.flow.read` → `flow.read`, `flow.flow.review` → `flow.review`,
  `machines.machine.write` → `machines.write`. Les autres restent à trois segments
  (`policy.service.admin`, `core.changeset.read`).
- Pourquoi : le doublon n'apporte rien à la lecture ; la ressource principale du module se
  nomme par le module.
- Amende : 005/K12 (forme `<module>.<objet>.<action>`).
- Décidé par : l'utilisateur (2026-10-22)

## R3 — Table de correspondance
- Décision : la correspondance ancien → nouveau nom est une table statique du code
  (`RENAMED_PERMISSIONS`, `kartenn-core/src/authz.rs`), seule source pour la migration des
  rôles (Q-018-3), les alias éventuels (Q-018-2), l'affichage de l'historique (R6) et la
  section « Anciens noms » du catalogue généré (`docs/spec/permissions.md`). Table
  complète : [`design.md`](design.md), « Correspondance des permissions ».
- Pourquoi : une seule table, testée, plutôt que des chaînes répétées dans la migration,
  le shell et la doc.
- Décidé par : Claude (choix interne, réversible)

## R4 — Les sortes d'objets ne changent pas
- Décision : seules les permissions sont renommées. Les sortes qualifiées
  (`vlan.vlan`, `vlan.group`, `machines.machine`, `machines.cluster`,
  `machines.interface`) restent ce qu'elles sont dans `module_object.kind`, les opérations
  et le contrat. La permission par défaut d'une sorte (`type_read_permission`,
  `type_write_permission`) applique R2 : sorte `m.m` → `m.read`, `m.write`.
- Pourquoi : la sorte est une valeur stockée et servie (contrat, outbox, audit) ; la
  changer serait une rupture sans bénéfice demandé.
- Décidé par : Claude (périmètre minimal de R2)

## R5 — Grammaire des entrées de rôle
- Décision : une entrée de rôle est `*`, une permission du catalogue (deux segments ou
  plus), `prop.<clé>.<action>` ou `prop.*.<action>`. La contrainte SQL de
  `role_permission.permission` (migration 0028), qui exige trois segments, est relâchée
  par une migration : `^(\*|[a-z][a-z0-9_]*(\.[a-z0-9_.*]+)?\.[a-z_]+)$`. Aucun module ne
  peut déclarer une ressource nommée comme une action (`read`, `write`, `propose`,
  `approve`, `admin`, …) : vérifié par un test du catalogue.
- Pourquoi : `flow.read` doit pouvoir être stocké ; l'interdiction évite qu'un nom à deux
  segments se lise de deux façons.
- Décidé par : Claude (conséquence directe de R2)

## R6 — Audit et historique jamais réécrits
- Décision : le journal d'audit scellé (ADR 0019) garde les anciens noms (`permission`
  de `authz.access.denied`, contenus de `rbac.role.changed`) ; l'historique des rôles
  aussi (Q-018-3). L'interface, quand elle affiche une entrée ancienne, montre le nouveau
  nom suivi de l'ancien (« `flow.read` (anciennement `flow.flow.read`) »), par R3.
- Pourquoi : un journal scellé ne se réécrit pas ; une lecture datée doit rester vraie.
- Décidé par : Claude (conséquence de D8, D10, ADR 0019)

## R7 — Clé interne, libellés par langue
- Décision : chaque valeur visible dans l'interface a une **clé** stable (`prod`) et des
  **libellés par langue** (`fr` : « Production », `en` : « Production »), avec repli sur la
  clé. La clé est toujours servie par l'API ; le libellé est ce que l'interface affiche.
- Pourquoi : la clé sert aux scripts, à l'import et aux investigations ; le libellé aux
  personnes, dans leur langue.
- Décidé par : l'utilisateur (2026-10-22)

## R8 — Deux familles de libellés
- Décision : (a) **données administrables** — valeurs de listes fermées des propriétés
  (environnements compris), déclarations de propriétés, rôles, types de flux, services,
  motifs, règles, conditions, signaux de la politique : libellés stockés en base, écrits par
  changeset, servis par l'API (Q-018-4 à 7). (b) **Codes du logiciel** — états, sortes
  d'objets, natures et usages, codes de règles, types de Problem Details, permissions,
  états et issues des flux : libellés dans les catalogues de l'interface (W11) ; ceux d'un
  module dans les catalogues de son plugin, sous des clés conventionnelles
  (`kinds.<sorte>`, `rules.<id>`, `permissions.<nom>`, `states.<état>`), que le shell lit
  par le SDK. L'API reste neutre en langue pour la famille (b).
- Pourquoi : W11 a écarté la traduction par le serveur pour les textes du logiciel ; une
  donnée saisie par un administrateur n'est, elle, connue que de la base.
- Décidé par : Claude (prolonge W11 ; la forme de (a) est à arbitrer)

## R9 — Langue par défaut et repli
- Décision : la langue par défaut de l'instance devient une configuration du noyau
  (`[i18n] default_locale`, `fr` par défaut), qui remplace la constante de la politique
  (`kartenn-policy/src/codec.rs`, `DEFAULT_LOCALE`). Étiquettes de langue BCP 47
  ([RFC 5646 §2.1](https://www.rfc-editor.org/rfc/rfc5646#section-2.1)). Résolution d'un
  libellé : langue de l'utilisateur, puis ses troncatures (`fr-CA` → `fr`, schéma
  « Lookup » de [RFC 4647 §3.4](https://www.rfc-editor.org/rfc/rfc4647#section-3.4)),
  puis la langue par défaut de l'instance, puis la clé. Le repli est signalé (attribut
  `lang` de l'élément rendu, exigé par WCAG 3.1.2).
- Pourquoi : RFC 4647 décrit le mécanisme de repli (choix de kartenn : la langue par défaut
  avant la clé) ; une seule langue par défaut pour les textes pédagogiques et les
  libellés.
- Décidé par : Claude (réversible)

## R10 — Libellés fournis par les modules
- Décision : les descripteurs de module portent les libellés FR et EN de leurs propriétés
  et valeurs de listes fermées ; `sync_modules` (003/P12) les écrit s'ils manquent ou si
  la version précédente venait du module. Un libellé posé par un administrateur n'est
  jamais écrasé par la synchronisation (comme une valeur de liste ajoutée, 003/P12).
- Pourquoi : un module livré doit s'afficher lisiblement sans configuration ; un
  administrateur doit pouvoir renommer pour son organisation.
- Décidé par : Claude (réversible)

## R11 — Codes internes toujours visibles
- Décision : l'interface affiche toujours le code technique à côté du message lisible :
  codes de Problem Details (slug du `type`), identifiants de règles des violations
  (`import.unknown-reference`, `flow.update.ended`…), règles des anomalies, règle décisive
  d'un verdict, clé d'un objet de politique. Le message n'est jamais masqué par le code,
  ni le code par le message.
- Pourquoi : faciliter les investigations (recherche dans les logs, la doc, un ticket) ;
  ne pas cacher l'information.
- Décidé par : l'utilisateur (2026-10-22)

## R12 — Forme du code affiché
- Décision : un élément `<k-code>` du SDK, utilisable par le shell et les plugins : texte
  du code dans un `<code>` monospace, après le message ; bouton de copie (icône et nom
  accessible « Copier le code `<code>` »), confirmation par une région `role="status"` ;
  sélection au clavier possible ; jamais tronqué sans moyen de lire le code entier.
  Pour un Problem Details : slug du `type` et, s'il existe, `instance`.
- Pourquoi : une seule forme dans tous les écrans ; copier évite les erreurs de saisie.
- Décidé par : Claude (réversible ; à confirmer par les maquettes, R17)

## R13 — Édition en place
- Décision : pour qui détient le droit d'écriture, les tableaux et l'arbre sont
  interactifs : un clic sur l'environnement d'un préfixe ouvre la liste fermée pour en
  choisir un autre. L'édition **ajoute une opération au brouillon local** (007/W10 b),
  simulé en continu, soumis en un changeset ; aucune route nouvelle. Les droits se lisent
  dans `/v1/me` (REQ-007-20), le serveur restant juge.
- Pourquoi : corriger dix valeurs ne doit pas demander dix fiches ; le brouillon montre le
  différentiel et les violations avant la soumission.
- Décidé par : l'utilisateur (2026-10-22)

## R14 — Arbre : édition par une action du nœud
- Décision : dans l'arbre des préfixes, l'édition d'une propriété passe par une action du
  nœud (menu contextuel, raccourci clavier) qui ouvre une liste déroulante accessible ;
  l'arbre garde le motif `tree` (WAI-ARIA APG). Les tableaux suivent le motif `grid`.
- Pourquoi : un `treegrid` virtualisé éditable est lourd et mal restitué par les lecteurs
  d'écran ; une action de nœud reste simple au clavier.
- Décidé par : Claude (réversible ; à confirmer par les maquettes)

## R15 — Filtres typés, langage de requête différé
- Décision : les filtres sur les propriétés ne sont plus du texte libre : la valeur se
  choisit selon le type (liste fermée, référence d'objet, date, nombre, booléen), avec
  autocomplétion. Une syntaxe de requête écrite (formules) viendra **plus tard**, à
  condition d'offrir une bonne autocomplétion et une coloration syntaxique.
- Décidé par : l'utilisateur (2026-10-22)

## R16 — L'URL reste l'état des filtres
- Décision : le constructeur de filtres produit la syntaxe de filtre déjà servie
  (`prop.<clé>[:op]=valeur`, `crates/kartenn-rest/src/filter.rs`) dans l'URL (007, « État :
  l'URL ») ; une URL écrite à la main reste lue et rendue dans le constructeur.
- Pourquoi : aucun changement du contrat de filtrage ; liens partageables inchangés.
- Décidé par : Claude

## R17 — Maquettes avant les écrans
- Décision : comme 017/U14 : les écrans touchés (code visible, libellés, cellules
  éditables, action de nœud, constructeur de filtres, formulaires de libellés) sont
  maquettés et validés par l'utilisateur avant toute tâche web de la 018. Les maquettes de
  la 017 (tâche 017-1) intègrent les mêmes formes.
- Décidé par : Claude (prolonge U14 ; à confirmer)

## R18 — Ordre avec la 017
- Décision : le renommage des permissions (tâches 2 à 7) précède les tâches 017-2 et
  017-3 (manifestes) ; les maquettes de la 017 et de la 018 se font ensemble.
- Pourquoi : la 017 est à 0/24 : ses manifestes et ses tests naissent avec les nouveaux
  noms au lieu d'être repris.
- Décidé par : Claude (à confirmer avec l'ordre de la 017)

## Questions à arbitrer

Chaque question porte ses options, la recommandation de Claude, et attend la décision de
l'utilisateur.

### Q-018-1 — Nom de la permission de dérogation
- Constat : la permission actuelle est `flow.flow.waive` ; `flow.waiver.grant`, cité en
  exemple par l'utilisateur, n'existe pas. R2 seule donnerait `flow.waive`.
- Options : (A) `flow.waiver.grant` : la dérogation est une ressource à part
  (`flow_waiver` : motif, risque accepté, fin, recertification), l'action est « accorder » ;
  (B) `flow.waive` : application mécanique de R2.
- Recommandation : **A**, conforme à l'exemple donné et au modèle (la dérogation a sa
  table et son cycle de vie).
- Décision : _à arbitrer_

### Q-018-2 — Compatibilité des anciens noms
- Constat : les noms apparaissent dans les rôles personnalisés, les scripts qui écrivent
  des rôles (`PUT /v1/roles/{key}`, opérations de rôle), les manifestes de plugins tiers
  chargés à chaud (champ `permission`) ; aucune instance de production n'est connue.
- Options :
  - (A) **aucune** : les anciens noms sont refusés dès la v0.2 (`core.role.invalid`) ; un
    manifeste qui les cite masque sa vue.
  - (B) **alias à l'entrée, temporaires** : jusqu'à la v0.3, un ancien nom reçu (écriture
    de rôle, manifeste) est remplacé par le nouveau avant validation, jamais stocké, et
    signalé (violation d'avertissement `core.permission.renamed` dans la réponse, log
    technique ; avertissement de console pour un manifeste). Les sorties ne portent que
    les nouveaux noms.
  - (C) alias permanents dans les deux sens (contrôle par l'un ou l'autre nom).
- Recommandation : **B** : coût faible (la table R3 existe de toute façon), protège les
  scripts et plugins existants, date de fin écrite. A acceptable si l'utilisateur confirme
  qu'aucun script ni plugin tiers n'existe. C écarté : deux noms pour un droit brouillent
  l'audit et la revue des rôles.
- Décision : _à arbitrer_

### Q-018-3 — Migration des rôles en base
- Constat : `role_permission` est bitemporel (`valid_period`, table d'historique,
  `changeset_id`) ; les liaisons citent le rôle par identifiant (non touchées) ; les rôles
  livrés sont resynchronisés au démarrage depuis le code (`kartenn-service/src/rbac.rs`) ;
  aucune migration SQL ne contient de nom de permission ; `approval_policy` n'en cite pas.
- Options :
  - (A) **changeset système** : une étape du job de migration (comme `sync_modules`)
    soumet un changeset de l'auteur `système`, motif `permission rename 018`, qui clôt
    chaque entrée ancienne et ouvre la nouvelle dans chaque rôle personnalisé, à l'instant
    de la migration ; idempotent (rien à faire au second passage) ; audité.
  - (B) `UPDATE` SQL des lignes courantes dans la migration : rapide, mais la version
    produite n'a pas de changeset propre (traçabilité perdue).
  - (C) `UPDATE` des lignes courantes **et** de l'historique : le passé est réécrit.
- Recommandation : **A** ; B et C contredisent D10 (l'historique dit ce qui était connu)
  et 004 (toute écriture par changeset). Une lecture datée avant la migration rend les
  anciens noms, affichés selon R6.
- Décision : _à arbitrer_

### Q-018-4 — Forme de l'API des libellés
- Constat : W11 a écarté les messages traduits par le serveur (`Accept-Language`) ; les
  textes pédagogiques de la politique sont déjà servis **en table** par langue
  (`texts: {fr: …, en: …}`, 010/S29) ; l'API n'a aucun traitement d'`Accept-Language`.
- Options :
  - (A) **table** : chaque objet sert `key` et `labels: {<langue>: <texte>}` ; le champ
    `label` existant reste, résolu dans la langue par défaut de l'instance (additif,
    006/H12) ; le client résout (R9).
  - (B) **résolu par `Accept-Language`** : `label` dans la langue négociée,
    `Content-Language`, `Vary: Accept-Language` ; ETag et caches par langue.
  - (C) les deux : table toujours, `label` négocié si l'en-tête est présent.
- Recommandation : **A** : API neutre en langue (W11), un seul ETag, cohérent avec
  `texts` ; quelques octets de plus par objet (deux langues livrées). B rouvre W11 et la
  006 ; C double les chemins de test sans besoin identifié.
- Décision : _à arbitrer_

### Q-018-5 — Les libellés sont-ils bitemporels ?
- Options :
  - (A) **versionnés avec leur objet** : un libellé change par changeset (opération de
    l'objet), apparaît au différentiel et à l'historique ; une lecture datée rend le
    libellé de la date.
  - (B) métadonnées hors du temps : écrites hors changeset, dernière écriture gagnante,
    seulement auditées.
  - (C) versionnés, mais l'interface affiche toujours le libellé courant, même à une date
    passée.
- Recommandation : **A** : rien n'échappe aux changesets (D11) ni à l'historique (D10) ;
  le libellé est une donnée saisie comme une autre. C écarté : une vue datée doit montrer
  ce qui était connu ; la clé, stable, assure la continuité.
- Conséquence : renommer un libellé exige le droit d'écriture de l'objet (pour une valeur
  de liste fermée : `core.property.admin`, ou le droit admin du module qui la décrit).
- Décision : _à arbitrer_

### Q-018-6 — Stockage des libellés
- Options :
  - (A) **colonne `labels jsonb`** (`{<langue>: <texte>}`) sur chaque table d'objet
    concernée (`property_declaration`, `property_enum_value`, `role`, tables de la
    politique), versionnée avec la ligne ; règle `core.label.invalid` (1 à 200
    caractères, sans caractère de contrôle, étiquette BCP 47 valide, au moins la langue par
    défaut) ; les colonnes `label` existantes migrent en `{<langue par défaut>: label}`.
  - (B) table partagée `object_label(object_id, locale, text, valid_period)` : générique,
    indexable pour la recherche, mais une jointure par lecture et un versionnement
    distinct de l'objet.
- Recommandation : **A** : précédent `texts jsonb` de la politique, pas de jointure, une
  seule version par objet. Les listes fermées sont courtes : la recherche par libellé
  (autocomplétion) se fait sans index dédié.
- Décision : _à arbitrer_

### Q-018-7 — Libellés des valeurs d'axes et des environnements
- Constat : un environnement est une valeur de `policy.environment` ; sa clé est dans
  `property_enum_value` (noyau), son libellé unique dans `policy_value.label` (module) ;
  les valeurs des autres listes fermées n'ont aucun libellé.
- Options :
  - (A) **le noyau porte les libellés de toute valeur de liste fermée** ;
    `policy_value.label` migre vers `property_enum_value.labels` et disparaît ;
    `policy_value` garde description, transverse, rang ; `value.describe` et
    `environment.create` écrivent les libellés du noyau.
  - (B) libellés par langue dans `policy_value` pour les axes, dans le noyau pour les
    autres listes.
- Recommandation : **A** : un seul endroit pour le libellé d'une valeur, quelle que soit
  la propriété ; le filtre typé et l'édition en place lisent la même source. Amende
  010/S2 et le design de la 010 (tableau des tables).
- Décision : _à arbitrer_

### Q-018-8 — Autocomplétion des filtres et de l'édition
- Constat : `GET /v1/property-declarations/{key}/values` rend des clés seules, sans
  libellé ni date ; aucune recherche par préfixe n'existe pour les références (machines,
  entités, sites…) hors `value=` des adresses et préfixes ; le catalogue des règles n'est
  pas servi.
- Options :
  - (A) **étendre les routes existantes** : `/values` rend `key`, `labels`, période,
    accepte `valid_at` et `q` (préfixe, insensible à la casse, sur la clé et les
    libellés) ; un paramètre `q` (préfixe sur le nom, le slug ou la valeur) sur les
    collections citables par une référence ; `GET /v1/rules` (identifiant, sévérité,
    module) pour filtrer les anomalies par règle.
  - (B) route générique `GET /v1/suggestions?field=<champ>&q=&valid_at=` : une réponse
    `[{key|id, labels, kind}]` pour tout champ (liste, référence, champ du noyau), réutilisable
    par le futur langage de requête.
  - (C) client seul : listes fermées chargées entières, références par les listes
    existantes filtrées côté client.
- Recommandation : **A** : additif, chaque route garde son contrôle de droits et son
  budget ; B à reconsidérer avec le langage de requête (REQ-018-30) ; C ne tient pas pour
  les références à 10⁶ objets.
- Budget proposé : suggestion en p95 < 50 ms serveur (classe « lecture d'un objet »,
  D14), 20 résultats par défaut.
- Décision : _à arbitrer_

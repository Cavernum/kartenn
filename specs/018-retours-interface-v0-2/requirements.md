---
status: implemented
updated: 2026-10-24
---

# 018 — Retours d'usage de la v0.2 : exigences

Statut : **validée le 2026-10-23** (Q-018-1 à 8 arbitrées).

## Résumé

Côté serveur des retours de l'utilisateur du 2026-10-22 : des noms de permissions sans
doublon (`flow.flow.read` → `flow.read`), sans compatibilité avec les anciens noms ; une
clé stable et des libellés par langue pour tout ce qui s'affiche, servis par un
**dictionnaire de libellés** négocié par langue, daté, mis en cache et invalidé par le
flux SSE de la 022 ; des routes de suggestion pour les filtres typés et l'édition en
place. Les écrans (codes visibles, édition en place, filtres typés, affichage clé et
libellé) sont spécifiés par la **021** (shell v2), qui consomme ce que la 018 livre ; la
018 est livrée avant les tâches web de la 021 (R18). Un langage de requête écrit est une
évolution différée.

Décisions : [`decisions.md`](decisions.md) (R1–R25, Q-018-1 à 8) ; D10, D11, D14, D36 ;
003/P12, REQ-003-5 ; 005/K12 ; 006/H12, H19 ; 007/W11 (amendée) ; 010/S2, S29 ;
011/F12 ; 022/R2, R3 ; 023/T15, T20.

### Vocabulaire

- **Clé** : identifiant stable d'une valeur, servi par l'API, jamais traduit (`prod`,
  `pending`, `core.prefix.duplicate`).
- **Libellé** : texte affiché pour une clé, dans une langue.
- **Donnée administrable** : valeur saisie en base par un administrateur ou un module
  (valeur de liste fermée, déclaration de propriété, rôle, objet de politique).
- **Code du logiciel** : valeur fixée par le code (permission, code de règle, type de
  problème, sorte d'objet, état, nature).
- **Dictionnaire** : réponse de `GET /v1/labels` : clés → libellés d'une langue, par
  espace.
- **Révision des libellés** : identifiant qui change à chaque écriture de libellé et à
  chaque version du serveur.
- Conventions EARS : comme dans
  [`002/requirements.md`](../002-noyau-adressage/requirements.md#conventions).

## Exigences

### A. Noms de permissions

#### REQ-018-1 — Règle de nommage
kartenn doit nommer chaque permission `<module>.<ressource>.<action>`, sauf quand la
ressource porte le nom du module, auquel cas elle doit s'écrire `<module>.<action>` ; les
permissions de propriété restent `prop.<clé>.<action>`. [R2, R5, amende 005/K12]

Acceptation :
- Le catalogue ne contient aucun nom `m.m.a` ; il contient `flow.read`, `machines.write`,
  `vlan.read`, `flow.waiver.grant`.
- Un test refuse un catalogue de module dont une ressource porte le nom d'une action.

#### REQ-018-2 — Correspondance appliquée partout
kartenn doit appliquer la table de correspondance (design, « Correspondance des
permissions ») au catalogue, aux rôles livrés, aux contrôles de droits, aux permissions
par défaut des sortes d'objets, aux descriptions du contrat et au catalogue généré
`docs/spec/permissions.md`. [R3, R4, Q-018-1]

Acceptation :
- Le rôle `reader` contient `flow.read`, `vlan.read`, `machines.read` ; une sorte
  `machines.machine` se lit sous `machines.read`.
- Une dérogation exige `flow.waiver.grant`.

#### REQ-018-3 — Anciens noms refusés
Quand une écriture de rôle cite un ancien nom, kartenn doit la refuser
(`core.role.invalid`) ; aucun alias ne doit exister, ni à l'entrée ni au contrôle.
[Q-018-2]

Acceptation :
- `PUT /v1/roles/r` avec `flow.flow.read` : 422, violation `core.role.invalid` qui nomme
  l'entrée.

#### REQ-018-4 — Migration des rôles personnalisés
Lors de la mise à jour d'une instance, le job de migration doit remplacer chaque ancien
nom des rôles personnalisés par le nouveau dans un changeset de l'auteur `système`, de
motif `permission rename 018`, idempotent et audité, sans toucher aux liaisons.
[Q-018-3]

Acceptation :
- Un rôle `{flow.flow.read, policy.service.admin}` devient
  `{flow.read, policy.service.admin}` ; ses liaisons donnent les mêmes droits avant et
  après ; un second passage ne crée aucun changeset.
- Le changeset est lisible dans `/v1/changesets` ; le rôle lu à une date antérieure rend
  l'ancien nom.

#### REQ-018-5 — Historique et audit intacts
kartenn ne doit réécrire ni le journal d'audit ni l'historique des rôles ; le
dictionnaire doit libeller aussi les anciens noms, en nommant le nouveau. [R6]

Acceptation :
- Un événement `authz.access.denied` antérieur garde `flow.flow.read` ;
  `GET /v1/labels?spaces=permission` contient `flow.flow.read` (« … — anciennement »).

#### REQ-018-6 — Aucune occurrence restante
Avec le renommage, kartenn doit livrer à jour les manifestes des plugins de première
partie, les tests de bout en bout, le contrat, la bibliothèque Python et `docs/spec/` ;
aucun ancien nom ne doit rester hors de la table de correspondance, de la migration, de
leurs tests et des journaux de décision. [R3]

Acceptation :
- Un test ne trouve aucun ancien nom dans `crates/`, `web/`, `clients/`, `docs/spec/`
  hors des exceptions listées.

### B. Clés et libellés

#### REQ-018-7 — Corps neutres en langue
L'API doit servir des corps d'objets qui ne dépendent pas de la langue : un objet qui cite
une valeur porte sa clé seule ; un objet propriétaire de libellés les porte en table
`labels: {<langue>: <texte>}` ; aucun champ `label` résolu. [R20]

Acceptation :
- `GET /v1/prefixes/{id}/properties` rend `policy.environment: ["prod"]` ; deux requêtes
  qui ne diffèrent que par `Accept-Language` rendent le même ETag.
- `GET /v1/roles/reader` rend `labels: {fr: "Lecteur", en: "Reader"}` et aucun `label`.

#### REQ-018-8 — Libellés des données administrables
kartenn doit conserver des libellés par langue, versionnés avec leur objet, pour les
valeurs de listes fermées des propriétés (environnements compris), les déclarations de
propriétés, les rôles, les types de flux, services, motifs, règles, conditions et
signaux de la politique ; le noyau doit porter ceux de toute valeur de liste fermée.
[R8, Q-018-5, Q-018-6, Q-018-7]

Acceptation :
- L'environnement `prod` a ses libellés dans `property_enum_value` ; `policy_value` n'a
  plus de colonne `label`.
- Le libellé lu à une date antérieure à son changement est l'ancien.

#### REQ-018-9 — Écriture des libellés
kartenn doit écrire les libellés avec leur objet propriétaire (`labels: {fr, en}`), par
changeset, sous le droit d'écriture de l'objet, la clé restant immuable (REQ-003-5) ;
un libellé doit compter de 1 à 200 caractères, sans caractère de contrôle, sous une
étiquette BCP 47 valide, et la langue par défaut doit être présente
(`core.label.invalid`). [R7, R9, Q-018-5]

Acceptation :
- `update_enum_value` change le libellé `en` de `prod` : différentiel montrant l'ancien
  et le nouveau ; clé inchangée ; événement SSE `labels` émis.
- Libellé vide, ou sans la langue par défaut : 422, `core.label.invalid`.

#### REQ-018-10 — Libellés des codes du logiciel
kartenn doit déclarer, à côté de leur définition dans le code, les libellés FR et EN des
permissions, codes de règles, types de problèmes, sortes d'objets (modules compris),
états (changesets, imports, flux), natures et usages ; un test doit vérifier que chaque
code en a dans les deux langues livrées. [R8]

Acceptation :
- Le test de complétude échoue si une règle nouvelle n'a pas de libellé `en`.

#### REQ-018-11 — Libellés fournis par les modules
Lors de la synchronisation des modules, kartenn doit écrire les libellés FR et EN que les
descripteurs déclarent pour leurs propriétés et valeurs, sans jamais écraser un libellé
posé par un administrateur. [R10, 003/P12]

Acceptation :
- Libellé `en` de `policy.environment` modifié par un administrateur, puis redéploiement :
  le libellé de l'administrateur demeure.

#### REQ-018-12 — Langue par défaut de l'instance
kartenn doit lire la langue par défaut de l'instance en configuration
(`[i18n] default_locale`, `fr` par défaut), la publier dans les capacités et l'utiliser
pour les libellés et les textes pédagogiques de la politique. [R9, 010/S29]

Acceptation :
- Instance en `en` : une règle `deny` sans texte anglais est refusée
  (`policy.text.missing`) ; `GET /v1/capabilities` rend `default_locale: "en"`.

#### REQ-018-13 — Migration des libellés existants
Lors de la mise à jour, kartenn doit reprendre chaque libellé existant (rôles, objets de
la politique, `policy_value.label`) comme libellé de la langue par défaut, puis supprimer
les colonnes `label` ; les rôles livrés doivent recevoir leurs libellés FR et EN.
[Q-018-6, Q-018-7, Q-018-2]

Acceptation :
- Un type de flux libellé « Administration à distance » avant : `labels.fr` égal après ;
  le rôle `reader` a `fr` « Lecteur », `en` « Reader ».

### C. Dictionnaire de libellés

#### REQ-018-14 — Route du dictionnaire
kartenn doit servir `GET /v1/labels` à tout appelant authentifié : pour une langue, les
libellés de tout ce qui en a un, rangés par espace (`value`, `property`, `role`,
`permission`, `rule`, `problem`, `kind`, `state`, espaces des modules), filtrables par
`spaces=`. [R19]

Acceptation :
- `GET /v1/labels?lang=fr&spaces=value` contient
  `value["policy.environment"]["prod"] = "Production"`.
- Sans authentification : 401.

#### REQ-018-15 — Négociation
Le dictionnaire doit choisir sa langue par `?lang=` sinon `Accept-Language`, selon le
schéma « Lookup » (RFC 4647 §3.4) puis la langue par défaut, rendre `Content-Language`
et `Vary: Accept-Language` ; aucune autre route ne doit varier selon la langue. Une
entrée sans libellé dans la langue servie doit prendre celui de la langue par défaut,
signalé ; une entrée sans aucun libellé doit être absente. [R21, R9]

Acceptation :
- `Accept-Language: fr-CA, en;q=0.5` : `Content-Language: fr`.
- `lang=en`, valeur libellée en français seulement : libellé français, présent dans la
  liste des replis de la réponse.

#### REQ-018-16 — Révision et ETag
Le dictionnaire doit porter un ETag `"<révision>.<empreinte>"` (006/H19) qui change avec
la révision des libellés, la langue servie et les espaces rendus, et répondre 304 à un
`If-None-Match` inchangé. [R22]

Acceptation :
- Deux lectures sans écriture : même ETag, 304 ; après un `update_enum_value` : ETag
  changé ; après une mise à jour du serveur qui change un libellé compilé : ETag changé.

#### REQ-018-17 — Lecture datée
Le dictionnaire doit accepter `valid_at` et `as_of` ; une réponse à `as_of` passé doit
être servie avec `Cache-Control: private, max-age=86400`, une réponse au présent avec
`no-cache`. [R22, D10]

Acceptation :
- Libellé de `prod` changé le 10 : lecture à `valid_at` du 5 : ancien libellé.

#### REQ-018-18 — Invalidation par SSE
Quand un changeset modifie la révision des libellés, kartenn doit émettre l'événement
`labels` (`{"revision": …}`) sur `GET /v1/events` à toute connexion ouverte ; aucune
autre donnée ne doit y figurer. [R23, amende 022/R2]

Acceptation :
- Un client abonné reçoit `event: labels` après un `update_enum_value`, puis relit
  `/v1/labels` (200, nouvel ETag).

#### REQ-018-19 — Droits
Le dictionnaire ne doit rendre un espace de données qu'à qui détient la lecture de la
collection qui le possède, sur une portée quelconque, ni les valeurs d'une propriété
sensible sans `prop.<clé>.read` ; les espaces de codes du logiciel sont rendus à tout
authentifié. [R24, D36]

Acceptation :
- Sans `policy.catalogue.read` : aucun espace `policy.*`, ETag différent de celui d'un
  lecteur de la politique.

#### REQ-018-20 — Budgets
Le dictionnaire complet d'une langue doit être servi en p95 < 50 ms serveur (304 en
p95 < 10 ms) et peser moins de 1 Mo non compressé sur le jeu de données des bancs.
[D14]

Acceptation :
- `bench-core` : mesures du dictionnaire dans ces budgets.

#### REQ-018-21 — Client Python
La bibliothèque `kartenn` doit garder le dictionnaire en cache par langue et révision,
l'invalider à l'événement `labels` (et à `ready`), et résoudre localement avec repli sur
la clé ; la CLI doit afficher les libellés en table et les clés dans les formats de
données. [R19, 023/T15, T20, T22]

Acceptation :
- `kartenn` en table montre « Production » ; `--keys` et `-o json` montrent `prod`.

### D. Suggestions

#### REQ-018-22 — Valeurs de listes fermées
`GET /v1/property-declarations/{key}/values` doit rendre pour chaque valeur sa clé et sa
période, accepter `valid_at` (valeurs valides à la date) et `q` (préfixe insensible à la
casse sur la clé et les libellés), et rendre 20 résultats par défaut. [Q-018-8]

Acceptation :
- `q=pro&valid_at=…` rend `prod` et `preprod`, pas une valeur close avant la date.

#### REQ-018-23 — Références et règles
kartenn doit accepter `q` (préfixe insensible à la casse sur le nom, le slug ou la
valeur) sur les collections citables par une référence, restreint aux objets lisibles,
et servir `GET /v1/rules` (identifiant, sévérité, module) ; chaque suggestion doit
tenir en p95 < 50 ms serveur à 10⁶ objets. [Q-018-8, D14]

Acceptation :
- `GET /v1/machines?q=web` ne rend jamais une machine non lisible ; banc sous 50 ms.
- `GET /v1/rules` contient `import.unknown-reference` et `flow.update.ended`.

### E. Évolution différée

#### REQ-018-24 — Langage de requête (hors de la 018)
Si un langage de requête écrit est livré, il doit : avoir une grammaire publiée ; se
traduire dans les mêmes filtres que le constructeur de la 021 (aller-retour sans
perte) ; offrir une autocomplétion contextuelle (champs, opérateurs selon le type,
valeurs avec libellés, par les routes de la section D) ; colorer la syntaxe sans porter
l'information par la seule couleur ; signaler une erreur avec sa position et son code ;
rester utilisable au clavier et par lecteur d'écran (WCAG 2.2 AA). [R15]

Acceptation : à écrire avec la spec qui le concevra.

## Repris par la 021 (shell v2)

Décisions de l'utilisateur dont les écrans relèvent de la 021, qui consomme la 018 :

| Décision | Ce que la 021 spécifie | Ce que la 018 fournit |
|---|---|---|
| R11 codes visibles | code monospace, copiable, à côté du message (proposition R12) | codes déjà servis ; libellés `problem`, `rule` du dictionnaire |
| R7 clé et libellé | affichage du libellé, clé lisible, `lang` en repli (WCAG 3.1.2) | dictionnaire, révision, événement `labels` |
| R13 édition en place | grille, combobox, action de nœud (proposition R14), brouillon | suggestions (D), `update_enum_value`, permissions par `/v1/me` |
| R15 filtres typés | constructeur, URL comme état (R16) | `/values`, `q`, `/v1/rules`, syntaxe `filter.rs` inchangée |
| R17 maquettes | maquettes de ces écrans | — |

## Non-objectifs

- Tout écran : spécifiés par la 021.
- La traduction des messages libres du serveur (`detail`, `message` d'une anomalie) :
  le code et le libellé du dictionnaire les accompagnent.
- Le renommage des sortes d'objets (R4).
- Le langage de requête écrit (REQ-018-24 en pose les conditions).
- Toute compatibilité avec les anciens noms ou l'ancien champ `label` (Q-018-2).

## Questions ouvertes

- [x] Q-018-1 à Q-018-8 : arbitrées le 2026-10-23 ([`decisions.md`](decisions.md)).
- [ ] R24 (filtrage du dictionnaire par les droits) : choix prudent de Claude, à
  confirmer par l'utilisateur à la revue du design de la tâche 10. (Claude)

---
status: proposed
updated: 2026-10-22
---

# 018 — Retours d'usage de la v0.2 : exigences

Statut : **proposée, en attente de validation**.

## Résumé

Retours de l'utilisateur du 2026-10-22, pour la v0.2 : des noms de permissions sans
doublon (`flow.flow.read` → `flow.read`) ; une clé stable et des libellés par langue pour
toute valeur visible ; les codes techniques toujours affichés à côté des messages ;
l'édition en place des tableaux et de l'arbre, par le brouillon ; des filtres typés avec
autocomplétion. Un langage de requête écrit est une évolution différée, dont seules les
exigences minimales sont posées ici. Les choix structurants (compatibilité, migration,
forme de l'API, bitemporalité et stockage des libellés, autocomplétion) sont **à
arbitrer** ([`decisions.md`](decisions.md), Q-018-1 à 8) ; les exigences qui en dépendent
le disent.

Décisions : [`decisions.md`](decisions.md) (R1–R18, Q-018-1 à 8) ; D10, D11, D14, D36 ;
003/P12, REQ-003-5 ; 005/K12 ; 006/H12 ; 007/W9 à W11, W14 ; 010/S2, S29 ; 011/F12 ;
017/U14.

### Vocabulaire

- **Clé** : identifiant stable d'une valeur, servi par l'API, jamais traduit (`prod`,
  `pending`, `core.prefix.duplicate`).
- **Libellé** : texte affiché pour une clé, par langue (`fr` : « Production »).
- **Donnée administrable** : valeur saisie en base par un administrateur ou un module
  (valeur de liste fermée, déclaration de propriété, rôle, objet de politique) (R8 a).
- **Code du logiciel** : valeur fixée par le code (état, sorte d'objet, nature, code de
  règle, type de problème, permission) (R8 b).
- **Code affiché** : clé ou identifiant technique montré à côté d'un message (R11).
- **Édition en place** : modification d'une valeur depuis un tableau ou l'arbre, qui
  ajoute une opération au brouillon (007/W10).
- Conventions EARS, « brouillon », « modification rapide », « vue » : comme dans
  [`007/requirements.md`](../007-web-shell-plugins/requirements.md).

## Exigences

### A. Noms de permissions

#### REQ-018-1 — Règle de nommage
kartenn doit nommer chaque permission `<module>.<ressource>.<action>`, sauf quand la
ressource porte le nom du module, auquel cas elle doit s'écrire `<module>.<action>` ; les
permissions de propriété restent `prop.<clé>.<action>`. [R2, R5, amende 005/K12]

Acceptation :
- Le catalogue ne contient aucun nom de la forme `m.m.a` ; il contient `flow.read`,
  `machines.write`, `vlan.read`.
- Un test refuse un catalogue de module dont une ressource porte le nom d'une action.

#### REQ-018-2 — Correspondance appliquée partout
kartenn doit appliquer la table de correspondance (design, « Correspondance des
permissions ») au catalogue, aux rôles livrés, aux contrôles de droits, aux permissions
par défaut des sortes d'objets et au catalogue généré `docs/spec/permissions.md`, qui
doit lister les anciens noms. [R3, R4]

Acceptation :
- Le rôle `reader` contient `flow.read`, `vlan.read`, `machines.read` ; une lecture de
  machine sous `machines.read` réussit, sous un rôle sans elle échoue (404).
- La sorte `machines.machine` se lit sous `machines.read`.

#### REQ-018-3 — Migration des rôles personnalisés
Lors de la mise à jour d'une instance, kartenn doit remplacer chaque ancien nom des rôles
personnalisés par le nouveau, de façon idempotente et traçable, sans toucher aux
liaisons. Forme selon Q-018-3 (recommandée : un changeset de l'auteur `système`).
[R3, Q-018-3]

Acceptation :
- Un rôle personnalisé `{flow.flow.read, policy.service.admin}` devient
  `{flow.read, policy.service.admin}` ; ses liaisons donnent les mêmes droits avant et
  après ; un second passage ne fait rien.
- (Si Q-018-3 = A) le changeset de migration est lisible dans `/v1/changesets` et le
  rôle lu à une date antérieure rend l'ancien nom.

#### REQ-018-4 — Historique et audit intacts
kartenn ne doit réécrire ni le journal d'audit ni l'historique des rôles ; quand
l'interface affiche un ancien nom, elle doit montrer le nouveau nom suivi de l'ancien.
[R6]

Acceptation :
- Un événement `authz.access.denied` antérieur garde `flow.flow.read` ; la vue d'audit
  l'affiche « `flow.read` (anciennement `flow.flow.read`) ».

#### REQ-018-5 — Compatibilité des anciens noms
Selon Q-018-2 : (A) un ancien nom reçu doit être refusé (`core.role.invalid`) ; ou
(B, recommandée) jusqu'à la v0.3, un ancien nom reçu dans une écriture de rôle ou un
manifeste de plugin doit être remplacé par le nouveau avant validation, jamais stocké, et
signalé (`core.permission.renamed`, avertissement). [Q-018-2]

Acceptation (B) :
- `PUT /v1/roles/r` avec `flow.flow.read` : 200, avertissement, rôle stocké avec
  `flow.read`.
- Un plugin dont la vue cite `vlan.vlan.read` s'affiche pour un lecteur, avec un
  avertissement en console.

#### REQ-018-6 — Contrats, SDK, plugins et documentation
kartenn doit livrer, avec le renommage, les manifestes des plugins de première partie,
les tests de bout en bout, les descriptions du contrat (`openapi-v1.json`), le README du
SDK et `docs/spec/` à jour ; aucune occurrence d'un ancien nom ne doit rester hors de la
table de correspondance, des tests de compatibilité et des journaux de décision.
[R3, R18]

Acceptation :
- Un test (ou `grep` en CI) ne trouve aucun ancien nom dans `crates/`, `web/`,
  `docs/spec/` hors des exceptions listées.

### B. Clés et libellés

#### REQ-018-7 — Clé toujours servie
L'API doit servir la clé de toute valeur affichable (champ `key`, ou la valeur elle-même
pour un code du logiciel), même quand un libellé existe. [R7]

Acceptation :
- `GET /v1/environments` rend `key: "prod"` pour chaque environnement, libellé présent
  ou non.

#### REQ-018-8 — Libellés des données administrables
kartenn doit conserver des libellés par langue pour les valeurs de listes fermées des
propriétés (environnements compris), les déclarations de propriétés, les rôles, les
types de flux, services, motifs, règles, conditions et signaux de la politique, et les
servir avec la clé. Forme : Q-018-4 (recommandée : table `labels`) ; stockage : Q-018-6 ;
environnements : Q-018-7. [R7, R8 a]

Acceptation :
- `GET /v1/property-declarations/policy.environment/values` rend
  `{key: "prod", labels: {fr: "Production", en: "Production"}}`.
- (Si Q-018-4 = A) le champ `label` existant reste, dans la langue par défaut.

#### REQ-018-9 — Résolution et repli
Quand l'interface affiche une valeur, elle doit afficher son libellé dans la langue de
l'utilisateur, sinon dans la langue tronquée, sinon dans la langue par défaut de
l'instance, sinon la clé ; un texte affiché dans une autre langue que celle de
l'interface doit porter l'attribut `lang`. [R9, WCAG 3.1.2]

Acceptation :
- Interface en anglais, valeur libellée en français seulement : libellé français,
  `lang="fr"` ; valeur sans libellé : la clé, en monospace.

#### REQ-018-10 — Écriture des libellés
kartenn doit permettre de créer et de modifier les libellés d'une donnée administrable
par changeset, sous le droit d'écriture de l'objet, la clé restant immuable
(REQ-003-5) ; un libellé doit compter de 1 à 200 caractères, sans caractère de contrôle,
sous une étiquette BCP 47 valide, la langue par défaut étant obligatoire
(`core.label.invalid`). Historisation : Q-018-5 (recommandée : versionnés avec l'objet).
[R7, Q-018-5, Q-018-6]

Acceptation :
- Renommer le libellé `en` de `prod` par changeset : différentiel montrant l'ancien et le
  nouveau libellé ; clé inchangée.
- Libellé vide, ou sans la langue par défaut : refusé (`core.label.invalid`).

#### REQ-018-11 — Libellés fournis par les modules
Lors de la synchronisation des modules, kartenn doit écrire les libellés FR et EN que les
descripteurs déclarent pour leurs propriétés et valeurs, sans jamais écraser un libellé
posé par un administrateur. [R10, 003/P12]

Acceptation :
- Libellé `en` de `policy.environment` modifié par un administrateur, puis redéploiement :
  le libellé de l'administrateur demeure.

#### REQ-018-12 — Libellés des codes du logiciel
L'interface doit afficher un libellé, dans chaque langue livrée, pour les états de
changeset et d'import, les sortes d'objets (modules compris), les natures et usages, les
états, issues et sortes des flux, les permissions, les codes de règles et les types de
problèmes ; ceux d'un module doivent venir des catalogues de son plugin ; à défaut, le
code. [R8 b, W11]

Acceptation :
- La fiche d'une machine titre « Machine » (et non `machines.machine`) ; `address.usage`
  `gateway` s'affiche « Passerelle » en français.
- Test de parité : chaque code d'une liste fermée du code a une clé de catalogue FR et EN.

#### REQ-018-13 — Langue par défaut de l'instance
kartenn doit lire la langue par défaut de l'instance en configuration
(`[i18n] default_locale`, `fr` par défaut) et l'utiliser pour les libellés et les textes
pédagogiques de la politique. [R9, 010/S29]

Acceptation :
- Instance réglée en `en` : une règle `deny` sans texte anglais est refusée
  (`policy.text.missing`).

#### REQ-018-14 — Migration des libellés existants
Lors de la mise à jour, kartenn doit reprendre chaque libellé existant (rôles, objets de
la politique, valeurs de `policy_value`) comme libellé de la langue par défaut, et donner
aux rôles livrés leurs libellés FR et EN. [Q-018-6, Q-018-7]

Acceptation :
- Le type de flux `ssh` libellé « Administration à distance » avant : `labels.fr` égal
  après ; le rôle `reader` a `fr` « Lecteur », `en` « Reader ».

### C. Codes internes visibles

#### REQ-018-15 — Code à côté du message
Quand l'interface affiche un problème (Problem Details), une violation, une anomalie, une
règle décisive d'un verdict ou un objet de politique, elle doit afficher le code
technique (slug du `type`, identifiant de règle, clé) à côté du message lisible, sans
masquer l'un par l'autre. [R11]

Acceptation :
- Import refusé : message traduit, puis `import.unknown-reference` en monospace.
- Violation `flow.update.ended` dans le brouillon : message du serveur et code.
- Anomalie : libellé de la règle (catalogue) et identifiant de la règle.

#### REQ-018-16 — Forme du code
Le code affiché doit être en monospace, sélectionnable, copiable par un bouton au nom
accessible qui confirme la copie par un message d'état, jamais tronqué sans moyen de le
lire en entier ; un Problem Details doit aussi montrer son `instance` quand il en a une.
[R12, WCAG 4.1.2, 4.1.3]

Acceptation :
- Copie au clavier (Tab, Entrée) : presse-papiers égal au code, message « Code copié »
  annoncé.

#### REQ-018-17 — Violations conservées
Le shell et le SDK doivent conserver les violations d'un Problem Details (règle,
sévérité, détail, pointeur) jusqu'à l'affichage. [R11]

Acceptation :
- Un 422 à deux violations affiche les deux, chacune avec son code.

### D. Édition en place

#### REQ-018-18 — Cellules et valeurs éditables
Où l'utilisateur détient, selon `/v1/me`, le droit d'écriture d'un objet ou d'une
propriété, et que le contexte temporel permet l'écriture (007/W9), l'interface doit rendre
éditables en place les valeurs à liste fermée, références, dates, nombres et booléens des
tableaux (listes du shell, panneau des propriétés, tableaux des plugins) ; sinon, elle
doit les rendre en lecture seule, sans contrôle d'édition. [R13, REQ-007-20, 22]

Acceptation :
- Lecteur : aucune cellule éditable ; éditeur : l'environnement d'un préfixe s'édite.
- `as_of` passé : aucune cellule éditable.

#### REQ-018-19 — Édition par le brouillon
Quand l'utilisateur valide une édition en place, l'interface doit ajouter l'opération
correspondante au brouillon (`update_prefix`, `assign_property`, `clear_property`,
`unassign_property`, opérations de module), marquer la valeur comme modifiée dans le
brouillon par un texte ou une icône nommée (pas par la seule couleur), et laisser la
simulation et la soumission au brouillon. [R13, 007/W10]

Acceptation :
- Changer trois environnements : trois opérations dans le brouillon, une simulation, une
  soumission en un changeset.
- Annuler l'opération dans le brouillon : la cellule revient à sa valeur lue.

#### REQ-018-20 — Valeur héritée
Quand l'utilisateur édite une propriété héritée, l'interface doit indiquer que
l'affectation remplacera l'ensemble hérité de l'origine nommée (D26), et offrir
« revenir à l'héritage » (`unassign_property`) et « effacer » (`clear_property`)
distinctement. [D26, 003/P4]

Acceptation :
- Préfixe héritant `{prod}` de son parent : l'éditeur nomme le parent ; choisir
  `{preprod}` ajoute `assign_property` ; « revenir à l'héritage » ajoute
  `unassign_property`.

#### REQ-018-21 — Arbre
Où l'arbre des préfixes affiche une propriété, l'interface doit offrir sur le nœud une
action « Changer <propriété> », au menu et au clavier, qui ouvre la liste de valeurs et
ajoute l'opération au brouillon. [R14]

Acceptation :
- Clavier seul : focaliser un nœud, ouvrir l'action, choisir une valeur, la voir marquée.

#### REQ-018-22 — Clavier et accessibilité
L'édition en place doit satisfaire WCAG 2.2 AA : tableaux au motif `grid` (flèches,
Entrée ou F2 pour éditer, Échap pour annuler, Entrée ou Tab pour valider), listes au
motif `combobox`, focus visible et non masqué (2.4.7, 2.4.11), cibles d'au moins 24 px
(2.5.8), pas de changement de contexte à la saisie (3.2.2), erreurs identifiées en texte
(3.3.1), état annoncé (4.1.3) ; axe-core sans violation. [R13, R14, 007/W14]

Acceptation :
- Parcours E2E au clavier seul : éditer deux cellules, en annuler une, soumettre.
- La ligne focalisée reste rendue dans un tableau virtualisé.

#### REQ-018-23 — Plugins
Le SDK doit offrir aux plugins l'édition en place d'une valeur (élément ou fonction) qui
lit les droits du contexte et ajoute au brouillon par `draft.add`. [R13, 007/W7]

Acceptation :
- Le plugin `machines` rend la sorte d'une machine éditable en place pour qui détient
  `machines.write`.

### E. Filtres typés

#### REQ-018-24 — Champ et opérateur
L'interface doit proposer les filtres d'une liste par un constructeur : choix du champ
(champs du noyau et propriétés déclarées, par leurs libellés), puis des opérateurs
permis par son type (égalité ou appartenance, intervalle, au-dessus, au-dessous,
présence, absence). [R15, 003/REQ-003-25]

Acceptation :
- Propriété de type liste fermée : opérateurs égalité/appartenance, présence, absence ;
  propriété date : intervalle, au-dessus, au-dessous en plus.

#### REQ-018-25 — Valeur selon le type
L'interface doit faire choisir la valeur selon le type : liste fermée → liste des valeurs
valides à `valid_at`, par libellé et clé ; référence → recherche d'objets lisibles du type
cible ; date → sélecteur de date ; entier → nombre borné ; booléen → choix oui/non ;
texte et FQDN → saisie validée. Aucune valeur hors liste ne doit pouvoir être soumise
pour une liste fermée. [R15]

Acceptation :
- Taper « pro » propose « Production (`prod`) » et « Préproduction (`preprod`) » ; une
  valeur close avant `valid_at` n'est pas proposée.

#### REQ-018-26 — Suggestions servies
kartenn doit servir les suggestions des listes fermées (clé, libellés, période) et des
références (recherche par préfixe sur nom, slug ou valeur) restreintes aux objets
lisibles, en p95 < 50 ms serveur à 10⁶ objets. Forme : Q-018-8 (recommandée : extension
des routes existantes). [R15, D14, Q-018-8]

Acceptation :
- Banc : suggestion de machine par préfixe de nom sur le jeu complet sous 50 ms (p95).

#### REQ-018-27 — L'URL reste l'état
Le constructeur doit écrire les filtres dans l'URL avec la syntaxe servie
(`prop.<clé>[:op]=valeur`) et relire une URL écrite à la main ; un filtre illisible doit
être signalé, pas ignoré en silence. [R16]

Acceptation :
- Coller `?prop.policy.environment=prod` : le constructeur montre « Environnement =
  Production (`prod`) ».

#### REQ-018-28 — Accessibilité des filtres
Le constructeur de filtres doit satisfaire WCAG 2.2 AA (motif `combobox`, étiquettes,
annonce du nombre de suggestions, suppression d'un filtre au clavier). [007/W14]

Acceptation :
- Ajouter, modifier et retirer un filtre au clavier seul ; axe-core sans violation.

#### REQ-018-29 — Filtres des anomalies et des plugins
Les filtres des anomalies (règle, domaine, entité) et des listes de plugins doivent
suivre le même constructeur : la règle se choisit dans le catalogue des règles. [R15]

Acceptation :
- Filtre « règle » des anomalies : liste des règles avec libellé et code.

### F. Évolution différée

#### REQ-018-30 — Langage de requête (différé, hors de la 018)
Si un langage de requête écrit est livré, il doit : avoir une grammaire publiée ; se
traduire dans les mêmes filtres que le constructeur (même sémantique, aller-retour entre
texte et constructeur sans perte) ; offrir une autocomplétion contextuelle (champs,
opérateurs selon le type, valeurs avec libellés) ; colorer la syntaxe sans porter
l'information par la seule couleur ; signaler une erreur avec sa position et son code ;
rester utilisable au clavier et par lecteur d'écran (WCAG 2.2 AA) ; ne jamais être
interprété autrement que par l'analyseur publié. [R15]

Acceptation : à écrire avec la spec qui le concevra.

### G. Transverse

#### REQ-018-31 — Parcours de bout en bout
kartenn doit fournir un parcours Playwright (Chromium, Firefox, axe-core à chaque étape) :
un éditeur filtre les préfixes par environnement, en change trois en place (tableau et
arbre), lit le différentiel, soumet ; une erreur affiche son code et le copie ; un rôle
cite les nouveaux noms. [REQ-007-34]

Acceptation :
- Le parcours passe dans `k-web-e2e`.

#### REQ-018-32 — Budgets
L'édition en place et les libellés ne doivent pas dégrader les budgets de REQ-007-32
(page de 100 lignes rendue en moins de 100 ms après la réponse) ni ceux de D14 (lecture
de liste filtrée < 150 ms). [D14, REQ-007-32]

Acceptation :
- `bench-core` : liste filtrée avec `labels` dans les budgets ; Vitest indicatif : 100
  lignes éditables rendues sous 100 ms.

## Non-objectifs

- Le langage de requête écrit (REQ-018-30 en pose les conditions).
- La traduction des messages libres du serveur (`detail` d'un problème, `message` d'une
  anomalie) : ils restent dans la langue du serveur, le code les accompagne (R11).
- Le renommage des sortes d'objets (R4).
- L'édition en place des objets sans droit lisible par `/v1/me` (le serveur reste juge).
- Les libellés des clés de propriétés écrites par un utilisateur dans une saisie libre.

## Questions ouvertes

- [ ] Q-018-1 à Q-018-8 ([`decisions.md`](decisions.md)) : à arbitrer par l'utilisateur.
- [ ] R12, R14, R17, R18 : choix de Claude à confirmer par les maquettes (tâche 1).

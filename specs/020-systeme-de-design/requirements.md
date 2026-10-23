---
status: draft
updated: 2026-10-23
---

# 020 — Système de design : exigences

## Résumé

kartenn se dote d'un **système de design** avant de réécrire son interface : un brief
validé, des **jetons** (couleurs claires et sombres, états, environnements, espacements,
rayons, bordures, élévation, typographie, densité, mouvement), une **architecture CSS en
couches** verrouillée par le lint, des **icônes** d'un seul style, une **bibliothèque de
composants** `@kartenn/ui` d'environ 25 composants accessibles, un **catalogue vivant**
(`/_ui`) où chaque composant figure dans tous ses états, des **maquettes** des écrans clés
construites avec ces mêmes composants, et une **régression visuelle** bloquante. Le shell
v2 (spec 021) et tous les plugins de première partie (VLAN, Machines, Politique, Flux)
s'écrivent avec cette bibliothèque, et rien d'autre.

Sources : arbitrages de l'utilisateur du 2026-10-23 (débat web D-1 à D-12, trois tours de
brief, icônes, police, direction « Ardoise ») consignés dans [`decisions.md`](decisions.md) ;
retours de l'utilisateur sur les maquettes du 2026-10-22 (cohérence des tailles, édition
en place, filtres typés, clé et libellé, codes visibles) ; D6, D17, D37 et Q-ouv-1, Q-ouv-6
([`../001-cadrage/decisions.md`](../001-cadrage/decisions.md)) ; spec 007 (W12 à W14,
REQ-007-30, 31, 33) ; ADR 0024, 0025, [0036](../../docs/adr/0036-systeme-de-design-kartenn-ui.md),
[0038](../../docs/adr/0038-partage-des-composants-avec-les-plugins.md),
[0039](../../docs/adr/0039-outillage-claude-qualite-interface.md) ; note
[`web-navigateur.md`](../../docs/reference/web-navigateur.md) (CSP, Custom Elements,
WCAG 2.2, APG).

**Brouillon du 2026-10-23.** Les arbitrages de fond sont rendus ; restent les
« Arbitrages demandés » et les questions Q-020-1 à 10 de [`decisions.md`](decisions.md).
Le framework (D-1) attend un prototype comparatif : les exigences sont écrites sans lui,
les points qui en dépendent sont marqués **[D-1]**. Rien n'est implémenté.

Ordre (D31 amendée) : la 022 (événements poussés) précède la 020 et la 021 ; la 020
précède les écrans de la 021 et ceux de la 017.

### Vocabulaire

- **Jeton** : propriété personnalisée CSS `--k-*`, seule source d'une valeur de style.
  *Primitif* (une teinte, un pas d'espacement), *sémantique* (un rôle : surface, texte
  atténué, état d'erreur) ou *de composant* (hauteur de ligne d'une table).
- **Densité** : jeu de valeurs de hauteur et d'espacement ; *compacte* (défaut) ou
  *confortable*.
- **Catalogue** : route de développement `/_ui` du shell qui montre chaque composant dans
  chacun de ses états, densités et modes.
- **Maquette** : écran clé construit dans le catalogue (`/_ui/ecrans/…`) avec les vrais
  composants et des données factices anonymisées.
- **Capture de référence** : image validée d'un état du catalogue ou d'une maquette, à
  laquelle chaque exécution compare le rendu.
- **Motif APG** : modèle d'interaction clavier et ARIA du WAI-ARIA Authoring Practices
  Guide (w3.org/WAI/ARIA/apg/patterns/).
- **Clé** et **libellé** : identifiant interne stable d'une valeur (`prod`) et texte affiché
  dans la langue de l'utilisateur (« Production »).
- **Code** : identifiant stable d'un problème ou d'une violation de l'API (`type` d'un
  problème RFC 9457, `code` d'une violation), à montrer tel quel.

## Exigences

### A. Brief et principes

#### REQ-020-1 — Brief validé
Le système de design doit suivre le brief validé (design, § « Brief ») : outil de travail
dense et quotidien ; neutres teintés de bleu, accent bleu du logo ; couleur réservée au
sens (états, environnements, différentiel), jamais seule porteuse d'information ; petits
rayons, bordures fines teintées, ombres pour les couches flottantes seulement ; aucune
animation gratuite. [B1, B2, WCAG 1.4.1]

Acceptation :
- Revue `ui-reviewer` de chaque maquette contre le brief : aucun écart non accepté.
- Chaque état coloré du catalogue porte aussi un libellé ou une icône.

#### REQ-020-2 — Densité
Le système doit offrir deux densités, **compacte par défaut** (lignes de table et d'arbre
de 28 px, contrôles de 28 px) et confortable (36 px), commutables par l'utilisateur et
gardées localement ; une cible interactive doit rester d'au moins 24 × 24 px dans les deux
densités. [B3, WCAG 2.5.8]

Acceptation :
- Le catalogue montre chaque composant dans les deux densités ; un test mesure la hauteur
  des lignes (28 et 36 px) et la taille des cibles (≥ 24 px).

#### REQ-020-3 — Thème
Le système doit suivre le thème clair ou sombre du système d'exploitation, et accepter un
choix forcé par l'utilisateur (`data-theme="light"` ou `"dark"` sur la racine), gardé
localement. [B4]

Acceptation :
- `prefers-color-scheme: dark` sans choix → mode sombre ; choix « clair » → mode clair
  quel que soit le système.

### B. Jetons

#### REQ-020-4 — Jetons à trois niveaux, source unique
Toute valeur de style (couleur, taille, espacement, rayon, bordure, ombre, durée, courbe,
police, hauteur de ligne) doit provenir d'un jeton `--k-*` défini dans le seul fichier de
jetons de `@kartenn/ui` ; les jetons sémantiques doivent être redéfinis pour le mode
sombre et la densité confortable, jamais les composants. [B5, ADR 0036]

Acceptation :
- Stylelint refuse une valeur brute hors du fichier de jetons (REQ-020-13).
- La liste des jetons publics (`tokens` du SDK) est égale à celle du fichier, par test.

#### REQ-020-5 — Couleurs de départ
Les jetons de couleur doivent partir des valeurs de la direction « Ardoise » (design,
§ « Couleurs »), en clair et en sombre, ajustées seulement là où un contraste exigé
n'est pas tenu ; tout ajustement est consigné. [B6, A6]

Acceptation :
- Le fichier de jetons reprend la table du design ; chaque écart à « Ardoise » y est
  justifié par un rapport de contraste.

#### REQ-020-6 — Contrastes vérifiés
Un test doit calculer, dans les deux modes, le contraste de chaque paire sémantique
déclarée (texte sur fond ≥ 4,5:1 ; texte large, icône porteuse de sens, bordure de
contrôle et anneau de focus sur leur fond ≥ 3:1) et échouer sinon. [WCAG 1.4.3, 1.4.11,
REQ-007-33]

Acceptation :
- `web/ui` : test de contraste vert ; une paire dégradée sous le seuil le fait échouer.

#### REQ-020-7 — Couleurs d'état
Le système doit définir quatre états sémantiques, chacun en texte, fond atténué et
bordure, dans les deux modes : `ok` (vert), `warn` (ambre), `ko` (rouge), `review` (violet,
« en revue »), plus `info` (accent) et `neutral` ; un état est toujours affiché avec son
libellé ou son icône. [B7, WCAG 1.4.1]

#### REQ-020-8 — Couleurs d'environnement
Le système doit offrir une palette fermée de couleurs d'environnement (six teintes
distinctes entre elles et des états, dans les deux modes), affichées en pastille à côté du
libellé de l'environnement, jamais seules. [B8, Q-020-1]

Acceptation :
- Le catalogue montre les six teintes à côté de leur libellé, dans les deux modes ; un
  test vérifie qu'aucune teinte d'environnement n'est égale à une couleur d'état.

#### REQ-020-9 — Espacement, rayons, bordures, élévation
Le système doit fixer une échelle d'espacement sur une base de 4 px, quatre rayons
(3, 5, 8 px et un rayon plein pour les pastilles), une épaisseur de bordure de 1 px
teintée (fine, forte, de contrôle), et des ombres réservées aux **couches flottantes**
(menu, popover, infobulle, dialogue, tiroir, toast). [B9]

Acceptation :
- Stylelint refuse `box-shadow` hors des composants flottants (liste fermée).

#### REQ-020-10 — Typographie
Le système doit servir **Geist** et **Geist Mono** (SIL OFL 1.1) en `woff2` depuis kartenn
(`font-src 'self'`), avec une échelle de tailles fermée (design), des **chiffres
tabulaires** pour les nombres, les dates et les colonnes, et la **chasse fixe** pour les
adresses, préfixes, VID, MAC, codes et identifiants. [B10, D-11]

Acceptation :
- Aucune requête de police hors de l'origine ; le rendu des captures ne dépend pas des
  polices installées.
- Une colonne de nombres reste alignée (même largeur de chiffres).

#### REQ-020-11 — Mouvement
Le système doit définir des durées (instantané, court, moyen) et des courbes en jetons,
réservées aux changements d'état utiles (ouverture d'une couche, apparition d'un toast,
dépliage) ; sous `prefers-reduced-motion: reduce`, toute transition de déplacement doit
être supprimée ou remplacée par un fondu court. [B11, WCAG 2.3.3]

Acceptation :
- Les captures et parcours tournent en `reducedMotion: 'reduce'` ; un test vérifie que les
  durées valent 0 sous cette préférence.

### C. Architecture CSS

#### REQ-020-12 — Couches
Toute feuille du shell, de `@kartenn/ui` et des plugins de première partie doit déclarer
ses règles dans une couche de l'ordre `@layer reset, tokens, base, components, plugins,
utilities` ; le CSS d'un plugin de première partie entre dans `plugins` sous le préfixe
`k-<plugin>-`. [B12, ADR 0036, 0038]

Acceptation :
- Stylelint refuse une règle hors couche ou une couche inconnue.

#### REQ-020-13 — Lint de style
Stylelint doit refuser, hors du fichier de jetons : toute couleur littérale, longueur
autre que `0`, `1px` de bordure et pourcentages, rayon, ombre, durée, courbe ou famille
de police littérale ; `!important` ; les sélecteurs d'identifiant ; une profondeur de
sélecteur au-delà de trois ; les propriétés physiques là où une propriété logique existe
(`margin-left` → `margin-inline-start`). Une règle de lint du code doit refuser les
éléments interactifs bruts (bouton, champ, liste de choix, table) dans les écrans du shell
et les plugins de première partie. [B13, D-4, ADR 0039] **[D-1]** pour la règle du code.

Acceptation :
- Un fichier de test avec chaque cas interdit fait échouer `k-web-check` ; le même fichier
  corrigé passe.

#### REQ-020-14 — Contraintes de la CSP
Aucun composant ne doit poser un attribut `style` en chaîne, ni injecter un élément
`<style>`, ni affecter `style.cssText` ; les valeurs dynamiques (position d'une couche,
largeur d'une colonne, décalage d'une ligne virtualisée) passent par
`style.setProperty` ou une variable CSS. Un parcours doit relever les violations CSP et
échouer s'il y en a. [B14, ADR 0025, 0038, web-navigateur.md § CSP Level 3]

Acceptation :
- Parcours du catalogue entier (chaque état ouvert) : zéro rapport `securitypolicyviolation`.

### D. Icônes

#### REQ-020-15 — Icônes
Le système doit utiliser les icônes **Lucide** (ISC), trait de 1,5 px, aux tailles 16 et
20 px, importées une à une ; les icônes métier absentes (préfixe, plage, VLAN, interface,
flux, environnement…) doivent être dessinées sur la même grille de 24 unités et le même
trait, et livrées par `@kartenn/ui`. Une icône seule porteuse de sens a un nom accessible ;
une icône décorative est cachée (`aria-hidden`). [B15]

Acceptation :
- Planche d'icônes au catalogue, Lucide et métier côte à côte, dans les deux tailles.
- Aucune police d'icônes, aucun sprite externe.

### E. Composants

#### REQ-020-16 — Inventaire et états
`@kartenn/ui` doit fournir les composants de l'inventaire (design, § « Inventaire »), et
chacun doit exister dans tous ses états pertinents : repos, survol, focus visible, actif,
désactivé, lecture seule, chargement, vide, erreur, sélectionné, en brouillon, en revue.
[B16, D-8]

Acceptation :
- Le catalogue a une page par composant ; chaque état listé au design y figure.

#### REQ-020-17 — Un seul jeu de tailles
Les contrôles doivent partager trois hauteurs (24, 28, 32 px en compacte ; 28, 36, 40 px
en confortable), un même alignement vertical du texte et des icônes, un même rembourrage
horizontal par taille ; un écran ne doit pas redéfinir la taille d'un composant. [B17,
retour du 2026-10-22]

Acceptation :
- Capture d'alignement au catalogue : bouton, champ, liste de choix, pastille de filtre
  et bouton-icône côte à côte, lignes de base alignées.

#### REQ-020-18 — Motifs APG et clavier
Chaque composant interactif doit suivre le motif APG nommé dans l'inventaire (rôles,
états ARIA, clavier) et avoir un test clavier automatisé qui rejoue ce motif ; un widget
fourni par la bibliothèque de primitives retenue ne doit pas être réécrit. [B18,
REQ-007-33] **[D-1]** pour la bibliothèque de primitives.

Acceptation :
- Par composant : test Playwright qui suit le motif (par exemple, arbre : flèches,
  Origine, Fin, `*`, saisie au clavier) et instantané ARIA (`toMatchAriaSnapshot`).

#### REQ-020-19 — Grille et arbre éditables
La grille de données et l'arbre doivent : se virtualiser (hauteur de ligne fixe tirée de
la densité, `aria-rowcount`/`aria-rowindex`, `aria-level`/`aria-setsize`/`aria-posinset`) ;
offrir l'**édition en place** d'une cellule quand l'hôte la permet (Entrée ou F2 pour
éditer, Échap pour annuler, Entrée ou Tab pour valider), avec validation, état
« en attente », « en brouillon », « en revue » et erreur portée par la cellule ; et, pour
un arbre à parent **déclaré** seulement, le **déplacement** d'un nœud selon REQ-020-31.
Le composant ne décide pas des droits : l'hôte lui dit quelles
cellules sont éditables. [B19, retour du 2026-10-22, WCAG 2.5.7]

Acceptation :
- Grille de 10⁴ lignes au catalogue : au plus quelques centaines de lignes dans le DOM ;
  édition d'une cellule au clavier seul ; erreur affichée sur la cellule et annoncée.
- Arbre de 10⁵ nœuds à parent déclaré : déplacement d'un nœud au clavier sans glisser.

#### REQ-020-31 — Déplacement contrôlé
L'arbre doit connaître le **mode de parenté** que lui donne l'hôte : `derived` (parent
déduit des données, comme l'arbre des préfixes par inclusion, D34) ou `declared` (parent
choisi, comme une entité, un site, un groupe de VLAN, une machine hébergée). En mode
`derived`, l'arbre ne doit offrir **aucun** déplacement : ni glisser, ni couper-coller, ni
action « Déplacer vers… ». En mode `declared` :
- pendant un glisser, chaque cible potentielle doit être vérifiée par la fonction
  `canMove(nœud, cible)` de l'hôte **avant** d'être mise en évidence ; une cible refusée
  montre un état de refus avec son motif (et le code de la règle serveur correspondante,
  quand il y en a un) ; un dépôt sur une cible refusée ne fait rien ;
- l'alternative au clavier (« Déplacer vers… ») ne liste que les cibles acceptées par
  `canMove` ;
- un dépôt accepté n'émet qu'une **intention** (`move({ nœud, cible })`) : l'arbre ne
  déplace jamais le nœud lui-même ; il montre le nœud dans un état « déplacement en
  attente » à sa place d'origine jusqu'à ce que l'hôte lui rende de nouvelles données ;
  un refus de l'hôte (état « déplacement refusé », avec code) est visible sur le nœud,
  resté à sa place. [B31, retour de l'utilisateur du 2026-10-23, WCAG 2.5.7]

Acceptation :
- Mode `derived` : aucun attribut `draggable`, aucune entrée de menu de déplacement, Ctrl+X
  sans effet.
- Mode `declared`, `canMove` qui refuse les descendants : survol d'un descendant pendant
  le glisser → état de refus avec motif, aucune mise en évidence de cible ; dépôt → aucun
  appel à `move`.
- « Déplacer vers… » ne propose aucune cible refusée par `canMove`.
- Dépôt accepté : le nœud reste à sa place, marqué « en attente », jusqu'aux nouvelles
  données de l'hôte.

#### REQ-020-32 — Invariant d'affichage de l'arbre
L'arbre ne doit afficher un nœud que sous le parent que lui donnent les **données** de
l'hôte (relation parent-enfant fournie par le modèle), jamais sous un parent déduit d'un
geste de l'utilisateur ; en développement et en test, l'arbre doit vérifier que chaque
ligne rendue a pour parent visuel le parent de son modèle, et lever une erreur sinon.
[B32, retour de l'utilisateur du 2026-10-23]

Acceptation :
- Test de propriété (Vitest) : suites aléatoires de dépôts, refus, réponses tardives et
  remplacements du modèle → à chaque rendu, parent visuel = parent du modèle.

#### REQ-020-20 — Filtres typés
Le système doit offrir une **pastille de filtre typé** (`propriété opérateur valeur`) et son
sélecteur : choix de la propriété dans une liste fermée avec autocomplétion, opérateurs
selon le type (égal, différent, contient, dans le préfixe, avant, après…), valeur par une
liste fermée avec autocomplétion quand le domaine est fermé (environnement, site, état),
sinon par un champ typé (préfixe, date, nombre). Un langage de requête écrit n'est pas
dans cette spec (Q-020-10). [B20, retour du 2026-10-22]

Acceptation :
- Au catalogue : ajout d'un filtre au clavier seul (motif combobox), modification et
  retrait d'une pastille ; une valeur hors type est refusée avant l'envoi.

#### REQ-020-21 — Clé et libellé
Le système doit offrir une **pastille clé/libellé** qui affiche le libellé dans la langue
de l'utilisateur, à défaut la clé, et donne accès à la clé (infobulle et copie) ; une
valeur sans libellé dans la langue affiche sa clé en chasse fixe, sans erreur. [B21,
retour du 2026-10-22]

Acceptation :
- `prod` avec libellés `{fr: "Production"}` : « Production » en français, `prod` en
  anglais (repli sur la clé) ; la clé est copiable dans les deux cas.

#### REQ-020-22 — Codes visibles
Le système doit offrir un **badge de code** en chasse fixe, copiable d'un clic ou au
clavier, placé à côté du message lisible ; tout état d'erreur montre le code du problème
(`type`, ou `code` de chaque violation) et l'identifiant de la requête (`Request-Id`),
tous deux copiables. [B22, retour du 2026-10-22]

Acceptation :
- Erreur 412 au catalogue : message, code `urn:kartenn:problem:precondition-failed`
  et identifiant de requête visibles et copiables ; la copie est annoncée.

#### REQ-020-23 — États des vues de données
Le système doit offrir, pour toute zone de données, un état de **chargement** en squelette
de taille fixe (aucun décalage à l'arrivée des données), un état **vide** (une phrase et
une action quand elle existe) et un état d'**erreur** (phrase, code, identifiant de
requête, action « Réessayer »). [B23, brief tour 3]

Acceptation :
- CLS mesuré à 0 entre squelette et données sur la grille et l'arbre du catalogue.

### F. Catalogue et maquettes

#### REQ-020-24 — Catalogue `/_ui`
Le shell doit servir, en développement et dans les builds de test, une route `/_ui` qui
liste chaque composant, chaque état, chaque densité et chaque mode, plus les jetons
(nuancier, échelles) et les icônes ; la route ne doit pas figurer dans le build de
production. [B24, D-8, Q-020-2]

Acceptation :
- Le build de production ne contient aucun module du catalogue (test sur le manifeste du
  build).

#### REQ-020-25 — Maquettes construites avec les vrais composants
Les écrans clés (design, § « Maquettes ») doivent être maquettés dans le catalogue avec
les composants de `@kartenn/ui` et des données factices anonymisées, dans les deux modes ;
aucun écran de la 021 ni de la 017 ne doit être développé avant la validation de sa
maquette par l'utilisateur, consignée dans [`decisions.md`](decisions.md). [B25, D-8,
017/U14]

Acceptation :
- Chaque maquette a ses captures de référence et une ligne de validation datée.

### G. Régression visuelle

#### REQ-020-26 — Captures bloquantes
Chaque état du catalogue et chaque maquette doivent avoir une capture de référence,
comparée à chaque exécution de la vérification web ; un écart au-delà du seuil doit faire
échouer la vérification. Les captures se font sur **Chromium seul**, références générées
**dans devenv** (navigateur et polices fournis par Nix), viewport et facteur d'échelle
fixes, animations coupées, horloge figée. [B26, D-9]

Acceptation :
- Un jeton de couleur modifié fait échouer les captures concernées ; la régénération passe
  par une commande devenv, jamais hors de devenv.

#### REQ-020-27 — Instantanés d'accessibilité
Chaque composant interactif et chaque maquette doivent avoir un instantané de l'arbre
d'accessibilité (rôles, noms, états), comparé à chaque exécution. [B27]

### H. Accessibilité

#### REQ-020-28 — WCAG 2.2 AA
Les composants et maquettes doivent satisfaire WCAG 2.2 AA : axe-core avec les règles
`wcag2a`, `wcag2aa`, `wcag21a`, `wcag21aa`, `wcag22aa` sans violation, bloquant ; focus
visible et jamais masqué par une couche collante (2.4.7, 2.4.11) ; cibles ≥ 24 px (2.5.8) ;
alternative au glisser (2.5.7) ; contenus au survol ou au focus maîtrisables (1.4.13) ;
reflow à 320 px CSS pour les écrans de consultation (1.4.10). [B28, REQ-007-33]

Acceptation :
- Parcours axe sur chaque page du catalogue et chaque maquette ; revue `a11y-auditor` par
  composant interactif, écarts consignés.

### I. Budgets

#### REQ-020-29 — Poids et rendu
`@kartenn/ui` doit tenir, mesuré au build et par `bench-web` (Chromium, CPU ralenti ×4) :
module JavaScript ≤ 90 Ko gzip, feuille ≤ 20 Ko gzip, polices préchargées ≤ 120 Ko ;
rendu d'une grille de 100 lignes ≤ 100 ms ; défilement d'un arbre de 10⁵ nœuds avec
≤ 5 % de trames perdues ; aucune tâche longue (> 50 ms) à l'ouverture d'une couche. Le
banc doit échouer si un budget est dépassé. [B29, D-10, REQ-007-32, Q-020-9] **[D-1]**
pour le poids du framework, compté à part.

### J. Licences

#### REQ-020-30 — Licences
`@kartenn/ui` doit être publié sous **MPL-2.0** (comme le SDK, Q-ouv-1) ; les licences des
polices (OFL) et des icônes (ISC) doivent être livrées avec le shell et listées par le
test de licences existant. [B30, D37, REQ-007-38]

## Non-objectifs

- Le langage de requête écrit (avec autocomplétion et coloration) : plus tard, spec
  propre (Q-020-10).
- Les écrans eux-mêmes : spec 021 (shell) et 017 (Politique, Flux).
- Les données des libellés par langue (stockage, versionnement) : côté serveur, spec 018
  (non fusionnée) ; la 020 n'offre que leur affichage.
- Un outil de design vectoriel ou un service de comparaison de captures externe.
- Des thèmes au-delà de clair et sombre (contraste élevé : à rouvrir si demandé).

## Questions ouvertes

Q-020-1 à 10, avec recommandations : [`decisions.md`](decisions.md), § « Questions ».

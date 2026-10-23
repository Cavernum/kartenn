# Système de design : WCAG 2.2, motifs APG, cascade CSS, captures Playwright, licences

Note de lecture pour la spec [020-systeme-de-design](../../specs/020-systeme-de-design/).
Recherche documentaire du 2026-10-23, complétée le 2026-10-24 par la lecture des fichiers
de licence des paquets épinglés.

Méthode : les pages ont été lues sur leurs sites officiels avec un outil qui renvoie des
extraits et des résumés, pas le texte brut. Les citations entre guillemets sont des
fragments courts relevés dans ces extraits ; le reste est paraphrasé. **Toute citation
reste à vérifier mot à mot** sur la page rendue. Les numéros de section des specs CSS
viennent des extraits et sont **à vérifier**. Les fichiers de licence des paquets (Geist,
Geist Mono, Lucide) ont été lus en entier, dans les archives publiées des versions
épinglées. Légende : **[N]** exigence normative ; **[I]** guide informatif ; **[T]** choix
de kartenn. Cette note ne répète pas [`web-navigateur.md`](web-navigateur.md)
(§ « Accessibilité : WCAG 2.2 AA », § « WAI-ARIA, APG, axe-core », § « CSP Level 3 ») :
elle la complète et la cite.

## Sources

| Document | Statut | Consulté |
|---|---|---|
| [WCAG22] [WCAG 2.2](https://www.w3.org/TR/WCAG22/) | Recommendation W3C du 2024-12-12 (`/TR/2024/REC-WCAG22-20241212/`) ; errata **non relus** | 2026-10-23 |
| [ARIA] [WAI-ARIA 1.2](https://www.w3.org/TR/wai-aria-1.2/) | Recommendation du 2023-06-06 ; errata **non relus** | 2026-10-23 |
| [APG] [ARIA Authoring Practices Guide, motifs](https://www.w3.org/WAI/ARIA/apg/patterns/) (button, checkbox, combobox, dialog-modal, alertdialog, disclosure, grid, menu-button, menu, switch, tabs, tooltip, treegrid, treeview, breadcrumb) | guide, non daté | 2026-10-23 |
| [APG-RMF] [APG, Read Me First](https://www.w3.org/WAI/ARIA/apg/practices/read-me-first/) | idem | 2026-10-23 |
| [CASCADE5] [CSS Cascading and Inheritance Level 5](https://www.w3.org/TR/css-cascade-5/) | l'extrait lu se déclare « Candidate Recommendation Snapshot, 13 January 2022 » ; statut courant **à vérifier** | 2026-10-23 |
| [MQ5] [Media Queries Level 5](https://www.w3.org/TR/mediaqueries-5/) | Working Draft du 2026-02-19 | 2026-10-23 |
| [CSP3], [CSSOM], [HTML] | voir `web-navigateur.md` § « Sources » | — |
| [MDN-STYLE] [MDN, `style-src`](https://developer.mozilla.org/en-US/docs/Web/HTTP/Reference/Headers/Content-Security-Policy/style-src) | documentation, pas une norme | 2026-10-23 |
| [PW-SNAP] [Playwright, visual comparisons](https://playwright.dev/docs/test-snapshots) ; [PW-API] [`PageAssertions`](https://playwright.dev/docs/api/class-pageassertions) ; [PW-ARIA] [aria snapshots](https://playwright.dev/docs/aria-snapshots) ; [PW-CLOCK] [clock](https://playwright.dev/docs/clock) ; [PW-EMU] [emulation](https://playwright.dev/docs/emulation) | documentation officielle, version non indiquée sur les pages | 2026-10-23 |
| [OFL] [SIL OFL 1.1, texte officiel](https://openfontlicense.org/open-font-license-official-text/) ; [OFL-FAQ](https://openfontlicense.org/ofl-faq/) | licence ; FAQ informative | 2026-10-23 |
| [GEIST] `LICENSE` des paquets [`@fontsource-variable/geist`](https://www.npmjs.com/package/@fontsource-variable/geist) et [`@fontsource-variable/geist-mono`](https://www.npmjs.com/package/@fontsource-variable/geist-mono) 5.3.0, livrés tels quels dans `web/v2/ui/src/fonts/OFL-*.txt` | fichiers lus en entier | 2026-10-24 |
| [LUCIDE] `LICENSE` du paquet [`lucide`](https://www.npmjs.com/package/lucide) 1.61.0 ([dépôt](https://github.com/lucide-icons/lucide)), livré dans `web/v2/ui/src/icons/LICENSE-lucide.txt` | fichier lu en entier | 2026-10-24 |
| [MPL] [MPL 2.0](https://www.mozilla.org/en-US/MPL/2.0/) et sa [FAQ](https://www.mozilla.org/en-US/MPL/2.0/FAQ/) | licence ; FAQ informative ; texte de la licence lu seulement pour le copier dans `web/v2/ui/LICENSE` | 2026-10-23 |

## Ce que disent les sources

### 1. WCAG 2.2 : critères retenus (tous [N])

Texte de [WCAG22], ancre `#<nom-du-critère>`, fragments littéraux (à vérifier).

- **1.4.1 Use of Color (A)** : « Color is not used as the only visual means of conveying
  information, indicating an action, prompting a response, or distinguishing a visual
  element. »
- **1.4.3 Contrast (Minimum) (AA)** : « The visual presentation of text and images of text
  has a contrast ratio of at least 4.5:1 », sauf : texte grand (« at least 3:1 ») ;
  composant inactif, décoration pure, texte invisible ou partie d'une image riche ;
  logotype.
- **1.4.10 Reflow (AA)** : pas de perte d'information ni de fonction, sans défilement dans
  deux dimensions, « at a width equivalent to 320 CSS pixels » (contenu à défilement
  vertical) ou « at a height equivalent to 256 CSS pixels » (horizontal) ; exception : les
  parties qui exigent une mise en page à deux dimensions « for usage or meaning ».
- **1.4.11 Non-text Contrast (AA)** : au moins 3:1 contre les couleurs adjacentes pour
  (a) l'information visuelle nécessaire pour identifier les composants d'interface et leurs
  états, sauf composant inactif ou apparence fixée par l'agent utilisateur ; (b) les parties
  de graphiques nécessaires à la compréhension.
- **1.4.13 Content on Hover or Focus (AA)** : le contenu affiché au survol ou au focus est
  fermable sans déplacer le pointeur ni le focus (Dismissible), survolable (Hoverable) et
  persistant jusqu'au retrait du déclencheur, à sa fermeture ou à sa péremption
  (Persistent).
- **2.1.4 Character Key Shortcuts (A)** : un raccourci fait de seuls caractères doit
  pouvoir être désactivé, ou reconfiguré avec une touche non imprimable, ou n'être actif
  qu'au focus du composant (« only active when that component has focus »).
- **2.2.1 Timing Adjustable (A)** : toute limite de temps fixée par le contenu peut être
  désactivée, réglée (au moins dix fois la valeur par défaut) ou prolongée après
  avertissement (« at least 20 seconds ») ; exceptions : temps réel, limite essentielle,
  plus de 20 heures.
- **2.3.3 Animation from Interactions (AAA)** : « Motion animation triggered by
  interaction can be disabled, unless the animation is essential ». Niveau AAA, hors de
  l'objectif AA ; voir Implications 12.
- **2.4.7 Focus Visible (AA)** : « Any keyboard operable user interface has a mode of
  operation where the keyboard focus indicator is visible. »
- **2.4.11 Focus Not Obscured (Minimum) (AA)** : « When a user interface component
  receives keyboard focus, the component is not entirely hidden due to author-created
  content. » Notes du critère **non relues**.
- **2.5.7 Dragging Movements (AA)** : toute fonction par glissement « can be achieved by a
  single pointer without dragging », sauf glissement essentiel ou fixé par l'agent
  utilisateur.
- **2.5.8 Target Size (Minimum) (AA)** : « at least 24 by 24 CSS pixels », sauf
  espacement (un cercle de 24 px centré sur la cible ne coupe aucune autre cible),
  équivalent sur la page, cible dans le texte, contrôle de l'agent utilisateur, présentation
  essentielle.
- **Définitions** (glossaire, à vérifier) : rapport de contraste « (L1 + 0.05) / (L2 +
  0.05) » ; luminance relative « L = 0.2126 * R + 0.7152 * G + 0.0722 * B », chaque canal
  linéarisé par « if RsRGB <= 0.04045 then R = RsRGB/12.92 else R = ((RsRGB+0.055)/1.055)
  ^ 2.4 » (le seuil valait 0.03928 avant mai 2021). Grand texte : 18 points, ou 14 points
  gras.

### 2. WAI-ARIA 1.2 et APG

- [ARIA] est la spécification **normative** ; ses exigences sur les rôles et attributs
  (`aria-setsize`, `aria-rowcount`…) sont citées dans `web-navigateur.md`.
- [APG] est **informatif** : [APG-RMF] dit qu'il illustre l'usage d'ARIA 1.2, qu'il faut
  éprouver les exemples avec les technologies d'assistance avant de les reprendre, et que
  les exemples « intentionally do not describe and implement coding techniques for working
  around problems ». Le mot « non normatif » n'apparaît pas dans les pages lues : **à
  vérifier** (pages « About », « Introduction »).

Résumé par motif ([APG] `…/patterns/<motif>/`, paraphrase) :

| Motif | Rôles, états, propriétés | Clavier |
|---|---|---|
| `button` | `button` ; `aria-pressed` (bascule) ; `aria-disabled` ; nom par le contenu, `aria-labelledby` ou `aria-label` | Espace et Entrée activent |
| `checkbox` | `checkbox` ; `aria-checked` `true`/`false`/`mixed` ; groupe `group` étiqueté | Espace |
| `combobox` | `combobox` ; `aria-expanded`, `aria-controls`, `aria-activedescendant`, `aria-autocomplete` ; popup `listbox`, `tree`, `grid` ou `dialog` | Bas entre dans le popup ; Alt+Bas l'ouvre ; Échap le ferme ; Entrée accepte ; le popup est hors de la séquence de tabulation |
| `dialog-modal` | `dialog`, `aria-modal="true"`, `aria-labelledby`, `aria-describedby` facultatif | Tab boucle dedans ; Échap ferme ; focus initial dedans ; retour au déclencheur |
| `alertdialog` | `alertdialog`, `aria-modal`, `aria-labelledby`, `aria-describedby` sur le message | comme le dialogue modal ; focus initial sur l'action la moins destructrice **non trouvé** dans la page lue |
| `disclosure` | bouton, `aria-expanded`, `aria-controls` facultatif | Entrée, Espace |
| `grid` | `grid`, `row`, `gridcell`, `columnheader`, `rowheader` ; `aria-rowcount`/`colcount` et `aria-rowindex`/`colindex` hors DOM ; `aria-selected`, `aria-sort`, `aria-readonly` | flèches ; Page préc./suiv. ; Origine/Fin ; Ctrl+Origine/Fin ; Entrée ou F2 éditent ; Échap revient ; un seul élément de la grille dans la séquence de tabulation |
| `menu-button`, `menu` | `button` + `aria-haspopup`, `aria-expanded` ; `menu`, `menuitem`, `menuitemcheckbox`, `menuitemradio` | Entrée, Espace, Bas ouvrent ; Haut/Bas, Origine/Fin ; Entrée active et ferme ; Échap ferme et rend le focus ; Tab ne parcourt pas les items |
| `switch` | `switch`, `aria-checked` ; l'étiquette ne change pas avec l'état | Espace (Entrée facultatif) |
| `tabs` | `tablist`, `tab` (`aria-selected`, `aria-controls`), `tabpanel` (`aria-labelledby`) | Gauche/Droite bouclent ; Origine/Fin ; activation automatique ou manuelle (Espace, Entrée) |
| `tooltip` | `tooltip`, `aria-describedby` sur le déclencheur | Échap ferme, le focus reste ; page marquée « work in progress », sans consensus |
| `treegrid` | `treegrid` ; `aria-expanded` sur les lignes parentes ; `aria-selected` | Droite déplie ou entre ; Gauche replie ; Haut/Bas ; Origine/Fin |
| `treeview` | `tree`, `treeitem`, `group` ; `aria-expanded` ; `aria-level`/`setsize`/`posinset` quand l'arbre n'est pas entier dans le DOM | Haut/Bas ; Droite ouvre ou descend ; Gauche ferme ou remonte ; Origine/Fin ; saisie de caractères recommandée ; `*` facultatif |
| `breadcrumb` | `nav` étiqueté ; `aria-current="page"` | aucun clavier propre |

### 3. Couches de cascade ([CASCADE5], § 6, ancre `#layering`)

- Les couches se trient « in the order in which they first are declared » : une
  instruction `@layer a, b, c;` fixe l'ordre avant tout contenu.
- Les styles hors couche forment une couche implicite, la dernière.
- Déclarations normales : « the declaration whose cascade layer is last wins », quelle que
  soit la spécificité ; `!important` : l'ordre s'inverse.
- Couches imbriquées triées dans leur parent ; `@import … layer(nom)` place la feuille
  dans la couche.

### 4. CSSOM et CSP `style-src`

- Voir `web-navigateur.md` § « CSP Level 3 » : `<style>` relève de `style-src-elem`,
  l'attribut `style` et `setAttribute('style', …)` de `style-src-attr` ; aucune violation
  observée pour `style.setProperty`.
- [MDN-STYLE], « Violation cases » : `<style>`, attributs `style` et `cssText` présentés
  comme bloqués ; écriture d'une propriété (`el.style.display = "none"`) non bloquée ;
  une note de la même page dit qu'aucun navigateur ne bloque aujourd'hui les méthodes
  CSSOM qui analysent des chaînes, ce qui contredit son propre exemple `cssText`. Le test
  WPT cité par `web-navigateur.md` ne relève pas de violation pour `cssText` : écart non
  tranché.

### 5. `prefers-reduced-motion` ([MQ5] § 12.1, à vérifier)

- [N] La fonctionnalité détecte que l'utilisateur a demandé au système de « minimize the
  amount of non-essential motion it uses » ; `reduce` : préférence pour une interface qui
  « removes or replaces » les animations de mouvement.

### 6. Déterminisme des captures Playwright

- [PW-API] `toHaveScreenshot` : `animations` (défaut `"disabled"`) arrête animations et
  transitions CSS et animations Web (finies avancées à leur fin, infinies ramenées au
  début) ; `caret` (défaut `"hide"`) ; `maxDiffPixelRatio` (0 à 1) ; `threshold` (défaut
  0,2, écart de couleur en YIQ) ; `scale` (défaut `"css"`) ; `mask`.
- [PW-SNAP] : « Browser rendering can vary based on the host OS, version, settings,
  hardware » ; générer et comparer les références dans le même environnement ; les noms
  des références portent le navigateur et la plateforme.
- [PW-EMU] : `viewport`, `deviceScaleFactor`, `colorScheme`, `locale`, `timezoneId` (qui
  n'agit que sur le navigateur, pas sur le lanceur) ; l'option `reducedMotion` n'est pas
  décrite sur cette page (**à vérifier** sur l'API de `BrowserContext`).
- [PW-CLOCK] : `setFixedTime` fige `Date.now()` et `new Date()`, les minuteries
  continuent ; `install`, `pauseAt` les arrêtent.
- [PW-ARIA] `toMatchAriaSnapshot` : gabarit YAML `role "nom" [attribut]`, ordre
  significatif, correspondance partielle (`contain` par défaut, `equal`, `deep-equal`) ;
  un seul fichier `.aria.yml` pour tous les navigateurs.

### 7. Licences

- **SIL OFL 1.1** ([OFL]) : « Modified Version » couvre l'ajout, la suppression ou la
  substitution de composants, conversion de format comprise ; conditions : pas de vente de
  la police seule ; notice de copyright et licence jointes à chaque copie (fichier texte,
  en-tête ou métadonnées) ; pas de nom réservé sur une version modifiée sans permission ;
  pas d'usage des noms des auteurs pour la promouvoir ; la police reste sous OFL.
  [OFL-FAQ] (informatif) : retirer des glyphes pour le Web est une modification permise,
  le nom réservé n'y est généralement pas permis (2.6) ; WOFF2 sans retouche des données
  n'est pas une modification (2.2.1) ; héberger la police pour `@font-face` est permis
  (1.8, 2.1).
- **Geist et Geist Mono** ([GEIST]) : `LICENSE` lu en entier : « Copyright 2024 The Geist
  Project Authors », « licensed under the SIL Open Font License, Version 1.1 », texte de
  la licence à la suite ; **aucun nom réservé** déclaré (« with Reserved Font Name »
  absent). Les fichiers des paquets sont des sous-ensembles par plage Unicode, en WOFF2.
- **Lucide** ([LUCIDE]) : `LICENSE` lu en entier : ISC (« Copyright (c) 2026 Lucide
  Icons and Contributors », notice et permission « appear in all copies »), puis MIT pour
  une liste d'icônes « derived from the Feather project », dont plusieurs de la liste
  fermée de kartenn (`check`, `chevron-*`, `plus`, `minus`, `x`, `search`, `info`,
  `lock`, `server`, `calendar`, `clock`, `command`, `database`, `external-link`,
  `log-out`, `move`, `trash`) ; la liste se relit à chaque montée.
- **MPL 2.0** ([MPL], FAQ) : copyleft par fichier ; un ensemble plus large qui l'importe
  garde sa propre licence ; un fichier modifié redistribué reste sous MPL.

## Implications pour kartenn

1. [N] 1.4.3, 1.4.11 : 4,5:1 et 3:1 sont des minima sans arrondi ; formule de WCAG 2.2
   (seuil 0,04045). Appliqué : `web/v2/ui/test/tokens.test.ts` calcule toutes les paires
   déclarées, `color-mix` en Oklab compris ; il a conduit à foncer `ok` et `warn` en clair
   (spec 020, design § « Couleurs »).
2. [N] 1.4.1 : tout état porté par la couleur a un second moyen (icône, libellé, signe).
   [T] `StatusBadge`, `EnvChip`, signes du différentiel (skill `kartenn-ui`, § 1).
3. [N] 2.1.4 : les raccourcis à une touche se désactivent (réglage de la 021) ; ceux d'un
   composant (saisie de caractères de l'arbre, `*`) ne sont actifs qu'à son focus.
4. [N] 2.2.1 : un toast qui se ferme seul est une limite de temps. [T] pause au survol et
   au focus, erreurs persistantes (020, `Toast`).
5. [N] 2.4.7, 2.4.11 : aucune règle axe-core pour ces critères ; [T] `scroll-padding` sous
   les barres collantes et vérification par parcours et `a11y-auditor`.
6. [N] 2.5.8 : contrôles de 24 px au moins dans les deux densités (jetons
   `--k-control-height-*`, testés). 2.5.7 : tout glisser a son alternative (« Déplacer
   vers… », Couper/Coller, 020 REQ-020-31).
7. [N] 1.4.13 : infobulles fermables par Échap, survolables, persistantes ; l'APG Tooltip
   étant sans consensus, [T] le respect de 1.4.13 se prouve par test.
8. [N] 1.4.10 : reflow à 320 px pour les écrans de consultation (REQ-020-28), les tables de
   données relevant de l'exception à deux dimensions.
9. [I] Les motifs APG sont informatifs : les exigences opposables sont ARIA et WCAG ; [T]
   chaque script clavier cite son motif. À trancher composant par composant : tabindex
   itinérant ou `aria-activedescendant` ; activation des onglets (manuelle, 020) ;
   bouclage des menus ; nombre de lignes de Page préc./suiv. dans la grille ; focus initial d'un
   `alertdialog`.
10. [N] Couches : l'ordre est déclaré une fois, en tête de `@kartenn/ui/style.css`
    (`styles/layers.css`) ; une règle hors couche battrait toute règle en couche : la
    règle Stylelint `kartenn/known-layer` l'interdit ; `!important` est refusé par le lint.
11. [N] CSP : REQ-020-14 tel quel ; l'absence de contrôle de `setProperty` est un
    comportement observé des moteurs, surveillé par le parcours `securitypolicyviolation`.
12. [N] Mouvement réduit : « removes or replaces » ; un fondu court est permis. 2.3.3 est
    AAA : [T] gardé comme REQ-020-11 (durées nulles sous `reduce`, sauf
    `--k-duration-fade`), hors de l'objectif AA et des étiquettes axe `wcag22aa`.
13. [T] Captures : Chromium seul, références générées et comparées dans devenv (navigateur
    et polices de Nix), `deviceScaleFactor: 1`, `animations: 'disabled'`,
    `caret: 'hide'`, `maxDiffPixelRatio: 0.001`, `threshold` par défaut ; `setFixedTime`
    ne fige pas les minuteries : les indicateurs de chargement et les temps relatifs se
    stabilisent par les données et `mask`.
14. [T] Instantanés ARIA en `contain` par défaut ; `equal` pour les composants à
    structure fixe (menus, onglets).
15. [N] OFL : licence et copyright livrés avec les fichiers (`OFL-Geist.txt`,
    `OFL-GeistMono.txt`, testés) ; sans nom réservé, la famille garde le nom « Geist »
    dans `@font-face` ; les fichiers des paquets sont livrés sans retouche.
16. [N] Lucide : `LICENSE-lucide.txt` (ISC et MIT) livré avec les icônes, testé ; la liste
    des icônes dérivées se relit à chaque montée de `lucide`. Les icônes métier de
    kartenn sont des dessins originaux, sous la MPL de `@kartenn/ui`.
17. [T] `@kartenn/ui` sous MPL-2.0 (`web/v2/ui/LICENSE`, en-tête dans chaque source,
    testés) : le shell (AGPL) et les plugins qui l'importent gardent leur licence.

## Non trouvé ou non relu

- Errata de WCAG 2.2 et d'ARIA 1.2 ; notes de 2.4.11 ; « Understanding » de 2.3.3, 2.1.4,
  2.2.1.
- Statut courant de CSS Cascade 5 ; numéros de section de CASCADE5 et MQ5.
- APG : mention « non normatif » ; focus initial de `alertdialog` ; `aria-level`,
  `posinset`, `setsize` du treegrid ; gestion du focus de la grille.
- Playwright : versions minimales de `toMatchAriaSnapshot` et `page.clock` ; option
  `reducedMotion` ; `snapshotPathTemplate` ; comportement de `performance.now` sous
  `setFixedTime`.
- MDN contre WPT sur `cssText` : écart à éprouver par le parcours CSP.
- Toutes les citations de seconde main (outil de résumé) sont à recouper mot à mot.

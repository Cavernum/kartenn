---
name: kartenn-ui
description: Contrat d'écriture de l'interface de kartenn (nouveau shell web/v2, bibliothèque @kartenn/ui, plugins de première partie) — jetons, couches CSS, composants et leurs motifs APG, états des vues, CSP, raccourcis réservés, pièges de Reka UI, clichés à éviter. À suivre pour toute écriture ou relecture sous web/.
paths:
  - "web/**"
---

# kartenn-ui — écrire l'interface de kartenn

Sources : spec 020 (`specs/020-systeme-de-design/`, design § « Brief », « Jetons »,
« Architecture CSS », « Inventaire des composants »), spec 021 (shell v2, § « Raccourcis »),
ADR 0036 à 0039, `docs/reference/systeme-de-design.md`. Ce skill les résume ; en cas
d'écart, la spec fait foi. Le code nouveau vit sous `web/v2/` (espace pnpm distinct,
non servi avant la bascule) ; `web/` ne reçoit que des correctifs.

## 1. Contrat

1. **Seulement `@kartenn/ui` et les jetons.** Un écran ou un plugin n'écrit ni bouton, ni
   champ, ni liste de choix, ni table brute (lint), n'importe pas `reka-ui` (lint), et ne
   pose aucune valeur brute : couleur, longueur, rayon, ombre, durée, police viennent d'un
   `var(--k-…)` (Stylelint). La seule source des valeurs est
   `web/v2/ui/src/tokens/tokens.css`.
2. **Tout composant nouveau remonte à l'agent principal.** Jamais de composant local à un
   écran qui double un composant de la bibliothèque ; la bibliothèque s'écrit par un seul
   agent à la fois (ADR 0039, § 6), un composant (ou une petite famille) par tâche, avec
   sa page de catalogue, ses captures et son script APG.
3. **Toute vue de données a trois états** : chargement en squelette de taille fixe ; vide
   avec une phrase et une action ; erreur avec le code copiable (`CodeBadge`) et
   l'identifiant de requête (`Request-Id`). Jamais de vue blanche.
4. **Lectures et mutations par le SDK** (clés de cache, préchargement, écritures, ADR
   0037) ; aucun `fetch` dans un écran.
5. **Couleur = sens, jamais seule** : un état a sa couleur, son icône **et** son libellé
   (`StatusBadge`) ; un environnement a sa pastille **et** son libellé (`EnvChip`) ; un
   différentiel a son signe (`+`, `−`, `~`) et son libellé.
6. **Clé et libellé** (REQ-020-21) : afficher le libellé dans la langue, la clé en
   infobulle et copiable ; libellé absent → la clé en chasse fixe, jamais une chaîne vide.
7. **Chasse fixe et chiffres tabulaires** pour adresses, préfixes, VID, MAC, codes,
   identifiants, compteurs et dates.

## 2. CSS

- Couches, dans cet ordre, déclarées une fois par `styles/layers.css` :
  `@layer reset, tokens, base, components, plugins, utilities`. Toute règle est dans une
  couche (règle `kartenn/known-layer`). Composants : `@layer components` ; plugins de
  première partie : `@layer plugins`, préfixe `k-<plugin>-`.
- Classes `k-<composant>` et `k-<composant>__<partie>` ; état en attribut ARIA ou
  `data-state` (pas de classe d'état) ; variante `data-variant` ; taille `data-size`.
- Propriétés logiques (`margin-inline-start`, `inline-size`), jamais physiques.
- Pas d'`!important`, pas d'identifiant, trois sélecteurs composés au plus.
- Plans : couches flottantes (menu, popover, liste, calendrier) et couches modales
  (dialogue, tiroir, feuille) sur `--k-z-modal`, la plus récente au-dessus (un menu dans
  un dialogue est sur lui) ; toasts sous elles (`--k-z-overlay`) ; infobulles au-dessus de
  tout (`--k-z-toast`).
- Ombres : `--k-shadow-overlay` et `--k-shadow-modal` dans les seules feuilles des
  composants flottants (liste `FLOATING` de `web/v2/stylelint.config.js`) ; ailleurs, un
  liseré intérieur `--k-shadow-inset-…` seulement.
- Une propriété personnalisée locale s'appelle `--k-<composant>-…` et vaut un jeton.
- Mouvement (020 § « Mouvement », politique et réalisation) : durées `--k-duration-*`,
  courbes `--k-ease-*`, amplitudes `--k-motion-*`, rotations `--k-turn-*` seulement ; sous
  mouvement réduit (préférence du système ou `data-motion="reduce"`), durées nulles sauf
  `fade`/`slow` et amplitudes nulles. `transform` (`translate`, `scale`, `rotate`),
  `opacity` et couleurs seulement ; opacité d'une couche en
  `max(var(--k-duration-medium), var(--k-duration-fade))`. Couche de Reka : entrée par
  `@starting-style`, sortie par `k-fade-out` (la `Presence` attend un `animationend`) ;
  focus placé sans attendre ; pas de `mode="out-in"`, pas de View Transitions, pas de
  minuterie. Jamais animés : tri, filtre, actualisation, valeurs, défilement, contenu d'un
  nœud, changement de page. Chevron de repli : `k-chevron` + `data-open`. Aucune animation
  en boucle hors indicateur de chargement ; le reflet du squelette (classe partagée
  `k-skeleton-fill`, jamais recopiée) tourne tant que la zone charge, arrêté par le
  mouvement réduit (système ou `data-motion="reduce"`, le mécanisme de WCAG 2.2.2).

### Structure avant style (spec 020 § « Structure avant style », 024/M12, M15)

La géométrie se règle avant la couleur : une maquette qui déborde ou se décale à une
largeur est fausse, même belle à 1280 px.
- **Pistes élastiques** : `minmax(0, 1fr)`, jamais `1fr` nu ; tout enfant de grille ou de
  flex qui porte un contenu long a `min-inline-size: 0`.
- **Contenu long** : il défile dans son emplacement (`overflow: auto`), jamais la page ;
  aucune page ne défile horizontalement de 1024 à 1920 px.
- **Tables** : en-tête et lignes dans une seule grille (`grid-template-columns` sur la
  grille, lignes en `subgrid`), en-tête collant dans le même conteneur de défilement ;
  jamais deux grilles qui « devraient » avoir les mêmes colonnes.
- **Surimpressions** : menus, sélecteurs, infobulles dans la couche `layers` du cadre
  (`providePortalTarget`) ; la surface en `isolation: isolate`, pour qu'un en-tête collant
  ne passe pas au-dessus du panneau droit ni d'un menu.
- **Barres d'outils** : `Toolbar` replie dans « … », jamais de retour à la ligne.
- **Messages** : une phrase va à la ligne, jamais d'ellipse sans moyen de lire le texte
  entier ; une liste longue dans un lieu étroit est une `ExpandableGrid`.
- **Cinq largeurs** : avant de rendre une maquette ou un écran, le voir à 1440, 1280,
  1024, 768 et 390 px, en clair et en sombre ; la capture le prouve ; rien ne défile de
  côté à 320 px hors d'une grille (`layout.spec`).
- **Adaptation** (spec 020 § « Adaptation à la largeur ») : le cadre seul lit la fenêtre
  (`@media` en syntaxe d'intervalle, seuils de `WIDTH_CLASSES` recopiés, `data-width`) ;
  un composant lit sa racine conteneur (`container-type: inline-size`, `@container` aux
  seuils de `CONTAINER_WIDTHS` seulement), jamais une `@media` de largeur ; `pointer` et
  `hover` en `@media` restent permis. Grilles : `priority` sur les colonnes secondaires
  (2 sous `md`, 3 sous `lg`), sélecteur « Colonnes » fourni par la grille ; tuiles en
  `Grid columns="auto"` ; rien de réservé au survol.
- **Droits** : une action interdite reste visible, `aria-disabled`, infobulle « Il faut
  `<permission>` » ; jamais d'encart permanent des droits.

## 3. CSP (ADR 0025, REQ-020-14)

- Aucun attribut `style` en chaîne (`style="…"`, `:style="'…'"`, gabarit), aucun `<style>`
  injecté, aucun `cssText`, aucun `setAttribute('style', …)`, aucun `v-html`.
- Valeur dynamique (position d'une couche, largeur de colonne, décalage d'une ligne
  virtualisée) : `el.style.setProperty('--k-…', v)` ou `:style` **en objet** dont les clés
  sont des jetons `--k-…` (CSSOM, permis par `style-src 'self'`).
- Reka UI : `ComboboxViewport`, `SelectViewport` et `Splitter*` injectent un `<style>` :
  interdits (lint).
  Tout autre composant pris en défaut par le parcours CSP rejoint la liste.

## 4. Inventaire (motif APG — à compléter au fil de P4)

Statut : ☐ à écrire, ◐ version visuelle, ● complet (états, clavier, captures).

| Composant | Motif APG ou rôle | Statut |
|---|---|---|
| `Icon` | `aria-hidden` par défaut ; nom obligatoire si seule porteuse de sens (`meaningful` + `label`, vérifié au typage) | ◐ |
| `Stack`, `Cluster`, `Grid`, `Split` | mise en page ; écart par `gap` (échelle), `Split` dans l'ordre de lecture | ◐ |
| `Button`, `IconButton` | Button ; occupé = `aria-disabled` + `aria-busy` (reste focalisable), bascule `aria-pressed`, raccourci `aria-keyshortcuts` ; `disabled` natif sort de l'ordre de tabulation : une action refusée avec motif prend `forbidden` ou `reason`, jamais `disabled` ; occupé ou refusé, un `submit` n'envoie rien ; `IconButton` : infobulle automatique de son nom et de son raccourci (`shortcut`), décorative, montée ouverte seulement ; refusé, son nom puis le motif, la description = le motif seul ; cibles ≥ 24 px (`sm` en compacte) | ● |
| `TextField`, `FormField` | champ natif étiqueté ; `FormField` : étiquette, erreur puis aide par `aria-describedby`, l'erreur dite dans une région `polite` présente vide avant elle ; l'emplacement reçoit `id`, `labelId`, `describedBy`, `invalid` ; `group` : groupe nommé et décrit (cases, bascules) ; `TextField` (sur `FormField`) : `aria-invalid`, en attente dit en mots (`aria-busy`), lecture seule, chasse fixe | ● |
| `Select`, `Combobox` | Select-Only Combobox (Reka : options focalisées ; Entrée, Espace, Bas, Haut ouvrent sur la choisie ; Échap sans choisir ; Tab choisit, ferme et passe au contrôle suivant ; `readonly` ne s'ouvre pas ; `inlineLabel` dans une barre) ; Combobox (`aria-autocomplete="list"`, sélection manuelle, `aria-activedescendant`, `aria-selected` sur la mise en avant, coche sur la valeur prise ; `suggest(texte, signal)` après 150 ms, annulée par un texte plus récent ; Alt+Bas ouvre sans bouger, Bas va à la première, Entrée prend la mise en avant sinon `submit`, Échap ferme et laisse le suivant à la couche, Origine/Fin dans le texte ; nombre, chargement, rien, échec dits par une région `polite` présente avant) ; liste commune `Select/options.css`, au moins aussi large que le champ | ● |
| `FilterChip`, `FilterPicker` | Button + Combobox en séquence dans un popover non modal (Reka) : la barre (`role="group"` nommé) tient les pastilles et « Ajouter un filtre » (`aria-haspopup="dialog"`, `aria-expanded` sur le déclencheur, pastille ou bouton ; le même une seconde fois ferme, un autre reprend le popover) ; dans le popover, défilant dans la fenêtre (1.4.10), « Annuler » puis « Appliquer » ; propriété dans sa liste fermée ouverte à l'ouverture (libellé puis clé ; clé seule en chasse fixe), puis l'opérateur (si plusieurs) ou la valeur ; valeur par liste fermée (`options` ou `suggest`, plusieurs pour `eq`) ou champ typé (`number`, `prefix`, `address`, `date` ; `within` prend un préfixe) refusé avant l'application, motif lié au champ ; deux Échap ; laissé à moitié, un filtre reste brouillon (hors URL), « Annuler » l'écarte ; focus rendu à la pastille appliquée ou au bouton ; appliqué et retiré dits par une région `polite` ; URL = paramètres de la route (`filtersToParams`, `filtersFromParams`, `checkFilter` ; valeur hors type relue et marquée invalide) | ● |
| `DateTimeField`, `PeriodPicker` | segments ISO 8601 (`AAAA-MM-JJ hh:mm`, `dir="ltr"`), chacun un Spinbutton (chiffres qui avancent, séparateurs, Haut/Bas qui bouclent, Origine/Fin, Gauche/Droite, Retour arrière ; Tab de segment en segment) dans un groupe nommé (`FormField` `group`) ; fuseau IANA et décalage écrits, décrivant l'heure ; valeur `AAAA-MM-JJ` ou instant RFC 3339 en UTC, `null` si vide, incomplète ou impossible ; impossible (31 février, 25 h, heure sautée) gardée, refusée au départ du champ ou par `check()`, motif lié et dit ; Date Picker Dialog : popover modal de Reka nommé par son bouton (Alt+Bas aussi), grille du mois nommée par son titre `aria-live`, flèches, Origine/Fin, Pg préc./suiv. (Maj : année), Entrée/Espace, Échap ; `PeriodPicker` : formulaire `valid_at`/`as_of`, raccourcis dits, refus à l'envoi (impossible, `as_of` futur) avec focus sur le champ, « Revenir au présent » hors présent ; la couche autour est à l'hôte ; pas de bibliothèque de dates (`Intl`) | ● |
| `Checkbox`, `Switch` | Checkbox (deux et trois états : modèle `true`, `false` ou `"mixed"` posé sur `indeterminate`, jamais `aria-checked` sur la case native ; mixte, elle se coche au basculement) ; Switch (`button` `role="switch"`, `aria-checked`, Espace et Entrée ; réglage pris aussitôt seulement, sinon `Checkbox`) ; étiquette englobante : la rangée est la cible (≥ 24 px, étiquette cachée comprise), l'aide hors du nom (`aria-describedby`) | ● |
| `Tabs` | Tabs (activation manuelle : flèches en boucle, Origine, Fin déplacent le focus ; Entrée, Espace activent ; un arrêt de tabulation, puis le panneau) | ● |
| `Menu` (bouton et menu) | Menu Button, Menu (un composant) ; Entrée, Espace, Bas ouvrent sur la première entrée, Haut sur la dernière ; flèches en boucle, Origine, Fin, saisie ; Échap rend le focus au bouton ; Tab, Maj+Tab ferment et passent au contrôle d'après ou d'avant le bouton ; raccourci par `ShortcutHint` (combinaison aussi en `aria-keyshortcuts`, séquence en description) ; entrée désactivée focalisable (`aria-disabled`), motif en description et infobulle ; entrée à cocher (`menuitemcheckbox`, menu gardé ouvert) ; entrées `radio` (`menuitemradio` en groupe, l'une cochée) | ● |
| `Dialog`, `AlertDialog`, `Drawer` | Dialog (Modal), Alert Dialog ; `Drawer` modal seulement ; `aria-modal` écrit, reste caché ; déclencheur dans `trigger` (`aria-haspopup="dialog"`, `aria-expanded`) ou bouton de l'hôte sur le modèle ; focus : `data-autofocus`, sinon premier champ du corps, sinon premier contrôle ; `AlertDialog` toujours sur « Annuler » (ignore `data-autofocus`) ; Tab en boucle ; Échap ; focus rendu à l'ouvreur, au bouton du menu si c'était une entrée (`internal/modal.ts`) ; `AlertDialog` : `cancel` (Échap, « Annuler » seulement, pas une fermeture par le modèle), `confirm`, pas de clic dehors | ● |
| `SidePanel` | `complementary` non modal + Tabs ; surimpression de la surface à toute largeur, jamais à côté (hôte positionné, surface `isolation: isolate` ; la surface garde sa largeur et ses colonnes), redimensionnable (Window Splitter maison) ; `cover` en classe étroite (toute la surface, sans poignée) ; `sheet` en classe compacte (toute la fenêtre, « Retour ») ; `rail` : replié, languette dans le flux (piste `auto` de l'hôte) dont le chevron « Ouvrir le panneau » le rouvre ; entre et sort depuis son bord (`<Transition>`, jamais au premier rendu) ; ce qu'il couvre est `inert` (`cover` : son conteneur ; `sheet` : Dialog (Modal), tout jusqu'à `[data-k-modal-scope]`) ; par-dessus la fin, publie `--k-side-panel-inset` (marge de défilement de l'hôte) | ● |
| `CommitForm`, `SplitButton` | champs étiquetés, Ctrl+Entrée ; Button + Menu Button (entrées désactivées avec motif) | ● |
| `Toolbar` | Toolbar ; actions repliées dans « … » par priorité ; sous 360 px, l'action principale seule ; entrées `more` toujours dans « … » (un seul « … » par barre) | ● |
| `MatrixGrid` | Grid ; en-têtes de lignes et de colonnes figés ; croix de survol ou de la cellule active (`data-cross`, en-têtes compris) ; `axes` : sens lignes ↓ colonnes → au coin et pointes d'accent sur les bordures de la croix (`data-flow` : fin de l'en-tête source, bas de l'en-tête destination, début et haut de la cellule montrée ; pseudo-éléments, retournées en RTL) (virtualisation au-delà de 50 × 50 à venir) | ● |
| `Toast` | `status` / `alert`, pause au survol et au focus ; maison : `createToasts` + `provideToasts`, une `ToastRegion` au cadre (`show`, `dismiss`) ; en bas au milieu, sous une couche modale (qui dit ses issues en elle, jamais par un toast) ; deux régions vivantes présentes vides, `aria-atomic="false"` ; erreur, avertissement, toast à action sans durée, les autres 8 s ; aucun toast ne prend le focus ; Tab les atteint (repère « Notifications ») ; survol, focus, page cachée, couche modale par-dessus les retiennent (2.2.1) ; Échap ou la croix ferment ; au clavier, focus au toast voisin sinon d'où il venait sinon au contenu principal, d'un clic à personne | ● |
| `Tooltip`, `Popover` | Tooltip (`useTooltip`, action refusée : `forbidden`, `reason` sur `Button`, `IconButton`, entrée de `Menu`) : focus clavier (`:focus-visible`) aussitôt, survol après 300 ms, survolable, fermé au départ du focus ou du pointeur (le pointeur qui part laisse ouverte celle du focus clavier), ou d'un défilement qui déplace l'ancre ; ouverte par `listeners(enabled, { pin, passEscape })`, Échap la ferme où que soit le focus (1.4.13) : sous le pointeur seul, rien d'autre ne reçoit la touche ; sous le focus, la couche autour reste (défaut empêché), sauf `passEscape` (infobulle du nom d'un `IconButton`) : la touche va ensuite au contrôle (son dialogue, sa grille) ; jamais sur un bouton dont le menu est ouvert (`aria-expanded`) ; un clic l'épingle si `pin`, sinon la ferme ; `Popover` : Dialog non modal (déclencheur `trigger`, nommé par son titre (un titre `h2`), sans description (corps structuré), focus sur lui ; déclencheur ouvert marqué ; Échap ou la croix rendent le focus ; Maj+Tab au début revient au déclencheur, Tab après la fin passe à ce qui le suit, sans sortir d'un dialogue modal ; borné à la fenêtre, corps qui défile = arrêt de tabulation) ; surface `k-popover` partagée (menu, `FilterPicker`) | ● |
| `Breadcrumb`, `SidebarNav` | Breadcrumb (`aria-current="page"` ; lien coupé : libellé entier en infobulle) ; Disclosure dans `navigation` | ◐ (`Breadcrumb` ●) |
| `CommandPalette` | Combobox dans un Dialog modal (liste `position="inline"`, sans Viewport) | ◐ |
| `DataGrid` | Grid (maison) ; une seule grille, lignes en `subgrid` (024) ; tri (`aria-sort`) et filtre de colonne (Alt+Bas, `filter`) ; navigation au clavier ; colonne de message `wrap` (va à la ligne) ; états de ligne `draft`, `review`, `removed` (barrée), ligne entrée dans un brouillon éclairée une fois (`data-fresh`) (virtualisation et édition à venir) ; racine conteneur, colonnes à `priority` masquées par `@container` et sélecteur « Colonnes » (`columnVisibility`), clé collante ; sélection Espace, Maj+Espace, Ctrl+A (`selectable` ou `"single"`) ; widgets d'une cellule hors tabulation (Entrée, F2, Échap) ; colonne jamais plus étroite que son en-tête ; erreur de cellule sur sa propre ligne | ◐ |
| `ExpandableGrid` | `DataGrid` compacte (compte de lignes, `visibleRows`, `compactColumns`) + `IconButton` « Agrandir » qui ouvre la même table dans un `Dialog` `lg` `contained` (focus piégé, Échap, focus rendu au bouton) ; tri et filtres de l'hôte, partagés | ● |
| `TreeGrid`, `Tree` | Treegrid ; Tree View ; garde d'affichage `misplaced` ; « + » d'un nœud (`addLabel`, événement `add`, 024/M10) ; clavier de la colonne d'arbre (Haut, Bas, Droite, Gauche, Entrée ou F2 vers le « + »), `expanded` en modèle | ◐ (`TreeGrid`) |
| `CodeBadge`, `KeyLabel`, `EnvChip`, `StatusBadge` | Button (copier, nom = contenu, « Copier » caché) ; — | ◐ |
| `Skeleton`, `EmptyState`, `ErrorState`, `InlineAlert` | `status` présent vide puis rempli, sans `aria-busy` (reflet en boucle `k-skeleton-fill`, fond fixe sous mouvement réduit) ; `alert` ; `InlineAlert` : `alert` pour une erreur, `status` sinon, contenu caché (`data-state="waiting"`, `visibility`) puis montré une image après sa région, donc annoncé, sans décalage ; `icon` remplace l'icône du ton ; le ton dit en mots, caché, avant le titre (« Avertissement : », 1.3.1) | ◐ (`InlineAlert` ●) |
| `DatePill`, `DraftCounter`, `TimeBand` | Button ; bascule du panneau (`aria-expanded`, `aria-controls` ; ouvert, icône en croix) et Menu Button des brouillons ; `status` | ◐ (`DraftCounter` ●) |
| `Kbd`, `ShortcutHint` | — ; `Kbd` : une combinaison, `kbd` imbriqués, noms de touches dans la langue (Maj, Origine, Suppr) ; `ShortcutHint` : une combinaison ou une séquence (`[["g"], ["p"]]`, « g puis p »), jamais une séquence dans un seul `Kbd` ; `aria-keyshortcuts` pour une combinaison seulement (`shortcutAria`) | ● |
| `DiffList` | liste groupée, Disclosure ; signe, mot caché, `del`/`ins` | ◐ |
| `DiffValue` | `ins`/`del` en ligne : signe attaché à sa valeur, mot caché ; `changed` avant → après | ◐ |
| `BarChart` | `role="img"` nommé par un résumé + table cachée ; SVG maison mesuré (`ResizeObserver`), infobulle décorative au survol ; l'image prend le focus, flèches de barre en barre (région `polite`) | ● |
| `BarList` | table (en-têtes de ligne, valeurs écrites), barres décoratives ; échelle `max` ou la plus grande valeur | ● |

Écrans de référence : les maquettes validées du catalogue (`/_ui/ecrans/E0` à `E10`,
design § « Maquettes », reprises par la 024) ; un écran de la 021 se compare à sa maquette, jamais à
l'ancien shell.

## 5. Raccourcis réservés (spec 021, § « Raccourcis »)

Ctrl+K (palette, partout) ; `/` (filtre de la vue) ; `?` (aide) ; `g` + lettre
(navigation : `d p r a e s c i b x h o`, et `v m t q f` des plugins) ; `c` (créer) ; `e`
(modifier) ; Ctrl+Entrée (appliquer ; action du bouton scindé) ; Échap (fermer,
annuler) ; `[` (barre latérale) ; `]` (panneau droit) ;
Alt+Flèche bas sur un en-tête de grille (filtre de colonne, convention maison, dite dans
l'aide). Un composant qui consomme les caractères (recherche à la saisie de l'arbre)
porte `data-k-typeahead`. Les raccourcis à une touche se désactivent (WCAG 2.1.4) ; ils
sont montrés par `Kbd` (une combinaison) ou `ShortcutHint` (une séquence) dans les menus,
les infobulles et l'aide, lus au registre du SDK.

## 6. Pièges de Reka UI (prototype, `docs/spikes/005-framework-web.md`)

Version exacte épinglée ; toute montée repasse les scripts clavier et le parcours CSP.
- Sélection de l'arbre : Reka stocke une **copie** ; sélection et opérations par
  **identifiant**, nœuds `markRaw`, listes en `shallowRef`.
- Arbre virtualisé : Haut/Bas visent le voisin **rendu** (appuis perdus) ; Haut et Bas
  gérés par `@kartenn/ui` sur l'index aplati.
- Pas de `*`, Entrée sélectionne, tampon de recherche pollué par les touches non
  imprimables : gestionnaire placé avant ceux de Reka (`stopImmediatePropagation`).
- `ComboboxViewport`, `SelectViewport`, `Splitter` : `<style>` injecté (CSP) ; interdits.
- Combobox ouverte : `aria-hidden` sur le reste de la page ; filtres en popover, deux Échap.
  Select ouvert : de même (modal, focus dans la liste) ; un test lit alors le bouton par
  sa classe, pas par son rôle.
- Combobox : Reka met en avant une suggestion de lui-même (la première à la saisie, la
  choisie à l'ouverture) ; la sélection manuelle l'efface après ses microtâches
  (`pointerleave` sur la racine, `setTimeout` 0) ; Bas et Alt+Bas, Origine et Fin passent
  par un gestionnaire en capture avant les siens ; `highlightFirstItem` (exposé, non
  documenté) met la première en avant ; Reka lie `aria-selected` à la valeur prise, par-dessus
  toute prop : il est réécrit sur les options d'après `data-highlighted`. Select : Reka garde
  Tab dans la liste (`preventDefault`) ; un gestionnaire en capture choisit, ferme et pose
  le focus sur le contrôle suivant. Le `--reka-*-trigger-width` est illisible
  (jetons seulement) : la largeur du champ est posée par CSSOM (`--k-list-anchor-width`).
- Une feuille `.css` importée depuis un composant crée un fragment vide que le build du
  shell (`kartenn-shared-modules`) ne trouve pas : une feuille partagée va dans `style.css`.
- Popover (`FilterPicker`) : l'auto-focus d'ouverture et de fermeture d'un `FocusScope`
  est un événement qui remonte (celui d'un `Select` ou d'une liste à l'intérieur compris)
  et qui vient deux fois à la fermeture : ne traiter que celui du popover (sa cible est
  l'enveloppe qui le contient), une fois. Sans `PopoverTrigger`, le focus se rend à la
  main ; un clic sur la barre elle-même (`interactOutside` annulé) ne ferme pas.
- `PopoverContent` reçoit `aria-labelledby` = son `PopoverTrigger`, par-dessus celui qu'on
  lui pose : un popover à déclencheur est nommé par son bouton (`DateTimeField`), sauf à le
  réécrire sur le dialogue à l'ouverture (`Popover`, nommé par son titre) ; le modal ne pose
  pas `aria-modal`, à écrire. Son `FocusScope` a `loop` et entend Tab sur le dialogue avant
  nos écouteurs : un Tab qui sort se traite en capture. Non modal, un menu ou un popover
  laisse Tab aller à la fin de la page (la couche est au portail) : Tab et Maj+Tab ferment
  et passent à côté du déclencheur (`internal/tabOrder.ts` : borné au dialogue qui contient
  le déclencheur, en boucle ; sans les gardes de focus `[data-reka-focus-guard]` que Reka
  pose aux bouts de la page). Une grille dont la piste `1fr` n'a qu'une hauteur maximale
  ne rétrécit pas : un corps qui doit défiler sous un `max-block-size` est en colonne
  flexible.
- Dialog modal : Reka rend le focus à l'élément actif au montage, sans repli quand il a
  disparu (entrée d'un menu) : `internal/modal.ts` (`useOpener`). Le `Toast` de Reka n'est
  pas pris : tout Échap de la page ferme tous ses toasts (`onKeyStroke` global).
- `SelectValue` ne montre que le libellé d'une option montée avant la valeur : un `Select`
  dont les options arrivent avec sa valeur se remonte (`:key`).
- Une couche qui s'efface (liste, `Select`) reste la couche du dessus jusqu'à la fin de son
  fondu : un Échap aussitôt est pour elle ; un script clavier attend qu'elle soit partie.
- `defineModel` tenu par l'hôte : la valeur relue juste après l'écriture est l'ancienne
  jusqu'au rendu de l'hôte ; un index se calcule sur la liste écrite, pas relue.
- Slots de `TreeVirtualizer` typés `Record<string, any>` : enveloppes typées dans
  `@kartenn/ui`, aucun transtypage dans les écrans.
- Ligne focalisée démontée par la molette : la garder montée.
- Nom accessible d'une ligne = tout son contenu : `aria-label` court, détail par
  `aria-describedby`.
- `DropdownMenuItem` écrit `aria-disabled` d'après sa prop `disabled`, qui sort aussi
  l'entrée des flèches : une entrée refusée garde `disabled` faux, `aria-disabled` posé par
  la directive de `Menu`, et `select` annulé.
- Tests au clavier : pas d'horloge figée (`page.clock.setFixedTime`) ; Vue ignore un
  événement daté de l'instant où le gestionnaire d'un ancêtre a été posé, et une horloge
  figée rend tous les instants égaux (`e2e/visual/clavier.spec.ts`).

## 7. Clichés à éviter (brief)

Cartes arrondies identiques à ombre grise ; dégradés décoratifs ; icônes colorées sans
sens ; animation d'entrée des listes ; gros titres creux ; couleur sans libellé. Aussi :
marges généreuses de vitrine (l'outil est dense : lignes de 28 px), texte gris trop pâle,
boutons primaires multiples dans une même zone.

## 8. Outillage

- Après chaque édition d'un fichier `web/**`, le hook `.claude/hooks/web-apres-edition.sh`
  passe Prettier, Stylelint et ESLint sur ce fichier ; un échec revient en diagnostic.
- Vérification : `k-web2-check` (Prettier, ESLint, Stylelint, vue-tsc, Vitest, captures) ;
  `k-web2-e2e` (parcours) ; références des captures : `k-web-shots --update`, dans devenv
  seulement, relues dans le diff avant commit.
- Rendu : catalogue `/_ui` (`?mode=light|dark`, `?densite=compacte|confortable`,
  `?langue=fr|en`) ; MCP `playwright`, `browser_snapshot` d'abord.
- Catalogue : un état = `{ name, description, props }` dans `<Nom>.catalogue.ts`, plus
  `slot` (texte), `demo` (composition), `interaction` (`hover`, `focus`, `press` joués
  avant la capture), `wide` (pleine largeur), `isolated` (couche modale : montrée seule,
  par `?etat=`, et vérifiée par axe sur sa page). Les couches flottantes vont dans
  `providePortalTarget` (le catalogue les garde dans `main`) ; les liens passent par
  `provideLink` (le routeur du shell). Captures : un état par mode, plus la page entière
  en densité confortable ; un survol attend le délai d'une infobulle avant la capture.
- Scripts clavier des composants de P4 : `web/v2/e2e/components/<composant>.spec.ts`
  (projet `visual`, donc `k-web2-check`), sur un état du catalogue ouvert par `openState`
  (horloge réelle) ; l'état `keyboard` d'un composant rassemble ce que le script joue.
- Relectures : sous-agents `ui-reviewer` (maquette, brief, doublons) et `a11y-auditor`
  (motif APG, WCAG 2.2 AA) à la fin de chaque composant, maquette ou écran.
